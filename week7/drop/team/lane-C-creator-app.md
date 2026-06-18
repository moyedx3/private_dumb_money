# 레인 C — 크리에이터 대시보드 + 인증/프로비저닝 클라이언트 (web)

> **담당: Web #2 (crypto-comfortable).** 신뢰 핸드셰이크의 **클라이언트 쪽 절반**이다. 서버 쪽 절반은 A2가 만든다(`lane-A2-enclave-platform.md`).
> 정확한 데이터 모양은 항상 [`interfaces.md`](./interfaces.md) 의 I3·I4·I5·I6을 본다. 이 문서는 "왜/어떻게"고, 숫자/필드의 정본(canonical)은 거기다.

---

## 1. 한 줄 요약

크리에이터가 콘텐츠를 **새 열쇠 `K_drop`으로 암호화**해 버킷에 올리고(I4), 서버가 보낸 **TEE 인증서(quote)를 브라우저에서 직접 검증**한 뒤(I6), 그 검증된 enclave 공개키로 `{K_drop, viewing key, 메타데이터}`를 **봉인(sealed box)해서 넣는다**(I5 provisioning). 마지막에 공개 카탈로그 엔트리를 등록한다(I3-a).

---

## 2. 큰 그림에서 내 위치

내가 하는 일은 딱 세 동작이다. 순서가 중요하다 — **②를 통과해야만 ③을 할 수 있다.**

```
   크리에이터 브라우저 (레인 C = 나)
   ┌──────────────────────────────────────────────────────────┐
   │ ① 콘텐츠 암호화 + 업로드                                    │
   │    K_drop=random32 → AES-256-GCM → blob ──────────────────┼──▶ 공개 버킷 (I4)
   │    h_content = sha256(blob)                                │     이름 = h_content
   │                                                            │
   │ ② 서버 검증  ◀──── GET /attest (quote_hex) ────────────────┼──── A2 (I6)
   │    - quote가 진짜 Intel TDX 서명인가? (dcap-qvl / t16z)      │
   │    - 측정값(Mrtd/Rtmr)이 공개 재현빌드 해시와 같은가?         │
   │    - report_data에서 enclave 공개키를 꺼낸다                 │
   │              │ 통과해야만 ↓                                 │
   │ ③ 비밀 넣기 (seal secret IN)                                │
   │    crypto_box_seal({drop_id,price,k_drop,ufvk,h_content},  │
   │                    enclave_pubkey) ───────────────────────┼──▶ POST /provision (I5) → A2
   │                                                            │
   │ ④ 공개 카탈로그 등록 ──────────────────────────────────────┼──▶ 카탈로그 (I3-a) → B가 봄
   └──────────────────────────────────────────────────────────┘
```

**큰 그림 한 컷(remote attestation이 처음이면 이것만 기억):** 나는 서버를 **신뢰하지 않는다.** 대신 서버가 내미는 하드웨어 서명 영수증(quote)을 받아, *그게 진짜 Intel 칩이 서명했고 + 내가 깃허브에서 본 그 코드 그대로 돌고 있다*는 걸 **내 브라우저에서 수학적으로 확인**한다. 그게 확인된 다음에야 비밀(`K_drop`)을 그 enclave만 열 수 있게 포장해 보낸다. 검증이 이 레인의 **유일한 load-bearing 단계**다(§8 첫 항목).

---

## 3. 내가 받는 것 / 내보내는 것

