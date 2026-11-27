//! MelloStudio wallpaper commands: settings, presets, and the user's image
//! library. Images live in `<config>/wallpapers/` under names the app
//! generates itself; the webview loads them through Tauri's asset protocol,
//! which is scoped to that folder only.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use base64::Engine as _;
use mellogram_core::wallpaper::{self, NamedWallpaper, Wallpaper, WallpaperSettings};
use serde::Serialize;
use tauri::{Emitter, State};

use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

const MAX_IMAGE_BYTES: usize = 20 * 1024 * 1024;
const MAX_IMAGES: usize = 200;

pub fn wallpaper_dir(config: &Path) -> PathBuf {
    config.join("wallpapers")
}

/// Percent-encodes everything except unreserved characters, like
/// `encodeURIComponent`, which is what Tauri's asset protocol expects.
fn encode_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len() * 3);
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// URL for an image in the wallpapers folder, in the form
/// `convertFileSrc` produces for the current platform.
pub fn image_url(config: &Path, file: &str) -> Option<String> {
    if !wallpaper::safe_image_file(file) {
        return None;
    }
    let path = wallpaper_dir(config).join(file);
    if !path.is_file() {
        return None;
    }
    Some(asset_url(&path))
}

/// URL for a file the webview may load through the (scoped) asset protocol,
/// in the form `convertFileSrc` produces for the current platform.
pub fn asset_url(path: &Path) -> String {
    let encoded = encode_component(&path.to_string_lossy());
    if cfg!(any(windows, target_os = "android")) {
        format!("http://asset.localhost/{encoded}")
    } else {
        format!("asset://localhost/{encoded}")
    }
}

fn changed(state: &AppState) -> R<()> {
    state.save()?;
    state.refresh_styles();
    let _ = state.app.emit("mg://wallpapers-changed", ());
    Ok(())
}

#[tauri::command]
pub fn get_wallpapers(state: S<'_>) -> WallpaperSettings {
    state.store.lock().unwrap().settings.wallpapers.clone()
}

#[tauri::command]
pub fn set_wallpapers(state: S<'_>, wallpapers: WallpaperSettings) -> R<()> {
    let mut wallpapers = wallpapers;
    wallpapers.saved.truncate(100);
    wallpapers.per_chat.retain(|id, _| id.len() <= 24 && id.chars().all(|c| c.is_ascii_digit() || c == '-'));
    state.store.lock().unwrap().settings.wallpapers = wallpapers;
    changed(&state)
}

#[derive(Serialize)]
pub struct PresetView {
    name: String,
    wallpaper: Wallpaper,
    /// Ready-to-use declarations for gallery swatches.
    css: Option<String>,
}

#[tauri::command]
pub fn wallpaper_presets(state: S<'_>) -> Vec<PresetView> {
    let dir = state.dir.clone();
    let saved = state.store.lock().unwrap().settings.wallpapers.saved.clone();
    wallpaper::presets()
        .into_iter()
        .chain(saved)
        .map(|NamedWallpaper { name, wallpaper }| PresetView {
            css: wallpaper.layer_css(&|f| image_url(&dir, f)),
            name,
            wallpaper,
        })
        .collect()
}

/// CSS declarations for an arbitrary wallpaper, for live previews.
#[tauri::command]
pub fn wallpaper_preview_css(state: S<'_>, wallpaper: Wallpaper) -> Option<String> {
    let dir = state.dir.clone();
    wallpaper.layer_css(&|f| image_url(&dir, f))
}

#[derive(Serialize)]
pub struct WallpaperImage {
    file: String,
    url: String,
    bytes: u64,
}

#[tauri::command]
pub fn list_wallpaper_images(state: S<'_>) -> R<Vec<WallpaperImage>> {
    let dir = wallpaper_dir(&state.dir);
    let mut out = Vec::new();
    if let Ok(rd) = std::fs::read_dir(&dir) {
        for e in rd.flatten() {
            let file = e.file_name().to_string_lossy().into_owned();
            if let Some(url) = image_url(&state.dir, &file) {
                out.push(WallpaperImage { bytes: e.metadata().map(|m| m.len()).unwrap_or(0), file, url });
            }
        }
    }
    out.sort_by(|a, b| b.file.cmp(&a.file));
    Ok(out)
}

/// Saves an image (base64 from the file picker). The type is detected from
/// the bytes, not the file name; SVG and anything unrecognised is refused.
#[tauri::command]
pub fn import_wallpaper_image(state: S<'_>, data: String) -> R<WallpaperImage> {
    let b64 = data.split_once(',').map(|(_, d)| d).unwrap_or(&data);
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim()).map_err(|_| "not valid image data".to_string())?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err(format!("images are limited to {} MB", MAX_IMAGE_BYTES / 1024 / 1024));
    }
    let ext = wallpaper::sniff_image(&bytes).ok_or("only PNG, JPEG, WebP and GIF images are supported")?;
    let dir = wallpaper_dir(&state.dir);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    if std::fs::read_dir(&dir).map(|rd| rd.count()).unwrap_or(0) >= MAX_IMAGES {
        return Err(format!("you already have {MAX_IMAGES} wallpaper images; delete some first"));
    }
    let mut rnd = [0u8; 6];
    getrandom::getrandom(&mut rnd).map_err(|e| e.to_string())?;
    let file = format!("wp-{}.{ext}", rnd.iter().map(|b| format!("{b:02x}")).collect::<String>());
    let path = dir.join(&file);
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    let url = image_url(&state.dir, &file).ok_or("could not register the image")?;
    Ok(WallpaperImage { file, url, bytes: bytes.len() as u64 })
}

#[tauri::command]
pub fn delete_wallpaper_image(state: S<'_>, file: String) -> R<()> {
    if !wallpaper::safe_image_file(&file) {
        return Err("invalid file name".into());
    }
    let path = wallpaper_dir(&state.dir).join(&file);
    if path.is_file() {
        std::fs::remove_file(path).map_err(|e| e.to_string())?;
    }
    // Anything still pointing at the image falls back to no wallpaper.
    {
        let mut store = state.store.lock().unwrap();
        let w = &mut store.settings.wallpapers;
        let uses = |wp: &Wallpaper| matches!(&wp.kind, wallpaper::WallpaperKind::Image { file: f, .. } if f == &file);
        if uses(&w.global) {
            w.global = Wallpaper::default();
        }
        if w.dark.as_ref().is_some_and(uses) {
            w.dark = None;
        }
        w.per_chat.retain(|_, wp| !uses(wp));
        w.saved.retain(|n| !uses(&n.wallpaper));
    }
    changed(&state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_like_encode_uri_component() {
        assert_eq!(encode_component("/home/a b/wp-1.png"), "%2Fhome%2Fa%20b%2Fwp-1.png");
        assert_eq!(encode_component("C:\\Users\\x"), "C%3A%5CUsers%5Cx");
    }

    #[test]
    fn urls_only_for_existing_safe_files() {
        let dir = std::env::temp_dir().join(format!("mg-wp-test-{}", std::process::id()));
        std::fs::create_dir_all(wallpaper_dir(&dir)).unwrap();
        std::fs::write(wallpaper_dir(&dir).join("wp-aa.png"), b"x").unwrap();
        assert!(image_url(&dir, "wp-aa.png").unwrap().ends_with("wp-aa.png"));
        assert!(image_url(&dir, "missing.png").is_none());
        assert!(image_url(&dir, "../settings.json").is_none());
    }
}
