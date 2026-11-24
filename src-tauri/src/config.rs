//! Settings shared by the PC window and the phone: shortcuts, the Movidesk text template, the daily
//! goal and how the phone behaves on the cable. Saved in `config.json`; published on "config".

use crate::{bus, diag};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase", default)]
pub struct Atalho {
    pub id: String,
    pub nome: String,
    /// "texto" (copies and pastes on the PC), "site", "programa" (on the PC) or "celular" (opens an app on the phone).
    pub tipo: String,
    pub valor: String,
    /// For "texto": also press Ctrl+V in the window in front.
    pub colar: bool,
    pub cor: String,
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase", default)]
pub struct Config {
    pub atalhos: Vec<Atalho>,
    /// Movidesk text of one period (each period is posted separately). Fields: {atividade} {tipo} {ticket} {descricao} {periodos} {inicio} {fim} {duracao} {horas} {data}.
    pub modelo: String,
    /// Kinds of activity offered when starting a time entry; "Ticket" is the one with a number.
    pub tipos: Vec<String>,
    pub meta_min: u32,
    /// Bring the IlhaBox app to the front when the phone is plugged in.
    pub abrir_ao_conectar: bool,
    /// Package of the EasyPac app on the phone; empty = found by name when the phone connects.
    pub easypac_pacote: String,
    pub inicio_turno: String,
    pub fim_turno: String,
    /// Side of the screens where the phone sits, for the mouse bridge: direita, esquerda, baixo, cima, desligado.
    pub mouse_lado: String,
}

impl Default for Config {
    fn default() -> Self {
        let at = |id: &str, nome: &str, tipo: &str, valor: &str, cor: &str| Atalho {
            id: id.into(),
            nome: nome.into(),
            tipo: tipo.into(),
            valor: valor.into(),
            colar: tipo == "texto",
            cor: cor.into(),
        };
        Self {
            atalhos: vec![
                at("saudacao", "Saudação", "texto", "Olá! Tudo bem? Sou o Alexandre, do suporte da EasyPac. Já estou verificando o seu chamado.", "#34d399"),
                at("aguarde", "Pedir um momento", "texto", "Só um momento, por favor, que estou analisando aqui.", "#60a5fa"),
                at("print", "Pedir print", "texto", "Consegue me enviar um print da tela com a mensagem que aparece?", "#f59e0b"),
                at("encerrar", "Encerramento", "texto", "Ficou alguma dúvida? Se precisar de algo mais, é só chamar. Tenha um ótimo dia!", "#a78bfa"),
            ],
            modelo: "{inicio} às {fim} ({duracao}) · {atividade}\n{descricao}".into(),
            tipos: ["Ticket", "WhatsApp", "Reunião", "Curso", "Dúvida interna", "Outro"].map(String::from).to_vec(),
            meta_min: 8 * 60,
            abrir_ao_conectar: true,
            easypac_pacote: String::new(),
            inicio_turno: "08:00".into(),
            fim_turno: "18:00".into(),
            mouse_lado: "direita".into(),
        }
    }
}

static CFG: Mutex<Option<(PathBuf, Config)>> = Mutex::new(None);

pub fn init(dir: PathBuf) {
    let path = dir.join("config.json");
    let cfg = match fs::read(&path) {
        Ok(b) => serde_json::from_slice(&b).unwrap_or_else(|e| {
            // Never overwrite a file we could not read: keep a copy before falling back to defaults.
            diag::log(&format!("config.json invalido ({e}); copia em config.bad.json"));
            let _ = fs::copy(&path, dir.join("config.bad.json"));
            Config::default()
        }),
        Err(_) => Config::default(),
    };
    bus::send("config", &cfg, true);
    *CFG.lock() = Some((path, cfg));
}

pub fn get() -> Config {
    CFG.lock().as_ref().map(|(_, c)| c.clone()).unwrap_or_default()
}

pub fn set(v: Value) -> Result<Value, String> {
    let cfg: Config = serde_json::from_value(v).map_err(|e| format!("ajustes inválidos: {e}"))?;
    let mut g = CFG.lock();
    let (path, cur) = g.as_mut().ok_or("ajustes não carregados")?;
    let tmp = path.with_extension("json.tmp");
    let bytes = serde_json::to_vec_pretty(&cfg).map_err(|e| e.to_string())?;
    fs::write(&tmp, bytes).and_then(|_| fs::rename(&tmp, &*path)).map_err(|e| e.to_string())?;
    crate::mouse::set_side(&cfg.mouse_lado);
    *cur = cfg.clone();
    bus::send("config", &cfg, true);
    Ok(Value::Null)
}
