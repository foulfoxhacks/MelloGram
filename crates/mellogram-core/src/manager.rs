//! Plugin lifecycle and hook dispatch.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::model::{IncomingMessage, OutgoingMessage};
use crate::plugin::{
    CommandDef, CommandOutcome, Platform, Plugin, PluginContext, PluginManifest, SendDecision,
    SettingDef, Toast, ToastLevel,
};
use crate::settings::Settings;
use crate::{Error, Result};

/// A plugin that panics this many times is switched off for the session.
const MAX_FAULTS: u32 = 3;

type ToastSink = Arc<dyn Fn(Toast) + Send + Sync>;

/// What the plugin manager UI needs to render one row.
#[derive(Debug, Clone, Serialize)]
pub struct PluginInfo {
    #[serde(flatten)]
    pub manifest: PluginManifest,
    pub enabled: bool,
    pub favorite: bool,
    pub supported: bool,
    pub settings_schema: Vec<SettingDef>,
    pub settings: Map<String, Value>,
    pub has_ui: bool,
    pub errors: Vec<String>,
    /// Enabled plugins that depend on this one.
    pub required_by: Vec<String>,
    pub stats: HookStats,
}

/// Per-plugin performance counters (this session only).
#[derive(Debug, Clone, Copy, Default, Serialize)]
pub struct HookStats {
    pub calls: u64,
    pub total_us: u64,
    pub max_us: u64,
    pub start_us: u64,
}

