# §1.7 x402 client (x402 클라이언트)

> **Cross-reference:**
> - §1.4(`04-deposit-tracking.md`) — Vercel cron이 1Click `SUCCESS` 시에만 이 서브시스템을 트리거함을 확립.
> - §1.5(`05-one-click-bridge.md`) — two-hop 설계: 1Click → `swapWallet` → x402 → 최종 수신자.
> - §1.6(`06-near-chain-signatures.md`) — EIP-3009 `transferWithAuthorization`이 EIP-712 위에 구현되고, MPC 서명이 두 번 수행되며, `signedPayload`는 실제로 tx hash를 저장함을 확립.

---

## 목적 (Purpose)

PAL의 x402 클라이언트 서브시스템은 **서버 사이드에서 HTTP 402-style 결제를 자동으로 실행하는 역할**을 담당한다. 상류 ZEC→USDC 스왑이 1Click API에서 `SUCCESS`로 전환된 직후 Vercel cron(`cronjob-check-deposits/route.ts`)이 `signX402TransactionWithChainSignature()`를 호출하고, 이 함수가 NEAR MPC를 두 번 사용해 Base mainnet USDC의 EIP-3009 `transferWithAuthorization`을 서명·브로드캐스트한다. PAL은 HTTP 402 challenge/response 사이클을 실제로 수행하지 않으며, content를 보호하는 외부 서버에 결제 proof를 보내는 대신 MPC로 USDC transfer를 직접 on-chain에서 실행하는 **close-loop 방식**을 채택하고 있다.

---

## 파일과 함수 (Files & functions)

| 파일 | 라인 | 함수/심볼 | 역할 |
|------|------|-----------|------|
| `app/api/relayer/cronjob-check-deposits/route.ts` | 15 | `GET(request)` | cron 엔트리포인트 — 모든 pending deposit을 순회하며 x402 트리거 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 34 | `checkSwapStatus(depositAddress)` | 1Click SDK 호출 → swap 상태 폴링 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 47 | `if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed)` | x402 실행 조건 게이트 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 80–88 | `payTo`, `maxAmountRequired`, `deadline`, `nonce` 추출 | x402 파라미터 조립 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 85 | `quote?.payTo \|\| tracking.recipient \|\| quote?.recipient` | `payTo` 3-단계 fallback 체인 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 87 | `Math.floor(Date.now() / 1000) + 3600` | deadline 재계산 (원본 quote deadline 무시) |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 88 | `` `0x${Date.now().toString(16)}` `` | timestamp 기반 nonce 생성 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 125 | `await import('@/lib/chainSig')` | 동적 import (cold start 회피) |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 127–132 | `signX402TransactionWithChainSignature({ payTo, ... })` | x402 서명+브로드캐스트 메인 호출 |
| `app/api/relayer/cronjob-check-deposits/route.ts` | 135–140 | `updateDepositTracking(...)` | tx hash를 `signedPayload` 컬럼에 저장 |
| `lib/chainSig.ts` | 210–401 | `signX402TransactionWithChainSignature(quote)` | **x402 핵심 함수** — EIP-712 도메인 구성, MPC #1(auth), tx 준비, MPC #2(tx), 브로드캐스트 |
| `lib/chainSig.ts` | 226 | `baseChainId = 8453` | Base mainnet chain ID 하드코딩 |
| `lib/chainSig.ts` | 230 | `USDC_CONTRACT = '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913'` | Base USDC 컨트랙트 주소 하드코딩 |
| `lib/chainSig.ts` | 234–239 | `domain = { name: 'USD Coin', version: '2', chainId: 8453, verifyingContract: USDC_CONTRACT }` | EIP-712 domain 구성 |
| `lib/chainSig.ts` | 241–249 | `types = { TransferWithAuthorization: [...] }` | EIP-712 타입 정의 (EIP-3009) |
| `lib/chainSig.ts` | 258–265 | `authorizationValue` | EIP-712 값 — from, to, value, validAfter, validBefore, nonce |
| `lib/chainSig.ts` | 280 | `signTypedDataWithChainSignature(domain, types, authorizationValue)` | MPC 서명 #1 (EIP-712 authorization hash) |
| `lib/chainSig.ts` | 128–201 | `signTypedDataWithChainSignature(domain, types, value)` | EIP-712 hash → `chainSignatureContract.sign()` → (v, r, s) 반환 |
| `lib/chainSig.ts` | 147–152 | `chainSignatureContract.sign({ payloads, path: 'base-1', keyType: 'Ecdsa', ... })` | MPC 서명 호출 #1 |
| `lib/chainSig.ts` | 304–342 | `iface.encodeFunctionData('transferWithAuthorization', [...])` | EIP-3009 calldata ABI 인코딩 |
| `lib/chainSig.ts` | 356–363 | `evmChain.prepareTransactionForSigningLegacy(...)` | legacy EVM tx 준비 + keccak256 해시 생성 |
| `lib/chainSig.ts` | 372–377 | `chainSignatureContract.sign({ payloads: hashesToSign, ... })` | MPC 서명 호출 #2 |
| `lib/chainSig.ts` | 388–391 | `evmChain.finalizeTransactionSigningLegacy(...)` | v,r,s 삽입 + RLP 직렬화 |
| `lib/chainSig.ts` | 394–396 | `publicClient.sendRawTransaction({ serializedTransaction: signedTx })` | Base mainnet 브로드캐스트 |
| `lib/chainSig.ts` | 401 | `return broadcastTxHash` | tx hash 반환 (이 시점에 브로드캐스트 완료) |
| `app/api/content/get-url/route.ts` | 9 | `GET(request)` | content unlock 엔드포인트 — `signedPayload` 존재 여부로 결제 검증 |
| `app/api/content/get-url/route.ts` | 52–63 | `if (!tracking.signedPayload)` | `signedPayload` 없으면 HTTP 402 반환 |
| `app/api/content/get-url/route.ts` | 97–105 | 성공 응답 | `redirectUrl`, `signedPayload` (tx hash), `verified: true` 반환 |
| `app/content/page.tsx` | 141–147 | `fetch(targetApiUrl, { headers: { 'X-PAYMENT': signedPayload } })` | **실제 X-PAYMENT 헤더 전송** — tx hash를 bearer로 사용 |
| `app/content/page.tsx` | 151–152 | `response.headers.get('X-PAYMENT-RESPONSE')` | 콘텐츠 서버의 결제 확인 헤더 수신 |
| `scripts/test-sign-x402-transaction.js` | 64–76 | `exampleQuote` | 테스트용 quote 구조: `payTo`, `maxAmountRequired: '0.1'`, `deadline`, `nonce` |
| `scripts/test-sign-x402-transaction.js` | 103–105 | `signX402TransactionWithChainSignature(exampleQuote)` | 단독 실행 테스트 |
| `contract/src/lib.rs` | 105–142 | `execute_x402_payment(intent_id, amount, recipient)` | NEAR Rust 컨트랙트의 x402 결제 메서드 (런타임 미사용 — 하단 §1.8 판정 참조) |
| `contract/src/lib.rs` | 126–138 | `Promise::new(self.x402_facilitator.clone()).function_call("pay", ...)` | `x402.near`에 `pay()` 호출 (설계된 경로, 실제 구현 미완성) |
| `contract/deploy.sh` | 15 | `X402_FACILITATOR="x402.near"` | 컨트랙트 init 파라미터 — facilitator NEAR account ID |