| 방향 | 무엇 | 상대 | 형식 (정본: interfaces.md) |
|---|---|---|---|
| **받음** | `GET /attest` → attestation | A2 | **I6**: `{ quote_hex, event_log, vm_config }`. quote 안에 `report_data[0..32] = sha256(enclave_provisioning_pubkey)`, `Mrtd`/`Rtmr` = 코드 측정값 |
| 내보냄 | 콘텐츠 blob (버킷에 PUT) | 버킷 → B | **I4**: `nonce(12) ‖ AES-256-GCM ciphertext ‖ tag(16)`, 키(파일이름) = `h_content = sha256(blob)` hex |
| 내보냄 | provisioning sealed-box (`POST /provision`) | A2 | **I5**: `crypto_box_seal({drop_id, price_zat, k_drop, creator_ufvk, h_content}, enclave_pubkey)` (payload는 CBOR/JSON) |
| 내보냄 | 공개 카탈로그 엔트리 | 카탈로그 → B | **I3-a**: `{ "drop_id", "price_zec", "h_content", "title" }` (구매자 목록에 노출됨) |

> 단위 함정: I3-a는 사람이 보는 `price_zec`(문자열 "0.01"), I5/I3-b는 enclave가 계산에 쓰는 `price_zat`(u64 zatoshi, 1 ZEC = 100,000,000 zat). **같은 가격을 두 단위로** 내보낸다 — 변환 한 곳에서 하고 테스트로 고정.

---

## 4. 만드는 것 (단계별)

스택: **TypeScript + 브라우저.** sealed box는 `libsodium-wrappers`, AES-GCM은 WebCrypto(`crypto.subtle`), quote 검증은 `@phala/dcap-qvl-web`(WASM) 또는 clean-wallet의 t16z 경유 검증(§5). 모든 비밀 처리는 **브라우저 안에서** 한다 — `K_drop`은 서버는 물론 내 백엔드에도 평문으로 보내지 않는다.

### (a) `K_drop` 생성 + 콘텐츠 암호화 → 업로드, `h_content` 계산

```ts
// 1. 드롭마다 새 마스터 키. 절대 재사용 금지(재사용 시 과거+미래 구매분 전부 노출).
const k_drop = crypto.getRandomValues(new Uint8Array(32));   // AES-256 키

// 2. AES-256-GCM. nonce는 12바이트 랜덤(드롭당 1회만 쓰는 키이므로 1회면 충분).
const nonce = crypto.getRandomValues(new Uint8Array(12));
const key = await crypto.subtle.importKey("raw", k_drop, "AES-GCM", false, ["encrypt"]);
const ct  = new Uint8Array(
  await crypto.subtle.encrypt({ name: "AES-GCM", iv: nonce }, key, plaintext)
);                                 // WebCrypto는 ciphertext 끝에 16바이트 tag를 붙여서 반환

// 3. I4 와이어 포맷: nonce(12) ‖ ciphertext ‖ tag(16)
const blob = new Uint8Array(12 + ct.length);
blob.set(nonce, 0); blob.set(ct, 12);

// 4. 버킷 키 = h_content. 이게 카탈로그의 h_content이기도 하다.
const h_content = toHex(new Uint8Array(await crypto.subtle.digest("SHA-256", blob)));
await bucketPut(h_content, blob);  // S3 presigned PUT 등
```

- 구매자 B는 `K_drop`을 받은 뒤 **같은 분해**(앞 12 = nonce, 뒤 16 = tag)로 복호화한다. 이 분해 규칙이 B와 정확히 일치해야 한다 → interfaces.md I4가 정본.
- 큰 파일이면 `crypto.subtle.digest`에 통째로 넘기지 말고 청크로 끊거나 worker로. (데모 규모면 그냥 통째로 OK.)

### (b) attestation 검증기: `GET /attest` → quote 검증 (이게 핵심)

remote attestation을 처음 보는 사람을 위한 3줄: **quote**는 Intel 칩이 서명한 영수증이다. 안에 "지금 이 VM에 올라간 코드의 해시(`Mrtd`/`Rtmr`)"와 "앱이 자유롭게 채워 넣는 64바이트 칸(`report_data`)"이 들어 있다. 우리는 이 칸에 `sha256(enclave_provisioning_pubkey)`를 박아둔다(A2가 quote 생성 시 그렇게 함). 그래서 quote 하나로 **"이 코드가 돌고 있다 + 이 공개키는 그 코드 것이다"** 가 한꺼번에 증명된다.

