# 레인 A1 현재 상태·목표·협업 요청서

> **보낸이**: 레인 A1(결제 엔진 / scanner, `origin/a1`). **받는이**: Lane B(구매자 앱), Lane A2(enclave/platform), Lane C(creator/content), Lane D(storage/bucket).  
> **맥락**: A1은 Zcash shielded 결제를 감지하고 memo를 파싱해 결제를 검증한 뒤, 구매자만 열 수 있는 dispatch blob을 생성하는 레인이다. 현재 구현은 `origin/a1` 기준이며, `master`의 A2/B/C/D 구현과 통합하려면 아래 interface gap을 닫아야 한다.  
> **주의**: 실제 UFVK/IVK, `K_drop`, seed phrase, private key는 문서·로그·public catalog에 절대 포함하지 않는다.

---

## 한눈에

| # | 대상 | A1이 현재 제공하는 것 | 최종 지원 목표 | 협업 필요 |
| --- | --- | --- | --- | --- |
| **A1-Now-1** | 전체 | lightwalletd로 실제 체인 블록/tx 조회 | 운영 polling loop | A2 배포/운영 경계 필요 |
| **A1-Now-2** | B | `A1B64:` memo decode | B가 문서만 보고 memo 생성 | `interfaces.md` I1에 text fallback 반영 |
| **A1-Now-3** | A2 | encrypted scan state trait/file 구현 | enclave sealing key 기반 state 보호 | `StateCipher`를 enclave sealing adapter로 교체 |
| **A1-Now-4** | C/A2 | drop config 기반 결제 검증 | creator 등록 API + sealed catalog DB | `deposit_addr`, metadata, secret provisioning 연결 |
| **A1-Now-5** | B/D | dispatch blob 생성 + bucket boundary put | buyer dispatch 조회 endpoint | dispatch list + trial-open, content/dispatch 분리 |

---

## 1. A1의 책임 범위

A1이 담당하는 핵심 흐름은 다음이다.

```text
creator/drop config 등록
→ buyer가 Zcash shielded payment 전송
→ A1 scanner가 lightwalletd에서 compact/full tx 조회
→ creator UFVK/IVK로 incoming note 복호화
→ memo에서 drop_id와 buyer e_pub 파싱
→ value_zat >= price_zat 검증
→ K_drop을 buyer e_pub으로 sealed-box 암호화
→ dispatch blob 생성
→ bucket에 저장
→ buyer가 dispatch blob을 조회해 K_drop 복호화
```

A1이 소유하는 공용 interface는 주로 두 가지다.

| interface | 설명 | 소유 |
| --- | --- | --- |
| **I1 memo** | buyer payment memo에 들어가는 `drop_id + e_pub` | A1/B |
| **I2 dispatch blob** | `K_drop`을 buyer `e_pub`으로 암호화한 80B blob | A1/B/D |

A1은 카탈로그 저장소, enclave attestation, content blob 자체, wallet UI는 직접 소유하지 않는다. 이들은 각각 A2/C/D/B와 interface로 연결한다.

---

## 2. 현재 A1 구현 상태 (`origin/a1` 기준)

### 2.1 체인 조회 / scanner

현재 가능한 동작:

- lightwalletd primary/backup endpoint 연결
- chain tip 조회
- 지정 블록 range의 compact block 조회
- compact txid 기반 full transaction 조회
- UFVK 기반 Sapling/Orchard incoming note 탐지
- shielded memo decode
- live smoke CLI 실행

구현 위치:

| 파일 | 역할 |
| --- | --- |
| `indexer/src/lightwalletd.rs` | lightwalletd gRPC client boundary |
| `indexer/src/detect.rs` | UFVK/IVK incoming note decrypt, memo 추출 |
| `indexer/src/scan_loop.rs` | block range scan → full tx fetch → detect → engine 연결 |
| `indexer/src/bin/scan-live.rs` | 실제 chain smoke CLI |

---

### 2.2 memo format

A1은 raw 40B memo와 text fallback memo를 모두 해석할 수 있다.

#### Raw form

```text
memo[0..8]  = drop_id : u64 big-endian
memo[8..40] = e_pub   : X25519 public key, 32 bytes
length      = 40 bytes
```

#### Text fallback form

```text
A1B64:<base64url_no_pad(drop_id(8B) || e_pub(32B))>
```

Test vector:

```text
drop_id = 1
e_pub   = 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
memo    = A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw
```

협업 주의:

- 일부 wallet은 raw binary memo 입력이 어렵기 때문에 `A1B64:` text fallback이 필요하다.
- `interfaces.md` I1에는 아직 raw 중심 설명만 있으므로 text fallback을 공용 계약으로 추가해야 한다.
- ZIP-321 `memo=`는 “온체인 memo bytes”를 base64url-no-pad로 감싼 값이다.

---

