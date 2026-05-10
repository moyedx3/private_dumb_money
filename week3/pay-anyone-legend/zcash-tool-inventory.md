# §3 Zcash dev-tool inventory + outsourcing story

## 3.1 What 1Click actually is

> **크로스 레퍼런스:** 이 섹션은 1Click 프로토콜 자체를 다룬다. PAL이 1Click을 구체적으로 어떻게 호출하는지는 [§1.5 1Click bridge](./05-one-click-bridge.md)를 참조하라.

---

### 한 단락 정의 (One-paragraph definition)

1Click(공식 명칭: "1Click Swap API")은 [NEAR Intents](https://docs.near-intents.org/) 프로토콜 위에 구축된 **크로스체인 swap REST API**로, Defuse Labs Limited(Gibraltar 법인)가 운영한다. 사용자(또는 PAL 같은 서비스)는 단일 API 호출(`POST /v0/quote`)로 원하는 자산 스왑 의도(intent)를 제출하면, 1Click이 NEAR Intents의 Market Maker(solver) 네트워크를 통해 최적 가격을 경쟁적으로 조달하고, 온체인 settlement(`intents.near` 스마트 컨트랙트)를 실행하여 목적지 체인의 지정 주소로 목적 자산을 전달한다. 이 과정에서 사용자 자산은 일시적으로 1Click의 "trusted swapping agent"가 보관하며, 스왑 실패 시 자동 환불(`refundTo` 주소)이 수행된다. PAL의 관점에서 1Click은 ZEC → USDC 변환과 Zcash deposit address 발급을 모두 담당하는 블랙박스 외부 서비스다 — PAL 코드베이스에는 Zcash 암호화 라이브러리가 단 한 줄도 없다.

---

### 운영 주체와 거버넌스 (Operator + governance)

#### 법인 및 운영 주체

공식 Terms of Service에 따르면:

> "1CS is a backend routing/services layer developed and maintained by **Defuse Labs Limited**, a company incorporated in **Gibraltar**."

— [1Click Terms of Service](https://docs.near-intents.org/security-compliance/terms-of-service)

| 항목 | 내용 |
|------|------|
| 운영 법인 | Defuse Labs Limited |
| 법인 설립지 | Gibraltar |
| 서비스명 | 1Click Swap API (1CS) |
| 브랜드 도메인 | `1click.chaindefuser.com`, `near-intents.org` |
| 관할 법원 | Gibraltar 법원 (분쟁 시 준거법) |
| 파트너 포털 | [https://partners.near-intents.org/home](https://partners.near-intents.org/home) |

Terms of Service는 Defuse Labs가 서비스를 단독으로 제어함을 명시한다:

> "We may suspend or modify 1CS (in whole or part) or terminate your access at any time for operational, security, legal, or sanctions-compliance reasons."

— [1Click Terms of Service](https://docs.near-intents.org/security-compliance/terms-of-service)

#### NEAR Foundation과의 관계

문서에 NEAR Foundation의 공식 지분·거버넌스 참여가 명시되어 있지 않다. NEAR Intents 프로토콜의 기반 인프라(`intents.near` 스마트 컨트랙트)는 NEAR 블록체인 위에 배포되어 있으나, 1Click Swap API 서비스 자체의 운영 주체는 Defuse Labs Limited다. 공식 문서는 두 주체를 구분 없이 함께 사용하므로 경계가 불명확하다.

#### 거버넌스 및 책임 한계

- **감사(audit):** NEAR Intents 스마트 컨트랙트는 독립 보안 감사를 받았다고 명시되어 있으나, 감사 보고서는 Google Drive 링크로만 제공 ([Security page](https://docs.near-intents.org/security-compliance/security)).
- **오픈소스:** API 서비스 코드의 오픈소스 여부에 대한 언급 없음. SDK(`@defuse-protocol/one-click-sdk-typescript`)는 npm에 공개.
- **책임 제한:** Terms of Service에 따르면 Defuse Labs의 최대 배상 책임은 **USD 100**이다 (무보증 "AS IS" 제공).
- **AML 스크리닝:** 모든 quote 요청이 NEAR Intents AML Portal, Binance AML, AMLBot & PureFi, TRM Labs에 의해 자동 스크리닝된다 ([Risk & Compliance](https://docs.near-intents.org/security-compliance/risk-and-compliance)).
- **법 집행 협조 창구:** 당국의 법적 요청은 `https://app.kodexglobal.com/nearintents/signin` 채널을 통해 처리된다 — 즉, 1Click은 법 집행 기관의 조회 요청에 응답하는 구조를 갖추고 있다.

---

### 아키텍처 (Architecture)

#### 전체 흐름

```
사용자/앱 (PAL)
│
│  POST /v0/quote  (originAsset, amount, recipient, refundTo, ...)
▼
1Click Swap API (https://1click.chaindefuser.com)
│  ← { depositAddress, quote: { amountIn, deadline, ... } }
│
│  [사용자가 depositAddress로 원본 자산 송금]
│
▼
Market Makers (Solvers) — 경쟁적 bid
│  Message Bus를 통한 intent broadcast
│  Best-price solver 선정
│
▼
intents.near (NEAR Protocol 스마트 컨트랙트)
│  On-chain settlement (atomic execution)
│  내부 ledger: 참여자별 토큰 잔고 추적
│
▼
Token Bridge
│  목적 체인으로 자산 출금 (예: USDC → Base mainnet)
│
▼
recipient 주소 (PAL의 경우: swapWallet EVM 주소)
```

#### 주요 구성 요소

**Market Makers (Solvers)**

- 사용자의 swap intent를 이행하는 유동성 공급자.
- Message Bus를 통해 경쟁적으로 quote를 제출하며, 최선 가격을 제출한 Market Maker가 선정된다 ([Market Makers docs](https://docs.near-intents.org/integration/market-makers)).
- 공식 문서는 허가(permissioned) 여부를 명시하지 않는다; 경쟁적 입찰 구조로 설명된다.
- 선정 기준: "The Message Bus collects responses and returns the top quotes to the user application."

**Settlement Contract (`intents.near`)**

- NEAR Protocol mainnet 배포 ([Treasury Addresses](https://docs.near-intents.org/security-compliance/treasury-addresses)).
- 내부 ledger 방식: 실제 토큰은 `intents.near`가 보관하고, 스왑/전송은 내부 잔고 기록 변경으로 처리.
- 원자적(atomic) 실행: "Transactions either complete with all conditions met, or they are reverted and funds returned to the refund address." ([What Are Intents](https://docs.near-intents.org/getting-started/what-are-intents))
- 토큰 출금 시만 token bridge를 통해 실제 이동.

**Token Bridges**

- NEAR Intents와 외부 체인 간 자산 이동 담당.
- EVM 통합 Treasury 주소: `0x2CfF890f0378a11913B6129B2E97417a2c302680`.
- Zcash용 별도 treasury address 존재 ([Treasury Addresses](https://docs.near-intents.org/security-compliance/treasury-addresses)).

**1Click Swap Agent**

- 사용자로부터 원본 자산을 일시 수탁하는 "trusted swapping agent".
- Market Maker 네트워크와의 조율 담당.
- swap 실패 시 자동 환불 처리.

**NEAR 체인의 역할**

NEAR Protocol은 settlement 레이어로 기능한다. 크로스체인 스왑은 NEAR 위에서 원자적으로 기록·정산되며, 실제 자산의 체인 간 이동은 token bridge를 통해 이루어진다. NEAR가 에스크로 역할을 한다고 볼 수 있으나, 실질적 자산 보관은 `intents.near` 컨트랙트가 수행한다.

---

### API 표면 (API surface)

Base URL: `https://1click.chaindefuser.com`

| Endpoint | Method | Purpose | PAL이 사용? | 비고 |
|----------|--------|---------|------------|------|
| `/v0/quote` | POST | Swap quote 요청 + deposit address 발급 | **Yes** (§1.5 `lib/oneClick.ts:102`) | originAsset, destinationAsset, amount, recipient, refundTo 등 포함. 응답에 `depositAddress` 포함. |
| `/v0/status` | GET | Swap 실행 상태 조회 (`?depositAddress=...`) | **Yes** (SDK `getExecutionStatus()`) | 응답 최상위 필드: `status` (enum). PAL은 `.status \|\| .executionStatus \|\| .state` 순 탐색 — 하지만 공식 응답은 `.status` 단일 필드. |
| `/v0/deposit/submit` | POST | 사용자 deposit tx hash 제출 | **Yes** (SDK `submitDepositTx()`) | 선택적. swap 처리 가속화 목적. 필드: `txHash`, `depositAddress`. |
| `/v0/tokens` | GET | 지원 토큰 목록 조회 | **Dead code** (`lib/oneClick.ts:17`에 구현, 호출 안 됨) | 각 토큰: `assetId`, `decimals`, `blockchain`, `symbol`, `price`. |
| `/v0/any-input/withdrawals` | GET | ANY_INPUT 방식 출금 내역 조회 | No | `depositAddress` query param 필수. 페이지네이션 지원. |
| 페이지네이션 트랜잭션 조회 | GET | 트랜잭션 히스토리 (Explorer API) | No | - |

**`getExecutionStatus` 응답 필드 (§3.1 deferred claim 해결)**

공식 API 스펙 ([Check Swap Execution Status](https://docs.near-intents.org/api-reference/oneclick/check-swap-execution-status))에 따르면 응답 최상위 필드는 **`.status`** 단일 키다:

```
{
  correlationId: string,
  quoteResponse: QuoteResponse,
  status: "KNOWN_DEPOSIT_TX" | "PENDING_DEPOSIT" | "INCOMPLETE_DEPOSIT"
        | "PROCESSING" | "SUCCESS" | "REFUNDED" | "FAILED",
  updatedAt: date-time,
  swapDetails: SwapDetails
}
```

PAL 코드(`cronjob-check-deposits/route.ts:37-40`)의 `.status || .executionStatus || .state` 삼중 탐색은 SDK 버전 불안정에 대한 방어 코드다. 현 API 스펙 기준으로는 `.status`만 존재한다. SDK 문서 예시(`status.status`)도 동일하게 `.status` 필드임을 확인:

```typescript
// 공식 SDK 예시 (docs.near-intents.org/integration/distribution-channels/1click-api/sdk)
const status = await OneClickService.getExecutionStatus(quote.quote.depositAddress!);
console.log(status.status);
```

**인증 (JWT)**

- JWT 없이: 0.2% 플랫폼 수수료 부과 (PAL의 README는 "0.1%"라고 하지만, 공식 문서 기준은 **0.2%**).
- JWT 있이: 기본 프로토콜 수수료 0.0001%(1 pip)만 부과.
- JWT 발급: [파트너 대시보드](https://partners.near-intents.org/home).
- 헤더: `Authorization: Bearer <JWT>`.

---

### Zcash 지원의 실제 (Critical finding)

> **핵심 판정: 1Click은 Zcash의 투명 주소(transparent address)만 지원한다. 실드(shielded) z-address는 지원되지 않는다.**

#### 공식 문서의 명시적 진술

[Chain Support 문서](https://docs.near-intents.org/resources/chain-support) 원문:

> **"⚠️ Partially supported - Transparent addresses only"**
>
> **Address Types: Transparent — `t1` or `t3` prefix**
> - Example: `t1ZCashExample...`

shielded 주소(`zs1...`, Sapling unified address) 또는 Orchard 주소에 대한 언급은 전혀 없다. "Partially supported"라는 표현이 명시되어 있으며, 지원 범위가 transparent address(`t1`/`t3` prefix)로 한정된다.

#### PAL 코드에서 관찰되는 동작

PAL은 `/v0/quote` 응답의 `depositAddress` 필드를 주소 형식 검증 없이 그대로 사용한다(`app/api/relayer/register-deposit/route.ts:66`). 따라서:

- 1Click이 `t1...` 또는 `t3...` (transparent) 주소를 반환하면, 사용자는 해당 transparent 주소로 ZEC를 송금한다.
- PAL의 README가 "Zcash shielded transactions"를 주장하는 것과 달리, 실제 deposit address는 **투명 주소**다.
- 공식 문서에 shielded 지원이 없으므로, Zcash L1에서도 sender와 amount가 공개된다.

이에 대해 공식 문서에 별도 설명이 없으며, PAL 코드베이스에도 주소 형식 검증 로직이 없다. 관찰된 동작(1Click 응답의 `depositAddress` pass-through)과 공식 문서(transparent-only)를 종합한 결론이다.

#### Zcash 자산 ID

1Click에서 ZEC는 NEAR Intents 래핑 토큰으로 표현된다:

```
nep141:zec.omft.near
```

([§1.5 참조](./05-one-click-bridge.md), `lib/oneClick.ts:178`)

1Click이 ZEC를 실제로 수신·처리하는 방식은 블랙박스이나, transparent address 기반이므로 표준 Zcash RPC 또는 Zebra 노드가 잔고를 모니터링하는 것으로 추정된다.

#### 프라이버시 함의

> **결론: PAL의 "Zcash shielded transactions hide amounts, sender, and recipient" 주장은 기술적으로 허위다. 이중 의미에서 프라이버시가 성립하지 않는다.**

**첫 번째 실패 — L1 레벨 (transparent address):**
1Click이 transparent address(`t1`/`t3`)를 발급하므로, Zcash L1 블록체인에서 해당 주소로의 모든 트랜잭션이 공개적으로 관찰 가능하다. ZEC 송신자(from), 수신 주소(to), 금액(amount)이 모두 블록체인 익스플로러에 노출된다. Zcash의 shielded pool이 제공하는 발신자/수신자/금액 은닉 특성이 전혀 적용되지 않는다.

**두 번째 실패 — API 레벨 (1Click visibility):**
§1.5에서 확인했듯이, PAL이 `/v0/quote`를 호출할 때 동일한 요청에 다음 정보가 모두 포함된다:

```typescript
// lib/oneClick.ts:86, 88
refundTo: senderAddress,       // 송신자 ZEC 주소
recipient: swapWallet,          // 최종 USDC 수신 EVM 주소
```

1Click(Defuse Labs Limited)은 (ZEC 송신자 주소, ZEC 금액, 목적지 EVM 주소, 목적지 체인)를 단일 API 요청 안에서 명시적으로 수신한다. Unlinkability(송신자-수신자 비연결성)는 기술적으로 전혀 없다.

**AML 스크리닝과의 상호작용:**
1Click은 "모든 quote 요청"이 자동 AML 스크리닝을 거친다고 명시한다. 즉, 사용자의 ZEC 주소와 목적지 EVM 주소가 외부 AML 데이터베이스와 대조된다. 법 집행 기관의 요청이 있으면 Kodex Global 포털을 통해 정보가 공개될 수 있다.

---

### Trust assumption (신뢰 가정)

PAL이 1Click에 의존함으로써 상속하는 신뢰 가정:

#### 1. 운영 가용성 — Solver 네트워크 의존성

| 시나리오 | PAL 결과 |
|----------|---------|
| `/v0/quote` 타임아웃/오류 | deposit 등록 불가; 사용자 QR 코드 미발급 |
| solver 네트워크 유동성 부재 | quote 불가; swap 미실행; ZEC 미수신 |
| `getExecutionStatus` 영구 PENDING | x402 미실행; 사용자는 ZEC만 납부, content 미수신 |
| 1Click 서비스 전면 종료 | PAL의 Zcash swap 기능 전체 마비 |

#### 2. 자산 보관 리스크 — Custody

ZEC를 deposit address로 송금한 순간부터 1Click(Defuse Labs)의 solver 지갑이 자산을 보관한다. PAL은 해당 주소의 spending key가 없고, refundTo 파라미터만 설정할 수 있다. 1Click이 부도·해킹·서비스 종료될 경우 ZEC 회수는 1Click의 선의(善意)에 달려 있다.

#### 3. 데이터 노출 및 법적 강제 공개

1Click에 제출되는 정보:
- 송신자 ZEC 주소 (`refundTo` 파라미터)
- ZEC 금액 (EXACT_OUTPUT 계산 결과)
- 목적지 EVM 주소 (`recipient` 파라미터)
- 목적지 체인 (Base 또는 Solana)
- 서비스 식별자 (`referral: 'anyone-pay'`)
- ZEC on-chain tx hash (선택적 `submitDepositTx` 호출 시)

이 데이터는 Gibraltar 준거법에 따라 Kodex Global 포털을 통한 법 집행 요청 시 공개될 수 있다. 사용자가 Zcash shielded pool에서 swap을 시작하더라도, 1Click API 레벨에서의 연결고리가 명시적으로 노출된다.

#### 4. 가격 및 수수료 리스크

1Click이 swap 환율을 결정한다(EXACT_OUTPUT이지만 input ZEC 양은 1Click이 계산). PAL은 `slippageTolerance: 100` (1%)을 설정하지만, 실제 환율의 공정성은 Market Maker 경쟁에 의존한다. 수수료: JWT 없이 0.2% 추가 수수료 부과.

#### 5. `SUCCESS` 보고 신뢰 — 독립 검증 없음

```typescript
// app/api/relayer/cronjob-check-deposits/route.ts:47
if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed) {
  // ← 체인 독립 검증 없이 1Click의 SUCCESS를 믿고 즉시 x402 실행
```

PAL은 1Click이 `SUCCESS`를 반환하면 즉시 x402 결제를 실행한다. `swapWallet`에 실제로 USDC가 입금되었는지 독립적으로 확인하지 않는다. 1Click API가 침해되거나 허위 `SUCCESS`를 반환하면 ZEC 미수신 상태에서 USDC x402가 실행된다.

---

### 회의록 / week2 비교

week2 ★★★ 평가는 Pay Anyone Legend를 다음과 같이 특징지었다:

> "Pay Anyone Legend = x402 결제 이전에 ZEC가 USDC로 변환되는 **funding 단계** (외부 swap에 위임). Zcash 측 구현은 얕다 (1Click API에 위임, Z-address 생성도 mock)."

Task 10 분석이 이 평가를 다음과 같이 구체화하고 일부 수정한다:

| week2 항목 | Task 10 결과 |
|-----------|-------------|
| "외부 swap에 위임" | **확인·정확.** 외부 서비스는 구체적으로 **Defuse Labs Limited(Gibraltar 법인)**가 운영하는 1Click Swap API. |
| "Zcash 측 구현 얕다" | **확인·확장.** 단순히 얕은 것이 아니라, Zcash 관련 네이티브 라이브러리가 단 하나도 없다. deposit address조차 1Click API 응답을 pass-through한 것. |
| "Z-address 생성도 mock" | **수정 필요.** `crypto.getRandomValues + zs1 prefix` mock 패턴은 존재하지 않는다. 실제 메커니즘은 1Click API에 완전 위임(Category C: Outsourced). 테스트 스크립트의 `zs1test123` 리터럴이 "mock"처럼 보였을 수 있으나 앱 런타임과 무관. |
| (미언급) | **신규 발견 — 가장 중요:** 1Click이 발급하는 deposit address는 **Zcash 투명 주소(transparent, t1/t3)** 뿐이다. PAL의 "Zcash shielded" 마케팅은 L1에서도, API 레벨에서도 성립하지 않는다. |
| (미언급) | **신규 발견:** 1Click은 (ZEC 송신자, ZEC 금액, 목적지 EVM 주소)를 동일 요청에서 명시적으로 수집하며, AML 스크리닝 및 법 집행 공개 채널을 운영한다. 프라이버시는 **기술적 보장이 아니라 Defuse Labs에 대한 법적 신뢰**에만 의존한다. |

**team 함의:** Category E ("x402 + Zcash") 접근에서 PAL을 참조할 때, "Zcash shielded swap을 외부 위임"이 아니라 "투명 주소 기반 ZEC → USDC 스왑을 Gibraltar 법인에 위임"임을 명확히 해야 한다. 진정한 Category E(실드 Zcash가 x402 settlement에 참여)를 구현하려면 PAL에서 재사용할 수 있는 Zcash 코드가 전혀 없으며, lightwalletd/Zebra RPC 연동, Sapling/Orchard key derivation, PCZT 기반 tx 생성을 처음부터 구현해야 한다.

---

### 인용 URL 목록 (Citation index)

| # | URL | 내용 |
|---|-----|------|
| 1 | [https://docs.near-intents.org/api-reference/oneclick/request-a-swap-quote](https://docs.near-intents.org/api-reference/oneclick/request-a-swap-quote) | `/v0/quote` 엔드포인트 전체 스펙; `depositAddress` 응답 필드 |
| 2 | [https://docs.near-intents.org/api-reference/oneclick/check-swap-execution-status](https://docs.near-intents.org/api-reference/oneclick/check-swap-execution-status) | `/v0/status` 응답 스펙; `.status` 필드 확인; 상태값 열거 |
| 3 | [https://docs.near-intents.org/api-reference/oneclick/submit-deposit-transaction-hash](https://docs.near-intents.org/api-reference/oneclick/submit-deposit-transaction-hash) | `/v0/deposit/submit` 스펙; `txHash` + `depositAddress` 필드 |
| 4 | [https://docs.near-intents.org/api-reference/oneclick/get-supported-tokens](https://docs.near-intents.org/api-reference/oneclick/get-supported-tokens) | `/v0/tokens` 스펙; `blockchain: "zec"` 열거 |
| 5 | [https://docs.near-intents.org/security-compliance/terms-of-service](https://docs.near-intents.org/security-compliance/terms-of-service) | 운영 주체: Defuse Labs Limited (Gibraltar); 거버넌스; 책임 한계 USD 100 |
| 6 | [https://docs.near-intents.org/resources/chain-support](https://docs.near-intents.org/resources/chain-support) | **Zcash: "⚠️ Partially supported - Transparent addresses only"**; t1/t3 prefix |
| 7 | [https://docs.near-intents.org/resources/fees](https://docs.near-intents.org/resources/fees) | 수수료 구조: JWT 없이 0.2% 플랫폼 수수료; ZEC 출금 0.1% 수수료 |
| 8 | [https://docs.near-intents.org/security-compliance/treasury-addresses](https://docs.near-intents.org/security-compliance/treasury-addresses) | `intents.near` settlement 컨트랙트; Zcash treasury address 존재 |
| 9 | [https://docs.near-intents.org/security-compliance/risk-and-compliance](https://docs.near-intents.org/security-compliance/risk-and-compliance) | AML 스크리닝; Kodex Global 법 집행 포털; TRM Labs 사용 |
| 10 | [https://docs.near-intents.org/getting-started/what-are-intents](https://docs.near-intents.org/getting-started/what-are-intents) | NEAR Intents 아키텍처; Market Maker 경쟁; atomic settlement |
| 11 | [https://docs.near-intents.org/integration/distribution-channels/1click-api/about-1click-api](https://docs.near-intents.org/integration/distribution-channels/1click-api/about-1click-api) | 1Click Swap API 정의; "trusted swapping agent"; Market Makers |
| 12 | [https://docs.near-intents.org/integration/distribution-channels/1click-api/authentication](https://docs.near-intents.org/integration/distribution-channels/1click-api/authentication) | JWT 인증; Partner Dashboard; 0.2% 수수료 면제 |
| 13 | [https://docs.near-intents.org/integration/distribution-channels/1click-api/fee-config](https://docs.near-intents.org/integration/distribution-channels/1click-api/fee-config) | `appFees` 파라미터; 50/50 revenue share |
| 14 | [https://docs.near-intents.org/integration/distribution-channels/1click-api/sdk](https://docs.near-intents.org/integration/distribution-channels/1click-api/sdk) | TypeScript SDK; `status.status` 필드; `submitDepositTx` 시그니처 |
| 15 | [https://docs.near-intents.org/integration/market-makers](https://docs.near-intents.org/integration/market-makers) | Market Maker 역할; Message Bus; solver 선정 메커니즘 |

---

## 3.2 How shielded tx execution is outsourced to 1Click

(filled at Task 12)

## 3.3 How z-address generation is faked

(filled at Task 12)

## 3.4 Inventory + what they should have used

(filled at Task 12)
