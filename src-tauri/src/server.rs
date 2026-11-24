//! Local HTTP server on 127.0.0.1:8777. The phone reaches it through `adb reverse` over the USB
//! cable; the settings window reaches it directly. The pages are public (they hold no data); every
//! /api route needs the key, which only the PC and the IlhaBox app on the phone know.

use crate::{bus, cmd, diag};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::mpsc::RecvTimeoutError;
use std::thread;
use std::time::Duration;
use tauri::AppHandle;
use tiny_http::{Header, Method, Request, Response, Server};

pub const PORT: u16 = 8777;
pub static KEY: std::sync::OnceLock<String> = std::sync::OnceLock::new();

fn header(k: &str, v: &str) -> Header {
    Header::from_bytes(k.as_bytes(), v.as_bytes()).expect("header")
}

fn cors(r: Response<std::io::Cursor<Vec<u8>>>) -> Response<std::io::Cursor<Vec<u8>>> {
    r.with_header(header("Access-Control-Allow-Origin", "*"))
        .with_header(header("Access-Control-Allow-Headers", "Authorization, Content-Type"))
        .with_header(header("Access-Control-Allow-Methods", "GET, POST, OPTIONS"))
        .with_header(header("Cache-Control", "no-store"))
}

fn json_reply(req: Request, code: u16, body: &Value) {
    let r = Response::from_string(body.to_string()).with_status_code(code).with_header(header("Content-Type", "application/json; charset=utf-8"));
    let _ = req.respond(cors(r));
}

fn query(url: &str, key: &str) -> Option<String> {
    url.split_once('?')?.1.split('&').find_map(|kv| kv.strip_prefix(&format!("{key}=")).map(str::to_string))
}

fn authorized(req: &Request, url: &str, key: &str) -> bool {
    let bearer = req.headers().iter().find(|h| h.field.equiv("Authorization")).map(|h| h.value.as_str().to_string());
    bearer.as_deref() == Some(&format!("Bearer {key}")) || query(url, "k").as_deref() == Some(key)
}

fn asset(app: &AppHandle, req: Request, path: &str) {
    let path = match path.trim_start_matches('/') {
        "" => "box.html",
        p => p,
    };
    match app.asset_resolver().get(path.to_string()) {
        Some(a) => {
            // Hashed bundles never change; the html must, so a new version shows up on reload.
            let cache = if path.starts_with("assets/") { "public, max-age=31536000, immutable" } else { "no-store" };
            let r = Response::from_data(a.bytes().to_vec()).with_header(header("Content-Type", a.mime_type())).with_header(header("Cache-Control", cache));
            let _ = req.respond(r);
        }
        None => {
            let _ = req.respond(Response::from_string("não encontrado").with_status_code(404));
        }
    }
}

/// Server-sent events written straight to the socket, flushed per event (tiny_http would buffer a
/// chunked body). The current state goes first so a fresh page needs no second request.
fn stream(req: Request) {
    let rx = bus::subscribe();
    let mut w = req.into_writer();
    let head = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream; charset=utf-8\r\nCache-Control: no-store\r\nConnection: keep-alive\r\nAccess-Control-Allow-Origin: *\r\nX-Accel-Buffering: no\r\n\r\n";
    let first = format!("{head}retry: 1500\nevent: snapshot\ndata: {}\n\n", bus::snapshot());
    if w.write_all(first.as_bytes()).and_then(|_| w.flush()).is_err() {
        return;
    }
    loop {
        let chunk = match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(line) => line,
            Err(RecvTimeoutError::Timeout) => ": ping\n\n".to_string(),
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if w.write_all(chunk.as_bytes()).and_then(|_| w.flush()).is_err() {
            return;
        }
    }
}

fn handle(app: AppHandle, key: String, mut req: Request) {
    let url = req.url().to_string();
    let path = url.split('?').next().unwrap_or("/").to_string();
    if *req.method() == Method::Options {
        let _ = req.respond(cors(Response::from_data(Vec::new()).with_status_code(204)));
        return;
    }
    if !path.starts_with("/api/") {
        return asset(&app, req, &path);
    }
    if !authorized(&req, &url, &key) {
        return json_reply(req, 401, &json!({ "erro": "chave inválida" }));
    }
    match (req.method().clone(), path.as_str()) {
        (Method::Get, "/api/events") => stream(req),
        (Method::Get, "/api/snapshot") => json_reply(req, 200, &bus::snapshot()),
        (Method::Post, p) if p.starts_with("/api/cmd/") => {
            let name = p.trim_start_matches("/api/cmd/").to_string();
            let mut body = String::new();
            let _ = req.as_reader().take(4 * 1024 * 1024).read_to_string(&mut body);
            let args: Value = serde_json::from_str(&body).unwrap_or(Value::Null);
            match cmd::run(&app, &name, args) {
                Ok(v) => json_reply(req, 200, &json!({ "ok": true, "data": v })),
                Err(e) => json_reply(req, 400, &json!({ "ok": false, "erro": e })),
            }
        }
        _ => json_reply(req, 404, &json!({ "erro": "rota desconhecida" })),
    }
}

pub fn spawn(app: AppHandle, key: String) {
    let _ = KEY.set(key.clone());
    thread::spawn(move || {
        let server = match Server::http(("127.0.0.1", PORT)) {
            Ok(s) => s,
            Err(e) => {
                diag::log(&format!("servidor nao abriu na porta {PORT}: {e}"));
                bus::keep("server", json!({ "ok": false, "erro": format!("porta {PORT} ocupada") }));
                return;
            }
        };
        diag::log(&format!("servidor ouvindo em 127.0.0.1:{PORT}"));
        bus::keep("server", json!({ "ok": true, "port": PORT }));
        for req in server.incoming_requests() {
            let (app, key) = (app.clone(), key.clone());
            thread::spawn(move || handle(app, key, req));
        }
    });
}
