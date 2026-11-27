//! MelloRPC's transport-neutral wire protocol, Discord IPC framing and
//! activity lease model. This crate deliberately contains no socket listener;
//! transports are separate so local IPC can be tested and secured independently.
#![forbid(unsafe_code)]

use std::collections::BTreeMap;

use melloid_core::{PresenceActivity, PresenceKind};
use serde::{Deserialize, Serialize};

pub const MELLO_RPC_VERSION: u16 = 1;
pub const MAX_FRAME_BYTES: usize = 1024 * 1024;
pub const MIN_LEASE_SECONDS: u32 = 10;
pub const MAX_LEASE_SECONDS: u32 = 300;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MelloRpcRequest {
    Hello { protocol_version: u16, client_id: String, client_name: Option<String> },
    SetActivity { client_id: String, activity: PresenceActivity, lease_seconds: u32 },
    ClearActivity { client_id: String },
    Ping { nonce: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MelloRpcResponse {
    pub ok: bool,
    pub nonce: Option<u64>,
    pub error: Option<String>,
    pub protocol_version: u16,
}
impl MelloRpcResponse {
    pub fn success() -> Self {
        Self { ok: true, nonce: None, error: None, protocol_version: MELLO_RPC_VERSION }
    }
    pub fn failure(error: impl Into<String>) -> Self {
        Self { ok: false, nonce: None, error: Some(error.into()), protocol_version: MELLO_RPC_VERSION }
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum FrameError {
    #[error("frame header is incomplete")]
    IncompleteHeader,
    #[error("frame payload is incomplete; expected {expected} bytes")]
    IncompletePayload { expected: usize },
    #[error("frame length {0} exceeds the 1 MiB limit")]
    TooLarge(usize),
    #[error("unsupported Discord IPC opcode {0}")]
    UnsupportedOpcode(u32),
    #[error("invalid JSON frame: {0}")]
    InvalidJson(String),
    #[error("unsupported Discord RPC command: {0}")]
    UnsupportedCommand(String),
    #[error("invalid activity: {0}")]
    InvalidActivity(String),
    #[error("unsupported MelloRPC protocol version {0}")]
    UnsupportedVersion(u16),
    #[error("invalid lease duration; allowed range is 10–300 seconds")]
    InvalidLease,
}

pub fn encode_mello_frame<T: Serialize>(message: &T) -> Result<Vec<u8>, FrameError> {
    let body = serde_json::to_vec(message).map_err(|e| FrameError::InvalidJson(e.to_string()))?;
    if body.len() > MAX_FRAME_BYTES { return Err(FrameError::TooLarge(body.len())); }
    let mut output = Vec::with_capacity(4 + body.len());
    output.extend_from_slice(&(body.len() as u32).to_le_bytes());
    output.extend_from_slice(&body);
    Ok(output)
}

/// Decode one four-byte-length-prefixed MelloRPC JSON frame. Returns the
/// decoded request and bytes consumed, allowing multiple frames in one read.
pub fn decode_mello_frame(bytes: &[u8]) -> Result<(MelloRpcRequest, usize), FrameError> {
    if bytes.len() < 4 { return Err(FrameError::IncompleteHeader); }
    let length = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte slice")) as usize;
    if length > MAX_FRAME_BYTES { return Err(FrameError::TooLarge(length)); }
    if bytes.len() < 4 + length { return Err(FrameError::IncompletePayload { expected: length }); }
    let request = serde_json::from_slice(&bytes[4..4 + length])
        .map_err(|e| FrameError::InvalidJson(e.to_string()))?;
    Ok((request, 4 + length))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum DiscordOpcode { Handshake = 0, Frame = 1, Close = 2, Ping = 3, Pong = 4 }
impl TryFrom<u32> for DiscordOpcode {
    type Error = FrameError;
    fn try_from(value: u32) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Handshake), 1 => Ok(Self::Frame), 2 => Ok(Self::Close),
            3 => Ok(Self::Ping), 4 => Ok(Self::Pong),
            other => Err(FrameError::UnsupportedOpcode(other)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscordIpcPacket { pub opcode: DiscordOpcode, pub payload: Vec<u8> }

/// Discord IPC uses an eight-byte little-endian header: opcode then payload length.
pub fn decode_discord_packet(bytes: &[u8]) -> Result<(DiscordIpcPacket, usize), FrameError> {
    if bytes.len() < 8 { return Err(FrameError::IncompleteHeader); }
    let opcode_raw = u32::from_le_bytes(bytes[0..4].try_into().expect("four-byte slice"));
    let length = u32::from_le_bytes(bytes[4..8].try_into().expect("four-byte slice")) as usize;
    if length > MAX_FRAME_BYTES { return Err(FrameError::TooLarge(length)); }
    if bytes.len() < 8 + length { return Err(FrameError::IncompletePayload { expected: length }); }
    let opcode = DiscordOpcode::try_from(opcode_raw)?;
    Ok((DiscordIpcPacket { opcode, payload: bytes[8..8 + length].to_vec() }, 8 + length))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiscordHandshake { pub v: u32, pub client_id: String }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActivitySource { MelloRpc, DiscordCompatibility }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PresenceLease {
    pub client_id: String,
    pub source: ActivitySource,
    pub activity: PresenceActivity,
    pub expires_at_ms: i64,
}

/// In-memory activity leases. Expired activity is never returned to UI or
/// published downstream; clients must refresh instead of leaving stale status.
#[derive(Debug, Default)]
pub struct PresenceRegistry { leases: BTreeMap<String, PresenceLease> }
impl PresenceRegistry {
    pub fn set(
        &mut self,
        client_id: impl Into<String>,
        source: ActivitySource,
        activity: PresenceActivity,
        lease_seconds: u32,
        now_ms: i64,
    ) -> Result<(), FrameError> {
        let client_id = client_id.into();
        if client_id.trim().is_empty() || client_id.len() > 128 {
            return Err(FrameError::InvalidActivity("client ID must be 1–128 characters".into()));
        }
        if !(MIN_LEASE_SECONDS..=MAX_LEASE_SECONDS).contains(&lease_seconds) {
            return Err(FrameError::InvalidLease);
        }
        validate_activity(&activity)?;
        self.leases.insert(client_id.clone(), PresenceLease {
            client_id, source, activity,
            expires_at_ms: now_ms.saturating_add(i64::from(lease_seconds) * 1000),
        });
        Ok(())
    }
    pub fn clear(&mut self, client_id: &str) -> bool { self.leases.remove(client_id).is_some() }
    pub fn get(&mut self, client_id: &str, now_ms: i64) -> Option<&PresenceLease> {
        self.prune(now_ms);
        self.leases.get(client_id)
    }
    pub fn active(&mut self, now_ms: i64) -> Vec<PresenceLease> {
        self.prune(now_ms);
        self.leases.values().cloned().collect()
    }
    fn prune(&mut self, now_ms: i64) {
        self.leases.retain(|_, lease| lease.expires_at_ms > now_ms);
    }
}

pub fn validate_activity(activity: &PresenceActivity) -> Result<(), FrameError> {
    if activity.application.trim().is_empty() || activity.application.chars().count() > 80 {
        return Err(FrameError::InvalidActivity("application must be 1–80 characters".into()));
    }
    if activity.activity.trim().is_empty() || activity.activity.chars().count() > 128 {
        return Err(FrameError::InvalidActivity("activity must be 1–128 characters".into()));
    }
    for (field, value) in [("details", &activity.details), ("state", &activity.state)] {
        if value.as_ref().is_some_and(|text| text.chars().count() > 128) {
            return Err(FrameError::InvalidActivity(format!("{field} must be 128 characters or fewer")));
        }
    }
    if let Some(url) = &activity.join_url {
        if !(url.starts_with("https://") || url.starts_with("http://")) {
            return Err(FrameError::InvalidActivity("join URL must use HTTP or HTTPS".into()));
        }
    }
    if let (Some(current), Some(maximum)) = (activity.party_current, activity.party_max) {
        if current > maximum {
            return Err(FrameError::InvalidActivity("party current size cannot exceed maximum size".into()));
        }
    }
    Ok(())
}

/// Convert a Discord Rich Presence activity object into the shared model.
/// Discord secret tokens are deliberately not mapped to public join URLs.
pub fn translate_discord_activity(
    client_id: &str,
    activity: &serde_json::Value,
) -> Result<Option<PresenceActivity>, FrameError> {
    if activity.is_null() { return Ok(None); }
    let object = activity.as_object()
        .ok_or_else(|| FrameError::InvalidActivity("Discord activity must be an object or null".into()))?;
    let details = object.get("details").and_then(|v| v.as_str()).map(str::to_owned);
    let state = object.get("state").and_then(|v| v.as_str()).map(str::to_owned);
    let title = details.clone()
        .or_else(|| state.clone())
        .or_else(|| object.get("name").and_then(|v| v.as_str()).map(str::to_owned))
        .unwrap_or_else(|| "Rich Presence".into());
    let kind = match object.get("type").and_then(|v| v.as_u64()).unwrap_or(0) {
        0 => PresenceKind::Playing, 1 => PresenceKind::Streaming,
        2 => PresenceKind::Listening, 3 => PresenceKind::Watching,
        5 => PresenceKind::Competing,
        _ => PresenceKind::Custom,
    };
    let timestamps = object.get("timestamps");
    let normalize_timestamp = |value: i64| if value > 10_000_000_000 { value / 1000 } else { value };
    let started_at = timestamps.and_then(|v| v.get("start")).and_then(|v| v.as_i64()).map(normalize_timestamp);
    let ends_at = timestamps.and_then(|v| v.get("end")).and_then(|v| v.as_i64()).map(normalize_timestamp);
    let party = object.get("party").and_then(|v| v.get("size")).and_then(|v| v.as_array());
    let party_current = party.and_then(|v| v.first()).and_then(|v| v.as_u64()).and_then(|v| u32::try_from(v).ok());
    let party_max = party.and_then(|v| v.get(1)).and_then(|v| v.as_u64()).and_then(|v| u32::try_from(v).ok());
    let assets = object.get("assets");
    let large_image = assets.and_then(|v| v.get("large_image")).and_then(|v| v.as_str()).map(str::to_owned);
    let small_image = assets.and_then(|v| v.get("small_image")).and_then(|v| v.as_str()).map(str::to_owned);
    let translated = PresenceActivity {
        application: client_id.to_owned(), activity: title, details, state, kind,
        started_at, ends_at, party_current, party_max, join_url: None,
        large_image, small_image,
    };
    validate_activity(&translated)?;
    Ok(Some(translated))
}

/// Translate Discord IPC FRAME payloads for SET_ACTIVITY and CLEAR_ACTIVITY.
pub fn translate_discord_command(client_id: &str, payload: &[u8]) -> Result<Option<PresenceActivity>, FrameError> {
    let value: serde_json::Value = serde_json::from_slice(payload)
        .map_err(|e| FrameError::InvalidJson(e.to_string()))?;
    let command = value.get("cmd").and_then(|v| v.as_str()).unwrap_or("");
    match command {
        "SET_ACTIVITY" => {
            let activity = value.get("args").and_then(|v| v.get("activity"))
                .ok_or_else(|| FrameError::InvalidActivity("SET_ACTIVITY is missing args.activity".into()))?;
            translate_discord_activity(client_id, activity)
        }
        "CLEAR_ACTIVITY" => Ok(None),
        other => Err(FrameError::UnsupportedCommand(other.to_owned())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn activity() -> PresenceActivity {
        PresenceActivity { application: "MelloGram".into(), activity: "Building MelloGram".into(),
            details: Some("MelloID".into()), ..Default::default() }
    }

    #[test]
    fn mello_rpc_frames_round_trip_and_allow_trailing_packets() {
        let request = MelloRpcRequest::Ping { nonce: 42 };
        let mut frame = encode_mello_frame(&request).unwrap();
        frame.extend_from_slice(&[1, 2, 3]);
        let (decoded, consumed) = decode_mello_frame(&frame).unwrap();
        assert_eq!(decoded, request);
        assert_eq!(consumed, frame.len() - 3);
    }

    #[test]
    fn mello_rpc_rejects_incomplete_and_oversized_frames() {
        assert_eq!(decode_mello_frame(&[1, 0]), Err(FrameError::IncompleteHeader));
        assert_eq!(decode_mello_frame(&u32::MAX.to_le_bytes()), Err(FrameError::TooLarge(u32::MAX as usize)));
    }

    #[test]
    fn discord_ipc_decodes_little_endian_packet() {
        let payload = br#"{"cmd":"CLEAR_ACTIVITY"}"#;
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(DiscordOpcode::Frame as u32).to_le_bytes());
        bytes.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        bytes.extend_from_slice(payload);
        let (packet, consumed) = decode_discord_packet(&bytes).unwrap();
        assert_eq!(packet.opcode, DiscordOpcode::Frame);
        assert_eq!(packet.payload, payload);
        assert_eq!(consumed, bytes.len());
    }

    #[test]
    fn discord_activity_maps_to_shared_presence_without_secret_leakage() {
        let input = serde_json::json!({
            "details": "Exploring the world", "state": "In a multiplayer session", "type": 0,
            "timestamps": { "start": 1700000000_i64 }, "party": { "size": [2, 8] },
            "assets": { "large_image": "world-art", "small_image": "avatar" },
            "secrets": { "join": "opaque-secret" }
        });
        let translated = translate_discord_activity("com.example.game", &input).unwrap().unwrap();
        assert_eq!(translated.application, "com.example.game");
        assert_eq!(translated.activity, "Exploring the world");
        assert_eq!(translated.started_at, Some(1_700_000_000));
        assert_eq!(translated.party_current, Some(2));
        assert_eq!(translated.party_max, Some(8));
        assert_eq!(translated.join_url, None);
    }


    #[test]
    fn discord_competing_activity_and_mixed_timestamp_units_are_supported() {
        let input = serde_json::json!({
            "details": "Tournament",
            "type": 5,
            "timestamps": { "start": 1_700_000_000_i64, "end": 1_700_000_100_000_i64 }
        });
        let translated = translate_discord_activity("com.example.game", &input).unwrap().unwrap();
        assert_eq!(translated.kind, PresenceKind::Competing);
        assert_eq!(translated.started_at, Some(1_700_000_000));
        assert_eq!(translated.ends_at, Some(1_700_000_100));
    }

    #[test]
    fn presence_leases_expire_and_can_be_cleared_per_client() {
        let mut registry = PresenceRegistry::default();
        registry.set("game-a", ActivitySource::MelloRpc, activity(), 30, 1_000).unwrap();
        registry.set("game-b", ActivitySource::DiscordCompatibility, activity(), 30, 1_000).unwrap();
        assert_eq!(registry.active(1_001).len(), 2);
        assert!(registry.clear("game-a"));
        assert_eq!(registry.active(1_002).len(), 1);
        assert!(registry.get("game-b", 31_001).is_none());
    }

    #[test]
    fn rejects_invalid_activity_and_lease_lengths() {
        let mut registry = PresenceRegistry::default();
        assert_eq!(registry.set("game-a", ActivitySource::MelloRpc, activity(), 2, 0), Err(FrameError::InvalidLease));
        let mut invalid = activity();
        invalid.party_current = Some(9);
        invalid.party_max = Some(2);
        assert!(registry.set("game-a", ActivitySource::MelloRpc, invalid, 30, 0).is_err());
    }
}
