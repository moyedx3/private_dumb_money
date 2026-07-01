# A1/A2 보안 결함 N1·N2 — 크리에이터 신원 바인딩 부재

- **브랜치:** `feat/a1-a2-integration` (HEAD `e50bf59`)
- **대상:** `week7/drop/indexer` (A1 결제엔진 + A2 enclave 플랫폼 통합)
- **작성일:** 2026-06-29
- **출처:** `a1-a2-integration-review.md`(기존 10개) 가 못 짚은 신규 결함 2건.
- **요약 불변식:** **모든 `drop_id`는 정확히 한 `creator_ufvk`가 소유한다.** 이 불변식이
  - **쓰기 경로(provision)** 에서 강제되지 않음 → **N2** (드롭 덮어쓰기/하이재킹)
  - **읽기 경로(dispatch)** 에서 존중되지 않음 → **N1** (교차 크리에이터 콘텐츠키 탈취)
  - N1·N2는 **같은 불변식의 양면**이다. 둘 다 닫아야 결제→언락 경로가 신뢰 가능.

> ## ⚠️ 심각도는 조건부 — 트리거 범위 먼저 읽을 것
> - **현재 단일-creator·단일-buyer 데모(ver2 PoC scope)에서는 N1·N2 둘 다 트리거되지 않는다.** 공격자 creator/드롭이 데모에 존재하지 않기 때문.
> - **멀티테넌트(creator 2명 이상) 또는 `/provision`이 공개망에 노출되는 순간 둘 다 악용 가능** → 그 맥락에서 N1=Critical(콘텐츠 절도), N2=High~Critical(결제 redirect/검열).
> - 즉 "현 데모를 깨는 버그"가 아니라 **"멀티테넌트/운용 단계로 가기 전 반드시 닫아야 할 설계 게이트"**. 아래 Critical 표기는 **운용 맥락 기준**이다.
> - **클래스 구분:** N1 = **기밀성**(피해자 콘텐츠/매출 절도). N2 = **무결성·가용성**(결제 redirect + 검열/DoS, 단 피해자 콘텐츠 기밀성은 유지 — 훔치진 못함).
> - **의존성 방향:** N2를 고치면(provision 인증) N1의 진입장벽이 오른다(공격자가 *승인된* creator여야 함). 그러나 N1은 **독립 수정 필요** — 악의적 승인 creator는 N2가 닫혀도 N1을 수행할 수 있다.

---

## N1 — 교차 크리에이터 콘텐츠키 탈취 (운용 맥락 🔴 Critical / 현 단일-creator 데모 미트리거)

### 무엇
dispatch가 "**누가 돈을 받았는지**"에 묶이지 않는다. 공격자가 자기 주소로 자가송금하면서
memo에 **피해자의 `drop_id`**를 적으면, enclave가 **피해자의 `K_drop`**(콘텐츠 마스터키)을
공격자에게 봉인해 내보낸다. 돈은 공격자 자신에게 갔으므로 피해자는 **0원**, 공격자는
**수수료만 내고** 피해자의 유료 콘텐츠를 푼다.

### 근거 코드
| 위치 | 문제 |
|---|---|
| `detect.rs:158,174` | `if let Some((note, _addr, memo)) = ...` — 노트 **수신주소 `_addr`를 버림**. 결제가 그 드롭의 `deposit_addr`로 왔는지 검증 불가. |
| `lib.rs:38` | `Catalog::lookup(drop_id) -> Option<DropConfig>` — **`drop_id`만**으로 조회. 스캔한 UFVK와 무관. |
| `scan_loop.rs:604` (`scan_catalog_once`) | `Engine::new(catalog.clone(), dispatch.clone())` — 매 UFVK에 **전체 카탈로그** 부여. UFVK_Y 스캔이 X의 드롭을 dispatch 가능. |
| `engine.rs:85` (`on_note`) | `self.cat.lookup(n.drop_id)` 후 creator 일치 검사 없이 `wrap_k_drop(&cfg.k_drop, &n.e_pub)`. |

