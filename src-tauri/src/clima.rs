//! Weather for the home screen. The same spot is read from six forecasts (ECMWF, NOAA GFS and DWD ICON
//! through Open-Meteo, MET Norway, wttr.in and CPTEC/INPE) and merged: medians for temperatures, the
//! most common condition, and the chance of rain from the sources' own odds plus how many see rain.
//! "Now" comes from the nearest INMET automatic station when there is a fresh reading, and INMET
//! warnings covering the spot are listed. The spot is the one set in Ajustes, or else Windows
//! location, or else the internet address.

use crate::clima_fontes::{self as fontes, Clock, Reading, HOUR};
use crate::{bus, config, diag, pc};
use parking_lot::Mutex;
use reqwest::Client;
use serde_json::{json, Value};
use std::sync::mpsc::{channel, Sender};
use std::sync::OnceLock;
use std::time::Duration;

const EVERY: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, PartialEq)]
struct Place {
    lat: f64,
    lon: f64,
    nome: String,
    origem: &'static str,
}

static KICK: OnceLock<Sender<()>> = OnceLock::new();
static AUTO: Mutex<Option<(Place, i64)>> = Mutex::new(None);

fn http() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| {
        Client::builder()
            .user_agent(concat!("IlhaBox/", env!("CARGO_PKG_VERSION"), " https://github.com/BySzx/ilhabox-releases"))
            .timeout(Duration::from_secs(12))
            .build()
            .unwrap_or_default()
    })
}

async fn get(url: &str) -> Option<Value> {
    http().get(url).send().await.ok()?.error_for_status().ok()?.json().await.ok()
}

pub fn refresh() {
    if let Some(k) = KICK.get() {
        let _ = k.send(());
    }
}

pub fn spawn() {
    let (tx, rx) = channel();
    let _ = KICK.set(tx);
    std::thread::spawn(move || loop {
        tauri::async_runtime::block_on(run());
        let _ = rx.recv_timeout(EVERY);
        while rx.try_recv().is_ok() {}
    });
}

async fn text(url: &str) -> Option<String> {
    let b = http().get(url).send().await.ok()?.error_for_status().ok()?.bytes().await.ok()?;
    Some(String::from_utf8_lossy(&b).into_owned())
}

