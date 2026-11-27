# MelloID — identity foundation

MelloID is MelloGram's identity and profile layer. It is intentionally a separate Rust subsystem from Telegram messaging so profile features can evolve without changing the MTProto session or chat pipeline.

## Implemented in the foundation

- Versioned MelloProfile schema (schema_version = 1).
- Public, unlisted, and private profile visibility.
- Structured bio sections with bounded lengths.
- External connection records with provider/category, display name, optional HTTP(S) profile URL, visibility, and verifier metadata.
- Badge records with distinct verified, achievement, community, official, and collectible categories.
- Ordered profile widgets with JSON configuration.
- A normalized activity model for playing, listening, watching, streaming, creating, working, and custom activities.
- Atomic local profile persistence in melloid-profile.json.
- Tauri commands get_melloid_profile and save_melloid_profile.
- Validation for slugs, display names, bio sizes, connection counts, URL schemes, and accent colors.

## Current boundaries

The local profile is a client-side draft/cache. Setting visibility to public does not publish it to the internet. A public profile requires an authenticated MelloID service, ownership verification, a public read API, abuse controls, and a hosted profile renderer. Those services are not present in this repository yet.

Likewise, connection records are not proof of ownership. The profile editor must not be allowed to set trusted verification metadata. A future verifier must issue and sign verification claims after OAuth or another provider-owned challenge. Imported profile data must be treated as untrusted.

Badges are data records at this stage. Once synchronized, badge definitions and grants must be server-authoritative; clients may display but must not mint official, verified, or achievement awards.

Presence is a normalized data contract only. The current foundation does not publish live activity or run a Discord IPC listener.

## Public profile service contract (next stage)

The client can export a static HTML profile when visibility is set to Public. The export includes only visible bio sections and connections, escapes user-provided text, and can be hosted independently. It does not provide synchronized updates, account ownership verification, or a canonical hosted MelloID URL.\n\nA hosted service should provide:

- Immutable account/profile ID independent of the editable slug.
- Authenticated profile writes and public read-only profile endpoints.
- Slug uniqueness and rename cooldowns.
- Per-section visibility filtering on the server, not just in the UI.
- Revocable OAuth connections and encrypted token storage.
- Signed verification claims and issuer provenance.
- Badge catalog, award history, and anti-forgery validation.
- Presence leases with expiry, source attribution, rate limits, and opt-out.
- Data export and account/profile deletion.

Telegram identity should be linked through a user-consented authorization flow. Do not expose Telegram phone numbers, access hashes, session files, or private profile fields through public MelloID endpoints.

## MelloRPC compatibility plan

The shared PresenceActivity model is transport-neutral. Native MelloRPC and the Discord compatibility adapter should both normalize into this model.

The Discord adapter should begin with the local desktop IPC transport and supported Rich Presence activity update/clear operations. It must be opt-in, bound to the local machine, and never expose a remote unauthenticated listener. Compatibility is incremental: document supported fields and gracefully ignore unsupported Discord fields rather than claiming complete SDK parity.

Native MelloRPC should have a documented local SDK/protocol, application registration, activity expiry, process ownership, and explicit user privacy controls. The public presence service should accept short-lived signed leases, not trust arbitrary client timestamps indefinitely.

## Profile Studio implementation

The desktop client now includes MelloID Studio: structured Bio Studio sections, display-name and slug editing, public/unlisted/private visibility intent, avatar/banner/accent customization, connection cards, per-item visibility, and a live preview. Users can export their local profile as JSON or download a static public HTML page. The HTML export requires Public visibility, includes only visible bio sections/connections/badges, escapes user text, and is a portable file rather than a hosted or synchronized profile URL.

## Planned delivery stages

1. Foundation: schema, validation, local persistence, Tauri read/write commands, and architecture boundaries.
2. Profile experience: editor, preview, public profile renderer, section privacy, and safe media handling.
3. Connections and badges: OAuth verifier service, public connection cards, badge catalog, award issuance, and achievement progress.
4. MelloRPC: native local protocol, Discord IPC compatibility bridge, activity cards, expiration, and presence privacy.
5. Widgets and social features: widget SDK/sandbox, timeline, squads, spaces, and community recognition.

## Local data

The profile is stored beside MelloGram settings in the application's config directory as melloid-profile.json. It is not included in portable settings backups yet. This avoids accidentally exporting personal profile or linked account metadata before the user-facing export/privacy controls exist.
