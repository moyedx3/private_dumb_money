# §1.4 Deposit tracking (입금 추적)

## 목적 (Purpose)

Deposit tracking 서브시스템은 사용자의 "ZEC를 보냈다"는 주장과 시스템의 "이제 x402 paywall을 해제하겠다"는 결정 사이의 다리 역할을 한다. 구체적으로는, 1Click API로부터 받은 Zcash deposit address를 Supabase `deposit_tracking` 테이블에 저장하고, Vercel cron이 1분마다 해당 주소로 1Click SDK의 `getExecutionStatus`를 폴링하여 swap이 `SUCCESS` 상태가 되면 `signX402TransactionWithChainSignature()`를 호출해 x402 결제를 실행하는 것이 핵심 임무다. **결정적으로, PAL은 체인(Zcash blockchain)을 직접 조회하지 않는다 — 1Click의 swap status 보고를 그대로 신뢰(trust)한다.** 체인 검증(lightwalletd, Zebra RPC 등)은 PAL 코드에 전혀 존재하지 않는다.

---

## 파일과 함수 (Files & functions)

| 파일 | 라인 | 함수/export | 역할 |
|------|------|-------------|------|
| `lib/depositTracking.ts` | 104 | `registerDeposit(depositAddress, intentId, amount, ...)` | 신규 deposit row를 Supabase에 upsert; 실패 시 in-memory fallback |
| `lib/depositTracking.ts` | 182 | `getDepositTracking(depositAddress)` | 단건 조회 (Supabase → in-memory fallback) |
| `lib/depositTracking.ts` | 212 | `markDepositConfirmed(depositAddress)` | `confirmed=true`, `confirmed_at=now()` 업데이트 |
| `lib/depositTracking.ts` | 253 | `getAllPendingDeposits()` | `confirmed=false` 전체 조회 |
| `lib/depositTracking.ts` | 289 | `getDepositTrackingBySwapWallet(swapWalletAddress)` | EVM 주소로 역조회 (content page용) |
| `lib/depositTracking.ts` | 323 | `updateDepositTracking(depositAddress, updates)` | 부분 업데이트 (x402 실행 결과, tx hash 등) |
| `lib/depositTracking.ts` | 365 | `getDepositsWithDeadlineRemaining()` | `deadline > now` 필터 — cronjob이 폴링할 대상 반환 |
| `lib/depositTracking.ts` | 401 | `getAllDeposits()` | 전체 조회 |
| `lib/depositTracking.ts` | 26 | `const depositTracking = new Map<string, DepositTracking>()` | in-memory fallback 저장소 |
| `lib/supabase-server.ts` | 16 | `supabaseServer` | service role key로 생성된 Supabase 클라이언트 (RLS 우회) |
| `app/api/relayer/register-deposit/route.ts` | 14 | `POST(request)` | 신규 deposit 등록; 1Click quote 호출 → `registerDeposit()` |
| `app/api/relayer/check-deposit/route.ts` | 85 | `POST(request)` | 단건 상태 조회; 1Click SDK `getExecutionStatus` 직접 호출 |
| `app/api/relayer/check-deposit/route.ts` | 17 | `checkDepositStatus(depositAddress, tracking?)` | 내부 helper; 1Click SDK status → 정규화 반환 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 15 | `GET(request)` | 크론 엔드포인트; 만료 미도래 전체 deposit 순회 → status 폴링 → x402 실행 |
| `app/api/relayer/submit-tx-hash/route.ts` | 9 | `POST(request)` | 사용자가 제출한 tx hash를 1Click SDK `submitDepositTx()`로 전달 (optional) |
| `app/api/relayer/test-supabase/route.ts` | 7 | `GET(request)` | Supabase 연결 및 테이블 존재 여부 테스트 (debug용) |
| `scripts/run-cronjob.js` | 18 | `runCronjob()` | 로컬 개발용 5초 폴링 루프; `GET /api/relayer/cronjob-check-deposits` 반복 호출 |
| `lib/oneClick.ts` | 138 | `checkSwapStatus(depositAddress)` | `OneClickService.getExecutionStatus(depositAddress)` 래퍼 |
| `lib/oneClick.ts` | 152 | `submitTxHash(txHash, depositAddress)` | `OneClickService.submitDepositTx({txHash, depositAddress})` 래퍼 |

