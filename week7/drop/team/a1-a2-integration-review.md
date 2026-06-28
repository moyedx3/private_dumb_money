# A1/A2 통합 브랜치 코드 리뷰

- **브랜치:** `feat/a1-a2-integration` (HEAD `9647f49` — "Make A1/A2 cloud handoff operational")
- **리뷰 대상:** `master...feat/a1-a2-integration` 의 `week7/drop/indexer` 코드 (Cargo.lock·문서 제외)
- **리뷰 일자:** 2026-06-28
- **리뷰 방식:** 8개 관점(정확성/엣지케이스/보안/중복/효율 등)으로 각각 탐색 → 후보별 독립 검증 → 38개 후보 중 26개 생존 → 상위 10개 보고 (12개는 오탐으로 기각)

---

## 결론: PR 올리기 전에 수정 필요 🚧

Phala 테스트로 **해피 패스(provision → 결제 → blob 다운로드)는 정상 동작**하는 게 확인됐습니다. 다만 이번에 머지한 **A1→A2 스캐너 경로**에 **치명적 보안 회귀 1건**과 **결제 정확성 버그 묶음**이 있습니다. 이건 데모 한 번 깔끔하게 돌렸을 때는 안 보이고, **재시작 / 배치·분할 결제 / reorg / 멀티 테넌트** 상황에서 터집니다.

→ 최소한 아래 **🔴 Critical + 🟠 High** 항목은 고치고 PR 올리는 걸 권장합니다.

---

## 🔴 Critical — PR 전 반드시 수정

### 1. 호스트가 모든 creator의 콘텐츠 키를 탈취 가능 — `week7/drop/indexer/src/main.rs:67`

`provisioning_seed()` 가 이제 호스트 환경변수 `A2_DEV_PROVISIONING_SEED_HEX` 로 **TEE 파생 시드를 덮어쓸 수 있게** 바뀌었습니다. `master` 에서는 시드가 오직 `ds.get_key("drop/provisioning")` 에서만 나왔습니다 — 즉 enclave 안에서만 존재했습니다.

**왜 치명적인가:** TEE의 핵심은 "호스트 운영자가 절대 볼 수 없는 private key를 enclave가 쥐고 있고, creator가 자기 콘텐츠 마스터키(`k_drop`)를 그 public key 앞으로 seal한다"는 것입니다. 이 변경은 그 자물쇠의 여벌 키를 만들어 현관 매트 밑에 두는 격입니다.

- 실제 배포 환경에 이 env var가 설정돼 있으면 (로컬 데모 잔재이거나, 호기심/악의를 가진 호스트가 주입), provisioning X25519 secret을 **호스트가 알게 됩니다**.
- `attest.rs` 는 `from_secret_key` 로 키페어를 만들기 때문에 **시드 = private key** 입니다.
- 그런데 `/attest` 는 여전히 그 pubkey에 대한 **진짜 TDX quote**를 발급합니다. → creator는 정상 attestation을 검증하고 `k_drop`을 seal하지만, 호스트는 그걸 그대로 복호화해서 **모든 creator의 콘텐츠 마스터키를 획득**합니다.

지금은 `eprintln` 경고 한 줄이 전부고, 실제 차단 로직이 없습니다.

**제안:** dev 전용 feature flag / `#[cfg(debug_assertions)]` 뒤로 숨기거나, 실제 dstack 소켓이 존재하는데 이 env var가 같이 설정돼 있으면 부팅을 거부하도록 가드 추가.

---

## 🟠 High — 결제 정확성 (구매자가 돈 내고 콘텐츠 못 받음)

### 2. 멀티 output 트랜잭션에서 첫 note 빼고 다 유실 — `engine.rs:80`

replay guard가 **순수 txid 기준**이고, 카탈로그/가격 체크보다 **먼저** 실행됩니다. 한 트랜잭션에 memo가 2개(드롭 2개 구매)면 첫 번째만 dispatch됩니다. 더 나쁜 케이스: unknown-drop이나 underpaid note가 먼저 정렬되면 그게 txid 슬롯을 차지해버리고, 진짜 결제된 note는 `None`을 반환 → **결제했는데 영원히 안 열림**.
→ guard 키를 `(txid, output_index)` 또는 `(txid, drop_id)` 로.

### 3. enclave 재시작 때마다 스캔 상태 전부 소실 — `scan_loop.rs:338`

`run_catalog_loop` 가 매 프로세스 시작마다 새 in-memory `HashMap` 을 만들고, **같은 브랜치에 들어있는 `EncryptedFileScanState` 영속화를 전혀 연결하지 않습니다.** 재시작/재배포가 일어나면 커서가 리셋되고 `last_scanned_height()` 가 `None` → 시작 높이가 **현재 체인 tip**으로 기본값 설정됨 → 스캐너가 죽어있던 동안 들어온 결제는 **영구히 스킵**.
→ 고칠 코드(`state.rs`)가 이미 diff 안에 있습니다. **연결만 안 됨.**

### 4. 늦게 provision된 결제를 커서가 건너뜀 — `scan_loop.rs:198`

드롭이 아직 카탈로그에 없어서 `on_note` 가 `None` 을 반환해도 블록 커서가 `max_height` 로 **무조건 전진**합니다. creator가 드롭을 provision하기 **전에** mined된 결제는 다시 스캔되지 않음 → 구매 유실, 복구 경로 없음.

