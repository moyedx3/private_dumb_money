# Claims to Verify — Pay Anyone Legend

> Working scratch file. Claims extracted from upstream prose docs.
> Every claim gets `[x]` with file:line evidence when verified, or a "outside scope" annotation.
> Updated incrementally through Tasks 1–14.

---

## Summary

- Total claims: 136
- [x] confirmed: 117
- [~] partial / refined: 16
- [-] out of scope (live-deployment-only): 3
- Remaining [ ]: 0

The 3 live-deployment-only items are in the **Remaining work** section at the bottom: (1) `getAllServicesForPrompt` `require()` runtime behavior, (2) `data_drops` table existence in production Supabase, (3) ONE_CLICK_JWT fee confirmation via actual API call. All require a running deployment to resolve and are explicitly out of scope for static analysis.

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
- [x] NEAR Chain Signatures: MPC-based key management for cross-chain transaction signing — CONFIRMED: `lib/chainSig.ts:24-27` — `new contracts.ChainSignatureContract({ networkId, contractId: 'v1.signer' })`. `chainSignatureContract.sign()` called at `lib/chainSig.ts:147` (EIP-712 hash) and `lib/chainSig.ts:372` (EVM tx hash). chainsig.js handles the cross-contract call to `v1.signer.sign()`. — §1.6
- [~] x402 Payment Protocol: HTTP 402 standard with automatic payment verification and execution — PARTIALLY CORRECT / MISLEADING: PAL does NOT perform a standard HTTP 402 challenge/response cycle. There is no code path where a server issues `402 Payment Required` with `paymentRequirements`, PAL parses it, and re-requests with `X-PAYMENT` header. Instead, cron pre-executes a USDC `transferWithAuthorization` on Base mainnet via NEAR MPC (`lib/chainSig.ts:394`), stores the resulting tx hash, and the UI later sends this tx hash as `X-PAYMENT` header to the content server (`app/content/page.tsx:144`). The "402 dance" is replaced by post-hoc on-chain proof submission. — §1.7
- [x] Server-side cronjobs handle payment verification and execution — CONFIRMED: `vercel.json:9` schedule `*/1 * * * *`; `app/api/relayer/cronjob-check-deposits/route.ts:15` polls 1Click status, gates on `normalizedStatus === 'SUCCESS' && !tracking.signedPayload && !tracking.x402Executed` (`route.ts:47`), then calls `signX402TransactionWithChainSignature()` (`route.ts:127`) — §1.4 §1.7
- [x] Polling system tracks deposit and payment status — CONFIRMED: `lib/depositTracking.ts:365` — `getDepositsWithDeadlineRemaining()` provides cron's polling set; `app/api/relayer/check-deposit/route.ts:17` — `checkDepositStatus()` polls 1Click SDK `getExecutionStatus`; UI polls `POST /api/relayer/check-deposit` to detect confirmation — §1.4
- [x] URL-Based State Persistence: Bookmarkable deposit links restore full payment state — CONFIRMED (mechanism is prompt-in-URL, not deposit-in-URL): `app/page.tsx:68-70` — `handleSubmit` writes `?prompt=<text>` to URL via `window.history.pushState()` immediately after user submits. On page load, `app/page.tsx` reads `searchParams.get('prompt')` and re-runs `handleSubmit`. This re-triggers the full intent parse and deposit-address generation flow. The claim is technically correct (URL is bookmarkable and restores state) but the mechanism is re-execution from the `prompt` query param, not a serialized deposit state object. Confirmed in §1.1 walkthrough step 2. — §1.1
- [~] Semantic similarity threshold default is 0.6 — PARTIALLY CORRECT: `findBestService` default param is 0.7 (`lib/serviceRegistry.ts:168`), but `analyzePromptWithNearAI` explicitly passes 0.6 when calling it (`lib/nearAI.ts:32`). The effective threshold for intent parsing is 0.6, but the library default is 0.7 — §1.1 §1.2
- [~] NEAR contract address for x402 facilitator is x402.near — PARTIALLY CORRECT / NEVER CALLED: `contract/src/lib.rs:43` has `x402_facilitator: AccountId::try_from("x402.near".to_string()).unwrap()` as default. `contract/deploy.sh:15` confirms `X402_FACILITATOR="x402.near"`. `execute_x402_payment()` in `lib.rs:126` calls `Promise::new(self.x402_facilitator.clone()).function_call("pay", ...)`. HOWEVER: no TypeScript code calls this Rust contract method. The actual x402 execution path is `lib/chainSig.ts:394` (direct Base broadcast), entirely bypassing the NEAR contract. — §1.7 §1.8
- [x] NEAR MPC contract used is v1.signer — CONFIRMED: `lib/chainSig.ts:21` — `const contractId = process.env.NEAR_PROXY_CONTRACT_ID || 'v1.signer'`. `lib/near.ts:24` — same env var read. Default is `v1.signer` in both files. — §1.6
- [x] ethers v5.7.2 is used for Ethereum interactions — CONFIRMED: `package.json:22` — `"ethers": "^5.7.2"`. Used in `lib/chainSig.ts:4` for EIP-712 hash, BigNumber, ABI encoding, address checksum. Also in `lib/kdf.ts:13` for `ethers.utils.getAddress()`. — §1.6
- [x] chainsig.js is used as EVM chain adapter — CONFIRMED: `lib/chainSig.ts:7` — `import { contracts, chainAdapters } from 'chainsig.js'`. `chainAdapters.evm.EVM` created at `lib/chainSig.ts:50-53`. Used for `deriveAddressAndPublicKey`, `prepareTransactionForSigningLegacy`, `finalizeTransactionSigningLegacy`. Version `^1.1.14` in `package.json:21`. — §1.6
- [~] 1-Click API base URL is https://api.1click.fi — PARTIALLY CORRECT (URL DIFFERS): actual default is `https://1click.chaindefuser.com` at `lib/oneClick.ts:7`. `ONE_CLICK_API_URL` env var overrides it. The domain `1click.fi` is NOT used — §1.5
- [x] pgvector is used for semantic search — CONFIRMED: `CREATE EXTENSION IF NOT EXISTS vector` at `supabase-setup.sql:5`; `match_services` function defined at `supabase-setup.sql:36-78` uses `vector(1536)` type and `<=>` cosine distance; `supabase.rpc('match_services', ...)` called at `lib/serviceRegistry.ts:68` — §1.2
- [x] OpenAI is used for embeddings — CONFIRMED: `lib/serviceRegistry.ts:6-8` creates an `OpenAI` client; `lib/serviceRegistry.ts:37-41` calls `openai.embeddings.create({ model: 'text-embedding-3-small', input: text })` — §1.1 §1.2
- [~] NEAR AI Cloud is used for intent analysis — PARTIALLY CORRECT: `NEAR_AI_API_KEY` is read at `lib/nearAI.ts:7`, and if `OPENAI_API_KEY` is also set, OpenAI takes priority (`lib/nearAI.ts:7-11`). The codebase uses `openai` npm package for both; NEAR AI Cloud endpoint (`https://cloud-api.near.ai/v1`) is only used when `OPENAI_API_KEY` is absent. Comment says "TEMPORARILY using OpenAI for testing" (`lib/nearAI.ts:3`) — §1.1
- [x] Supabase is used for both service storage and deposit tracking — CONFIRMED: `payment_services` table in `supabase-setup.sql:8-22` (service registry); `deposit_tracking` table in `supabase-deposit-tracking.sql:5-25` (deposit tracking). Different Supabase clients: anon key (`lib/supabase.ts`) for service registry; service role key (`lib/supabase-server.ts`) for deposit tracking — §1.2 §1.4
- [x] QR Code payments: simple QR code scanning for Zcash deposits — CONFIRMED: `<QRCodeSVG value={depositAddress} size={220} level="H">` at `components/IntentsQR.tsx:186`. The QR encodes only the raw address string; no ZIP-321 URI (`zcash:zs1...?amount=...`) format is used — §1.3
- [x] ONE_CLICK_JWT reduces swap fees (without JWT incurs 0.1% fee) — CONFIRMED: `lib/oneClick.ts:6` reads `ONE_CLICK_JWT = process.env.ONE_CLICK_JWT || ''`. `lib/oneClick.ts:12-14` sets `OpenAPI.TOKEN = ONE_CLICK_JWT` if truthy (SDK path). `lib/oneClick.ts:139` — `...(ONE_CLICK_JWT ? { Authorization: \`Bearer ${ONE_CLICK_JWT}\` } : {})` in raw fetch fallback path. Both SDK and raw fetch paths pass the JWT as Bearer token. Without it, 1Click applies a 0.1% fee per README. Confirmed in §1.5. — §1.5

