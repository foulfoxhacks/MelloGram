//! Activity tracker commands: the redacted traffic log, and optional WakaTime
//! heartbeats ("time spent communicating"). The WakaTime key is kept in the
//! sealed secret store and only ever sent to api.wakatime.com. Heartbeats
//! name the app, never a chat, contact or message.

use std::sync::Arc;

use base64::Engine as _;
use serde::Serialize;
use tauri::State;

use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

const WAKATIME_HOST: &str = "api.wakatime.com";
const SECRET: &str = "wakatime";

/// WakaTime keys are UUIDs, optionally prefixed with `waka_`.
pub fn valid_wakatime_key(key: &str) -> bool {
    let k = key.strip_prefix("waka_").unwrap_or(key);
    k.len() == 36
        && k.bytes().enumerate().all(|(i, b)| if [8, 13, 18, 23].contains(&i) { b == b'-' } else { b.is_ascii_hexdigit() })
}

#[derive(Serialize)]
pub struct TrafficView {
    entries: Vec<crate::traffic::TrafficEntry>,
    totals: crate::traffic::TrafficTotals,
}

#[tauri::command]
pub fn traffic_log(state: S<'_>) -> TrafficView {
    let (entries, totals) = state.traffic.snapshot();
    TrafficView { entries, totals }
}

#[tauri::command]
pub fn traffic_clear(state: S<'_>) {
    state.traffic.clear();
}

#[derive(Serialize)]
pub struct WakaStatus {
    configured: bool,
}

#[tauri::command]
pub fn wakatime_status(state: S<'_>) -> WakaStatus {
    WakaStatus { configured: state.secrets.has(SECRET) }
}

/// Saves (or with `None`, removes) the WakaTime API key.
#[tauri::command]
pub fn wakatime_set_key(state: S<'_>, key: Option<String>) -> R<()> {
    match key.map(|k| k.trim().to_owned()).filter(|k| !k.is_empty()) {
        None => {
            state.secrets.delete(SECRET);
            Ok(())
        }
        Some(k) if valid_wakatime_key(&k) => state.secrets.set(SECRET, &k),
        Some(_) => Err("That doesn't look like a WakaTime API key (waka_ followed by a UUID).".into()),
    }
}

/// Sends one heartbeat. The UI calls this at most every two minutes while
/// the window is focused and you are active.
#[tauri::command]
pub async fn wakatime_heartbeat(state: S<'_>) -> R<()> {
    let Some(key) = state.secrets.get(SECRET) else { return Ok(()) };
    let auth = format!("Basic {}", base64::engine::general_purpose::STANDARD.encode(key.as_bytes()));
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs_f64()).unwrap_or(0.0);
    let body = serde_json::json!({
        "entity": "MelloGram",
        "type": "app",
        "category": "communicating",
        "time": now,
        "project": "Telegram",
        "plugin": concat!("mellogram/", env!("CARGO_PKG_VERSION"), " mellogram-activity/1"),
    });
    let payload = body.to_string();
    let len = payload.len() as u64;
    let status = tauri::async_runtime::spawn_blocking(move || -> R<u16> {
        let agent = ureq::AgentBuilder::new().try_proxy_from_env(true).timeout(std::time::Duration::from_secs(15)).build();
        let resp = agent
            .post(&format!("https://{WAKATIME_HOST}/api/v1/users/current/heartbeats"))
            .set("Authorization", &auth)
            .set("Content-Type", "application/json")
            .send_string(&payload);
        match resp {
            Ok(r) => Ok(r.status()),
            Err(ureq::Error::Status(code, _)) => Ok(code),
            Err(e) => Err(e.to_string()),
        }
    })
    .await
    .map_err(|e| e.to_string())??;
    state.traffic.record("out", "https", WAKATIME_HOST, len, "WakaTime heartbeat");
    match status {
        200..=299 => Ok(()),
        401 | 403 => Err("WakaTime rejected the API key.".into()),
        code => Err(format!("WakaTime answered {code}.")),
    }
}

const MELLOID_HOST: &str = "id.mellogram.foulfoxhacks.dev";

/// Global badges: looks up which of these Telegram users have a public
/// MelloID profile, and their visible badges. Only user ids (positive) are
/// sent; at most 50 per call.
#[tauri::command]
pub async fn melloid_badges(state: S<'_>, ids: Vec<i64>) -> R<serde_json::Value> {
    let mut ids: Vec<i64> = ids.into_iter().filter(|id| *id > 0).collect();
    ids.sort_unstable();
    ids.dedup();
    ids.truncate(50);
    if ids.is_empty() {
        return Ok(serde_json::json!({}));
    }
    let query = ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",");
    let qlen = query.len() as u64;
    let body = tauri::async_runtime::spawn_blocking(move || -> R<String> {
        let agent = ureq::AgentBuilder::new().try_proxy_from_env(true).timeout(std::time::Duration::from_secs(15)).build();
        let resp = agent.get(&format!("https://{MELLOID_HOST}/api/v1/badges")).query("ids", &query).call().map_err(|e| e.to_string())?;
        let mut text = String::new();
        std::io::Read::read_to_string(&mut std::io::Read::take(resp.into_reader(), 256 * 1024), &mut text).map_err(|e| e.to_string())?;
        Ok(text)
    })
    .await
    .map_err(|e| e.to_string())??;
    state.traffic.record("out", "https", MELLOID_HOST, qlen, "badge lookup");
    state.traffic.record("in", "https", MELLOID_HOST, body.len() as u64, "badge lookup");
    let value: serde_json::Value = serde_json::from_str(&body).map_err(|_| "MelloID sent an unexpected reply".to_string())?;
    Ok(value.get("users").cloned().unwrap_or_else(|| serde_json::json!({})))
}

#[cfg(test)]
mod tests {
    use super::valid_wakatime_key;

    #[test]
    fn key_format() {
        assert!(valid_wakatime_key("waka_12345678-1234-1234-1234-123456789abc"));
        assert!(valid_wakatime_key("12345678-1234-1234-1234-123456789abc"));
        assert!(!valid_wakatime_key("waka_nope"));
        assert!(!valid_wakatime_key("12345678x1234-1234-1234-123456789abc"));
    }
}
