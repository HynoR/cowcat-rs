#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

pushd "$ROOT_DIR/wasm" >/dev/null
cargo build --target wasm32-unknown-unknown --release
popd >/dev/null

cp "$ROOT_DIR/wasm/target/wasm32-unknown-unknown/release/cowcat_pow_wasm.wasm" "$ROOT_DIR/static/assets/catpaw.wasm"
echo "Wrote $ROOT_DIR/static/assets/catpaw.wasm"
