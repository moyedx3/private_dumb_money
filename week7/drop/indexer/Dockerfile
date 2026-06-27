# syntax=docker/dockerfile:1.6
# Reproducible build of drop-indexer (A2 HTTP/TEE runtime + A1 scanner). Build context = this directory.
#   docker build -t ghcr.io/<owner>/<repo>/drop-indexer:<tag> .
# For TRUE reproducibility, pin the builder base by @sha256 digest after your first build
# (`docker inspect --format='{{index .RepoDigests 0}}' rust:1.90-bookworm`) and rebuild.
# The A1 scanner uses lightwalletd gRPC; build.rs compiles proto files with vendored protoc.
# Rust >= 1.85 is sufficient for the current dependency set.

FROM rust:1.90-bookworm AS builder
WORKDIR /build

# Dependency prefetch layer (cache hit when only src changes). The crate has both a [lib]
# (src/lib.rs) and a [[bin]] (src/main.rs), so the stub must provide both.
COPY Cargo.toml Cargo.lock build.rs ./
COPY proto ./proto
RUN mkdir src \
 && echo 'fn main() {}' > src/main.rs \
 && echo '' > src/lib.rs \
 && cargo build --release --locked \
 && rm -rf src target/release/drop-indexer \
           target/release/deps/drop_indexer-* \
           target/release/deps/libdrop_indexer-*

COPY src ./src
RUN cargo build --release --locked --bin drop-indexer

FROM debian:bookworm-slim AS runtime
RUN apt-get update \
 && apt-get install -y --no-install-recommends ca-certificates \
 && rm -rf /var/lib/apt/lists/*
COPY --from=builder /build/target/release/drop-indexer /usr/local/bin/drop-indexer
RUN mkdir -p /data/bucket
ENV PORT=8080 BUCKET_DIR=/data/bucket DSTACK_SOCKET=/var/run/dstack.sock
EXPOSE 8080
CMD ["drop-indexer"]
