//! The plugin contract.
//!
//! A plugin is a value implementing [`Plugin`]. Every hook has a default
//! no-op implementation, so a plugin only overrides what it needs. Hooks run on
//! the backend thread and must be fast and non-blocking; long work belongs in a
//! task spawned from [`Plugin::start`].

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::model::{IncomingMessage, OutgoingMessage};

/// Static description of a plugin, shown in the plugin manager.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct PluginManifest {
    /// Stable identifier, kebab-case. Used as the settings key.
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    pub authors: &'static [&'static str],
    pub version: &'static str,
    pub category: Category,
    /// Other plugin ids that must be enabled first. Enabling this plugin
    /// enables them automatically.
    pub dependencies: &'static [&'static str],
    pub platforms: &'static [Platform],
    /// Required plugins are always on and cannot be disabled.
    pub required: bool,
    pub enabled_by_default: bool,
    /// Lower runs earlier in hook chains. Default 0.
    pub priority: i32,
    /// Feature this plugin adapts, for the docs ("Vencord: ClearURLs").
    pub inspired_by: Option<&'static str>,
    /// Capabilities the plugin uses. Shown before enabling and when
    /// inspecting shared profiles; `Storage` is enforced by the host.
    pub permissions: &'static [Permission],
}

/// Declared plugin capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Permission {
    /// Sees message text as it arrives or loads.
    ReadMessages,
    /// Can rewrite or cancel what you send.
    ModifyOutgoing,
    /// Runs JavaScript in the window (adds buttons, menus, renderers).
    Ui,
    /// Keeps its own data file on this device.
    Storage,
    /// Makes network requests.
    Network,
    /// Reads or writes the clipboard.
    Clipboard,
    /// Injects CSS.
    Styles,
}

