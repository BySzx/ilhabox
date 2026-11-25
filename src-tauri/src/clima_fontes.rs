//! The weather sources' answers read into one shape, and merged. No I/O here: `now` and the start of
//! the local day come in from the caller.

use serde_json::{json, Value};

pub const HOUR: i64 = 3_600_000;

pub struct Reading {
    pub fonte: &'static str,
    pub agora: f64,
    pub sensacao: Option<f64>,
    pub umidade: Option<f64>,
    pub vento: Option<f64>,
    pub min: f64,
    pub max: f64,
    pub cond: &'static str,
    pub dia: bool,
    pub chuva_mm: f64,
    pub prob: Option<f64>,
    pub horas: Vec<(i64, f64, f64)>,
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

fn min_max(clock: Clock, points: impl Iterator<Item = (i64, f64)>) -> Option<(f64, f64)> {
    let (a, b) = clock.today();
    points.filter(|(t, _)| *t >= a && *t < b).fold(None, |acc, (_, v)| Some(acc.map_or((v, v), |(lo, hi): (f64, f64)| (lo.min(v), hi.max(v)))))
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
        agora: cur["temperature_2m"].as_f64()?,
        sensacao: cur["apparent_temperature"].as_f64(),
        umidade: cur["relative_humidity_2m"].as_f64(),
        vento: cur["wind_speed_10m"].as_f64(),
        min,
        max,
        cond: wmo(cur["weather_code"].as_i64()?),
        dia: cur["is_day"].as_i64() != Some(0),
        chuva_mm: next3().filter_map(|i| at(i, &mm)).sum(),
        prob: next3().filter_map(|i| at(i, &prob)).reduce(f64::max),
        horas: times.iter().enumerate().filter_map(|(i, t)| Some((*t, at(i, &temp)?, at(i, &mm).unwrap_or(0.0)))).collect(),
    })
}

/// "2026-11-25T12:00:00Z" → ms.
fn iso_ms(s: &str) -> Option<i64> {
    let n = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, m, d, hh, mi) = (n(0..4)?, n(5..7)?, n(8..10)?, n(11..13)?, n(14..16)?);
    let (y2, m2) = if m <= 2 { (y - 1, m + 9) } else { (y, m - 3) };
    let era = y2.div_euclid(400);
    let yoe = y2 - era * 400;
    let doy = (153 * m2 + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(((days * 24 + hh) * 60 + mi) * 60_000)
}

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
        agora: temp(cur)?,
        sensacao: None,
        umidade: details["relative_humidity"].as_f64(),
        vento: details["wind_speed"].as_f64().map(|ms| ms * 3.6),
        min,
        max,
        cond: symbol(code),
        dia: !code.ends_with("_night"),
        chuva_mm: series.iter().filter(|(t, _)| clock.next3(*t)).filter_map(|(_, d)| rain(d)).sum(),
        prob: None,
        horas: series.iter().filter_map(|(t, d)| Some((*t, temp(d)?, rain(d).unwrap_or(0.0)))).collect(),
    })
}

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

pub fn wttr(v: &Value, clock: Clock) -> Option<Reading> {
    let num = |x: &Value| x.as_str().and_then(|s| s.trim().parse::<f64>().ok());
    let cur = &v["current_condition"][0];
    let day = &v["weather"][0];
    let now = clock.now;
    let hours: Vec<(i64, &Value)> = day["hourly"].as_array()?.iter().filter_map(|h| Some((clock.day + num(&h["time"])? as i64 / 100 * HOUR, h))).collect();
    let slot = hours.iter().rev().find(|(t, _)| *t <= now).map(|(_, h)| *h);
    Some(Reading {
        fonte: "wttr.in",
        agora: num(&cur["temp_C"])?,
        sensacao: num(&cur["FeelsLikeC"]),
        umidade: num(&cur["humidity"]),
        vento: num(&cur["windspeedKmph"]),
        min: num(&day["mintempC"])?,
        max: num(&day["maxtempC"])?,
        cond: wwo(num(&cur["weatherCode"])? as i64),
        dia: true,
        chuva_mm: slot.and_then(|h| num(&h["precipMM"])).unwrap_or(0.0),
        prob: slot.and_then(|h| num(&h["chanceofrain"])),
        horas: hours.iter().filter_map(|(t, h)| Some((*t, num(&h["tempC"])?, num(&h["precipMM"]).unwrap_or(0.0) / 3.0))).collect(),
    })
}

// ---------------------------------------------------------------- merge

fn median(mut v: Vec<f64>) -> Option<f64> {
    if v.is_empty() {
        return None;
    }
    v.sort_by(f64::total_cmp);
    let m = v.len() / 2;
    Some(if v.len().is_multiple_of(2) { (v[m - 1] + v[m]) / 2.0 } else { v[m] })
}

const SEVERITY: [&str; 6] = ["sol", "parcial", "nublado", "neblina", "chuva", "tempestade"];

/// Everything the phone shows except where it is (the caller adds that).
pub fn merge(all: &[Reading], clock: Clock) -> Value {
    let med = |f: &dyn Fn(&Reading) -> Option<f64>| median(all.iter().filter_map(f).collect());
    let round1 = |x: f64| (x * 10.0).round() / 10.0;

    let mut votes = [0usize; SEVERITY.len()];
    for r in all {
        if let Some(i) = SEVERITY.iter().position(|c| *c == r.cond) {
            votes[i] += 1;
        }
    }
    let top = votes.iter().copied().max().unwrap_or(0);
    let cond = SEVERITY.iter().zip(votes).rev().find(|(_, n)| *n == top).map_or("nublado", |(c, _)| *c);

    let rainy = all.iter().filter(|r| r.chuva_mm >= 0.2).count() as f64 / all.len() as f64 * 100.0;
    let odds: Vec<f64> = all.iter().filter_map(|r| r.prob).chain([rainy]).collect();
    let prob = (odds.iter().sum::<f64>() / odds.len() as f64).round();

    let temps: Vec<f64> = all.iter().map(|r| r.agora).collect();
    let spread = temps.iter().copied().fold(f64::MIN, f64::max) - temps.iter().copied().fold(f64::MAX, f64::min);

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

    json!({
        "temp": med(&|r| Some(r.agora)).map(round1),
        "sensacao": med(&|r| r.sensacao).map(round1),
        "umidade": med(&|r| r.umidade).map(f64::round),
        "vento": med(&|r| r.vento).map(f64::round),
        "min": med(&|r| Some(r.min)).map(round1),
        "max": med(&|r| Some(r.max)).map(round1),
        "cond": cond,
        "dia": all.iter().filter(|r| r.dia).count() * 2 >= all.len(),
        "chuva": prob,
        "mm": med(&|r| Some(r.chuva_mm)).map(round1),
        "diferenca": round1(spread),
        "horas": horas,
        "fontes": all.iter().map(|r| json!({ "nome": r.fonte, "temp": round1(r.agora), "cond": r.cond })).collect::<Vec<_>>(),
        "em": clock.now,
    })
}


#[cfg(test)]
mod tests {
    #[test]
    fn iso() {
        assert_eq!(super::iso_ms("1970-01-02T00:00:00Z"), Some(86_400_000));
        assert_eq!(super::iso_ms("2026-11-25T12:00:00Z"), Some(1_795_608_000_000));
    }

    #[test]
    fn median() {
        assert_eq!(super::median(vec![3.0, 1.0, 2.0]), Some(2.0));
        assert_eq!(super::median(vec![4.0, 1.0, 2.0, 3.0]), Some(2.5));
    }
}
