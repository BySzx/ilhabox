//! The weather sources' answers read into one shape, and merged. No I/O here: `now` and the start of
//! the local day come in from the caller.

use serde_json::{json, Value};

pub const HOUR: i64 = 3_600_000;

/// One forecast. Daily-only sources (CPTEC) leave the "now" fields empty.
pub struct Reading {
    pub fonte: &'static str,
    pub agora: Option<f64>,
    pub sensacao: Option<f64>,
    pub umidade: Option<f64>,
    pub vento: Option<f64>,
    pub min: Option<f64>,
    pub max: Option<f64>,
    pub cond: Option<&'static str>,
    pub dia: bool,
    pub chuva_mm: Option<f64>,
    pub prob: Option<f64>,
    pub horas: Vec<(i64, f64, f64)>,
}

impl Reading {
    pub fn copy(&self) -> Self {
        Reading { horas: self.horas.clone(), ..*self }
    }

    fn daily(fonte: &'static str, min: f64, max: f64) -> Self {
        Reading { fonte, agora: None, sensacao: None, umidade: None, vento: None, min: Some(min), max: Some(max), cond: None, dia: true, chuva_mm: None, prob: None, horas: Vec::new() }
    }
}

/// What a nearby INMET station measured.
pub struct Measured {
    pub estacao: String,
    pub km: f64,
    pub temp: f64,
    pub umidade: Option<f64>,
    pub vento: Option<f64>,
    pub chuva: Option<f64>,
    pub em: i64,
}

pub struct Alert {
    pub evento: String,
    pub grau: String,
    pub ate: Option<i64>,
    pub texto: String,
}

#[derive(Clone, Copy)]
pub struct Clock {
    pub now: i64,
    pub day: i64,
}

impl Clock {
    fn today(self) -> (i64, i64) {
        (self.day, self.day + 24 * HOUR)
    }
    fn next3(self, t: i64) -> bool {
        t + HOUR > self.now && t < self.now + 3 * HOUR
    }
}

fn min_max(clock: Clock, points: impl Iterator<Item = (i64, f64)>) -> Option<(f64, f64)> {
    let (a, b) = clock.today();
    points.filter(|(t, _)| *t >= a && *t < b).fold(None, |acc, (_, v)| Some(acc.map_or((v, v), |(lo, hi): (f64, f64)| (lo.min(v), hi.max(v)))))
}

// ---------------------------------------------------------------- dates

fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    era * 146_097 + yoe * 365 + yoe / 4 - yoe / 100 + doy - 719_468
}

/// "2026-11-25T12:00:00Z" (or with "-03:00", or a space instead of T) → ms.
pub fn iso_ms(s: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d, hh, mi) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13).unwrap_or(0), n(14..16).unwrap_or(0));
    let mut ms = ((days_from_civil(y, m, d) * 24 + hh) * 60 + mi) * 60_000;
    let tail = s.get(19..).unwrap_or("");
    if let Some(i) = tail.find(['+', '-']) {
        let sign = if tail[i..].starts_with('-') { -1 } else { 1 };
        let oh = tail.get(i + 1..i + 3).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
        let om = tail.get(i + 4..i + 6).and_then(|x| x.parse::<i64>().ok()).unwrap_or(0);
        ms -= sign * (oh * 60 + om) * 60_000;
    }
    Some(ms)
}

/// ms → "2026-11-25" (UTC).
pub fn ymd(ms: i64) -> String {
    let z = ms.div_euclid(86_400_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

// ---------------------------------------------------------------- tiny XML

fn unescape(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix("<![CDATA[").and_then(|x| x.strip_suffix("]]>")).unwrap_or(s);
    s.replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&#39;", "'").replace("&apos;", "'").replace("&amp;", "&")
}

/// Inner text of every <name …>…</name>, in order (no nesting of the same name).
pub fn tags<'a>(xml: &'a str, name: &str) -> Vec<&'a str> {
    let (open, close) = (format!("<{name}"), format!("</{name}>"));
    let mut out = Vec::new();
    let mut rest = xml;
    while let Some(i) = rest.find(&open) {
        let after = &rest[i + open.len()..];
        if !after.starts_with(['>', ' ', '\n', '\t', '\r']) {
            rest = after;
            continue;
        }
        let Some(gt) = after.find('>') else { break };
        let body = &after[gt + 1..];
        let Some(end) = body.find(&close) else { break };
        out.push(&body[..end]);
        rest = &body[end + close.len()..];
    }
    out
}

