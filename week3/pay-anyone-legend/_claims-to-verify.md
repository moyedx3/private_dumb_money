# Claims to Verify — Pay Anyone Legend

> Working scratch file. Claims extracted from upstream prose docs.
> Every claim gets `[x]` with file:line evidence when verified, or a "outside scope" annotation.
> Updated incrementally through Tasks 1–14.

---

## File-to-Subsystem Index

```tagged-files
app/api/content/get-url/route.ts          → x402
app/api/health/route.ts                   → shared
app/api/parse-intent/route.ts             → intent-parser
app/api/relayer/check-deposit/route.ts    → deposit-tracking
app/api/relayer/cronjob-check-deposits/route.ts → deposit-tracking
app/api/relayer/register-deposit/route.ts → deposit-tracking
app/api/relayer/submit-tx-hash/route.ts   → deposit-tracking
app/api/relayer/test-supabase/route.ts    → deposit-tracking
app/api/services/route.ts                 → service-registry
app/content/page.tsx                      → x402
app/layout.tsx                            → shared
app/page.tsx                              → shared
app/receipt/page.tsx                      → shared
components/AmbientBackground.tsx          → shared (unrelated — pure UI)
components/CreateServiceModal.tsx         → service-registry
components/FloatingInput.tsx              → shared (unrelated — pure UI)
components/IntentFlowDiagram.tsx          → intent-parser (UI layer)
components/IntentsQR.tsx                  → z-address
components/ServicesList.tsx               → service-registry
contract/Cargo.toml                       → rust-contract
contract/build.sh                         → rust-contract
contract/deploy.sh                        → rust-contract
contract/src/lib.rs                       → rust-contract
contract/test-contract.sh                 → rust-contract
contract/update-env.sh                    → rust-contract
lib/chainSig.ts                           → chain-signatures
lib/depositTracking.ts                    → deposit-tracking
lib/intentParser.ts                       → intent-parser
lib/kdf.ts                                → chain-signatures
lib/near.ts                               → chain-signatures
lib/nearAI.ts                             → intent-parser
lib/oneClick.ts                           → one-click
lib/serviceRegistry.test.ts               → service-registry
lib/serviceRegistry.ts                    → service-registry
lib/session.ts                            → shared
lib/sessionStore.ts                       → shared
lib/supabase-server.ts                    → service-registry
lib/supabase.ts                           → service-registry
scripts/run-cronjob.js                    → deposit-tracking
scripts/setup-supabase.ts                 → service-registry
scripts/test-sign-x402-transaction.js     → x402
scripts/validate-sql.ts                   → shared
```

---

## Claims extracted from upstream prose docs

### From README.md

