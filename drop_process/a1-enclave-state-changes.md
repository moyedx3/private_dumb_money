# A1 Enclave Scanner State Changes

## 목적

운영자가 서버 파일시스템을 볼 수 있어도 scanner cursor/replay state의 평문을 볼 수 없도록, state 저장 경계를 `StateCipher` 기반 암호화 파일로 분리했다. 실제 운영에서는 복호화 키가 host/env에 있으면 안 되며, enclave/TEE 내부 sealing key로만 `StateCipher`를 구현해야 한다.

## 추가/변경된 파일

| 파일 | 변경 내용 |
| --- | --- |
| `indexer/src/state.rs` | 신규. `ScanState`, `MemoryScanState`, `StateCipher`, 개발용 `SecretboxStateCipher`, `EncryptedFileScanState` 추가. 상태 직렬화는 자체 binary format을 사용하고 파일에는 secretbox 암호문만 저장한다. |
| `indexer/src/lib.rs` | `state` 모듈 export 추가. |
| `indexer/src/scan_loop.rs` | `scan_once_with_state`와 `process_incoming_notes_with_state` 추가. 이미 처리된 txid는 full tx fetch 전에 skip하고, dispatch가 생성된 txid만 replay state에 기록한다. 범위 스캔 성공 후 `last_scanned_height`를 갱신한다. |
| `indexer/src/bin/scan-live.rs` | 선택적으로 `A1_STATE_FILE` + 개발용 `A1_STATE_KEY_HEX`를 받아 encrypted state를 load/save하도록 연결. 미설정 시 기존 수동 smoke 동작은 유지된다. |
| `.env.example` | 개발용 encrypted state smoke 환경변수 예시 추가. |

## 저장되는 state 의미

평문 구조는 enclave 내부에서만 존재해야 한다.

```text
last_scanned_height: 마지막으로 성공 처리한 block height
seen_txids: dispatch가 생성된 txid set
```

파일에는 위 구조가 그대로 저장되지 않는다. 저장 전 `StateCipher::seal()`을 거치며, 현재 개발 구현은 dryoc/libsodium-compatible secretbox(XSalsa20-Poly1305)를 사용한다.

## 보안 경계

### Host/operator가 볼 수 있는 것

```text
scan_state.enc 암호문
lightwalletd endpoint
bucket put 결과
프로세스 실행 여부
```

### Enclave 내부에만 있어야 하는 것

```text
creator UFVK/IVK
K_drop
decoded memo(drop_id, e_pub)
plaintext scan state
state encryption key
sealed-box 생성 전 재료
```

## 중요한 제한

`SecretboxStateCipher::from_hex_key`와 `A1_STATE_KEY_HEX`는 개발 smoke용이다. 운영 서버의 env는 운영자가 볼 수 있으므로, 이것만으로는 “운영자도 state를 볼 수 없음” 요구를 만족하지 않는다.

운영 구성에서는 다음처럼 교체해야 한다.

```rust
struct EnclaveStateCipher;

impl StateCipher for EnclaveStateCipher {
    fn seal(&self, plaintext: &[u8]) -> anyhow::Result<Vec<u8>> {
        // TEE sealing key로 enclave 내부에서만 암호화
        todo!()
    }

    fn open(&self, ciphertext: &[u8]) -> anyhow::Result<Vec<u8>> {
        // 같은 enclave measurement/policy에서만 복호화
        todo!()
    }
}
```

즉 host는 암호문 파일만 들고 있고, 키 파생/복호화/재암호화는 enclave 내부에서만 실행되어야 한다.

## 개발 smoke 실행 예시

```bash
A1_UFVK=<creator_ufvk> \
A1_SCAN_START=3387683 \
A1_SCAN_END=3387683 \
A1_DEMO_DROP_ID=1 \
A1_DEMO_PRICE_ZAT=10000 \
A1_STATE_FILE=.omx/a1-scan-state.enc \
A1_STATE_KEY_HEX=<64_hex_dev_key> \
cargo run --manifest-path indexer/Cargo.toml --bin scan-live
```

같은 state 파일로 다시 같은 range를 실행하면 이미 dispatch된 txid는 state에서 skip된다.

## 현재 완료된 것

- scanner state 추상화 추가
- encrypted file state 저장/로드 구현
- 암호문 파일이 평문 magic/txid를 포함하지 않는 단위 테스트 추가
- wrong key 복호화 실패 테스트 추가
- state-aware scan path 추가
- `scan-live`에 optional encrypted state 연결

## 아직 남은 것

- 실제 TEE sealing key 기반 `StateCipher` 구현
- 운영용 long-running polling binary 또는 service loop
- reorg 정책: `last_scanned_height`만 전진하는 단순 cursor에서 confirmation depth/reorg rollback 정책으로 확장 필요
- 실제 bucket 구현과 state save의 원자적 트랜잭션 경계 정리

## 검증 결과

```bash
cargo fmt --manifest-path indexer/Cargo.toml -- --check
cargo check --manifest-path indexer/Cargo.toml
cargo test --manifest-path indexer/Cargo.toml
```

추가 live smoke:

```bash
A1_STATE_FILE=/tmp/a1-state-test.enc \
A1_STATE_KEY_HEX=<dev_key> \
A1_SCAN_START=3387683 A1_SCAN_END=3387683 \
cargo run --manifest-path indexer/Cargo.toml --bin scan-live
```

1회차는 dispatch 1건과 `bucket.put ... len=80`을 생성했고, 같은 state 파일로 2회차를 실행했을 때 이미 dispatch된 txid는 skip되어 dispatch 0건이 됐다. 저장 파일은 `A1STATE1` 평문 magic과 display txid ASCII를 포함하지 않는 것을 확인했다.
