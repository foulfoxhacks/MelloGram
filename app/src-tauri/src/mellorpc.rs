//! Opt-in local IPC listeners for native MelloRPC and Discord Rich Presence.
//!
//! The bridge never opens a TCP listener. Unix builds use filesystem sockets;
//! Windows builds use named pipes. Both listeners are disabled by default.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use melloid_core::PresenceActivity;
use mellorpc_core::{
    decode_discord_packet, decode_mello_frame, encode_mello_frame, translate_discord_command,
    ActivitySource, DiscordHandshake, DiscordOpcode, MelloRpcRequest, MelloRpcResponse,
    MAX_FRAME_BYTES, MAX_LEASE_SECONDS, MELLO_RPC_VERSION,
};
use serde_json::{json, Value};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use crate::state::AppState;

#[cfg(windows)]
#[cfg(windows)]
#[derive(Clone, Copy)]
enum EndpointKind { Native, Discord }

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}

fn refresh_presence(state: &AppState) {
    state.publish_presence();
}

async fn read_packet<R: AsyncRead + Unpin>(
    stream: &mut R,
    header_size: usize,
) -> io::Result<Option<Vec<u8>>> {
    let mut header = vec![0u8; header_size];
    match stream.read_exact(&mut header).await {
        Ok(_) => {}
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => return Ok(None),
        Err(error) => return Err(error),
    }
    let length = if header_size == 4 {
        u32::from_le_bytes(header[0..4].try_into().expect("four-byte header")) as usize
    } else if header_size == 8 {
        u32::from_le_bytes(header[4..8].try_into().expect("eight-byte header")) as usize
    } else {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "unsupported IPC header size"));
    };
    if length > MAX_FRAME_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "IPC frame exceeds 1 MiB"));
    }
    let mut packet = header;
    packet.resize(header_size + length, 0);
    stream.read_exact(&mut packet[header_size..]).await?;
    Ok(Some(packet))
}

async fn write_mello_response<W: AsyncWrite + Unpin>(
    stream: &mut W,
    response: &MelloRpcResponse,
) -> io::Result<()> {
    let frame = encode_mello_frame(response)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    stream.write_all(&frame).await
}

async fn handle_native<S>(mut stream: S, state: Arc<AppState>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let mut identity: Option<String> = None;
    loop {
        let packet = match read_packet(&mut stream, 4).await {
            Ok(Some(packet)) => packet,
            Ok(None) => break,
            Err(error) => {
                tracing::debug!("MelloRPC client disconnected: {error}");
                break;
            }
        };
        let request = match decode_mello_frame(&packet) {
            Ok((request, _)) => request,
            Err(error) => {
                let _ = write_mello_response(&mut stream, &MelloRpcResponse::failure(error.to_string())).await;
                break;
            }
        };

        let (response, close) = match request {
            MelloRpcRequest::Hello { protocol_version, client_id, .. } => {
                if protocol_version != MELLO_RPC_VERSION {
                    (MelloRpcResponse::failure(format!("unsupported MelloRPC protocol version {protocol_version}")), true)
                } else if identity.is_some() || client_id.trim().is_empty() || client_id.len() > 128 {
                    (MelloRpcResponse::failure("invalid or repeated Hello handshake"), true)
                } else {
                    identity = Some(client_id);
                    (MelloRpcResponse::success(), false)
                }
            }
            MelloRpcRequest::Ping { nonce } => {
                if identity.is_some() {
                    let mut response = MelloRpcResponse::success();
                    response.nonce = Some(nonce);
                    (response, false)
                } else {
                    (MelloRpcResponse::failure("send Hello before other requests"), true)
                }
            }
            MelloRpcRequest::SetActivity { client_id, activity, lease_seconds } => {
                if identity.as_deref() != Some(client_id.as_str()) {
                    (MelloRpcResponse::failure("client ID does not match the local handshake"), true)
                } else {
                    let result = state.rpc_presence.lock().unwrap().set(
                        client_id,
                        ActivitySource::MelloRpc,
                        activity,
                        lease_seconds,
                        now_ms(),
                    );
                    match result {
                        Ok(()) => {
                            refresh_presence(&state);
                            (MelloRpcResponse::success(), false)
                        }
                        Err(error) => (MelloRpcResponse::failure(error.to_string()), false),
                    }
                }
            }
            MelloRpcRequest::ClearActivity { client_id } => {
                if identity.as_deref() != Some(client_id.as_str()) {
                    (MelloRpcResponse::failure("client ID does not match the local handshake"), true)
                } else {
                    state.rpc_presence.lock().unwrap().clear(&client_id);
                    refresh_presence(&state);
                    (MelloRpcResponse::success(), false)
                }
            }
        };
        if write_mello_response(&mut stream, &response).await.is_err() || close { break; }
    }
    if let Some(client_id) = identity {
        state.rpc_presence.lock().unwrap().clear(&client_id);
        refresh_presence(&state);
    }
}

