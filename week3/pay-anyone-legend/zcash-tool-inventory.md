# §3 Zcash dev-tool inventory + outsourcing story

## 3.1 What 1Click actually is

> **크로스 레퍼런스:** 이 섹션은 1Click 프로토콜 자체를 다룬다. PAL이 1Click을 구체적으로 어떻게 호출하는지는 [§1.5 1Click bridge](./subsystems/05-one-click-bridge.md)를 참조하라.

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

([§1.5 참조](./subsystems/05-one-click-bridge.md), `lib/oneClick.ts:178`)

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

## 3.2 어떻게 shielded tx 실행을 1Click에 outsource하는가

> **크로스 레퍼런스:** 이 섹션의 API 세부 사항은 [§1.5 1Click bridge](./subsystems/05-one-click-bridge.md)에서 비롯된다. 호출 시퀀스의 각 스텝에 file:line 인용이 명시된다.

---

### 호출 시퀀스 (Call sequence)

PAL이 1Click에 보내는 API 호출은 다음 세 단계로 요약된다. 각 단계는 이 시퀀스가 PAL의 어느 코드에서 시작되는지를 file:line으로 표시한다. 전체 happy path의 더 긴 버전은 [§1.5 walkthrough](./subsystems/05-one-click-bridge.md)를 참조하라.

1. **`POST /v0/quote` — ZEC→USDC swap quote 요청 + Zcash deposit address 발급**

   - **Caller:** `lib/oneClick.ts:102`, `app/api/relayer/register-deposit/route.ts:55`
   - **Payload (요청):**
     ```typescript
     // lib/oneClick.ts:78–132
     {
       dry: false,
       swapType: 'EXACT_OUTPUT',
       slippageTolerance: 100,              // 1%
       originAsset: 'nep141:zec.omft.near', // ASSETS.ZCASH
       depositType: 'ORIGIN_CHAIN',
       destinationAsset: 'nep141:base-0x833589fcd6edb6e08f4c7c32d4f71b54bda02913.omft.near', // USDC_BASE
       amount: usdcToSmallestUnit(amount),  // USDC 최소 단위
       refundTo: senderAddress,             // 송신자 ZEC 주소 ← 핵심 프라이버시 노출 지점
       refundType: 'ORIGIN_CHAIN',
       recipient: swapWallet,               // NEAR Chain Sig 파생 EVM 주소 ← 최종 USDC 수신
       recipientType: 'DESTINATION_CHAIN',
       deadline: new Date(Date.now() + 3 * 60 * 1000).toISOString(), // 3분 후
       referral: 'anyone-pay',
     }
     ```
   - **Response (응답):** `{ depositAddress: "t1...", swapId: "...", sessionId: "...", quote: { amountInFormatted, deadline, ... } }`
   - **핵심:** `depositAddress`는 1Click solver 네트워크가 소유·제어하는 **Zcash transparent address(t1/t3)**다. PAL이 생성한 것이 아니다 (`lib/oneClick.ts:126`).

2. **SDK `OneClickService.submitDepositTx({txHash, depositAddress})` — ZEC on-chain tx hash 제출 (선택적)**

   - **Caller:** `lib/oneClick.ts:155`, `app/api/relayer/submit-tx-hash/route.ts:34`
   - **Payload:** `{ txHash: <사용자 ZEC 송금 tx hash>, depositAddress: <위 step 1 주소> }`
   - **목적:** swap 처리를 1Click solver가 앞당겨 실행할 수 있도록 힌트 제공. 필수가 아니며, 미제출 시에도 1Click이 on-chain 모니터링으로 입금을 감지한다.

3. **SDK `OneClickService.getExecutionStatus(depositAddress)` — swap 진행 상태 폴링**

   - **Caller:** `lib/oneClick.ts:141`, `app/api/relayer/cronjob-check-deposits/route.ts:34`, `app/api/relayer/check-deposit/route.ts:20`, `app/api/content/get-url/route.ts:69`
   - **Payload:** `depositAddress` 문자열 (1Click 내부적으로 이것이 order ID)
   - **Response:** `{ status: "PENDING_DEPOSIT" | "PROCESSING" | "SUCCESS" | "INCOMPLETE_DEPOSIT" | "REFUNDED" | "FAILED", ... }`
   - **PAL의 응답 탐색 로직:** `(statusResponse as any).status || .executionStatus || .state` 삼중 탐색 (`cronjob-check-deposits/route.ts:37–40`) — SDK 버전 불안정에 대한 방어 코드.
   - **트리거 조건:** `normalizedStatus === 'SUCCESS'` 시에만 PAL이 x402 결제를 실행한다 (`cronjob-check-deposits/route.ts:47`). 이 SUCCESS 판정에 대한 독립 on-chain 검증은 **없다** (§3.2 아래 참조).

---

### 핵심 outsourcing 책임 분담 (Responsibility split)