### 2.3 결제 검증 / engine

현재 A1 engine은 감지한 note에 대해 다음을 수행한다.

```text
memo decode
→ drop_id로 catalog lookup
→ value_zat >= price_zat 확인
→ seen txid replay guard
→ dispatch blob 생성
→ bucket put
```

구현 위치:

| 파일 | 역할 |
| --- | --- |
| `indexer/src/engine.rs` | 결제 검증, replay guard, dispatch 생성 orchestration |
| `indexer/src/dispatch.rs` | sealed-box wrapping, bucket key 생성 |
| `indexer/src/memo.rs` | memo encode/decode |

---

### 2.4 dispatch blob

A1은 구매자 `e_pub`으로 `K_drop`을 sealed-box 암호화한다.

```text
dispatch_blob = crypto_box_seal(K_drop, buyer_e_pub)
              = ek_pub(32) || ciphertext+MAC(48)
              = 80 bytes
```

bucket key:

```text
bucket_key = blake2b256(ek_pub || txid)
```

여기서 `ek_pub`은 sealed box가 내부적으로 생성한 ephemeral public key이며 dispatch blob의 앞 32B다. 따라서 buyer는 dispatch blob을 받기 전에는 `ek_pub`을 모른다. 즉 `GET /dispatch/{bucket_key}`만 public 조회 방식으로 두면 **bucket key를 알아야 blob을 받고, blob을 받아야 bucket key 계산에 필요한 ek_pub을 알 수 있는 순환 의존**이 생긴다.

특징:

- dispatch blob은 구매자의 `e_priv`로만 열 수 있다.
- bucket key에는 `drop_id`, buyer 식별자, creator 식별자를 넣지 않는다.
- `bucket_key`는 dispatch list에서 발견한 뒤 단건 조회할 때 쓰는 key다.
- buyer discovery는 `GET /dispatch` 목록을 받아 각 dispatch blob을 trial-open하는 방식이 기본이다.

---

### 2.5 encrypted scan state

A1은 scanner 재시작과 중복 처리 방지를 위해 scan state를 저장한다.

저장 대상:

```text
last_scanned_height
seen_txids
```

현재 구현:

- `ScanState`
- `MemoryScanState`
- `StateCipher`
- `SecretboxStateCipher` dev adapter
- `EncryptedFileScanState`

운영 목표:

- host filesystem에 plaintext state를 남기지 않는다.
- TEE에서는 `StateCipher` 구현체를 enclave sealing key 기반으로 교체한다.
- 운영자가 raw `seen_txids`, UFVK, `K_drop`을 볼 수 없게 한다.

---

### 2.6 API service vector

현재 A1에는 실제 HTTP framework 없이 service vector 형태의 API boundary가 있다.

현재 의도된 route shape:

```text
POST /api/creators/{creator_id}/drops
GET  /api/catalog                               # public catalog with deposit_addr
GET  /api/buyers/dispatch                       # dispatch key list
GET  /api/buyers/dispatch/{bucket_key}          # dispatch blob direct get
```

현재 가능한 동작:

- creator drop 등록 request 검증
- in-memory catalog 저장
- public catalog에 `deposit_addr` 노출
- dispatch blob list/lookup
- engine이 생성한 dispatch를 buyer lookup boundary에서 조회

아직 부족한 것:

- 실제 HTTP server adapter
- buyer-friendly dispatch list/recent endpoint
- sealed catalog DB
- external bucket backend

---

## 3. 최종적으로 A1이 지원할 것

### 3.1 Creator drop 등록

최종적으로 A1/A2는 creator가 다음 정보를 등록할 수 있게 해야 한다.

```text
creator_id
drop_id
price_zat
K_drop
creator_ufvk 또는 scan 가능한 view key
deposit_addr
h_content
public metadata(title 등)
```

민감 정보:

- `K_drop`
- `creator_ufvk` / IVK

이 값들은 public catalog에 포함하지 않고 enclave/sealed storage에서만 다룬다.

---

### 3.2 Public catalog 제공

Lane B가 결제 URL을 만들 수 있도록 public catalog가 필요하다.

예상 public entry:

```json
{
  "drop_id": 1,
  "creator_id": "creator-1",
  "title": "example drop",
  "price_zat": 10000,
  "price_zec": "0.00010000",
  "deposit_addr": "<shielded unified address>",
  "h_content": "<content blob key>",
  "memo_format": "A1B64"
}
```

주의:

- `deposit_addr`는 shielded address여야 한다.
- `t1`/`t3` transparent address는 memo가 사라지므로 provision/catalog 단계에서 거부해야 한다.
- `UFVK`, `K_drop`은 public entry에 절대 포함하지 않는다.

---

### 3.3 Live polling scanner

운영에서는 A1이 주기적으로 chain tip을 확인하고 새 블록을 스캔한다.

