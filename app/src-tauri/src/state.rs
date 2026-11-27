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
    /// A ban, removal, restriction or lost access. `alerts` are the texts
    /// plugins chose to raise (empty = record only).
    Membership { event: mellogram_core::relationships::MembershipEvent, alerts: Vec<String> },
    /// A user's online status changed.
    Presence { user_id: i64, presence: mellogram_telegram::Presence },
    /// Nothing for the UI (internal bookkeeping only).
    Quiet,
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
    pub background: crate::background::BackgroundState,
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
            background: Default::default(),
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
                let changed = matches!(event, TgEvent::ChatChanged { .. });
                let ui = state.process_event(event);
                if changed {
                    // A channel changed state: re-read the chat list and look for lost access.
                    let s = Arc::clone(&state);
                    tauri::async_runtime::spawn(async move { s.check_lost_access().await });
                }
                if let UiEvent::NewMessage(m) = &ui {
                    crate::background::notify_message(&state, m);
                }
                let _ = state.app.emit("mg://event", ui);
            }
        });
        Ok(())
    }

    fn process_event(&self, event: TgEvent) -> UiEvent {
        let plugins = self.plugins.lock().unwrap();
        match event {
            TgEvent::NewMessage(mut m) => {
                if let (Some(id), Some(name)) = (m.sender_id, m.sender_name.clone()) {
                    self.background.user_names.lock().unwrap().insert(id, name);
                }
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
            TgEvent::Membership(event) => {
                drop(plugins);
                self.membership(event)
            }
            TgEvent::ChatChanged { .. } => UiEvent::Quiet,
            TgEvent::Presence { user_id, presence } => UiEvent::Presence { user_id, presence },
        }
    }

    /// Enriches, de-duplicates, logs and alerts a membership event.
    pub fn membership(&self, mut event: mellogram_core::relationships::MembershipEvent) -> UiEvent {
        let bg = &self.background;
        if event.chat_title.is_none() {
            event.chat_title = bg.chat_titles.lock().unwrap().get(&event.chat_id).cloned();
        }
        {
            let names = bg.user_names.lock().unwrap();
            if event.user_name.is_none() {
                event.user_name = names.get(&event.user_id).cloned();
            }
            if event.actor_name.is_none() {
                event.actor_name = event.actor_id.and_then(|a| names.get(&a).cloned());
            }
        }
        if !bg.first_sighting(&event) {
            return UiEvent::Quiet; // same change reported twice (update + service message)
        }
        bg.log(event.clone());
        let alerts: Vec<String> = self.plugins.lock().unwrap().on_membership(&event).into_iter().map(|(_, t)| t).collect();
        for text in &alerts {
            crate::background::notify_text(self, "MelloGram · Bond Watch", text);
        }
        UiEvent::Membership { event, alerts }
    }

    /// Re-reads the full chat list and reports chats that disappeared.
    pub async fn check_lost_access(self: &Arc<Self>) {
        let Ok(tg) = self.telegram().await else { return };
        // Debounce bursts of channel updates.
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        const LIMIT: usize = 2000;
        let Ok(chats) = tg.dialogs(LIMIT).await else { return };
        let complete = chats.len() < LIMIT;
        let lost = self.background.update_known_chats(&chats, complete);
        for (chat_id, title) in lost {
            let event = mellogram_core::relationships::MembershipEvent {
                chat_id,
                chat_title: Some(title),
                user_id: 0,
                user_name: None,
                actor_id: None,
                actor_name: None,
                kind: mellogram_core::relationships::MembershipKind::AccessLost,
                until: None,
                is_self: true,
                date: chrono::Utc::now().timestamp(),
                source: mellogram_core::relationships::EventSource::ChatList,
            };
            let ui = self.membership(event);
            let _ = self.app.emit("mg://event", ui);
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
