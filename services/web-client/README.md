<!-- coderabbit-ai-generated:v1 -->

<!-- coderabbit-ai-generated-section:v1:2937af2c27ce8c8666fb110f5c34969d70d42e57bb45e1f08286873ac784bd63 -->

# MelloGram web client

MelloGram Web is a Telegram client served by a Cloudflare Worker. It signs in
to an existing Telegram account with QR or phone, keeps the session in the
browser, lists recent chats and groups, pages history, sends text and
downloads small files. It also serves the MTProto relay and an in-memory
overlay demo. Sign-in can be switched off per deployment with
`TELEGRAM_WEB_BETA=false`.

## Telegram beta

The browser uses [mtcute](https://github.com/mtcute/mtcute) for MTProto key
exchange, encrypted RPCs, QR token refresh, data centre migration and two-step
verification. The Worker relays opaque bytes to one of five fixed Telegram data
centres. The authorization key and peer cache live in this browser's IndexedDB
(`mellogram-web`), so a reload keeps you signed in. The Worker does not store
accounts, sessions or messages.

The beta supports QR sign-in and phone sign-in where Telegram delivers a
supported code, recent direct chats and groups, paged message history, text
sending and downloads up to 4 MiB. Some verification methods, account creation,
broadcast channels, group topics, uploads and ephemeral content require the
desktop or Android app. Ordinary Telegram chats do not use the MelloGram
overlay. The demo generates throwaway identities and never stores their keys.

**Sign out** revokes the session at Telegram and clears the local copy.
**Forget this browser** deletes the local session without contacting Telegram,
for a shared or borrowed machine; the session then stays listed in
**Settings → Devices** and should be ended there. Closing the tab is not a
sign-out.

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

Sign-in needs no configuration: the Worker falls back to MelloGram's own
public Telegram application (the same pair the desktop app ships) and
`/api/telegram-config` returns it. To use a different application, set
`TELEGRAM_API_ID` and `TELEGRAM_API_HASH` in the Worker's variables; both are
validated, and an incomplete pair disables sign-in instead of mixing
applications. Set `TELEGRAM_WEB_BETA=false` to turn browser sign-in off
entirely. For Wrangler dev, the same variables can go in the ignored
`.dev.vars` file, and `PUBLIC_BASE_URL` must be the exact browser origin.
Existing sessions remain authorized until they sign out or are revoked.

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
   reconnect, a reload that restores the session, and sign-out. Confirm the
   session disappears from Devices, and that Forget this browser removes the
   `mellogram-web` IndexedDB database.
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