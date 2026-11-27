//! MelloGram core: the backend-neutral heart of the client.
//!
//! * [`plugin`] — the [`Plugin`](plugin::Plugin) trait every feature implements.
//! * [`manager`] — loads plugins, resolves dependencies, dispatches hooks and
//!   isolates plugin panics so one broken plugin cannot take the client down.
//! * [`settings`] — the on-disk settings store plus backup / restore.
//! * [`theme`] — CSS theme discovery, metadata, ordering and QuickCSS.
//! * [`model`] — the message types plugins see. They are independent of the
//!   Telegram library so plugins can be unit-tested without a network.
#![forbid(unsafe_code)]

pub mod appearance;
pub mod background;
pub mod manager;
pub mod model;
pub mod plugin;
pub mod relationships;
pub mod settings;
pub mod theme;
pub mod wallpaper;
pub mod workspace;

pub use manager::{PluginInfo, PluginManager};
pub use model::{GeoInfo, IncomingMessage, MediaInfo, MediaKind, MessageBadge, OutgoingFile, OutgoingMessage};
pub use plugin::{
    Category, CommandDef, Permission, CommandOutcome, Platform, Plugin, PluginContext, PluginError,
    PluginManifest, SendDecision, SettingDef, SettingKind, Toast, ToastLevel,
};
pub use settings::{Backup, Settings, SettingsStore};
pub use theme::{Theme, ThemeManager, ThemeMeta};
pub use appearance::Appearance;
pub use wallpaper::{Wallpaper, WallpaperSettings};
pub use workspace::{ProfilePackage, Workspace, WorkspaceSummary};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("invalid JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unknown plugin `{0}`")]
    UnknownPlugin(String),
    #[error("plugin `{plugin}` requires `{dependency}`, which is not installed")]
    MissingDependency { plugin: String, dependency: String },
    #[error("cannot disable `{plugin}`: required by {dependents:?}")]
    RequiredBy { plugin: String, dependents: Vec<String> },
    #[error("plugin `{0}` is not available on this platform")]
    Unsupported(String),
    #[error("plugin `{plugin}` failed to start: {reason}")]
    StartFailed { plugin: String, reason: String },
    #[error("unknown setting `{key}` for plugin `{plugin}`")]
    UnknownSetting { plugin: String, key: String },
    #[error("invalid value for `{key}`: {reason}")]
    InvalidSetting { key: String, reason: String },
    #[error("unknown theme `{0}`")]
    UnknownTheme(String),
    #[error("invalid backup: {0}")]
    InvalidBackup(String),
}

pub type Result<T, E = Error> = std::result::Result<T, E>;
