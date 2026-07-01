# A1-A2 병합 이후 가능해진 것과 파트별 작업 요청

> 상태: `feat/a1-a2-integration` 기준.
> 목적: 기존 `a1-a2-integration-conflicts.md`의 “병합 전 충돌 예측” 문서를 제거하고,
> **병합 후 실제로 가능해진 기능**과 **영향받는 A1/A2/B/C 파트의 후속 작업**을 정리한다.
>
> 이 문서는 Lane D를 별도 파트로 다루지 않는다. 현재 bucket/storage 역할은 독립 Lane D가 아니라
> **A2 indexer 내부 `FsBucket`과 HTTP route**가 맡는다.

---

## 1. A1-A2를 병합하면 무엇이 가능해지나

A1과 A2가 한 런타임 안에서 연결되면서, 이제 서버는 단순 mock이 아니라 아래 흐름을 실제로 수행할 수 있다.

1. **C가 TEE를 검증하고 비밀을 넣는다.**
   - `GET /attest`로 quote와 provisioning public key를 받는다.
   - C는 `K_drop`, `creator_ufvk`, `deposit_addr`, `h_content`를 enclave public key로 sealed-box 암호화한다.
   - `POST /provision`으로 A2 내부 catalog에 등록한다.

2. **A1이 실제 Zcash 결제를 감지한다.**
   - provision된 UFVK를 사용해 lightwalletd에서 shielded payment를 스캔한다.
   - memo에서 `drop_id || e_pub` 또는 `A1B64:` 텍스트 폴백을 디코드한다.
   - 금액이 catalog price 이상인지 확인한다.

3. **A1이 구매자 전용 unlock blob을 만든다.**
   - `K_drop`을 buyer의 `e_pub`으로 `crypto_box_seal`한다.
   - 80-byte dispatch blob을 A2 dispatch bucket에 저장한다.

4. **B가 서버에서 unlock 후보를 가져올 수 있다.**
   - `GET /dispatch`로 dispatch key 목록을 받는다.
   - `GET /dispatch/:key`로 80-byte blob을 받는다.
   - 자신의 `e_priv`로 열리는 blob만 골라 `K_drop`을 복원한다.

즉 병합 후 가능한 핵심은 다음 한 줄이다.

```text
TEE provision + real Zcash memo payment scan + buyer-specific K_drop dispatch
```

---

## 2. 현재 동작 시나리오

```text
[C: Creator]
  │
  │ 1. GET /attest
  │    - TDX quote
  │    - provisioning_pubkey_hex
  ▼
[A2: TEE HTTP/runtime]
  │
  │ 2. C verifies quote, then POST /provision with sealed payload
  │    payload = { drop_id, price_zat, k_drop, creator_ufvk, h_content, deposit_addr }
  ▼
[A2: CatalogStore]
  │
  │ 3. provisioned DropConfig becomes readable by A1 inside server
  ▼
[A1: Scanner + Engine]
  │
  │ 4. scan lightwalletd using creator_ufvk
  │ 5. read payment memo = drop_id || e_pub
  │ 6. verify payment amount >= price_zat
  │ 7. dispatch_blob = crypto_box_seal(k_drop, e_pub)
  ▼
[A2: Dispatch FsBucket]
  │
  │ 8. GET /dispatch -> [bucket_key]
  │ 9. GET /dispatch/:bucket_key -> 80-byte sealed blob
  ▼
[B: Buyer]

[B: Buyer]
  │
  │ 10. crypto_box_seal_open(blob, e_pub, e_priv)
  │ 11. recovered K_drop decrypts content blob from GET /bucket/:h_content
  ▼
[Unlocked content]
```

현재 클라우드 smoke 기준으로 확인된 것:

```text
GET /health        -> 200 ok
GET /attest        -> quote_hex + provisioning_pubkey_hex
POST /provision    -> 200, sealed payload accepted
GET /catalog       -> public entry, no k_drop/creator_ufvk leak
GET /dispatch      -> dispatch key list
GET /dispatch/:key -> 80-byte sealed blob
PUT/GET /bucket    -> content blob roundtrip
```

---

## 3. 현재 API 책임 경계

| 파트 | 현재 책임 | 실제 route / 모듈 |
| --- | --- | --- |
| A1 | payment scan, memo decode, amount check, dispatch sealing | `scan_loop`, `detect`, `memo`, `engine`, `dispatch` |
| A2 | TEE attestation, sealed provision, catalog, HTTP routes, current bucket implementation | `/attest`, `/provision`, `/catalog`, `/bucket/:key`, `/dispatch`, `/dispatch/:key` |
| B | buyer keypair, payment memo, dispatch polling/open, content decrypt | buyer app |
| C | content encrypt/upload, attestation verification, sealed provision | creator app |

