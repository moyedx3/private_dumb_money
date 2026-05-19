# Latch — 시스템 사양

- **상태**: V1 사양.
- **Hard requirements** (위반 시 설계가 무효):
  1. Buyer는 결제 없이 `D`를 추출할 수 없다.
  2. Seller는 listing 후 `D`를 변경할 수 없다.
  3. 어떤 프로토콜 단계도 어느 쪽의 장기 Zcash spending key를 요구하지 않는다 — per-trade Orchard 키만.
  4. Escrow 컨트랙트는 문서화된 verdict-resolution path 외에는 자금을 풀지 않는다.

---

## 1. Goals and non-goals

### Goals

1. **Buyer는 shielded ZEC로 결제.** Sender 익명성은 Zcash Orchard pool에서 제공.
2. **Seller는 listing 시점에 `D`에 commit.** On-chain `H_D`와 `metadata_hash`는 listing 후 immutable; AAD-bound AEAD가 사후 substitution을 탐지.
3. **양측 모두 per-trade Orchard 키.** 장기 wallet은 절대 안 건드림. Challenge가 노출하는 건 분쟁 거래의 per-trade `IVK_b`뿐, 그 외엔 아무것도 아님.
4. **Bonded slashing 있는 federated verifier 집합**이 분쟁을 해결. Permissionless fraud-proof window가 corrupted verdict를 누구든 뒤집고 verifier bond를 몰수할 수 있게 함.
5. **Permissionless settlement** — verdict + window 만료 후. 누구든 `settle_trade` 호출 가능; code path는 입력이 주어지면 deterministic.

### Non-goals

- **Subscription 또는 반복 access.** 일회성 디지털 재화 구매만.
- **DRM 또는 piracy 방지.** 결제한 Buyer가 `K`와 `E_K(D)`를 재배포 가능.
- **Network-layer privacy** (Zcash 브로드캐스트와 NEAR escrow 호출 사이의 IP 상관). 프로토콜 범위 밖.
- **Custody-free cross-chain 결제.** ZEC ↔ NEAR custody는 federated PoA bridge가 담당; § 7에서 bridge가 무엇을 보관하는지 상세히.
- **Catalog 모더레이션, KYC, 분쟁 항소.** Protocol layer에서 enforce 안 함.

---

## 2. High-level architecture

