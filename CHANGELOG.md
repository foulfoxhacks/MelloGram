## Login code delivery and resend — 2026-11-27

- Invoke `auth.sendCode` directly so the login screen can report the actual delivery channel: another signed-in Telegram session, SMS, a phone call, email, or Fragment. Third-party clients can be refused SMS entirely, so "we sent a code" was often false.
- Added "Resend code" using `auth.resendCode` with the `next_type` Telegram nominates, plus a countdown from the server's `timeout`.
- Added "Use a different number", which clears the pending login locally and cancels the code with `auth.cancelCode`.
- Phone numbers are normalised to E.164 on the backend before `auth.sendCode`; common RPC errors (flood limits, invalid or published API ID, banned numbers, SMS creation failures) now explain the next step.
- Handled `auth.sentCodeSuccess` (previously a panic) and `auth.sentCodePaymentRequired`, and replicated the private post-sign-in session setup for the raw `auth.signIn` path.

## Steam OpenID verification hardening — 2026-11-27

- Added a testable Steam OpenID response validator for exact return URL, signed identity fields, SteamID64, allowed OP endpoint, and recent nonce checks.
- The callback uses the validated OP endpoint for server-side check_authentication.
- Expanded connection visibility and disconnect endpoints to include Steam.
- Added a distinct steam-openid verification issuer marker.

## MelloID Steam account linking — 2026-11-27

- Added Steam Community OpenID 2.0 linking with callback assertion verification and nonce replay protection.
- Added optional Steam Web API player summary enrichment; account verification does not require a Steam Web API key.
- Added Steam to verified public connections and dashboard privacy controls.

## MelloID verified connections — 2026-11-27

- Added GitHub OAuth account linking (read:user) and Discord OAuth account linking (identify).
- Bound one-time PKCE connection requests to the signed-in Telegram session.
- Stored stable provider account IDs and public profile metadata only; provider tokens are not persisted.
- Added per-provider visibility toggles and disconnect controls.
- Added verified external accounts to public profiles only from owner-bound D1 records.
- Added database uniqueness constraints to prevent one external account being linked to multiple MelloID owners.

## MelloID Public Profiles service scaffold — 2026-11-27

- Added a Cloudflare Worker/D1 service with Telegram OpenID Connect login and PKCE.
- Added owner-only publishing, unique profile handles, public/unlisted/private visibility, and public projection filtering.
- Added dashboard import from MelloGram JSON exports and public profile pages.
- Sanitized uploaded connection verification and badge awards so profile imports cannot forge trust.
- Added Node.js tests for profile validation, public filtering, unsafe links, and trusted badge stripping.

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