---

## 연결 (Wiring)

```
┌──────────────────────────────────────────────────────────────────────────┐
│                   x402 클라이언트 서브시스템 데이터 흐름                   │
└──────────────────────────────────────────────────────────────────────────┘

[§1.4 1Click swap]
1Click.getExecutionStatus() → SUCCESS
            │
            ▼
cronjob-check-deposits/route.ts (GET, Vercel cron */1 * * * *)
  ┌─────────────────────────────────────────────────────────────────────┐
  │  quote 필드 추출:                                                    │
  │    payTo        = tracking.recipient  (§1.1 AI 파싱된 x402 수신자)  │
  │    amount       = tracking.amount     (USDC 금액 문자열)             │
  │    deadline     = Date.now()/1000 + 3600  (재계산, 원본 무시)        │
  │    nonce        = 0x${Date.now().toString(16)}                       │
  └─────────────────────────────────────────────────────────────────────┘
            │
            ▼
lib/chainSig.ts — signX402TransactionWithChainSignature(quote)
  ┌─────────────────────────────────────────────────────────────────────┐
  │  [1] evmChain.deriveAddressAndPublicKey(accountId, 'base-1')        │
  │      → swapWallet EVM 주소 (§1.5 에서 1Click recipient로 설정됨)    │
  │  [2] EIP-712 domain + TransferWithAuthorization 타입 + value 구성   │
  │  [3] signTypedDataWithChainSignature() → MPC #1                     │
  │      → chainSignatureContract.sign() → NEAR v1.signer              │
  │      ← { v, r, s } (EIP-3009 authorization 서명)                   │
  │  [4] iface.encodeFunctionData('transferWithAuthorization', [...])   │
  │  [5] evmChain.prepareTransactionForSigningLegacy()                  │
  │      → legacy EVM tx + keccak256 hashesToSign                       │
  │  [6] chainSignatureContract.sign(hashesToSign) → MPC #2             │
  │      → NEAR v1.signer                                               │
  │      ← { v, r, s } (EVM tx 서명)                                   │
  │  [7] evmChain.finalizeTransactionSigningLegacy()                    │
  │  [8] publicClient.sendRawTransaction() → Base mainnet               │
  │      ← broadcastTxHash (0x...)                                      │
  └─────────────────────────────────────────────────────────────────────┘
            │  broadcastTxHash
            ▼
updateDepositTracking(depositAddress, {
  signedPayload: transactionHash,    ← tx hash (명칭 오해 주의)
  x402Executed: true,
  confirmed: true,
})            │
            ▼ (폴링 또는 redirect)
app/content/page.tsx
  GET /api/content/get-url?address={depositAddress}
  → { redirectUrl, signedPayload: txHash, verified: true }
            │
            ▼
fetch(redirectUrl, {
  headers: { 'X-PAYMENT': signedPayload }  ← tx hash를 bearer로 전송
})
  → content JSON + 'X-PAYMENT-RESPONSE' 헤더
```

