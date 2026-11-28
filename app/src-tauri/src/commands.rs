//! `invoke` handlers. Every command returns `Result<_, String>` so errors show
//! up in the UI as readable toasts.

use std::io::Read as _;
use std::sync::Arc;

use mellogram_core::{CommandDef, OutgoingFile, OutgoingMessage, PluginInfo, Theme};
use mellogram_core::settings::{GeneralSettings, NotificationSettings, RpcSettings};
use mellogram_telegram::{ChatSummary, LoginState, SendResult};
use melloid_core::{BadgeCategory, MelloProfile};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tauri::{Emitter, State};

use crate::state::{AppState, UiEvent};

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

// ---- app & settings ----------------------------------------------------------

#[tauri::command]
pub fn app_info(state: S<'_>) -> Value {
    json!({
        "name": "MelloGram",
        "version": env!("CARGO_PKG_VERSION"),
        "platform": std::env::consts::OS,
        "arch": std::env::consts::ARCH,
        "config_dir": state.dir,
        "themes_dir": state.themes.dir(),
        "changelog": include_str!("../../../CHANGELOG.md"),
    })
}

#[tauri::command]
pub fn get_settings(state: S<'_>) -> Value {
    let store = state.store.lock().unwrap();
    let s = &store.settings;
    json!({
        "general": s.general,
        "notifications": s.notifications,
        "rpc": s.rpc,
        "telegram": { "api_id": s.telegram.api_id, "has_hash": s.telegram.api_hash.is_some() },
        "themes": { "enabled": s.themes.enabled, "quick_css": s.themes.quick_css,
                    "quick_css_enabled": s.themes.quick_css_enabled, "variables": s.themes.variables },
    })
}

#[tauri::command]
pub fn set_general(state: S<'_>, general: GeneralSettings) -> R<()> {
    let launch = general.launch_at_login;
    let changed = state.store.lock().unwrap().settings.general.launch_at_login != launch;
    state.store.lock().unwrap().settings.general = general;
    state.save()?;
    if changed {
        crate::background::apply_autostart(&state.app, launch);
    }
    Ok(())
}

#[tauri::command]
pub fn set_notifications(state: S<'_>, notifications: NotificationSettings) -> R<()> {
    state.store.lock().unwrap().settings.notifications = notifications;
    state.save()
}

#[tauri::command]
pub fn set_rpc_settings(state: S<'_>, rpc: RpcSettings) -> R<()> {
    state.store.lock().unwrap().settings.rpc = rpc;
    state.save()
}

#[tauri::command]
pub async fn set_credentials(state: S<'_>, api_id: i32, api_hash: String) -> R<()> {
    let api_hash = api_hash.trim().to_owned();
    if api_id <= 0 || api_hash.len() != 32 || !api_hash.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err("API ID must be a positive number and API hash 32 hex characters (from my.telegram.org).".into());
    }
    {
        let mut store = state.store.lock().unwrap();
        store.settings.telegram.api_id = Some(api_id);
        store.settings.telegram.api_hash = Some(api_hash.clone());
    }
    state.save()?;
    state.connect(api_id, api_hash).await
}

// ---- MelloID identity ------------------------------------------------------------

/// Returns the locally stored MelloID profile. The profile is not published
/// until an authenticated public-profile service is connected.
#[tauri::command]
pub fn get_melloid_profile(state: S<'_>) -> MelloProfile {
    state.melloid.lock().unwrap().profile().clone()
}

/// Validates and atomically persists the editable MelloID profile.
#[tauri::command]
pub fn save_melloid_profile(state: S<'_>, mut profile: MelloProfile) -> R<MelloProfile> {
    let mut store = state.melloid.lock().unwrap();
    let previous = store.profile();

    // Preserve verifier claims only when the underlying account identity has
    // not changed. Changing provider/account/link invalidates the old claim.
    for connection in &mut profile.connections {
        if let Some(trusted) = previous.connections.iter().find(|record| {
            record.id == connection.id
                && record.provider == connection.provider
                && record.display_name == connection.display_name
                && record.profile_url == connection.profile_url
        }) {
            connection.verification_issuer = trusted.verification_issuer.clone();
            connection.verified_at = trusted.verified_at.clone();
        } else {
            connection.verification_issuer = None;
            connection.verified_at = None;
        }
    }

    // Only self-curated cosmetic collectibles may be authored by the client.
    // Keep server-issued and trusted badge categories from the prior store.
    let trusted_badges = previous.badges.iter()
        .filter(|badge| badge.server_issued || badge.category != BadgeCategory::Collectible)
        .cloned()
        .collect::<Vec<_>>();
    profile.badges.retain(|badge| badge.category == BadgeCategory::Collectible && !badge.server_issued);
    for badge in &mut profile.badges {
        badge.issuer = "self".into();
        badge.awarded_at = None;
        badge.server_issued = false;
    }
    profile.badges.extend(trusted_badges);

    store.save(profile).map_err(err)?;
    Ok(store.profile().clone())
}

