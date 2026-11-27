//! `invoke` handlers. Every command returns `Result<_, String>` so errors show
//! up in the UI as readable toasts.

use std::io::Read as _;
use std::sync::Arc;

use mellogram_core::{CommandDef, OutgoingFile, OutgoingMessage, PluginInfo, Theme};
use mellogram_core::settings::{GeneralSettings, NotificationSettings};
use mellogram_telegram::{ChatSummary, LoginState, SendResult};
use melloid_core::MelloProfile;
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
        "telegram": { "api_id": s.telegram.api_id, "has_hash": s.telegram.api_hash.is_some() },
        "themes": { "enabled": s.themes.enabled, "quick_css": s.themes.quick_css,
                    "quick_css_enabled": s.themes.quick_css_enabled, "variables": s.themes.variables },
    })
}

#[tauri::command]
pub fn set_general(state: S<'_>, general: GeneralSettings) -> R<()> {
    state.store.lock().unwrap().settings.general = general;
    state.save()
}

#[tauri::command]
pub fn set_notifications(state: S<'_>, notifications: NotificationSettings) -> R<()> {
    state.store.lock().unwrap().settings.notifications = notifications;
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
pub fn save_melloid_profile(state: S<'_>, mut profile: MelloProfile) -> R<()> {
    // Verification and awarded badges are reserved for a trusted issuer.
    // The local editor cannot manufacture trust claims or achievement grants.
    for connection in &mut profile.connections {
        connection.verification_issuer = None;
        connection.verified_at = None;
    }
    profile.badges.clear();
    state.melloid.lock().unwrap().save(profile).map_err(err)
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
    state.telegram().await?.dialogs(limit.unwrap_or(200)).await.map_err(err)
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
}

#[derive(Debug, Serialize)]
#[serde(tag = "status", rename_all = "kebab-case")]
pub enum SendOutcome {
    Sent(SendResult),
    /// A plugin consumed or cancelled the message.
    Cancelled { reason: Option<String> },
}

#[tauri::command]
pub async fn send_message(state: S<'_>, request: SendRequest) -> R<SendOutcome> {
    let mut msg = OutgoingMessage::text(request.chat_id, request.text);
    msg.reply_to = request.reply_to;
    msg.silent = request.silent;
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
    let msg = match processed {
        Ok(m) => m,
        Err(reason) => {
            crate::state::Counters::bump(&state.counters.cancelled_sends);
            return Ok(SendOutcome::Cancelled { reason });
        }
    };
    let sent = state.telegram().await?.send(&msg).await;
    if sent.is_err() {
        crate::state::Counters::bump(&state.counters.send_failures);
    }
    let mut result = sent.map_err(err)?;
    crate::state::Counters::bump(&state.counters.sends);
    state.decorate(std::slice::from_mut(&mut result.message));
    Ok(SendOutcome::Sent(result))
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
        let mut body = String::new();
        resp.into_reader().take(2 * 1024 * 1024 + 1).read_to_string(&mut body).map_err(err)?;
        if body.len() > 2 * 1024 * 1024 {
            return Err("theme is larger than 2 MB".into());
        }
        Ok(body)
    })
    .await
    .map_err(err)??;
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
