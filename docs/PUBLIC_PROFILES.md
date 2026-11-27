# MelloID Public Profiles — deployment guide

This repository now includes a deployable Cloudflare Worker + D1 service at
services/melloid-public. It hosts public profile pages at /@/handle and an
owner dashboard at /dashboard. Owners sign in with Telegram, then import a
JSON profile export from MelloGram → MelloID Studio.

The service is scaffolded and tested in CI, but it is not deployed from this
repository. It requires a Cloudflare account, a domain, a D1 database, and
Telegram Login credentials created through @BotFather.

## Authentication

The service uses Telegram's current OpenID Connect Authorization Code flow
with PKCE. It requests only the openid and profile scopes. See Telegram's
[Log In With Telegram documentation](https://core.telegram.org/bots/telegram-login)
for the current endpoints, scopes, and token claims. It does not request
a phone number or bot write permission.

Create a bot in @BotFather, open its Login Widget configuration, register the
website origin and callback URL as Allowed URLs, and obtain the Client ID and
Client Secret. Keep the signing algorithm at RS256 (the Worker currently
allows RS256 only). The callback path is:

    https://YOUR_PROFILE_DOMAIN/auth/telegram/callback

The Worker validates the ID token signature against Telegram's JWKS endpoint,
then checks issuer, audience, expiry, nonce, and the numeric subject ID.
Telegram's documented endpoints are oauth.telegram.org/auth,
oauth.telegram.org/token, and oauth.telegram.org/.well-known/jwks.json.

## Cloudflare deployment

From services/melloid-public:

1. Install Node.js 20+ and run npm install.
2. Create the D1 database: npx wrangler d1 create melloid-public.
3. Copy the generated database ID into wrangler.toml, replacing the placeholder.
4. Set PUBLIC_BASE_URL in wrangler.toml to your real HTTPS profile domain.
5. Apply the schema: npx wrangler d1 migrations apply DB --remote.
6. Store the Telegram Client ID: npx wrangler secret put TELEGRAM_CLIENT_ID.
7. Store the Telegram Client Secret: npx wrangler secret put TELEGRAM_CLIENT_SECRET.
8. Deploy: npm run deploy.
9. Attach your profile domain to the Worker in Cloudflare and confirm the same
   origin/callback are registered in @BotFather.

For local development, use npm run dev after configuring a local D1 database
and .dev.vars values. Never commit .dev.vars or Telegram Client Secrets.

## Profile flow

- Sign in at /auth/telegram/start.
- Open /dashboard and import the JSON file exported by MelloGram.
- Publish; the service validates the profile and claims the unique slug.
- Share the resulting https://YOUR_PROFILE_DOMAIN/@/handle URL.
- Delete the hosted copy from the dashboard when needed.

The hosted page is a public projection. It omits hidden biography sections,
hidden connections, disabled widgets, local IDs, and local activity presence.
Uploaded account links are not treated as verified. Self-curated collectible
badges can be shown, but uploaded data cannot grant official, verified,
achievement, community, or server-issued badges.

Private profiles are not returned by the public API. Unlisted profiles are
available by direct URL but have no discovery/listing endpoint. Profile JSON
uploads are capped at 256 KiB on the server (the desktop export/import UI uses
a 1 MiB local file ceiling).

## Operational hardening before a public launch

- Configure Cloudflare edge rate limits for /auth/telegram/start,
  /auth/telegram/callback, and write endpoints.
- Add an explicit data retention and account deletion policy.
- Set up alerting and D1 backups.
- Add a provider OAuth verifier before displaying any linked account as
  verified.
- Add a hosted presence lease endpoint only after user privacy and rate limits
  are in place.
- Configure social preview metadata and custom error pages for public pages.

## Verified account connections

The dashboard supports GitHub and Discord account linking through their
official OAuth authorization-code flows with S256 PKCE, plus Steam through
Steam Community OpenID 2.0:

- GitHub requests only read:user and reads the authorized /user profile.
- Discord requests only identify and reads /users/@me.
- Steam uses a one-time OpenID state tied to the current Telegram session and
  validates the signed assertion against Steam's own check_authentication endpoint.
- The callback state is one-time, expires after 10 minutes, and is bound to
  the current Telegram-backed MelloID session.
- The provider access token is used only to fetch the account identity; it is
  not stored in D1, returned to the browser, or retained by the Worker.
- D1 stores the provider's stable account ID, display name, profile URL,
  approved avatar URL, verification timestamp, and the owner's public/private
  visibility choice.
- The same provider account cannot be linked to two different MelloID owners.
  Each owner may link one GitHub, one Discord, and one Steam account.
- Steam profile name and avatar are optional metadata. Set STEAM_WEB_API_KEY
  as a Wrangler secret to enrich the SteamID64 with the public player summary;
  Steam OpenID verification works without this key.
- Users can hide a link from their public profile or disconnect it at any time.

Configure these secrets with Wrangler (do not commit them):

    npx wrangler secret put GITHUB_OAUTH_CLIENT_ID
    npx wrangler secret put GITHUB_OAUTH_CLIENT_SECRET
    npx wrangler secret put DISCORD_OAUTH_CLIENT_ID
    npx wrangler secret put DISCORD_OAUTH_CLIENT_SECRET
    npx wrangler secret put STEAM_WEB_API_KEY

Register these callback URLs with your GitHub OAuth App and Discord
application:

    https://YOUR_PROFILE_DOMAIN/auth/connections/github/callback
    https://YOUR_PROFILE_DOMAIN/auth/connections/discord/callback

Steam does not require an OAuth client ID, secret, or separately registered
callback. The return URL is constructed by MelloID and includes a one-time
state parameter. The initial OpenID request fixes the realm to your profile
domain. The callback requires an exact return URL match, a SteamID64
claimed_id, signed identity fields, a recent response_nonce, and a successful
server-to-server check_authentication response before saving the account.
A nonce hash is stored with a short expiry to prevent replay.

The GitHub OAuth App needs the matching callback URL; the Worker asks only
for read:user. The Discord application needs the matching redirect URL and
the identify scope. The service does not request GitHub repository access,
Discord email, Discord guild access, or Discord connections.

The service stores a Telegram numeric subject ID to associate an owner with
their profile. It does not request or store a phone number, Telegram MTProto
session, API hash, or access hash.
