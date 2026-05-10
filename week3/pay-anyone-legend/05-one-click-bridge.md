# §1.5 1Click bridge (1Click 브리지)

> **Cross-reference:** 이 문서는 PAL 코드에서 1Click을 *어떻게 호출하는지*만 다룬다.
> 1Click이 *무엇인지* (Defuse Protocol, NEAR Intents, solver 네트워크, API surface 전체)는 [§3.1 Zcash tool inventory](./zcash-tool-inventory.md)에서 다룬다.

---

## 목적 (Purpose)

PAL의 1Click bridge 서브시스템은 **Zcash → USDC 크로스체인 swap을 완전히 외부 서비스에 위임하는 계층**이다. `lib/oneClick.ts`는 `@defuse-protocol/one-click-sdk-typescript@0.1.14` SDK와 raw `fetch`를 조합하여 세 가지 1Click API 작업(quote 요청, tx hash 제출, 실행 상태 폴링)을 추상화한다. PAL은 1Click이 반환하는 Zcash deposit address를 사용자에게 보여주고, 1Click이 `SUCCESS`를 보고할 때 x402 결제를 실행하는 것 외에 swap 로직에 직접 관여하지 않는다 — ZEC 수신, 크로스체인 프로토콜, USDC 전달 모두 1Click(Defuse Protocol) 책임이다.

---

## 파일과 함수 (Files & functions)

| 파일 | 라인 | 함수/심볼 | 역할 |
|------|------|-----------|------|
| `lib/oneClick.ts` | 1–14 | 모듈 초기화 | `OpenAPI.BASE`, `OpenAPI.TOKEN` 설정; SDK 구성 |
| `lib/oneClick.ts` | 17–43 | `getAvailableTokens()` | `GET /v0/tokens` — 지원 토큰 목록 조회 (PAL 내부에서 직접 호출되지 않음) |
| `lib/oneClick.ts` | 65–134 | `getSwapQuote(params)` | `POST /v0/quote` — ZEC→USDC swap quote 요청; deposit address 반환 |
| `lib/oneClick.ts` | 138–148 | `checkSwapStatus(depositAddress)` | SDK `OneClickService.getExecutionStatus()` 래퍼 |
| `lib/oneClick.ts` | 152–166 | `submitTxHash(txHash, depositAddress)` | SDK `OneClickService.submitDepositTx()` 래퍼 |
| `lib/oneClick.ts` | 169–179 | `ASSETS` (const) | 지원 asset ID 매핑 (`nep141:zec.omft.near` 등) |
| `app/api/relayer/register-deposit/route.ts` | 3 | import | `getSwapQuote`, `ASSETS`, `getAvailableTokens`, `checkSwapStatus` import |
| `app/api/relayer/register-deposit/route.ts` | 55–91 | `POST()` 내부 | `getSwapQuote()` 호출 — deposit 등록 단계에서 quote 요청 |
| `app/api/relayer/register-deposit/route.ts` | 66 | - | `depositAddress = quote.depositAddress \|\| quote.quote?.depositAddress \|\| quote.address` |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 3 | import | `checkSwapStatus` import |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 34 | `GET()` 내부 | `checkSwapStatus(depositAddress)` 호출 — cron 루프 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 37–40 | - | status 필드 다중 탐색: `.status \|\| .executionStatus \|\| .state` |
| `app/api/relayer/check-deposit/route.ts` | 3 | import | `checkSwapStatus` import |
| `app/api/relayer/check-deposit/route.ts` | 20 | `checkDepositStatus()` | `checkSwapStatus(depositAddress)` 호출 — UI 폴링 단계 |
| `app/api/relayer/submit-tx-hash/route.ts` | 2 | import | `submitTxHash` import |
| `app/api/relayer/submit-tx-hash/route.ts` | 34 | `POST()` 내부 | `submitTxHash(txHash, depositAddress)` 호출 |
| `app/api/content/get-url/route.ts` | 3 | import | `checkSwapStatus` import |
| `app/api/content/get-url/route.ts` | 69 | `GET()` 내부 | `checkSwapStatus(depositAddress)` 호출 — content 잠금 해제 전 최종 검증 |

