//! Persistent settings plus Backup & Restore.
//!
//! Everything lives in one `settings.json` in the config directory. Writes go
//! to a temporary file that is then renamed, so a crash never leaves a
//! half-written file. Backups deliberately exclude Telegram credentials.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::{Error, Result};

pub const SETTINGS_VERSION: u32 = 1;
pub const BACKUP_FORMAT: &str = "mellogram-backup";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    pub version: u32,
    pub general: GeneralSettings,
    pub telegram: TelegramSettings,
    pub plugins: BTreeMap<String, PluginState>,
    pub themes: ThemeSettings,
    pub notifications: NotificationSettings,
    pub appearance: crate::appearance::Appearance,
    pub workspaces: Vec<crate::workspace::Workspace>,
    pub active_workspace: Option<String>,
    pub privacy: PrivacySettings,
    pub rpc: RpcSettings,
    pub wallpapers: crate::wallpaper::WallpaperSettings,
}

/// Local-only privacy controls. These change what this screen shows; they
/// cannot stop other people from copying what you send them.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PrivacySettings {
    /// Salted PBKDF-style SHA-256 hash of the app-lock PIN, hex. `None` = no lock.
    pub lock_pin_hash: Option<String>,
    pub lock_salt: Option<String>,
    /// Lock after this many minutes without input; 0 = only manually.
    pub lock_after_minutes: u32,
    pub lock_on_start: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            version: SETTINGS_VERSION,
            general: GeneralSettings::default(),
            telegram: TelegramSettings::default(),
            plugins: BTreeMap::new(),
            themes: ThemeSettings::default(),
            notifications: NotificationSettings::default(),
            appearance: Default::default(),
            workspaces: Vec::new(),
            active_workspace: None,
            privacy: PrivacySettings::default(),
            rpc: RpcSettings::default(),
            wallpapers: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralSettings {
    /// Check GitHub releases on launch.
    pub auto_check_updates: bool,
    /// Show the developer tab and allow opening webview devtools.
    pub developer_mode: bool,
    /// Minimise to tray instead of quitting when the window closes.
    pub close_to_tray: bool,
    /// Turn off the window frame and draw our own title bar.
    pub frameless: bool,
    /// Start hidden in the tray (desktop).
    pub start_minimized: bool,
    /// Start MelloGram when you sign in to your computer (desktop).
    pub launch_at_login: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            auto_check_updates: true,
            developer_mode: false,
            close_to_tray: false,
            frameless: false,
            start_minimized: false,
            launch_at_login: false,
        }
    }
}


/// Local rich-presence listeners. Both are opt-in and restart-required.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct RpcSettings {
    /// Enable the native MelloRPC local IPC endpoint.
    pub native_enabled: bool,
    /// Enable the Discord IPC Rich Presence compatibility endpoint.
    pub discord_compat_enabled: bool,
}

/// Telegram application credentials from <https://my.telegram.org>.
/// Never included in backups.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct TelegramSettings {
    pub api_id: Option<i32>,
    pub api_hash: Option<String>,
}

// Built-in application credentials. These are client application identifiers,
// not a user's Telegram login/session token. Optional build environment values
// can override them for development or alternate distributions.
const BUILTIN_TELEGRAM_API_ID: i32 = 30886269;
const BUILTIN_TELEGRAM_API_HASH: &str = "957f1de9d7ccb6e6639fec6ef79ad217";