```
   ┌──────────────────────────────────────────────────────────────────────┐
   │                              SELLER                                  │
   │  - per-listing K 생성 (AES-256, OS CSPRNG)                            │
   │  - H_D = BLAKE2b-256(D); metadata_hash = BLAKE2b-256(JSON) 계산       │
   │  - AAD = metadata_hash로 E_K(D) 암호화; IPFS 업로드                    │
   │  - buyer가 나타나면 per-trade OVK_s 생성                              │
   │  - Zcash z2z note memo (ZIP-302 V1 layout) 안에 K 담아 전달            │
   └────────┬───────────────────────────────────────────────────┬─────────┘
            │ create_listing / register_delivery / submit_delivery │
            │ (Zcash 측: UA_b로 z2z)                              │
            ▼                                                       ▼
   ┌──────────────────────────────────────┐               ┌──────────────┐
   │      NEAR ESCROW CONTRACT             │               │  ZCASH       │
   │  - listing, reservation, trade        │               │  ORCHARD     │
   │  - challenge, verifier set, bond      │               │  POOL        │
   │  - state 변경마다 1개 event 발생       │               │              │
   │  - 11 external entry point            │               │  - delivery  │
   │  - 모든 state transition을 on-chain    │               │    note + cm │
   │    enforce                            │               │  - K(32 B)를 │
   └──┬───────────────────────┬───────────┘               │    담은 512  │
      ▲                       ▲                            │    byte memo │
      │ finalize_payment      │ submit_attestation         └────┬─────────┘
      │ (bridge attestor)     │ (등록된 verifier)                │
      │                       │                                   │
   ┌──┴────────────────────┐  │  ┌────────────────────────────────┴──────┐
   │  PoA BRIDGE            │  │  │             BUYER                     │
   │  - 5–7 validator       │  │  │  - per-trade Orchard 키 생성           │
   │  - MPC threshold key   │  │  │    (UA_b, IVK_b, buyer_ivk_commit)    │
   │  - Buyer의 shielded     │  │  │  - listing reserve, bridge transparent │
   │    결제를 받는 Zcash    │  │  │    주소로 z2t 결제                     │
   │    transparent 주소를   │  │  │  - IVK_b로 memo 복호 → K 추출         │
   │    custody              │  │  │  - E_K(D) fetch, K로 복호,            │
   │  - 그 deposit과 같은    │  │  │    H(D') = H_D 검증                   │
   │    wrapped 토큰을        │  │  │  - 불일치 시: challenge file          │
   │    NEAR에 mint          │  │  │    (IVK_b on-chain reveal)            │
   └────────────────────────┘  │  └───────────────────────────────────────┘
                               │
                          ┌────┴────────────────────┐
                          │  VERIFIER SET            │
                          │  - 3–5 federated 데몬    │
                          │  - on-chain 입력으로만   │
                          │    predicate 재실행      │
                          │  - bonded; verifier      │
                          │    verdict에 대한 성공한 │
                          │    fraud proof 시 bond   │
                          │    몰수                  │
                          └─────────────────────────┘

                              ┌──────────────────────────────┐
                              │   IPFS                       │
                              │  - CID로 E_K(D) 호스팅       │
                              │  - ciphertext는 public,      │
                              │    K 없이는 무용지물          │
                              └──────────────────────────────┘
```

---

## 3. Components

| # | 컴포넌트 | 호스팅 | 보유 데이터 |
|---|---|---|---|
| 1 | **Seller 로컬 도구** | Seller 머신 | `D`, listing마다 fresh `K`, per-trade `OVK_s`, 전체 Zcash wallet |
| 2 | **Buyer 로컬 도구** | Buyer 머신 | per-trade Orchard 키쌍 (`SK_b`, `UFVK_b`, `IVK_b`, `UA_b`), 메모리 only |
| 3 | **Escrow 컨트랙트** | NEAR (배포당 한 계정) | listings, reservations, trades, challenges, verifier set, verifier bonds |
| 4 | **IPFS** | 운영자의 Kubo 노드 (public read) | `E_K(D)` blob만 |
| 5 | **PoA Bridge** | 5–7 validator 인프라 + MPC threshold key | Buyer가 lock한 ZEC를 transparent Zcash 주소에 보관; 그 주소의 spending key; NEAR의 wrapped-token mint 권한 |
| 6 | **Verifier 데몬** | 3–5 식별 가능한 운영자 | per-verifier signing key; escrow 컨트랙트에 lock된 verifier bond; revealed `IVK_b` / fetched delivery memo / IPFS blob에 대한 일시 접근 |

Escrow 컨트랙트는 다음 external entry point를 노출. Caller 인증은 on-chain에서 enforce.

| Entry point | Caller | 목적 |
|---|---|---|
| `create_listing` | seller | listing 게시, `seller_collateral` 첨부 |
| `cancel_listing` | seller | reserve되지 않은 listing 내림 |
| `reserve_listing` | buyer | `UA_b`와 `buyer_ivk_commit`를 특정 listing에 묶음 |
| `finalize_payment` | bridge attestor | bridge가 결제 확인 시 reserved listing을 `PaymentLocked` trade로 전환 |
| `register_delivery` | seller | Seller의 per-trade `OVK_s` commit |
| `submit_delivery` | seller | `delivery_cm` (Zcash note commitment) publish; `T_challenge` 시작 |
| `file_challenge` | buyer | 분쟁 개시, `IVK_b` reveal, `buyer_challenge_collateral` 첨부 |
| `submit_attestation` | 등록된 verifier | verdict 기록 (`Honest` / `Fraud` / `Inconclusive`) |
| `submit_fraud_proof` | anyone | `T_fraud_proof` 안에 resolved verdict 무효화, corrupt verifier bond 몰수 |
| `settle_trade` | anyone | 최종 verdict에 따라 자금 분배; permissionless |
| `register_verifier_bond` | anyone | verifier 자격을 위해 bond 첨부 |

