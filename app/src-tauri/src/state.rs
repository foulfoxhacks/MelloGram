//! Application state shared by every command.

use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use mellogram_core::{IncomingMessage, PluginManager, SettingsStore, ThemeManager, Toast};
use melloid_core::MelloIdStore;
use mellorpc_core::PresenceRegistry;
use mellogram_telegram::{Telegram, TelegramConfig, TgEvent};
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, watch, Mutex as AsyncMutex};

use mellogram_e2e::{E2eState, GroupDistribution};

/// Recent messages, so edits can be compared with what was on screen.
const MESSAGE_CACHE: usize = 20_000;
/// How many members one sweep reads. Hitting this means the list is partial.
/// Hard cap on a roster read. Large groups are followed to this many members;
/// above it the read is reported as incomplete, never as a full roster.
const SWEEP_MEMBER_CAP: usize = 5_000;

/// Upper bound for one connect attempt (dial, auth key, `initConnection`). The
/// login screen gives `login_state` 15 s, so a real error arrives before its
/// own generic "unreachable" message.
pub const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(12);

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
    pub media: crate::media::MediaState,
    pub tg: AsyncMutex<Option<Arc<Telegram>>>,
    /// Held across a connect attempt: concurrent callers wait for the one in
    /// flight instead of opening a second socket and update loop.
    pub connect_lock: AsyncMutex<()>,
    /// The client whose update loop is feeding the UI, with its stop signal, so a
    /// replaced client can be shut down instead of delivering every update twice
    /// (and so starting updates twice for one client is a no-op).
    pub updates_stop: AsyncMutex<Option<(Arc<Telegram>, watch::Sender<bool>)>>,
    /// Set when the published client's update stream ended for good. A client in
    /// that state can still answer requests but will never deliver updates, so
    /// `ensure_connected` re-dials instead of reusing it.
    pub connection_stale: AtomicBool,
    /// End-to-end privacy overlay: identity, sessions and per-chat mode.
    pub e2e: E2eState,
    /// Redacted record of network exchanges (Settings → Traffic Log plugin).
    pub traffic: crate::traffic::TrafficLog,
    /// Sealed third-party API keys.
    pub secrets: crate::secrets::Secrets,
}