- **Inputs:**
  - `quote.payTo` — AI(`lib/nearAI.ts:43–44`)가 파싱한 EVM 주소; `tracking.recipient`에 저장됨
  - `quote.maxAmountRequired` — `tracking.amount` (USDC 금액 문자열, 예: `"10"`)
  - `quote.deadline` — 항상 `Date.now()/1000 + 3600`으로 재계산 (`cronjob-check-deposits/route.ts:87`)
  - `quote.nonce` — `` `0x${Date.now().toString(16)}` `` (`route.ts:88`)
  - `tracking.x402Executed`, `tracking.signedPayload` — idempotency 체크 (`route.ts:47`)

- **Outputs:**
  - Base mainnet USDC `transferWithAuthorization` tx (on-chain, 브로드캐스트 완료)
  - `transactionHash` → Supabase `deposit_tracking.signed_payload` 컬럼
  - `/api/content/get-url` 엔드포인트: tx hash를 `X-PAYMENT` 헤더로 content 서버에 전달

- **Dependencies (internal):**
  - `lib/chainSig.ts` — MPC 서명 + Base 브로드캐스트 전담 (§1.6)
  - `lib/depositTracking.ts` — `updateDepositTracking()`, `getDepositTracking()` (§1.4)
  - `lib/oneClick.ts` — `checkSwapStatus()` (§1.5)

- **Dependencies (external):**
  - NEAR MPC `v1.signer` 컨트랙트 — 모든 서명 연산의 실행자
  - Base mainnet RPC `https://mainnet.base.org` — tx 브로드캐스트 대상
  - USDC 컨트랙트 `0x833589fcd6edb6e08f4c7c32d4f71b54bda02913` (Base mainnet)
  - Supabase `deposit_tracking` 테이블 — state 영속화

---

## 라이브러리 (Libraries)

| Package | Version | 사용 위치 | 용도 |
|---------|---------|-----------|------|
| `ethers` | `^5.7.2` | `lib/chainSig.ts:4` | EIP-712 hash (`_TypedDataEncoder.hash`), ABI 인코딩 (`Interface`), BigNumber, 주소 체크섬, `parseUnits` |
| `viem` | `^2.0.0` | `lib/chainSig.ts:5,6` | Base RPC 클라이언트 (`createPublicClient`), `sendRawTransaction` |
| `chainsig.js` | `^1.1.14` | `lib/chainSig.ts:7` | MPC 컨트랙트 호출 (`ChainSignatureContract.sign`), EVM tx 어댑터 (`EVM.prepareTransactionForSigningLegacy`, `finalizeTransactionSigningLegacy`, `deriveAddressAndPublicKey`) |
| `@near-js/accounts` | (chainsig.js 내 전이) | `lib/chainSig.ts:8` | NEAR `Account` 클래스 |
| `@near-js/crypto` | (chainsig.js 내 전이) | `lib/chainSig.ts:9` | `KeyPair`, `KeyPairString` |
| `@near-js/providers` | (chainsig.js 내 전이) | `lib/chainSig.ts:11` | `JsonRpcProvider` (FastNEAR RPC) |
| `@near-js/signers` | (chainsig.js 내 전이) | `lib/chainSig.ts:12` | `KeyPairSigner` |
| `@defuse-protocol/one-click-sdk-typescript` | `^0.1.14` | `lib/oneClick.ts:4` | 1Click `getExecutionStatus()` 폴링 (x402 트리거 upstream) |

---

## 워크스루 — happy path (상세 단계)

