# Pay Anyone Legend — 구체적 예시 Walkthrough (시나리오 한 편)

> **읽는 사람:** 팀원 / 처음 이 시스템 보는 사람
> **목적:** *"진짜로 PAL을 한 번 써보면 어떤 일이 벌어지는가"* 를 한 사용자의 결제 시나리오로 처음~끝까지 따라가본다.
> **연관:** [`TEAM-WALKTHROUGH.md`](./TEAM-WALKTHROUGH.md)는 서브시스템별 분석, 이 문서는 **하나의 거래를 시간순으로 추적**.

---

## 시나리오 설정

**등장 인물:**
- 👤 **김민준** — Zcash 지갑 보유. ChatGPT Pro 구독을 한 번만 결제하고 싶음
- 🌐 **PAL 웹앱** — `https://anyone-pay.com` (가상 도메인)
- 🤖 **OpenAI / NEAR AI 클라우드** — intent 파싱용
- 🗄️ **Supabase** — PAL의 백엔드 DB
- 🔄 **1Click API** — `1click.chaindefuser.com` (Defuse Labs Limited, Gibraltar)
- ⚙️ **NEAR MPC `v1.signer`** — NEAR 메인넷 컨트랙트
- 🟦 **Base 메인넷** — USDC가 흐르는 EVM 체인
- 🛒 **가맹점 콘텐츠 서버** — ChatGPT 결제 paywall (외부)

**가정 데이터:**
- 김민준의 Zcash 주소: `t1KMjAsd...mNz9` (transparent 주소로 가정)
- 가맹점 USDC 수신 주소: `0x03fBbA1b1A455d028b074D9abC2b23d3EF786943` (Base)
- 결제 금액: **20 USDC** 상당의 ZEC
- ChatGPT 콘텐츠 URL: `https://chatgpt.com/premium-link?session=xyz`
- 시각: `2026-05-13 14:23:00 UTC` (T0)

---

## ⏱️ T0 + 0초 — 김민준 자연어 입력

**김민준이 PAL 웹앱에 접속해서 입력창에 친다:**

```
"Pay ChatGPT Pro"
```

### 무슨 일이 일어나나?

```
[브라우저: components/FloatingInput.tsx:36]
   value = "Pay ChatGPT Pro"
   onSubmit("Pay ChatGPT Pro")
        ↓
[브라우저: app/page.tsx:346 handleSubmit]
   URL에 ?prompt=Pay+ChatGPT+Pro 추가
   setIsLoading(true)
   parseIntent("Pay ChatGPT Pro")
        ↓
[브라우저: lib/intentParser.ts:16]
   POST /api/parse-intent
   body: { query: "Pay ChatGPT Pro" }
```

---

## ⏱️ T0 + 1초 — Intent parsing 서버 진입

```
[서버: app/api/parse-intent/route.ts:16]
   body { query: "Pay ChatGPT Pro" } 수신
        ↓
[서버: lib/nearAI.ts:29 analyzePromptWithNearAI()]
```

### 1단계: pgvector 시맨틱 검색

```
[lib/serviceRegistry.ts:30 generateEmbedding("Pay ChatGPT Pro")]
   → OpenAI text-embedding-3-small 호출
   → vector[1536] 반환 (예: [0.012, -0.034, ..., 0.087])
        ↓
[lib/serviceRegistry.ts:68]
   supabase.rpc('match_services', {
     query_embedding: [0.012, ...],
     match_threshold: 0.6,
     match_count: 10
   })
```

**Supabase 내부:**
```sql
-- supabase가 cosine 거리로 정렬해서 가장 가까운 service 찾음
SELECT *, 1 - (embedding <=> $1) AS similarity
FROM payment_services
WHERE active = true
  AND 1 - (embedding <=> $1) > 0.6
ORDER BY embedding <=> $1
LIMIT 10;
```

**가정:** ChatGPT가 service registry에 등록되어 있고 cosine 유사도 0.87로 매칭.

```
matchedService = {
  id:                "chatgpt-pro-1",
  name:              "ChatGPT Pro",
  amount:            "20",
  currency:          "USDC",
  chain:             "base",
  url:               "https://chatgpt.com/premium-link?session=xyz",
  receivingAddress:  "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943"
}
```

### 2단계: AnalyzedIntent 즉시 조립 (LLM 호출 X)

