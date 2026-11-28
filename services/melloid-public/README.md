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

<!-- coderabbit-ai-generated:v1 -->

<!-- coderabbit-ai-generated-section:v1:759abdcf2388c85e4650d0eadef3f656d281233b14ba6918e3f062e0537e57ec -->

The error page ends with a `Reference:` code. The Worker warning in
`npx wrangler tail` also reports the configured widget username and the
public bot ID from the token prefix. It does not log the token or callback
field values.

| Reference | Meaning | Fix |
| --- | --- | --- |
| `signature` | The callback HMAC does not match the configured token and received fields | Start a fresh login. If it recurs, check the current bot token and whether the callback query changed in transit. |
| `bot_token_missing` | Verification had no token | Keep `TELEGRAM_BOT_TOKEN` configured. |
| `expired` | The callback is outside the allowed time window | Start a fresh sign-in. |
| `missing` | The callback page was opened without Telegram's response | Start from **Continue with Telegram**. |
| `malformed_hash` / `malformed_id` / `malformed_auth_date` / `duplicate_field` | A required callback field is invalid or duplicated | Use the widget from a fresh login page and check any proxy or redirect rules affecting the callback. |

For a recurring `signature` error, verify the current token privately with
Telegram Bot API's [getMe](https://core.telegram.org/bots/api#getme); check the
returned bot ID and username. A token prefix can match the correct bot even
if the token was replaced. If needed, put the current token on the production
`melloid-public` Worker from this directory with
`npx wrangler secret put TELEGRAM_BOT_TOKEN`, then start a fresh sign-in.
If it still fails, check whether redirects or proxy rules add or alter
parameters on `/auth/telegram/widget`. Keep the signed callback URL private.



<!-- /coderabbit-ai-generated:v1 --><!-- coderabbit-ai-generated:v1 -->

<!-- coderabbit-ai-generated-section:v1:2142de5495360f88ed075ec8d5e7567a0de535bee0849a591d58a3c0c890ba14 -->

Fresh widget sign-ins are bound to the browser and consumed once. Apply the
D1 migrations before deploying this Worker. A `state` or `used` reference
means the user should start again from **Continue with Telegram**.

Both login paths now use the Telegram user ID. Previous OIDC builds used
`sub` as the database key. If you have records created that way, reconcile
them before rollout: back up D1, verify the mapping using the distinct
`sub` and `id` claims from a verified Telegram ID token, and resolve any
profile or connection conflicts before moving ownership. Existing rows are
left intact; a login will never guess the mapping.

After deploying to staging with its own Telegram credentials, sign in using
a dedicated account, check that the dashboard sees the expected profile,
and sign out. If you support both methods, also check the same account
through both. The automated deployment smoke check covers the configured
entry point and redirects without a real user account.

The bot token is trimmed before use

<!-- /coderabbit-ai-generated:v1 -->, so a trailing space or newline left by a
copy/paste no longer breaks verification. Fields this build does not know are
part of the signature rather than a rejection, so a new field from Telegram
cannot lock everyone out; an added field that Telegram did not sign fails as
`signature`.
