# §1.2 Service registry (서비스 레지스트리)

## 목적 (Purpose)

Service registry 서브시스템은 PAL(Pay Anyone Legend)에 등록된 결제 가능 서비스 목록을 Supabase에 저장하고, 사용자 자연어 쿼리와 가장 유사한 서비스를 pgvector 코사인 유사도 검색으로 찾아주는 역할을 한다. 서비스마다 OpenAI `text-embedding-3-small` 임베딩이 insert 시점에 1회 생성되어 저장되며, 검색 시에는 쿼리만 다시 임베딩하여 저장된 벡터와 비교하는 비대칭 구조다. intent parser([§1.1 intent parser](./01-intent-parser.md))가 `findBestService`를 호출하는 진입점이 되고, 관리자 UI(`CreateServiceModal`)가 write path를 담당한다.

## 파일과 함수 (Files & functions)

- `supabase-setup.sql:1-106` — 핵심 DDL. `payment_services` 테이블, IVFFlat 인덱스, `match_services` PostgreSQL 함수, `updated_at` 트리거 정의
- `supabase-deposit-tracking.sql:1-53` — 별도 DDL. `deposit_tracking` 테이블 및 인덱스 (service registry와는 다른 테이블이지만 같은 Supabase 인스턴스)
- `lib/supabase.ts:1-37` — anon key 기반 Supabase 브라우저/서버 공용 클라이언트(`supabase`) 생성. 환경 변수 누락 시 `null` 반환
- `lib/supabase-server.ts:1-24` — service role key 기반 서버 전용 클라이언트(`supabaseServer`) 생성. RLS 우회 목적; deposit tracking 전용
- `lib/serviceRegistry.ts:1-384` — 서비스 레지스트리 전체 로직
  - `:7-9` — `OpenAI` 클라이언트 초기화 (API key: `OPENAI_API_KEY || NEAR_AI_API_KEY`)
  - `:30-46` — `generateEmbedding(text)` — `text-embedding-3-small`로 1536-dim 벡터 생성
  - `:52-96` — `searchServicesSemantic(query, threshold)` — 공개 함수; 쿼리 임베딩 → `match_services` RPC → `PaymentService[]` 반환, 실패 시 keyword fallback
  - `:101-162` — `searchServicesKeyword(query)` — 비공개 fallback; active 전체 조회 후 점수 기반 필터
  - `:168-171` — `findBestService(query, threshold=0.7)` — `searchServicesSemantic` 결과의 첫 번째 항목만 반환
  - `:176-209` — `getServiceById(id)` — ID로 단일 서비스 조회 (url 포함)
  - `:214-246` — `getAllServices()` — 활성 서비스 전체를 created_at 내림차순으로 반환
  - `:251-301` — `addService(service)` — insert-time 임베딩 생성 후 Supabase insert
  - `:306-362` — `updateService(id, updates)` — name/description/keywords 변경 시 임베딩 재생성
  - `:367-383` — `deleteService(id)` — 실제 삭제가 아닌 `active = false` soft-delete
- `lib/serviceRegistry.test.ts:1-88` — 통합 테스트 스크립트 (실제 Supabase 연결 필요)
- `app/api/services/route.ts:1-179` — Next.js API route; GET/POST/PUT/DELETE 구현
  - `:14-54` — GET: 전체 목록 또는 `?q=` 시맨틱 검색 또는 `?id=` 단일 조회. URL 필드는 보안상 id 조회 시에만 노출
  - `:57-113` — POST: currency=USDC, chain∈{base,solana}, receivingAddress 유효성 검증 후 `addService` 호출
  - `:116-146` — PUT: `?id=` 파라미터로 `updateService` 호출
  - `:149-178` — DELETE: soft-delete via `deleteService`
- `components/ServicesList.tsx:23-109` — 클라이언트 컴포넌트. `useEffect`에서 `GET /api/services` 호출, 서비스 목록 렌더링 (url 제외)
- `components/CreateServiceModal.tsx:13-276` — 클라이언트 컴포넌트. 서비스 생성 폼; chain은 `base`/`solana` 중 선택, currency는 USDC 고정
- `scripts/setup-supabase.ts:50-129` — supabase-setup.sql을 읽어 실행하는 유틸리티 스크립트. 단, Supabase JS 클라이언트는 DDL 직접 실행 불가 → 대시보드 수동 실행 안내만 제공

