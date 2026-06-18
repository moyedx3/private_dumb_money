# 레인 B — 구매자 웹앱 (+ 데모/지갑 로지스틱스) · "Web #1"

> 옆 문서들 먼저: [`00-overview.md`](./00-overview.md)(큰 그림), [`interfaces.md`](./interfaces.md)(주고받는 데이터의 **정확한 바이트 모양** — 여기 적힌 번호 I1·I2·I3-a·I4가 전부 거기서 옴). 정확한 값은 항상 `interfaces.md`가 정답이다.

---

## 1. 한 줄 요약

**구매자가 보는 화면 전부.** 공개 카탈로그를 띄우고 → 구매할 때마다 일회용 X25519 키쌍을 새로 만들고 → 그 공개키를 메모에 실은 **ZIP-321 결제 QR**을 그려서 Zashi로 결제하게 하고 → 버킷을 폴링하며 새 꾸러미(dispatch blob)를 내 개인키로 하나씩 열어보다가 **열리는 것**에서 콘텐츠 열쇠 `K_drop`을 꺼내고 → 콘텐츠 blob을 받아 복호화해 화면에 띄운다. 스파이크 #1(이 레인의 핵심 — Zashi가 ZIP-321 메모를 실어 보내는가)은 **진짜 폰에서 이미 통과**했다.

