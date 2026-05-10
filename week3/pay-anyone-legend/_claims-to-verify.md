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

- [ ] Zcash shielded transactions hide amounts, sender, and recipient — verify if real ZK proof execution is in the codebase or if it delegates to 1Click — lib/oneClick.ts, components/IntentsQR.tsx — §1.5 §1.3
- [ ] Automatic bridging from Zcash to Base/Solana via 1-Click API — verify 1Click integration exists and targets Base/Solana — lib/oneClick.ts — §1.5
- [x] AI-Powered Intent Recognition: natural language processing to understand payment intents — CONFIRMED: `analyzePromptWithNearAI` in `lib/nearAI.ts:29` calls pgvector embedding search (`lib/serviceRegistry.ts:37`) then LLM chat completion (`lib/nearAI.ts:112`) with `gpt-4o-mini` (OpenAI) or `deepseek-chat-v3-0324` (NEAR AI Cloud) — §1.1
- [ ] Semantic Service Matching: AI-powered search matches user queries to services (e.g., "Pay onlyfan" → OnlyFans) — verify pgvector similarity search is called — lib/serviceRegistry.ts — §1.2
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
- [ ] 1-Click API base URL is https://api.1click.fi — verify ONE_CLICK_API_URL env usage — lib/oneClick.ts — §1.5
- [ ] pgvector is used for semantic search — verify the vector extension and match_services function — lib/supabase.ts, lib/serviceRegistry.ts — §1.2
- [x] OpenAI is used for embeddings — CONFIRMED: `lib/serviceRegistry.ts:6-8` creates an `OpenAI` client; `lib/serviceRegistry.ts:37-41` calls `openai.embeddings.create({ model: 'text-embedding-3-small', input: text })` — §1.1 §1.2
- [~] NEAR AI Cloud is used for intent analysis — PARTIALLY CORRECT: `NEAR_AI_API_KEY` is read at `lib/nearAI.ts:7`, and if `OPENAI_API_KEY` is also set, OpenAI takes priority (`lib/nearAI.ts:7-11`). The codebase uses `openai` npm package for both; NEAR AI Cloud endpoint (`https://cloud-api.near.ai/v1`) is only used when `OPENAI_API_KEY` is absent. Comment says "TEMPORARILY using OpenAI for testing" (`lib/nearAI.ts:3`) — §1.1
- [ ] Supabase is used for both service storage and deposit tracking — verify two separate tables/schemas exist — supabase-setup.sql, supabase-deposit-tracking.sql — §1.2 §1.4
- [ ] QR Code payments: simple QR code scanning for Zcash deposits — verify QR code generation component — components/IntentsQR.tsx — §1.3
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

- [ ] payment_services table has fields: id, name, keywords, amount, currency, resource_key, contract_id, chain, description, active, embedding — verify in supabase-setup.sql — §1.2
- [ ] data_drops table has fields: id, service_id, resource_key, contract_id, encrypted_data, required_payment_amount, required_payment_token, intent_type, action, private_key_encrypted — verify in supabase-setup.sql — §1.2 §1.7
- [ ] match_services function performs semantic search using vector similarity with parameters query_embedding, match_threshold, match_count — verify in supabase-setup.sql — §1.2
- [ ] Vector similarity index exists on payment_services.embedding — verify CREATE INDEX statement — supabase-setup.sql — §1.2
- [ ] Two tables created by supabase-setup.sql: payment_services and data_drops — verify SQL file — §1.2
- [ ] pgvector extension required for payment_services (not deposit_tracking) — verify in SQL — supabase-setup.sql — §1.2

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

- [ ] The OpenAI client in `lib/serviceRegistry.ts` reads EITHER `OPENAI_API_KEY` or `NEAR_AI_API_KEY` as its API key (`lib/serviceRegistry.ts:8`); confirm which key is actually required for the service registry's embedding calls in practice — §1.2
- [ ] `getAllServicesForPrompt` at `lib/nearAI.ts:216` uses a dynamic `require('./serviceRegistry')` (CommonJS inside ESM); verify that this does not cause a runtime error in Next.js serverless functions — §1.1
- [ ] `detectChainForDomain` in `lib/nearAI.ts:279` falls back to `'ethereum'` as default chain, but the rest of the codebase only supports `'base'` and `'solana'`; confirm whether any code path actually calls this function in production and what happens when it returns `'ethereum'` — §1.1
- [ ] `lib/nearAI.ts` has hardcoded `bridgeFrom: 'zcash'` in both the service match path (`lib/nearAI.ts:44`) and the LLM system prompt example JSON (`lib/nearAI.ts:94`); verify that all intent paths ultimately produce `bridgeFrom: 'zcash'` — §1.1 §1.3
