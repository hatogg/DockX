//! Windows media session (any app that shows in the system media flyout: Spotify, YouTube in browsers,
//! Apple Music, players…) + synced lyrics from LRCLIB.

use super::util::*;
use crate::ui::lrc;
use std::sync::mpsc::{Receiver, Sender};
use std::time::Duration;
use windows::Foundation::TimeSpan;
use windows::Graphics::Imaging::*;
use windows::Media::Control::*;
use windows::Storage::Streams::IRandomAccessStream;
use windows::core::Interface;
use windows::Win32::Networking::WinHttp::*;
use windows::Win32::System::Com::*;

#[derive(Clone, Debug)]
pub enum MediaCmd {
    Toggle,
    Pause,
    Next,
    Prev,
    Seek(f64),
    NextPlayer,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct Snapshot {
    pub has: bool,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub app: String,
    pub playing: bool,
    pub position: f64,
    pub sampled_unix: f64,
    pub duration: f64,
    pub can_prev: bool,
    pub can_next: bool,
    pub players: Vec<String>,
    pub player_idx: usize,
    /// set only when the artwork changed: (w, h, premultiplied RGBA)
    pub art: Option<(u32, u32, Vec<u8>)>,
    pub art_id: u64,
}

pub fn friendly_app(aumid: &str) -> String {
    let low = aumid.to_lowercase();
    let known = [
        ("spotify", "Spotify"),
        ("chrome", "Chrome"),
        ("msedge", "Microsoft Edge"),
        ("308046b0af4a39cb", "Firefox"),
        ("firefox", "Firefox"),
        ("opera", "Opera"),
        ("brave", "Brave"),
        ("yandex", "Яндекс Браузер"),
        ("vivaldi", "Vivaldi"),
        ("zunemusic", "Медіапрогравач"),
        ("applemusic", "Apple Music"),
        ("itunes", "iTunes"),
        ("vlc", "VLC"),
        ("aimp", "AIMP"),
        ("foobar", "foobar2000"),
        ("deezer", "Deezer"),
        ("telegram", "Telegram"),
        ("discord", "Discord"),
        ("soundcloud", "SoundCloud"),
        ("youtube", "YouTube Music"),
    ];
    for (k, v) in known {
        if low.contains(k) {
            return v.to_string();
        }
    }
    let base = aumid.split('!').next().unwrap_or(aumid);
    let base = base.rsplit(['\\', '/']).next().unwrap_or(base);
    let base = base.trim_end_matches(".exe").trim_end_matches(".EXE");
    let base = base.rsplit('.').next().unwrap_or(base);
    base.split('_').next().unwrap_or(base).to_string()
}

fn ticks_now() -> i64 {
    // FILETIME ticks (100 ns since 1601) — same epoch as WinRT DateTime
    ((unix_now() * 1e7) as i64) + 116444736000000000
}

pub fn spawn(tx: Sender<Snapshot>, cmds: Receiver<MediaCmd>, wake: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("DockX media".into())
        .spawn(move || unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
            let mut manager: Option<GlobalSystemMediaTransportControlsSessionManager> = None;
            let mut last = Snapshot::default();
            let mut last_key = String::new();
            let mut art_id = 0u64;
            let mut chosen: Option<String> = None;
            let mut last_send = 0u64;
            let mut pending: Option<MediaCmd> = None;
            loop {
                if manager.is_none() {
                    manager = GlobalSystemMediaTransportControlsSessionManager::RequestAsync().and_then(|op| op.get()).ok();
                    if manager.is_none() {
                        std::thread::sleep(Duration::from_secs(5));
                        continue;
                    }
                }
                let m = manager.clone().unwrap();
                // sessions
                let mut sessions = Vec::new();
                if let Ok(list) = m.GetSessions() {
                    for s in list {
                        sessions.push(s);
                    }
                }
                let ids: Vec<String> = sessions.iter().map(|s| s.SourceAppUserModelId().map(|x| x.to_string()).unwrap_or_default()).collect();
                let mut idx = None;
                if let Some(c) = &chosen {
                    idx = ids.iter().position(|i| i == c);
                }
                if idx.is_none() {
                    if let Ok(cur) = m.GetCurrentSession() {
                        let cid = cur.SourceAppUserModelId().map(|x| x.to_string()).unwrap_or_default();
                        idx = ids.iter().position(|i| *i == cid);
                    }
                }
                if idx.is_none() && !sessions.is_empty() {
                    idx = Some(0);
                }
                // commands
                let mut queue: Vec<MediaCmd> = pending.take().into_iter().collect();
                while let Ok(c) = cmds.try_recv() {
                    queue.push(c);
                }
                for c in queue {
                    if let MediaCmd::NextPlayer = c {
                        if !ids.is_empty() {
                            let n = (idx.unwrap_or(0) + 1) % ids.len();
                            chosen = Some(ids[n].clone());
                            idx = Some(n);
                        }
                        continue;
                    }
                    if let Some(s) = idx.and_then(|i| sessions.get(i)) {
                        let r = match c {
                            MediaCmd::Toggle => s.TryTogglePlayPauseAsync(),
                            MediaCmd::Pause => s.TryPauseAsync(),
                            MediaCmd::Next => s.TrySkipNextAsync(),
                            MediaCmd::Prev => s.TrySkipPreviousAsync(),
                            MediaCmd::Seek(t) => s.TryChangePlaybackPositionAsync((t * 1e7) as i64),
                            MediaCmd::NextPlayer => unreachable!(),
                        };
                        if let Ok(op) = r {
                            let _ = op.get();
                        }
                    }
                }
                let mut snap = Snapshot { players: ids.iter().map(|i| friendly_app(i)).collect(), player_idx: idx.unwrap_or(0), ..Default::default() };
                if let Some(s) = idx.and_then(|i| sessions.get(i)) {
                    snap.has = true;
                    snap.app = friendly_app(&ids[idx.unwrap()]);
                    if let Ok(p) = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
                        snap.title = p.Title().map(|x| x.to_string()).unwrap_or_default();
                        snap.artist = p.Artist().map(|x| x.to_string()).unwrap_or_default();
                        snap.album = p.AlbumTitle().map(|x| x.to_string()).unwrap_or_default();
                        let key = format!("{}|{}|{}", snap.app, snap.title, snap.artist);
                        if key != last_key {
                            last_key = key;
                            art_id += 1;
                            snap.art = p.Thumbnail().ok().and_then(|t| decode_art(&t).ok());
                            if snap.art.is_none() {
                                // some players publish the artwork a moment later
                                std::thread::sleep(Duration::from_millis(600));
                                if let Ok(p2) = s.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
                                    snap.art = p2.Thumbnail().ok().and_then(|t| decode_art(&t).ok());
                                }
                            }
                        }
                    }
                    if snap.title.is_empty() {
                        snap.has = false;
                    }
                    if let Ok(info) = s.GetPlaybackInfo() {
                        snap.playing = info.PlaybackStatus().map(|st| st == GlobalSystemMediaTransportControlsSessionPlaybackStatus::Playing).unwrap_or(false);
                        if let Ok(c) = info.Controls() {
                            snap.can_next = c.IsNextEnabled().unwrap_or(false);
                            snap.can_prev = c.IsPreviousEnabled().unwrap_or(false);
                        }
                    }
                    if let Ok(tl) = s.GetTimelineProperties() {
                        let pos = tl.Position().map(|t: TimeSpan| t.Duration).unwrap_or(0) as f64 / 1e7;
                        let start = tl.StartTime().map(|t| t.Duration).unwrap_or(0) as f64 / 1e7;
                        let end = tl.EndTime().map(|t| t.Duration).unwrap_or(0) as f64 / 1e7;
                        let upd = tl.LastUpdatedTime().map(|d| d.UniversalTime).unwrap_or(0);
                        let mut p = pos - start;
                        if snap.playing && upd > 0 {
                            let since = (ticks_now() - upd) as f64 / 1e7;
                            if (0.0..36000.0).contains(&since) {
                                p += since;
                            }
                        }
                        snap.duration = (end - start).max(0.0);
                        snap.position = if snap.duration > 0.0 { p.clamp(0.0, snap.duration) } else { p.max(0.0) };
                    }
                }
                snap.sampled_unix = unix_now();
                snap.art_id = art_id;
                // send on any real change, and every 2 s to resync the position
                let expected = if last.playing { last.position + (snap.sampled_unix - last.sampled_unix) } else { last.position };
                let jumped = (snap.position - expected).abs() > 1.5;
                let changed = snap.has != last.has
                    || snap.title != last.title
                    || snap.artist != last.artist
                    || snap.playing != last.playing
                    || snap.players != last.players
                    || snap.player_idx != last.player_idx
                    || snap.can_next != last.can_next
                    || (snap.duration - last.duration).abs() > 0.5
                    || snap.art.is_some()
                    || jumped;
                if changed || tick_ms() - last_send > 2000 {
                    last_send = tick_ms();
                    let mut keep = snap.clone();
                    keep.art = None;
                    last = keep;
                    if tx.send(snap).is_err() {
                        return;
                    }
                    wake();
                }
                // react quickly to commands, otherwise poll ~3×/s
                let mut waited = 0;
                while waited < 350 {
                    std::thread::sleep(Duration::from_millis(50));
                    waited += 50;
                    if let Ok(c) = cmds.try_recv() {
                        pending = Some(c);
                        break;
                    }
                }
            }
        })
        .ok();
}

