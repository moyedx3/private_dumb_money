# Lane B → A1/A2 요청서 재검토 결과

검토 기준 파일:

```text
origin/master:week7/drop/team/lane-B-requests-to-A1-A2.md
```

주의: 현재 작업 브랜치 `a1`의 worktree에는 이 파일이 없고, 원격 `origin/master`에 존재한다. 아래 내용은 해당 원격 master 파일을 직접 읽고 현재 A1 구현 상태와 대조한 결과다.

---

## 요약

Lane B가 A1/A2에 요청한 항목은 총 4개다.

| 요청 ID | 대상 | 요청 | 현재 상태 |
| --- | --- | --- | --- |
| `R-A2-1` | A2 | dispatch 목록 list endpoint 추가 | **미충족** |
| `R-A2-2` | A2 | public catalog에 `deposit_addr` 추가 | **미충족** |
| `R-A2-3` | A2 | dispatch blob과 content blob 목록 분리 | **미충족** |
| `R-A1-1` | A1 | `A1B64:` text memo fallback을 `interfaces.md` I1에 문서화 | **코드는 충족, 문서는 미충족** |

현재 A1 core는 결제 감지, memo decode, dispatch 생성, encrypted scan state, API service vector까지 구현되어 있다. 하지만 Lane B 요청서 기준으로는 **buyer가 실제 브라우저에서 결제 URL을 만들고 dispatch를 폴링해 unlock하는 데 필요한 public-facing 접점이 아직 부족하다.**

특히 즉시 막히는 부분은 다음 3개다.

```text
1. public catalog에 deposit_addr 없음
2. dispatch 목록 endpoint 없음
3. A1B64 memo fallback이 interfaces.md에 없음
```

---

## R-A2-1 + R-A2-3 — dispatch 전용 list endpoint

### Lane B 요청

B는 구매 후 dispatch blob을 이름으로 직접 찾을 수 없다.

이유:

```text
bucket_key = blake2b256(ek_pub || txid)
```

여기서 `ek_pub`는 A1이 sealed box를 만들 때 생성되는 랜덤 ephemeral public key다. 구매자는 dispatch blob을 받기 전에는 이 값을 알 수 없다.

따라서 B의 설계는 다음과 같다.

```text
GET /dispatch
→ dispatch key 목록 수신
→ 각 key로 80B dispatch blob fetch
→ buyer e_priv로 trial-open
→ 열리는 blob만 내 것
```

또한 content blob과 dispatch blob이 같은 목록에 섞이면 B가 큰 content blob까지 매번 다운로드해 trial-open하려고 하므로 비효율적이다. 그래서 dispatch 전용 목록이 필요하다.

### 현재 A1 구현 상태

현재 A1에는 다음이 있다.

- `Bucket::put(key, blob)` boundary
- `ApiVectors::lookup_dispatch(bucket_key)`
- `ApiVectors::dispatch_blobs()` 내부 helper
- `GET /api/buyers/dispatch/{bucket_key}` contract 문서화

하지만 Lane B가 요구한 형태의 endpoint/vector는 아직 없다.

현재 방식의 문제:

```text
GET /api/buyers/dispatch/{bucket_key}
```

이 방식은 구매자가 `bucket_key`를 이미 알고 있어야 한다. Lane B 설계에서는 구매자가 이를 알 수 없으므로 production unlock flow에 부족하다.

### 필요한 구현

A1/A2 통합 관점에서 다음 중 하나가 필요하다.

#### 권장: dispatch 전용 list vector

```rust
pub fn list_dispatch_keys(&self) -> Vec<String>
pub fn list_dispatch_blobs(&self) -> Vec<DispatchBlobRecord>
```

HTTP endpoint 예시:

```http
GET /api/dispatch
GET /api/dispatch/{key}
```

또는:

```http
GET /api/dispatch/recent
```

응답 예시:

```json
[
  {
    "bucket_key": "...",
    "blob": "<base64 dispatch blob>"
  }
]
```

B는 이 목록의 각 blob에 대해:

```js
try {
  const k_drop = sodium.crypto_box_seal_open(blob, e_pub, e_priv);
} catch {
  // 내 것이 아니므로 skip
}
```

를 수행한다.

### 판정

```text
R-A2-1: 미충족
R-A2-3: 미충족
```

현재 `bucket_key` 직접 조회 vector는 보조 기능으로는 유용하지만, Lane B가 요청한 polling/trial-open flow를 완전히 만족하지 않는다.

---

## R-A2-2 — public catalog에 deposit_addr 추가

### Lane B 요청

B는 결제 URL을 다음 형식으로 만든다.