## 연결 (Wiring)

- **Inputs:**
  - 사용자 자연어 쿼리 문자열 — `lib/nearAI.ts:32`에서 `findBestService(prompt, 0.6)` 형태로 전달 ([§1.1 intent parser](./01-intent-parser.md) 참조)
  - 관리자 폼 제출 — `components/CreateServiceModal.tsx:41-54` POST body
  - `GET /api/services?q=` 검색 쿼리 — `app/api/services/route.ts:33-39`
  - `GET /api/services?id=` — payment flow에서 url 획득 목적

- **Outputs:**
  - `PaymentService | null` — intent parser에 반환되는 최우선 매칭 서비스
  - `PaymentService[]` — 전체 목록 또는 시맨틱 검색 결과 (url 숨김)
  - `PaymentService` with url — id 조회 시만 url 포함

- **Dependencies (internal):**
  - `lib/supabase.ts` → `supabase` (anon key 클라이언트); serviceRegistry.ts가 직접 import
  - `lib/supabase-server.ts` → `supabaseServer` (service role 클라이언트); deposit tracking에서만 사용 (service registry는 anon key 클라이언트 사용)

- **Dependencies (external):**
  - Supabase + pgvector: `payment_services` 테이블, `match_services` RPC (`supabase-setup.sql`)
  - OpenAI API: `text-embedding-3-small` 모델 (임베딩 생성)
  - `@supabase/supabase-js ^2.86.0`
  - `openai ^6.9.1`

## 라이브러리 (Libraries)

| Package | Version | Used for |
|---------|---------|----------|
| `@supabase/supabase-js` | `^2.86.0` | Supabase 클라이언트, `match_services` RPC 호출, CRUD 쿼리 (`lib/supabase.ts:2`, `lib/serviceRegistry.ts:4`) |
| `openai` | `^6.9.1` | `text-embedding-3-small` 임베딩 생성 (`lib/serviceRegistry.ts:5`, `:37`) |
| `next` | `^15.0.0` | API route (`app/api/services/route.ts`) 및 클라이언트 컴포넌트 인프라 |
| pgvector (PostgreSQL extension) | Supabase managed | `vector(1536)` 컬럼 타입 및 `<=>` 코사인 거리 연산자 (`supabase-setup.sql:5`, `:69`) |

## 워크스루 — happy path

### A. SQL 스키마 (supabase-setup.sql)

```sql
-- supabase-setup.sql:5
CREATE EXTENSION IF NOT EXISTS vector;

-- supabase-setup.sql:8-22
CREATE TABLE IF NOT EXISTS payment_services (
  id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
  name TEXT NOT NULL,
  keywords TEXT[] NOT NULL,
  amount TEXT NOT NULL,
  currency TEXT NOT NULL DEFAULT 'USD',
  url TEXT NOT NULL, -- Direct URL to content/service
  chain TEXT NOT NULL,
  receiving_address TEXT, -- Receiving address for Base, Solana, and USDC payments
  description TEXT,
  active BOOLEAN DEFAULT true,
  embedding vector(1536), -- OpenAI text-embedding-3-small dimension
  created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW(),
  updated_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- supabase-setup.sql:25-28 (IVFFlat index, cosine distance)
CREATE INDEX IF NOT EXISTS payment_services_embedding_idx
ON payment_services
USING ivfflat (embedding vector_cosine_ops)
WITH (lists = 100);

-- supabase-setup.sql:31-33 (partial index for active services)
CREATE INDEX IF NOT EXISTS payment_services_active_idx
ON payment_services (active)
WHERE active = true;
```

**IVFFlat 파라미터:** `lists = 100`. `m` / `ef_construction` 파라미터는 IVFFlat에 존재하지 않음(HNSW 전용). 빌드 시 centroids 100개 클러스터 사용. 검색 시 `ivfflat.probes` 기본값(1) 적용.

