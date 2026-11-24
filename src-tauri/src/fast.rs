//! Low-latency stream for the mouse bridge on its own port (8778). tiny_http leaves Nagle's
//! algorithm on, which holds small writes until the previous one is acknowledged: tens of ms per
//! pointer move through the adb tunnel. Here every socket is TCP_NODELAY and every event is flushed.

use crate::diag;
use parking_lot::Mutex;
use std::io::{BufRead, BufReader, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::mpsc::{sync_channel, RecvTimeoutError, SyncSender, TrySendError};
use std::thread;
use std::time::Duration;

pub const PORT: u16 = 8778;
static SUBS: Mutex<Vec<SyncSender<String>>> = Mutex::new(Vec::new());

/// Sends one event to every open pointer stream.
pub fn send(json: &str) {
    let line = format!("data: {json}\n\n");
    SUBS.lock().retain(|tx| !matches!(tx.try_send(line.clone()), Err(TrySendError::Disconnected(_) | TrySendError::Full(_))));
}

fn serve(stream: TcpStream, key: &str) -> std::io::Result<()> {
    let _ = stream.set_nodelay(true);
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut first = String::new();
    reader.read_line(&mut first)?;
    // Skip the rest of the request head.
    let mut line = String::new();
    while reader.read_line(&mut line)? > 2 {
        line.clear();
    }
    let mut w = stream;
    let ok = first.split_whitespace().nth(1).and_then(|p| p.split_once("?k=")).is_some_and(|(path, k)| path == "/ponteiro" && k == key);
    if !ok {
        return w.write_all(b"HTTP/1.1 401 Unauthorized\r\nContent-Length: 0\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n");
    }
    w.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nCache-Control: no-store\r\nAccess-Control-Allow-Origin: *\r\nConnection: keep-alive\r\n\r\nretry: 1000\n\n")?;
    w.flush()?;
    let (tx, rx) = sync_channel(256);
    SUBS.lock().push(tx);
    loop {
        let chunk = match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(l) => l,
            Err(RecvTimeoutError::Timeout) => ": ping\n\n".into(),
            Err(RecvTimeoutError::Disconnected) => return Ok(()),
        };
        w.write_all(chunk.as_bytes())?;
        w.flush()?;
    }
}

pub fn spawn(key: String) {
    thread::spawn(move || {
        let listener = match TcpListener::bind(("127.0.0.1", PORT)) {
            Ok(l) => l,
            Err(e) => return diag::log(&format!("canal do mouse nao abriu na porta {PORT}: {e}")),
        };
        for stream in listener.incoming().flatten() {
            let key = key.clone();
            thread::spawn(move || {
                let _ = serve(stream, &key);
            });
        }
    });
}