/// What a shared profile would change, shown before importing it.
#[derive(Debug, Clone, Serialize)]
pub struct PackageReport {
    pub name: String,
    pub author: String,
    pub description: String,
    pub created_at: String,
    pub app_version: String,
    pub plugins: Vec<PackagePlugin>,
    pub unknown_plugins: Vec<String>,
    pub themes: Vec<PackageTheme>,
    pub quick_css_bytes: usize,
    pub appearance_customised: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackagePlugin {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub customised_settings: usize,
    pub permissions: Vec<crate::plugin::Permission>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageTheme {
    pub file: String,
    pub name: String,
    pub bytes: usize,
    pub enabled: bool,
}

pub struct PluginManager {
    /// Sorted by (priority, id) so hook order is deterministic.
    plugins: Vec<Arc<dyn Plugin>>,
    manifests: HashMap<String, PluginManifest>,
    enabled: HashSet<String>,
    contexts: HashMap<String, PluginContext>,
    faults: Mutex<HashMap<String, (u32, Vec<String>)>>,
    stats: Mutex<HashMap<String, HookStats>>,
    toasts: Option<ToastSink>,
}

impl PluginManager {
    pub fn new(plugins: Vec<Arc<dyn Plugin>>, toasts: Option<ToastSink>) -> Self {
        let mut plugins = plugins;
        plugins.sort_by_key(|p| {
            let m = p.manifest();
            (m.priority, m.id)
        });
        let manifests = plugins
            .iter()
            .map(|p| {
                let m = p.manifest();
                (m.id.to_owned(), m)
            })
            .collect::<HashMap<_, _>>();
        assert_eq!(manifests.len(), plugins.len(), "duplicate plugin ids");
        Self {
            plugins,
            manifests,
            enabled: HashSet::new(),
            contexts: HashMap::new(),
            faults: Mutex::new(HashMap::new()),
            stats: Mutex::new(HashMap::new()),
            toasts,
        }
    }

    fn find(&self, id: &str) -> Result<&Arc<dyn Plugin>> {
        self.plugins
            .iter()
            .find(|p| p.manifest().id == id)
            .ok_or_else(|| Error::UnknownPlugin(id.into()))
    }

    fn supported(m: &PluginManifest) -> bool {
        m.platforms.contains(&Platform::current())
    }

    /// Builds a context from stored values, filling in defaults and dropping
    /// keys the plugin no longer declares.
    fn context_for(&self, plugin: &dyn Plugin, settings: &Settings) -> PluginContext {
        let id = plugin.manifest().id;
        let stored = settings.plugins.get(id).map(|s| &s.settings);
        let values = plugin
            .settings()
            .iter()
            .map(|def| {
                let value = stored
                    .and_then(|s| s.get(def.key))
                    .and_then(|v| def.validate(v).ok())
                    .unwrap_or_else(|| def.default_value());
                (def.key.to_owned(), value)
            })
            .collect();
        PluginContext::new(id, values, self.toasts.clone())
    }

    fn wants_enabled(m: &PluginManifest, settings: &Settings) -> bool {
        m.required
            || settings
                .plugins
                .get(m.id)
                .and_then(|s| s.enabled)
                .unwrap_or(m.enabled_by_default)
    }

    /// Starts every plugin that should be on. Call once at launch.
    pub fn init(&mut self, settings: &Settings) {
        let ids: Vec<&'static str> = self
            .plugins
            .iter()
            .map(|p| p.manifest())
            .filter(|m| Self::wants_enabled(m, settings))
            .map(|m| m.id)
            .collect();
        for id in ids {
            if let Err(e) = self.start_with_deps(id, settings, &mut Vec::new()) {
                tracing::warn!("{e}");
                self.record_fault(id, e.to_string());
            }
        }
    }

    fn start_with_deps(&mut self, id: &str, settings: &Settings, chain: &mut Vec<String>) -> Result<Vec<String>> {
        if self.enabled.contains(id) {
            return Ok(Vec::new());
        }
        let manifest = self.manifests.get(id).cloned().ok_or_else(|| Error::UnknownPlugin(id.into()))?;
        if !Self::supported(&manifest) {
            return Err(Error::Unsupported(id.into()));
        }
        if chain.iter().any(|c| c == id) {
            return Err(Error::StartFailed { plugin: id.into(), reason: format!("dependency cycle {chain:?}") });
        }
        chain.push(id.to_owned());
        let mut started = Vec::new();
        for dep in manifest.dependencies {
            if !self.manifests.contains_key(*dep) {
                return Err(Error::MissingDependency { plugin: id.into(), dependency: (*dep).into() });
            }
            started.extend(self.start_with_deps(dep, settings, chain)?);
        }
        chain.pop();

        let plugin = Arc::clone(self.find(id)?);
        let ctx = self.context_for(plugin.as_ref(), settings);
        let started_at = std::time::Instant::now();
        let result = catch_unwind(AssertUnwindSafe(|| plugin.start(&ctx)));
        self.stats.lock().unwrap().entry(id.to_owned()).or_default().start_us = started_at.elapsed().as_micros() as u64;
        match result {
            Ok(Ok(())) => {}
            Ok(Err(e)) => return Err(Error::StartFailed { plugin: id.into(), reason: e.0 }),
            Err(_) => return Err(Error::StartFailed { plugin: id.into(), reason: "panicked".into() }),
        }
        self.contexts.insert(id.to_owned(), ctx);
        self.enabled.insert(id.to_owned());
        started.push(id.to_owned());
        Ok(started)
    }

    /// Enables a plugin (and its dependencies) and persists the choice.
    /// Returns every plugin id that was started.
    pub fn enable(&mut self, id: &str, settings: &mut Settings) -> Result<Vec<String>> {
        let started = self.start_with_deps(id, settings, &mut Vec::new())?;
        for sid in &started {
            settings.plugins.entry(sid.clone()).or_default().enabled = Some(true);
        }
        self.faults.lock().unwrap().remove(id);
        Ok(started)
    }

    pub fn disable(&mut self, id: &str, settings: &mut Settings) -> Result<()> {
        let manifest = self.manifests.get(id).ok_or_else(|| Error::UnknownPlugin(id.into()))?;
        if manifest.required {
            return Err(Error::RequiredBy { plugin: id.into(), dependents: vec!["MelloGram".into()] });
        }
        let dependents = self.dependents(id);
        if !dependents.is_empty() {
            return Err(Error::RequiredBy { plugin: id.into(), dependents });
        }
        self.stop(id);
        settings.plugins.entry(id.to_owned()).or_default().enabled = Some(false);
        Ok(())
    }

    fn stop(&mut self, id: &str) {
        if self.enabled.remove(id) {
            if let (Ok(plugin), Some(ctx)) = (self.find(id).cloned(), self.contexts.remove(id)) {
                let _ = catch_unwind(AssertUnwindSafe(|| plugin.stop(&ctx)));
            }
        }
    }

    pub fn shutdown(&mut self) {
        let ids: Vec<String> = self.enabled.iter().cloned().collect();
        for id in ids {
            self.stop(&id);
        }
    }

    fn dependents(&self, id: &str) -> Vec<String> {
        let mut out: Vec<String> = self
            .enabled
            .iter()
            .filter(|e| self.manifests[e.as_str()].dependencies.contains(&id))
            .cloned()
            .collect();
        out.sort();
        out
    }

    pub fn is_enabled(&self, id: &str) -> bool {
        self.enabled.contains(id) && !self.is_faulted(id)
    }

    /// Validates and stores one setting. Restarts the plugin if the setting
    /// asks for it so the new value takes effect.
    pub fn set_setting(&mut self, id: &str, key: &str, value: Value, settings: &mut Settings) -> Result<()> {
        let plugin = Arc::clone(self.find(id)?);
        let def = plugin
            .settings()
            .into_iter()
            .find(|d| d.key == key)
            .ok_or_else(|| Error::UnknownSetting { plugin: id.into(), key: key.into() })?;
        let value = def.validate(&value).map_err(|reason| Error::InvalidSetting { key: key.into(), reason })?;
        settings.plugins.entry(id.to_owned()).or_default().settings.insert(key.to_owned(), value);
        if self.enabled.contains(id) {
            let ctx = self.context_for(plugin.as_ref(), settings);
            if def.restart_needed {
                if let Some(old) = self.contexts.get(id) {
                    let _ = catch_unwind(AssertUnwindSafe(|| plugin.stop(old)));
                }
                let _ = catch_unwind(AssertUnwindSafe(|| plugin.start(&ctx)));
            }
            self.contexts.insert(id.to_owned(), ctx);
        }
        Ok(())
    }

    /// Restores a plugin's settings to defaults.
    pub fn reset_settings(&mut self, id: &str, settings: &mut Settings) -> Result<()> {
        let plugin = Arc::clone(self.find(id)?);
        settings.plugins.entry(id.to_owned()).or_default().settings.clear();
        if self.enabled.contains(id) {
            let ctx = self.context_for(plugin.as_ref(), settings);
            self.contexts.insert(id.to_owned(), ctx);
        }
        Ok(())
    }

    pub fn list(&self, settings: &Settings) -> Vec<PluginInfo> {
        let faults = self.faults.lock().unwrap();
        self.plugins
            .iter()
            .map(|p| {
                let manifest = p.manifest();
                let id = manifest.id;
                PluginInfo {
                    enabled: self.enabled.contains(id),
                    favorite: settings.plugins.get(id).is_some_and(|s| s.favorite),
                    supported: Self::supported(&manifest),
                    settings_schema: p.settings(),
                    settings: self.context_for(p.as_ref(), settings).settings_map(),
                    has_ui: p.ui_script().is_some(),
                    errors: faults.get(id).map(|(_, e)| e.clone()).unwrap_or_default(),
                    required_by: self.dependents(id),
                    stats: self.stats.lock().unwrap().get(id).copied().unwrap_or_default(),
                    manifest,
                }
            })
            .collect()
    }

    // ---- fault isolation -------------------------------------------------

    fn is_faulted(&self, id: &str) -> bool {
        self.faults.lock().unwrap().get(id).is_some_and(|(n, _)| *n >= MAX_FAULTS)
    }

    fn record_fault(&self, id: &str, message: String) {
        let mut faults = self.faults.lock().unwrap();
        let entry = faults.entry(id.to_owned()).or_default();
        entry.0 += 1;
        entry.1.push(message);
        if entry.1.len() > 10 {
            entry.1.remove(0);
        }
        if entry.0 == MAX_FAULTS {
            if let Some(sink) = &self.toasts {
                sink(Toast {
                    plugin: id.to_owned(),
                    level: ToastLevel::Error,
                    message: format!("{id} crashed repeatedly and was paused for this session"),
                });
            }
        }
    }

    /// Runs `f` for every healthy enabled plugin, in order.
    fn each<F: FnMut(&dyn Plugin, &PluginContext)>(&self, mut f: F) {
        for plugin in &self.plugins {
            let id = plugin.manifest().id;
            if !self.enabled.contains(id) || self.is_faulted(id) {
                continue;
            }
            let Some(ctx) = self.contexts.get(id) else { continue };
            let t = std::time::Instant::now();
            let result = catch_unwind(AssertUnwindSafe(|| f(plugin.as_ref(), ctx)));
            let us = t.elapsed().as_micros() as u64;
            {
                let mut stats = self.stats.lock().unwrap();
                let s = stats.entry(id.to_owned()).or_default();
                s.calls += 1;
                s.total_us += us;
                s.max_us = s.max_us.max(us);
            }
            if result.is_err() {
                tracing::error!("plugin `{id}` panicked in a hook");
                self.record_fault(id, "panicked while handling an event".into());
            }
        }
    }

    // ---- hooks -----------------------------------------------------------

    /// Slash command dispatch then `before_send` chain. Returns `None` when
    /// the message should not be sent, with an optional reason.
    pub fn process_outgoing(&self, mut msg: OutgoingMessage) -> std::result::Result<OutgoingMessage, Option<String>> {
        if let Some((name, args)) = parse_command(&msg.text) {
            let (name, args) = (name.to_owned(), args.to_owned());
            let original = std::mem::replace(&mut msg.text, args);
            let mut outcome = CommandOutcome::NotHandled;
            self.each(|p, ctx| {
                if outcome == CommandOutcome::NotHandled {
                    outcome = p.run_command(ctx, &name, &mut msg);
                }
            });
            match outcome {
                CommandOutcome::NotHandled => msg.text = original,
                CommandOutcome::Send => {}
                CommandOutcome::Consumed(reason) => return Err(reason),
            }
        }
        let mut cancel = None;
        self.each(|p, ctx| {
            if cancel.is_none() {
                if let SendDecision::Cancel(reason) = p.before_send(ctx, &mut msg) {
                    cancel = Some(reason);
                }
            }
        });
        if let Some(reason) = cancel {
            return Err(Some(reason));
        }
        if msg.text.trim().is_empty() && msg.attachments.is_empty() {
            return Err(None);
        }
        Ok(msg)
    }

    pub fn commands(&self) -> Vec<CommandDef> {
        let mut out = Vec::new();
        self.each(|p, ctx| {
            for (name, description) in p.commands(ctx) {
                out.push(CommandDef { name, description, plugin: ctx.plugin_id().to_owned() });
            }
        });
        out.sort_by(|a, b| a.name.cmp(&b.name));
        out
    }

    pub fn on_message(&self, msg: &mut IncomingMessage) {
        self.each(|p, ctx| p.on_message(ctx, msg));
    }

    pub fn on_edit(&self, previous: Option<&IncomingMessage>, msg: &mut IncomingMessage) {
        self.each(|p, ctx| p.on_edit(ctx, previous, msg));
    }

    /// `true` when any plugin wants the deleted messages kept on screen.
    pub fn on_delete(&self, chat_id: Option<i64>, ids: &[i32]) -> bool {
        let mut keep = false;
        self.each(|p, ctx| keep |= p.on_delete(ctx, chat_id, ids));
        keep
    }

    pub fn allow_typing(&self, chat_id: i64) -> bool {
        let mut allow = true;
        self.each(|p, ctx| allow &= p.allow_typing(ctx, chat_id));
        allow
    }

    /// `(plugin id, css)` for every enabled plugin that ships CSS.
    pub fn css(&self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        self.each(|p, ctx| {
            if let Some(css) = p.css(ctx) {
                out.push((ctx.plugin_id().to_owned(), css));
            }
        });
        out
    }

    /// `(plugin id, script, settings)` for the frontend.
    pub fn ui_scripts(&self) -> Vec<(String, &'static str, Map<String, Value>)> {
        let mut out = Vec::new();
        self.each(|p, ctx| {
            if let Some(script) = p.ui_script() {
                out.push((ctx.plugin_id().to_owned(), script, ctx.settings_map()));
            }
        });
        out
    }

    /// Summarises what importing a shared profile would do.
    pub fn inspect_package(&self, pkg: &crate::workspace::ProfilePackage) -> PackageReport {
        let ws = &pkg.workspace;
        let mut plugins = Vec::new();
        let mut unknown = Vec::new();
        let mut warnings = Vec::new();
        for (id, state) in &ws.plugins {
            match self.manifests.get(id) {
                Some(m) => {
                    let enabled = state.enabled.unwrap_or(m.enabled_by_default);
                    if enabled && m.permissions.contains(&crate::plugin::Permission::ModifyOutgoing) {
                        warnings.push(format!("{} will be able to change messages you send", m.name));
                    }
                    plugins.push(PackagePlugin {
                        id: id.clone(),
                        name: m.name.to_owned(),
                        enabled,
                        customised_settings: state.settings.len(),
                        permissions: m.permissions.to_vec(),
                    });
                }
                None => unknown.push(id.clone()),
            }
        }
        if !unknown.is_empty() {
            warnings.push(format!("{} plugin(s) are not installed and will be ignored", unknown.len()));
        }
        let quick_css_bytes = ws.themes.quick_css.len();
        if quick_css_bytes > 0 {
            warnings.push("contains custom CSS (it can restyle anything but cannot run code)".into());
        }
        let themes = pkg
            .themes
            .iter()
            .map(|t| PackageTheme {
                file: t.file.clone(),
                name: crate::theme::parse_meta(&t.css).name.unwrap_or_else(|| t.file.clone()),
                bytes: t.css.len(),
                enabled: ws.themes.enabled.contains(&t.file),
            })
            .collect();
        PackageReport {
            name: pkg.name.clone(),
            author: pkg.author.clone(),
            description: pkg.description.clone(),
            created_at: pkg.created_at.clone(),
            app_version: pkg.app_version.clone(),
            plugins,
            unknown_plugins: unknown,
            themes,
            quick_css_bytes,
            appearance_customised: ws.appearance != crate::appearance::Appearance::default(),
            warnings,
        }
    }

    /// Category → plugin ids, for the sidebar filter.
    pub fn categories(&self) -> BTreeMap<String, Vec<String>> {
        let mut out: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for m in self.manifests.values() {
            let key = serde_json::to_value(m.category).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default();
            out.entry(key).or_default().push(m.id.to_owned());
        }
        for ids in out.values_mut() {
            ids.sort();
        }
        out
    }
}

/// Splits `/name rest` into `("name", "rest")`. `//text` escapes a slash.
pub fn parse_command(text: &str) -> Option<(&str, &str)> {
    let body = text.strip_prefix('/')?;
    if body.starts_with('/') {
        return None;
    }
    let (name, args) = body.split_once(char::is_whitespace).unwrap_or((body, ""));
    let valid = !name.is_empty() && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    valid.then_some((name, args.trim_start()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::{Category, PluginError};
    use std::sync::atomic::{AtomicU32, Ordering};

    struct Upper;
    impl Plugin for Upper {
        fn manifest(&self) -> PluginManifest {
            PluginManifest { dependencies: &["base"], ..PluginManifest::new("upper", "Upper", Category::Chat) }
        }
        fn before_send(&self, _: &PluginContext, m: &mut OutgoingMessage) -> SendDecision {
            m.text = m.text.to_uppercase();
            SendDecision::Continue
        }
    }

    struct Base;
    impl Plugin for Base {
        fn manifest(&self) -> PluginManifest {
            PluginManifest { priority: -10, ..PluginManifest::new("base", "Base", Category::Utility) }
        }
        fn settings(&self) -> Vec<SettingDef> {
            vec![SettingDef::text("suffix", "Suffix", "!")]
        }
        fn before_send(&self, ctx: &PluginContext, m: &mut OutgoingMessage) -> SendDecision {
            m.text.push_str(ctx.str("suffix"));
            SendDecision::Continue
        }
        fn run_command(&self, _: &PluginContext, name: &str, m: &mut OutgoingMessage) -> CommandOutcome {
            match name {
                "echo" => CommandOutcome::Send,
                "noop" => CommandOutcome::Consumed(Some("done".into())),
                _ => {
                    let _ = m;
                    CommandOutcome::NotHandled
                }
            }
        }
    }

    struct Bomb(AtomicU32);
    impl Plugin for Bomb {
        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("bomb", "Bomb", Category::Developer)
        }
        fn on_message(&self, _: &PluginContext, _: &mut IncomingMessage) {
            self.0.fetch_add(1, Ordering::SeqCst);
            panic!("boom");
        }
    }

    struct FailsToStart;
    impl Plugin for FailsToStart {
        fn manifest(&self) -> PluginManifest {
            PluginManifest::new("fails", "Fails", Category::Developer)
        }
        fn start(&self, _: &PluginContext) -> std::result::Result<(), PluginError> {
            Err(PluginError("nope".into()))
        }
    }

    fn manager() -> PluginManager {
        PluginManager::new(
            vec![Arc::new(Upper), Arc::new(Base), Arc::new(Bomb(AtomicU32::new(0))), Arc::new(FailsToStart)],
            None,
        )
    }

    #[test]
    fn enabling_pulls_in_dependencies_and_blocks_disabling_them() {
        let mut m = manager();
        let mut s = Settings::default();
        let started = m.enable("upper", &mut s).unwrap();
        assert_eq!(started, vec!["base", "upper"]);
        assert!(matches!(m.disable("base", &mut s), Err(Error::RequiredBy { .. })));
        m.disable("upper", &mut s).unwrap();
        m.disable("base", &mut s).unwrap();
        assert_eq!(s.plugins["base"].enabled, Some(false));
    }

    #[test]
    fn hooks_run_in_priority_order_with_settings() {
        let mut m = manager();
        let mut s = Settings::default();
        m.enable("upper", &mut s).unwrap();
        m.set_setting("base", "suffix", Value::String("?".into()), &mut s).unwrap();
        // base (priority -10) appends first, then upper uppercases.
        let out = m.process_outgoing(OutgoingMessage::text(1, "hi")).unwrap();
        assert_eq!(out.text, "HI?");
        assert!(m.set_setting("base", "suffix", Value::Bool(true), &mut s).is_err());
        assert!(m.set_setting("base", "nope", Value::Bool(true), &mut s).is_err());
    }

    #[test]
    fn commands_are_dispatched() {
        let mut m = manager();
        let mut s = Settings::default();
        m.enable("base", &mut s).unwrap();
        assert_eq!(m.process_outgoing(OutgoingMessage::text(1, "/echo hey")).unwrap().text, "hey!");
        assert_eq!(m.process_outgoing(OutgoingMessage::text(1, "/noop")), Err(Some("done".into())));
        assert_eq!(m.process_outgoing(OutgoingMessage::text(1, "/unknown x")).unwrap().text, "/unknown x!");
        assert_eq!(parse_command("//escaped"), None);
        assert_eq!(parse_command("/a  b c"), Some(("a", "b c")));
    }

    #[test]
    fn panicking_plugin_is_paused_after_repeated_faults() {
        let mut m = manager();
        let mut s = Settings::default();
        m.enable("bomb", &mut s).unwrap();
        let mut msg = IncomingMessage::default();
        for _ in 0..5 {
            m.on_message(&mut msg);
        }
        assert!(!m.is_enabled("bomb"));
        let info = m.list(&s).into_iter().find(|p| p.manifest.id == "bomb").unwrap();
        assert_eq!(info.errors.len(), 3);
    }

    #[test]
    fn start_failure_is_reported() {
        let mut m = manager();
        let mut s = Settings::default();
        assert!(matches!(m.enable("fails", &mut s), Err(Error::StartFailed { .. })));
        assert!(!m.is_enabled("fails"));
    }
}
