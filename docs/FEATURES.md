# MelloGram feature map: Vencord → Telegram

MelloGram adapts the *feature set* of the Vencord Discord mod to a Telegram
desktop client written in Rust. It does not reuse Vencord's code (Vencord is
GPL-3.0; MelloGram is an independent implementation and is also licensed
GPL-3.0-or-later). The plugin names are our own; the **Adapts** column records
which Vencord feature inspired each one.

Status: ✅ shipped in 0.1 · 🛠 planned (phase in brackets, see [ROADMAP](ROADMAP.md)) · ➖ Telegram already does this natively · ❌ not applicable or not appropriate on Telegram.

One important difference from Discord: Telegram **allows third-party clients**.
You register your own app at my.telegram.org and use the official MTProto
API, so MelloGram does not need to patch or inject into anyone else's client.
Features still have to respect the
[Telegram API Terms of Service](https://core.telegram.org/api/terms); that is
why a few Vencord ideas are marked ❌ below.

## 1. Core client-mod functionality

| Vencord | MelloGram | Status |
|---|---|---|
| Plugin manager (enable/disable/configure) | Settings → Plugins | ✅ |
| Plugin search & categories | Search box, category + status filters | ✅ |
| Plugin settings | Auto-rendered from each plugin's schema (bool, text, number, select, list) | ✅ |
| Plugin dependencies | Enabling auto-enables dependencies; disabling a dependency is blocked while needed | ✅ |
| Favourites | ☆ on each plugin card, favourites sort first | ✅ |
| Error reporting | Crashing plugins are isolated (`catch_unwind`), errors shown on the card, auto-paused after 3 faults | ✅ |
| Reset plugin settings | "Reset to defaults" in each plugin's settings | ✅ |
| Theme manager | Settings → Themes: enable, order, edit, delete | ✅ |
| QuickCSS | Live editor, applied as you type | ✅ |
| Theme imports | From `.css` file, from https URL, or create new | ✅ |
| Theme variables | `--vars` in a theme's `:root` become an editor with colour pickers | ✅ |
| Backup & Restore | JSON export/import of plugins, settings, themes, QuickCSS (credentials excluded) | ✅ |
| Settings sync (cloud) | Optional, authenticated, end-to-end encrypted sync | 🛠 (7) |
| Updater | Version + changelog shown; GitHub release check | 🛠 (2) — check toggle exists, fetch not wired yet |
| Changelog | Settings → About & Updates | ✅ |
| Notifications (custom, positioned) | Toasts with position + duration settings | ✅ |
| Developer tools | Developer tab: composed stylesheet, live event log, reload UI; webview devtools in debug builds | ✅ |
| Browser extension / userscript | — | ❌ MelloGram is a native desktop app |
| Stable / PTB / Canary | Release channels (stable / beta builds) | 🛠 (2) |
| Platforms | Windows + Linux (macOS builds from the same code) | ✅ |

## 2. Plugins by category

### A. Appearance & interface

| Vencord | MelloGram | Status |
|---|---|---|
| ClientTheme | **Client Colors** (`client-colors`) — accent, background, corner radius | ✅ |
| ShowMeYourName | **Name Display** (`name-display`) | ✅ |
| DontRoundMyTimestamps | **Precise Timestamps** (`precise-timestamps`) | ✅ |
| ThemeAttributes | Every element has stable `mg-*` classes + `data-*` attributes | ✅ (built in) |
| BetterFolders | Chat folders sidebar with custom icons | 🛠 (2) |
| BetterSettings | Searchable settings, keyboard shortcut Ctrl+, | ✅ (built in) |
| AlwaysAnimate | Always play animated avatars / stickers | 🛠 (3, needs media) |
| AlwaysExpandRoles | Always expand admin titles in member lists | 🛠 (3) |
| IrcColors | Per-user colours hashed from user id | 🛠 (2) |
| RoleColorEverywhere | Admin-title colours in mentions/lists | 🛠 (3) |
| LoadingQuotes | Custom loading-screen quotes | 🛠 (2) |
| TypingTweaks | Show who is typing with avatars | 🛠 (3, needs typing updates) |
| Decor / FakeProfileThemes / USRBG | Third-party cosmetics shown to other users | ❌ requires an external service other clients can't see |
| NoProfileThemes | — | ➖ Telegram has no profile themes to hide |
| PlainFolderIcon | Option in the folders feature | 🛠 (2) |
| FixDiscordCss | — | ❌ Discord-specific |
| ColorSighted | Colour-blind friendly status dots | 🛠 (2) |
| oneko | Original cursor-following pet with our own art | 🛠 (later) |

### B. Chat & messaging

| Vencord | MelloGram | Status |
|---|---|---|
| ClearURLs | **Link Scrubber** (`link-scrubber`) — utm_*, fbclid, gclid, host-specific `si`, … | ✅ |
| TextReplace | **Text Replace** (`text-replace`) — plain and `re:` regex rules | ✅ |
| CustomCommands | **Quick Commands** (`quick-commands`) — /shrug, /tableflip, /lenny, your own | ✅ |
| CharacterCounter | **Character Counter** (`char-counter`) — of Telegram's 4096 limit | ✅ |
| MessageLogger | **Message History** (`message-history`) — deleted kept struck-through, edit history; memory only | ✅ |
| MessageLatency | **Send Latency** (`send-latency`) | ✅ |
| SilentMessageToggle | **Silent Send** (`silent-send`) — bell toggle, `/silent`, always-silent | ✅ |
| SilentTyping | **Silent Typing** (`silent-typing`) with per-chat allow list | ✅ |
| Unindent | **Unindent** (`unindent`) | ✅ |
| PreviewMessage | Rendered preview of formatting before sending | 🛠 (3) |
| QuickReply | Ctrl+↑/↓ to pick a message to reply to/edit | 🛠 (2) — double-click to reply is in |
| ReplyTimestamp | Timestamp in reply previews | 🛠 (2) |
| MessageLinkEmbeds | Preview `t.me/c/…/123` links inline | 🛠 (3) |
| BetterSearch | In-chat and global search with filters | 🛠 (2) — backend API exists |
| JumpToTop | Jump to first message | 🛠 (2) |
| MessageClickActions | Configurable double-click / modifier-click actions | 🛠 (2) |
| MoreQuickReactions | Configurable quick-reaction bar | 🛠 (3) |
| NoBlockedMessages | Hide messages from blocked users in groups | 🛠 (3) |
| QuickMention | Mention button on messages | 🛠 (2) |
| RevealAllSpoilers | Ctrl+click reveals all spoilers | 🛠 (3, needs entities) |
| GifPaste, ImageLink, HideMedia | Media-dependent | 🛠 (3) |
| AddAttachments (while editing) | Telegram allows replacing media on edit | 🛠 (3) |
| NoReplyMention | — | ➖ Telegram replies don't ping by default in the same way |
| TypingIndicator | Typing indicator in chat list | 🛠 (3) |
| ValidReply / ValidUser | — | ❌ Discord-specific bug fixes |

### C. Emoji, stickers & GIFs

| Vencord | MelloGram | Status |
|---|---|---|
| FavoriteEmojiFirst, CopyEmojiMarkdown | Emoji picker features | 🛠 (3) |
| ExpressionManager | Sticker-set manager (reorder, archive, clone own packs) | 🛠 (4) |
| StickerPaste, BetterGifPicker, GifProviderSwitcher, BetterGifAltText | Sticker/GIF picker features | 🛠 (3) |
| FakeNitro | — | ❌ using Premium-only features without Premium breaks the API ToS |
| SuperReactionTweaks, GreetStickerPicker | — | ❌ Discord-specific |

### D. Media & files

| Vencord | MelloGram | Status |
|---|---|---|
| AnonymiseFileNames | **Anonymise Uploads** (`anonymise-uploads`) | ✅ |
| BetterUploadButton | Drag-and-drop attachments with chips | ✅ (built in) |
| ImageZoom, PictureInPicture, ViewIcons, NoMosaic | Media viewer | 🛠 (3) |
| ImageFilename, CopyFileContents, DirectFileDownload, VoiceDownload | File features | 🛠 (3) |
| ReverseImageSearch | Context-menu reverse search (user-chosen engine) | 🛠 (3) |
| VoiceMessages | Record and send voice notes | 🛠 (4) |
| FixImagesQuality | Always load full-size photos | 🛠 (3) |
| Dearrow | Community titles for YouTube link previews | 🛠 (later) |
| UnsuppressEmbeds | **Link Preview Control** (`link-previews`) — the Telegram counterpart | ✅ |
| WebContextMenus | — | ➖ native app already has full menus |

### E. Voice, calls & streaming

Telegram calls use a separate protocol (tgcalls / WebRTC). MelloGram 0.1 has
no call support; it is planned as its own phase.

| Vencord | MelloGram | Status |
|---|---|---|
| CallTimer, VolumeBooster, NotificationVolume, VoiceChatDoubleClick | Call UI features | 🛠 (6) |
| VcNarrator | Text-to-speech for voice-chat joins/leaves | 🛠 (6) |
| WebScreenShare(Fixes), BiggerStreamPreview | Screen sharing in group calls | 🛠 (6) |
| DisableCallIdle, SecretRingToneEnabler | — | ❌ Discord-specific |
| XSOverlay | Forward notifications to VR overlay | 🛠 (later) |

### F. Profiles, contacts & chats

| Vencord | MelloGram | Status |
|---|---|---|
| PinDMs | **Local Pins** (`pin-chats`) — unlimited, device-local | ✅ |
| ReadAllNotificationsButton | **Read All** (`read-all`) | ✅ |
| ServerInfo / MemberCount | Chat info panel with member count | 🛠 (2) |
| PermissionsViewer | Show admin rights / default permissions for a chat | 🛠 (3) — backend API exists |
| MutualGroupDMs | Groups in common | 🛠 (3) |
| PlatformIndicators | — | ❌ Telegram does not expose the device a user is on |
| RelationshipNotifier | Notify when a contact removes you / leaves a group you share | 🛠 (4) |
| ShowConnections, ReviewDB | — | ❌ no Telegram equivalent / third-party service |
| ShowHiddenChannels / ShowHiddenThings | — | ❌ cannot and should not bypass access controls |
| ShowTimeoutDuration | Show restriction end time in groups you moderate | 🛠 (3) |
| CopyUserURLs | "Copy t.me link" in user/chat menus | 🛠 (2) — "Copy ID" is in |
| ForceOwnerCrown, BetterRoleContext, BetterRoleDot, AccountPanelServerProfile | Admin-title display tweaks | 🛠 (3) |
| FriendInvites, SortFriendRequests, ImplicitRelationships | Contacts manager | 🛠 (4) |

### G. Music & activity

| Vencord | MelloGram | Status |
|---|---|---|
| SpotifyControls / MusicRichPresence | "Now playing" in your bio or a pinned status via MPRIS (Linux) / SMTC (Windows), opt-in | 🛠 (4) |
| SpotifyShareCommands | `/np` command that shares the current track | 🛠 (4) |
| CustomRPC, GameActivityToggle, IgnoreActivities | — | ❌ Telegram has no rich presence |
| AutoDNDWhilePlaying, StreamerModeOnStream | Auto do-not-disturb / streamer mode (hide phone numbers, previews) | 🛠 (4) |
| SpotifyCrack | — | ❌ circumvents another service's terms |

### H. Privacy & notifications

| Vencord | MelloGram | Status |
|---|---|---|
| NoTrack | — | ➖ MelloGram has no telemetry at all |
| OnePingPerDM | Notify once per chat until you read it | 🛠 (2) |
| NoSystemBadge, NoPendingCount | Taskbar badge controls | 🛠 (2) |
| BlurNSFW | Blur media in chosen chats until hovered | 🛠 (3) |
| CustomIdle | Control when you appear offline (Telegram "last seen" privacy is server-side) | ➖ |
| AlwaysTrust, OpenInApp | Link-open confirmation and app handlers | 🛠 (2) |

### I. Developer & diagnostics

| Vencord | MelloGram | Status |
|---|---|---|
| ViewRaw | **View Raw** (`view-raw`) | ✅ |
| Developer settings / ConsoleShortcuts | Developer tab, `window.MelloGram` in devtools | ✅ |
| CrashHandler | Plugin crash isolation + corrupt-settings recovery (`settings.json.broken`) | ✅ |
| StartupTimings | Startup timing panel | 🛠 (2) |
| ShikiCodeblocks | Syntax-highlighted code blocks | 🛠 (2) |
| Experiments | Feature flags for MelloGram's own experiments | 🛠 (8) |
| ReactErrorDecoder, F8Break, NoDevtoolsWarning, ConcatenatedComponentExtractor, DevCompanion | — | ❌ specific to patching Discord's React bundle |

### J. Convenience

| Vencord | MelloGram | Status |
|---|---|---|
| SendTimestamps | Insert localised dates (Telegram has no timestamp markup, so it inserts text) | 🛠 (2) |
| FixCodeblockGap, Unindent | Code-block fixes | ✅ (Unindent) |
| NewGuildSettings | Default mute/archive settings for newly joined groups | 🛠 (3) |
| ReplaceGoogleSearch | Choose the search engine for "Search the web" | 🛠 (2) |
| WebKeybinds, NoF1 | Configurable keyboard shortcuts | 🛠 (2) |
| KeepCurrentChannel | Multi-account support keeps the open chat | 🛠 (5) |
| PauseInvitesForever, OverrideForumDefaults, NoOnboardingDelay, MutedServersActivityBadge, VesktopPatches, WebPWA, WebRichPresence | — | ❌ Discord/web specific |

## 3. Beyond Vencord (from the Control Center proposal)

| Idea | Status |
|---|---|
| Extension marketplace with ratings, collections, presets | 🛠 (10) |
| Third-party plugins (sandboxed WASM backend + permissioned UI scripts) | 🛠 (8–9) |
| Extension permissions manager | 🛠 (9) |
| Per-profile settings and admin-managed presets | 🛠 (7) |
| Rollback after failed updates, scheduled backups | 🛠 (7) |
| Theme sharing | 🛠 (10) |

The catalog table for the shipped plugins can be regenerated with:

```bash
cargo run -p mellogram-plugins --example catalog -- --markdown
```
