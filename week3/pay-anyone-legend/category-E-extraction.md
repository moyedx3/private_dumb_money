# §2 Category-E (x402 + Zcash) reference extraction

## 2.1 PAL의 "x402 + Zcash"는 무엇이고, Secure Legion / NLx402와 어떻게 다른가

### 비교 표

| 차원 | PAL (Pay Anyone Legend) | Secure Legion (NLx402) |
|------|-------------------------|------------------------|
| **Zcash의 역할** | upstream funding asset — transparent t-addr에 ZEC를 보내면 1Click이 USDC로 swap. Shielding 없음 ([§1.3](./subsystems/03-z-address-generation.md) `lib/oneClick.ts:126`, [§3.1](./zcash-tool-inventory.md)) | x402 quote_hash의 carrier — NLx402 `quote_hash`가 Zcash shielded tx의 encrypted memo 필드에 박힘 (`NLx402:<quote_hash>`). Zcash L1이 영수증 transport [week2 §#26] |
| **x402 facilitator** | 없음 — in-process EVM broadcast. `lib/chainSig.ts:394` `publicClient.sendRawTransaction()` 로 Base mainnet에 USDC `transferWithAuthorization`을 직접 브로드캐스트 ([§1.7](./subsystems/07-x402-client.md)) | NLx402 — PCEF (Perkins Coie Entrepreneur Fund, 501(c)(3) nonprofit)가 운영하는 Solana 기반 x402 facilitator. quote_hash로 replay 방어 [week2 §#26] |
| **Settlement chain/asset** | Base mainnet USDC — chain ID `8453`, USDC contract `0x833589fcd6edb6e08f4c7c32d4f71b54bda02913` (`lib/chainSig.ts:226,230`, [§1.7](./subsystems/07-x402-client.md)) | Solana / USDC or SOL + Zcash memo로 영수증 [week2 §#26] |
| **Privacy guarantee** | 무효 — 1Click이 sender ZEC addr + recipient EVM addr를 동일 `/v0/quote` 요청에서 봄. AML 스크리닝 자동 적용. t-addr deposit이므로 L1에서도 공개 ([§1.5](./subsystems/05-one-click-bridge.md), [§3.1](./zcash-tool-inventory.md)) | Zcash shielded tx의 unlinkability 그대로 유지. memo는 incoming viewing key로만 복호화 가능. L1 observer에게는 amount + memo 불투명 [week2 §#26] |
| **우리 팀이 차용 가능한 부분** | NEAR Chain Signatures의 EIP-3009 서명 패턴 (`lib/chainSig.ts:147,372`, [§1.6](./subsystems/06-near-chain-signatures.md)), Vercel cron deposit polling state machine ([§1.4](./subsystems/04-deposit-tracking.md)), intent 파싱 + pgvector 서비스 레지스트리 ([§1.1](./subsystems/01-intent-parser.md), [§1.2](./subsystems/02-service-registry.md)) | memo carry pattern — `NLx402:<quote_hash>`를 memo에 넣는 구조, 영수증 검증 워크플로우 (incoming viewing key로 memo 스캔 + hash 매칭) [week2 §#26] |

---

### 아키텍처적 차이와 우리 팀 설계에 대한 함의

Pay Anyone Legend의 "x402 + Zcash" 결합은 실제로는 완전히 분리된 두 레이어다. Zcash는 funding 단계에서만 등장하고, x402 settlement는 Base USDC 위에서 독립적으로 이루어진다. 사용자가 QR로 ZEC를 1Click의 transparent deposit address(`t1...` 또는 `t3...`)로 전송하면, 1Click(Defuse Labs Limited, Gibraltar 법인)이 USDC로 swap한 뒤 PAL의 `swapWallet` EVM 주소로 전달하고, 그 시점에 Vercel cron이 NEAR Chain Signatures로 USDC `transferWithAuthorization`을 서명·브로드캐스트한다 (`lib/chainSig.ts:210–401`, `app/api/relayer/cronjob-check-deposits/route.ts:127`). x402 헤더(`X-PAYMENT`)에는 표준 EIP-3009 authorization payload가 아닌 **이미 브로드캐스트된 Ethereum tx hash**가 bearer로 들어간다 (`app/content/page.tsx:144`). 즉 PAL에서 "x402"는 선불 온체인 USDC transfer + post-hoc proof 제출이며, Zcash는 이 흐름에서 USDC를 공급하는 전처리 자산일 뿐이다.

Secure Legion / NLx402 패턴은 반대 방향의 설계다. 여기서 Zcash tx 자체가 x402 결제 proof의 carrier가 된다. Merchant가 `(quote_id, quote_hash, amount, expiry)`를 생성해 사용자에게 전달하면, 사용자가 ZEC shielded tx를 보낼 때 encrypted memo 필드에 `NLx402:<quote_hash>`를 삽입한다. Merchant(또는 facilitator)는 incoming viewing key로 memo를 복호화하고 `quote_hash`가 local SQLCipher DB의 미결제 quote와 매칭되는지 확인한다 [week2 §#26]. 이 구조에서 Zcash shielded tx = 결제 + 영수증의 두 역할을 동시에 수행하며, replay 방어는 `quote_hash`의 유일성으로 보장된다. L1 observer에게는 amount와 memo 내용이 모두 불투명하므로 Zcash의 privacy 보장이 end-to-end로 유지된다.

이 두 패턴은 카테고리 E 프로젝트를 설계할 때 서로 다른 trade-off를 제시한다. PAL 패턴(ZEC → USDC → x402)은 기존 x402 EVM 인프라를 그대로 재사용할 수 있고 NEAR Chain Signatures 서명 코드가 구체적으로 lift 가능하지만, Zcash의 privacy 보장이 완전히 소실되고 Defuse Labs Limited에 대한 영구적 신뢰 의존이 생긴다. Secure Legion 패턴(Zcash memo carry)은 Zcash privacy를 settlement 계층까지 유지하지만 NLx402(Solana, PCEF)에 facilitator 의존성이 있고, 현재 공개된 NLx402 spec 문서가 없어 독립 구현 비용이 높다. **우리 팀이 category E를 선택할 경우, 두 패턴 모두에서 배울 점이 있으나 어느 쪽도 그대로 복사하면 핵심 문제(PAL: privacy 소실, Secure Legion: 외부 Solana facilitator 의존)가 남는다. 진정한 차별화는 Zcash 자체를 x402 settlement rail로 직접 만드는 방향에 있다** — 이것이 §2.4에서 구체화한다.

---

## 2.2 PAL의 정확한 결제 시퀀스 (lift-and-use를 위한 reference)

아래는 PAL의 전체 payment flow를 "내일 이 흐름을 복제하려면" 기준으로 재구성한 참조 시퀀스다. 각 스텝에 `<file:line>` 인용과 담당 서브시스템 링크를 명시했다.

---

**[사용자 → Intent 입력]**

**Step 1.** 사용자가 `FloatingInput.tsx`에 자연어를 입력하고 제출한다 (`components/FloatingInput.tsx:36`). `app/page.tsx:346`의 `handleSubmit`이 수신하여 URL에 `?prompt=` 파라미터를 기록하고 `parseIntent(text)`를 호출한다. URL persistence로 QR 복구가 가능하다. [§1.1](./subsystems/01-intent-parser.md)

**Step 2.** 브라우저 측 `parseIntent()` (`lib/intentParser.ts:16`)가 `POST /api/parse-intent`에 `{ query }` JSON body를 전송한다.

**Step 3.** Next.js API route (`app/api/parse-intent/route.ts:16`)가 수신하여 `analyzePromptWithNearAI(query)`를 서버 사이드에서 호출한다. 먼저 pgvector 시맨틱 검색(`findBestService(prompt, 0.6)`, `lib/nearAI.ts:32`)으로 등록된 서비스와 매칭을 시도하고, 임계값 0.6 미달 시 LLM chat completion(OpenAI `gpt-4o-mini` 또는 NEAR AI Cloud `deepseek-chat-v3-0324`, `lib/nearAI.ts:112–121`)을 호출해 `{ amount, currency, chain, receivingAddress, bridgeFrom: 'zcash' }`를 추출한다. [§1.1](./subsystems/01-intent-parser.md), [§1.2](./subsystems/02-service-registry.md)

**Step 4.** 서비스가 시맨틱 검색으로 매칭되면 LLM 호출 없이 `matchedService.amount`, `matchedService.url`, `matchedService.receivingAddress`를 즉시 사용한다 (`lib/nearAI.ts:36–50`). LLM 경로에서는 `response_format: { type: 'json_object' }`로 구조화 JSON을 강제하며, USDT는 자동으로 USDC로 정규화된다 (`lib/nearAI.ts:173`). [§1.1](./subsystems/01-intent-parser.md)

**Step 5.** `ParsedIntent`가 클라이언트로 반환되고 (`lib/intentParser.ts:31`), `app/page.tsx:408`에서 `isComplete` 여부를 판단해 완전한 intent면 `generateDepositAddress()`를 호출한다. [§1.1](./subsystems/01-intent-parser.md)

---

**[Deposit Address 발급 + QR 코드 표시]**

**Step 6.** `app/page.tsx:499`의 `generateDepositAddress()`가 `POST /api/relayer/register-deposit`에 `{ intentId, amount, recipient, chain, redirectUrl, senderAddress }`를 전송한다.

**Step 7.** 서버가 먼저 NEAR Chain Signatures로 `swapWallet` EVM 주소를 결정론적으로 파생한다 (`getEthereumAddressFromProxyAccount()`, `lib/chainSig.ts:112`). 이 주소는 `(NEAR_PROXY_ACCOUNT_ID, path='base-1')`로 고정 파생되므로 모든 사용자/모든 주문이 동일한 EVM 주소를 사용한다 (`lib/chainSig.ts:18`, `lib/chainSig.ts:94`). **보안 취약점: `MPC_PATH = 'base-1'` 하드코딩으로 사용자 격리 없음.** [§1.6](./subsystems/06-near-chain-signatures.md)

**Step 8.** 서버가 1Click API에 `POST /v0/quote`를 호출한다 (`lib/oneClick.ts:102`). Request body:
```
originAsset: 'nep141:zec.omft.near'          # ZEC
destinationAsset: 'nep141:base-0x833589...omft.near' # USDC on Base
swapType: 'EXACT_OUTPUT'                      # 출력 USDC 고정, 입력 ZEC 계산
amount: usdcToSmallestUnit(intent.amount)
recipient: swapWallet                          # 중간 EVM 주소 (최종 수신자 아님)
refundTo: senderAddress                        # ZEC 환불 주소 → 1Click에 sender 노출
```
1Click(Defuse Labs Limited, Gibraltar)이 `depositAddress`(transparent t-addr)를 반환한다. 이 시점에 1Click은 sender ZEC addr + recipient EVM addr + amount를 동일 요청에서 취득한다. AML 스크리닝 자동 적용. [§1.5](./subsystems/05-one-click-bridge.md), [§3.1](./zcash-tool-inventory.md)

**Step 9.** `depositAddress = quote.depositAddress || quote.quote?.depositAddress || quote.address` 로 추출하고 (`lib/oneClick.ts:126`, `app/api/relayer/register-deposit/route.ts:66`), `registerDeposit(depositAddress, ...)` 로 Supabase `deposit_tracking` 테이블에 upsert한다 (`lib/depositTracking.ts:104`). Primary key = `deposit_address`, 1Click swap order ID로도 사용된다. [§1.3](./subsystems/03-z-address-generation.md), [§1.4](./subsystems/04-deposit-tracking.md)

**Step 10.** 서버가 `{ depositAddress, zcashAmount, deadline, ... }`를 응답하고, 클라이언트가 `<QRCodeSVG value={depositAddress} size={220} level="H">` (`components/IntentsQR.tsx:186`)로 QR 코드를 렌더링한다. QR에는 raw 주소 문자열만 인코딩된다 — ZIP-321 `zcash:?amount=` URI 형식은 없다 (footgun). [§1.3](./subsystems/03-z-address-generation.md)

---

**[ZEC 입금 → 1Click Swap 폴링]**

**Step 11.** 사용자가 Zcash 지갑으로 `depositAddress`(t-addr)에 ZEC를 전송한다. 선택적으로 `POST /api/relayer/submit-tx-hash`에 ZEC tx hash를 제출해 swap 처리를 가속할 수 있다 (`app/api/relayer/submit-tx-hash/route.ts:33`). PAL은 tx hash 형식을 `txHash.length < 10` 외에 검증하지 않고 1Click에 그대로 전달한다. [§1.5](./subsystems/05-one-click-bridge.md)

**Step 12.** Vercel cron(`vercel.json:9`, schedule `*/1 * * * *`)이 매 1분마다 `GET /api/relayer/cronjob-check-deposits`를 호출한다 (`app/api/relayer/cronjob-check-deposits/route.ts:15`). Supabase에서 `deadline > now()`인 deposit 전체를 가져와 각각 `checkSwapStatus(depositAddress)` → `OneClickService.getExecutionStatus(depositAddress)` (`lib/oneClick.ts:141`)를 호출한다. PAL은 Zcash blockchain을 직접 조회하지 않는다 — 1Click SDK 응답을 blind trust한다. [§1.4](./subsystems/04-deposit-tracking.md), [§1.5](./subsystems/05-one-click-bridge.md)

**Step 13. [실패 경로] `INCOMPLETE_DEPOSIT` 상태** — 사용자가 필요 금액보다 적게 보낸 경우. `check-deposit/route.ts:65–68`에서 `{ incompleteDeposit: true }`를 반환하지만 후속 처리 없음. 추가 입금 안내 UX 없음. 1Click이 내부적으로 `REFUNDED`로 전이할 때까지 deposit이 limbo 상태. **`POST /api/relayer/refund`는 존재하지 않는다** (`_claims-to-verify.md:203`, DEPLOY.md 주장 refuted). [§1.4](./subsystems/04-deposit-tracking.md), [§1.5](./subsystems/05-one-click-bridge.md)

---

**[x402 실행 → Content Unlock]**

**Step 14.** 1Click status가 `SUCCESS`로 전환되면 cron이 `if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed)` (`cronjob-check-deposits/route.ts:47`) 조건을 통과하여 x402 실행 분기에 진입한다. idempotency guard로 중복 실행을 방지한다.

**Step 15.** x402 파라미터 조립 (`cronjob-check-deposits/route.ts:80–88`):
```
payTo     = quote?.payTo || tracking.recipient  # 실질적으로 항상 tracking.recipient
deadline  = Date.now()/1000 + 3600              # 원본 quote deadline 무시, 1시간 재계산
nonce     = 0x${Date.now().toString(16)}        # timestamp 기반 (암호학적 무작위 아님)
```
[§1.7](./subsystems/07-x402-client.md)

**Step 16.** `signX402TransactionWithChainSignature({ payTo, maxAmountRequired, deadline, nonce })` 호출 (`lib/chainSig.ts:210`). 내부적으로 두 번의 NEAR MPC 서명이 발생한다:
- **MPC #1** (`lib/chainSig.ts:147`): EIP-712 `TransferWithAuthorization` hash 서명 → `(v, r, s)` — USDC 컨트랙트가 검증할 EIP-3009 authorization
- **MPC #2** (`lib/chainSig.ts:372`): legacy EVM tx hash 서명 → RLP 직렬화 후 `sendRawTransaction`
두 서명 모두 path `'base-1'`, keyType `'Ecdsa'`로 `v1.signer` NEAR MPC 컨트랙트를 호출한다. 총 대기 시간 최대 ~60초. [§1.6](./subsystems/06-near-chain-signatures.md)

**Step 17.** `publicClient.sendRawTransaction({ serializedTransaction: signedTx })` (`lib/chainSig.ts:394`)가 Base mainnet에 ERC-20 `transferWithAuthorization`을 브로드캐스트하고 `broadcastTxHash`를 반환한다. **브로드캐스트는 이 함수 내에서 완료**된다. Settlement chain: Base mainnet (chainId `8453`). Settlement asset: USDC. [§1.7](./subsystems/07-x402-client.md)

**Step 18.** cron이 `updateDepositTracking(depositAddress, { signedPayload: transactionHash, x402Executed: true, confirmed: true })` (`cronjob-check-deposits/route.ts:135`)로 Supabase를 업데이트한다. `signed_payload` 컬럼에 저장되는 값은 "signed payload"가 아니라 **Ethereum tx hash** 문자열이다 — 컬럼명 오해 주의. [§1.4](./subsystems/04-deposit-tracking.md)

**Step 19.** UI가 `POST /api/relayer/check-deposit`을 폴링하다 `signedPayload`가 채워진 것을 감지하고, `GET /api/content/get-url?address={depositAddress}` → `{ redirectUrl, signedPayload: txHash, verified: true }` 를 받는다 (`app/api/content/get-url/route.ts:97`). [§1.7](./subsystems/07-x402-client.md)

**Step 20. [실패 경로] content get-url이 HTTP 402 반환** — `signedPayload`가 없거나 (`get-url/route.ts:52`) 1Click status가 `SUCCESS`가 아닌 경우 (`get-url/route.ts:63`). UI는 "결제 처리 중" 상태를 보여주며 다음 cron 실행을 기다린다.

**Step 21.** `app/content/page.tsx:141–147`가 `fetch(targetApiUrl, { headers: { 'X-PAYMENT': signedPayload } })`를 호출한다. `X-PAYMENT` 헤더 값은 Base mainnet Ethereum tx hash다 — 표준 x402 `PAYMENT-SIGNATURE` 헤더(base64 인코딩된 `PaymentPayload` JSON)가 아니다 (v1 헤더 이름 사용, EIP-3009 payload 아닌 tx hash bearer). [§1.7](./subsystems/07-x402-client.md)

**Step 22.** 외부 content 서버가 `X-PAYMENT` 헤더를 수신하여 tx hash를 검증하고 content JSON + `X-PAYMENT-RESPONSE` 헤더를 반환한다 (`app/content/page.tsx:151`). PAL 코드 범위 밖 — content 서버가 실제로 Base blockchain에서 tx를 검증하는지는 content 서버 구현에 달려 있다. [§1.7](./subsystems/07-x402-client.md)

---

> **더 깊은 커버리지:**
> - Steps 1–5 → [§1.1 intent parser](./subsystems/01-intent-parser.md), [§1.2 service registry](./subsystems/02-service-registry.md)
> - Steps 6–10 → [§1.3 z-address generation](./subsystems/03-z-address-generation.md), [§1.5 1Click bridge](./subsystems/05-one-click-bridge.md)
> - Steps 11–13 → [§1.4 deposit tracking](./subsystems/04-deposit-tracking.md)
> - Steps 14–18 → [§1.6 NEAR Chain Signatures](./subsystems/06-near-chain-signatures.md), [§1.7 x402 client](./subsystems/07-x402-client.md)
> - Steps 19–22 → [§1.7 x402 client](./subsystems/07-x402-client.md)
> - Rust contract role → [§1.8 NEAR Rust contract](./subsystems/08-near-rust-contract.md) (결론: 모든 위 단계에서 dead code)

---

## 2.3 Lift-and-use vs Redo

### Lift-and-use (그대로 가져올 것)

#### 1. Intent parsing pipeline (LLM completion + 서비스 레지스트리 시맨틱 검색)

`lib/intentParser.ts:16`, `lib/nearAI.ts:29`, `app/api/parse-intent/route.ts:16`. 자연어 결제 의도를 `{ amount, currency, chain, receivingAddress, bridgeFrom }` 구조체로 변환하는 3단계 파이프라인 (pgvector 시맨틱 검색 → LLM chat completion → rule-based fallback)은 asset에 무관하다. LLM system prompt에 `bridgeFrom: 'zcash'`가 하드코딩되어 있어 (`lib/nearAI.ts:44,94`) PAL의 Zcash-first 가정이 이미 반영되어 있다. Category E 프로젝트에서 `receivingAddress` 필드를 Zcash shielded address(또는 Unified Address)로 확장하면 intent parsing 레이어는 변경 없이 재사용할 수 있다. [§1.1](./subsystems/01-intent-parser.md)

#### 2. Supabase + pgvector 서비스 레지스트리 스키마

`supabase-setup.sql:8–78`, `lib/serviceRegistry.ts:52–96`. `payment_services` 테이블 DDL(IVFFlat `vector(1536)` 인덱스 포함), `match_services` PostgreSQL 함수(코사인 거리 `<=>`, threshold 필터), insert-time 임베딩 생성 + 검색-time 쿼리 임베딩의 비대칭 패턴이 완전히 재사용 가능하다. `receiving_address` 컬럼을 ZEC UA 형식으로 확장하고, `chain` 제약을 `'base' | 'solana'` → `'base' | 'solana' | 'zcash'`로 확장하면 된다. RLS 없이 anon key 클라이언트로 동작하므로 보안 설정만 검토 필요. [§1.2](./subsystems/02-service-registry.md)

#### 3. NEAR Chain Signatures 패턴 (서버 사이드 EVM 서명)

`lib/chainSig.ts:24–53` (모듈 초기화), `lib/chainSig.ts:87–103` (`deriveAddressAndPublicKey`), `lib/chainSig.ts:128–201` (`signTypedDataWithChainSignature`). NEAR proxy account의 private key(`NEAR_PROXY_PRIVATE_KEY`)를 환경 변수로 보유하고, `chainsig.js` SDK가 NEAR MPC `v1.signer`에 cross-contract call로 서명을 요청하는 패턴은 EVM 외의 다른 체인에도 적용 가능하다. Zcash 서명을 NEAR MPC로 수행하려면 NEAR MPC가 secp256k1 이외의 키 타입을 지원해야 하므로 직접 적용은 불가하지만, EVM settlement 레이어(예: Base에 ZEC 영수증 anchor)가 있다면 그대로 재사용할 수 있다. [§1.6](./subsystems/06-near-chain-signatures.md)

#### 4. EIP-3009 `transferWithAuthorization` payload 구성

`lib/chainSig.ts:234–265` (EIP-712 domain + types + value 구성), `lib/chainSig.ts:304–342` (calldata ABI 인코딩). EIP-712 typed data hash → MPC 서명 → `transferWithAuthorization` calldata 조립의 전체 흐름이 구체적이고 동작 검증된 코드다. USDC on Base를 settlement asset으로 계속 사용하는 경우 그대로 lift 가능하다. EIP-3009 nonce(`0x${Date.now().toString(16)}`)의 timestamp 기반 생성은 암호학적 무작위가 아니어서 교체 권장 (`lib/chainSig.ts:88`). [§1.6](./subsystems/06-near-chain-signatures.md), [§1.7](./subsystems/07-x402-client.md)

#### 5. Vercel cron deposit polling state machine

`app/api/relayer/cronjob-check-deposits/route.ts:15–160`, `vercel.json:7–11`. `deadline > now()` 필터로 미만료 deposit만 순회하고, 외부 swap 상태 폴링 → SUCCESS 감지 → 결제 실행 → DB 업데이트 의 idempotent 루프 구조는 재사용 가능하다. 1Click SDK 의존을 제거하고 lightwalletd 폴링으로 교체하면 Category E Zcash 버전으로 전환된다. cron 인증(`CRON_SECRET`)이 주석 처리된 것은 반드시 수정해야 한다 (`route.ts:17–21`). [§1.4](./subsystems/04-deposit-tracking.md)

#### 6. `deposit_tracking` SQL 스키마

`supabase-deposit-tracking.sql:5–25`. `deposit_address TEXT PRIMARY KEY`, `quote_data JSONB`, `signed_payload TEXT`, `deadline TIMESTAMP WITH TIME ZONE`, `x402_executed BOOLEAN`, `confirmed BOOLEAN`, `redirect_url TEXT` 구조가 Zcash 기반 Category E에서도 거의 그대로 사용 가능하다. `swap_wallet_address` 컬럼은 USDC 중간 주소로 Zcash-native 흐름에서는 제거하거나 Zcash nullifier로 대체할 수 있다. partial index(`deadline IS NOT NULL AND confirmed = false`)가 cron 폴링 최적화에 유효하다. `signed_payload` 컬럼의 실제 값이 tx hash임을 문서화하거나 컬럼명을 변경해야 한다. [§1.4](./subsystems/04-deposit-tracking.md)

---

### Redo (Category E를 위해 다시 만들어야 할 것)

#### 1. 실제 Zcash shielded address 파생

PAL에는 Zcash native 암호화 라이브러리가 단 하나도 없다. `bech32`(`lib/kdf.ts:8`)는 Cosmos 주소 전용이고, `bs58check`는 Bitcoin 전용이다 ([§1.3](./subsystems/03-z-address-generation.md), [§3.1](./zcash-tool-inventory.md)). Category E에서 사용자에게 진짜 Zcash shielded address(Orchard Unified Address 또는 Sapling address)를 발급하려면 처음부터 구현해야 한다. 권장 경로: Rust `zcash_client_backend` + `orchard` crate로 ZIP-32 HD derivation → Orchard spending key → Unified Address 인코딩. 브라우저 환경이 필요하면 WASM으로 컴파일하거나 `@d4mr/t2z-wasm` (week2 #20 t2z) 참조.

#### 2. 실제 x402 facilitator 연동 또는 Zcash-native settlement 경로

PAL은 외부 facilitator 없이 USDC `transferWithAuthorization`을 직접 브로드캐스트한다 (`lib/chainSig.ts:394`, [§1.7](./subsystems/07-x402-client.md)). Category E에서 Zcash를 settlement asset으로 쓰려면 표준 x402 facilitator가 shielded tx를 `/verify`하고 `/settle`할 수 있어야 한다. 현재 어떤 공개된 x402 facilitator도 Zcash shielded pool을 지원하지 않는다 (Background reading §privacy implications 참조). 독립 구현 방향: lightwalletd를 통한 incoming viewing key 기반 잔액 확인 + nullifier set 조회를 `/verify` 로직으로 사용.

#### 3. 사용자별 / 주문별 key isolation

PAL은 `MPC_PATH = 'base-1'`로 하드코딩되어 있어 모든 사용자/주문이 동일한 `swapWallet` EVM 주소를 공유한다 (`lib/chainSig.ts:18`, [§1.6](./subsystems/06-near-chain-signatures.md)). 실제로 `deriveAddressAndPublicKey(derivationPath?)` 함수 시그니처에 path 파라미터가 있지만 내부에서 `const path = 'base-1'`로 재정의하여 무시된다 (`lib/chainSig.ts:94`). Category E에서 사용자별 또는 주문별 ephemeral Zcash address를 발급하려면 per-intent key derivation이 필수다. NEAR MPC의 path 파라미터를 `'zcash-{intentId}'` 패턴으로 활용하거나, ZIP-32 HD path를 per-intent로 파생하는 방식을 고려해야 한다.

#### 4. Zcash-native deposit 검증 (체인 직접 조회)

PAL은 1Click SDK의 `getExecutionStatus()` 응답을 blind trust하며 Zcash blockchain을 직접 조회하지 않는다 (`lib/oneClick.ts:141`, [§1.4](./subsystems/04-deposit-tracking.md)). lightwalletd, Zebra RPC, 또는 기타 Zcash 풀노드/SPV 클라이언트와의 통신이 코드 전체에 단 한 줄도 없다 ([§3.1](./zcash-tool-inventory.md)). Category E에서 Zcash 입금을 독립적으로 검증하려면 lightwalletd 구독(`BlockRange` streaming) + incoming viewing key(IVK)로 note 복호화 + memo 파싱이 필요하다. `zcash_client_backend`의 `compact_block_scanner` 또는 `ZcashLightClientKit`(iOS) 참조.

#### 5. 환불 / 재시도 / 분쟁 경로

`POST /api/relayer/refund`는 존재하지 않는다 (`_claims-to-verify.md` DEPLOY.md refuted claim, [§1.4](./subsystems/04-deposit-tracking.md)). `INCOMPLETE_DEPOSIT` 상태는 limbo로 빠지고, x402 실행 실패 후 deadline 만료 시 USDC가 `swapWallet`에 묶인다 ([§1.7](./subsystems/07-x402-client.md)). Category E에서 Zcash settlement를 사용하면 환불은 ZEC 기준으로 처리되어야 한다. HTLC 기반 timelock 환불 패턴(week2 #10 Shadow Swap의 `zcash-htlc-builder`)이 명시적 환불 흐름을 자연스럽게 포함한다.

#### 6. Replay 방어 — Zcash nullifier 또는 memo `quote_hash` 기반

PAL은 EIP-3009 nonce(`Date.now()` hex, `lib/chainSig.ts:88`)로 replay를 방어하지만 이는 EVM USDC 컨트랙트 레이어의 방어다 ([§1.7](./subsystems/07-x402-client.md)). Zcash settlement를 사용하면 replay 방어 메커니즘을 새로 설계해야 한다. 두 가지 검증된 접근: (a) Zcash nullifier — note 소비 시 전 세계적으로 유일한 nullifier가 공개 기록됨, (b) Secure Legion의 `NLx402:<quote_hash>` memo carry pattern — memo 복호화 + local DB hash 매칭으로 replay 차단 [week2 §#26].

#### 7. 제3자 AML/감시 연결 제거

1Click은 모든 quote 요청에 NEAR Intents AML Portal, Binance AML, AMLBot & PureFi, TRM Labs 스크리닝을 자동 적용하고 법 집행 기관 조회 창구(Kodex Global)를 운영한다 ([§3.1](./zcash-tool-inventory.md)). PAL이 1Click을 사용하는 한 이 감시 체계를 우회할 수 없다. Category E에서 진정한 privacy를 달성하려면 1Click 의존을 완전히 제거하고, 자체 Zcash lightwalletd 노드 + 자체 swap 인프라로 대체해야 한다. 완전 제거가 어렵다면 중간 단계로 viewing key disclosure scope를 최소화(merchant IVK만 공개, 제3자 없음)하는 구조를 우선 적용한다.

---

## 2.4 차별화 여지 (Differentiation room for our team)

> **Zcash를 x402 settlement rail로 직접 만드는 것** — PAL이 우회한 공백 — 이 Category E의 핵심 차별화 방향이다. 아래 제안들은 PAL의 구체적 취약점을 각각 다른 Zcash primitive로 해결한다.

---

> **1. Zcash memo를 x402 quote_hash carrier로 사용 (Secure Legion 패턴 직접 적용)**
>
> What it changes vs. PAL: PAL은 Zcash와 x402를 완전히 분리해 ZEC → USDC → Base x402 경로를 거친다. 이 대안은 Zcash shielded tx 자체가 x402 영수증의 carrier가 되도록 한다. Merchant가 `(quote_id, quote_hash, amount, expiry)`를 생성하면 사용자가 ZEC tx memo 필드에 `NLx402:<quote_hash>`를 삽입하여 결제와 영수증을 단일 L1 tx로 처리한다. 1Click을 거치지 않아 Defuse Labs Limited의 AML 연결이 완전히 사라진다.
>
> Concrete primitive: `NLx402:<quote_hash>` memo encoding [week2 §#26 Secure Legion]. Merchant 수신 측은 incoming viewing key(IVK)로 memo를 복호화해 `quote_hash`가 local SQLCipher DB의 미결제 quote와 매칭되는지 확인 — replay 방어 내장. PCEF NLx402 facilitator(Solana)를 Zcash 자체 검증으로 대체하면 외부 facilitator 의존도 제거된다.
>
> Implementation note: MVP 범위 — lightwalletd IVK 스캔 + memo 복호화 + quote_hash 매칭. `zcash_client_backend`의 `scan_cached_blocks` + `decrypt_transaction`이 핵심 Rust 의존. NLx402 PCEF의 공개 spec이 없으므로 Secure Legion 코드에서 역산하거나 독립 spec 작성이 필요하다.

---

> **2. ZIP-321 payment URI를 QR에 인코딩**
>
> What it changes vs. PAL: PAL은 `<QRCodeSVG value={depositAddress}>` (`components/IntentsQR.tsx:186`)로 bare address만 QR에 인코딩한다. 사용자는 amount를 지갑에서 별도로 입력해야 하고, 잘못 입력하면 `INCOMPLETE_DEPOSIT` limbo 상태가 된다.
>
> Concrete primitive: [ZIP-321](https://zips.z.cash/zip-0321) payment URI 스펙. `zcash:<address>?amount=<zec_amount>&memo=<base64url_memo>`를 QR에 인코딩하면 사용자가 스캔만 해도 amount와 memo가 자동으로 채워진다. Amount 필드로 정확한 ZEC 금액(1Click EXACT_OUTPUT 계산값)을, memo 필드로 `quote_hash`나 `intentId`를 인코딩할 수 있다. 대부분의 현대 Zcash 지갑(Zashi, YWallet 등)이 ZIP-321을 지원한다.
>
> Implementation note: PAL의 `register-deposit` 응답에서 `zcashAmount`와 `depositAddress`를 받아 ZIP-321 URI를 조립하는 단순한 함수 추가로 구현 가능. 기존 `getSwapQuote` 응답의 `amountInFormatted` 필드를 사용. `memo` 파라미터에 `quote_hash`를 Base64url로 인코딩하면 Proposal 1(memo carry)과 자연스럽게 결합된다.

---

> **3. Viewing-key 기반 영수증 — 머천트 독립 검증**
>
> What it changes vs. PAL: PAL은 1Click SDK의 `getExecutionStatus()` 응답을 blind trust하며 Zcash chain을 직접 조회하지 않는다 ([§1.4](./subsystems/04-deposit-tracking.md)). 1Click이 거짓 `SUCCESS`를 반환하면 ZEC 미수신 상태에서 x402가 실행된다.
>
> Concrete primitive: UFVK (Unified Full Viewing Key) 모니터링 패턴 [week2 #52 Overpay]. Merchant가 UFVK를 보유하고, lightwalletd `CompactBlock` 스트림을 구독하여 incoming note를 복호화 + Postgres 동기화한다. Merchant는 제3자(1Click, PAL 서버)를 신뢰하지 않고 ZEC 입금을 직접 확인한다. viewing key는 spending key를 노출하지 않으므로 merchant가 ZEC를 인출할 수 없다 — selective disclosure의 가장 단순한 형태.
>
> Implementation note: `zcash_client_backend`의 `wallet_light_client` 기능이 UFVK + lightwalletd 연동을 제공한다. week2 #52 Overpay가 이 패턴을 working demo 수준으로 구현했지만 GitHub repo가 비공개 — 아키텍처 참조 후 독립 구현 필요. week2 #4 Zcash↔Aztec Bridge의 `bridge_watcher.rs`가 UFVK 폴링 + memo 복호화 코드의 공개된 가장 깨끗한 reference다.

---

> **4. USDC 브릿지 제거 — Zcash를 직접 settlement asset으로**
>
> What it changes vs. PAL: PAL의 핵심 취약점은 ZEC가 USDC로 변환되면서 Defuse Labs Limited(Gibraltar 법인) 의존과 AML 연결이 발생한다는 것이다. ZEC → USDC 스왑을 제거하고 ZEC 자체를 x402 settlement asset으로 직접 사용하면 이 의존이 완전히 사라진다.
>
> Concrete primitive: Zcash shielded tx가 결제이고 merchant가 ZEC를 직접 수령한다. x402 facilitator는 `/verify` 단계에서 lightwalletd를 통해 shielded note 수령과 nullifier를 확인하고, `/settle` 단계는 Zcash tx 브로드캐스트다 — EVM `transferWithAuthorization`이 아니다. 이 구조는 현재 표준 x402 spec의 EVM `exact` scheme을 Zcash-native scheme으로 확장하는 것이며, 새 scheme 정의가 필요하다 (Background reading §privacy implications 참조).
>
> Implementation note: 가장 큰 범위의 변경이다. MVP로 먼저 "merchant가 ZEC 수령을 수동으로 확인하는 반-자동 x402 흐름"을 구현하고, 이후 lightwalletd 기반 자동 확인으로 확장하는 단계별 접근이 현실적이다. Zcash tx 브로드캐스트는 `zcash_client_backend`의 `submit_transaction`을 사용.

---

> **5. 사용자별 ephemeral z-address (per-intent key isolation)**
>
> What it changes vs. PAL: PAL은 `MPC_PATH = 'base-1'` 하드코딩으로 모든 사용자/주문이 동일한 `swapWallet`을 공유한다 (`lib/chainSig.ts:18`). 이는 funding UTXO들의 시간적 연관성, swapWallet의 USDC 잔액 추적 등 여러 linkability 경로를 남긴다.
>
> Concrete primitive: ZIP-32 HD path per-intent파생 또는 Orchard one-time Unified Address 생성 [week2 #19 Zipher 캡슐 패턴]. 각 x402 quote에 고유한 `intentId`를 부여하고, `m/32'/133'/0'/{intentId}'` 형태의 ZIP-32 derivation path로 ephemeral Orchard 주소를 파생한다. Merchant는 파생된 주소의 IVK만 저장하고, 결제 완료 후 spending key를 폐기한다.
>
> Implementation note: NEAR MPC의 path 파라미터(`'zcash-{intentId}'`)를 사용하면 NEAR MPC 인프라를 재활용하면서 per-intent isolation을 달성할 수 있다 — 단, NEAR MPC가 Zcash secp256k1 curve 이외의 서명(Orchard는 RedJubjub)을 지원해야 하므로 EVM settlement와 혼합하거나 독립 key management가 필요하다.

---

> **6. PCZT 기반 multi-party signing (책임 있는 결제)**
>
> What it changes vs. PAL: PAL의 `swapWallet`은 NEAR proxy key를 보유한 서버가 단독 서명한다. 키 탈취 시 전체 USDC 유출이 가능하다. PCZT(Partially Created Zcash Transaction, ZIP-374) 기반 멀티파티 서명으로 서버, merchant, escrow agent가 각각 부분 서명을 기여하면 단일 포인트 실패를 제거할 수 있다.
>
> Concrete primitive: `pczt` Rust crate (ZIP-374) [week2 #54 Temi, #20/#21 t2z]. propose → prove → sign → combine → finalize 5단계 흐름. TypeScript 환경에서는 Temi가 napi-rs FFI로 같은 흐름을 구현했다 (week2 #54). 이 패턴을 x402 settlement에 적용하면 merchant가 proposal을 생성하고 사용자가 spending key로 서명하며 facilitator가 최종화하는 3-party x402 흐름이 가능하다.
>
> Implementation note: PCZT는 현재 표준화 진행 중 (ZIP-374). Temi의 TypeScript wrapper가 가장 구체적인 starting point다. MVP에서는 2-party(사용자 + merchant)만 구현하고, escrow를 제3자 서명자로 추가하는 것은 Phase 2로 미룰 수 있다.

---

> **7. lightwalletd 직접 연동 — Zcash chain verification**
>
> What it changes vs. PAL: PAL의 전체 deposit tracking이 1Click SDK의 응답에만 의존한다 ([§1.4](./subsystems/04-deposit-tracking.md), [§1.5](./subsystems/05-one-click-bridge.md)). lightwalletd를 직접 연동하면 Zcash chain 상태를 독립 검증할 수 있어 1Click 의존을 완전히 제거하거나 보조 검증 수단으로 사용할 수 있다.
>
> Concrete primitive: lightwalletd gRPC API — `GetCompactBlocks`, `GetTransaction`, `SendTransaction`. Cron 폴링 대신 `BlockRangeStream`을 구독하면 near-real-time 입금 감지가 가능하다. PAL의 `getDepositsWithDeadlineRemaining()` 폴링 루프를 lightwalletd stream으로 대체하는 것이 가장 직접적인 교체다.
>
> Implementation note: 자체 lightwalletd 노드 운영 또는 신뢰할 수 있는 공개 노드 사용. 테스트넷(Zcash testnet)에 먼저 구축하고 메인넷으로 이전하는 순서. `zcash_client_backend`의 `scanning` 모듈이 lightwalletd 연동 Rust 레이어를 제공하며, Next.js 서버와는 gRPC-web 또는 REST 변환 레이어를 통해 연결할 수 있다.

## Background reading: x402 facilitator landscape

### x402 프로토콜 한 단락 정의

x402는 HTTP 402 "Payment Required" 상태 코드를 이용해 서버가 클라이언트에게 결제를 자동으로 요구하고 클라이언트(또는 AI 에이전트)가 서명된 결제 proof를 포함하여 재요청하는 **인터넷 네이티브 결제 표준**이다. 원래 Coinbase가 제안했으나 현재는 [x402 Foundation](https://github.com/x402-foundation/x402)으로 이관된 오픈 스탠다드로, "결제를 완전히 HTTP 계층 위에서 처리하여 account, session, API key 없이 stablecoin 결제를 가능하게 한다"는 것이 핵심 목표다. 프로토콜 명세는 [x402-specification-v2.md](https://github.com/x402-foundation/x402/blob/main/specs/x402-specification-v2.md)에서 관리되며, v1(2025-08 출시)과 v2(2025-12 개정) 두 버전이 존재하고 PAL의 코드는 v1 필드명(`maxAmountRequired`, `X-PAYMENT` 헤더)을 사용한다.

---

### 메시지 포맷

#### `PaymentRequired` (서버 → 클라이언트, HTTP 402 응답)

서버는 HTTP 402 응답과 함께 `PAYMENT-REQUIRED` 헤더에 아래 JSON을 **base64 인코딩**하여 전달한다 (v2 기준; v1에서는 동일 JSON이 `X-PAYMENT-REQUIRED` 헤더로 전달됨).

```json
{
  "x402Version": 2,
  "error": "PAYMENT-SIGNATURE header is required",
  "resource": {
    "url": "https://api.example.com/premium-data",
    "description": "Access to premium market data",
    "mimeType": "application/json"
  },
  "accepts": [
    {
      "scheme": "exact",
      "network": "eip155:84532",
      "amount": "10000",
      "asset": "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
      "payTo": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
      "maxTimeoutSeconds": 60,
      "extra": {
        "name": "USDC",
        "version": "2"
      }
    }
  ]
}
```

`accepts` 배열 안의 `PaymentRequirements` 오브젝트 주요 필드 (v2 명칭 기준):

| 필드 | 타입 | 설명 |
|------|------|------|
| `scheme` | string | 결제 scheme (`"exact"`, `"upto"` 등) |
| `network` | string | CAIP-2 체인 식별자 (예: `"eip155:8453"` = Base mainnet) |
| `amount` | string | 결제 금액 (atomic unit; v1에서는 `maxAmountRequired`) |
| `asset` | string | ERC-20 컨트랙트 주소 또는 ISO 4217 통화 코드 |
| `payTo` | string | 수신자 지갑 주소 |
| `maxTimeoutSeconds` | number | 결제 완료 허용 최대 시간 (초) |
| `extra` | object | scheme별 추가 정보 (EVM `exact`의 경우 `name`, `version`) |
| `resource` | string | (v1에서는 `PaymentRequirements` 내부 필드였으나 v2에서는 상위 `resource` 오브젝트로 분리) |

**v1 vs v2 필드명 차이:** v1은 `network`가 문자열 이름(`"base-sepolia"`)이고 `PaymentRequirements` 내부에 `resource`, `description`, `mimeType`이 포함됐다. v2는 `network`가 CAIP-2 형식(`"eip155:84532"`)이고 이 필드들이 상위 `resource` 오브젝트로 분리됐다. PAL 코드(`lib/chainSig.ts:80-88`)는 v1 필드명(`maxAmountRequired`)을 사용한다.

---

#### `PAYMENT-SIGNATURE` 헤더 (클라이언트 → 서버, `PaymentPayload`)

클라이언트는 `PAYMENT-SIGNATURE` 헤더에 아래 JSON을 **base64 인코딩**하여 전달한다 (v2 기준; v1에서는 `X-PAYMENT` 헤더 사용).

```json
{
  "x402Version": 2,
  "resource": {
    "url": "https://api.example.com/premium-data",
    "description": "Access to premium market data",
    "mimeType": "application/json"
  },
  "accepted": {
    "scheme": "exact",
    "network": "eip155:84532",
    "amount": "10000",
    "asset": "0x036CbD53842c5426634e7929541eC2318f3dCF7e",
    "payTo": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
    "maxTimeoutSeconds": 60,
    "extra": { "name": "USDC", "version": "2" }
  },
  "payload": {
    "signature": "0x2d6a7588...571c",
    "authorization": {
      "from": "0x857b06519E91e3A54538791bDbb0E22373e36b66",
      "to": "0x209693Bc6afc0C5328bA36FaF03C514EF312287C",
      "value": "10000",
      "validAfter": "1740672089",
      "validBefore": "1740672154",
      "nonce": "0xf3746613c2d920b5fdabc0856f2aeb2d4f88ee6037b8cc5d04a71a4462f13480"
    }
  }
}
```

**헤더 이름 버전 차이:**
- v1: `X-PAYMENT` (PAL 코드 `app/content/page.tsx:144`가 사용하는 이름)
- v2: `PAYMENT-SIGNATURE`

---

#### `exact` scheme on EVM — EIP-3009 typed data 구조

`payload.authorization`은 EIP-3009 `transferWithAuthorization`의 EIP-712 typed data이다. 명세([x402-specification-v2.md §6.1.1](https://github.com/x402-foundation/x402/blob/main/specs/x402-specification-v2.md))에서 직접 인용:

```javascript
const authorizationTypes = {
  TransferWithAuthorization: [
    { name: "from",        type: "address" },
    { name: "to",          type: "address" },
    { name: "value",       type: "uint256" },
    { name: "validAfter",  type: "uint256" },
    { name: "validBefore", type: "uint256" },
    { name: "nonce",       type: "bytes32" },
  ],
};
```

- `from`: 결제자 지갑 (서명자)
- `to`: 수신자 지갑 (`payTo`와 동일해야 함)
- `value`: 결제 금액 (atomic unit)
- `validAfter` / `validBefore`: authorization 유효 시간 창 (Unix timestamp)
- `nonce`: 32-byte 랜덤값 — EIP-3009 컨트랙트 수준에서 사용 후 소각, replay 방어

Settlement는 facilitator가 `transferWithAuthorization(from, to, value, validAfter, validBefore, nonce, v, r, s)`를 ERC-20 컨트랙트에 직접 호출하여 실행한다. Facilitator는 금액이나 수신자를 변경할 수 없다 — 명세 인용: *"The Facilitator cannot modify the amount or destination. They serve only as the transaction broadcaster."*

또한 EVM에서는 `exact` scheme이 EIP-3009 외에 **Permit2** (Uniswap, proxy 컨트랙트 `0x402085c248EeA27D92E8b30b2C58ed07f9E20001`)와 **ERC-7710** (delegation 기반)도 지원한다.

---

#### `PAYMENT-RESPONSE` 헤더 (서버 → 클라이언트, `SettlementResponse`)

서버는 settlement 완료 후 `PAYMENT-RESPONSE` 헤더에 아래 JSON을 **base64 인코딩**하여 반환한다:

```json
{
  "success": true,
  "transaction": "0x1234567890abcdef...",
  "network": "eip155:84532",
  "payer": "0x857b06519E91e3A54538791bDbb0E22373e36b66"
}
```

| 필드 | 타입 | 설명 |
|------|------|------|
| `success` | boolean | settlement 성공 여부 |
| `transaction` | string | on-chain tx hash |
| `network` | string | CAIP-2 체인 식별자 |
| `payer` | string | 결제자 지갑 주소 |
| `errorReason` | string | 실패 시 오류 이유 |

출처: [x402 HTTP transport spec](https://github.com/x402-foundation/x402/blob/main/specs/transports-v2/http.md)

---

### 알려진 facilitator 구현

| 이름 | 운영자 | 지원 체인 | 지원 asset | 출처 / URL |
|------|--------|-----------|------------|------------|
| Coinbase x402 facilitator | Coinbase / x402 Foundation | Base mainnet, Base Sepolia, Polygon, Arbitrum, World, Solana, Avalanche (+ 추가 예정) | USDC (EIP-3009 & Permit2), EURC, SPL tokens | [https://x402.org/facilitator](https://x402.org/facilitator) — Cloudflare 공식 예제에서 사용 확인 ([Cloudflare x402 docs](https://developers.cloudflare.com/agents/x402/)) |
| PayAI x402 Facilitator | PayAI, Inc. | Solana, Base, Polygon, Avalanche, Sei, SKALE, XLayer, Peaq, IoTeX, KiteAI | Stablecoins + 커스텀 토큰 | [https://facilitator.payai.network](https://facilitator.payai.network) |
| second-state x402-facilitator | Second-State (자체 호스팅 템플릿) | Base, Avalanche, Polygon, Sei, Solana (RPC URL 설정으로 추가 가능) | USDC, USDT | [github.com/second-state/x402-facilitator](https://github.com/second-state/x402-facilitator) (self-hosted, Docker 배포) |
| OpenZeppelin x402 facilitator | OpenZeppelin (Stellar 특화) | Stellar | USDC (Stellar) | [docs.openzeppelin.com/relayer/guides/stellar-x402-facilitator-guide](https://docs.openzeppelin.com/relayer/guides/stellar-x402-facilitator-guide) |
| NLx402 | PCEF (Perkins Coie Entrepreneur Fund, 501(c)(3) nonprofit) | Solana (추정; 공식 문서 없음) | USDC (추정) | 주간 2 참조; [Secure Legion GitHub](https://github.com/Secure-Legion) acknowledgments에서 "NLx402 payment protocol core logic, attributed to PCEF" 확인 — 독립 문서 미공개 |

**NLx402에 대한 별도 설명:** NLx402는 Secure Legion의 messaging app ([Secure-Legion/android](https://github.com/Secure-Legion/android))의 acknowledgments 섹션에서 "PCEF (Perkins Coie Entrepreneur Fund, 501(c)(3))가 개발한 payment protocol core logic"으로 언급된다. 공식 spec 문서나 독립 GitHub repo는 공개되지 않았으며, Solana mainnet 기반 facilitator로 추정된다. Secure Legion의 구조에서 NLx402는 `NLx402:<quote_hash>` 형태의 memo를 Zcash shielded transaction의 encrypted memo field에 삽입함으로써 Zcash shielded tx가 x402 결제 proof를 동시에 carry하는 방식으로 사용됐다 (week2 §#26 참조). **권위 있는 1차 출처 없음 — week2 reference와 Secure Legion acknowledgment 외 공식 문서 미발견.**

---

### facilitator API 표면

x402 compliant facilitator는 다음 HTTP 엔드포인트를 반드시 노출해야 한다 (출처: [x402-specification-v2.md §7](https://github.com/x402-foundation/x402/blob/main/specs/x402-specification-v2.md)):

| 엔드포인트 | 메서드 | 설명 |
|-----------|--------|------|
| `/verify` | POST | `PaymentPayload` + `PaymentRequirements`를 받아 signature 검증, balance 확인, tx simulation을 수행하고 `{ isValid, payer }` 반환. **blockchain 상태는 변경하지 않음.** |
| `/settle` | POST | `/verify`와 동일한 body를 받아 `transferWithAuthorization`(또는 Permit2/ERC-7710 등 scheme별 함수)을 실제로 blockchain에 broadcast하고 tx hash를 반환. **가스비는 facilitator가 부담.** |
| `/supported` | GET | facilitator가 지원하는 `(scheme, network)` 쌍 목록과 signer 주소를 반환. |

**`/verify` 요청 body 예시:**
```json
{
  "x402Version": 2,
  "paymentPayload": { /* PaymentPayload 전체 */ },
  "paymentRequirements": { /* PaymentRequirements 한 항목 */ }
}
```

**`/verify` 성공 응답:**
```json
{ "isValid": true, "payer": "0x857b..." }
```

**`/settle` 성공 응답:**
```json
{
  "success": true,
  "payer": "0x857b...",
  "transaction": "0x1234...",
  "network": "eip155:84532"
}
```

Resource server는 `/verify` 만 호출하고 자체적으로 settlement를 처리할 수도 있고, 또는 `/verify` 없이 `/settle`만 호출할 수도 있다 — 두 경우 모두 명세가 허용한다.

---

### Privacy implications: settlement asset이 privacy-preserving이면 어떻게 변하는가

x402의 현행 EVM `exact` scheme은 **투명한 ERC-20 토큰** (USDC on Base)을 전제로 설계되어 있다. Settlement asset을 shielded ZEC처럼 privacy-preserving 자산으로 바꾸면 프로토콜의 여러 전제가 근본적으로 달라진다.

#### `payTo` 필드 — shielded address가 되는가?

현행 명세에서 `payTo`는 EVM checksum address(예: `0x209693Bc...`) 또는 Solana public key다. Zcash shielded 결제로 전환하면 `payTo`는 `u1...` 또는 `zs1...` 형식의 Unified Address 또는 Sapling address가 되어야 한다. 이 주소는 EVM 주소와 형식이 완전히 다르므로 기존 facilitator의 주소 파싱·검증 로직이 호환되지 않는다. 또한 shielded address는 수신자가 자발적으로 disclosure key를 공개하지 않는 한 on-chain에서 잔액 조회 자체가 불가능하다.

#### facilitator의 `/verify` 능력 — shielded balance를 검증할 수 있는가?

현행 EVM `exact` scheme에서 `/verify`는 (1) signature ecrecover, (2) `balanceOf(from)` 조회, (3) `transferWithAuthorization` simulation 세 단계를 수행한다. Zcash shielded pool에서는 이 세 단계 모두 작동하지 않는다:

- **Signature ecrecover 불가:** Zcash shielded tx의 서명은 EIP-712 구조가 아니라 Sapling/Orchard spending key에서 파생된 재편증명(spend proof)이다.
- **`balanceOf` 조회 불가:** shielded note는 암호화되어 있어 facilitator가 특정 address의 잔액을 외부에서 조회할 방법이 없다. 잔액 검증은 spending key(또는 incoming viewing key)를 가진 주체만 수행할 수 있다.
- **Simulation 불가:** Zcash 결제는 EVM smart contract call이 아니라 zk-SNARK proof(Sapling/Orchard) 생성과 UTXO 소비를 수반하며, facilitator가 이를 사전 시뮬레이션하는 표준 인터페이스가 없다.

결론적으로, 표준 x402 `/verify` 인터페이스는 shielded asset에 적용되지 않는다 — **facilitator가 검증 역할을 수행하려면 새로운 scheme 정의가 필요하다.**

#### Replay 방어 메커니즘 — nonce vs Zcash nullifier vs memo carry

현행 EVM `exact` scheme의 replay 방어는 EIP-3009 nonce에 의존한다: nonce는 ERC-20 컨트랙트에 사용 후 기록되며, 동일 nonce로 두 번 `transferWithAuthorization`을 호출하면 컨트랙트 수준에서 revert된다 (명세 §10.1 인용: *"EIP-3009 contracts inherently prevent nonce reuse at the smart contract level"*).

Zcash shielded tx에서의 replay 방어 메커니즘은 구조적으로 다르다:

- **Nullifier:** Zcash는 각 shielded note 소비 시 전 세계적으로 유일한 nullifier를 블록체인에 공개적으로 기록하고, 동일 note를 두 번 소비하면 nullifier 중복으로 거부된다. 이는 EIP-3009 nonce와 유사한 역할이지만, 검증 주체가 "ERC-20 컨트랙트"가 아니라 "Zcash 풀 전체"이며 facilitator가 직접 nullifier 집합에 접근하려면 lightwalletd 또는 full node가 필요하다.

- **Memo field carry (NLx402 패턴):** Secure Legion이 제안한 방식은 `NLx402:<quote_hash>`를 Zcash shielded tx의 encrypted memo field에 삽입하는 것이다. 이 경우 Zcash tx 자체가 x402 결제 proof의 carrier가 되며, facilitator는 수신자의 incoming viewing key로 memo를 복호화해 `quote_hash`를 확인함으로써 replay를 방어한다. 그러나 이 방식은 facilitator가 수신자의 incoming viewing key에 접근 가능해야 한다는 전제를 포함하며, 수신자의 개인정보 trade-off를 수반한다.

- **결제 후 proof 제출 (PAL 방식):** PAL은 반대 방향으로 접근한다 — shielded ZEC를 먼저 1Click으로 USDC로 swap한 뒤, USDC transfer의 tx hash를 `X-PAYMENT` 헤더의 bearer로 사용한다. 이 경우 replay 방어는 Base의 USDC 컨트랙트(EIP-3009 nonce)가 담당하고, Zcash privacy는 funding 단계에서만 존재한다. x402 protocol flow와 Zcash privacy가 완전히 분리된 구조다.

세 접근 방식 모두 "Zcash를 x402 settlement asset으로 직접 쓰는" 경우의 문제를 서로 다른 방식으로 회피하고 있다. Zcash를 x402 settlement rail로 직접 통합하는 facilitator — nullifier를 `/verify`의 replay check로 사용하고, shielded tx의 viewing key 기반 검증을 `/settle` 흐름에 통합하는 — 는 현재 공개된 구현 중에서 발견되지 않는다. 이 공백이 우리 팀의 Category-E 차별화 포인트로 남겨진 영역이다 (Task 11에서 구체화).