async fn run() {
    let Some(place) = place().await else {
        bus::keep("clima", json!({ "erro": "não consegui achar sua localização; escolha a cidade em Ajustes" }));
        return;
    };
    let (lat, lon) = (place.lat, place.lon);
    let clock = Clock { now: pc::now_ms(), day: crate::watch::today_at(0) };

    let urls = [
        fontes::open_meteo_url(lat, lon, "ecmwf_ifs025"),
        fontes::open_meteo_url(lat, lon, "gfs_seamless"),
        fontes::open_meteo_url(lat, lon, "icon_seamless"),
        format!("https://api.met.no/weatherapi/locationforecast/2.0/compact?lat={lat:.4}&lon={lon:.4}"),
        format!("https://wttr.in/{lat:.4},{lon:.4}?format=j1"),
    ];
    let jobs = urls.map(|u| tauri::async_runtime::spawn(async move { get(&u).await }));
    let cptec_job = tauri::async_runtime::spawn(text_owned(fontes::cptec_url(lat, lon)));
    let station_job = tauri::async_runtime::spawn(measured(lat, lon, clock));
    let alerts_job = tauri::async_runtime::spawn(alerts(lat, lon, city(&place.nome), clock.now));
    let mut answers = Vec::new();
    for j in jobs {
        answers.push(j.await.ok().flatten());
    }
    let read = |i: usize, f: &dyn Fn(&Value) -> Option<Reading>| answers[i].as_ref().and_then(f);
    let today = fontes::ymd(clock.day + 12 * HOUR);
    let cptec = cptec_job.await.ok().flatten().and_then(|x| fontes::cptec(&x, &today));
    let tried: Vec<(&str, Option<Reading>)> = vec![
        ("ECMWF", read(0, &|v| fontes::open_meteo(v, "ECMWF", clock))),
        ("NOAA GFS", read(1, &|v| fontes::open_meteo(v, "NOAA GFS", clock))),
        ("DWD ICON", read(2, &|v| fontes::open_meteo(v, "DWD ICON", clock))),
        ("MET Norway", read(3, &|v| fontes::met_no(v, clock))),
        ("wttr.in", read(4, &|v| fontes::wttr(v, clock))),
        ("CPTEC/INPE", cptec),
    ];
    let measured = station_job.await.ok().flatten();
    let (alerts, alerts_ok) = alerts_job.await.unwrap_or((Vec::new(), false));

    let status: Vec<Value> = tried
        .iter()
        .map(|(nome, r)| match r {
            Some(r) => json!({ "nome": nome, "ok": true, "temp": r.agora, "min": r.min, "max": r.max, "cond": r.cond }),
            None => json!({ "nome": nome, "ok": false }),
        })
        .chain([
            json!({ "nome": "INMET estação", "ok": measured.is_some() }),
            json!({ "nome": "INMET avisos", "ok": alerts_ok }),
        ])
        .collect();
    let all: Vec<Reading> = tried.into_iter().filter_map(|(_, r)| r).collect();
    diag::log(&format!("clima {} fontes={} medido={} avisos={}", place.nome, all.iter().map(|r| r.fonte).collect::<Vec<_>>().join(","), measured.is_some(), alerts.len()));
    if all.iter().all(|r| r.agora.is_none()) && measured.is_none() {
        bus::keep("clima", json!({ "erro": "nenhuma fonte de clima respondeu", "local": place.nome, "fontes": status }));
        return;
    }
    let mut out = fontes::merge(&all, measured.as_ref(), &alerts, clock);
    out["local"] = json!(place.nome);
    out["origem"] = json!(place.origem);
    out["fontes"] = json!(status);
    bus::keep("clima", out);
}

async fn text_owned(url: String) -> Option<String> {
    text(&url).await
}

fn city(place: &str) -> String {
    place.rsplit(',').next().unwrap_or("").trim().to_string()
}

// ---------------------------------------------------------------- INMET

static STATIONS: Mutex<Option<(Vec<fontes::Station>, i64)>> = Mutex::new(None);

/// Temperature measured now at the nearest INMET automatic station (up to 40 km).
async fn measured(lat: f64, lon: f64, clock: Clock) -> Option<fontes::Measured> {
    let fresh = STATIONS.lock().as_ref().is_some_and(|(l, at)| !l.is_empty() && clock.now - at < 24 * HOUR);
    if !fresh {
        let list = fontes::inmet_stations(&get("https://apitempo.inmet.gov.br/estacoes/T").await?);
        *STATIONS.lock() = Some((list, clock.now));
    }
    let (code, nome, slat, slon, dist) = {
        let g = STATIONS.lock();
        let (st, d) = fontes::nearest(&g.as_ref()?.0, (lat, lon), 40.0)?;
        (st.code.clone(), st.nome.clone(), st.lat, st.lon, d)
    };
    let url = format!("https://apitempo.inmet.gov.br/estacao/{}/{}/{code}", fontes::ymd(clock.now - 24 * HOUR), fontes::ymd(clock.now));
    let st = fontes::Station { code, nome, lat: slat, lon: slon };
    fontes::inmet_measured(&get(&url).await?, &st, dist, clock)
}

static CAPS: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// INMET warnings in force here. The CAP files never change, so each is downloaded once.
async fn alerts(lat: f64, lon: f64, city: String, now: i64) -> (Vec<fontes::Alert>, bool) {
    let Some(rss) = text("https://apiprevmet3.inmet.gov.br/avisos/rss").await else { return (Vec::new(), false) };
    let links = fontes::rss_links(&rss);
    let mut out = Vec::new();
    for link in &links {
        let cached = CAPS.lock().iter().find(|(l, _)| l == link).map(|(_, x)| x.clone());
        let cap = match cached {
            Some(x) => x,
            None => {
                let Some(x) = text(link).await else { continue };
                CAPS.lock().push((link.clone(), x.clone()));
                x
            }
        };
        if let Some(a) = fontes::cap_alert(&cap, (lat, lon), &city, now) {
            out.push(a);
        }
    }
    CAPS.lock().retain(|(l, _)| links.contains(l));
    (out, true)
}

