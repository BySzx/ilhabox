//! Watches the PC once a second and turns what it sees into suggestions on the phone: a Movidesk
//! ticket opened in the browser, a long time with WhatsApp in front, nothing being timed, one
//! period running for hours, the end of the shift. Locking the PC (Win+L) pauses the running
//! activity and dims the phone; unlocking brings both back.

use crate::{bus, config, diag, work};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::System::StationsAndDesktops::{CloseDesktop, OpenInputDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::core::w;
use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::RemoteDesktop::{WTSRegisterSessionNotification, NOTIFY_FOR_THIS_SESSION};
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::VARIANT;
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationValuePattern, TreeScope_Descendants, UIA_ControlTypePropertyId, UIA_DocumentControlTypeId,
    UIA_EditControlTypeId, UIA_NamePropertyId, UIA_TextControlTypeId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetForegroundWindow, GetMessageW, GetWindowTextW, GetWindowThreadProcessId, RegisterClassW, TranslateMessage, MSG,
    WINDOW_EX_STYLE, WNDCLASSW, WS_POPUP,
};

const DESKTOP_SWITCHDESKTOP: u32 = 0x0100;
const WM_QUERYENDSESSION: u32 = 0x0011;
const WM_POWERBROADCAST: u32 = 0x0218;
const WM_WTSSESSION_CHANGE: u32 = 0x02B1;
const WTS_SESSION_LOCK: usize = 7;
const WTS_SESSION_UNLOCK: usize = 8;
const PBT_APMSUSPEND: usize = 4;
const PBT_APMRESUMESUSPEND: usize = 7;
const PBT_APMRESUMEAUTOMATIC: usize = 0x12;

/// Lock state from Windows' own session events (Win+L, lock screen after sleep).
static LOCKED: AtomicBool = AtomicBool::new(false);
static EVENTS: AtomicBool = AtomicBool::new(false);

unsafe extern "system" fn session_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_WTSSESSION_CHANGE => {
            match wp.0 {
                WTS_SESSION_LOCK => LOCKED.store(true, Ordering::SeqCst),
                WTS_SESSION_UNLOCK => LOCKED.store(false, Ordering::SeqCst),
                _ => {}
            }
            LRESULT(0)
        }
        // Windows is shutting down or logging off: nothing keeps counting overnight.
        WM_QUERYENDSESSION => {
            let paused = work::pause_running();
            diag::log(&format!("windows desligando; pausado={paused:?}"));
            LRESULT(1)
        }
        WM_POWERBROADCAST => {
            match wp.0 {
                PBT_APMSUSPEND => LOCKED.store(true, Ordering::SeqCst),
                // Waking up without a password shows no lock screen, so no unlock event: ask the desktop.
                PBT_APMRESUMESUSPEND | PBT_APMRESUMEAUTOMATIC => LOCKED.store(locked_poll(), Ordering::SeqCst),
                _ => {}
            }
            LRESULT(1)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// Hidden window that receives lock/unlock, sleep and shutdown from Windows.
fn session_events() {
    thread::spawn(|| unsafe {
        let Ok(inst) = GetModuleHandleW(None) else { return };
        let class = w!("IlhaBoxSessao");
        let wc = WNDCLASSW { lpfnWndProc: Some(session_proc), hInstance: inst.into(), lpszClassName: class, ..Default::default() };
        RegisterClassW(&wc);
        let Ok(hwnd) = CreateWindowExW(WINDOW_EX_STYLE(0), class, w!("IlhaBox"), WS_POPUP, 0, 0, 0, 0, None, None, Some(inst.into()), None) else {
            diag::log("sessao: janela oculta nao criada");
            return;
        };
        match WTSRegisterSessionNotification(hwnd, NOTIFY_FOR_THIS_SESSION) {
            Ok(()) => EVENTS.store(true, Ordering::SeqCst),
            Err(e) => diag::log(&format!("sessao: sem avisos de bloqueio ({})", e.message())),
        }
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    });
}

fn locked() -> bool {
    if EVENTS.load(Ordering::SeqCst) { LOCKED.load(Ordering::SeqCst) } else { locked_poll() }
}
static DISMISSED: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

fn locked_poll() -> bool {
    unsafe {
        match OpenInputDesktop(DESKTOP_CONTROL_FLAGS(0), false, DESKTOP_ACCESS_FLAGS(DESKTOP_SWITCHDESKTOP)) {
            Ok(d) => {
                let _ = CloseDesktop(d);
                false
            }
            Err(_) => true,
        }
    }
}

const BROWSERS: [&str; 6] = ["Brave", "Google Chrome", "Microsoft\u{200b} Edge", "Microsoft Edge", "Opera", "Vivaldi"];

fn is_browser(title: &str) -> bool {
    BROWSERS.iter().any(|b| title.ends_with(b))
}

/// Address-bar text of a Chromium browser window, through UI Automation (the window title only
/// has the page title; Movidesk puts the ticket number in the address).
fn browser_url(ua: &IUIAutomation, hwnd: HWND) -> Option<String> {
    unsafe {
        let root = ua.ElementFromHandle(hwnd).ok()?;
        let cond = ua.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(UIA_EditControlTypeId.0)).ok()?;
        let edit = root.FindFirst(TreeScope_Descendants, &cond).ok()?;
        let value: IUIAutomationValuePattern = edit.GetCurrentPatternAs(UIA_ValuePatternId).ok()?;
        let url = value.CurrentValue().ok()?.to_string();
        (url.contains('.') && !url.contains(' ')).then_some(url)
    }
}

