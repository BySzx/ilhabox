//! Plain-text journal of what the app loaded and saved, in the config folder (`diag.log`).
//! It exists to answer "why did my settings vanish?" with facts instead of guesses.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

static DIR: OnceLock<PathBuf> = OnceLock::new();
const MAX: u64 = 512 * 1024;

pub fn init(dir: PathBuf) {
    let _ = DIR.set(dir);
}

pub fn log(msg: &str) {
    let Some(dir) = DIR.get() else { return };
    let path = dir.join("diag.log");
    if fs::metadata(&path).map(|m| m.len() > MAX).unwrap_or(false) {
        let _ = fs::rename(&path, dir.join("diag.old.log"));
    }
    let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0);
    if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
        let _ = writeln!(f, "{secs} pid={} {msg}", std::process::id());
    }
}
