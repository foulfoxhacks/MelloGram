# MelloRPC — native presence and Discord compatibility

MelloRPC is MelloGram's local Rich Presence engine. It is separate from
Telegram MTProto and normal chat state. It accepts native MelloRPC activity
messages and a supported subset of Discord Rich Presence IPC commands.

## Features implemented

- Versioned native MelloRPC request/response model.
- Four-byte little-endian length-prefixed JSON framing for native clients.
- Discord IPC eight-byte little-endian opcode/length framing.
- Discord handshake and READY response.
- Supported Discord commands: SET_ACTIVITY and CLEAR_ACTIVITY.
- Discord IPC PING/PONG and CLOSE handling.
- Activity conversion for details, state, type, timestamps, party size, and
  large/small image identifiers.
- Discord secret tokens are deliberately not exposed as public join URLs.
- Per-client in-memory activity leases, bounded to 10–300 seconds.
- Native client activity refresh, connection cleanup, and expiry.
- Windows named-pipe transport and Unix-domain socket transport.
- Local MelloID presence projection and UI event when profile presence is on.
- No TCP, LAN, or public network listener.

## Settings and endpoints

Both listeners are disabled by default. Enable them in
Settings → MelloRPC Presence, then restart MelloGram.

| Platform | Native MelloRPC | Discord compatibility |
|---|---|---|
| Windows | \\\\.\\pipe\\mellorpc-v1 | \\\\.\\pipe\\discord-ipc-N |
| Linux/macOS | mellorpc.sock inside the MelloGram config directory | discord-ipc-N in the selected local temp/runtime directory |

On Unix-like systems, the native socket is created with owner-only filesystem
permissions. The Discord compatibility listener searches the usual runtime
and temporary directories. On Windows, the listeners use named pipes with the
operating system's default named-pipe security descriptor; an explicit
current-user SID ACL remains a hardening item.

Only local IPC endpoints are created. No public TCP server is started.

## Native client protocol

A dependency-free Node.js example is available at
docs/examples/mellorpc-node-example.js. It connects to the endpoint, sends
the v1 Hello handshake, sets a 120-second activity lease, refreshes it every
45 seconds, and clears activity on shutdown.

Native clients send a four-byte little-endian payload length followed by UTF-8
JSON. Example handshake:

    {"type":"hello","protocol_version":1,"client_id":"com.example.game","client_name":"Example game"}

Example activity:

    {"type":"set_activity","client_id":"com.example.game","lease_seconds":120,"activity":{"application":"Example Game","activity":"Playing","kind":"playing"}}

Optional activity fields include details, state, started_at, ends_at,
party_current, party_max, join_url, large_image, and small_image. A native
client should refresh its lease before expiry. The server accepts only a
matching client ID after a successful Hello.

## Discord compatibility limits

This is a compatibility bridge, not a full Discord client. It currently
supports the Rich Presence activity update and clear commands plus IPC
heartbeat/close handling. It does not implement every Discord RPC command,
all Discord SDK side channels, invitation/join flows, or guarantee parity with
every Discord SDK version.

When Discord is already running and owns the first IPC endpoint, MelloGram
tries a free subsequent endpoint. Some SDKs only use the first discovered
endpoint, so testing may require closing Discord or configuring the SDK to
connect to the MelloGram endpoint.

## Security and privacy model

- User opt-in is required before either listener starts.
- Endpoints are local IPC only, never TCP.
- Frame size is capped at 1 MiB.
- Activity fields and URLs are validated.
- Leases expire and activity is cleared on client disconnect.
- Profile activity is hidden when the MelloID show_presence setting is off.
- No Discord secret token or join secret is copied into public profile fields.

The next hardening pass should add explicit Windows named-pipe ACLs, per-app
allow/block controls, connection concurrency/rate limits, fuzz testing, and
socket-level integration tests.

## Next MelloRPC work

1. Integration tests against Unix sockets and Windows named pipes.
2. Per-application approval and revocation UI.
3. Application registration/identity metadata and a small Rust/TS SDK.
4. Improved Discord RPC command coverage based on a compatibility matrix.
5. Optional authenticated remote presence leases for hosted MelloID profiles.
