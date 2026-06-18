# A1-a 작업 계획 — 실제 lightwalletd 체인 조회까지

## 목표

A1 전체 중 `결제 감지/권한 검증`의 초입만 먼저 분리한다. 이번 작업의 완료 기준은 **TEE/엔진/dispatch 없이도 실제 lightwalletd gRPC 서버에 접속해서 체인 tip, compact block range, 선택 tx raw bytes를 조회할 수 있음**이다.

## 범위

### 이번 작업에 포함

1. `drop-indexer` Rust crate 최소 골격 생성
2. week5에서 검증된 `lightwalletd.rs` client와 proto 복사
3. live endpoint 설정용 `.env.example` / 로컬 `.env` 준비
4. `check-lightwalletd` 바이너리 작성
5. 실제 lightwalletd로 다음 조회 검증
   - `GetLatestBlock` → current tip
   - `GetBlockRange` → compact block 목록과 tx 수
   - 선택: `GetTransaction(txid)` → raw tx bytes

### 이번 작업에서 제외

- creator UFVK/IVK 복호화
- memo decode
- price 검증
- `K_drop` sealed-box dispatch
- bucket upload
- TEE attestation/provisioning

## 파일 계획

```text
indexer/
  Cargo.toml
  build.rs
  proto/
    service.proto
    compact_formats.proto
  src/
    lib.rs
    lightwalletd.rs
    bin/check-lightwalletd.rs
.env.example
.env                 # 로컬 실행용, gitignore
.gitignore
```

## 환경 변수

```bash
LIGHTWALLETD_URL=https://zec.rocks:443
LIGHTWALLETD_BACKUP_URL=https://mainnet.lightwalletd.com:9067
A1_SCAN_START=<optional height>
A1_SCAN_END=<optional height>
A1_FETCH_FIRST_TX=true
A1_TXID_HEX=<optional display txid hex>
RUST_LOG=info
```

## 실행

```bash
cargo run --manifest-path indexer/Cargo.toml --bin check-lightwalletd
```

특정 범위 조회:

```bash
A1_SCAN_START=3000000 A1_SCAN_END=3000002 \
  cargo run --manifest-path indexer/Cargo.toml --bin check-lightwalletd
```

특정 tx raw 조회:

```bash
A1_TXID_HEX=<64-char-display-txid> \
  cargo run --manifest-path indexer/Cargo.toml --bin check-lightwalletd
```

## 완료 기준

- [x] crate가 빌드된다.
- [x] live lightwalletd에서 chain tip을 가져온다.
- [x] live lightwalletd에서 compact block range를 가져온다.
- [x] compact block의 첫 txid로 full raw tx bytes를 가져온다.

## 다음 단계

이 조회 기반 위에 `detect.rs`를 추가해서 creator UFVK로 incoming note trial-decrypt를 붙인다. 그 다음 `memo.rs`와 price 검증으로 A1-a를 완성한다.
