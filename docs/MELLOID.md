# MelloID — identity and profile system

MelloID is MelloGram's identity layer, deliberately separated from the Telegram
MTProto session and chat pipeline. It provides a customizable local profile,
a portable public HTML export, and the data contracts needed for a hosted
profile service later.

## Available in the client

- Versioned MelloProfile schema (schema_version = 1) with local atomic
  persistence in melloid-profile.json.
- Profile handle, display name, headline, avatar and banner URLs, accent color.
- Six built-in profile themes, five avatar frames, and five decorative effects.
  These are allowlisted preset tokens, not arbitrary CSS.
- Public, unlisted, and private visibility intent.
- Up to 12 structured Bio Studio sections with per-section visibility.
- Up to 40 external connection cards with service/category, display name,
  optional HTTP(S) URL, and per-connection visibility.
- Verifier metadata for trusted account links. A local edit preserves a
  verification claim only when the existing connection ID, provider, display
  name, and profile URL remain unchanged. Changing those values invalidates
  the claim.
- Up to 100 badge records. Users can create up to 24 self-curated collectible
  badges; the client cannot mint verified, achievement, community, or official
  awards. Existing trusted/server-issued badge records are preserved by local
  edits.
- Up to 24 reorderable widgets: Games, Music, Projects, Communities, Activity,
  About, and Custom Link. Each widget can be enabled/disabled and has bounded
  text items and an optional HTTP(S) link.
- Live profile preview, JSON export/import, and static public HTML export.

## Export and import behavior

**JSON export** is a full local backup. It can contain hidden biography sections,
private connection cards, verifier metadata, badge records, and profile
preferences. Treat the file as private and store it accordingly.

**JSON import** accepts files up to 1 MiB and requires the supported schema
version. The application validates the imported object through the native Rust
model before saving it. Trust is not imported blindly:
- Existing connection verification is preserved only when it still matches
  the same local connection identity.
- New or altered connections lose verifier metadata.
- Imported collectible badges are rewritten as self-issued.
- Verified, achievement, community, official, and server-issued badges are
  retained only from the current local profile, not granted by an import.

**Static public HTML export** requires the profile visibility to be Public.
It includes visible bio sections, connections, badges, enabled widgets, the
chosen appearance presets, and activity only when presence display is enabled.
Text is HTML-escaped and link destinations are restricted to HTTP(S). The
result is a portable HTML file that the owner can host on their own website.
It is not a canonical MelloID URL and does not synchronize future edits.

## What Public means today

The visibility field is the intended audience setting used by the editor and
exporter. It does not upload a profile to a MelloGram-hosted website. No public
profile API, account directory, slug registry, or remote synchronization
service is currently deployed.

A hosted public profile service remains a separate stage. It should provide:

- Immutable server-side profile IDs independent of editable slugs.
- Authenticated profile writes and anonymous, read-only public views.
- Server-side visibility filtering and slug uniqueness/cooldowns.
- Revocable OAuth/account-link flows and encrypted provider tokens.
- Signed verification claims with issuer provenance.
- Server-authoritative badge catalog and award history.
- Presence leases with expiry, source attribution, privacy controls, and rate
  limits.
- Data export, profile deletion, abuse reporting, and account recovery.

Telegram phone numbers, access hashes, MTProto session files, and other
private Telegram credentials must never be exposed in public profile APIs.

## Data and API boundaries

The Rust crate melloid-core owns the profile model, validation, and local store.
Tauri commands get_melloid_profile and save_melloid_profile are the UI boundary.
Telegram connection state is not used as profile identity.

The shared PresenceActivity model is transport-neutral. The local MelloRPC
service can project an unexpired local activity lease into the local profile
when show_presence is enabled. This local projection is not remote publication
or public presence synchronization.

## Next MelloID work

1. Hosted public profile service and profile pages.
2. OAuth/ownership verification for supported connection providers.
3. Server-issued badge catalog, award rules, and achievement progress.
4. Per-provider connection management, token revocation, and sync controls.
5. Public profile discovery, share links, and abuse/privacy controls.