```sql
-- supabase-setup.sql:36-78 (match_services 함수)
CREATE OR REPLACE FUNCTION match_services(
  query_embedding vector(1536),
  match_threshold float,
  match_count int
)
RETURNS TABLE (
  id uuid, name text, keywords text[], amount text, currency text,
  url text, chain text, receiving_address text, description text,
  active boolean, similarity float
)
LANGUAGE plpgsql
AS $$
BEGIN
  RETURN QUERY
  SELECT
    payment_services.id, ...,
    1 - (payment_services.embedding <=> query_embedding) as similarity
  FROM payment_services
  WHERE
    payment_services.active = true
    AND payment_services.embedding IS NOT NULL
    AND 1 - (payment_services.embedding <=> query_embedding) > match_threshold
  ORDER BY payment_services.embedding <=> query_embedding
  LIMIT match_count;
END;
$$;
```

거리 메트릭: **코사인 거리** (`<=>` 연산자). `similarity = 1 - cosine_distance`로 유사도 변환 후 필터링.

```sql
-- supabase-setup.sql:81-92 (updated_at 트리거)
CREATE OR REPLACE FUNCTION update_updated_at_column()
RETURNS TRIGGER AS $$
BEGIN
  NEW.updated_at = NOW();
  RETURN NEW;
END;
$$ LANGUAGE plpgsql;

CREATE TRIGGER update_payment_services_updated_at
  BEFORE UPDATE ON payment_services
  FOR EACH ROW
  EXECUTE FUNCTION update_updated_at_column();
```

**RLS 설정:** `payment_services`에 대한 RLS 정책이 SQL 파일에 없음. 기본적으로 RLS 비활성화 상태이며, anon key 클라이언트(`lib/supabase.ts`)를 사용하므로 서버/클라이언트 모두 접근 가능하다.

**시드 데이터:** SQL 파일에 없음. 예제 INSERT는 주석으로 제공만 됨(`supabase-setup.sql:94-105`).

---

### B. Write path — 서비스 등록 happy path

**1. 관리자 폼 제출 — `components/CreateServiceModal.tsx:26-53`**

```typescript
// CreateServiceModal.tsx:38-54
const response = await fetch('/api/services', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    name: formData.name,
    keywords,           // string[] (콤마 분리 후 trim)
    amount: formData.amount,
    currency: formData.currency,  // 항상 'USDC' (UI에서 고정)
    url: formData.url,
    chain: formData.chain,        // 'base' 또는 'solana' (select)
    receivingAddress: formData.receivingAddress,
    description: formData.description,
  }),
})
```

UI에서 currency='USDC', chain∈{'base','solana'}가 이미 고정/선택됨. amount 최솟값 0.5 USDC 검증(`CreateServiceModal.tsx:31-36`).

**2. API route 유효성 검증 — `app/api/services/route.ts:57-112`**

```typescript
// route.ts:70-90
if (currency !== 'USDC') {
  return NextResponse.json({ error: 'Only USDC currency is supported' }, { status: 400 })
}
if (chain !== 'base' && chain !== 'solana') {
  return NextResponse.json({ error: 'Only Base and Solana chains are supported' }, { status: 400 })
}
if (!receivingAddress) {
  return NextResponse.json({ error: 'Receiving address is required' }, { status: 400 })
}
```

Route handler는 서버에서 실행되며 currency, chain, receivingAddress를 이중 검증한다.

**3. `addService` 호출 — `lib/serviceRegistry.ts:251-301`**

```typescript
// serviceRegistry.ts:259-260
const searchText = `${service.name} ${service.description || ''} ${service.keywords.join(' ')}`
const embedding = await generateEmbedding(searchText)
```

임베딩 생성 텍스트는 `name + description + keywords` 연결 문자열이다. 이 임베딩은 **insert 시점에 1회만 생성**된다.

**4. 임베딩 생성 — `lib/serviceRegistry.ts:37-41`**

```typescript
// serviceRegistry.ts:37-41
const response = await openai.embeddings.create({
  model: 'text-embedding-3-small',
  input: text,
})
return response.data[0].embedding  // number[1536]
```

OpenAI `text-embedding-3-small` 모델, 출력 차원 **1536**. API 키가 없으면 `null`을 반환하고 임베딩 없이 insert됨(semantic search에서 제외).

**5. Supabase insert — `lib/serviceRegistry.ts:264-280`**

```typescript
// serviceRegistry.ts:264-280
const { data, error } = await supabase
  .from('payment_services')
  .insert({
    name, keywords, amount, currency, url, chain,
    receiving_address: service.receivingAddress,
    description,
    active: service.active !== false,
    embedding: embedding,
  })
  .select()
  .single()
```