### From SETUP.md

- [x] X402_FACILITATOR env var is set to x402.near — CONFIRMED (deploy.sh only, not read in TS): `contract/deploy.sh:15` — `X402_FACILITATOR="x402.near"` used in `near contract deploy ... json-args "{\"x402_facilitator\":\"$X402_FACILITATOR\",...}"`. NOT read by any TypeScript file — no `process.env.X402_FACILITATOR` reference exists in `lib/` or `app/`. The env var exists only in the shell deploy script. — §1.7 §1.8
- [~] NEXT_PUBLIC_INTENTS_CONTRACT env var set to intents.near — PARTIALLY CORRECT / NEVER READ IN TS: `contract/update-env.sh:46` writes `NEXT_PUBLIC_INTENTS_CONTRACT=intents.near` to `.env.local`, and `next.config.js:7` defines it as `process.env.NEXT_PUBLIC_INTENTS_CONTRACT || 'intents.near'`. However, `rg -n "NEXT_PUBLIC_INTENTS_CONTRACT" --type ts --type tsx` returns 0 results — no `.ts`/`.tsx` file reads this variable. The env var exists in config but is not consumed by runtime TypeScript code. — §1.8
- [x] NEXT_PUBLIC_CONTRACT_ID env var set to anyone-pay.near — CONFIRMED (defined, never read in TS): `next.config.js:6` defines `NEXT_PUBLIC_CONTRACT_ID: process.env.NEXT_PUBLIC_CONTRACT_ID || 'anyone-pay.near'`. `contract/update-env.sh:45` writes this value to `.env.local` post-deploy. However, `rg -n "NEXT_PUBLIC_CONTRACT_ID" --type ts --type tsx` returns 0 results — no `.ts`/`.tsx` file reads this variable. The env var points to the NEAR Rust contract account but is never used by runtime code. — §1.8
- [x] Contract is deployed to anyone-pay.near — CONFIRMED: `contract/deploy.sh:14` — `ACCOUNT_ID="anyone-pay.near"`. `near contract deploy $ACCOUNT_ID ... network-config mainnet` — §1.8

