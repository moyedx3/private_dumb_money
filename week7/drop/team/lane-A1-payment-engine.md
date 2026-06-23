# Lane A1 — 결제-플로우 엔진 (Rust #1)

> 같이 읽기: [`00-overview.md`](./00-overview.md) (프로젝트 전체) · [`interfaces.md`](./interfaces.md) (바이트 계약 — I1·I2·I3) · [`../plan-a1-payment-flow.md`](../plan-a1-payment-flow.md) (이 레인의 TDD 단계별 계획). 스펙 근거: [`../spec.md`](../spec.md) §4.3·§7.3 + 체인지로그 C1–C6 (특히 C6 = 브랜치 함정).
>
> **읽는 사람:** Rust는 능숙하지만 Zcash/TEE는 처음인 개발자. 도메인 지식은 본문에서 풀어 쓰고, 정확한 바이트 배치는 매번 다시 유도하지 말고 `interfaces.md`의 번호를 가리킨다.

## 1. 한 줄 요약

TEE 서버 안에서 도는 `drop-indexer` 크레이트를 만든다 — 크리에이터의 **IVK(보기전용 incoming 키)** 로 들어온 shielded 결제를 감지하고, 메모에서 `(drop_id, e_pub)`를 복원하고, 금액이 가격 이상이면 콘텐츠 열쇠 `K_drop`을 구매자 공개키로 봉인해 버킷에 올린다.

## 2. 큰 그림에서 내 위치

```
구매자 결제(메모) ─▶ Zcash 체인 ─▶ [A1: drop-indexer] ─▶ dispatch blob ─▶ 버킷
                                    IVK로 감지 → 메모 해독          (게시판)
        (A2가 IVK/price/K_drop를 mock Catalog로 공급) ┘   └▶ (D가 mock Bucket로 받음)
```

A1은 **체인에서 결제가 들어오는 쪽**과 **버킷에 blob이 나가는 쪽** 사이의 순수 로직이다. 양옆 부품(A2의 Catalog, D의 Bucket)은 **트레이트로 mock** 해서 A1 혼자 끝까지 빌드·테스트된다.

## 3. 내가 받는 것 / 내보내는 것

| 방향 | 무엇 | 인터페이스 | 경로 |
|---|---|---|---|
| **받음** | 구매자 메모 `drop_id(8 BE) ‖ e_pub(32)` = 40B raw | **I1** (A1 소유) | 체인 → full tx의 `enc_ciphertext` 안 |
| **받음** | `DropConfig { price_zat, k_drop, creator_ufvk }` | **I3-b** (A2 소유) | `Catalog::lookup(drop_id)` — **mock** |
| **내보냄** | dispatch blob = `crypto_box_seal(K_drop, e_pub)` = 80B | **I2** (A1 소유) | `Bucket::put(key, blob)` — **mock**, 키 = `blake2b256(ek_pub ‖ txid)` |

A1이 **포맷을 소유**하는 건 I1(메모)·I2(dispatch blob) 둘뿐 — 이 둘의 바이트 배치는 `interfaces.md`와 **반드시 일치**해야 하고(구매자 앱 B가 같은 걸로 인코딩/디코딩한다), 나머지는 mock 경계 뒤라 내가 바꿀 수 없다.

> mock 하는 두 경계 (남이 소유, A1은 시그니처만 동결):
> ```rust
> // A2 소유 (Catalog). 값: I3-b.
> pub struct DropConfig { pub price_zat: u64, pub k_drop: [u8; 32], pub creator_ufvk: String }
> pub trait Catalog: Send + Sync { fn lookup(&self, drop_id: u64) -> Option<DropConfig>; }
> // D 소유 (Bucket).
> #[async_trait::async_trait]
> pub trait Bucket: Send + Sync { async fn put(&self, key: &str, bytes: &[u8]) -> anyhow::Result<()>; }
> ```

## 4. 만드는 것 (단계별)

> **각 단계의 정확한 실패 테스트 → 구현 → 커밋은 [`plan-a1-payment-flow.md`](../plan-a1-payment-flow.md)에 그대로 있다.** 여기선 **무엇을 만들고 왜 그런지**만 설명한다. 파일은 `week7/drop/indexer/` 아래 새 크레이트.

**Task 0 — ✅ 완료: 크레이트 골격 + 검증된 lightwalletd 클라이언트 복사.** `Cargo.toml`(스캐너 의존성 미러 + `dryoc`·`blake2` 추가), `build.rs`, `src/lib.rs`(모듈 선언 + 위의 두 mock 트레이트). lightwalletd 클라이언트는 **새로 짜지 않고 통째로 복사**한다(§5).