### 5. confirmation depth 없음 → reorg로 키가 공짜로 풀림 — `engine.rs:79`

스캐너가 체인 tip까지 읽고 **0 confirmation**으로 dispatch합니다. tip 블록이 reorg로 빠지면 결제는 무효가 되는데 `K_drop` blob은 이미 publish됨 → 구매자는 사라진 결제에 대한 복호화 키를 그대로 보유. (게다가 커서는 reorg된 높이를 다시 안 봅니다.)
→ confirmation depth 게이트 추가 필요.

### 6. 분할 결제 거부 — `engine.rs:92`

가격 체크가 **note 1개씩** 전체 가격과 비교합니다. 여러 note로 나눠 보낸 결제는 합계가 충분해도 매 note가 "underpaid" 처리됨.
→ `(txid/drop)` 단위 합산 필요.

---

## 🟠 High — 가용성 (멀티 테넌트 전체에 영향)

### 7. UFVK 하나 잘못되면 전체 dispatch 정지 — `scan_loop.rs:317`

`scan_catalog_once` 가 UFVK 파싱에 `?` 를 쓰고, provision 시 deposit 주소만 검증하고 UFVK는 검증하지 않습니다. creator 한 명이 malformed/오타 `creator_ufvk` (혹은 테스트 placeholder `uview1secret`)로 드롭을 등록하면 **전체 스캔 패스가 플랫폼 전역으로 abort** → 그 드롭을 지울 때까지 모두 멈춤.
→ 드롭별 스캔을 감싸서 한 creator 오류가 격리되게.

---

## 🟡 Medium

### 8. blob_key가 비결정적 → `/dispatch` 무한 증가 — `engine.rs:103`

`blob_key` 가 sealed box의 **랜덤** ephemeral pubkey에서 파생됩니다. 재시작 후 같은 결제를 다시 dispatch하면 **다른 bucket 키**로 덮어쓰기 대신 새 blob을 씁니다. #3(재시작 시 과거 블록 재스캔)와 합쳐지면, 재시작마다 모든 과거 결제에 대해 새 blob을 publish → dispatch 목록과 구매자 polling 비용이 무한 증가. ("idempotent on txid" 문서 주장과 모순.)
→ 키를 안정적인 값에서 파생 (예: `blake2b(txid || drop_id)`).

---

## 🟡 Lower — 하드닝 ("plausible"로 분류, 한 번 보면 좋음)

### 9. memo 오파싱 — `memo.rs:49`
`decode_raw_memo` 가 magic/구조 체크 없이 **40바이트 이상이면 무조건** `drop_id||e_pub` 로 해석. 앞 8바이트가 우연히 live drop_id와 겹치는 무관한 memo가 dispatch 목록을 오염시키고 그 txid의 replay 슬롯을 소모.
→ version/magic prefix 추가.

### 10. 비밀키가 `Debug` 출력 가능해짐 — `lib.rs:27`
`DropConfig`(32바이트 `k_drop` 보유)에 `#[derive(Debug)]` 가 추가됨. `master` 는 콘텐츠 마스터키가 `{:?}` 로 로깅되지 못하도록 일부러 뺐었습니다. 이제 trace/eprintln 한 줄이 호스트 가시 로그로 키를 흘릴 수 있음.
→ `Debug` derive 제거 또는 redaction 구현.

---

## 검토 후 제외된 항목 (오탐 12건)

대부분 런타임 영향 없는 DRY/스타일 지적(중복 base64url 코덱, 진단 바이너리들의 copy-paste된 `load_dotenv`, 구조 동일한 struct 중복 등)이라 기각했습니다. 좀 더 그럴듯했던 2건도 조사 후 **버그 아님**으로 확인:

- "plaintext backup 엔드포인트에 TLS 강제 → failover 안 됨" → **오탐**
- "A1 스캐너가 기본적으로 OFF" → **오탐**

즉 노이즈는 걸러진 상태이고, 위 10개가 실제 시그널입니다.

---

## 권장 조치 순서

**블로커 (PR 전 필수):**
- [ ] 1. `main.rs:67` — provisioning 시드 env override 가드 (보안 회귀)
- [ ] 3. `scan_loop.rs:338` — `EncryptedFileScanState` 영속화 연결 (이미 만들어진 코드)
- [ ] 2. `engine.rs:80` — replay guard 키를 `(txid, drop_id)` 로
- [ ] 7. `scan_loop.rs:317` — 드롭별 UFVK 오류 격리
- [ ] 5. `engine.rs:79` — confirmation depth 게이트
- [ ] 6. `engine.rs:92` — 분할 결제 합산
- [ ] 4. `scan_loop.rs:198` — 미provision 결제에 대해 커서 전진 보류

**후속 (PR 후 가능):**
- [ ] 8. `engine.rs:103` — 결정적 blob_key
- [ ] 9. `memo.rs:49` — memo magic prefix
- [ ] 10. `lib.rs:27` — `Debug` derive 제거

> 참고: #3과 #8, #5가 서로 얽혀 있습니다. #3(상태 영속화)을 먼저 고치면 #8(재시작 재publish)의 트리거가 상당 부분 사라집니다.