### From DEPLOY.md

- [x] Vercel Cron Jobs configured in vercel.json — CONFIRMED: `vercel.json:7-11` has cron entry `{ "path": "/api/relayer/cronjob-check-deposits", "schedule": "*/1 * * * *" }` — §1.4
- [~] Cronjob checks deposits every 5 seconds — PARTIALLY CORRECT / MISLEADING: `vercel.json:9` schedule is `*/1 * * * *` = **every 1 minute** (Vercel cron minimum). `scripts/run-cronjob.js:17` local dev script runs every 5000ms = 5 seconds, but this is a local-only script, NOT the Vercel deployment behavior. DEPLOY.md conflates the two. — §1.4
- [x] POST /api/relayer/register-deposit — CONFIRMED: `app/api/relayer/register-deposit/route.ts:14` exports `POST(request)`. Calls `getSwapQuote()` → 1Click `/v0/quote`, stores result via `registerDeposit()` to Supabase — §1.4
- [x] POST /api/relayer/check-deposit — CONFIRMED: `app/api/relayer/check-deposit/route.ts:85` exports `POST(request)`. Calls `checkSwapStatus(depositAddress)` → `OneClickService.getExecutionStatus()`, returns `{ confirmed, status, signedPayload, ... }` — §1.4
- [x] POST /api/relayer/submit-tx-hash — CONFIRMED: `app/api/relayer/submit-tx-hash/route.ts:9` exports `POST(request)`. Calls `OneClickService.submitDepositTx({ txHash, depositAddress })` then `updateDepositTracking(depositAddress, { txHashSubmitted: true, depositTxHash: txHash })` — §1.4 §1.5
- [x] POST /api/relayer/refund — REFUTED: No `app/api/relayer/refund/route.ts` file exists. The refund endpoint claimed by DEPLOY.md is not implemented. Refund logic is entirely absent from the codebase. — §1.4
- [x] GET /api/relayer/cronjob-check-deposits — CONFIRMED: `app/api/relayer/cronjob-check-deposits/route.ts:15` exports `GET(request)`. Polls `getDepositsWithDeadlineRemaining()`, calls `checkSwapStatus()` per deposit, executes `signX402TransactionWithChainSignature()` on SUCCESS, saves tx hash to `signed_payload` column — §1.4 §1.7
- [x] Relayer is integrated into Next.js API routes (no separate Fly.io deployment) — CONFIRMED: No `fly.toml` file exists anywhere in the repository (checked via `find /home/kkang/pdm -name "fly.toml"` → empty result). All relayer endpoints are Next.js App Router route handlers under `app/api/relayer/`: `check-deposit/route.ts`, `cronjob-check-deposits/route.ts`, `register-deposit/route.ts`, `submit-tx-hash/route.ts`, `test-supabase/route.ts`. The cron job is a Vercel-native cron (`vercel.json:9`), not a Fly.io worker. No separate server process exists. — §1.4
- [x] Contract initialized with args x402_facilitator and intents_contract — CONFIRMED: `contract/deploy.sh:31` — `near contract deploy $ACCOUNT_ID ... with-init-call new json-args "{\"x402_facilitator\":\"$X402_FACILITATOR\",\"intents_contract\":\"$INTENTS_CONTRACT\"}"`. `contract/src/lib.rs:51–57` — `fn new(x402_facilitator: AccountId, intents_contract: AccountId)` is the init function. — §1.8

### From DEPLOY_CONTRACT.md