pub fn tag(xml: &str, name: &str) -> Option<String> {
    tags(xml, name).first().map(|s| unescape(s))
}

fn html_text(s: &str) -> String {
    let mut out = String::new();
    let mut inside = false;
    for c in unescape(s).chars() {
        match c {
            '<' => inside = true,
            '>' => {
                inside = false;
                out.push(' ');
            }
            _ if !inside => out.push(c),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

// ---------------------------------------------------------------- Open-Meteo (ECMWF, GFS, ICON)

fn wmo(code: i64) -> &'static str {
    match code {
        0 | 1 => "sol",
        2 => "parcial",
        3 => "nublado",
        45 | 48 => "neblina",
        95..=99 => "tempestade",
        51..=86 => "chuva",
        _ => "nublado",
    }
}

pub fn open_meteo_url(lat: f64, lon: f64, model: &str) -> String {
    format!(
        "https://api.open-meteo.com/v1/forecast?latitude={lat:.4}&longitude={lon:.4}&models={model}\
         &current=temperature_2m,apparent_temperature,relative_humidity_2m,weather_code,is_day,wind_speed_10m\
         &hourly=temperature_2m,precipitation,precipitation_probability&forecast_days=2&timeformat=unixtime&timezone=auto"
    )
}

pub fn open_meteo(v: &Value, fonte: &'static str, clock: Clock) -> Option<Reading> {
    let cur = &v["current"];
    let h = &v["hourly"];
    let times: Vec<i64> = h["time"].as_array()?.iter().filter_map(|t| t.as_i64().map(|s| s * 1000)).collect();
    let col = |k: &str| -> Vec<Option<f64>> { h[k].as_array().map(|a| a.iter().map(Value::as_f64).collect()).unwrap_or_default() };
    let (temp, mm, prob) = (col("temperature_2m"), col("precipitation"), col("precipitation_probability"));
    let at = |i: usize, c: &Vec<Option<f64>>| c.get(i).copied().flatten();
    let next3 = || times.iter().enumerate().filter(|(_, t)| clock.next3(**t)).map(|(i, _)| i);
    let (min, max) = min_max(clock, times.iter().enumerate().filter_map(|(i, t)| at(i, &temp).map(|v| (*t, v))))?;
    Some(Reading {
        fonte,
        agora: Some(cur["temperature_2m"].as_f64()?),
        sensacao: cur["apparent_temperature"].as_f64(),
        umidade: cur["relative_humidity_2m"].as_f64(),
        vento: cur["wind_speed_10m"].as_f64(),
        min: Some(min),
        max: Some(max),
        cond: Some(wmo(cur["weather_code"].as_i64()?)),
        dia: cur["is_day"].as_i64() != Some(0),
        chuva_mm: Some(next3().filter_map(|i| at(i, &mm)).sum()),
        prob: next3().filter_map(|i| at(i, &prob)).reduce(f64::max),
        horas: times.iter().enumerate().filter_map(|(i, t)| Some((*t, at(i, &temp)?, at(i, &mm).unwrap_or(0.0)))).collect(),
    })
}

// ---------------------------------------------------------------- MET Norway

fn symbol(code: &str) -> &'static str {
    match code {
        c if c.contains("thunder") => "tempestade",
        c if c.contains("rain") || c.contains("sleet") || c.contains("snow") => "chuva",
        c if c.contains("fog") => "neblina",
        c if c.starts_with("partlycloudy") => "parcial",
        c if c.starts_with("cloudy") => "nublado",
        _ => "sol",
    }
}

pub fn met_no(v: &Value, clock: Clock) -> Option<Reading> {
    let now = clock.now;
    let series: Vec<(i64, &Value)> = v["properties"]["timeseries"].as_array()?.iter().filter_map(|e| Some((iso_ms(e["time"].as_str()?)?, &e["data"]))).collect();
    let (_, cur) = series.iter().rev().find(|(t, _)| *t <= now).or(series.first())?;
    let details = &cur["instant"]["details"];
    let temp = |d: &Value| d["instant"]["details"]["air_temperature"].as_f64();
    let rain = |d: &Value| d["next_1_hours"]["details"]["precipitation_amount"].as_f64();
    let code = cur["next_1_hours"]["summary"]["symbol_code"].as_str().or(cur["next_6_hours"]["summary"]["symbol_code"].as_str()).unwrap_or("");
    let (min, max) = min_max(clock, series.iter().filter_map(|(t, d)| Some((*t, temp(d)?))))?;
    Some(Reading {
        fonte: "MET Norway",
        agora: Some(temp(cur)?),
        sensacao: None,
        umidade: details["relative_humidity"].as_f64(),
        vento: details["wind_speed"].as_f64().map(|ms| ms * 3.6),
        min: Some(min),
        max: Some(max),
        cond: Some(symbol(code)),
        dia: !code.ends_with("_night"),
        chuva_mm: Some(series.iter().filter(|(t, _)| clock.next3(*t)).filter_map(|(_, d)| rain(d)).sum()),
        prob: None,
        horas: series.iter().filter_map(|(t, d)| Some((*t, temp(d)?, rain(d).unwrap_or(0.0)))).collect(),
    })
}

// ---------------------------------------------------------------- wttr.in

fn wwo(code: i64) -> &'static str {
    match code {
        113 => "sol",
        116 => "parcial",
        119 | 122 => "nublado",
        143 | 248 | 260 => "neblina",
        200 | 386 | 389 | 392 | 395 => "tempestade",
        _ => "chuva",
    }
}