// ---- login -----------------------------------------------------------------------

async fn after_login(state: &Arc<AppState>, login: LoginState) -> R<LoginState> {
    if matches!(login, LoginState::Ready { .. }) {
        state.start_updates().await?;
        let _ = state.app.emit("mg://event", UiEvent::LoggedIn);
    }
    Ok(login)
}

#[tauri::command]
pub async fn login_state(state: S<'_>) -> R<Option<LoginState>> {
    let Some(tg) = state.tg.lock().await.clone() else { return Ok(None) };
    tg.login_state().await.map(Some).map_err(err)
}

#[tauri::command]
pub async fn request_code(state: S<'_>, phone: String) -> R<LoginState> {
    state.telegram().await?.request_code(&phone).await.map_err(err)
}

#[tauri::command]
pub async fn submit_code(state: S<'_>, code: String) -> R<LoginState> {
    let login = state.telegram().await?.submit_code(&code).await.map_err(err)?;
    after_login(&state, login).await
}

#[tauri::command]
pub async fn submit_password(state: S<'_>, password: String) -> R<LoginState> {
    let login = state.telegram().await?.submit_password(&password).await.map_err(err)?;
    after_login(&state, login).await
}

#[tauri::command]
pub async fn logout(state: S<'_>) -> R<()> {
    state.telegram().await?.logout().await.map_err(err)?;
    *state.tg.lock().await = None;
    let _ = std::fs::remove_file(state.session_path());
    state.connect_if_configured().await
}

// ---- chats -----------------------------------------------------------------------

#[tauri::command]
pub async fn list_chats(state: S<'_>, limit: Option<usize>) -> R<Vec<ChatSummary>> {
    let mut chats = state.telegram().await?.dialogs(limit.unwrap_or(200)).await.map_err(err)?;
    let limit = limit.unwrap_or(200);
    // Never show an overlay ciphertext as the chat's last message.
    for chat in &mut chats {
        if chat.last_text.as_deref().is_some_and(mellogram_e2e::is_overlay_text) {
            chat.last_text = Some("🔒 Encrypted message".into());
        }
    }
    let complete = chats.len() < limit;
    let lost = state.background.update_known_chats(&chats, complete);
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
        let ui = state.membership(event);
        let _ = state.app.emit("mg://event", ui);
    }
    Ok(chats)
}

#[tauri::command]
pub async fn load_history(state: S<'_>, chat_id: i64, before: Option<i32>) -> R<Vec<mellogram_core::IncomingMessage>> {
    let mut msgs = state.telegram().await?.history(chat_id, 50, before).await.map_err(err)?;
    crate::state::Counters::bump(&state.counters.history_loads);
    state.decorate(&mut msgs);
    Ok(msgs)
}

#[derive(Debug, Deserialize)]
pub struct SendRequest {
    chat_id: i64,
    text: String,
    #[serde(default)]
    reply_to: Option<i32>,
    #[serde(default)]
    silent: bool,
    #[serde(default)]
    attachments: Vec<String>,
    /// Deliver later (unix seconds): a Telegram scheduled message.
    #[serde(default)]
    schedule_at: Option<i64>,
}

