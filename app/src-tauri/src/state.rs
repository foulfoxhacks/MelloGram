//! Application state shared by every command.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use mellogram_core::{IncomingMessage, PluginManager, SettingsStore, ThemeManager, Toast};
use melloid_core::MelloIdStore;
use mellorpc_core::PresenceRegistry;
use mellogram_telegram::{Telegram, TelegramConfig, TgEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex as AsyncMutex};

/// Recent messages, so edits can be compared with what was on screen.
const MESSAGE_CACHE: usize = 20_000;

#[derive(Default)]
pub struct MessageCache {
    map: HashMap<(i64, i32), IncomingMessage>,
    order: VecDeque<(i64, i32)>,
}

impl MessageCache {
    pub fn insert(&mut self, msg: &IncomingMessage) {
        let key = (msg.chat_id, msg.id);
        if self.map.insert(key, msg.clone()).is_none() {
            self.order.push_back(key);
            if self.order.len() > MESSAGE_CACHE {
                if let Some(old) = self.order.pop_front() {
                    self.map.remove(&old);
                }
            }
        }
    }

    pub fn get(&self, chat_id: i64, id: i32) -> Option<&IncomingMessage> {
        self.map.get(&(chat_id, id))
    }
}

/// Payload of the `mg://event` channel the UI listens on.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum UiEvent {
    NewMessage(IncomingMessage),
    Edited(IncomingMessage),
    Deleted { chat_id: Option<i64>, ids: Vec<i32>, keep: bool },
    Disconnected { reason: String },
    LoggedIn,
}

/// Session counters for the Diagnostics page.
#[derive(Default)]
pub struct Counters {
    pub messages: AtomicU64,
    pub history_loads: AtomicU64,
    pub sends: AtomicU64,
    pub send_failures: AtomicU64,
    pub cancelled_sends: AtomicU64,
    pub disconnects: AtomicU64,
}

impl Counters {
    pub fn bump(c: &AtomicU64) {
        c.fetch_add(1, Ordering::Relaxed);
    }
}

pub struct AppState {
    pub app: AppHandle,
    pub started: Instant,
    /// Plugins, themes, appearance and QuickCSS are skipped this session.
    pub safe_mode: bool,
    /// The previous session did not shut down cleanly.
    pub unclean_shutdown: bool,
    pub counters: Counters,
    /// (failed attempts, locked-out-until) for the app-lock PIN.
    pub lock_failures: Mutex<(u32, Option<Instant>)>,
    pub dir: PathBuf,
    pub store: Mutex<SettingsStore>,
    /// Local MelloID profile cache; remote publication is handled by a future service.
    pub melloid: Mutex<MelloIdStore>,
    /// Active local RPC presence leases; remote publication is handled separately.
    pub rpc_presence: Mutex<PresenceRegistry>,
    pub plugins: Mutex<PluginManager>,
    pub themes: ThemeManager,
    pub cache: Mutex<MessageCache>,
    pub tg: AsyncMutex<Option<Arc<Telegram>>>,
}

impl AppState {
    pub fn open(app: AppHandle, dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let store = SettingsStore::open(&dir)?;
        let melloid = MelloIdStore::open(&dir)?;
        let themes = ThemeManager::new(dir.join("themes"))?;
        // Crash / safe-mode bookkeeping.
        let running = dir.join("running.lock");
        let unclean_shutdown = running.exists();
        let _ = std::fs::write(&running, std::process::id().to_string());
        let safe_marker = dir.join("safe-mode");
        let safe_mode = std::env::var_os("MELLOGRAM_SAFE_MODE").is_some()
            || std::env::args().any(|a| a == "--safe-mode")
            || safe_marker.exists();
        let _ = std::fs::remove_file(&safe_marker);
        if safe_mode {
            tracing::warn!("starting in safe mode: plugins, themes and custom CSS are off");
        }
        install_bundled_themes(&themes);

        let toast_app = app.clone();
        let sink = Arc::new(move |t: Toast| {
            let _ = toast_app.emit("mg://toast", t);
        });
        let mut plugins = PluginManager::new(mellogram_plugins::builtin(), Some(sink));
        if !safe_mode {
            plugins.init(&store.settings);
        }
        tracing::info!("config directory: {}", dir.display());
        Ok(Self {
            app,
            started: Instant::now(),
            safe_mode,
            unclean_shutdown,
            counters: Counters::default(),
            lock_failures: Mutex::new((0, None)),
            dir,
            store: Mutex::new(store),
            melloid: Mutex::new(melloid),
            rpc_presence: Mutex::new(PresenceRegistry::default()),
            plugins: Mutex::new(plugins),
            themes,
            cache: Mutex::new(MessageCache::default()),
            tg: AsyncMutex::new(None),
        })
    }


    /// Projects the newest unexpired local activity lease into the local MelloID
    /// profile. The hosted public profile service is a separate future stage.
    pub fn publish_presence(&self) {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .min(i64::MAX as u128) as i64;
        let next = self.rpc_presence.lock().unwrap().active(now_ms).into_iter()
            .max_by_key(|lease| lease.expires_at_ms)
            .map(|lease| lease.activity);
        let mut profile_store = self.melloid.lock().unwrap();
        let mut profile = profile_store.profile().clone();
        let visible = if profile.show_presence { next } else { None };
        if profile.presence != visible {
            profile.presence = visible.clone();
            if let Err(error) = profile_store.save(profile) {
                tracing::warn!("could not persist MelloID presence: {error}");
            }
            let _ = self.app.emit("mellorpc://presence", visible);
        }
    }

    pub fn session_path(&self) -> PathBuf {
        self.dir.join("telegram.session")
    }

