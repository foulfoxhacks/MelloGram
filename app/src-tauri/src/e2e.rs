//! `invoke` handlers for the end-to-end privacy overlay.
//!
//! The protocol work, the sealed store and the per-chat rules live in the
//! `mellogram-e2e` crate so they are covered by the workspace test run; this
//! module only exposes them to the UI.

use std::sync::Arc;

use mellogram_e2e::{ChatView, E2eStatus, GroupView, PeerView};
use serde::Serialize;
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

// ---- groups ---------------------------------------------------------------

/// What one round of key sharing achieved.
#[derive(Debug, Serialize)]
pub struct ShareReport {
    pub group: GroupView,
    /// Members who received our sender key.
    pub sent: Vec<i64>,
    /// Members we could not reach, with the reason.
    pub skipped: Vec<(i64, String)>,
    /// True when the group has more members than one share pass covers. The
    /// rest were not contacted; they will get the key on the next pass.
    pub truncated: bool,
}

/// One share or ask pass covers this many members. Bigger groups are handled
/// over several passes rather than in one burst of direct messages.
const SHARE_MEMBER_CAP: usize = 500;

#[tauri::command]
pub fn e2e_group_state(state: S<'_>, chat_id: i64) -> GroupView {
    state.e2e.group_state(chat_id)
}

/// Turns group encryption on and creates our sender key. The member list is
/// read straight away so a key request can be answered without waiting for the
/// next sweep.
#[tauri::command]
pub async fn e2e_group_enable(state: S<'_>, chat_id: i64) -> R<GroupView> {
    let view = state.e2e.group_enable(chat_id)?;
    let _ = state.sweep_group_keys().await;
    Ok(view)
}

#[tauri::command]
pub fn e2e_group_disable(state: S<'_>, chat_id: i64) -> R<()> {
    state.e2e.group_disable(chat_id)
}

/// Replaces our sender key; the group must be given the new one afterwards.
#[tauri::command]
pub fn e2e_group_rotate(state: S<'_>, chat_id: i64) -> R<GroupView> {
    state.e2e.group_rotate(chat_id)
}

/// Sends our current sender key to every member we can reach. Each envelope
/// travels inside that member's pairwise encrypted chat, so only they can read
/// it and it is authenticated by the pairwise ratchet.
#[tauri::command]
pub async fn e2e_group_share(state: S<'_>, chat_id: i64) -> R<ShareReport> {
    let tg = state.telegram().await?;
    let me = tg.ensure_self_id().await.map_err(|e| e.to_string())?;
    state.e2e.set_self_id(me);
    let (members, complete) = tg
        .chat_members_all(chat_id, SHARE_MEMBER_CAP)
        .await
        .map_err(|e| e.to_string())?;
    let ids: Vec<i64> = members
        .iter()
        .map(|m| m.id)
        .filter(|id| *id != me && *id > 0)
        .collect();
    let envelopes = state.e2e.group_distributions(chat_id, &ids)?;
    let mut sent = Vec::new();
    let mut skipped = Vec::new();
    for envelope in envelopes {
        let armored = match state.e2e.encrypt_for_peer(envelope.peer_id, &envelope.text) {
            Ok(armored) => armored,
            Err(e) => {
                skipped.push((envelope.peer_id, e));
                continue;
            }
        };
        let message = mellogram_core::OutgoingMessage {
            chat_id: envelope.peer_id,
            text: armored,
            link_preview: false,
            ..Default::default()
        };
        match tg.send(&message).await {
            Ok(_) => sent.push(envelope.peer_id),
            Err(e) => skipped.push((envelope.peer_id, e.to_string())),
        }
    }
    Ok(ShareReport {
        group: state.e2e.group_state(chat_id),
        sent,
        skipped,
        truncated: !complete,
    })
}

/// Asks every reachable member who has not sent us their key yet for it.
#[tauri::command]
pub async fn e2e_group_request_keys(state: S<'_>, chat_id: i64) -> R<ShareReport> {
    let tg = state.telegram().await?;
    let me = tg.ensure_self_id().await.map_err(|e| e.to_string())?;
    state.e2e.set_self_id(me);
    let (members, complete) = tg
        .chat_members_all(chat_id, SHARE_MEMBER_CAP)
        .await
        .map_err(|e| e.to_string())?;
    let request = state.e2e.request_group_key(chat_id)?;
    let have: std::collections::HashSet<i64> = state
        .e2e
        .group_state(chat_id)
        .senders
        .iter()
        .map(|sender| sender.sender_id)
        .collect();
    let mut sent = Vec::new();
    let mut skipped = Vec::new();
    for member in members.iter() {
        if member.id == me || member.id <= 0 || have.contains(&member.id) {
            continue;
        }
        if !state.e2e.has_pairwise(member.id) {
            skipped.push((member.id, "no one-to-one overlay session yet".to_string()));
            continue;
        }
        let armored = match state.e2e.encrypt_for_peer(member.id, &request) {
            Ok(armored) => armored,
            Err(e) => {
                skipped.push((member.id, e));
                continue;
            }
        };
        let message = mellogram_core::OutgoingMessage {
            chat_id: member.id,
            text: armored,
            link_preview: false,
            ..Default::default()
        };
        match tg.send(&message).await {
            Ok(_) => sent.push(member.id),
            Err(e) => skipped.push((member.id, e.to_string())),
        }
    }
    Ok(ShareReport {
        group: state.e2e.group_state(chat_id),
        sent,
        skipped,
        truncated: !complete,
    })
}

/// Re-reads group member lists now; returns how many groups now need a new key.
#[tauri::command]
pub async fn e2e_group_sweep(state: S<'_>) -> R<usize> {
    state.sweep_group_keys().await
}