---

## 연결 (Wiring)

```
                    ┌──────────────────────────────────────────────────────────────┐
                    │                  lib/oneClick.ts                              │
                    │                                                              │
  [§1.3 register-  │                                                              │
   deposit route]  │  getSwapQuote(params)                                        │
  ParsedIntent ───▶│    POST https://1click.chaindefuser.com/v0/quote              │
  (amount, chain,  │    ← depositAddress (Zcash 수신 주소, 1Click 소유)            │
   recipient)      │    ← quote (amountInFormatted, deadline, ...)                │
                    │                                                              │
  [사용자 선택적]   │  submitTxHash(txHash, depositAddress)                         │
  ZEC tx hash ────▶│    SDK OneClickService.submitDepositTx()                      │
                    │    (swap 처리 가속화 — optional)                              │
                    │                                                              │
  [Vercel cron,    │  checkSwapStatus(depositAddress)                             │
   UI 폴링,        │    SDK OneClickService.getExecutionStatus()                   │
   content 잠금    │    ← { status: 'PENDING_DEPOSIT' | 'PROCESSING' |            │
   해제]    ───────▶│              'SUCCESS' | 'INCOMPLETE_DEPOSIT' |              │
                    │              'REFUNDED' | 'FAILED' }                         │
                    └──────────────────────────────────────────────────────────────┘
                            │ depositAddress (§1.3 QR)
                            │ swap status (§1.4 cron → x402 trigger)
                            ▼
                    1Click API / Defuse Protocol
                    https://1click.chaindefuser.com
```

- **Inputs:**
  - Quote 요청: `{ senderAddress, recipientAddress(EVM), originAsset, destinationAsset, amount, dry, sessionId }` (`lib/oneClick.ts:65–94`)
  - Tx hash 제출: `{ txHash, depositAddress }` (`lib/oneClick.ts:152`)
  - Status 폴링: `depositAddress` 문자열 (`lib/oneClick.ts:138`)

- **Outputs:**
  - Quote 응답: `{ depositAddress, swapId, sessionId, quote: { amountInFormatted, deadline, ... } }` → §1.3(QR 코드), §1.4(Supabase 저장)
  - Status 응답: `{ status, executionStatus, state }` 중 하나 → §1.4(cron x402 트리거), §1.7(content 잠금 해제)

- **Dependencies (internal):**
  - 없음 — `lib/oneClick.ts`는 프로젝트 내부 모듈에 의존하지 않음 (순수 external API client)

- **Dependencies (external):**
  - `@defuse-protocol/one-click-sdk-typescript@0.1.14` — `OneClickService`, `OpenAPI` (SDK transport)
  - `https://1click.chaindefuser.com` — 1Click HTTP REST API (`/v0/quote`, SDK internal endpoints)
  - 환경 변수: `ONE_CLICK_API_URL` (override), `ONE_CLICK_JWT` (선택적 Bearer token)

---

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `@defuse-protocol/one-click-sdk-typescript` | `0.1.14` | `OneClickService.getExecutionStatus(depositAddress)` (status 폴링), `OneClickService.submitDepositTx({txHash, depositAddress})` (tx hash 제출). SDK 내부 transport는 OpenAPI generator 기반 fetch wrapper. |
| `fetch` (built-in) | Web API | `getAvailableTokens()`, `getSwapQuote()` — SDK를 우회하여 raw `fetch`로 `/v0/tokens`, `/v0/quote` 직접 호출 |

> `axios`는 사용되지 않는다. quote 요청은 SDK가 아닌 raw `fetch`로 구현되어 있다 (`lib/oneClick.ts:102`).
> SDK는 status 폴링(`getExecutionStatus`)과 tx hash 제출(`submitDepositTx`)에만 사용된다.

---

## 워크스루 — happy path

