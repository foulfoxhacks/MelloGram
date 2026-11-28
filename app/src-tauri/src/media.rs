//! Media cache, downloads, saving and location sending.
//!
//! Files are stored in `<app cache>/media/` under names the app generates
//! (`<chat>_<message>_<variant>.<ext>`); the extension comes from a fixed
//! allow-list, never from Telegram-supplied file names. The webview loads them
//! through the asset protocol, whose scope is limited to that folder.

use std::collections::HashMap;
use std::io::Read as _;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use mellogram_telegram::MediaVariant;
use serde::Serialize;
use tauri::{Manager, State};

use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

/// Decompressed Lottie JSON is capped; real stickers are well under 1 MB.
const MAX_LOTTIE_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Default)]
pub struct MediaState {
    /// One lock per file so concurrent requests download it once.
    inflight: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

fn cache_dir(state: &AppState) -> R<PathBuf> {
    let dir = state.app.path().app_cache_dir().map_err(|e| e.to_string())?.join("media");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir)
}

/// File extension from a MIME type, from a fixed allow-list.
pub fn extension_for(mime: Option<&str>, thumbnail: bool) -> &'static str {
    if thumbnail {
        return "jpg";
    }
    match mime.unwrap_or("") {
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "video/mp4" => "mp4",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "audio/ogg" | "audio/opus" => "ogg",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/mp4" | "audio/m4a" | "audio/x-m4a" => "m4a",
        "audio/flac" => "flac",
        "audio/wav" | "audio/x-wav" => "wav",
        "application/x-tgsticker" => "tgs",
        "application/pdf" => "pdf",
        "text/plain" => "txt",
        _ => "bin",
    }
}

/// Inflates a TGS (gzip-compressed Lottie) file to JSON with a size cap.
pub fn inflate_tgs(compressed: &[u8]) -> R<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(compressed)
        .take(MAX_LOTTIE_BYTES + 1)
        .read_to_end(&mut out)
        .map_err(|_| "animated sticker is not valid gzip".to_string())?;
    if out.len() as u64 > MAX_LOTTIE_BYTES {
        return Err("animated sticker is too large".into());
    }
    serde_json::from_slice::<serde::de::IgnoredAny>(&out).map_err(|_| "animated sticker is not valid Lottie JSON".to_string())?;
    Ok(out)
}

#[derive(Serialize)]
pub struct MediaFile {
    url: String,
    bytes: u64,
}

/// Downloads (once) and returns a loadable URL for a message's media.
#[tauri::command]
pub async fn media_file(state: S<'_>, chat_id: i64, message_id: i32, variant: String, mime: Option<String>) -> R<Option<MediaFile>> {
    let thumb = variant == "thumb";
    let max_bytes = u64::from(state.store.lock().unwrap().settings.media.max_download_mb.clamp(1, 4000)) * 1024 * 1024;
    let ext = extension_for(mime.as_deref(), thumb);
    let dir = cache_dir(&state)?;
    let stem = format!("{chat_id}_{message_id}_{}", if thumb { "thumb" } else { "full" });
    let final_path = dir.join(format!("{stem}.{}", if ext == "tgs" { "json" } else { ext }));

    let lock = {
        let mut map = state.media.inflight.lock().unwrap();
        Arc::clone(map.entry(stem.clone()).or_default())
    };
    let _guard = lock.lock().await;
    let downloaded = !final_path.is_file();
    if downloaded {
        let tg = state.telegram().await?;
        let raw = dir.join(format!("{stem}.{ext}.dl"));
        let variant = if thumb { MediaVariant::Thumbnail } else { MediaVariant::Full };
        if !tg.download_media(chat_id, message_id, variant, &raw, max_bytes).await.map_err(|e| e.to_string())? {
            return Ok(None);
        }
        if ext == "tgs" {
            let bytes = std::fs::read(&raw).map_err(|e| e.to_string())?;
            let _ = std::fs::remove_file(&raw);
            std::fs::write(&final_path, inflate_tgs(&bytes)?).map_err(|e| e.to_string())?;
        } else {
            std::fs::rename(&raw, &final_path).map_err(|e| e.to_string())?;
        }
    }
    state.media.inflight.lock().unwrap().remove(&stem);
    let bytes = std::fs::metadata(&final_path).map(|m| m.len()).unwrap_or(0);
    if downloaded {
        state.traffic.record("in", "telegram", "upload.getFile", bytes, if thumb { "media thumbnail" } else { "media file" });
    }
    Ok(Some(MediaFile { url: crate::wallpapers::asset_url(&final_path), bytes }))
}