주의: 현재 “bucket/storage”는 독립 파트가 아니다. 아래 route들은 A2 안에 있다.

```text
PUT /bucket/:key
GET /bucket/:key
GET /dispatch
GET /dispatch/:key
```

---

## 4. 파트별 작업 요청

### A1 요청 — scanner/engine 완성도 확인

**목표:** A1의 고유 역할인 “결제 감지 → memo decode → dispatch 생성”을 A2 런타임 안에서 안정적으로 유지한다.

요청사항:

1. **fresh live payment 검증**
   - 이미 채굴된 과거 payment replay가 아니라, provision 이후 새 결제를 보내고 자동 dispatch 생성까지 확인한다.
   - 완료 기준: 새 tx block 이후 `/dispatch`에 새 80-byte blob이 추가된다.

2. **scan cursor / replay 정책 명확화**
   - 현재 운영은 `A1_SCAN_START`, batch size, poll interval에 의존한다.
   - 재시작 시 같은 범위를 다시 스캔해도 중복 dispatch가 생기지 않는지 확인한다.
   - production 전에는 cursor와 seen-txid 상태를 영속화할지 결정한다.

3. **lightwalletd 장애/백업 동작 확인**
   - primary 실패 시 `LIGHTWALLETD_BACKUP_URL`로 fallback 되는지 확인한다.
   - 실패 로그가 UFVK, memo, K_drop 같은 secret을 노출하지 않아야 한다.

4. **memo 형식 고정 유지**
   - raw 40B와 `A1B64:` 텍스트 폴백 둘 다 유지한다.
   - B와 같은 test vector를 공유한다.

완료 기준:

```text
provisioned drop 존재
→ live payment mined
→ A1 scanner detects memo
→ dispatch blob created
→ blob length = 80 bytes
```

---

### A2 요청 — TEE/runtime/API 경계 고정

**목표:** A2의 고유 역할인 “검증 가능한 TEE 입구 + secret-in + 공개 API”를 보존한다.

요청사항:

1. **Phala 배포 설정 확정**
   - Docker build는 `build.rs`와 `proto/`를 포함해야 한다.
   - Phala는 `linux/amd64` 이미지를 pull하므로 amd64 image push가 필요하다.
   - compose에는 A1 scanner env가 명시되어야 한다.

2. **`/provision` 계약 문서화**
   - `GET /provision`은 405가 정상이다.
   - 실제 등록은 `POST /provision` + sealed binary body만 허용한다.

3. **catalog 저장 정책 결정**
   - 현재 catalog는 in-memory다.
   - CVM 재시작/재배포 후에는 re-provision이 필요하다.
   - production 전에는 영속화 또는 명시적 re-provision 운영 절차를 결정한다.

4. **secret 비노출 보장 유지**
   - `/catalog`에는 `k_drop`, `creator_ufvk`가 절대 나오면 안 된다.
   - logs에도 UFVK, K_drop, sealed plaintext가 나오면 안 된다.

5. **현재 bucket 역할 명확화**
   - 지금은 A2 내부 `FsBucket`이 content/dispatch 저장을 맡는다.
   - 별도 storage service가 생기기 전까지 B/C는 A2 route를 기준으로 연동한다.

완료 기준:

```text
GET /attest        returns valid quote/pubkey
POST /provision    stores internal DropConfig
GET /catalog       exposes public fields only
GET /dispatch/:key serves 80-byte blob
PUT/GET /bucket    roundtrips content blob
```

---

### B 요청 — 실제 dispatch unlock 경로 연결

**목표:** mock dispatch가 아니라 A1-A2 서버가 만든 실제 dispatch blob으로 `K_drop`을 복원한다.

요청사항:

1. **catalog 기반 구매 흐름 연결**
   - `GET /catalog`에서 `drop_id`, `price_zec`, `h_content`, `deposit_addr`를 읽는다.
   - `deposit_addr`는 shielded address여야 하며, transparent address면 결제를 막는다.

2. **구매별 keypair 생성/보관**
   - 구매 1건마다 `(e_priv, e_pub)`를 새로 만든다.
   - `e_pub`은 payment memo에 넣는다.
   - `e_priv`는 unlock 전까지 보관해야 한다. 잃으면 해당 구매는 복구 불가다.

