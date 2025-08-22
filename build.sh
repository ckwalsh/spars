#!/bin/bash

set -ex

MUSL_TARGET=x86_64-unknown-linux-musl

cargo build --bins --examples --release
cargo build --bins --examples --release --target "$MUSL_TARGET"

PLAID_TOOLCHAIN=nightly-2025-08-18
rustup component add rust-src --toolchain "$PLAID_TOOLCHAIN"
rustup component add rust-src --toolchain "$PLAID_TOOLCHAIN" --target "$MUSL_TARGET"

RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
  cargo "+$PLAID_TOOLCHAIN" build \
    --bins --examples \
    --profile plaid \
    -Z build-std="std,panic_abort" \
    -Z build-std-features="optimize_for_size,panic_immediate_abort"

RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none" \
  cargo "+$PLAID_TOOLCHAIN" build \
    --bins --examples \
    --profile plaid \
    --target "$MUSL_TARGET" \
    -Z build-std="std,panic_abort" \
    -Z build-std-features="optimize_for_size,panic_immediate_abort"

docker build . --tag spars:latest
docker build . -f examples/Dockerfile.hyper --tag spars:examples-hyper

set +x

echo "# Numeric Sizes"

(find . -name 'spars' -type f -executable; find . -name 'hyper' -type f -executable) | sort | xargs -n1 ls -s --block-size=1 | sed "s/ /\t/"

echo ""
docker image inspect spars:examples-hyper | jq '.[0].Size' | tr -d '\n'
echo -e '\tDocker spars:examples-hyper'
docker image inspect spars:latest | jq '.[0].Size' | tr -d '\n'
echo -e '\tDocker spars:latest'

echo ""
echo "# Human Readable Sizes"

(find . -name 'spars' -type f -executable; find . -name 'hyper' -type f -executable) | sort | xargs -n1 ls -sh | sed "s/ /\t/"

echo ""
numfmt --to=iec-i --suffix=B --format="%0f" $(docker image inspect spars:examples-hyper | jq '.[0].Size') | tr -d '\n'
echo -e '\tDocker spars:examples-hyper'
numfmt --to=iec-i --suffix=B --format="%0f" $(docker image inspect spars:latest | jq '.[0].Size') | tr -d '\n'
echo -e '\tDocker spars:latest'