fn num(x: &Value) -> Option<f64> {
    x.as_f64().or_else(|| x.as_str().and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok()))
}

pub fn wttr(v: &Value, clock: Clock) -> Option<Reading> {
    let cur = &v["current_condition"][0];
    let day = &v["weather"][0];
    let now = clock.now;
    let hours: Vec<(i64, &Value)> = day["hourly"].as_array()?.iter().filter_map(|h| Some((clock.day + num(&h["time"])? as i64 / 100 * HOUR, h))).collect();
    let slot = hours.iter().rev().find(|(t, _)| *t <= now).map(|(_, h)| *h);
    Some(Reading {
        fonte: "wttr.in",
        agora: Some(num(&cur["temp_C"])?),
        sensacao: num(&cur["FeelsLikeC"]),
        umidade: num(&cur["humidity"]),
        vento: num(&cur["windspeedKmph"]),
        min: num(&day["mintempC"]),
        max: num(&day["maxtempC"]),
        cond: Some(wwo(num(&cur["weatherCode"])? as i64)),
        dia: true,
        chuva_mm: Some(slot.and_then(|h| num(&h["precipMM"])).unwrap_or(0.0)),
        prob: slot.and_then(|h| num(&h["chanceofrain"])),
        horas: hours.iter().filter_map(|(t, h)| Some((*t, num(&h["tempC"])?, num(&h["precipMM"]).unwrap_or(0.0) / 3.0))).collect(),
    })
}

// ---------------------------------------------------------------- CPTEC/INPE (daily)

pub fn cptec_url(lat: f64, lon: f64) -> String {
    format!("http://servicos.cptec.inpe.br/XML/cidade/7dias/{lat:.2}/{lon:.2}/previsaoLatLon.xml")
}

/// Today's minimum and maximum from the 7-day XML.
pub fn cptec(xml: &str, today: &str) -> Option<Reading> {
    let days = tags(xml, "previsao");
    let day = days.iter().find(|d| tag(d, "dia").as_deref() == Some(today)).or(days.first())?;
    let n = |k: &str| tag(day, k).and_then(|s| s.trim().replace(',', ".").parse::<f64>().ok());
    Some(Reading::daily("CPTEC/INPE", n("minima")?, n("maxima")?))
}

// ---------------------------------------------------------------- INMET stations (measured)

pub struct Station {
    pub code: String,
    pub nome: String,
    pub lat: f64,
    pub lon: f64,
}

pub fn km(a: (f64, f64), b: (f64, f64)) -> f64 {
    let (la1, la2) = (a.0.to_radians(), b.0.to_radians());
    let (dla, dlo) = ((b.0 - a.0).to_radians(), (b.1 - a.1).to_radians());
    let h = (dla / 2.0).sin().powi(2) + la1.cos() * la2.cos() * (dlo / 2.0).sin().powi(2);
    6371.0 * 2.0 * h.sqrt().asin()
}

