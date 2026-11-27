# Roadmap

Aligned with the platform spec's milestones. See [PLATFORM.md](PLATFORM.md)
for feature-level status.

| # | Milestone | State |
|---|---|---|
| 1 | **Client foundation** — Telegram integration, app shell, settings, persistence, theme engine, security boundaries | ✅ |
| 2 | **Plugin engine** — loader, settings, permissions (declared), lifecycle hooks, error isolation, first 20+ plugins | ✅ 30 built-in plugins · 🛠 third-party loading |
| 3 | **Customization studio** — visual editor, CSS editor, fonts, workspaces | ✅ Appearance Studio, QuickCSS, workspaces · 🛠 layout editor, icon/sound packs, media UI |
| 4 | **Cloud infrastructure** — accounts, API gateway, E2E-encrypted sync, devices, backups, profile sharing service | 🛠 next |
| 5 | **Community ecosystem** — plugin/theme marketplace, repositories, developer portal | 🛠 |
| 6 | **Diagnostics & analytics** — dashboard, plugin profiling, testing center, support bundles | ✅ local · 🛠 network inspector |
| 7 | **Webhooks & automation** — signed webhooks with SSRF protection, rule builder | 🛠 |
| 8 | **Administration** — admin console, moderation, API management, feature flags | 🛠 |
| 9 | **Expansion** — mobile clients, more platforms, accessibility | 🟡 Android/iOS build from the same code; responsive UI |
| 10 | **Ecosystem maturity** — certification, public SDK, audits | 🛠 |

Next up, in order: media (photos/stickers/voice), chat folders and
search, third-party plugins (WASM + permission prompts), then the cloud
service.