- [x] Zcash shielded transactions hide amounts, sender, and recipient — REFUTED AS STATED: PAL performs no ZK proof execution. The deposit address is returned by 1Click API (`lib/oneClick.ts:126`); PAL only QR-displays it. All shielded tx logic (if any) is entirely inside 1Click (Defuse Protocol). The README claim is misleading — §1.3 §1.5
- [ ] Automatic bridging from Zcash to Base/Solana via 1-Click API — verify 1Click integration exists and targets Base/Solana — lib/oneClick.ts — §1.5
- [x] AI-Powered Intent Recognition: natural language processing to understand payment intents — CONFIRMED: `analyzePromptWithNearAI` in `lib/nearAI.ts:29` calls pgvector embedding search (`lib/serviceRegistry.ts:37`) then LLM chat completion (`lib/nearAI.ts:112`) with `gpt-4o-mini` (OpenAI) or `deepseek-chat-v3-0324` (NEAR AI Cloud) — §1.1
- [x] Semantic Service Matching: AI-powered search matches user queries to services (e.g., "Pay onlyfan" → OnlyFans) — CONFIRMED: `searchServicesSemantic` in `lib/serviceRegistry.ts:52-96` calls `match_services` RPC via `supabase.rpc('match_services', {...})` at `lib/serviceRegistry.ts:68`. pgvector `<=>` cosine distance operator is used in `supabase-setup.sql:69-74` — §1.2
- [ ] NEAR Chain Signatures: MPC-based key management for cross-chain transaction signing — verify v1.signer is called for tx signing — lib/chainSig.ts, lib/near.ts — §1.6
- [ ] x402 Payment Protocol: HTTP 402 standard with automatic payment verification and execution — verify 402 challenge/response cycle exists — app/api/content/get-url/route.ts, scripts/test-sign-x402-transaction.js — §1.7
- [ ] Server-side cronjobs handle payment verification and execution — verify cronjob exists in vercel.json and does deposit + x402 execution — app/api/relayer/cronjob-check-deposits/route.ts — §1.4
- [ ] Polling system tracks deposit and payment status — verify polling loop or status endpoint exists — lib/depositTracking.ts, app/api/relayer/check-deposit/route.ts — §1.4
- [ ] URL-Based State Persistence: Bookmarkable deposit links restore full payment state — verify payment state is encoded in URL — app/page.tsx, app/receipt/page.tsx — §0
- [~] Semantic similarity threshold default is 0.6 — PARTIALLY CORRECT: `findBestService` default param is 0.7 (`lib/serviceRegistry.ts:168`), but `analyzePromptWithNearAI` explicitly passes 0.6 when calling it (`lib/nearAI.ts:32`). The effective threshold for intent parsing is 0.6, but the library default is 0.7 — §1.1 §1.2
- [ ] NEAR contract address for x402 facilitator is x402.near — verify env var X402_FACILITATOR and any call to it — contract/src/lib.rs, lib/chainSig.ts — §1.8
- [ ] NEAR MPC contract used is v1.signer — verify NEAR_PROXY_CONTRACT_ID usage — lib/near.ts, lib/chainSig.ts — §1.6
- [ ] ethers v5.7.2 is used for Ethereum interactions — verify in package.json — §1.6
- [ ] chainsig.js is used as EVM chain adapter — verify import/usage — lib/chainSig.ts — §1.6
- [~] 1-Click API base URL is https://api.1click.fi — PARTIALLY CORRECT (URL DIFFERS): actual default is `https://1click.chaindefuser.com` at `lib/oneClick.ts:7`. `ONE_CLICK_API_URL` env var overrides it. The domain `1click.fi` is NOT used — §1.5
- [x] pgvector is used for semantic search — CONFIRMED: `CREATE EXTENSION IF NOT EXISTS vector` at `supabase-setup.sql:5`; `match_services` function defined at `supabase-setup.sql:36-78` uses `vector(1536)` type and `<=>` cosine distance; `supabase.rpc('match_services', ...)` called at `lib/serviceRegistry.ts:68` — §1.2
- [x] OpenAI is used for embeddings — CONFIRMED: `lib/serviceRegistry.ts:6-8` creates an `OpenAI` client; `lib/serviceRegistry.ts:37-41` calls `openai.embeddings.create({ model: 'text-embedding-3-small', input: text })` — §1.1 §1.2
- [~] NEAR AI Cloud is used for intent analysis — PARTIALLY CORRECT: `NEAR_AI_API_KEY` is read at `lib/nearAI.ts:7`, and if `OPENAI_API_KEY` is also set, OpenAI takes priority (`lib/nearAI.ts:7-11`). The codebase uses `openai` npm package for both; NEAR AI Cloud endpoint (`https://cloud-api.near.ai/v1`) is only used when `OPENAI_API_KEY` is absent. Comment says "TEMPORARILY using OpenAI for testing" (`lib/nearAI.ts:3`) — §1.1
- [x] Supabase is used for both service storage and deposit tracking — CONFIRMED: `payment_services` table in `supabase-setup.sql:8-22` (service registry); `deposit_tracking` table in `supabase-deposit-tracking.sql:5-25` (deposit tracking). Different Supabase clients: anon key (`lib/supabase.ts`) for service registry; service role key (`lib/supabase-server.ts`) for deposit tracking — §1.2 §1.4
- [x] QR Code payments: simple QR code scanning for Zcash deposits — CONFIRMED: `<QRCodeSVG value={depositAddress} size={220} level="H">` at `components/IntentsQR.tsx:186`. The QR encodes only the raw address string; no ZIP-321 URI (`zcash:zs1...?amount=...`) format is used — §1.3
- [ ] ONE_CLICK_JWT reduces swap fees (without JWT incurs 0.1% fee) — verify JWT is passed to 1Click calls — lib/oneClick.ts — §1.5