```typescript
// lib/nearAI.ts:36-49
return {
  action:           "pay",
  amount:           "20",
  currency:         "USDC",
  recipient:        "https://chatgpt.com/premium-link?session=xyz",
  chain:            "base",
  needsBridge:      true,
  bridgeFrom:       "zcash",   // ◄── 하드코딩
  bridgeTo:         "base",
  serviceId:        "chatgpt-pro-1",
  serviceName:      "ChatGPT Pro",
  redirectUrl:      "https://chatgpt.com/premium-link?session=xyz",
  receivingAddress: "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943"
}
```

### 3단계: API route가 ParsedIntent 래핑 후 응답

```json
{
  "intent_type": "payment",
  "amount":      "20",
  "redirect_url":"https://chatgpt.com/premium-link?session=xyz",
  "chain":       "base",
  "needsBridge": true,
  "bridgeTo":    "base",
  "metadata": {
    "action":           "pay",
    "currency":         "USDC",
    "serviceId":        "chatgpt-pro-1",
    "serviceName":      "ChatGPT Pro",
    "receivingAddress": "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943"
  }
}
```

→ 김민준 브라우저로 응답. 1초 안에 처리됨.

---

## ⏱️ T0 + 1.5초 — 클라이언트가 결제 플로우 시작

```
[브라우저: app/page.tsx:408]
   isComplete = true (모든 필드 채워짐)
        ↓
[generateDepositAddress() 호출, app/page.tsx:499]
   intentId = "intent-1715608980000"
   POST /api/relayer/register-deposit
   body: {
     intentId:       "intent-1715608980000",
     intentType:     "payment",
     amount:         "20",
     recipient:      "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943",
     senderAddress:  "t1KMjAsd...mNz9",
     chain:          "base",
     redirectUrl:    "https://chatgpt.com/premium-link?session=xyz",
     serviceId:      "chatgpt-pro-1",
     metadata:       {...}
   }
```

---

## ⏱️ T0 + 2초 — 서버가 swapWallet 파생

```
[서버: app/api/relayer/register-deposit/route.ts:47]
   swapWallet = await getEthereumAddressFromProxyAccount()
        ↓
[lib/chainSig.ts:112 → :87 deriveAddressAndPublicKey]
   evmChain.deriveAddressAndPublicKey(
     accountId: "anyone-pay.near",
     path:      "base-1"   // ◄── 하드코딩
   )
```

**MPC 파생 계산 (chainsig.js SDK 내부):**
```
epsilon = sha3_256(
  "near-mpc-recovery v0.1.0 epsilon derivation:anyone-pay.near,base-1"
) = 0x47a2c8d1...

master_pk    = (Mx, My)   ← NEAR MPC 공개키 (메인넷 고정)
child_pk     = master_pk + epsilon * G
uncompressed = 0x04 || Cx || Cy
EVM 주소     = keccak256(uncompressed[1:])[12:]
            = "0xABC1234567890DEF1234567890ABCDEF12345678"
```

→ **swapWallet 주소 결정.** 김민준뿐 아니라 모든 PAL 사용자가 이 주소를 공유함 (MPC_PATH 하드코딩 때문).

```
swapWallet = "0xABC1234567890DEF1234567890ABCDEF12345678"
```

---

## ⏱️ T0 + 2.5초 — 1Click에 quote 요청

```
[서버: lib/oneClick.ts:102]
   POST https://1click.chaindefuser.com/v0/quote
   headers: {
     "Content-Type": "application/json",
     "Authorization": "Bearer <ONE_CLICK_JWT>"  // 환경변수에 있으면
   }
   body: {
     dry:                false,
     swapType:           "EXACT_OUTPUT",
     slippageTolerance:  100,  // 1%
     originAsset:        "nep141:zec.omft.near",  // ZEC
     depositType:        "ORIGIN_CHAIN",
     destinationAsset:   "nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near",
     amount:             "20000000",  // 20 USDC (6 decimals)
     refundTo:           "t1KMjAsd...mNz9",          // 김민준 ZEC 주소
     refundType:         "ORIGIN_CHAIN",
     recipient:          "0xABC1234567890DEF1234567890ABCDEF12345678",  // swapWallet
     recipientType:      "DESTINATION_CHAIN",
     deadline:           "2026-05-13T14:26:00.000Z",  // T0 + 3분
     referral:           "anyone-pay",
     quoteWaitingTimeMs: 3000,
     sessionId:          "session_1715608980000_a7b9x"
   }
```

