#!/bin/bash
set -e

RUSTFLAGS='-C link-arg=-s' cargo +stable build --target wasm32-unknown-unknown --release

near deploy swap-v1.forbitswap.testnet --wasmFile ./target/wasm32-unknown-unknown/release/my_contract.wasm