> **전제 조건:** `ONE_CLICK_API_URL=https://1click.chaindefuser.com`, `ONE_CLICK_JWT`는 설정되어 있거나 없음(없으면 0.1% 수수료 추가).

**1단계: 인텐트 빌드** — 사용자가 "Pay OnlyFans $10"를 제출하면 `lib/nearAI.ts:43–44`가:

```typescript
// lib/nearAI.ts:43–44
{ currency: 'USDC', amount: '10', chain: 'base',
  bridgeFrom: 'zcash',  // ← 하드코딩 (항상 zcash)
  receivingAddress: '0xABC...' }
```

를 생성. `register-deposit` 서버 핸들러가 이 값을 받는다.

**2단계: PAL이 1Click `/v0/quote` 호출** (`app/api/relayer/register-deposit/route.ts:55`, `lib/oneClick.ts:102`)

```typescript
// lib/oneClick.ts:78–110 (핵심 발췌)
const quoteRequest: QuoteRequest = {
  dry: false,                              // 실제 실행 (테스트 아님)
  swapType: 'EXACT_OUTPUT',               // 출력 USDC 고정, 입력 ZEC 계산
  slippageTolerance: 100,                 // 1% slippage
  originAsset: 'nep141:zec.omft.near',   // ASSETS.ZCASH — 하드코딩
  depositType: 'ORIGIN_CHAIN',
  destinationAsset: 'nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near', // USDC_BASE
  amount: usdcToSmallestUnit('10'),       // "10000000" (10 USDC, 6 decimals)
  refundTo: process.env.REFUND_ZCASH_ADDRESS || senderAddress,
  refundType: 'ORIGIN_CHAIN',
  recipient: swapWallet,                  // NEAR Chain Sig으로 파생된 EVM 주소
  recipientType: 'DESTINATION_CHAIN',
  deadline: new Date(Date.now() + 3 * 60 * 1000).toISOString(), // 3분 후
  referral: 'anyone-pay',
  quoteWaitingTimeMs: 3000,
  sessionId: `session_${Date.now()}_${Math.random().toString(36).substring(2, 9)}`,
}
const response = await fetch(`${ONE_CLICK_API_URL}/v0/quote`, {
  method: 'POST',
  headers: {
    'Content-Type': 'application/json',
    ...(ONE_CLICK_JWT ? { Authorization: `Bearer ${ONE_CLICK_JWT}` } : {}),
  },
  body: JSON.stringify(quoteRequest),
})
```

> **주목:** `recipient`는 최종 사용자의 x402 결제 수신 주소(`receivingAddress`)가 **아니다.** NEAR Chain Signatures로 파생된 EVM 중간 주소(`swapWallet`)다. PAL이 USDC를 중간 EVM 주소로 받아 나중에 x402로 전달하는 2단계 구조다.

**3단계: 1Click이 deposit address + quote_data 반환** (`lib/oneClick.ts:124–128`)

```typescript
return {
  ...data,
  depositAddress: data.depositAddress || data.quote?.depositAddress || data.address,
  swapId: data.swapId || data.id || data.depositAddress,
  sessionId: responseSessionId,
}
```

`depositAddress`는 1Click solver가 소유·관리하는 Zcash 수신 주소다 (t-address 또는 z-address; PAL은 형식을 검증하지 않음). 이 주소가 `deposit_tracking.deposit_address` PK로 저장된다.

**4단계: PAL이 quote를 Supabase에 저장, QR 코드 표시** (`app/api/relayer/register-deposit/route.ts:110–123`, `lib/depositTracking.ts:104`)

```typescript
// register-deposit/route.ts:80, 110–122
quoteData = quote  // 전체 응답 저장
const result = await registerDeposit(
  depositAddress,
  intentId,
  amount,
  recipient,      // 원래 x402 수신자 (onlyfans 주소)
  swapId,
  intentType,
  swapWallet,     // NEAR Chain Sig EVM 주소
  nearAccountId,
  chain,
  redirectUrl,
  quoteData,      // 전체 1Click quote JSON → quote_data JSONB 컬럼
  deadline
)
```

