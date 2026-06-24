# Lane C — Creator Web App

크리에이터가 보는 화면 전부. 콘텐츠 파일 선택 → 드롭마다 새 `K_drop` 생성 → AES-256-GCM 암호화 → 버킷 업로드 → `GET /attest` quote 검증 → 검증된 enclave 공개키로 `K_drop`/UFVK 봉인 → `POST /provision` → 공개 카탈로그 등록.

스펙: [`../team/lane-C-creator-app.md`](../team/lane-C-creator-app.md) · 계약: [`../team/interfaces.md`](../team/interfaces.md) · 디자인: [`./DESIGN.md`](./DESIGN.md)

## 실행

```bash
npm install
npx playwright install chromium
npm run dev      # 127.0.0.1:5173
npm test         # vitest (28 tests)
npm run build    # tsc + vite build
```

- **로컬 UI/QA 모드**: Playwright가 실제 Vite 페이지를 띄우고 인덱서 응답을 route-mock한다. 서버/체인/실 quote 없이 happy path, attestation fail-closed, validation, double-submit, responsive 상태를 전부 확인한다.
- **Live indexer 모드**: `VITE_DROP_INDEXER_URL`, `VITE_DROP_EXPECTED_MEASUREMENT_HEX`, `VITE_DROP_QVL_MODULE_URL`을 넣고 실제 `/health`·`/catalog`·`/attest`에 붙는다. live env가 없으면 smoke는 **성공이 아니라 skipped-live**로 기록된다.

## 한 줄 요약

브라우저가 먼저 quote를 검증해서 “이 공개키는 우리가 핀한 enclave 코드가 만든 키”라고 확인한 뒤에, 그 공개키로 비밀을 sealed box에 넣는다. 검증 실패면 provisioning은 멈춘다.

## 큰 흐름

```
크리에이터 브라우저 (Lane C)
┌──────────────────────────────────────────────────────────────┐
│ ① 파일 암호화                                                 │
│    K_drop=random32 → AES-256-GCM → I4 blob                    │
│    h_content=sha256(blob)                                     │
│                         │                                    │
│                         └──────────────▶ upload blob          │
│                                                              │
│ ② 서버 인증 확인                                              │
│    GET /attest → quote_hex + provisioning_pubkey_hex          │
│    Check 1: quote verifier says ok                            │
│    Check 2: codeMeasurement == EXPECTED_MEASUREMENT           │
│    Check 3: report_data[0..32] == sha256(pubkey)              │
│                                                              │
│ ③ 비밀 봉인                                                   │
│    crypto_box_seal(I5 payload, verified enclave pubkey)       │
│                         │                                    │
│                         └──────────────▶ POST /provision      │
│                                                              │
│ ④ 공개 카탈로그 등록                                          │
│    { drop_id, price_zec, h_content, title }                   │
│                         └──────────────▶ public catalog       │
└──────────────────────────────────────────────────────────────┘
```

핵심은 ②. `quote` 검증과 공개키 바인딩이 통과해야만 ③으로 간다. 이걸 느슨하게 만들면 서버 운영자가 자기 공개키를 넣어 `K_drop`을 읽을 수 있으므로 제품 보장이 깨진다.

## 바이트 계약 (상대 레인과 일치해야)

| 인터페이스 | 형식 | 파일 |
|---|---|---|
| I3-a catalog | `{ drop_id, price_zec, h_content, title }` — 공개, 비밀 없음 | `api.ts` |
| I4 content blob | `nonce(12) ‖ AES-256-GCM ciphertext ‖ tag(16)`, `h_content=sha256(blob)` | `content.ts` |
| I5 provision | `POST /provision`, sealed JSON `{ drop_id, price_zat, k_drop, creator_ufvk, h_content }` | `provision.ts` |
| I6 attest | `{ quote_hex, provisioning_pubkey_hex }`를 Zod로 파싱 후 quote verifier 결과와 바인딩 검사 | `api.ts`, `attestation.ts` |

- `price_zec`는 사람이 보는 문자열 단위다. `price_zat`는 enclave가 계산하는 zatoshi 정수다. `price.ts`가 변환을 맡고 테스트가 소수점/안전정수 경계를 고정한다.
- I4는 Buyer 앱의 `content.ts`와 같은 분해 규칙을 쓴다. 앞 12바이트가 nonce, WebCrypto 결과의 뒤 16바이트가 tag다.
- JSON에는 byte 타입이 없어서 I5 payload의 `k_drop`은 32 raw bytes를 64자 hex 문자열로 넣는다. sealed box 밖으로는 평문 `k_drop`이 나오지 않는다.