/// Texts of the page shown in a browser window, in reading order (names fetched in one batch).
fn page_texts(ua: &IUIAutomation, hwnd: HWND) -> Option<Vec<String>> {
    unsafe {
        let root = ua.ElementFromHandle(hwnd).ok()?;
        let doc_cond = ua.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(UIA_DocumentControlTypeId.0)).ok()?;
        let doc = root.FindFirst(TreeScope_Descendants, &doc_cond).ok()?;
        let text_cond = ua.CreatePropertyCondition(UIA_ControlTypePropertyId, &VARIANT::from(UIA_TextControlTypeId.0)).ok()?;
        let cache = ua.CreateCacheRequest().ok()?;
        cache.AddProperty(UIA_NamePropertyId).ok()?;
        let all = doc.FindAllBuildCache(TreeScope_Descendants, &text_cond, &cache).ok()?;
        let n = all.Length().ok()?.min(6000);
        Some((0..n).filter_map(|i| all.GetElement(i).ok()?.CachedName().ok().map(|b| b.to_string().trim().to_string())).filter(|t| !t.is_empty()).collect())
    }
}

/// Subject of the open Movidesk ticket: the line right above "Ticket aberto via … em …".
pub fn ticket_subject(texts: &[String]) -> Option<String> {
    let at = texts.iter().position(|t| t.starts_with("Ticket aberto") || t.contains(" aberto via "))?;
    texts[..at].iter().rev().find(|t| t.chars().count() > 3 && t.chars().count() < 200 && !t.chars().all(|c| c.is_ascii_digit())).cloned()
}

/// Subject of the ticket in the browser window in front (slow: reads the whole page).
pub fn foreground_subject() -> Option<String> {
    let ua = automation()?;
    page_texts(&ua, unsafe { GetForegroundWindow() }).as_deref().and_then(ticket_subject)
}

fn automation() -> Option<IUIAutomation> {
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED).ok();
        CoCreateInstance(&CUIAutomation, None, CLSCTX_INPROC_SERVER).ok()
    }
}

/// Ticket number in a Movidesk address, e.g. "empresa.movidesk.com/Ticket/EditByProtocol/202601010001".
pub fn parse_ticket_url(url: &str) -> Option<String> {
    let low = url.to_lowercase();
    if !low.contains("movidesk") {
        return None;
    }
    let path = &url[low.find("/ticket")?..];
    path.split(|c: char| !c.is_ascii_digit()).rfind(|d| (4..=14).contains(&d.len())).map(str::to_string)
}

/// Title and (for a browser) address of the window in front.
pub fn foreground() -> (String, Option<String>) {
    let title = foreground_title();
    let url = if is_browser(&title) { automation().and_then(|ua| browser_url(&ua, unsafe { GetForegroundWindow() })) } else { None };
    (title, url)
}

