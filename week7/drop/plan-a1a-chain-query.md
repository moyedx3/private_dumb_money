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

## 후속 구현: UFVK memo probe

`probe-ufvk` 바이너리는 현재 조회 기반 위에서 UFVK/IVK trial-decrypt까지 수행한다. Sapling/Orchard 모두 external scope와 internal scope를 시도하므로, `zecscope-scanner`에서 보이는 내부/change-scope note도 full transaction decrypt 경로에서 놓치지 않는다.

범위 스캔:

```bash
A1_UFVK=<creator_ufvk> A1_SCAN_START=<height> A1_SCAN_END=<height> \
  cargo run --manifest-path indexer/Cargo.toml --bin probe-ufvk
```

단일 tx 조회:

```bash
A1_UFVK=<creator_ufvk> A1_TXID_HEX=<64-char-display-txid> A1_TX_HEIGHT=<height> \
  cargo run --manifest-path indexer/Cargo.toml --bin probe-ufvk
```

성공 시 `incoming_notes.total`, `value_zat`, `memo_utf8`, `memo_hex`를 출력한다.

## 구현 메모 — Zcash decrypt dependency

`detect.rs`는 week5 probe와 같은 `zcash_keys 0.13` / `zcash_primitives 0.27` 계열을 사용한다. 이 조합은 `orchard 0.13.1`에 맞춰져 있는데 해당 crate 버전이 crates.io에서 yanked 상태라 새 lockfile만으로는 선택이 실패한다. 그래서 week5에서 이미 검증된 lockfile 계열을 재사용해 동일 버전을 고정했다. 다음에 Zcash crate를 업그레이드할 때는 `zcash_keys`/`zcash_primitives`/`orchard`를 한 묶음으로 올려야 한다.

## 후속 구현: zecscope-scanner compact-block smoke

`zecscope-scan` 바이너리는 docs.rs의 `zecscope_scanner::{Scanner, ScanRequest, Network}` API 형태에 맞춰 lightwalletd compact block을 변환한 뒤 UFVK로 incoming candidate를 찾는다. 이 경로는 compact-block 기반이라 full tx raw fetch가 필요 없고 빠르지만, `zecscope-scanner 0.1.0` 구현상 memo는 항상 `None`으로 반환된다. memo 확인은 계속 `probe-ufvk`의 full transaction decrypt 경로가 담당한다.

```bash
A1_UFVK=<creator_ufvk> A1_SCAN_START=<height> A1_SCAN_END=<height> \
  cargo run --manifest-path indexer/Cargo.toml --bin zecscope-scan
```

선택 환경 변수:

```bash
A1_NETWORK=mainnet        # 또는 testnet. 미설정이면 UFVK prefix로 추정
A1_KEY_ID=a1-creator      # 결과 추적용 표시 이름
```

검증 결과: 제공받은 UFVK와 `3363060..=3363067` 범위로 실행했을 때 `blocks.fetched=8`, `compact_txs.fetched=10`, `zecscope.matches=1`이 출력되었다. 이 hit는 Orchard incoming candidate로 amount `74999` zatoshi이며, memo는 zecscope compact scan 단계에서는 노출되지 않는다.

의존성 메모: `zecscope-scanner 0.1.0`은 `orchard 0.11.0`을 요구하고 이 버전은 crates.io에서 yanked 상태다. 빌드를 재현하려고 lockfile에 해당 버전을 명시 고정했다. lockfile을 새로 만들면 의존성 선택이 실패할 수 있으므로, 업그레이드 시에는 zecscope 또는 Zcash crate 묶음을 같이 교체해야 한다.