---

## 4. End-to-end 흐름

### 4.1 Listing

Seller는 체인과 완전히 offline 상태로:

1. OS CSPRNG에서 `K` 생성 (32 bytes).
2. `H_D = BLAKE2b-256(D)`와 `metadata_hash = BLAKE2b-256(metadata_json)` 계산.
3. `E_K(D) = AES-256-GCM.encrypt(K, D, AAD = metadata_hash)` 암호화. Blob은 `nonce(12) ‖ ciphertext(n) ‖ tag(16)`.
4. `E_K(D)`를 IPFS에 업로드, `CID` 수령.
5. `create_listing { H_D, CID, metadata_hash, price, lifetime_ns }` 호출, `seller_collateral = price` 첨부.

이후 `Listing.status = Active`. On-chain 레코드는 `(H_D, CID, metadata_hash, price, lifetime_ns)`만 노출하고 `D`나 `K`에 대해서는 아무것도 노출하지 않음.

### 4.2 Reserve와 payment

```
 BUYER (로컬)          BRIDGE                  NEAR ESCROW
       │                  │                          │
       │ per-trade Orchard 키 생성                    │
       │ → SK_b, UFVK_b, IVK_b, UA_b                  │
       │ → buyer_ivk_commit = H(IVK_b)                │
       │                                              │
       │ reserve_listing { UA_b, buyer_ivk_commit,    │
       │                   expected_payment }         │
       │ ─────────────────────────────────────────────▶
       │                                              │
       │                              Listing → Reserved
       │                              Reservation pending (T_reservation)
       │                                              │
       │ bridge transparent 주소로                    │
       │ z2t shielded payment (amount = price)        │
       │ ──────────────────────────▶                  │
       │                  │                           │
       │     validator quorum이 deposit 감지,         │
       │     MPC threshold로 attestation 서명         │
       │                  │                           │
       │                  │ finalize_payment {        │
       │                  │   reservation_id,          │
       │                  │   bridge_attestation }     │
       │                  ─────────────────────────────▶
       │                                              │
       │                              wrapped 토큰 mint
       │                              Trade 생성
       │                              status = PaymentLocked
       │                              T_key_delivery 시작 (24h)
```

`UA_b`는 fresh per-trade Unified Address. 4.3에서 Seller의 z2z delivery note의 수신지로 사용. Zcash diversifier 설계로 `UA_b`는 Buyer의 장기 wallet과 unlinkable.

### 4.3 Delivery

```
 SELLER                ZCASH CHAIN               NEAR ESCROW
       │                   │                           │
       │ register_delivery { OVK_s commitment }        │
       │ ──────────────────────────────────────────────▶
       │                                               │
       │ 512-byte memo 구성:                            │
       │   byte 0       = 0xF5         (ZIP-302 binary marker)
       │   bytes 1..33  = K            (32-byte AES key)
       │   byte 33      = 0x01         (Latch V1 version)
       │   bytes 34..512 = 0x00 …      (zero padding, 엄격 검증)
       │                                               │
       │ z2z 트랜잭션:                                 │
       │   from: OVK_s에서 derive된 주소               │
       │   to:   UA_b                                  │
       │   memo: 위 512 bytes                          │
       │                                               │
       │ ── shielded note ──▶│                         │
       │                     │ note는 UA_b 키로 암호화 │
       │                     │ delivery_cm이 Zcash에 발생
       │                                               │
       │ submit_delivery { delivery_cm,                │
       │                   seller_key_commit }         │
       │ ──────────────────────────────────────────────▶
       │                                               │
       │                              Trade → Delivered
       │                              T_challenge 시작 (48h)
```

