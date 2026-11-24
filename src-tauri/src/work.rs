//! Time entries for Movidesk. One entry is one activity (a ticket, a course, a meeting…) made of
//! one or more periods: it can be paused (bathroom, a WhatsApp question in between) and continued
//! later, keeping its original start. States: running (last period open), paused (no open period,
//! not concluded) and concluded. Only one entry runs at a time; starting or continuing another one
//! pauses it. Stored in `apontamentos.json` (atomic writes); the phone gets the last 14 days.

use crate::{bus, diag, pc::now_ms};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const KEEP_DAYS: i64 = 60;
const SEND_DAYS: i64 = 14;
const DAY_MS: i64 = 86_400_000;

#[derive(Serialize, Deserialize, Clone, Copy, Default)]
#[serde(default)]
pub struct Seg {
    pub s: i64,
    pub e: Option<i64>,
}

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub id: u64,
    /// Kind of activity: "Ticket", "WhatsApp", "Reunião", "Curso"… (empty in old entries = Ticket).
    pub tipo: String,
    pub ticket: String,
    pub desc: String,
    pub segs: Vec<Seg>,
    /// Concluded: no longer paused, counts as "to post in Movidesk".
    pub done: bool,
    /// Marked once it was pasted into Movidesk.
    pub sent: bool,
    /// First start and, once concluded, last end (kept for the phone and for old files).
    pub start: i64,
    pub end: Option<i64>,
}

impl Entry {
    fn running(&self) -> bool {
        self.segs.last().is_some_and(|s| s.e.is_none())
    }

    fn close(&mut self, at: i64) {
        if let Some(s) = self.segs.last_mut().filter(|s| s.e.is_none()) {
            s.e = Some(at.max(s.s));
        }
    }

    /// Old files had one start/end per entry and no periods.
    fn migrate(&mut self) {
        if self.segs.is_empty() {
            self.segs.push(Seg { s: self.start, e: self.end });
            self.done = self.end.is_some();
        }
    }

