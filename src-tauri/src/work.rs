//! Time entries for Movidesk: a stopwatch per ticket. Entries live in `apontamentos.json` in the
//! config folder (atomic writes); the phone gets the last 14 days on the "work" topic and builds
//! the text to paste into Movidesk itself.

use crate::{bus, diag, pc::now_ms};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const KEEP_DAYS: i64 = 60;
const SEND_DAYS: i64 = 14;
const DAY_MS: i64 = 86_400_000;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Entry {
    pub id: u64,
    /// Kind of activity: "Ticket", "WhatsApp", "Reunião", "Curso"… (empty in old entries = Ticket).
    pub tipo: String,
    pub ticket: String,
    pub desc: String,
    pub start: i64,
    pub end: Option<i64>,
    /// Marked once it was pasted into Movidesk, so the day view shows what is still pending.
    pub sent: bool,
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
    let recent: Vec<&Entry> = book.entries.iter().filter(|e| e.start >= since || e.end.is_none()).collect();
    bus::send("work", serde_json::json!({ "entries": recent, "offsetMin": crate::pc::utc_offset_min() }), true);
}

pub fn init(dir: PathBuf) {
    let path = dir.join("apontamentos.json");
    let mut book: Book = fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()).unwrap_or_default();
    let cutoff = now_ms() - KEEP_DAYS * DAY_MS;
    book.entries.retain(|e| e.start >= cutoff || e.end.is_none());
    publish(&book);
    *BOOK.lock() = Some((path, book));
}

fn with<T>(f: impl FnOnce(&mut Book) -> Result<T, String>) -> Result<T, String> {
    let mut g = BOOK.lock();
    let (path, book) = g.as_mut().ok_or("apontamentos não carregados")?;
    let out = f(book)?;
    book.entries.sort_by_key(|e| e.start);
    write(path, book);
    publish(book);
    Ok(out)
}

fn next_id(book: &Book) -> u64 {
    book.entries.iter().map(|e| e.id).max().unwrap_or(0) + 1
}

fn stop_running(book: &mut Book, at: i64) {
    for e in book.entries.iter_mut().filter(|e| e.end.is_none()) {
        e.end = Some(at.max(e.start));
    }
}

fn s(args: &Value, k: &str) -> Option<String> {
    args.get(k).and_then(Value::as_str).map(|v| v.trim().to_string())
}

fn n(args: &Value, k: &str) -> Option<i64> {
    args.get(k).and_then(Value::as_i64)
}

/// Starts a stopwatch; whatever was running stops at the same instant (one thing at a time).
pub fn start(args: &Value) -> Result<Value, String> {
    let tipo = s(args, "tipo").filter(|t| !t.is_empty()).unwrap_or_else(|| "Ticket".into());
    let ticket = if tipo == "Ticket" { s(args, "ticket").unwrap_or_default() } else { String::new() };
    let desc = s(args, "desc").unwrap_or_default();
    with(|b| {
        let at = now_ms();
        stop_running(b, at);
        let e = Entry { id: next_id(b), tipo, ticket, desc, start: at, end: None, sent: false };
        b.entries.push(e.clone());
        Ok(serde_json::to_value(e).unwrap_or_default())
    })
}

pub fn stop() -> Result<Value, String> {
    with(|b| {
        let at = now_ms();
        let id = b.entries.iter().find(|e| e.end.is_none()).map(|e| e.id);
        stop_running(b, at);
        Ok(serde_json::json!({ "id": id }))
    })
}

/// Edits any field; times are epoch ms. An entry can never end before it starts.
pub fn update(args: &Value) -> Result<Value, String> {
    let id = args.get("id").and_then(Value::as_u64).ok_or("sem id")?;
    with(|b| {
        let e = b.entries.iter_mut().find(|e| e.id == id).ok_or("apontamento não existe")?;
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
        if let Some(v) = n(args, "start") {
            e.start = v;
        }
        if let Some(v) = n(args, "end") {
            e.end = Some(v);
        }
        if let Some(v) = args.get("sent").and_then(Value::as_bool) {
            e.sent = v;
        }
        if let Some(end) = e.end {
            if end < e.start {
                return Err("o fim está antes do início".into());
            }
        }
        Ok(Value::Null)
    })
}

pub fn remove(args: &Value) -> Result<Value, String> {
    let id = args.get("id").and_then(Value::as_u64).ok_or("sem id")?;
    with(|b| {
        let before = b.entries.len();
        b.entries.retain(|e| e.id != id);
        if b.entries.len() == before { Err("apontamento não existe".into()) } else { Ok(Value::Null) }
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
