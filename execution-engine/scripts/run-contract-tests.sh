#!/usr/bin/env bash

set -o errexit

CONTRACTS=(
    "mint-token"
    "transfer-to-account"
)

for CONTRACT in "${CONTRACTS[@]}"; do
    cargo build -p "${CONTRACT}" --target wasm32-unknown-unknown
done

cargo test -p casperlabs-engine-grpc-server -- --ignored --nocapture