---

## 연결 (Wiring)

```
                    ┌─────────────────────────────────────────────────────────────┐
                    │                  deposit tracking 서브시스템                  │
                    │                                                             │
  [§1.1 intent]     │                                                             │
  ParsedIntent ────▶│ POST /api/relayer/register-deposit                          │
  (amount, chain,  │   ↓  getSwapQuote()  [lib/oneClick.ts:65]                   │
   recipient,      │   ↓  1Click /v0/quote ──→ depositAddress                    │
   redirectUrl)    │   ↓  registerDeposit() → Supabase deposit_tracking (upsert) │
                    │                                                             │
  사용자 (선택적)    │ POST /api/relayer/submit-tx-hash                             │
  ZEC tx hash ─────▶│   ↓  submitTxHash() → 1Click submitDepositTx()              │
                    │   ↓  updateDepositTracking(txHashSubmitted=true)            │
                    │                                                             │
  Vercel cron      │ GET /api/relayer/cronjob-check-deposits  (*/1 * * * *)      │
  (1분마다) ────────▶│   ↓  getDepositsWithDeadlineRemaining()                     │
                    │   ↓  for each deposit:                                      │
                    │       checkSwapStatus(depositAddress)                       │
                    │       → OneClickService.getExecutionStatus() [§1.5]        │
                    │       if status == SUCCESS:                                 │
                    │           signX402TransactionWithChainSignature() [§1.7]   │
                    │           updateDepositTracking(signedPayload=txHash,       │
                    │                               x402Executed=true)           │
                    │                                                             │
  UI (폴링)         │ POST /api/relayer/check-deposit                             │
  (address or      │   ↓  checkSwapStatus() → 1Click SDK status                 │
   signedData)─────▶│   ↓  if confirmed: markDepositConfirmed()                   │
                    │   ↓  return { signedPayload, x402Executed, ... }           │
                    └─────────────────────────────────────────────────────────────┘
                                                    │ signedPayload (Ethereum tx hash)
                                                    ▼
                                           app/content/page.tsx  [§1.7]
                                           (x402 content unlock)
```

- **Inputs:**
  - `POST /api/relayer/register-deposit`: `{ intentId, intentType, amount, recipient, senderAddress, chain, redirectUrl, serviceId, metadata }` — intent parser 결과 (`app/api/relayer/register-deposit/route.ts:16`)
  - `POST /api/relayer/submit-tx-hash`: `{ txHash, depositAddress }` — 사용자가 직접 제출하는 ZEC 트랜잭션 해시 (optional, 속도 향상용)
  - `POST /api/relayer/check-deposit`: `{ address?, signedData? }` — UI가 주기적으로 폴링
  - `GET /api/relayer/cronjob-check-deposits`: Vercel cron이 `*/1 * * * *` 스케줄로 호출
- **Outputs:**
  - `register-deposit` 응답: `{ depositAddress, swapId, quote, zcashAmount, deadline, ... }` → UI에서 QR 코드 렌더링
  - `check-deposit` 응답: `{ confirmed, status, signedPayload, x402Executed, redirectUrl, ... }` → UI가 content page로 redirect
  - `cronjob-check-deposits` 응답: `{ success, checked, results[] }` (주로 로그/디버그용)
  - Supabase `deposit_tracking` 테이블 업데이트: `signedPayload` (Ethereum tx hash), `x402Executed=true`, `confirmed=true`
- **Dependencies (internal):**
  - `lib/oneClick.ts` — `checkSwapStatus`, `submitTxHash` ([§1.5 1Click bridge](./05-one-click-bridge.md))
  - `lib/chainSig.ts` — `signX402TransactionWithChainSignature` ([§1.7 x402 client](./07-x402-client.md))
  - `lib/chainSig.ts` — `getEthereumAddressFromProxyAccount` (`register-deposit`에서 swap 수신 EVM 주소 파생)
  - `lib/supabase-server.ts` — `supabaseServer` (service role key 클라이언트)
