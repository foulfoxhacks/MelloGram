# MelloRPC — protocol foundation

MelloRPC is MelloGram's transport-neutral rich-presence protocol. It is kept
separate from Telegram MTProto and from the profile editor.

## Implemented in the protocol core

- Versioned native MelloRPC request and response types.
- Four-byte little-endian length-prefixed JSON framing for native clients.
- Discord IPC packet decoding using the eight-byte little-endian opcode/length
  header.
- Discord opcode definitions for handshake, frame, close, ping and pong.
- Translation of Discord SET_ACTIVITY and CLEAR_ACTIVITY payloads into the
  shared MelloID PresenceActivity model.
- Mapping of activity details/state, type, start/end timestamps, party size,
  and large/small image identifiers.
- Per-client in-memory presence leases with a bounded 10–300 second lifetime.
- Input limits for activity labels, client IDs, URLs, party sizes and frame
  sizes.
- Tests for framing, translation, secret non-disclosure, validation and expiry.

## Compatibility boundary

The compatibility layer currently parses protocol frames and translates the
supported Rich Presence activity updates. It does not yet open a Discord IPC
socket, impersonate a Discord application, implement every Discord RPC command,
or guarantee full Discord SDK compatibility.

Discord secret values are opaque join tokens, not public URLs. They are never
copied to the MelloID join URL field.

## Security model for the transport stage

The future desktop listener must:

- Bind only to the current user's local IPC endpoint (Unix domain socket on
  Unix-like systems and a restricted named pipe on Windows).
- Never open a TCP listener on a public or LAN interface.
- Validate the handshake and client ID before accepting activity.
- Restrict frame size, concurrent clients, request rate and activity lease.
- Associate each lease with the originating local process/client.
- Clear activity on disconnect or lease expiry.
- Require user opt-in and expose per-application block/allow controls.
- Publish only the fields the user has allowed in MelloID presence settings.

## Local transport implementation

The desktop client can now opt into two local listeners from Settings → MelloRPC
Presence. Both are off by default and require an application restart after
changing the settings.

- Native MelloRPC uses `mellorpc.sock` under the MelloGram config directory on
  Unix-like desktops, or the `mellorpc-v1` named pipe on Windows.
- Discord compatibility searches the standard local `discord-ipc-N` endpoint
  locations on Unix-like desktops and `discord-ipc-N` named pipes on Windows.
- The bridge accepts the v1 handshake and supported Rich Presence
  `SET_ACTIVITY` / `CLEAR_ACTIVITY` commands, plus PING/PONG and CLOSE.
- A connected Discord client renews its local activity lease while connected;
  closing the IPC connection clears its activity immediately.
- The local registry projects the newest unexpired lease into the profile and
  emits a local `mellorpc://presence` UI event. Public profile hosting/sync is
  still separate.

Both listeners are disabled by default. Users opt in under Settings → MelloRPC Presence and restart the client to apply changes. Only local sockets/named pipes are opened. No TCP, LAN, or public network listener is created. On Unix sockets, filesystem permissions are restricted
to the current user. On Windows, the named-pipe security descriptor is the
OS process default; a later hardening pass should explicitly constrain ACLs to
the current user SID.

When Discord itself is already running and owns the first standard IPC path,
MelloGram will try a free subsequent endpoint. Some SDKs prefer the first live
Discord endpoint and will keep using Discord, so compatibility testing may
require closing Discord or configuring the app's IPC path. This bridge does
not claim full Discord RPC parity.

## Next transport work

1. Implement the native MelloRPC local IPC listener and a small client SDK.
2. Implement the Discord IPC handshake and SET_ACTIVITY/CLEAR_ACTIVITY
   compatibility endpoint over the platform's local IPC mechanism.
3. Add application-specific privacy controls and activity presentation in
   MelloGram profiles.
4. Add integration tests using local socket pairs and Windows named pipes.
5. Connect the lease registry to the optional hosted MelloID presence service.

The protocol crate currently has no network or socket dependencies. Keeping the
transport outside this crate makes its framing and translation testable without
starting the application or exposing a listening endpoint.