> **낯선 단어?** Zcash 용어(ZIP-321, 가려진 주소, 메모, sealed box)는 이 문서 맨 아래 [부록 — 용어 표](#부록--용어-표)에 한 줄씩 정리해뒀다. 본문은 비유와 그림 위주로 간다.

**비유 한 컷.** 너는 *자판기 앞 화면*을 만든다. 손님이 동전을 넣는 행위(결제)는 옆 기계(Zashi)가 하고, 물건(열쇠)은 게시판에 누가 슬쩍 붙여놓는다. 네 일은 (a) 메뉴를 보여주고 (b) "이 손님 거"라고 표시할 **일회용 자물쇠 한 쌍**을 즉석에서 깎아서 그 *공개* 자물쇠를 결제 쪽지에 끼워 보내고 (c) 게시판에 붙은 꾸러미들을 네가 가진 *비밀* 열쇠로 하나씩 따보다가 **딱 맞아 열리는 것**을 찾아 물건을 꺼내는 것이다.

---

## 2. 큰 그림에서 내 위치

```
                          [ 내 레인 B = 구매자 브라우저 ]
   ┌───────────────────────────────────────────────────────────────────────┐
   │                                                                         │
   │  (a) 카탈로그            (c) ZIP-321 QR        (d) 버킷 폴링 +          │
   │      페이지   ──────▶       + 결제   ──────▶      trial-open  ──────▶    │
   │  I3-a 통째로 fetch       memo=base64url        각 blob을 e_priv로       │
   │  (드롭별 호출 없음)       (drop_id‖e_pub)        seal_open → K_drop      │
   │      │                      │                       │                   │
   │      │ 보여주기            │ 화면에 QR             │ 열린 1개에서        │
   │      ▼                      ▼                       ▼  K_drop 획득       │
   │  [목록 렌더]          [손님이 Zashi로 스캔]    (e) h_content로          │
   │                                                  콘텐츠 blob fetch       │
   │                                                  → AES-256-GCM 복호화    │
   │                                                  → [콘텐츠 렌더] 🎉      │
   └───────────────────────────────────────────────────────────────────────┘
        ▲                         │                          ▲
        │ I3-a (공개 카탈로그)    │ 가려진 tx (메모 운반)    │ I2 dispatch blob
        │                         ▼                          │ I4 content blob
   [버킷/카탈로그]          [Zcash 체인]               [공개 버킷]
                                 │                          ▲
                                 └──▶ [TEE 서버 A1] ─── 결제 감지 → 열쇠 포장 → blob 게시
```

핵심 흐름 한 줄: **catalog → QR/pay → poll → decrypt → render.** 서버(A1) 쪽은 점선 안쪽이라 네가 신경 쓸 필요 없다 — 너는 체인에 메모를 띄우고, 버킷에서 blob을 받는 두 접점만 본다.

---

## 3. 내가 받는 것 / 내보내는 것

| 방향 | 무엇 | 인터페이스 | 모양 (정확값은 `interfaces.md`) |
|---|---|---|---|
| **받음(read)** | 공개 카탈로그 | **I3-a** | `{ "drop_id": 1, "price_zec": "0.01", "h_content": "<버킷 키>", "title": "..." }` 의 목록(JSON) |
| **내보냄(produce)** | 메모 | **I1** | `drop_id(8, big-endian u64) ‖ e_pub(32, X25519)` = **40B raw**. ZIP-321 URI 안엔 이 40B를 **base64url(패딩 없음)** 한 문자열로 `memo=`에 넣는다. |
| **받음(consume)** | dispatch blob | **I2** | 버킷에 올라오는 **libsodium sealed box** = `crypto_box_seal(K_drop, e_pub)` = `ek_pub(32) ‖ ct+MAC(48)` = **80B** |
| **받음(consume)** | content blob | **I4** | 버킷의 암호화 콘텐츠 = `nonce(12) ‖ AES-256-GCM ciphertext ‖ tag(16)` |

- **나는 카탈로그를 만들지 않는다** — C(크리에이터)가 등록하고 A2가 게시한다. 나는 *읽기만* 한다(I3-a, 공개 JSON. 내부 DropConfig인 I3-b는 enclave 안에만 있고 절대 안 보인다).
- **나는 dispatch blob을 만들지 않는다** — A1이 만들어 버킷에 올린다. 나는 *열어보기만* 한다.
- **내가 유일하게 체인/서버로 내보내는 것은 I1 메모뿐이다** — 그것도 직접 보내는 게 아니라, QR로 그려서 손님의 Zashi가 대신 체인에 실어 보낸다. (즉 내 코드가 Zcash 트랜잭션을 만들지 않는다. 그래서 가볍다.)

---

## 4. 만드는 것 (단계별)

> 라이브러리: 암호는 **`libsodium-wrappers`**(브라우저 WASM). 서버 A1의 Rust **`dryoc`**와 같은 Curve25519 sealed box라 바이트 호환된다 — 한쪽이 `crypto_box_seal`로 싸면 다른 쪽이 `crypto_box_seal_open`으로 푼다. AES-256-GCM은 브라우저 내장 **WebCrypto**(`crypto.subtle`)면 충분. QR은 가벼운 QR 라이브러리 하나(예: `qrcode`).

### (a) 카탈로그 페이지 — **통째로 한 번에** 가져온다

- 앱이 뜰 때 공개 카탈로그(I3-a) **전체**를 *한 번* fetch해서 클라이언트에서 렌더한다. `drop_id`별 엔드포인트를 **만들지 않는다.**
- **왜 통째로?** 드롭별로 호출하면 "이 IP가 드롭 #7만 조회했다"는 *카탈로그 열람 지문*이 서버 로그에 남는다(spec §7.3). 전부 한 번에 받으면 누가 무엇에 관심 있는지 서버가 모른다. 프라이버시 제품이라 이건 협상 불가.
- 목록 아이템: `title` + `price_zec` 표시, 클릭하면 (c)의 결제 화면으로. `h_content`는 화면엔 안 띄우고 (e)에서 콘텐츠 가져올 때 쓰려고 들고만 있는다.

### (b) 구매마다 일회용 키쌍 — `crypto_box_keypair` (X25519)

```js
await sodium.ready;
const { publicKey: e_pub, privateKey: e_priv } = sodium.crypto_box_keypair();
// e_pub: 32B (메모에 들어감), e_priv: 32B (절대 안 나감, blob 열 때만 씀)
```

- **구매 1건당 새로** 만든다. 절대 재사용 금지 — 재사용하면 서로 다른 구매가 같은 키로 묶여 프라이버시가 깨진다(spec §5: "Reused ephemeral keys would break this").
- **메모리에 보관**이 기본. 탭이 살아있는 동안만 `e_priv`를 들고 있는다.
- (선택) `IndexedDB`에 **24시간** 보관 옵션 — 탭을 닫거나 새로고침해도 결제가 살아남게(§7.3). 보관하면 만료 타이머도 같이. 안 하면 §8의 "탭 닫으면 구매 날아감" 함정을 사용자에게 반드시 경고.

### (c) ZIP-321 URI 만들고 QR 렌더

목표 URI 모양:
```
zcash:<deposit_addr>?amount=<ZEC>&memo=<base64url( drop_id(8) ‖ e_pub(32) )>
```

만드는 법:
```js
// 1) 40바이트 메모 raw 조립: drop_id(u64 big-endian) ‖ e_pub(32)
const memo = new Uint8Array(40);
new DataView(memo.buffer).setBigUint64(0, BigInt(drop_id), false); // false = big-endian
memo.set(e_pub, 8);

// 2) base64url, 패딩 없음 (sodium 사용 — 손으로 +/= 치환하다 틀리지 말 것)
const memo_b64url = sodium.to_base64(memo, sodium.base64_variants.URLSAFE_NO_PADDING);

// 3) ZIP-321 URI. amount는 ZEC 단위 문자열(카탈로그 price_zec 그대로), addr은 가려진(shielded) deposit 주소
const uri = `zcash:${deposit_addr}?amount=${price_zec}&memo=${memo_b64url}`;
```
- 이 `uri`를 QR 라이브러리로 그려서 화면에 띄운다. 끝. 손님이 Zashi로 스캔하면 주소·금액·메모가 다 채워진다(스파이크 #1에서 확인).
- **deposit_addr는 반드시 가려진(shielded) 주소** — 투명(transparent) 주소면 메모가 통째로 사라진다(§8 1번 함정). 카탈로그가 주는 주소가 가려진 주소인지 확인.
- `drop_id`/`e_pub` 자르기·붙이기 순서는 **I1과 글자 하나까지 동일**해야 A1이 읽는다.

### (d) 버킷 폴링 + 각 blob을 `crypto_box_seal_open` (= "맞는 열쇠 찾기")

- 결제 후, 버킷에 새로 올라오는 dispatch blob들을 주기적으로 폴링한다(예: 3~5초 간격, ~30초 내 언락 목표).
- **버킷 키에 내 식별자가 안 들어간다** — blob 파일명은 `blake2b256(ek_pub ‖ txid)`라 "내 거"를 이름으로 골라낼 수 없다(프라이버시). 그래서 **새로 올라온 blob을 전부 받아 하나씩 따본다**:
```js
// blob: 80B sealed box. 내 (e_pub, e_priv)로 열리면 그게 내 K_drop, 아니면 null
const opened = sodium.crypto_box_seal_open(blob, e_pub, e_priv); // 실패 시 throw → try/catch
if (opened) { /* opened == K_drop (32B). 찾았다! */ }
```
- 동작 원리(직관): sealed box는 "**받는 사람 공개키로만** 열리는 봉인 봉투"다. A1이 *내 e_pub*으로 싼 봉투만 *내 e_priv*로 열린다. 남의 봉투는 `seal_open`이 실패(throw)하므로 `try/catch`로 넘기고 다음 걸 시도한다. 열리는 1개 = 내 구매에 대한 응답.
- 이미 따본 blob은 다시 안 따게 캐싱. 열린 순간 폴링 멈추고 (e)로.

### (e) 콘텐츠 blob fetch → AES-256-GCM 복호화 → 렌더

```js
// 1) (a)에서 들고 있던 h_content로 콘텐츠 blob 받기 (I4)
const blob = new Uint8Array(await (await fetch(bucketUrl(h_content))).arrayBuffer());

// 2) nonce(12) ‖ ciphertext+tag 로 자르고 WebCrypto로 복호화. GCM은 tag(16)가 ct 뒤에 붙어있는 형식
const nonce = blob.slice(0, 12);
const ctWithTag = blob.slice(12); // ciphertext ‖ tag(16) — WebCrypto가 끝 16B를 tag로 처리
const key = await crypto.subtle.importKey("raw", opened /* =K_drop */, "AES-GCM", false, ["decrypt"]);
const plaintext = new Uint8Array(
  await crypto.subtle.decrypt({ name: "AES-GCM", iv: nonce, tagLength: 128 }, key, ctWithTag)
);
// 3) plaintext를 콘텐츠 타입에 맞게 렌더(이미지면 Blob URL, 텍스트면 그대로 등)
```
- `K_drop`은 (d)에서 열린 `opened` 32바이트 그대로. AES-256이라 키도 32바이트로 딱 맞는다.
- 복호화가 실패(예외)하면 → 잘못된 blob을 K_drop으로 썼거나 콘텐츠가 손상. 사용자에게 명확한 에러.

---

## 5. 재사용 (바닥부터 안 짠다)

- **스파이크 #1이 이미 증명한 경로** — "Zashi가 ZIP-321 QR의 `memo=`를 실제로 체인에 실어 보낸다." (진짜 폰, mainnet, 메모 `spike12|drop=1|epub=TESTKEY`가 그대로 실림, 0.0001 ZEC, txid `ae11a454…`.) (c)의 결제 화면은 이 검증된 레시피를 *그대로* 따른다.
- **정확한 ZIP-321 + base64url-메모 레시피**는 `spikes.md`(#1 스텝)에 박제돼 있다: `zcash:<shielded_addr>?amount=...&memo=<base64url>`, 메모는 512B 미만(우린 40B). 그 문서가 (c)의 사실상 의사코드다.
- **UFVK/UA 생성 도구 재사용** — 스파이크에서 쓴 `week5/clean-wallet-mvp/apps/scanner/src/bin/gen-ua.rs`(또는 `gen-ufvk.rs`)로 테스트용 수신 주소를 만든다. (테스트할 때 "내가 보낸 메모가 진짜 실렸나"를 확인하는 수신 측 주소.)
- **QR 라이브러리** 하나(예: `qrcode`) — QR은 직접 구현하지 말 것.
- **암호 라이브러리** `libsodium-wrappers`(키쌍·sealed box·base64url) + WebCrypto(AES-GCM). 서버 `dryoc`와 바이트 호환.

---

## 6. 테스트하는 법

**(1) 스파이크 #1 연쇄 테스트 재현 (실제 메모 경로 확인 — 가장 중요)**
1. `gen-ua`/`gen-ufvk`로 내가 읽을 수 있는 **가려진** UA(수신용 B)를 하나 만든다.
2. (c) 코드로 ZIP-321 URI를 만들어 QR을 그린다 — 단, `e_pub` 자리는 진짜 키쌍에서 뽑은 값으로(스파이크처럼 `TESTKEY` 더미여도 경로 확인엔 충분).
3. **데모에 쓸 바로 그 폰 + 바로 그 Zashi 빌드**로 QR을 스캔 → 주소·금액·**메모**가 다 채워지는지 compose 화면에서 눈으로 확인 → 결제(mainnet, ~0.0001 ZEC).
4. 수신 측에서 받은 tx의 메모를 열어 **내가 인코딩한 40B와 byte-identical**인지 확인. (Zashi 두 번째 지갑이면 받은 메모를 바로 보여준다.)
   - **PASS:** 메모가 내가 넣은 것과 바이트 단위로 일치.
   - **FAIL 신호:** Zashi가 QR의 ZIP-321을 거부 / 주소·금액은 채우는데 **메모를 떨군다** / 수신자를 투명으로 오인해 메모 비활성화 / base64url을 깨뜨림. → 이건 (c)가 죽는다는 뜻이니 즉시 팀에 보고(§8 마지막 항목).

**(2) 버킷을 mock으로 막아두고 (d)·(e) 단위 테스트 (A1 없이 혼자)**
- A1이 아직 없으니, **캔 dispatch blob**을 직접 만들어 버킷을 흉내 낸다:
  ```js
  // 테스트 픽스처: 내가 아는 K_drop을 내 e_pub으로 싸서 가짜 blob 생성
  const K_drop = sodium.randombytes_buf(32);
  const fakeBlob = sodium.crypto_box_seal(K_drop, e_pub);      // 이걸 버킷 응답으로 흉내
  // → (d)가 이 blob을 seal_open해서 같은 K_drop을 복원하는지 검증
  ```
- 콘텐츠 쪽(I4)도 마찬가지로 캔 데이터를 만든다: 평문 → `AES-256-GCM(K_drop)` → `nonce ‖ ct ‖ tag` 로 조립해 버킷 응답으로 주고, (e)가 같은 평문을 복원하는지 확인. (이건 C 레인이 만드는 형식이라, C와 같은 바이트 레이아웃으로 픽스처를 맞춘다.)
- **음성/엣지:** 남의 blob(다른 키로 싼 것)을 섞어 넣어 (d)가 그건 조용히 건너뛰고 *내 것만* 여는지; 잘못된 K_drop으로 (e)가 깔끔히 에러 내는지; 탭 새로고침 후 `IndexedDB` 경로가 `e_priv`를 복구하는지.

**(3) A1이 라이브되면** mock을 실 버킷으로 바꿔 (1)→(d)→(e) 전체를 한 번에: 팀원이 결제 → 내 브라우저에서 ~30초 내 자동 언락.

---

## 7. 완료 기준 (Definition of Done)

- [ ] **카탈로그**: 앱 로드 시 I3-a **전체를 한 번** fetch해 목록 렌더. 드롭별 엔드포인트 호출 **없음**(§7.3 준수).
- [ ] **키쌍**: 구매 1건당 `crypto_box_keypair`로 새 X25519 키쌍 생성, `e_priv`는 메모리(또는 옵트인 `IndexedDB` 24h)에만. 재사용 절대 없음.
- [ ] **QR/결제**: `zcash:<shielded_addr>?amount=...&memo=<base64url(drop_id‖e_pub)>` URI를 만들고 QR 렌더. 메모 40B 레이아웃이 **I1과 바이트 일치**.
- [ ] **폴링/언락**: 새 blob들을 폴링해 각각 `crypto_box_seal_open(_, e_pub, e_priv)` → 열리는 1개에서 `K_drop` 복원. 남의 blob은 조용히 건너뜀.
- [ ] **복호화/렌더**: `h_content`로 콘텐츠 blob을 받아 `nonce ‖ AES-256-GCM ‖ tag`를 `K_drop`으로 복호화해 화면에 렌더.
- [ ] **스파이크 #1 재현**: 데모용 폰+Zashi 빌드에서 메모가 byte-identical로 체인에 실리는 것을 *직접* 확인(§6-(1) PASS).
- [ ] **mock 검증**: A1 없이 캔 blob/콘텐츠로 (d)·(e) 단위 테스트 통과.
- [ ] **데모 로지스틱스**: 자금 충전된 mainnet 테스트 지갑들 준비 + **데모 런북** + **폴백 스크린캐스트**(라이브가 깨질 때 틀 영상)까지 손에 있음.
- [ ] **함정 경고 UX**: §8의 "탭 닫으면 구매 소실", "가려진 주소만" 경고가 사용자 흐름에 노출됨.

---

## 8. 주의 / 함정

1. **메모는 가려진(shielded) 수신자에게만 살아남는다.** 투명(transparent: `t1`/`t3`) 주소로 보내면 Zashi가 메모를 **통째로 떨군다** → A1이 `drop_id`/`e_pub`를 절대 못 받음 → 영원히 언락 안 됨. 카탈로그가 주는 `deposit_addr`가 **반드시 Sapling/Orchard 가려진 주소**인지 확인하고, QR 만들 때 한 번 더 가드. (spec §1: "Payment is to a shielded address (required).")

2. **일회용 키는 단발·복구 불가.** `e_priv`를 잃으면 그 구매는 **영구 소실**된다(돈은 나갔는데 열쇠를 못 받음). 탭을 닫거나 새로고침하면 메모리의 `e_priv`가 날아간다(spec §1 non-goals, §7.3).
   - **완화:** (a) `IndexedDB` 24h 보관 옵션으로 새로고침/탭 닫힘에 견디게; (b) blob 보관창(버킷이 blob을 얼마간 들고 있게)으로 늦게 돌아와도 따게; (c) 결제 *전에* "이 탭/브라우저를 닫지 마세요 — 닫으면 구매가 날아갑니다" 명확 경고; (d) 옵션으로 `e_priv` 복구 파일 내보내기. **최소한 (c)는 필수.**

3. **네트워크 계층 상관관계는 문서화된 out-of-scope 한계.** 구매자 IP가 버킷을 폴링하고 Zashi가 tx를 브로드캐스트하는 시점/IP를 같은 관찰자가 보면, *체인 밖에서* 둘을 엮을 여지가 있다(Tor/믹스넷 없이는 못 막음). 이건 **데모 범위에서 의도적으로 안 막는다** — 위협 모델에 적힌 알려진 한계이지 버그가 아니다(spec §7.3, §1 non-goals). UI/문서에 한 줄 명시.

4. **데모에 쓸 바로 그 Zashi 빌드/기기에서 메모 경로를 검증.** 스파이크 #1은 특정 빌드·OS에서 통과했다. Zashi 버전이 바뀌면 ZIP-321/메모 처리가 또 달라질 수 있다(`zcash:` 처리에 역사적 공백 있었음 — spikes.md #1). **데모 직전, 실제 데모 폰의 실제 Zashi 빌드로** §6-(1)을 다시 한 번 PASS시켜라. 버전·OS를 런북에 적어둘 것.

5. **base64url은 손으로 치환하지 말 것.** 패딩 유무, `+/`↔`-_` 치환을 직접 하다 한 글자 틀리면 A1이 메모를 못 읽는다. `sodium.to_base64(..., URLSAFE_NO_PADDING)`만 써라. (interfaces.md I1: "base64url(패딩 없음)".)

6. **바이트 호환 깨짐 주의.** sealed box(I2)·콘텐츠(I4) 모두 서버/크리에이터(Rust `dryoc`, AES-GCM)와 **같은 곡선·같은 tag 위치·같은 nonce 길이**여야 한다. 단위 테스트는 반드시 *상대편이 만들 바이트와 동일한 픽스처*로 할 것(§6-(2)). 의심되면 `interfaces.md`가 정답이고, 거기서 어긋나면 그 문서를 고치고 팀 공지.

---

## 부록 — 용어 표

| 용어 | 한 줄 뜻 |
|---|---|
| **ZIP-321** | Zcash 결제 요청을 `zcash:<주소>?amount=...&memo=...` URI로 표준화한 규격. QR로 만들면 지갑이 스캔해 폼을 자동으로 채운다. |
| **가려진(shielded) 결제** | 보내는 사람·금액이 체인에 안 드러나는 Zcash 결제(Sapling/Orchard 풀). 우리 프라이버시의 뿌리. 반대는 투명(transparent) 결제. |
| **메모(memo)** | 가려진 결제에 붙이는 512바이트 쪽지(ZIP-302). 우린 여기에 40B(`drop_id‖e_pub`)를 raw로 싣는다. **투명 결제엔 메모가 없다.** |
| **e_pub / e_priv** | 구매 1건마다 새로 만드는 일회용 X25519 키쌍. 공개키는 메모에 실어 보내고, 개인키는 절대 안 나가며 blob 열 때만 쓴다. |
| **sealed box** | "받는 사람 공개키로만 열리는 봉인 봉투"(libsodium). A1이 `crypto_box_seal(K_drop, e_pub)`로 싸고, 나는 `crypto_box_seal_open`으로 푼다. |
| **dispatch blob (I2)** | A1이 `K_drop`을 내 `e_pub`으로 싼 80B 꾸러미. 버킷에 올라온다. 내 개인키로만 열린다. |
| **content blob (I4)** | 크리에이터가 콘텐츠를 `K_drop`으로 AES-256-GCM 암호화한 것. `nonce(12)‖ct‖tag(16)`. 서버는 절대 못 본다. |
| **K_drop** | 콘텐츠를 잠근 32바이트 마스터 열쇠(AES-256). blob을 열면 이게 나오고, 이걸로 콘텐츠를 푼다. |
| **버킷(bucket)** | blob들이 올라가는 공개 저장소(S3/Blossom 등). 파일명에 구매자/드롭 식별자가 안 들어간다(프라이버시). |
| **trial-open / 폴링** | 버킷의 새 blob을 전부 받아 내 키로 하나씩 열어보다 "열리는 1개"를 찾는 것. 내 blob을 이름으로 못 고르니 이렇게 한다. |
| **libsodium-wrappers / dryoc** | 각각 JS·Rust의 libsodium 호환 암호 라이브러리. 같은 Curve25519 sealed box라 바이트 단위로 호환된다. |
| **Zashi** | 구매자가 결제에 쓰는 모바일 Zcash 지갑. QR을 스캔해 가려진 결제+메모를 만든다(스파이크 #1에서 검증). |
