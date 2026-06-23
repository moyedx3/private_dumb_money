# 레인 B — 구현 착수 전 설계 점검 + 킥오프 프롬프트

> 대상: [`lane-B-buyer-app.md`](./lane-B-buyer-app.md), 교차검증 [`interfaces.md`](./interfaces.md). 목적: **구현 시작 전 마지막 설계 점검** + 바로 빌드 들어갈 수 있는 프롬프트/픽스처/마일스톤 제공.
> 결론: B 설계는 대체로 구현 준비됨(spike #1·#2·#3가 핵심 위험 제거). 단 **인터페이스 레벨 블로커 3개(B1·B2·B4)를 닫기 전엔 (c)/(d) 단계를 production 경로로 못 짠다.** 블로커는 B 혼자 못 정하고 팀 합의(interfaces.md)가 필요 — Part B에 닫는 구체값 초안.

---

## Part A — 설계 점검 결과

### 🔴 블로커 (착수 전/통합 전 필수)

#### B1. 카탈로그 I3-a에 `deposit_addr`가 없다 → 결제 URI 못 만듦

- lane-B §4(c)는 `zcash:${deposit_addr}?amount=...&memo=...`를 만든다(line 102). `deposit_addr`가 반드시 필요.
- 그런데 **interfaces.md I3-a(line 42)에 그 필드가 없다**: `{ drop_id, price_zec, h_content, title }`. lane-B §3 표(line 51)도 동일하게 주소 누락.
- 즉 **B는 카탈로그만 보고는 어디로 입금할지 모른다.** spike #1에선 `gen-ua`로 만든 테스트 주소를 직접 썼을 뿐(§5 line 144), production 경로(카탈로그→주소)가 비어 있음.
- **영향**: M3(QR/결제) production 불가. (dev 중엔 테스트 UA 하드코딩으로 우회 가능 → B 혼자 진행은 됨, 통합 전 필수.)
- **고침**: I3-a에 `deposit_addr`(가려진 주소 문자열) 추가. 출처 = C가 provisioning(I5) 때 같이 제출, A2가 I3-a에 게시. (Part B-1)

#### B2. spike #1이 검증한 건 **텍스트 memo**, I1 production은 **바이너리 40B** → 비-UTF-8 memo 미검증

- spike #1 memo: `spike12|drop=1|epub=TESTKEY` (§5 line 142) = **printable ASCII = 유효 UTF-8**.
- I1 production memo: `drop_id(u64 BE) ‖ e_pub(32 raw)` = **raw 40바이트** (interfaces.md I1). big-endian u64는 상위 바이트가 `0x00`, e_pub는 랜덤 바이트 → **유효 UTF-8 아님**.
- 다수 지갑이 memo를 ZIP-302 UTF-8 텍스트로 취급해 **비-UTF-8 바이트를 거부/손상**시킨다. spike는 텍스트라 통과했고, **바이너리 경로는 한 번도 진짜 Zashi를 안 거쳤다.** §6(1) step2가 "TESTKEY 더미여도 경로 확인엔 충분"이라 한 건 **이 차이를 가린다** — 더미가 텍스트면 바이너리 케이스를 검증하지 못함.
- **영향**: 가장 큰 단일 실패점. I1 위에 (c) 전체가 올라가는데, Zashi가 바이너리 memo를 떨구면 A1이 `drop_id/e_pub`를 영영 못 받아 **언락이 절대 안 됨**.
- **고침 (택1, Part B-2)**:
  - (옵션 1) I1 바이너리 유지 + **데모 폰·데모 빌드로 진짜 40B 바이너리 memo를 즉시 검증**(M6를 앞으로). 통과하면 그대로.
  - (옵션 2, 안전) I1을 **printable 인코딩**으로: 온체인 memo를 `zd1:` + base64url(40B) ASCII 텍스트(~58자, 512B 한참 아래)로. 항상 유효 UTF-8 보장. A1이 prefix 파싱. — *권장: 옵션1을 먼저 싸게 테스트, 깨지면 옵션2로 즉시 전환할 수 있게 둘 다 코드 준비.*

#### B4. 버킷 list/열거 API가 인터페이스에 없다 → 폴링 못 함

- lane-B §4(d)는 "새로 올라온 blob을 **전부 받아** 하나씩 따본다"(line 111). blob 키 = `blake2b256(ek_pub‖txid)`(I2 line 36) = 랜덤 → **"내 거"를 이름으로 못 고름**(프라이버시상 의도된 것).
- 그러려면 **버킷의 blob 목록을 열거**하는 수단이 있어야 하는데, **interfaces.md엔 I2(개별 blob + 키)만 있고 "어떻게 새 blob 목록을 받나"가 없다.**
- **영향**: M4(폴링) production 불가. (dev 중엔 mock 버킷으로 우회.)
- **고침 (Part B-3)**: 버킷 열거 계약 정의 — 최소 `GET /dispatch/` → blob 키 배열, 또는 append-only 인덱스 객체 + cursor. B는 "이미 따본 키" 캐싱으로 증분(§4(d) line 118).

### 🟡 수정/명확화 (구현 중 처리)

#### B5. opened blob → 어느 drop인지 매핑이 빠짐

- I2 sealed payload = **`K_drop`(32B)만**(interfaces.md I2). v0의 `drop_id‖K_drop`과 달리 **drop_id가 blob에 없다.**
- 그래서 blob이 열려 K_drop을 얻어도 **어느 drop의 콘텐츠(h_content)를 받을지** 그 자체로는 모른다. §4(e)는 "h_content로"라고만 해 단일 구매를 암묵 가정.
- **고침**: B가 **`(e_priv ↔ drop_id ↔ h_content)`를 묶어서 보관**하고, trial-open은 보관 중인 각 e_priv를 각 blob에 시도 → 어떤 e_priv로 열렸는지로 drop을 식별 → 그 h_content fetch. 단일 구매면 자명하지만 **다중 동시 구매를 지원하려면 필수**. (대안: A1이 sealed payload를 `drop_id(8)‖K_drop(32)`=40B로 바꿔 blob 88B. 그럼 blob만으로 drop 식별 가능 — 단 I2 변경이라 A1 합의 필요.)

#### B6. `crypto_box_seal_open` 실패는 **throw**다 — `if(opened)` 분기는 죽은 코드

- §4(d) line 114는 "실패 시 throw → try/catch"라면서 line 115에서 `if (opened)`도 씀. libsodium-wrappers의 `crypto_box_seal_open`은 **MAC 실패 시 예외를 던진다**(falsy 반환 아님). 성공하면 항상 bytes.
- **고침**: `try { opened = seal_open(...) } catch { continue }` 로만. `if(opened)`에 의존 금지. (남의 blob = catch로 조용히 skip.)

#### B7. 콘텐츠 무결성 검증 빠짐 (선택적 강화)

- I4: `h_content = sha256(blob)`. B는 fetch한 콘텐츠 blob에 대해 **`sha256(blob) === h_content` 검증 후 복호화**하면 변조를 일찍 잡는다. §4(e)엔 없음.
- AES-GCM tag가 어차피 무결성을 보장하므로(잘못된 ct → 인증 실패) **보안상 필수는 아님**. 명확한 에러·조기 탐지용 권장.

#### B8. 잔가지

- **drop_id JSON number 정밀도**: 카탈로그 drop_id가 JSON 숫자(I3-a). JS `BigInt(drop_id)`로 변환하나, drop_id가 2^53 넘으면 카탈로그 파싱 단계에서 이미 정밀도 손실. 데모(작은 id)는 무해. 권장: 키울 거면 string.
- **amount 포맷**: `price_zec` 문자열을 URI에 **그대로** 통과(재포맷 금지) → A1의 `price_zat` 검증과 어긋나지 않게. ZIP-321 amount는 ≤8 소수.
- **IndexedDB e_priv 보관**(§4(b)): 새로고침 복구 ↔ XSS 유출 트레이드. 옵트인 + 24h 만료 유지. recovery 파일 옵션(§8.2d)도 동일 주의.
- **over-fetch 유지**: §4(d)의 "전부 받아 따본다"는 프라이버시상 옳음. "내 결제 이후만" 같은 최적화는 타이밍 상관 재유입 → 데모 범위 밖 한계로 문서화(§8.3)된 대로 둠.

### ✅ 잘 맞는 것 (확인됨)

- 80B sealed box 산식(`ek_pub32 + (K_drop32 + MAC16)=48` = 80) — lane-B §3·§4(d) ↔ interfaces.md I2 일치. ✓
- AES-GCM 레이아웃 `nonce12 ‖ ct ‖ tag16`, WebCrypto가 끝 16B를 tag로 — §4(e) ↔ I4 일치. ✓
- 카탈로그 통째 fetch(드롭별 엔드포인트 없음) — 프라이버시 요구(spec §7.3) 준수. ✓
- libsodium-wrappers ↔ dryoc 동일 Curve25519 sealed box → 바이트 호환. ✓ (spike에서 라이브러리 정합 전제 확인)

---

## Part B — interfaces.md에 반영할 확정값 초안 (팀 confirm 필요)

> B 혼자 못 정함. 첫 통합 전 팀이 confirm하고 interfaces.md를 고친다. 아래는 제안.

**B-1. I3-a에 `deposit_addr` 추가:**
```json
{ "drop_id": 1, "price_zec": "0.01", "h_content": "<버킷 키>",
  "title": "고양이 사진", "deposit_addr": "u1... | zs1..." }
```
- 제약: **반드시 Sapling/Orchard 가려진 주소**(투명 t-addr면 memo 소실, §8 함정1).
- 출처: C가 provisioning(I5)에 `deposit_addr` 포함 제출 → A2가 I3-a에 게시. (또는 A2가 `creator_ufvk`에서 파생. 단 데모는 C가 명시 제출이 단순.)
- (선택) drop별 diversified 주소로 로테이션 가능(같은 UFVK로 스캔). 데모는 1주소도 무방.

**B-2. I1 memo 인코딩 결정** — 옵션1(바이너리) 먼저 테스트, 폴백 옵션2(printable) 준비:
```
옵션2(폴백): 온체인 memo = ASCII 텍스트
  memo_text = "zd1:" + base64url_nopad( drop_id(8 BE) ‖ e_pub(32) )   // ~58 ASCII자
  ZIP-321 memo= 파라미터 = base64url_nopad( utf8_bytes(memo_text) )
  A1: memo_text를 prefix "zd1:"로 검증 후 base64url decode → 40B → drop_id/e_pub
```
- B-2 결정 = **M6를 앞당겨 진짜 폰으로 옵션1 검증**한 결과로 확정. (Part C M6)

**B-3. 버킷 dispatch 열거 계약:**
```
GET /dispatch/index            → { "keys": ["<hex>", ...], "cursor": "<opaque>" }   // 또는
GET /dispatch/index?since=<cursor> → 그 이후 신규 키만
개별: GET /dispatch/<key>      → 80B blob (I2)
```
- B는 `since` cursor + 로컬 "따본 키" 캐시로 증분 폴링. (단순 list로 시작, 규모 시 append-only 로그.)

**B-4. (선택) I2 sealed payload에 drop_id 포함?** — B5 해결용. 포함 시 blob 80→88B, B가 다중 구매를 간단히 처리. 미포함 유지 시 B가 `e_priv↔drop_id` 매핑. **A1과 합의해 택1.**

---

## Part C — 레인 B 구현 킥오프 프롬프트 (바로 실행 가능)

> 아래를 dev(또는 코딩 에이전트)에게 그대로 넘겨 시작. 블로커는 mock으로 우회하며 병행, 통합 전 Part B로 닫는다.

```
역할: Unlockable Drop의 구매자 웹앱(레인 B)을 구현한다. 정적 SPA. 백엔드 없음.
정답 계약: week7/drop/team/interfaces.md (I1·I2·I3-a·I4). 스펙: lane-B-buyer-app.md.
바이트가 어긋나면 interfaces.md가 정답이고, 거기서 막히면 그 문서를 고치고 팀 공지.

스택(권장):
- TypeScript + Vite (정적 빌드). React 불필요 — 화면 적음. 필요시만.
- libsodium-wrappers (키쌍·sealed box·base64url)  ※ await sodium.ready 게이트 필수
- WebCrypto crypto.subtle (AES-256-GCM 복호화)
- qrcode (QR 렌더)  ※ QR 직접 구현 금지
- vitest (단위 테스트)
- 상태: 메모리 기본 + 옵트인 IndexedDB(e_priv 24h, 만료 타이머)

마일스톤 (각 끝에 verify):
M0 스캐폴드 — Vite TS 프로젝트, 위 deps, `await sodium.ready` 부팅 게이트.
   verify: 빈 페이지 로드 + sodium.ready 콘솔 확인.
M1 카탈로그 — I3-a 전체를 1회 fetch(드롭별 엔드포인트 없음), title+price_zec 목록 렌더,
   h_content·deposit_addr는 보관만(미표시). 클릭→결제 화면.
   verify: mock catalog.json으로 목록 렌더, 드롭별 호출 0건(네트워크 탭).
M2 키쌍 — 구매 1건당 crypto_box_keypair(X25519). e_priv는 메모리(또는 옵트인 IndexedDB).
   (e_priv, drop_id, h_content)를 한 묶음으로 보관(=B5 매핑). 재사용 절대 금지.
   verify: 구매 2회 → 서로 다른 e_pub 2개, 매핑 보관 확인.
M3 ZIP-321 + QR — memo 40B(drop_id u64 BE ‖ e_pub 32) → base64url(URLSAFE_NO_PADDING),
   uri=`zcash:${deposit_addr}?amount=${price_zec}&memo=${b64}`, QR 렌더.
   deposit_addr는 B1 닫히기 전 테스트 UA 하드코딩. 가려진 주소 가드(투명이면 거부).
   ※ memo 인코딩은 B2 결정(옵션1 바이너리 / 옵션2 zd1:텍스트) — 둘 다 함수로 분리해 스왑 가능하게.
   verify: 생성 memo 40B를 디코드해 byte-identical 재구성(round-trip 단위테스트).
M4 폴링 + trial-open — 버킷 열거(B4 계약, 없으면 mock)로 신규 blob 받아 보관 중 각 e_priv로
   crypto_box_seal_open(blob, e_pub, e_priv) try/catch. 열린 1개→K_drop, 그 e_priv의 drop 식별.
   따본 키 캐시로 증분. 열리면 폴링 중단.
   verify: 내 blob 1 + 남의 blob 2 섞은 mock → 내 것만 열고 K_drop 복원, 남의 건 조용히 skip.
M5 콘텐츠 복호 — 식별된 drop의 h_content로 I4 blob fetch, (선택)sha256==h_content 검증,
   nonce12‖ct‖tag16를 K_drop으로 AES-256-GCM 복호 → 콘텐츠 타입별 렌더.
   verify: M4의 K_drop으로 mock 콘텐츠 복호→평문 일치. 잘못된 K_drop→깔끔한 에러.
M6 [B2 닫기 — 우선] 데모 폰·데모 Zashi 빌드로 진짜 40B 바이너리 memo를 ZIP-321 QR로 스캔→
   mainnet ~0.0001 ZEC 결제→수신 지갑에서 받은 memo가 byte-identical인지 확인.
   PASS면 옵션1 확정. FAIL(memo 떨굼/손상)이면 옵션2(zd1:텍스트)로 전환 후 재시도.
   런북에 Zashi 버전·OS 기록.
M7 mock 픽스처 + 단위테스트(아래 픽스처). M4·M5를 A1/C 없이 단독 검증.
M8 데모 로지스틱스 — 충전된 mainnet 테스트 지갑, 데모 런북, 폴백 스크린캐스트,
   함정 경고 UX("탭 닫으면 구매 소실"/"가려진 주소만"/네트워크상관 한 줄).

테스트 픽스처 (상대편 바이트와 동일하게):
- dispatch blob(I2): K_drop=randombytes(32); fakeBlob=crypto_box_seal(K_drop, e_pub) → 80B.
  → M4가 seal_open으로 같은 K_drop 복원하는지.
- content blob(I4): plaintext → AES-256-GCM(K_drop) → nonce12‖ct‖tag16 조립(= C 레인 레이아웃).
  → M5가 같은 평문 복원하는지. h_content=sha256(blob).
- 음성: 다른 키로 싼 blob을 섞어 M4가 skip하는지; 잘못된 K_drop으로 M5가 에러내는지;
  IndexedDB 경로가 새로고침 후 e_priv·매핑 복구하는지.

DoD: lane-B-buyer-app.md §7 전부 + M6 PASS + 위 단위테스트 통과.

주의(함정, lane-B §8):
- 가려진(shielded) 주소만 — 투명이면 memo 소실(가드 필수).
- e_priv 분실=구매 영구 소실 → 결제 전 "탭 닫지 마세요" 경고 필수(최소).
- base64url은 sodium.to_base64(URLSAFE_NO_PADDING)만 — 손치환 금지.
- sealed box·AES-GCM 바이트는 dryoc/C와 동일 곡선·tag위치·nonce길이. 픽스처를 상대 바이트로.
```

---

## Part D — 착수 순서 요약

1. **B2 먼저 싸게 검증**(M6 일부) — 진짜 폰으로 바이너리 40B memo 한 번. 결과로 I1 인코딩 확정. *제일 위험한 가정이라 제일 먼저.*
2. **팀 30분**: Part B 4개(deposit_addr / memo 인코딩 / 버킷 열거 / blob에 drop_id 포함?) confirm → interfaces.md 패치.
3. B는 M0~M3를 mock(테스트 UA·mock catalog)으로 **즉시 병행 시작** — 블로커가 B 단독 진행을 막진 않음.
4. B1·B4 닫히면 mock→실계약 교체, M4·M5 통합.
5. M6 전체 PASS + M7 단위테스트 → M8 데모 준비.