`delivery_cm`이 cross-chain binding evidence: Zcash에서 delivery note의 note commitment로 존재하고, `submit_delivery`로 NEAR에 등록. Verifier가 나중에 둘이 같은 note인지 확인.

### 4.4 Buyer 검증 (offline)

체인을 건드리지 않고:

1. Buyer가 per-trade `IVK_b`로 Zcash 체인을 스캔, delivery note를 찾고 복호.
2. Memo bytes `1..33`에서 `K` 추출. Marker `0xF5`, version `0x01`, byte 33 이후 zero padding 모두 엄격하게 검증; 어느 deviation도 fraud로 처리.
3. Buyer가 `CID`로 IPFS에서 `E_K(D)` fetch.
4. `D' = AES-256-GCM.decrypt(K, E_K(D), AAD = metadata_hash)` 복호. 틀린 `K`, tampered ciphertext, 틀린 AAD는 GCM tag 검증 실패.
5. `BLAKE2b-256(D')`를 listing의 `H_D`와 비교. 일치 → honest delivery.

어느 단계가 실패하면 Buyer의 옵션은 § 4.6 (challenge).

### 4.5 Happy settlement

`T_challenge` (48h)가 `file_challenge` 없이 만료되면 누구나 `settle_trade` 호출 가능:

- Seller가 `price + seller_collateral` 수령.
- Trade가 `Settled`로 이동; listing이 `Completed`로 이동.

Settlement caller는 permissionless. Window 만료 후엔 누구도 막을 수 없음.

### 4.6 Challenge와 resolution

```
 BUYER                 NEAR ESCROW          VERIFIER            NEAR ESCROW
       │                   │                  │                       │
       │ file_challenge { revealed_ivk_b, reason }                    │
       │ + buyer_challenge_collateral (price의 50%)                   │
       │ ─────────────────────────────────────────────────────────────▶
       │                                                              │
       │ 컨트랙트 검증: H(revealed_ivk_b) == buyer_ivk_commit          │
       │ ──▶ Trade → Challenged                                       │
       │     T_verification 시작 (24h)                                │
       │                                                              │
       │                       off-chain fetch:                       │
       │                         - 체인에서 revealed_ivk_b            │
       │                         - 체인에서 delivery_cm               │
       │                         - Zcash note 복호 → memo             │
       │                         - memo[1..33]에서 K 추출             │
       │                         - IPFS에서 E_K(D) fetch              │
       │                         - § 4.4 step 2–5 재실행              │
       │                       → Verdict ∈ { Honest, Fraud,           │
       │                                     Inconclusive }           │
       │                                                              │
       │                 submit_attestation { verdict, signature }    │
       │                 ────────────────────────────────────────────────▶
       │                                                              │
       │                                    quorum 도달 →            │
       │                                    Trade → Resolved          │
       │                                    T_fraud_proof 시작 (12h)  │
       │                                                              │
       │ (옵션, anyone) submit_fraud_proof { evidence }                │
       │ ──▶ verdict 무효화                                            │
       │     attesting verifier bond 몰수                              │
       │     T_verification 재개, 다른 verifier가 attest                │
       │                                                              │
       │ settle_trade   (T_fraud_proof 후, anyone)                    │
       │ ──▶ 최종 verdict에 따른 자금 흐름 (§ 5)                       │
```

Verdict `Inconclusive`는 verifier가 입력에 도달하지 못함을 의미 (IPFS unreachable, malformed payload). `Honest`나 `Fraud`와 구분되고 다른 fund flow 생성.

---

## 5. Verdict별 자금 흐름

`price = P`. `seller_collateral = P` (price의 100%). `buyer_challenge_collateral = P/2` (price의 50%). 값은 `settle_trade` 시점의 net delta.

