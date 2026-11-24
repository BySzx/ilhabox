//! Mouse bridge: push the PC mouse past the screen edge on the phone's side and it moves into the
//! IlhaBox panel; clicks, wheel and typing go to the phone until the mouse comes back over the same
//! edge. Low-level hooks only park the cursor and forward events; the page on the phone draws the
//! pointer and clicks its own elements.

use crate::{adb, diag, fast};
use serde_json::json;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::OnceLock;
use std::thread;
use std::time::{Duration, Instant};
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, GetKeyState, ToUnicode, VIRTUAL_KEY, VK_BACK, VK_CAPITAL, VK_CONTROL, VK_DELETE, VK_DOWN, VK_ESCAPE, VK_LEFT, VK_LWIN, VK_MENU,
    VK_RETURN, VK_RIGHT, VK_RWIN, VK_SHIFT, VK_TAB, VK_UP,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateCursor, SetSystemCursor, SystemParametersInfoW, OCR_APPSTARTING, OCR_HAND, OCR_IBEAM, OCR_NORMAL, OCR_WAIT, SPI_SETCURSORS,
    SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, CallNextHookEx, GetMessageW, GetSystemMetrics, SetCursorPos, SetWindowsHookExW, KBDLLHOOKSTRUCT, MSG, MSLLHOOKSTRUCT, SM_CXVIRTUALSCREEN,
    SM_CYVIRTUALSCREEN, SM_XVIRTUALSCREEN, SM_YVIRTUALSCREEN, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE,
    WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN,
};

/// 0 off, 1 right, 2 left, 3 below, 4 above (where the phone sits next to the screens).
static SIDE: AtomicU8 = AtomicU8::new(1);
static ACTIVE: AtomicBool = AtomicBool::new(false);
static OUT: OnceLock<Sender<Ev>> = OnceLock::new();

/// Screen pixels of mouse travel that cross the whole phone, horizontally (landscape panel).
const SPAN_X: f64 = 900.0;
const SPAN_Y: f64 = 420.0;

enum Ev {
    On(f64, f64),
    Off,
    Move(f64, f64),
    Button(&'static str, &'static str, f64, f64),
    Wheel(i32, f64, f64),
    Key(String),
    Text(String),
}

struct Hook {
    /// Where the cursor waits (screen center, so deltas work in every direction) while on the phone.
    park: POINT,
    /// Where it crossed the edge; it comes back there.
    entry: POINT,
    x: f64,
    y: f64,
    edge_hits: u8,
    last_check: Instant,
}

static mut HOOK: Option<Hook> = None;

pub fn set_side(side: &str) {
    let v = match side {
        "direita" => 1,
        "esquerda" => 2,
        "baixo" => 3,
        "cima" => 4,
        _ => 0,
    };
    SIDE.store(v, Ordering::Relaxed);
    if v == 0 {
        release();
    }
}

fn send(e: Ev) {
    if let Some(tx) = OUT.get() {
        let _ = tx.send(e);
    }
}

fn screen() -> (i32, i32, i32, i32) {
    unsafe {
        let x = GetSystemMetrics(SM_XVIRTUALSCREEN);
        let y = GetSystemMetrics(SM_YVIRTUALSCREEN);
        (x, y, x + GetSystemMetrics(SM_CXVIRTUALSCREEN), y + GetSystemMetrics(SM_CYVIRTUALSCREEN))
    }
}

/// Puts the cursor back on the PC, just inside the edge it left through.
fn release() {
    if !ACTIVE.swap(false, Ordering::SeqCst) {
        return;
    }
    // SAFETY: HOOK is only touched on the hook thread, and release() runs there (or with hooks idle).
    #[allow(static_mut_refs)]
    if let Some(h) = unsafe { HOOK.as_ref() } {
        let (l, t, r, b) = screen();
        let p = match SIDE.load(Ordering::Relaxed) {
            1 => POINT { x: r - 40, y: h.entry.y },
            2 => POINT { x: l + 40, y: h.entry.y },
            3 => POINT { x: h.entry.x, y: b - 40 },
            _ => POINT { x: h.entry.x, y: t + 40 },
        };
        unsafe {
            let _ = SetCursorPos(p.x, p.y);
        }
        show_cursor();
    }
    send(Ev::Off);
}