### From SETUP.md

- [ ] X402_FACILITATOR env var is set to x402.near — verify it is read and used in code — lib/chainSig.ts or contract/ — §1.7 §1.8
- [ ] NEXT_PUBLIC_INTENTS_CONTRACT env var set to intents.near — verify it is used in TS code — lib/near.ts or contract/ — §1.8
- [ ] NEXT_PUBLIC_CONTRACT_ID env var set to anyone-pay.near — verify it points to the NEAR Rust contract — lib/near.ts — §1.8
- [ ] Contract is deployed to anyone-pay.near — verify deploy.sh target account — contract/deploy.sh — §1.8

### From DEPLOY.md

- [ ] Vercel Cron Jobs configured in vercel.json — verify vercel.json has cron entry for /api/relayer/cronjob-check-deposits — vercel.json — §1.4
- [ ] Cronjob checks deposits every 5 seconds — verify schedule in vercel.json — vercel.json — §1.4
- [ ] POST /api/relayer/register-deposit — registers deposit addresses — verify route handler — app/api/relayer/register-deposit/route.ts — §1.4
- [ ] POST /api/relayer/check-deposit — checks deposit status — verify route handler — app/api/relayer/check-deposit/route.ts — §1.4
- [ ] POST /api/relayer/submit-tx-hash — submits transaction hash to speed up swap — verify route handler — app/api/relayer/submit-tx-hash/route.ts — §1.4 §1.5
- [ ] POST /api/relayer/refund — handles refunds — verify route handler exists — app/api/relayer/ — §1.4
- [ ] GET /api/relayer/cronjob-check-deposits — cronjob endpoint that checks deposits and executes x402 payments — verify handler logic — app/api/relayer/cronjob-check-deposits/route.ts — §1.4 §1.7
- [ ] Relayer is integrated into Next.js API routes (no separate Fly.io deployment) — verify no fly.toml or separate server — §0
- [ ] Contract initialized with args x402_facilitator and intents_contract — verify init call in deploy.sh — contract/deploy.sh — §1.8

### From DEPLOY_CONTRACT.md

- [ ] Contract method get_intent(intent_id: String) — view method exists — contract/src/lib.rs — §1.8
- [ ] Contract method create_intent(intent_id, intent_type, deposit_address, amount, redirect_url) — change method exists — contract/src/lib.rs — §1.8
- [ ] Contract method mark_funded(intent_id) — marks intent as funded, caller is "relayer only" — verify caller restriction — contract/src/lib.rs — §1.8
- [ ] Contract method execute_x402_payment(intent_id, amount, recipient) — executes x402 payment on-chain — verify implementation — contract/src/lib.rs — §1.7 §1.8
- [ ] Contract method verify_deposit(intent_id) — verifies deposit via NEAR Intents — verify implementation and what "verify via NEAR Intents" means — contract/src/lib.rs — §1.4 §1.8
- [ ] Contract is deployed to mainnet (anyone-pay.near) — verify target in deploy.sh — contract/deploy.sh — §1.8

### From SUPABASE_SETUP.md

