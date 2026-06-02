# dstack TEE simulator — Phala 의 공식 https://github.com/Dstack-TEE/dstack
# sdk/simulator 를 빌드해 컨테이너로 만든다. 실 TDX 없이 PhalaAttestation 코드 경로 검증용.
#
# Dstack-TEE 의 코드는 master branch 기준. 안정성 위해 commit pinning 도 가능 (선택).

FROM rust:1.82-slim AS builder

RUN apt-get update && apt-get install -y --no-install-recommends \
    git \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /build
RUN git clone --depth 1 https://github.com/Dstack-TEE/dstack.git

WORKDIR /build/dstack/sdk/simulator
# build.sh 가 cargo build --release 와 socket setup 을 함.
RUN bash build.sh

# Runtime image — slim debian 에 simulator binary 만.
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# build.sh 결과물 위치는 sdk/simulator 디렉토리 안. 정확한 binary path 가 환경에 따라 다를 수 있어
# 디렉토리 전체를 복사하고 entrypoint 가 알아서 실행.
COPY --from=builder /build/dstack/sdk/simulator /opt/dstack-simulator
WORKDIR /opt/dstack-simulator

# socket 파일이 /shared 에 만들어지도록 — compose 의 volume 마운트가 거기.
# 기본 simulator 는 cwd 에 socket 을 만드는 것 같음 — symlink 또는 working dir 변경.
RUN mkdir -p /shared && ln -sf /shared /opt/dstack-simulator/sockets

# 실 socket 위치를 /shared 로 (binary 가 cwd 기준이면 cwd 가 /shared 여야).
WORKDIR /shared

ENTRYPOINT ["/opt/dstack-simulator/dstack-simulator"]
