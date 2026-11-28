# MelloID public profiles

## Deployment

Live at `https://id.mellogram.foulfoxhacks.dev` (Worker `melloid-public`,
D1 database `melloid-public`; migrations applied). D1's free tier (5 GB,
5M reads and 100k writes per day) covers this service, so it costs nothing
at MelloGram's scale.

- `.github/workflows/web-client.yml` tests, applies migrations, deploys and
  smoke-tests on pushes to `main`, given `CLOUDFLARE_API_TOKEN` and
  `CLOUDFLARE_ACCOUNT_ID` repository secrets.
- Manual: `npx wrangler d1 migrations apply melloid-public --remote` then
  `npx wrangler deploy`. Where Static Assets uploads cannot authenticate, use
  `node scripts/embed-assets.mjs && npx wrangler deploy -c wrangler.embedded.toml`.
- Sign-in answers `503` until one Telegram login method is configured:
  - **Login Widget** (simplest): create a bot with @BotFather, run `/setdomain`
    for `id.mellogram.foulfoxhacks.dev`, then set the secret
    `TELEGRAM_BOT_TOKEN` and the variable `TELEGRAM_BOT_USERNAME`.
  - **OpenID Connect**: secrets `TELEGRAM_CLIENT_ID` and `TELEGRAM_CLIENT_SECRET`
    (takes precedence when set). Account
  linking additionally needs the GitHub / Discord client secrets and
  `STEAM_WEB_API_KEY`. Public profile pages work without them.

## When sign-in says it failed

The error page ends with a `Reference:` code, and the same reason is logged by
`npx wrangler tail` (field names only, never their values or the token):

| Reference | Meaning | Fix |
| --- | --- | --- |
| `signature` | The payload was signed by a different bot than `TELEGRAM_BOT_TOKEN` | Re-set the secret with `npx wrangler secret put TELEGRAM_BOT_TOKEN` using the token of the bot named by `TELEGRAM_BOT_USERNAME` |
| `expired` | `auth_date` is more than 10 minutes old | Sign in again without leaving the page open |
| `missing` | The callback page was opened directly | Start again from **Continue with Telegram** |
| `malformed_hash` / `malformed_id` / `malformed_auth_date` | The query string was not a Telegram payload | Nothing; a real attempt never produces these |
| `duplicate_field` | A parameter arrived twice | Nothing; a real attempt never produces this |

The bot token is trimmed before use, so a trailing space or newline left by a
copy/paste no longer breaks verification. Fields this build does not know are
part of the signature rather than a rejection, so a new field from Telegram
cannot lock everyone out; an added field that Telegram did not sign fails as
`signature`.