/// Decodes the session thumbnail, centre-cropped to a 256×256 square.
unsafe fn decode_art(r: &windows::Storage::Streams::IRandomAccessStreamReference) -> windows::core::Result<(u32, u32, Vec<u8>)> {
    let stream = r.OpenReadAsync()?.get()?;
    let s: IRandomAccessStream = stream.cast()?;
    let dec = BitmapDecoder::CreateAsync(&s)?.get()?;
    let (w, hgt) = (dec.OrientedPixelWidth()?, dec.OrientedPixelHeight()?);
    if w == 0 || hgt == 0 {
        return Err(windows::core::Error::empty());
    }
    let side = 256u32;
    let k = side as f64 / w.min(hgt) as f64;
    let (sw, sh) = (((w as f64 * k).round() as u32).max(side), ((hgt as f64 * k).round() as u32).max(side));
    let tr = BitmapTransform::new()?;
    tr.SetScaledWidth(sw)?;
    tr.SetScaledHeight(sh)?;
    tr.SetInterpolationMode(BitmapInterpolationMode::Fant)?;
    tr.SetBounds(BitmapBounds { X: (sw - side) / 2, Y: (sh - side) / 2, Width: side, Height: side })?;
    let pd = dec
        .GetPixelDataTransformedAsync(BitmapPixelFormat::Rgba8, BitmapAlphaMode::Premultiplied, &tr, ExifOrientationMode::RespectExifOrientation, ColorManagementMode::DoNotColorManage)?
        .get()?;
    let arr = pd.DetachPixelData()?;
    let v = arr.to_vec();
    if v.len() < (side * side * 4) as usize {
        return Err(windows::core::Error::empty());
    }
    Ok((side, side, v))
}