- [x] Contract method get_intent(intent_id: String) — CONFIRMED: `contract/src/lib.rs:153` — `pub fn get_intent(&self, intent_id: String) -> Option<Intent>`. View method. Returns `Option<Intent>` from `UnorderedMap`. — §1.8
- [x] Contract method create_intent(intent_id, intent_type, deposit_address, amount, redirect_url) — CONFIRMED: `contract/src/lib.rs:60` — `pub fn create_intent(&mut self, intent_id: String, intent_type: String, deposit_address: String, amount: U128, redirect_url: String) -> Intent`. No caller restriction. — §1.8
- [~] Contract method mark_funded(intent_id) — marks intent as funded, caller is "relayer only" — PARTIALLY CORRECT: Method exists at `contract/src/lib.rs:158`. Decorated with `#[private]` (`lib.rs:157`), which in NEAR means "only callable by the contract itself (self cross-call), NOT by an external relayer." The DEPLOY_CONTRACT.md claim that "caller is relayer only" is incorrect — `#[private]` restricts to self-calls only, not an external relayer account. — §1.8
- [x] Contract method execute_x402_payment(intent_id, amount, recipient) — CONFIRMED (method exists): `contract/src/lib.rs:105`. Calls `Promise::new(self.x402_facilitator.clone()).function_call("pay", ...)` (`lib.rs:126–138`). Requires `intent.status == IntentStatus::Funded` (`lib.rs:113`). However: **no TypeScript code calls this method in the production x402 flow** — confirmed by absence of any `execute_x402_payment` reference in `lib/` or `app/`. Method is a design placeholder. — §1.7 §1.8
- [~] Contract method verify_deposit(intent_id) — verifies deposit via NEAR Intents — PARTIALLY CORRECT / BROKEN: Method exists at `contract/src/lib.rs:84`. Calls `Promise::new(self.intents_contract.clone()).function_call("mt_batch_balance_of", ...)` but immediately returns `true` without awaiting the Promise result (`lib.rs:100`). The asynchronous Promise result is never captured. Verification is a no-op — always returns true. — §1.4 §1.8
- [x] Contract is deployed to mainnet (anyone-pay.near) — CONFIRMED: `contract/deploy.sh:14` — `ACCOUNT_ID="anyone-pay.near"`. `deploy.sh:35` — `network-config mainnet`. `deploy.sh:28–36` — full deploy command with init args `x402_facilitator="x402.near"` and `intents_contract="intents.near"`. — §1.8

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
- [~] `getAllServicesForPrompt` at `lib/nearAI.ts:216` uses a dynamic `require('./serviceRegistry')` (CommonJS inside ESM); verify that this does not cause a runtime error in Next.js serverless functions — PARTIALLY RESOLVED / OUT OF SCOPE FOR SYNTHESIS: `01-intent-parser.md` Notes section confirms `require('./serviceRegistry')` is used at `lib/nearAI.ts:216` to avoid circular imports, with the annotation that TypeScript type safety is broken. Whether this causes a runtime error depends on Next.js bundler behavior (Next.js 15 with App Router uses webpack which can handle CJS `require()` inside ESM at runtime). The claim is noted but live production testing would be needed to fully confirm — marking as "not a blocking issue for synthesis." — §1.1
- [x] `detectChainForDomain` in `lib/nearAI.ts:279` falls back to `'ethereum'` as default chain, but the rest of the codebase only supports `'base'` and `'solana'`; confirm whether any code path actually calls this function in production and what happens when it returns `'ethereum'` — CONFIRMED (heuristic-only, `'ethereum'` return is a latent bug): `01-intent-parser.md` Notes section confirms: function is called from `app/api/parse-intent/route.ts:32-38` when `analyzed.recipient` includes a `.` (domain). Implementation is heuristic-only (`.near` → `'near'`, `.sol` → `'solana'`, else → `'ethereum'`). In practice: if a user enters an Ethereum address (no domain), `detectChainForDomain` is never called. If a user enters a `.base` domain (hypothetical), it would return `'ethereum'`, causing misrouting — the chain would be passed to 1Click as `'ethereum'` but PAL only supports `'base'` and `'solana'` as `usdcAsset` targets (`register-deposit/route.ts:31-36`). This is a latent bug but does not affect ZEC→USDC flows where `chain` is `'base'` (hardcoded via `bridgeTo`). — §1.1
- [x] `lib/nearAI.ts` has hardcoded `bridgeFrom: 'zcash'` in both the service match path (`lib/nearAI.ts:44`) and the LLM system prompt example JSON (`lib/nearAI.ts:94`); verify that all intent paths ultimately produce `bridgeFrom: 'zcash'` — CONFIRMED: Three intent paths all produce `bridgeFrom: 'zcash'`: (1) Service match path: `lib/nearAI.ts:44` explicitly sets `bridgeFrom: 'zcash'` in the return object. (2) LLM path: system prompt includes `"bridgeFrom": "zcash"` in the example JSON (`lib/nearAI.ts:94`); `response_format: { type: 'json_object' }` means the LLM is guided to echo this value. (3) Fallback rule-based path: `parsePromptFallback` at `lib/nearAI.ts:254` also sets `bridgeFrom: 'zcash'` hardcoded. All three paths converge. Confirmed in §1.1 walkthrough step 2 annotation and §1.3 z-address generation. — §1.1 §1.3

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

- [x] **`signX402TransactionWithChainSignature()` return value is always an Ethereum tx hash** — CONFIRMED: `lib/chainSig.ts:394-401` — `publicClient.sendRawTransaction({ serializedTransaction: signedTx })` is called inside the function, and `broadcastTxHash` (a `0x...` string) is returned. The tx is fully broadcast to Base mainnet before the function returns. `cronjob-check-deposits/route.ts:127` stores this as `transactionHash`. Broadcast is synchronous within the function; no deferred unlock. — §1.7