`app/page.tsx:279`가 `depositAddress`를 URL에 저장(`?depositAddr=...`)하고, `components/IntentsQR.tsx:186`이 QR 코드로 렌더링. ([§1.3 참조](./03-z-address-generation.md))

**5단계: 사용자가 ZEC를 deposit address로 송금; 선택적으로 tx hash 제출** (`app/api/relayer/submit-tx-hash/route.ts:34`)

```typescript
// submit-tx-hash/route.ts:34
await submitTxHash(txHash, depositAddress)
// → lib/oneClick.ts:152–165
const a = await OneClickService.submitDepositTx({ txHash, depositAddress })
```

이 단계는 optional이다. tx hash를 제출하면 1Click이 swap 처리를 가속화한다. PAL은 tx hash 형식을 `txHash.length < 10` 외에 검증하지 않는다 (`submit-tx-hash/route.ts:23`).

**6단계: Cron이 `getExecutionStatus(depositAddress)`를 폴링** (`app/api/relayer/cronjob-check-deposits/route.ts:34`, `lib/oneClick.ts:141`)

```typescript
// cronjob-check-deposits/route.ts:34–40
const statusResponse = await checkSwapStatus(depositAddress)
// ↓ lib/oneClick.ts:141
const status = await OneClickService.getExecutionStatus(depositAddress)

// status 필드 다중 탐색 (SDK 응답 키 불확실)
const status = (statusResponse as any).status ||
               (statusResponse as any).executionStatus ||
               (statusResponse as any).state ||
               'PENDING_DEPOSIT'
const normalizedStatus = String(status).toUpperCase()
```

Vercel cron이 `*/1 * * * *` (매 1분)마다 이 endpoint를 GET으로 호출한다 (`vercel.json:9`). ([§1.4 참조](./04-deposit-tracking.md))

**7단계: SUCCESS 시 1Click이 `swapWallet`(EVM)에 USDC 전달; PAL이 x402 트리거**

```typescript
// cronjob-check-deposits/route.ts:47–132
if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed) {
  const { signX402TransactionWithChainSignature } = await import('@/lib/chainSig')
  const transactionHash = await signX402TransactionWithChainSignature({
    payTo,            // tracking.recipient (원래 x402 수신자)
    maxAmountRequired: String(maxAmountRequired),
    deadline: Math.floor(Date.now() / 1000) + 3600,
    nonce: `0x${Date.now().toString(16)}`,
  })
  await updateDepositTracking(depositAddress, {
    signedPayload: transactionHash,  // Ethereum tx hash (ERC-20 transferWithAuthorization)
    x402Executed: true,
    confirmed: true,
  })
}
```

1Click이 `SUCCESS`를 보고하면 PAL은 NEAR Chain Signatures를 통해 EVM에서 ERC-20 USDC 전송을 서명·브로드캐스트하고, 그 결과 Ethereum tx hash를 `signed_payload` 컬럼에 저장한다. ([§1.7 참조](./07-x402-client.md))

> content unlock도 1Click status를 재확인한다 (`app/api/content/get-url/route.ts:69`): `getUrl` 핸들러가 `checkSwapStatus(depositAddress)`를 다시 호출하여 `SUCCESS`가 아니면 HTTP 402를 반환한다.

---

## 노트 / 특이사항 / 주의점 (Notes / quirks / footguns)

### 1. 1Click endpoints PAL이 실제로 호출하는 목록 (교체 비용 분석)

PAL이 1Click 서비스에 노출된 표면적(surface area):

