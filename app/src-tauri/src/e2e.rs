//! `invoke` handlers for the end-to-end privacy overlay.
//!
//! The protocol work, the sealed store and the per-chat rules live in the
//! `mellogram-e2e` crate so they are covered by the workspace test run; this
//! module only exposes them to the UI.

use std::sync::Arc;

use mellogram_e2e::{ChatView, E2eStatus, PeerView};
use tauri::State;

use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

#[tauri::command]
pub fn e2e_status(state: S<'_>) -> E2eStatus {
    state.e2e.status()
}

/// Returns your public key, creating the identity on first call.
#[tauri::command]
pub fn e2e_identity(state: S<'_>) -> R<String> {
    state.e2e.identity_bundle()
}

#[tauri::command]
pub fn e2e_reset_identity(state: S<'_>) -> R<()> {
    state.e2e.reset_identity()
}

#[tauri::command]
pub fn e2e_import_peer(state: S<'_>, peer_id: i64, key: String) -> R<PeerView> {
    state.e2e.import_peer(peer_id, &key)
}

#[tauri::command]
pub fn e2e_forget_peer(state: S<'_>, peer_id: i64) -> R<()> {
    state.e2e.forget_peer(peer_id)
}

#[tauri::command]
pub fn e2e_chat_state(state: S<'_>, chat_id: i64) -> ChatView {
    state.e2e.chat_state(chat_id)
}

#[tauri::command]
pub fn e2e_set_chat(state: S<'_>, chat_id: i64, enabled: bool) -> R<ChatView> {
    state.e2e.set_chat(chat_id, enabled)
}

#[tauri::command]
pub fn e2e_verify_peer(state: S<'_>, peer_id: i64, digits: String) -> R<bool> {
    state.e2e.verify_peer(peer_id, &digits)
}

#[tauri::command]
pub fn e2e_clear_history(state: S<'_>) -> R<usize> {
    state.e2e.clear_history()
}
