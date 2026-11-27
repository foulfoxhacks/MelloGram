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
