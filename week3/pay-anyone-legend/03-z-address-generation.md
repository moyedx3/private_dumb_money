# §1.3 Z-address generation (Z-주소 생성)

> **핵심 판정: (C) — Outsourced.** PAL은 Zcash z-address를 자체 생성하지 않는다.
> deposit address 전체가 1Click API(`/v0/quote` 응답의 `depositAddress` 필드)에서 온다.
> `crypto.getRandomValues + 'zs1' prefix` 패턴은 이 코드베이스 어디에도 존재하지 않는다.

---

## 목적 (Purpose)

Pay Anyone Legend(PAL)의 "Z-address generation" 서브시스템은 실질적으로 **생성(generation)이 아니라 수신(reception)**이다. 사용자가 결제 인텐트를 제출하면 PAL은 1Click API(`https://1click.chaindefuser.com/v0/quote`)를 호출하고, 응답 JSON에 포함된 `depositAddress` 필드를 그대로 사용자에게 보여준다. PAL 측에는 Zcash spending key, viewing key, ZIP-32 HD 경로 유도, bech32 인코딩 등 Zcash 주소를 직접 파생할 수 있는 코드가 전혀 없다. 이 서브시스템이 담당하는 유일한 역할은 1Click으로부터 받은 deposit address를 Supabase `deposit_tracking` 테이블에 저장하고 QR 코드로 렌더링하는 것이다.

---

## 파일과 함수 (Files & functions)

| 파일 | 핵심 라인 | 역할 |
|------|-----------|------|
| `lib/oneClick.ts` | 65–134 | `getSwapQuote()` — 1Click `/v0/quote` 호출, 응답의 `depositAddress` 추출 |
| `lib/oneClick.ts` | 126 | `depositAddress: data.depositAddress \|\| data.quote?.depositAddress \|\| data.address` — 실제 추출 지점 |
| `app/api/relayer/register-deposit/route.ts` | 55–91 | `POST /api/relayer/register-deposit` — `getSwapQuote()`를 호출하여 `depositAddress`를 받고 Supabase에 저장 |
| `app/api/relayer/register-deposit/route.ts` | 66 | `depositAddress = quote.depositAddress \|\| quote.quote?.depositAddress \|\| quote.address` — register 단에서의 재추출 |
| `lib/depositTracking.ts` | 104–180 | `registerDeposit()` — `depositAddress`를 Supabase `deposit_tracking.deposit_address` (PK)로 저장 |
| `components/IntentsQR.tsx` | 186–191 | `<QRCodeSVG value={depositAddress} ...>` — QR 코드 렌더링 |
| `app/page.tsx` | 499–557 | `generateDepositAddress()` — 프론트엔드 진입점; `/api/relayer/register-deposit` 호출 후 `data.depositAddress`를 UI state에 저장 |
| `contract/deploy.sh` | 54 | `"deposit_address\":\"zs1test123\"` — 테스트 스크립트에서 하드코딩된 더미 문자열 (앱 로직과 무관) |
| `contract/test-contract.sh` | 14 | `"deposit_address\":\"zs1test123456789\"` — 동일 목적의 테스트 더미 |

> **보조 파일** `lib/kdf.ts`는 `bech32`를 import하지만, cosmos 체인과 XRP Ledger 주소를 위한 것이다. Zcash 주소와는 무관하다 (`lib/kdf.ts:8`, `lib/kdf.ts:164–165`).

---

## 연결 (Wiring)

