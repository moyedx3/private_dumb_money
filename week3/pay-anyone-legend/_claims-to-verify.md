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
- [x] Automatic bridging from Zcash to Base/Solana via 1-Click API — CONFIRMED: `lib/oneClick.ts:169-179` defines `ASSETS.ZCASH = 'nep141:zec.omft.near'`, `ASSETS.USDC_BASE`, `ASSETS.USDC_SOLANA`. `register-deposit/route.ts:31-36` selects `usdcAsset` based on chain. `getSwapQuote()` called with `originAsset: ASSETS.ZCASH`, `destinationAsset: usdcAsset`. Both Base and Solana destination chains are supported. — §1.5
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

- [x] Vercel Cron Jobs configured in vercel.json — CONFIRMED: `vercel.json:7-11` has cron entry `{ "path": "/api/relayer/cronjob-check-deposits", "schedule": "*/1 * * * *" }` — §1.4
- [~] Cronjob checks deposits every 5 seconds — PARTIALLY CORRECT / MISLEADING: `vercel.json:9` schedule is `*/1 * * * *` = **every 1 minute** (Vercel cron minimum). `scripts/run-cronjob.js:17` local dev script runs every 5000ms = 5 seconds, but this is a local-only script, NOT the Vercel deployment behavior. DEPLOY.md conflates the two. — §1.4
- [x] POST /api/relayer/register-deposit — CONFIRMED: `app/api/relayer/register-deposit/route.ts:14` exports `POST(request)`. Calls `getSwapQuote()` → 1Click `/v0/quote`, stores result via `registerDeposit()` to Supabase — §1.4
- [x] POST /api/relayer/check-deposit — CONFIRMED: `app/api/relayer/check-deposit/route.ts:85` exports `POST(request)`. Calls `checkSwapStatus(depositAddress)` → `OneClickService.getExecutionStatus()`, returns `{ confirmed, status, signedPayload, ... }` — §1.4
- [x] POST /api/relayer/submit-tx-hash — CONFIRMED: `app/api/relayer/submit-tx-hash/route.ts:9` exports `POST(request)`. Calls `OneClickService.submitDepositTx({ txHash, depositAddress })` then `updateDepositTracking(depositAddress, { txHashSubmitted: true, depositTxHash: txHash })` — §1.4 §1.5
- [x] POST /api/relayer/refund — REFUTED: No `app/api/relayer/refund/route.ts` file exists. The refund endpoint claimed by DEPLOY.md is not implemented. Refund logic is entirely absent from the codebase. — §1.4
- [x] GET /api/relayer/cronjob-check-deposits — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:15` exports `GET(request)`. Polls `getDepositsWithDeadlineRemaining()`, calls `checkSwapStatus()` per deposit, executes `signX402TransactionWithChainSignature()` on SUCCESS, saves tx hash to `signed_payload` column — §1.4 §1.7
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

- [x] deposit_tracking table exists with columns: deposit_address (TEXT, PRIMARY KEY), quote_data (JSONB), deadline (TIMESTAMP), signed_payload (TEXT) — CONFIRMED: `supabase-deposit-tracking.sql:5-25`. All four claimed columns present. Additional columns not mentioned in claim: `intent_id, amount, recipient, swap_wallet_address, near_account_id, confirmed, x402_executed, tx_hash_submitted, deposit_tx_hash, chain, intent_type, redirect_url, swap_id` — §1.4
- [x] quote_data JSONB stores full quote from 1-Click API — CONFIRMED: `app/api/relayer/register-deposit/route.ts:80` sets `quoteData = quote` (full response); `registerDeposit()` stores it at `lib/depositTracking.ts:115`. Also stores `metadata` merged into quoteData (`register-deposit/route.ts:100-106`) — §1.4 §1.5
- [~] signed_payload column stores signed x402 payment payload after cronjob executes — PARTIALLY CORRECT / COLUMN NAME MISLEADING: column is used (`supabase-deposit-tracking.sql:24`), but the stored value is an **Ethereum transaction hash** (not a "signed payload" / Base64 encoded bytes). `cronjob-check-deposits/route.ts:135` stores `transactionHash` (return value of `signX402TransactionWithChainSignature()`) into `signedPayload` — §1.4 §1.7
- [x] Cronjob calls OneClickService.getExecutionStatus to check 1Click swap status — CONFIRMED: `lib/oneClick.ts:141` — `OneClickService.getExecutionStatus(depositAddress)` is the exact call. Called via `checkSwapStatus()` wrapper. Cronjob invokes at `app/api/relayer/cronjob-check-deposits/route.ts:34` — §1.4 §1.5
- [x] Cronjob executes x402 payment only if 1Click status is SUCCESS — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:47` — `if (normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed)` — §1.4 §1.7
- [x] System falls back to in-memory storage if Supabase is not configured — CONFIRMED: `lib/depositTracking.ts:26` — `const depositTracking = new Map<string, DepositTracking>()`. All CRUD functions check `if (supabaseServer)` first, fall back to this Map. **However**, in-memory Map does not survive across serverless invocations — effectively non-functional in Vercel deployment without Supabase — §1.4
- [x] check-deposit route retrieves signedPayload from Supabase; UI redirects to content page with signedPayload — CONFIRMED: `app/api/relayer/check-deposit/route.ts:193` returns `signedPayload: tracking?.signedPayload`. UI reads this field and redirects to content page — §1.4 §1.7

