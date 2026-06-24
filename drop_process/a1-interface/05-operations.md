# A1 Interface — Operations and Verification

## 8. Live smoke CLI

## 8.1 Manual range scan

```bash
A1_UFVK=<creator_ufvk> \
A1_SCAN_START=<height> \
A1_SCAN_END=<height> \
A1_DEMO_DROP_ID=1 \
A1_DEMO_PRICE_ZAT=10000 \
cargo run --manifest-path indexer/Cargo.toml --bin scan-live
```

성공 기대:

```text
incoming_notes=1
decoded_memos=1
dispatches=1
bucket.put ... len=80
```

## 8.2 Encrypted state smoke

```bash
A1_UFVK=<creator_ufvk> \
A1_SCAN_START=<height> \
A1_SCAN_END=<height> \
A1_STATE_FILE=/tmp/a1-state-test.enc \
A1_STATE_KEY_HEX=<64_hex_dev_key> \
cargo run --manifest-path indexer/Cargo.toml --bin scan-live
```

같은 state file로 2회 실행 시 기대:

```text
1회차: dispatches=1
2회차: dispatches=0
```

---

## 9. 환경 변수

| 변수 | 용도 | 운영 주의 |
| --- | --- | --- |
| `LIGHTWALLETD_URL` | primary lightwalletd endpoint | public endpoint 가능 |
| `LIGHTWALLETD_BACKUP_URL` | backup lightwalletd endpoint | optional |
| `A1_UFVK` | live scan용 creator UFVK | 운영에서는 host env에 두지 말고 enclave secret으로 주입 |
| `A1_SCAN_START` | scan start height | smoke/manual |
| `A1_SCAN_END` | scan end height | smoke/manual |
| `A1_DEMO_DROP_ID` | demo catalog drop id | smoke/manual |
| `A1_DEMO_PRICE_ZAT` | demo price | smoke/manual |
| `A1_DEMO_K_DROP_HEX` | demo K_drop | smoke/manual |
| `A1_STATE_FILE` | encrypted state file path | host-visible ciphertext file |
| `A1_STATE_KEY_HEX` | dev-only state key | 운영 금지, enclave sealing으로 교체 |

---

## 12. Verification status

현재 테스트:

```bash
cargo fmt --manifest-path indexer/Cargo.toml -- --check
cargo check --manifest-path indexer/Cargo.toml
cargo test --manifest-path indexer/Cargo.toml
```

최근 통과 결과:

```text
30 passed
0 failed
1 ignored
```

검증되는 항목:

```text
memo raw/A1B64 codec
dispatch sealed box buyer-open
engine amount/replay/catalog checks
scan loop plumbing
state encryption/decryption
state-aware replay skip
API creator registration
API buyer bucket_key lookup
```

---
