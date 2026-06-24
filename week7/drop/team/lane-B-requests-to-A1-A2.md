# 레인 B → A1·A2 요청서

> **보낸이**: 레인 B(구매자 웹앱). **받는이**: A1(결제엔진, `origin/a1`), A2(enclave 플랫폼, `master`).
> **맥락**: B 구현 착수 전 코드 점검 결과, B가 의존하는 접점이 비어 있거나 계약문서(`interfaces.md`)에 없음. 아래 **R-A2-1~3**을 닫아주면 B의 결제 URL 생성·폴링·언락 경로가 production으로 열린다. **R-A2-4**는 기능 블로커가 아니라 **설계 정합성(프라이버시)** 항목 — 데모는 무관, 운용 전 필요.
> **B가 자기 쪽에서 처리하는 것**(요청 아님): `e_priv↔drop_id↔h_content` 매핑(B5), memo 인코더, mock 픽스처, 콘텐츠 복호화.
> **주의**: A1(`origin/a1`)과 A2(`master`)가 아직 미머지. 이 요청들은 **통합 빌드에 반영**되어야 B 최종 테스트가 가능.

---

## 한눈에

| #          | 받는이 | 요청                                                           | 크기             | B에서 막히는 것        |
| ---------- | --- | ------------------------------------------------------------ | -------------- | ---------------- |
| **R-A2-1** | A2  | 버킷 **dispatch 목록(list) 엔드포인트** 추가                            | 소              | M4 폴링(blob 못 훑음) |
| **R-A2-2** | A2  | 카탈로그(I3-a)에 **`deposit_addr`** 추가                            | 중              | M3 결제 URL(주소 없음) |
| **R-A2-3** | A2  | **dispatch blob ↔ content blob 분리**(list에서)                  | 소 (R-A2-1과 동시) | 큰 content 헛다운로드  |
| **R-A1-1** | A1  | **텍스트 memo 폴백(`A1B64:`)을 `interfaces.md` I1에 문서화** + 기본형식 결정 | 문서 (코드 거의 없음)  | B가 memo 형식 못 맞춤  |
| **R-A2-4** | A2  | **공개 버킷을 인덱서(TEE 호스트)에서 분리** — buyer 읽기 surface를 TEE 아닌 별도 저장소로 | 운용 (데모 무관) | (기능 안 막힘) 프라이버시 정합성 |

---

## R-A2-1 + R-A2-3 — dispatch 전용 list 엔드포인트 (둘이 한 작업)

**무엇.** B가 버킷에서 **dispatch blob 목록만** HTTP로 받을 수 있는 라우트.

**왜.**
- dispatch blob 키 = `blake2b256(ek_pub‖txid)`(A1 `dispatch.rs`) → **랜덤·불투명**. B는 "내 blob"을 키로 못 찍는다 → **목록을 받아 전부 trial-open**해야 한다(프라이버시 설계상 의도된 것).
- 그런데 현재 라우트는 **`GET /bucket/:key`(단건)뿐**(`server.rs:48-58`). `list()`는 `bucket.rs:49`에 **함수로 존재하고 테스트도 되지만 HTTP로 안 뚫려 있다.** → B는 브라우저라 호출 불가 → **폴링 자체가 불가능**.
- 게다가 content blob(I4, sha256 키)과 dispatch blob(I2, blake2b 키)이 **같은 디렉토리에 섞여** 있어 `list()`가 둘 다 반환한다. content는 암호화된 이미지/파일이라 **MB 단위**일 수 있는데, B가 폴링마다 그걸 받아서 `seal_open` 시도→실패→폐기하면 **큰 파일 헛다운로드**가 매 폴링 발생.

**어디.** `week7/drop/indexer/src/server.rs`(라우트), `bucket.rs`(저장/list).

**제안 (둘 중 택1).**

