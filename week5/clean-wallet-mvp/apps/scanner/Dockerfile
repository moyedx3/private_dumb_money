# syntax=docker/dockerfile:1.6
# Build context expected: repo root of clean-wallet-mvp (week5/clean-wallet-mvp/).
# Build invocation:  docker build -f apps/scanner/Dockerfile -t clean-wallet-scanner .

FROM rust:1.88-bookworm AS builder
WORKDIR /build

# Install protoc — required by tonic-build at compile time.
RUN apt-get update && apt-get install -y --no-install-recommends \
    protobuf-compiler \
 && rm -rf /var/lib/apt/lists/*

# Copy the workspace root first (cheap; rebuilds when deps change)
COPY Cargo.toml Cargo.lock ./
COPY packages ./packages
COPY apps/scanner/Cargo.toml ./apps/scanner/Cargo.toml
COPY apps/scanner/build.rs ./apps/scanner/build.rs
COPY apps/scanner/proto ./apps/scanner/proto

# Pre-fetch dependencies for a layer cache hit when only source changes.
# The crate declares an explicit [lib] target (src/lib.rs) alongside the binary,
# so the prefetch stub must provide both or `cargo build` fails to read lib.rs.
RUN mkdir -p apps/scanner/src && \
    echo 'fn main() {}' > apps/scanner/src/main.rs && \
    echo '// prefetch stub' > apps/scanner/src/lib.rs && \
    cargo build --release -p clean-wallet-scanner && \
    rm -rf apps/scanner/src \
           target/release/clean-wallet-scanner \
           target/release/deps/clean_wallet_scanner-* \
           target/release/deps/libclean_wallet_scanner-* \
           target/release/deps/clean_wallet_scanner.d

COPY apps/scanner/src ./apps/scanner/src
RUN cargo build --release -p clean-wallet-scanner --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/clean-wallet-scanner /usr/local/bin/scanner

EXPOSE 8080
ENV LIGHTWALLETD_PRIMARY="https://testnet.zec.rocks:443"
ENV NETWORK="testnet"
ENV DSTACK_SOCKET="/var/run/dstack.sock"
ENV RUST_LOG="info,clean_wallet_scanner=debug"
CMD ["/usr/local/bin/scanner"]