`updated_at` 트리거가 자동으로 설정됨(`supabase-setup.sql:89-92`). 응답에서 `PaymentService` 객체를 생성하여 API route로 반환.

**6. HTTP 201 응답 반환 — `app/api/services/route.ts:105`**

```typescript
return NextResponse.json(service, { status: 201 })
```

---

### C. Read path — 서비스 목록 조회

**7. ServicesList 마운트 — `components/ServicesList.tsx:27-42`**

```typescript
// ServicesList.tsx:27-42
useEffect(() => {
  loadServices()
}, [])

const loadServices = async () => {
  const response = await fetch('/api/services')
  const data = await response.json()
  setServices(data.services || [])
}
```

컴포넌트 마운트 시 `GET /api/services`를 호출한다.

**8. GET handler — `app/api/services/route.ts:43-46`**

```typescript
// route.ts:43-46
const services = await getAllServices()
// Remove URL from response for security
const servicesWithoutUrl = services.map(({ url, ...service }) => service)
return NextResponse.json({ services: servicesWithoutUrl })
```

`url` 필드는 보안상 목록 응답에서 제외된다. id로 단일 조회할 때만 url 포함.

**9. `getAllServices` — `lib/serviceRegistry.ts:214-246`**

```typescript
// serviceRegistry.ts:219-225
const { data, error } = await supabase
  .from('payment_services')
  .select('*')
  .eq('active', true)
  .order('created_at', { ascending: false })
```

벡터 연산 없이 단순 SELECT. active=true인 서비스만 내림차순 반환.

---

### D. Search path — intent parser → 시맨틱 검색

**10. intent parser에서 호출 — `lib/nearAI.ts:32`**

```typescript
// nearAI.ts:32
const matchedService = await findBestService(prompt, 0.6)
```

threshold 0.6을 명시적으로 전달. (라이브러리 기본값 0.7과 다름 — 아래 노트 참조)

**11. `findBestService` — `lib/serviceRegistry.ts:168-171`**

```typescript
// serviceRegistry.ts:168-171
export async function findBestService(query: string, threshold: number = 0.7): Promise<PaymentService | null> {
  const matches = await searchServicesSemantic(query, threshold)
  return matches.length > 0 ? matches[0] : null
}
```

**12. 쿼리 임베딩 생성 — `lib/serviceRegistry.ts:59-63`**

```typescript
// serviceRegistry.ts:59-63
const queryEmbedding = await generateEmbedding(query)
if (!queryEmbedding) {
  return searchServicesKeyword(query)  // keyword fallback
}
```

매 검색 시마다 쿼리 임베딩을 새로 생성한다(OpenAI API 1회 호출). 서비스 임베딩은 이미 DB에 저장된 값을 재사용.

**13. `match_services` RPC 호출 — `lib/serviceRegistry.ts:68-72`**

```typescript
// serviceRegistry.ts:68-72
const { data, error } = await supabase.rpc('match_services', {
  query_embedding: queryEmbedding,
  match_threshold: threshold,
  match_count: 10,
})
```

`match_count: 10`으로 최대 10개의 결과를 요청. 실제로는 `findBestService`에서 첫 번째만 사용.

**14. SQL 함수 내 필터링 — `supabase-setup.sql:71-76`**

```sql
WHERE
  payment_services.active = true
  AND payment_services.embedding IS NOT NULL
  AND 1 - (payment_services.embedding <=> query_embedding) > match_threshold
ORDER BY payment_services.embedding <=> query_embedding
LIMIT match_count;
```

코사인 거리(`<=>`)로 정렬 후 유사도(`1 - distance`) > threshold인 결과만 반환. threshold 미달 시 빈 배열 반환 → `findBestService`는 `null` 반환 → intent parser는 LLM chat completion 경로로 진입.

**15. 결과 매핑 및 반환 — `lib/serviceRegistry.ts:80-91`**

```typescript
// serviceRegistry.ts:80-91
return (data || []).map((row: any) => ({
  id: row.id, name: row.name, keywords: row.keywords || [],
  amount: row.amount, currency: row.currency,
  url: row.url || row.resource_key,  // legacy 컬럼 지원
  chain: row.chain,
  receivingAddress: row.receiving_address,
  description: row.description, active: row.active,
}))
```

