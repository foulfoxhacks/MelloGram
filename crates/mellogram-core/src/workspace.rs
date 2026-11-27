//! Workspaces and shareable profile packages.
//!
//! A *workspace* is a complete customisation environment (plugins and their
//! settings, themes, QuickCSS, appearance, notifications). Switching saves the
//! live configuration back into the active workspace and loads the target.
//!
//! A *profile package* is a workspace plus the theme files it uses, as a single
//! JSON file that can be shared. Packages are always inspected before import
//! and are imported as a new, inactive workspace; nothing runs until the user
//! switches to it.

use serde::{Deserialize, Serialize};

use crate::appearance::Appearance;
use crate::settings::{BackupTheme, NotificationSettings, PluginState, Settings, ThemeSettings};
use crate::{Error, Result};

pub const PACKAGE_FORMAT: &str = "mellogram-profile";
pub const PACKAGE_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub plugins: std::collections::BTreeMap<String, PluginState>,
    pub themes: ThemeSettings,
    pub appearance: Appearance,
    pub wallpapers: crate::wallpaper::WallpaperSettings,
    pub notifications: NotificationSettings,
    pub updated_at: String,
}

impl Default for Workspace {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: "Workspace".into(),
            icon: "🗂".into(),
            description: String::new(),
            plugins: Default::default(),
            themes: ThemeSettings::default(),
            appearance: Appearance::default(),
            wallpapers: Default::default(),
            notifications: NotificationSettings::default(),
            updated_at: String::new(),
        }
    }
}

/// Summary for the workspace switcher (no bulky settings).
#[derive(Debug, Clone, Serialize)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub icon: String,
    pub description: String,
    pub active: bool,
    pub enabled_plugins: usize,
    pub themes: usize,
    pub updated_at: String,
}

fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn slug(name: &str) -> String {
    let s: String = name
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c.to_ascii_lowercase() } else { '-' })
        .collect();
    let mut collapsed = String::with_capacity(s.len());
    for c in s.chars() {
        if !(c == '-' && collapsed.ends_with('-')) {
            collapsed.push(c);
        }
    }
    let s = collapsed.trim_matches('-').to_owned();
    if s.is_empty() { "workspace".into() } else { s.chars().take(40).collect() }
}

impl Settings {
    /// Captures the live configuration as a workspace.
    pub fn snapshot(&self, id: String, name: String) -> Workspace {
        Workspace {
            id,
            name,
            plugins: self.plugins.clone(),
            themes: self.themes.clone(),
            appearance: self.appearance.clone(),
            wallpapers: self.wallpapers.clone(),
            notifications: self.notifications.clone(),
            updated_at: now(),
            ..Workspace::default()
        }
    }

    fn apply_workspace(&mut self, ws: &Workspace) {
        self.plugins = ws.plugins.clone();
        self.themes = ws.themes.clone();
        self.appearance = ws.appearance.clone();
        self.wallpapers = ws.wallpapers.clone();
        self.notifications = ws.notifications.clone();
    }

    fn unique_id(&self, name: &str) -> String {
        let base = slug(name);
        let mut id = base.clone();
        let mut n = 2;
        while self.workspaces.iter().any(|w| w.id == id) {
            id = format!("{base}-{n}");
            n += 1;
        }
        id
    }

    pub fn workspace_summaries(&self) -> Vec<WorkspaceSummary> {
        self.workspaces
            .iter()
            .map(|w| WorkspaceSummary {
                id: w.id.clone(),
                name: w.name.clone(),
                icon: w.icon.clone(),
                description: w.description.clone(),
                active: self.active_workspace.as_deref() == Some(&w.id),
                enabled_plugins: w.plugins.values().filter(|p| p.enabled == Some(true)).count(),
                themes: w.themes.enabled.len(),
                updated_at: w.updated_at.clone(),
            })
            .collect()
    }

    /// Writes the live configuration into the active workspace, if any.
    pub fn save_active_workspace(&mut self) {
        let Some(id) = self.active_workspace.clone() else { return };
        let snap = self.snapshot(id.clone(), String::new());
        if let Some(ws) = self.workspaces.iter_mut().find(|w| w.id == id) {
            ws.plugins = snap.plugins;
            ws.themes = snap.themes;
            ws.appearance = snap.appearance;
            ws.wallpapers = snap.wallpapers;
            ws.notifications = snap.notifications;
            ws.updated_at = snap.updated_at;
        }
    }

    /// Creates a workspace from the current configuration and makes it active.
    pub fn create_workspace(&mut self, name: &str, icon: &str) -> String {
        self.save_active_workspace();
        let id = self.unique_id(name);
        let mut ws = self.snapshot(id.clone(), name.trim().to_owned());
        if !icon.trim().is_empty() {
            ws.icon = icon.trim().chars().take(4).collect();
        }
        self.workspaces.push(ws);
        self.active_workspace = Some(id.clone());
        id
    }

    pub fn switch_workspace(&mut self, id: &str) -> Result<()> {
        let target = self
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or_else(|| Error::InvalidSetting { key: "workspace".into(), reason: format!("unknown workspace `{id}`") })?;
        self.save_active_workspace();
        self.apply_workspace(&target);
        self.active_workspace = Some(target.id);
        Ok(())
    }

