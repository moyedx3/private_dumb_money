# §1.1 Intent parser (자연어 의도 파싱)

## 목적 (Purpose)

Intent parser 서브시스템은 사용자가 자연어로 입력한 결제 요청(예: "Pay 0.1 USDC to 0x03fBbA… on Base")을 구조화된 결제 의도 객체(`ParsedIntent`)로 변환하는 역할을 담당한다. 이 변환 과정은 두 단계로 이루어진다: 먼저 Supabase + pgvector의 임베딩 기반 시맨틱 검색으로 등록된 서비스와 매칭하고, 매칭 실패 시 LLM 완성(chat completion)을 통해 결제 필드(amount, currency, chain, receivingAddress)를 추출한다. 이 서브시스템이 존재하는 이유는 사용자가 x402 결제를 자연어로 표현할 수 있도록 UX 추상화 계층을 제공하기 위함이다.

## 파일과 함수 (Files & functions)

- `lib/intentParser.ts:16` — `parseIntent(query: string): Promise<ParsedIntent>` — 브라우저(클라이언트)측 진입점. `/api/parse-intent`로 POST 요청을 보내고 응답을 `ParsedIntent`로 매핑함
- `lib/nearAI.ts:29` — `analyzePromptWithNearAI(prompt: string): Promise<AnalyzedIntent>` — 서버측 핵심 로직. 서비스 레지스트리 시맨틱 검색 → LLM chat completion → 규칙 기반 fallback 순서로 실행
- `lib/nearAI.ts:215` — `getAllServicesForPrompt(): Promise<string>` — LLM system prompt에 포함할 서비스 목록을 문자열로 직렬화
- `lib/nearAI.ts:224` — `parsePromptFallback(prompt: string): AnalyzedIntent` — API 키 없거나 LLM 오류 시 regex로 결제 필드를 직접 추출하는 rule-based fallback
- `lib/nearAI.ts:279` — `detectChainForDomain(domain: string): Promise<string>` — recipient가 도메인인 경우 체인 추론 (heuristic; 실제 레지스트리 조회 없음)
- `app/api/parse-intent/route.ts:16` — `POST(request: NextRequest)` — Next.js API route handler. `analyzePromptWithNearAI` 호출 후 결과를 `ParsedIntent` 구조로 래핑하여 반환
- `lib/serviceRegistry.ts:168` — `findBestService(query: string, threshold: number): Promise<PaymentService | null>` — `analyzePromptWithNearAI`에서 호출; pgvector 시맨틱 검색 위임
- `lib/serviceRegistry.ts:30` — `generateEmbedding(text: string): Promise<number[] | null>` — OpenAI `text-embedding-3-small` 모델로 쿼리 임베딩 생성
- `components/IntentFlowDiagram.tsx:76` — `IntentFlowDiagram` — UI 전용; intent 파싱 결과를 단계별 플로우 다이어그램으로 시각화 (파싱 로직 없음)
- `app/page.tsx:346` — `handleSubmit(text: string)` — 사용자 입력 수신 후 `parseIntent()` 호출, 결과로 QR 코드 생성 또는 AI 메시지 표시를 결정
- `components/FloatingInput.tsx:36` — `handleSubmit(e: React.FormEvent)` — 폼 제출 이벤트를 `onSubmit` prop(= `app/page.tsx`의 `handleSubmit`)으로 위임

## 연결 (Wiring)

- **Inputs:**
  - 사용자 자연어 입력 문자열 (`query: string`) — `components/FloatingInput.tsx`의 폼 submit 이벤트로부터
  - 선택적: `user_account` (현재 API route에서 파싱되지만 사용하지 않음 — `app/api/parse-intent/route.ts:19`)
- **Outputs:**
  - `ParsedIntent` 객체 (intent_type, amount, redirect_url, metadata{action, recipient, currency, chain, receivingAddress, serviceId, serviceName}, chain, needsBridge, bridgeFrom, bridgeTo, aiMessage)
  - 불완전 intent의 경우: `aiMessage` 필드만 채워진 `ParsedIntent` (결제 플로우 차단 대신 UI에 안내 메시지 표시)