### 🚨 여기서 일어나는 privacy 사건

**1Click(Defuse Labs Limited, Gibraltar)이 한 번에 보게 되는 정보:**
- 김민준의 ZEC 주소 (refundTo)
- 송금 예정 ZEC 금액 (quote 응답에서 계산됨)
- 최종 USDC 수신 EVM 주소 (recipient)
- referral = "anyone-pay" (어느 앱에서 왔는지)

→ **AML 스크리닝 (TRM Labs 외 3개) 자동 적용.** 김민준이 sanctioned address였다면 여기서 거절됨.

### 1Click 응답 (T0 + 3.5초)

```json
{
  "depositAddress": "t1QcXyZ...kLmN8",   ← 1Click solver가 소유하는 transparent 주소
  "swapId":         "swap_a7b9x123",
  "sessionId":      "session_1715608980000_a7b9x",
  "quote": {
    "amountIn":          "0.123456",    // 약 0.12 ZEC
    "amountInFormatted": "0.123456 ZEC",
    "amountOut":         "20000000",
    "amountOutFormatted":"20 USDC",
    "deadline":          "2026-05-13T14:26:00.000Z",
    "payTo":             null,          // 1Click이 안 줌 → PAL이 fallback 씀
    ...
  }
}
```

---

## ⏱️ T0 + 4초 — Supabase에 deposit 등록

```
[서버: lib/depositTracking.ts:104 registerDeposit()]
   supabaseServer.from('deposit_tracking').upsert({
     deposit_address:     "t1QcXyZ...kLmN8",   ← Primary Key (1Click 주소 = 주문 ID)
     intent_id:           "intent-1715608980000",
     amount:              "20",
     recipient:           "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943",
     swap_wallet_address: "0xABC1234567890DEF1234567890ABCDEF12345678",
     near_account_id:     "anyone-pay.near",
     swap_id:             "swap_a7b9x123",
     intent_type:         "payment",
     chain:               "base",
     redirect_url:        "https://chatgpt.com/premium-link?session=xyz",
     quote_data:          { /* 전체 1Click 응답 JSON */ },
     deadline:            "2026-05-13T14:26:00.000Z",
     confirmed:           false,
     x402_executed:       false,
     tx_hash_submitted:   false,
     signed_payload:      null,
     created_at:          "2026-05-13T14:23:04.000Z"
   })
```

### 서버 응답을 클라이언트에 전달

```json
{
  "depositAddress": "t1QcXyZ...kLmN8",
  "swapId":         "swap_a7b9x123",
  "zcashAmount":    "0.123456",
  "deadline":       "2026-05-13T14:26:00.000Z",
  "quote":          {...}
}
```

---

## ⏱️ T0 + 4.5초 — QR 코드 표시

```
[브라우저: app/page.tsx:924]
   <IntentsQR depositAddress="t1QcXyZ...kLmN8" amount="0.123456" ... />
        ↓
[components/IntentsQR.tsx:186]
   <QRCodeSVG
     value="t1QcXyZ...kLmN8"   ← raw 주소 문자열 (ZIP-321 URI 아님)
     size={220}
     level="H"
   />
```

**김민준이 보는 화면:**
```
┌─────────────────────────────────────────┐
│   ChatGPT Pro 결제                       │
│                                          │
│   ┌──────────────┐                       │
│   │ ▓▓▓▓▓▓▓▓▓▓▓▓ │   QR 코드             │
│   │ ▓ QR HERE  ▓ │                       │
│   │ ▓▓▓▓▓▓▓▓▓▓▓▓ │                       │
│   └──────────────┘                       │
│                                          │
│   0.123456 ZEC 송금해주세요               │
│   주소: t1QcXyZ...kLmN8                  │
│   유효시간: 02:54 남음                    │
└─────────────────────────────────────────┘
```

> ⚠️ **이 화면의 함정:** QR이 ZIP-321 URI가 아니라 그냥 주소 문자열이라, 김민준이 Zcash 지갑에서 **수동으로 0.123456 ZEC를 입력**해야 함. 0.123455 입력하면 `INCOMPLETE_DEPOSIT` 발생.

---

## ⏱️ T0 + 30초 — 김민준 ZEC 송금