검증은 **세 개를 전부** 통과해야 한다(clean-wallet의 3-check 모델 그대로):

```ts
const att = await fetch(`${SERVER}/attest`).then(r => r.json());   // I6: { quote_hex, ... }

// Check 1 — 서명이 진짜 Intel TDX 루트까지 체인되는가?  (dcap-qvl WASM 또는 t16z)
//   ← 이걸 통과 못하면 그냥 누가 흉내 낸 quote다. 절대 다음으로 넘어가면 안 됨.
const v = await verifyQuote(att);          // §5: clean-wallet verify-quote.ts 재사용
if (!v.ok) throw new Error("Check 1 실패: Intel 서명 아님 → 신뢰 불가");

// Check 2 — 측정값(코드 해시)이 우리가 공개한 재현빌드 해시와 일치하는가?
//   ← report_data만 맞고 측정값이 다르면, "검증된 공개키"가 *우리가 모르는 코드* 것이다 → 무의미.
//   기대 측정값은 A2가 CI에서 재현빌드로 산출해 공개한 값(EXPECTED_MEASUREMENT)을 핀으로 박는다.
if (v.codeMeasurement?.toLowerCase() !== EXPECTED_MEASUREMENT.toLowerCase())
  throw new Error("Check 2 실패: 측정값 불일치 → 우리가 감사한 그 코드가 아님");

// Check 3 — report_data[0..32] 가 정말 enclave 공개키의 해시인가? (= 공개키를 quote에 묶기)
//   report_data는 64바이트(hex 128자); 앞 32바이트만 우리 바인딩. clean-wallet의
//   reportDataBindsArtifact() 가 정확히 reportData[0..32] vs 기대해시(hex)를 비교한다.
const enclavePubkey = att.enclave_provisioning_pubkey;     // I6가 함께 줌 (또는 별 필드)
const expect = sha256_hex(enclavePubkey);                  // libsodium/WebCrypto sha256
if (!reportDataBindsArtifact(v.reportData!, expect))
  throw new Error("Check 3 실패: 공개키가 quote에 묶여있지 않음 → 가짜 공개키 주입 가능");

// 세 개 통과 → 이 enclavePubkey 로만 (c)에서 봉인한다.
```

- **왜 report_data 바인딩이 중요한가:** 이게 없으면 공격자가 *진짜 quote*(Check 1 통과)에다 *자기 공개키*를 끼워 넘길 수 있다. 그러면 나는 검증을 "통과"했다고 믿고 공격자 키로 `K_drop`을 봉인 → 서버 주인이 읽음. Check 3이 "이 공개키 = 이 quote의 코드가 만든 것"을 못 박는다.
- clean-wallet의 `verifyQuote()`는 `Mrtd`/`report_data`를 뽑아 `{ ok, codeMeasurement, reportData }`로 돌려준다. 우리 검증기는 이 위에 Check 2(측정값 핀)와 Check 3(공개키 바인딩)만 얹으면 된다.

### (c) provisioning 클라이언트: sealed box → `POST /provision`

```ts
import sodium from "libsodium-wrappers";
await sodium.ready;

const payload = {                         // I5 payload (CBOR/JSON; 정본 interfaces.md)
  drop_id,                                // u64
  price_zat,                              // u64 zatoshi (I3-a price_zec 에서 변환)
  k_drop: sodium.to_base64(k_drop),       // 32B 마스터 키
  creator_ufvk,                           // UFVK 문자열 (IVK 추출용 viewing key — 절대 spending key 아님)
  h_content,                              // (a)에서 계산한 버킷 키
};
const sealed = sodium.crypto_box_seal(
  encodePayload(payload),                 // CBOR or UTF-8 JSON 바이트
  sodium.from_hex(stripPubkey(enclavePubkey))   // ← Check 1·2·3 통과한 그 키만!
);
await fetch(`${SERVER}/provision`, { method: "POST", body: sealed });   // I5
```