`resource_key` fallback(`lib/serviceRegistry.ts:86`)이 있어 구 버전 데이터와 호환됨.

**16. intent parser에서 서비스 매칭 활용 — `lib/nearAI.ts:36-50`**

서비스가 매칭되면 LLM 호출 없이 `matchedService`의 `amount`, `currency`, `chain`, `url` 필드로 즉시 `AnalyzedIntent`를 구성한다([§1.1 intent parser](./01-intent-parser.md) §워크스루 step 5 참조).

---

### E. `data_drops` 테이블 (§1.7 cross-reference)

`supabase-setup.sql`에는 `data_drops` 테이블이 존재하지 않는다. SUPABASE_SETUP.md에서 언급한 `data_drops` 테이블은 실제 SQL 파일에서 발견되지 않음 — §1.7 x402 client 섹션에서 별도 확인 필요. `deposit_tracking` 테이블은 별도 파일(`supabase-deposit-tracking.sql`)에서 정의됨.

## 노트 / 특이사항 / 주의점 (Notes / quirks / footguns)

### 1. 실제 similarity threshold — 0.6 vs 0.7 불일치

**검증 결과:** 라이브러리 기본값과 intent parser 호출값이 다르다.

- `findBestService` 기본 파라미터: `threshold = 0.7` (`lib/serviceRegistry.ts:168`)
- `searchServicesSemantic` 기본 파라미터: `threshold = 0.7` (`lib/serviceRegistry.ts:52`)
- intent parser의 실제 호출: `findBestService(prompt, 0.6)` (`lib/nearAI.ts:32`)
- GET `/api/services?q=` 시맨틱 검색 API route: `searchServicesSemantic(query, 0.7)` (`app/api/services/route.ts:36`)

README의 "임계값 0.6" 주장은 **intent parser 경로에서만 정확**하다. 관리자 검색 API(`/api/services?q=`)는 0.7을 사용한다. 라이브러리 기본값은 0.7이며 명시적으로 0.6을 전달하는 호출자는 intent parser뿐이다.

### 2. 임베딩 재계산 비용

**서비스 저장 시:** 임베딩이 insert-time에 1회 생성 후 DB에 저장 → 이후 재계산 없음. name/description/keywords 변경 시에만 재생성(`lib/serviceRegistry.ts:312-317`).

**검색 시:** 쿼리 임베딩은 **매 검색마다 새로 생성**된다(`lib/serviceRegistry.ts:59`). 즉, 사용자가 자연어를 입력할 때마다 OpenAI embedding API 호출이 1회 발생한다. 캐싱 없음. 사용자 트래픽이 높으면 embedding API 비용이 선형으로 증가한다.

### 3. `receiving_address` 컬럼 — 형식 제약 없음

`supabase-setup.sql:16`에서 `receiving_address TEXT` — 타입이 `TEXT`이며 CHECK constraint가 없다. SQL 레벨에서는 어떤 체인의 주소도, 어떤 형식도 허용된다.

애플리케이션 레벨에서의 제약은 `app/api/services/route.ts:78-83`에서만 걸림:
- `chain ∈ {'base', 'solana'}`만 허용
- receivingAddress 존재 여부만 검사 (포맷 검증 없음)

UI(`components/CreateServiceModal.tsx:226-238`)에서 chain에 따라 placeholder를 `0x...` (Base) 또는 `Solana wallet address...`로 바꾸지만, 실제 포맷 검증은 하지 않는다. 즉, **Base EVM 주소를 Solana chain 서비스에, 또는 그 반대로 입력해도 DB 저장이 된다.** x402 결제 컨텍스트에서 `receiving_address`는 1Click 브릿지가 스왑 후 USDC를 보내는 최종 주소이므로, 체인-주소 불일치는 실제 자금 손실로 이어질 수 있다.

**Category E (x402 + Zcash) 관련성:** `receiving_address`는 ZEC 수신 주소가 아니라 **브릿지 완료 후 USDC를 받을 Base 또는 Solana 주소**다. Zcash는 입금 측(funding asset)이고, `receiving_address`는 결제 대상 주소다. `payment_token` 또는 `currency` 필드를 Zcash로 설정하는 경로는 없다 — currency는 API 레이어에서 USDC로 고정된다(`app/api/services/route.ts:70-74`).