김민준이 Zcash 지갑(예: Zashi, ECC YWallet)에서:
1. QR 스캔 → 주소 자동 입력
2. amount `0.123456` 수동 입력
3. Send

```
[Zcash 메인넷에 트랜잭션 broadcast]
   from:   t1KMjAsd...mNz9  (김민준)
   to:     t1QcXyZ...kLmN8  (1Click solver)
   amount: 0.123456 ZEC
   tx hash: abc123...

[Zcash 블록체인이 confirm — 약 75초 평균]
```

### 🚨 여기서 일어나는 privacy 사건 (L1)

transparent 주소라서 **Zcash 익스플로러에서 누구나 볼 수 있음:**
- sender: `t1KMjAsd...mNz9` (김민준)
- recipient: `t1QcXyZ...kLmN8` (1Click)
- amount: 0.123456 ZEC

→ shielded라면 막혔을 정보가 다 노출됨.

---

## ⏱️ T0 + 2분 — Vercel cron 첫 polling

```
[Vercel: schedule */1 * * * *]
   GET /api/relayer/cronjob-check-deposits
        ↓
[서버: cronjob-check-deposits/route.ts:26]
   deposits = await getDepositsWithDeadlineRemaining()
   → Supabase SELECT * FROM deposit_tracking WHERE deadline > now()
   → 김민준 deposit 포함 N개 반환
        ↓
   for each deposit:
     statusResponse = await checkSwapStatus("t1QcXyZ...kLmN8")
     ↓
[lib/oneClick.ts:141]
   await OneClickService.getExecutionStatus("t1QcXyZ...kLmN8")
   → { status: "PROCESSING" }   // 아직 swap 중
        ↓
   normalizedStatus = "PROCESSING"
   → SUCCESS 아니므로 skip, 다음 1분에 재확인
```

---

## ⏱️ T0 + 3분 — Vercel cron 두 번째 polling

```
[같은 흐름 다시]
   getExecutionStatus("t1QcXyZ...kLmN8")
   → { status: "SUCCESS" }   // 💥 swap 완료!
```

**1Click 내부에서 일어난 일 (PAL 모름):**
1. solver가 김민준 ZEC 받음
2. solver가 NEAR Intents (`intents.near` 컨트랙트)로 atomic settlement
3. solver가 token bridge로 USDC 20개를 Base에 전달
4. USDC 20개가 **swapWallet `0xABC1234...` 주소에 도착** ← Base 익스플로러로 검증 가능

```
[Base 익스플로러에서 swapWallet 잔고 조회]
0xABC1234567890DEF1234567890ABCDEF12345678
  USDC balance: 20.000000
```

---

## ⏱️ T0 + 3분 1초 — x402 실행 분기 진입

```
[cronjob-check-deposits/route.ts:47]
   if (normalizedStatus === "SUCCESS"
       && !tracking.signedPayload
       && !tracking.x402Executed) {
     // 분기 진입
   }
```

### x402 파라미터 조립

```
payTo             = tracking.recipient
                  = "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943"
maxAmountRequired = "20"
deadline          = Math.floor(Date.now() / 1000) + 3600
                  = 1715612581 + 3600 = 1715616181   (T0 + 1시간 3분)
nonce             = `0x${Date.now().toString(16)}`
                  = "0x1907c52d50a"
```

---

## ⏱️ T0 + 3분 2초 — signX402TransactionWithChainSignature 호출

```
[cronjob-check-deposits/route.ts:125]
   const { signX402TransactionWithChainSignature } = await import('@/lib/chainSig')
        ↓
[lib/chainSig.ts:210]
   signX402TransactionWithChainSignature({
     payTo:             "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943",
     maxAmountRequired: "20",
     deadline:          1715616181,
     nonce:             "0x1907c52d50a"
   })
```

### Step A. swapWallet 주소 재파생 (검증)

```
evmChain.deriveAddressAndPublicKey("anyone-pay.near", "base-1")
   → address = "0xABC1234..."  (T0 + 2초에 받은 것과 동일)
```

### Step B. EIP-712 데이터 구성