```text
zcash:<deposit_addr>?amount=<ZEC>&memo=<base64url(drop_id || e_pub)>
```

따라서 buyer 앱이 public catalog에서 반드시 알아야 하는 값은 다음이다.

```text
drop_id
price_zec
h_content
title
deposit_addr
```

### 현재 문서 상태

`interfaces.md`의 I3-a public catalog 예시는 현재 다음 형태다.

```json
{
  "drop_id": 1,
  "price_zec": "0.01",
  "h_content": "<콘텐츠 blob 버킷 키>",
  "title": "고양이 사진"
}
```

여기에 `deposit_addr`가 없다.

### 현재 A1 구현 상태

현재 `ApiVectors::register_creator_drop` request는 다음만 받는다.

```rust
RegisterCreatorDropRequest {
    creator_id: String,
    creator_ufvk: String,
    price_zat: u64,
    k_drop: [u8; 32],
}
```

즉 현재 service vector에도 다음 public metadata가 없다.

```text
title
price_zec
h_content
deposit_addr
```

### 문제

`deposit_addr`가 없으면 B는 결제 URI를 만들 수 없다.

또한 주소는 반드시 shielded address여야 한다. 투명 주소(`t1`, `t3`)면 Zcash memo가 사라져 A1이 `drop_id/e_pub`를 읽지 못한다.

### 필요한 구현

public catalog entry 타입이 필요하다.

```rust
pub struct PublicCatalogEntry {
    pub drop_id: u64,
    pub title: String,
    pub price_zec: String,
    pub h_content: String,
    pub deposit_addr: String,
}
```

creator registration도 확장해야 한다.

```rust
pub struct RegisterCreatorDropRequest {
    pub creator_id: String,
    pub creator_ufvk: String,
    pub price_zat: u64,
    pub price_zec: String,
    pub k_drop: [u8; 32],
    pub title: String,
    pub h_content: String,
    pub deposit_addr: String,
}
```

그리고 public catalog 전체 조회 vector가 필요하다.

```rust
pub fn list_public_catalog(&self) -> Vec<PublicCatalogEntry>
```

HTTP endpoint 예시:

```http
GET /api/catalog
```

중요: Lane B 문서상 drop별 catalog 조회는 만들지 않는 것이 좋다. 전체 catalog를 한 번에 fetch해야 열람 지문이 줄어든다.

### 필수 검증

provision/register 시 최소한 다음을 거부해야 한다.

```text
deposit_addr starts_with "t1"
deposit_addr starts_with "t3"
empty deposit_addr
```

가능하면 `deposit_addr`가 제출된 `creator_ufvk`로 viewable한지도 검증해야 한다. 어려우면 우선 shielded address guard라도 필요하다.

### 판정

```text
R-A2-2: 미충족
```

현재 가장 큰 blocker다. B는 `deposit_addr` 없이는 결제 QR/URI를 만들 수 없다.

---

## R-A1-1 — A1B64 text memo fallback 문서화

### Lane B 요청

A1 코드는 memo를 두 형식으로 받을 수 있다.

1. raw 40B

```text
drop_id(8 bytes BE) || e_pub(32 bytes)
```

2. text fallback

```text
A1B64:<base64url_no_pad(raw40)>
```

하지만 공용 계약문서 `interfaces.md` I1에는 raw 형식만 적혀 있다. Lane B는 문서만 보고 구현해야 하므로 `A1B64:` prefix와 ZIP-321 wrapping 방식을 명시해야 한다.

### 현재 A1 코드 상태

`indexer/src/memo.rs`에는 이미 구현되어 있다.

```rust
pub const TEXT_MEMO_PREFIX: &str = "A1B64:";
```

테스트 vector도 있다.

```text
A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw
```

즉 코드 구현은 되어 있다.

### 현재 문서 상태

현재 `team/interfaces.md` I1에는 raw memo만 설명되어 있다.

부족한 내용:

```text
A1B64 prefix
text fallback 형식
ZIP-321 memo= 파라미터가 어떤 bytes를 base64url하는지
기본 형식 결정 기준
test vector
```

### 필요한 문서 패치

`team/interfaces.md` I1에 다음 내용을 추가해야 한다.

```text
A1은 두 memo payload 형식을 디코드한다.

1. Raw payload
   drop_id(8, u64 BE) || e_pub(32) = 40B

2. Text fallback
   ASCII "A1B64:" || base64url_no_pad(raw40)

ZIP-321 memo= 파라미터는 온체인 memo bytes를 base64url_no_pad한 값이다.

- raw형:
  on-chain memo bytes = raw40
  URI memo=base64url_no_pad(raw40)

- text fallback형:
  on-chain memo bytes = utf8("A1B64:" + base64url_no_pad(raw40))
  URI memo=base64url_no_pad(on-chain memo bytes)

고정 prefix: A1B64:

Test vector:
  drop_id = 1
  e_pub = 000102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f
  text fallback = A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw
```