| Outcome | Buyer net | Seller net | Verifier bond | Resolution trigger |
|---|---|---|---|---|
| Happy path (challenge 없음) | `−P` | `+P` | 0 | `T_challenge`가 `file_challenge` 없이 만료 |
| Verdict = `Fraud` | `+P` | `−P` | 0 | Buyer 결제 + Seller collateral + Buyer challenge collateral 모두 Buyer에게; Seller가 collateral 몰수 |
| Verdict = `Honest` | `−(P + P/2)` | `+(P + P/2)` | 0 | Buyer의 price + challenge collateral 모두 Seller에게 |
| Verdict = `Inconclusive` | 0 | 0 | 0 | 양측이 본인 deposit 환불 |
| Verdict에 대한 fraud proof | re-attestation에 따라 | re-attestation에 따라 | slash된 verifier에 `−bond` | `T_fraud_proof` 안에 `submit_fraud_proof`가 verdict 무효화; 다른 verifier가 attest; corrupt verifier bond 몰수 |

---

## 6. 암호 primitive

| Primitive | 용도 | 구성 |
|---|---|---|
| **BLAKE2b-256** | `H_D`, `metadata_hash`, `buyer_ivk_commit` | 256-bit 출력의 plain BLAKE2b; `subtle::ConstantTimeEq`로 constant-time 비교 |
| **AES-256-GCM** | `E_K(D)` | 표준 NIST 구성. Nonce는 96-bit, 암호화마다 `OsRng`로 새로 생성 + ciphertext 앞에 prepend. AAD는 `metadata_hash`에 묶임 — 다른 AAD로 복호 시 tag 검증 실패. Tag는 128-bit |
| **Orchard note encryption** | `K`를 운반하는 delivery memo | Zcash Orchard note encryption scheme. Note는 `UA_b`의 diversified transmission key로 암호화; buyer가 `IVK_b`로 `(note, address, memo)` 복원 |
| **Orchard note commitment** | `delivery_cm` cross-chain binding | 표준 Orchard cm: note 필드에 대한 Pedersen-style commitment. Note가 block에 포함될 때 Zcash에서 자동 생성 |
| **ZIP-302 V1 memo layout** | Delivery note의 512-byte payload | `byte 0 = 0xF5` (ZIP-302 binary-payload marker), `bytes 1..33 = K`, `byte 33 = 0x01` (Latch V1 version), `bytes 34..512 = 0x00` (zero padding, decode 시 엄격 검증) |
| **Per-trade Orchard 키** | Buyer 측 privacy + 선택적 IVK reveal | Buyer의 `SK_b`는 거래마다 새로 sample. `IVK_b`는 `UFVK_b`에서 derive. `UA_b`는 single Orchard diversifier 있는 unified address. `buyer_ivk_commit = BLAKE2b-256(IVK_b)`가 on-chain commitment; 컨트랙트가 `file_challenge` 시점에 `H(revealed_ivk_b) == buyer_ivk_commit` 검증 |

대칭 키 `K`는 메모리에서 `Zeroize` + `ZeroizeOnDrop`, `Debug` impl 없음, constant-time 비교. Secret 또는 hash 출력에 닿는 비교는 constant-time 사용.

---

## 7. 보안 속성

### 보장됨

| 속성 | 메커니즘 |
|---|---|
| **Seller가 listing 후 `D`를 substitute 못함** | `H_D`가 listing 시점 on-chain commit; verifier가 복호된 plaintext를 re-hash하여 비교 |
| **Seller가 metadata를 사후 swap 못함** | `metadata_hash`가 AES-GCM tag에 AAD로 묶임; 다른 metadata로 복호 시 tag 검증 실패 |
| **Buyer가 결제 없이 `D`를 추출 못함** | `K`는 `UA_b`로 주소된 Zcash note 안에만 존재; `UA_b`는 `finalize_payment`가 shielded 결제의 bridge attestation을 확인한 후에만 그 note를 수령 |
| **Per-trade key isolation** | Buyer의 `SK_b`는 거래마다 sample; challenge는 그 한 거래의 note만 복호하는 `IVK_b`만 노출 |
| **Verifier 담합 detectable + slashable** | `T_fraud_proof` 안에 누구나 fraud proof 제출 가능; corrupt verifier bond 몰수, 다른 verifier가 재 attest 필요 |
| **Verdict 주어지면 settlement deterministic** | `settle_trade` code path는 total: verdict + 시간 window가 fund flow 유일 결정 |
| **Seller가 happy-path 지급을 위해 online일 필요 없음** | `settle_trade`는 permissionless; `T_challenge` 만료 후 어느 계정이든 trigger 가능 |