```javascript
domain = {
  name:              "USD Coin",
  version:           "2",
  chainId:           8453,
  verifyingContract: "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913"  // USDC on Base
}

types = {
  TransferWithAuthorization: [
    { name: "from",        type: "address" },
    { name: "to",          type: "address" },
    { name: "value",       type: "uint256" },
    { name: "validAfter",  type: "uint256" },
    { name: "validBefore", type: "uint256" },
    { name: "nonce",       type: "bytes32" }
  ]
}

authorizationValue = {
  from:        "0xABC1234567890DEF1234567890ABCDEF12345678",  // swapWallet
  to:          "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943",  // 가맹점
  value:       20_000_000n,                                    // 20 USDC
  validAfter:  0n,
  validBefore: 1715616181n,
  nonce:       "0x00000000000000000000000000000000000000000000000001907c52d50a"
}

eip712Hash = keccak256(EIP-712-encode(domain, types, value))
           = "0x71b2c8a4..."
```

---

## ⏱️ T0 + 3분 3초 — MPC 사인 #1 시작

```
[lib/chainSig.ts:147]
   chainSignatureContract.sign({
     payloads:      [eip712Hash bytes],
     path:          "base-1",
     keyType:       "Ecdsa",
     signerAccount: account   // anyone-pay.near
   })
        ↓
[NEAR 메인넷 cross-contract call]
   anyone-pay.near → v1.signer.sign(payload, path, key_version)
   첨부: 300 TGas + 1 NEAR deposit
```

### NEAR 메인넷 내부 — MPC 노드들이 일함

```
[v1.signer 컨트랙트]
   request received: payload, path="base-1", caller="anyone-pay.near"
        ↓
[MPC 노드 N개에 broadcast]
   각 노드가 자기 secret share로 부분 서명 생성
        ↓
[threshold (예: 3-of-5) 모이면 결합]
   → big_r, s, recovery_id 생성
        ↓
[v1.signer가 caller에게 응답]
```

### T0 + 3분 18초 — MPC #1 응답 받음 (15초 후)

```
response = {
  big_r: { affine_point: "04abcd..." },
  s:     { scalar:       "ef1234..." },
  recovery_id: 0
}

→ PAL이 추출:
v = recovery_id + 27 = 27
r = first 32 bytes of big_r
s = scalar
```

---

## ⏱️ T0 + 3분 19초 — ecrecover 검증

```
[lib/chainSig.ts:283]
   recoveredAddress = ethers.utils.recoverAddress(eip712Hash, { r, s, v })
                    = "0xABC1234567890DEF1234567890ABCDEF12345678"

   if (recoveredAddress.toLowerCase() === swapWallet.toLowerCase()) {
     // ✅ 검증 통과
   } else {
     // ❌ abort (NEAR gas만 손해, Base broadcast 안 함)
   }
```

> 만약 여기서 mismatch였다면 next cron(1분 후)에 재시도. **silent on-chain failure 방지 안전망.**

---

## ⏱️ T0 + 3분 20초 — transferWithAuthorization calldata 인코딩

```javascript
iface = new ethers.utils.Interface([
  "function transferWithAuthorization(address from, address to, uint256 value, uint256 validAfter, uint256 validBefore, bytes32 nonce, uint8 v, bytes32 r, bytes32 s)"
])

calldata = iface.encodeFunctionData("transferWithAuthorization", [
  "0xABC1234...",                                                    // from = swapWallet
  "0x03fBbA1b...",                                                   // to = 가맹점
  20_000_000n,                                                       // 20 USDC
  0n,                                                                // validAfter
  1715616181n,                                                       // validBefore
  "0x00...01907c52d50a",                                             // nonce
  27,                                                                // v (사인 #1)
  "0xabcd...",                                                       // r (사인 #1)
  "0xef12..."                                                        // s (사인 #1)
])

// calldata = "0xe1560fd3000000000000000000000000abc1234..." (약 450 bytes)
```

---

## ⏱️ T0 + 3분 21초 — Legacy EVM tx 준비

```
[lib/chainSig.ts:356]
   evmChain.prepareTransactionForSigningLegacy({
     from:     "0xABC1234...",
     to:       "0x833589fcd6edb6e08f4c7c32d4f71b54bda02913",  // USDC contract
     value:    0n,
     data:     "0xe1560fd3...",
     gasPrice: 100_000_000n,    // 0.1 gwei (하드코딩)
     gas:      150_000n,         // (하드코딩)
   })
        ↓
   {
     transaction: { nonce: 47, gasPrice, gas, to, value, data, chainId: 8453, ... },
     hashesToSign: ["0x9f3e..."]   // RLP-encoded tx의 keccak256
   }
```

---

## ⏱️ T0 + 3분 22초 — MPC 사인 #2 시작

