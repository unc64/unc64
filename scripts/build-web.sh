#!/usr/bin/env bash
# Rebuild the ROM image and the WebAssembly machine for web/.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo run -q -p unc-asm -- rom/system.s -o rom/system.bin --range E000-FFFF --fill FF
cargo build -q --release --target wasm32-unknown-unknown -p unc-wasm
cp target/wasm32-unknown-unknown/release/unc_wasm.wasm web/unc.wasm
echo "web/unc.wasm: $(wc -c < web/unc.wasm) bytes"
echo "Run it: python3 -m http.server -d web 8064, then open http://localhost:8064"