// ───────────────────────── lyrics ─────────────────────────

#[derive(Clone, Debug)]
pub struct LyricsReq {
    pub key: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub duration: f64,
}

#[derive(Clone, Debug)]
pub enum LyricsRes {
    Synced(String),
    Plain(String),
    NotFound,
}

pub fn spawn_lyrics(rx: Receiver<LyricsReq>, tx: Sender<(String, LyricsRes)>, wake: impl Fn() + Send + 'static) {
    std::thread::Builder::new()
        .name("DockX lyrics".into())
        .spawn(move || {
            while let Ok(mut req) = rx.recv() {
                // only the newest request matters
                while let Ok(r) = rx.try_recv() {
                    req = r;
                }
                let res = find_lyrics(&req);
                if tx.send((req.key.clone(), res)).is_err() {
                    return;
                }
                wake();
            }
        })
        .ok();
}

fn cache_path(req: &LyricsReq) -> std::path::PathBuf {
    let mut hsh: u64 = 1469598103934665603;
    for b in format!("{}|{}", req.artist.to_lowercase(), req.title.to_lowercase()).bytes() {
        hsh ^= b as u64;
        hsh = hsh.wrapping_mul(1099511628211);
    }
    appdata_dir().join("lyrics").join(format!("{:016x}.lrc", hsh))
}