### 공격 시나리오 (관리자 권한 불필요)
**전제:**
- 피해자 드롭 X = `(drop_id_X, price_X, k_drop_X)` 는 공개 카탈로그에 노출됨(`drop_id_X`·`price_X` 공개).
- 공격자는 **실제로 디코드되는 본인 소유 UFVK**(`creator_ufvk_Y`)가 필요하다 — 쓰레기 문자열이면 `detect.rs:1112` `UnifiedFullViewingKey::decode` 실패 → `?` 전파로 스캔 패스가 abort(#7/N3). 따라서 공격자는 **자기 Zcash 지갑을 만들어 UFVK를 export**한다(쉬움, 그러나 "아무 문자열"은 아님).
- 드롭 Y 등록은 **provision이 열려 있음(N2)에 의존** — N2가 닫히면 이 단계 난이도가 오른다.
- 자가송금분 `price_X`만큼 **일시 유동성** 필요(자기 앞 송금이라 회수됨).

1. 공격자가 자기 드롭 Y를 provision — 위 실제 `creator_ufvk_Y`·자기 `deposit_addr_Y`. (Y의 내용은 무의미. 목적은 Y의 UFVK를 스캔 대상에 넣는 것.)
2. 공격자가 `price_X` ZEC를 **자기 `deposit_addr_Y`로 자가송금**, memo = `drop_id_X ‖ 공격자_e_pub`.
3. 스캐너가 Y의 UFVK 스캔 → 노트 감지(수신=Y, 금액=`price_X`) → memo 디코드 → `drop_id_X`.
4. `lookup(drop_id_X)` → X의 `cfg`(`k_drop_X`, `price_X`). `value(=price_X) >= price_X` 통과.
5. **`k_drop_X`를 공격자 `e_pub`로 봉인해 dispatch.** 공격자가 자기 `e_priv`로 열어 X의 콘텐츠키 획득.
6. 2번의 송금은 자기 자신 앞 → 회수. **순비용 ≈ tx 수수료. 피해자 X 수익 0.**

핵심 불변식("X에게 내야 X 콘텐츠가 열린다") 붕괴 = 콘텐츠 절도.

### PoC (결정론적, 체인 불필요 — 엔진 레벨 재현)
현 코드는 creator 검사가 없어 아래가 `Some(dispatch)`를 반환 = 버그 입증. (수정 후엔 `None`이어야 함.)
```rust
// cat: drop_id_X(=1) 소유자 = UFVK_X.  engine은 UFVK_Y를 스캔 중이라고 가정.
// (현재 Engine은 scanned_ufvk를 모르므로, 이 PoC는 "전역 lookup이 타 creator 드롭을 푼다"를 보인다.)
let cat = single_drop(/*drop_id*/1, /*price*/10_000, /*k_drop*/[9u8;32], /*ufvk*/"uview1_X");
let bucket = MockBucket::default();
let mut eng = Engine::new(cat, bucket.clone());            // 수정안: + scanned_ufvk="uview1_Y"
let out = eng.on_note(&Note{ drop_id:1, e_pub: attacker_epub, value_zat:10_000, txid:[7u8;32] }).await.unwrap();
assert!(out.is_some());          // ← 현재: 통과(=취약). N1 수정 후: is_none() 이어야 함.
assert_eq!(bucket.count(), 1);   // ← X의 k_drop이 공격자 e_pub로 봉인되어 publish됨
```
testnet 풀스택 PoC도 가능(드롭 2개 + 자가송금 1건). 단 본 평가에서 **실행은 안 함**(Rust 툴체인+testnet 필요) — 위는 코드 경로 정적 추적 기반.

### 근본 원인
dispatch 대상 드롭이 **공격자 통제값(memo.drop_id)** 으로 선택되는데, 그 드롭이
**실제 결제를 받은 크리에이터의 것인지** 확인하지 않는다.

### 해결방안

**[권장] B2 — dispatch 시 creator 바인딩 검사 (최소·가장 감사 가능).**
스캐너는 이미 **UFVK별**로 스캔한다(`scan_catalog_once`). 그 UFVK를 엔진에 주입하고,
dispatch 직전에 "이 드롭이 지금 결제받은 크리에이터 소유인가"를 강제한다.

```rust
// engine.rs
pub struct Engine<C: Catalog, B: Bucket> {
    cat: C,
    bucket: B,
    seen: SeenTxids,
    scanned_ufvk: String,           // ← 이 엔진이 결제를 감지 중인 creator UFVK
}

impl<C: Catalog, B: Bucket> Engine<C, B> {
    pub fn new(cat: C, bucket: B, scanned_ufvk: String) -> Self {
        Self { cat, bucket, seen: SeenTxids::default(), scanned_ufvk }
    }

    pub async fn on_note(&mut self, n: &Note) -> anyhow::Result<Option<PaymentDispatch>> {
        if !self.seen.first_time(&n.txid) { return Ok(None); }

        let Some(cfg) = self.cat.lookup(n.drop_id) else {
            tracing::warn!(drop_id = n.drop_id, "drop config not found; skipping");
            return Ok(None);
        };

        // N1 게이트: 푸는 드롭은 반드시 '실제로 이 결제를 받은' 크리에이터 소유여야 한다.
        if cfg.creator_ufvk != self.scanned_ufvk {
            tracing::warn!(
                drop_id = n.drop_id,
                "memo drop_id belongs to a different creator than the paid UFVK; refusing dispatch"
            );
            return Ok(None);
        }

        if n.value_zat < cfg.price_zat { /* 기존 underpaid 처리 */ return Ok(None); }

        let blob = wrap_k_drop(&cfg.k_drop, &n.e_pub)?;
        let key = blob_key(&blob[..EPHEMERAL_PUBLIC_KEY_LEN], &n.txid);
        self.bucket.put(&key, &blob).await?;
        Ok(Some(PaymentDispatch { drop_id: n.drop_id, txid: n.txid, value_zat: n.value_zat, bucket_key: key }))
    }
}
```
호출처 갱신: `scan_loop.rs:604` → `Engine::new(catalog.clone(), dispatch.clone(), ufvk.clone())`.
나머지 호출처(`api.rs:390`, `bin/scan-live.rs:178`, `engine.rs`/`scan_loop.rs` 테스트)도 인자 추가 필요(blast radius 소).

> **동등 대안 B1** — 카탈로그를 creator로 필터한 뷰를 엔진에 주입: `Engine::new(catalog.scoped_to(&ufvk), dispatch.clone())`.
> 엔진은 "자기가 풀 수 있는 드롭만" 보게 됨. on_note 변경 없음, `CatalogStore::scoped_to(ufvk)` 추가 필요. 효과는 B2와 동일.

**[후속·심층방어] A — 결제 수신주소 ↔ `deposit_addr` 대조.**
`detect.rs`가 버리는 `_addr`를 보존해 반환하고, 노트 수신주소가 `cfg.deposit_addr`(가 게시한 UA)의
수신자 중 하나인지 확인. 같은 creator가 **여러 드롭**(같은 UFVK, 다른 주소)을 운영할 때
드롭 간 혼동까지 차단. 단 UA는 여러 receiver를 묶으므로 **문자열 단순 비교 불가** — 노트의
구체 receiver가 게시 UA에 포함되는지 매칭하는 작업 필요 → PoC 범위 밖, 후속.

> B2(또는 B1)만으로 **교차 크리에이터 절도는 완전 차단**된다(공격자 Y 스캔은 Y 소유 드롭만 dispatch).
> 같은 creator 내부 드롭 혼동은 가격검사가 대부분 막고(비싼 드롭을 싸게 못 풂), 잔여분은 A로 후속.

### 완료 기준
- UFVK_Y로 감지된 결제가 `creator_ufvk == UFVK_Y`인 드롭만 dispatch.
- memo에 타 크리에이터 `drop_id`를 넣은 결제는 dispatch 거부(`None`) + 경고 로그.

### 추가할 테스트
```rust
// engine.rs / scan_loop.rs
// "다른 creator의 drop_id를 가리키는 memo로 결제 → dispatch 안 됨"
// cat: drop_id_X 소유자 = UFVK_X. engine.scanned_ufvk = UFVK_Y.
// on_note(Note{ drop_id: X, value>=price_X, ... }) => Ok(None), bucket.count()==0.
```

---

## N2 — 인증 없는 드롭 덮어쓰기 / 하이재킹 (운용 맥락 🔴 High~Critical / 현 단일-creator 데모 미트리거)

> **클래스 = 무결성·가용성** (결제 redirect + 검열/DoS). N1과 달리 **피해자 콘텐츠 기밀성은 깨지지 않는다** — 공격자는 피해자 `k_drop`을 모르므로 콘텐츠를 **훔치진 못한다**. 덮어쓰기로 **무력화/redirect**할 뿐.

### 무엇
`/provision`은 공개된 enclave provisioning pubkey(`/attest`로 노출)만 있으면 **누구나** 호출 가능.
그리고 카탈로그는 `drop_id` 키로 **무조건 덮어쓴다**. 따라서 공격자가 **피해자의 `drop_id`**로
provision하면 피해자 카탈로그 엔트리를 교체할 수 있다 — `deposit_addr`를 자기 주소로 바꿔
**buyer 결제를 가로채거나**, `k_drop`을 교체해 **콘텐츠를 무력화**한다(buyer가 받은 키로 복호화 실패).
단 정당 creator가 재-provision하면 복구되므로 **가역적**(덮어쓰기 경쟁) — "영구" 잠금은 아니다.

### 근거 코드
| 위치 | 문제 |
|---|---|
| `server.rs:80-93` (`provision_h`) | 인증/레이트리밋 없음. sealed payload만 열면 통과. |
| `provision.rs` `open_provision` | `drop_id`를 **payload(공격자 통제)** 에서 그대로 취함. |
| `catalog.rs:18-20` (`insert`) | `self.inner.write().insert(drop_id, (cfg, title))` — 기존 소유자 검증 없이 **덮어씀**. |

### 공격 시나리오
1. 공격자가 `/attest`로 enclave pubkey 획득(정상 공개값).
2. 공격자가 `ProvisionPayload{ drop_id: drop_id_피해자, deposit_addr: 공격자주소, k_drop: 임의, creator_ufvk: 공격자ufvk, ... }`를 봉인해 `POST /provision`.
3. `catalog.insert(drop_id_피해자, ...)` 가 피해자 엔트리 **덮어씀**.
4. 공개 카탈로그가 이제 **공격자 `deposit_addr`** 게시 → 이후 buyer 결제가 **공격자에게** 감.
   (`k_drop` 교체 시: 기존 콘텐츠 blob은 옛 키로 암호화돼 있어 buyer가 받은 새 키로 **복호화 실패** → 콘텐츠 무력화. 정당 creator 재-provision으로 가역.)

C4("re-provision은 `drop_id` 기준 idempotent — 재전송 안전") 설계는 **정직한 creator만 재provision**을
암묵 가정하나, 그 가정이 **강제되지 않는다**.

### PoC (결정론적, 체인·네트워크 불필요 — 카탈로그 레벨 재현)
현 코드는 소유권 검사가 없어 두 번째 insert가 성공·덮어쓴다 = 버그 입증. (수정 후엔 거부되어야 함.)
```rust
let store = CatalogStore::default();
store.insert(1, cfg(/*ufvk*/"uview1_VICTIM", /*addr*/"u1victim"), "victim".into());
store.insert(1, cfg(/*ufvk*/"uview1_ATTACKER", /*addr*/"u1attacker"), "evil".into()); // 현재: 성공
assert_eq!(store.public_entries()[0].deposit_addr, "u1attacker"); // ← 피해자 주소가 공격자 것으로 교체됨
// N2 수정 후: 2번째 insert가 Err(OwnershipMismatch), deposit_addr는 "u1victim" 유지.
```
HTTP PoC: 같은 `drop_id`로 다른 creator payload를 `POST /provision` 2회 → 2번째 `200 OK` + `GET /catalog`의 `deposit_addr` 교체 확인. 충돌 경고/거부 없음.

### 근본 원인
`drop_id` ↔ creator 소유권이 **provision 시점에 바인딩되지 않음**. 누구나 임의 `drop_id`를 주장/교체.

### 해결방안

**[권장] First-writer-wins — `drop_id`를 최초 `creator_ufvk`에 고정.**
`creator_ufvk`는 **봉인되어 들어오고 카탈로그에 공개되지 않는다**(secret 유지 — `catalog.rs` 테스트가
`!json.contains("uview1secret")` 보장). 따라서 공격자는 피해자의 UFVK를 **모르므로** 일치시킬 수 없다.

```rust
// catalog.rs
#[derive(Debug)]
pub enum CatalogError { OwnershipMismatch }

impl CatalogStore {
    /// 최초 등록자(creator_ufvk)만 같은 drop_id를 덮어쓸 수 있다.
    pub fn insert(&self, drop_id: u64, cfg: DropConfig, title: String) -> Result<(), CatalogError> {
        let mut map = self.inner.write().unwrap();
        if let Some((existing, _)) = map.get(&drop_id) {
            if existing.creator_ufvk != cfg.creator_ufvk {
                return Err(CatalogError::OwnershipMismatch); // 다른 creator의 drop_id 탈취 차단
            }
        }
        map.insert(drop_id, (cfg, title));
        Ok(())
    }
}
```
```rust
// server.rs provision_h — 매핑
match s.inner.catalog.insert(drop_id, cfg, title) {
    Ok(()) => StatusCode::OK,
    Err(_) => StatusCode::CONFLICT,   // 409: 이미 다른 creator 소유
}
```
- 정직한 creator의 재-provision(C4)은 **같은 `creator_ufvk`** 라 그대로 통과 → idempotent 유지.
- 공격자는 피해자 UFVK를 몰라 일치 불가 → 하이재킹 차단.

**[후속] 잔여 위협 2건.**
- **스쿼팅**: 공격자가 피해자보다 **먼저** 피해자 의도 `drop_id`를 선점하면 피해자가 등록 못 함(하이재킹은 아니고 DoS). → `drop_id = truncate(blake2b(creator_ufvk ‖ nonce))` 처럼 **creator에서 파생**하면 선점 자체가 불가. 단 `drop_id`는 현재 C/B 계약상 creator-선택 u64 → I-계약 변경이라 후속 v2.
- **provision 스팸**: 인증이 없어 무한 드롭 생성으로 in-memory 카탈로그 메모리 고갈 가능. → 레이트리밋 / 자원 상한 / (운용 시) creator 인증. 데모 범위 밖이나 기록.

### 완료 기준
- 이미 존재하는 `drop_id`에 **다른 `creator_ufvk`** 로 provision → 거부(409) + 카탈로그 불변.
- 같은 `creator_ufvk`의 재-provision → 정상 덮어쓰기(C4 유지).

### 추가할 테스트
```rust
// catalog.rs / server.rs
// 1) 같은 creator 재provision: insert(1, cfg_ufvkA_price500) → insert(1, cfg_ufvkA_price700) => Ok, price=700.
// 2) 타 creator 하이재킹: insert(1, cfg_ufvkA) → insert(1, cfg_ufvkB) => Err(OwnershipMismatch), 엔트리는 A 유지.
// 3) HTTP: 2번 시나리오를 POST /provision 2회로 → 두 번째 409, GET /catalog의 deposit_addr 불변.
```

---

## 적용 순서 · 주의

1. **N2 먼저(`catalog.rs`/`server.rs`)** — 표면 작고 독립적. `insert` 시그니처가 `Result`로 바뀌니
   기존 호출처(`server.rs:92`, `catalog.rs` 테스트 `catalog.rs:82`)를 `?`/`unwrap`으로 갱신.
2. **N1(`engine.rs`/`scan_loop.rs`)** — `Engine::new`에 `scanned_ufvk` 인자 추가 → **모든 호출처**
   (`api.rs:390`, `bin/scan-live.rs:178`, 테스트들) 갱신 필요. 컴파일러가 다 잡아줌.
3. **#7(UFVK 조기검증)과 N1을 같이 처리.** `provision.rs`에서 `creator_ufvk`를
   `UnifiedFullViewingKey::decode`로 검증하는 그 지점이 곧 **N1의 creator 바인딩 신뢰 기반**이다.
   단 검증을 엄격히 하면 테스트 placeholder(`uview1secret`/`uview1x`/`uview1demo`/`uview1mock`)가
   **전부 거부**돼 provision/engine/catalog/scan 테스트가 깨진다 → 픽스처를 실제 디코드 가능한
   UFVK(또는 testnet `uviewtest1...` 벡터)로 교체.
4. **테스트로 회귀 고정**: 위 N1·N2 음성 테스트(절도 거부 / 하이재킹 거부)를 머지 게이트에 포함.

> **PoC 범위 메모.** ver2 재평가는 단일-creator·단일-buyer 해피패스를 PoC로 descope했다.
> 그 범위에선 N1·N2가 트리거되지 않는다(크리에이터 1명, 신뢰). 그러나 **둘 다 일반 사용자가
> 권한 없이 트리거**하며, 데모를 "여러 creator가 올리는 마켓"으로 한 발만 키우면 즉시 악용된다.
> 비용(각각 수십 줄)이 낮고 핵심 자금/콘텐츠 무결성을 지키므로 **머지 게이트 권장**.