- **Dependencies (external):**
  - 1Click API `https://1click.chaindefuser.com` — `/v0/quote`, `getExecutionStatus`, `submitDepositTx`
  - Supabase — `deposit_tracking` 테이블 CRUD

---

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `@supabase/supabase-js` | `^2.86.0` | `supabase-server.ts`의 service role client; `deposit_tracking` 테이블 upsert/select/update (`lib/depositTracking.ts:146`) |
| `@defuse-protocol/one-click-sdk-typescript` | `0.1.14` | `OneClickService.getExecutionStatus(depositAddress)` — swap 상태 폴링; `OneClickService.submitDepositTx()` — tx hash 제출 (`lib/oneClick.ts:141`, `lib/oneClick.ts:155`) |
| `next` | `^15.0.0` | API route 인프라 (`NextRequest`, `NextResponse`) |

---

## Supabase 스키마 (`supabase-deposit-tracking.sql`)

```sql
-- supabase-deposit-tracking.sql:5–25
CREATE TABLE IF NOT EXISTS deposit_tracking (
  deposit_address TEXT PRIMARY KEY,          -- 1Click deposit address = 주문 식별자
  intent_id TEXT NOT NULL,
  amount TEXT NOT NULL,                      -- USDC 금액 (문자열)
  recipient TEXT,                            -- x402 결제 수신자 (원래 payment address)
  swap_wallet_address TEXT,                  -- NEAR Chain Sig으로 파생된 EVM 주소 (swap 수신)
  near_account_id TEXT,                      -- Chain Sig에 사용되는 NEAR account
  created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
  confirmed BOOLEAN DEFAULT false,
  confirmed_at TIMESTAMP WITH TIME ZONE,
  swap_id TEXT,
  intent_type TEXT,
  chain TEXT,                                -- 'base' | 'solana'
  x402_executed BOOLEAN DEFAULT false,
  redirect_url TEXT,
  tx_hash_submitted BOOLEAN DEFAULT false,
  deposit_tx_hash TEXT,                      -- 사용자가 submit한 ZEC tx hash
  quote_data JSONB,        -- 1Click /v0/quote 전체 응답 (exchange rate, amountIn 등 포함)
  deadline TIMESTAMP WITH TIME ZONE,        -- quote 유효기한 (cronjob 필터 기준)
  signed_payload TEXT      -- x402 실행 후 저장되는 Ethereum 트랜잭션 해시
);
```

인덱스:
```sql
-- supabase-deposit-tracking.sql:28–38
CREATE INDEX IF NOT EXISTS deposit_tracking_intent_id_idx ON deposit_tracking(intent_id);
CREATE INDEX IF NOT EXISTS deposit_tracking_swap_wallet_address_idx ON deposit_tracking(swap_wallet_address);
CREATE INDEX IF NOT EXISTS deposit_tracking_confirmed_idx ON deposit_tracking(confirmed);
CREATE INDEX IF NOT EXISTS deposit_tracking_deadline_idx ON deposit_tracking(deadline);
CREATE INDEX IF NOT EXISTS deposit_tracking_x402_executed_idx ON deposit_tracking(x402_executed);
CREATE INDEX IF NOT EXISTS deposit_tracking_created_at_idx ON deposit_tracking(created_at);

-- deadline 필터 최적화 (partial index)
CREATE INDEX IF NOT EXISTS deposit_tracking_deadline_remaining_idx
ON deposit_tracking(deadline)
WHERE confirmed = false AND deadline IS NOT NULL;
```

RLS:
```sql
-- supabase-deposit-tracking.sql:52
ALTER TABLE deposit_tracking DISABLE ROW LEVEL SECURITY;
-- → service role key로만 접근하므로 RLS 비활성화; anon key 접근 없음
```

> `payment_services` 테이블(`supabase-setup.sql`)과 달리 `vector` extension이 없다. pgvector는 서비스 레지스트리 전용.

---

## 워크스루 — happy path

### 상태 머신 다이어그램

1Click API가 정의하는 실제 status 값 (주석: `app/api/relayer/check-deposit/route.ts:6–15`) 및 PAL 내부 boolean flag를 기반으로 한 상태 전이:

