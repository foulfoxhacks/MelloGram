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
- **UI.** The web client advertises the overlay as unavailable until this is
  wired and tested end to end.

## Why this is enough for a spike

The hard part was never the JS surface — it was whether the crypto stack,
randomness source and serialization all survive a wasm build without Tauri or
std::fs. They do. The remaining work is product work: a storage decision, the
envelope crate split, and the web UI.