```
사용자 입력 (intent)
        │
        ▼
app/page.tsx::generateDepositAddress()          [app/page.tsx:499]
        │  POST /api/relayer/register-deposit
        ▼
app/api/relayer/register-deposit/route.ts       [route.ts:55]
        │  getSwapQuote({ originAsset: 'nep141:zec.omft.near',
        │                 destinationAsset: USDC_BASE | USDC_SOLANA,
        │                 amount: usdcAmount,
        │                 recipientAddress: swapWallet (EVM), ... })
        ▼
lib/oneClick.ts::getSwapQuote()                 [oneClick.ts:65]
        │  POST https://1click.chaindefuser.com/v0/quote
        ▼
1Click API (외부 서비스)
        │  응답: { depositAddress: "zs1...", ... }
        ◀
lib/oneClick.ts:126  ← depositAddress 추출
        │
        ▼
app/api/relayer/register-deposit/route.ts:110
        │  registerDeposit(depositAddress, ...)
        ▼
lib/depositTracking.ts::registerDeposit()       [depositTracking.ts:104]
        │  Supabase: deposit_tracking.deposit_address (PK) ← depositAddress
        │
        ▼
app/page.tsx → components/IntentsQR.tsx         [IntentsQR.tsx:186]
        │  <QRCodeSVG value={depositAddress} size={220} level="H" />
```

- **Inputs:** 사용자 intent에서 추출된 `amount`(USDC), `chain`(base|solana), `recipient`(x402 수신자 EVM 주소)
- **Outputs:** 1Click API가 반환한 deposit address 문자열 (실질적으로는 1Click solver가 제어하는 Zcash 수신 주소); Supabase row; QR 코드
- **Dependencies (internal):** `lib/oneClick.ts` (`getSwapQuote`), `lib/depositTracking.ts` (`registerDeposit`), `lib/chainSig.ts` (`getEthereumAddressFromProxyAccount` — swap 완료 후 USDC를 받을 EVM 주소 파생에 사용)
- **Dependencies (external):** 1Click API (`https://1click.chaindefuser.com`), `@defuse-protocol/one-click-sdk-typescript@0.1.14`, Supabase

---

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `@defuse-protocol/one-click-sdk-typescript` | `0.1.14` | 1Click API SDK; `OneClickService.getExecutionStatus()`, `OneClickService.submitDepositTx()` |
| `bech32` | `^2.0.0` | **Zcash 무관** — `lib/kdf.ts`에서 cosmos/XRP Ledger 주소 인코딩에만 사용 |
| `bs58check` | `^4.0.0` | **Zcash 무관** — `lib/kdf.ts`에서 Bitcoin 주소 인코딩에만 사용 |
| `js-sha3` | `^0.9.3` | **Zcash 무관** — `lib/kdf.ts`에서 NEAR MPC 경로 derivation용 sha3_256 |
| `qrcode.react` | `^3.1.0` | QR 코드 SVG 렌더링 (`components/IntentsQR.tsx`) |

> 주목할 부재(absence): `@zec`, `zcash-wasm`, `librustzcash`, `bellman`, `zcash_client_backend`, `lightwalletd-client`, `orchard`, `sapling-crypto`, `pczt` — 실제 Zcash 암호화 라이브러리는 단 하나도 없다.

---

## 워크스루 — happy path

1. **사용자가 인텐트 제출** — 예: "Pay OnlyFans $10". `FloatingInput.tsx`가 `app/page.tsx`의 `handleSubmit`으로 전달.

2. **인텐트 파싱** — `lib/nearAI.ts::analyzePromptWithNearAI()` 가 OpenAI/NEAR AI LLM을 통해 `{ amount: "10", currency: "USDC", chain: "base", bridgeFrom: "zcash", receivingAddress: "0x..." }`를 추출 (`lib/nearAI.ts:43–44`, `lib/nearAI.ts:93–95`).

3. **generateDepositAddress 호출** — `app/page.tsx:499`. `intentId = intent-{timestamp}` 생성 후 `/api/relayer/register-deposit`에 POST.