### From SUPABASE_ENV_VARS.md

- [x] SUPABASE_SERVICE_ROLE_KEY is used for server-side operations (cronjobs, API routes) and bypasses RLS — CONFIRMED: `lib/supabase-server.ts:6` reads `SUPABASE_SERVICE_ROLE_KEY`; `createClient(url, serviceKey)` at `lib/supabase-server.ts:17`. RLS is also explicitly disabled on the table (`supabase-deposit-tracking.sql:52`). `supabaseServer` is imported by all relayer routes via `lib/depositTracking.ts:2` — §1.4
- [x] Log message "✅ Supabase server client initialized" appears when service role key is present — CONFIRMED: `lib/supabase-server.ts:12` — `console.log('✅ Supabase server client initialized')` — §1.4
- [x] Log message "⚠️ Supabase service role key not found" appears when key is missing — CONFIRMED: `lib/supabase-server.ts:9` — `console.warn('⚠️ Supabase service role key not found. Using in-memory storage as fallback.')` — §1.4

### From SUPABASE_SETUP_INSTRUCTIONS.md

- [x] deposit_tracking table primary key is deposit_address (TEXT) — CONFIRMED: `supabase-deposit-tracking.sql:6` — `deposit_address TEXT PRIMARY KEY` — §1.4
- [x] deposit_tracking table does NOT require vector extension — CONFIRMED: `supabase-deposit-tracking.sql` has no `CREATE EXTENSION vector` reference. The file creates only the `deposit_tracking` table and its indexes. pgvector is in `supabase-setup.sql` only — §1.4

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

---

### NEW claims discovered while reading deposit tracking (Task 4)

#### §1.4 — Deposit tracking (additional)

- [x] **`POST /api/relayer/refund` does NOT exist** — CONFIRMED: DEPLOY.md claims this route exists but `app/api/relayer/` contains only: `check-deposit/`, `cronjob-check-deposits/`, `register-deposit/`, `submit-tx-hash/`, `test-supabase/`. No `refund/` directory. User refund on x402 execution failure is unimplemented. — §1.4

- [x] **Cron handler authentication is commented out** — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:17-21` contains commented-out CRON_SECRET check. The endpoint is publicly callable without any token. Same applies to `test-supabase` route (`app/api/relayer/test-supabase/route.ts:7` — no auth at all). — §1.4

- [x] **`deadline` in x402 execution is always re-computed as `Date.now() + 3600s`** — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:87` sets `const deadline = Math.floor(Date.now() / 1000) + 3600`. The quote's original deadline (stored in `quote_data`) is NOT used for the x402 `deadline` parameter — a fresh 1-hour deadline is synthesized at execution time. — §1.4 §1.7