```
[lib/chainSig.ts:372]
   chainSignatureContract.sign({
     payloads:      hashesToSign,   // ["0x9f3e..."]
     path:          "base-1",
     keyType:       "Ecdsa",
     signerAccount: account
   })
```

### T0 + 3분 38초 — MPC #2 응답 받음 (16초 후)

```
response = {
  big_r: { affine_point: "04beef..." },
  s:     { scalar:       "cafe..." },
  recovery_id: 1
}

v = 28
r = ...
s = ...
```

---

## ⏱️ T0 + 3분 39초 — 사인 삽입 + Base broadcast

```
[lib/chainSig.ts:388]
   signedTx = evmChain.finalizeTransactionSigningLegacy({
     transaction: preparedTx,
     rsvSignatures: signature
   })
   // → RLP 직렬화된 signed tx hex
        ↓
[lib/chainSig.ts:394]
   broadcastTxHash = await publicClient.sendRawTransaction({
     serializedTransaction: signedTx
   })
        ↓
[viem이 https://mainnet.base.org에 eth_sendRawTransaction]
        ↓
[Base 메인넷에 tx 진입]
   tx hash: "0xdef456789abc..."
```

### T0 + 3분 42초 — Base 블록 포함 (블록 시간 ~2초)

```
[Base 메인넷에서 처리]
   USDC contract(0x833589...).transferWithAuthorization(
     from=0xABC1234..., to=0x03fBbA1b..., value=20_000_000,
     v=27, r=..., s=...   ← 사인 #1 검증됨
   )
        ↓
   USDC 잔고 업데이트:
     0xABC1234...  : 20.000000 → 0.000000
     0x03fBbA1b... : 0.000000  → 20.000000
        ↓
   tx 상태: ✅ Confirmed
   Base 익스플로러: https://basescan.org/tx/0xdef456789abc...
```

---

## ⏱️ T0 + 3분 43초 — Supabase 업데이트

```
[cronjob-check-deposits/route.ts:135]
   updateDepositTracking("t1QcXyZ...kLmN8", {
     signedPayload: "0xdef456789abc...",   // ◄── Ethereum tx hash (컬럼명 오해 주의)
     x402Executed:  true,
     confirmed:     true,
     confirmedAt:   "2026-05-13T14:26:43.000Z"
   })
```

**Supabase에서:**
```sql
UPDATE deposit_tracking
SET signed_payload = '0xdef456789abc...',
    x402_executed  = true,
    confirmed      = true,
    confirmed_at   = NOW()
WHERE deposit_address = 't1QcXyZ...kLmN8'
```

---

## ⏱️ T0 + 4분 — 김민준 UI가 알아챔

김민준 브라우저는 그동안 백그라운드에서 1초마다 폴링 중이었음:

```
[브라우저: GET /api/relayer/check-deposit?address=t1QcXyZ...kLmN8]
   ...
   (이전까지는 모두 { confirmed: false, signedPayload: null } 반환)
   ...
[T0 + 4분 — 폴링 결과 바뀜]
   {
     confirmed:     true,
     signedPayload: "0xdef456789abc...",
     x402Executed:  true,
     redirectUrl:   "https://chatgpt.com/premium-link?session=xyz",
     status:        "SUCCESS"
   }
```

---

## ⏱️ T0 + 4분 1초 — content page로 redirect

```
[app/page.tsx가 자동으로 navigate]
   router.push(`/content?address=t1QcXyZ...kLmN8`)
        ↓
[app/content/page.tsx 마운트]
   fetch('/api/content/get-url?address=t1QcXyZ...kLmN8')
        ↓
[get-url/route.ts:69]
   tracking = await getDepositTracking("t1QcXyZ...kLmN8")
   if (!tracking.signedPayload) → return 402
   else {
     swapStatus = await checkSwapStatus(...)
     if (swapStatus !== "SUCCESS") → return 402
     else → return {
       redirectUrl:   "https://chatgpt.com/premium-link?session=xyz",
       signedPayload: "0xdef456789abc...",
       verified:      true
     }
   }
```

---

## ⏱️ T0 + 4분 2초 — 가맹점 콘텐츠 서버 호출

```
[app/content/page.tsx:141]
   fetch("https://chatgpt.com/premium-link?session=xyz", {
     method: "GET",
     headers: {
       "X-PAYMENT":    "0xdef456789abc...",   // ◄── Base tx hash를 bearer로
       "Content-Type": "application/json"
     }
   })
```