| Endpoint | Caller (file:line) | PAL에서의 목적 | PAL이 가정하는 것 |
|----------|-------------------|---------------|-----------------|
| `POST /v0/quote` (raw fetch) | `lib/oneClick.ts:102` → `register-deposit/route.ts:55` | ZEC→USDC swap quote 획득 + Zcash deposit address 수신 | 응답에 `depositAddress` 또는 `quote.depositAddress` 또는 `address` 필드가 존재; 1Click이 해당 주소로의 ZEC 입금을 감지·처리 |
| `GET /v0/tokens` (raw fetch) | `lib/oneClick.ts:19` → **직접 호출 없음** (export만 됨) | 지원 토큰 목록 조회 (현재 앱에서 호출 안 됨) | — |
| SDK `OneClickService.getExecutionStatus(depositAddress)` | `lib/oneClick.ts:141` → `cronjob-check-deposits:34`, `check-deposit:20`, `get-url/route.ts:69` | swap 진행 상태 폴링 (SUCCESS 감지 시 x402 트리거) | 응답에 `.status` 또는 `.executionStatus` 또는 `.state` 중 하나 존재; `SUCCESS` 상태가 ZEC → USDC 변환 완료를 의미 |
| SDK `OneClickService.submitDepositTx({txHash, depositAddress})` | `lib/oneClick.ts:155` → `submit-tx-hash/route.ts:34` | 사용자가 제출한 ZEC tx hash를 1Click에 전달해 swap 가속화 (optional) | 1Click이 이 tx hash를 온체인에서 확인하고 swap을 앞당겨 처리 |

**1Click 교체 시 교체해야 할 것:**
1. `POST /v0/quote` — Zcash deposit address 발급 주체를 교체해야 함 (자체 Zcash wallet 인프라 또는 대안 서비스)
2. `getExecutionStatus` — ZEC 입금 감지 로직을 교체해야 함 (lightwalletd, Zebra RPC, 또는 독립적인 체인 모니터링)
3. `submitDepositTx` — 경우에 따라 불필요해지거나 자체 구현 필요

---

### 2. 신뢰 모델 (Trust model) / 위협 분석

**1Click이 통제하는 것:**
- Zcash deposit address 생성 및 소유 (spending key가 1Click/solver에 있음)
- ZEC 입금 감지 시점 및 방법
- swap에 사용되는 시장 환율 (EXACT_OUTPUT이지만 rate는 1Click이 결정)
- swap 완료 보고(`SUCCESS`) — PAL이 독립 검증 없이 신뢰
- USDC 전달 타이밍 및 실제 수령 여부
- `REFUNDED` 시 refundTo로의 ZEC 반환 (PAL은 `refundTo`만 설정, 실행은 1Click)

**PAL이 통제하는 것:**
- `recipient` 파라미터 — 최종 USDC 수신 EVM 주소 (NEAR Chain Sig 파생 `swapWallet`)
- `refundTo` 파라미터 — swap 실패 시 ZEC 반환 주소
- x402 결제 실행 트리거 시점 (SUCCESS 감지 후)
- x402 `payTo` 주소 — 최종 서비스 결제 수신자 (`tracking.recipient`)

**핵심 취약점 — blind trust:**

```typescript
// cronjob-check-deposits/route.ts:47
if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed) {
  // ← 체인 독립 검증 없이 1Click의 SUCCESS를 믿고 즉시 x402 실행
```

PAL은 1Click의 `SUCCESS` 상태 외에 어떠한 체인 독립 검증도 수행하지 않는다. lightwalletd, Zebra RPC, ZcashLightClientKit 등 Zcash 온체인 검증 라이브러리는 전체 코드베이스에 단 한 줄도 없다. 이 의미:

- 1Click API가 침해되거나 `SUCCESS`를 잘못 보고하면, PAL은 ZEC를 받지 않고도 x402 결제를 실행한다.
- 1Click API가 다운되면 PAL의 swap 기능 전체가 멈춘다.
- 1Click이 `recipient`(swapWallet)에 실제로 USDC를 전달했는지 PAL은 확인하지 않는다.

**ZEC 보관 위치 (§7 open question 답):**

```
사용자 ZEC → 1Click deposit address (1Click solver 지갑, spending key는 1Click 보유)
           → solver가 swap 실행 (cross-chain, NEAR Intents 기반 추정)
           → USDC가 PAL의 swapWallet (NEAR Chain Sig 파생 EVM 주소)에 도착
           → PAL이 x402로 최종 서비스 주소(payTo)에 USDC 전달
```