// ---------------------------------------------------------------- location

async fn place() -> Option<Place> {
    let c = config::get();
    if let (Some(lat), Some(lon)) = (c.clima_lat, c.clima_lon) {
        return Some(Place { lat, lon, nome: c.clima_nome.clone(), origem: "manual" });
    }
    if let Some((p, at)) = AUTO.lock().clone() {
        if pc::now_ms() - at < 6 * HOUR {
            return Some(p);
        }
    }
    let p = match windows_location() {
        Some((lat, lon)) => Some(Place { lat, lon, nome: place_name(lat, lon).await.unwrap_or_default(), origem: "windows" }),
        None => ip_location().await,
    }?;
    *AUTO.lock() = Some((p.clone(), pc::now_ms()));
    Some(p)
}

/// Windows location service (Wi-Fi based, neighbourhood-level). Off or denied → None.
#[cfg(windows)]
fn windows_location() -> Option<(f64, f64)> {
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        let read = || -> windows::core::Result<(f64, f64)> {
            use windows::Devices::Geolocation::Geolocator;
            let p = Geolocator::new()?.GetGeopositionAsync()?.join()?.Coordinate()?.Point()?.Position()?;
            Ok((p.Latitude, p.Longitude))
        };
        let _ = tx.send(read());
    });
    match rx.recv_timeout(Duration::from_secs(15)) {
        Ok(Ok(p)) => Some(p),
        Ok(Err(e)) => {
            diag::log(&format!("localizacao do windows indisponivel: {e}"));
            None
        }
        Err(_) => None,
    }
}

#[cfg(not(windows))]
fn windows_location() -> Option<(f64, f64)> {
    None
}

async fn ip_location() -> Option<Place> {
    let v = get("https://ipwho.is/").await?;
    let (lat, lon) = (v["latitude"].as_f64()?, v["longitude"].as_f64()?);
    Some(Place { lat, lon, nome: v["city"].as_str().unwrap_or("").into(), origem: "internet" })
}

async fn place_name(lat: f64, lon: f64) -> Option<String> {
    let v = get(&format!("https://nominatim.openstreetmap.org/reverse?lat={lat:.4}&lon={lon:.4}&format=json&zoom=14&accept-language=pt-BR")).await?;
    let a = &v["address"];
    let bairro = ["suburb", "neighbourhood", "quarter"].iter().find_map(|k| a[k].as_str());
    let cidade = ["city", "town", "village", "municipality"].iter().find_map(|k| a[k].as_str());
    match (bairro, cidade) {
        (Some(b), Some(c)) => Some(format!("{b}, {c}")),
        (None, Some(c)) => Some(c.into()),
        (Some(b), None) => Some(b.into()),
        _ => None,
    }
}

/// City search for the settings (Open-Meteo geocoding).
pub fn search(text: &str) -> Result<Value, String> {
    let url = reqwest::Url::parse_with_params("https://geocoding-api.open-meteo.com/v1/search", &[("name", text), ("count", "6"), ("language", "pt")]).map_err(|e| e.to_string())?;
    let v = tauri::async_runtime::block_on(get(url.as_str())).ok_or("busca de cidade sem resposta")?;
    let list: Vec<Value> = v["results"]
        .as_array()
        .map(|l| {
            l.iter()
                .map(|r| {
                    let parts: Vec<&str> = ["name", "admin1", "country"].iter().filter_map(|k| r[k].as_str()).collect();
                    json!({ "nome": parts.join(", "), "lat": r["latitude"], "lon": r["longitude"] })
                })
                .collect()
        })
        .unwrap_or_default();
    Ok(json!(list))
}