| 책임 | 1Click이 담당 | PAL이 담당 |
|------|---------------|-----------|
| ZEC 입금 주소 생성 | ✓ (transparent t-addr; [§3.1](#31-what-1click-actually-is), `lib/oneClick.ts:126`) | ✗ |
| ZEC 입금 모니터링 | ✓ (`getExecutionStatus` — solver 내부 on-chain 감시) | ✗ (단순 polling만: `cronjob-check-deposits/route.ts:34`) |
| ZEC custody during swap | ✓ (Defuse Labs Limited, solver 지갑 spending key 보유) | ✗ |
| ZEC → USDC 환전 | ✓ (solver network; `intents.near` atomic settlement) | ✗ |
| USDC를 swapWallet로 전달 | ✓ (token bridge → `recipient` EVM 주소) | ✗ |
| swapWallet에서 최종 recipient로 transfer | ✗ | ✓ (NEAR Chain Signatures + EIP-3009 `transferWithAuthorization`; `lib/chainSig.ts:210–401`, [§1.6](./subsystems/06-near-chain-signatures.md)) |
| 영수증 / 검증 | ✗ (독립 검증 없음) | ✓ (Supabase `signed_payload` 컬럼에 Ethereum tx hash 저장; `cronjob-check-deposits/route.ts:135`) |
| 환불 / 분쟁 처리 | ✗ (Defuse Labs 운영자 재량; Terms of Service 최대 배상 USD 100) | ✗ (구현 안 됨; `POST /api/relayer/refund` 존재하지 않음; [§1.4](./subsystems/04-deposit-tracking.md)) |

---

### "PAL은 ZEC 트랜잭션을 한 줄도 만들지 않는다"

이 점은 구체적으로 강조할 가치가 있다. PAL 코드베이스 전체에는 Zcash 트랜잭션을 생성(construct), 서명(sign), 브로드캐스트(broadcast), 심지어 직렬화(serialize)하는 코드가 단 한 줄도 없다. ZEC를 custody하는 주소의 spending key는 PAL 서버 어느 환경 변수에도 저장되지 않는다 (`lib/chainSig.ts:17–39` — 저장된 것은 NEAR proxy key뿐). PAL이 Zcash와 맺는 유일한 접점은 세 가지 REST 호출(`POST /v0/quote`, SDK `submitDepositTx`, SDK `getExecutionStatus`)뿐이며, 이 호출들은 모두 1Click API(Defuse Labs Limited)로 향한다. Zcash 체인을 직접 읽거나 쓰는 코드는 없다 — lightwalletd/Zebra RPC 호출 없음, Zcash RPC JSON-RPC 호출 없음, on-chain tx hash를 Zcash explorer에서 검증하는 코드 없음 ([§1.3](./subsystems/03-z-address-generation.md), [§1.5](./subsystems/05-one-click-bridge.md) 전반 확인).

---

### "외부 swap에 위임"의 정확한 정체

week2 ★★★ 문서는 PAL의 Zcash 통합을 **"외부 swap에 위임"**으로 표현했다. Task 10–12의 분석은 이 표현을 다음과 같이 구체화하고 일부 수정한다.

"외부 swap"은 단순한 탈중앙 DEX 프로토콜이 아니다. 그것은 **Defuse Labs Limited(Gibraltar 법인)**가 단독 운영하는 **1Click Swap API**이며, 내부적으로는 NEAR Protocol 위의 `intents.near` 스마트 컨트랙트와 Message Bus 기반 solver 네트워크가 실행한다 ([§3.1](#31-what-1click-actually-is)). 이 solver 네트워크는 PAL의 x402 intent와는 완전히 별개인 NEAR Intents 생태계의 swap intent 메커니즘으로 동작하며 — 즉 "x402-orthogonal"하다. 결정적으로, 1Click API는 모든 quote 요청을 AML 스크리닝(NEAR Intents AML Portal, Binance AML, AMLBot & PureFi, TRM Labs)에 자동으로 통과시키며, 법 집행 기관의 요청이 있을 경우 Kodex Global 포털을 통해 거래 정보를 공개한다 ([§3.1 Trust assumption](#trust-assumption-신뢰-가정)).

따라서 week2의 "외부 swap에 위임"이라는 표현의 정확한 내용은 다음과 같다:

> **PAL은 Zcash transparent-address 기반 ZEC → USDC swap을 Gibraltar 법인(Defuse Labs Limited)의 중앙화 API에 위임하고 있으며, 이 API는 전체 거래 linkage(ZEC 송신자 주소 + ZEC 금액 + 목적지 EVM 주소)를 수신하고 AML 스크리닝 및 법 집행 공개 체계를 갖추고 있다.**

Category E 팀이 이를 참조할 때 "외부 swap 위임"을 단순한 아키텍처 패턴이 아니라, 실제 법인·신뢰·프라이버시 모델의 선택으로 이해해야 한다.

---

## 3.3 z-address generation을 어떻게 mock으로 처리하는가

> **크로스 레퍼런스:** 이 섹션의 코드 증거는 [§1.3 Z-address generation](./subsystems/03-z-address-generation.md)에서 비롯된다. 판정 (C)는 이 섹션이 최종 확정한다.

---

### Verdict 재진술

**판정 (C) — Outsourced (완전 위임):** PAL은 Zcash deposit address를 자체 생성하지 않는다. deposit address 전체가 1Click API의 `/v0/quote` 응답에 포함된 `depositAddress` 필드에서 온다 (`lib/oneClick.ts:126`, `app/api/relayer/register-deposit/route.ts:66`). PAL 측에서 Zcash 주소를 생성하거나 파생하는 코드는 단 한 줄도 없다.

---

### "랜덤 zs1 prefix" 가설 검증

week2 ★★★ 분석 문서는 PAL의 Zcash 구현을 다음과 같이 특징지었다:

> "Z-address 생성도 mock... Zcash 측 구현은 얕다"

이 표현은 (A) 실제 Sapling/Orchard 키 파생, (B) `crypto.getRandomValues`로 랜덤 바이트를 생성한 후 `'zs1'` prefix를 붙이는 합성 mock, (C) 외부 API에 완전 위임 중 어느 하나임을 시사한다. 주석이나 리뷰 컨텍스트로 볼 때 week2 시점에서는 (B) 패턴이 실제일 가능성을 주요 가설로 취급했다.

**실제 코드 분석 결과, 이 가설은 명시적으로 반증된다.**

`zs1` 문자열은 이 코드베이스에 **두 곳**에만 등장한다:

| 파일:라인 | 내용 |
|-----------|------|
| `contract/deploy.sh:54` | `"deposit_address\":\"zs1test123\"` |
| `contract/test-contract.sh:14` | `"deposit_address\":\"zs1test123456789\"` |

두 곳 모두 NEAR 컨트랙트의 `create_intent()` 메서드를 CLI로 테스트하기 위한 **shell script 내 하드코딩 literal**이다 — 앱 런타임이 생성하는 값이 아니며, Next.js 서버 코드가 이 값을 읽거나 사용하는 경로가 전혀 없다. `rg -n "getRandomValues" --type ts --type tsx` 검색은 Zcash 주소 생성 목적의 호출을 반환하지 않는다 — `lib/kdf.ts`에는 `crypto.subtle.digest`(SHA-256)가 있지만 이는 Bitcoin address derivation용이다 ([§1.6](./subsystems/06-near-chain-signatures.md), `lib/kdf.ts:82–107`).

실제 앱에서 deposit address는 오직 1Click REST API가 반환한다:

```typescript
// lib/oneClick.ts:122–128 — PAL의 "주소 생성" 코드 전부
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

이 두 구문이 PAL의 "z-address generation" 코드 전체다. week2의 "얕다(shallow)"는 평가는 정확하지만, 그 구체적 메커니즘은 (B)가 아닌 **(C) — 완전 API 위임**이다.

---

### 어떻게 시스템이 작동하는가

실제 Zcash 주소 파생 코드가 없음에도 불구하고 사용자 플로우가 동작하는 이유는 **1Click solver 네트워크가 실제 유효한 Zcash transparent address를 반환하기 때문**이다. 사용자 플로우를 추적하면:

1. **사용자가 인텐트 제출** — "Pay OnlyFans $10" 같은 자연어가 `lib/nearAI.ts:43–44`에서 `{ amount: "10", currency: "USDC", chain: "base", bridgeFrom: "zcash" }`로 파싱된다 ([§1.1](./subsystems/01-intent-parser.md)).

2. **`POST /api/relayer/register-deposit` 호출** — `app/page.tsx:499`의 `generateDepositAddress()`가 서버 route를 호출한다.

3. **1Click `/v0/quote` 응답에서 `depositAddress` 수신** — `lib/oneClick.ts:126`이 `data.depositAddress`를 추출한다. 이것이 1Click solver 인프라가 제어하는 실제 Zcash **transparent address**(`t1...` 또는 `t3...` prefix)다. 1Click 공식 문서에 따르면 Zcash는 "Partially supported - Transparent addresses only"이다 ([§3.1 Zcash 지원의 실제](#zcash-지원의-실제-critical-finding), `docs.near-intents.org/resources/chain-support`).

4. **Supabase `deposit_tracking` 저장** — `lib/depositTracking.ts:104`가 `depositAddress`를 primary key로 저장한다. 이 주소는 동시에 **1Click 내부의 swap order ID**로도 기능한다 — `OneClickService.getExecutionStatus(depositAddress)` (`lib/oneClick.ts:141`)가 동일한 주소로 상태를 조회한다.

5. **QR 코드 렌더링** — `components/IntentsQR.tsx:186`이 `<QRCodeSVG value={depositAddress} size={220} level="H" />`로 렌더링한다. **ZIP-321 URI 형식(`zcash:t1...?amount=...`)은 사용되지 않는다** — 주소 문자열 자체만 인코딩된다.

요컨대 "deposit 식별 handle"은 `depositAddress` 문자열 자체가 담당한다. 이 주소가 동시에 (1) 사용자가 ZEC를 보낼 수신 주소, (2) 1Click swap order를 추적하는 key, (3) Supabase row의 PK 세 가지 역할을 한다.

---

### 사용자가 일반 Zcash 지갑으로 보내면?

1Click이 반환하는 `depositAddress`는 실제 유효한 Zcash **transparent address**(`t1`/`t3`)이므로, 일반 Zcash 지갑 사용자가 이 주소로 ZEC를 보내는 것은 기술적으로 가능하다. 그러나 다음과 같은 제한이 있다:

| 시나리오 | 결과 |
|----------|------|
| 일반 Zcash 지갑에서 transparent send (t-addr → t-addr) | ✓ 가능 — t-addr은 유효한 on-chain 주소 |
| Zcash shielded send (z-addr → 이 t-addr) | ✗ 불가능 — 1Click이 shielded deposits를 거부한다 ([§3.1 Zcash 지원의 실제](#zcash-지원의-실제-critical-finding)) |
| 메모(memo) 포함 전송 | ✗ 무의미 — 1Click은 memo를 처리하지 않는다. Zcash transparent send는 표준적으로 memo를 지원하지 않는다 (memo는 shielded tx의 기능). ZIP-321 URI도 생성되지 않으므로 amount hint도 없다 |
| Zcash L1에서의 프라이버시 | ✗ 완전 노출 — transparent send이므로 ZEC 송신자 t-addr, 수신 t-addr(`depositAddress`), 금액이 Zcash 블록체인 익스플로러에 공개적으로 관찰 가능하다. Zcash shielded pool의 익명성이 전혀 적용되지 않는다 |

**프라이버시 주장 붕괴:** PAL의 README는 "Zcash shielded transactions hide amounts, sender, and recipient"를 주장한다. 실제 코드와 1Click 공식 문서를 종합하면, 이 주장은 **L1 레벨에서도, 1Click API 레벨에서도** 성립하지 않는다. L1에서는 transparent address이므로 on-chain 데이터가 공개되고, API 레벨에서는 1Click이 `refundTo`(송신자 주소)와 `recipient`(수신 EVM 주소)를 동일 quote 요청에서 명시적으로 수신한다 (`lib/oneClick.ts:86, 88`; [§1.5 프라이버시 이야기](./subsystems/05-one-click-bridge.md)).

---

### Category E에 대한 함의

PAL에서 재사용할 수 있는 Zcash 암호화 코드는 **없다**. 우리 팀이 Category E(x402 + Zcash)에서 Zcash를 진정한 settlement asset으로 사용하려면 다음을 처음부터 구현해야 한다:

- **실제 Zcash 주소 파생:** ZIP-32 HD path 기반 Orchard 또는 Sapling key derivation. `bech32` NPM 패키지(`bech32@2.0.0`)는 PAL에 이미 존재하지만 Cosmos/XRP Ledger 주소에만 사용된다 (`lib/kdf.ts:163–165`) — Orchard address의 bech32 HRP(`u1` Unified Address 또는 `zs1` Sapling address) 인코딩은 별개의 구현이 필요하다.
- **Lightwalletd 또는 Zebra RPC 연동:** on-chain 입금 확인을 위한 체인 상태 조회.
- **ZIP-321 URI 생성:** QR 코드에 `zcash:zs1...?amount=0.01&memo=...` 형식의 payment URI를 포함해야 사용자의 Zcash 지갑이 amount와 memo를 자동으로 채울 수 있다.
- **Zcash shielded tx 생성 및 서명:** PCZT 또는 `zcash_client_backend` 기반 tx 구성.
- **Memo carry:** x402 quote_hash를 Zcash shielded tx의 encrypted memo 필드에 담아야 Zcash tx 자체가 결제 proof가 될 수 있다.

이 구현 항목들은 §3.4 Part B의 native Zcash dev tool 카탈로그에서 구체적인 도구와 연결된다.

---

## 3.4 Inventory + 그들이 사용했어야 할 것

### Part A: 실제로 PAL의 package.json에 있는 것

Step 12.1에서 추출한 전체 dependency 목록을 분류한다. `dependencies` 섹션의 모든 패키지가 포함된다 (`package.json` 전체 확인, 2026-05-11 기준):

| Package | Version | Classification | Used by | Note |
|---------|---------|----------------|---------|------|
| `@defuse-protocol/one-click-sdk-typescript` | `0.1.14` | **near-related** | [§1.5](./subsystems/05-one-click-bridge.md) `lib/oneClick.ts:3–4` | Zcash 관련 기능의 전담 수행자 — 완전 위임 |
| `@supabase/supabase-js` | `^2.86.0` | **infra** | [§1.2](./subsystems/02-service-registry.md), [§1.4](./subsystems/04-deposit-tracking.md) `lib/supabase.ts`, `lib/supabase-server.ts` | 서비스 레지스트리 + deposit tracking DB |
| `@types/elliptic` | `^6.4.18` | **crypto-primitive** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts` | TypeScript type declaration only; secp256k1 관련 |
| `autoprefixer` | `^10.4.16` | **infra** | CSS 빌드 | CSS 벤더 prefix 자동화 — Zcash와 무관 |
| `bech32` | `^2.0.0` | **crypto-primitive** ⚠️ | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts:163–165` | **Cosmos 주소 인코딩에만 사용 — Zcash 주소와 무관.** Orchard/Sapling HRP(`u1`, `zs1`) 인코딩에 사용할 수 있는 범용 라이브러리이지만, 이 코드베이스에서는 그렇게 사용되지 않는다 |
| `bn.js` | `^5.2.2` | **crypto-primitive** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts:3` | Big number 연산 (secp256k1 scalar) — NEAR Chain Signatures용 |
| `bs58check` | `^4.0.0` | **crypto-primitive** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts:82–107` | **Bitcoin/Dogecoin Base58Check 인코딩 — Zcash 무관** |
| `chainsig.js` | `^1.1.14` | **near-related** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/chainSig.ts:7` | NEAR Chain Signatures EVM adapter; `ChainSignatureContract`, `EVM` adapter |
| `class-variance-authority` | `^0.7.0` | **infra** | UI 컴포넌트 | Tailwind 클래스 변형 유틸리티 — Zcash와 무관 |
| `clsx` | `^2.1.0` | **infra** | UI 컴포넌트 | className 조합 유틸리티 — Zcash와 무관 |
| `dotenv` | `^16.3.1` | **infra** | 환경 변수 로드 | 표준 .env 처리 |
| `elliptic` | `^6.6.1` | **crypto-primitive** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts:1` | secp256k1 타원곡선 연산 — NEAR Chain Signatures key derivation용 |
| `ethers` | `^5.7.2` | **evm-related** | [§1.6](./subsystems/06-near-chain-signatures.md), [§1.7](./subsystems/07-x402-client.md) `lib/chainSig.ts:4` | EIP-712 hash 생성, BigNumber, ABI 인코딩, EVM 주소 체크섬 |
| `framer-motion` | `^11.0.0` | **infra** | UI 애니메이션 | 순수 UI 라이브러리 — Zcash와 무관 |
| `js-sha3` | `^0.9.3` | **crypto-primitive** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts:6` | sha3_256 해시 — NEAR MPC epsilon derivation에서 사용; Zcash와 무관 |
| `keccak` | `^3.0.4` | **crypto-primitive** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts:4` | keccak256 — EVM 주소 derivation (Ethereum 특화) |
| `lucide-react` | `^0.344.0` | **infra** | UI 아이콘 | 순수 UI 라이브러리 |
| `near-api-js` | `^0.44.2` | **near-related** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/chainSig.ts:1–3` | NEAR Account 인스턴스 생성; 구버전 (현 latest는 4.x) |
| `near-seed-phrase` | `^0.2.1` | **near-related** | (직접 호출 미확인; 아마 near-api-js 의존성) | NEAR account 시드 구문 유틸리티 |
| `next` | `^15.0.0` | **infra** | 전체 앱 프레임워크 | Next.js App Router |
| `openai` | `^6.9.1` | **infra** | [§1.1](./subsystems/01-intent-parser.md), [§1.2](./subsystems/02-service-registry.md) `lib/nearAI.ts:1`, `lib/serviceRegistry.ts:6` | gpt-4o-mini intent 파싱 + text-embedding-3-small 임베딩 |
| `postcss` | `^8.4.32` | **infra** | CSS 빌드 | Tailwind 전처리 |
| `qrcode.react` | `^3.1.0` | **infra** | [§1.3](./subsystems/03-z-address-generation.md) `components/IntentsQR.tsx:186` | deposit address QR 렌더링 |
| `react` | `^18.3.1` | **infra** | UI 전체 | React |
| `react-dom` | `^18.3.1` | **infra** | UI 전체 | React DOM |
| `react-hot-toast` | `^2.4.1` | **infra** | UI 알림 | Toast 알림 |
| `tailwindcss` | `^3.4.0` | **infra** | UI 스타일 | CSS 프레임워크 |
| `viem` | `^2.0.0` | **evm-related** | [§1.7](./subsystems/07-x402-client.md) `lib/chainSig.ts:44–47` | viem `createPublicClient` — Base mainnet RPC 클라이언트 |
| `xrpl` | `^4.4.3` | **payment-protocol** | [§1.6](./subsystems/06-near-chain-signatures.md) `lib/kdf.ts:5` | XRP Ledger 주소 파생 (`lib/kdf.ts:109–180` `xrpLedger` case) |

**총 29개 dependency 분류 완료.**

#### 핵심 관찰

- **zcash-native 패키지: 0개.** `@zec`, `zcash-wasm`, `librustzcash`, `bellman`, `zcash_client_backend`, `lightwalletd-client`, `orchard`, `sapling-crypto`, `pczt`, `neon-js/zcash` 등 Zcash 암호화 라이브러리는 단 하나도 없다 ([§1.3 라이브러리](./subsystems/03-z-address-generation.md)).

- **`bech32`는 "Zcash에 사용할 수 있었지만 사용하지 않은" 유일한 패키지다.** bech32 v2는 Zcash Sapling (`zs1`) 주소의 bech32m 인코딩과 Orchard Unified Address의 구성에 이론적으로 활용 가능하다. 실제로는 `lib/kdf.ts:163–165`에서 Cosmos 주소(`atom1...`, `osmo1...` 등)에만 사용된다.

- **실질적 "data plane"은 `@supabase/supabase-js` + `openai` + `@defuse-protocol/one-click-sdk-typescript`다.** 세 패키지가 PAL의 핵심 외부 의존성이며, 이 중 1Click SDK가 Zcash 기능 전체를 담당한다.

---

### Part B: Native Zcash dev tools that PAL didn't use

아래 카탈로그는 PAL이 "실제로 Zcash를 직접 다루었다면" 사용했어야 할 도구들이다. 각 항목에 대해 — 무엇인지, URL, 우리 팀이 Category E에서 맡길 역할 — 을 기술한다.

---

#### 1. `zcash_client_backend` (Rust crate)

- **URL:** [https://docs.rs/zcash_client_backend](https://docs.rs/zcash_client_backend/latest/zcash_client_backend/) | [GitHub: zcash/librustzcash](https://github.com/zcash/librustzcash/tree/main/zcash_client_backend)
- **무엇인가:** Zcash lightweight client 구현을 위한 핵심 Rust 라이브러리로, wallet sync, block scan, shielded transaction 복호화, balance 계산 API를 제공한다. ZF(Zcash Foundation) 및 ECC(Electric Coin Company)가 관리하는 `librustzcash` 모노레포의 일부다.
- **우리 팀의 역할:** Category E에서 shielded 입금 감지(lightwalletd와 연동해 블록 스캔), incoming viewing key로 memo 필드 복호화, Orchard/Sapling output 확인에 사용한다. 이것이 PAL의 `OneClickService.getExecutionStatus()` 호출을 실제 on-chain 검증으로 대체하는 핵심 컴포넌트다.

---

#### 2. `zcash_primitives` (Rust crate)

- **URL:** [https://docs.rs/zcash_primitives](https://docs.rs/zcash_primitives/latest/zcash_primitives/) | [GitHub: zcash/librustzcash](https://github.com/zcash/librustzcash/tree/main/zcash_primitives)
- **무엇인가:** Zcash 프로토콜의 암호화 기본 구성 요소(BLAKE2b, JubJub 타원곡선, RedJubJub 서명, Sapling note encryption, Orchard circuit 구조)와 transaction 구조체를 제공하는 Rust 라이브러리다.
- **우리 팀의 역할:** Sapling viewing key, Orchard viewing key 파생 및 note 복호화에 직접 활용한다. `zcash_client_backend`의 의존성이기도 하므로, `zcash_client_backend`를 사용하면 자동으로 활용된다. x402 quote_hash를 memo 필드에 인코딩하는 구체적 타입 정의도 이 crate에서 나온다.

---

#### 3. `librustzcash` (Rust crate 모노레포)

- **URL:** [https://github.com/zcash/librustzcash](https://github.com/zcash/librustzcash)
- **무엇인가:** ECC가 관리하는 Rust crate 집합으로, `zcash_primitives`, `zcash_client_backend`, `zcash_client_sqlite`, `zcash_keys`, `zcash_address`, `zcash_transparent`, `pczt` 등 Zcash 관련 Rust 라이브러리 전체를 포함한다. 과거에는 단일 umbrella crate로 불렸으나 현재는 다수의 독립 crate로 구성된다.
- **우리 팀의 역할:** 이 레포가 Category E Rust backend의 핵심 의존성 출발점이다. `Cargo.toml`에 `zcash_client_backend`, `zcash_primitives`, `pczt`를 추가하는 것으로 Zcash 프로토콜 스택 전체에 접근한다.

---

#### 4. `zcashd` JSON-RPC

- **URL:** [https://zcash.github.io/rpc/](https://zcash.github.io/rpc/) | [GitHub: zcash/zcash](https://github.com/zcash/zcash)
- **무엇인가:** `zcashd`(Zcash의 원조 full node 구현, Bitcoin Core 기반)가 노출하는 JSON-RPC API. `z_sendmany`, `z_getbalance`, `z_getnewaddress`, `z_listreceivedbyaddress` 등 shielded 주소 관련 메서드를 포함한다.
- **우리 팀의 역할:** PAL이 `lib/oneClick.ts:102`에서 1Click `/v0/quote`를 호출하여 deposit address를 받는 대신, `zcashd` RPC의 `z_getnewaddress`로 자체 shielded address를 생성하고 `z_listreceivedbyaddress`로 입금을 확인하는 구조로 교체할 수 있다. 단, `zcashd` 실행 비용이 높고 Zebra로의 마이그레이션 권고가 있어 실제 구현에서는 다음 항목(lightwalletd)이 더 현실적이다.

---

#### 5. `lightwalletd` (gRPC 서버)

- **URL:** [https://github.com/zcash/lightwalletd](https://github.com/zcash/lightwalletd)
- **무엇인가:** Zcash full node(zcashd 또는 Zebra)의 앞단에서 동작하는 **bandwidth-efficient gRPC 서버**. 모바일/경량 지갑이 full node 없이 Zcash 블록체인에 접근할 수 있도록 compact block 스트리밍, 트랜잭션 제출, 상태 조회 API를 제공한다. Zcash Foundation 관리.
- **우리 팀의 역할:** PAL의 `cronjob-check-deposits/route.ts:34`가 `OneClickService.getExecutionStatus()`를 폴링하는 대신, lightwalletd의 `GetBlockRange` / `GetTransaction` gRPC 엔드포인트를 구독하여 입금을 실시간으로 감지한다. `zcash_client_backend`와 함께 사용하여 compact block에서 shielded note를 스캔한다. 공개 lightwalletd 엔드포인트(`lightwalletd.electriccoin.co:9067` 등)를 사용하거나 자체 호스팅할 수 있다.

---

#### 6. `ZcashLightClientKit` (Swift SDK, iOS)

- **URL:** [https://github.com/zcash/ZcashLightClientKit](https://github.com/zcash/ZcashLightClientKit)
- **무엇인가:** Zcash iOS 경량 지갑 SDK. Swift로 작성되었으며 내부 암호화 연산은 FFI를 통해 Rust 코드(`zcash_client_backend` 기반 `zcashlc`)로 위임한다. Zashi(ECC 공식 iOS 지갑), Unstoppable Wallet, ZecWallet 등이 이 SDK를 사용한다.
- **우리 팀의 역할:** PAL은 모바일 SDK가 없지만, Category E 팀이 iOS 모바일 지갑 사용자를 지원하려 한다면 QR 스캔 후 ZEC 송금 UX에 참조 구현으로 사용한다. 특히 shielded send + memo 삽입 패턴을 이 SDK에서 가장 완성도 있게 확인할 수 있다.

---

#### 7. `pczt` (Rust crate, ZIP-374)

- **URL:** [https://docs.rs/pczt](https://docs.rs/pczt/latest/pczt/) | [GitHub: zcash/librustzcash/tree/main/pczt](https://github.com/zcash/librustzcash/tree/main/pczt)
- **무엇인가:** PCZT(Partially Created Zcash Transaction) 포맷을 구현하는 Rust crate. Bitcoin의 PSBT(BIP 174, BIP 370)에 대응하는 Zcash 버전으로, 하나의 트랜잭션 생성 작업을 여러 독립적 주체(proposer, approver, signer, combiner)로 분리하여 수행할 수 있도록 설계되었다.
- **우리 팀의 역할:** Category E에서 merchant(서비스 제공자)와 사용자가 Zcash 트랜잭션 생성에 각자 서명을 기여하는 시나리오(예: merchant가 memo field를 제안하고 user wallet이 입력/서명을 추가)에 사용할 수 있다. PAL의 NEAR Chain Signatures가 EVM tx 서명을 분산하는 방식과 개념적으로 유사하지만, 이는 Zcash-native 멀티파티 서명이다.

---

#### 8. ZIP-321 (Payment URI standard)

- **URL:** [https://zips.z.cash/zip-0321](https://zips.z.cash/zip-0321)
- **무엇인가:** Zcash payment request URI 표준. `zcash:<address>?amount=<amount>&memo=<base64>&message=<text>` 형식으로 수신 주소, 금액, memo, 메시지를 단일 URI에 인코딩한다. 이 URI를 QR 코드로 인코딩하면 지갑이 자동으로 필드를 채워 전송 준비 상태를 만든다. 복수 수신자(multiple payments)도 지원한다.
- **우리 팀의 역할:** PAL은 `components/IntentsQR.tsx:186`에서 raw `depositAddress` 문자열을 QR 인코딩한다 — ZIP-321 URI 없이. Category E에서 QR 코드에 `zcash:zs1...?amount=0.01&memo=<base64-encoded-x402-quote-hash>`를 인코딩하면 사용자의 Zcash 지갑(Zashi, Unstoppable 등)이 금액과 memo를 자동 입력한다. 이것이 x402 quote_hash를 Zcash memo field에 carry하는 가장 UX-friendly한 구현 경로다.

---

#### 9. Zcash TypeScript/JavaScript 생태계 — 부재(absence)가 자체 발견

Zcash의 JavaScript/TypeScript 네이티브 암호화 라이브러리 생태계는 **현저히 빈약하다.** PAL이 `@defuse-protocol/one-click-sdk-typescript`라는 JS SDK를 사용해 1Click에 위임하는 것이 "JS 생태계의 Zcash native library 부재"에 대한 현실적 대응이기도 하다. 확인된 현황:

- **공식 Zcash JavaScript library 없음:** ECC나 Zcash Foundation이 공식 유지하는 TS/JS Zcash 암호화 라이브러리가 현재 존재하지 않는다.
- **`zingolib` (Rust, Zingo Labs):** Rust로 작성된 경량 Zcash wallet library로 `lightwalletd`와 통신한다. URL: [https://github.com/zingolabs/zingolib](https://github.com/zingolabs/zingolib). JS/TS 바인딩은 없으나 wasm 컴파일 가능성이 있다.
- **WASM 경로:** `zcash_client_backend`를 WebAssembly로 컴파일하여 Node.js/브라우저에서 사용하는 것이 기술적으로 가능하지만, 공개 유지관리되는 npm 패키지로는 제공되지 않는다.
- **함의:** Category E에서 TypeScript 기반 Zcash native 구현을 선택하면 상당한 선행 투자(Rust → WASM 빌드 파이프라인, 또는 별도 Rust backend microservice)가 필요하다. 이것 자체가 팀 결정에 중요한 입력값이다.

---

#### 10. Zashi (참조 구현)

- **URL:** [https://github.com/Electric-Coin-Company/zashi-android](https://github.com/Electric-Coin-Company/zashi-android) (Android) / iOS는 `ZcashLightClientKit` 기반
- **무엇인가:** ECC가 개발·유지하는 공식 Zcash 모바일 지갑. `zcash_client_backend` + `lightwalletd` 패턴의 실제 프로덕션 구현이며, Orchard shielded send, ZIP-321 URI 파싱, PCZT 지원이 포함된다.
- **우리 팀의 역할:** "Zcash를 실제로 보내고 받는 참조 구현"으로 사용한다. 특히 memo 필드 작성/읽기, shielded send, ZIP-321 QR 생성 등 우리가 구현해야 하는 기능의 실제 코드를 확인하는 데 유용하다.

---

### Part C: Recommendation summary

우리 팀이 Category E(x402 + Zcash)를 선택할 경우, PAL 코드베이스에서 Zcash native 스택을 재사용할 수 있는 것은 없다. 최소한의 Zcash-native 스택을 새로 구성한다면 대략 다음과 같다:

- **Rust backend (서버 사이드):** `zcash_client_backend` crate — shielded tx 생성, viewing key 기반 note 스캔·복호화, balance 계산. `zcash_primitives`는 의존성으로 자동 포함.
- **체인 모니터링:** `lightwalletd` — 공개 엔드포인트 사용(초기) 또는 자체 호스팅(프로덕션). compact block 스트리밍으로 shielded 입금을 실시간 감지. PAL의 1분 cron 폴링(`vercel.json:9`)을 대체.
- **클라이언트 사이드 주소 표시:** ZIP-321 URI 생성 — `zcash:<unified-address>?amount=<zec>&memo=<b64-quote-hash>` 형식으로 QR 인코딩. `bech32` 패키지(`bech32@2.0.0`)는 Unified Address 인코딩에 활용 가능하지만, Orchard key material 파생은 Rust에서 처리해야 한다.
- **멀티파티 서명 (선택적):** `pczt` — merchant가 memo를 제안하고 사용자가 서명하는 시나리오에 유용. PAL의 NEAR Chain Signatures 패턴을 Zcash-native로 대응하는 방향.

이 권고는 방향 제시이지 확정 설계가 아니다. Rust backend를 Next.js 기반 PAL 아키텍처와 어떻게 통합할지(별도 microservice? WASM 컴파일?), lightwalletd를 직접 호스팅할지 공개 엔드포인트에 의존할지, PCZT를 얼마나 초기부터 적용할지 — 이런 구체적 설계 결정은 팀의 별도 설계 단계에서 이루어진다. 이 섹션은 그 결정을 위한 정보 제공이 목적이다.