**Task 1 — ✅ 완료: 메모 코덱 (`memo.rs`, I1 소유).** `encode_memo(drop_id, &e_pub) -> Vec<u8>`(8B big-endian ‖ 32B)와 `decode_memo(&[u8]) -> Option<(u64,[u8;32])>`. **왜:** Zcash 메모 필드는 512바이트인데 우리는 앞 40바이트만 쓴다 — 나머지 0 패딩은 무시(`len() < 40`이면 `None`). 구매자 앱 B는 이 40바이트를 ZIP-321 URI의 `memo=`에 **base64url(패딩 없음)** 로 싣고, Zashi가 디코드해 체인에 raw로 넣는다. A1은 raw로 다시 읽는다. 일반 지갑이 raw memo를 못 넣는 경우를 위해 `A1B64:<base64url(raw40)>` 텍스트 fallback도 받는다.

**Task 2 — ✅ 완료: branch-tolerant tx 리더 (`detect.rs`).** `patch_v5_branch_to_nu5(raw)` + `read_tx_lenient(raw, net, height)`. **왜 (= C6, 함정 §8):** v5 트랜잭션은 바이트 `[8..12]`에 **consensus branch id**를 박아 넣는다. 우리 `zcash_primitives` 빌드보다 **새 네트워크 업그레이드(NU)** 로 만들어진 거래는 `Transaction::read`가 *"invalid consensus branch id"* 로 실패한다 — v5 바이트 레이아웃은 동일하고 branch id는 **노트 복호화와 무관**한데도. 그래서 정상 파싱이 실패하면 박힌 branch를 NU5(`0xC2D6_D0B4`)로 덮어쓰고 재시도한다. 스파이크 #2에서 실제로 터진 함정(`0x5437f330`)이며 probe가 이미 이렇게 우회한다.

**Task 3 — 🟡 부분 완료: IVK incoming 감지기 (`detect.rs`).** `detect_incoming(ufvk_str, raw_tx, net, height) -> Vec<IncomingNote{ value_zat, memo: Vec<u8> }>`. UFVK를 디코드해 **External/Internal scope IVK**를 Sapling·Orchard 둘 다 뽑고, full tx의 모든 shielded output(Sapling) / action(Orchard)을 **IVK trial-decryption** 한다 — Sapling은 `try_sapling_note_decryption`, Orchard는 `OrchardDomain::for_action` + `try_note_decryption`. 성공하면 `(note, addr, memo)`에서 **value와 memo를 둘 다 보존**한다. **왜 incoming(IVK)인가:** clean-wallet은 "내가 *보낸* 돈"을 OVK로 복원(outgoing)하고 메모를 버린다. 우리는 "*누가 크리에이터에게 냈나*"를 봐야 하므로 IVK incoming 경로이고 메모를 **지키는** 게 핵심(함정 §8). Sapling은 ZIP-212 enforcement를 Canopy 활성 여부로 `On`/`GracePeriod` 분기(probe·`scan.rs`와 동일).

**Task 4 — ✅ 완료: sealed-box dispatch wrap (`dispatch.rs`, I2 소유).** `wrap_k_drop(&k_drop, &e_pub) -> Vec<u8>`(= libsodium `crypto_box_seal`, `ek_pub(32) ‖ ct+MAC(48)` = 80B)와 `blob_key(ek_pub_prefix, &txid) -> String`(= `blake2b256(ek_pub ‖ txid)` hex). **왜:** 서버는 구매자의 일회용 공개키 `e_pub`로 `K_drop`을 봉인하므로 **구매자만**(`e_priv`로 `crypto_box_seal_open`) 푼다. 버킷 키에 `drop_id`나 구매자 식별자를 안 넣는 이유는 blob unlinkability(스펙 §5) — 키만 봐선 누가/무엇을 산 건지 모른다. 곡선은 Curve25519로 양쪽 동일(Rust `dryoc` ↔ JS `libsodium-wrappers`).

**Task 5 — ✅ 완료: replay guard (`engine.rs`).** `SeenTxids`(내부 `HashSet<[u8;32]>`), `first_time(&txid) -> bool`(처음이면 true, 재방문이면 false). **왜:** 같은 결제 tx를 두 번 보면 blob을 두 번 올리면 안 된다(스펙 §7.3 replay/nullifier 추적). **데모 한정 in-memory** — 프로덕션은 재시작 후 재-dispatch를 막으려 영속화해야 함(코드 주석으로 명시; Open-Q replay window).