**사전 조건:** 사용자가 ZEC를 1Click deposit address로 전송했고, 1Click swap이 `SUCCESS` 상태로 전환됨.

---

**Step 1. Vercel cron이 GET /api/relayer/cronjob-check-deposits 호출**
```
vercel.json:9 — "schedule": "*/1 * * * *"
  → cronjob-check-deposits/route.ts:15 — export async function GET(request)
```
Vercel이 1분마다 이 엔드포인트를 호출한다. 인증은 코드에서 주석 처리되어 있어 (`route.ts:17–21`) 누구나 호출 가능하다.

---

**Step 2. deadline 미도래 deposit 전체 조회**
```
cronjob-check-deposits/route.ts:26
  const deposits = await getDepositsWithDeadlineRemaining()
  // lib/depositTracking.ts:365 — deadline > now 필터
```
Supabase `deposit_tracking` 테이블에서 `deadline > now()`인 row를 모두 가져온다.

---

**Step 3. 각 deposit에 대해 1Click swap 상태 폴링**
```
cronjob-check-deposits/route.ts:34
  const statusResponse = await checkSwapStatus(depositAddress)
  // lib/oneClick.ts:138 → OneClickService.getExecutionStatus(depositAddress)
  
const status = (statusResponse as any).status ||
               (statusResponse as any).executionStatus ||
               (statusResponse as any).state ||
               'PENDING_DEPOSIT'
```
`depositAddress`(= 1Click swap order ID)로 상태를 조회한다. SDK 응답 필드명이 불확실해 3중 fallback 패턴을 사용한다.

---

**Step 4. SUCCESS 조건 게이트 — x402 실행 결정**
```
cronjob-check-deposits/route.ts:47
  if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed)
```
1Click swap이 `SUCCESS`이고 아직 서명·실행되지 않은 경우에만 x402 실행으로 진입한다. `!tracking.signedPayload`로 idempotency를 보장한다.

---

**Step 5. x402 파라미터 추출**
```
cronjob-check-deposits/route.ts:80–88

const quote = quoteData?.quote || quoteData?.quoteResponse || quoteData
const payTo = quote?.payTo || tracking.recipient || quote?.recipient
// → 실질적으로 항상 tracking.recipient (AI 파싱 EVM 주소)
const maxAmountRequired = quote?.maxAmountRequired || quote?.amount || tracking.amount
const deadline = Math.floor(Date.now() / 1000) + 3600   // 원본 quote deadline 무시
const nonce = `0x${Date.now().toString(16)}`             // timestamp 기반
```
`payTo`는 3-단계 fallback이지만 1Click `/v0/quote` 응답에 `payTo` 필드가 없어 실제로는 항상 `tracking.recipient`가 사용된다. `tracking.recipient`는 `register-deposit/route.ts:114`에서 AI 파싱된 service의 `receivingAddress`가 저장된 값이다.

---

**Step 6. signX402TransactionWithChainSignature 호출**
```
cronjob-check-deposits/route.ts:125–132

const { signX402TransactionWithChainSignature } = await import('@/lib/chainSig')
const transactionHash = await signX402TransactionWithChainSignature({
  payTo,
  maxAmountRequired: String(maxAmountRequired),
  deadline: Math.floor(Date.now() / 1000) + 3600,
  nonce: String(nonce),
})
```
동적 import로 chainsig 모듈을 로드한다 (서버리스 cold start 시 모듈 초기화 문제 회피).

---

**Step 7. swapWallet EVM 주소 파생 (chainsig.js)**
```
lib/chainSig.ts:223
  const { address } = await evmChain.deriveAddressAndPublicKey(accountId, MPC_PATH)
  // MPC_PATH = 'base-1' (하드코딩, lib/chainSig.ts:18)
```
NEAR MPC `v1.signer`의 공개키로부터 secp256k1 child key를 파생해 EVM 주소(`swapWallet`)를 도출한다. 이 주소는 §1.5에서 1Click `recipientAddress`로 등록된 것이므로 USDC가 여기 도착해 있다.

---

**Step 8. EIP-712 domain + TransferWithAuthorization 타입 + value 구성**
```
lib/chainSig.ts:234–265

const domain = {
  name: 'USD Coin',
  version: '2',
  chainId: 8453,   // Base mainnet
  verifyingContract: '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913',
}

const types = {
  TransferWithAuthorization: [
    { name: 'from',        type: 'address' },
    { name: 'to',          type: 'address' },
    { name: 'value',       type: 'uint256' },
    { name: 'validAfter',  type: 'uint256' },
    { name: 'validBefore', type: 'uint256' },
    { name: 'nonce',       type: 'bytes32' },
  ],
}

const authorizationValue = {
  from:        ethers.utils.getAddress(address),     // swapWallet
  to:          ethers.utils.getAddress(quote.payTo), // 최종 수신자
  value:       amountInWei,                          // USDC 6 decimals
  validAfter:  BigNumber.from(0),
  validBefore: BigNumber.from(quote.deadline),       // Unix sec
  nonce:       ethers.utils.hexZeroPad(nonceBytes, 32),
}
```
이것이 **EIP-3009 `TransferWithAuthorization`**의 EIP-712 typed data 전체 구조다. USDC 컨트랙트가 이 서명을 검증해 `transferWithAuthorization`을 실행한다.