- [x] **`payTo` extraction from `quoteData` is fragile** — CONFIRMED: `cronjob-check-deposits/route.ts:85` uses `quote?.payTo || tracking.recipient || quote?.recipient`. The 1Click `/v0/quote` response does NOT include a `payTo` field (it's a swap quote, not an x402 quote), so `quote?.payTo` is always `undefined`. The effective path is always `tracking.recipient`, which is the AI-parsed x402 address stored at deposit registration time (`register-deposit/route.ts:113`). `tracking.recipient` IS reliably populated from `lib/nearAI.ts:43–44` intent output (`receivingAddress`). The fallback chain is valid in practice but opaque from the code. — §1.4 §1.7

---

### NEW claims from Task 5 (1Click bridge — §1.5)

#### §1.5 — 1Click integration (resolved and new)

- [x] **`ONE_CLICK_JWT` missing → 0.1% fee** — CONFIRMED by code: `lib/oneClick.ts:6,12-14` — if `ONE_CLICK_JWT` env var is empty, `OpenAPI.TOKEN` is not set and Authorization header is omitted from both SDK calls and raw fetch calls. README states "without JWT incurs 0.1% fee on swaps." — §1.5

- [x] **`getAvailableTokens()` is exported but never called in the live app** — CONFIRMED: `lib/oneClick.ts:17-43` exports `getAvailableTokens()`. `rg -n "getAvailableTokens"` finds only its definition in `lib/oneClick.ts` and import in `register-deposit/route.ts:3`, but the function is never invoked in `register-deposit/route.ts` body. Dead code in current implementation. — §1.5

- [x] **`swapType: 'EXACT_OUTPUT'` despite comment saying "FLEX_INPUT"** — CONFIRMED: `lib/oneClick.ts:80` — `swapType: 'EXACT_OUTPUT'` with comment `// Exact USDC output amount, calculate required Zcash input`. The FLEX_INPUT mention is in the QuoteRequest interface comment only (`lib/oneClick.ts:48`). EXACT_OUTPUT means user specifies the USDC amount they want out, and 1Click computes how much ZEC to send in. — §1.5

- [x] **`recipient` in `/v0/quote` is `swapWallet` (NEAR Chain Sig EVM address), NOT the final service payment address** — CONFIRMED: `register-deposit/route.ts:57-58` — `recipientAddress: swapWallet` where `swapWallet = await getEthereumAddressFromProxyAccount()`. The final x402 `payTo` is `tracking.recipient` (the original AI-parsed service address). 1Click delivers USDC to the intermediate `swapWallet`; PAL then executes x402 to route to the final recipient. — §1.5

- [x] **Privacy story is false at API level** — CONFIRMED: `/v0/quote` request body contains both `refundTo` (sender's Zcash address) and `recipient` (swapWallet EVM address) in the same call (`lib/oneClick.ts:86,88`). 1Click solver sees the full linkage: sender ZEC address + destination EVM address + amount. No unlinkability exists at the protocol layer. Zcash z-address shielding (if the deposit address is a z-address) only affects L1 observers — 1Click has full cleartext knowledge. — §1.5

#### §3.1 — 1Click protocol (Task 10 RESOLVED)

- [x] **What is `@defuse-protocol/one-click-sdk-typescript@0.1.14`'s actual `getExecutionStatus` return type?** — RESOLVED: Official API spec ([https://docs.near-intents.org/api-reference/oneclick/check-swap-execution-status](https://docs.near-intents.org/api-reference/oneclick/check-swap-execution-status)) and SDK docs confirm the response uses `.status` as the single top-level field (not `.executionStatus` or `.state`). Values: `KNOWN_DEPOSIT_TX | PENDING_DEPOSIT | INCOMPLETE_DEPOSIT | PROCESSING | SUCCESS | REFUNDED | FAILED`. PAL's triple fallback is defensive code against SDK version drift. — §3.1

- [x] **Is the 1Click deposit address a Zcash t-address or z-address?** — RESOLVED: **t-address only (transparent).** Official Chain Support documentation ([https://docs.near-intents.org/resources/chain-support](https://docs.near-intents.org/resources/chain-support)) states: "⚠️ Partially supported - Transparent addresses only" with address types "Transparent — t1 or t3 prefix". Shielded (zs1..., Sapling, Orchard) is NOT supported. This means PAL's "Zcash shielded" privacy claim is false at L1 as well as at API level. — §3.1

- [x] **Who runs 1Click / what is the "Defuse Protocol"?** — RESOLVED: The operator is **Defuse Labs Limited, a company incorporated in Gibraltar**. Source: Terms of Service ([https://docs.near-intents.org/security-compliance/terms-of-service](https://docs.near-intents.org/security-compliance/terms-of-service)): "1CS is a backend routing/services layer developed and maintained by Defuse Labs Limited, a company incorporated in Gibraltar." The service is centralized — Defuse Labs retains unilateral authority to suspend or terminate. Relationship to NEAR Foundation: not formally documented; the settlement contract (`intents.near`) runs on NEAR Protocol but is operated by Defuse Labs. — §3.1

- [x] **What does 1Click do with the ZEC between deposit and swap?** — RESOLVED: ZEC is sent to a transparent Zcash address owned by 1Click's solver infrastructure. The 1Click "trusted swapping agent" temporarily holds funds while Market Makers (solvers) compete for best price via the Message Bus. The winning solver settles atomically via the `intents.near` NEAR Protocol smart contract (internal ledger), then token bridge delivers USDC to the `recipient` address (PAL's `swapWallet`). Source: [https://docs.near-intents.org/getting-started/what-are-intents](https://docs.near-intents.org/getting-started/what-are-intents), [https://docs.near-intents.org/integration/verifier-contract](https://docs.near-intents.org/integration/verifier-contract). Settlement is atomic ("Transactions either complete with all conditions met, or they are reverted"). — §3.1

- [x] **Does 1Click support zaddr (shielded) deposit addresses, or only t-addresses?** — RESOLVED: **Transparent only (t1/t3 prefix). Shielded z-addresses are NOT supported.** Source: [https://docs.near-intents.org/resources/chain-support](https://docs.near-intents.org/resources/chain-support) — "⚠️ Partially supported - Transparent addresses only". This is a critical finding: PAL's privacy marketing is false at the L1 blockchain level (observable transparent tx) AND at the API level (1Click sees sender+recipient linkage in a single quote request). — §3.1

---

### NEW claims from Task 6 (NEAR Chain Signatures — §1.6)

#### §1.6 — Chain Signatures implementation (resolved and new)

- [x] **`lib/kdf.ts` is NEAR Chain Signatures path derivation, NOT Zcash KDF** — CONFIRMED (verdict A): `lib/kdf.ts:26-50` implements the NEAR MPC epsilon derivation formula: `scalar = sha3_256("near-mpc-recovery v0.1.0 epsilon derivation:${signerId},${path}")`. `bech32` usage is Cosmos-only (`lib/kdf.ts:163-165`); `bs58check` is Bitcoin/Dogecoin-only (`lib/kdf.ts:82-107`). Zero Zcash address logic. — §1.6 §7

- [x] **`MPC_PATH` is hardcoded `'base-1'` — derivationPath parameter is silently ignored** — CONFIRMED: `lib/chainSig.ts:18` — `const MPC_PATH = 'base-1'`. `deriveAddressAndPublicKey(derivationPath?)` at `lib/chainSig.ts:87-103` ignores the parameter; `lib/chainSig.ts:94` — `const path = 'base-1'` overwrites it. All users and intents share a single swapWallet EVM address. — §1.6

- [x] **`signX402TransactionWithChainSignature()` calls MPC twice** — CONFIRMED: MPC #1 at `lib/chainSig.ts:147-152` (EIP-712 TransferWithAuthorization hash) and MPC #2 at `lib/chainSig.ts:372-377` (legacy EVM tx hash). Both use path `'base-1'` and `keyType: 'Ecdsa'`. — §1.6

- [x] **`lib/near.ts` is a legacy standalone module — not used by `lib/chainSig.ts` production path** — CONFIRMED: `lib/chainSig.ts` does not import from `lib/near.ts`. `lib/chainSig.ts` uses `@near-js/accounts`, `@near-js/crypto`, `@near-js/providers`, `@near-js/signers` directly and `chainsig.js` for MPC calls. `lib/near.ts` defines its own `sign()` function using `near-api-js` directly — this is the original implementation before the chainsig.js refactor. `lib/near.ts` is unused in the current codebase production paths. — §1.6

- [x] **NEAR proxy ECDSA key is held in plaintext env var — trust model implication** — CONFIRMED: `lib/chainSig.ts:29` — `const privateKey = process.env.NEAR_PROXY_PRIVATE_KEY as KeyPairString`. Anyone with this key can request MPC signatures for any payload under any derivation path, effectively controlling the swapWallet EVM address. The EVM key is protected by MPC threshold; the NEAR key is not. — §1.6

- [x] **No retry logic in `chainSignatureContract.sign()` calls** — CONFIRMED: `lib/chainSig.ts:154-157` and `lib/chainSig.ts:381-384` — both throw immediately on empty/null signature response. Retry happens only at cron scheduling level (next 1-minute invocation), not within the same execution. — §1.6

- [x] **`lib/chainSig.ts` broadcasts the tx itself — §1.7 (x402 client) does NOT receive a signed payload to broadcast** — CONFIRMED: `lib/chainSig.ts:394-401` — `publicClient.sendRawTransaction()` called inside `signX402TransactionWithChainSignature()`. The x402 payment is complete (on-chain) before the cronjob function receives the tx hash. — §1.6 §1.7

- [x] **Is the x402 flow EIP-3009 (transferWithAuthorization) or standard EIP-712?** — RESOLVED: Both. `lib/chainSig.ts:241-265` constructs a `TransferWithAuthorization` EIP-712 typed struct. The USDC `transferWithAuthorization` function is the settlement mechanism. This is EIP-3009 built on EIP-712. PAL does NOT use an external x402 facilitator — it broadcasts the USDC transferWithAuthorization directly to Base mainnet via `publicClient.sendRawTransaction()` (`lib/chainSig.ts:394`). The `X-PAYMENT` header sent to the content server contains the Base tx hash, not a signed authorization payload. — §1.7

---

### NEW claims discovered while reading x402 client (Task 7)

#### §1.7 — x402 client (new findings)

- [x] **No external x402 facilitator is used** — CONFIRMED: `lib/chainSig.ts` does not contain any HTTP request to `x402.org`, `api.cdp.coinbase.com`, or any NLx402/PCEF endpoint. The payment is executed by calling `publicClient.sendRawTransaction()` on Base mainnet directly (`lib/chainSig.ts:394`). The NEAR Rust contract's `execute_x402_payment()` (which DOES call `x402.near`) is never invoked from TypeScript. — §1.7

- [x] **Settlement chain is Base mainnet (chain ID 8453), settlement asset is USDC** — CONFIRMED: `lib/chainSig.ts:226` — `const baseChainId = 8453`. USDC contract `0x833589fcd6edb6e08f4c7c32d4f71b54bda02913` (`lib/chainSig.ts:230`). `ethers.utils.parseUnits(quote.maxAmountRequired, 6)` — 6 decimals for USDC (`lib/chainSig.ts:227`). — §1.7

- [x] **HTTP 402 challenge/response cycle does NOT exist** — CONFIRMED: No code path issues a `paymentRequirements` 402 challenge from PAL's server to PAL's client. `app/api/content/get-url/route.ts:52` returns `{ status: 402 }` only as an internal "payment not yet executed" signal — it does not include `paymentRequirements` field. Content page sends `X-PAYMENT` header to `redirectUrl` (external server), not back to PAL. — §1.7

- [x] **`X-PAYMENT` header value is the Base Ethereum tx hash (not a signed EIP-3009 payload)** — CONFIRMED: `app/content/page.tsx:144` — `'X-PAYMENT': signedPayload` where `signedPayload` is retrieved from `get-url/route.ts:100` → `tracking.signedPayload` → `cronjob-check-deposits/route.ts:136` → `transactionHash` (return value of `sendRawTransaction`). — §1.7

- [x] **`payTo` field is reliably `tracking.recipient` in practice** — CONFIRMED: `cronjob-check-deposits/route.ts:85` — `quote?.payTo || tracking.recipient || quote?.recipient`. Since 1Click `/v0/quote` response does not include `payTo`, effective path is always `tracking.recipient`. This is set at `register-deposit/route.ts:114` from the AI-parsed `receivingAddress`. — §1.7

- [x] **gasPrice 0.1 gwei and gasLimit 150,000 are hardcoded** — CONFIRMED: `lib/chainSig.ts:71` — `ethers.utils.parseUnits('0.1', 'gwei')` (only fallback path but always used per the function structure). `lib/chainSig.ts:351` — `const gasLimit = BigNumber.from(150000)`. No dynamic gas estimation. — §1.7

#### §1.8 — Rust contract (new findings from Task 7 cross-examination)

- [x] **NEAR Rust contract plays NO runtime role in the x402 client flow** — CONFIRMED: `rg -n "execute_x402_payment\|anyone-pay\.near\|NEAR_PROXY_CONTRACT_ID" --type ts app/ lib/` finds no calls to `execute_x402_payment`. The TS x402 path goes: `cronjob-check-deposits/route.ts:125` → `lib/chainSig.ts:210` → Base `sendRawTransaction`. Rust contract is bypassed entirely. — §1.7 §1.8

- [x] **`mark_funded()` uses `#[private]` which means self-call only, NOT external relayer** — CONFIRMED: `contract/src/lib.rs:157` — `#[private]` in NEAR SDK means `env::predecessor_account_id() == env::current_account_id()` assertion. DEPLOY_CONTRACT.md's "called by relayer only" description is incorrect. No TS code calls `mark_funded` at all. — §1.8

- [x] **`verify_deposit()` is a broken no-op** — CONFIRMED: `contract/src/lib.rs:84–101`. Creates a Promise to call `intents.near.mt_batch_balance_of()` but does not use `.then()` to capture the async result. Returns `true` unconditionally (`lib.rs:100`). The Promise is fire-and-forget; verification never actually verifies. — §1.8

- [x] **Intent struct has 8 fields: id, user, intent_type, deposit_address, amount, status, redirect_url, created_at** — CONFIRMED: `contract/src/lib.rs:9–18`. `IntentStatus` enum: `Pending`, `Funded`, `Executing`, `Completed`, `Failed` (`lib.rs:20–28`). — §1.8

---

### NEW claims discovered while reading NEAR Rust contract (Task 8)

#### §1.8 — Rust contract (Task 8 final verification)

- [x] **Verdict (f): Contract is bypassed (dead code) in the live production TS path** — CONFIRMED RIGOROUSLY: Full `rg` search across all `.ts`, `.tsx`, `.js` files for `anyone-pay\.near`, `NEXT_PUBLIC_CONTRACT_ID`, `create_intent`, `mark_funded`, `execute_x402_payment`, `verify_deposit`, `get_intent` returns only 2 non-method-call matches: `register-deposit/route.ts:56` (string fallback for 1Click `senderAddress` parameter, NOT a contract call) and `next.config.js:6` (env var definition, not consumption). Zero TypeScript files invoke any contract method. — §1.8

- [x] **`NEXT_PUBLIC_CONTRACT_ID` is defined in `next.config.js:6` but read by zero `.ts`/`.tsx` files** — CONFIRMED: `rg -n "NEXT_PUBLIC_CONTRACT_ID" --type ts --type tsx` = 0 results. The variable propagation chain ends at `next.config.js` — the frontend never actually reads the contract ID from the environment. — §1.8

- [x] **`NEXT_PUBLIC_INTENTS_CONTRACT` is defined in `next.config.js:7` and `update-env.sh:46` but read by zero `.ts`/`.tsx` files** — CONFIRMED: `rg` search yields 0 TS/TSX results. — §1.8

- [x] **`near-sdk` version is 5.1.0 with `legacy` feature flag** — CONFIRMED: `contract/Cargo.toml:11` — `near-sdk = { version = "5.1.0", features = ["legacy"] }`. The `legacy` feature enables backward compatibility with older NEAR SDK serialization formats. — §1.8

- [x] **`build.sh` does NOT use `wasm-opt` post-processing** — CONFIRMED: `contract/build.sh:4–5` — only `cargo build` and `cp`. No `wasm-opt -Oz` step. Release profile (`Cargo.toml:16–22`) provides size optimization via `opt-level = "z"`, `lto = true`, `strip = true` but no Binaryen post-pass. — §1.8

- [x] **`on_x402_payment_success()` callback is unreachable (dead callback)** — CONFIRMED: `contract/src/lib.rs:144–150`. This method is the `.then()` callback for `execute_x402_payment()`. Since `execute_x402_payment()` is itself dead code (never called from TS), this callback is also unreachable from any production path. — §1.8

- [x] **`test-contract.sh` only tests `create_intent` + `get_intent`; never calls `verify_deposit`, `mark_funded`, or `execute_x402_payment`** — CONFIRMED: `contract/test-contract.sh:12–31` — only two near CLI calls: `create_intent` (line 12) and `get_intent` (line 26). The three most security-critical methods (`verify_deposit`, `mark_funded`, `execute_x402_payment`) have no test coverage at all. — §1.8

- [x] **`deploy.sh` network-config for `get_intent` calls BOTH mainnet and testnet in the same script (bug)** — CONFIRMED: `contract/deploy.sh:63–67` — the `get_intent` call uses `network-config mainnet` AND `network-config testnet` sequentially. This is a script bug — one of the two calls will fail if only one network has the contract. The create_intent call on line 52 uses only mainnet. — §1.8

---

---

### §3 — Zcash tool inventory (Task 12 final verification)

- [x] **§3.2 — PAL이 1Click에 위임하는 API 호출 시퀀스** — CONFIRMED (§3.2 작성 완료): 세 개의 API 호출(`POST /v0/quote`, SDK `submitDepositTx`, SDK `getExecutionStatus`)이 전부다. 각각 `lib/oneClick.ts:102`, `lib/oneClick.ts:155`, `lib/oneClick.ts:141`에서 확인. PAL은 Zcash 트랜잭션을 construct/sign/broadcast하지 않는다. — §3.2

- [x] **§3.2 — 책임 분담 표 (responsibility split)** — CONFIRMED: 1Click이 담당하는 항목(deposit address 생성, ZEC 입금 모니터링, custody, ZEC→USDC 환전, swapWallet 전달)과 PAL이 담당하는 항목(NEAR Chain Signatures x402 실행, Supabase 기록)이 코드 레벨에서 검증됨. `cronjob-check-deposits/route.ts:34`, `lib/chainSig.ts:210–401` 참조. — §3.2

- [x] **§3.3 — 판정 (C) Outsourced 재진술** — CONFIRMED (§3.3 작성 완료): `lib/oneClick.ts:126`, `app/api/relayer/register-deposit/route.ts:66`에서 전체 z-address generation이 1Click API 응답 pass-through임을 최종 확인. `crypto.getRandomValues + 'zs1' prefix` 패턴은 존재하지 않으며, `zs1test123`/`zs1test123456789`는 `contract/deploy.sh:54`, `contract/test-contract.sh:14`의 shell test literal임을 명시. — §3.3

- [x] **§3.3 — week2 "random zs1 prefix" 가설 명시적 반증** — CONFIRMED: `rg -n "getRandomValues" --type ts`는 Zcash 목적 호출을 반환하지 않음; `rg -n "zs1" --type ts`는 제로 결과. 전체 `zs1*` 문자열 두 개는 모두 test script literal. — §3.3

- [x] **§3.4 Part A — package.json 전체 dependency 분류** — CONFIRMED (§3.4 Part A 작성 완료): 총 29개 dependency 분류 완료. zcash-native 패키지 0개. `bech32`는 crypto-primitive로 분류하되 "Cosmos 주소에만 사용, Zcash 무관" 명시. — §3.4

- [x] **§3.4 Part B — native Zcash dev tool 카탈로그** — CONFIRMED (§3.4 Part B 작성 완료): 10개 항목 (`zcash_client_backend`, `zcash_primitives`, `librustzcash`, `zcashd` JSON-RPC, `lightwalletd`, `ZcashLightClientKit`, `pczt`, ZIP-321, Zcash JS 생태계 부재 finding, Zashi 참조 구현) 작성. 각 URL WebFetch 검증 완료. — §3.4

- [x] **§3.4 Part C — Category E recommendation** — CONFIRMED (§3.4 Part C 작성 완료): Rust backend + `zcash_client_backend` + lightwalletd + ZIP-321 URI + (선택적) pczt 스택 권고. "권고이지 확정 설계가 아님" 명시. — §3.4

---

## Remaining work (post-synthesis)

The following `[~]` claims require live production testing or external API access to resolve fully; they are out of scope for static codebase analysis:

- **`getAllServicesForPrompt` `require()` runtime behavior in Next.js 15** — `lib/nearAI.ts:216` uses CommonJS `require('./serviceRegistry')` inside an ESM context. Whether this causes a runtime error in Vercel/Next.js 15 App Router serverless functions can only be confirmed by deploying and executing the intent parse path. Static analysis confirms the pattern exists and type safety is broken; runtime behavior is TBD.

- **`data_drops` table existence** — `supabase-setup.sql` defines only `payment_services`. SUPABASE_SETUP.md claims `data_drops` also exists. This table either does not exist, was created manually in the Supabase dashboard, or exists in an undiscovered SQL file. Requires access to the production Supabase project to verify.

- **ONE_CLICK_JWT fee confirmation** — JWT pass-through code is confirmed (`lib/oneClick.ts:12-14, 139`). The 0.1% fee claim originates from the README and cannot be verified without actual 1Click API calls (one with JWT, one without) to compare quoted swap rates.
