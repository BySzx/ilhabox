//! The USB side. Every 2 s it asks adb which phones are plugged in; for a new one it opens the
//! tunnel (`adb reverse`), installs or updates the IlhaBox app, hands it the key and brings it to
//! the front. It never restarts the adb server: Android Studio shares it, and a restart would cut a
//! debug session of the EasyPac app.

use crate::{bus, config, diag, pc, server::PORT};
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

pub const PACKAGE: &str = "com.ilhabox";
const ACTIVITY: &str = "com.ilhabox/.Main";
const RECEIVER: &str = "com.ilhabox/.Chave";
const OLD_PACKAGE: &str = "dev.alexandre.ilhabox";
/// Version of the APK shipped inside this build; the phone app is replaced when it has an older one.
const APK_VERSION: u32 = parse_u32(include_str!("../../android/versao.txt"));
const TOOLS_URL: &str = "https://dl.google.com/android/repository/platform-tools-latest-windows.zip";

const fn parse_u32(s: &str) -> u32 {
    let b = s.as_bytes();
    let (mut i, mut n) = (0, 0u32);
    while i < b.len() {
        if b[i].is_ascii_digit() {
            n = n * 10 + (b[i] - b'0') as u32;
        }
        i += 1;
    }
    n
}

#[derive(Serialize, Clone, Default, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Phone {
    /// "sem-adb" | "baixando-adb" | "desconectado" | "autorizar" | "instalando" | "conectado" | "erro"
    pub status: String,
    pub serial: String,
    pub modelo: String,
    /// Every EasyPac app on the phone (Supervisor, Monitoramento…).
    pub easypacs: Vec<String>,
    pub app_versao: u32,
    pub erro: String,
}

fn publish(p: &Phone) {
    bus::send("phone", p, true);
}

fn candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();
    for var in ["ANDROID_HOME", "ANDROID_SDK_ROOT"] {
        if let Ok(v) = std::env::var(var) {
            out.push(PathBuf::from(v).join("platform-tools").join("adb.exe"));
        }
    }
    if let Some(d) = dirs::data_local_dir() {
        out.push(d.join("Android").join("Sdk").join("platform-tools").join("adb.exe"));
    }
    if let Ok(p) = std::env::var("PATH") {
        out.extend(std::env::split_paths(&p).map(|d| d.join("adb.exe")));
    }
    for var in ["ProgramFiles(x86)", "ProgramFiles"] {
        if let Ok(v) = std::env::var(var) {
            out.push(PathBuf::from(v).join("Android").join("android-sdk").join("platform-tools").join("adb.exe"));
        }
    }
    out.push(own_tools().join("adb.exe"));
    out
}

fn own_tools() -> PathBuf {
    dirs::data_local_dir().unwrap_or_default().join("IlhaBox").join("platform-tools")
}

/// Prefers the adb of the Android SDK: two different adb versions kill each other's server.
fn find_adb() -> Option<PathBuf> {
    candidates().into_iter().find(|p| p.is_file())
}

/// Official platform-tools from Google, only when the PC has no adb at all.
fn download_tools() -> Result<PathBuf, String> {
    let base = own_tools().parent().map(Path::to_path_buf).ok_or("sem pasta local")?;
    std::fs::create_dir_all(&base).map_err(|e| e.to_string())?;
    let zip = base.join("platform-tools.zip");
    let bytes = tauri::async_runtime::block_on(async {
        let r = reqwest::get(TOOLS_URL).await.map_err(|e| e.to_string())?;
        if !r.status().is_success() {
            return Err(format!("download respondeu {}", r.status()));
        }
        r.bytes().await.map_err(|e| e.to_string())
    })?;
    std::fs::write(&zip, &bytes).map_err(|e| e.to_string())?;
    let script = format!(
        "Expand-Archive -LiteralPath '{}' -DestinationPath '{}' -Force",
        zip.display().to_string().replace('\'', "''"),
        base.display().to_string().replace('\'', "''")
    );
    let (ok, out) = pc::run("powershell", &["-NoProfile", "-NonInteractive", "-Command", &script])?;
    let _ = std::fs::remove_file(&zip);
    let adb = own_tools().join("adb.exe");
    if ok && adb.is_file() { Ok(adb) } else { Err(format!("não consegui extrair: {}", out.trim())) }
}

fn adb(exe: &Path, args: &[&str]) -> Result<String, String> {
    let exe = exe.to_string_lossy().to_string();
    let (ok, out) = pc::run(&exe, args)?;
    if ok { Ok(out) } else { Err(out.trim().to_string()) }
}