swap 진행 중 ZEC는 1Click(또는 Defuse Protocol solver)이 custodying한다. PAL은 ZEC에 대한 어떠한 custody도 갖지 않는다.

**`recipient` 존중 여부:**

PAL이 `/v0/quote`에 넘기는 `recipient`는 `swapWallet` (NEAR Chain Sig EVM 주소)이고, 최종 서비스 결제 수신자는 `tracking.recipient` (intent에서 파싱된 x402 수신자)다. 1Click은 `swapWallet`에 USDC를 보내고, PAL이 x402를 실행할 때 `tracking.recipient`로 전달한다. 즉, PAL이 지정한 `recipient`(swapWallet)를 1Click이 존중하는 것을 PAL은 신뢰하지만, 독립 검증하지 않는다.

**취소/환불:**

- 사용자 단독 취소: 불가능 — 한번 ZEC를 deposit address로 보내면 PAL이나 사용자가 되돌릴 수 없다.
- 1Click 자동 환불(`REFUNDED`): `check-deposit/route.ts:47–49`에서 감지하고 `{ refunded: true }` 반환하지만, PAL은 이 시점에 사용자에게 자동 알림을 보내거나 후속 행동을 취하지 않는다.
- PAL 구현 환불 endpoint: **없음** — DEPLOY.md가 `POST /api/relayer/refund` 존재를 주장하지만, 실제로는 구현되지 않았다 (`app/api/relayer/` 디렉토리에 없음). ([§1.4 참조](./04-deposit-tracking.md))

---

### 3. base URL 불일치 (Docs vs. Code)

| 출처 | URL |
|------|-----|
| README/DEPLOY.md 언급 | `https://api.1click.fi` |
| 실제 코드 (`lib/oneClick.ts:7`) | `https://1click.chaindefuser.com` |
| SDK comment (`lib/oneClick.ts:1`) | `https://github.com/near-examples/near-intents-examples` |

`ONE_CLICK_API_URL` 환경변수로 override 가능하지만 기본값은 `https://1click.chaindefuser.com` (Defuse Protocol의 chaindefuser 도메인)이다. `1click.fi` 도메인은 코드 어디에도 없다. 이 불일치는 upstream README가 현재 API와 다른 도메인을 가리키고 있었음을 시사한다.

---

### 4. 모든 swap 경로는 ZEC → USDC로 고정

`bridgeFrom: 'zcash'`는 `lib/nearAI.ts:44`에서 하드코딩된다:

```typescript
// lib/nearAI.ts:44
bridgeFrom: 'zcash',  // 항상 zcash; 다른 originAsset 경로 없음
```

`register-deposit/route.ts:40–36`에서 `originAsset`은 항상 `ASSETS.ZCASH = 'nep141:zec.omft.near'`이고, `destinationAsset`만 chain에 따라 `USDC_BASE` 또는 `USDC_SOLANA`로 분기된다. 즉, PAL에서 1Click을 통한 가능한 swap 경로는 다음 두 가지뿐이다:

- `nep141:zec.omft.near` → `nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near` (USDC on Base)
- `nep141:zec.omft.near` → `nep141:sol-5ce3bf3a31af18be40ba30f721101b4341690186.omft.near` (USDC on Solana)

다른 origin asset이나 destination asset을 지원하는 코드 경로는 없다. ([§1.1 참조](./01-intent-parser.md))

---

### 5. JWT와 수수료

```typescript
// lib/oneClick.ts:6, 12–14
const ONE_CLICK_JWT = process.env.ONE_CLICK_JWT || ''
if (ONE_CLICK_JWT) {
  OpenAPI.TOKEN = ONE_CLICK_JWT  // SDK에 Bearer token 설정
}
// raw fetch에도 동일하게 적용:
...(ONE_CLICK_JWT ? { Authorization: `Bearer ${ONE_CLICK_JWT}` } : {})
```

