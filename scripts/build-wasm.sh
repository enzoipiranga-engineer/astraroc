#!/usr/bin/env bash
# Compila o núcleo para WebAssembly e gera os bindings JS/TS em packages/core-js/wasm.
set -euo pipefail
cd "$(dirname "$0")/.."
cargo build -p astra-wasm --target wasm32-unknown-unknown --release
wasm-bindgen target/wasm32-unknown-unknown/release/astra_wasm.wasm \
  --target web --out-dir packages/core-js/wasm