```text
load encrypted scan state
→ latest block 확인
→ last_scanned_height+1 .. tip range scan
→ incoming note detect
→ memo parse
→ payment verify
→ dispatch put
→ encrypted scan state save
```

운영에서 추가로 정해야 할 것:

- confirmation depth
- reorg handling
- lightwalletd endpoint failover policy
- scan interval
- state backup/restore 방식

---

### 3.4 Dispatch 저장/조회

최종적으로 buyer는 결제 후 dispatch blob을 찾을 수 있어야 한다.

단, `GET /dispatch/{bucket_key}`만으로는 buyer discovery가 불가능하다. `bucket_key = blake2b256(ek_pub || txid)`인데 `ek_pub`은 dispatch blob 내부에 들어있으므로, buyer는 blob을 받기 전까지 `bucket_key`를 계산할 수 없다. 따라서 buyer는 먼저 dispatch key 목록을 받고, 각 blob을 trial-open해 자기 blob을 찾는다.

권장 endpoint:

```text
GET /dispatch              -> dispatch key list
GET /dispatch/{bucket_key} -> dispatch_blob direct get
```

대안 endpoint:

```text
GET /dispatch/recent?since=<cursor> -> dispatch key list 증분 조회
GET /dispatch/by-tx/{txid}          -> wallet이 txid를 안정적으로 제공하는 경우
```

권장 방향:

- 기본 discovery는 `dispatch list + trial-open`으로 둔다.
- 규모가 커지면 `recent?since=<cursor>` 증분 조회를 추가한다.
- dispatch blob과 content blob 저장소를 분리한다.
- content blob은 `h_content`로 단건 조회하게 한다.

---

### 3.5 Enclave-only secret handling

최종 보안 목표는 다음 plaintext가 enclave 밖에 노출되지 않는 것이다.

```text
creator UFVK/IVK
K_drop
raw scan state
sealed catalog DB plaintext
```

A1은 이미 `StateCipher` 경계를 가지고 있으므로 A2는 이를 enclave sealing adapter로 연결하면 된다.

---

## 4. Lane별 협업 요청

## R-B-1 — B는 `A1B64:` memo 생성기를 공용 계약에 맞춘다

**무엇.** Buyer app은 catalog의 `drop_id`, buyer가 생성한 `e_pub`을 사용해 A1 memo를 만든다.

**형식.**

```text
A1B64:<base64url_no_pad(drop_id(8B BE) || e_pub(32B))>
```

**완료 기준.** B의 memo encoder가 위 test vector와 byte-identical하게 일치한다.

---

## R-B-2 — B는 dispatch list로 blob을 발견하고 trial-open한다

**무엇.** Buyer가 결제 후 dispatch blob을 찾고 `crypto_box_seal_open`으로 `K_drop`을 복원한다.

**현재 gap.** A1의 `bucket_key = blake2b256(ek_pub || txid)`는 buyer가 사전에 계산할 수 없다. `ek_pub`은 dispatch blob 안에 들어있기 때문이다. 따라서 buyer에게 `bucket_key`를 먼저 요구하는 조회 방식은 UX상 순환 의존이다.

**결정.** B는 dispatch key 목록을 받은 뒤 각 dispatch blob을 가져와 로컬에서 trial-open한다.

```text
GET /dispatch          -> dispatch key list
GET /dispatch/{key}    -> dispatch blob 80B
seal_open(blob, e_pub, e_priv)
```

**대안.**

- 개선: `recent?since=cursor`로 증분 polling
- txid를 wallet에서 얻을 수 있다면 `by-tx`도 후보
- `buyer_e_pub` 또는 hash hint 직접 조회는 MVP 편의 API로는 가능하지만, 팀 기본 privacy 설계는 list + trial-open이다.

**완료 기준.** B가 결제 후 dispatch list에서 받은 blob 후보 중 하나를 buyer `e_priv`로 열어 `K_drop`을 복원한다.

---

## R-A2-1 — A2는 sealed provisioning과 catalog DB를 A1 service vector에 연결한다

**무엇.** Creator가 등록한 secret 값을 host가 보지 못하게 enclave 내부에 저장하고 A1 engine이 조회할 수 있게 한다.

**필요 값.**

```text
drop_id
price_zat
K_drop
creator_ufvk 또는 IVK
deposit_addr
h_content
metadata
```

**완료 기준.** A1이 `Catalog::lookup(drop_id)`로 enclave/sealed DB의 drop config를 얻고, public catalog는 민감 정보를 제외한 값만 반환한다.

---

## R-A2-2 — A2는 scan state sealing adapter를 제공한다

**무엇.** A1의 `StateCipher`를 dev key가 아니라 enclave sealing key로 구현한다.

**왜.** 운영자는 scanner state file을 볼 수 없어야 하며, 재시작 후에도 replay guard가 유지되어야 한다.