```
                ┌──────────────────────────────────────────────┐
                │       1Click status (외부)                    │
                │       PAL DB flag (내부)                      │
                └──────────────────────────────────────────────┘

  [register-deposit 호출]
          │
          ▼
  ┌─────────────────┐
  │  PENDING_DEPOSIT │  ← confirmed=false, x402_executed=false
  │  (1Click status) │    deposit_address가 Supabase에 저장됨
  └────────┬────────┘
           │  사용자가 ZEC를 deposit_address로 송금
           │  (1Click이 체인 상에서 감지)
           ▼
  ┌─────────────────┐
  │   PROCESSING    │  ← 1Click이 Market Maker를 통해 swap 실행 중
  │  (1Click status) │    PAL은 폴링만 함
  └────────┬────────┘
           │  swap 완료 (USDC가 swapWalletAddress에 도착)
           │  ── 또는 ──
           │  INCOMPLETE_DEPOSIT: 입금액 부족
           │  REFUNDED: swap 실패, refundTo로 ZEC 반환
           │  FAILED: swap 실패
           ▼
  ┌─────────────────┐
  │    SUCCESS      │  ← 1Click status == 'SUCCESS'
  │  (1Click status) │    PAL cronjob이 이 상태를 감지
  └────────┬────────┘
           │  signX402TransactionWithChainSignature() 호출 [§1.7]
           │  (app/api/relayer/cronjob-check-deposits/route.ts:127)
           ▼
  ┌─────────────────────────────┐
  │  x402_executed=true         │  ← Supabase 업데이트
  │  signed_payload=txHash      │    txHash = Ethereum 트랜잭션 해시
  │  confirmed=true             │    (ERC-20 transferWithAuthorization)
  └────────┬────────────────────┘
           │  UI가 check-deposit 폴링에서 signedPayload 수신
           ▼
  ┌─────────────────┐
  │   DONE (UI)     │  ← app/content/page.tsx가 signedPayload로
  │                 │    x402 content를 unlock
  └─────────────────┘
```

### 1단계: Deposit 등록 — `POST /api/relayer/register-deposit`

`app/page.tsx`의 `generateDepositAddress()`가 인텐트 정보를 담아 서버에 POST한다.

```typescript
// app/api/relayer/register-deposit/route.ts:46–47
const swapWallet = await getEthereumAddressFromProxyAccount()
// → NEAR Chain Signatures로 파생된 EVM 주소 (swap USDC 수신용)

// route.ts:55–91
const quote = await getSwapQuote({
  senderAddress: senderAddress || 'anyone-pay.near',
  recipientAddress: swapWallet,        // swap 후 USDC를 받을 EVM 주소
  originAsset: ASSETS.ZCASH,           // 'nep141:zec.omft.near'
  destinationAsset: usdcAsset,         // USDC_BASE or USDC_SOLANA
  amount: usdcToSmallestUnit(amount),
  dry: false,                          // 실제 실행 (테스트 아님)
})

depositAddress = quote.depositAddress  // 1Click이 생성·반환한 주소
swapId = depositAddress                // deposit address가 곧 swap order ID
```

서버는 `registerDeposit(depositAddress, intentId, amount, recipient, swapId, ...)` 를 호출하여 Supabase에 저장하고, 응답에 `{ depositAddress, zcashAmount, deadline, ... }` 를 포함시킨다 (`route.ts:242–250`).

**신뢰 가정:** 이 단계에서 `depositAddress`가 실제 유효한 Zcash 주소인지 PAL은 검증하지 않는다. 1Click API가 반환한 값을 그대로 사용한다.

### 2단계: Cron 폴링 루프 — `GET /api/relayer/cronjob-check-deposits`

Vercel이 `*/1 * * * *` (매 1분)마다 이 endpoint를 GET으로 호출한다.

