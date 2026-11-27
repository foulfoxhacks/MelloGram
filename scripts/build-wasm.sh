#!/usr/bin/env bash
# Rebuild the committed WASM overlay bundle used by the web client.
#
#   scripts/build-wasm.sh
#
# Requires the wasm32-unknown-unknown target and a wasm-bindgen CLI whose
# version matches Cargo.lock (wasm-bindgen refuses to load mismatched output).
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
OUT="$ROOT/services/web-client/public/wasm"
EXPECTED="$(awk '/^name = "wasm-bindgen"$/{getline; gsub(/[^0-9.]/,"",$0); print; exit}' "$ROOT/Cargo.lock")"

command -v wasm-bindgen >/dev/null 2>&1 || {
  echo "wasm-bindgen $EXPECTED is required." >&2
  echo "Install it with: cargo install wasm-bindgen-cli --version $EXPECTED --locked" >&2
  exit 1
}
ACTUAL="$(wasm-bindgen --version | awk '{print $2}')"
[ "$ACTUAL" = "$EXPECTED" ] || {
  echo "wasm-bindgen $ACTUAL does not match Cargo.lock ($EXPECTED); install $EXPECTED." >&2
  exit 1
}

rustup target add wasm32-unknown-unknown >/dev/null
cargo build -p ametherian-truee2e-wasm --release --target wasm32-unknown-unknown
mkdir -p "$OUT"
wasm-bindgen --target web --no-typescript --out-dir "$OUT" --out-name mellogram_overlay \
  "$ROOT/target/wasm32-unknown-unknown/release/ametherian_truee2e_wasm.wasm"
echo "Wrote $OUT/mellogram_overlay.js and _bg.wasm"
