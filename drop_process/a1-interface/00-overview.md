# A1 Interface — Overview

## 0. 현재 A1 구현 상태 요약

A1은 현재 다음 흐름을 코드로 수행할 수 있다.

```text
creator/drop config 등록 또는 mock
→ buyer가 Zcash shielded payment 전송
→ lightwalletd에서 block/full tx 조회
→ creator UFVK로 incoming note decrypt
→ memo에서 drop_id/e_pub 파싱
→ amount >= price_zat 검증
→ K_drop을 buyer e_pub으로 sealed-box wrapping
→ dispatch blob 생성
→ bucket boundary에 put
→ buyer dispatch lookup service vector에서 조회 가능
```

실제 mainnet smoke도 완료됨.

```text
height=3387683
incoming_notes=1
decoded_memos=1
dispatches=1
value_zat=10000
blob_len=80
```

---

## 1. 모듈/파일 맵

| 파일 | 역할 |
| --- | --- |
| `indexer/src/memo.rs` | A1 memo payload encode/decode. raw 40B + `A1B64:` text fallback 지원. |
| `indexer/src/detect.rs` | UFVK 기반 Sapling/Orchard incoming note decrypt. full tx memo 추출. |
| `indexer/src/dispatch.rs` | `K_drop` sealed-box wrapping, dispatch blob key 생성. |
| `indexer/src/engine.rs` | 결제 검증 엔진. replay check, catalog lookup, amount check, dispatch 생성, bucket put. |
| `indexer/src/scan_loop.rs` | lightwalletd range scan → full tx fetch → detect → memo decode → engine 연결. state-aware scan 포함. |
| `indexer/src/state.rs` | encrypted cursor/replay state 경계. `ScanState`, `StateCipher`, encrypted file state. |
| `indexer/src/api.rs` | HTTP/enclave adapter가 호출할 API service vector. creator drop 등록, buyer dispatch 조회. |
| `indexer/src/bin/scan-live.rs` | 실제 lightwalletd live smoke CLI. optional encrypted state 사용 가능. |

---