- [x] **`nonce` for x402 is timestamp-derived** — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:88` sets `const nonce = \`0x${Date.now().toString(16)}\``. No cryptographically random nonce is used. Two executions within the same millisecond would produce the same nonce (extremely unlikely but theoretically possible). — §1.4 §1.7

- [x] **In-memory fallback is non-functional in Vercel serverless** — CONFIRMED: `lib/depositTracking.ts:26` defines `const depositTracking = new Map()` at module scope. In Next.js serverless (Vercel), each invocation is a separate Lambda; the Map is re-initialized empty. Data written in one invocation is not visible to another. Supabase is functionally required for the system to work. — §1.4

- [x] **`swapType: 'EXACT_OUTPUT'` is used in the 1Click quote** — CONFIRMED: `lib/oneClick.ts:80`. Despite the comment saying "FLEX_INPUT", the code sets `swapType: 'EXACT_OUTPUT'` — the user provides a target USDC output amount, and 1Click calculates how much ZEC input is needed. — §1.4 §1.5

#### §1.5 — 1Click status states (new)

- [~] **1Click `getExecutionStatus` response structure** — PARTIALLY RESOLVED: `cronjob-check-deposits/route.ts:37-40` and `check-deposit/route.ts:27-30` and `get-url/route.ts:70-73` all check `.status || .executionStatus || .state` in that order. The triple fallback pattern + `as any` cast confirms SDK response field is not known statically. `node_modules/@defuse-protocol/one-click-sdk-typescript` was not present in the local clone — SDK type definitions require Task 10 (§3.1) analysis of the SDK source. Known status values from `check-deposit/route.ts:6-15` docs comment: `PENDING_DEPOSIT`, `PROCESSING`, `SUCCESS`, `INCOMPLETE_DEPOSIT`, `REFUNDED`, `FAILED`. — §1.5

- [x] **1Click status `INCOMPLETE_DEPOSIT` does not trigger x402 execution or user notification** — CONFIRMED: `check-deposit/route.ts:65-68` returns `{ incompleteDeposit: true }` but neither the cronjob nor the check-deposit route performs any follow-up action. No automatic retry, no user alert, no re-deposit UI. The deposit enters a limbo state — cron skips it (not SUCCESS), and PAL has no refund endpoint. 1Click may eventually auto-refund (status `REFUNDED`) but that is 1Click's internal policy, not PAL behavior. — §1.4 §1.5

#### §1.7 — x402 trigger (new)

- [ ] **`signX402TransactionWithChainSignature()` return value is always an Ethereum tx hash** — `cronjob-check-deposits/route.ts:127-132` calls this function and stores the result as `transactionHash`. Verify in `lib/chainSig.ts` that the return type is indeed a tx hash string and not a signed-but-unbroadcast payload. Relevant for understanding whether x402 unlock is synchronous or deferred. — §1.7