## 파일 맵

```
src/
  bytes.ts        hex/sha256/byte 헬퍼
  content.ts      K_drop 생성, AES-256-GCM 암호화, I4 blob 생성
  price.ts        price_zec ↔ price_zat 변환
  provision.ts    I5 payload 생성 + libsodium sealed box
  attestation.ts  quote 결과 정규화, measurement 핀, report_data↔pubkey 바인딩
  api.ts          indexer HTTP 경계: /attest, /catalog, upload, /provision
  main.tsx        creator UI와 단계별 상태/에러 표시
  styles.css      DESIGN.md 토큰 기반 화면 스타일

scripts/
  check-secret-sinks.mjs        K_drop/UFVK 평문 sink 정적 검사
  http-smoke*.mjs               live indexer smoke + quote verifier 연결
  http-smoke-redaction-probe.mjs  smoke stdout/evidence redaction 회귀 검사

tests/
  creator-flow.spec.ts          실제 Chromium UI 플로우: happy/failure/validation/double-submit/responsive
```

## 검증기(QVL) 처리 방식

이 앱은 `@phala/dcap-qvl-web`을 번들 기본값으로 넣지 않는다. 해당 패키지의 production advisory와 브라우저 export 모양 때문에, 기본 의존성으로 두면 “검증되는 척하지만 실제 live 기본 경로가 깨지는” 상태가 된다.

대신 live quote 검증은 명시적으로 주입한다:

```bash
VITE_DROP_QVL_MODULE_URL="$NODE_OR_BROWSER_IMPORTABLE_VERIFIER"
```

검증 모듈은 `verifyQuote(quoteHex)` 또는 `verify(quoteHex)`를 export해야 하고, 결과는 `{ ok, codeMeasurement, reportData }` 또는 테스트가 허용하는 alias를 반환해야 한다. 로컬 Playwright QA는 `window.dropQuoteVerifier` mock을 설치해 UI 흐름을 검증한다.

## 테스트하는 법

```bash
npm test
npm run build
npm run test:browser -- --project=chromium
node scripts/check-secret-sinks.mjs
npm audit --omit=dev
npm run qa:lane-c
```

`npm run qa:lane-c`는 로컬 Lane C gate다. 순서대로 unit test, production build, Chromium browser QA, secret-sink scan, HTTP smoke를 실행한다.

live env가 없으면 마지막 HTTP smoke는 이렇게 끝난다:

```text
LIVE SMOKE SKIPPED: missing VITE_DROP_INDEXER_URL, VITE_DROP_EXPECTED_MEASUREMENT_HEX. No deployed indexer was contacted.
```

이건 “live 통과”가 아니라 “live를 안 돌렸다”는 정직한 결과다. live로 보려면:

```bash
VITE_DROP_INDEXER_URL="$LIVE_INDEXER_URL" \
VITE_DROP_EXPECTED_MEASUREMENT_HEX="$EXPECTED_MEASUREMENT" \
VITE_DROP_QVL_MODULE_URL="$NODE_IMPORTABLE_VERIFIER" \
npm run qa:http-smoke
```

보안상 `VITE_DROP_INDEXER_URL`에 credential, query, fragment가 들어가면 smoke가 setup 단계에서 거부한다. 에러 stdout/evidence도 `[redacted-env]`로 마스킹한다.

## 알려진 한계 / 팀 싱크 포인트

- **실 quote Check 1은 아직 외부 통합 증거가 필요하다.** 로컬 QA는 mock verifier로 UI와 fail-closed를 검증한다. spike3/A2 실 quote로 `VITE_DROP_QVL_MODULE_URL` verifier를 붙여 한 번 더 확인해야 full lane DoD가 닫힌다.
- **A2와 I5 payload 인코딩을 고정해야 한다.** 현재 구현은 sealed JSON이다. A2가 CBOR을 요구하면 `provision.ts`의 encode 지점과 Rust decoder를 같은 골든 픽스처로 맞춰야 한다.
- **B와 I4 복호 라운드트립을 최종 확인해야 한다.** content blob 분해 규칙은 Buyer와 맞춰 뒀지만, 실제 카탈로그→구매→dispatch→복호 전체는 B/A2 통합에서 한 번 더 본다.
- **비밀은 브라우저 밖으로 평문 전송하지 않는다.** `check-secret-sinks.mjs`가 console/log/storage/download/DOM sink를 훑고, Playwright는 실패 경로에서 provisioning이 호출되지 않는지 본다.
