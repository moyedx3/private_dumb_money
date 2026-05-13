# Pay Anyone Legend — 팀 워크스루 (공유용)

> **읽는 사람:** 팀원 / 카테고리 E (x402 + Zcash) 후보 reference 평가자
> **목적:** Pay Anyone Legend(PAL, #37 by @kurodenjiro)가 **실제로 어떻게 동작하는지** + **우리 프로젝트가 무엇을 베끼고 무엇을 다시 만들지** 한 번에 이해.
> **작성 기준:** 2026-05-11 시점 master 브랜치 (`/home/kkang/anyone-pay` 로컬 클론) — read-only research, 실행 없음.
> **연관 자료:** 본 문서는 같은 디렉토리의 8개 서브시스템 deep-dive(`01-…md` ~ `08-…md`)와 `category-E-extraction.md`, `zcash-tool-inventory.md`의 요약/네비게이션 레이어다. 깊은 인용·근거가 필요할 때마다 그쪽으로 링크한다.

---

## 📑 목차

- [STEP 0 — 한 장 요약](#step-0--한-장-요약)
- [STEP 1 — Intent parser (자연어 → 결제 의도)](#step-1--intent-parser-자연어--결제-의도)
- (STEP 2~8은 진행하면서 추가)

---

# STEP 0 — 한 장 요약

## 0.1 "Pay Anyone Legend는 뭐 하는 프로젝트야?"

> **자연어로 'Pay onlyfan'이라고 말하면 → AI가 의도 파싱 → ZEC로 결제 → 자동으로 USDC로 환전 → x402 paywall 해제까지 한 큐에 해주는 AI 결제 어시스턴트.**

겉으로 보이는 베팅 카드 4장:

1. **x402** (HTTP 402 paywall 프로토콜)
2. **Zcash shielded payment** (프라이버시)
3. **NEAR Chain Signatures** (키를 가지지 않고 EVM 서명)
4. **AI intent parsing** (OpenAI + pgvector)

## 0.2 "그래서 실제로 어떻게 동작해?" — 5단계 happy path

```
1. 사용자: "Pay onlyfan"
       ↓
2. AI intent parsing → {amount, currency, chain, address}
       ↓
3. 1Click API에 'ZEC → USDC' quote 요청 → depositAddress(QR) 표시
       ↓
4. 사용자가 ZEC 입금 → Vercel cron이 1Click polling → SUCCESS 감지
       ↓
5. NEAR MPC가 USDC tx 서명 → Base에 broadcast → tx hash를 X-PAYMENT 헤더로 paywall 해제
```

## 0.3 ⚠️ 가장 먼저 알아야 할 **충격 포인트 4개**

| # | README가 말하는 것 | 실제 코드가 하는 것 |
|---|---|---|
| 1 | "x402 결제 프로토콜 구현" | **표준 HTTP 402 dance 없음.** USDC를 미리 on-chain push하고 tx hash를 X-PAYMENT bearer로 사용. facilitator 없음. |
| 2 | "Zcash shielded payment" | **Zcash 코드 0줄.** 입금 주소부터 swap까지 전부 1Click(Defuse Labs Limited, Gibraltar)에 외주. |
| 3 | "shielded — hides amounts/sender/recipient" | **1Click은 transparent t-address만 지원.** shielded 자체가 불가능. |
| 4 | "프라이버시 결제" | **1Click `/v0/quote` 한 요청에 sender ZEC + recipient EVM이 함께 담김.** 1Click이 모든 linkage를 봄 + AML 스크리닝. |

## 0.4 🧭 한 줄 결론

> **"PAL은 'x402 + Zcash' 프로젝트가 아니라 'AI intent + NEAR Chain Signatures + 1Click bridge + Base USDC push' 프로젝트다. x402와 Zcash 양쪽 모두 이름만 빌렸다 — 그래서 우리가 진짜로 구현하면 차별화 여지는 매우 크다."**

## 0.5 🏗️ 아키텍처 한 눈에

```
[User] → "Pay onlyfan"
   │
   ▼
[Next.js: /api/parse-intent] ──► [OpenAI 또는 NEAR AI Cloud]
   │                              + [Supabase pgvector 시맨틱 검색]
   │
   ▼ ParsedIntent
[/api/relayer/register-deposit] ──► [1Click API]  ◄── 진짜 brain은 여기
   │                                    │ depositAddress (t-addr)
   │                                    │
   ▼                                    │
[Supabase deposit_tracking]             │
   ▲                                    │
   │ tx hash                            │
[Vercel cron */1 * * * *] ──polling──► [1Click] (SUCCESS?)
   │
   ▼ SUCCESS 시
[NEAR MPC v1.signer] ──sig──► [Base USDC contract] (transferWithAuthorization)
   │
   ▼ tx hash
[User] ──X-PAYMENT: txHash──► [Merchant content server] → paywall 해제
```

> 곁가지로 **NEAR Rust contract**가 있는데 **완전히 dead code**다 (런타임에서 호출 안 됨). 무시하자.

## 0.6 📋 우리 팀이 가져갈 수 있는 것들 (큰 그림)

| 카테고리 | 가져갈 수 있는가 | 핵심 이유 |
|---|---|---|
| **AI intent parser + pgvector service registry** | ✅ **lift-and-use** | 모듈로서 잘 떨어지고, 자연어 → 결제 의도 변환이 필요할 때 그대로 쓸 만함 |
| **NEAR Chain Signatures MPC 서명 패턴** | ✅ **lift-and-use** | `chainsig.js` + `v1.signer`로 EVM tx 서명 흐름이 깔끔 |
| **1Click 의존 패턴** | ❌ **redo** — 진짜 shielded를 하려면 1Click을 빼야 함 (privacy 이유) |
| **"x402" 구현** | ❌ **redo** — 진짜 HTTP 402 challenge/response를 구현해야 함 |
| **Zcash 측 전체 스택** | ❌ **fresh build** — PAL은 한 줄도 없으므로 베낄 게 없음 |
| **Vercel cron polling + Supabase 상태 머신** | 🟡 **참고만** — 패턴은 좋지만 1분 주기는 결제용으로 너무 느림 |
| **NEAR Rust contract** | ❌ **무시** — dead code |

---

# STEP 1 — Intent parser (자연어 → 결제 의도)

> **한 줄 요약:** 사용자가 "Pay 0.1 USDC to 0x… on Base" 같은 자연어를 던지면, 서버가 **(1) pgvector 시맨틱 검색 → (2) LLM chat completion → (3) regex fallback** 3단계로 `ParsedIntent` JSON을 만든다. **이름은 NEAR AI지만 사실 OpenAI 클라이언트**다.
>
> **deep dive:** [`01-intent-parser.md`](./subsystems/01-intent-parser.md)

## 1.1 "이 서브시스템은 왜 있어?"

- x402 결제를 **자연어로 시작할 수 있게 만드는 UX 추상화 계층**.
- 두 가지 진입 경로:
  - 등록된 서비스(예: OnlyFans, ChatGPT)면 → 시맨틱 매칭 한 방으로 `{amount, currency, chain, address}` 다 채워짐
  - 자유 입력이면 → LLM이 필드 추출

## 1.2 데이터 흐름 (단순화)

```
[브라우저: FloatingInput 입력]
        │ "Pay 0.1 USDC to 0x… on Base"
        ▼
[lib/intentParser.ts: parseIntent()]  ◄── client-side fetch wrapper
        │ POST /api/parse-intent
        ▼
[app/api/parse-intent/route.ts]  ◄── server entry
        │
        ▼
[lib/nearAI.ts: analyzePromptWithNearAI()]
        │
        ├── (1) findBestService(prompt, 0.6)
        │       └─► Supabase RPC: match_services (pgvector cosine)
        │            ├─ 매칭 성공 → 그 자리에서 ParsedIntent 조립 (LLM 호출 X)
        │            └─ 미매칭 → 다음 단계
        │
        ├── (2) chat.completions.create()
        │       ├─ OPENAI_API_KEY 있음 → gpt-4o-mini @ api.openai.com
        │       └─ 없음             → deepseek-chat-v3-0324 @ cloud-api.near.ai
        │       (response_format = json_object 강제)
        │
        └── (3) parsePromptFallback()  ◄── API 키 둘 다 없을 때
                └─ regex로 숫자/도메인/0x주소 추출
        ▼
[ParsedIntent JSON 반환]
        ▼
[app/page.tsx → generateDepositAddress() 또는 aiMessage 표시]
```

## 1.3 핵심 파일 (file:line)

| 역할 | 위치 |
|---|---|
| 클라이언트 fetch 래퍼 | `lib/intentParser.ts:16` `parseIntent()` |
| 서버 핵심 로직 | `lib/nearAI.ts:29` `analyzePromptWithNearAI()` |
| 서비스 시맨틱 검색 | `lib/serviceRegistry.ts:168` `findBestService()` |
| 쿼리 임베딩 생성 | `lib/serviceRegistry.ts:30` `generateEmbedding()` (OpenAI `text-embedding-3-small`) |
| pgvector RPC 호출 | `lib/serviceRegistry.ts:68` `supabase.rpc('match_services', …)` |
| API route | `app/api/parse-intent/route.ts:16` `POST` |
| Regex fallback | `lib/nearAI.ts:224` `parsePromptFallback()` |
| 도메인 → 체인 추론 | `lib/nearAI.ts:279` `detectChainForDomain()` (heuristic only) |

## 1.4 🎬 구체적 예시 2개 (실제 흐름)

### 예시 A — 등록된 서비스 매칭 케이스: `"Pay onlyfan"`

이 경우 pgvector 시맨틱 검색에서 OnlyFans 레코드와 매칭 (유사도 ≥ 0.6)되어 **LLM을 전혀 호출하지 않는다.**

```
[1] 사용자: "Pay onlyfan"
       ↓
[2] FloatingInput → app/page.tsx → parseIntent("Pay onlyfan")
       ↓ POST /api/parse-intent { query: "Pay onlyfan" }
[3] app/api/parse-intent/route.ts
       ↓ analyzePromptWithNearAI("Pay onlyfan")
[4] OpenAI text-embedding-3-small으로 "Pay onlyfan" 벡터화
       ↓
[5] Supabase RPC: match_services(embedding, threshold=0.6)
       ↓
[6] payment_services 테이블에서 OnlyFans 레코드 매칭 (예: 유사도 0.82)
       → matchedService = {
           id: "onlyfans-1",
           name: "OnlyFans",
           amount: "9.99",
           currency: "USDC",
           chain: "base",
           url: "https://onlyfans.com/...",
           receivingAddress: "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943"
         }
       ↓
[7] lib/nearAI.ts:36-49가 즉시 AnalyzedIntent 조립 후 return
       → { action: "pay", amount: "9.99", currency: "USDC",
           chain: "base", needsBridge: true,
           bridgeFrom: "zcash",     ◄── 💥 하드코딩 #1 (line 43)
           bridgeTo: "base",
           recipient: "https://onlyfans.com/...",
           receivingAddress: "0x03fBbA..." }
       ↓
[8] API route가 ParsedIntent로 래핑 → 클라이언트에 응답
       ↓
[9] app/page.tsx → generateDepositAddress() → 1Click /v0/quote 호출 → QR 표시
```

**핵심:** OnlyFans 같은 등록 서비스는 **DB에 미리 입력된 메타데이터**(amount, chain, receivingAddress)를 그대로 끌어다 쓴다. LLM은 안 부른다 (비용 0, 임베딩 1회).

---

### 예시 B — 자유 입력 케이스: `"Pay 0.1 USDC to 0x03fBbA... on Base"`

서비스 매칭이 안 되므로 LLM이 필드를 추출한다.

```
[1] 사용자: "Pay 0.1 USDC to 0x03fBbA1b1A455d028b074D9abC2b23d3EF786943 on Base"
       ↓
[2-3] FloatingInput → /api/parse-intent → analyzePromptWithNearAI(...)
       ↓
[4-5] 임베딩 생성 → match_services(threshold=0.6)
       ↓
[6] 매칭 실패 (이런 자유 입력은 등록 서비스랑 코사인 유사도가 0.6 미만)
       ↓
[7] getAllServicesForPrompt()로 전체 서비스 목록을 system prompt에 끼움
       ↓
[8] openai.chat.completions.create({
       model: process.env.OPENAI_API_KEY ? 'gpt-4o-mini' : 'deepseek-chat-v3-0324',
       messages: [
         { role: 'system', content: systemPrompt },  ◄── 💥 여기 prompt에 하드코딩 #2
         { role: 'user',   content: "Pay 0.1 USDC to 0x... on Base" }
       ],
       response_format: { type: 'json_object' }
     })
       ↓
[9] LLM 응답:
     {
       "action": "pay",
       "amount": "0.1",
       "currency": "USDC",
       "chain": "base",
       "needsBridge": true,
       "bridgeFrom": "zcash",    ◄── 💥 LLM이 system prompt 예시를 그대로 따라함
       "bridgeTo": "base",
       "receivingAddress": "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943"
     }
       ↓
[10] currency 정규화 (USDT인 경우만, 여기선 이미 USDC) +
     hasCompleteData = true 검사 통과
       ↓
[11] 클라이언트 응답 → generateDepositAddress() → 1Click quote → QR
```

**핵심:** 자유 입력은 LLM이 4개 필드(`amount`, `currency`, `chain`, `receivingAddress`)를 뽑되, **`bridgeFrom`은 LLM이 "추론"하는 게 아니라 system prompt에 박힌 예시 JSON을 따라하는 것**이다 (사용자가 BTC 출금 의도여도 결과는 `"zcash"`로 나옴).

---

## 1.5 💥 "항상 ZEC 입금 시작" 하드코딩, **어디에 있는가**

이게 PAL이 *"Zcash 결제 앱"*으로 보이게 만드는 가장 핵심적인 design constraint이다. **두 군데**에 박혀 있고, **둘 다 코드 변경 없이는 우회 불가능**하다.

### 위치 #1 — 서비스 매칭 분기 (코드 레벨)

**`lib/nearAI.ts:34-49`** — 시맨틱 검색이 hit 했을 때 즉시 조립되는 `AnalyzedIntent`:

```typescript
// lib/nearAI.ts:34
if (matchedService) {
  // If service is found, use its details with direct URL
  return {
    action: 'pay',
    amount: matchedService.amount,
    currency: matchedService.currency,
    recipient: matchedService.url,
    chain: matchedService.chain,
    needsBridge: true,
    bridgeFrom: 'zcash',                  // ◄── 💥 line 43: 무조건 zcash
    bridgeTo: matchedService.chain,
    serviceId: matchedService.id,
    serviceName: matchedService.name,
    redirectUrl: matchedService.url,
    receivingAddress: matchedService.receivingAddress,
  }
}
```

> service 매칭이 되면 — 그 서비스가 BTC를 받든 ETH를 받든 — `bridgeFrom`은 **무조건 `'zcash'`**.

### 위치 #2 — LLM system prompt 안에 박힌 예시 JSON

**`lib/nearAI.ts:86-97`** — `analyzePromptWithNearAI`의 LLM system prompt 중 "complete payment intent 예시" 블록:

```typescript
// lib/nearAI.ts:86-97 (system prompt 문자열 안에)
For COMPLETE payment intents (all required fields present), respond with this JSON structure:
{
  "action": "pay",
  "amount": "0.1",
  "currency": "USDC",
  "recipient": "",
  "chain": "base",
  "needsBridge": true,
  "bridgeFrom": "zcash",                  // ◄── 💥 line 94: LLM에 예시로 박혀 있음
  "bridgeTo": "base",
  "receivingAddress": "0x03fBbA1b1A455d028b074D9abC2b23d3EF786943"
}
```

> LLM은 "사용자가 어디서 출금하고 싶은지"를 추론하는 게 아니라 **예시 JSON의 패턴을 그대로 모방한다**. response_format이 json_object로 강제되어 있고 예시 안에 `bridgeFrom: "zcash"`가 박혀 있으면, gpt-4o-mini는 99% 이걸 따라 한다.

### 그래서 결과적으로

- **모든 결제 = ZEC 입금 강제** (사용자가 "I have BTC" 같은 의도를 표현해도)
- **이건 architectural decision이지 LLM의 판단이 아니다** — PAL이 *"Zcash 결제 앱"*을 표방하기 위한 의도된 design
- **우리 팀에는 두 가지 의미:**
  1. PAL을 reference로 쓸 때 이 줄을 **반드시 인지하고 분리**해야 함 (안 그러면 우리도 항상 ZEC 입금이 됨)
  2. PAL의 "AI intent parser"는 *진짜로 자유로운 의도 파싱이 아니라* "ZEC → X로 보내는 결제만" 파싱하는 좁은 도메인 어시스턴트라는 점

### 다른 곳에도 흔적이 있는가?

`bridgeFrom`이 다른 값으로 덮어쓰이는 곳은 코드에 없다. `rg -n "bridgeFrom"` 결과:
- `lib/nearAI.ts:43` (매칭 분기 — 위 #1)
- `lib/nearAI.ts:94` (LLM 예시 — 위 #2)
- 그 외에는 모두 **읽기만** 하는 위치 (UI 표시, 1Click 호출 시 origin chain 결정 등).

즉, **하드코딩은 이 두 줄이 전부**고, 우리가 PAL의 intent parser를 lift-and-use 할 때 **이 두 줄만 손보면 multi-source가 가능**해진다.

---

## 1.6 ⚠️ 팀에게 강조해야 할 함정 (footguns)

1. **`lib/nearAI.ts`는 사실 OpenAI 클라이언트다.**
   파일 최상단 주석에 *"TEMPORARILY using OpenAI for testing"* (`lib/nearAI.ts:3`). `openai` npm 패키지를 그대로 쓰고 `baseURL`만 NEAR AI Cloud로 바꾸는 방식. **NEAR AI 전용 SDK는 없다.**

2. **모델이 환경변수로 갈린다.**
   `OPENAI_API_KEY` 있음 → `gpt-4o-mini` / 없음 → `deepseek-chat-v3-0324`. **응답 품질이 환경마다 달라진다** — 우리가 채택할 때는 모델 고정 권장.

3. **Prompt injection 노출면.**
   `getAllServicesForPrompt()`가 Supabase에서 가져온 service name/keywords/URL을 system prompt에 그대로 삽입함 (`lib/nearAI.ts:69-71`). **악의적인 서비스를 DB에 등록하면 LLM 조작 가능.** 우리가 reference로 쓸 때 admin/저장 단계에 sanitize 들어가야 함.

4. **`detectChainForDomain`은 placeholder다.**
   주석에 *"In production, this would query a registry or API"* (`lib/nearAI.ts:279`). `.near` / `.sol` suffix만 보고 나머지는 default `ethereum`. **체인 감지가 부정확할 수 있다.**

5. **`user_account` 필드 미사용.**
   API route가 body에서 받지만 어디서도 안 씀 (`app/api/parse-intent/route.ts:19`). 자리 표시자.

6. **`require()` in ES Module.**
   `lib/nearAI.ts:216`에서 순환 import 회피용으로 CommonJS `require()`를 동적 호출. TypeScript 타입 안전성이 깨짐.

7. **비용 패턴.**
   - 서비스 매칭 O → embedding 1회만
   - 서비스 매칭 X → embedding 1회 + chat completion 1회
   완전한 happy path는 OpenAI 호출 **최소 2회**.

## 1.5 🧪 우리 프로젝트에 가져갈 수 있을까?

### ✅ 가져갈 만한 것 (lift-and-use)

- **3단계 fallback 구조 (semantic → LLM → regex)**
  자연어 결제에서 LLM 비용을 줄이는 좋은 패턴. 자주 쓰는 service는 임베딩으로 cache하고 자유 입력만 LLM에 떨굼.

- **`response_format: { type: 'json_object' }` + 명시적 RULES/REQUIRED FIELDS system prompt 패턴**
  LLM 출력 안정성에 효과적. 그대로 모방 가능.

- **USDT → USDC 자동 정규화 같은 후처리 normalize 단계**
  결제 stablecoin alias 처리는 우리 도메인에서도 필요.

### ❌ 다시 만들어야 할 것 (redo)

- **NEAR AI 위장 레이어**
  우리는 처음부터 깨끗하게 OpenAI 또는 우리 LLM provider를 명시. `baseURL` 트릭 쓰지 말 것.

- **prompt injection 노출**
  service registry 입력 단계에 input validation/escape 필수.

- **`detectChainForDomain` placeholder**
  진짜 chain registry(예: ENS reverse / Solana SNS / Base name service)를 붙이거나, 사용자가 명시적으로 선택하게 함.

### 🟡 참고만 (skip 또는 thin)

- **pgvector 시맨틱 검색 자체**
  쓸 만하지만 우리 도메인(자유 결제 vs. 등록된 service catalog)에 따라 가치 차이. 자유 결제 위주면 굳이 안 필요.

## 1.6 우리 차별화 포인트 (Category E 진짜 빌드 관점)

- **PAL의 intent parser는 결제 의도(amount/currency/chain/address)만 뽑는다.**
  우리가 진짜 x402 + Zcash 빌드면, intent에 **"shielded vs transparent" 선택**, **"메모/PII 포함 여부"**, **"긴급도(latency vs 비용)"** 같은 **프라이버시·정책 의도**도 같이 뽑을 수 있음. PAL은 이 axis가 아예 없다.

- **PAL은 모든 결제가 ZEC 입금에서 시작한다고 system prompt에 하드코드 가정.**
  `bridgeFrom: 'zcash'`가 LLM system prompt 기본값 (`lib/nearAI.ts:94-95`). 우리는 multi-source가 가능하도록 유연하게 설계할 수 있음.

---

---

# STEP 2 — Service registry (pgvector 시맨틱 검색)

> **한 줄 요약:** Supabase `payment_services` 테이블에 결제 가능한 서비스를 등록하고, **사용자 자연어 쿼리와 cosine similarity로 매칭**해주는 시맨틱 검색 모듈. 등록 시 1회 임베딩, 검색 시마다 쿼리 임베딩.
>
> **deep dive:** [`02-service-registry.md`](./subsystems/02-service-registry.md)

## 2.1 "이게 왜 있어?"

[STEP 1](#step-1--intent-parser-자연어--결제-의도)에서 *"Pay onlyfan"* 같은 짧은 자연어를 LLM 호출 없이 처리하기 위한 **카탈로그 + 시맨틱 매처**. 두 진입로:

- **Write path:** 관리자가 `CreateServiceModal`로 서비스 등록 → insert 시점에 임베딩 생성 → DB 저장
- **Read path:** intent parser가 `findBestService(prompt, 0.6)` 호출 → 쿼리 임베딩 → `match_services` RPC → 가장 유사한 service

## 2.2 데이터 모델 (스키마)

```sql
-- supabase-setup.sql:5,8-22,25-28
CREATE EXTENSION vector;

CREATE TABLE payment_services (
  id UUID PRIMARY KEY,
  name TEXT NOT NULL,
  keywords TEXT[] NOT NULL,
  amount TEXT NOT NULL,
  currency TEXT DEFAULT 'USD',         -- API에서 'USDC'로 강제
  url TEXT NOT NULL,                   -- 결제 후 redirect할 콘텐츠 URL
  chain TEXT NOT NULL,                 -- 'base' or 'solana'
  receiving_address TEXT,              -- swap 후 USDC 받을 EVM/Solana 주소
  description TEXT,
  active BOOLEAN DEFAULT true,
  embedding vector(1536),              -- text-embedding-3-small 차원
  created_at, updated_at TIMESTAMPTZ
);

CREATE INDEX payment_services_embedding_idx
  ON payment_services USING ivfflat (embedding vector_cosine_ops)
  WITH (lists = 100);
```

**핵심 SQL 함수 — `match_services`:**

```sql
-- supabase-setup.sql:36-78
RETURN QUERY
SELECT ..., 1 - (embedding <=> query_embedding) AS similarity
FROM payment_services
WHERE active = true
  AND embedding IS NOT NULL
  AND 1 - (embedding <=> query_embedding) > match_threshold
ORDER BY embedding <=> query_embedding   -- 코사인 거리 오름차순
LIMIT match_count;
```

> `<=>`는 pgvector의 **코사인 거리** 연산자. `similarity = 1 - distance`로 변환해서 threshold 비교.

## 2.3 🎬 구체적 예시 — OnlyFans 서비스 등록 → "Pay onlyfan" 검색

### Write (등록 한 번)

```
[관리자: CreateServiceModal에서 폼 입력]
  name: "OnlyFans"
  keywords: ["onlyfans", "subscription", "creator"]
  amount: "9.99"
  currency: "USDC"      (UI 고정)
  chain: "base"         (select)
  url: "https://onlyfans.com/..."
  receivingAddress: "0x03fBbA..."
       ↓ POST /api/services
[API route: currency=='USDC', chain∈{base,solana} 검증]
       ↓
[lib/serviceRegistry.ts:259 addService()]
  searchText = "OnlyFans  onlyfans subscription creator"
       ↓
[OpenAI text-embedding-3-small (입력: searchText)]
  → vector[1536]
       ↓
[Supabase INSERT into payment_services with embedding 컬럼]
       ↓
HTTP 201 + 새 PaymentService 반환
```

> 임베딩 텍스트 = `name + " " + description + " " + keywords.join(' ')` (`serviceRegistry.ts:259-260`).
> **이 임베딩은 insert 시점 1회만 생성**되고 DB에 영구 저장됨. name/description/keywords 수정 시에만 재생성.

### Read (매 검색마다)

```
[intent parser: findBestService("Pay onlyfan", 0.6)]
       ↓
[OpenAI text-embedding-3-small (입력: "Pay onlyfan")]  ◄── 매번 호출
  → query_embedding vector[1536]
       ↓
[supabase.rpc('match_services', {
   query_embedding,
   match_threshold: 0.6,
   match_count: 10
 })]
       ↓
[Postgres가 IVFFlat 인덱스로 검색
   → OnlyFans 레코드, similarity 0.82
   → 다른 서비스들은 0.6 미만이라 제외]
       ↓
findBestService는 첫 번째(가장 유사한) 결과만 return
  → PaymentService { id, name="OnlyFans", amount="9.99", ... }
       ↓
[STEP 1.4 예시 A의 [7]번 step으로 연결]
```

## 2.4 ⚠️ 팀에게 강조할 함정 (footguns)

### 1. **threshold가 호출자마다 다르다 (0.6 vs 0.7)**

| 호출자 | threshold |
|---|---|
| `findBestService` 기본값 | 0.7 |
| intent parser `lib/nearAI.ts:32` | **0.6** (명시적 전달) |
| `/api/services?q=` 검색 API | 0.7 |

→ 같은 쿼리도 어디서 부르냐에 따라 결과가 다를 수 있음. README의 *"임계값 0.6"*은 **intent parser 경로에서만** 정확.

### 2. **쿼리 임베딩 = 매 검색마다 OpenAI API 호출 1회 (캐싱 없음)**

사용자 트래픽 ↑ → OpenAI 임베딩 비용이 선형 증가.
**해결책 (우리 팀):**
- Redis/LRU에 최근 N개 쿼리 임베딩 캐시 (예: `"pay onlyfan"` → vector)
- 또는 hash 키로 in-memory 짧은 캐시

### 3. **`receiving_address`에 chain-format 검증 없음**

`receiving_address TEXT`, CHECK constraint 없음. API route도 `chain ∈ {base, solana}`만 검사하지 **주소 형식은 검증 안 함**.
→ **Base chain에 Solana 주소를 등록해도 통과됨**. swap 완료 후 USDC가 잘못된 chain으로 가서 **자금 손실** 가능.

### 4. **prompt injection 노출 (STEP 1과 연결)**

`getAllServicesForPrompt()` (STEP 1.6에서 다룬 footgun)이 이 테이블의 **`name` / `keywords` / `url`을 그대로 LLM system prompt에 삽입**. 악성 service 등록 → LLM 조작 가능. **service 등록 단계에서 input sanitization 필수**.

### 5. **`url` 필드는 GET /api/services 응답에서 숨겨짐**

```typescript
// app/api/services/route.ts:43-46
const servicesWithoutUrl = services.map(({ url, ...service }) => service)
```

→ 목록 응답에는 `url` 제거. id로 단일 조회할 때만 `url` 포함. *"결제 안 한 사람한테 콘텐츠 URL 노출 안 한다"*는 의도.

### 6. **delete는 soft-delete only**

`deleteService(id)`는 실제로는 `UPDATE active=false`. row 자체는 남아 있음. 임베딩도 남아서 인덱스 사이즈에 영향.

### 7. **`data_drops` 테이블 — 문서엔 있는데 SQL엔 없음**

SUPABASE_SETUP.md는 `data_drops` 테이블을 언급하지만 **`supabase-setup.sql`에 정의 없음**. 어디서 만들어졌는지 불명. 우리가 만약 PAL을 직접 띄울 거면 이거 추적 필요.

### 8. **`scripts/setup-supabase.ts`는 DDL 실행 못 함**

Supabase JS SDK는 DDL 직접 실행 불가. 스크립트가 결국 *"대시보드에서 수동 실행하세요"* 안내만 출력. **deploy 자동화에 함정**.

### 9. **RLS 정책 미설정**

`payment_services` 테이블에 RLS policy 없음. anon key 클라이언트로 누구나 접근 가능 (현재는 RLS 자체가 꺼져 있어 무관하지만, 켜는 순간 작동 멈춤).

### 10. **`resource_key` legacy 컬럼 fallback**

코드 5군데서 `row.url || row.resource_key`. 스키마엔 없는 컬럼인데 fallback 처리되어 있음 → 과거 스키마 흔적. 우리가 쓸 거면 정리 대상.

## 2.5 🎯 우리 프로젝트에 가져갈 거 / 다시 만들 거

### ✅ Lift-and-use

- **pgvector `<=>` cosine + IVFFlat 인덱스 패턴 자체** — 자연어 매칭이 필요한 카탈로그라면 그대로 모방 가능
- **`match_services` SQL 함수 시그니처** (`query_embedding, match_threshold, match_count`) — 좋은 인터페이스
- **insert-time embedding, query-time embedding 비대칭 구조** — 합리적 (저장은 1회, 검색은 다회)
- **GET 목록에서 민감 필드(`url`) 제거하는 패턴** — 우리 도메인에도 적용 가능

### ❌ Redo

- **threshold 0.6 vs 0.7 불일치** → 한 곳에 상수로 통일
- **임베딩 캐싱 없음** → Redis/in-memory LRU 추가
- **`receiving_address` 검증 없음** → chain별 정규식 (Base = `0x[0-9a-fA-F]{40}`, Solana = base58 32~44자, Zcash = `zs1...`/`u1...`/`t1...`/`t3...`) 추가
- **soft-delete만** → audit/compliance 정책에 따라 hard-delete 옵션 추가 검토
- **service 등록 input sanitization 부재** → LLM prompt injection 방어
- **RLS 정책 미설정** → 우리는 처음부터 RLS 켜고 service role 분리

### 🟡 참고만

- **`data_drops` 흔적** — 우리 데이터 모델엔 필요 없을 가능성 높음
- **`scripts/setup-supabase.ts`** — 효용 없으니 무시. Supabase migrations CLI 또는 raw psql 사용 권장

## 2.6 🚀 우리 차별화 포인트

- **PAL은 등록 service만 시맨틱 매칭한다.** 우리는 **(a) 등록 service + (b) 사용자 contact 리스트 + (c) 메모 패턴** 등 multi-source 시맨틱 검색을 합칠 수 있음 → "Pay mom" 같은 query도 자연스럽게 처리 가능
- **PAL의 임베딩 텍스트는 `name + description + keywords`만.** 우리는 **multilingual 처리, 약어(LoL → League of Legends), 동의어(저번 거래 상대)** 같은 enrichment 추가 가능
- **PAL의 `currency`는 `USDC`로 API에서 하드코딩.** 진짜 카테고리 E라면 우리는 **`payment_asset = "ZEC"`(shielded)**를 카탈로그 entry 수준에서 지원 가능 → "그 서비스는 ZEC 직결제 가능"이라는 메타데이터를 시맨틱 결과에 노출 가능

---

---

# STEP 3 — Z-address generation (👻 truth: PAL은 안 만든다)

> **한 줄 요약:** PAL은 Zcash 주소를 **단 한 줄도 생성하지 않는다.** 사용자에게 보여주는 deposit address(QR)는 전부 **1Click API 응답의 `depositAddress` 필드를 그대로 pass-through**한 값이다. week2의 *"`crypto.getRandomValues + 'zs1'` 패턴으로 가짜 생성한다"* 주장은 틀렸다 — **가짜로 만드는 게 아니라 외주.**
>
> **deep dive:** [`03-z-address-generation.md`](./subsystems/03-z-address-generation.md)

## 3.1 "이 모듈이 진짜로 하는 일은?"

이름은 "Z-address generation"이지만 **실제 동작은 "Z-address reception"**이다.

```
[PAL이 1Click에 quote 요청]
     ↓
[1Click 응답에 depositAddress 포함]
     ↓
[PAL이 그 값을 Supabase에 저장 + QR로 표시]
```

PAL 측 코드:
- ❌ Zcash spending key, viewing key 없음
- ❌ ZIP-32 HD path derivation 없음
- ❌ Orchard/Sapling key derivation 없음
- ❌ bech32m Zcash 인코딩 없음
- ❌ Zcash 암호화 라이브러리 **0개**

→ `package.json`에 `@zec`, `zcash-wasm`, `librustzcash`, `bellman`, `orchard`, `sapling-crypto`, `pczt`, `zcash_client_backend` 같은 **Zcash 관련 의존성 단 하나도 없다.** 검증 완료.

## 3.2 "그럼 그 'address generation' 코드 전체는 어디 있어?"

**`lib/oneClick.ts:122-128` + `app/api/relayer/register-deposit/route.ts:66-69` — 이 두 곳이 전부다.**

```typescript
// lib/oneClick.ts:122-128 (1Click 응답 처리)
return {
  ...data,
  depositAddress: data.depositAddress
                  || data.quote?.depositAddress
                  || data.address,   // ◄── 이 한 줄이 PAL의 "z-address generation"
  swapId: data.swapId || data.id || data.depositAddress,
  sessionId: responseSessionId,
}
```

```typescript
// app/api/relayer/register-deposit/route.ts:66-69
depositAddress = quote.depositAddress
                 || quote.quote?.depositAddress
                 || quote.address    // ◄── 같은 fallback 체인을 한 번 더
if (!depositAddress) {
  throw new Error('No deposit address found in quote response')
}
```

> **이게 전부다.** 진짜로 이 두 fallback chain이 PAL의 Zcash deposit address "생성" 코드 100%.

## 3.3 🎬 흐름 예시 — 사용자 "Pay 10 USDC to 0x… on Base" 입력 시

```
[사용자: "Pay 10 USDC to 0x... on Base"]
   ↓ STEP 1 intent parser → ParsedIntent
[app/page.tsx::generateDepositAddress()]
   ↓ POST /api/relayer/register-deposit
[register-deposit/route.ts: getSwapQuote(...) 호출]
   ↓
[lib/oneClick.ts: POST https://1click.chaindefuser.com/v0/quote]
   요청 body:
   {
     "originAsset":     "nep141:zec.omft.near",         ◄── ZEC
     "destinationAsset":"nep141:base-0x8335...omft.near", ◄── USDC on Base
     "amount":          "10000000",                      ◄── USDC 6 decimals
     "recipient":       "<swapWallet — MPC 파생 EVM 주소>",
     "refundTo":        "<sender ZEC 주소 또는 env REFUND_ZCASH_ADDRESS>",
     "deadline":        "<+3분>",
     "slippageTolerance": 100
   }
   ↓
[1Click API 응답]
   {
     "depositAddress": "t1Qc...",   ◄── 💥 이게 사용자에게 보여줄 주소
     "swapId": "...",
     ...
   }
   ↓
[PAL: registerDeposit(depositAddress, ...) → Supabase 저장]
[PAL: 응답을 클라이언트로 전달]
   ↓
[IntentsQR.tsx: <QRCodeSVG value={depositAddress} ...>]
   ↓
[사용자가 QR 스캔 → ZEC 송금 → 1Click solver 주소가 수신]
```

## 3.4 ⚠️ 그래서 무슨 문제가 생기는데?

### 1. **이 주소는 1Click이 소유한다 (PAL이 아님)**
- spending key/viewing key를 PAL이 안 가짐
- 사용자가 송금하면 **1Click의 solver wallet이 수령**
- PAL은 그저 polling으로 "1Click이 받았다고 했나?"만 확인

### 2. **1Click이 transparent t-address만 지원**
- 공식 문서: *"⚠️ Partially supported — Transparent addresses only (`t1`/`t3` prefix)"*
- 즉, **PAL이 사용자에게 보여주는 QR은 `zs1...` shielded 주소가 아니라 `t1...`/`t3...` transparent 주소**
- README의 *"shielded transactions hide amounts/sender/recipient"* 주장은 **L1에서 false**

### 3. **1Click이 다운되면 자금 회수 메커니즘이 얇다**
- `refundTo: process.env.REFUND_ZCASH_ADDRESS || params.senderAddress` (`lib/oneClick.ts:86`)
- `REFUND_ZCASH_ADDRESS` 환경변수는 optional
- env 미설정 + sender 주소 검증도 안 함 → 잘못된 주소면 송금된 ZEC 회수 불가

### 4. **QR 코드는 ZIP-321 URI가 아니라 그냥 주소 문자열**
- `<QRCodeSVG value={depositAddress} ...>` (`components/IntentsQR.tsx:186`)
- 표준은 `zcash:zs1...?amount=...` (ZIP-321) 형식
- 사용자가 QR 스캔해도 **amount는 자동으로 안 채워짐** → 사용자가 수동으로 10 ZEC를 입력해야 함
- amount mismatch 시 swap 실패 (slippage 1%만 허용)

### 5. **`zs1test123` 같은 더미가 코드에 있긴 한데...**
- `contract/deploy.sh:54`: `"deposit_address\":\"zs1test123\"`
- `contract/test-contract.sh:14`: `"deposit_address\":\"zs1test123456789\"`
- **이건 shell 스크립트의 NEAR contract test literal**이지 앱 런타임 코드 아님
- → week2 리뷰어가 이걸 보고 *"mock 생성"*이라고 결론 내렸을 가능성 있음. **잘못된 해석.**

## 3.5 🔍 week2 claim 정정

| week2 claim | 실제 |
|---|---|
| "z-address 생성도 mock (`crypto.getRandomValues + 'zs1' prefix`)" | ❌ 그런 패턴은 **이 코드베이스에 존재하지 않는다.** `crypto.getRandomValues`는 `lib/kdf.ts`의 Bitcoin 주소 파생(NEAR Chain Sig) 내부에서만 호출되고 Zcash와 무관 |
| "Zcash 측 구현은 얕다" | ✅ **맞음**. 다만 방식이 "fake generation(B)"이 아니라 "**완전 outsource(C)**" |

→ 핵심 메시지: *"가짜로 만드는 게 아니라, 아예 만들지를 않는다."*

## 3.6 🎯 우리 프로젝트 관점

### ❌ 가져갈 게 없음 (literally zero)

**PAL의 Zcash 측에는 우리가 lift-and-use 할 코드가 한 줄도 없다.** 1Click 호출 두 줄과 QR 렌더링 한 줄이 전부.

### ✅ "이렇게 외주하면 편하긴 하다" — 만약 우리도 외주한다면 (👎 비권장)

- 1Click 한 줄 호출로 deposit address 받기 → 그냥 표시
- **트레이드오프:** privacy 0 (1Click이 sender ZEC + recipient EVM을 같은 요청에 봄, AML 스크리닝됨)
- 우리가 *"진짜 카테고리 E"*로 간다면 → 외주는 cat E의 가치 자체를 무너뜨림

### ❌ Redo / Fresh build (우리가 진짜 카테고리 E로 가려면)

1. **Zcash key derivation**
   - ZIP-32 HD path + Sapling 또는 Orchard
   - 권장 스택: `zcash_client_backend` (Rust) 또는 모바일이라면 `ZcashLightClientKit` (Swift) / Kotlin SDK
   - JS-first 팀이라면 → Rust 백엔드 마이크로서비스 + Node가 RPC로 호출

2. **Zcash 체인 상태 조회**
   - lightwalletd 또는 Zebra RPC 연동
   - PAL은 1Click polling만 하지 실제 체인 조회는 0번 함

3. **ZIP-321 URI**
   - `zcash:zs1...?amount=...&memo=...` 형식으로 QR 인코딩
   - 사용자가 amount 자동 입력되어야 slippage 사고 방지

4. **Zcash transaction 생성/서명**
   - PCZT (Partially Constructed Zcash Transaction) 또는 직접 sapling/orchard tx 빌더
   - 우리가 shielded settlement 하려면 이게 핵심

## 3.7 🚀 우리 차별화 포인트

**이게 카테고리 E 차별화의 핵심 포인트다.**

- **PAL 전제:** "Zcash는 funding asset, 진짜 결제는 USDC on Base"
- **우리가 진짜 카테고리 E를 한다면:**
  - **Zcash가 settlement asset 그 자체.** ZEC가 x402 결제의 끝이고, USDC로 환전 안 함
  - **shielded 주소 직접 생성** + **viewing key 기반 입금 확인** → 1Click에 sender-recipient linkage 노출 안 함
  - **메모(memo) 필드로 x402 challenge nonce 전달** → 결제 콘텐츠와 연결
  - **regtest / testnet 모드** → 개발/테스트 환경에서 mainnet 의존 없이 동작

→ 한마디로: PAL은 *"Zcash라는 단어를 쓴다"*에 그치고, 우리가 *"Zcash로 실제 결제한다"*로 가면 그 격차 자체가 차별화.

---

---

# STEP 4 — Deposit tracking (cron polling + Supabase 상태 머신)

> **한 줄 요약:** Vercel cron이 **1분마다** Supabase `deposit_tracking` 테이블의 미만료 deposit들을 순회하며 1Click SDK `getExecutionStatus(depositAddress)`를 **outbound 폴링**한다. 응답이 `SUCCESS`면 즉시 NEAR Chain Sig으로 x402 tx를 서명·broadcast. **Zcash 체인을 직접 조회하는 코드는 한 줄도 없다 — 1Click 응답을 blind trust한다.**
>
> **deep dive:** [`04-deposit-tracking.md`](./subsystems/04-deposit-tracking.md)

## 4.1 "이 모듈이 진짜로 하는 일은?"

> "사용자가 'ZEC 보냈다'고 한 시점"과 "x402 paywall을 해제할 수 있는 시점" 사이의 다리 역할.

```
[사용자가 ZEC 입금] ……(PAL은 모른다, 체인 조회 안 함)……
                                                    │
                          [1Click이 swap 완료해서 status를 SUCCESS로]
                                                    │
[Vercel cron 1분마다 1Click polling] ◄──────────────┘
        │ SUCCESS 감지
        ▼
[NEAR MPC로 USDC tx 서명 → Base에 broadcast]
        │ tx hash
        ▼
[Supabase deposit_tracking.signed_payload에 저장]
        │
        ▼
[UI가 폴링으로 signed_payload 받음 → content unlock]
```

## 4.2 Supabase 스키마 핵심

```sql
-- supabase-deposit-tracking.sql:5-25
CREATE TABLE deposit_tracking (
  deposit_address TEXT PRIMARY KEY,    -- ◄── 1Click이 준 주소 = 주문 ID
  intent_id TEXT NOT NULL,
  amount TEXT NOT NULL,                 -- USDC 금액
  recipient TEXT,                       -- x402 결제 수신자 (원래 payment address)
  swap_wallet_address TEXT,             -- MPC 파생 EVM 주소 (swap 후 USDC 도착지)
  near_account_id TEXT,
  confirmed BOOLEAN DEFAULT false,
  swap_id TEXT,
  chain TEXT,                           -- 'base' | 'solana'
  x402_executed BOOLEAN DEFAULT false,
  redirect_url TEXT,
  tx_hash_submitted BOOLEAN DEFAULT false,
  deposit_tx_hash TEXT,                 -- 사용자가 submit한 ZEC tx hash (optional)
  quote_data JSONB,                     -- 1Click /v0/quote 응답 전체
  deadline TIMESTAMPTZ,                 -- quote 유효기한
  signed_payload TEXT                   -- ◄── 컬럼명과 달리 Ethereum tx hash 저장
);

ALTER TABLE deposit_tracking DISABLE ROW LEVEL SECURITY;
-- → service role key로만 접근, anon 접근 차단
```

> ⚠️ **컬럼명 함정:** `signed_payload`는 이름과 달리 "서명된 바이트"가 아니라 **이미 broadcast된 Ethereum tx hash 문자열**이다. UI는 이걸 x402의 `X-PAYMENT` bearer로 그대로 씀.

## 4.3 🎬 상태 머신 흐름

```
[POST /api/relayer/register-deposit]
       │ 1Click /v0/quote → depositAddress, deadline 받음
       │ Supabase insert
       ▼
┌──────────────────┐
│ PENDING_DEPOSIT   │  confirmed=false, x402_executed=false
│ (1Click status)   │  사용자에게 QR 표시
└────────┬─────────┘
         │ 사용자가 ZEC 송금 (PAL은 모름)
         │ 1Click이 체인에서 감지
         ▼
┌──────────────────┐
│   PROCESSING     │  1Click이 solver 통해 swap 실행 중
│ (1Click status)  │  PAL은 1분마다 폴링만
└────────┬─────────┘
         │ swap 완료 → USDC가 swapWallet에 도착
         │  ── 또는 ──
         │ INCOMPLETE_DEPOSIT: 입금액 부족
         │ REFUNDED: swap 실패, refundTo로 ZEC 반환
         │ FAILED: swap 실패
         ▼
┌──────────────────┐
│    SUCCESS       │  ◄── 💥 cronjob이 여기 감지하면 x402 실행
│ (1Click status)  │
└────────┬─────────┘
         │ signX402TransactionWithChainSignature() 호출 (STEP 6/7)
         │ → MPC 서명 → Base에 broadcast → Ethereum tx hash 받음
         ▼
┌──────────────────────────────┐
│ Supabase update              │
│   signed_payload = txHash    │
│   x402_executed = true       │
│   confirmed = true           │
└────────┬─────────────────────┘
         │
         ▼
[UI 폴링이 signed_payload 감지 → app/content/page.tsx로 redirect]
[content page가 tx hash를 X-PAYMENT 헤더로 paywall 해제]
```

## 4.4 ⏱️ "5초 vs 1분" 미스터리 해소

| 위치 | 주기 |
|---|---|
| **Vercel 배포 (실제 운영)** | `*/1 * * * *` = **1분** (`vercel.json:9`) |
| `scripts/run-cronjob.js` (로컬 개발 전용) | `INTERVAL_MS = 5000` = 5초 |
| DEPLOY.md / README 주장 | "every 5 seconds" ❌ |

> Vercel **Hobby tier의 cron 최소 주기가 1분**이라 5초는 불가능. README가 로컬 스크립트 동작을 운영 환경처럼 잘못 적은 것. **결제용으로 1분은 느린 편 — 우리 팀이 채택한다면 webhook 또는 SSE 전환 검토 필요.**

## 4.5 ⚠️ 진짜 무서운 함정들

### 1. **인증이 주석 처리되어 있다 (cronjob endpoint)**

```typescript
// app/api/relayer/cronjob-check-deposits/route.ts:17-21
// Optional: Add authentication/authorization check here
// const authHeader = request.headers.get('authorization')
// if (authHeader !== `Bearer ${process.env.CRON_SECRET}`) {
//   return NextResponse.json({ error: 'Unauthorized' }, { status: 401 })
// }
```

**누구나 cronjob을 임의 호출 가능 → 임의로 x402 실행을 트리거할 수 있음.**

### 2. **`/api/relayer/test-supabase`도 인증 없음**

누구나 production DB에 test row INSERT/DELETE 가능. 경쟁 조건 시 잔류 가능. Supabase 연결 정보(URL, service role 사용 여부) 노출.

### 3. **x402 실행 실패 + deadline 만료 = 자금 영구 분실**

- cronjob에서 `signX402TransactionWithChainSignature()` 실패하면
- `signed_payload`/`x402_executed` 안 채워짐
- 1분 후 다시 시도하지만 **deadline이 만료되면 그 deposit은 cron 순회 목록에서 빠짐**
- 사용자는 ZEC 보냈지만 콘텐츠 못 받음 + 환불 endpoint 없음
- **DEPLOY.md는 `POST /api/relayer/refund`가 있다고 주장하지만 그 route 파일 자체가 존재하지 않음**

### 4. **In-memory fallback은 Vercel에서 작동 안 함**

`SUPABASE_SERVICE_ROLE_KEY` 없으면 `Map<string, DepositTracking>`으로 fallback (`lib/depositTracking.ts:26`).
- Vercel serverless = invocation마다 새 프로세스 → **invocation 간 공유 안 됨**
- `register-deposit`에서 저장한 데이터를 `check-deposit`에서 못 읽음
- 사실상 fallback이 아니라 *"로컬 dev에서만 의미 있음"*

### 5. **Blind trust on 1Click `SUCCESS`**

- Zcash 체인 조회 코드 0줄 (lightwalletd, zebrad, 어떤 RPC도 없음)
- 1Click이 거짓 SUCCESS 반환하거나 침해되면 → PAL은 받지도 않은 ZEC에 대해 x402 실행
- 사용자가 submit하는 ZEC tx hash도 길이 ≥ 10만 체크 (`submit-tx-hash/route.ts:24`) — 유효성 검증 없음 (단, 이건 트리거가 아니라 1Click에 hint로만 전달됨)

### 6. **`payTo` 추론이 fragile**

```typescript
// cronjob-check-deposits/route.ts:85
const payTo = quote?.payTo || tracking.recipient || quote?.recipient
```

3단 fallback. `quote.payTo`가 1Click 응답에 항상 있다는 보장 없음. **잘못된 주소로 USDC 보낼 위험.**

### 7. **`deadline`이 두 군데에 있고 의미가 다름**

- `tracking.deadline` = 1Click quote 유효기한 (cron 필터용)
- x402 EIP-3009 `deadline` = `Math.floor(Date.now()/1000) + 3600` (cronjob이 즉석으로 +1시간 박음, `route.ts:88`)
- → 둘이 다른 시간축. 디버깅할 때 혼동 주의.

## 4.6 🎯 우리 프로젝트 관점

### ✅ Lift-and-use

- **Supabase 상태 머신 패턴 자체** — `(deposit_address PK) + (confirmed, x402_executed, signed_payload) + deadline filter`로 멱등성 보장하는 구조는 깔끔. 우리도 모방 가능.
- **partial index `WHERE confirmed = false AND deadline IS NOT NULL`** — cron 쿼리 비용 최적화 패턴
- **`quote_data JSONB` 통째 저장** — 외부 API 응답을 통째로 보관하는 audit 패턴 (디버깅에 유용)
- **swap_wallet_address indexing** — content page에서 EVM 주소 → deposit 역조회 패턴

### ❌ Redo

| 항목 | 우리는 어떻게 |
|---|---|
| **인증 주석 처리됨** | CRON_SECRET 등 처음부터 의무 (활성화) |
| **`test-supabase` 공개** | env=production이면 비활성화, 또는 admin만 |
| **환불 endpoint 미구현** | 명세에 있으면 처음부터 구현. x402 실행 실패 시 자동 환불 |
| **1Click blind trust** | 우리는 Zcash 직접 조회 (lightwalletd / Zebra RPC) |
| **1분 cron polling** | webhook 또는 server-sent events. 결제용으로 1분은 너무 느림 |
| **`signed_payload` 컬럼명** | `evm_tx_hash` 같이 정직한 이름. PAL은 오해 유발 |
| **`payTo` 3단 fallback** | 명시적 single source — intent에서 받은 그대로만 |
| **deadline 2개 의미** | 분리된 컬럼명 (`quote_deadline` vs `x402_deadline`) |
| **In-memory fallback** | 아예 제거. 환경변수 없으면 startup에서 fail-fast |

### 🟡 참고만

- **submit-tx-hash optional endpoint** — 우리가 Zcash 직접 조회하면 사용자 입력 필요 없음. 다만 UX 가속화 옵션으로 검토 가능

## 4.7-bonus 🧠 진짜 쉬운 설명 — "1Click → Supabase → cron → MPC 사인" 흐름

### 먼저 흔한 오해 정정

```
❌ 잘못된 모델
1Click이 swap 완료 → 자동으로 Supabase에 SUCCESS 반영
                  → cron이 Supabase 보고 SUCCESS 발견 → MPC 사인

✅ 실제 모델
1Click은 자기 내부 DB에만 SUCCESS 기록. PAL 시스템엔 안 알려줌.
PAL의 cron이 1분마다 1Click API에 "이 deposit 어떻게 됐어?" 라고 물음.
1Click이 "SUCCESS" 답하면, 그제서야 PAL이 Supabase에 SUCCESS + MPC 사인 진행.
```

→ 핵심: **1Click → Supabase 푸시는 없음.** PAL이 1Click한테 항상 pull. Supabase는 PAL 자체가 쓰는 거.

### 7단계로 쪼개기

1. **cron 깨어남 (1분마다)** — Vercel이 `/api/relayer/cronjob-check-deposits` 호출
2. **Supabase 조회** — *"deadline 안 지났고 x402 아직 실행 안 된 deposit 리스트 줘"*
3. **각 deposit마다 1Click한테 status 물음** — `getExecutionStatus(depositAddress)`
4. **`SUCCESS`인 것만 다음 단계** — 나머지는 다음 1분에 다시 확인
5. **NEAR MPC에 사인 부탁** — `signX402TransactionWithChainSignature()` 호출 (사인 2번 발생, 아래 참조)
6. **Base 메인넷에 broadcast** — MPC 서명으로 USDC `transferWithAuthorization` tx 쏨
7. **Supabase 업데이트** — `signed_payload`, `x402_executed`, `confirmed` 갱신. UI 폴링이 감지하고 콘텐츠 redirect

### 5단계 더 자세히 — NEAR MPC 사인이 진짜 어떻게?

**컨셉:** NEAR Chain Signatures = "NEAR 위에서 굴러가는 분산 키 보관소".
- 진짜 EVM 개인키 = **누구도 안 가짐.** MPC 노드들이 비밀 공유로 나눠 가짐
- 누가 사인 요청 가능 = NEAR 계정 + path 조합으로 권한 분리
- → **PAL은 EVM 개인키를 한 번도 다루지 않는다.** *키 없이 서명* 패턴

**PAL이 쓰는 식:**
```
NEAR 계정: anyone-pay.near
MPC path:  "base-1"   ◄── 모든 사용자가 공유하는 단일 path (하드코딩!)

이 두 개 조합으로 결정론적 EVM 주소 1개 파생
  → swapWallet = 0xABC...  (모든 PAL 사용자가 공유)
```

1Click swap 완료 시 USDC가 이 swapWallet으로 도착. swapWallet 키는 NEAR MPC가 분산 보관.

**💥 사인 2번 발생하는 이유:** EIP-3009 표준 따르려고 2단계 분리.

| 사인 # | 무엇을 사인하나 | 용도 |
|---|---|---|
| **#1 Authorization** | EIP-712 typed data ("swapWallet이 0xMERCHANT한테 10 USDC 보내는 걸 허가, nonce, expiry") | 원래 표준 x402의 `X-PAYMENT` 헤더 내용 자체 |
| **#2 Transaction** | 위 (v,r,s)를 데이터로 박은 EVM tx envelope (chainId, gas, to=USDC contract, data=transferWithAuthorization call) | Base 메인넷에 broadcast할 raw tx |

**왜 2번?** swapWallet이 *authorizer*면서 동시에 *submitter* (= broadcaster). 원래 x402에서는 사용자가 authorize, facilitator가 submit으로 나뉘는데, PAL은 facilitator 안 쓰니까 swapWallet 혼자 두 역할 → MPC 사인 2회.

**끝나면:**
```
[MPC 서명 2개]
   ↓ viem으로 raw tx 조립
publicClient.sendRawTransaction()
   ↓
Base 메인넷 → Ethereum tx hash 받음
   ↓
Supabase signed_payload 컬럼에 저장
   ↓
UI가 이 hash를 가맹점한테 X-PAYMENT로 전달 → paywall 해제
```

### 이 흐름에서 우리가 주목할 함정

| 포인트 | 의미 |
|---|---|
| **`MPC_PATH = 'base-1'` 하드코딩** | 모든 사용자가 같은 swapWallet 공유. A 사용자 USDC가 B 결제에 쓰일 가능성. per-user isolation = 0 |
| **MPC 사인 2번** | NEAR Chain Sig 호출 2회 → latency + NEAR gas 2배. EIP-3009 안 쓰면 1번으로 단축 가능 |
| **사인 #1은 원래 X-PAYMENT 그 자체** | 진짜 x402였다면 #1만 사인하고 facilitator로 보내면 끝. PAL은 facilitator가 없어 #2까지 필요 |
| **anyone-pay.near 단일 계정** | 모든 사인 요청이 1개 NEAR 계정에서 나감. NEAR access key 털리면 → swapWallet 전체 도용 가능 |

(상세한 NEAR Chain Sig 메커니즘은 [STEP 6](#step-6--near-chain-signatures-키-없이-evm-서명) 본편에서 다룸)

---

### 🧭 자주 헷갈리는 포인트 — "1Click이 swapWallet에 돈 옮긴 거야? NEAR는 어디 끼는데?"

**핵심: 체인이 3개라서 헷갈리는 거다.**

| 체인 | 역할 | 여기서 움직이는 자산 |
|---|---|---|
| **Zcash 체인** | 사용자가 ZEC 넣는 곳 | 사용자 ZEC → 1Click의 solver 지갑 |
| **NEAR 체인** | **사인만 해주는 서비스** (값은 안 움직임) | 자산 0. `v1.signer` 컨트랙트가 사인만 |
| **Base 체인** | USDC가 실제로 흐르는 곳 | 1Click solver → swapWallet → 가맹점 |

→ NEAR = "돈이 흐르는 길"이 아니라 "**사인 도장 찍어주는 출장소**".

**체인별로 누가 뭐 하는지:**

```
═══ Zcash ═══
[사용자] ──ZEC 10개──▶ [1Click 입금 주소]
                              │ 체인 밖에서 solver 매칭
                              ▼
═══ Base ═══
                      [1Click solver] ──USDC 10개──▶ [swapWallet 0xABC...]
                                                            │
                                                            │ 누구 키로 사인?
                                                            ▼
═══ NEAR ═══
                              [anyone-pay.near]
                                    │ v1.signer.sign(payload, path="base-1")
                                    ▼
                              [v1.signer MPC 노드들]
                                    │ 분산 서명 → (v,r,s) 반환
                                    ▼
═══ Base (다시) ═══
                              [PAL이 받은 서명으로 raw tx 조립 → Base broadcast]
                                    │
                                    ▼
                              [USDC 10개 → 0xMERCHANT 수령]
                              [tx hash 받음 → Supabase 저장 → UI 알림]
```

**가장 중요한 통찰 2개:**

1. **1Click이 USDC를 swapWallet으로 진짜 옮겨준다 ✅**
   "swap"의 결과는 swapWallet 주소에 USDC 도착. 사용자가 보낸 ZEC를 1Click solver가 가져가고, solver가 자기 USDC를 swapWallet에 보내는 환전소 모델. 끝난 시점에 swapWallet에 진짜 USDC 들어 있음 (Base 익스플로러로 잔고 조회 가능).

2. **NEAR는 자산을 한 톨도 안 움직인다 ❌**
   NEAR 역할 = "사인 찍어주는 인감 서비스". ZEC도 USDC도 NEAR 체인을 안 거침. NEAR는 오로지: ① PAL이 "사인해줘" 요청 → ② MPC 노드 분산 서명 → ③ `(v,r,s)` 반환.
   왜 그럴 권한이 있냐? **swapWallet의 EVM 주소 자체가 NEAR MPC로부터 파생된 것**이라서. 같은 secp256k1 곡선 → "NEAR 계정 + path = EVM 주소" 결정론적 매핑.

**보너스 — 가스비는?**
Base에 tx broadcast하려면 ETH 필요. swapWallet도 ETH가 있어야 함. PAL 코드에는 충전 로직 없음 — 운영자가 미리 채워두거나, 1Click이 swap 시 gas dust 같이 보내주거나. **production-ready 부족 포인트.**

**한 줄 정리:**
> "1Click이 Base 위에서 swapWallet에 USDC를 떨궈주고, NEAR는 그 swapWallet 키를 분산 보관하다가 PAL이 요청할 때 사인만 해준다. 자산은 Zcash와 Base에만 흐른다. NEAR는 자산 통로가 아니라 키 서비스."

---

### 🤔 자주 나오는 후속 질문 — "NEAR MPC 굳이 왜 써? 그냥 지갑 생성하면 안 되는 거?"

**답: 사실 그래도 됐고, PAL은 MPC의 장점을 거의 못 누리고 있다.**

**옵션 비교:**

| 항목 | 일반 지갑 (privKey in env) | NEAR MPC |
|---|---|---|
| 구현 복잡도 | ⭐ 매우 단순 | ⭐⭐⭐ 복잡 |
| 사인 latency | 즉시 (ms) | NEAR 라운드트립 (수 초) |
| 비용 | 무료 | NEAR gas (사인마다) |
| 키 보관 | 서버에 있음 | MPC 노드 분산 |
| 서버 털리면 | 💀 개인키·자금 전액 도용 | 😐 NEAR access key 안 털렸으면 안전 |
| NEAR access key 털리면 | (무관) | 💀 swapWallet 전체 도용 |
| 멀티체인 동시 | 각각 키 관리 | 같은 계정 + path만 변경 |
| Per-user 주소 | 사용자 수만큼 키 관리 | path만 user_id로 바꾸면 됨 |

**NEAR MPC가 진짜 빛나는 경우 (3가지 중 하나):**
1. **Per-user 지갑** — path = `user_id`로 사용자마다 다른 주소 파생, 백엔드 키 0개
2. **멀티체인 동시** — 같은 NEAR 계정으로 BTC/ETH/SOL 사인
3. **컴플라이언스/내러티브** — "회사는 키를 보관하지 않습니다"

**PAL이 셋 다 활용 못 한 이유:**
- `MPC_PATH = 'base-1'` 하드코딩 → per-user X
- Base만 실제 사용 → 멀티체인 X
- 해커톤 프로젝트 → 컴플라이언스 narrative 가치 낮음

**그럼 PAL은 왜 MPC를 썼나? (추정)**
1. **해커톤 트랙 요구사항** — NEAR 관련 트랙이면 MPC 안 쓸 수 없음 (가장 유력)
2. **Trustless 마케팅 포인트** — README narrative용
3. **미래의 per-user 확장 가능성을 열어둠** — 다만 마이그레이션 흔적은 없음

**PAL이 MPC 선택으로 손해 본 것:**
- 사인 한 번에 NEAR 호출 2회 (EIP-712 auth + EVM tx) → latency
- NEAR gas 비용
- NEAR 네트워크 의존 (NEAR 다운 → PAL 결제 멈춤)
- per-user를 못 쓰는데 MPC 비용은 다 지불 = **worst of both worlds**

**우리 팀의 선택지 3개:**

| 옵션 | 권장도 | 이유 |
|---|---|---|
| A. **일반 지갑 + KMS** | ⭐ 가장 단순/빠름 | 키 보관 자체가 차별화 포인트 아니면 충분 |
| B. **NEAR MPC를 진짜 활용 (per-user)** | path = `user_id_hash`로 PAL 못 한 걸 함 | 차별화 포인트, 단 latency+의존 부담 |
| C. **Zcash 직결제 → 중간 EVM 자체를 없앰** | ⭐⭐⭐ 가장 카테고리 E다운 | swapWallet도 EIP-3009도 불필요. 사인은 Zcash 지갑만 |

**한 줄 정리:**
> NEAR MPC는 per-user 지갑이나 멀티체인이 필요할 때 빛난다. PAL은 둘 다 안 써서 MPC 비용만 내고 이득은 거의 못 얻음. 그냥 지갑으로 했어도 동작했을 것. 우리는 "왜 MPC를 쓰는가?"에 명확한 답이 있어야 함 — "PAL이 썼으니까"는 답이 아님.

---

### 🔐 Per-user MPC가 진짜로 빛나는 시나리오

**케이스 A: 단일 shared swapWallet (PAL 현 디자인)**
```
[사용자 A] ──ZEC──┐
[사용자 B] ──ZEC──┼──▶ [1Click] ──▶ [swapWallet 0xABC] ◄── 모든 USDC 한 통에 혼재
[사용자 C] ──ZEC──┘                        잔고: 1만 명 USDC fungible mix
```

**케이스 B: Per-user swapWallet (path = user_id 또는 intent_id)**
```
[사용자 A] ──ZEC──▶ [swapWallet_A 0xAA1...] ── 오직 A USDC
[사용자 B] ──ZEC──▶ [swapWallet_B 0xBB2...] ── 오직 B USDC
[사용자 C] ──ZEC──▶ [swapWallet_C 0xCC3...] ── 오직 C USDC
```

**Per-user의 장점:**

| 항목 | 단일 shared | per-user |
|---|---|---|
| 사용자 간 자금 격리 | ❌ 섞임 | ✅ 완전 분리 |
| Race condition | ⚠️ 가능 | ✅ 차단 |
| 환불 | "잔고 중 A 몫" oracle 의존 | "swapWallet_A 통째로 A에게" 단순 |
| 공격 꿀단지 | 단일 점에 모임 | 분산 |
| 회계·세무 | 1Click 응답 사후 추정 | 주소가 곧 사용자 매핑 |
| KYC/AML | 입출금 분리 어려움 | 주소별 사용자 ID 자명 |

**MPC vs 일반 지갑 (per-user 시나리오에서):**

- **일반 지갑 per-user:** 사용자 1만 명 = privKey 1만 개. DB/HSM 보관 부담. backup/rotation 운영 지옥
- **MPC per-user:** 백엔드 키 0개. user_id 추가 = path 문자열 추가. 털릴 거 자체가 없음

**Per-user MPC 단점:**

| 단점 | 설명 |
|---|---|
| 각 swapWallet에 ETH 충전 필요 | 새 사용자마다 Base ETH dust 필요 |
| NEAR gas + 1 NEAR deposit per sign | 사용자/결제 수 비례 비용 증가 |
| NEAR access key 단일 장애점 유지 | path 분리해도 NEAR proxy key 털리면 전부 |
| NEAR network 의존 | 다운 시 격리 의미 없음 |

**한 줄:**
> 일반 지갑은 사용자 수만큼 키 보관 부담 선형 증가. MPC + per-user path는 백엔드 키 0개. 사용자별 격리 + 운영 부담 최소화 = per-user MPC의 정체성. **PAL은 이걸 안 해서 MPC 쓰는 의미가 거의 사라짐.**

---

### 🔬 NEAR MPC 동작 원리 (STEP 6 preview)

**큰 그림:**
```
[PAL 서버] ──functionCall(v1.signer, "sign", payload, path)──▶ [NEAR 메인넷]
                                                                    │
                                                                    ▼
                                                          [v1.signer 컨트랙트]
                                                                    │ broadcast
                                                                    ▼
                                                          [MPC 노드 N개 (NEAR 밸리데이터)]
                                                                    │ threshold 부분 사인 결합
                                                                    ▼
                                                          [(big_r, s, recovery_id)]
                                                                    │ NEAR 응답
                                                                    ▼
[PAL이 (v,r,s) 조립 → Base raw tx broadcast]
```

**Epsilon Derivation (주소 파생 공식):**
```
scalar epsilon = sha3_256("near-mpc-recovery v0.1.0 epsilon derivation:"
                          + accountId + "," + path)
child_pubkey   = master_pubkey + epsilon * G   (secp256k1 point 덧셈)
EVM 주소       = keccak256(child_pubkey_uncompressed)[12:]
```

- 같은 `(accountId, path)` → 항상 같은 EVM 주소 (결정론적)
- 개인키 = 어디에도 없음. MPC 노드가 share만 보유

**PAL이 사인 요청하는 12단계 실제 흐름:**

```
[1]  NEAR 계정 로드 (env에서 privKey)
[2]  MPC_PATH = 'base-1' (하드코딩)
[3]  swapWallet 주소 파생 (epsilon derivation via chainsig.js SDK)
[4]  EIP-712 도메인+타입+값 구성 (USDC, swapWallet→merchant)
[5]  ⭐ MPC 사인 #1 (EIP-712 해시) — 5~30초 + 1 NEAR + 300 TGas
[6]  ecrecover 검증 — swapWallet 일치 확인
[7]  transferWithAuthorization calldata 인코딩
[8]  Legacy EVM tx 준비 → RLP 해시
[9]  ⭐ MPC 사인 #2 (tx 해시) — 또 5~30초 + 또 1 NEAR + 300 TGas
[10] v,r,s 삽입 + RLP 인코딩
[11] viem publicClient.sendRawTransaction → Base broadcast
[12] tx hash 반환 → Supabase signed_payload 저장
```

**디테일 5가지 (꼭 알아야 할):**

1. **사인 1회 = 5~30초.** 2회 = 10~60초. cron 1분 주기와 동급
2. **사인마다 1 NEAR + 300 TGas.** 결제 1건당 2 NEAR. 소액 결제에 부담
3. **`NEAR_PROXY_PRIVATE_KEY`가 env 평문** — MPC 보호의 한계. 이게 보안 경계
4. **`lib/kdf.ts` (TypeScript 구현) vs `chainsig.js` SDK** 이중 존재. production은 SDK
5. **`lib/near.ts`도 별도 legacy** — `account.functionCall` 직접 호출 방식. chainSig.ts와 공존

**우리 팀 카테고리 E 체크리스트 (MPC 쓸 거면):**
- [ ] Per-user path 설계 (`user_id` or `intent_id` 해시)
- [ ] NEAR proxy key를 KMS/HSM에 (env 평문 금지)
- [ ] 사인 1회로 단축 가능한지 검토 (EIP-3009 불필요면 #1 생략)
- [ ] NEAR 다운 fallback 정책
- [ ] 사인 latency를 UX와 맞춤 (30초 로딩 처리)
- [ ] NEAR gas 비용 회계 transparent

**또는 — 그냥 일반 지갑 + KMS:** Zcash 결제 자체가 차별화면 EVM 사인 인프라는 단순한 게 나음.

## 4.7 🚀 우리 차별화 포인트

### 1. **체인 직접 조회 (Trust minimization)**
- PAL: 1Click blind trust
- 우리: **lightwalletd로 viewing key 기반 입금 확인** → 1Click 안 거치고도 검증 가능
- 또는 Zebra RPC로 transparent 입금 확인 (transparent 케이스)

### 2. **Webhook / push 모델**
- PAL: 1분 outbound polling
- 우리: lightwalletd가 새 블록 알려주는 streaming RPC + 즉시 처리 → **latency 1분 → 수 초**

### 3. **환불 자동화**
- PAL: 실패 시 영구 분실, 환불 endpoint 미구현
- 우리: x402 실행 실패 시 자동으로 sender ZEC로 환불 트랜잭션 트리거. **failure-mode를 처음부터 설계**

### 4. **메모 기반 idempotency**
- PAL: `deposit_address PK`로 idempotency
- 우리: **Zcash memo에 x402 challenge nonce 박아서 cross-validate** → on-chain proof 자체가 멱등키

### 5. **다중 deposit 지원**
- PAL: 한 사용자 한 deposit 가정
- 우리: viewing key로 같은 주소에 여러 입금을 받고 메모로 구분 가능 → batching

---

---

# STEP 5 — 1Click bridge (Defuse Labs Limited, 외주의 실체)

> **한 줄 요약:** PAL이 *"Zcash 프로젝트"*라고 우길 수 있게 해준 모든 무거운 일을 대신 해주는 **Gibraltar 소재 회사(Defuse Labs Limited)의 중앙화 REST API**. 사용자 ZEC를 받고 → 자기 solver 네트워크로 swap 실행 → 우리 swapWallet에 USDC를 떨궈줌. **여기가 카테고리 E 차별화의 핵심 — 진짜 카테고리 E면 이걸 빼야 한다.**
>
> **deep dive:** [`05-one-click-bridge.md`](./subsystems/05-one-click-bridge.md) + [`zcash-tool-inventory.md` §3.1](./zcash-tool-inventory.md)

## 5.1 1Click이 진짜로 뭐야?

| 항목 | 내용 |
|---|---|
| **공식 명칭** | 1Click Swap API (= 1CS) |
| **운영 법인** | **Defuse Labs Limited** |
| **법인 설립지** | **Gibraltar** (지브롤터, 영국령 자치 지역) |
| **API 도메인** | `https://1click.chaindefuser.com` |
| **상위 프로토콜** | [NEAR Intents](https://docs.near-intents.org/) |
| **온체인 settlement** | `intents.near` 스마트 컨트랙트 (NEAR 메인넷) |
| **거래 모델** | Solver/Market Maker 경쟁 입찰 (off-chain) → NEAR settlement (on-chain) |
| **준거법** | Gibraltar 법 |
| **최대 배상 책임** | **USD $100** (Terms of Service 명시, "AS IS" 제공) |
| **AML 스크리닝** | TRM Labs, Binance AML, AMLBot, PureFi (모든 quote 요청 자동 스크리닝) |
| **법 집행 협조 채널** | [Kodex Global](https://app.kodexglobal.com/nearintents/signin) (당국 요청 응답 체계 구축됨) |

> ⚠️ **이게 결정적이에요.** PAL은 *"Zcash 결제 앱"*을 표방하지만, ZEC 처리는 Gibraltar 법인이 운영하는 중앙화 API에 100% 위임됨. 그 회사는 **모든 거래에 AML 스크리닝을 돌리고 법 집행에 응답하는 체계**를 갖추고 있음.

## 5.2 NEAR Intents가 뭐고, 1Click이랑 어떻게 다른가?

```
┌────────────────────────────────────┐
│  NEAR Intents                       │  ← 프로토콜 (오픈 표준)
│  - intents.near 스마트 컨트랙트       │
│  - Market Maker 경쟁 입찰 메커니즘    │
│  - atomic settlement                │
└────────────────────────────────────┘
              ▲
              │ 이 위에 구축됨
              │
┌────────────────────────────────────┐
│  1Click Swap API (= 1CS)            │  ← 상품 (Defuse Labs Limited 운영 REST API)
│  - "사용자가 단일 API 호출로 swap"   │
│  - depositAddress 발급              │
│  - solver 네트워크 매칭              │
│  - AML/KYC/법 집행 응답              │
└────────────────────────────────────┘
              ▲
              │ 호출
              │
        [PAL 같은 앱]
```

→ **NEAR Intents = 프로토콜**, **1Click = 그 위에 올린 상품 (한 회사가 운영)**. PAL은 1Click(상품) 레이어를 호출하고, intents.near(프로토콜) 레이어와는 직접 상호작용 안 함.

## 5.3 PAL이 실제로 호출하는 1Click API 3개

| API | PAL 호출 위치 | PAL의 가정 |
|---|---|---|
| **`POST /v0/quote`** (raw fetch) | `lib/oneClick.ts:102` → `register-deposit/route.ts:55` | 응답에 `depositAddress` 필드 있음. 그 주소로 들어온 ZEC를 1Click이 알아서 처리. |
| **SDK `getExecutionStatus(depositAddress)`** | `lib/oneClick.ts:141` → cron, UI 폴링, content unlock 다 사용 | `.status \|\| .executionStatus \|\| .state` 중 하나에 SUCCESS 들어 있을 거임 (SDK 응답 타입 불확실해서 3단 fallback) |
| **SDK `submitDepositTx({txHash, depositAddress})`** | `lib/oneClick.ts:155` → `submit-tx-hash/route.ts:34` | 사용자가 제출한 ZEC tx hash를 1Click에 알려서 swap 가속화 (optional) |

→ 이게 전부. 4번째 호출 없음. **이 3개를 우리가 대체할 수 있어야 1Click 의존을 벗어남.**

## 5.4 🎬 흐름 (PAL 관점에서)

```
[PAL] ──POST /v0/quote──▶ [1Click]
  body: {
    originAsset:     "nep141:zec.omft.near",         ◄── ZEC (NEAR-wrapped)
    destinationAsset:"nep141:base-0x833589...omft.near", ◄── USDC on Base
    amount:          "10000000",                      (10 USDC, 6 decimals)
    recipient:       "<swapWallet — MPC 파생 EVM>",   ◄── USDC 받을 EVM 주소
    refundTo:        "<sender ZEC 주소>",             ◄── swap 실패 시 ZEC 반환처
    deadline:        "<+3분>",
    slippageTolerance: 100,                           (1%)
    swapType:        "EXACT_OUTPUT",                  ◄── USDC 고정, ZEC 변동
  }
                              │
                              ▼ 응답
  { depositAddress: "t1Qc...",  ◄── 💥 transparent t-address (shielded 아님!)
    swapId:         "...",
    quote: {        amountInFormatted, deadline, ... } }

[PAL이 depositAddress를 사용자에게 QR로 표시]
[사용자가 ZEC 송금 → 1Click solver wallet이 받음]

  ┌─── 사용자 모르는 곳 ──────────────────────────────┐
  │ 1Click solver 네트워크가 경쟁 입찰                │
  │ Market Maker 선정 → intents.near에서 atomic 정산 │
  │ Token Bridge로 USDC를 Base에 전달                │
  └──────────────────────────────────────────────────┘

[1Click] ──USDC 10개──▶ [swapWallet on Base]

[PAL cron] ──getExecutionStatus(t1Qc...)──▶ [1Click]
                              ◄── { status: "SUCCESS" }
[PAL이 NEAR MPC 사인 → x402 실행]
```

## 5.5 💥 가장 중요한 발견 4가지

### 1. **1Click은 transparent t-address만 지원** (shielded 안 됨)

공식 문서 원문:
> ⚠️ Partially supported — Transparent addresses only
> Address Types: Transparent — `t1` or `t3` prefix

→ **README의 *"shielded transactions hide amounts, sender, and recipient"*는 L1에서 false.** transparent t-주소는 Zcash 익스플로러에서 누구나 sender·amount·recipient 다 볼 수 있음.

### 2. **Privacy 붕괴 2단**

| 실패 지점 | 노출 정보 |
|---|---|
| **L1 (Zcash 블록체인)** | 사용자가 t1/t3 주소로 송금 → 익스플로러에서 sender·amount·to 다 공개 |
| **API (1Click /v0/quote)** | 단일 요청에 `refundTo`(sender ZEC) + `recipient`(EVM 주소) 함께 담김 → **1Click이 sender ↔ EVM 연결 다 봄** + AML 스크리닝 |

→ "프라이버시"는 기술적 보장이 아니라 **Defuse Labs에 대한 법적 신뢰**에만 의존.

### 3. **수수료 — README 거짓말**

| 출처 | 주장 |
|---|---|
| PAL README | "JWT 없이 0.1%" |
| 공식 문서 | **JWT 없이 0.2%** (+ ZEC 출금 0.1% 별도) |
| JWT 발급 시 | 0.0001% (= 1 pip; partner dashboard에서 발급) |

→ JWT 없으면 사용자 결제액의 ~0.3% 정도가 1Click에게 흘러감.

### 4. **법적 책임 = $100 캡, AML/법 집행 응답 체계 운영 중**

- Terms of Service: 최대 배상 책임 **USD $100**, AS-IS 제공
- AML 스크리닝: 모든 quote 요청 자동 스크리닝 (TRM Labs 등 4개)
- 법 집행 포털: Kodex Global을 통해 당국 요청에 응답

→ **현실적으로 1Click은 "프라이버시 도구"가 아니라 "AML 컴플라이언트 swap 서비스"**.

## 5.6 ⚠️ PAL이 1Click을 호출하면서 만든 footguns

### 1. **`as any`로 SDK 응답 타입 우회**
```typescript
const status = (statusResponse as any).status
            || (statusResponse as any).executionStatus
            || (statusResponse as any).state
            || 'PENDING_DEPOSIT'
```
SDK 응답 타입이 명확하지 않아 3단 fallback. **SDK 버전 업그레이드 시 silent break 위험.**

### 2. **base URL 불일치 — README vs 코드**
- README/DEPLOY.md: `https://api.1click.fi` ← 코드에 없음
- 실제 코드: `https://1click.chaindefuser.com` (default)
- `ONE_CLICK_API_URL` env로 override 가능

### 3. **모든 swap 경로가 ZEC → USDC로 고정**
`originAsset = 'nep141:zec.omft.near'`가 하드코딩 (`bridgeFrom: 'zcash'`와 일관). 다른 origin asset 지원 코드 없음.

### 4. **PAL은 `recipient` 응답 검증 안 함**
1Click이 USDC를 실제로 swapWallet에 보냈는지 PAL 코드에서 독립 확인 없음. **1Click이 거짓 SUCCESS 보고하면 PAL은 받지도 않은 USDC로 x402 실행.**

### 5. **`INCOMPLETE_DEPOSIT` 처리 없음**
사용자가 quote 금액보다 적게 ZEC 보내면 `INCOMPLETE_DEPOSIT` 상태로 됨. PAL은 감지만 하고 **사용자 알림·재입금 안내·자동 환불 다 없음.** 자금 limbo 상태.

### 6. **`/v0/quote` deadline = 3분 (하드코딩)**
사용자가 3분 안에 ZEC 송금 못 하면 swap 만료. **Zcash 트랜잭션 confirmation 시간 고려하면 빠듯.**

## 5.7 🎯 우리 프로젝트 관점

### ✅ Lift-and-use (만약 우리도 외주 갈 거면)

- **Thin client 패턴** — `lib/oneClick.ts`의 호출 구조 (raw fetch + SDK 혼용)는 깔끔. 다른 swap aggregator API(예: LiFi, Squid) 붙일 때 모방 가능
- **`refundTo` 파라미터 패턴** — 실패 시 환불 주소를 처음부터 명시하는 건 좋은 습관

### ❌ Redo / 완전히 다른 길

**카테고리 E의 본질은 "Zcash를 진짜 결제 자산으로 쓴다"인데, 1Click 의존이 이걸 무력화함.** 우리가 진짜로 Cat E면 1Click을 빼고:

| 1Click이 했던 것 | 우리가 어떻게 대체할까 |
|---|---|
| Zcash deposit address 발급 | **자체 Zcash 지갑 인프라** (ZIP-32 + Sapling/Orchard) |
| ZEC 입금 감지 | **lightwalletd 또는 Zebra RPC 자체 운영** |
| ZEC → USDC 환전 | **선택:** ① 환전 자체를 안 함 (ZEC가 settlement asset) ② 자체 DEX/AMM 통합 ③ 1Click을 *선택적으로* 사용하되 사용자가 명시 동의 |
| AML 스크리닝 | 우리 책임. 컴플라이언스 정책 명확화 |
| ZEC 일시 custody | 사용자 wallet에 직접 (non-custodial) |

### 🟡 참고만

- **Solver/Market Maker 경쟁 모델** — 시장 가격 효율성에 좋음. 우리가 자체 swap 만들면 이 패턴 차용 가능
- **NEAR Intents `intents.near`** — settlement 레이어로 활용 검토 가능 (다만 NEAR 의존 추가)

## 5.8 🚀 우리 차별화 포인트 (Cat E의 진짜 본질)

PAL이 1Click을 쓰는 순간 다음이 다 사라진다:
- ❌ Shielded 결제 (transparent만 가능)
- ❌ Trust minimization (Defuse Labs 신뢰 필요)
- ❌ AML-free flow (모든 거래 스크리닝됨)
- ❌ 송신자-수신자 unlinkability (단일 API 요청에 같이 담김)

**우리가 진짜 Cat E면 — 1Click을 빼고:**

1. **Native Zcash shielded settlement** — sender도 receiver도 amount도 L1에서 비공개. 진짜 프라이버시
2. **No custody, no third party** — solver 없이 사용자가 직접 결제. 1Click 같은 중개자가 못 봄
3. **Self-hosted lightwalletd** — 우리만의 Zcash 노드. 사용자별 viewing key만 보관
4. **메모 기반 x402 challenge** — Zcash memo 필드 (= 512 bytes encrypted)에 x402 challenge nonce를 넣어 **on-chain proof 자체가 결제 증명**
5. **Optional 1Click 통합** — 사용자가 *"USDC로 받고 싶다"* 명시 선택 시에만, 명확한 trust trade-off 안내 후

→ **이게 진짜 Category E 차별화의 핵심.** PAL과 우리의 가장 큰 갈림길이 바로 여기.

## 5.9 한 줄 정리

> **"PAL의 1Click 의존은 'Zcash 프로젝트'가 아니라 'Gibraltar 회사가 운영하는 transparent address 기반 ZEC→USDC swap 서비스를 외주한 프로젝트'다. 진짜 Category E를 하려면 1Click을 빼고 native Zcash + 자체 lightwalletd로 가야 한다. 그게 우리의 가장 큰 차별화 포인트."**

---

---

# STEP 6 — NEAR Chain Signatures (키 없이 EVM 서명)

> **한 줄 요약:** "EVM 개인키를 아무도 갖지 않으면서" Base 메인넷에 USDC 트랜잭션을 서명하는 메커니즘. NEAR 위에 올라간 **threshold signature 서비스(`v1.signer`)**에 PAL이 사인을 부탁하면, MPC 노드들이 분산 서명해서 `(v, r, s)` 돌려줌. **카테고리 E에서 lift-and-use 잠재력이 가장 큰 서브시스템.**
>
> **deep dive:** [`06-near-chain-signatures.md`](./subsystems/06-near-chain-signatures.md)

> ⚠️ STEP 4.7-bonus + NEAR MPC 후속 질문에서 핵심은 이미 다뤘음. 이 STEP 6은 **그 위에 깊이를 더하는 본편** — MPC 이론, epsilon 수식의 의미, 실제 데이터 흐름, 3개 코드 경로의 관계.

## 6.1 "MPC가 진짜 뭐야?" — 한 단락 개념

**MPC (Multi-Party Computation) = 여러 참여자가 각자 비밀을 가지고 있을 때, 그 비밀을 합치지 않고도 함수를 함께 계산하는 암호 기법.**

**Threshold Signature (MPC의 응용):**
- 개인키 `sk`를 `N`개 조각으로 쪼개서 노드 `N`명에게 분배 (Shamir Secret Sharing 등)
- 그 중 `t`개 이상이 협력하면 사인 가능, `t-1`개로는 불가능
- 사인 과정에서 **`sk` 전체가 한 곳에 모이는 순간이 없음**
- 결과는 **일반 ECDSA 서명**과 구분 불가능 (EVM이 그대로 검증)

**즉:** "EVM 입장에서는 평범한 사인이지만, 사인 만드는 쪽은 분산"인 마법.

## 6.2 NEAR Chain Signatures 구조

```
                ┌─────────────────────────────────────────┐
                │   NEAR Protocol 메인넷                    │
                │                                          │
                │   v1.signer 컨트랙트                      │
                │     - 사용자 사인 요청 receive             │
                │     - MPC 노드들에게 broadcast            │
                │     - 응답 모아 결합                      │
                │                                          │
                │   MPC 노드 (N개)                          │
                │     - 각자 master_sk의 share 보유         │
                │     - 사인 요청 시 부분 사인 생성          │
                │     - threshold (t) 만큼 모이면 결합 가능 │
                └─────────────────────────────────────────┘
                              ▲
                              │ NEAR functionCall
                              │ + 300 TGas + 1 NEAR deposit
                              │
                              ▼
                ┌─────────────────────────────────────────┐
                │   사용자 (PAL 서버)                       │
                │     - NEAR 계정 access key 보유           │
                │     - "이 payload, 이 path로 사인해줘"    │
                │     - 응답: (big_r, s, recovery_id)     │
                └─────────────────────────────────────────┘
```

**핵심 시스템 키:**
- **`master_pk` (master public key)** — MPC 네트워크 전체가 공유하는 단일 공개키. 메인넷에서 모든 사용자가 같은 master_pk를 보고 자기 child key 파생
- **`master_sk`** — 어디에도 없음. 노드들이 share만 보유

## 6.3 Epsilon Derivation 깊게 — 왜 결정론적이고 안전한가

### 공식 다시
```
epsilon = sha3_256("near-mpc-recovery v0.1.0 epsilon derivation:"
                   + accountId + "," + path)
child_pubkey = master_pubkey + epsilon * G       (secp256k1 점 덧셈)
EVM 주소 = keccak256(child_pubkey_uncompressed[1:])[12:]
                              ↑ 04 prefix 제거
```

### 왜 이게 작동하는가? (BIP32 비슷한 원리)

타원곡선의 성질:
- 점 A에 스칼라 k를 더하면 (= `A + k*G`), 새 점이 나옴
- 만약 A에 대응하는 sk가 `a`였다면, 새 점에 대응하는 sk는 `a + k` (mod n)
- 즉, **(master_sk, master_pk) → (master_sk + epsilon, master_pk + epsilon*G)**

→ **MPC 노드들은 자기가 가진 share에 epsilon만 더해서 child key share를 만들 수 있다.** epsilon은 공개 정보(sha3_256 결과)니까 모두가 같은 값을 계산 가능. master_sk를 모이지 않고도 child sk로 사인 가능.

### 왜 안전한가?

- `epsilon`은 `accountId`와 `path`에 묶여 있음 (sha3_256 입력)
- 다른 NEAR 계정이 같은 `path = "base-1"`을 써도 **다른 child 주소** 파생
- → `anyone-pay.near` + `"base-1"`로 파생된 swapWallet에 사인할 수 있는 건 **`anyone-pay.near` access key를 가진 사람만**

→ NEAR 계정 access key가 사실상 인증 토큰. 사인 요청 시 v1.signer가 `request.caller == anyone-pay.near` 확인.

### 예시 데이터

```
accountId  = "anyone-pay.near"
path       = "base-1"

epsilon = sha3_256("near-mpc-recovery v0.1.0 epsilon derivation:anyone-pay.near,base-1")
        = 0x47a2... (32 bytes)

master_pk = (Mx, My)  ← MPC 네트워크 공개키, 메인넷 고정
child_pk  = (Mx, My) + epsilon * G  = (Cx, Cy)

uncompressed = 0x04 || Cx (32B) || Cy (32B)
hash         = keccak256(uncompressed[1:])
EVM 주소     = "0x" + hash[12:32]  ← 마지막 20 bytes
              = "0xABC1234..."     ← 이게 swapWallet
```

## 6.4 두 사인의 정확한 차이 — 데이터까지

### 사인 #1: EIP-712 Authorization Hash

**무엇을 사인하나:**
EIP-712 typed data → 다음 구조를 keccak256으로 해시한 32 bytes.

```typescript
domain = {
  name: 'USD Coin',
  version: '2',
  chainId: 8453,                                  // Base mainnet
  verifyingContract: '0x833589fcd6edb6e08f4c7c32d4f71b54bda02913',  // USDC on Base
}
types = {
  TransferWithAuthorization: [
    { name: 'from',         type: 'address' },
    { name: 'to',           type: 'address' },
    { name: 'value',        type: 'uint256' },
    { name: 'validAfter',   type: 'uint256' },
    { name: 'validBefore',  type: 'uint256' },
    { name: 'nonce',        type: 'bytes32' },
  ]
}
value = {
  from:        swapWallet,                     // 0xABC...
  to:          quote.payTo,                    // 가맹점 EVM 주소
  value:       10_000_000n,                    // 10 USDC (6 decimals)
  validAfter:  0,
  validBefore: deadline,                       // Unix timestamp
  nonce:       hexZeroPad('0x...', 32),
}

hash = ethers.utils._TypedDataEncoder.hash(domain, types, value)
     = 0x71b2...   (32 bytes)
```

**왜 이 사인이 필요한가:**
EIP-3009 표준에 따르면, USDC 컨트랙트의 `transferWithAuthorization()`은 `(from, to, value, ..., v, r, s)`를 받아 **on-chain에서 `ecrecover(hash, v, r, s) == from`을 검증**한다. 이 사인이 검증 통과해야 USDC가 실제로 옮겨짐. → **EVM 컨트랙트가 검증할 사인.**

### 사인 #2: Legacy EVM Transaction Hash

**무엇을 사인하나:**
실제 broadcast할 트랜잭션의 RLP 인코딩의 keccak256 해시.

```typescript
tx = {
  to:       '0x833589...',                     // USDC contract
  value:    0n,
  data:     iface.encodeFunctionData('transferWithAuthorization', [
              from, to, value, validAfter, validBefore, nonce,
              v_sig1, r_sig1, s_sig1                            // ◄── 사인 #1 결과 박힘
            ]),
  gasPrice: 100_000_000n,                       // 0.1 gwei
  gasLimit: 150_000n,
  nonce:    swapWalletNonce,                    // Base 위 swapWallet의 nonce
  chainId:  8453,
}

rlpEncoded = RLP.encode([tx.nonce, tx.gasPrice, tx.gasLimit, tx.to,
                         tx.value, tx.data, tx.chainId, 0, 0])
txHash     = keccak256(rlpEncoded)              // 32 bytes
```

**왜 이 사인이 필요한가:**
EVM 노드(Base 메인넷)가 raw transaction을 받아서 `ecrecover(txHash, v, r, s) == tx.from`을 검증해야 처리해줌. → **EVM 노드 자체가 검증할 사인.**

### 비교 표

| | 사인 #1 (Authorization) | 사인 #2 (Transaction) |
|---|---|---|
| 무엇을 해시하나 | EIP-712 typed data | RLP-encoded EVM tx |
| 검증 주체 | USDC 컨트랙트 (`ecrecover` 내부) | Base 노드들 (블록 포함 전) |
| 결과물 | (v,r,s) → tx data에 박힘 | (v,r,s) → tx envelope 마지막 |
| 표준 | EIP-712 / EIP-3009 | EIP-155 legacy |
| 왜 필요? | 가맹점에 X-PAYMENT bearer로 전달할 *증명* | 실제로 USDC를 옮기는 *broadcast* |

→ **#1은 "데이터 사인", #2는 "트랜잭션 사인".** 같은 swapWallet 키로 둘 다 사인하지만 의미가 다름.

## 6.5 PAL 코드의 3가지 경로 (혼란 정리)

PAL 레포에 NEAR Chain Sig 관련 코드가 **세 곳**에 있음. 이게 헷갈리니까 정리.

| 파일 | 역할 | production 사용? |
|---|---|---|
| `lib/chainSig.ts` | **메인 경로** — `chainsig.js@1.1.14` SDK 사용 | ✅ (cronjob → x402 실행 시 호출) |
| `lib/kdf.ts` | epsilon derivation 직접 구현 (pure TS) | ❌ (legacy, reference. SDK가 같은 일 함) |
| `lib/near.ts` | `account.functionCall('v1.signer', 'sign', ...)` 직접 호출 | ❌ (legacy. chainsig.js로 대체됨) |

→ **production은 `lib/chainSig.ts` + chainsig.js SDK 경로만.** `lib/kdf.ts`는 알고리즘을 직접 보고 싶을 때 reading material. `lib/near.ts`는 SDK 도입 전 코드 잔재.

## 6.6 ecrecover 검증 — PAL이 한 번 더 확인하는 단계

사인 #1 직후 PAL은 검증:

```typescript
// lib/chainSig.ts:283-300
recoveredAddress = ethers.utils.recoverAddress(eip712Hash, { r, s, v })
if (recoveredAddress.toLowerCase() !== swapWallet.toLowerCase()) {
  throw new Error('Signature recovery mismatch')
}
```

**왜 중요한가:**
- v 값(recovery_id 0 또는 1 → 27 또는 28 변환)이 잘못 박히면 `ecrecover`가 엉뚱한 주소를 돌려줌
- → **on-chain broadcast 후 reverts 발생 = NEAR gas + Base gas 모두 낭비**
- 사전 검증하면 throw 후 NEAR gas만 손해. cron이 다음 1분에 재시도

→ 이게 **silent on-chain failure 방지** 안전망. 우리도 lift-and-use 시 모방 필수.

## 6.7 Trust model 깊이 — "분산"의 진짜 의미

### 보호되는 것 ✅
- Base 위의 swapWallet `swapWallet_sk` — **누구도 보유 안 함**
- MPC 노드 1개 침해로는 사인 못 만듦 (threshold `t`개 필요)

### 보호되지 않는 것 ❌
- **`NEAR_PROXY_PRIVATE_KEY`가 서버 env에 평문 저장** (`lib/chainSig.ts:29`)
- 이 키 = `anyone-pay.near` access key
- 이 키 털리면:
  - 공격자가 v1.signer에게 임의 payload 사인 요청 가능
  - 모든 path 사용 가능 (예: `base-1` 외 다른 path도)
  - **swapWallet에서 임의 주소로 USDC 유출**

→ **MPC의 분산 보안이 의미를 가지려면 NEAR access key 보안이 동등하게 엄격해야 함.** PAL은 env 평문이라 사실상 이 보안 경계가 무너진 상태.

### 결론
PAL의 MPC 사용은 *"이론적으로는 EVM 키 없음, 실질적으로는 NEAR 키 1개가 단일 장애점"*. 진짜 분산 보안을 누리려면:
- NEAR access key를 HSM/KMS에 보관
- 또는 access key를 limited function-call key로 제한 (path 화이트리스트)

## 6.8 성능과 비용 (실제 수치)

| 항목 | 값 | 비고 |
|---|---|---|
| MPC 사인 1회 latency | **5~30초** | `lib/near.ts:71` 주석 명시 |
| 전체 결제 (사인 2번) | **10~60초** | cron 1분 주기와 거의 동급 |
| 사인당 NEAR gas | **300 TGas** | `lib/near.ts:116` (`3 * 10^14`) |
| 사인당 NEAR deposit | **1 NEAR** | `lib/near.ts:59` (`parseNearAmount('1')`) |
| 결제 1건당 NEAR 비용 | **2 NEAR** + 600 TGas | 사인 2회 합산 |
| Base gas price (하드코딩) | **0.1 gwei** | `lib/chainSig.ts:71` |
| Base gas limit | **150,000** | `lib/chainSig.ts:351` (USDC transferWithAuthorization 표준 비용) |

→ 사용자 결제 $10에 대해 NEAR 비용 ~$5-10 수준일 수 있음 (NEAR 시세에 따라). **소액 결제에는 명백히 비효율.**

## 6.9 🎯 우리 프로젝트 관점

### ✅ Lift-and-use (NEAR MPC 쓸 거면)

- **`chainsig.js` SDK 사용 패턴 자체** — `ChainSignatureContract` + `EVM chain adapter` 조합은 깔끔
- **ecrecover 사전 검증 (`lib/chainSig.ts:283-300`)** — silent failure 방지. 필수 모방
- **dynamic import (`await import('@/lib/chainSig')`)** — cron route에서 cold start 비용 줄이는 패턴
- **두 사인 분리 구조** — EIP-3009 쓸 거면 동일하게 분리해야 함
- **에러 격리 (개별 deposit failure → catch + continue)** — cron이 다른 deposit까지 안 멈춤

### ❌ Redo

| 항목 | 우리는 어떻게 |
|---|---|
| `MPC_PATH = 'base-1'` 하드코딩 | path = `user_id_hash` 또는 `intent_id_hash`로 per-user 파생 |
| `NEAR_PROXY_PRIVATE_KEY` env 평문 | KMS/HSM/Vault 보관. function-call key + path 화이트리스트 |
| 두 번 사인 (NEAR 호출 2회) | EIP-3009 안 쓰면 1회로 단축 (Zcash 직결제면 NEAR 자체가 불필요) |
| `lib/near.ts`, `lib/kdf.ts` legacy 잔재 | 처음부터 chainsig.js만. 코드베이스 정리 |
| 하드코딩 gas price/limit | 동적 가스 estimation (`viem.estimateGas` 등) |
| NEAR 다운 시 fallback 없음 | retry policy + circuit breaker + 사용자 안내 |

### 🟡 참고만

- **`lib/kdf.ts`의 epsilon 알고리즘 구현** — SDK 동작 이해용 reference. production은 SDK 직접 사용 권장

## 6.10 🚀 우리 차별화 포인트

### Cat E 관점에서 NEAR MPC를 쓸 가치가 있는가?

**선택지 3개 (다시):**

| 선택 | 가치 | 비용 |
|---|---|---|
| **A. NEAR MPC + per-user path** | 백엔드 키 0개, 사용자별 격리, 멀티체인 확장 가능 | NEAR latency·gas, NEAR 의존 |
| **B. 일반 지갑 + KMS** | 단순, 빠름 | 사용자 수만큼 키 관리 부담 |
| **C. Zcash 직결제 — EVM 단계 자체 제거** | 가장 카테고리 E다움, 사인은 사용자 Zcash 지갑이 함 | EVM 통합 포기 (USDC bridge 별도 옵션화) |

### 권장 — Hybrid 접근

1. **메인 결제 경로**: Zcash 직결제 (선택지 C). 사용자가 ZEC 보내면 가맹점이 ZEC 받음. 사인은 사용자 wallet
2. **선택적 경로**: 가맹점이 *"USDC가 좋다"* 명시 시, NEAR MPC + per-user path로 USDC bridge (선택지 A)
3. **EVM 키는 절대 보유 안 함**: KMS도 우리는 안 씀. MPC가 키 보관 부담 0으로 만들어주는 게 그제서야 가치 발휘

→ **PAL은 모든 결제를 NEAR MPC + EVM bridge로 강제했지만, 우리는 "Zcash 직결제가 기본 + MPC bridge는 선택"으로 가면 진짜 카테고리 E.**

## 6.11 한 줄 정리

> *"NEAR Chain Signatures = EVM 위에 키 없이 사인하는 마법. 이론적으로는 분산 보안이지만, NEAR access key 하나로 모든 게 무너질 수 있어서 KMS 보관 필수. PAL은 single path 하드코딩으로 MPC의 진짜 가치(per-user, 멀티체인)를 놓침. 우리는 per-user path + Zcash 직결제 hybrid가 정답."*

---

---

# STEP 7 — x402 client (PAL의 "x402"는 가짜다)

> **한 줄 요약:** PAL의 x402는 **HTTP 402 challenge/response 사이클을 전혀 수행하지 않는다.** Facilitator도 없다. 대신 *"USDC를 미리 broadcast하고 그 tx hash를 X-PAYMENT 헤더로 전달"*하는 post-hoc proof 방식. **진짜 x402와는 구조가 완전히 다르며, 우리가 카테고리 E를 하면 진짜 x402를 구현해야 한다.**
>
> **deep dive:** [`07-x402-client.md`](./subsystems/07-x402-client.md)

## 7.1 먼저 — "진짜" x402가 뭐였더라?

x402는 Coinbase가 표준화한 **HTTP-native 결제 프로토콜**. HTTP 402 status code(`Payment Required`)를 활용:

### 표준 x402 흐름

```
[1] 클라이언트: GET /premium-content
        ↓
[2] 서버: HTTP 402 Payment Required
         Body: {
           paymentRequirements: {
             scheme: "exact",
             network: "base",
             maxAmountRequired: "10000000",
             payTo: "0xMERCHANT",
             asset: "0x833589... (USDC)",
             nonce: "...",
             deadline: ...
           }
         }
        ↓
[3] 클라이언트: 사용자 지갑으로 EIP-3009 authorization 서명
        ↓
[4] 클라이언트: GET /premium-content
              X-PAYMENT: <base64-encoded authorization>
        ↓
[5] 서버: facilitator에 X-PAYMENT 검증 요청
         (Coinbase facilitator: POST /verify {authorization})
        ↓
[6] facilitator: 서명·nonce·deadline 검증 → OK
        ↓
[7] 서버: facilitator에 settle 요청
         POST /settle {authorization}
        ↓
[8] facilitator: USDC.transferWithAuthorization(...)을 broadcast
        ↓
[9] 서버: content 반환 + X-PAYMENT-RESPONSE 헤더
```

**핵심:**
- **서버가 먼저 402 발행** → 클라이언트가 X-PAYMENT로 재요청
- **facilitator가 검증·정산 대행** (사용자/머천트가 가스 안 냄)
- **X-PAYMENT = EIP-712 서명된 authorization (off-chain)**, tx hash 아님

### Facilitator 종류 (시장에 존재)

| Facilitator | 운영 | 체인/자산 |
|---|---|---|
| **Coinbase Base facilitator** | Coinbase Developer Platform | Base mainnet, USDC |
| **NLx402 (PCEF)** | Secure Legion 등이 사용 | Solana, native facilitator |
| **PayAI, Second-State, OpenZeppelin** | 다양 | 다양 |

## 7.2 PAL의 "x402"는 뭘 하는가? — 완전히 다른 흐름

### PAL의 흐름

```
[1] 사용자: AI intent 입력 → ParsedIntent
        ↓
[2] PAL이 1Click quote 받음 → QR (ZEC 입금 주소)
        ↓
[3] 사용자 ZEC 입금
        ↓
[4] cron이 1Click SUCCESS 감지 (1분 polling)
        ↓
[5] cron이 즉시 NEAR MPC로 USDC tx 서명·broadcast
        ← 이 시점에 USDC가 swapWallet → 가맹점으로 이미 옮겨감
        ↓
[6] cron이 tx hash를 signed_payload 컬럼에 저장
        ↓
[7] UI가 폴링하다가 signed_payload 받음 → content page redirect
        ↓
[8] content page가 가맹점 URL에 GET 요청
              X-PAYMENT: <tx hash>     ◄── 💥 진짜 x402는 EIP-712 서명, PAL은 tx hash
        ↓
[9] 가맹점 서버가 X-PAYMENT 받음 → (검증은 가맹점 책임, PAL 코드 밖)
```

### 차이 핵심

| | 표준 x402 | PAL의 "x402" |
|---|---|---|
| **HTTP 402 발행** | ✅ 서버가 발행 | ❌ 어디서도 발행 안 함 |
| **X-PAYMENT 내용** | EIP-712 서명된 authorization (off-chain) | **이미 broadcast된 tx hash** |
| **Facilitator** | 있음 (Coinbase, NLx402 등) | ❌ 없음 |
| **검증 시점** | 결제 *전* (facilitator가 verify) | 결제 *후* (post-hoc, 가맹점이 알아서) |
| **누가 가스를 내는가** | facilitator | swapWallet (= NEAR MPC가 사인한 PAL 운영자 지갑) |
| **체인 사인** | 클라이언트 = 사용자 지갑 | 서버 = NEAR MPC (사용자는 ZEC 송금만) |

→ PAL의 흐름은 *"x402 challenge/response"*가 아니라 ***"USDC 미리 보내고 tx hash로 영수증 처리"***. 둘은 **다른 프로토콜**이라 봐도 됨.

## 7.3 PAL이 x402로 부르는 이유 (추정)

1. **결과적으로 같은 효과** — 사용자가 콘텐츠 받음, 가맹점이 USDC 받음
2. **X-PAYMENT 헤더 이름이 같음** — 표면적 호환성
3. **EIP-3009 `transferWithAuthorization` 사용** — x402의 settlement 표준과 동일
4. **해커톤 narrative** — "x402 결제 프로젝트"라고 부르고 싶었을 것

**하지만 실질:** 표준 x402의 **검증 가능성·재현성·facilitator 모델**은 다 빠짐.

## 7.4 🎬 PAL의 18단계 실제 흐름

```
Step  1. Vercel cron */1 * * * * 호출
Step  2. Supabase에서 deadline > now인 deposit 전체 조회
Step  3. 각 deposit마다 1Click.getExecutionStatus() 폴링
Step  4. SUCCESS && !signedPayload && !x402Executed 게이트
Step  5. 파라미터 추출:
            payTo    = tracking.recipient
            amount   = tracking.amount
            deadline = Date.now()/1000 + 3600   ◄── 원본 quote deadline 무시
            nonce    = `0x${Date.now().toString(16)}` ◄── timestamp 기반
Step  6. await import('@/lib/chainSig') (dynamic)
Step  7. swapWallet 주소 파생 (chainsig.js)
Step  8. EIP-712 domain + types + value 구성
Step  9. EIP-712 hash → MPC #1 사인 → (v, r, s)
Step 10. ecrecover 검증 (불일치 시 abort)
Step 11. transferWithAuthorization calldata 인코딩
Step 12. legacy EVM tx 준비 → hashesToSign
Step 13. MPC #2 사인 → (v', r', s')
Step 14. v,r,s 삽입 + RLP 직렬화
Step 15. viem publicClient.sendRawTransaction() → Base mainnet
Step 16. Supabase: signed_payload = txHash, x402_executed = true
Step 17. UI가 /api/content/get-url로 폴링 → signedPayload 수신
Step 18. UI: fetch(redirectUrl, headers: { 'X-PAYMENT': txHash })
         ← 가맹점 서버 응답 + X-PAYMENT-RESPONSE 헤더
```

→ **여기서 사인 2번이 [STEP 6](#step-6--near-chain-signatures-키-없이-evm-서명)에서 본 그것.**

## 7.5 ⚠️ 진짜 위험한 함정 5개

### 1. **`X-PAYMENT`가 tx hash라는 비표준성**

표준 x402 facilitator(Coinbase 등)는 X-PAYMENT가 EIP-712 서명일 거라 기대. PAL이 보내는 tx hash는 facilitator 검증을 통과 못 함.
→ **PAL은 x402-호환 가맹점한테 보낼 수 없음.** 가맹점이 *"X-PAYMENT를 tx hash로 해석"*하도록 PAL 전용으로 만들어야 됨.

### 2. **nonce가 timestamp 기반 (cryptographically random 아님)**

```typescript
const nonce = `0x${Date.now().toString(16)}`  // millisecond
```

EIP-3009 표준은 `bytes32 nonce`가 unique per authorization이라야 함. PAL의 millisecond nonce는:
- 같은 ms 내 두 번 실행되면 **"authorization already used" revert**
- 실제 충돌 확률 낮지만 **이론적으로 가능 + 컴플라이언스 측면 비안전**
- 표준은 `crypto.getRandomValues(new Uint8Array(32))` 같은 cryptographic nonce 권장

### 3. **deadline이 quote와 별개로 1시간 재계산**

```typescript
const deadline = Math.floor(Date.now() / 1000) + 3600
```

1Click quote의 deadline은 무시되고 항상 *"지금부터 1시간"*. 1Click quote가 만료된 후에도 x402 실행 가능 → **unsync 가능**.

### 4. **x402 실패 시 환불 메커니즘 0**

ZEC→USDC swap은 이미 성공한 상태에서 x402 transfer만 실패하면:
- USDC가 swapWallet에 묶임
- DEPLOY.md가 `/api/relayer/refund`를 약속하지만 **파일 자체가 존재 안 함**
- 사용자 자금 손실 가능. cron 레벨 재시도(1분 주기, deadline까지)만 안전망

### 5. **가맹점이 검증 안 하면 PAL 신뢰가 전부**

`get-url/route.ts:52`는 Supabase의 `signedPayload` 존재만 확인. 가맹점 서버가 *"X-PAYMENT 헤더 있으면 OK"*만 보고 통과시키면:
- PAL DB만 신뢰. on-chain 검증 안 함
- **누군가 PAL DB에 직접 write 가능하면 free content 가능**
- 표준 x402의 facilitator-mediated verification이 빠져서 생긴 보안 갭

## 7.6 PAL vs Secure Legion (NLx402) — Week2 핵심 결론 다시

| | PAL | Secure Legion |
|---|---|---|
| Zcash 역할 | **upstream funding** (ZEC → USDC) | **x402 carrier** (memo에 NLx402 quote hash 삽입) |
| Settlement chain | Base | Solana (NLx402) |
| Privacy | 0 (1Click이 모든 linkage 봄) | ✅ Zcash shielded memo |
| Facilitator | ❌ 없음 | NLx402 (PCEF) |
| Replay 방어 | timestamp nonce (약함) | NLx402 quote_hash (강함) |
| "x402 + Zcash" 정합성 | 이름만 빌림 | 진짜 Zcash가 x402 carrier |

→ **Secure Legion이 더 카테고리 E다움.** 다만 Solana 의존 + NLx402 facilitator 의존.

**우리 차별화 기회:** Secure Legion = Zcash memo carrier + Solana facilitator. **우리는 Zcash 자체를 settlement rail로 + facilitator도 in-Zcash로** 가면 둘 다 넘는 포지션.

## 7.7 🎯 우리 프로젝트 관점

### ✅ Lift-and-use

- **`X-PAYMENT` 헤더 + EIP-3009 `transferWithAuthorization` 조합** — Base/EVM 쪽 결제면 이건 표준
- **idempotency 게이트 (`!signedPayload && !x402Executed`)** — 깔끔
- **post-hoc proof도 합법적 패턴** — *우리가 facilitator 운영하기 부담스러우면* 이걸 쓸 수도 있음 (단, 가맹점이 on-chain 검증해야)

### ❌ Redo

| 항목 | 우리는 어떻게 |
|---|---|
| nonce timestamp 기반 | `crypto.getRandomValues(new Uint8Array(32))` cryptographic random |
| deadline quote 무시 | quote에서 받은 deadline 그대로 (한 source of truth) |
| 환불 endpoint 없음 | 처음부터 구현 — x402 실패 시 자동 환불 트리거 |
| 가맹점 검증 위임 | 우리가 표준 facilitator 호출 또는 자체 facilitator 구현 |
| HTTP 402 challenge 없음 | **표준 x402 dance 구현** (서버 402 → 클라 X-PAYMENT) |
| `signedPayload` 명칭 | `evm_tx_hash` 같이 정직한 이름 |

### 🟡 참고만

- PAL의 *"USDC 미리 broadcast 후 tx hash 전달"* 패턴 — 빠른 hackathon용으로는 OK, production에선 표준으로 가야

## 7.8 🚀 우리 차별화 — 진짜 Cat E 빌드 시 4가지 옵션

### 옵션 A. Coinbase facilitator 그대로 쓰기 (Base USDC 결제만)

- 표준 x402 dance 구현
- Coinbase facilitator API 호출
- Zcash 관여 없음 — **Cat E라 부르기 어려움**

### 옵션 B. NLx402 (PCEF) 활용 + Zcash memo carrier (Secure Legion 패턴)

- Solana NLx402 facilitator 사용
- Zcash shielded memo에 quote hash 박아 결제 증명
- Solana 의존 + Zcash carrier 결합

### 옵션 C. ★ Native Zcash x402 facilitator 직접 구현 (가장 카테고리 E다움)

- HTTP 402 dance + Zcash settlement
- 우리가 facilitator 역할:
  1. 서버가 402 발행 (`scheme: "shielded-zcash"`, `payTo: zs1...`, ...)
  2. 클라이언트가 Zcash 지갑으로 메모에 challenge nonce 넣어 송금
  3. 우리 facilitator가 viewing key로 입금 확인 → settle 완료
- **PAL과 Secure Legion 둘 다 못 한 영역** = 진짜 차별화

### 옵션 D. Hybrid (B + C)

- 기본은 옵션 C (Zcash 직결제)
- 가맹점이 "USDC 받고 싶다" 명시 시 옵션 A로 fallback
- 가장 실용적

## 7.9 한 줄 정리

> *"PAL의 x402는 HTTP 402도 없고 facilitator도 없는 '미리 USDC 보내고 tx hash로 영수증 처리'다. 진짜 x402는 challenge/response + facilitator 모델. 우리가 카테고리 E를 하면 옵션 C(Native Zcash x402 facilitator)가 PAL과 Secure Legion 둘 다 넘는 차별화 포지션."*

---

---

# STEP 8 — NEAR Rust contract (dead code 카탈로그)

> **한 줄 요약:** `contract/src/lib.rs`의 `AnyonePay` 컨트랙트는 *"NEAR 네이티브 x402 facilitator"* 설계의 청사진이지만, **프로덕션 TypeScript 코드 어디에서도 호출되지 않는 완전한 dead code**. 배포 스크립트와 테스트 스크립트에서만 호출됨. 우리에게 가치 있는 건 **설계 아이디어**(코드 자체는 X).
>
> **deep dive:** [`08-near-rust-contract.md`](./subsystems/08-near-rust-contract.md)

## 8.1 "이 컨트랙트 왜 있어?"

설계 의도는 분명함:
1. `create_intent()` — 결제 인텐트를 on-chain에 기록
2. `verify_deposit()` — NEAR Intents에 ZEC 입금 확인 cross-contract call
3. `mark_funded()` — Funded 상태로 전환
4. `execute_x402_payment()` — `x402.near` facilitator에 `pay()` cross-contract call
5. `on_x402_payment_success()` — callback으로 Completed 상태 처리

→ **NEAR 위에서 결제 전체를 처리하는 trustless 모델.** 만약 동작한다면 PAL의 현재 아키텍처(서버리스 cron + offchain MPC)보다 훨씬 trustless함.

## 8.2 💀 Dead Code Matrix — 모든 메서드 호출자 현황

| 메서드 | 런타임 TS 호출 | 스크립트 호출 | 판정 |
|---|---|---|---|
| `new(...)` `#[init]` | ❌ 없음 | `deploy.sh` | dead (배포 전용) |
| `create_intent(...)` | ❌ 없음 | `deploy.sh`, `test-contract.sh` | dead (테스트 전용) |
| `verify_deposit(...)` | ❌ 없음 | ❌ 없음 | **완전 dead** |
| `execute_x402_payment(...)` | ❌ 없음 | ❌ 없음 | **완전 dead** |
| `on_x402_payment_success(...)` | ❌ 없음 | ❌ 없음 | **완전 dead (unreachable callback)** |
| `get_intent(...)` | ❌ 없음 | `deploy.sh`, `test-contract.sh` | dead (테스트 전용) |
| `mark_funded(...)` | ❌ 없음 | ❌ 없음 | **완전 dead** |

> `rg -n "execute_x402_payment\|create_intent\|mark_funded\|verify_deposit" --type ts --type tsx --type js` → **0건**.
> `NEXT_PUBLIC_CONTRACT_ID`는 `next.config.js`에 정의만 되고 어떤 .ts/.tsx도 안 읽음.

## 8.3 🐛 구현 자체의 버그 3가지

### 1. **`mark_funded` `#[private]` ↔ DEPLOY_CONTRACT.md 모순**

```rust
#[private]
pub fn mark_funded(&mut self, intent_id: String) { ... }
```

- `#[private]` 매크로 의미: **`predecessor_account_id == current_account_id`** (= 컨트랙트 자기 자신만 호출 가능)
- DEPLOY_CONTRACT.md 주장: "Called by relayer only"
- → 외부 relayer가 호출하면 **반드시 panic** (구조적으로 불가능)

설계자가 `#[private]`을 *"외부 relayer 접근 제어"*로 잘못 이해한 듯. 진짜 relayer-only는 `assert_eq!(predecessor, RELAYER_ID)` 패턴.

### 2. **`verify_deposit` — fire-and-forget no-op**

```rust
pub fn verify_deposit(&self, intent_id: String) -> bool {
    Promise::new(self.intents_contract.clone())
        .function_call("mt_batch_balance_of", ...);   // ◄── Promise 바인딩 없음
    true   // ◄── 항상 true. 비동기 결과 안 기다림
}
```

세 가지 문제:
- **`&self` 불변 참조** — Promise callback 등록 불가 (`&mut self` + `.then()` 필요)
- **Promise drop** — 생성된 Promise가 변수에 바인딩 안 되어 즉시 drop
- **무조건 true** — 검증 안 하고 항상 통과

→ 주석에 *"In production, this would call intents.near to verify deposit"* — 본인이 stub임을 인정.

### 3. **`x402.near` 컨트랙트의 실존 불확실**

```rust
Promise::new(self.x402_facilitator.clone())   // = "x402.near"
    .function_call("pay", { amount, recipient, token: "usdc" }, ...)
```

- `x402.near`가 NEAR mainnet에 실제 컨트랙트로 배포되어 있는지 **검증 안 됨**
- `pay()` 메서드 ABI 정의 PAL 코드에 없음
- 메서드 args (`amount`, `recipient`, `token: "usdc"`)는 임시 설계로 보임

→ 호출하려는 *"facilitator"*가 진짜 존재하는지 모르는 상태.

## 8.4 🎯 우리 프로젝트 관점

### ❌ Lift-and-use: 코드는 거의 없음

dead code인데다 버그까지 있어서 그대로 가져갈 게 없음.

### ✅ Lift-and-use: 설계 아이디어

**"NEAR 컨트랙트가 결제 흐름의 hub" 패턴은 가치 있는 청사진:**
- 인텐트가 on-chain에 기록 → 누구나 audit 가능
- cross-contract call로 facilitator 분리 → 단일 책임
- callback 패턴으로 상태 전이 → 비동기 처리

우리가 이 설계를 진짜 구현하면 **PAL이 못 한 trustless 모델** 완성.

### ❌ Redo (만약 우리가 NEAR 컨트랙트 경로 갈 거면)

| 항목 | 우리는 어떻게 |
|---|---|
| `verify_deposit` no-op | `&mut self` + `.then()` callback 패턴으로 비동기 검증 |
| `mark_funded` `#[private]` 오용 | relayer account ID를 state에 저장 + `assert_eq!` 명시 |
| `x402.near` 불명확 | facilitator 컨트랙트 ABI 명세부터 작성 + 배포 |
| TS 연결 부재 | `lib/near.ts` 또는 새 모듈에서 컨트랙트 메서드 RPC 호출 |
| `update-env.sh` 무용 | env가 실제 TS에서 사용되도록 연결 |
| build에 `wasm-opt` 없음 | `cargo-near` 또는 binaryen `wasm-opt -Oz` 추가 |

## 8.5 🚀 우리 차별화 — NEAR-native Cat E facilitator (옵션 C 변형)

[STEP 7](#step-7--x402-client-pal의-x402는-가짜다)에서 제시한 옵션 C를 한 단계 더 — *"facilitator를 우리가 운영"* 대신 **"facilitator를 NEAR 컨트랙트로 배포"**:

```
사용자 ZEC 입금 → lightwalletd가 viewing key로 감지
       ↓
PAL이 (또는 우리 컨트랙트가) intent.status = Funded
       ↓
NEAR 컨트랙트 cat-e-facilitator.near.pay()
       ├── 자체 USDC reserve로 결제 OR
       └── Zcash 직결제 confirm + content unlock 토큰 발행
       ↓
on-chain audit log (누구나 검증 가능)
```

→ PAL의 컨트랙트 설계가 *"꿈만 그리고 안 만든 영역"*. 우리가 만들면 PAL/Secure Legion 둘 다 못 한 영역.

## 8.6 한 줄 정리

> *"PAL의 NEAR Rust 컨트랙트는 dead code지만 설계 아이디어는 가치 있다. 'NEAR-native x402 facilitator' 청사진을 우리가 진짜로 구현하면, PAL이 끝내지 못한 trustless 모델 + 카테고리 E 차별화를 동시에 잡을 수 있다."*

---

# STEP 9 — 종합: Lift-and-use vs Redo 매트릭스 + 차별화 제안

> **이 STEP은 STEP 1~8을 한 페이지로 압축한 의사결정 도구.** 팀 회의에서 *"PAL에서 뭘 베끼고 뭘 새로 만들지"*를 정할 때 그대로 던질 수 있는 매트릭스.

## 9.1 🟢 Lift-and-use 매트릭스 (PAL에서 베낄 만한 것)

| 출처 | 항목 | 가치 | 변경 필요? |
|---|---|---|---|
| STEP 1 | 3단계 fallback (semantic → LLM → regex) | LLM 비용 절감 패턴 | 미세 조정 |
| STEP 1 | `response_format: {type: 'json_object'}` + RULES system prompt | LLM 안정성 | 그대로 |
| STEP 2 | pgvector `<=>` cosine + IVFFlat 인덱스 | 시맨틱 검색 표준 | threshold 통일 |
| STEP 2 | `match_services(query, threshold, count)` 시그니처 | 깔끔한 인터페이스 | 그대로 |
| STEP 2 | insert-time vs query-time 임베딩 비대칭 | 합리적 비용 모델 | 캐싱 추가 |
| STEP 2 | GET 응답에서 민감 필드(`url`) 제거 | 보안 패턴 | 그대로 |
| STEP 4 | Supabase 상태 머신 (PK + flags + deadline filter) | 멱등성 보장 | 컬럼명 정리 |
| STEP 4 | partial index `WHERE confirmed=false AND deadline IS NOT NULL` | cron 쿼리 최적화 | 그대로 |
| STEP 4 | `quote_data JSONB` 통째 저장 | audit/디버깅용 | 그대로 |
| STEP 5 | thin client 호출 패턴 | swap aggregator 통합용 | 그대로 |
| STEP 5 | `refundTo` 명시 습관 | 환불 디자인 | 그대로 |
| STEP 6 | `chainsig.js` SDK 사용 패턴 | NEAR MPC 표준 | per-user path |
| STEP 6 | **ecrecover 사전 검증** | silent failure 방지 | **필수 모방** |
| STEP 6 | dynamic import 패턴 | cold start 절감 | 그대로 |
| STEP 6 | 개별 deposit error 격리 | cron robustness | 그대로 |
| STEP 7 | EIP-3009 `transferWithAuthorization` | EVM 결제 표준 | 그대로 |
| STEP 7 | idempotency 게이트 (`!signedPayload && !x402Executed`) | 중복 방지 | 그대로 |

## 9.2 🔴 Redo 매트릭스 (PAL에서 다시 만들 것)

| 출처 | 항목 | 왜 다시? | 우리는 어떻게 |
|---|---|---|---|
| STEP 1 | "NEAR AI" 위장 레이어 | 호환성 ↓, 보안 ↓ | 명시적 OpenAI / Anthropic 선택 |
| STEP 1 | `bridgeFrom: 'zcash'` 하드코딩 2군데 | 모든 결제 = ZEC 강제 | multi-source 지원 |
| STEP 1 | prompt injection 노출 | 보안 취약점 | service 등록 단계 sanitize |
| STEP 1 | `detectChainForDomain` placeholder | 부정확한 chain 추론 | 실제 chain registry / 사용자 선택 |
| STEP 2 | threshold 0.6 vs 0.7 불일치 | 일관성 X | 단일 상수 |
| STEP 2 | 쿼리 임베딩 캐싱 없음 | OpenAI 비용 선형 증가 | Redis/LRU 캐시 |
| STEP 2 | `receiving_address` 형식 검증 없음 | 자금 손실 위험 | chain별 정규식 + Zcash 추가 |
| STEP 2 | RLS 정책 미설정 | 보안 default 위험 | 처음부터 RLS on |
| STEP 3 | Z-address 생성 = 외주 | privacy 0 | **자체 ZIP-32 + Sapling/Orchard** |
| STEP 3 | QR이 raw 주소 (ZIP-321 아님) | amount mismatch 위험 | ZIP-321 URI |
| STEP 3 | refundTo env optional | 회수 불가 가능 | 필수 + 검증 |
| STEP 4 | cronjob 인증 주석 처리 | 누구나 실행 트리거 | CRON_SECRET 의무 |
| STEP 4 | `/test-supabase` 인증 없음 | DB 노출 | production 비활성화 |
| STEP 4 | 환불 endpoint 미구현 | 자금 영구 분실 | 처음부터 구현 + 자동 환불 |
| STEP 4 | In-memory fallback | Vercel에서 작동 X | 제거 + fail-fast |
| STEP 4 | 1Click `SUCCESS` blind trust | 1Click 침해 시 즉사 | Zcash 직접 조회 |
| STEP 4 | 1분 polling | 결제용으로 느림 | webhook / SSE |
| STEP 4 | `signed_payload` 컬럼명 | 오해 유발 | `evm_tx_hash` 정직 |
| STEP 4 | `payTo` 3단 fallback | 1Click 응답 형식 변하면 잘못된 주소 | 단일 source |
| STEP 4 | deadline 두 의미 | 디버깅 혼동 | `quote_deadline` vs `x402_deadline` 분리 |
| STEP 5 | 1Click 전체 의존 | privacy/trust/compliance 다 무너짐 | **자체 Zcash 인프라** |
| STEP 5 | SDK 응답 `as any` 3단 fallback | silent break | 명시적 타입 |
| STEP 5 | `INCOMPLETE_DEPOSIT` 처리 없음 | 자금 limbo | 알림 + 재입금 UX |
| STEP 5 | quote deadline 3분 하드코딩 | Zcash confirm에 빠듯 | 동적 설정 |
| STEP 6 | `MPC_PATH = 'base-1'` 하드코딩 | per-user 격리 0 | **path = user_id_hash** |
| STEP 6 | `NEAR_PROXY_PRIVATE_KEY` env 평문 | 보안 경계 무너짐 | **KMS/HSM** + path 화이트리스트 |
| STEP 6 | 사인 2회 (NEAR 호출 2회) | latency·비용 2배 | EIP-3009 안 쓰면 1회 |
| STEP 6 | legacy `lib/near.ts`, `lib/kdf.ts` | 코드베이스 어지러움 | chainsig.js만 |
| STEP 6 | 하드코딩 gas | 가스 인플레/디플레 대응 X | 동적 estimation |
| STEP 7 | HTTP 402 dance 없음 | x402 호환성 X | **표준 dance 구현** |
| STEP 7 | nonce timestamp 기반 | 충돌·보안 약함 | **cryptographic random** |
| STEP 7 | deadline 1시간 재계산 | quote와 unsync | quote 그대로 사용 |
| STEP 7 | X-PAYMENT가 tx hash | 표준 x402 가맹점 사용 불가 | EIP-712 서명 |
| STEP 7 | 가맹점 검증 위임 | PAL DB 신뢰 전부 | facilitator 호출 / 자체 facilitator |
| STEP 8 | NEAR 컨트랙트 dead code | 코드 자체는 가치 X | 진짜 구현 또는 제거 |
| STEP 8 | `mark_funded` `#[private]` 오용 | 외부 호출 불가 | 명시적 relayer 체크 |
| STEP 8 | `verify_deposit` no-op | 검증 안 함 | `&mut self` + callback |
| STEP 8 | `x402.near` 불명확 | 호출 대상 불명 | ABI 명세 + 배포 |

## 9.3 🚀 7가지 차별화 제안 (Cat E 빌드 시)

PAL/Secure Legion에 없는 영역들. **우선순위 순서.**

### 🥇 D1. Native Zcash shielded settlement (옵션 C)
- ZEC가 funding asset이 아니라 **settlement asset 자체**
- shielded 주소(`zs1...`/Orchard) 직접 생성, viewing key 기반 입금 확인
- → PAL/Secure Legion 둘 다 못 한 핵심 영역. **가장 큰 차별화.**

### 🥈 D2. Memo 기반 x402 challenge (Secure Legion 패턴 + 자체 facilitator)
- Zcash memo (512 bytes encrypted)에 x402 challenge nonce
- on-chain proof 자체가 결제 증명 → replay 방어, idempotency, privacy 한방
- Secure Legion은 NLx402(Solana) 의존 — 우리는 in-Zcash facilitator

### 🥉 D3. 표준 HTTP 402 challenge/response 구현
- 서버가 402 발행 → 클라이언트 X-PAYMENT 재요청
- PAL은 안 함, 표준 호환 가맹점과 협업 가능
- Coinbase facilitator도 옵션으로 fallback

### D4. Per-user MPC path (NEAR Chain Sig 활용 시)
- `path = user_id_hash`로 사용자별 격리
- PAL이 못 한 MPC 진짜 활용
- 백엔드 키 0개로 사용자별 swapWallet

### D5. Trust-minimal deposit tracking
- 1Click blind trust → 자체 lightwalletd / Zebra RPC
- 1분 polling → streaming RPC (webhook 또는 SSE)
- latency 1분 → 수 초

### D6. NEAR-native facilitator 컨트랙트 (옵션 C 발전형)
- PAL이 그린 청사진을 진짜로 구현
- `cat-e-facilitator.near`에 `verify_deposit + execute_payment` 통합
- on-chain audit log

### D7. 자동 환불 + failure-mode 처음부터 설계
- x402 실패 → 자동 sender ZEC 환불 트리거
- INCOMPLETE_DEPOSIT → 사용자 알림 + 재입금 UX
- PAL은 다 안 함

## 9.4 🎯 최종 권장 — 우리 팀 Cat E 빌드 시 5가지 결정 사항

1. **Settlement = native Zcash shielded** (옵션 C/D 하이브리드)
   - Bridge USDC는 *선택지*로만, default 아님
2. **Facilitator = in-house Zcash facilitator** + Coinbase fallback
3. **Wallet model = MPC per-user path** OR **사용자 wallet 직결제**
   - 우리가 USDC bridge 운영하면 MPC per-user, 순수 Zcash면 사용자 wallet
4. **Trust = lightwalletd self-hosted**, 1Click 완전 제거 (선택 시에만 옵션)
5. **Code reuse from PAL = ~30%** (intent parser, service registry, ecrecover 검증, chainsig.js 패턴)

→ **PAL을 베끼는 게 아니라, PAL이 못 한 모든 영역을 채우는 방향.** *"이름만 Zcash"*에서 *"진짜 Zcash"*로.

## 9.5 한 문단 임원 요약 (CEO에게 한 문단으로 보고할 때)

> *"Pay Anyone Legend는 'x402 + Zcash' 프로젝트를 표방하지만 실제로는 Gibraltar 회사(Defuse Labs)의 transparent address swap API에 Zcash 처리를 100% 위임한 상태이며, x402도 표준 HTTP 402 dance 없이 USDC를 미리 broadcast해 tx hash를 영수증으로 쓰는 비표준 구현이다. PAL에서 우리가 그대로 가져갈 만한 것은 NEAR Chain Signatures 사인 패턴, pgvector 시맨틱 검색, Supabase 상태 머신 정도(코드 30% 수준)에 그치고, 진짜 카테고리 E 차별화는 PAL이 비워둔 영역 — native shielded Zcash settlement + memo 기반 x402 challenge + in-house Zcash facilitator — 을 직접 구현하는 데 있다."*

---

---

# 🏁 마무리

**STEP 0~9 모두 완료.** 이 문서는 팀 회의에서 다음 용도로 그대로 쓸 수 있어요:

- **STEP 0** — 한 페이지 high-level 발표 (CEO/PM용)
- **STEP 1~8** — 각 서브시스템 deep dive (엔지니어 onboarding)
- **STEP 9** — 의사결정 매트릭스 (스코프 회의)

## 함께 보면 좋은 문서

- **[`CONCRETE-EXAMPLE-WALKTHROUGH.md`](./CONCRETE-EXAMPLE-WALKTHROUGH.md)** — 한 사용자의 결제 흐름을 처음부터 끝까지 구체적 데이터로 따라가는 시나리오 문서
- **[`README.md`](./README.md)** — §0 big picture + Mermaid arch map
- **[`category-E-extraction.md`](./category-E-extraction.md)** — §2 PAL vs Secure Legion 비교 + 차별화 상세
- **[`zcash-tool-inventory.md`](./zcash-tool-inventory.md)** — §3 1Click의 실체 + Zcash 도구 생태계
- **[`_claims-to-verify.md`](./_claims-to-verify.md)** — 136개 claim 검증 매트릭스
- 서브시스템 deep dive 8개 (`01-` ~ `08-`) — 각 STEP의 근거 코드

## 변경 이력

- 2026-05-13: STEP 0~9 완성. 별도 concrete example walkthrough 문서 작성.
- **STEP 5** — 1Click bridge (Defuse Labs Limited, 외주의 실체)
- **STEP 6** — NEAR Chain Signatures (키 없이 EVM tx 서명)
- **STEP 7** — x402 client (PAL의 가짜 x402 vs 진짜 x402)
- **STEP 8** — NEAR Rust contract (dead code 카탈로그)
- **STEP 9** — 종합: lift-and-use vs redo 매트릭스 + 차별화 제안 7가지