/// Telegram accepts scheduled messages up to 365 days ahead; anything in the
/// past (or within the next 10 seconds) is refused rather than sent now.
pub fn validate_schedule(at: i64, now: i64) -> Result<i64, String> {
    if at < now + 10 {
        return Err("Pick a time in the future.".into());
    }
    if at > now + 365 * 24 * 3600 {
        return Err("Telegram only schedules up to a year ahead.".into());
    }
    Ok(at)
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum SendOutcome {
    Sent(SendResult),
    /// A plugin consumed or cancelled the message.
    Cancelled { reason: Option<String> },
    /// Queued on Telegram's servers for delivery at `at`; not in the chat yet.
    Scheduled { at: i64 },
}

#[tauri::command]
pub async fn send_message(state: S<'_>, request: SendRequest) -> R<SendOutcome> {
    let mut msg = OutgoingMessage::text(request.chat_id, request.text);
    msg.reply_to = request.reply_to;
    msg.silent = request.silent;
    if let Some(at) = request.schedule_at {
        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs() as i64).unwrap_or(0);
        msg.schedule_at = Some(validate_schedule(at, now)?);
    }
    msg.attachments = request
        .attachments
        .into_iter()
        .map(|p| {
            let path = std::path::PathBuf::from(p);
            let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_else(|| "file".into());
            OutgoingFile { path, file_name }
        })
        .collect();
    let processed = state.plugins.lock().unwrap().process_outgoing(msg);
    let mut msg = match processed {
        Ok(m) => m,
        Err(reason) => {
            crate::state::Counters::bump(&state.counters.cancelled_sends);
            return Ok(SendOutcome::Cancelled { reason });
        }
    };
    let tg = state.telegram().await?;
    // End-to-end overlay: the text is encrypted here and only the ciphertext
    // ever reaches Telegram. The plaintext is kept locally for the bubble.
    let mut encrypted_plaintext: Option<String> = None;
    if state.e2e.chat_enabled(msg.chat_id) {
        if !msg.attachments.is_empty() {
            return Err("Encrypted messages can't carry attachments yet — send the file separately or turn encryption off for this chat.".into());
        }
        let me = tg.ensure_self_id().await.map_err(err)?;
        state.e2e.set_self_id(me);
        let plain = msg.text.clone();
        msg.text = if msg.chat_id < 0 {
            state.e2e.group_encrypt(msg.chat_id, &plain)?
        } else {
            state.e2e.encrypt_for_chat(msg.chat_id, &plain)?
        };
        msg.link_preview = false;
        encrypted_plaintext = Some(plain);
    }
    let scheduled = msg.schedule_at;
    let out_bytes = msg.text.len() as u64 + msg.attachments.iter().filter_map(|a| std::fs::metadata(&a.path).ok()).map(|m| m.len()).sum::<u64>();
    let sent = tg.send(&msg).await;
    state.traffic.record("out", "telegram", "messages.sendMessage", out_bytes, if scheduled.is_some() { "scheduled message" } else { "message" });
    if sent.is_err() {
        crate::state::Counters::bump(&state.counters.send_failures);
    }
    let mut result = sent.map_err(err)?;
    crate::state::Counters::bump(&state.counters.sends);
    if let Some(plain) = encrypted_plaintext {
        state.e2e.remember_sent(result.message.chat_id, result.message.id, &plain);
        result.message.text = plain;
        result.message.add_class("mg-e2e");
        result.message.badges.push(mellogram_core::MessageBadge {
            plugin: "e2e".into(),
            label: "🔒".into(),
            tooltip: Some("Sent with the MelloGram end-to-end overlay".into()),
        });
    }
    if let Some(at) = scheduled {
        return Ok(SendOutcome::Scheduled { at });
    }
    state.decorate(std::slice::from_mut(&mut result.message));
    Ok(SendOutcome::Sent(result))
}

#[cfg(test)]
mod schedule_tests {
    use super::validate_schedule;

    #[test]
    fn schedule_window() {
        let now = 1_000_000;
        assert!(validate_schedule(now - 5, now).is_err());
        assert!(validate_schedule(now + 5, now).is_err());
        assert_eq!(validate_schedule(now + 3600, now), Ok(now + 3600));
        assert!(validate_schedule(now + 366 * 24 * 3600, now).is_err());
    }
}

/// Called by the composer while typing; plugins may veto the broadcast.
#[tauri::command]
pub async fn typing(state: S<'_>, chat_id: i64) -> R<bool> {
    if !state.plugins.lock().unwrap().allow_typing(chat_id) {
        return Ok(false);
    }
    state.telegram().await?.send_typing(chat_id).await.map_err(err)?;
    Ok(true)
}