impl Default for TelegramSettings {
    fn default() -> Self {
        let api_id = option_env!("MELLOGRAM_TELEGRAM_API_ID")
            .and_then(|value| value.parse::<i32>().ok())
            .filter(|value| *value > 0)
            .or(Some(BUILTIN_TELEGRAM_API_ID));
        let api_hash = option_env!("MELLOGRAM_TELEGRAM_API_HASH")
            .map(str::trim)
            .filter(|value| value.len() == 32 && value.chars().all(|c| c.is_ascii_hexdigit()))
            .map(str::to_owned)
            .or_else(|| Some(BUILTIN_TELEGRAM_API_HASH.to_owned()));
        Self { api_id, api_hash }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct PluginState {
    /// `None` means "use the manifest default".
    pub enabled: Option<bool>,
    pub favorite: bool,
    pub settings: Map<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct ThemeSettings {
    /// Enabled theme file names, in the order they are applied.
    pub enabled: Vec<String>,
    pub quick_css: String,
    pub quick_css_enabled: bool,
    /// Per-theme overrides for `--variables` exposed by a theme.
    pub variables: BTreeMap<String, BTreeMap<String, String>>,
}

impl Default for ThemeSettings {
    fn default() -> Self {
        Self {
            enabled: Vec::new(),
            quick_css: String::new(),
            quick_css_enabled: true,
            variables: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct NotificationSettings {
    pub position: ToastPosition,
    pub timeout_ms: u32,
    pub use_native: bool,
    /// Native notifications for new messages while MelloGram is hidden or unfocused.
    pub desktop: bool,
    /// Include sender and text; off shows only "New message".
    pub show_previews: bool,
    /// Do Not Disturb: no message notifications at all.
    pub dnd: bool,
    pub quiet_hours: bool,
    /// Local hours, 0–23. A range that wraps midnight (22 → 7) is supported.
    pub quiet_start: u8,
    pub quiet_end: u8,
    /// Chats that never notify.
    pub muted_chats: Vec<i64>,
    /// Notify for your own messages sent from other devices.
    pub notify_own: bool,
}

impl Default for NotificationSettings {
    fn default() -> Self {
        Self {
            position: ToastPosition::BottomRight,
            timeout_ms: 5000,
            use_native: false,
            desktop: true,
            show_previews: true,
            dnd: false,
            quiet_hours: false,
            quiet_start: 22,
            quiet_end: 7,
            muted_chats: Vec::new(),
            notify_own: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ToastPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

/// Owns the settings file.
#[derive(Debug)]
pub struct SettingsStore {
    path: PathBuf,
    pub settings: Settings,
}

impl SettingsStore {
    /// Loads `dir/settings.json`, creating defaults if it does not exist. A
    /// corrupt file is moved aside to `settings.json.broken` rather than lost.
    pub fn open(dir: &Path) -> Result<Self> {
        fs::create_dir_all(dir)?;
        let path = dir.join("settings.json");
        let settings = match fs::read_to_string(&path) {
            Ok(text) => match serde_json::from_str::<Settings>(&text) {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("settings.json is unreadable ({e}); starting from defaults");
                    fs::rename(&path, dir.join("settings.json.broken"))?;
                    Settings::default()
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Settings::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(Self { path, settings })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn save(&self) -> Result<()> {
        write_atomic(&self.path, &serde_json::to_vec_pretty(&self.settings)?)
    }

    pub fn plugin(&mut self, id: &str) -> &mut PluginState {
        self.settings.plugins.entry(id.to_owned()).or_default()
    }

    /// Builds a portable backup. `themes` are `(file name, css)` pairs.
    pub fn export(&self, themes: Vec<(String, String)>) -> Backup {
        let mut settings = self.settings.clone();
        settings.telegram = TelegramSettings::default();
        settings.privacy.lock_pin_hash = None;
        settings.privacy.lock_salt = None;
        Backup {
            format: BACKUP_FORMAT.into(),
            version: SETTINGS_VERSION,
            created_at: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            settings,
            themes: themes
                .into_iter()
                .map(|(file, css)| BackupTheme { file, css })
                .collect(),
        }
    }

    /// Replaces settings with the backup's, keeping local Telegram credentials.
    /// Returns the themes contained in the backup so the caller can install them.
    pub fn import(&mut self, json: &str) -> Result<Vec<BackupTheme>> {
        let backup: Backup = serde_json::from_str(json)?;
        backup.validate()?;
        let credentials = std::mem::take(&mut self.settings.telegram);
        let lock = (self.settings.privacy.lock_pin_hash.take(), self.settings.privacy.lock_salt.take());
        self.settings = backup.settings;
        self.settings.telegram = credentials;
        (self.settings.privacy.lock_pin_hash, self.settings.privacy.lock_salt) = lock;
        self.save()?;
        Ok(backup.themes)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Backup {
    pub format: String,
    pub version: u32,
    pub created_at: String,
    pub app_version: String,
    pub settings: Settings,
    #[serde(default)]
    pub themes: Vec<BackupTheme>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BackupTheme {
    pub file: String,
    pub css: String,
}

impl Backup {
    pub fn validate(&self) -> Result<()> {
        if self.format != BACKUP_FORMAT {
            return Err(Error::InvalidBackup(format!("not a MelloGram backup (format `{}`)", self.format)));
        }
        if self.version > SETTINGS_VERSION {
            return Err(Error::InvalidBackup(format!(
                "made by a newer MelloGram (version {}, this build understands {SETTINGS_VERSION})",
                self.version
            )));
        }
        for theme in &self.themes {
            if crate::theme::sanitize_file_name(&theme.file).as_deref() != Some(theme.file.as_str()) {
                return Err(Error::InvalidBackup(format!("unsafe theme file name `{}`", theme.file)));
            }
        }
        Ok(())
    }
}

pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let tmp = path.with_extension("tmp");
    fs::write(&tmp, bytes)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tempdir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mellogram-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        dir
    }


    #[test]
    fn rpc_listeners_are_disabled_by_default() {
        let settings = Settings::default();
        assert!(!settings.rpc.native_enabled);
        assert!(!settings.rpc.discord_compat_enabled);
    }

    #[test]
    fn roundtrip_and_backup_excludes_credentials() {
        let dir = tempdir("settings");
        let mut store = SettingsStore::open(&dir).unwrap();
        store.settings.telegram.api_id = Some(12345);
        store.settings.telegram.api_hash = Some("secret".into());
        store.plugin("link-scrubber").enabled = Some(true);
        store.settings.themes.quick_css = "body{color:red}".into();
        store.save().unwrap();

        let reopened = SettingsStore::open(&dir).unwrap();
        assert_eq!(reopened.settings, store.settings);

        let backup = store.export(vec![("a.theme.css".into(), "x".into())]);
        let json = serde_json::to_string(&backup).unwrap();
        assert!(!json.contains("secret"));

        let mut other = SettingsStore::open(&tempdir("settings-b")).unwrap();
        other.settings.telegram.api_id = Some(1);
        let themes = other.import(&json).unwrap();
        assert_eq!(themes.len(), 1);
        assert_eq!(other.settings.telegram.api_id, Some(1));
        assert_eq!(other.settings.themes.quick_css, "body{color:red}");
    }

    #[test]
    fn rejects_foreign_and_unsafe_backups() {
        let mut store = SettingsStore::open(&tempdir("settings-c")).unwrap();
        assert!(store.import(r#"{"format":"other","version":1,"created_at":"","app_version":"","settings":{}}"#).is_err());
        let evil = r#"{"format":"mellogram-backup","version":1,"created_at":"","app_version":"","settings":{},
            "themes":[{"file":"../../evil.css","css":""}]}"#;
        assert!(store.import(evil).is_err());
    }

    #[test]
    fn corrupt_file_is_moved_aside() {
        let dir = tempdir("settings-d");
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("settings.json"), "{not json").unwrap();
        let store = SettingsStore::open(&dir).unwrap();
        assert_eq!(store.settings, Settings::default());
        assert!(dir.join("settings.json.broken").exists());
    }
}
