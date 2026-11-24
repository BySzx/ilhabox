//! IlhaBox: the work version of the Ilha. Nothing on the PC screen but a tray icon; the panel runs
//! on the test phone over the USB cable. Personal features of the Ilha are not part of this app.

mod adb;
mod bus;
mod cmd;
mod config;
mod diag;
mod lyrics;
mod media;
mod mouse;
mod pc;
mod server;
mod update;
mod whats;
mod work;

use rand::RngCore;
use std::fs;
use std::path::Path;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, WindowEvent};
use tauri_plugin_autostart::ManagerExt;

struct Key(String);

/// The key the phone and the settings window use on the API; made once, kept in the config folder.
fn load_key(dir: &Path) -> String {
    let path = dir.join("box.key");
    if let Ok(k) = fs::read_to_string(&path) {
        let k = k.trim().to_string();
        if k.len() >= 32 {
            return k;
        }
    }
    let mut b = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut b);
    let k: String = b.iter().map(|x| format!("{x:02x}")).collect();
    let _ = fs::write(&path, &k);
    k
}

fn show_settings(app: &AppHandle) {
    if let Some(w) = app.get_webview_window("ajustes") {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
    }
}

#[tauri::command]
fn box_key(key: tauri::State<Key>) -> String {
    key.0.clone()
}

#[tauri::command]
fn autostart(app: AppHandle, on: Option<bool>) -> bool {
    let al = app.autolaunch();
    match on {
        Some(true) => {
            let _ = al.enable();
        }
        Some(false) => {
            let _ = al.disable();
        }
        None => {}
    }
    al.is_enabled().unwrap_or(false)
}

#[tauri::command]
fn quit(app: AppHandle) {
    mouse::show_cursor();
    app.exit(0);
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _, _| show_settings(app)))
        .plugin(tauri_plugin_autostart::init(tauri_plugin_autostart::MacosLauncher::LaunchAgent, None))
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![box_key, autostart, quit])
        .setup(|app| {
            let dir = app.path().app_config_dir()?;
            fs::create_dir_all(&dir)?;
            diag::init(dir.clone());
            diag::log(&format!("INICIO versao={} config={}", app.package_info().version, dir.display()));

            // First run: an agent the user never sees must come back with Windows by itself.
            let marker = dir.join("primeira-vez.ok");
            if !marker.exists() {
                let _ = app.autolaunch().enable();
                let _ = fs::write(&marker, "");
            }

            let key = load_key(&dir);
            app.manage(Key(key.clone()));
            config::init(dir.clone());
            work::init(dir.clone());
            server::spawn(app.handle().clone(), key.clone());
            media::spawn();
            whats::spawn(dir.clone());
            adb::spawn(app.handle().clone(), key);
            mouse::spawn(&config::get().mouse_lado);
            update::spawn(app.handle().clone());

            let open = MenuItem::with_id(app, "open", "Ajustes", true, None::<&str>)?;
            let phone = MenuItem::with_id(app, "phone", "Mostrar no celular", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Sair", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&open, &phone, &quit])?;
            let mut tray = TrayIconBuilder::with_id("ilhabox")
                .tooltip("IlhaBox")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, ev| match ev.id().as_ref() {
                    "open" => show_settings(app),
                    "phone" => {
                        let _ = cmd::run(app, "abrir_ilhabox", serde_json::Value::Null);
                    }
                    "quit" => {
                        mouse::show_cursor();
                        app.exit(0)
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, ev| {
                    if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                        show_settings(tray.app_handle());
                    }
                });
            if let Some(icon) = app.default_window_icon() {
                tray = tray.icon(icon.clone());
            }
            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|w, ev| {
            // Closing the settings only hides them: the agent keeps serving the phone.
            if let WindowEvent::CloseRequested { api, .. } = ev {
                api.prevent_close();
                let _ = w.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("erro ao iniciar a IlhaBox");
}