### 보장 안 됨

- **Custody-free funds.** PoA bridge가 transparent Zcash 주소에 실제 ZEC를 보관. § 9.1.
- **Network-layer 익명성.** Buyer의 Zcash 브로드캐스트와 NEAR `reserve_listing` 사이의 IP 상관은 다루지 않음. § 9.5.
- **IPFS 영속성.** Seller가 `E_K(D)`를 불안정한 host에 pin 가능. Mitigation은 운영적: Buyer가 결제 잠금 전에 `E_K(D)` fetch.
- **DRM 또는 anti-piracy.** 결제한 Buyer가 `K`와 `E_K(D)`를 재배포 가능.
- **분실된 per-trade `SK_b` 복구.** Per-trade 키는 single-shot; settle 전 분실 시 구매 forfeit.

---

## 8. 신뢰 모델

| Entity | 요구되는 신뢰 | 이유 |
|---|---|---|
| **Escrow 컨트랙트 코드 + NEAR consensus** | High — state machine 정확성 + escrow chain의 깊은 reorg 없음 | 컨트랙트가 법; confirmation보다 깊은 reorg는 기록된 state를 revert 가능 |
| **Zcash 프로토콜** (Halo 2 proof, note commitment, IVK / OVK 복호) | High — shielded 결제 soundness + cross-chain `cm` evidence | 표준 Zcash 가정 |
| **암호 primitive** (BLAKE2b-256, AES-256-GCM, Orchard note encryption) | High — collision resistance + AEAD tag soundness + 정확한 note 복호 | Off-the-shelf, 잘 검증된 구성 |
| **PoA Bridge** | High — bridge의 validator quorum이 Buyer의 결제를 받는 Zcash transparent 주소의 spending key를 보유하고, NEAR에 wrapped 토큰을 mint하는 attestation을 서명 | 가장 큰 single 신뢰 포인트. `MAX_LISTING_PRICE_YOCTO`로 bound |
| **Verifier 집합** | Medium per-challenge — attest하는 verifier majority가 predicate 정직 실행 | 성공한 fraud proof는 corrupt verifier bond 몰수; permissionless audit window가 verdict 분쟁을 허용 |
| **IPFS 운영자** | Low — ciphertext만 보임 | Blob 삭제로 DoS 가능, 내용 읽기 불가능 |
| **각자의 client SW** | 본인 자금에 대해 High | 표준 wallet hygiene; per-trade 키 격리가 단일 compromised 키의 blast radius를 한 거래로 제한 |

프로토콜이 **명시적으로** 신뢰하지 않는 것:

- Escrow chain에서 Buyer 결제의 custody를 가진 어떤 단일 주체. Escrow 컨트랙트가 보관; 문서화된 path 외에는 사람도 서비스도 자금 이동 불가능.
- 무엇이 팔렸는지 사후에 결정할 수 있는 어떤 주체. `H_D`는 listing 시점 commit.
- Verifier 집합 + fraud-proof window 밖의 조정자. 사적 운영자에 대한 항소 없음.
- Buyer 신원을 알 수 있는 어떤 주체. Per-trade Orchard 키로 성공한 challenge는 per-trade 키만 태우고 buyer의 장기 wallet은 노출 안 함.

---

## 9. Trade-off와 알려진 단점

모든 설계 선택에는 비용이 있음. 대략 심각도 순.

### 9.1 Bridge가 실제 ZEC를 보관

