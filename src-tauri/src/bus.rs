//! Everything the phone and the settings window see passes through here: the latest value of
//! each topic (for a page that just loaded) plus a live feed for the open event streams.

use parking_lot::Mutex;
use serde::Serialize;
use serde_json::{Map, Value};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender, TrySendError};

static LAST: Mutex<Option<Map<String, Value>>> = Mutex::new(None);
static SUBS: Mutex<Vec<SyncSender<String>>> = Mutex::new(Vec::new());

fn frame(topic: &str, v: &Value) -> String {
    format!("event: {topic}\ndata: {v}\n\n")
}

/// Stores `v` as the current value of `topic` without telling anyone (history that a later event extends).
pub fn keep(topic: &str, v: impl Serialize) {
    let v = serde_json::to_value(v).unwrap_or(Value::Null);
    LAST.lock().get_or_insert_with(Map::new).insert(topic.to_string(), v);
}

/// Sends `v` to every open stream; `keep` also makes it the topic's current value.
pub fn send(topic: &str, v: impl Serialize, keep_it: bool) {
    let v = serde_json::to_value(v).unwrap_or(Value::Null);
    let line = frame(topic, &v);
    if keep_it {
        LAST.lock().get_or_insert_with(Map::new).insert(topic.to_string(), v);
    }
    // A stream whose buffer is full is a dead or stuck client: dropping it frees the thread.
    SUBS.lock().retain(|tx| !matches!(tx.try_send(line.clone()), Err(TrySendError::Disconnected(_) | TrySendError::Full(_))));
}

pub fn snapshot() -> Value {
    Value::Object(LAST.lock().clone().unwrap_or_default())
}

pub fn get(topic: &str) -> Option<Value> {
    LAST.lock().as_ref().and_then(|m| m.get(topic).cloned())
}

pub fn subscribe() -> Receiver<String> {
    let (tx, rx) = sync_channel(512);
    SUBS.lock().push(tx);
    rx
}