`ONE_CLICK_JWT`가 없으면 1Click이 swap에 0.1% 추가 수수료를 부과한다(README 주장). SDK와 raw fetch 모두 `ONE_CLICK_JWT`를 동일하게 적용한다. JWT가 없어도 기능은 동작하지만 수수료가 높아진다.

---

### 6. SDK 응답 필드 불확실성 (status 키)

```typescript
// cronjob-check-deposits/route.ts:37–40
const status = (statusResponse as any).status ||
               (statusResponse as any).executionStatus ||
               (statusResponse as any).state ||
               'PENDING_DEPOSIT'
```

동일한 패턴이 `check-deposit/route.ts:27–30`, `get-url/route.ts:70–73`에도 반복된다. SDK 응답 타입이 확실하지 않아 세 가지 가능한 키를 순서대로 시도한다. `as any`로 TypeScript 타입 검사를 우회하는 것이 코드 전반에 걸쳐 나타난다. `@defuse-protocol/one-click-sdk-typescript@0.1.14`의 실제 응답 타입이 무엇인지, 현재 코드베이스에서는 `node_modules`가 없어 확인 불가 (Task 10에서 SDK 소스 분석 필요).

---

### 7. INCOMPLETE_DEPOSIT 동작

```typescript
// check-deposit/route.ts:65–68
if (status === 'INCOMPLETE_DEPOSIT') {
  console.log(`⚠️ Incomplete deposit. Status: ${status}`)
  return { confirmed: false, status, incompleteDeposit: true, statusResponse }
}
```

`INCOMPLETE_DEPOSIT`은 사용자가 quote에서 요구한 ZEC보다 적게 보낸 경우 발생한다. PAL은 이 상태를 감지하여 `{ incompleteDeposit: true }`를 반환하지만:
- 사용자에게 자동 알림이 없다.
- 추가 입금을 허용하는 UX가 없다.
- 1Click이 자동으로 `REFUNDED`로 전이시킬 수 있지만, 그 타이밍은 1Click 정책에 따름.
- PAL 코드에서 `INCOMPLETE_DEPOSIT`에 대한 재시도 또는 안내 로직이 없다 — 사실상 **deposit이 limbo 상태가 된다.**

---

### 8. 1Click 장애 시 PAL 실패 분석

| 시나리오 | PAL 결과 |
|----------|---------|
| `/v0/quote` 타임아웃/오류 | `register-deposit` HTTP 500 반환; deposit 등록 불가; 사용자는 QR 코드 못 받음 |
| `getExecutionStatus` 오류 | cronjob이 해당 deposit을 `catch` 처리하고 넘어감; 다음 cron 실행(1분 후) 재시도 |
| 1Click이 영구적으로 `PENDING` 유지 | `deadline > now` 조건 실패 시 cron 순회에서 제외; x402 미실행; 사용자는 ZEC만 납부하고 content 못 받음 |
| 1Click이 `SUCCESS` 허위 보고 | PAL이 x402 결제 즉시 실행 (ZEC 미수신 상태에서) |
| 1Click이 `REFUNDED` 반환 | PAL이 `{ refunded: true }` 반환하지만 사용자 알림/환불 UI 없음 |
| `submitDepositTx` 오류 | `submit-tx-hash/route.ts:48–57`에서 HTTP 500 반환; swap 가속화 실패. 단, tx hash 미제출은 기능 상실이 아님(optional이므로 cron 폴링이 계속 진행됨) |

재시도 로직: PAL 코드에는 fetch 수준의 retry나 exponential backoff가 없다. 단일 실패 시 `throw` 또는 `catch` + `console.error`로 처리된다. 복구는 cron의 자연적 재시도(1분 주기)에만 의존한다.

---

### 9. 프라이버시 이야기 (Privacy story)

PAL의 README는 "Zcash shielded transactions hide amounts, sender, and recipient"를 주장한다. 실제 코드 분석 결과는 다르다:

**1Click solver가 알게 되는 정보:**