/// A chat's small profile photo, cached by photo id (a new photo gets a new
/// file). Only digits are accepted for the id, so it can never form a path.
#[tauri::command]
pub async fn chat_photo(state: S<'_>, chat_id: i64, photo_id: String, big: Option<bool>) -> R<Option<String>> {
    let big = big.unwrap_or(false);
    if photo_id.is_empty() || photo_id.len() > 20 || !photo_id.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
        return Err("invalid photo id".into());
    }
    let id: i64 = photo_id.parse().map_err(|_| "invalid photo id".to_string())?;
    let dir = cache_dir(&state)?;
    let stem = format!("avatar_{chat_id}_{id}{}", if big { "_big" } else { "" });
    let path = dir.join(format!("{stem}.jpg"));
    let lock = {
        let mut map = state.media.inflight.lock().unwrap();
        Arc::clone(map.entry(stem.clone()).or_default())
    };
    let _guard = lock.lock().await;
    let cached = path.is_file();
    let result = if cached {
        Ok(true)
    } else {
        match state.telegram().await {
            Ok(tg) => tg.download_profile_photo(chat_id, id, big, &path).await.map_err(|e| e.to_string()),
            Err(e) => Err(e),
        }
    };
    state.media.inflight.lock().unwrap().remove(&stem);
    let ok = result?;
    if ok && !cached {
        state.traffic.record("in", "telegram", "upload.getFile", std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0), "profile photo");
    }
    Ok(ok.then(|| crate::wallpapers::asset_url(&path)))
}

/// Lottie JSON for an animated (TGS) sticker, delivered over IPC so the UI
/// never needs fetch/CORS access to the cache.
#[tauri::command]
pub async fn media_lottie(state: S<'_>, chat_id: i64, message_id: i32) -> R<Option<String>> {
    let Some(_) = media_file(state.clone(), chat_id, message_id, "full".into(), Some("application/x-tgsticker".into())).await? else {
        return Ok(None);
    };
    let path = cache_dir(&state)?.join(format!("{chat_id}_{message_id}_full.json"));
    let text = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Ok(Some(text))
}

/// Extensions that could run code if opened; saved files never auto-open,
/// and the UI warns for these.
const RISKY: &[&str] = &["exe", "msi", "bat", "cmd", "com", "scr", "ps1", "vbs", "js", "jse", "wsf", "hta", "jar", "apk", "app", "dmg", "pkg", "sh", "run", "deb", "rpm", "appimage", "lnk", "reg", "dll", "so", "dylib"];

/// Keeps letters, digits and a few safe characters; never a path.
pub fn safe_file_name(name: &str) -> String {
    let base = Path::new(name).file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let cleaned: String = base.chars().map(|c| if c.is_alphanumeric() || " ._-()[]".contains(c) { c } else { '_' }).collect();
    let cleaned = cleaned.trim().trim_start_matches('.').chars().take(120).collect::<String>();
    if cleaned.is_empty() { "file".into() } else { cleaned }
}

#[derive(Serialize)]
pub struct SavedFile {
    path: String,
    risky: bool,
}

