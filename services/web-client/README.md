<!-- coderabbit-ai-generated:v1 -->

<!-- coderabbit-ai-generated-section:v1:2937af2c27ce8c8666fb110f5c34969d70d42e57bb45e1f08286873ac784bd63 -->

# MelloGram web client

MelloGram Web is a Telegram client served by a Cloudflare Worker. The default
deployment serves a status page, the MTProto relay and an in-memory overlay
demo. An opt-in beta adds existing-account sign-in and basic chats.

## Telegram beta

The browser uses [mtcute](https://github.com/mtcute/mtcute) for MTProto key
exchange, encrypted RPCs, QR token refresh, data centre migration and two-step
verification. The Worker relays opaque bytes to one of five fixed Telegram data
centres. The browser holds the authorization key in memory. The Worker does not
store accounts, sessions or messages.

The beta supports QR sign-in and phone sign-in where Telegram delivers a
supported code, recent direct chats and groups, paged message history, text
sending and downloads up to 4 MiB. Some verification methods, account creation,
broadcast channels, group topics, uploads and ephemeral content require the
desktop or Android app. Ordinary Telegram chats do not use the MelloGram
overlay. The demo generates throwaway identities and never stores their keys.

Use **Sign out** to revoke the Telegram session. Closing or reloading the tab
clears its local state but does not revoke the authorization at Telegram. Users
can review and revoke sessions in Telegram under **Settings → Devices**.

## Develop and deploy

From `services/web-client`, using Node 22 or later:

```sh
npm ci
npm run check
npm run dev -- --infer-origin-from-routes=false --var PUBLIC_BASE_URL:http://127.0.0.1:8787
```

Open `http://127.0.0.1:8787/`. Use Wrangler when testing the relay; the
dependency-free shell adapter (`npm run preview`) cannot open relay sockets.
Both Wrangler configs build the browser bundle before serving or deploying.
The embedded config additionally packages the static files into the Worker.

| Host | Routes |
| --- | --- |
| `wc.mellogram.foulfoxhacks.dev` | Shell, same-origin API and relay |
| `wc-api.mellogram.foulfoxhacks.dev` | Status and health API, plus relay |

`/api/health` reports Worker liveness, not Telegram connectivity.
`/api/status` reports the features available on that deployment. When the beta
is enabled, `/api/telegram-config` on the web host returns the public Telegram
application ID and hash. Treat those as public application identifiers. Never
use a Telegram account password, verification code or session as a Worker
binding.

For a staging Worker, set `TELEGRAM_WEB_BETA=true`, `TELEGRAM_API_ID` and
`TELEGRAM_API_HASH` in the Worker's variables, using an application registered
with Telegram. For Wrangler dev, the same variables can go in the ignored
`.dev.vars` file. Set `PUBLIC_BASE_URL` to the exact browser origin. Leave
`TELEGRAM_WEB_BETA` unset or set it to `false` to disable new browser logins.
Existing tabs remain authorized until they sign out or close.

GitHub Actions tests the client and validates the deployment bundle before
deploying. Normal deployment uses `npm run deploy` and needs the existing
`CLOUDFLARE_API_TOKEN` and `CLOUDFLARE_ACCOUNT_ID` repository secrets.

## Release checks

Before enabling the beta on the public domains:

1. On a staging deployment, verify HTTPS, HSTS, API CORS, WebAssembly MIME
   types, both allowed and rejected WebSocket origins, and the Worker bundle.
2. With a dedicated Telegram account, exercise QR refresh and sign-in,
   phone code and two-step verification, a DC redirect, cancel and retry,
   recent chats, older history, message send and receive, a small attachment,
   reconnect, and sign-out. Confirm the session disappears from Devices.
<!-- coderabbit-ai-generated:v1 -->

<!-- coderabbit-ai-generated-section:v1:95c1d5d3b391ccda4530e1c0670a02d18184f1c20e559d74f5412457440cc069 -->

3. Verify and monitor the Worker rate limit for `/relay/dc/*` (30 upgrades
   per IP per minute in each Cloudflare location). Add edge rules if staging
   traffic requires them. The browser Origin check is not client authentication.
   Each connection lasts at most ten minutes and carries up to 8 MiB per
   direction. The four-connection cap applies to each isolate.

<!-- /coderabbit-ai-generated:v1 -->
4. Keep chat overlay disabled until browser secret storage, key changes,
   durable ratchet updates and desktop interoperability are designed and
   tested. See [WASM overlay](../../docs/WASM.md) and
   [end-to-end encryption](../../docs/E2E.md).

Telegram references: [QR login](https://core.telegram.org/api/qr-login),
[authorization and test accounts](https://core.telegram.org/api/auth),
and [API terms](https://core.telegram.org/api/terms).


<!-- /coderabbit-ai-generated:v1 -->