#[tauri::command]
pub async fn mark_read(state: S<'_>, chat_id: i64) -> R<()> {
    state.telegram().await?.mark_read(chat_id).await.map_err(err)
}

#[tauri::command]
pub async fn mark_all_read(state: S<'_>) -> R<usize> {
    state.telegram().await?.mark_all_read().await.map_err(err)
}

#[tauri::command]
pub async fn delete_messages(state: S<'_>, chat_id: i64, ids: Vec<i32>) -> R<()> {
    state.telegram().await?.delete(chat_id, &ids).await.map_err(err)
}

#[tauri::command]
pub async fn edit_message(state: S<'_>, chat_id: i64, id: i32, text: String) -> R<()> {
    if state.e2e.chat_enabled(chat_id) {
        return Err("Encrypted messages can't be edited — the ciphertext on Telegram's side would no longer match. Send a new message instead.".into());
    }
    state.telegram().await?.edit(chat_id, id, &text).await.map_err(err)
}

// ---- plugins ---------------------------------------------------------------------

fn plugins_changed(state: &AppState) -> R<()> {
    state.save()?;
    state.refresh_styles();
    let _ = state.app.emit("mg://plugins-changed", ());
    Ok(())
}

#[tauri::command]
pub fn list_plugins(state: S<'_>) -> Vec<PluginInfo> {
    let store = state.store.lock().unwrap();
    state.plugins.lock().unwrap().list(&store.settings)
}

#[tauri::command]
pub fn set_plugin_enabled(state: S<'_>, id: String, enabled: bool) -> R<Vec<String>> {
    if enabled && state.safe_mode {
        return Err("MelloGram is in safe mode; restart normally to enable plugins.".into());
    }
    let changed = {
        let mut store = state.store.lock().unwrap();
        let mut plugins = state.plugins.lock().unwrap();
        if enabled {
            plugins.enable(&id, &mut store.settings).map_err(err)?
        } else {
            plugins.disable(&id, &mut store.settings).map_err(err)?;
            vec![id]
        }
    };
    plugins_changed(&state)?;
    Ok(changed)
}

#[tauri::command]
pub fn set_plugin_setting(state: S<'_>, id: String, key: String, value: Value) -> R<()> {
    {
        let mut store = state.store.lock().unwrap();
        state.plugins.lock().unwrap().set_setting(&id, &key, value, &mut store.settings).map_err(err)?;
    }
    plugins_changed(&state)
}

#[tauri::command]
pub fn reset_plugin_settings(state: S<'_>, id: String) -> R<()> {
    {
        let mut store = state.store.lock().unwrap();
        state.plugins.lock().unwrap().reset_settings(&id, &mut store.settings).map_err(err)?;
    }
    plugins_changed(&state)
}

#[tauri::command]
pub fn set_plugin_favorite(state: S<'_>, id: String, favorite: bool) -> R<()> {
    state.store.lock().unwrap().plugin(&id).favorite = favorite;
    state.save()
}

#[tauri::command]
pub fn list_commands(state: S<'_>) -> Vec<CommandDef> {
    state.plugins.lock().unwrap().commands()
}

#[tauri::command]
pub fn ui_plugins(state: S<'_>) -> Vec<Value> {
    state
        .plugins
        .lock()
        .unwrap()
        .ui_scripts()
        .into_iter()
        .map(|(id, script, settings)| json!({ "id": id, "script": script, "settings": settings }))
        .collect()
}

// ---- themes ----------------------------------------------------------------------

#[tauri::command]
pub fn list_themes(state: S<'_>) -> R<Vec<Theme>> {
    state.themes.list().map_err(err)
}

#[tauri::command]
pub fn get_theme_source(state: S<'_>, file: String) -> R<String> {
    state.themes.get(&file).map(|t| t.css).map_err(err)
}

#[tauri::command]
pub fn install_theme(state: S<'_>, name: String, css: String) -> R<Theme> {
    if css.len() > 2 * 1024 * 1024 {
        return Err("theme is larger than 2 MB".into());
    }
    let theme = state.themes.install(&name, &css).map_err(err)?;
    state.refresh_styles();
    Ok(theme)
}