### 기본 형식 결정

Lane B 요청서에서는 기본 형식을 Zashi 실측 후 결정하라고 되어 있다.

현실적인 권장안:

```text
기본: raw40 ZIP-321 memo
fallback: A1B64 text memo
```

단, 실제 데모 폰/Zashi 빌드에서 raw binary memo가 깨지거나 거부되면 기본을 `A1B64`로 바꿔야 한다.

### 판정

```text
R-A1-1: 코드 충족, 문서 미충족
```

A1이 지금 당장 처리해야 하는 가장 작은 작업은 `team/interfaces.md` 문서 패치다.

---

## 현재 A1 구현 기준 추가 관찰

### 1. API vector가 일부 선행 구현됨

현재 A1에는 `indexer/src/api.rs`가 추가되어 있다.

현재 가능한 것:

```text
creator registration
Catalog::lookup(drop_id)
Engine::on_note(...)
Bucket::put(bucket_key, dispatch_blob)
lookup_dispatch(bucket_key)
```

하지만 Lane B 요청과 비교하면 아직 부족하다.

부족한 것:

```text
list_public_catalog()
list_dispatch_keys() 또는 list_recent_dispatches()
deposit_addr/title/h_content public metadata
```

### 2. A2/master 요청과 A1 구현 범위가 섞여 있음

Lane B 요청서의 `R-A2-*`는 master branch의 A2/server/bucket 쪽을 대상으로 한다.

하지만 현재 A1 브랜치에도 service vector를 만들었기 때문에, 통합 시 다음 중 하나로 정리해야 한다.

```text
A2 HTTP/server layer가 A1 ApiVectors 같은 service layer를 호출
또는 A2의 bucket/catalog 구현이 같은 contract를 별도로 구현
```

핵심은 B가 보는 public contract가 아래처럼 맞아야 한다는 점이다.

```http
GET /api/catalog
GET /api/dispatch or /api/dispatch/recent
GET /api/dispatch/{key}
```

---

## 우선순위별 실행 계획

## 1순위 — interfaces.md I1 업데이트

대상: A1

작업:

```text
team/interfaces.md I1에 A1B64 text fallback 문서화
test vector 추가
raw/text ZIP-321 wrapping 방식 명시
```

이유:

```text
코드는 이미 있으므로 문서만 닫으면 B가 memo encoder를 안전하게 구현 가능
```

---

## 2순위 — public catalog entry 확장

대상: A2 중심, A1 service vector도 맞춰야 함

작업:

```text
PublicCatalogEntry 추가
RegisterCreatorDropRequest 확장
list_public_catalog() 추가
GET /api/catalog contract 확정
```

필수 필드:

```text
drop_id
title
price_zec
h_content
deposit_addr
```

---

## 3순위 — dispatch list/recent 추가

대상: A2 중심, A1 service vector도 맞춰야 함

작업:

```text
list_dispatch_keys() 또는 list_recent_dispatches() 추가
GET /api/dispatch 또는 GET /api/dispatch/recent contract 확정
content blob과 dispatch blob 분리
```

완료 조건:

```text
B가 content blob을 섞어 받지 않고 80B dispatch blob만 trial-open 가능
```

---

## 4순위 — enclave provisioning/attestation

대상: A2

작업:

```text
GET /attest
POST /provision
sealed payload open inside enclave
encrypted catalog DB-backed store
```

현재 A1의 plaintext `RegisterCreatorDropRequest`는 개발 vector로만 봐야 한다. 운영에서는 host API가 `creator_ufvk`와 `k_drop` 평문을 보면 안 된다.

---

## 최종 결론

`origin/master`의 Lane B 요청서를 기준으로 보면, 현재 A1은 core payment engine 쪽은 많이 충족했다.

충족된 것:

```text
memo decode
A1B64 decode 코드
UFVK chain scan
amount check
dispatch sealed box 생성
bucket_key 생성
encrypted scan state
creator/buyer service vector 초안
```

아직 부족한 것:

```text
A1B64 interfaces.md 문서화
public catalog 전체 조회
catalog deposit_addr 포함
dispatch recent/list 조회
content/dispatch blob 분리
attested sealed provisioning
```

가장 먼저 처리할 것은 다음 3개다.

```text
1. team/interfaces.md I1에 A1B64 문서화
2. PublicCatalogEntry에 deposit_addr 포함
3. dispatch list/recent vector 추가
```
