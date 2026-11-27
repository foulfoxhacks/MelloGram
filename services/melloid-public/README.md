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
- Sign-in answers `503` until the Worker secrets `TELEGRAM_CLIENT_ID` and
  `TELEGRAM_CLIENT_SECRET` are set (`npx wrangler secret put …`). Account
  linking additionally needs the GitHub / Discord client secrets and
  `STEAM_WEB_API_KEY`. Public profile pages work without them.