**Task 6 — ✅ 완료: 엔진 (`engine.rs`).** `Engine::on_note(&Note{ drop_id, e_pub, value_zat, txid })`: ① `first_time`(replay 컷) → ② `cat.lookup(drop_id)`(없으면 무시) → ③ `value_zat < price_zat`면 무시(warn) → ④ `wrap_k_drop` → ⑤ `blob_key` → ⑥ `bucket.put`. txid에 대해 **멱등**. **왜:** 이게 "결제 → 검증 → 포장 → 게시"의 본체. mock Catalog/Bucket로 단위 테스트.

**Task 7 — ✅ 완료: 스캔 루프 (`scan_loop.rs`).** `scan_once(client, ufvk, net, start, end, &mut engine)`: `fetch_block_range`로 compact 블록 받음 → 각 `vtx.txid`마다 `fetch_transaction`(full tx) → `detect_incoming` → 각 노트마다 `decode_memo` → `engine.on_note`. (`run_loop`은 커서~tip을 주기적으로 `scan_once`.) **왜 full tx를 또 받나 (= 함정 §8):** **메모는 compact 블록에 없다** — compact는 ciphertext 앞 52바이트만 실어 *감지*만 가능, 메모는 full `enc_ciphertext`의 52..564 바이트에 있다. 그래서 `GetTransaction(txid)`가 필수(스펙 §4.3 ①②③; lightwalletd에 "어떤 tx를 보는지" metadata가 새는 건 데모에선 수용).

**Task 8 — ⬜ 미시작: 라이브 스모크 바이너리 (`src/bin/scan-live.rs`).** `<creator-ufvk> <start> <end>`를 받아 `GrpcClient::new("https://zec.rocks:443", None)` + 데모 드롭 하나짜리 in-memory Catalog + `put(key, len)`만 찍는 로깅 Bucket로 `scan_once` 한 번. 스파이크 #2의 실제 결제 블록을 훑어 `put(...)` 한 줄이 찍히면 통과(테스트 아닌 수동 E2E).


## 4.1 현재 검수 결과 / 단계 표기 (2026-06-21)

상태 기준: ✅ 완료 · 🟡 부분 완료 · ⬜ 미시작. 현재 구현은 가이드의 큰 방향과 맞고, 이제 **스캔 루프(Task 7)** 까지 구현됐다. 아직 **라이브 스모크(Task 8)** 와 운영용 encrypted cursor/replay state가 남아 있다.

| 단계 | 상태 | 검수 결과 | 근거 |
|---|---:|---|---|
| Task 0 — 크레이트 골격 + lightwalletd | ✅ | 크레이트, proto/build, lightwalletd client, 진단 CLI, `DropConfig`/`Catalog`/`Bucket` mock 경계까지 있음. | `indexer/Cargo.toml`, `build.rs`, `src/lightwalletd.rs`, `src/bin/check-lightwalletd.rs`, `src/lib.rs`. |
| Task 1 — memo codec(I1) | ✅ | 40B `drop_id || e_pub` raw 인코딩/디코딩과 `A1B64:` 텍스트 memo fallback 구현 및 단위 테스트 통과. | `src/memo.rs`; raw roundtrip, 짧은 입력 거부, 패딩 무시, `A1B64` roundtrip 테스트. |
| Task 2 — branch-tolerant tx reader | ✅ | v5 branch-id NU5 패치 및 lenient reader 구현, txid byte-order helper 포함. | `src/detect.rs`; `patches_unknown_branch_on_v5`, txid roundtrip 테스트. |
| Task 3 — IVK/UFVK incoming detector | 🟡 | Sapling/Orchard incoming decrypt 경로 구현. 현재 코드는 External뿐 아니라 Internal scope도 스캔해 실제 블록 `3363067` Orchard note를 발견한 상태. 다만 가이드의 hermetic golden fixture(`ae11a454…`)는 아직 없음. | `src/detect.rs`, `src/bin/probe-ufvk.rs`, `tests/live_chain_memo.rs`(ignored live test). |
| Extra — zecscope compact scan smoke | ✅ | compact block 기반 후보 탐색 CLI 구현. memo 확인용 본 경로는 아니고 빠른 후보 탐색용. | `src/zecscope_adapter.rs`, `src/bin/zecscope-scan.rs`. |
| Task 4 — sealed-box dispatch(I2) | ✅ | `K_drop` sealed-box 80B 생성, bucket key `blake2b256(ek_pub || txid)` 구현 및 buyer-open 테스트 통과. | `src/dispatch.rs`; `buyer_can_open_dispatch_blob`, `blob_key_is_blake2b_256_hex`. |
| Task 5 — replay guard | ✅ | `SeenTxids::first_time` 구현. 중복 txid는 재게시하지 않음. 단 데모 범위 in-memory라 프로덕션 영속화는 남음. | `src/engine.rs`; `rejects_duplicate_txid`, `duplicate_txid_does_not_republish`. |
| Task 6 — payment engine | ✅ | decoded note 기준 replay check → catalog lookup → amount check → sealed-box wrap → opaque key → `Bucket::put` 연결 완료. | `src/engine.rs`; valid/underpay/duplicate/unknown-drop tests. |
| Task 7 — scan loop | ✅ | `scan_once`가 lightwalletd block/tx fetch → detect → memo decode → engine dispatch를 연결. 단위 테스트는 compact→full fetch와 memo→engine dispatch wiring을 검증함. | `src/scan_loop.rs`; `scan_loop::tests::*`. |
| Task 8 — live smoke binary | ⬜ | 미구현. 실제 UFVK/range로 scan_once를 실행해 `put(...)`까지 확인하는 CLI 없음. | `src/bin/scan-live.rs` 없음. |