Buyer의 결제는 5–7개 validator가 MPC threshold scheme으로 공동 통제하는 transparent 주소로 들어가는 z2t Zcash 트랜잭션. NEAR escrow 컨트랙트는 deposit과 같은 wrapped 토큰을 mint; ZEC 본체는 NEAR로 건너오지 않음. Bridge는:

- 일어나지 않은 deposit에 대해 `finalize_payment`를 fabricate (컨트랙트의 wrapped-token 발행을 drain).
- 실제 deposit에 대해 attest 거부 (Buyer ZEC가 transparent 주소에 갇힘).
- MPC layer에서 compromise되어 모든 in-transit 자금 노출.

**Mitigation:** `MAX_LISTING_PRICE_YOCTO` 파라미터가 listing당 worst-case 손실을 한정. Validator set 자체가 collateralised + 식별 가능, fraudulent attestation에 평판적 + 경제적 비용. Escrow 컨트랙트의 `T_reservation` window가 bridge 조치 없이 buyer 자금이 무기한 reservation에 잡히지 않게 함.

**This does NOT mitigate:** 고가치 batch 중 bridge MPC quorum의 조율된 compromise. Listing cap 인상은 이 신뢰 가정의 재평가 필요.

### 9.2 `Inconclusive`는 soft outcome

Verifier가 `E_K(D)`를 fetch 못 하거나 malformed로 발견하면 `Inconclusive` 반환. 양측이 본인 deposit 환불받고 거래는 어느 쪽도 punish 없이 종료. IPFS blob을 reachable하게 유지하지 않은 Seller가 거짓말 한 적 없는 Seller와 같은 on-chain trace 생성.

**Mitigation:** Buyer 측 규율 — 결제 잠금 전에 IPFS에서 `E_K(D)` fetch. 불안정하게 pin하는 Seller는 분쟁이 아니라 Buyer를 잃음.

**This does NOT mitigate:** Challenge time 직전까지 reliably pin해두고 unpin하는 Seller.

### 9.3 Fraud proof는 permissive

`submit_fraud_proof`는 non-empty `evidence` blob이면 받고 challenge 재개. 의도는 절차적: 다음에 다른 verifier가 attest해야 하고, corrupt attester의 bond는 attestation 사실 자체로 slash. 제공된 evidence가 sound한지에 대한 on-chain cryptographic 검증 없음.

**Mitigation:** Verifier slashing이 "거짓 attest 후 뒤집힘"을 경제적으로 손해로 만듦 — 단일 bad attestation은 항상 attester의 bond 비용.

**This does NOT mitigate:** Slash를 흡수할 수 있는 bond margin을 가진 verifier가 특정 거래를 grief하려는 경우.

### 9.4 DRM 없음, anti-piracy 없음

결제한 Buyer가 `K`와 `E_K(D)`를 누구에게나 재배포 가능. 모든 디지털 재화 마켓플레이스 공통 문제. 프로토콜은 해결 시도 X.

### 9.5 Network-layer 상관

Buyer의 Zcash 브로드캐스트와 NEAR `reserve_listing` 호출 둘 다 IP 주소에서 발생. 둘 다 보는 network observer는 Buyer를 거래에 correlate 가능. 프로토콜은 cryptographic privacy 제공; network-layer privacy는 운영 layer의 Tor 또는 mixnet 필요.

### 9.6 Per-trade 키 수명은 single-shot

Buyer의 per-trade `SK_b`는 자기 머신에만 존재. `reserve_listing`과 `settle_trade` 사이 wallet 닫기는 구매 forfeit. Per-trade 키쌍은 거래 전 기간 동안 persist 필요.

### 9.7 Catalog spam은 protocol layer에서 막지 않음

`seller_collateral` 있으면 누구나 무엇이든 list. Spam의 경제적 floor는 collateral 비용; collateral 몰수를 감수하는 adversary는 catalog 오염 가능.

---