**완료 기준.** host disk에는 encrypted state만 남고, enclave 내부에서만 `last_scanned_height`와 `seen_txids`가 복호화된다.

---

## R-C-1 — C는 creator 등록 payload에 payment/catalog 필드를 포함한다

**무엇.** Creator/content flow가 A1/A2에 drop 정보를 넘길 때 결제에 필요한 필드를 포함한다.

**필수.**

```text
price_zat
K_drop
creator_ufvk 또는 view key
deposit_addr
h_content
public title/metadata
```

**완료 기준.** 등록된 drop이 public catalog에 보이고, buyer가 catalog만 보고 결제 주소·금액·memo를 만들 수 있다.

---

## R-D-1 — D는 dispatch blob과 content blob 저장소를 분리한다

**무엇.** Dispatch polling에서 큰 content blob이 섞이지 않도록 저장소 또는 index를 분리한다.

**권장 route.**

```text
GET /dispatch            -> dispatch key list
GET /dispatch/{key}      -> dispatch blob 80B direct get
GET /content/{h_content} -> encrypted content blob
```

**완료 기준.** B가 dispatch polling 중 content blob을 다운로드하지 않는다.

---

## 5. 통합 시 필요한 공용 계약 변경

### 5.1 `interfaces.md` I1 보강

현재 I1에는 raw memo 중심 설명이 있다. 아래 내용을 추가해야 한다.

```text
A1은 두 memo 형식을 모두 decode한다.

(raw)
  drop_id(8, u64 BE) || e_pub(32) = 40B

(text fallback)
  "A1B64:" + base64url_no_pad(drop_id(8B BE) || e_pub(32B))

ZIP-321 memo= 파라미터는 온체인 memo bytes를 base64url_no_pad로 감싼 값이다.
```

### 5.2 public catalog 보강

현재 A1 service vector와 master HTTP catalog 계약에 `deposit_addr`를 반영했다.

반영된 핵심값:

```text
deposit_addr
price_zat 또는 price_zec
memo_format
```

주의: `deposit_addr`는 shielded address여야 하며 `t1`/`t3` transparent address는 memo 결제를 처리할 수 없어 거부한다.

### 5.3 dispatch list/recent endpoint 추가

현재 `bucket_key` 단건 lookup만으로는 buyer polling UX가 부족하다. `bucket_key` 계산에 필요한 `ek_pub`이 dispatch blob 내부에 있으므로 buyer가 먼저 알 수 없기 때문이다.

추가 후보:

```text
GET /dispatch/recent?since=<cursor>
GET /dispatch
GET /dispatch/{key}
```

---

## 6. 현재 A1 검증 상태

A1에서 이미 확인한 것:

- unit tests 통과
- lightwalletd 연결 가능
- 실제 chain block/full tx 조회 가능
- UFVK 기반 incoming note detect 가능
- memo decode 가능
- dispatch blob 생성 가능
- encrypted scan state smoke 가능
- 이미 처리한 txid 중복 방지 가능

문서에 실제 UFVK/seed/private key는 포함하지 않는다.

---

## 7. A1의 남은 작업 후보

| 우선순위 | 작업 | 이유 |
| --- | --- | --- |
| Done | `interfaces.md` I1에 `A1B64:` 문서화 | B encoder와 A1 decoder 정합성 |
| Done | public catalog에 `deposit_addr` 반영 | B가 결제 URL 생성 가능 |
| Done/P1 | dispatch list endpoint / recent는 후속 | B가 dispatch blob 발견 가능, bucket_key 순환 의존 제거 |
| P1 | creator registration HTTP adapter | C/A2와 실제 연결 |
| P2 | enclave `StateCipher` adapter | 운영 보안 |
| P2 | sealed catalog DB | 운영자 secret 노출 방지 |
| P2 | confirmation/reorg 정책 | 운영 안정성 |
| P2 | external bucket backend | production storage |

---

## 부록 — 관련 코드 위치

| 항목 | 위치 |
| --- | --- |
| memo raw/text fallback | `origin/a1:week7/drop/indexer/src/memo.rs` |
| dispatch blob/key | `origin/a1:week7/drop/indexer/src/dispatch.rs` |
| payment engine | `origin/a1:week7/drop/indexer/src/engine.rs` |
| scan loop | `origin/a1:week7/drop/indexer/src/scan_loop.rs` |
| encrypted scan state | `origin/a1:week7/drop/indexer/src/state.rs` |
| API service vector | `origin/a1:week7/drop/indexer/src/api.rs` |
| live smoke CLI | `origin/a1:week7/drop/indexer/src/bin/scan-live.rs` |
| 기존 팀 계약 문서 | `master:week7/drop/team/interfaces.md` |
| Lane B 요청서 | `master:week7/drop/team/lane-B-requests-to-A1-A2.md` |