4. **서버 사이드: 1Click quote 요청** — `app/api/relayer/register-deposit/route.ts:55` 에서 `getSwapQuote()` 호출:

   ```typescript
   // lib/oneClick.ts:65–134 (핵심 발췌)
   const quoteRequest: QuoteRequest = {
     dry: false,
     swapType: 'EXACT_OUTPUT',
     slippageTolerance: 100,               // 1%
     originAsset: 'nep141:zec.omft.near',  // ASSETS.ZCASH
     depositType: 'ORIGIN_CHAIN',
     destinationAsset: 'nep141:base-0x8335...omft.near', // USDC_BASE
     amount: usdcToSmallestUnit(amount),   // USDC 단위 변환
     refundTo: senderAddress,
     recipient: swapWallet,                // NEAR Chain Sig으로 파생된 EVM 주소
     recipientType: 'DESTINATION_CHAIN',
     deadline: new Date(Date.now() + 3*60*1000).toISOString(),
   }
   const response = await fetch(`${ONE_CLICK_API_URL}/v0/quote`, {
     method: 'POST',
     body: JSON.stringify(quoteRequest),
   })
   const data = await response.json()
   // ↓↓↓ 이것이 유일한 "address 생성" 지점 ↓↓↓
   depositAddress = data.depositAddress || data.quote?.depositAddress || data.address
   //              ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
   //              PAL이 아니라 1Click이 생성·반환한 주소. 이 라인이 전부다.
   ```
   (`lib/oneClick.ts:102–126`)

5. **Supabase 저장** — `registerDeposit(depositAddress, ...)` 가 `deposit_tracking` 테이블에 `deposit_address` (PK)로 저장 (`lib/depositTracking.ts:104–180`).

6. **프론트엔드로 반환** — `route.ts:242–250`에서 `{ depositAddress, zcashAmount, deadline, ... }` JSON 응답.

7. **QR 코드 렌더링** — `app/page.tsx:924`에서 `<IntentsQR depositAddress={intentData.depositAddress} ...>` 렌더링; `components/IntentsQR.tsx:186`에서 `<QRCodeSVG value={depositAddress} size={220} level="H">`. **QR에는 주소 문자열만 인코딩**된다; ZIP-321 `zcash:zs1...?amount=...` URI 형식은 사용하지 않는다.

---

## 노트 / 특이사항 / 주의점 (Notes / quirks / footguns)

### 1. 판정: (C) — Outsourced

**명시적 판정: 이 코드베이스의 deposit address 생성은 Category (C) — Outsourced다.** PAL은 Zcash z-address를 자체 생성하지 않는다. 1Click API가 반환한 주소를 수동 전달(pass-through)할 뿐이다. `crypto.getRandomValues + 'zs1' prefix` 합성 문자열 패턴(B)도 존재하지 않는다. `deploy.sh`와 `test-contract.sh`에 `zs1test123` 같은 하드코딩 더미가 있지만, 이는 NEAR 컨트랙트 테스트 스크립트의 placeholder일 뿐 앱 로직과 완전히 분리된다 (`contract/deploy.sh:54`, `contract/test-contract.sh:14`).

### 2. 주소 생성 함수 원문 (verbatim)

```typescript
// lib/oneClick.ts:122–128 — PAL이 "생성"하는 전부
return {
  ...data,
  depositAddress: data.depositAddress || data.quote?.depositAddress || data.address,
  swapId: data.swapId || data.id || data.depositAddress,
  sessionId: responseSessionId,
}
```

```typescript
// app/api/relayer/register-deposit/route.ts:66–69
depositAddress = quote.depositAddress || quote.quote?.depositAddress || quote.address
if (!depositAddress) {
  throw new Error('No deposit address found in quote response')
}
```

이 두 줄이 PAL의 "z-address generation" 코드 전체다.

### 3. 시스템이 그럼에도 작동하는 이유

ZEC의 입금 추적은 deposit address 자체의 유효성이 아니라 **1Click 서비스의 내부 상태(swap order)**에 의존한다. `deposit_tracking.deposit_address`는 primary key로 사용되어 1Click API의 `OneClickService.getExecutionStatus(depositAddress)` 호출 키로 쓰인다 (`lib/oneClick.ts:141`). 즉, deposit address는 PAL에게 단지 **주문 식별자(order ID)**로 기능한다. 실제 ZEC 수신과 USDC 스왑은 1Click(Defuse Protocol)이 전담한다.

### 4. 실제 사용자가 이 주소로 ZEC를 송금하면?

