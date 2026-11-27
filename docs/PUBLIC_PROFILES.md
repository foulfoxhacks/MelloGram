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

The service stores a Telegram numeric subject ID to associate an owner with
their profile. It does not request or store a phone number, Telegram MTProto
session, API hash, or access hash.