/// The window in front belongs to the installed WhatsApp app (not WhatsApp Web in a browser).
fn whatsapp_app_in_front(title: &str) -> bool {
    let exe = unsafe {
        let hwnd = GetForegroundWindow();
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
        let Ok(h) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else { return false };
        let mut buf = [0u16; 520];
        let mut len = buf.len() as u32;
        let ok = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).is_ok();
        let _ = CloseHandle(h);
        if !ok {
            return false;
        }
        String::from_utf16_lossy(&buf[..len as usize]).to_lowercase()
    };
    let name = exe.rsplit('\\').next().unwrap_or(&exe);
    name.contains("whatsapp") || (name == "applicationframehost.exe" && title.contains("WhatsApp"))
}

pub fn foreground_title() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut buf) as usize;
        String::from_utf16_lossy(&buf[..n])
    }
}

/// (minutes since midnight, weekday 0 = Sunday, day of the month)
fn local_clock() -> (u32, u16, u16) {
    let t = unsafe { GetLocalTime() };
    (t.wHour as u32 * 60 + t.wMinute as u32, t.wDayOfWeek, t.wDay)
}

/// Epoch ms of today at `minutes` past local midnight.
fn today_at(minutes: u32) -> i64 {
    let t = unsafe { GetLocalTime() };
    let since_midnight = (t.wHour as i64 * 3600 + t.wMinute as i64 * 60 + t.wSecond as i64) * 1000 + t.wMilliseconds as i64;
    crate::pc::now_ms() - since_midnight + minutes as i64 * 60_000
}

fn minutes(hhmm: &str) -> Option<u32> {
    let (h, m) = hhmm.split_once(':')?;
    Some(h.trim().parse::<u32>().ok()? * 60 + m.trim().parse::<u32>().ok()?)
}

/// Ticket number and subject from a Movidesk window title, e.g.
/// "Ticket #48230 - Erro ao emitir NF-e - Movidesk - Google Chrome" → ("48230", "Erro ao emitir NF-e").
pub fn parse_ticket(title: &str, word: &str) -> Option<(String, String)> {
    if word.is_empty() || !title.to_lowercase().contains(&word.to_lowercase()) {
        return None;
    }
    let chars: Vec<char> = title.chars().collect();
    let mut best: Option<(usize, usize, bool)> = None;
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_ascii_digit() {
            let start = i;
            while i < chars.len() && chars[i].is_ascii_digit() {
                i += 1;
            }
            let len = i - start;
            let hashed = start > 0 && chars[start - 1] == '#';
            if (4..=14).contains(&len) && best.is_none_or(|b| hashed && !b.2) {
                best = Some((start, i, hashed));
            }
        } else {
            i += 1;
        }
    }
    let (a, b, _) = best?;
    let number: String = chars[a..b].iter().collect();
    let noise = ["google chrome", "microsoft edge", "mozilla firefox", "opera", "brave", "ticket", "movidesk", word];
    let mut parts = title.to_string();
    for sep in [" - ", " | ", " — ", " – ", " · "] {
        parts = parts.replace(sep, "\u{1}");
    }
    let subject = parts
        .split('\u{1}')
        .map(|p| p.replace(&format!("#{number}"), "").replace(&number, "").trim().trim_matches(['#', ':']).trim().to_string())
        .find(|p| p.chars().count() > 2 && !noise.iter().any(|n| p.to_lowercase() == n.to_lowercase()))
        .unwrap_or_default();
    Some((number, subject))
}

fn suggest(id: &str, text: &str, detail: &str, actions: Value) {
    if DISMISSED.lock().as_ref().and_then(|m| m.get(id)).is_some_and(|t| t.elapsed() < Duration::from_secs(30 * 60)) {
        return;
    }
    if bus::get("sugestao").and_then(|v| v["id"].as_str().map(str::to_string)).as_deref() == Some(id) {
        return;
    }
    diag::log(&format!("sugestao {id}: {text}"));
    bus::send("sugestao", json!({ "id": id, "text": text, "detail": detail, "actions": actions, "at": crate::pc::now_ms() }), true);
}

/// The phone closed a suggestion: it stays quiet for 30 min.
pub fn dismiss(id: &str) -> Result<Value, String> {
    DISMISSED.lock().get_or_insert_with(HashMap::new).insert(id.to_string(), Instant::now());
    if bus::get("sugestao").and_then(|v| v["id"].as_str().map(str::to_string)).as_deref() == Some(id) {
        bus::send("sugestao", Value::Null, true);
    }
    Ok(Value::Null)
}

fn clear(prefix: &str) {
    if bus::get("sugestao").and_then(|v| v["id"].as_str().map(str::to_string)).is_some_and(|i| i.starts_with(prefix)) {
        bus::send("sugestao", Value::Null, true);
    }
}