impl PluginManifest {
    pub const fn new(id: &'static str, name: &'static str, category: Category) -> Self {
        Self {
            id,
            name,
            description: "",
            authors: &["MelloGram"],
            version: "1.0.0",
            category,
            dependencies: &[],
            platforms: &[Platform::Windows, Platform::Linux, Platform::MacOs],
            required: false,
            enabled_by_default: false,
            priority: 0,
            inspired_by: None,
            permissions: &[],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Category {
    Appearance,
    Chat,
    Media,
    Privacy,
    Profiles,
    Notifications,
    Developer,
    Utility,
    Productivity,
    Accessibility,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Platform {
    Windows,
    Linux,
    MacOs,
}

impl Platform {
    pub fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::MacOs
        } else {
            Self::Linux
        }
    }
}

/// One configurable option. Rendered automatically by the settings UI.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SettingDef {
    pub key: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    #[serde(flatten)]
    pub kind: SettingKind,
    /// Changing it requires restarting the plugin.
    pub restart_needed: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum SettingKind {
    Bool { default: bool },
    Text { default: String, multiline: bool },
    Number { default: f64, min: Option<f64>, max: Option<f64> },
    Select { default: String, options: Vec<(String, String)> },
    /// A list of strings, e.g. replacement rules.
    List { default: Vec<String> },
}

impl SettingDef {
    pub fn boolean(key: &'static str, label: &'static str, default: bool) -> Self {
        Self::of(key, label, SettingKind::Bool { default })
    }

    pub fn text(key: &'static str, label: &'static str, default: &str) -> Self {
        Self::of(
            key,
            label,
            SettingKind::Text {
                default: default.into(),
                multiline: false,
            },
        )
    }

    pub fn number(key: &'static str, label: &'static str, default: f64, min: f64, max: f64) -> Self {
        Self::of(
            key,
            label,
            SettingKind::Number {
                default,
                min: Some(min),
                max: Some(max),
            },
        )
    }

    pub fn select(key: &'static str, label: &'static str, default: &str, options: &[(&str, &str)]) -> Self {
        Self::of(
            key,
            label,
            SettingKind::Select {
                default: default.into(),
                options: options
                    .iter()
                    .map(|(v, l)| ((*v).to_owned(), (*l).to_owned()))
                    .collect(),
            },
        )
    }

    pub fn list(key: &'static str, label: &'static str, default: &[&str]) -> Self {
        Self::of(
            key,
            label,
            SettingKind::List {
                default: default.iter().map(|s| (*s).to_owned()).collect(),
            },
        )
    }

    fn of(key: &'static str, label: &'static str, kind: SettingKind) -> Self {
        Self {
            key,
            label,
            description: "",
            kind,
            restart_needed: false,
        }
    }

    pub fn describe(mut self, description: &'static str) -> Self {
        self.description = description;
        self
    }

    pub fn default_value(&self) -> Value {
        match &self.kind {
            SettingKind::Bool { default } => Value::Bool(*default),
            SettingKind::Text { default, .. } => Value::String(default.clone()),
            SettingKind::Number { default, .. } => serde_json::json!(default),
            SettingKind::Select { default, .. } => Value::String(default.clone()),
            SettingKind::List { default } => serde_json::json!(default),
        }
    }

    /// Validates and normalises a value coming from the UI or a backup.
    pub fn validate(&self, value: &Value) -> Result<Value, String> {
        match (&self.kind, value) {
            (SettingKind::Bool { .. }, Value::Bool(_)) => Ok(value.clone()),
            (SettingKind::Text { .. }, Value::String(_)) => Ok(value.clone()),
            (SettingKind::Number { min, max, .. }, Value::Number(n)) => {
                let n = n.as_f64().ok_or("not a finite number")?;
                if min.is_some_and(|m| n < m) || max.is_some_and(|m| n > m) {
                    return Err(format!("{n} is out of range"));
                }
                Ok(value.clone())
            }
            (SettingKind::Select { options, .. }, Value::String(s)) => {
                if options.iter().any(|(v, _)| v == s) {
                    Ok(value.clone())
                } else {
                    Err(format!("`{s}` is not one of the options"))
                }
            }
            (SettingKind::List { .. }, Value::Array(items)) if items.iter().all(Value::is_string) => {
                Ok(value.clone())
            }
            _ => Err("wrong type".into()),
        }
    }
}

/// What a `before_send` hook decided.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SendDecision {
    Continue,
    /// Drop the message. The reason is shown to the user as a toast.
    Cancel(String),
}

/// A slash command offered in the composer, e.g. `/shrug`.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct CommandDef {
    pub name: String,
    pub description: String,
    pub plugin: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CommandOutcome {
    /// Not this plugin's command.
    NotHandled,
    /// The message was rewritten; continue sending it.
    Send,
    /// Command consumed the message; send nothing. Optional toast.
    Consumed(Option<String>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ToastLevel {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Toast {
    pub plugin: String,
    pub level: ToastLevel,
    pub message: String,
}

type ToastSink = Arc<dyn Fn(Toast) + Send + Sync>;

/// Per-call context handed to hooks: the plugin's own settings plus a way to
/// talk to the user.
#[derive(Clone)]
pub struct PluginContext {
    plugin: String,
    settings: Map<String, Value>,
    toasts: Option<ToastSink>,
}

impl PluginContext {
    pub fn new(plugin: impl Into<String>, settings: Map<String, Value>, toasts: Option<ToastSink>) -> Self {
        Self {
            plugin: plugin.into(),
            settings,
            toasts,
        }
    }

    /// Context with the plugin's declared defaults, handy in tests.
    pub fn with_defaults(plugin: &dyn Plugin) -> Self {
        let settings = plugin
            .settings()
            .iter()
            .map(|d| (d.key.to_owned(), d.default_value()))
            .collect();
        Self::new(plugin.manifest().id, settings, None)
    }

    pub fn plugin_id(&self) -> &str {
        &self.plugin
    }

    /// All resolved settings (declared keys only, defaults filled in).
    pub fn settings_map(&self) -> Map<String, Value> {
        self.settings.clone()
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.settings.get(key)
    }

    pub fn set(&mut self, key: &str, value: Value) {
        self.settings.insert(key.to_owned(), value);
    }

    pub fn bool(&self, key: &str) -> bool {
        self.get(key).and_then(Value::as_bool).unwrap_or(false)
    }

    pub fn str(&self, key: &str) -> &str {
        self.get(key).and_then(Value::as_str).unwrap_or("")
    }

    pub fn number(&self, key: &str) -> f64 {
        self.get(key).and_then(Value::as_f64).unwrap_or(0.0)
    }

    pub fn list(&self, key: &str) -> Vec<&str> {
        self.get(key)
            .and_then(Value::as_array)
            .map(|a| a.iter().filter_map(Value::as_str).collect())
            .unwrap_or_default()
    }

    pub fn toast(&self, level: ToastLevel, message: impl Into<String>) {
        if let Some(sink) = &self.toasts {
            sink(Toast {
                plugin: self.plugin.clone(),
                level,
                message: message.into(),
            });
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[error("{0}")]
pub struct PluginError(pub String);

/// Implemented by every MelloGram feature.
pub trait Plugin: Send + Sync + 'static {
    fn manifest(&self) -> PluginManifest;

    /// Options rendered in the plugin's settings panel.
    fn settings(&self) -> Vec<SettingDef> {
        Vec::new()
    }

    /// Called when the plugin is enabled (or on launch if already enabled).
    fn start(&self, _ctx: &PluginContext) -> Result<(), PluginError> {
        Ok(())
    }

    /// Called when the plugin is disabled or the client shuts down.
    fn stop(&self, _ctx: &PluginContext) {}

    /// Rewrite or cancel a message before it leaves the client.
    fn before_send(&self, _ctx: &PluginContext, _msg: &mut OutgoingMessage) -> SendDecision {
        SendDecision::Continue
    }

    /// Slash commands this plugin provides.
    fn commands(&self, _ctx: &PluginContext) -> Vec<(String, String)> {
        Vec::new()
    }

    /// Handle `/name args`. `msg.text` already has the command stripped to `args`.
    fn run_command(
        &self,
        _ctx: &PluginContext,
        _name: &str,
        _msg: &mut OutgoingMessage,
    ) -> CommandOutcome {
        CommandOutcome::NotHandled
    }

    /// A message arrived or was loaded from history. Decorate it.
    fn on_message(&self, _ctx: &PluginContext, _msg: &mut IncomingMessage) {}

    /// A message was edited. `previous` is the last version the client saw.
    fn on_edit(&self, _ctx: &PluginContext, _previous: Option<&IncomingMessage>, _msg: &mut IncomingMessage) {}

    /// Messages were deleted. Return `true` to keep showing them, marked deleted.
    fn on_delete(&self, _ctx: &PluginContext, _chat_id: Option<i64>, _ids: &[i32]) -> bool {
        false
    }

    /// Return `false` to stop the client from broadcasting "typing…".
    fn allow_typing(&self, _ctx: &PluginContext, _chat_id: i64) -> bool {
        true
    }

    /// CSS injected while the plugin is enabled.
    fn css(&self, _ctx: &PluginContext) -> Option<String> {
        None
    }

    /// Frontend half of the plugin. Evaluated in the UI with a `MG` API object
    /// and the plugin's settings; see `docs/PLUGIN_API.md`.
    fn ui_script(&self) -> Option<&'static str> {
        None
    }
}
