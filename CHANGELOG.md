## MelloID profile appearance presets — 2026-11-27

- Added six built-in profile themes, five avatar frame styles, and five profile effects.
- Added allowlisted appearance validation so exported profiles cannot inject arbitrary classes or scripts.
- Included selected theme/frame/effect presets in live preview and static HTML exports.

## MelloID modular profile widgets — 2026-11-27

- Added reorderable Games, Music, Projects, Communities, Activity, About and Custom Link widgets.
- Added per-widget visibility, bounded item content and safe HTTP(S) links.
- Included enabled widgets in static public profile exports.
- Added validation tests for widget item limits and URL schemes.

## MelloID collectible badges and native RPC example — 2026-11-27

- Added up to 24 self-curated collectible badges with bounded titles/descriptions and per-badge visibility.
- Preserved issuer-awarded badges and verified connections during ordinary profile edits.
- Added a dependency-free Node.js MelloRPC client example with automatic lease refresh.
- Extended CI JavaScript syntax checks to the profile editor and RPC sample client.

## MelloRPC local IPC service — 2026-11-27

- Added opt-in native MelloRPC and Discord Rich Presence IPC listeners.
- Added Windows named-pipe and Unix domain socket transports; no TCP listener.
- Added runtime lease expiry, disconnect cleanup, and profile presence events.
- Added MelloRPC settings (both listeners disabled by default; restart required).

## MelloID and MelloRPC foundation — 2026-11-27

- Added MelloID Studio with structured bios, profile appearance, visibility, connection cards and a live preview.
- Added portable JSON and static public HTML profile exports; the HTML export omits hidden profile elements.
- Added MelloRPC protocol types, native JSON framing, Discord IPC packet decoding, Rich Presence translation and bounded activity leases.
- Added Rust tests for identity validation, local persistence, RPC framing, Discord translation and lease expiry.
- Added frontend JavaScript syntax validation to CI.

# Changelog

## 0.1.0 — first preview

- Plugin manager: search, categories, favourites, per-plugin settings, dependency handling, crash isolation.
- 17 built-in plugins (Link Scrubber, Text Replace, Quick Commands, Message History, Silent Typing, Silent Send, …).
- Theme manager with ordering, `--variable` editor, install from file, URL or paste.
- QuickCSS editor with live preview.
- Backup & Restore (Telegram credentials are never exported).
- Telegram: phone + code + 2FA login, chat list, history, sending text and files, live updates.
