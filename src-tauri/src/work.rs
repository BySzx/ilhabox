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
    /// Posted in Movidesk (each period is its own Movidesk entry).
    pub sent: bool,
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
    /// Checklist items ticked during the work (go into the Movidesk text).
    pub check: Vec<String>,
    /// Concluded: no longer paused, counts as "to post in Movidesk".
    pub done: bool,
    /// Every period posted in Movidesk (derived from the periods).
    pub sent: bool,
    /// First start and, once concluded, last end (kept for the phone and for old files).
    pub start: i64,
    pub end: Option<i64>,
}

impl Entry {
    pub fn running(&self) -> bool {
        self.segs.last().is_some_and(|s| s.e.is_none())
    }

    fn close(&mut self, at: i64) {
        if let Some(s) = self.segs.last_mut().filter(|s| s.e.is_none()) {
            s.e = Some(at.max(s.s));
        }
    }

    /// Old files had one start/end per entry and no periods, then one "sent" per entry.
    fn migrate(&mut self) {
        if self.segs.is_empty() {
            self.segs.push(Seg { s: self.start, e: self.end, sent: self.sent });
            self.done = self.end.is_some();
        }
        if self.sent && self.segs.iter().all(|g| !g.sent) {
            for g in &mut self.segs {
                g.sent = true;
            }
        }
    }