## 10. 프로토콜 파라미터

배포된 컨트랙트의 상수.

| Symbol | 값 | 의미 |
|---|---|---|
| `T_RESERVATION` | 1시간 | Reservation이 `finalize_payment` 없이 유지될 수 있는 최대 시간 |
| `T_KEY_DELIVERY` | 24시간 | Seller가 `finalize_payment` 후 `submit_delivery` 호출까지의 시간 |
| `T_CHALLENGE` | 48시간 | Buyer가 `submit_delivery` 후 challenge 제출 가능한 window |
| `T_VERIFICATION` | 24시간 | Verifier 집합이 `file_challenge` 후 attest까지의 시간 |
| `T_FRAUD_PROOF` | 12시간 | Verdict이 fraud proof로 분쟁될 수 있는 window |
| `SELLER_COLLATERAL` | `price`의 100% | `Fraud` 시 몰수; 그 외엔 반환 |
| `BUYER_CHALLENGE_COLLATERAL` | `price`의 50% | `Honest` 시 몰수; 그 외엔 반환 |
| `MIN_LISTING_LIFETIME` | 1시간 | `create_listing`의 `lifetime_ns` 하한 |
| `MAX_LISTING_LIFETIME` | 365일 | `create_listing`의 `lifetime_ns` 상한 |
| `MAX_LISTING_PRICE` | 100 NEAR (testnet) | `create_listing`의 `price` 상한 |
| `VERIFIER_QUORUM` | 배포 시 설정 | Challenge resolve에 필요한 동의 attestation 수 |
| `MEMO_SIZE` | 512 bytes | Zcash memo 필드 크기 (프로토콜이 부여) |
| `MEMO_LEAD_BINARY` | `0xF5` | ZIP-302 marker byte |
| `MEMO_PROTOCOL_VERSION` | `0x01` | Latch V1 version byte |

---

## 11. 용어집

- **`D`** — cleartext 디지털 재화 (파일, 데이터셋, 레시피 — serialisable한 모든 것).
- **`K`** — per-listing 32-byte AES-256 키.
- **`E_K(D)`** — `AES-256-GCM(K, D, AAD = metadata_hash)`, `nonce(12) ‖ ciphertext(n) ‖ tag(16)`로 운반.
- **`H_D`** — `BLAKE2b-256(D)`, cleartext 재화에 대한 공개 commitment.
- **`metadata_hash`** — `BLAKE2b-256(metadata_json)`; AEAD tag에 AAD로 묶임.
- **`CID`** — `E_K(D)`의 IPFS content identifier.
- **`UA_b`** — Buyer의 per-trade Zcash Unified Address.
- **`IVK_b`** — Buyer의 per-trade Incoming Viewing Key; Buyer가 challenge file 시에만 on-chain reveal.
- **`UFVK_b`** — Buyer의 per-trade Unified Full Viewing Key; private.
- **`SK_b`** — Buyer의 per-trade spending key; 절대 on-chain X.
- **`buyer_ivk_commit`** — `BLAKE2b-256(IVK_b)`; Buyer가 challenge 시 reveal해야 하는 per-trade IVK에 대한 on-chain commitment.
- **`OVK_s`** — Seller의 per-trade Outgoing Viewing Key.
- **memo** — Delivery Zcash note의 512-byte ZIP-302 V1 binary memo, `K`를 운반.
- **`delivery_cm`** — Seller delivery note의 Orchard note commitment; cross-chain binding evidence.
- **attestation** — Verifier의 서명된 verdict (`Honest`, `Fraud`, `Inconclusive`).
- **fraud proof** — `T_fraud_proof` 안에 resolved verdict 무효화 + attesting verifier bond 몰수를 위해 제공되는 bytes.
- **seller collateral** — Seller가 `create_listing` 시점 예치하는 자금; `Fraud` 시 몰수.
- **buyer challenge collateral** — Buyer가 `file_challenge` 시점 예치하는 자금; `Honest` 시 몰수.