```typescript
// app/api/relayer/cronjob-check-deposits/route.ts:26–47
const deposits = await getDepositsWithDeadlineRemaining()
// → Supabase: SELECT * WHERE deadline > now() (partial index 활용)

for (const [depositAddress, tracking] of deposits) {
  // 1Click SDK를 폴링 (체인 직접 조회 없음)
  const statusResponse = await checkSwapStatus(depositAddress)
  // checkSwapStatus = OneClickService.getExecutionStatus(depositAddress)
  //                                  [lib/oneClick.ts:141]

  const status = statusResponse.status ||
                 statusResponse.executionStatus ||
                 statusResponse.state ||
                 'PENDING_DEPOSIT'
  const normalizedStatus = String(status).toUpperCase()
  // → 'PENDING_DEPOSIT' | 'PROCESSING' | 'SUCCESS' |
  //   'INCOMPLETE_DEPOSIT' | 'REFUNDED' | 'FAILED'

  if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed) {
    // x402 실행 분기 (Step 3으로)
  }
}
```

`getDepositsWithDeadlineRemaining()`의 Supabase 쿼리:
```typescript
// lib/depositTracking.ts:373–376
await supabaseServer
  .from('deposit_tracking')
  .select('*')
  .not('deadline', 'is', null)
  .gt('deadline', now)          // deadline이 아직 미도래인 것만
```

**신뢰 가정:** PAL은 1Click SDK의 `getExecutionStatus()` 응답을 blind trust한다. ZEC 입금이 실제로 Zcash blockchain에서 확인됐는지 여부를 독립적으로 검증하는 코드가 없다.

### 3단계: x402 결제 실행 — status == 'SUCCESS'

```typescript
// app/api/relayer/cronjob-check-deposits/route.ts:62–132
if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed) {
  const quoteData = typeof tracking.quoteData === 'string'
    ? JSON.parse(tracking.quoteData)
    : tracking.quoteData

  // quote에서 x402 결제 파라미터 추출
  const payTo = quote?.payTo || tracking.recipient || quote?.recipient
  const maxAmountRequired = quote?.maxAmountRequired || quote?.amount || tracking.amount
  const deadline = Math.floor(Date.now() / 1000) + 3600  // 현재 시각 + 1시간 (고정)
  const nonce = `0x${Date.now().toString(16)}`

  // NEAR Chain Signatures로 EVM tx 서명 후 브로드캐스트 [§1.7]
  const { signX402TransactionWithChainSignature } = await import('@/lib/chainSig')
  const transactionHash = await signX402TransactionWithChainSignature({
    payTo,
    maxAmountRequired: String(maxAmountRequired),
    deadline: Math.floor(Date.now() / 1000) + 3600,
    nonce: String(nonce),
  })

  // Ethereum tx hash를 signed_payload 컬럼에 저장
  await updateDepositTracking(depositAddress, {
    signedPayload: transactionHash,  // Ethereum tx hash (ERC-20 transferWithAuthorization)
    x402Executed: true,
    confirmed: true,
    confirmedAt: Date.now()
  })
}
```

**컬럼명과 실제 값의 불일치 주의:** `signed_payload` 컬럼에 저장되는 것은 "signed payload"(서명된 바이트)가 아니라 이미 브로드캐스트된 **Ethereum 트랜잭션 해시** 문자열이다 (`app/api/relayer/cronjob-check-deposits/route.ts:135`). 컬럼명이 오해를 유발한다.

### 4단계: UI 폴링 및 content unlock — `POST /api/relayer/check-deposit`

```typescript
// app/api/relayer/check-deposit/route.ts:85–198 (핵심 발췌)
const status = await checkDepositStatus(statusAddress, tracking)
// → 1Click SDK getExecutionStatus 직접 호출 (cronjob과 독립적으로)

if (tracking && status.confirmed && !tracking.confirmed) {
  await markDepositConfirmed(statusAddress)
}

return NextResponse.json({
  confirmed,
  status: status.status || 'PENDING_DEPOSIT',
  signedPayload: tracking?.signedPayload,  // cronjob이 저장한 Ethereum tx hash
  x402Executed: tracking?.x402Executed || false,
  redirectUrl: tracking?.redirectUrl,
  // ...
})
```

UI는 `signedPayload`가 채워졌음을 감지하면 `redirectUrl`로 이동하고 content page가 `signedPayload`(Ethereum tx hash)를 x402 결제 증명으로 사용하여 paywall을 해제한다 ([§1.7 x402 client](./07-x402-client.md)).

### 5단계 (선택적): tx hash 제출 — `POST /api/relayer/submit-tx-hash`