- **Dependencies (internal):**
  - `lib/serviceRegistry.ts` → `findBestService`, `getAllServices` (시맨틱 서비스 매칭) — [§1.2 service registry](./02-service-registry.md)
  - `lib/supabase.ts` → Supabase 클라이언트 (serviceRegistry를 통해 간접 의존)
- **Dependencies (external):**
  - `openai` npm 패키지 (`lib/nearAI.ts:59`, `lib/serviceRegistry.ts:6`)
  - OpenAI API 또는 NEAR AI Cloud API (환경 변수로 선택)
  - Supabase + pgvector (서비스 임베딩 검색)

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `openai` | `^6.9.1` | LLM chat completion (`lib/nearAI.ts:59`, `lib/nearAI.ts:112`) 및 임베딩 생성 (`lib/serviceRegistry.ts:37`) |
| `@supabase/supabase-js` | `^2.86.0` | pgvector 시맨틱 검색 — `match_services` RPC 호출 (`lib/serviceRegistry.ts:68`) |
| `next` | `^15.0.0` | API route (`app/api/parse-intent/route.ts`) 및 클라이언트 fetch 인프라 |

> NEAR AI SDK 별도 패키지 없음. `lib/nearAI.ts`는 `openai` 패키지의 `baseURL`을 `https://cloud-api.near.ai/v1`로 교체하는 방식으로 NEAR AI Cloud를 호출한다 (`lib/nearAI.ts:9`).

## 워크스루 — happy path

아래는 사용자가 "Pay 0.1 USDC to 0x03fBbA… on Base"를 입력하는 경우 (서비스 레지스트리 미매칭, OpenAI API 키 존재)를 기준으로 한 happy path이다.

**1. 사용자 입력 — `components/FloatingInput.tsx:36-39`**
```typescript
// FloatingInput.tsx:36
const handleSubmit = (e: React.FormEvent) => {
  e.preventDefault()
  if (value.trim()) onSubmit(value.trim())
}
```
사용자가 텍스트를 입력하고 Enter 키 또는 Submit 버튼을 누르면 `onSubmit` prop이 호출된다.

**2. 페이지 핸들러 진입 — `app/page.tsx:346-358`**
```typescript
// app/page.tsx:346
const handleSubmit = async (text: string) => {
  setQuery(text)
  setIsLoading(true)
  // URL에 prompt 파라미터 추가
  const newUrl = new URL(window.location.href)
  newUrl.searchParams.set('prompt', text)
  window.history.pushState({}, '', newUrl.toString())
  // ...
  const parsed = await parseIntent(text)
```
`app/page.tsx`의 `handleSubmit`은 로딩 상태를 켜고, URL을 업데이트한 뒤 `parseIntent(text)`를 호출한다.

**3. 클라이언트측 fetch 발송 — `lib/intentParser.ts:16-26`**
```typescript
// lib/intentParser.ts:16
export async function parseIntent(query: string): Promise<ParsedIntent> {
  const response = await fetch('/api/parse-intent', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ query }),
  })
```
`lib/intentParser.ts`의 `parseIntent`는 **브라우저에서 실행**되며, `query`를 JSON body에 담아 `/api/parse-intent`로 POST 요청을 전송한다.

**4. API route 수신 — `app/api/parse-intent/route.ts:16-29`**
```typescript
// app/api/parse-intent/route.ts:16
export async function POST(request: NextRequest) {
  const body = await request.json()
  const { query, user_account } = body
  if (!query || typeof query !== 'string') {
    return NextResponse.json({ error: 'Query is required...' }, { status: 400 })
  }
  const analyzed = await analyzePromptWithNearAI(query)
```
Next.js API route가 요청을 받아 `query`를 검증하고 `analyzePromptWithNearAI`를 호출한다. **이 함수부터 서버에서 실행**된다.

**5. 서비스 레지스트리 시맨틱 검색 시도 — `lib/nearAI.ts:32`**
```typescript
// lib/nearAI.ts:32
const matchedService = await findBestService(prompt, 0.6)
```
임계값 0.6(60%)으로 `findBestService`를 호출한다. 내부적으로 `searchServicesSemantic`이 실행된다.