#[tauri::command]
pub async fn install_theme_url(state: S<'_>, url: String) -> R<Theme> {
    if !url.starts_with("https://") {
        return Err("theme URLs must use https://".into());
    }
    let fetch_url = url.clone();
    let css = tauri::async_runtime::spawn_blocking(move || -> R<String> {
        let agent = ureq::AgentBuilder::new().try_proxy_from_env(true).build();
        let resp = agent.get(&fetch_url).call().map_err(err)?;
        // Byte count recorded below once the size is known.
        let mut body = String::new();
        resp.into_reader().take(2 * 1024 * 1024 + 1).read_to_string(&mut body).map_err(err)?;
        if body.len() > 2 * 1024 * 1024 {
            return Err("theme is larger than 2 MB".into());
        }
        Ok(body)
    })
    .await
    .map_err(err)??;
    state.traffic.record("in", "https", crate::traffic::redact_url(&url), css.len() as u64, "theme download");
    let name = url.rsplit('/').next().unwrap_or("theme").trim_end_matches(".css").to_owned();
    install_theme(state, name, css)
}

#[tauri::command]
pub fn remove_theme(state: S<'_>, file: String) -> R<()> {
    {
        let mut store = state.store.lock().unwrap();
        state.themes.remove(&file, &mut store.settings.themes).map_err(err)?;
    }
    state.save()?;
    state.refresh_styles();
    Ok(())
}

#[tauri::command]
pub fn set_enabled_themes(state: S<'_>, files: Vec<String>) -> R<()> {
    for f in &files {
        state.themes.get(f).map_err(err)?;
    }
    state.store.lock().unwrap().settings.themes.enabled = files;
    state.save()?;
    state.refresh_styles();
    Ok(())
}

#[tauri::command]
pub fn set_theme_variable(state: S<'_>, file: String, name: String, value: Option<String>) -> R<()> {
    if !name.starts_with("--") {
        return Err("variable names start with --".into());
    }
    {
        let mut store = state.store.lock().unwrap();
        let vars = store.settings.themes.variables.entry(file).or_default();
        match value {
            Some(v) => vars.insert(name, v),
            None => vars.remove(&name),
        };
    }
    state.save()?;
    state.refresh_styles();
    Ok(())
}

#[tauri::command]
pub fn set_quick_css(state: S<'_>, css: String, enabled: bool) -> R<()> {
    {
        let mut store = state.store.lock().unwrap();
        store.settings.themes.quick_css = css;
        store.settings.themes.quick_css_enabled = enabled;
    }
    state.save()?;
    state.refresh_styles();
    Ok(())
}

#[tauri::command]
pub fn get_stylesheet(state: S<'_>) -> String {
    state.stylesheet()
}

// ---- backup ----------------------------------------------------------------------

#[tauri::command]
pub fn export_backup(state: S<'_>) -> R<String> {
    let themes = state.themes.list().map_err(err)?.into_iter().map(|t| (t.file, t.css)).collect();
    let backup = state.store.lock().unwrap().export(themes);
    serde_json::to_string_pretty(&backup).map_err(err)
}

#[tauri::command]
pub fn import_backup(state: S<'_>, json: String) -> R<usize> {
    let themes = state.store.lock().unwrap().import(&json).map_err(err)?;
    for t in &themes {
        state.themes.install(&t.file, &t.css).map_err(err)?;
    }
    state.reload_plugins();
    Ok(themes.len())
}

/// Bond Watch: recent bans, removals, restrictions and lost access (newest first).
#[tauri::command]
pub fn membership_log(state: S<'_>) -> Vec<mellogram_core::relationships::MembershipEvent> {
    state.background.log_snapshot()
}

/// Full info for the chat info panel (user, group, supergroup or channel).
#[tauri::command]
pub async fn chat_info(state: S<'_>, chat_id: i64) -> R<mellogram_telegram::ChatInfo> {
    state.telegram().await?.chat_info(chat_id).await.map_err(err)
}

/// Members of a group or channel, where Telegram lets you see them.
#[tauri::command]
pub async fn chat_members(state: S<'_>, chat_id: i64, limit: Option<usize>) -> R<Vec<mellogram_telegram::Member>> {
    state.telegram().await?.chat_members(chat_id, limit.unwrap_or(100)).await.map_err(err)
}