unsafe extern "system" fn on_mouse(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(None, code, wp, lp);
    }
    let m = &*(lp.0 as *const MSLLHOOKSTRUCT);
    let side = SIDE.load(Ordering::Relaxed);
    #[allow(static_mut_refs)]
    let Some(h) = HOOK.as_mut() else { return CallNextHookEx(None, code, wp, lp) };
    // LLMHF_INJECTED: our own SetCursorPos and remote-control tools.
    if m.flags & 1 != 0 || side == 0 {
        return CallNextHookEx(None, code, wp, lp);
    }
    let msg = wp.0 as u32;
    if !ACTIVE.load(Ordering::Relaxed) {
        if msg == WM_MOUSEMOVE {
            let (l, t, r, b) = screen();
            let at_edge = match side {
                1 => m.pt.x >= r - 1,
                2 => m.pt.x <= l,
                3 => m.pt.y >= b - 1,
                _ => m.pt.y <= t,
            };
            // A deliberate push (several moves pressed against the edge), not a cursor passing by.
            h.edge_hits = if at_edge { h.edge_hits.saturating_add(1) } else { 0 };
            if h.edge_hits >= 3 && adb::current().is_ok() {
                h.edge_hits = 0;
                h.entry = m.pt;
                h.park = POINT { x: (l + r) / 2, y: (t + b) / 2 };
                let fx = (m.pt.x - l) as f64 / (r - l).max(1) as f64;
                let fy = (m.pt.y - t) as f64 / (b - t).max(1) as f64;
                (h.x, h.y) = match side {
                    1 => (0.02, fy),
                    2 => (0.98, fy),
                    3 => (fx, 0.04),
                    _ => (fx, 0.96),
                };
                h.last_check = Instant::now();
                ACTIVE.store(true, Ordering::SeqCst);
                send(Ev::On(h.x, h.y));
                let _ = SetCursorPos(h.park.x, h.park.y);
                hide_cursor();
                return LRESULT(1);
            }
        }
        return CallNextHookEx(None, code, wp, lp);
    }

    // Active: the cursor stays parked; every event becomes a phone event.
    if h.last_check.elapsed() > Duration::from_secs(1) {
        h.last_check = Instant::now();
        if adb::current().is_err() {
            release();
            return CallNextHookEx(None, code, wp, lp);
        }
    }
    match msg {
        WM_MOUSEMOVE => {
            let (dx, dy) = ((m.pt.x - h.park.x) as f64, (m.pt.y - h.park.y) as f64);
            h.x += dx / SPAN_X;
            h.y += dy / SPAN_Y;
            let back = match side {
                1 => h.x < 0.0,
                2 => h.x > 1.0,
                3 => h.y < 0.0,
                _ => h.y > 1.0,
            };
            if back {
                release();
                return LRESULT(1);
            }
            h.x = h.x.clamp(0.0, 1.0);
            h.y = h.y.clamp(0.0, 1.0);
            send(Ev::Move(h.x, h.y));
        }
        WM_LBUTTONDOWN => send(Ev::Button("down", "left", h.x, h.y)),
        WM_LBUTTONUP => send(Ev::Button("up", "left", h.x, h.y)),
        WM_RBUTTONDOWN => send(Ev::Button("down", "right", h.x, h.y)),
        WM_RBUTTONUP => send(Ev::Button("up", "right", h.x, h.y)),
        WM_MOUSEWHEEL => send(Ev::Wheel(((m.mouseData >> 16) as u16 as i16) as i32, h.x, h.y)),
        _ => {}
    }
    LRESULT(1)
}

/// While the mouse is on the phone the PC pointer is made invisible (it is parked mid-screen).
fn hide_cursor() {
    let and = [0xFFu8; 128];
    let xor = [0u8; 128];
    for id in [OCR_NORMAL, OCR_IBEAM, OCR_HAND, OCR_APPSTARTING, OCR_WAIT] {
        unsafe {
            if let Ok(c) = CreateCursor(None, 0, 0, 32, 32, and.as_ptr().cast(), xor.as_ptr().cast()) {
                let _ = SetSystemCursor(c, id);
            }
        }
    }
}