**6. 쿼리 임베딩 생성 — `lib/serviceRegistry.ts:37-41`**
```typescript
// lib/serviceRegistry.ts:37
const response = await openai.embeddings.create({
  model: 'text-embedding-3-small',
  input: text,
})
return response.data[0].embedding
```
OpenAI `text-embedding-3-small` 모델로 쿼리를 벡터화한다.

**7. pgvector 시맨틱 검색 — `lib/serviceRegistry.ts:68-72`**
```typescript
// lib/serviceRegistry.ts:68
const { data, error } = await supabase.rpc('match_services', {
  query_embedding: queryEmbedding,
  match_threshold: threshold,
  match_count: 10,
})
```
Supabase의 `match_services` PostgreSQL 함수를 RPC로 호출한다. 이 예시에서는 0.6 이상 유사도의 서비스가 없다고 가정한다.

**8. LLM 진입 판단 — `lib/nearAI.ts:52-59`**
```typescript
// lib/nearAI.ts:52
if (!NEAR_AI_API_KEY) {
  return parsePromptFallback(prompt)
}
// ...
const openai = new OpenAI({
  baseURL: NEAR_AI_BASE_URL,
  apiKey: NEAR_AI_API_KEY,
})
```
서비스 미매칭이고 API 키가 존재하므로 LLM 경로로 진입한다. `OPENAI_API_KEY`가 있으면 `baseURL`은 `https://api.openai.com/v1`, 없으면 `https://cloud-api.near.ai/v1`으로 설정된다 (`lib/nearAI.ts:9-11`).

**9. 서비스 목록 직렬화 — `lib/nearAI.ts:65`**
```typescript
// lib/nearAI.ts:65
const availableServices = await getAllServicesForPrompt()
```
`getAllServicesForPrompt`가 Supabase에서 전체 서비스 목록을 가져와 `"- {name} (keywords: ...) - Amount: ... Chain: ... URL: ..."` 형식의 문자열로 직렬화한 뒤, system prompt에 삽입한다 (`lib/nearAI.ts:69`).

**10. LLM chat completion 호출 — `lib/nearAI.ts:112-121`**
```typescript
// lib/nearAI.ts:112
const completion = await openai.chat.completions.create({
  model: process.env.OPENAI_API_KEY
    ? 'gpt-4o-mini'
    : 'deepseek-chat-v3-0324',
  messages: [
    { role: 'system', content: systemPrompt },
    { role: 'user', content: prompt },
  ],
  response_format: { type: 'json_object' },
})
```
`OPENAI_API_KEY`가 있으면 `gpt-4o-mini`, 없으면 NEAR AI Cloud의 `deepseek-chat-v3-0324` 모델을 사용한다. `response_format: { type: 'json_object' }`로 JSON 출력을 강제한다.

**11. 응답 파싱 및 통화 정규화 — `lib/nearAI.ts:123-173`**
```typescript
// lib/nearAI.ts:123
const response = completion.choices[0].message.content
const parsed = JSON.parse(response) as AnalyzedIntent
// USDT → USDC 자동 변환
if (originalCurrency === 'USDT' || ...) { parsed.currency = 'USDC' }
```
LLM 응답 JSON을 파싱하고, USDT 등 USD 계열 통화를 USDC로 자동 변환한다. amount, currency, chain, receivingAddress가 모두 채워졌으면 `hasCompleteData = true`.

**12. `AnalyzedIntent` 반환 — `lib/nearAI.ts:173`**
```typescript
return parsed  // lib/nearAI.ts:173
```
완전한 `AnalyzedIntent`가 `analyzePromptWithNearAI`의 호출자(API route)로 반환된다.

**13. API route에서 chain 추론 및 응답 구성 — `app/api/parse-intent/route.ts:32-70`**
```typescript
// app/api/parse-intent/route.ts:32
let targetChain = analyzed.chain
if (analyzed.recipient && analyzed.recipient.includes('.')) {
  targetChain = await detectChainForDomain(analyzed.recipient)
}
const parsed: ParsedIntent = {
  intent_type: 'payment',
  amount: analyzed.amount,
  redirect_url: finalRedirectUrl,
  metadata: { ... },
  chain: targetChain,
  needsBridge: analyzed.needsBridge,
  ...
}
return NextResponse.json(parsed)
```
recipient가 도메인이면 `detectChainForDomain`으로 체인을 추론한 뒤, `ParsedIntent`를 조립하여 JSON으로 응답한다.

