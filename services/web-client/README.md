# MelloGram web client

The browser client for MelloGram, served as a Cloudflare Worker at
`wc.mellogram.foulfoxhacks.dev`.

**This is a skeleton with a working transport.** It serves a real shell, a real
status API and the MTProto relay that a browser needs to reach Telegram at all.
Nothing here signs in yet, stores messages or claims encryption it does not have,
and the capability list says exactly that.

## What exists today

- `GET /` — static shell (`public/`), served through the `ASSETS` binding.
- `GET /api/health` — liveness.
- `GET /api/status` — service version and the capability list the page renders.
  Mutating methods are refused with `405` and an `Allow` header; unknown API
  routes are JSON `404`s.
- Security headers on every response: strict CSP (no inline script or style,
  `wasm-unsafe-eval` for the overlay module only), `nosniff`, `DENY` framing,
  `Permissions-Policy`, `no-store` on JSON.
- **Run the protocol** on the page: the Rust overlay, compiled to WebAssembly,
  generates two throwaway identities, shows their 60-digit safety number and
  round-trips a message. Everything is in memory; nothing is stored.
- `GET /relay/dc/1` … `/relay/dc/5` — the MTProto TCP relay (below). It opens a
  WebSocket that pipes opaque bytes to one Telegram data centre.

Capabilities live in `src/capabilities.js`, which is also what the tests assert
against, so the page cannot drift from the code.

## The relay

A browser cannot open a TCP socket and Telegram has no HTTP API, so the page
reaches a data centre through this Worker: it upgrades to a WebSocket and the
Worker pipes the bytes over TCP with `cloudflare:sockets`. The relay never parses,
logs or stores traffic, and holds no Telegram credential, `api_id` or session —
every byte is opaque MTProto.

It is deliberately **not** a TCP proxy: the destination comes from a fixed table
(`src/relay.js`) chosen by data-centre number, so no host or port is ever taken
from the client; only `PUBLIC_BASE_URL`'s origin may connect (browsers cannot
forge `Origin`, and a relay anyone may open is an open proxy); and each
connection has an 8 MiB per-direction byte budget, a 10-minute lifetime and a
cap of 4 concurrent connections per isolate.

`public/mtproto/transport.js` is the browser half: abridged and intermediate
framing, a reader that survives frames split across socket chunks, and the
transport tag (`0xef`, or `0xeeeeeeee`) that must precede the first frame.

Verified end to end against production, not just in unit tests: a `req_pq_multi`
sent through `wrangler dev` over `/relay/dc/2` reached DC2 and `resPQ` came back
(a 100-byte message, `auth_key_id` 0, constructor `0x05162463`). Two things that
only that run revealed are now pinned in code and tests: a packet sent without
the transport tag makes Telegram close the socket, and a WebSocket message that
is not an `ArrayBuffer` silently became zero bytes with a plain
`new Uint8Array(event.data)`.

## What is deliberately missing

- **Sign-in.** The transport is in place; the authorization key exchange and QR
  login that sit on top of it are not. They need a second DH generation, the
  encrypted message layer and `auth.exportLoginToken`, and none of it should
  ship before it is tested against a real data centre.
- **Chats and messages.** They need sign-in first. No server-side storage is
  provisioned, and none should be added for message content.
- **Overlay storage and chat wiring.** The protocol runs in the browser, but
  where a device secret lives (and how it would be shared with the desktop app)
  is still an open design question. Until it is answered, the demo's identities
  are discarded when the page closes and no chat uses the overlay.

## Development

```bash
cd services/web-client
npm install
npm test                     # node --test, no network
node dev.mjs 8787            # preview the shell without Wrangler
scripts/build-wasm.sh        # rebuild public/wasm after changing the crate
```

### Deployment

One Worker serves two custom domains (see `wrangler.toml`):

| Host | Serves |
| --- | --- |
| `wc.mellogram.foulfoxhacks.dev` | the shell, plus same-origin `/api/*` |
| `wc-api.mellogram.foulfoxhacks.dev` | the API only (`/api/*`); anything else is a JSON 404 |

Transport and endpoint rules, all covered by `npm test`:

- HTTPS only: plain HTTP `GET`/`HEAD` gets a `308` to HTTPS, writes get `403`;
  every response sends `Strict-Transport-Security` and the CSP includes
  `upgrade-insecure-requests`.
- The API grants CORS to the web client origin (`PUBLIC_BASE_URL`) only,
  with `Vary: Origin`; other origins get no CORS headers and a `403` preflight.
  No credentials are ever accepted cross-origin.
- Only `GET`, `HEAD` and `OPTIONS` exist; everything else is `405`.

`.github/workflows/web-client.yml` tests and deploys on pushes to `main` that
touch this folder, then smoke-tests both hostnames over TLS and checks HSTS
and CORS. Add the repository secrets `CLOUDFLARE_API_TOKEN` (Workers Scripts
edit + Workers Routes on the `foulfoxhacks.dev` zone) and
`CLOUDFLARE_ACCOUNT_ID`; without them the workflow tests and skips the deploy.
Manual deploy: `npx wrangler deploy`. The Worker needs no secrets, database or
KV namespace yet.

## Layout

```
src/worker.js        routing, security headers, asset passthrough
src/relay.js         MTProto relay: data-centre table, policy, WebSocket pipe
src/capabilities.js  service metadata and the capability list
public/              static shell (index.html, styles.css, app.js)
public/mtproto/      browser transport framing for the relay
public/wasm/         committed wasm-bindgen output for the overlay demo
test/                node:test suite for routing, headers, relay policy and framing
```
