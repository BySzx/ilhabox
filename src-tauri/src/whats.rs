//! WhatsApp: reads the toasts Chrome/Edge (WhatsApp Web) or WhatsApp Desktop hand to Windows,
//! straight from the local notification store, read-only. Nothing is sent anywhere.

use base64::{engine::general_purpose::STANDARD, Engine};
use rusqlite::{params, types::ValueRef, Connection, OpenFlags};
use serde::Serialize;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{fs, thread};
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, IsIconic, IsWindowVisible, SetForegroundWindow, ShowWindow, SW_RESTORE,
};

const EPOCH_DIFF_100NS: i64 = 116_444_736_000_000_000;
const HISTORY_MS: i64 = 24 * 3600 * 1000;
const MAX_AVATAR: u64 = 1024 * 1024;
const MAX_LIST: usize = 120;

#[derive(Serialize, Clone)]
pub struct Msg {
    id: i64,
    at: i64,
    chat: String,
    text: String,
    avatar: Option<String>,
}

fn db_path() -> Option<PathBuf> {
    dirs::data_local_dir().map(|d| d.join("Microsoft").join("Windows").join("Notifications").join("wpndatabase.db"))
}

fn now_ms() -> i64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as i64).unwrap_or(0)
}

fn stamp(p: &PathBuf) -> Option<SystemTime> {
    let wal = p.with_extension("db-wal");
    let a = fs::metadata(p).and_then(|m| m.modified()).ok();
    let b = fs::metadata(wal).and_then(|m| m.modified()).ok();
    a.max(b)
}

fn open(p: &PathBuf) -> rusqlite::Result<Connection> {
    let c = Connection::open_with_flags(p, OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX)?;
    c.busy_timeout(Duration::from_millis(250))?;
    Ok(c)
}

fn unescape(s: &str) -> String {
    s.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&#39;", "'")
        .replace("&amp;", "&")
}

fn attr<'a>(tag: &'a str, name: &str) -> Option<&'a str> {
    let key = format!("{name}=\"");
    let i = tag.find(&key)? + key.len();
    let end = tag[i..].find('"')?;
    Some(&tag[i..i + end])
}

fn texts(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<text") {
        let after = &rest[i..];
        let Some(gt) = after.find('>') else { break };
        let tag = &after[..gt];
        if tag.ends_with('/') {
            rest = &after[gt + 1..];
            continue;
        }
        let body = &after[gt + 1..];
        let Some(end) = body.find("</text>") else { break };
        if attr(tag, "placement") != Some("attribution") {
            let t = unescape(body[..end].trim());
            if !t.is_empty() {
                out.push(t);
            }
        }
        rest = &body[end + 7..];
    }
    out
}

/// Decodes %XX escapes (accents, spaces) in file URIs.
fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// Turns the many forms a toast image can take into a local path.
/// Store apps (WhatsApp Desktop) use ms-appdata:///local|temp|roaming/ inside their package folder.
fn image_path(src: &str, handler: &str) -> Option<PathBuf> {
    let src = percent_decode(src.trim());
    let lower = src.to_lowercase();
    if let Some(p) = lower.strip_prefix("file:///").map(|_| &src[8..]) {
        return Some(PathBuf::from(p.replace('/', "\\")));
    }
    if let Some(rest) = lower.strip_prefix("ms-appdata:///").map(|_| &src[14..]) {
        let family = handler.split('!').next().filter(|f| f.contains('_'))?;
        let (kind, file) = rest.split_once('/')?;
        let folder = match kind.to_lowercase().as_str() {
            "local" => "LocalState",
            "temp" => "TempState",
            "roaming" => "RoamingState",
            _ => return None,
        };
        return Some(dirs::data_local_dir()?.join("Packages").join(family).join(folder).join(file.replace('/', "\\")));
    }
    if src.len() > 2 && src.as_bytes()[1] == b':' {
        return Some(PathBuf::from(src));
    }
    None
}

fn mime(bytes: &[u8]) -> &'static str {
    match bytes {
        [0xFF, 0xD8, ..] => "image/jpeg",
        [0x89, b'P', b'N', b'G', ..] => "image/png",
        [b'R', b'I', b'F', b'F', _, _, _, _, b'W', b'E', b'B', b'P', ..] => "image/webp",
        [b'G', b'I', b'F', ..] => "image/gif",
        _ => "image/png",
    }
}

/// Every `<image src>` in a toast, raw (for diagnostics).
fn image_srcs(xml: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find("<image") {
        let after = &rest[i..];
        let Some(gt) = after.find('>') else { break };
        if let Some(src) = attr(&after[..gt], "src") {
            out.push(unescape(src));
        }
        rest = &after[gt..];
    }
    out
}

fn avatar(xml: &str, handler: &str) -> Option<String> {
    for src in image_srcs(xml) {
        if src.starts_with("http://") || src.starts_with("https://") || src.starts_with("data:image/") {
            return Some(src);
        }
        let Some(path) = image_path(&src, handler) else { continue };
        let Ok(meta) = fs::metadata(&path) else { continue };
        if meta.len() == 0 || meta.len() > MAX_AVATAR {
            continue;
        }
        let Ok(bytes) = fs::read(&path) else { continue };
        return Some(format!("data:{};base64,{}", mime(&bytes), STANDARD.encode(&bytes)));
    }
    None
}