**14. 클라이언트 응답 매핑 — `lib/intentParser.ts:31-45`**
```typescript
// lib/intentParser.ts:31
const data = await response.json()
return {
  type: data.intent_type,
  intent_type: data.intent_type,
  amount: data.amount,
  redirectUrl: data.redirect_url,
  redirect_url: data.redirect_url,
  metadata: data.metadata,
  chain: data.chain || data.metadata?.chain,
  needsBridge: data.needsBridge ?? data.metadata?.needsBridge,
  bridgeTo: data.bridgeTo || data.metadata?.bridgeTo,
  aiMessage: data.aiMessage,
}
```
`lib/intentParser.ts`가 API 응답을 `ParsedIntent`로 매핑하여 `app/page.tsx`의 `handleSubmit`으로 반환한다.

**15. 결과 처리 — `app/page.tsx:408-441`**
`isComplete`가 true이면 `generateDepositAddress()`를 호출하여 Zcash 입금 주소와 QR코드를 생성하고 결제 플로우를 시작한다. false이면 `aiMessage`를 UI에 표시하여 사용자에게 누락된 필드를 안내한다.

## 노트 / 특이사항 / 주의점 (Notes / quirks / footguns)

- **NEAR AI SDK가 아닌 OpenAI 패키지 사용:** `lib/nearAI.ts`의 파일명과 주석은 "NEAR AI Cloud integration"이라고 표기하지만, 실제로는 `openai` npm 패키지를 사용하고 `baseURL`만 `https://cloud-api.near.ai/v1`로 교체한다. 파일 최상단 주석에도 "TEMPORARILY using OpenAI for testing"이라고 명시되어 있다 (`lib/nearAI.ts:3`). 실제 동작은 `OPENAI_API_KEY`가 있으면 OpenAI, 없으면 NEAR AI Cloud다.

- **AI 모델 이중 분기:** `OPENAI_API_KEY` 존재 여부에 따라 모델이 `gpt-4o-mini` (OpenAI) vs `deepseek-chat-v3-0324` (NEAR AI Cloud)로 나뉜다 (`lib/nearAI.ts:113-115`). 환경 변수 구성이 달라지면 동작하는 모델이 바뀌므로, 프로덕션 vs 개발 환경에서 응답 품질이 달라질 수 있다.

- **Prompt injection 노출면:** system prompt에 Supabase에서 조회한 서비스 이름/키워드/URL이 그대로 삽입된다 (`lib/nearAI.ts:69-71`, `getAllServicesForPrompt`). 악의적인 서비스 이름이나 키워드를 DB에 등록하면 LLM을 조작할 수 있는 prompt injection 취약점이 존재한다.

- **API 키 미설정 시 rule-based fallback:** `OPENAI_API_KEY`도 `NEAR_AI_API_KEY`도 없으면 `parsePromptFallback`이 실행된다 (`lib/nearAI.ts:54-56`). 이 fallback은 regex로 숫자, 도메인, 0x 주소를 추출하며 LLM을 전혀 사용하지 않는다 (`lib/nearAI.ts:224-276`).

- **서버 전용 vs 클라이언트 전용 분리:** `lib/intentParser.ts`는 브라우저에서 실행되고 `lib/nearAI.ts`는 Next.js API route(서버)에서만 실행된다. API 키가 서버측 환경 변수로만 노출되므로 클라이언트에 키가 유출되지 않는 구조다.

- **`user_account` 미사용:** API route가 `user_account`를 body에서 파싱하지만 어디서도 사용하지 않는다 (`app/api/parse-intent/route.ts:19`). 추후 기능을 위한 자리 표시자로 보인다.

- **`detectChainForDomain` 미구현:** `lib/nearAI.ts:279-289`의 `detectChainForDomain`은 주석에 "In production, this would query a registry or API"라고 명시하며, 현재는 `.near`/`.sol` 도메인 suffix heuristic과 default `'ethereum'` 반환만 구현되어 있다. Base나 Solana 주소를 recipient로 받았을 때 체인 감지가 정확하지 않을 수 있다.

