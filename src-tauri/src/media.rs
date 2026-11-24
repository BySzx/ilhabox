//! System media (SMTC): Spotify, browsers, any player that publishes to Windows media controls.

use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Serialize;
use std::{thread, time::Duration};
use windows::core::Result as WinResult;
use windows::Media::Control::{
    GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as Manager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as Status,
};
use windows::Storage::Streams::DataReader;
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

const EPOCH_DIFF_100NS: i64 = 116_444_736_000_000_000;

#[derive(Serialize, Clone, PartialEq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MediaState {
    pub active: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub playing: bool,
    pub position_ms: i64,
    pub duration_ms: i64,
    pub sampled_at: i64,
    pub app: String,
}

#[derive(Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct ArtPayload {
    key: String,
    data_url: Option<String>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Windows' "current" session is often a paused browser tab while Spotify plays: prefer one that is playing.
fn pick(mgr: &Manager) -> WinResult<Session> {
    let current = mgr.GetCurrentSession().ok();
    let is_playing = |s: &Session| {
        s.GetPlaybackInfo().and_then(|i| i.PlaybackStatus()).map(|st| st == Status::Playing).unwrap_or(false)
    };
    if let Some(c) = current.as_ref().filter(|c| is_playing(c)) {
        return Ok(c.clone());
    }
    if let Ok(all) = mgr.GetSessions() {
        if let Some(p) = all.into_iter().find(is_playing) {
            return Ok(p);
        }
    }
    current.ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL))
}

fn session() -> WinResult<Session> {
    pick(&Manager::RequestAsync()?.join()?)
}

/// Title/artist/album; the cross-process call that fails most (RPC_E_CALL_CANCELED while the player is busy).
type Props = (String, String, String);

fn read_props(s: &Session) -> WinResult<Props> {
    let p = s.TryGetMediaPropertiesAsync()?.join()?;
    Ok((p.Title()?.to_string(), p.Artist()?.to_string(), p.AlbumTitle()?.to_string()))
}

fn read_state(s: &Session, props: &Props) -> WinResult<MediaState> {
    let playing = s.GetPlaybackInfo()?.PlaybackStatus()? == Status::Playing;
    let tl = s.GetTimelineProperties()?;
    let pos = tl.Position()?.Duration / 10_000;
    let end = tl.EndTime()?.Duration / 10_000;
    let upd = tl.LastUpdatedTime()?.UniversalTime;
    let sampled = if upd > 0 { (upd - EPOCH_DIFF_100NS) / 10_000 } else { now_ms() };
    Ok(MediaState {
        active: true,
        title: props.0.clone(),
        artist: props.1.clone(),
        album: props.2.clone(),
        playing,
        position_ms: pos,
        duration_ms: end,
        sampled_at: sampled.min(now_ms()),
        app: s.SourceAppUserModelId()?.to_string(),
    })
}

fn read_art(s: &Session) -> WinResult<Option<String>> {
    let props = s.TryGetMediaPropertiesAsync()?.join()?;
    let Ok(thumb) = props.Thumbnail() else { return Ok(None) };
    let stream = thumb.OpenReadAsync()?.join()?;
    let size = stream.Size()? as u32;
    if size == 0 || size > 8 * 1024 * 1024 {
        return Ok(None);
    }
    let mime = stream.ContentType().map(|m| m.to_string()).unwrap_or_default();
    let reader = DataReader::CreateDataReader(&stream)?;
    reader.LoadAsync(size)?.join()?;
    let mut buf = vec![0u8; size as usize];
    reader.ReadBytes(&mut buf)?;
    let mime = if mime.starts_with("image/") { mime } else { "image/jpeg".into() };
    Ok(Some(format!("data:{mime};base64,{}", STANDARD.encode(buf))))
}

fn extrapolated(m: &MediaState, at: i64) -> i64 {
    if m.playing { m.position_ms + (at - m.sampled_at) } else { m.position_ms }
}

fn differs(a: &MediaState, b: &MediaState) -> bool {
    if a.active != b.active || a.title != b.title || a.artist != b.artist || a.playing != b.playing {
        return true;
    }
    if (a.duration_ms - b.duration_ms).abs() > 1500 {
        return true;
    }
    let t = now_ms();
    (extrapolated(a, t) - extrapolated(b, t)).abs() > 2000
}