현재 검수 결론: **Task 0~7은 구현됐고, 전체 완성률은 약 65~70%**다. 다음 연결 작업은 `scan-live.rs`(Task 8)와 encrypted cursor/replay state다.

## 5. 재사용

**그대로 복사 (NEW 아님):**

- `week5/clean-wallet-mvp/apps/scanner/src/lightwalletd.rs` → `indexer/src/lightwalletd.rs`. `GrpcClient`(primary/backup failover) + `LightwalletdClient` 트레이트 + `#[cfg(test)] MockClient`(Task 7 테스트가 `raw_txs`로 full tx를 canned로 먹임)를 그대로 쓴다. 실제 gRPC `GetLatestBlock`/`GetBlockRange`/`GetTransaction`이 검증돼 있음.
- `week5/clean-wallet-mvp/apps/scanner/proto/*`(`service.proto`, `compact_formats.proto`) → `indexer/proto/`, 그리고 동일한 `build.rs`(`tonic_build`로 두 proto 컴파일, 모듈 `cash.z.wallet.sdk.rpc`).

**검증된 로직 이식 (probe → 프로덕션화):**

- `apps/scanner/src/bin/ivk-incoming-probe.rs`의 **IVK incoming 감지 + 메모 보존**(`UnifiedFullViewingKey::decode` → `to_ivk(External)` → Sapling `try_sapling_note_decryption` / Orchard `try_note_decryption`, ZIP-212 분기 포함)을 `detect.rs`로 옮긴다.
- 같은 파일의 **branch 패치 폴백**(`raw[0..4] == [0x05,0,0,0x80]`이면 `[8..12]`를 NU5로) → `patch_v5_branch_to_nu5` / `read_tx_lenient`.

**NEW 코드 (이 레인이 새로 짜는 것):** `memo.rs`(I1 코덱), `dispatch.rs`(`dryoc` sealed box + blake2b 키), `engine.rs`(검증+replay+wrap+publish), `scan_loop.rs`(루프 wiring), `lib.rs`의 두 mock 트레이트. probe는 *결과를 출력*만 했지, 검증·봉인·게시·replay는 없다 — 그게 전부 새 코드다.

## 6. 테스트하는 법

- **골든 픽스처 (hermetic):** 스파이크 #2의 **실제 mainnet tx `ae11a454…`** raw 바이트를 한 번 떠서 `indexer/tests/fixtures/spike12_tx.bin`에, 그 UFVK를 `spike12_ufvk.txt`에 커밋한다. `detect_incoming` 테스트는 이걸 `include_bytes!` 해서 **value 10_000 zatoshi(0.0001 ZEC)** 와 메모가 복원되는지 본다. 네트워크 의존 없이 항상 같은 결과 → CI 안전.
- **단위 테스트 (mock):** `memo` 라운드트립/길이 거부, `dispatch`(구매자 `e_priv`로 unseal 성공 + 80B 확인), `engine`(정가 결제 → blob 1개 / underpay → 0개, mock `Catalog`+`Bucket`), `scan_loop`(`MockClient`에 픽스처 먹여 compact→full→detect→memo→wrap→publish 전체 사슬).
- **라이브 스모크:** `cargo run -p drop-indexer --bin scan-live -- "<spike12_ufvk>" <h> <h>` 를 그 결제가 든 블록에 돌려 `put(...)` 한 줄 확인.

## 7. 완료 기준 (Definition of Done)