fn discord_packet(opcode: DiscordOpcode, payload: &[u8]) -> io::Result<Vec<u8>> {
    if payload.len() > MAX_FRAME_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "Discord IPC frame exceeds 1 MiB"));
    }
    let mut frame = Vec::with_capacity(8 + payload.len());
    frame.extend_from_slice(&(opcode as u32).to_le_bytes());
    frame.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    frame.extend_from_slice(payload);
    Ok(frame)
}

async fn send_discord_json<W: AsyncWrite + Unpin>(stream: &mut W, value: &Value) -> io::Result<()> {
    let body = serde_json::to_vec(value)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))?;
    let frame = discord_packet(DiscordOpcode::Frame, &body)?;
    stream.write_all(&frame).await
}

async fn handle_discord<S>(mut stream: S, state: Arc<AppState>)
where
    S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
{
    let first = match read_packet(&mut stream, 8).await {
        Ok(Some(packet)) => packet,
        Ok(None) => return,
        Err(error) => {
            tracing::debug!("Discord IPC handshake read failed: {error}");
            return;
        }
    };
    let (packet, _) = match decode_discord_packet(&first) {
        Ok(packet) => packet,
        Err(error) => {
            tracing::debug!("Invalid Discord IPC handshake: {error}");
            return;
        }
    };
    if packet.opcode != DiscordOpcode::Handshake {
        tracing::debug!("Discord IPC client did not begin with a handshake");
        return;
    }
    let handshake: DiscordHandshake = match serde_json::from_slice::<DiscordHandshake>(&packet.payload) {
        Ok(handshake) if handshake.v == 1
            && !handshake.client_id.trim().is_empty()
            && handshake.client_id.len() <= 80 => handshake,
        _ => {
            let _ = send_discord_json(&mut stream, &json!({
                "cmd": "DISPATCH",
                "evt": "ERROR",
                "data": { "code": 4000, "message": "Invalid MelloGram Discord IPC handshake" },
            })).await;
            return;
        }
    };
    let client_id = handshake.client_id;
    let registry_key = format!("discord:{client_id}");
    let ready = json!({
        "cmd": "DISPATCH",
        "evt": "READY",
        "data": {
            "v": 1,
            "config": {
                "cdn_host": "cdn.discordapp.com",
                "api_endpoint": "//discord.com/api",
                "environment": "production"
            },
            "user": {
                "id": "0",
                "username": "MelloGram Local",
                "discriminator": "0000",
                "avatar": null
            }
        }
    });
    if send_discord_json(&mut stream, &ready).await.is_err() { return; }

    let mut last_activity: Option<PresenceActivity> = None;
    loop {
        let packet_bytes = match tokio::time::timeout(Duration::from_secs(60), read_packet(&mut stream, 8)).await {
            Err(_) => {
                if let Some(activity) = last_activity.clone() {
                    let result = state.rpc_presence.lock().unwrap().set(
                        registry_key.clone(),
                        ActivitySource::DiscordCompatibility,
                        activity,
                        MAX_LEASE_SECONDS,
                        now_ms(),
                    );
                    if result.is_ok() { refresh_presence(&state); }
                }
                continue;
            }
            Ok(Ok(Some(packet))) => packet,
            Ok(Ok(None)) => break,
            Ok(Err(error)) => {
                tracing::debug!("Discord IPC read failed: {error}");
                break;
            }
        };
        let (packet, _) = match decode_discord_packet(&packet_bytes) {
            Ok(packet) => packet,
            Err(error) => {
                tracing::debug!("Discord IPC packet decode failed: {error}");
                break;
            }
        };
        match packet.opcode {
            DiscordOpcode::Ping => {
                if let Ok(pong) = discord_packet(DiscordOpcode::Pong, &packet.payload) {
                    if stream.write_all(&pong).await.is_err() { break; }
                }
            }
            DiscordOpcode::Close => break,
            DiscordOpcode::Pong => {}
            DiscordOpcode::Handshake => {
                let _ = send_discord_json(&mut stream, &json!({
                    "cmd": "DISPATCH",
                    "evt": "ERROR",
                    "data": { "code": 4000, "message": "Handshake has already completed" },
                })).await;
                break;
            }
            DiscordOpcode::Frame => {
                let value: Value = match serde_json::from_slice(&packet.payload) {
                    Ok(value) => value,
                    Err(error) => {
                        let _ = send_discord_json(&mut stream, &json!({
                            "cmd": "DISPATCH", "evt": "ERROR",
                            "data": { "code": 4000, "message": error.to_string() },
                        })).await;
                        continue;
                    }
                };
                let command = value.get("cmd").and_then(Value::as_str).unwrap_or("UNKNOWN");
                let nonce = value.get("nonce").cloned().unwrap_or(Value::Null);
                match translate_discord_command(&client_id, &packet.payload) {
                    Ok(Some(activity)) => {
                        let result = state.rpc_presence.lock().unwrap().set(
                            registry_key.clone(),
                            ActivitySource::DiscordCompatibility,
                            activity.clone(),
                            MAX_LEASE_SECONDS,
                            now_ms(),
                        );
                        match result {
                            Ok(()) => {
                                last_activity = Some(activity);
                                refresh_presence(&state);
                                if send_discord_json(&mut stream, &json!({
                                    "cmd": command, "data": null, "nonce": nonce,
                                })).await.is_err() { break; }
                            }
                            Err(error) => {
                                if send_discord_json(&mut stream, &json!({
                                    "cmd": command, "evt": "ERROR",
                                    "data": { "code": 4000, "message": error.to_string() },
                                    "nonce": nonce,
                                })).await.is_err() { break; }
                            }
                        }
                    }
                    Ok(None) => {
                        last_activity = None;
                        state.rpc_presence.lock().unwrap().clear(&registry_key);
                        refresh_presence(&state);
                        if send_discord_json(&mut stream, &json!({
                            "cmd": command, "data": null, "nonce": nonce,
                        })).await.is_err() { break; }
                    }
                    Err(error) => {
                        if send_discord_json(&mut stream, &json!({
                            "cmd": command, "evt": "ERROR",
                            "data": { "code": 4000, "message": error.to_string() },
                            "nonce": nonce,
                        })).await.is_err() { break; }
                    }
                }
            }
        }
    }
    state.rpc_presence.lock().unwrap().clear(&registry_key);
    refresh_presence(&state);
}