*옵션 A — dispatch 전용 store 분리 (권장):*
```rust
// dispatch blob과 content blob을 다른 store에 둔다.
//   content → <root>/content   (C가 put, B가 h_content 키로 단건 get)
//   dispatch → <root>/dispatch  (A1이 put, B가 목록+단건 get)
// server.rs 라우트:
.route("/dispatch",      get(dispatch_list_h))   // → Json<Vec<String>>  (dispatch 키만)
.route("/dispatch/:key", get(dispatch_get_h))    // dispatch blob 단건
.route("/bucket/:key",   get(bucket_get_h).put(bucket_put_h)) // content (그대로)
// A1은 dispatch blob을 /dispatch store에 put하도록 연결.
```
- B는 `GET /dispatch`로 **dispatch 키만** 받아 trial-open, content는 자기가 필요한 1개만 `GET /bucket/<h_content>`로 받음. content를 폴링 목록에서 영영 안 봄.

*옵션 B — 한 store 유지 + dispatch 인덱스 객체:*
- A1이 dispatch blob을 put할 때 키를 `dispatch_index`(append-only)에 추가. `GET /dispatch/index` → 그 목록. content는 인덱스에 안 들어감.
- `valid_key`가 hex-only(`bucket.rs:26`)라 prefix로 구분하긴 어려움 → 인덱스 객체 방식이 그 제약을 안 건드림.

**(선택) 증분.** 데모는 전체 목록 반환으로 충분. 규모 키우면 `?since=<cursor>` 추가(B가 "이미 따본 키" 캐시로 증분하긴 하나, 서버 cursor면 전송량 절감).

**완료 기준.** B가 `GET /dispatch`(또는 `/dispatch/index`)로 **dispatch 키 배열만** 받고, 각 키를 `GET /dispatch/:key`로 80B blob을 받아 `seal_open` 가능. content blob이 그 목록에 안 섞임.

---

## R-A2-2 — 카탈로그(I3-a)에 `deposit_addr` 추가

**무엇.** 공개 카탈로그 엔트리에 buyer가 입금할 **가려진(shielded) 수신 주소**.

**왜.** B는 결제 URL `zcash:<deposit_addr>?amount=...&memo=...`를 만든다. 그런데 **현재 시스템 어디에도 입금 주소가 없다** — 공개 `CatalogEntry`(`lib.rs:51`, `catalog.rs:28`)에도 없고, 내부 `DropConfig`(`lib.rs:19`)에도 `creator_ufvk`만 있고 주소가 없다. → **buyer가 어디로 입금할지 모름. B의 M3가 통째로 막힘.**

**어디.** `week7/drop/indexer/src/lib.rs`(`ProvisionPayload`·`CatalogEntry`), `catalog.rs`(`public_entries`), `provision.rs`(open).

**제안 (출처 택1 — P1이라 옵션 2 권장).**

*옵션 2 — creator가 자기 주소 제출 (권장, 파생 코드 0):*
```rust
// lib.rs — I5 provision 입력에 추가
pub struct ProvisionPayload {
    pub drop_id: u64,
    pub price_zat: u64,
    pub k_drop: String,
    pub creator_ufvk: String,
    pub h_content: String,
    pub deposit_addr: String,   // ← 추가: 가려진 수신 주소(creator 본인 주소)
}
// lib.rs — I3-a 공개 엔트리에 추가
pub struct CatalogEntry {
    pub drop_id: u64,
    pub price_zec: String,
    pub h_content: String,
    pub title: String,
    pub deposit_addr: String,   // ← 추가
}
// DropConfig에도 보관 → catalog.public_entries()가 그대로 복사 게시.
```
- creator는 자기 주소를 이미 아니까(자기 지갑) 그냥 제출. P1 모델(creator가 주소 소유)과 일치.

*옵션 1 — `creator_ufvk`에서 파생:* zcash 키 라이브러리로 ufvk→UA 파생(어디서든 가능, TEE 전용 아님). 일관성 자동 보장 + diversified 로테이션 쉬움. 단 파생 코드 필요 → A2보다 zcash 라이브러리 있는 A1/C가 적합.

**필수 검증 (둘 다).**
- **반드시 가려진(Sapling/Orchard) 주소.** 투명 주소(`t1`/`t3`)면 **memo가 통째로 사라져** A1이 `drop_id/e_pub`를 못 받는다(lane-B §8 함정1). → `deposit_addr`가 `t1`/`t3`로 시작하면 provision **거부**.
- (옵션 2) 가능하면 그 주소가 제출한 `creator_ufvk`로 **viewable한지** 확인(안 맞으면 A1이 그 결제를 스캔 못 함). 어려우면 최소한 creator 신뢰 + 가려진주소 가드.