/// Copies a downloaded file to Downloads/MelloGram. Never opens it.
#[tauri::command]
pub async fn save_media(state: S<'_>, chat_id: i64, message_id: i32, mime: Option<String>, file_name: Option<String>) -> R<SavedFile> {
    let Some(file) = media_file(state.clone(), chat_id, message_id, "full".into(), mime.clone()).await? else {
        return Err("this message has no downloadable file".into());
    };
    let _ = file;
    let ext = extension_for(mime.as_deref(), false);
    let src = cache_dir(&state)?.join(format!("{chat_id}_{message_id}_full.{}", if ext == "tgs" { "json" } else { ext }));
    let downloads = state.app.path().download_dir().map_err(|e| e.to_string())?.join("MelloGram");
    std::fs::create_dir_all(&downloads).map_err(|e| e.to_string())?;
    let mut name = safe_file_name(file_name.as_deref().unwrap_or(&format!("mellogram-{message_id}.{ext}")));
    if Path::new(&name).extension().is_none() {
        name.push('.');
        name.push_str(ext);
    }
    let mut dest = downloads.join(&name);
    let mut n = 1;
    while dest.exists() {
        let stem = Path::new(&name).file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let e = Path::new(&name).extension().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        dest = downloads.join(format!("{stem} ({n}).{e}"));
        n += 1;
    }
    std::fs::copy(&src, &dest).map_err(|e| e.to_string())?;
    let risky = dest.extension().map(|e| RISKY.contains(&e.to_string_lossy().to_ascii_lowercase().as_str())).unwrap_or(false);
    Ok(SavedFile { path: dest.to_string_lossy().into_owned(), risky })
}

#[tauri::command]
pub async fn send_location(state: S<'_>, chat_id: i64, lat: f64, long: f64, silent: Option<bool>) -> R<mellogram_core::IncomingMessage> {
    let mut msg = state.telegram().await?.send_location(chat_id, lat, long, silent.unwrap_or(false)).await.map_err(|e| e.to_string())?;
    state.decorate(std::slice::from_mut(&mut msg));
    Ok(msg)
}

#[tauri::command]
pub fn get_media_settings(state: S<'_>) -> mellogram_core::settings::MediaSettings {
    state.store.lock().unwrap().settings.media.clone()
}

#[tauri::command]
pub fn set_media_settings(state: S<'_>, media: mellogram_core::settings::MediaSettings) -> R<()> {
    state.store.lock().unwrap().settings.media = media;
    state.save()
}

/// Deletes the media cache.
#[tauri::command]
pub fn clear_media_cache(state: S<'_>) -> R<u64> {
    let dir = cache_dir(&state)?;
    let mut freed = 0;
    for e in std::fs::read_dir(&dir).map_err(|e| e.to_string())?.flatten() {
        if let Ok(m) = e.metadata() {
            if m.is_file() && std::fs::remove_file(e.path()).is_ok() {
                freed += m.len();
            }
        }
    }
    Ok(freed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extensions_are_allow_listed() {
        assert_eq!(extension_for(Some("image/jpeg"), false), "jpg");
        assert_eq!(extension_for(Some("application/x-msdownload"), false), "bin");
        assert_eq!(extension_for(Some("text/html"), false), "bin");
        assert_eq!(extension_for(Some("video/mp4"), true), "jpg");
    }

    #[test]
    fn file_names_are_sanitised() {
        assert_eq!(safe_file_name("../../etc/passwd"), "passwd");
        assert_eq!(safe_file_name("C:\\Windows\\evil.exe"), "C__Windows_evil.exe");
        assert_eq!(safe_file_name(".bashrc"), "bashrc");
        assert_eq!(safe_file_name(""), "file");
        assert_eq!(safe_file_name("photo 2026 (1).jpg"), "photo 2026 (1).jpg");
    }

    #[test]
    fn tgs_inflation_is_capped_and_validated() {
        use std::io::Write as _;
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
        gz.write_all(br#"{"v":"5.5.2","fr":60,"layers":[]}"#).unwrap();
        let ok = gz.finish().unwrap();
        assert!(inflate_tgs(&ok).is_ok());
        assert!(inflate_tgs(b"not gzip").is_err());
        let mut gz = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::best());
        gz.write_all(&vec![b' '; (MAX_LOTTIE_BYTES + 10) as usize]).unwrap();
        assert!(inflate_tgs(&gz.finish().unwrap()).is_err(), "gzip bomb rejected");
    }
}