- [x] 크레이트 빌드/테스트 통과: `cargo test --manifest-path indexer/Cargo.toml` PASS. `Catalog`/`Bucket` mock 경계도 `lib.rs`에 있음.
- [x] `memo.rs`: raw 라운드트립 + 짧은 입력 거부 + 패딩 무시 + `A1B64:` 텍스트 fallback 테스트 통과 — raw 배치는 `interfaces.md` I1과 일치.
- [ ] `detect.rs`: branch-tolerant 파싱 테스트는 통과. **`ae11a454…` 골든 픽스처** 기반 value 10_000 & 메모 복원 테스트는 아직 없음.
- [x] `dispatch.rs`: sealed box 80B, 구매자 `e_priv`로 unseal 성공 — 배치가 `interfaces.md` I2와 일치.
- [x] `engine.rs`: 정가→blob 1개 / underpay→0개 / 중복 txid→재게시 안 함.
- [x] `scan_loop.rs`: `scan_once`가 compact→full fetch와 memo→engine dispatch wiring을 단위 테스트로 통과. 실제 raw-tx fixture E2E는 후속.
- [ ] `scan-live` 바이너리가 실제 스파이크 #2 결제에서 `put(...)`를 한 번 찍음 — 미구현.
- [ ] **레인 밖(올바름)**: secret-IN provisioning·attestation·catalog 영속화·bucket 구현은 A2/D 몫 — 여기선 mock.

## 8. 주의 / 함정

- **C6 — branch-tolerant 디코딩은 필수, 안 하면 조용히 결제를 다 놓친다.** 우리 librustzcash 빌드보다 새 mainnet NU로 만들어진 v5 tx는 `Transaction::read`가 실패한다. 이건 크래시가 아니라 **인덱서가 그 NU 이후 모든 결제를 소리 없이 못 보는 장애**다(스파이크 #2에서 실제 발생: branch `0x5437f330`). 반드시 `read_tx_lenient`로 박힌 branch를 NU5로 덮어쓰고 재파싱한다(branch id는 노트 복호화와 무관, tx 유효성은 lightwalletd를 신뢰). **이 레인이 C6의 오너** — 안 하면 다음 mainnet 업그레이드 때 인덱서가 깜깜해진다.
- **메모는 compact 블록에 없다.** compact는 ciphertext 앞 52바이트만 → 감지만 가능. 메모(우리 40B I1 페이로드)는 full `enc_ciphertext`의 52..564 바이트에 있으므로 `GetTransaction(txid)`로 **full tx를 따로 받아** 거기서 IVK 복호화해야 한다. compact만 복호화하면 메모가 빈/깨진 값으로 나온다.
- **incoming(IVK) 경로지, outgoing(OVK)가 아니다.** clean-wallet `scan.rs`는 OVK + `try_*_output_recovery`로 *보낸* 노트를 복원하고 `_memo`로 메모를 버린다. 여기서 그 코드를 복붙하면 안 된다 — `to_ivk(External)` + `try_sapling_note_decryption`/Orchard incoming을 쓰고 메모를 **지킨다**.
- **골든 픽스처의 메모 = 스파이크의 stand-in 문자열이지 진짜 I1 메모가 아니다.** 스파이크 #1/#2가 체인에 실은 메모는 `spike12|drop=1|epub=TESTKEY`(사람이 읽는 텍스트)로, I1의 동결 배치(`drop_id` u64 BE ‖ 32B `e_pub`)와 다르다. 그래서 `spike12_tx.bin`은 **"IVK가 메모를 그대로 복원한다"** 만 증명한다 — 그 바이트에 `decode_memo`를 돌리면 임의의 `(drop_id, e_pub)`로 갈릴 뿐 의미는 없다. **진짜 end-to-end 검증**(`decode_memo`가 의도한 `drop_id`/`e_pub`를 내놓는 것)은 B가 인코딩한 **진짜 40B I1 메모**를 실은 새 결제로 따로 해야 한다. Day-1 kickoff에서 B와 I1·I2 배치를 못 박은 뒤 그 결제를 만들어 두면 좋다.
- **Day-1에 I1·I2를 B와 동결.** 구매자 앱 B가 40B 메모를 base64url로 인코딩하고 80B blob을 `crypto_box_seal_open` 한다 — 한 바이트라도 어긋나면 붙일 때 깨진다. Task 1/4 들어가기 전 kickoff에서 `interfaces.md` 기준으로 확정.
- **replay guard는 데모 한정 in-memory** — 프로덕션은 재시작 시 재-dispatch를 막으려 영속화 필요(코드에 명시). `K_drop`은 forward secrecy 없음(스펙 §7.4) — 이 레인 범위는 아니지만 알아둘 것.