### 4. `data_drops` 테이블 — 소스 불일치

SUPABASE_SETUP.md claims(`_claims-to-verify.md` §SUPABASE_SETUP.md 섹션)에서는 `data_drops` 테이블이 `supabase-setup.sql`에 있다고 명시하지만, **실제 `supabase-setup.sql`에는 `data_drops` 테이블이 존재하지 않는다.** `supabase-setup.sql`에는 `payment_services` 테이블만 정의됨. `data_drops`는 별도 SQL 파일이 있거나, 대시보드에서 수동으로 생성한 것으로 추정된다. §1.7 x402 client 섹션 조사 시 별도 확인 필요.

### 5. `deposit_tracking` 테이블과의 관계

`deposit_tracking` 테이블(`supabase-deposit-tracking.sql:5-25`)은 service registry와 같은 Supabase 인스턴스를 공유하지만 **별개의 테이블**이며, pgvector 확장을 사용하지 않는다. service registry가 읽거나 쓰는 테이블이 아니다. `deposit_tracking`의 `intent_type`, `chain`, `redirect_url` 컬럼이 service registry의 출력값(서비스의 `url`, `chain`)을 반영하는 구조이지만, 직접적인 FK 관계는 없다.

### 6. `supabase-server.ts`는 service registry에서 미사용

`lib/supabase-server.ts`(service role key 클라이언트)는 이름과 달리 service registry에서 import하지 않는다. `lib/serviceRegistry.ts:4`는 anon key 기반의 `lib/supabase.ts`만 import한다. service role 클라이언트는 deposit tracking API route들이 사용한다. 즉, **service registry 전체가 anon key로 동작**하며, RLS가 활성화되어 있다면 접근 제한을 받는다(현재는 RLS 정책 미설정이므로 무관).

### 7. keyword fallback — 임베딩 미설정 시 동작

`generateEmbedding`이 `null`을 반환하면(API 키 없음 또는 API 오류) `searchServicesKeyword`로 자동 fallback된다(`lib/serviceRegistry.ts:61-63`). keyword search는 DB에서 전체 목록을 가져와 JS에서 점수를 계산하는 방식이므로 서비스 수가 많아지면 비효율적이다. threshold 개념이 없어 낮은 품질의 매칭도 반환할 수 있다.

### 8. `resource_key` legacy 컬럼

`lib/serviceRegistry.ts:86`, `:150`, `:199`, `:236`, `:291`에서 `row.url || row.resource_key`로 url 필드를 읽는다. `supabase-setup.sql`에는 `url` 컬럼만 있고 `resource_key`는 없다. 이는 초기 스키마에 `resource_key`가 있었다가 `url`로 이름이 바뀐 것으로 추정되며, 이전 데이터와의 호환성을 위한 fallback이다.

### 9. `scripts/setup-supabase.ts` 한계

스크립트가 DDL 실행을 시도하지만 Supabase JS 클라이언트는 DDL을 직접 실행할 수 없다(`scripts/setup-supabase.ts:92-93`). 결국 수동 실행 안내만 출력하는 유틸리티 스크립트다. `supabase-setup.sql` 대비 추가 기능 없음.

## 답한 open questions (from the spec §7)

**Q: pgvector는 어떤 거리 메트릭을 사용하는가?**
A: 코사인 거리 (`<=>` 연산자). `supabase-setup.sql:27`의 인덱스가 `vector_cosine_ops`를 사용하며, `match_services` 함수(`supabase-setup.sql:69-74`)가 `1 - (embedding <=> query_embedding)` 공식으로 유사도를 계산한다.

**Q: Supabase는 service storage와 deposit tracking 모두에 사용되는가?**
A: 확인됨. 두 목적 모두 같은 Supabase 인스턴스를 사용하지만 별개의 테이블(`payment_services` vs `deposit_tracking`)과 별개의 클라이언트(`supabase` anon key vs `supabaseServer` service role key)로 분리되어 있다.

**Q: §7의 나머지 open questions는 이 서브시스템에 해당하지 않는다.**
intent parser, NEAR chain signatures, x402 facilitator, 1Click API 관련 질문들은 각각 §1.1, §1.6, §1.7, §1.5에서 다루어야 한다.