fn find_lyrics(req: &LyricsReq) -> LyricsRes {
    let cp = cache_path(req);
    if let Ok(text) = std::fs::read_to_string(&cp) {
        if let Some(rest) = text.strip_prefix("#synced\n") {
            return LyricsRes::Synced(rest.to_string());
        }
        if let Some(rest) = text.strip_prefix("#plain\n") {
            return LyricsRes::Plain(rest.to_string());
        }
        if text.starts_with("#none") {
            let fresh = std::fs::metadata(&cp).and_then(|m| m.modified()).ok().and_then(|t| t.elapsed().ok()).map(|e| e.as_secs() < 3 * 86400).unwrap_or(false);
            if fresh {
                return LyricsRes::NotFound;
            }
        }
    }
    // candidates: as reported, cleaned title, "Artist - Title" from video titles
    let mut cands: Vec<(String, String)> = Vec::new();
    let title = lrc::clean_title(&req.title);
    if !req.artist.is_empty() {
        cands.push((req.artist.clone(), title.clone()));
    }
    if let Some((a, t)) = lrc::split_artist_title(&req.title) {
        cands.push((a, t));
    }
    if req.artist.is_empty() {
        cands.push((String::new(), title.clone()));
    }
    let mut res = LyricsRes::NotFound;
    'outer: for (artist, track) in &cands {
        if !artist.is_empty() {
            let mut path = format!("/api/get?track_name={}&artist_name={}", lrc::url_encode(track), lrc::url_encode(artist));
            if !req.album.is_empty() {
                path += &format!("&album_name={}", lrc::url_encode(&req.album));
            }
            if req.duration > 1.0 {
                path += &format!("&duration={}", req.duration.round() as i64);
            }
            if let Some((200, body)) = http_get("lrclib.net", &path) {
                if let Some(r) = pick(&body) {
                    res = r;
                    break 'outer;
                }
            }
        }
        let mut q = format!("/api/search?track_name={}", lrc::url_encode(track));
        if !artist.is_empty() {
            q += &format!("&artist_name={}", lrc::url_encode(artist));
        }
        if let Some((200, body)) = http_get("lrclib.net", &q) {
            if let Ok(serde_json::Value::Array(items)) = serde_json::from_str::<serde_json::Value>(&body) {
                // prefer synced, close duration
                let mut best: Option<(f64, LyricsRes)> = None;
                for it in items.iter().take(15) {
                    let d = it.get("duration").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let score = if req.duration > 1.0 { (d - req.duration).abs() } else { 0.0 };
                    if req.duration > 1.0 && score > 8.0 {
                        continue;
                    }
                    if let Some(r) = pick(&it.to_string()) {
                        let synced = matches!(r, LyricsRes::Synced(_));
                        let sc = score - if synced { 1000.0 } else { 0.0 };
                        if best.as_ref().map(|b| sc < b.0).unwrap_or(true) {
                            best = Some((sc, r));
                        }
                    }
                }
                if let Some((_, r)) = best {
                    res = r;
                    break 'outer;
                }
            }
        }
    }
    let text = match &res {
        LyricsRes::Synced(s) => format!("#synced\n{}", s),
        LyricsRes::Plain(s) => format!("#plain\n{}", s),
        LyricsRes::NotFound => "#none\n".to_string(),
    };
    write_atomic(&cp, &text);
    res
}

fn pick(body: &str) -> Option<LyricsRes> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    if let Some(s) = v.get("syncedLyrics").and_then(|x| x.as_str()) {
        if !s.trim().is_empty() {
            return Some(LyricsRes::Synced(s.to_string()));
        }
    }
    if let Some(s) = v.get("plainLyrics").and_then(|x| x.as_str()) {
        if !s.trim().is_empty() {
            return Some(LyricsRes::Plain(s.to_string()));
        }
    }
    None
}

/// HTTPS GET with WinHTTP (system proxy, system TLS). Returns (status, body).
pub fn http_get(host: &str, path: &str) -> Option<(u32, String)> {
    unsafe {
        let ua = h("DockX/2.0 (https://github.com/hatogg)");
        let ses = WinHttpOpen(&ua, WINHTTP_ACCESS_TYPE_AUTOMATIC_PROXY, None, None, 0);
        if ses.is_null() {
            return None;
        }
        let _ = WinHttpSetTimeouts(ses, 4000, 4000, 6000, 8000);
        let con = WinHttpConnect(ses, &h(host), 443, 0);
        if con.is_null() {
            let _ = WinHttpCloseHandle(ses);
            return None;
        }
        let req = WinHttpOpenRequest(con, &h("GET"), &h(path), None, None, std::ptr::null(), WINHTTP_FLAG_SECURE);
        let mut out = None;
        if !req.is_null() {
            if WinHttpSendRequest(req, None, None, 0, 0, 0).is_ok() && WinHttpReceiveResponse(req, std::ptr::null_mut()).is_ok() {
                let mut status = 0u32;
                let mut len = 4u32;
                let _ = WinHttpQueryHeaders(
                    req,
                    WINHTTP_QUERY_STATUS_CODE | WINHTTP_QUERY_FLAG_NUMBER,
                    None,
                    Some(&mut status as *mut u32 as *mut _),
                    &mut len,
                    std::ptr::null_mut(),
                );
                let mut body = Vec::new();
                loop {
                    let mut avail = 0u32;
                    if WinHttpQueryDataAvailable(req, &mut avail).is_err() || avail == 0 {
                        break;
                    }
                    let mut buf = vec![0u8; avail as usize];
                    let mut read = 0u32;
                    if WinHttpReadData(req, buf.as_mut_ptr() as *mut _, avail, &mut read).is_err() || read == 0 {
                        break;
                    }
                    body.extend_from_slice(&buf[..read as usize]);
                    if body.len() > 4 << 20 {
                        break;
                    }
                }
                out = Some((status, String::from_utf8_lossy(&body).to_string()));
            }
            let _ = WinHttpCloseHandle(req);
        }
        let _ = WinHttpCloseHandle(con);
        let _ = WinHttpCloseHandle(ses);
        out
    }
}
