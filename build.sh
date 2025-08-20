#!/bin/bash

set -ex

cargo build 
cargo build --release
docker build . --tag ckwalsh/spars:latest

set +x

echo "# Numeric Sizes"
find . -name 'spars3' -type f | xargs ls -la
docker image inspect ckwalsh/spars:latest | jq '.[0].Size'

echo "# Human Readable Sizes"
find . -name 'spars3' -type f | xargs ls -lah
numfmt --to=iec-i --suffix=B --format="%.2f" $(docker image inspect ckwalsh/spars:latest | jq '.[0].Size')