    fn refresh(&mut self) {
        self.segs.sort_by_key(|s| s.s);
        self.start = self.segs.first().map(|s| s.s).unwrap_or(self.start);
        self.end = if self.done { self.segs.last().and_then(|s| s.e) } else { None };
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Book {
    entries: Vec<Entry>,
}

static BOOK: Mutex<Option<(PathBuf, Book)>> = Mutex::new(None);

fn write(path: &PathBuf, book: &Book) {
    let tmp = path.with_extension("json.tmp");
    let ok = serde_json::to_vec_pretty(book).ok().is_some_and(|b| fs::write(&tmp, b).is_ok()) && fs::rename(&tmp, path).is_ok();
    if !ok {
        diag::log("apontamentos: falha ao gravar");
    }
}

fn publish(book: &Book) {
    let since = now_ms() - SEND_DAYS * DAY_MS;
    let recent: Vec<&Entry> = book.entries.iter().filter(|e| e.start >= since || !e.done).collect();
    bus::send("work", serde_json::json!({ "entries": recent }), true);
}

pub fn init(dir: PathBuf) {
    let path = dir.join("apontamentos.json");
    let mut book: Book = fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    for e in &mut book.entries {
        e.migrate();
        e.refresh();
    }
    let cutoff = now_ms() - KEEP_DAYS * DAY_MS;
    book.entries.retain(|e| e.start >= cutoff || !e.done);
    publish(&book);
    *BOOK.lock() = Some((path, book));
}

fn with<T>(f: impl FnOnce(&mut Book) -> Result<T, String>) -> Result<T, String> {
    let mut g = BOOK.lock();
    let (path, book) = g.as_mut().ok_or("apontamentos não carregados")?;
    let out = f(book)?;
    for e in &mut book.entries {
        e.refresh();
    }
    book.entries.sort_by_key(|e| e.start);
    write(path, book);
    publish(book);
    Ok(out)
}

fn next_id(book: &Book) -> u64 {
    book.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1
}

/// Pauses whatever is running (it stays open to be continued).
fn pause_all(book: &mut Book, at: i64) {
    for e in book.entries.iter_mut().filter(|e| e.running()) {
        e.close(at);
    }
}

fn entry(book: &Book, args: &Value) -> Result<usize, String> {
    let id = args.get("id").and_then(Value::as_u64).ok_or("sem id")?;
    book.entries.iter().position(|e| e.id == id).ok_or_else(|| "apontamento não existe".into())
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(Value::as_str).map(|v| v.trim().to_string())
}

/// New activity; the one running is paused, not concluded.
pub fn start(args: &Value) -> Result<Value, String> {
    let tipo = s(args, "tipo").filter(|t| !t.is_empty()).unwrap_or_else(|| "Ticket".into());
    let ticket = if tipo == "Ticket" { s(args, "ticket").unwrap_or_default() } else { String::new() };
    let desc = s(args, "desc").unwrap_or_default();
    with(|b| {
        let at = now_ms();
        pause_all(b, at);
        let e = Entry { id: next_id(b), tipo, ticket, desc, segs: vec![Seg { s: at, e: None }], start: at, ..Default::default() };
        b.entries.push(e.clone());
        Ok(serde_json::to_value(e).unwrap_or_default())
    })
}

pub fn pause() -> Result<Value, String> {
    with(|b| {
        pause_all(b, now_ms());
        Ok(Value::Null)
    })
}

/// Continues an entry (paused or concluded) in a new period; the one running is paused.
pub fn resume(args: &Value) -> Result<Value, String> {
    with(|b| {
        let i = entry(b, args)?;
        if b.entries[i].running() {
            return Ok(Value::Null);
        }
        let at = now_ms();
        pause_all(b, at);
        let e = &mut b.entries[i];
        e.segs.push(Seg { s: at, e: None });
        e.done = false;
        e.sent = false;
        Ok(Value::Null)
    })
}

/// Concludes one entry, or the running one when no id is given.
pub fn finish(args: &Value) -> Result<Value, String> {
    with(|b| {
        let at = now_ms();
        let i = match args.get("id") {
            Some(_) => entry(b, args)?,
            None => b.entries.iter().position(Entry::running).ok_or("nada em andamento")?,
        };
        let e = &mut b.entries[i];
        e.close(at);
        e.done = true;
        Ok(Value::Null)
    })
}

/// Edits fields and, with "segs", the periods (epoch ms; only the last may be open).
pub fn update(args: &Value) -> Result<Value, String> {
    with(|b| {
        let i = entry(b, args)?;
        let e = &mut b.entries[i];
        if let Some(v) = s(args, "tipo").filter(|t| !t.is_empty()) {
            e.tipo = v;
        }
        if let Some(v) = s(args, "ticket") {
            e.ticket = v;
        }
        if !e.tipo.is_empty() && e.tipo != "Ticket" {
            e.ticket.clear();
        }
        if let Some(v) = s(args, "desc") {
            e.desc = v;
        }
        if let Some(v) = args.get("sent").and_then(Value::as_bool) {
            e.sent = v;
        }
        if let Some(list) = args.get("segs") {
            let mut segs: Vec<Seg> = serde_json::from_value(list.clone()).map_err(|_| "períodos inválidos")?;
            segs.sort_by_key(|s| s.s);
            if segs.is_empty() {
                return Err("o apontamento precisa de pelo menos um período".into());
            }
            for (k, sg) in segs.iter().enumerate() {
                match sg.e {
                    Some(end) if end < sg.s => return Err("um período termina antes de começar".into()),
                    None if k + 1 != segs.len() => return Err("só o último período pode estar aberto".into()),
                    _ => {}
                }
                if let (Some(end), Some(next)) = (sg.e, segs.get(k + 1)) {
                    if end > next.s {
                        return Err("dois períodos se sobrepõem".into());
                    }
                }
            }
            e.segs = segs;
            if e.running() {
                e.done = false;
            }
        }
        Ok(Value::Null)
    })
}

pub fn remove(args: &Value) -> Result<Value, String> {
    with(|b| {
        let i = entry(b, args)?;
        b.entries.remove(i);
        Ok(Value::Null)
    })
}

/// Marks several entries as already pasted into Movidesk.
pub fn mark_sent(args: &Value) -> Result<Value, String> {
    let ids: Vec<u64> = args.get("ids").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
    with(|b| {
        for e in b.entries.iter_mut().filter(|e| ids.contains(&e.id)) {
            e.sent = true;
        }
        Ok(Value::Null)
    })
}
