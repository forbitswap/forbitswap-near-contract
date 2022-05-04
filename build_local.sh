#!/bin/bash
set -e

RUSTFLAGS='-C link-arg=-s' cargo +stable build --target wasm32-unknown-unknown --release

near deploy tihu.testnet --wasmFile ./target/wasm32-unknown-unknown/release/forbitswap_near_contract.wasm