| 정보 | 시점 |
|------|------|
| 송신자 ZEC 주소 | `refundTo` 파라미터 (`lib/oneClick.ts:86`) |
| 수신 예정 ZEC 금액 | `EXACT_OUTPUT` quote의 계산 결과 |
| 최종 USDC 수신 EVM 주소 | `recipient` 파라미터 (`swapWallet`) |
| 목표 체인 (Base/Solana) | `destinationAsset` 파라미터 |
| `referral: 'anyone-pay'` | `/v0/quote` 요청에 포함 |
| ZEC on-chain tx hash (선택적) | `submitDepositTx` 호출 시 |

**결론: PAL의 privacy story는 성립하지 않는다.** 1Click solver는 (Zcash 송신자, ZEC tx, 최종 USDC 수신 EVM 주소)를 모두 알게 된다. Zcash z-address의 shielded 특성(발신자·금액 은닉)이 L1에서 성립하더라도, 1Click API 레벨에서 이 연결고리가 명시적으로 노출된다. 특히 `refundTo`와 `recipient`가 동일 quote 요청에 포함되므로 unlinkability(송신자-수신자 비연결성)는 전혀 없다. 1Click(Defuse Protocol)을 신뢰한다는 가정 하에서만 프라이버시가 "도덕적으로" 보호된다 — 기술적 보장이 아니다.

---

## 답한 open questions (spec §7)

### "Where does ZEC actually live during the swap?"

**답:** swap 진행 중 ZEC는 **1Click(Defuse Protocol) solver 지갑**에 있다. PAL이 제공하는 deposit address는 1Click이 생성·소유하는 주소다(`lib/oneClick.ts:126`에서 API 응답의 `depositAddress`를 그대로 사용). PAL은 해당 주소의 spending key나 viewing key를 갖지 않는다. ZEC 수신 → swap 실행 → USDC 전달의 전체 과정이 1Click 내부에서 이루어지며, PAL은 결과 상태(`SUCCESS`, `REFUNDED` 등)만 polling으로 수신한다.

### "1Click JWT 미사용 시 수수료"

**답:** `ONE_CLICK_JWT` 환경변수가 없으면 Authorization 헤더 없이 API를 호출하고 1Click이 0.1% 추가 수수료를 부과한다(`lib/oneClick.ts:6`, `12–14`). JWT는 SDK와 raw fetch 양쪽에 모두 적용된다.

### "getExecutionStatus response field name"

**답:** 코드가 `.status || .executionStatus || .state`를 순서대로 시도하는 것으로 보아(`cronjob-check-deposits/route.ts:37–40`), PAL 개발 당시 `@defuse-protocol/one-click-sdk-typescript@0.1.14`의 응답 타입이 명확하지 않았음을 시사한다. `node_modules`가 없어 SDK 소스로 직접 확인 불가 — §3.1(Task 10)에서 SDK 소스 분석 필요.

### "INCOMPLETE_DEPOSIT 동작"

**답:** `INCOMPLETE_DEPOSIT`은 `check-deposit/route.ts:65–68`에서 감지된다. PAL은 `{ incompleteDeposit: true }`를 응답하지만 후속 처리(알림, 재입금 안내, 자동 환불)는 없다. 1Click이 내부적으로 결국 `REFUNDED`로 전이시킬 수 있으나, 그 동작은 1Click 정책 영역이다 (§3.1 Task 10 확인 필요).

---

## 크로스 레퍼런스

- **Z-address 생성 상세** — [§1.3 Z-address generation](./03-z-address-generation.md): deposit address가 1Click에서 오는 전체 경위
- **Deposit tracking 상세** — [§1.4 Deposit tracking](./04-deposit-tracking.md): Supabase 저장, cron 폴링, swap status 상태 머신
- **x402 실행 상세** — [§1.7 x402 client](./07-x402-client.md): `signX402TransactionWithChainSignature()`, ERC-20 `transferWithAuthorization`, NEAR Chain Signatures
- **1Click 프로토콜 분석** — §3.1(Task 10): 1Click이 무엇인지, Defuse Protocol/NEAR Intents, solver 네트워크, API surface 전체 분석
