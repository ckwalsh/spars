FROM rustlang/rust:nightly AS builder

WORKDIR /app

# Get the toolchain installed early
COPY rust-toolchain.plaid.toml ./rust-toolchain.toml
RUN rustup show

COPY Cargo.toml Cargo.lock  ./
COPY src/ src/

ENV RUSTFLAGS="-Zlocation-detail=none -Zfmt-debug=none"

ARG FEATURES="signal-hook"

RUN touch README.md

RUN cargo build \
  --profile plaid \
  --target x86_64-unknown-linux-musl \
  -Z build-std="std,panic_abort" \
  -Z build-std-features="optimize_for_size" \
  --no-default-features \
  --features "$FEATURES"

# upx image hasn't been updated for 8 years, pin the sha256 in case it is ever compromised
FROM gruebel/upx@sha256:99891d91d6e409ad0dcdb4c70839f105ebf20421bebf896bfc4df827d5a8b19e AS upx

COPY --from=builder /app/target/x86_64-unknown-linux-musl/plaid/spars /spars-uncompressed
RUN upx --best --lzma -o /spars /spars-uncompressed

FROM scratch

COPY --from=upx /spars /spars

WORKDIR /public

ENV RUST_MIN_STACK=8192

ENV ADDR=0.0.0.0
ENV PORT=3000
ENV ROOT=/public
ENV FALLBACK_PATH=/404.html
ENV ALLOW_HIDDEN=false

ENTRYPOINT ["/spars"]