pub fn start(state: Arc<AppState>, native_enabled: bool, discord_enabled: bool) {
    if !native_enabled && !discord_enabled { return; }

    let maintenance = Arc::clone(&state);
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(2));
        loop {
            interval.tick().await;
            refresh_presence(&maintenance);
        }
    });

    #[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))]
    unix_local::start(state, native_enabled, discord_enabled);

    #[cfg(windows)]
    windows_local::start(state, native_enabled, discord_enabled);

    #[cfg(any(target_os = "android", target_os = "ios"))]
    {
        let _ = (state, native_enabled, discord_enabled);
        tracing::warn!("MelloRPC local desktop IPC listeners are unavailable on mobile builds");
    }
}

#[cfg(all(unix, not(target_os = "android"), not(target_os = "ios")))]
mod unix_local {
    use super::*;
    use std::os::unix::fs::{FileTypeExt, PermissionsExt};
    use std::path::{Path, PathBuf};
    use tokio::net::{UnixListener, UnixStream};

    async fn bind_socket(path: &Path) -> io::Result<UnixListener> {
        if let Ok(metadata) = std::fs::symlink_metadata(path) {
            if metadata.file_type().is_socket() {
                match UnixStream::connect(path).await {
                    Ok(_) => return Err(io::Error::new(io::ErrorKind::AddrInUse, "IPC endpoint is already active")),
                    Err(error) if matches!(error.kind(), io::ErrorKind::ConnectionRefused | io::ErrorKind::NotFound) => {
                        std::fs::remove_file(path)?;
                    }
                    Err(error) => return Err(error),
                }
            } else {
                return Err(io::Error::new(io::ErrorKind::AddrInUse, "IPC path exists and is not a socket"));
            }
        }
        let listener = UnixListener::bind(path)?;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        Ok(listener)
    }

    async fn listen_native(path: PathBuf, state: Arc<AppState>) -> io::Result<()> {
        let listener = bind_socket(&path).await?;
        tracing::info!("MelloRPC native IPC listening at {}", path.display());
        loop {
            let (stream, _) = listener.accept().await?;
            let state = Arc::clone(&state);
            tokio::spawn(async move { handle_native(stream, state).await; });
        }
    }