fn tag(e: &work::Entry) -> String {
    if e.tipo.is_empty() || e.tipo == "Ticket" { format!("#{}", if e.ticket.is_empty() { "—" } else { &e.ticket }) } else { e.tipo.clone() }
}

pub fn spawn(dir: PathBuf) {
    bus::keep("pc", json!({ "locked": false }));
    session_events();
    let alive = dir.join("vivo.txt");
    // Something left running while the app was closed or the PC was off: cut it at the last minute
    // the PC was seen in use, and say so.
    let last_seen: Option<i64> = std::fs::read_to_string(&alive).ok().and_then(|t| t.trim().parse().ok());
    if let Some(seen) = last_seen.filter(|t| crate::pc::now_ms() - t > 15 * 60_000) {
        if let Some((e, cut)) = work::cut_running(seen) {
            let hm = |ms: i64| {
                let m = ((ms - today_at(0)).rem_euclid(86_400_000)) / 60_000;
                format!("{:02}:{:02}", m / 60, m % 60)
            };
            diag::log(&format!("abertura: {} cortado em {cut}", e.id));
            bus::keep(
                "sugestao",
                json!({ "id": "cortado", "text": format!("{} ficou rodando com a IlhaBox fechada", tag(&e)), "detail": format!("Parei às {}, o último momento em que o PC estava em uso. Confira em Horas.", hm(cut)),
                        "actions": [{ "label": "Ver em Horas", "go": "horas" }, { "label": "Ok" }], "at": crate::pc::now_ms() }),
            );
        }
    }
    thread::spawn(move || {
        let ua = automation();
        let mut url: Option<String> = None;
        let mut url_at = Instant::now();
        let mut subjects: HashMap<String, String> = HashMap::new();
        let mut was_locked = false;
        let mut locked_at = Instant::now();
        let mut paused_by_lock: Option<u64> = None;
        let mut title_since: (String, Instant) = (String::new(), Instant::now());
        let mut whats_since: Option<Instant> = None;
        let mut idle_since = Instant::now();
        let mut last_idle_note: Option<Instant> = None;
        let mut long_noted: Option<(u64, i64)> = None;
        let mut closing_noted: Option<u16> = None;
        let mut shift_end_noted: Option<u16> = None;
        let mut morning_noted: Option<u16> = None;
        let mut beat = Instant::now() - Duration::from_secs(120);
        loop {
            thread::sleep(Duration::from_secs(1));
            let cfg = config::get();
            let now_locked = locked();

            if now_locked != was_locked {
                was_locked = now_locked;
                bus::send("pc", json!({ "locked": now_locked }), true);
                if now_locked {
                    locked_at = Instant::now();
                    paused_by_lock = if cfg.pausar_ao_travar { work::pause_running() } else { None };
                    diag::log(&format!("pc travado; pausado={paused_by_lock:?}"));
                } else {
                    let away = locked_at.elapsed();
                    idle_since = Instant::now();
                    if let Some(id) = paused_by_lock.take() {
                        let entry = work::snapshot().into_iter().find(|e| e.id == id);
                        if let Some(e) = entry.filter(|e| !e.done && !e.running()) {
                            // Bathroom, coffee: back within the short-pause limit just carries on, no question.
                            if away <= Duration::from_secs(cfg.juntar_pausa_min.max(2) as u64 * 60) {
                                let _ = work::resume(&json!({ "id": id }));
                            } else {
                                suggest(
                                    "voltou",
                                    &format!("Você voltou ({} min fora)", away.as_secs() / 60),
                                    &format!("{} ficou pausado enquanto o PC estava travado", tag(&e)),
                                    json!([
                                        { "label": format!("Continuar {}", tag(&e)), "cmd": "work_resume", "args": { "id": id } },
                                        { "label": "Era outra coisa", "go": "horas" }
                                    ]),
                                );
                            }
                        }
                    }
                }
            }
            if now_locked {
                continue;
            }
            if beat.elapsed() >= Duration::from_secs(60) {
                beat = Instant::now();
                let _ = std::fs::write(&alive, crate::pc::now_ms().to_string());
            }

            let entries = work::snapshot();
            let running = entries.iter().find(|e| e.running());
            if running.is_some() {
                idle_since = Instant::now();
                clear("parado");
            }

            // A Movidesk ticket in front for 3 s.
            let title = foreground_title();
            if title != title_since.0 {
                title_since = (title.clone(), Instant::now());
                url = None;
            }
            // The address is read once the tab settled, and again every few seconds (navigation keeps the title).
            if is_browser(&title) && title_since.1.elapsed() >= Duration::from_secs(2) && (url.is_none() || url_at.elapsed() >= Duration::from_secs(4)) {
                url = ua.as_ref().and_then(|ua| browser_url(ua, unsafe { GetForegroundWindow() }));
                url_at = Instant::now();
            }
            // The subject is on the page itself: read once per ticket, when the ticket is first seen.
            let from_url = url.as_deref().and_then(parse_ticket_url).map(|n| {
                if !subjects.contains_key(&n) && title_since.1.elapsed() >= Duration::from_secs(3) {
                    let found = ua.as_ref().and_then(|ua| page_texts(ua, unsafe { GetForegroundWindow() })).as_deref().and_then(ticket_subject);
                    subjects.insert(n.clone(), found.unwrap_or_default());
                }
                let subject = subjects.get(&n).cloned().unwrap_or_default();
                (n, subject)
            });
            if title_since.1.elapsed() >= Duration::from_secs(3) {
                if let Some((number, subject)) = from_url.or_else(|| parse_ticket(&title, &cfg.movidesk_palavra)) {
                    let already = running.is_some_and(|r| r.ticket == number);
                    if !already {
                        let known = entries.iter().rev().find(|e| e.ticket == number);
                        let (label, cmd, args) = match known {
                            Some(e) => (format!("Continuar #{number}"), "work_resume", json!({ "id": e.id })),
                            None => (format!("Iniciar #{number}"), "work_start", json!({ "tipo": "Ticket", "ticket": number, "desc": subject })),
                        };
                        suggest(
                            &format!("ticket-{number}"),
                            &format!("Ticket #{number} aberto no Movidesk"),
                            &subject,
                            json!([{ "label": label, "cmd": cmd, "args": args }, { "label": "Agora não" }]),
                        );
                    } else {
                        clear(&format!("ticket-{number}"));
                    }
                }
            }

            // WhatsApp in front for a while, and not what is being timed.
            let in_whats = !is_browser(&title) && title.contains("WhatsApp") && whatsapp_app_in_front(&title);
            whats_since = if in_whats { whats_since.or(Some(Instant::now())) } else { None };
            let whats_long = cfg.whats_min > 0 && whats_since.is_some_and(|t| t.elapsed() >= Duration::from_secs(cfg.whats_min as u64 * 60));
            if whats_long && running.is_none_or(|r| r.tipo != "WhatsApp") {
                let detail = running.map(|r| format!("{} vai ficar pausado", tag(r))).unwrap_or_default();
                suggest(
                    "whats",
                    "Apontando WhatsApp?",
                    &detail,
                    json!([{ "label": "Iniciar WhatsApp", "cmd": "work_start", "args": { "tipo": "WhatsApp" } }, { "label": "Agora não" }]),
                );
            }

            let (min, weekday, mday) = local_clock();

            // First time the PC is in use today: what was left unposted on the previous days.
            if morning_noted != Some(mday) {
                morning_noted = Some(mday);
                let n = work::unsent_before(today_at(0));
                if n > 0 {
                    suggest(
                        &format!("manha-{mday}"),
                        &format!("Ficaram {n} período(s) sem lançar no Movidesk"),
                        "de dias anteriores",
                        json!([{ "label": "Lançar agora", "go": "fechamento", "args": { "anterior": true } }, { "label": "Depois" }]),
                    );
                }
            }

            // End of the shift: what is still running stops there; the phone asks if you stayed.
            if let Some(end) = minutes(&cfg.fim_turno) {
                if (1..=5).contains(&weekday) && min >= end && shift_end_noted != Some(mday) {
                    shift_end_noted = Some(mday);
                    if let Some((e, _)) = work::cut_running(today_at(end)) {
                        suggest(
                            "fim-turno",
                            &format!("Pausei {} às {} (fim do turno)", tag(&e), cfg.fim_turno),
                            "Ainda está trabalhando nisso?",
                            json!([{ "label": "Sim, continuar contando", "cmd": "work_reopen", "args": { "id": e.id } }, { "label": "Não, pode parar" }]),
                        );
                    }
                }
            }
            let shift = (1..=5).contains(&weekday)
                && minutes(&cfg.inicio_turno).is_none_or(|s| min >= s)
                && minutes(&cfg.fim_turno).is_none_or(|e| min < e);

            // Nothing being timed during the shift.
            let idle_long = idle_since.elapsed() >= Duration::from_secs(cfg.lembrete_min as u64 * 60);
            let note_due = last_idle_note.is_none_or(|t| t.elapsed() >= Duration::from_secs(20 * 60));
            if shift && cfg.lembrete_min > 0 && running.is_none() && idle_long && note_due {
                {
                    last_idle_note = Some(Instant::now());
                    let mut actions = vec![json!({ "label": "Novo apontamento", "go": "horas" }), json!({ "label": "Tô livre" })];
                    if let Some(p) = entries.iter().filter(|e| !e.done).max_by_key(|e| e.segs.last().and_then(|g| g.e).unwrap_or(0)) {
                        actions.insert(0, json!({ "label": format!("Continuar {}", tag(p)), "cmd": "work_resume", "args": { "id": p.id } }));
                    }
                    suggest(
                        &format!("parado-{}", crate::pc::now_ms() / 60_000),
                        &format!("Nada sendo apontado há {} min", idle_since.elapsed().as_secs() / 60),
                        "",
                        Value::Array(actions),
                    );
                }
            }

            // One period running for hours.
            if let Some(r) = running.filter(|_| cfg.lembrete_longo_min > 0) {
                let seg = r.segs.last().map(|g| g.s).unwrap_or(r.start);
                let mins = (crate::pc::now_ms() - seg) / 60_000;
                if mins >= cfg.lembrete_longo_min as i64 && long_noted != Some((r.id, seg)) {
                    long_noted = Some((r.id, seg));
                    suggest(
                        &format!("longo-{}-{seg}", r.id),
                        &format!("Ainda em {}?", tag(r)),
                        &format!("Rodando há {}h{:02}", mins / 60, mins % 60),
                        json!([{ "label": "Sim, continuo" }, { "label": "Concluir", "cmd": "work_finish", "args": { "id": r.id } }]),
                    );
                }
            }

            // Ten minutes before the end of the shift: what is still to post in Movidesk.
            if let Some(end) = minutes(&cfg.fim_turno) {
                if (1..=5).contains(&weekday) && min + 10 >= end && min < end + 60 && closing_noted != Some(weekday) {
                    let today = today_at(0);
                    let pending = entries
                        .iter()
                        .filter(|e| e.tipo != "Almoço")
                        .flat_map(|e| e.segs.iter())
                        .filter(|g| g.e.is_some() && !g.sent && g.s >= today)
                        .count();
                    closing_noted = Some(weekday);
                    if pending > 0 {
                        suggest(
                            "fechamento",
                            "Fechamento do dia",
                            &format!("{pending} período(s) para lançar no Movidesk"),
                            json!([{ "label": "Lançar agora", "go": "fechamento" }, { "label": "Depois" }]),
                        );
                    }
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::parse_ticket;

    #[test]
    fn tickets() {
        assert_eq!(
            parse_ticket("Ticket #48230 - Erro ao emitir NF-e - Movidesk - Google Chrome", "movidesk"),
            Some(("48230".into(), "Erro ao emitir NF-e".into()))
        );
        assert_eq!(parse_ticket("48230 | Sem acesso | Movidesk", "movidesk").map(|t| t.0), Some("48230".into()));
        assert_eq!(parse_ticket("YouTube - Google Chrome", "movidesk"), None);
        assert_eq!(super::parse_ticket_url("empresa.movidesk.com/Ticket/EditByProtocol/202601010001"), Some("202601010001".into()));
        assert_eq!(super::parse_ticket_url("https://empresa.movidesk.com/Ticket/Edit/48230?x=1"), Some("48230".into()));
        assert_eq!(super::parse_ticket_url("empresa.movidesk.com/Ticket"), None);
        let page: Vec<String> = ["CLIENTE EXEMPLO", "202601010001", "Erro ao gerar relatório", "Ticket aberto via sistema pelo agente FULANO em 01/01/2026 16:49"]
            .map(String::from)
            .to_vec();
        assert_eq!(super::ticket_subject(&page), Some("Erro ao gerar relatório".into()));
    }
}
