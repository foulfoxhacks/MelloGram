# WASM overlay spike

The browser client needs the overlay, and the overlay is Rust. This spike
answers one question: **does `ametherian-truee2e` build and work as
WebAssembly?** It does.

## Result

- `crates/ametherian-truee2e` compiles for `wasm32-unknown-unknown` with one
  feature: `wasm`, which turns on `getrandom/wasm_js` (Web Crypto through
  wasm-bindgen). `.cargo/config.toml` selects that backend for the wasm target
  only (`--cfg getrandom_backend="wasm_js"`).
- `crates/ametherian-truee2e-wasm` exposes the protocol through wasm-bindgen and
  passes its tests natively:

  | Binding | Purpose |
  | --- | --- |
  | `WasmIdentity.generate()` / `fromSecret(bytes)` / `secretBytes()` | Device identity |
  | `WasmIdentity.bundle()` | Armored public bundle to share |
  | `WasmIdentity.safetyNumber(bundle)` | 60-digit verification string |
  | `WasmSession.start(identity, bundle, chatId)` | Initiator handshake |
  | `WasmSession.encrypt(text, chatId)` / `decrypt(armored, chatId)` | Pairwise messages |
  | `WasmSession.state()` / `fromState(bytes)` | Ratchet persistence |
  | `accept(identity, initial, chatId)` | Responder half: session state + the plaintext that arrived |

  ```bash
  rustup target add wasm32-unknown-unknown
  cargo test -p ametherian-truee2e-wasm                     # native, no browser
  cargo build -p ametherian-truee2e-wasm --release --target wasm32-unknown-unknown
  wasm-bindgen --target web --out-dir services/web-client/public/wasm \
    target/wasm32-unknown-unknown/release/ametherian_truee2e_wasm.wasm
  ```

## What is deliberately not bound yet

- **Persistence.** The web client must seal `secretBytes()` and session state
  itself; the spike does not choose IndexedDB, a passphrase or a key hierarchy.
  Until it does, the browser build must not store overlay state.
- **Group sender keys and the app envelopes** (`mg-e2e-groupkey:v1:` and
  friends). Those live in `mellogram-e2e`, which is desktop glue; moving the
  envelope logic into a shared crate is the next step, not a protocol change.
- **Chat UI.** The overlay is wired into the web shell as an in-memory demo, not
  into messaging: no conversation uses it until persistence is decided.

## How it is wired into the web client

The browser bundle is committed, so deploying the web client needs no Rust
toolchain:

```bash
scripts/build-wasm.sh     # rebuilds services/web-client/public/wasm from the crate
```

- `services/web-client/public/wasm/mellogram_overlay.{js,_bg.wasm}` — the
  wasm-bindgen `web` target output (about 380 KB total).
- The shell loads it only when the reader presses **Run the protocol**, so the
  page start costs nothing.
- The Worker's CSP allows `'wasm-unsafe-eval'` for scripts and nothing else; the
  wasm file is served as `application/wasm`.
- Chat ids cross the boundary as JavaScript numbers; the binding rejects
  fractional, non-finite or unsafe values instead of rounding them.
- Tests: four native Rust tests, five Node tests that load the committed wasm
  and run a whole pairwise exchange, so the bundle cannot drift from the crate
  unnoticed.

## Why this is enough for a spike

The hard part was never the JS surface — it was whether the crypto stack,
randomness source and serialization all survive a wasm build without Tauri or
std::fs. They do. The remaining work is product work: a storage decision, the
envelope crate split, and the web UI.