```typescript
// app/api/relayer/submit-tx-hash/route.ts:33–38
await submitTxHash(txHash, depositAddress)
// → OneClickService.submitDepositTx({ txHash, depositAddress })
// 1Click에 ZEC tx hash를 알려 swap 처리를 앞당김 (optional)

await updateDepositTracking(depositAddress, {
  txHashSubmitted: true,
  depositTxHash: txHash  // Supabase deposit_tx_hash 컬럼에 저장
})
```

이 엔드포인트는 사용자가 Zcash 지갑에서 txid를 수동으로 복사해 전달하는 UX를 상정한다. 서버는 해당 tx hash의 유효성이나 체인 존재 여부를 검증하지 않고 (`if txHash.length < 10`만 체크, `submit-tx-hash/route.ts:24`) 그대로 1Click에 전달한다.

---

## 노트 / 특이사항 / 주의점 (Notes / quirks / footguns)

### 1. Trust verdict (신뢰 판정): PAL은 1Click을 blind trust한다

**명시적 판정:** PAL은 Zcash 체인 상태를 직접 검증하지 않는다. lightwalletd, Zebra RPC, 또는 기타 Zcash 풀 노드/SPV 클라이언트와의 통신이 코드베이스 어디에도 없다. 유일한 "확인" 수단은 1Click SDK의 `getExecutionStatus(depositAddress)` 응답이며 (`lib/oneClick.ts:141`, `app/api/relayer/cronjob-check-deposits/route.ts:34`), 이 API가 `status: "SUCCESS"`를 반환하면 PAL은 그것을 사실로 간주하고 즉시 x402 결제를 실행한다. 이는 §7 open question("Is the Supabase deposit tracking actually verifying chain state via lightwalletd or RPC, or just trusting a webhook from 1Click?")에 대한 직접적인 답이다 — 단, 1Click 소통 방식은 webhook(inbound)이 아니라 **폴링(outbound)**이다.

이 신뢰 모델의 함의:
- 1Click이 정직하게 동작하는 한 시스템은 작동한다.
- 1Click API가 침해되거나 잘못된 `SUCCESS`를 반환하면 PAL은 아무 ZEC를 받지 않았어도 x402 결제를 실행한다.
- 사용자 또한 tx hash를 임의로 제출할 수 있지만 (`submit-tx-hash`), 이는 swap 가속화 용도일 뿐이며 x402 실행 트리거가 되지는 않는다 (트리거는 오직 1Click status == SUCCESS).

### 2. 실제 cron 스케줄 — "every 5 seconds" 주장 해소

DEPLOY.md와 `scripts/run-cronjob.js`의 주석(`INTERVAL_MS = 5000`, `run-cronjob.js:17`)은 "5초마다" 실행한다고 명시한다. 그러나 **Vercel에 실제로 등록된 스케줄은 다르다:**

```json
// vercel.json:7–11
"crons": [
  {
    "path": "/api/relayer/cronjob-check-deposits",
    "schedule": "*/1 * * * *"
  }
]
```

`*/1 * * * *`는 **매 1분**마다 실행하는 standard cron 표현식이다. Vercel Hobby tier의 cron 최소 주기는 1분이며, Pro tier 이상에서만 더 짧은 주기가 허용된다. "5초마다"는 `scripts/run-cronjob.js`가 로컬 개발용으로 제공하는 script의 동작이며, Vercel deployment에서의 실제 최소 주기는 **1분**이다.

요약:
- Vercel cron: `*/1 * * * *` = **1분 주기** (`vercel.json:9`)
- 로컬 개발 script: 5000ms = **5초 주기** (`scripts/run-cronjob.js:17`, 로컬 전용)
- DEPLOY.md의 "every 5 seconds" 주장: **Vercel 배포 환경에서는 틀림** (로컬 script 동작을 오기재)

### 3. In-memory fallback — Supabase 미설정 시

`SUPABASE_SERVICE_ROLE_KEY` (또는 `NEXT_PUBLIC_SUPABASE_URL`) 환경변수가 없으면 `supabaseServer`는 `null`이 된다 (`lib/supabase-server.ts:16`). 이 경우 `lib/depositTracking.ts`의 모든 함수는 `if (supabaseServer)` 분기를 건너뛰고 프로세스 메모리 `Map<string, DepositTracking>`에 직접 읽고 쓴다 (`lib/depositTracking.ts:26`).