**완료 기준.** `GET /catalog` 응답 각 엔트리에 `deposit_addr`(가려진 주소)가 있고, B가 그걸로 ZIP-321 URL을 만들 수 있음. 투명 주소는 provision 단계에서 거부됨.

---

## R-A1-1 — 텍스트 memo 폴백을 `interfaces.md` I1에 문서화

**무엇.** A1이 이미 지원하는 **2번째 memo 형식**(`A1B64:` 텍스트)을 공용 계약문서에 적고, raw/텍스트 중 **기본형식 결정**.

**왜.** B가 memo를 만들고 A1이 읽는다 → **형식이 글자 하나까지 일치해야** 결제 인식됨. A1 `memo.rs`는 **두 형식**을 받는다:
1. raw `drop_id(8 BE)‖e_pub(32)` = 40B
2. 텍스트 `A1B64:<base64url_nopad(40B)>` ← UTF-8 memo만 되는 지갑용 폴백

그런데 **이 텍스트 형식과 prefix `A1B64:`가 A1 Rust 코드(`memo.rs`)에만 있고 `interfaces.md` I1엔 없다.** → B가 코드를 안 보면 모르고, 다른 prefix를 쓰면(예: 잘못 알려진 `zd1:`) A1이 텍스트 memo를 못 읽어 **언락 실패**. 일부 지갑이 raw 바이너리 memo를 거부할 수 있어 이 폴백이 실제로 필요할 가능성이 높다.

**어디.** `week7/drop/team/interfaces.md` I1(문서). 코드(`memo.rs`)는 이미 구현·테스트됨 — **변경 거의 없음, 문서화 + 결정이 핵심.**

**제안 — I1에 추가할 내용:**
```
I1 메모 — 2형식 (A1이 둘 다 디코드)
(raw)         drop_id(8, u64 BE) ‖ e_pub(32) = 40B
(텍스트 폴백) "A1B64:" + base64url_nopad(위 40B)     // UTF-8 memo만 되는 지갑용

ZIP-321 memo= 파라미터 = base64url_nopad(온체인 memo 바이트)
  raw형:    온체인 memo = 40 raw 바이트     → memo= = b64url(40B)
  텍스트형: 온체인 memo = ASCII "A1B64:..."  → memo= = b64url(utf8("A1B64:..."))

기본형식: (Zashi 실측 후 결정 — B가 데모 폰으로 raw 40B 바이너리 통과 테스트.
          통과하면 raw 기본, 떨구면 텍스트 기본.)
prefix "A1B64:" — 고정(freeze).

교차구현 test vector (B의 base64url이 A1과 바이트 일치하는지 검증):
  encode_text_memo(drop_id=1, e_pub=[0,1,2,...,31])
    == "A1B64:AAAAAAAAAAEAAQIDBAUGBwgJCgsMDQ4PEBESExQVFhcYGRobHB0eHw"
```

**완료 기준.** `interfaces.md` I1에 두 형식·ZIP-321 wrapping·prefix·기본형식·test vector가 적혀 있음. B는 그 문서만 보고 인코더 구현 가능.

**B쪽 후속(요청 아님).** B가 데모 폰·데모 Zashi 빌드로 raw 40B 바이너리 memo가 byte-identical로 실리는지 실측(B의 M6) → 결과를 A1과 공유해 기본형식 확정.

---

## R-A2-4 — 공개 버킷을 인덱서(TEE 호스트)에서 분리 (설계 정합성 · 프라이버시)

**무엇.** buyer가 읽는 모든 것(카탈로그 · content blob · dispatch blob)을 **인덱서가 직접 서빙하지 말고**, buyer가 폴링하는 **별도 dumb 저장소**(S3/CloudFront · Blossom/NIP-96)에 둔다. TEE/인덱서·A1은 거기에 **쓰기(put)만**, buyer는 거기서 **읽기(get)만**.

