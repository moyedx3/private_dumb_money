# Attested Scanner — Phala dstack / Intel TDX 배포용 컨테이너.
# 실제 배포 절차는 docs/deploy-phala.md.
#
# 멀티스테이지: (1) Rust 사이드카 빌드 → (2) Node 런타임에 바이너리만 복사.

# === Stage 1: Rust 사이드카 빌드 ===
FROM rust:1-slim AS rust-builder

WORKDIR /build
COPY apps/zcash-scanner-rs ./apps/zcash-scanner-rs

WORKDIR /build/apps/zcash-scanner-rs
RUN cargo build --release

# === Stage 2: Node 런타임 ===
FROM node:24-slim

WORKDIR /app

# Stage 1에서 빌드된 Rust 사이드카 바이너리 복사.
COPY --from=rust-builder \
     /build/apps/zcash-scanner-rs/target/release/zcash-scanner-rs \
     /usr/local/bin/zcash-scanner-rs

# 루트 package.json + core + scanner만 복사 (웹 앱·Rust 소스·lockfile 제외).
COPY package.json ./
COPY packages/core ./packages/core
COPY apps/scanner ./apps/scanner

# core·scanner는 외부 런타임 의존성이 없어 install은 workspace 링크만 만든다.
RUN npm install --omit=dev --ignore-scripts

ENV PORT=8080
ENV ZCASH_SCANNER_BIN=/usr/local/bin/zcash-scanner-rs
EXPOSE 8080

# Node 24는 TypeScript 타입 스트리핑이 기본 활성화돼 .ts를 직접 실행한다.
CMD ["node", "apps/scanner/src/server.ts"]