fn shell(exe: &Path, serial: &str, cmd: &[&str]) -> Result<String, String> {
    let mut args = vec!["-s", serial, "shell"];
    args.extend_from_slice(cmd);
    adb(exe, &args)
}

/// (serial, state, model) of every phone adb sees.
fn devices(exe: &Path) -> Result<Vec<(String, String, String)>, String> {
    let out = adb(exe, &["devices", "-l"])?;
    Ok(out
        .lines()
        .skip_while(|l| !l.starts_with("List of devices"))
        .skip(1)
        .filter_map(|l| {
            let mut parts = l.split_whitespace();
            let serial = parts.next()?.to_string();
            let state = parts.next()?.to_string();
            let model = l.split_whitespace().find_map(|p| p.strip_prefix("model:")).unwrap_or("").replace('_', " ");
            Some((serial, state, model))
        })
        .collect())
}

fn installed_version(exe: &Path, serial: &str) -> u32 {
    shell(exe, serial, &["dumpsys", "package", PACKAGE])
        .ok()
        .and_then(|out| out.split_whitespace().find_map(|w| w.strip_prefix("versionCode=").and_then(|v| v.parse().ok())))
        .unwrap_or(0)
}

/// EasyPac apps on the phone: the packages set in the settings (comma-separated), or every
/// package with "easypac" in its name.
fn find_easypacs(exe: &Path, serial: &str) -> Vec<String> {
    let set: Vec<String> = config::get().easypac_pacote.split(',').map(|p| p.trim().to_string()).filter(|p| !p.is_empty()).collect();
    if !set.is_empty() {
        return set;
    }
    let mut found: Vec<String> = shell(exe, serial, &["pm", "list", "packages"])
        .map(|out| out.lines().filter_map(|l| l.trim().strip_prefix("package:")).filter(|p| p.to_lowercase().contains("easypac")).map(str::to_string).collect())
        .unwrap_or_default();
    found.sort();
    found
}

fn tunnel(exe: &Path, serial: &str) -> Result<String, String> {
    for p in [PORT, crate::fast::PORT] {
        let port = format!("tcp:{p}");
        adb(exe, &["-s", serial, "reverse", &port, &port])?;
    }
    Ok(String::new())
}

fn give_key(exe: &Path, serial: &str, key: &str) -> Result<String, String> {
    shell(exe, serial, &["am", "broadcast", "-n", RECEIVER, "--es", "k", key])
}

pub fn open_app(exe: &Path, serial: &str) -> Result<String, String> {
    shell(exe, serial, &["am", "start", "-n", ACTIVITY])
}

/// True once per PC version: the phone page loaded before an update still runs the old screens.
fn pc_updated(app: &AppHandle) -> bool {
    let Ok(dir) = app.path().app_data_dir() else { return false };
    let file = dir.join("versao-celular.txt");
    let now = app.package_info().version.to_string();
    if std::fs::read_to_string(&file).is_ok_and(|v| v.trim() == now) {
        return false;
    }
    let _ = std::fs::write(&file, &now);
    true
}

/// First connection of a phone: tunnel, app up to date, key, and (if wanted) the panel in front.
fn setup(app: &AppHandle, exe: &Path, serial: &str, model: &str, key: &str) -> Phone {
    let mut p = Phone { status: "instalando".into(), serial: serial.into(), modelo: model.into(), ..Default::default() };
    publish(&p);
    if let Err(e) = tunnel(exe, serial) {
        return Phone { status: "erro".into(), erro: format!("túnel USB falhou: {e}"), ..p };
    }
    p.app_versao = installed_version(exe, serial);
    if p.app_versao < APK_VERSION {
        let apk = app.path().resolve("ilhabox.apk", tauri::path::BaseDirectory::Resource).ok().filter(|f| f.metadata().is_ok_and(|m| m.len() > 0));
        match apk {
            Some(apk) => {
                diag::log(&format!("instalando app no celular {serial}: {} -> {APK_VERSION}", p.app_versao));
                let path = apk.to_string_lossy().to_string();
                match adb(exe, &["-s", serial, "install", "-r", &path]) {
                    Ok(_) => p.app_versao = APK_VERSION,
                    Err(e) => {
                        diag::log(&format!("instalacao falhou: {e}"));
                        return Phone { status: "erro".into(), erro: format!("não consegui instalar o app: {e}"), ..p };
                    }
                }
            }
            None if p.app_versao == 0 => return Phone { status: "erro".into(), erro: "este build veio sem o app do celular".into(), ..p },
            None => {}
        }
    }
    if shell(exe, serial, &["pm", "list", "packages", OLD_PACKAGE]).is_ok_and(|o| o.lines().any(|l| l.trim() == format!("package:{OLD_PACKAGE}"))) {
        let _ = shell(exe, serial, &["pm", "uninstall", OLD_PACKAGE]);
    }
    if let Err(e) = give_key(exe, serial, key) {
        diag::log(&format!("chave nao entregue: {e}"));
    }
    p.easypacs = find_easypacs(exe, serial);
    if config::get().abrir_ao_conectar {
        let fresh = if pc_updated(app) { shell(exe, serial, &["am", "start", "-S", "-n", ACTIVITY]) } else { open_app(exe, serial) };
        if let Err(e) = fresh {
            diag::log(&format!("nao abriu o app: {e}"));
        }
    }
    diag::log(&format!("celular conectado {serial} modelo={model} app=v{} easypac={:?}", p.app_versao, p.easypacs));
    Phone { status: "conectado".into(), ..p }
}