---

**Step 9. EIP-712 hash 생성 + MPC 서명 #1 (authorization)**
```
lib/chainSig.ts:138–152

const hash = ethers.utils._TypedDataEncoder.hash(domain, types, authorizationValue)
const hashBytes = ethers.utils.arrayify(hash)
const hashToSign = Array.from(hashBytes)

const signature = await chainSignatureContract.sign({
  payloads: [hashToSign],
  path: 'base-1',
  keyType: 'Ecdsa',
  signerAccount: account,   // NEAR proxy account
})
// ← { big_r: { affine_point }, s: { scalar }, recovery_id }
```
NEAR MPC 네트워크(`v1.signer`)에 cross-contract call이 발생한다. 응답에서 `recovery_id`를 추출해 `v = recoveryId + 27`로 변환한다 (`lib/chainSig.ts:167–186`).

---

**Step 10. ecrecover 검증**
```
lib/chainSig.ts:283–300

const recoveredAddress = ethers.utils.recoverAddress(hash, { r, s, v })
if (recoveredAddress.toLowerCase() !== address.toLowerCase()) {
  throw new Error(`Signature verification failed: ...`)
}
```
서명이 `swapWallet` 주소로 복원되지 않으면 즉시 abort한다.

---

**Step 11. transferWithAuthorization calldata ABI 인코딩**
```
lib/chainSig.ts:304–342

const iface = new ethers.utils.Interface([
  'function transferWithAuthorization(address from, address to, uint256 value, uint256 validAfter, uint256 validBefore, bytes32 nonce, uint8 v, bytes32 r, bytes32 s)'
])
const data = iface.encodeFunctionData('transferWithAuthorization', [
  from, to, value, validAfter, validBefore, nonce, v, rBytes32, sBytes32
])
```
EIP-3009 표준 함수 시그니처로 calldata를 인코딩한다.

---

**Step 12. Legacy EVM tx 준비 + keccak256 해시 생성**
```
lib/chainSig.ts:356–363

const { transaction: preparedTx, hashesToSign } =
  await evmChain.prepareTransactionForSigningLegacy({
    from:     address as `0x${string}`,
    to:       '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913',
    value:    BigInt(0),
    data:     data as `0x${string}`,
    gasPrice: gasPrice.toBigInt(),   // 0.1 gwei (하드코딩)
    gas:      BigInt(150000),         // 하드코딩
  })
```
chainsig.js가 RLP 인코딩된 legacy tx의 keccak256 해시(`hashesToSign`)를 반환한다.

---

**Step 13. MPC 서명 #2 (EVM tx hash)**
```
lib/chainSig.ts:372–377

const signature = await chainSignatureContract.sign({
  payloads: hashesToSign,
  path: 'base-1',
  keyType: 'Ecdsa',
  signerAccount: account,
})
```
두 번째 NEAR cross-contract call. 이 서명이 실제 on-chain tx에 대한 서명이다.

---

**Step 14. 서명 삽입 + RLP 직렬화**
```
lib/chainSig.ts:388–391

const signedTx = evmChain.finalizeTransactionSigningLegacy({
  transaction: preparedTx as any,
  rsvSignatures: signature,
})
```

---

**Step 15. Base mainnet 브로드캐스트**
```
lib/chainSig.ts:394–401

const broadcastTxHash = await publicClient.sendRawTransaction({
  serializedTransaction: signedTx as `0x${string}`,
})
console.log(`View on Base Explorer: https://basescan.org/tx/${broadcastTxHash}`)
return broadcastTxHash
```
`viem`이 `https://mainnet.base.org`에 `eth_sendRawTransaction`을 호출한다. **브로드캐스트는 이 함수 내부에서 완전히 완료**된다. 반환값은 Base의 Ethereum tx hash다.

---

**Step 16. tx hash를 Supabase에 저장**
```
cronjob-check-deposits/route.ts:135–140

await updateDepositTracking(depositAddress, {
  signedPayload: transactionHash,   // 명칭과 달리 실제 값은 tx hash
  x402Executed: true,
  confirmed: true,
  confirmedAt: Date.now()
})
```