    pub async fn connect_if_configured(self: &Arc<Self>) -> Result<(), String> {
        let creds = self.store.lock().unwrap().settings.telegram.clone();
        match (creds.api_id, creds.api_hash) {
            (Some(id), Some(hash)) if !hash.is_empty() => self.connect(id, hash).await,
            _ => Ok(()),
        }
    }

    pub async fn connect(self: &Arc<Self>, api_id: i32, api_hash: String) -> Result<(), String> {
        let tg = Telegram::connect(TelegramConfig { api_id, api_hash, session_path: self.session_path() })
            .await
            .map_err(|e| e.to_string())?;
        *self.tg.lock().await = Some(Arc::clone(&tg));
        if tg.client().is_authorized().await.unwrap_or(false) {
            self.start_updates().await?;
        }
        Ok(())
    }

    pub async fn telegram(&self) -> Result<Arc<Telegram>, String> {
        self.tg
            .lock()
            .await
            .clone()
            .ok_or_else(|| "Telegram is not configured yet — add your API ID and hash in Settings → Telegram.".into())
    }

    /// Pipes Telegram updates through the plugin hooks to the UI.
    pub async fn start_updates(self: &Arc<Self>) -> Result<(), String> {
        let tg = self.telegram().await?;
        let (tx, mut rx) = mpsc::unbounded_channel();
        tg.start_updates(tx).await.map_err(|e| e.to_string())?;
        let state = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            while let Some(event) = rx.recv().await {
                match &event {
                    TgEvent::NewMessage(_) => Counters::bump(&state.counters.messages),
                    TgEvent::Disconnected { .. } => Counters::bump(&state.counters.disconnects),
                    _ => {}
                }
                let ui = state.process_event(event);
                let _ = state.app.emit("mg://event", ui);
            }
        });
        Ok(())
    }

    fn process_event(&self, event: TgEvent) -> UiEvent {
        let plugins = self.plugins.lock().unwrap();
        match event {
            TgEvent::NewMessage(mut m) => {
                plugins.on_message(&mut m);
                self.cache.lock().unwrap().insert(&m);
                UiEvent::NewMessage(m)
            }
            TgEvent::Edited(mut m) => {
                let mut cache = self.cache.lock().unwrap();
                let previous = cache.get(m.chat_id, m.id).cloned();
                plugins.on_message(&mut m);
                plugins.on_edit(previous.as_ref(), &mut m);
                cache.map.insert((m.chat_id, m.id), m.clone());
                UiEvent::Edited(m)
            }
            TgEvent::Deleted { chat_id, ids } => {
                let keep = plugins.on_delete(chat_id, &ids);
                UiEvent::Deleted { chat_id, ids, keep }
            }
            TgEvent::Disconnected { reason } => UiEvent::Disconnected { reason },
        }
    }

    /// Runs history through the same hooks as live messages.
    pub fn decorate(&self, messages: &mut [IncomingMessage]) {
        let plugins = self.plugins.lock().unwrap();
        let mut cache = self.cache.lock().unwrap();
        for m in messages {
            if let Some(cached) = cache.get(m.chat_id, m.id) {
                // Keep history recorded earlier in the session.
                m.edit_history = cached.edit_history.clone();
                m.badges = cached.badges.clone();
            }
            plugins.on_message(m);
            cache.insert(m);
        }
    }

    pub fn stylesheet(&self) -> String {
        if self.safe_mode {
            return String::new();
        }
        let mut css = vec![];
        let store = self.store.lock().unwrap();
        let appearance = store.settings.appearance.to_css();
        if !appearance.is_empty() {
            css.push(("appearance-studio".to_owned(), appearance));
        }
        let dir = self.dir.clone();
        let wallpaper = store.settings.wallpapers.to_css(&|f| crate::wallpapers::image_url(&dir, f));
        if !wallpaper.is_empty() {
            css.push(("wallpaper-engine".to_owned(), wallpaper));
        }
        css.extend(self.plugins.lock().unwrap().css());
        self.themes.compose(&store.settings.themes, &css)
    }

    /// Restarts plugins from the current settings (after a workspace switch
    /// or restore) and pushes the new styles and plugin list to the UI.
    pub fn reload_plugins(&self) {
        if !self.safe_mode {
            let store = self.store.lock().unwrap();
            let mut plugins = self.plugins.lock().unwrap();
            plugins.shutdown();
            plugins.init(&store.settings);
        }
        self.refresh_styles();
        let _ = self.app.emit("mg://plugins-changed", ());
    }

    /// Re-sends the composed stylesheet to the UI (themes / plugin CSS changed).
    pub fn refresh_styles(&self) {
        let _ = self.app.emit("mg://stylesheet", self.stylesheet());
    }

    pub fn save(&self) -> Result<(), String> {
        self.store.lock().unwrap().save().map_err(|e| e.to_string())
    }

    pub fn shutdown(&self) {
        self.plugins.lock().unwrap().shutdown();
        self.store.lock().unwrap().settings.save_active_workspace();
        let _ = self.save();
        let _ = std::fs::remove_file(self.dir.join("running.lock"));
    }
}

/// Copies the example themes into the themes folder on first run.
fn install_bundled_themes(themes: &ThemeManager) {
    const BUNDLED: &[(&str, &str)] = &[
        ("Midnight Mello", include_str!("../../../themes/midnight-mello.theme.css")),
        ("Paper", include_str!("../../../themes/paper.theme.css")),
        ("Compact", include_str!("../../../themes/compact.theme.css")),
    ];
    let marker = themes.dir().join(".bundled-v1");
    if marker.exists() {
        return;
    }
    for (name, css) in BUNDLED {
        if let Err(e) = themes.install(name, css) {
            tracing::warn!("could not install bundled theme {name}: {e}");
        }
    }
    let _ = std::fs::write(marker, "");
}