### 가맹점 서버 동작 (PAL 코드 밖)

만약 가맹점이 표준 x402 호환이면:
- ❌ X-PAYMENT 형식이 잘못됨 (EIP-712 서명이어야 하는데 tx hash가 옴) → 거절
- PAL이 호환되는 가맹점만 받음 (사실상 PAL 전용 가맹점)

만약 가맹점이 PAL 전용 검증을 하면:
- Base 익스플로러로 `0xdef456...` 조회 → `to == 가맹점 주소`, `value == 20 USDC` 확인
- ✅ 통과 → content 반환

### 가맹점 응답

```
HTTP 200 OK
X-PAYMENT-RESPONSE: {"hash":"0xdef456...","status":"settled"}
Content-Type: application/json

{ "content": "...ChatGPT Pro 권한 활성화...",
  "sessionToken": "..." }
```

---

## ⏱️ T0 + 4분 3초 — 김민준 콘텐츠 수신

```
[app/content/page.tsx:151]
   data = await response.json()
   paymentResponseHeader = response.headers.get("X-PAYMENT-RESPONSE")
   settlementInfo = JSON.parse(paymentResponseHeader)
   setContent({ ...data, settlementHash: settlementInfo.hash })
```

**김민준이 보는 화면:**

```
┌─────────────────────────────────────────┐
│   ✅ 결제 완료!                          │
│                                          │
│   ChatGPT Pro 권한이 활성화되었습니다.    │
│                                          │
│   Settlement: 0xdef456789abc...          │
│   View on BaseScan ↗                     │
└─────────────────────────────────────────┘
```

---

# 📊 전체 흐름 요약 (시간순)

| 시각 | 단계 | 주체 | 비고 |
|---|---|---|---|
| T0 | "Pay ChatGPT Pro" 입력 | 김민준 | |
| T0+1초 | Intent parsing | OpenAI embedding + Supabase RPC | 1.5초 |
| T0+2초 | swapWallet 파생 | chainsig.js SDK + NEAR query | |
| T0+3초 | 1Click `/v0/quote` 호출 | PAL → Gibraltar | privacy 사건 #1 (API 노출) |
| T0+4초 | Supabase deposit_tracking insert | PAL | |
| T0+4.5초 | QR 표시 | 브라우저 | ZIP-321 아님 (footgun) |
| T0+30초 | ZEC 송금 | 김민준 Zcash 지갑 | privacy 사건 #2 (L1 공개) |
| T0+2분 | cron 첫 폴링 | Vercel | PROCESSING |
| T0+3분 | cron 두 번째 폴링 | Vercel | **SUCCESS!** |
| T0+3분 1초 | x402 분기 진입 | cronjob | |
| T0+3분 3초 | **MPC 사인 #1 시작** | PAL → NEAR | EIP-712 authorization |
| T0+3분 18초 | MPC #1 응답 (15초) | NEAR → PAL | |
| T0+3분 19초 | ecrecover 검증 | PAL | silent failure 방지 |
| T0+3분 22초 | **MPC 사인 #2 시작** | PAL → NEAR | EVM tx hash |
| T0+3분 38초 | MPC #2 응답 (16초) | NEAR → PAL | |
| T0+3분 39초 | Base broadcast | viem → Base RPC | |
| T0+3분 42초 | Base 블록 포함 | Base mainnet | tx hash 확정 |
| T0+3분 43초 | Supabase update | PAL | `signed_payload = tx hash` |
| T0+4분 | UI 폴링이 알아챔 | 브라우저 | |
| T0+4분 1초 | content page redirect | 브라우저 | |
| T0+4분 2초 | 가맹점 X-PAYMENT 호출 | 브라우저 → ChatGPT | 비표준 x402 |
| T0+4분 3초 | 콘텐츠 수신 + 표시 | 브라우저 | 완료 |

**총 소요 시간: ~4분.**

---

# 💸 자금 흐름 요약

```
[김민준 ZEC 지갑]
       │ 0.123456 ZEC
       ▼
[1Click solver Zcash 주소 t1QcXyZ...]
       │
       │ (1Click 내부: solver 네트워크 매칭, intents.near atomic settle, token bridge)
       │
       ▼
[swapWallet on Base 0xABC1234...]
       │ 20 USDC 도착
       │
       │ (cron이 MPC로 사인 + broadcast)
       │ 20 USDC
       ▼
[가맹점 0x03fBbA1b...]
       │ 20 USDC 수령 ✅
       ▼
[김민준이 ChatGPT 콘텐츠 받음]
```

