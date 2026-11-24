//! Updates from the public BySzx/ilhabox-releases: checked every 30 min and on request; the phone
//! shows a button, one tap downloads, installs and restarts the PC side. The phone app itself is
//! updated by adb when the new PC side sees an older one on the phone.

use crate::{bus, diag};
use serde_json::{json, Value};
use std::time::Duration;
use tauri::AppHandle;
use tauri_plugin_updater::UpdaterExt;

async fn check(app: &AppHandle) -> Result<Option<String>, String> {
    let current = app.package_info().version.to_string();
    let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?;
    let version = update.map(|u| u.version);
    diag::log(&format!("atualizacao verificada atual={current} nova={version:?}"));
    bus::send("update", json!({ "current": current, "version": version, "progress": Value::Null }), true);
    Ok(version)
}

pub fn spawn(app: AppHandle) {
    bus::keep("update", json!({ "current": app.package_info().version.to_string(), "version": Value::Null }));
    tauri::async_runtime::spawn(async move {
        tokio_sleep(Duration::from_secs(15)).await;
        loop {
            let _ = check(&app).await;
            tokio_sleep(Duration::from_secs(30 * 60)).await;
        }
    });
}

async fn tokio_sleep(d: Duration) {
    // tauri's runtime is tokio; a blocking sleep on a spawned blocking task keeps this crate free of a tokio dependency.
    let _ = tauri::async_runtime::spawn_blocking(move || std::thread::sleep(d)).await;
}

pub fn check_now(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::block_on(check(&app)).map(|v| json!({ "version": v }))
}

/// Starts the download in the background; progress goes out on "update" and the app restarts at the end.
pub fn install(app: AppHandle) -> Result<Value, String> {
    tauri::async_runtime::spawn(async move {
        let current = app.package_info().version.to_string();
        let run = async {
            let update = app.updater().map_err(|e| e.to_string())?.check().await.map_err(|e| e.to_string())?.ok_or("já está na versão mais nova")?;
            let version = update.version.clone();
            diag::log(&format!("atualizando para {version}"));
            let mut done = 0usize;
            let (cur, ver) = (current.clone(), version.clone());
            update
                .download_and_install(
                    move |chunk, total| {
                        done += chunk;
                        let p = total.map(|t| done as f64 / t as f64);
                        bus::send("update", json!({ "current": cur, "version": ver, "progress": p }), true);
                    },
                    || {},
                )
                .await
                .map_err(|e| e.to_string())?;
            Ok::<(), String>(())
        };
        match run.await {
            Ok(()) => app.restart(),
            Err(e) => {
                diag::log(&format!("atualizacao falhou: {e}"));
                bus::send("update", json!({ "current": current, "version": Value::Null, "erro": e }), true);
            }
        }
    });
    Ok(Value::Null)
}
