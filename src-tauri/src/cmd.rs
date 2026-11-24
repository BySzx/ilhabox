//! Every action the phone or the settings window can ask for, by name (POST /api/cmd/{name}).

use crate::{adb, config, lyrics, media, pc, update, whats, work};
use serde_json::{json, Value};
use tauri::AppHandle;

fn str_arg(args: &Value, k: &str) -> String {
    args.get(k).and_then(Value::as_str).unwrap_or_default().to_string()
}

fn atalho(id: &str) -> Result<Value, String> {
    let a = config::get().atalhos.into_iter().find(|a| a.id == id).ok_or("atalho não existe")?;
    match a.tipo.as_str() {
        "texto" => {
            pc::set_clipboard(&a.valor)?;
            if a.colar {
                pc::paste();
            }
        }
        "site" if a.valor.starts_with("https://") || a.valor.starts_with("http://") => open::that(&a.valor).map_err(|e| e.to_string())?,
        "programa" => open::that(&a.valor).map_err(|e| e.to_string())?,
        "celular" => return open_phone_app(&a.valor),
        _ => return Err("tipo de atalho inválido".into()),
    }
    Ok(Value::Null)
}

/// Starts an app on the phone through adb (works whatever Android lets apps see of each other).
fn open_phone_app(package: &str) -> Result<Value, String> {
    if package.is_empty() || !package.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_') {
        return Err("pacote inválido".into());
    }
    let (exe, serial) = adb::current()?;
    let (ok, out) = pc::run(
        &exe.to_string_lossy(),
        &["-s", &serial, "shell", "monkey", "-p", package, "-c", "android.intent.category.LAUNCHER", "1"],
    )?;
    if ok && !out.contains("No activities found") { Ok(Value::Null) } else { Err(format!("não abriu {package}")) }
}

pub fn run(app: &AppHandle, name: &str, args: Value) -> Result<Value, String> {
    match name {
        "media" => media::control(&str_arg(&args, "action"), args.get("positionMs").and_then(Value::as_i64)).map(|_| Value::Null),
        "lyrics" => {
            let (artist, title, album) = (str_arg(&args, "artist"), str_arg(&args, "title"), str_arg(&args, "album"));
            let dur = args.get("durationS").and_then(Value::as_u64).unwrap_or(0);
            Ok(json!(tauri::async_runtime::block_on(lyrics::lyrics(&artist, &title, &album, dur))))
        }
        "work_start" => work::start(&args),
        "work_stop" | "work_finish" => work::finish(&args),
        "work_pause" => work::pause(),
        "work_resume" => work::resume(&args),
        "work_update" => work::update(&args),
        "work_add" => work::add(&args),
        "janela_atual" => {
            let (title, url) = crate::watch::foreground();
            let ticket = url
                .as_deref()
                .and_then(crate::watch::parse_ticket_url)
                .map(|n| (n, crate::watch::foreground_subject().unwrap_or_default()))
                .or_else(|| crate::watch::parse_ticket(&title, &config::get().movidesk_palavra));
            Ok(json!({ "title": title, "url": url, "ticket": ticket }))
        }
        "sugestao_fechar" => crate::watch::dismiss(&str_arg(&args, "id")),
        "work_delete" => work::remove(&args),
        "work_sent" => work::mark_sent(&args),
        "copiar" => pc::set_clipboard(&str_arg(&args, "texto")).map(|_| Value::Null),
        "atalho" => atalho(&str_arg(&args, "id")),
        "whats_abrir" => Ok(json!(whats::focus())),
        "whats_limpar" => whats::clear(&str_arg(&args, "chat")),
        "abrir_celular_app" => open_phone_app(&str_arg(&args, "pacote")),
        "celular_home" => {
            let (exe, serial) = adb::current()?;
            pc::run(&exe.to_string_lossy(), &["-s", &serial, "shell", "input", "keyevent", "KEYCODE_HOME"]).map(|_| Value::Null)
        }
        "abrir_ilhabox" => {
            let (exe, serial) = adb::current()?;
            adb::open_app(&exe, &serial).map(|_| Value::Null)
        }
        "config_set" => config::set(args),
        "abrir_painel_pc" => {
            let key = crate::server::KEY.get().ok_or("servidor parado")?;
            open::that(format!("http://127.0.0.1:{}/#k={key}", crate::server::PORT)).map(|_| Value::Null).map_err(|e| e.to_string())
        }
        "update_install" => update::install(app.clone()),
        "update_check" => update::check_now(app.clone()),
        _ => Err(format!("comando desconhecido: {name}")),
    }
}