static ADB: parking_lot::Mutex<Option<(PathBuf, String)>> = parking_lot::Mutex::new(None);

/// adb path and serial of the phone in use, for commands coming from the panel.
pub fn current() -> Result<(PathBuf, String), String> {
    ADB.lock().clone().ok_or_else(|| "nenhum celular conectado".into())
}

pub fn spawn(app: AppHandle, key: String) {
    thread::spawn(move || {
        let mut exe: Option<PathBuf> = None;
        let mut known: HashMap<String, Phone> = HashMap::new();
        let mut last_tunnel = Instant::now();
        let mut last_pub = Phone::default();
        let mut download_failed = false;
        let mut failed_at: HashMap<String, Instant> = HashMap::new();
        loop {
            if exe.as_ref().is_none_or(|e| !e.is_file()) {
                exe = find_adb();
                if exe.is_none() && !download_failed {
                    publish(&Phone { status: "baixando-adb".into(), ..Default::default() });
                    match download_tools() {
                        Ok(p) => exe = Some(p),
                        Err(e) => {
                            diag::log(&format!("adb indisponivel: {e}"));
                            download_failed = true;
                        }
                    }
                }
                if let Some(e) = &exe {
                    diag::log(&format!("adb: {}", e.display()));
                }
            }
            let Some(exe_path) = exe.clone() else {
                let p = Phone { status: "sem-adb".into(), erro: "não achei o adb e o download falhou".into(), ..Default::default() };
                if p != last_pub {
                    publish(&p);
                    last_pub = p;
                }
                thread::sleep(Duration::from_secs(30));
                download_failed = false;
                continue;
            };
            let list = devices(&exe_path).unwrap_or_default();
            let ready: HashSet<String> = list.iter().filter(|d| d.1 == "device").map(|d| d.0.clone()).collect();
            known.retain(|serial, _| ready.contains(serial));
            // A phone whose setup failed (install refused, tunnel error) is tried again every 20 s.
            failed_at.retain(|serial, at| {
                let retry = at.elapsed() > Duration::from_secs(20);
                if retry {
                    known.remove(serial);
                }
                !retry && ready.contains(serial)
            });
            for (serial, state, model) in &list {
                if state == "device" && !known.contains_key(serial) {
                    let p = setup(&app, &exe_path, serial, model, &key);
                    if p.status == "erro" {
                        failed_at.insert(serial.clone(), Instant::now());
                    }
                    known.insert(serial.clone(), p);
                }
            }
            // The tunnel dies with a replug or an adb server restart by another tool: renew it quietly.
            if last_tunnel.elapsed() > Duration::from_secs(10) {
                for serial in known.keys() {
                    let _ = tunnel(&exe_path, serial);
                }
                last_tunnel = Instant::now();
            }
            let unauthorized = list.iter().find(|d| d.1 == "unauthorized");
            let now = if let Some(p) = known.values().find(|p| p.status == "conectado").or_else(|| known.values().next()) {
                p.clone()
            } else if let Some((serial, _, model)) = unauthorized {
                Phone { status: "autorizar".into(), serial: serial.clone(), modelo: model.clone(), ..Default::default() }
            } else {
                Phone { status: "desconectado".into(), ..Default::default() }
            };
            *ADB.lock() = (now.status == "conectado").then(|| (exe_path.clone(), now.serial.clone()));
            if now != last_pub {
                publish(&now);
                last_pub = now;
            }
            thread::sleep(Duration::from_secs(2));
        }
    });
}