**중요한 문제점들:**
- Next.js serverless 환경에서는 invocation마다 새 프로세스가 뜨므로, in-memory Map이 **invocation 간에 공유되지 않는다.** `register-deposit`에서 저장한 데이터가 `check-deposit` invocation에서는 없을 수 있다.
- Vercel의 경우 각 API route 호출이 독립적인 Lambda 함수 실행이므로, Supabase 없이는 deposit tracking이 사실상 동작하지 않는다.
- In-memory fallback은 로컬 `npm run dev` (단일 Node 프로세스) 환경에서만 의미가 있다.

경고 로그: `lib/supabase-server.ts:9` — `"⚠️ Supabase service role key not found. Using in-memory storage as fallback."`

### 4. `test-supabase` 엔드포인트 — 프로덕션 보안 위험

`GET /api/relayer/test-supabase`는 인증 없이 누구나 호출할 수 있다:

```typescript
// app/api/relayer/test-supabase/route.ts:7–84
export async function GET(request: NextRequest) {
  // ← 인증 체크 없음. CRON_SECRET, API key 등 어떤 guard도 없음.
  if (!supabaseServer) {
    return NextResponse.json({ success: false, ... }, { status: 500 })
  }
  // Test 2: 실제로 테스트 row를 INSERT하고 DELETE함
  const testData = {
    deposit_address: 'test-' + Date.now(),
    intent_id: 'test-intent',
    ...
  }
  await supabaseServer.from('deposit_tracking').upsert(testData, ...).select()
  await supabaseServer.from('deposit_tracking').delete().eq('deposit_address', testData.deposit_address)
```

이 엔드포인트는:
1. Supabase 연결 정보(URL, service role key 사용 여부)를 노출한다.
2. 프로덕션 DB에 `test-{timestamp}` row를 생성 후 삭제한다 (경쟁 조건 시 잔류 가능).
3. 공개 URL에서 인증 없이 접근 가능하다.

Vercel 배포 환경에서 이 endpoint를 비활성화하거나 `CRON_SECRET` 등으로 보호하지 않으면 외부에서 Supabase 연결 상태를 무제한 조회/테스트할 수 있다. 현 코드 상태에서는 **프로덕션 보안 리스크**로 분류된다.

크론 핸들러(`cronjob-check-deposits`)도 마찬가지로 인증이 주석 처리되어 있다:
```typescript
// app/api/relayer/cronjob-check-deposits/route.ts:17–21
// Optional: Add authentication/authorization check here
// const authHeader = request.headers.get('authorization')
// if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
//   return NextResponse.json({ error: 'Unauthorized' }, { status: 401 })
// }
```

이 주석 처리된 코드가 활성화되지 않으면 누구나 cron endpoint를 임의 호출하여 x402 결제 실행을 시도할 수 있다.

### 5. x402 실행 실패 시 재시도 및 복구 처리

`cronjob-check-deposits`에서 `signX402TransactionWithChainSignature()` 호출이 실패하면:

```typescript
// app/api/relayer/cronjob-check-deposits/route.ts:149–157
} catch (error) {
  console.error(`  ❌ Error executing x402 payment for ${depositAddress}:`, error)
  results.push({
    depositAddress,
    status: normalizedStatus,
    action: 'x402_error',
    error: error instanceof Error ? error.message : 'Unknown error'
  })
}
```

`x402Executed`와 `signedPayload`가 업데이트되지 않으므로, **다음 cron 실행(1분 후)에 동일한 deposit이 다시 시도**된다. 단, deadline이 아직 유효해야 한다 (`getDepositsWithDeadlineRemaining()`의 필터 조건, `lib/depositTracking.ts:373–376`). deadline이 만료되면 해당 deposit은 cron 순회 목록에서 제외되고 **영구적으로 x402가 실행되지 않는다.**