> **NEAR 체인은 자산을 한 톨도 거치지 않음.** 사인 서비스 역할만 (15초 × 2 = 30초의 latency 추가).

---

# 🚨 이 거래에서 발생한 함정·위험 (Post-mortem)

| # | 시점 | 함정 | 영향 |
|---|---|---|---|
| 1 | T0+3초 | 1Click이 sender ZEC + recipient EVM을 같은 요청에서 봄 | privacy 0 |
| 2 | T0+3초 | 1Click이 AML 스크리닝 | sanctioned면 거절됨 |
| 3 | T0+4.5초 | QR이 raw 주소 (ZIP-321 아님) | 김민준이 amount 잘못 치면 swap 실패 |
| 4 | T0+30초 | transparent t-address → 익스플로러에서 공개 | L1 privacy 0 |
| 5 | T0+2~3분 | cron 1분 주기 | latency 추가 |
| 6 | T0+3분 3~38초 | MPC 사인 2회 = 약 30초 추가 | latency |
| 7 | T0+3분 1초 | swapWallet에 USDC 20 도착했지만 다른 사용자 USDC와 섞일 수 있음 (shared) | per-user 격리 0 |
| 8 | T0+3분 1초 | nonce = `Date.now().toString(16)` | 같은 ms 충돌 시 revert |
| 9 | T0+4분 2초 | X-PAYMENT가 tx hash → 표준 x402 가맹점 사용 불가 | PAL 전용 |
| (잠재) | 모든 단계 | `/api/relayer/refund` 없음 | 어떤 실패든 자금 회수 수단 없음 |

---

# 🌈 만약 우리 팀이 카테고리 E를 진짜로 구현한다면?

같은 시나리오(*"Pay ChatGPT Pro"*) 우리 버전:

```
[T0] 김민준: "Pay ChatGPT Pro"
       │
[T0+1초] Intent parsing (PAL과 동일)
       │
[T0+2초] HTTP 402 Payment Required 발행
       │ paymentRequirements: {
       │   scheme:   "shielded-zcash",
       │   payTo:    "zs1OURMERCHANTSAPLINGADDR...",  ← shielded
       │   amount:   "20.0 ZEC equivalent",
       │   nonce:    crypto.getRandomValues(32),
       │   deadline: T0 + 5분,
       │   memo:     "x402:<challenge_nonce>"
       │ }
       │
[T0+2.5초] 김민준 Zcash 지갑이 shielded tx 생성
       │   to:     zs1...  (가맹점 shielded)
       │   amount: 20 ZEC equivalent
       │   memo:   "x402:<challenge_nonce>"   ← 512 bytes encrypted memo
       │
[T0+30초] Zcash 메인넷 confirm
       │   L1 observer가 보는 것:
       │     - sender:    🔒 shielded
       │     - recipient: 🔒 shielded
       │     - amount:    🔒 shielded
       │     - memo:      🔒 encrypted
       │
[T0+30초] 가맹점이 viewing key로 memo 복호화
       │ → challenge_nonce 매칭 확인
       │
[T0+31초] HTTP 200 + 콘텐츠
```

**우리 버전의 우위:**
- ❌ 1Click 없음 → privacy 보존, AML linkage 없음
- ❌ swapWallet 없음 → per-user 격리 자동 보장
- ❌ MPC 없음 → latency 30초 절감
- ❌ Supabase polling 없음 → 가맹점이 직접 viewing key 스캔
- ❌ NEAR 의존 없음 → 한 체인만 의존
- ✅ shielded settlement → 진짜 카테고리 E

**총 소요 시간 (추정): ~30~60초** (PAL의 4분 대비 1/4 이하)

---

# 한 줄 정리

> *"PAL 거래 한 건이 끝까지 가는 데 약 4분 걸리고, 그 사이에 김민준의 ZEC 송금 정보는 Gibraltar 회사와 Zcash 익스플로러에 다 노출된다. 우리가 진짜 카테고리 E를 구현하면 같은 거래가 30~60초 안에 끝나고 어떤 외부 관찰자도 보지 못한다."*
