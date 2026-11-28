//! Network & Tunnels settings: MTProto obfuscation and an optional SOCKS5
//! proxy, which is also how MelloGram rides a Tailscale, WireGuard or Tor
//! tunnel (each exposes a local SOCKS5 port). A proxy password is moved into
//! the sealed secret store; the settings file only keeps `user@host:port`.
//! Changes apply on the next connection (restart MelloGram).

use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::state::AppState;

type S<'a> = State<'a, Arc<AppState>>;
type R<T> = Result<T, String>;

const PROXY_SECRET: &str = "proxy-password";

#[derive(Serialize)]
pub struct NetworkView {
    obfuscated_transport: bool,
    /// Proxy URL without any password.
    proxy_url: Option<String>,
    proxy_has_password: bool,
}

#[derive(Deserialize)]
pub struct NetworkUpdate {
    obfuscated_transport: bool,
    proxy_url: Option<String>,
}

/// Splits `socks5h://user:pass@host:port` into the URL without the password
/// and the password, if any.
pub fn split_password(url: &str) -> (String, Option<String>) {
    let Some((scheme, rest)) = url.split_once("://") else { return (url.to_owned(), None) };
    let Some((userinfo, host)) = rest.rsplit_once('@') else { return (url.to_owned(), None) };
    match userinfo.split_once(':') {
        Some((user, pass)) if !pass.is_empty() => (format!("{scheme}://{user}@{host}"), Some(pass.to_owned())),
        _ => (url.to_owned(), None),
    }
}

/// Puts the stored password back for connecting.
pub fn with_password(url: &str, password: Option<&str>) -> String {
    match (password, url.split_once("://")) {
        (Some(p), Some((scheme, rest))) => match rest.rsplit_once('@') {
            Some((user, host)) if !user.contains(':') => format!("{scheme}://{user}:{p}@{host}"),
            _ => url.to_owned(),
        },
        _ => url.to_owned(),
    }
}

pub fn proxy_for_connect(state: &AppState, stored: Option<String>) -> Option<String> {
    let url = stored?;
    let pass = state.secrets.get(PROXY_SECRET);
    Some(with_password(&url, pass.as_ref().map(|p| p.as_str())))
}

#[tauri::command]
pub fn get_network_settings(state: S<'_>) -> NetworkView {
    let net = state.store.lock().unwrap().settings.network.clone();
    NetworkView {
        obfuscated_transport: net.obfuscated_transport,
        proxy_url: net.proxy_url.map(|u| split_password(&u).0),
        proxy_has_password: state.secrets.has(PROXY_SECRET),
    }
}

#[tauri::command]
pub fn set_network_settings(state: S<'_>, network: NetworkUpdate) -> R<()> {
    let proxy = network.proxy_url.map(|u| u.trim().to_owned()).filter(|u| !u.is_empty());
    let stored = match proxy {
        None => {
            state.secrets.delete(PROXY_SECRET);
            None
        }
        Some(url) => {
            mellogram_core::settings::validate_proxy_url(&url)?;
            let (clean, pass) = split_password(&url);
            match pass {
                Some(p) => state.secrets.set(PROXY_SECRET, &p)?,
                // Same user/host without a password typed: keep the saved one.
                None if clean.contains('@') => {}
                None => state.secrets.delete(PROXY_SECRET),
            }
            Some(clean)
        }
    };
    {
        let mut store = state.store.lock().unwrap();
        store.settings.network.obfuscated_transport = network.obfuscated_transport;
        store.settings.network.proxy_url = stored;
    }
    state.save()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn password_is_split_out_and_restored() {
        let (clean, pass) = split_password("socks5h://alice:s3cr:et@10.0.0.1:1080");
        assert_eq!(clean, "socks5h://alice@10.0.0.1:1080");
        assert_eq!(pass.as_deref(), Some("s3cr:et"));
        assert_eq!(with_password(&clean, pass.as_deref()), "socks5h://alice:s3cr:et@10.0.0.1:1080");
        assert_eq!(split_password("socks5h://127.0.0.1:9050"), ("socks5h://127.0.0.1:9050".into(), None));
        assert_eq!(with_password("socks5h://127.0.0.1:9050", Some("x")), "socks5h://127.0.0.1:9050");
    }
}