/// Reloads the user's cursor scheme (also at startup, in case a crash left it hidden).
pub fn show_cursor() {
    unsafe {
        let _ = SystemParametersInfoW(SPI_SETCURSORS, 0, None, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
    }
}

fn down(vk: VIRTUAL_KEY) -> bool {
    unsafe { GetAsyncKeyState(vk.0 as i32) < 0 }
}

unsafe extern "system" fn on_key(code: i32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    if code < 0 || !ACTIVE.load(Ordering::Relaxed) {
        return CallNextHookEx(None, code, wp, lp);
    }
    let k = &*(lp.0 as *const KBDLLHOOKSTRUCT);
    let vk = VIRTUAL_KEY(k.vkCode as u16);
    // The Windows key (and Ctrl+Alt+Del, which no hook sees) always belongs to the PC.
    if vk == VK_LWIN || vk == VK_RWIN || down(VK_LWIN) || down(VK_RWIN) {
        release();
        return CallNextHookEx(None, code, wp, lp);
    }
    let msg = wp.0 as u32;
    if msg != WM_KEYDOWN && msg != WM_SYSKEYDOWN {
        return LRESULT(1);
    }
    let named = match vk {
        VK_RETURN => Some("Enter"),
        VK_BACK => Some("Backspace"),
        VK_DELETE => Some("Delete"),
        VK_TAB => Some("Tab"),
        VK_ESCAPE => Some("Escape"),
        VK_LEFT => Some("ArrowLeft"),
        VK_RIGHT => Some("ArrowRight"),
        VK_UP => Some("ArrowUp"),
        VK_DOWN => Some("ArrowDown"),
        _ => None,
    };
    if let Some(n) = named {
        send(Ev::Key(n.into()));
        return LRESULT(1);
    }
    // Text as the PC's own layout types it (ABNT2 accents, AltGr), without touching its dead-key state.
    let mut state = [0u8; 256];
    for v in [VK_SHIFT, VK_CONTROL, VK_MENU] {
        if down(v) {
            state[v.0 as usize] = 0x80;
        }
    }
    if GetKeyState(VK_CAPITAL.0 as i32) & 1 != 0 {
        state[VK_CAPITAL.0 as usize] = 1;
    }
    let ctrl_only = down(VK_CONTROL) && !down(VK_MENU);
    let mut buf = [0u16; 8];
    let n = ToUnicode(k.vkCode, k.scanCode, Some(&state), &mut buf, 0x4);
    if n > 0 && !ctrl_only {
        let text: String = String::from_utf16_lossy(&buf[..n as usize]).chars().filter(|c| !c.is_control()).collect();
        if !text.is_empty() {
            send(Ev::Text(text));
        }
    }
    LRESULT(1)
}

/// Collapses runs of moves (only the latest position matters) and publishes the rest in order.
fn forward(rx: Receiver<Ev>) {
    let mut pending: Option<(f64, f64)> = None;
    loop {
        let ev = match pending {
            Some(_) => rx.recv_timeout(Duration::from_millis(6)).ok(),
            None => rx.recv().ok(),
        };
        let flush = |p: &mut Option<(f64, f64)>| {
            if let Some((x, y)) = p.take() {
                fast::send(&json!({ "t": "move", "x": x, "y": y }).to_string());
            }
        };
        match ev {
            None => flush(&mut pending),
            Some(Ev::Move(x, y)) => pending = Some((x, y)),
            Some(other) => {
                flush(&mut pending);
                let v = match other {
                    Ev::On(x, y) => json!({ "t": "on", "x": x, "y": y }),
                    Ev::Off => json!({ "t": "off" }),
                    Ev::Button(t, b, x, y) => json!({ "t": t, "button": b, "x": x, "y": y }),
                    Ev::Wheel(d, x, y) => json!({ "t": "wheel", "dy": d, "x": x, "y": y }),
                    Ev::Key(k) => json!({ "t": "key", "key": k }),
                    Ev::Text(s) => json!({ "t": "text", "text": s }),
                    Ev::Move(..) => continue,
                };
                fast::send(&v.to_string());
            }
        }
    }
}

pub fn spawn(side: &str) {
    set_side(side);
    let (tx, rx) = channel();
    let _ = OUT.set(tx);
    thread::spawn(move || forward(rx));
    thread::spawn(|| unsafe {
        show_cursor();
        HOOK = Some(Hook { park: POINT::default(), entry: POINT::default(), x: 0.0, y: 0.0, edge_hits: 0, last_check: Instant::now() });
        let m = SetWindowsHookExW(WH_MOUSE_LL, Some(on_mouse), None, 0);
        let k = SetWindowsHookExW(WH_KEYBOARD_LL, Some(on_key), None, 0);
        if m.is_err() || k.is_err() {
            diag::log("ponte do mouse: hooks recusados pelo Windows");
            return;
        }
        // Low-level hooks are called through this thread's message loop.
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {}
    });
}