    fn discord_dirs() -> Vec<PathBuf> {
        let mut directories = Vec::new();
        for key in ["XDG_RUNTIME_DIR", "TMPDIR", "TMP", "TEMP"] {
            if let Some(value) = std::env::var_os(key) {
                let path = PathBuf::from(value);
                if path.is_dir() && !directories.contains(&path) { directories.push(path); }
            }
        }
        let fallback = PathBuf::from("/tmp");
        if fallback.is_dir() && !directories.contains(&fallback) { directories.push(fallback); }
        directories
    }

    async fn listen_discord(state: Arc<AppState>) -> io::Result<()> {
        let mut bound = None;
        'directories: for directory in discord_dirs() {
            for index in 0..10 {
                let path = directory.join(format!("discord-ipc-{index}"));
                match bind_socket(&path).await {
                    Ok(listener) => { bound = Some((listener, path)); break 'directories; }
                    Err(error) if error.kind() == io::ErrorKind::AddrInUse => continue,
                    Err(error) => tracing::debug!("Could not bind Discord IPC {}: {error}", path.display()),
                }
            }
        }
        let Some((listener, path)) = bound else {
            return Err(io::Error::new(io::ErrorKind::AddrNotAvailable, "no free Discord IPC socket was found"));
        };
        tracing::info!("Discord Rich Presence compatibility IPC listening at {}", path.display());
        loop {
            let (stream, _) = listener.accept().await?;
            let state = Arc::clone(&state);
            tokio::spawn(async move { handle_discord(stream, state).await; });
        }
    }

    pub fn start(state: Arc<AppState>, native_enabled: bool, discord_enabled: bool) {
        if native_enabled {
            let path = state.dir.join("mellorpc.sock");
            let native_state = Arc::clone(&state);
            tauri::async_runtime::spawn(async move {
                if let Err(error) = listen_native(path, native_state).await {
                    tracing::warn!("MelloRPC native IPC listener stopped: {error}");
                }
            });
        }
        if discord_enabled {
            tauri::async_runtime::spawn(async move {
                if let Err(error) = listen_discord(state).await {
                    tracing::warn!("Discord compatibility IPC listener stopped: {error}");
                }
            });
        }
    }
}

#[cfg(windows)]
mod windows_local {
    use super::*;
    use tokio::net::windows::named_pipe::{NamedPipeServer, ServerOptions};

    async fn serve_pipe(
        mut server: NamedPipeServer,
        name: String,
        kind: EndpointKind,
        state: Arc<AppState>,
    ) {
        loop {
            if let Err(error) = server.connect().await {
                tracing::warn!("MelloGram named pipe {name} stopped: {error}");
                return;
            }
            let connected = server;
            match ServerOptions::new().create(&name) {
                Ok(next) => server = next,
                Err(error) => {
                    tracing::warn!("Could not create next instance of {name}: {error}");
                    tokio::spawn(async move {
                        match kind {
                            EndpointKind::Native => handle_native(connected, state).await,
                            EndpointKind::Discord => handle_discord(connected, state).await,
                        }
                    });
                    return;
                }
            }
            let state = Arc::clone(&state);
            let kind = kind;
            tokio::spawn(async move {
                match kind {
                    EndpointKind::Native => handle_native(connected, state).await,
                    EndpointKind::Discord => handle_discord(connected, state).await,
                }
            });
        }
    }

    pub fn start(state: Arc<AppState>, native_enabled: bool, discord_enabled: bool) {
        if native_enabled {
            let name = String::from(r"\\.\pipe\mellorpc-v1");
            let native_state = Arc::clone(&state);
            tauri::async_runtime::spawn(async move {
                match ServerOptions::new().first_pipe_instance(true).create(&name) {
                    Ok(server) => serve_pipe(server, name, EndpointKind::Native, native_state).await,
                    Err(error) => tracing::warn!("Could not start native MelloRPC named pipe: {error}"),
                }
            });
        }
        if discord_enabled {
            tauri::async_runtime::spawn(async move {
                for index in 0..10 {
                    let name = format!(r"\\.\pipe\discord-ipc-{index}");
                    match ServerOptions::new().first_pipe_instance(true).create(&name) {
                        Ok(server) => {
                            tracing::info!("Discord Rich Presence compatibility listening at {name}");
                            serve_pipe(server, name, EndpointKind::Discord, state).await;
                            return;
                        }
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
                        Err(error) => tracing::debug!("Could not bind Discord pipe {name}: {error}"),
                    }
                }
                tracing::warn!("No free Discord IPC named pipe was found");
            });
        }
    }
}