impl AppState {
    pub fn open(app: AppHandle, dir: PathBuf) -> Result<Self, Box<dyn std::error::Error>> {
        let store = SettingsStore::open(&dir)?;
        let melloid = MelloIdStore::open(&dir)?;
        let themes = ThemeManager::new(dir.join("themes"))?;
        let e2e = E2eState::open(&dir);
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
        // The first question every login report asks: are the API credentials
        // there, and is a session already saved? Flags only, never the values.
        let telegram = &store.settings.telegram;
        tracing::info!(
            "telegram: api_id {}, api_hash {}, session {}",
            if matches!(telegram.api_id, Some(id) if id > 0) { "set" } else { "missing" },
            if telegram.api_hash.as_deref().map_or(false, |h| h.len() == 32) { "set" } else { "missing" },
            if dir.join("telegram.session").exists() { "present" } else { "absent" },
        );
        let secrets = crate::secrets::Secrets::new(&dir);
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
            media: Default::default(),
            tg: AsyncMutex::new(None),
            connect_lock: AsyncMutex::new(()),
            updates_stop: AsyncMutex::new(None),
            connection_stale: AtomicBool::new(false),
            e2e,
            traffic: crate::traffic::TrafficLog::default(),
            secrets,
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

    /// Connects with the saved credentials, bounded by `timeout` and serialised
    /// with every other connect attempt. Safe to call from a command: if the
    /// startup task already connected while this waited, the existing client is
    /// reused instead of a second one being stacked on top of it.
    pub async fn ensure_connected(self: &Arc<Self>, timeout: std::time::Duration) -> Result<(), String> {
        if self.has_live_client().await {
            return Ok(());
        }
        let attempt = async {
            let _serial = self.connect_lock.lock().await;
            if self.has_live_client().await {
                return Ok(());
            }
            self.connect_configured_locked().await
        };
        match tokio::time::timeout(timeout, attempt).await {
            Ok(result) => result,
            Err(_) => Err(format!(
                "Telegram did not answer within {}s. Check the connection and try again.",
                timeout.as_secs()
            )),
        }
    }

    /// A published client only counts while its update loop is alive. One whose
    /// stream died stays visible to `telegram()` (requests may still work) but is
    /// re-dialled by the next connect, which is what makes Retry recover.
    async fn has_live_client(&self) -> bool {
        self.tg.lock().await.is_some() && !self.connection_stale.load(Ordering::Relaxed)
    }

    /// Caller must hold `connect_lock`.
    async fn connect_configured_locked(self: &Arc<Self>) -> Result<(), String> {
        // Normalised again here so a credential cleared at runtime still falls
        // back to the built-in application instead of silently disabling connect.
        let creds = self.store.lock().unwrap().settings.telegram.clone().normalized();
        match (creds.api_id, creds.api_hash) {
            (Some(id), Some(hash)) => self.connect_locked(id, hash).await,
            _ => Ok(()),
        }
    }

    pub async fn connect(self: &Arc<Self>, api_id: i32, api_hash: String) -> Result<(), String> {
        let _serial = self.connect_lock.lock().await;
        self.connect_locked(api_id, api_hash).await
    }

    /// Caller must hold `connect_lock`. Nothing is written to `self.tg` until the
    /// whole attempt has succeeded: a dial that fails, an update loop that
    /// refuses to start, or a timeout that cancels this future all leave shared
    /// state exactly as they found it (no client for a first connect, the
    /// working client for a credential change). Publishing first would let
    /// `login_state`'s fast path hand out a client that never delivers updates.
    async fn connect_locked(self: &Arc<Self>, api_id: i32, api_hash: String) -> Result<(), String> {
        let net = self.store.lock().unwrap().settings.network.clone();
        let proxy_url = crate::network::proxy_for_connect(self, net.proxy_url).filter(|u| mellogram_core::settings::validate_proxy_url(u).is_ok());
        let tg = Telegram::connect(TelegramConfig {
            api_id,
            api_hash,
            session_path: self.session_path(),
            obfuscated: net.obfuscated_transport,
            proxy_url,
            app_version: env!("CARGO_PKG_VERSION").into(),
        })
            .await
            .map_err(|e| e.to_string())?;
        let authorized = tg.client().is_authorized().await.unwrap_or(false);
        // Past this point the attempt replaces whatever is published, so the old
        // client's loop is stopped: two live loops would deliver the same update
        // twice. Until the new client is published the old one is marked stale,
        // so a failure here sends the next connect (Retry) back to the dialler
        // instead of reusing a client whose loop just went quiet.
        // Stale first: if this attempt is cancelled between the two calls below,
        // the old client is already marked as having no loop, so the next connect
        // re-dials instead of trusting it.
        self.connection_stale.store(true, Ordering::Relaxed);
        self.stop_updates().await;
        // Keep the published client locked until the new loop and client are
        // installed. An immediately exiting loop can then mark the new client
        // stale after publication rather than being mistaken for an old loop.
        let mut published = self.tg.lock().await;
        if authorized {
            // The overlay binds messages to our own user id.
            if let Ok(mellogram_telegram::LoginState::Ready { me }) = tg.login_state().await {
                self.e2e.set_self_id(me.id);
            }
            // Errors from here leave `self.tg` untouched, so the attempt has
            // nothing half-connected to clean up.
            self.start_updates_for(Arc::clone(&tg)).await?;
        }
        // Last step: only a client whose update loop is already running is
        // published, so every caller that sees it can rely on it.
        *published = Some(tg);
        self.connection_stale.store(false, Ordering::Relaxed);
        Ok(())
    }

    pub async fn telegram(&self) -> Result<Arc<Telegram>, String> {
        self.tg
            .lock()
            .await
            .clone()
            .ok_or_else(|| "Telegram is not configured yet — add your API ID and hash in Settings → Telegram.".into())
    }

    /// Sends queued group key replies through their pairwise sessions.
    pub async fn deliver_group_replies(
        &self,
        replies: Vec<GroupDistribution>,
    ) -> Result<(), String> {
        let tg = self.telegram().await?;
        for reply in replies {
            let armored = match self.e2e.encrypt_for_peer(reply.peer_id, &reply.text) {
                Ok(armored) => armored,
                Err(e) => {
                    tracing::warn!("could not encrypt a group key reply for {}: {e}", reply.peer_id);
                    continue;
                }
            };
            let message = mellogram_core::OutgoingMessage {
                chat_id: reply.peer_id,
                text: armored,
                link_preview: false,
                ..Default::default()
            };
            if let Err(e) = tg.send(&message).await {
                tracing::warn!("could not send a group key reply to {}: {e}", reply.peer_id);
            }
        }
        Ok(())
    }

    /// Re-reads the member list of every encrypted group. A removal that never
    /// arrived as an event still forces a rotation this way.
    pub async fn sweep_group_keys(&self) -> Result<usize, String> {
        let groups = self.e2e.encrypted_groups();
        if groups.is_empty() {
            return Ok(0);
        }
        let tg = self.telegram().await?;
        let me = tg.ensure_self_id().await.map_err(|e| e.to_string())?;
        self.e2e.set_self_id(me);
        let mut flagged = 0;
        for chat_id in groups {
            match tg.chat_members_all(chat_id, SWEEP_MEMBER_CAP).await {
                Ok((members, complete)) => {
                    let ids: Vec<i64> = members
                        .iter()
                        .map(|m| m.id)
                        .filter(|id| *id != me)
                        .collect();
                    if self.e2e.group_note_members(chat_id, &ids, complete).rotation_needed {
                        flagged += 1;
                    }
                }
                Err(e) => {
                    tracing::debug!("group key sweep could not read members of {chat_id}: {e}")
                }
            }
        }
        Ok(flagged)
    }

    /// Starts the periodic key sweep. Runs a little after sign-in, then every
    /// half hour while the app is open.
    pub fn start_key_sweep(self: &Arc<Self>) {
        let state = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(90)).await;
            loop {
                if let Err(e) = state.sweep_group_keys().await {
                    tracing::debug!("group key sweep skipped: {e}");
                }
                tokio::time::sleep(std::time::Duration::from_secs(30 * 60)).await;
            }
        });
    }

    /// Pipes Telegram updates through the plugin hooks to the UI.
    pub async fn start_updates(self: &Arc<Self>) -> Result<(), String> {
        let tg = self.telegram().await?;
        self.start_updates_for(tg).await
    }

    /// Stops the running update loop, if any. Called when the client it belongs
    /// to is replaced or signed out, so no loop outlives its client.
    pub async fn stop_updates(&self) {
        if let Some((_, stop)) = self.updates_stop.lock().await.take() {
            let _ = stop.send(true);
        }
    }

    /// Starts the update loop for `tg`, which does not have to be published in
    /// `self.tg` yet. `connect_locked` uses this to start updates *before*
    /// publishing, so a failure here leaves no client behind. If the caller is
    /// cancelled after the loop starts, its receiver is dropped and the loop
    /// ends on its next send.
    async fn start_updates_for(self: &Arc<Self>, tg: Arc<Telegram>) -> Result<(), String> {
        let (tx, mut rx) = mpsc::unbounded_channel();
        tg.start_updates(tx).await.map_err(|e| e.to_string())?;
        // The new subscription is live by now, so the previous loop is told to
        // stop: only one loop may feed the UI, or a replaced client keeps
        // delivering alongside its replacement. `watch` remembers the value, so
        // a stop sent before the old loop next waits for it is not lost.
        let (stop_tx, mut stop_rx) = watch::channel(false);
        {
            let mut slot = self.updates_stop.lock().await;
            // `Telegram::start_updates` is idempotent: for a client that is already
            // feeding the UI it returns without starting a second stream. Keep
            // that loop, instead of stopping it and spawning one that can never
            // receive anything.
            if slot.as_ref().is_some_and(|(owner, _)| Arc::ptr_eq(owner, &tg)) {
                return Ok(());
            }
            if let Some((_, previous)) = slot.replace((Arc::clone(&tg), stop_tx)) {
                let _ = previous.send(true);
            }
        }
        let client = tg;
        let state = Arc::clone(self);
        tauri::async_runtime::spawn(async move {
            // Set only when this loop is asked to stop. Every other way it ends
            // leaves a client that will never deliver updates again.
            let mut stopped = false;
            loop {
                // `biased` checks the stop first every iteration. Without it a
                // backlog of queued updates can win the poll and a replaced loop
                // delivers several more events before it notices it was stopped.
                let event = tokio::select! {
                    biased;
                    _ = stop_rx.changed() => {
                        // Only a `true` is our stop; a sender that went away
                        // without saying so is an unexpected exit.
                        stopped = *stop_rx.borrow();
                        None
                    }
                    event = rx.recv() => event,
                };
                let Some(event) = event else { break };
                match &event {
                    TgEvent::NewMessage(m) => {
                        Counters::bump(&state.counters.messages);
                        state.traffic.record("in", "telegram", "updates.newMessage", m.text.len() as u64, "message");
                    }
                    TgEvent::Edited(m) => state.traffic.record("in", "telegram", "updates.editMessage", m.text.len() as u64, "edit"),
                    TgEvent::Disconnected { .. } => Counters::bump(&state.counters.disconnects),
                    _ => {}
                }
                let changed = matches!(event, TgEvent::ChatChanged { .. });
                let ui = state.process_event(event);
                // A decrypted message may have been a group key request; answer
                // it over the same pairwise session it arrived on.
                let replies = state.e2e.take_replies();
                if !replies.is_empty() {
                    let s = Arc::clone(&state);
                    tauri::async_runtime::spawn(async move {
                        if let Err(e) = s.deliver_group_replies(replies).await {
                            tracing::warn!("group key replies were not delivered: {e}");
                        }
                    });
                }
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
            // Exiting any other way — the stream ended, or its sender vanished —
            // means this client will never deliver again. It can still answer
            // requests, so it has to be marked stale or the next connect would
            // trust it and Retry would never re-dial. Only while it is still the
            // published client, never one that is being replaced right now.
            if !stopped {
                let current = state.tg.lock().await;
                if current.as_ref().is_some_and(|c| Arc::ptr_eq(c, &client)) {
                    state.connection_stale.store(true, Ordering::Relaxed);
                }
            }
        });
        self.start_key_sweep();
        Ok(())
    }

    fn process_event(&self, event: TgEvent) -> UiEvent {
        let plugins = self.plugins.lock().unwrap();
        match event {
            TgEvent::NewMessage(mut m) => {
                if let (Some(id), Some(name)) = (m.sender_id, m.sender_name.clone()) {
                    self.background.user_names.lock().unwrap().insert(id, name);
                }
                // Overlay messages are decrypted before plugins and the UI see them.
                let _ = self.e2e.apply_incoming(&mut m);
                plugins.on_message(&mut m);
                self.cache.lock().unwrap().insert(&m);
                UiEvent::NewMessage(m)
            }
            TgEvent::Edited(mut m) => {
                let mut cache = self.cache.lock().unwrap();
                let previous = cache.get(m.chat_id, m.id).cloned();
                let _ = self.e2e.apply_incoming(&mut m);
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
        // A group that loses a member must rotate its sender keys, or the
        // person who left could still read what is sent afterwards.
        if event.chat_id < 0
            && matches!(
                event.kind,
                mellogram_core::relationships::MembershipKind::Banned
                    | mellogram_core::relationships::MembershipKind::Removed
                    | mellogram_core::relationships::MembershipKind::Left
            )
        {
            self.e2e.group_mark_rotation(event.chat_id);
        }
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
            let _ = self.e2e.apply_incoming(m);
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
        self.e2e.flush();
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