    pub fn duplicate_workspace(&mut self, id: &str) -> Result<String> {
        self.save_active_workspace();
        let mut copy = self
            .workspaces
            .iter()
            .find(|w| w.id == id)
            .cloned()
            .ok_or_else(|| Error::InvalidSetting { key: "workspace".into(), reason: format!("unknown workspace `{id}`") })?;
        copy.name = format!("{} (copy)", copy.name);
        copy.id = self.unique_id(&copy.name);
        copy.updated_at = now();
        let new_id = copy.id.clone();
        self.workspaces.push(copy);
        Ok(new_id)
    }

    pub fn update_workspace_meta(&mut self, id: &str, name: &str, icon: &str, description: &str) -> Result<()> {
        let ws = self
            .workspaces
            .iter_mut()
            .find(|w| w.id == id)
            .ok_or_else(|| Error::InvalidSetting { key: "workspace".into(), reason: format!("unknown workspace `{id}`") })?;
        if !name.trim().is_empty() {
            ws.name = name.trim().chars().take(60).collect();
        }
        if !icon.trim().is_empty() {
            ws.icon = icon.trim().chars().take(4).collect();
        }
        ws.description = description.trim().chars().take(280).collect();
        Ok(())
    }

    /// Deleting the active workspace keeps the live configuration as-is.
    pub fn delete_workspace(&mut self, id: &str) {
        self.workspaces.retain(|w| w.id != id);
        if self.active_workspace.as_deref() == Some(id) {
            self.active_workspace = None;
        }
    }

    /// Adds a package's workspace as a new, inactive workspace.
    pub fn add_imported_workspace(&mut self, mut ws: Workspace) -> String {
        ws.id = self.unique_id(&ws.name);
        ws.updated_at = now();
        let id = ws.id.clone();
        self.workspaces.push(ws);
        id
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProfilePackage {
    pub format: String,
    pub version: u32,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub app_version: String,
    pub workspace: Workspace,
    #[serde(default)]
    pub themes: Vec<BackupTheme>,
}

impl ProfilePackage {
    pub fn new(workspace: Workspace, author: &str, themes: Vec<BackupTheme>) -> Self {
        Self {
            format: PACKAGE_FORMAT.into(),
            version: PACKAGE_VERSION,
            name: workspace.name.clone(),
            author: author.trim().to_owned(),
            description: workspace.description.clone(),
            created_at: now(),
            app_version: env!("CARGO_PKG_VERSION").into(),
            workspace,
            themes,
        }
    }

    pub fn parse(json: &str) -> Result<Self> {
        if json.len() > 8 * 1024 * 1024 {
            return Err(Error::InvalidBackup("profile package is larger than 8 MB".into()));
        }
        let pkg: Self = serde_json::from_str(json)?;
        if pkg.format != PACKAGE_FORMAT {
            return Err(Error::InvalidBackup(format!("not a MelloGram profile (format `{}`)", pkg.format)));
        }
        if pkg.version > PACKAGE_VERSION {
            return Err(Error::InvalidBackup(format!("profile needs a newer MelloGram (format v{})", pkg.version)));
        }
        for t in &pkg.themes {
            if crate::theme::sanitize_file_name(&t.file).as_deref() != Some(t.file.as_str()) {
                return Err(Error::InvalidBackup(format!("unsafe theme file name `{}`", t.file)));
            }
        }
        Ok(pkg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspaces_switch_and_preserve_changes() {
        let mut s = Settings::default();
        s.themes.quick_css = "a{}".into();
        let personal = s.create_workspace("Personal", "🏠");
        s.themes.quick_css = "personal{}".into();

        let work = s.create_workspace("Work", "💼");
        assert_eq!(s.active_workspace.as_deref(), Some(work.as_str()));
        s.themes.quick_css = "work{}".into();
        s.appearance.bubble_radius = 2;

        s.switch_workspace(&personal).unwrap();
        assert_eq!(s.themes.quick_css, "personal{}");
        assert_eq!(s.appearance.bubble_radius, 10);

        s.switch_workspace(&work).unwrap();
        assert_eq!(s.themes.quick_css, "work{}");
        assert_eq!(s.appearance.bubble_radius, 2);

        let copy = s.duplicate_workspace(&work).unwrap();
        assert_eq!(copy, "work-copy");
        assert_eq!(s.workspace_summaries().len(), 3);
        s.delete_workspace(&work);
        assert_eq!(s.active_workspace, None);
        assert!(s.switch_workspace("nope").is_err());
    }

    #[test]
    fn package_roundtrip_and_validation() {
        let mut s = Settings::default();
        let id = s.create_workspace("Midnight Fox", "🦊");
        let ws = s.workspaces.iter().find(|w| w.id == id).unwrap().clone();
        let pkg = ProfilePackage::new(ws, "tester", vec![BackupTheme { file: "a.css".into(), css: "x".into() }]);
        let json = serde_json::to_string(&pkg).unwrap();
        let parsed = ProfilePackage::parse(&json).unwrap();
        assert_eq!(parsed.name, "Midnight Fox");
        let new_id = s.add_imported_workspace(parsed.workspace);
        assert_eq!(new_id, "midnight-fox-2");
        assert_eq!(s.active_workspace.as_deref(), Some(id.as_str()), "import must not activate");

        let bad = json.replace("a.css", "../x.css");
        assert!(ProfilePackage::parse(&bad).is_err());
        assert!(ProfilePackage::parse(r#"{"format":"x","version":1,"name":"","workspace":{}}"#).is_err());
    }
}