/// Automatic stations that are working, from apitempo /estacoes/T.
pub fn inmet_stations(v: &Value) -> Vec<Station> {
    let s = |x: &Value| x.as_str().map(|s| s.trim().to_string());
    v.as_array()
        .map(|l| {
            l.iter()
                .filter(|e| s(&e["CD_SITUACAO"]).is_none_or(|x| x.to_lowercase().starts_with("operante")))
                .filter_map(|e| Some(Station { code: s(&e["CD_ESTACAO"])?, nome: s(&e["DC_NOME"]).unwrap_or_default(), lat: num(&e["VL_LATITUDE"])?, lon: num(&e["VL_LONGITUDE"])? }))
                .collect()
        })
        .unwrap_or_default()
}

pub fn nearest(list: &[Station], at: (f64, f64), max_km: f64) -> Option<(&Station, f64)> {
    list.iter().map(|s| (s, km(at, (s.lat, s.lon)))).filter(|(_, d)| *d <= max_km).min_by(|a, b| a.1.total_cmp(&b.1))
}

/// Latest hourly record with a temperature, from apitempo /estacao/{start}/{end}/{code}. Hours are UTC.
pub fn inmet_measured(v: &Value, st: &Station, dist: f64, clock: Clock) -> Option<Measured> {
    let rows = v.as_array()?;
    rows.iter()
        .filter_map(|r| {
            let hr = r["HR_MEDICAO"].as_str()?.trim();
            let hh: i64 = hr.get(..2).unwrap_or(hr).parse().ok()?;
            let at = iso_ms(r["DT_MEDICAO"].as_str()?)? + hh * HOUR;
            Some((at, r, num(&r["TEM_INS"])?))
        })
        .filter(|(at, _, _)| *at <= clock.now + HOUR && clock.now - *at <= 3 * HOUR)
        .max_by_key(|(at, _, _)| *at)
        .map(|(at, r, temp)| Measured {
            estacao: st.nome.clone(),
            km: dist,
            temp,
            umidade: num(&r["UMD_INS"]),
            vento: num(&r["VEN_VEL"]).map(|ms| ms * 3.6),
            chuva: num(&r["CHUVA"]),
            em: at,
        })
}

// ---------------------------------------------------------------- INMET warnings (CAP)

/// Links of the warnings in the INMET RSS feed.
pub fn rss_links(xml: &str) -> Vec<String> {
    tags(xml, "item").iter().filter_map(|i| tag(i, "link").or_else(|| tag(i, "guid"))).filter(|l| l.starts_with("http")).collect()
}

fn inside(poly: &[(f64, f64)], p: (f64, f64)) -> bool {
    let mut c = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a.1 > p.1) != (b.1 > p.1) && p.0 < (b.0 - a.0) * (p.1 - a.1) / (b.1 - a.1) + a.0 {
            c = !c;
        }
    }
    c
}

/// A CAP warning that covers this point (by polygon, or by the city name in the area text) and is still on.
pub fn cap_alert(xml: &str, at: (f64, f64), city: &str, now: i64) -> Option<Alert> {
    let info = tags(xml, "info").into_iter().next().unwrap_or(xml);
    let ate = tag(info, "expires").and_then(|s| iso_ms(&s));
    if ate.is_some_and(|t| t < now) {
        return None;
    }
    let areas = tags(info, "area");
    let by_polygon = areas.iter().flat_map(|a| tags(a, "polygon")).any(|p| {
        let pts: Vec<(f64, f64)> = p.split_whitespace().filter_map(|pair| {
            let (la, lo) = pair.split_once(',')?;
            Some((la.trim().parse().ok()?, lo.trim().parse().ok()?))
        }).collect();
        pts.len() >= 3 && inside(&pts, at)
    });
    let fold = |s: &str| s.to_lowercase();
    let by_name = !city.is_empty() && areas.iter().any(|a| tag(a, "areaDesc").is_some_and(|d| fold(&d).contains(&fold(city))));
    if !by_polygon && !by_name {
        return None;
    }
    let grau = match tag(info, "severity").unwrap_or_default().as_str() {
        "Extreme" => "Grande perigo",
        "Severe" => "Perigo",
        _ => "Perigo potencial",
    };
    Some(Alert {
        evento: tag(info, "event").unwrap_or_else(|| "Aviso".into()),
        grau: grau.into(),
        ate,
        texto: tag(info, "description").map(|d| html_text(&d)).unwrap_or_default(),
    })
}

