# MelloGram web client

The browser client for MelloGram, served as a Cloudflare Worker at
`wc.mellogram.foulfoxhacks.dev`.

**This is a skeleton.** It serves a real shell and a real status API, and it
reports every unfinished feature as unfinished. Nothing here signs in, stores
messages or claims encryption it does not have.

## What exists today

- `GET /` — static shell (`public/`), served through the `ASSETS` binding.
- `GET /api/health` — liveness.
- `GET /api/status` — service version and the capability list the page renders.
  Mutating methods are refused with `405` and an `Allow` header; unknown API
  routes are JSON `404`s.
- Security headers on every response: strict CSP (no inline script or style),
  `nosniff`, `DENY` framing, `Permissions-Policy`, `no-store` on JSON.

Capabilities live in `src/capabilities.js`, which is also what the tests assert
against, so the page cannot drift from the code.

## What is deliberately missing

- **Sign-in.** A Telegram login handshake in the browser has to be designed and
  tested before it exists; the desktop app owns account login today.
- **Chats and messages.** They need sign-in first. No server-side storage is
  provisioned, and none should be added for message content.
- **The end-to-end overlay.** The protocol is Rust (`crates/ametherian-truee2e`).
  A browser client needs it compiled to WebAssembly and a way to share the
  device identity with the desktop app. Until that is done, the web client must
  not offer encryption.

## Development

```bash
cd services/web-client
npm install
npm test            # node --test, no network
npx wrangler dev    # http://localhost:8787
```

Deploy with `npx wrangler deploy` once the Cloudflare account and the
`wc.mellogram.foulfoxhacks.dev` route are configured. The Worker needs no
secrets, database or KV namespace yet; adding one should come with the feature
that needs it.

## Layout

```
src/worker.js        routing, security headers, asset passthrough
src/capabilities.js  service metadata and the capability list
public/              static shell (index.html, styles.css, app.js)
test/                node:test suite for routing, headers and capability claims
```