/// How long a failing read keeps showing the last song before the island says nothing is playing.
const GRACE_MS: i64 = 8000;

pub fn spawn() {
    thread::Builder::new()
        .name("media".into())
        .spawn(move || {
            unsafe {
                let _ = CoInitializeEx(None, COINIT_MULTITHREADED).ok();
            }
            let mut mgr: Option<Manager> = None;
            let mut last = MediaState::default();
            let mut art_key = String::new();
            let mut art = ArtPayload::default();
            // Last good properties per app: a busy player still answers playback/timeline but not properties.
            let mut props_cache: (String, Props) = Default::default();
            let mut last_app = String::from("-");
            let mut fails = 0u32;
            let mut ok_at = now_ms();
            loop {
                if mgr.is_none() {
                    mgr = Manager::RequestAsync().and_then(|op| op.join()).ok();
                }
                let read = mgr.as_ref().ok_or_else(|| windows::core::Error::from(windows::Win32::Foundation::E_FAIL)).and_then(|m| {
                    let s = pick(m)?;
                    let app_id = s.SourceAppUserModelId()?.to_string();
                    let props = match read_props(&s) {
                        Ok(p) => {
                            props_cache = (app_id.clone(), p.clone());
                            p
                        }
                        Err(e) if props_cache.0 == app_id && !props_cache.1 .0.is_empty() => {
                            if fails == 0 {
                                crate::diag::log(&format!("midia propriedades falharam, usando as ultimas: {}", e.message()));
                            }
                            props_cache.1.clone()
                        }
                        Err(e) => return Err(e),
                    };
                    let st = read_state(&s, &props)?;
                    let key = format!("{}\u{1}{}", st.title, st.artist);
                    if key != art_key && !st.title.is_empty() {
                        art_key = key.clone();
                        art = ArtPayload { key, data_url: read_art(&s).ok().flatten() };
                        crate::bus::send("media-art", &art, true);
                    }
                    Ok(st)
                });
                let cur = match read {
                    Ok(c) => {
                        if fails > 0 {
                            crate::diag::log(&format!("midia voltou apos {fails} falhas"));
                        }
                        fails = 0;
                        ok_at = now_ms();
                        c
                    }
                    Err(e) => {
                        fails += 1;
                        if fails == 1 {
                            crate::diag::log(&format!("midia falhou: {} ({:#x})", e.message(), e.code().0));
                        }
                        // A stale manager keeps failing: rebuild it, and the thread's COM state after a longer streak.
                        if fails.is_multiple_of(3) {
                            mgr = None;
                        }
                        if fails.is_multiple_of(10) {
                            unsafe {
                                CoUninitialize();
                                let _ = CoInitializeEx(None, COINIT_MULTITHREADED).ok();
                            }
                        }
                        if now_ms() - ok_at < GRACE_MS { last.clone() } else { MediaState::default() }
                    }
                };
                if cur.active && cur.app != last_app {
                    crate::diag::log(&format!("midia app={} tocando={} titulo={:?}", cur.app, cur.playing, cur.title));
                    last_app = cur.app.clone();
                }
                if !cur.active {
                    last_app.clear();
                }
                if differs(&cur, &last) {
                    crate::bus::send("media", &cur, true);
                }
                let wait = if fails > 0 { 500 } else if cur.playing { 900 } else { 1800 };
                last = cur;
                thread::sleep(Duration::from_millis(wait));
            }
        })
        .expect("media thread");
}

pub fn control(action: &str, position_ms: Option<i64>) -> Result<(), String> {
    let run = || -> WinResult<bool> {
        let s = session()?;
        let ok = match action {
            "toggle" => s.TryTogglePlayPauseAsync()?.join()?,
            "next" => s.TrySkipNextAsync()?.join()?,
            "prev" => s.TrySkipPreviousAsync()?.join()?,
            "seek" => s.TryChangePlaybackPositionAsync(position_ms.unwrap_or(0) * 10_000)?.join()?,
            _ => false,
        };
        Ok(ok)
    };
    run().map(|_| ()).map_err(|e| e.message())
}
