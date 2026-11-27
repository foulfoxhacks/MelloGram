# MelloGram customization platform: spec status

This tracks the customization-platform spec (Telegram client + customization
engine + cloud ecosystem) against what is built. Status: ✅ built · 🟡
partial · 🛠 planned (milestone) · ❌ will not build.

Principle kept throughout: **the client works fully offline.** Cloud
features will only add to it.

## Architecture mapping

The spec's module list maps onto the current crates like this. Modules are
split out into their own crates as they grow.

| Spec module | Where it lives now | Status |
|---|---|---|
| client-core | `app/src-tauri` (state, commands) | ✅ |
| telegram-adapter | `crates/mellogram-telegram` (grammers, pure-Rust MTProto) | ✅ |
| plugin-engine | `crates/mellogram-core::manager` | ✅ |
| plugin-sdk | `Plugin` trait (Rust) + `MG` API (UI), see [PLUGIN_API](PLUGIN_API.md) | ✅ (built-in plugins) |
| theme-engine | `crates/mellogram-core::theme` | ✅ |
| layout-engine | `crates/mellogram-core::appearance` (Appearance Studio) | 🟡 layout presets, no drag-and-drop editor yet |
| settings-engine | `crates/mellogram-core::settings` | ✅ |
| workspace-engine | `crates/mellogram-core::workspace` | ✅ |
| sharing-engine | `ProfilePackage` + inspection | ✅ file-based |
| security-engine | permissions, app lock, secret guard, safe mode | 🟡 no sandbox for third-party code yet |
| diagnostics-engine | `app/src-tauri/src/extras.rs` | ✅ |
| sync-engine, api-client, marketplace | — | 🛠 (M4–M5) |
| automation-engine | — | 🛠 (M7) |
| apps/desktop | Tauri (Windows, Linux; macOS from the same code) | ✅ |
| apps/mobile | Tauri mobile (Android, iOS) with responsive UI | 🟡 builds in CI; not tested on a device yet |
| apps/web, apps/admin, services/* | — | 🛠 (M4, M8) |

## 1.1 Messaging customization

| Feature | Status |
|---|---|
| Bubble styles: rounded, squared, compact, glass, transparent (outline), flat | ✅ Appearance Studio |
| Bubble radius, spacing, max width | ✅ |
| Message gradients (in/out), chat backgrounds and per-chat wallpapers | ✅ |
| Custom fonts, size, line height, letter spacing | ✅ |
| Entrance animations (fade/slide/pop), reduced motion | ✅ |
| Timestamp visibility (inline / hover / hidden) and format | ✅ (format via Precise Timestamps) |
| Avatar shape and size | ✅ |
| Username colours, per-contact overrides | ✅ Chat Colorizer |
| Density (compact / normal / comfortable) | ✅ |
| Quote styling (bar / box / minimal) | ✅ |
| Keyword highlighting rules with colours | ✅ Message Highlight |
| Per-contact / per-group appearance | ✅ per-chat overrides (accent, bubbles, wallpaper) |
| Message templates | ✅ Message Templates |
| Code block themes | 🟡 mono font + colours; syntax highlighting 🛠 (M3) |
| Hover actions / toolbar customization, read & typing indicators, reactions, stickers, link-preview cards | 🛠 (M3, needs media/reactions support) |

## 1.2 Interface and layout

| Feature | Status |
|---|---|
| Sidebar width, interface scaling, chat-list previews on/off | ✅ |
| Glass panels, custom scrollbars | ✅ |
| Command palette (Ctrl+P): settings, chats, plugins, workspaces, actions | ✅ |
| Keyboard navigation (Ctrl+K search, Ctrl+, settings, Ctrl+L lock) | ✅ |
| Responsive single-pane mobile layout, long-press menus | ✅ |
| Folders, custom navigation, floating panels, multi-window, frameless/acrylic window | 🛠 (M3) |

## 1.3 / 1.4 Plugin ecosystem

| Feature | Status |
|---|---|
| Enable/disable, config panels, dependencies, error reporting, crash isolation | ✅ |
| Declared permissions shown before enabling and on import | ✅ (storage enforced by host) |
| Per-plugin performance monitoring (calls, avg/max, start time) | ✅ |
| Plugin profiles | ✅ via workspaces |
| Load-order management | 🟡 priority in manifest; not user-editable |
| Third-party plugins: install, sandbox, signing, updates, rollback | 🛠 (M2b/M9) — WASM backend + permissioned UI |
| Plugins shipped | **30** (see `cargo run -p mellogram-plugins --example catalog -- --markdown`) |

From the proposed library of 100, these exist now: MessageLogger (Message
History), MessageCharacterCounter, MessageTemplates, MessageHighlight,
MessageLinkCleaner (Link Scrubber), MessageCopyEnhancer, MessageExport,
MessageStatistics, MessageTimestampFormatter (Precise Timestamps),
ChatColorizer, CompactMode/ChatDensityManager (Appearance Studio),
CustomScrollbars, InterfaceFontManager, BlurSensitiveContent /
HideChatPreviews / HideAvatars / HideUsernames / LocalPrivacyMode /
ScreenshotPrivacyOverlay (Privacy Mode), ClipboardWarning (Secret Guard),
AutoLockInterface (App lock), QuickNotes / ContactNotes, MessageBookmarks /
ConversationBookmarks, LinkCollection, QuickCalculator, CustomSnippets.

## 1.5 Customization studio

✅ Appearance Studio with expandable sections and live preview, colour
pickers, gradient editor, theme variables editor, CSS editor (QuickCSS),
import/export (themes, profiles). 🛠 shadow/animation editors, icon & sound
managers, drag-and-drop layout editor, theme versioning/rollback (M3).

## 2. Cloud & API — planned (M4)

Not built yet. Design commitments already reflected in the client:

- Telegram auth stays separate: login codes, 2FA passwords, session keys
  and API hashes are never exported, never in backups/profiles/bundles.
- Profiles are versioned (`format`, `version`) so old clients can reject
  newer packages safely.
- Sync will be optional, end-to-end encrypted before upload, selective.

## 2.3 Shareable profiles

✅ Export a workspace or the live setup as `.mgprofile.json`; ✅ inspect
before import (plugins, permissions, settings count, themes, CSS size,
warnings); ✅ import as a new inactive workspace that never overwrites your
themes. 🛠 public URLs, QR codes, device-to-device transfer, repositories.

## 4. Privacy & user controls

✅ blur previews/names/messages/media, hide avatars, privacy screen on
focus loss, panic blur (Ctrl+Shift+H), app lock with salted 120k-round
SHA-256 PIN, auto-lock timeout, lock on start, brute-force back-off,
secret detection before sending, plugin permission overview. 🛠 Telegram
session list, notification redaction, retention policies (M3).

## 5. Statistics & diagnostics

✅ Local dashboard (uptime, Telegram state, MTProto ping, message/send
counters, plugin & theme counts, storage, memory), plugin performance table,
testing center (storage, settings integrity, theme validation, QuickCSS,
plugin health/dependencies/permissions/performance, send-pipeline benchmark,
workspace integrity, shutdown state, Telegram config/auth/connectivity, DNS,
disk usage), support bundle with secrets redacted, safe mode with crash
detection. 🛠 network inspector, crash reports (M6).

## 6. Webhooks & automation — planned (M7)

When built, outgoing webhooks will validate destinations against
internal/metadata addresses (re-checked after DNS and redirects), sign
payloads, and back off on failure. Message automation will respect
Telegram's rate limits and anti-spam rules; no unsolicited bulk messaging.

## 7. Workspaces

✅ create from current setup, switch instantly (title-bar chip, palette,
settings), duplicate, rename/icon/description, delete, export/share.
Each workspace holds plugins + plugin settings, themes + variables,
QuickCSS, Appearance Studio and notification settings. 🛠 scheduled /
automatic switching, per-device assignment, compare/diff (M3/M7).

## 8. Admin console — planned (M8)

Separate app; administrators will never be able to read Telegram
conversations or decrypt private customization data.

## 10. Additional

- Accessibility: reduced motion, scaling, dyslexia-friendly font option,
  font/spacing controls ✅; high-contrast theme and screen-reader pass 🛠.
- AI-assisted customization 🛠 — opt-in, never sees conversations by default.

## Will not build

- Anything that bypasses Telegram server-side permissions or reveals other
  users' private data.
- Using Premium-only features without Premium.
- Collecting Telegram login codes, 2FA passwords or session keys in any
  MelloGram service.