- `crypto_box_seal`은 **상대 공개키로만 열 수 있는 봉투**다. 송신자 키쌍을 매번 즉석에서 만들어 쓰고 버려서(`ek_pub ‖ ciphertext+MAC`) **누가 보냈는지도 안 남는다.** A2의 enclave가 KMS 파생 개인키로 `crypto_box_seal_open` 해야만 평문이 나온다 → 서버 주인은 암호문만 본다(이게 스파이크 #3에서 진짜 하드웨어로 검증된 "encrypt-to-enclave").
- 곡선은 Curve25519. A2(Rust `dryoc`) ↔ 나(`libsodium-wrappers`) 호환됨 (I2/I5 동일).
- **`creator_ufvk`는 viewing key(UFVK)다. spending key는 절대 넣지 않는다** — Phase 1에서 enclave는 들어온 결제를 *볼 수만* 있으면 되고(IVK), 돈은 못 빼야 한다(spec §4.1, 5번 주석).

### (d) 공개 카탈로그 등록 (I3-a)

```ts
await registerCatalog({                   // I3-a: 구매자 B가 목록에서 보는 공개 JSON
  drop_id,
  price_zec,                              // 사람이 보는 단위 "0.01"
  h_content,                              // (a)의 버킷 키 — B가 이걸로 콘텐츠 blob을 받음
  title,                                  // "고양이 사진" 등
});
```

- I3는 "C가 등록, A2가 보관/게시, B가 조회". 등록 트랜스포트(파일/HTTP)는 A2와 합의 — interfaces.md 각주의 미정 선택지(카탈로그 저장소 메모리 vs 파일)를 첫날 같이 못 박는다.
- **여기엔 비밀이 1도 없다.** `K_drop`/UFVK는 (c)의 sealed box로만 나간다.

---

## 5. 재사용

- **clean-wallet의 attestation 검증기 (`week5/clean-wallet-mvp/apps/web/lib/verify-quote.ts`).** `verifyQuote(quote)` = quote를 Intel 체인까지 검증(내부적으로 t16z Trust Center / dcap-qvl 경유)하고 `{ ok, codeMeasurement, reportData }`를 돌려줌. `reportDataBindsArtifact(reportDataHex, hashHex)` = `reportData[0..32]` vs 기대해시 비교 — Check 3에 그대로 쓴다. 타입 `Quote`/`QuoteVerification`도 재사용. README의 "3-check" 모델(Check 1 서명 진위 / Check 2·3 바인딩)이 §4(b)의 설계 그대로다.
- **spike3 attestation 출력 포맷 (`week7/drop/spike3/RUNBOOK.md`).** 진짜 Phala `tdx.small` CVM에서 나온 실제 quote 필드를 그대로 본다: `Mrtd f06dfda6…`, `Rtmr0–3`, 30-entry event log. **앱 코드는 `Rtmr3`에 측정**된다(compose 해시) — `Mrtd`는 공유 dstack 베이스 이미지라 clean-wallet과 같다. 즉 Check 2에서 핀으로 박을 "우리 코드" 측정값은 `Mrtd`가 아니라 `Rtmr3`(또는 compose 측정)일 수 있다 → **무엇을 핀할지 A2와 정확히 맞춘다.** 검증은 `proof.t16z.com`에서 동일 quote로 교차 확인 가능(Check 1이 진짜 통과하는 환경).
- **libsodium sealed box.** I2(서버→구매자)와 I5(나→서버)가 같은 `crypto_box_seal` 프리미티브 → 한 번 익히면 양쪽에 쓴다. JS는 `libsodium-wrappers`, Rust는 `dryoc`, 곡선 Curve25519로 통일.
- 새로 짜는 건 **(a) AES-GCM 암호화 파이프라인 + (c)→(d) provisioning UI 흐름**뿐. infra(검증기·sealed box·quote 포맷)는 다 있다.

---

## 6. 테스트하는 법

1. **진짜 quote로 검증기 검증 (Check 1 실제 통과 확인).** spike3 CVM(`dropspike3`)의 실제 quote를 `phala cvms attestation --cvm-id dropspike3`로 뽑아 §4(b)에 먹인다. `proof.t16z.com`에서도 같은 quote를 올려 Check 1("signature genuine")이 **PASS**인지 교차 확인 — 시뮬레이터는 dev 키라 항상 Check 1 FAIL이므로, 진짜 통과는 실하드웨어 quote로만 확인된다. Check 2는 `Rtmr3`/측정값이 `EXPECTED_MEASUREMENT`와 일치하는지, Check 3은 `report_data[0..32] == sha256(pubkey)`인지 각각 단위테스트로 고정.
2. **sealed box 라운드트립 (A2 로컬 dstack 시뮬레이터 상대).** (c)에서 만든 `sealed`를 A2의 로컬 dstack simulator 빌드(`POST /provision`)에 보내고, enclave가 `crypto_box_seal_open` 후 내부 카탈로그(I3-b)에 `{drop_id, price_zat, k_drop, ufvk}`가 정확히 들어갔는지 확인. **payload 인코딩(CBOR vs JSON)·필드 순서·키 길이(k_drop 32B)** 가 Rust 쪽 디코드와 바이트로 맞물리는지가 핵심 — 골든 픽스처 하나 박아두면 회귀 안전.
3. **운영자(operator)가 못 연다는 것 증명 (네거티브).** enclave 개인키가 **없는** 상태에서 `sealed`를 `crypto_box_seal_open`(아무 키쌍으로) 시도 → **반드시 실패**해야 한다. 이게 "서버가 `K_drop`을 못 읽는다"는 보장의 클라이언트 측 증거. + Check 1을 일부러 실패시킨 quote(시뮬레이터 dev 키)에는 §4(b)가 **provisioning을 거부**하는지(throw) 테스트 — fail-closed.
4. **엔드투엔드(레인 B와):** (a)로 올린 콘텐츠를 B가 `K_drop`으로 복호화해 똑같은 평문이 나오는지 — I4 분해 규칙(nonce12/tag16)이 양쪽 일치하는지 최종 확인.

---

## 7. 완료 기준 (Definition of Done)

- [ ] **(a)** 드롭마다 `K_drop = random 32 bytes` 생성, AES-256-GCM 암호화, I4 포맷(`nonce12 ‖ ct ‖ tag16`) blob을 버킷에 `h_content=sha256(blob)` 이름으로 업로드. `K_drop` 재사용 없음(드롭당 신규).
- [ ] **(b)** `GET /attest` → **Check 1·2·3 전부** 통과해야만 진행하는 검증기. 하나라도 실패 시 throw(fail-closed). 측정값은 공개 `EXPECTED_MEASUREMENT`(A2 재현빌드 산출값)에 핀. `report_data[0..32] == sha256(enclave_pubkey)` 확인.
- [ ] **(c)** 검증 통과한 enclave 공개키로만 `crypto_box_seal(I5 payload, pubkey)` → `POST /provision`. payload는 A2와 합의한 인코딩(CBOR/JSON), `k_drop`/`price_zat`/`creator_ufvk`(spending key 아님)/`h_content` 포함.
- [ ] **(d)** I3-a 공개 카탈로그 엔트리 등록(`drop_id, price_zec, h_content, title`) — 비밀 미포함.
- [ ] **테스트:** spike3 실 quote로 §6-1 통과 · 시뮬레이터 상대 sealed box 라운드트립(§6-2) 통과 · 운영자-못-연다 네거티브(§6-3) 통과 · B와 복호화 라운드트립(§6-4) 통과.
- [ ] `interfaces.md` 의 I3·I4·I5·I6 필드/단위와 바이트 단위로 일치(특히 `price_zec`↔`price_zat` 변환, nonce12/tag16 분해).
- [ ] **모든 비밀 처리(K_drop, UFVK, seal)가 브라우저 안**에서 — 평문이 서버/백엔드로 새지 않음(§8 마지막).

---

## 8. 주의 / 함정

- **검증을 건너뛰거나 느슨하게 하면 전체 보장이 무너진다 — 검증이 이 레인의 load-bearing 단계.** Check 1(Intel 서명)을 통과 안 한 quote의 공개키로 봉인하면, 그건 그냥 *아무 서버*의 공개키일 수 있다 → 그 서버 주인이 `K_drop`을 읽는다 → "서버는 콘텐츠를 못 본다"는 제품의 핵심 약속이 통째로 거짓이 된다. 데모 압박에 `if (!v.ok)`를 주석 처리하는 순간 제품이 죽는다. **fail-closed로 짜고, 테스트(§6-3)로 박아둔다.**
- **측정값은 반드시 REPRODUCIBLE 빌드와 일치해야 의미가 있다(A2와 협의).** Check 2가 비교하는 `EXPECTED_MEASUREMENT`는, *공개된 소스를 그대로 빌드하면 누구나 같은 해시가 나오는* 재현빌드여야 한다. 재현 불가능하면 "측정값 일치"는 *무엇과* 일치하는지 알 수 없는 빈 확인이 된다. 또한 A2가 이미지를 재빌드하면 측정값이 바뀐다(`[C4]` 측정값 바인딩) → 그때마다 내 핀 값을 갱신해야 한다. **무엇을 핀할지(`Mrtd` vs `Rtmr3`/compose 해시)와 그 값의 출처(CI 산출물)를 A2와 첫날 못 박는다.**
- **revenue-privacy 손실은 Phase 2 얘기 — 내 Phase 1 관심사 아님.** 크리에이터의 매출이 노출되는 건 Phase 2의 unshield→transparent 경로(`[C5]`) 한정이고, 크리에이터가 transparent 목적지를 *직접 선택*할 때만 발생한다. Phase 1의 나는 `K_drop`/UFVK 봉인까지만 — 매출 프라이버시 옵트인 UI 같은 건 만들지 않는다.
- **"quote를 서버에 올려서 검증" 하지 마라 — 클라이언트에서 검증한다.** quote 검증을 어떤 서버(내 백엔드 포함)에 위임하면, 그 서버가 "통과했어요"라고 거짓말할 수 있는 **새 신뢰 주체**가 생긴다 — TEE를 쓰는 의미 자체가 사라진다. dcap-qvl은 WASM이라 브라우저에서 직접 돈다. (clean-wallet은 편의상 `/api/verify-quote`로 프록시하지만, 우리 보안 모델에선 검증 로직을 **브라우저에서** 돌리거나, 최소한 t16z 같은 *우리가 신뢰하기로 명시한* 독립 검증자에만 의존한다. "내 서버가 OK라더라"는 금지.)
- **`K_drop` 재사용 금지 / 평문 유출 금지.** 드롭마다 새 키(재사용 시 forward secrecy 없음 — 한 키 노출로 그 드롭 전 구매분 노출). 그리고 `K_drop`·UFVK·seal 입력 평문이 절대 브라우저 밖으로 나가지 않게 — 업로드는 *암호문(blob)* 과 *sealed box* 뿐.
- **단위/포맷 어긋남이 조용한 킬러.** `price_zec`(문자열)↔`price_zat`(u64), I4의 nonce12/tag16 분해, I5 payload 인코딩(CBOR vs JSON)·필드 순서 — 하나라도 B/A2와 어긋나면 복호화/디코드가 말없이 깨진다. 전부 골든 픽스처로 고정하고 interfaces.md를 정본으로.