**왜 — 원래 설계와 다르다.**
- 원래 설계: buyer는 **버킷만** 폴링하고 **TEE와 직접 통신하지 않는다**(oblivious "눈 가린 우체부" — project-scope §2, spec §7.3). dispatch를 버킷 경유로 하는 이유 *자체가* "buyer↔TEE 연결을 만들지 않아 TEE가 buyer를 못 보게" 하려는 것이다.
- 현재 구현: 버킷 = 인덱서와 **같은 호스트**. `server.rs`가 `/catalog`·`/bucket/:key`·(R-A2-1 후)`/dispatch`를 전부 같은 axum 서버에서 서빙한다. → **buyer가 카탈로그를 받고 dispatch를 폴링할 때마다 TEE 호스트에 접속 → TEE 호스트가 buyer IP·폴링 패턴을 본다.**
- 결과: "honest-but-curious 서버도 누가 뭘 샀는지 모른다"는 핵심 속성이 **네트워크 레이어에서 약화**된다. spec §7.3이 "buyer IP 상관"을 out-of-scope 한계로 적었는데, 버킷을 분리하면 *줄일 수 있던* 그 표면을 인덱서와 합치며 오히려 키운다(TEE가 polling IP를 직접 봄).

**현재 B 구현 상태.** `buyer/src/api.ts`가 카탈로그·dispatch·content를 **단일 `indexerUrl`** 한 곳에서 읽는다(`App.tsx` 기본 `http://localhost:8080`). 즉 B의 읽기 surface 전체가 인덱서를 가리킨다 — 위 구현과 일치(데모용).

**데모 vs 운용.**
- **데모**: 인덱서가 버킷 겸함 OK(A2 §f "데모는 로컬파일/S3 중 단순한 쪽"). 당장 차단 아님.
- **운용**: blob·카탈로그를 **enclave 호스트 밖**(CDN/Blossom)에 두고 buyer가 *그걸* 폴링. 인덱서/A1은 거기에 put만 → buyer 읽기 트래픽을 TEE가 안 본다.

**제안.**
- **A2(인프라)**: blob·카탈로그 저장을 외부 객체 저장소로. put 경로만 인덱서/A1 안. buyer 공개 읽기 URL = 그 저장소. (카탈로그는 R2 서명과 함께 두면 변조 방지까지.)
- **B(config 분리, 소)**: buyer 읽기 base를 인덱서가 아니라 **버킷**으로. `VITE_DROP_BUCKET_URL`(카탈로그·content·dispatch 읽기) 도입. 데모는 인덱서와 같은 값, 운용 땐 CDN. (`api.ts` 작은 변경.)

**완료 기준.** buyer의 카탈로그·dispatch·content 읽기가 **인덱서(TEE 호스트)가 아닌 별도 저장소**로 가고, 인덱서는 buyer 읽기 트래픽을 보지 않는다. (데모는 동일 호스트 허용하되 config로 분리 가능한 상태.)

**크기.** 운용 인프라 변경(중) + B config 분리(소). **데모 차단 아님 — 설계 정합성/프라이버시 항목.**

---

## 부록 — 근거 코드 위치

| 항목 | 위치 (브랜치) |
|---|---|
| 라우트(단건만, list 미노출) | `server.rs:48-58` (master) |
| `list()` 존재·테스트 | `bucket.rs:49-56`, `:73` (master) |
| `CatalogEntry`(deposit_addr 없음) | `lib.rs:51-57`, `catalog.rs:28-33` (master) |
| `ProvisionPayload`(deposit_addr 없음) | `lib.rs:42-48`, `provision.rs:3` (master) |
| `valid_key` hex-only | `bucket.rs:26-28` (master) |
| memo 2형식·`A1B64:` | `memo.rs` `TEXT_MEMO_PREFIX`, `encode_text_memo`, `decode_memo` (origin/a1) |
| dispatch blob(K_drop만, 80B, blake2b 키) | `dispatch.rs` `wrap_k_drop`, `blob_key` (origin/a1) |
| 버킷=인덱서 동일 호스트(분리 안 됨) | `server.rs:48-58`(catalog·bucket·dispatch 동일 서버) · B `api.ts` 단일 `indexerUrl` |

> 별개(이번 요청 아님, 인지용): 카탈로그가 in-memory라 재시작 시 전 drop 소실 → 재-provision 필요(`catalog.rs:9-13` 주석에 명시). 데모 범위로 수용, production은 영속 필요.