---

**Step 17. UI가 content 페이지로 redirect — X-PAYMENT 헤더 전송**
```
app/content/page.tsx:27
  fetch(`/api/content/get-url?address=${depositAddress}`)
  → app/api/content/get-url/route.ts:52–63
    if (!tracking.signedPayload) → HTTP 402  (아직 처리 중)
    if (swapStatus !== 'SUCCESS') → HTTP 402
    else → { redirectUrl, signedPayload: txHash, verified: true }

app/content/page.tsx:141–147
  fetch(targetApiUrl, {
    method: 'GET',
    headers: {
      'X-PAYMENT': signedPayload,    // Base mainnet tx hash를 bearer로 전달
      'Content-Type': 'application/json',
    },
  })
```
이 `X-PAYMENT` 헤더를 수신하는 외부 content 서버(`redirectUrl`)가 tx hash를 검증하고 `X-PAYMENT-RESPONSE` 헤더와 함께 content를 반환한다.

---

**Step 18. content 수신 및 렌더링**
```
app/content/page.tsx:149–158

if (response.ok) {
  const data = await response.json()
  const paymentResponseHeader = response.headers.get('X-PAYMENT-RESPONSE')
  const settlementInfo = paymentResponseHeader ? JSON.parse(paymentResponseHeader) : {}
  setContent({ ...data, settlementHash: settlementInfo.hash, paidBy: address })
}
```

---

**두 번의 MPC 서명 호출 요약:**

| 단계 | 입력 payload | 목적 | 결과 |
|------|-------------|------|------|
| MPC #1 (`lib/chainSig.ts:147`) | EIP-712 `TransferWithAuthorization` 해시 | USDC 컨트랙트가 검증할 EIP-3009 authorization 서명 | `(v, r, s)` — calldata에 삽입 |
| MPC #2 (`lib/chainSig.ts:372`) | legacy EVM tx의 RLP keccak256 해시 | Base mainnet 전송을 위한 tx 서명 | 서명된 tx → `sendRawTransaction` |

---

## 노트 / 특이사항 / 주의점 (Notes / quirks / footguns)

---

### Facilitator 판정 (가장 중요)

**판정: PAL은 외부 x402 facilitator를 전혀 사용하지 않는다. 결제는 on-chain 직접 실행이다.**

근거:

1. **Coinbase Base x402 facilitator 미사용:** `lib/chainSig.ts` 전체 어디에도 `https://x402.org/facilitator`, `https://api.cdp.coinbase.com`, 또는 그 외 외부 facilitator URL에 대한 HTTP 요청이 없다. PAL은 USDC `transferWithAuthorization`을 직접 Base mainnet에 broadcast한다 (`lib/chainSig.ts:394`).

2. **NLx402 (PCEF/Solana) 미사용:** NLx402, PCEF, Solana 관련 URL 또는 라이브러리가 코드 어디에도 없다. 1Click SDK(`@defuse-protocol/one-click-sdk-typescript`)는 Zcash→USDC swap용이며, x402 facilitator 역할을 하지 않는다.

3. **Self-hosted x402 server 미존재:** `app/api/` 하위에 `X-PAYMENT` 헤더를 accept하는 내부 엔드포인트가 없다. `/api/content/get-url/route.ts`는 `signedPayload`(tx hash)의 존재 여부만 DB에서 조회할 뿐, HTTP 402 challenge를 발행하는 서버가 아니다.

4. **NEAR Rust 컨트랙트 (`x402.near` 호출) — 설계 경로이지만 실제 TS 코드에서 호출 없음:**
   - `contract/src/lib.rs:105–142`의 `execute_x402_payment()`은 `Promise::new(self.x402_facilitator.clone()).function_call("pay", ...)` 로 `x402.near`에 `pay()`를 호출하는 NEAR 컨트랙트 메서드다 (`deploy.sh:15` — `X402_FACILITATOR="x402.near"`).
   - 그러나 **어떤 TypeScript API route도 이 컨트랙트 메서드를 호출하지 않는다.** `rg -n "execute_x402_payment\|anyone-pay.near" --type ts`를 실행하면 결과가 없다.
   - `x402.near`가 실제로 on-chain에 배포된 facilitator 컨트랙트인지도 확인되지 않는다.
   - **결론: 이 Rust 컨트랙트 경로는 설계 단계의 placeholder다. 실제 런타임 x402 실행 경로는 TS `lib/chainSig.ts`의 직접 broadcast다.**

