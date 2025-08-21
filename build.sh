#!/bin/bash

set -ex

cargo build --all-targets
cargo build --all-targets --release

PLAID_TOOLCHAIN=nightly-2025-08-18
PLAID_TARGET=x86_64-unknown-linux-musl
rustup component add rust-src --toolchain "$PLAID_TOOLCHAIN" --target "$PLAID_TARGET"

RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
  cargo "+$PLAID_TOOLCHAIN" build \
    --target "$PLAID_TARGET" \
    --bins --examples \
    --profile plaid \
    -Z build-std="std,panic_abort" \
    -Z build-std-features="optimize_for_size,panic_immediate_abort"

docker build . --tag spars:latest
docker build . -f Dockerfile.hyper --tag spars:hyper

set +x

echo "# Numeric Sizes"

(find . -name 'spars' -type f -executable; find . -name 'hyper' -type f -executable) | sort | xargs -n1 ls -s --block-size=1 | sed "s/ /\t/"

echo ""
docker image inspect spars:hyper | jq '.[0].Size' | tr -d '\n'
echo -e '\tDocker spars:hyper'
docker image inspect spars:latest | jq '.[0].Size' | tr -d '\n'
echo -e '\tDocker spars:latest'

echo ""
echo "# Human Readable Sizes"

(find . -name 'spars' -type f -executable; find . -name 'hyper' -type f -executable) | sort | xargs -n1 ls -sh | sed "s/ /\t/"

echo ""
numfmt --to=iec-i --suffix=B --format="%0f" $(docker image inspect spars:hyper | jq '.[0].Size') | tr -d '\n'
echo -e '\tDocker spars:hyper'
numfmt --to=iec-i --suffix=B --format="%0f" $(docker image inspect spars:latest | jq '.[0].Size') | tr -d '\n'
echo -e '\tDocker spars:latest'