3. **memo encoder 확정**
   - raw 형식: `drop_id(8B BE) || e_pub(32B)`
   - wallet raw memo가 불안하면 텍스트 폴백: `A1B64:<base64url_no_pad(raw40)>`
   - ZIP-321 `memo=` wrapping까지 A1 test vector와 맞춘다.

4. **dispatch polling + trial-open 구현**
   - `GET /dispatch`로 key 배열을 받는다.
   - 각 key에 대해 `GET /dispatch/:key`로 80-byte blob을 받는다.
   - 보관 중인 `(e_pub, e_priv)`로 `crypto_box_seal_open`을 시도한다.
   - 열리는 blob 하나가 해당 구매의 `K_drop`이다.

5. **content decrypt 연결**
   - `GET /bucket/:h_content`로 encrypted content blob을 받는다.
   - 복원한 `K_drop`으로 AES-GCM decrypt한다.

완료 기준:

```text
B generates e_pub/e_priv
→ sends payment memo with e_pub
→ downloads dispatch blob
→ opens blob with e_priv
→ recovered K_drop decrypts content
```

---

### C 요청 — 실제 provision UI 정합성 맞추기

**목표:** 일회성 raw script 없이 creator UI만으로 A2 `/provision`에 drop을 등록한다.

요청사항:

1. **provision payload에 `deposit_addr` 추가**
   - 현재 A2는 provision payload에 `deposit_addr`를 요구한다.
   - C UI/schema/type/test 모두 아래 shape에 맞춰야 한다.

   ```json
   {
     "drop_id": 1,
     "price_zat": 10000,
     "k_drop": "<64 hex>",
     "creator_ufvk": "<UFVK>",
     "h_content": "<64 hex>",
     "deposit_addr": "<shielded address>"
   }
   ```

2. **attestation 검증 후에만 seal**
   - `GET /attest`에서 quote와 `provisioning_pubkey_hex`를 받는다.
   - quote의 `report_data`가 provisioning pubkey hash에 묶여 있는지 확인한다.
   - 측정값 검증 실패 시 provision을 중단한다.

3. **content upload 순서 고정**
   - content를 `K_drop`으로 AES-GCM 암호화한다.
   - `h_content = sha256(blob)`를 계산한다.
   - `PUT /bucket/:h_content`로 업로드한다.
   - 그 뒤 `h_content`를 provision payload에 포함한다.

4. **재배포 후 re-provision 안내**
   - A2 provisioning key는 dstack/KMS measurement에 묶인다.
   - 이미지가 바뀌면 pubkey도 바뀔 수 있으므로, redeploy 후 기존 creator는 re-provision해야 한다.

5. **secret logging 금지**
   - UI console/log/test artifact에 `k_drop`, `creator_ufvk`가 남지 않게 한다.

완료 기준:

```text
C UI encrypts content
→ uploads content blob
→ verifies /attest
→ seals payload including deposit_addr
→ POST /provision returns 200
→ GET /catalog shows public entry
```

---

## 5. 이번 병합으로 영향받아 아직 막힐 수 있는 항목

| 영향 항목 | 현재 상태 | 필요한 파트 |
| --- | --- | --- |
| Creator UI live provision | A2는 `deposit_addr`를 요구하지만 C payload에는 아직 반영 필요 | C |
| Buyer live unlock | dispatch blob은 존재하지만, B가 보관한 `e_priv`로 열어야 최종 확인 가능 | B |
| catalog 재시작 내구성 | in-memory라 재시작 시 re-provision 필요 | A2 |
| scanner 재시작 내구성 | scan start/cursor/replay 정책을 운영 기준으로 고정해야 함 | A1/A2 |
| Phala 배포 아키텍처 | Phala는 linux/amd64 image 필요 | A2 |

---

## 6. 병합 전 최종 체크리스트

```text
[ ] A1: fresh live payment -> dispatch 생성 확인
[ ] A2: Phala deploy compose/image 설정 확정
[ ] B : real dispatch blob을 e_priv로 열어 K_drop 복원
[ ] C : UI에서 deposit_addr 포함 sealed /provision 성공
[ ] 공통: /catalog에 secret 비노출 확인
[ ] 공통: buyer/creator/indexer test + build 통과
```

현재까지 확인된 서버 측 사실:

```text
A1-A2 cloud smoke: 통과
A2 API smoke: 통과
indexer tests: 통과
buyer tests/build: 통과
creator tests/build: 통과
```

다만 B/C의 “실제 UI 기반 end-to-end”는 위 요청사항을 반영한 뒤 다시 확인해야 한다.