- [~] payment_services table has fields: id, name, keywords, amount, currency, resource_key, contract_id, chain, description, active, embedding — PARTIALLY CORRECT: actual columns are `id, name, keywords, amount, currency, url, chain, receiving_address, description, active, embedding, created_at, updated_at` (`supabase-setup.sql:8-22`). No `resource_key` column (it's `url`), no `contract_id` column. `resource_key` appears only as a legacy fallback in `lib/serviceRegistry.ts:86` for old DB rows — §1.2
- [~] data_drops table has fields: id, service_id, resource_key, contract_id, encrypted_data, required_payment_amount, required_payment_token, intent_type, action, private_key_encrypted — NOT IN supabase-setup.sql: `data_drops` table is entirely absent from `supabase-setup.sql` (which only defines `payment_services`). Table either exists in a separate SQL file not found in the repo, or was created manually in the Supabase dashboard. Claim cannot be verified from codebase — §1.2 §1.7
- [x] match_services function performs semantic search using vector similarity with parameters query_embedding, match_threshold, match_count — CONFIRMED: `CREATE OR REPLACE FUNCTION match_services(query_embedding vector(1536), match_threshold float, match_count int)` at `supabase-setup.sql:36-78`. Uses cosine distance `<=>` and similarity threshold filter — §1.2
- [x] Vector similarity index exists on payment_services.embedding — CONFIRMED: `CREATE INDEX IF NOT EXISTS payment_services_embedding_idx ON payment_services USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100)` at `supabase-setup.sql:25-28`. IVFFlat algorithm with 100 lists. Note: HNSW is not used — §1.2
- [~] Two tables created by supabase-setup.sql: payment_services and data_drops — PARTIALLY INCORRECT: `supabase-setup.sql` creates only ONE table: `payment_services` (`supabase-setup.sql:8`). `data_drops` is absent. `deposit_tracking` is in a separate file (`supabase-deposit-tracking.sql:5`). Two SQL files total, each creating one table — §1.2
- [x] pgvector extension required for payment_services (not deposit_tracking) — CONFIRMED: `CREATE EXTENSION IF NOT EXISTS vector` at `supabase-setup.sql:5` (alongside `payment_services`). `supabase-deposit-tracking.sql` has no vector extension reference — §1.2

### From SUPABASE_DEPOSIT_TRACKING.md

- [ ] deposit_tracking table exists with columns: deposit_address (TEXT, PRIMARY KEY), quote_data (JSONB), deadline (TIMESTAMP), signed_payload (TEXT) — verify in supabase-deposit-tracking.sql — §1.4
- [ ] quote_data JSONB stores full quote from 1-Click API including deposit address, amounts (ZEC, USDC), exchange rates, all quote metadata — verify structure — lib/depositTracking.ts — §1.4 §1.5
- [ ] signed_payload column stores signed x402 payment payload after cronjob executes — verify column used in cronjob handler — app/api/relayer/cronjob-check-deposits/route.ts — §1.4 §1.7
- [ ] Cronjob calls OneClickService.getExecutionStatus to check 1Click swap status — verify the exact method call — lib/oneClick.ts, app/api/relayer/cronjob-check-deposits/route.ts — §1.4 §1.5
- [ ] Cronjob executes x402 payment only if 1Click status is SUCCESS — verify conditional logic — app/api/relayer/cronjob-check-deposits/route.ts — §1.4 §1.7
- [ ] System falls back to in-memory storage if Supabase is not configured — verify in-memory fallback in depositTracking.ts — lib/depositTracking.ts — §1.4
- [ ] check-deposit route retrieves signedPayload from Supabase; UI redirects to content page with signedPayload — verify redirect logic — app/api/relayer/check-deposit/route.ts, app/content/page.tsx — §1.4 §1.7

### From SUPABASE_ENV_VARS.md

- [ ] SUPABASE_SERVICE_ROLE_KEY is used for server-side operations (cronjobs, API routes) and bypasses RLS — verify the service role client is used in cronjob and relayer routes — lib/supabase-server.ts — §1.4
- [ ] Log message "✅ Supabase server client initialized" appears when service role key is present — verify in lib/supabase-server.ts — §1.4
- [ ] Log message "⚠️ Supabase service role key not found" appears when key is missing — verify in lib/supabase-server.ts — §1.4

### From SUPABASE_SETUP_INSTRUCTIONS.md

- [ ] deposit_tracking table primary key is deposit_address (TEXT) — verify in supabase-deposit-tracking.sql — §1.4
- [ ] deposit_tracking table does NOT require vector extension — verify SQL file — §1.4

---

### NEW claims discovered while reading intent parser (Task 1)

#### §1.1 — Intent parser

- [x] The OpenAI client in `lib/serviceRegistry.ts` reads EITHER `OPENAI_API_KEY` or `NEAR_AI_API_KEY` as its API key (`lib/serviceRegistry.ts:8`); confirm which key is actually required — CONFIRMED: `apiKey: process.env.OPENAI_API_KEY || process.env.NEAR_AI_API_KEY || ''` at `lib/serviceRegistry.ts:8`. In practice `OPENAI_API_KEY` is preferred; NEAR AI key is accepted as fallback. If neither is set, `generateEmbedding` warns and returns `null`, disabling semantic search (`lib/serviceRegistry.ts:31-34`) — §1.2
- [ ] `getAllServicesForPrompt` at `lib/nearAI.ts:216` uses a dynamic `require('./serviceRegistry')` (CommonJS inside ESM); verify that this does not cause a runtime error in Next.js serverless functions — §1.1
- [ ] `detectChainForDomain` in `lib/nearAI.ts:279` falls back to `'ethereum'` as default chain, but the rest of the codebase only supports `'base'` and `'solana'`; confirm whether any code path actually calls this function in production and what happens when it returns `'ethereum'` — §1.1
- [ ] `lib/nearAI.ts` has hardcoded `bridgeFrom: 'zcash'` in both the service match path (`lib/nearAI.ts:44`) and the LLM system prompt example JSON (`lib/nearAI.ts:94`); verify that all intent paths ultimately produce `bridgeFrom: 'zcash'` — §1.1 §1.3

---

### NEW claims discovered while reading service registry (Task 2)

#### §1.2 — Service registry

- [x] IVFFlat index is used (not HNSW) for pgvector similarity search — CONFIRMED: `USING ivfflat (embedding vector_cosine_ops) WITH (lists = 100)` at `supabase-setup.sql:26-28`. No HNSW index exists in the schema — §1.2
- [x] `receiving_address` column is a free-form TEXT with no chain/format constraint at DB level — CONFIRMED: `receiving_address TEXT` at `supabase-setup.sql:16`, no CHECK constraint. Validation only at API route level: chain must be 'base' or 'solana', but address format is not validated (`app/api/services/route.ts:78-90`) — §1.2
- [x] currency is USDC-only for new service registrations — CONFIRMED: API route validates `currency !== 'USDC'` and rejects at `app/api/services/route.ts:70-74`. SQL schema allows `DEFAULT 'USD'` but app layer enforces USDC — §1.2
- [x] `deleteService` performs soft-delete (active=false), not actual DELETE — CONFIRMED: `lib/serviceRegistry.ts:373-377` sets `active: false` without SQL DELETE — §1.2
- [x] `url` field is hidden from GET /api/services list response for security — CONFIRMED: `route.ts:44-45` destructures url out before responding; url is only included for `?id=` single-fetch (`route.ts:30`) — §1.2
- [~] `data_drops` table referenced by SUPABASE_SETUP.md exists in supabase-setup.sql — REFUTED: `data_drops` is completely absent from `supabase-setup.sql`. Only `payment_services` is defined. `data_drops` may exist in an undiscovered SQL file or was created manually — §1.2 §1.7
- [x] `deposit_tracking` table has `quote_data JSONB`, `deadline TIMESTAMP WITH TIME ZONE`, `signed_payload TEXT` columns — CONFIRMED: `supabase-deposit-tracking.sql:22-24` — §1.4
- [x] `deposit_tracking` table `deposit_address TEXT PRIMARY KEY` — CONFIRMED: `supabase-deposit-tracking.sql:6` — §1.4
- [x] `deposit_tracking` table RLS is disabled — CONFIRMED: `ALTER TABLE deposit_tracking DISABLE ROW LEVEL SECURITY` at `supabase-deposit-tracking.sql:52` — §1.4

---

### NEW claims discovered while reading z-address generation (Task 3)

#### §1.3 — Z-address generation (DEFINITIVE VERIFICATION)

- [x] **Spec §7 open question — "Verify the week2 claim that z-address generation is `crypto.getRandomValues + 'zs1' prefix`"** — VERDICT: **Partially Refuted / Corrected.** The code does NOT use `crypto.getRandomValues` + `'zs1'` prefix to synthesize a z-address. No such pattern exists in any `.ts`/`.js`/`.tsx` file. Instead, deposit address is **fully outsourced (Category C)** to the 1Click API: `lib/oneClick.ts:126` extracts `data.depositAddress` from the `/v0/quote` API response and `app/api/relayer/register-deposit/route.ts:66` re-extracts it. The `zs1test123` strings found in `contract/deploy.sh:54` and `contract/test-contract.sh:14` are hardcoded shell test literals for the NEAR contract's `create_intent()` method — they are not produced by any JavaScript runtime code path. Week2's "얕다 (shallow)" characterization is correct; the mechanism is C (outsourced), not B (synthetic mock). — §1.3 §7

- [x] **No Zcash native library imported** — CONFIRMED: `package.json` contains zero Zcash cryptography packages. `bech32@2.0.0` is present but used exclusively for cosmos/XRP Ledger address derivation in `lib/kdf.ts:164-165`. `bs58check@4.0.0` and `js-sha3@0.9.3` are similarly Zcash-unrelated. — §1.3 §3.4

- [x] **QR code carries raw address string only, not ZIP-321 URI** — CONFIRMED: `components/IntentsQR.tsx:186` passes `value={depositAddress}` (a plain string) to `<QRCodeSVG>`. No `zcash:` URI scheme or ZIP-321 `?amount=` parameter is constructed anywhere in the codebase. — §1.3

- [x] **deposit_address (Supabase PK) is the 1Click order tracking key** — CONFIRMED: `lib/oneClick.ts:141` calls `OneClickService.getExecutionStatus(depositAddress)` using the address as the lookup key. `app/api/relayer/cronjob-check-deposits/route.ts:34` iterates all deposits and calls `checkSwapStatus(depositAddress)`. The address doubles as both the Zcash receive address AND the 1Click swap order ID. — §1.3 §1.4 §1.5

#### §1.5 — 1Click integration (new observations)

- [x] **1Click API actual base URL is `https://1click.chaindefuser.com`** (not `https://api.1click.fi` as claimed) — CONFIRMED: `lib/oneClick.ts:7` sets `ONE_CLICK_API_URL = process.env.ONE_CLICK_API_URL || 'https://1click.chaindefuser.com'`. This is the Defuse Protocol / chaindefuser domain, distinct from the claimed `1click.fi`. — §1.5

- [x] **1Click SDK used: `@defuse-protocol/one-click-sdk-typescript@0.1.14`** — CONFIRMED: `package.json` and `lib/oneClick.ts:3-4` import `OneClickService` and `OpenAPI` from this package. — §1.5

- [x] **Zcash asset ID used with 1Click is `nep141:zec.omft.near`** — CONFIRMED: `lib/oneClick.ts:178` defines `ASSETS.ZCASH = 'nep141:zec.omft.near'`. This is the NEAR Intents (Defuse) wrapped ZEC token ID. — §1.5

#### §1.4 — Deposit tracking (new observations)

- [x] **`signedPayload` column stores the Ethereum transaction hash (not a Base64 payload)** — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:135` calls `updateDepositTracking(depositAddress, { signedPayload: transactionHash, ... })` where `transactionHash` is the return value of `signX402TransactionWithChainSignature()` — an Ethereum tx hash string. — §1.4 §1.7

- [x] **Cronjob does NOT use a webhook from 1Click; it polls 1Click via SDK** — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:34` calls `checkSwapStatus(depositAddress)` which calls `OneClickService.getExecutionStatus(depositAddress)` at `lib/oneClick.ts:141`. No inbound webhook handler exists. — §1.4 §1.5