    fn refresh(&mut self) {
        self.segs.sort_by_key(|s| s.s);
        self.start = self.segs.first().map(|s| s.s).unwrap_or(self.start);
        self.end = if self.done { self.segs.last().and_then(|s| s.e) } else { None };
        self.sent = !self.segs.is_empty() && self.segs.iter().all(|g| g.sent);
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
struct Book {
    entries: Vec<Entry>,
    /// Stretches marked "livre" (nothing to post): they stop showing as gaps.
    livres: Vec<Seg>,
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
    let livres: Vec<&Seg> = book.livres.iter().filter(|g| g.s >= since).collect();
    bus::send("work", serde_json::json!({ "entries": recent, "livres": livres }), true);
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
    book.livres.retain(|g| g.s >= cutoff);
    normalize(&mut book);
    for e in &mut book.entries {
        e.refresh();
    }
    publish(&book);
    *BOOK.lock() = Some((path, book));
}

fn with<T>(f: impl FnOnce(&mut Book) -> Result<T, String>) -> Result<T, String> {
    let mut g = BOOK.lock();
    let (path, book) = g.as_mut().ok_or("apontamentos não carregados")?;
    let out = f(book)?;
    normalize(book);
    for e in &mut book.entries {
        e.refresh();
    }
    book.entries.sort_by_key(|e| e.start);
    write(path, book);
    publish(book);
    Ok(out)
}

/// Same activity: same kind and ticket; for other kinds, the same description (or one of them empty).
fn same_activity(a: &Entry, b: &Entry) -> bool {
    let kind = |e: &Entry| if e.tipo.is_empty() { "Ticket".to_string() } else { e.tipo.clone() };
    kind(a) == kind(b)
        && if kind(a) == "Ticket" { !a.ticket.is_empty() && a.ticket == b.ticket } else { a.desc.is_empty() || b.desc.is_empty() || a.desc == b.desc }
}

/// Anything else timed between `from` and `to`, apart from the given entries.
fn busy_between(book: &Book, skip: &[u64], from: i64, to: i64) -> bool {
    let now = now_ms();
    book.entries.iter().filter(|e| !skip.contains(&e.id)).flat_map(|e| e.segs.iter()).any(|g| g.e.unwrap_or(now) > from && g.s < to)
}

/**
 * Short breaks (bathroom, coffee, a minute doing nothing) never split an activity: periods of the
 * same activity apart by up to "juntar_pausa_min", with nothing else timed in between, become
 * one — also when the activity was started again with Iniciar instead of Continuar. Periods
 * already posted in Movidesk are never touched.
 */
fn normalize(book: &mut Book) {
    let join_ms = crate::config::get().juntar_pausa_min as i64 * 60_000;
    if join_ms == 0 {
        return;
    }
    // Two entries of the same activity, one right after the other: the later one joins the earlier.
    loop {
        book.entries.sort_by_key(|e| e.start);
        let mut pair = None;
        'search: for (i, a) in book.entries.iter().enumerate() {
            let Some(a_end) = a.segs.last().and_then(|g| g.e) else { continue };
            if a.segs.last().is_some_and(|g| g.sent) {
                continue;
            }
            for (j, b) in book.entries.iter().enumerate() {
                let Some(first) = b.segs.first() else { continue };
                if i == j || first.sent || first.s < a_end || first.s - a_end > join_ms || !same_activity(a, b) {
                    continue;
                }
                if !busy_between(book, &[a.id, b.id], a_end, first.s) {
                    pair = Some((i, j));
                    break 'search;
                }
            }
        }
        let Some((i, j)) = pair else { break };
        let b = book.entries.remove(j);
        let i = if j < i { i - 1 } else { i };
        let a = &mut book.entries[i];
        if a.desc.is_empty() {
            a.desc = b.desc;
        }
        a.check.extend(b.check.into_iter().filter(|c| !a.check.contains(c)).collect::<Vec<_>>());
        a.segs.extend(b.segs);
        a.done = b.done;
    }
    // Inside one entry: consecutive periods with a short break and nothing else in between.
    let snapshot: Vec<(u64, Vec<Seg>)> = book.entries.iter().map(|e| (e.id, e.segs.clone())).collect();
    for e in &mut book.entries {
        e.segs.sort_by_key(|g| g.s);
        let mut out: Vec<Seg> = Vec::with_capacity(e.segs.len());
        for g in e.segs.drain(..) {
            if let Some(prev) = out.last_mut() {
                if let Some(end) = prev.e {
                    let others = snapshot.iter().filter(|(id, _)| *id != e.id).flat_map(|(_, s)| s.iter());
                    let blocked = others.clone().any(|o| o.e.unwrap_or(i64::MAX) > end && o.s < g.s);
                    if !prev.sent && !g.sent && g.s >= end && g.s - end <= join_ms && !blocked {
                        prev.e = g.e;
                        continue;
                    }
                }
            }
            out.push(g);
        }
        e.segs = out;
    }
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

/// Copy of the entries, for the watcher's reminders and suggestions.
pub fn snapshot() -> Vec<Entry> {
    BOOK.lock().as_ref().map(|(_, b)| b.entries.clone()).unwrap_or_default()
}

/// Pauses what is running and says which one it was (the PC got locked).
pub fn pause_running() -> Option<u64> {
    let id = snapshot().into_iter().find(Entry::running).map(|e| e.id)?;
    pause().ok()?;
    Some(id)
}

/// A finished entry typed in afterwards (filling a gap of the day).
pub fn add(args: &Value) -> Result<Value, String> {
    let tipo = s(args, "tipo").filter(|t| !t.is_empty()).unwrap_or_else(|| "Ticket".into());
    let ticket = if tipo == "Ticket" { s(args, "ticket").unwrap_or_default() } else { String::new() };
    let desc = s(args, "desc").unwrap_or_default();
    let segs: Vec<Seg> = args.get("segs").cloned().and_then(|v| serde_json::from_value(v).ok()).ok_or("sem horário")?;
    if segs.is_empty() || segs.iter().any(|g| g.e.is_none_or(|e| e <= g.s)) {
        return Err("horário inválido".into());
    }
    with(|b| {
        let e = Entry { id: next_id(b), tipo, ticket, desc, segs, done: true, ..Default::default() };
        b.entries.push(e);
        Ok(Value::Null)
    })
}

/// New activity; the one running is paused, not concluded.
pub fn start(args: &Value) -> Result<Value, String> {
    let tipo = s(args, "tipo").filter(|t| !t.is_empty()).unwrap_or_else(|| "Ticket".into());
    let ticket = if tipo == "Ticket" { s(args, "ticket").unwrap_or_default() } else { String::new() };
    let desc = s(args, "desc").unwrap_or_default();
    with(|b| {
        let at = now_ms();
        pause_all(b, at);
        let e = Entry { id: next_id(b), tipo, ticket, desc, segs: vec![Seg { s: at, e: None, sent: false }], start: at, ..Default::default() };
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

/// Continues an entry (paused or concluded); the one running is paused. After a short pause
/// with nothing else timed in between (bathroom, coffee) the same period just goes on, so the
/// day does not fill up with tiny Movidesk entries.
pub fn resume(args: &Value) -> Result<Value, String> {
    let join_ms = crate::config::get().juntar_pausa_min as i64 * 60_000;
    with(|b| {
        let i = entry(b, args)?;
        if b.entries[i].running() {
            return Ok(Value::Null);
        }
        let at = now_ms();
        pause_all(b, at);
        let id = b.entries[i].id;
        let last = b.entries[i].segs.last().copied();
        let other_between = |from: i64| {
            b.entries.iter().filter(|o| o.id != id).flat_map(|o| o.segs.iter()).any(|g| g.e.unwrap_or(at) > from && g.s < at)
        };
        let join = last.and_then(|g| g.e).is_some_and(|end| at - end <= join_ms && !g_sent(&last) && !other_between(end));
        let e = &mut b.entries[i];
        if join {
            if let Some(g) = e.segs.last_mut() {
                g.e = None;
            }
        } else {
            e.segs.push(Seg { s: at, e: None, sent: false });
        }
        e.done = false;
        Ok(Value::Null)
    })
}

fn g_sent(g: &Option<Seg>) -> bool {
    g.is_some_and(|g| g.sent)
}

/// Marks a stretch of the day as free time (nothing to post): it stops showing as a gap.
pub fn free(args: &Value) -> Result<Value, String> {
    let s = args.get("s").and_then(Value::as_i64).ok_or("sem início")?;
    let e = args.get("e").and_then(Value::as_i64).ok_or("sem fim")?;
    if e <= s {
        return Err("horário inválido".into());
    }
    with(|b| {
        b.livres.push(Seg { s, e: Some(e), sent: true });
        Ok(Value::Null)
    })
}

/// Turns all periods of an entry into one, from the first start to the last end.
pub fn join(args: &Value) -> Result<Value, String> {
    with(|b| {
        let i = entry(b, args)?;
        let e = &mut b.entries[i];
        let (Some(first), Some(last)) = (e.segs.first().copied(), e.segs.last().copied()) else { return Ok(Value::Null) };
        e.segs = vec![Seg { s: first.s, e: last.e, sent: false }];
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
        if let Some(v) = args.get("check").and_then(Value::as_array) {
            e.check = v.iter().filter_map(Value::as_str).map(str::to_string).collect();
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
        if let Some(v) = args.get("sent").and_then(Value::as_bool) {
            for g in &mut e.segs {
                g.sent = v && g.e.is_some();
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

/// Marks periods as posted in Movidesk: whole entries ("ids") or single periods
/// ("periods": [{ "id", "s" }], the period identified by its start).
pub fn mark_sent(args: &Value) -> Result<Value, String> {
    let ids: Vec<u64> = args.get("ids").and_then(Value::as_array).map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_default();
    let periods: Vec<(u64, i64)> = args
        .get("periods")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(|p| Some((p.get("id")?.as_u64()?, p.get("s")?.as_i64()?))).collect())
        .unwrap_or_default();
    with(|b| {
        for e in &mut b.entries {
            let all = ids.contains(&e.id);
            for g in e.segs.iter_mut().filter(|g| g.e.is_some()) {
                if all || periods.contains(&(e.id, g.s)) {
                    g.sent = true;
                }
            }
        }
        Ok(Value::Null)
    })
}

/// Undoes an automatic pause: the last period of the entry is open again (it never stopped).
pub fn reopen(args: &Value) -> Result<Value, String> {
    with(|b| {
        let i = entry(b, args)?;
        let at = now_ms();
        pause_all(b, at);
        let e = &mut b.entries[i];
        if let Some(g) = e.segs.last_mut() {
            g.e = None;
            g.sent = false;
        }
        e.done = false;
        Ok(Value::Null)
    })
}

/// Closes what is running at a given time (end of the shift, last moment the PC was in use).
/// Returns the entry and where it was cut.
pub fn cut_running(at: i64) -> Option<(Entry, i64)> {
    with(|b| {
        let e = b.entries.iter_mut().find(|e| e.running()).ok_or("nada rodando")?;
        e.close(at);
        let cut = e.segs.last().and_then(|g| g.e).unwrap_or(at);
        Ok((e.clone(), cut))
    })
    .ok()
}

/// Periods already closed and not posted in Movidesk on days before `before` (last week).
pub fn unsent_before(before: i64) -> usize {
    let since = before - 7 * DAY_MS;
    snapshot()
        .iter()
        .filter(|e| e.tipo != "Almoço")
        .flat_map(|e| e.segs.iter())
        .filter(|g| g.e.is_some() && !g.sent && g.s < before && g.s >= since)
        .count()
}
