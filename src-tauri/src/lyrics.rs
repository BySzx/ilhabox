//! Synced lyrics from lrclib.net (free, no key), same rules as the Ilha.

use reqwest::Client;
use serde_json::{json, Value};
use std::sync::OnceLock;

fn http() -> &'static Client {
    static C: OnceLock<Client> = OnceLock::new();
    C.get_or_init(|| Client::builder().user_agent("IlhaBox (github.com/BySzx)").timeout(std::time::Duration::from_secs(12)).build().unwrap_or_default())
}

fn plain(s: &str) -> String {
    s.chars().filter(|c| c.is_alphanumeric() || c.is_whitespace()).collect::<String>().to_lowercase().split_whitespace().collect::<Vec<_>>().join(" ")
}

/// True when a search result is really this song, not just the first hit for similar words.
fn same_song(v: &Value, artist: &str, title: &str, duration_s: u64) -> bool {
    let (t, a) = (plain(title), plain(artist));
    let (rt, ra) = (plain(v["trackName"].as_str().unwrap_or("")), plain(v["artistName"].as_str().unwrap_or("")));
    let close = |x: &str, y: &str| !x.is_empty() && !y.is_empty() && (x == y || x.contains(y) || y.contains(x));
    let length_ok = duration_s == 0 || v["duration"].as_f64().is_none_or(|d| (d - duration_s as f64).abs() <= 4.0);
    close(&t, &rt) && close(&a, &ra) && length_ok
}


/// Synced lyrics when there are any, else the plain text; Err only when lrclib could not be reached.
pub async fn lyrics(artist: &str, title: &str, album: &str, duration_s: u64) -> Result<Value, String> {
    // No artist = not a song (a WhatsApp voice note, a video): searching would grab random lyrics.
    if artist.trim().is_empty() || title.trim().is_empty() {
        return Ok(json!({}));
    }
    let mut q = vec![("artist_name", artist.to_string()), ("track_name", title.to_string())];
    if !album.is_empty() {
        q.push(("album_name", album.to_string()));
    }
    if duration_s > 0 {
        q.push(("duration", duration_s.to_string()));
    }
    let mut erro = None;
    let mut plain = None;
    match http().get("https://lrclib.net/api/get").query(&q).send().await {
        Ok(r) => {
            if let Ok(v) = r.json::<Value>().await {
                if let Some(s) = v["syncedLyrics"].as_str().filter(|s| !s.is_empty()) {
                    return Ok(json!({ "synced": s }));
                }
                plain = v["plainLyrics"].as_str().filter(|s| !s.is_empty()).map(String::from);
            }
        }
        Err(e) => erro = Some(e),
    }
    let searches = [
        vec![("artist_name", artist.to_string()), ("track_name", title.to_string())],
        vec![("q", format!("{artist} {title}"))],
    ];
    for q in searches {
        let list = match http().get("https://lrclib.net/api/search").query(&q).send().await {
            Ok(r) => r.json::<Value>().await.unwrap_or(Value::Null),
            Err(e) => {
                erro = Some(e);
                continue;
            }
        };
        let hits: Vec<&Value> = list.as_array().map(|l| l.iter().filter(|v| same_song(v, artist, title, duration_s)).collect()).unwrap_or_default();
        if let Some(s) = hits.iter().find_map(|v| v["syncedLyrics"].as_str().filter(|s| !s.is_empty())) {
            return Ok(json!({ "synced": s }));
        }
        plain = plain.or_else(|| hits.iter().find_map(|v| v["plainLyrics"].as_str().filter(|s| !s.is_empty()).map(String::from)));
        erro = None;
    }
    match (plain, erro) {
        (Some(p), _) => Ok(json!({ "plain": p })),
        (None, Some(e)) => {
            crate::diag::log(&format!("letra: lrclib sem resposta: {e}"));
            Err(format!("o site das letras (lrclib.net) não respondeu: {}", if e.is_timeout() { "demorou demais".to_string() } else if e.is_connect() { "sem conexão, a rede pode estar bloqueando".to_string() } else { e.to_string() }))
        }
        (None, None) => Ok(json!({})),
    }
}