- [x] **`payTo` extraction from `quoteData` is fragile** — CONFIRMED: `cronjob-check-deposits/route.ts:85` uses `quote?.payTo || tracking.recipient || quote?.recipient`. The 1Click `/v0/quote` response does NOT include a `payTo` field (it's a swap quote, not an x402 quote), so `quote?.payTo` is always `undefined`. The effective path is always `tracking.recipient`, which is the AI-parsed x402 address stored at deposit registration time (`register-deposit/route.ts:113`). `tracking.recipient` IS reliably populated from `lib/nearAI.ts:43–44` intent output (`receivingAddress`). The fallback chain is valid in practice but opaque from the code. — §1.4 §1.7

---

### NEW claims from Task 5 (1Click bridge — §1.5)

#### §1.5 — 1Click integration (resolved and new)

- [x] **`ONE_CLICK_JWT` missing → 0.1% fee** — CONFIRMED by code: `lib/oneClick.ts:6,12-14` — if `ONE_CLICK_JWT` env var is empty, `OpenAPI.TOKEN` is not set and Authorization header is omitted from both SDK calls and raw fetch calls. README states "without JWT incurs 0.1% fee on swaps." — §1.5

- [x] **`getAvailableTokens()` is exported but never called in the live app** — CONFIRMED: `lib/oneClick.ts:17-43` exports `getAvailableTokens()`. `rg -n "getAvailableTokens"` finds only its definition in `lib/oneClick.ts` and import in `register-deposit/route.ts:3`, but the function is never invoked in `register-deposit/route.ts` body. Dead code in current implementation. — §1.5

- [x] **`swapType: 'EXACT_OUTPUT'` despite comment saying "FLEX_INPUT"** — CONFIRMED: `lib/oneClick.ts:80` — `swapType: 'EXACT_OUTPUT'` with comment `// Exact USDC output amount, calculate required Zcash input`. The FLEX_INPUT mention is in the QuoteRequest interface comment only (`lib/oneClick.ts:48`). EXACT_OUTPUT means user specifies the USDC amount they want out, and 1Click computes how much ZEC to send in. — §1.5

- [x] **`recipient` in `/v0/quote` is `swapWallet` (NEAR Chain Sig EVM address), NOT the final service payment address** — CONFIRMED: `register-deposit/route.ts:57-58` — `recipientAddress: swapWallet` where `swapWallet = await getEthereumAddressFromProxyAccount()`. The final x402 `payTo` is `tracking.recipient` (the original AI-parsed service address). 1Click delivers USDC to the intermediate `swapWallet`; PAL then executes x402 to route to the final recipient. — §1.5

- [x] **Privacy story is false at API level** — CONFIRMED: `/v0/quote` request body contains both `refundTo` (sender's Zcash address) and `recipient` (swapWallet EVM address) in the same call (`lib/oneClick.ts:86,88`). 1Click solver sees the full linkage: sender ZEC address + destination EVM address + amount. No unlinkability exists at the protocol layer. Zcash z-address shielding (if the deposit address is a z-address) only affects L1 observers — 1Click has full cleartext knowledge. — §1.5

#### §3.1 — 1Click protocol (for Task 10)

- [ ] **What is `@defuse-protocol/one-click-sdk-typescript@0.1.14`'s actual `getExecutionStatus` return type?** — SDK type definitions needed. The triple `.status || .executionStatus || .state` fallback in PAL code suggests the SDK may have changed its schema between versions. Task 10 must read SDK source or npm published types. — §3.1

- [ ] **Is the 1Click deposit address a Zcash t-address or z-address?** — PAL does not validate the format of `depositAddress` returned by `/v0/quote`. If it's a t-address, Zcash shielding is irrelevant (transparent). If z-address, the shielded properties apply on Zcash L1 but not at the API level. Task 10 must check 1Click docs/API response samples. — §3.1

- [ ] **Who runs 1Click / what is the "Defuse Protocol"?** — `lib/oneClick.ts:1` cites `https://github.com/near-examples/near-intents-examples`. Base URL is `chaindefuser.com`. Task 10 must identify the operator, legal entity, solver network design, and whether the service is decentralized or centralized. — §3.1

- [ ] **What does 1Click do with the ZEC between deposit and swap?** — From PAL's perspective, ZEC goes into the deposit address and USDC comes out at `swapWallet`. The internal mechanism (NEAR Intents, solver auction, bridging protocol) is opaque to PAL. Task 10 must analyze 1Click's own documentation and NEAR Intents architecture. — §3.1

- [ ] **Does 1Click support zaddr (shielded) deposit addresses, or only t-addresses?** — Critical for PAL's privacy claim. If deposit addresses are t-addresses, the "Zcash shielded" marketing is entirely false even at L1. Task 10 must determine this from 1Click API docs or test responses. — §3.1