문제점 정리:
- **x402 실행 실패 후 deadline 만료:** 사용자는 ZEC를 납부했지만 content를 받지 못한다. PAL에는 이 경우에 대한 사용자 알림이나 자동 환불 로직이 없다.
- **ZEC는 1Click의 `refundTo` 주소로 반환**(`refundTo: process.env.REFUND_ZCASH_ADDRESS || params.senderAddress`, `lib/oneClick.ts:86`)될 수도 있지만, 이는 swap 자체가 실패한 경우(`REFUNDED` status)에 한정된다 — x402 실행 단계의 실패는 별개다.
- **중복 실행 방지:** `!tracking.signedPayload && !tracking.x402Executed` 조건(`route.ts:47`)으로 이미 x402가 실행된 경우 재실행을 방지한다. 이 idempotency guard는 정상 경로에서는 작동하지만, 실행 실패 시 재시도를 막지 않는다 (의도적 설계).
- **사용자 환불 endpoint:** DEPLOY.md는 `POST /api/relayer/refund`가 존재한다고 주장하지만, `app/api/relayer/` 디렉토리에는 이 route가 없다. 환불 로직은 구현되지 않았다.

### 6. `quote_data` JSONB 활용 및 `payTo` 파라미터의 불확실성

x402 실행 시 `payTo` (결제 수신 주소)를 다음 우선순위로 탐색한다:
```typescript
// cronjob-check-deposits/route.ts:85
const payTo = quote?.payTo || tracking.recipient || quote?.recipient
```

`tracking.recipient`는 intent parser가 추출한 원래 x402 결제 주소(예: `0x03fBbA...`)이다. 그러나 `quote?.payTo`가 1Click quote 응답에 항상 포함된다는 보장이 없으며, quote 응답 구조에 따라 `payTo`가 `undefined`가 될 수 있다 (`route.ts:90–104`의 missing fields 처리 참조). 이는 실제 결제 수신자 주소가 잘못 설정될 수 있는 잠재적 버그다.

---

## 답한 open questions (spec §7)

### §7 질문: "Is the Supabase deposit tracking actually verifying chain state (via lightwalletd or RPC), or just trusting a webhook from 1Click?"

**답: PAL은 체인 상태를 직접 검증하지 않는다. 1Click SDK의 `getExecutionStatus()` 폴링 응답을 그대로 신뢰한다.**

더 정확히는, 1Click과의 통신이 "webhook(inbound)"이 아니라 "polling(outbound)"이라는 점에서 질문의 전제도 수정이 필요하다:

- 1Click이 PAL에 inbound webhook을 보내는 코드는 전혀 없다.
- PAL의 cronjob이 `OneClickService.getExecutionStatus(depositAddress)`를 매 1분마다 능동적으로 호출한다 (`lib/oneClick.ts:141`, `app/api/relayer/cronjob-check-deposits/route.ts:34`).
- 이 응답에서 `status == 'SUCCESS'`가 되면 PAL은 즉시 x402 결제를 실행한다.
- lightwalletd, zebrad, Zcash RPC, 또는 기타 Zcash 체인 조회 코드는 PAL 전체 코드베이스에 단 한 줄도 없다.

**파일:라인 근거:**
- `lib/oneClick.ts:138–147` — `checkSwapStatus`가 `OneClickService.getExecutionStatus(depositAddress)`만 호출
- `app/api/relayer/cronjob-check-deposits/route.ts:34` — cronjob의 유일한 상태 확인 수단
- `app/api/relayer/check-deposit/route.ts:20` — UI 폴링도 동일하게 1Click SDK만 사용
- lightwalletd/Zebra import: **전체 코드베이스에 없음** (검증 완료)

---

## 크로스 레퍼런스

- **1Click SDK 상세** — [§1.5 1Click bridge](./05-one-click-bridge.md): `OneClickService.getExecutionStatus` 응답 구조, 1Click이 어떻게 ZEC를 USDC로 swap하는지
- **x402 실행 상세** — [§1.7 x402 client](./07-x402-client.md): `signX402TransactionWithChainSignature()`의 NEAR Chain Signatures 연동, ERC-20 `transferWithAuthorization` 서명
- **Z-address 생성 상세** — [§1.3 Z-address generation](./03-z-address-generation.md): deposit address가 1Click으로부터 오는 전체 경위