fn is_whatsapp(xml: &str, handler: &str) -> bool {
    xml.contains("web.whatsapp.com") || handler.to_lowercase().contains("whatsapp")
}

fn strip_unread(line: &str) -> String {
    let l = line.trim().trim_start_matches(['✉', '📩', '\u{fe0f}']).trim();
    let lower = l.to_lowercase();
    let digits = lower.trim_start_matches(|c: char| c.is_ascii_digit()).trim_start();
    let summary = digits.len() < lower.len()
        && ["mensagens não lidas", "mensagem não lida", "novas mensagens", "nova mensagem", "unread messages", "unread message", "new messages", "new message"]
            .iter()
            .any(|k| digits.starts_with(k));
    if summary { String::new() } else { line.trim().to_string() }
}

fn parse(id: i64, arrival: i64, xml: &str, handler: &str) -> Option<Msg> {
    let t = texts(xml);
    let chat = t.first()?.clone();
    // WhatsApp Desktop appends an unread summary ("✉ 2 mensagens não lidas") as extra lines.
    let text = t[1..].iter().map(|l| strip_unread(l)).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n");
    let at = if arrival > 0 { (arrival - EPOCH_DIFF_100NS) / 10_000 } else { now_ms() };
    Some(Msg { id, at, chat, text, avatar: avatar(xml, handler) })
}

fn fetch(c: &Connection, after: i64) -> rusqlite::Result<(Vec<Msg>, i64)> {
    let mut st = c.prepare_cached(
        "SELECT n.Id, n.ArrivalTime, n.Payload, IFNULL(h.PrimaryId, '')
         FROM Notification n LEFT JOIN NotificationHandler h ON h.RecordId = n.HandlerId
         WHERE n.Id > ?1 AND n.Type = 'toast' ORDER BY n.Id LIMIT 300",
    )?;
    let mut rows = st.query(params![after])?;
    let mut out = Vec::new();
    let mut last = after;
    while let Some(r) = rows.next()? {
        let id: i64 = r.get(0)?;
        last = last.max(id);
        let arrival: i64 = r.get(1).unwrap_or(0);
        let xml = match r.get_ref(2)? {
            ValueRef::Text(b) | ValueRef::Blob(b) => String::from_utf8_lossy(b).into_owned(),
            _ => continue,
        };
        let handler: String = r.get(3).unwrap_or_default();
        if is_whatsapp(&xml, &handler) {
            if let Some(m) = parse(id, arrival, &xml, &handler) {
                out.push(m);
            }
        }
    }
    Ok((out, last))
}

pub fn spawn() {
    thread::Builder::new()
        .name("whats".into())
        .spawn(move || {
            let Some(path) = db_path() else { return };
            let mut conn: Option<Connection> = None;
            let mut last_id = -1i64;
            let mut seen: Option<SystemTime> = None;
            let mut list: Vec<Msg> = Vec::new();
            loop {
                thread::sleep(Duration::from_millis(1200));
                let s = stamp(&path);
                if s.is_some() && s == seen && last_id >= 0 {
                    continue;
                }
                if conn.is_none() {
                    conn = open(&path).ok();
                }
                let Some(c) = &conn else { continue };
                if last_id < 0 {
                    match fetch(c, 0) {
                        Ok((all, last)) => {
                            let cutoff = now_ms() - HISTORY_MS;
                            list = all.into_iter().filter(|m| m.at >= cutoff).collect();
                            let skip = list.len().saturating_sub(MAX_LIST);
                            list.drain(..skip);
                            crate::bus::send("whats-history", &list, true);
                            last_id = last;
                            seen = s;
                        }
                        Err(_) => conn = None,
                    }
                    continue;
                }
                match fetch(c, last_id) {
                    Ok((news, last)) => {
                        for m in news {
                            crate::bus::send("whats", &m, false);
                            list.push(m);
                        }
                        let skip = list.len().saturating_sub(MAX_LIST);
                        list.drain(..skip);
                        crate::bus::keep("whats-history", &list);
                        last_id = last;
                        seen = s;
                    }
                    Err(_) => conn = None,
                }
            }
        })
        .expect("whats thread");
}

unsafe extern "system" fn find(hwnd: HWND, lp: LPARAM) -> BOOL {
    if IsWindowVisible(hwnd).as_bool() {
        let mut buf = [0u16; 256];
        let n = GetWindowTextW(hwnd, &mut buf) as usize;
        if String::from_utf16_lossy(&buf[..n]).contains("WhatsApp") {
            *(lp.0 as *mut HWND) = hwnd;
            return BOOL(0);
        }
    }
    BOOL(1)
}

/// Brings the WhatsApp window (installed PWA, desktop app or the browser tab in front) forward.
pub fn focus() -> bool {
    let mut found = HWND::default();
    unsafe {
        let _ = EnumWindows(Some(find), LPARAM(&mut found as *mut HWND as isize));
        if found.is_invalid() {
            return false;
        }
        if IsIconic(found).as_bool() {
            let _ = ShowWindow(found, SW_RESTORE);
        }
        SetForegroundWindow(found).as_bool()
    }
}