// ---------------------------------------------------------------- merge

pub fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let m = v.len() / 2;
    Some(if v.len().is_multiple_of(2) { (v[m - 1] + v[m]) / 2.0 } else { v[m] })
}

const SEVERITY: [&str; 6] = ["sol", "parcial", "nublado", "neblina", "chuva", "tempestade"];

fn round1(x: f64) -> f64 {
    (x * 10.0).round() / 10.0
}

/// Everything the phone shows except where it is and the status of each source (the caller adds those).
pub fn merge(all: &[Reading], measured: Option<&Measured>, alerts: &[Alert], clock: Clock) -> Value {
    let med = |f: &dyn Fn(&Reading) -> Option<f64>| median(all.iter().filter_map(f).collect());

    let mut votes = [0usize; SEVERITY.len()];
    for c in all.iter().filter_map(|r| r.cond) {
        if let Some(i) = SEVERITY.iter().position(|s| *s == c) {
            votes[i] += 1;
        }
    }
    let top = votes.iter().copied().max().unwrap_or(0);
    let cond = SEVERITY.iter().zip(votes).rev().find(|(_, n)| *n == top && top > 0).map_or("nublado", |(c, _)| *c);

    let rain: Vec<f64> = all.iter().filter_map(|r| r.chuva_mm).collect();
    let mut odds: Vec<f64> = all.iter().filter_map(|r| r.prob).collect();
    if !rain.is_empty() {
        odds.push(rain.iter().filter(|mm| **mm >= 0.2).count() as f64 / rain.len() as f64 * 100.0);
    }
    if measured.and_then(|m| m.chuva).is_some_and(|mm| mm >= 0.2) {
        odds.push(100.0);
    }
    let prob = if odds.is_empty() { 0.0 } else { (odds.iter().sum::<f64>() / odds.len() as f64).round() };

    let temps: Vec<f64> = all.iter().filter_map(|r| r.agora).collect();
    let spread = if temps.is_empty() { 0.0 } else { temps.iter().copied().fold(f64::MIN, f64::max) - temps.iter().copied().fold(f64::MAX, f64::min) };

    let first = (clock.now / HOUR + 1) * HOUR;
    let horas: Vec<Value> = (0..12)
        .map(|k| first + k * HOUR)
        .filter_map(|t| {
            let at = |r: &Reading| r.horas.iter().find(|h| h.0 == t).copied();
            let temp = median(all.iter().filter_map(|r| at(r).map(|h| h.1)).collect())?;
            let mm = median(all.iter().filter_map(|r| at(r).map(|h| h.2)).collect()).unwrap_or(0.0);
            Some(json!({ "t": t, "temp": round1(temp), "mm": round1(mm) }))
        })
        .collect();

    let forecast_now = med(&|r| r.agora);
    let now_temp = measured.map(|m| m.temp).or(forecast_now);
    let with_measured = |f: Option<f64>, m: Option<f64>| m.or(f).map(f64::round);
    let dia_votes: Vec<bool> = all.iter().filter(|r| r.agora.is_some()).map(|r| r.dia).collect();

    json!({
        "temp": now_temp.map(round1),
        "previsto": forecast_now.map(round1),
        "sensacao": med(&|r| r.sensacao).map(round1),
        "umidade": with_measured(med(&|r| r.umidade), measured.and_then(|m| m.umidade)),
        "vento": with_measured(med(&|r| r.vento), measured.and_then(|m| m.vento)),
        "min": med(&|r| r.min).map(|v| round1(now_temp.map_or(v, |t| v.min(t)))),
        "max": med(&|r| r.max).map(|v| round1(now_temp.map_or(v, |t| v.max(t)))),
        "cond": cond,
        "dia": dia_votes.is_empty() || dia_votes.iter().filter(|d| **d).count() * 2 >= dia_votes.len(),
        "chuva": prob,
        "mm": median(rain).map(round1),
        "diferenca": round1(spread),
        "horas": horas,
        "medido": measured.map(|m| json!({ "estacao": m.estacao, "km": m.km.round(), "temp": round1(m.temp), "em": m.em })),
        "alertas": alerts.iter().map(|a| json!({ "evento": a.evento, "grau": a.grau, "ate": a.ate, "texto": a.texto })).collect::<Vec<_>>(),
        "em": clock.now,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates() {
        assert_eq!(iso_ms("1970-01-02T00:00:00Z"), Some(86_400_000));
        assert_eq!(iso_ms("2026-11-25T12:00:00Z"), Some(1_795_608_000_000));
        assert_eq!(iso_ms("2026-11-25T09:00:00-03:00"), Some(1_795_608_000_000));
        assert_eq!(iso_ms("2026-11-25"), Some(1_795_564_800_000));
        assert_eq!(ymd(1_795_608_000_000), "2026-11-25");
        assert_eq!(ymd(951_782_400_000), "2000-02-29");
    }

    #[test]
    fn medians() {
        assert_eq!(median(vec![3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(median(vec![4.0, 1.0, 2.0, 3.0]), Some(2.5));
    }

    #[test]
    fn xml() {
        let x = r#"<cidade><nome>Cidade</nome><previsao><dia>2026-11-24</dia><tempo>pn</tempo><maxima>30</maxima><minima>18</minima></previsao><previsao><dia>2026-11-25</dia><tempo>pt</tempo><maxima>29</maxima><minima>19</minima></previsao></cidade>"#;
        let r = cptec(x, "2026-11-25").unwrap();
        assert_eq!((r.min, r.max), (Some(19.0), Some(29.0)));
        assert_eq!(tag("<a><b x=\"1\"><![CDATA[oi &amp; tchau]]></b></a>", "b").as_deref(), Some("oi & tchau"));
        assert_eq!(tags("<item><link>a</link></item><items/><item><link>b</link></item>", "item").len(), 2);
    }

    #[test]
    fn stations() {
        let v: Value = serde_json::from_str(r#"[{"CD_ESTACAO":"A701","DC_NOME":"SAO PAULO - MIRANTE","VL_LATITUDE":"-23.4963","VL_LONGITUDE":"-46.6200","CD_SITUACAO":"Operante"},{"CD_ESTACAO":"A771","DC_NOME":"SAO PAULO - INTERLAGOS","VL_LATITUDE":-23.7244,"VL_LONGITUDE":-46.6775,"CD_SITUACAO":"Pane"}]"#).unwrap();
        let l = inmet_stations(&v);
        assert_eq!(l.len(), 1);
        let (s, d) = nearest(&l, (-23.55, -46.63), 40.0).unwrap();
        assert_eq!(s.code, "A701");
        assert!((5.0..7.0).contains(&d));
        let now = iso_ms("2026-11-25T15:40:00Z").unwrap();
        let rows: Value = serde_json::from_str(r#"[{"DT_MEDICAO":"2026-11-25","HR_MEDICAO":"1400","TEM_INS":"25.1","UMD_INS":"70","VEN_VEL":"2.0","CHUVA":"0"},{"DT_MEDICAO":"2026-11-25","HR_MEDICAO":"1500","TEM_INS":"26.3","UMD_INS":"66","VEN_VEL":"3.0","CHUVA":"0.4"},{"DT_MEDICAO":"2026-11-25","HR_MEDICAO":"1600","TEM_INS":null}]"#).unwrap();
        let m = inmet_measured(&rows, s, d, Clock { now, day: 0 }).unwrap();
        assert_eq!(m.temp, 26.3);
        assert_eq!(m.em, iso_ms("2026-11-25T15:00:00Z").unwrap());
    }

    #[test]
    fn warnings() {
        let cap = r#"<alert><info><event>Tempestade</event><severity>Severe</severity><expires>2099-01-01T00:00:00-03:00</expires><description>Chuva entre 30 e 60 mm/h.</description><area><areaDesc>Norte Pioneiro Paranaense</areaDesc><polygon>-23.0,-51.0 -23.0,-50.0 -24.0,-50.0 -24.0,-51.0 -23.0,-51.0</polygon></area></info></alert>"#;
        let a = cap_alert(cap, (-23.5, -50.5), "", 0).unwrap();
        assert_eq!((a.evento.as_str(), a.grau.as_str()), ("Tempestade", "Perigo"));
        assert!(cap_alert(cap, (-25.4, -49.3), "Curitiba", 0).is_none());
        assert!(cap_alert(cap, (-25.4, -49.3), "pioneiro", 0).is_some());
        assert!(cap_alert(cap.replace("2099", "2001").as_str(), (-23.5, -50.5), "", 1_000_000_000_000_000).is_none());
    }
}