5. **HTTP 402 challenge/response 사이클 없음:** PAL은 표준 x402 flow(서버가 `402 Payment Required` → 클라이언트가 `X-PAYMENT` 헤더로 재요청)를 실행하지 않는다. cron이 MPC로 USDC transfer를 먼저 on-chain에서 완료한 뒤, UI가 Base tx hash를 `X-PAYMENT` 헤더로 content 서버에 보내는 **post-hoc proof** 방식이다. Content 서버가 이 tx hash를 실제로 Base blockchain에서 검증하는지는 content 서버 코드에 달려 있으며, PAL 코드 범위 밖이다.

---

### Settlement asset / chain (명시)

- **Settlement chain:** Base mainnet (chain ID `8453`, `lib/chainSig.ts:226`)
- **Settlement asset:** USDC on Base — 컨트랙트 주소 `0x833589fcd6edb6e08f4c7c32d4f71b54bda02913` (`lib/chainSig.ts:230`)
- **Standard:** EIP-3009 `transferWithAuthorization`, EIP-712 typed data
- **NEAR context:** MPC signing은 NEAR mainnet(`v1.signer`)에서 수행되지만, 결제 자체는 Base에서 발생한다.

---

### Secure Legion NLx402 패턴과의 비교

Secure Legion(Week2 §#26)은 **NLx402 quote hash를 Zcash shielded tx의 memo 필드에 직접 삽입**하는 방식이다. 즉 `NLx402:<quote_hash>`가 Zcash L1의 encrypted memo에 담겨 전달되며, Zcash 자체가 x402 영수증의 carrier가 된다 (`/week2/project-references-by-idea.md:275`). 이 설계에서 Zcash shielded tx = 결제 + 영수증 두 역할을 동시에 수행하고, NLx402 facilitator(PCEF, Solana)가 `quote_hash` 기반으로 replay를 방어한다.

PAL은 정반대의 구조다. PAL에서 Zcash는 단지 **upstream funding asset**에 불과하다 — ZEC를 1Click(Defuse Protocol)에 보내 USDC로 swap한 뒤, Base USDC를 직접 MPC로 transfer한다. Zcash memo를 전혀 사용하지 않으며, Zcash blockchain을 직접 쿼리하지도 않는다. x402 settlement는 Zcash와 완전히 분리된 Base EVM 레이어에서 발생하고, 1Click API가 Zcash→USDC 연결의 블랙박스 역할을 한다. **Week2의 핵심 요약이 그대로 적용된다**: Secure Legion = Zcash가 x402 carrier, PAL = x402 이전에 ZEC가 USDC로 변환되는 funding 단계.

우리 팀의 차별화 narrative가 여기서 명확해진다: PAL이 남긴 공백 — "Zcash 자체를 x402 settlement asset으로 직접 만들기" — 는 Secure Legion이 memo carrier로 채웠지만, Solana NLx402 facilitator 의존성이 있었다. 우리가 Zcash를 settlement rail로 직접 쓰는 in-protocol facilitator를 설계하면 두 프로젝트 모두를 넘어서는 포지션이 된다.

---

### Replay / nonce / deadline 동작

`nonce`는 `0x${Date.now().toString(16)}` — Unix timestamp를 hex로 변환한 값이다 (`cronjob-check-deposits/route.ts:88`). 이는 **암호학적으로 무작위(random)가 아니다**. EIP-3009 spec은 `bytes32 nonce`가 "unique per authorization"이어야 한다고 요구하며, `transferWithAuthorization`은 nonce를 사용되면 소각(spend)한다. 따라서 동일 millisecond 내 두 번 실행이 발생하면 두 번째 tx는 "authorization already used" 오류로 reverts된다. 실제 충돌 확률은 매우 낮으나 이론적으로 존재한다.

`deadline`은 원본 1Click quote에 저장된 deadline을 무시하고 항상 `Date.now()/1000 + 3600`으로 재계산한다 (`route.ts:87`). 이는 1Click quote의 만료 시간과 x402 authorization의 유효 시간이 별개로 관리됨을 의미한다. cron이 지연되더라도 1시간 이내라면 authorization이 유효하다는 점에서 관대한 설계지만, 1Click 측 quote가 만료된 후에도 x402 실행을 시도할 수 있어 불일치가 발생할 수 있다.

---

### Merchant가 PAL을 신뢰하는가 — closed-loop 여부

PAL에서 x402 "서버"와 "클라이언트"가 같은 도메인이 아니다. Content 서버(`redirectUrl`)는 PAL이 소유하지 않은 외부 서비스다. PAL이 `X-PAYMENT: <txHash>` 헤더를 보내면, content 서버가 Base blockchain에서 해당 tx를 독립적으로 검증하는 것이 올바른 설계다. 그러나 **PAL 코드 자체는 이 검증을 강제하지 않는다** — `get-url/route.ts`는 Supabase DB의 `signedPayload` 존재 여부만 확인한다 (`route.ts:52`). Content 서버가 tx hash를 검증하지 않고 헤더 존재만으로 access를 허용한다면 payment verification은 사실상 PAL 서버에 대한 신뢰로 귀결된다.

---

### x402 실패 시 동작

x402 실행(`signX402TransactionWithChainSignature()`)이 실패하면:
- `cronjob-check-deposits/route.ts:149–157`에서 `action: 'x402_error'`로 results에 기록되고 개별 deposit 처리를 중단하지 않는다.
- `x402Executed`, `signedPayload`는 업데이트되지 않으므로 다음 cron invocation(1분 후)에서 재시도가 자동으로 이루어진다 (`route.ts:47` — 조건 `!tracking.signedPayload && !tracking.x402Executed`).
- **그러나 ZEC는 이미 1Click에 전달되어 USDC로 스왑이 완료된 상태다.** USDC가 `swapWallet`에 도착했으나 x402 전송이 실패한 경우 환불 메커니즘이 없다 — `/api/relayer/refund` 엔드포인트는 존재하지 않는다 (`_claims-to-verify.md:203`). 사용자는 자금을 잃는다.
- MPC 서명 자체 실패(`Failed to get signature from MPC contract`, `lib/chainSig.ts:155`, `lib/chainSig.ts:382`)도 retry 없이 예외를 전파한다. cron 레벨 retry만 존재한다.

---

### `signedPayload` 컬럼 명칭 혼동

Supabase `deposit_tracking.signed_payload` 컬럼에 저장되는 값은 "서명된 payload bytes"가 아니라 **Base mainnet Ethereum tx hash**다. UI(`app/content/page.tsx:144`)도 이를 `X-PAYMENT` 헤더의 값으로 사용하며, content 서버는 이 값을 기반으로 tx를 on-chain에서 조회해야 한다. 컬럼 명칭이 암호화 payload와 tx hash를 혼동하게 만드는 구조적 오해 소지가 있다.

---

### Rust 컨트랙트의 x402 흐름 내 역할 판정 (§1.8 preview)

`contract/src/lib.rs`의 `AnyonePay` 컨트랙트는 x402 설계의 **비활성 placeholder**다:
- `execute_x402_payment()` (`lib.rs:105`)는 `x402.near`에 `pay()`를 호출하는 Promise를 구성하지만, 어떤 TS 코드도 이 메서드를 호출하지 않는다.
- `verify_deposit()` (`lib.rs:84`)는 `intents.near.mt_batch_balance_of()`를 호출하는 구성을 가지지만 항상 `true`를 반환한다 (`lib.rs:100`).
- `create_intent()` (`lib.rs:60`)와 `get_intent()` (`lib.rs:153`)는 `deploy.sh:54`에서 테스트용으로 호출되지만, 프로덕션 TS flow에서는 사용되지 않는다.
- **결론: NEAR Rust 컨트랙트는 x402 클라이언트 런타임에 아무런 역할을 하지 않는다.** 등록/메타데이터 시간에도 마찬가지다 — PAL의 모든 상태는 Supabase에 저장되며 NEAR 컨트랙트는 실제로 상태를 보유하지 않는다 (프로덕션 기준).

---

## 답한 open questions (from the spec §7)

**Q1: 어떤 x402 facilitator가 사용되는가?**

**A: 없음 (None).** PAL은 Coinbase, NLx402(PCEF), 또는 자체 hosted facilitator를 사용하지 않는다. USDC `transferWithAuthorization`을 NEAR MPC로 서명하고 viem으로 Base mainnet에 직접 broadcast한다 (`lib/chainSig.ts:394`). NEAR Rust 컨트랙트(`x402.near` 호출 포함)는 설계 placeholder로, 현재 TS 코드 어디서도 호출되지 않는다.

**Q2: 402 challenge/response dance가 어디서 일어나는가?**

**A: 표준 402 dance는 존재하지 않는다.** 표준 flow(서버가 `402 + paymentRequirements` → 클라이언트가 `X-PAYMENT`로 재요청)를 수행하는 코드가 없다. 대신 cron이 미리 on-chain USDC transfer를 실행하고, UI가 나중에 tx hash를 `X-PAYMENT` 헤더로 content 서버에 제출하는 **post-hoc 방식**이다.

**Q3: x402 실패 시 환불/재시도는?**

**A:** 재시도는 cron 레벨(1분 주기)에서만 존재한다. 환불 메커니즘은 없다. ZEC→USDC swap이 완료된 후 x402 전송이 최종 실패하면 USDC가 `swapWallet`에 묶인다 (PAL 코드 기준 복구 불가).
