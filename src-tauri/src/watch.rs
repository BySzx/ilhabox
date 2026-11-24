//! Watches the PC once a second and turns what it sees into suggestions on the phone: a Movidesk
//! ticket opened in the browser, a long time with WhatsApp in front, nothing being timed, one
//! period running for hours, the end of the shift. Locking the PC (Win+L) pauses the running
//! activity and dims the phone; unlocking brings both back.

use crate::{bus, config, diag, work};
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::System::StationsAndDesktops::{CloseDesktop, OpenInputDesktop, DESKTOP_ACCESS_FLAGS, DESKTOP_CONTROL_FLAGS};
use windows::Win32::System::SystemInformation::GetLocalTime;
use windows::Win32::UI::WindowsAndMessaging::{GetForegroundWindow, GetWindowTextW};

const DESKTOP_SWITCHDESKTOP: u32 = 0x0100;
static DISMISSED: Mutex<Option<HashMap<String, Instant>>> = Mutex::new(None);

fn locked() -> bool {
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

pub fn foreground_title() -> String {
    unsafe {
        let hwnd = GetForegroundWindow();
        let mut buf = [0u16; 512];
        let n = GetWindowTextW(hwnd, &mut buf) as usize;
        String::from_utf16_lossy(&buf[..n])
    }
}

/// (minutes since midnight, weekday 0 = Sunday)
fn local_clock() -> (u32, u16) {
    let t = unsafe { GetLocalTime() };
    (t.wHour as u32 * 60 + t.wMinute as u32, t.wDayOfWeek)
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
            if (4..=8).contains(&len) && best.is_none_or(|b| hashed && !b.2) {
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

pub fn spawn() {
    bus::keep("pc", json!({ "locked": false }));
    thread::spawn(|| {
        let mut was_locked = false;
        let mut locked_at = Instant::now();
        let mut paused_by_lock: Option<u64> = None;
        let mut title_since: (String, Instant) = (String::new(), Instant::now());
        let mut whats_since: Option<Instant> = None;
        let mut idle_since = Instant::now();
        let mut last_idle_note: Option<Instant> = None;
        let mut long_noted: Option<(u64, i64)> = None;
        let mut closing_noted: Option<u16> = None;
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
                            if away < Duration::from_secs(120) {
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
            }
            if title_since.1.elapsed() >= Duration::from_secs(3) {
                if let Some((number, subject)) = parse_ticket(&title, &cfg.movidesk_palavra) {
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
            let in_whats = title.contains("WhatsApp");
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

            let (min, weekday) = local_clock();
            let shift = (1..=5).contains(&weekday)
                && minutes(&cfg.inicio_turno).is_none_or(|s| min >= s)
                && minutes(&cfg.fim_turno).is_none_or(|e| min < e);

            // Nothing being timed during the shift.
            let idle_long = idle_since.elapsed() >= Duration::from_secs(cfg.lembrete_min as u64 * 60);
            let note_due = last_idle_note.is_none_or(|t| t.elapsed() >= Duration::from_secs(20 * 60));
            if shift && cfg.lembrete_min > 0 && running.is_none() && idle_long && note_due {
                {
                    last_idle_note = Some(Instant::now());
                    let mut actions = vec![json!({ "label": "Novo apontamento", "go": "horas" })];
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
                    let pending = entries.iter().filter(|e| e.done && !e.sent && e.tipo != "Almoço" && crate::pc::now_ms() - e.start < 86_400_000).count();
                    closing_noted = Some(weekday);
                    if pending > 0 {
                        suggest(
                            "fechamento",
                            "Fechamento do dia",
                            &format!("{pending} apontamento(s) para lançar no Movidesk"),
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
    }
}