- **서비스 매칭 임계값 불일치:** `findBestService`의 기본 임계값은 `0.7`이지만 (`lib/serviceRegistry.ts:168`), `analyzePromptWithNearAI`에서 명시적으로 `0.6`을 전달한다 (`lib/nearAI.ts:32`). README의 "임계값 0.6" 주장은 코드의 명시적 인자와 일치한다.

- **비용 특성:** 완전한 happy path에서는 OpenAI API 호출이 최소 2회 발생한다: 서비스 시맨틱 검색용 embedding 호출 1회 + chat completion 1회. 서비스가 매칭되면 embedding 호출만 발생하고 chat completion은 생략된다.

- **`getAllServicesForPrompt`의 `require()` 사용:** `lib/nearAI.ts:216`에서 `const { getAllServices } = require('./serviceRegistry')`를 사용하는데, 이는 ES Module 파일에서 CommonJS `require()`를 동적으로 호출하는 것으로, 순환 import를 피하려는 의도로 보이지만 TypeScript 타입 안전성이 깨진다.

## 답한 open questions (from the spec §7)

**Q: Which AI provider does Pay Anyone Legend actually call?**
A: 두 AI provider를 동시에 지원한다. `OPENAI_API_KEY` 환경 변수가 있으면 OpenAI (`gpt-4o-mini` 모델, `https://api.openai.com/v1`)를 사용하고, 없으면 NEAR AI Cloud (`deepseek-chat-v3-0324` 모델, `https://cloud-api.near.ai/v1`)를 사용한다. 두 경우 모두 `openai` npm 패키지 클라이언트를 사용하며 `baseURL`만 교체한다 (`lib/nearAI.ts:7-11`, `lib/nearAI.ts:113-115`). 임베딩 생성은 항상 OpenAI API(`text-embedding-3-small`)를 사용한다 (`lib/serviceRegistry.ts:37-41`).

**Q: What is the prompt structure?**
A: system prompt는 "You are an AI assistant that analyzes payment intents for Anyone Pay." 지시로 시작하며, Supabase에서 가져온 서비스 목록을 삽입한 뒤 4개의 RULES와 REQUIRED FIELDS 명세, 그리고 완전한 intent와 불완전한 intent 각각의 예시 JSON을 포함한다 (`lib/nearAI.ts:67-110`). `response_format: { type: 'json_object' }`로 JSON 출력이 강제된다 (`lib/nearAI.ts:121`).

**Q: Where does intent parsing happen (client or server)?**
A: 파싱 로직은 완전히 서버에서 실행된다. `lib/intentParser.ts`는 클라이언트(브라우저)에서 실행되는 얇은 fetch 래퍼일 뿐이며, 실제 분석 로직(`analyzePromptWithNearAI`)은 Next.js API route인 `app/api/parse-intent/route.ts`에서 서버 사이드로만 실행된다. AI API 키도 서버 환경 변수에만 존재한다.

**Q: Is intent extraction rule-based, embedding-based, or LLM-completion-based?**
A: 세 가지 방식이 우선순위 순으로 혼합된다: (1) pgvector 임베딩 시맨틱 검색 (threshold 0.6) → (2) LLM chat completion (gpt-4o-mini 또는 deepseek-chat-v3-0324) → (3) regex rule-based fallback. 서비스 레지스트리에 매칭 서비스가 있으면 LLM 호출 없이 임베딩 경로만으로 처리된다 (`lib/nearAI.ts:32-50`).

**Q: §7 has no question specific to this subsystem regarding x402 or Zcash.**
A: Intent parser 자체는 x402 protocol이나 Zcash 처리와 직접적인 관계가 없다. 단, LLM에 hardcode된 system prompt에서 `bridgeFrom: 'zcash'`가 기본값으로 설정되어 있어 (`lib/nearAI.ts:94-95`, `lib/nearAI.ts:44`), 모든 결제가 Zcash 입금으로 시작한다는 아키텍처적 가정이 intent 파싱 단계부터 반영됨을 알 수 있다.