1Click이 반환하는 `depositAddress`가 실제 Zcash z-address(또는 t-address)라면, 사용자가 ZEC를 송금했을 때 **1Click의 solver 지갑**이 수신한다. PAL에는 해당 주소의 spending key나 viewing key가 없다. 만약 1Click API가 존재하지 않거나, 잘못된 형식의 주소를 반환하거나(예: API 오류로 `undefined`), 서비스가 다운되면 — 송금된 ZEC는 회수 불가능하거나 refundTo 주소로 반환된다. PAL은 `refundTo: process.env.REFUND_ZCASH_ADDRESS || params.senderAddress`를 설정하지만 `REFUND_ZCASH_ADDRESS` 환경변수는 선택적이다 (`lib/oneClick.ts:86`).

### 5. Category E 함의

PAL의 Zcash 통합은 완전히 (C) Outsourced다. 우리 팀이 Category E (x402 + Zcash)에서 **Zcash를 x402 settlement asset으로 직접 사용**하려면, PAL에서 재사용할 수 있는 Zcash 암호화 코드가 없다. 직접 구현해야 할 항목:

- **실제 Zcash z-address 파생**: ZIP-32 HD path + Orchard 또는 Sapling key derivation (`zcash_client_backend`, `orchard` crate, 또는 `@zec-js` 계열 라이브러리)
- **Lightwalletd 또는 Zebra RPC 연동**: 입금 확인을 위한 체인 상태 조회
- **Zcash transaction 생성 및 서명**: PCZT 또는 유사 방식
- **x402 response 서명을 Zcash 트랜잭션으로**: 현재 PAL은 EVM USDC `transferWithAuthorization`를 사용한다 (`lib/chainSig.ts:304–342`)

---

## 답한 open questions (spec §7)

### §7 질문: "Verify the week2 claim that z-address generation is `crypto.getRandomValues + 'zs1' prefix`"

**판정: 부분적으로 수정(Partially Refuted / Corrected).**

week2 claim은 z-address가 어떤 형태로든 PAL이 합성(synthesize)한다는 것을 전제한다. 실제 코드를 보면:

- `crypto.getRandomValues`는 PAL의 어떤 TS/JS 파일에도 z-address 생성 목적으로 호출되지 않는다. 검색 결과 `crypto.getRandomValues`는 이 codebase에서 `lib/kdf.ts`의 Bitcoin 주소 파생 내부(`crypto.subtle.digest`)에만 나타나며, Zcash와 무관하다.
- `'zs1'` 문자열은 `contract/deploy.sh:54`와 `contract/test-contract.sh:14`의 하드코딩 테스트 더미(`"zs1test123"`, `"zs1test123456789"`)에만 등장한다. 이는 NEAR 컨트랙트의 `create_intent()` 메서드 테스트용 인수이며, 앱 런타임과 무관하다.
- 실제 앱 코드에서 deposit address는 1Click `/v0/quote` API가 반환한다 (`lib/oneClick.ts:126`, `app/api/relayer/register-deposit/route.ts:66`).

week2 claim이 완전히 틀린 것은 아니다. PAL의 Zcash 구현이 "얕다(shallow)"는 판단은 정확하다. 단, 그 방식이 "(B) random bytes + zs1 prefix"가 아니라 "(C) 1Click API에 완전 위임"이다. 더미 `zs1test123`이 week2 리뷰어에게 "mock 생성" 패턴으로 보였을 가능성이 있으나, 이는 테스트 스크립트의 shell literal이지 앱 로직이 아니다.

**파일:라인 근거:**
- `contract/deploy.sh:54` — `"deposit_address\":\"zs1test123\"` (테스트 더미; 앱 로직 아님)
- `contract/test-contract.sh:14` — `"deposit_address\":\"zs1test123456789\"` (동일)
- `lib/oneClick.ts:126` — 실제 앱에서의 deposit address 출처 (1Click API 응답)
- `app/api/relayer/register-deposit/route.ts:66` — register 단에서 재확인
- Zcash native library import: **없음** (전체 `package.json` 검증 완료)
