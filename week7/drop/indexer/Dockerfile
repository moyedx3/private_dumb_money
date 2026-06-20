# syntax=docker/dockerfile:1.6
# Reproducible build of drop-indexer (Lane A2). Build context = this directory.
#   docker build -t ghcr.io/<owner>/<repo>/drop-indexer:<tag> .
# For TRUE reproducibility, pin the builder base by @sha256 digest after your first build
# (`docker inspect --format='{{index .RepoDigests 0}}' rust:1.90-bookworm`) and rebuild.
# No protoc needed (A2 uses no gRPC; the dstack client is raw HTTP-over-UDS).
# Rust >= 1.89 required (dryoc 0.8 sets rust-version 1.89, edition 2024).

FROM rust:1.90-bookworm AS builder
WORKDIR /build

# Dependency prefetch layer (cache hit when only src changes). The crate has both a [lib]
# (src/lib.rs) and a [[bin]] (src/main.rs), so the stub must provide both.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
 && echo 'fn main() {}' > src/main.rs \
 && echo '' > src/lib.rs \
 && cargo build --release --locked \
 && rm -rf src target/release/drop-indexer \
           target/release/deps/drop_indexer-* \
           target/release/deps/libdrop_indexer-*

COPY src ./src
RUN cargo build --release --locked

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/drop-indexer /usr/local/bin/drop-indexer
RUN mkdir -p /data/bucket
ENV PORT=8080 BUCKET_DIR=/data/bucket DSTACK_SOCKET=/var/run/dstack.sock
EXPOSE 8080
CMD ["drop-indexer"]
