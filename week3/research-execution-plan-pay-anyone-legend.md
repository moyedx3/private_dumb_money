# Pay Anyone Legend Research Execution Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Produce the per-subsystem deep-dive documentation in `week3/pay-anyone-legend/`, satisfying the spec at `week3/research-plan-pay-anyone-legend.md`.

**Architecture:** Bottom-up reading. One subsystem per file, each written from the same template. Big-picture and cross-cuts written last after all subsystem details are in.

**Tech Stack of the project under study:** Next.js 15 + Supabase + pgvector + NEAR Rust contract + NEAR Chain Signatures (`chainsig.js`) + 1Click SDK (`@defuse-protocol/one-click-sdk-typescript`) + OpenAI embeddings + various crypto libs (`bech32`, `bs58check`, `elliptic`, `ethers`, `viem`).

**Reference repo (read-only):** `/home/kkang/anyone-pay`

**Output language:** Korean prose with English technical terms (matches week2 deliverable style).

**Conventions used by every Task 1–8 (subsystem walkthrough):**

Each `week3/pay-anyone-legend/0X-*.md` file uses this template:

```markdown
# §1.X <Subsystem Name>

## Purpose
<2-3 sentence summary of what this subsystem is responsible for and why it
exists in Pay Anyone Legend's architecture.>

## Files & functions
- `<repo-path>:<line>` — <one-line description of what's there>
- ...

## Wiring
- **Inputs:** <what data/calls flow in, from where>
- **Outputs:** <what data/calls flow out, to where>
- **Dependencies (internal):** <which other subsystems it calls>
- **Dependencies (external):** <APIs, services, on-chain contracts>

## Libraries
| Package | Version | Used for |
|---------|---------|----------|
| ... | ... | ... |

## Walkthrough (happy path)
<Step-by-step trace through the code with file:line code excerpts. The
reader should be able to reconstruct the data flow without opening the
repo. ~10–30 numbered steps.>

## Notes / quirks / footguns
- <observations: things that surprised us, things that look broken,
  things that are mocked, places where the README's claim doesn't match
  the code, security concerns, performance concerns>

## Open questions answered for this subsystem
<Pull from the open-questions list in the spec; mark which were answered
here and copy the answer.>
```

---

## Task 0: Setup — scaffold the directory and the per-section files

**Files:**
- Create: `week3/pay-anyone-legend/README.md` (placeholder; written in full at Task 13)
- Create: `week3/pay-anyone-legend/01-intent-parser.md` through `08-near-rust-contract.md`
- Create: `week3/pay-anyone-legend/category-E-extraction.md`
- Create: `week3/pay-anyone-legend/zcash-tool-inventory.md`
- Create: `week3/pay-anyone-legend/_claims-to-verify.md` (working scratch — committed but treated as ephemeral; a place to log every concrete claim from upstream docs that we want to verify or refute against the code)

- [ ] **Step 0.1: Read all upstream prose docs end to end and capture concrete claims**

Read each of these and append every concrete, falsifiable claim to `_claims-to-verify.md`:
- Read: `/home/kkang/anyone-pay/README.md`
- Read: `/home/kkang/anyone-pay/SETUP.md`
- Read: `/home/kkang/anyone-pay/DEPLOY.md`
- Read: `/home/kkang/anyone-pay/DEPLOY_CONTRACT.md`
- Read: `/home/kkang/anyone-pay/SUPABASE_SETUP.md`
- Read: `/home/kkang/anyone-pay/SUPABASE_DEPOSIT_TRACKING.md`
- Read: `/home/kkang/anyone-pay/SUPABASE_ENV_VARS.md`
- Read: `/home/kkang/anyone-pay/SUPABASE_SETUP_INSTRUCTIONS.md`

A "concrete claim" = a statement that can be verified by reading code (e.g., "Cronjobs handle payment verification" → check that there's a cron, that it does verification). Skip marketing prose ("production-ready", "privacy-first").

For each claim, log: `- [ ] <claim verbatim> — <which file/route to inspect> — <subsystem tag §1.X>`

- [ ] **Step 0.2: Create empty subsystem files using the template**

For each of the 10 output files, create the file with the template skeleton (Purpose / Files & functions / Wiring / Libraries / Walkthrough / Notes / Open questions answered).

The README.md at this stage gets just a "Status: WIP" line and a list of the files-to-be-written linked. Full §0 content is filled at Task 13.

- [ ] **Step 0.3: Build a file-to-subsystem index**

Run inside the read-only clone:

```bash
cd /home/kkang/anyone-pay && find app lib components contract scripts -type f \( -name "*.ts" -o -name "*.tsx" -o -name "*.rs" -o -name "*.toml" -o -name "*.sql" -o -name "*.sh" -o -name "*.js" \) | sort
```

Tag every file with one of: `intent-parser | service-registry | z-address | deposit-tracking | one-click | chain-signatures | x402 | rust-contract | shared | unrelated`. Save the tagged list as a comment block at the top of `_claims-to-verify.md`. This is the index for Tasks 1–8.

- [ ] **Step 0.4: Commit the scaffold**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/
git commit -m "Scaffold week3 Pay Anyone Legend per-subsystem deep-dive structure"
```

---

## Task 1: §1.1 Intent parser

**Files (read):**
- `/home/kkang/anyone-pay/lib/intentParser.ts`
- `/home/kkang/anyone-pay/lib/nearAI.ts`
- `/home/kkang/anyone-pay/app/api/parse-intent/route.ts`
- `/home/kkang/anyone-pay/components/IntentFlowDiagram.tsx` (UI layer; skim only for usage)

**File (write):** `week3/pay-anyone-legend/01-intent-parser.md`

- [ ] **Step 1.1: Read the three core files in full**

Read `lib/intentParser.ts`, `lib/nearAI.ts`, `app/api/parse-intent/route.ts` in their entirety. Note function signatures and exported names.

- [ ] **Step 1.2: Trace the call path**

```bash
cd /home/kkang/anyone-pay && rg -n "parseIntent|nearAI|callAI" lib app components --type ts --type tsx
```

Identify which UI component triggers `POST /api/parse-intent`, what the route handler does with the body, which library function it calls, and where the result is returned.

- [ ] **Step 1.3: Identify the AI provider(s)**

Look for OpenAI client construction and NEAR AI client construction. Find:
- Which model is used (embedding model? chat model?)
- Where the API key is read
- What prompt template is used (if any) — copy it verbatim into the doc
- Whether intent extraction is rule-based, embedding-based, or LLM-completion-based

- [ ] **Step 1.4: Write the §1.1 file**

Fill the template. The Walkthrough section must trace from "user submits a string in the UI" through to "structured intent returned" with file:line excerpts.

Notes section must call out: any prompt-injection surface, whether the AI call is server-side only, fallback behaviors when the AI key is missing.

- [ ] **Step 1.5: Mark answered claims**

In `_claims-to-verify.md` mark all `§1.1`-tagged claims as `[x]` with one-line evidence. Move any newly-discovered claims about other subsystems into their respective sections.

- [ ] **Step 1.6: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/01-intent-parser.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.1 intent parser walkthrough"
```

---

## Task 2: §1.2 Service registry

**Files (read):**
- `/home/kkang/anyone-pay/lib/serviceRegistry.ts`
- `/home/kkang/anyone-pay/lib/serviceRegistry.test.ts`
- `/home/kkang/anyone-pay/lib/supabase.ts`
- `/home/kkang/anyone-pay/lib/supabase-server.ts`
- `/home/kkang/anyone-pay/app/api/services/route.ts`
- `/home/kkang/anyone-pay/supabase-setup.sql`
- `/home/kkang/anyone-pay/components/ServicesList.tsx`
- `/home/kkang/anyone-pay/components/CreateServiceModal.tsx`

**File (write):** `week3/pay-anyone-legend/02-service-registry.md`

- [ ] **Step 2.1: Read the SQL schema first**

Read `supabase-setup.sql`. Capture:
- Table names + column types
- Whether `pgvector` is used; embedding column dimensionality
- Any RLS policies, triggers, or stored functions
- Indexes (especially HNSW/IVFFlat for vector search)

Include the schema verbatim (or trimmed) in the doc's Walkthrough.

- [ ] **Step 2.2: Read the registry library**

Read `lib/serviceRegistry.ts` and the matching `*.test.ts`. Identify:
- The semantic search function — embedding model, similarity threshold, distance metric (cosine? L2?)
- CRUD surface
- Where embeddings get computed (insert-time? query-time?)

- [ ] **Step 2.3: Trace from UI to DB**

`components/CreateServiceModal.tsx` → `app/api/services/route.ts` (POST) → `lib/serviceRegistry.ts` → Supabase. Then for read path: `components/ServicesList.tsx` → GET → semantic search.

- [ ] **Step 2.4: Write the §1.2 file**

Walkthrough must include the SQL schema, the embedding pipeline (compute → insert), and the search pipeline (query → embed → similarity → threshold filter → results).

Notes must call out: similarity threshold default value (README says 0.6 — verify), whether OpenAI embedding API is called inline on every search (cost/latency), schema for service-receiving address fields (this matters for Task 3 z-address).

- [ ] **Step 2.5: Mark answered claims & commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/02-service-registry.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.2 service registry walkthrough"
```

---

## Task 3: §1.3 Z-address generation (verify the week2 mock claim)

**Files (read):**
Discovery first — find every place that creates or formats a `zs1...` address. Then read those files.

- [ ] **Step 3.1: Find all z-address generation sites**

```bash
cd /home/kkang/anyone-pay && rg -n "zs1|getRandomValues|generateZ|zcashAddress|createZcash|orchard|sapling" --type ts --type tsx --type rust --type js
```

Save the file:line list. Read each file fully.

- [ ] **Step 3.2: Determine what the code actually does**

For each generation site, determine: is this real Zcash address derivation (using bech32 + a real spending key + ZIP-32 derivation), or is it a synthetic string that just looks like a z-address? Specifically check:
- Is `bech32` (npm) used to encode a real Orchard/Sapling raw address bytes? Or just used to encode random bytes with `zs` HRP?
- Is there any real key derivation (HD path, ZIP-32, viewing key, spending key, IVK/OVK)?
- Where is the user expected to actually receive funds? Is the address ever used as a destination for a real Zcash tx, or is it just displayed in the QR and the deposit detection happens through a different channel (e.g., 1Click webhook)?

- [ ] **Step 3.3: Write the §1.3 file**

Walkthrough must include the verbatim code excerpt for the z-address generation function with file:line.

Notes section must answer:
- Is this a mock? (Yes / No / Hybrid)
- If yes, why does the system still work — what carries the actual deposit identification (a 1Click order id? a Supabase row id encoded in the QR? a different field in the QR payload?)
- What would break if a real user tried to send ZEC to this address from a normal Zcash wallet?
- This subsystem feeds the Zcash inventory in §3 — flag the file:line evidence to be reused in `zcash-tool-inventory.md` Task 12.

- [ ] **Step 3.4: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/03-z-address-generation.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.3 z-address generation walkthrough (mock verification)"
```

---

## Task 4: §1.4 Deposit tracking

**Files (read):**
- `/home/kkang/anyone-pay/lib/depositTracking.ts`
- `/home/kkang/anyone-pay/app/api/relayer/check-deposit/route.ts`
- `/home/kkang/anyone-pay/app/api/relayer/cronjob-check-deposits/route.ts`
- `/home/kkang/anyone-pay/app/api/relayer/register-deposit/route.ts`
- `/home/kkang/anyone-pay/app/api/relayer/submit-tx-hash/route.ts`
- `/home/kkang/anyone-pay/app/api/relayer/test-supabase/route.ts`
- `/home/kkang/anyone-pay/supabase-deposit-tracking.sql`
- `/home/kkang/anyone-pay/scripts/run-cronjob.js`

**File (write):** `week3/pay-anyone-legend/04-deposit-tracking.md`

- [ ] **Step 4.1: Read the deposit tracking SQL schema**

Read `supabase-deposit-tracking.sql`. Capture the table(s), states machine (e.g., `pending → bridging → completed`), and any timestamp columns.

- [ ] **Step 4.2: Read the cron handler**

Read `app/api/relayer/cronjob-check-deposits/route.ts` and `scripts/run-cronjob.js`. Identify:
- How often the cron runs (Vercel cron config in `vercel.json`)
- What it polls — Zcash chain directly? lightwalletd? 1Click status API? Supabase only?
- The state-transition logic

```bash
cd /home/kkang/anyone-pay && cat vercel.json
```

- [ ] **Step 4.3: Read the per-route handlers**

For each `app/api/relayer/*/route.ts`, capture: HTTP method, what it accepts, what it returns, what it writes to Supabase. Especially `submit-tx-hash` — does the user submit their own tx hash (manual proof), or is this server-discovered?

- [ ] **Step 4.4: Write the §1.4 file**

Walkthrough must include the state machine diagram (ASCII or list) and the polling loop pseudocode.

Notes must answer:
- Is the chain actually queried for the deposit, or is the system trusting either a 1Click webhook or user-submitted tx hash?
- This is the answer to one of the spec's open questions ("Is the Supabase deposit tracking actually verifying chain state, or just trusting a webhook from 1Click?")

- [ ] **Step 4.5: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/04-deposit-tracking.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.4 deposit tracking walkthrough"
```

---

## Task 5: §1.5 1Click bridge integration

**Files (read):**
- `/home/kkang/anyone-pay/lib/oneClick.ts`
- The 1Click SDK package: `node_modules/@defuse-protocol/one-click-sdk-typescript/` — read its `package.json` README plus the top-level exported types (no `node_modules` install needed; we can `npm pack`-style read by fetching from npm or from the unpacked dir if installed). For this task, **prefer reading the published documentation** since `node_modules/` may not exist locally.

**File (write):** `week3/pay-anyone-legend/05-one-click-bridge.md`

- [ ] **Step 5.1: Read `lib/oneClick.ts` end-to-end**

Capture every function exported, every endpoint of the 1Click API touched, every parameter shape.

- [ ] **Step 5.2: Find every call site**

```bash
cd /home/kkang/anyone-pay && rg -n "oneClick|OneClick|one-click|OneClickService|@defuse-protocol" --type ts --type tsx
```

Trace where these calls fit into the user flow — typically: register a quote, submit a deposit address, poll status, finalize.

- [ ] **Step 5.3: External research — the 1Click protocol itself**

Use WebFetch/WebSearch to get the **official 1Click documentation**:
- Try: https://docs.near-intents.org/ , https://near-intents.org/ , https://www.defuse.org/, GitHub `defuse-protocol`
- Capture: who runs it (Defuse Protocol, NEAR Foundation, etc.), what assets/chains, the on-chain Settlement architecture, the off-chain Solver architecture, the privacy/trust model
- This same content is what powers §3.1 in `zcash-tool-inventory.md` — write a short version here, and a longer version in Task 12.

- [ ] **Step 5.4: Write the §1.5 file**

Walkthrough must show: how Pay Anyone Legend's app calls 1Click, what it sends, what it expects back, who actually performs the ZEC→USDC swap (Defuse solver network), and how settlement assurance is achieved.

Notes must answer:
- Is the swap atomic from the user's perspective? What's the trust assumption?
- Where does ZEC actually live during the swap — does the user's deposit address belong to 1Click/Defuse, to a solver, or to Pay Anyone Legend itself?
- The "Z-address generation is faked" finding from Task 3 should reconcile here: if the address is fake, the deposit must hit a 1Click-controlled address.

- [ ] **Step 5.5: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/05-one-click-bridge.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.5 1Click bridge integration walkthrough"
```

---

## Task 6: §1.6 NEAR Chain Signatures (MPC)

**Files (read):**
- `/home/kkang/anyone-pay/lib/chainSig.ts`
- `/home/kkang/anyone-pay/lib/near.ts`
- `/home/kkang/anyone-pay/lib/kdf.ts`
- `/home/kkang/anyone-pay/lib/session.ts` and `/home/kkang/anyone-pay/lib/sessionStore.ts` (if related to MPC session keys)
- `/home/kkang/anyone-pay/scripts/test-sign-x402-transaction.js`

**File (write):** `week3/pay-anyone-legend/06-near-chain-signatures.md`

- [ ] **Step 6.1: Read `lib/chainSig.ts` end-to-end**

Identify the MPC signing entry point. What does it accept (a hash? a tx?), what does it return (an `r`, `s`, `v` triple? a signed serialized tx?), which NEAR contract is called (`v1.signer`?).

- [ ] **Step 6.2: Read `lib/kdf.ts`**

Determine: is this a Key Derivation Function for deriving deterministic Ethereum addresses from a NEAR account + a `path` string (the standard NEAR Chain Signatures pattern), or is it generic crypto-utility code? This answers another spec open question.

- [ ] **Step 6.3: Read `lib/near.ts`**

Capture: how the NEAR account is loaded (env var private key — `NEAR_PROXY_PRIVATE_KEY`), which `near-api-js` patterns are used.

- [ ] **Step 6.4: Read `scripts/test-sign-x402-transaction.js`**

This is the most concrete usage example — read it as if it's a tutorial.

- [ ] **Step 6.5: Write the §1.6 file**

Walkthrough must show: NEAR account → derive Ethereum address via KDF → build EVM tx → request MPC signature from `v1.signer` → assemble signed tx → broadcast.

Notes must call out:
- Trust model: anyone with `NEAR_PROXY_PRIVATE_KEY` can request signatures for any path, so the proxy account is effectively the security boundary.
- Latency / cost of an MPC signature in production.

- [ ] **Step 6.6: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/06-near-chain-signatures.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.6 NEAR Chain Signatures walkthrough"
```

---

## Task 7: §1.7 x402 client

**Files (read):**
- All TS files containing the substring `x402` or `402`. From discovery: `lib/chainSig.ts`, `lib/depositTracking.ts`, `app/api/relayer/check-deposit/route.ts`, `app/api/relayer/cronjob-check-deposits/route.ts`, `app/api/relayer/register-deposit/route.ts`, `app/api/content/get-url/route.ts`, `app/content/page.tsx`, `app/page.tsx`, `app/layout.tsx`
- `/home/kkang/anyone-pay/scripts/test-sign-x402-transaction.js`
- `/home/kkang/anyone-pay/contract/src/lib.rs`

**File (write):** `week3/pay-anyone-legend/07-x402-client.md`

- [ ] **Step 7.1: Trace where the 402 challenge originates**

```bash
cd /home/kkang/anyone-pay && rg -n "402|x402|X-PAYMENT|x-payment|paymentRequirements|paymentRequired" --type ts --type tsx --type js -g '!*.lock'
```

Identify which endpoint returns HTTP 402 and what payload it contains. Capture the exact `paymentRequirements` shape (asset, amount, recipient, facilitator URL, …).

- [ ] **Step 7.2: Trace the 402 response handler**

Find where the client (Pay Anyone Legend's own server, since this is server-to-server) parses the 402 challenge, signs the payment payload via Chain Signatures, and re-requests the resource with `X-PAYMENT` header.

- [ ] **Step 7.3: Identify the facilitator**

Determine: which x402 facilitator is being called? Coinbase's facilitator on Base (USDC)? NLx402 (Solana, PCEF)? A self-hosted one in `contract/src/lib.rs`? Capture the facilitator URL or contract id, and the chain/asset of settlement.

- [ ] **Step 7.4: Read the Rust contract for any x402-related logic**

Read `contract/src/lib.rs` — does the contract serve as the x402 facilitator (settling payments on NEAR)? Or is it doing something else (service registry on-chain, escrow)?

- [ ] **Step 7.5: Write the §1.7 file**

Walkthrough must show the full 402 dance: client requests resource → server returns 402 + requirements → client constructs payment payload → client signs (via Task 6) → client retries with `X-PAYMENT` → server (or facilitator) verifies → resource served.

Notes must answer:
- Which facilitator? Which chain? Which asset?
- This is the pivotal answer for §2 (Category-E extraction): is x402 here actually settling on Zcash, or on USDC-on-Base after a 1Click bridge?
- Compare directly to Secure Legion's `NLx402:<quote_hash>` memo pattern (week2 reference).

- [ ] **Step 7.6: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/07-x402-client.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.7 x402 client walkthrough"
```

---

## Task 8: §1.8 NEAR Rust contract

**Files (read):**
- `/home/kkang/anyone-pay/contract/Cargo.toml`
- `/home/kkang/anyone-pay/contract/src/lib.rs`
- `/home/kkang/anyone-pay/contract/build.sh`
- `/home/kkang/anyone-pay/contract/deploy.sh`
- `/home/kkang/anyone-pay/contract/test-contract.sh`
- `/home/kkang/anyone-pay/contract/update-env.sh`

**File (write):** `week3/pay-anyone-legend/08-near-rust-contract.md`

- [ ] **Step 8.1: Read Cargo.toml**

Capture dependencies (`near-sdk` version, x402-related crates if any), profile.release settings.

- [ ] **Step 8.2: Read `lib.rs` end-to-end**

Enumerate every public method (`#[near_bindgen]`/`#[payable]`/etc.). For each method capture: signature, what it stores in state, what it asserts, what it logs.

- [ ] **Step 8.3: Determine the contract's role**

After reading: is this contract (a) the x402 facilitator, (b) an on-chain service registry, (c) an escrow for payments, (d) something else, (e) some combination? This answers the open question "Does the NEAR Rust contract participate in the payment flow, or is it just service-registry / metadata?"

- [ ] **Step 8.4: Read the deploy/build scripts to understand the deployment model**

Capture: target (mainnet?), account id pattern, any one-time init args.

- [ ] **Step 8.5: Write the §1.8 file**

Walkthrough must include each public method with its signature and a one-line "what it does" annotation, plus the deployment model.

Notes must connect the contract to the rest: which TS code calls it, with what args.

- [ ] **Step 8.6: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/08-near-rust-contract.md week3/pay-anyone-legend/_claims-to-verify.md
git commit -m "Write §1.8 NEAR Rust contract walkthrough"
```

---

## Task 9: External research — x402 facilitator landscape (for §2.1)

**File (write):** Append a "Background reading: x402 facilitator landscape" section to `week3/pay-anyone-legend/category-E-extraction.md`. (The rest of `category-E-extraction.md` is written at Task 11.)

- [ ] **Step 9.1: Pull the x402 spec**

WebFetch:
- https://github.com/coinbase/x402 (the canonical repo and spec)
- Anything from Coinbase's docs on the Base x402 facilitator

Capture the wire format of `paymentRequirements` and the `X-PAYMENT` header.

- [ ] **Step 9.2: Pull what's known about NLx402**

WebSearch / WebFetch for "NLx402 PCEF" and any Perkins Coie Entrepreneur Fund references. Cross-check with the week2 finding that NLx402 is a Solana variant facilitator.

- [ ] **Step 9.3: Write the background section**

Should have:
- One-paragraph definition of HTTP 402 / x402.
- The two known facilitator implementations (Coinbase Base, NLx402 PCEF Solana) with the differences.
- A short summary of "what changes when settlement asset is privacy-preserving" — this is the angle for §2.

- [ ] **Step 9.4: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/category-E-extraction.md
git commit -m "Add x402 facilitator landscape background to category-E doc"
```

---

## Task 10: External research — 1Click / Defuse / NEAR Intents (for §3.1)

**File (write):** Section §3.1 inside `week3/pay-anyone-legend/zcash-tool-inventory.md`.

- [ ] **Step 10.1: Pull official 1Click documentation**

WebFetch:
- https://docs.near-intents.org/ (NEAR Intents docs hub)
- https://github.com/defuse-protocol (canonical repo)
- npm: `https://www.npmjs.com/package/@defuse-protocol/one-click-sdk-typescript`

- [ ] **Step 10.2: Capture the architecture**

Write a 1-2 page explainer with:
- Who runs the system (Defuse Protocol, governance, NEAR Foundation involvement)
- What problem it solves (cross-chain swaps without bridging)
- Architecture: Solvers, Settlement contract, intents, quotes, deposit addresses
- Privacy and trust model (who sees what; who custodies funds during the swap)
- The `@defuse-protocol/one-click-sdk-typescript` 0.1.14 surface area

- [ ] **Step 10.3: Connect to Pay Anyone Legend's usage**

Cross-reference with the §1.5 walkthrough notes: which 1Click endpoints does Pay Anyone Legend actually exercise, and what 1Click features go unused.

- [ ] **Step 10.4: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/zcash-tool-inventory.md
git commit -m "Add §3.1 1Click protocol explainer to Zcash tool inventory"
```

---

## Task 11: §2 Category-E (x402 + Zcash) reference extraction

**File (write):** Complete the rest of `week3/pay-anyone-legend/category-E-extraction.md` (the §9 background is already there).

- [ ] **Step 11.1: Re-read all subsystem walkthroughs in order**

Read each `01-*.md` through `08-*.md` you just wrote. In a scratch file, jot:
- Code blocks that look reusable for Category E (x402 + Zcash)
- Code blocks that are NOT reusable because they're outsourced (1Click, OpenAI) and would need a Zcash-native replacement
- Architectural moves (intent-driven UI, semantic service search, deposit polling) that are reusable regardless of asset

- [ ] **Step 11.2: Write §2.1 — "What 'x402 + Zcash' means in this codebase vs. Secure Legion / NLx402"**

Use the week2 ★★★ doc's Section E content as a reference for the Secure Legion / NLx402 pattern. Make explicit:
- Pay Anyone Legend's Zcash role: **funding asset, not settlement asset** (per the deposit + bridge + then x402 design — to be confirmed by §1.7's findings)
- Secure Legion's Zcash role: **carrier of the x402 quote_hash in the memo field**
- Two genuinely different patterns; ours could pick either or invent a third

- [ ] **Step 11.3: Write §2.2 — exact 402 → quote → MPC sign → execute call sequence**

A numbered list of 10–20 steps, each annotated with `<file:line>` from the project. This is the "if you wanted to clone this flow tomorrow" reference.

- [ ] **Step 11.4: Write §2.3 — what's lift-and-use vs what we'd have to redo**

Two columns:
- **Lift-and-use:** Chain Signatures pattern, intent parsing pipeline, semantic service registry, deposit polling state machine, x402 client wire format, `paymentRequirements` parsing
- **Redo:** Anything Zcash-native (real address derivation, real shielded send, real receipt verification via viewing key, settlement on Zcash)

- [ ] **Step 11.5: Write §2.4 — differentiation room**

Concrete proposals: e.g., "use Zcash memo to carry `x402-quote-id`", "ZIP-321 payment URI as facilitator output", "viewing-key-based receipt for the merchant", "stop bridging to USDC altogether". Each proposal references a week2 ★★★ project as evidence the primitive exists.

- [ ] **Step 11.6: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/category-E-extraction.md
git commit -m "Write §2 category-E reference extraction"
```

---

## Task 12: §3 Zcash dev-tool inventory + outsourcing story

**File (write):** Complete the rest of `week3/pay-anyone-legend/zcash-tool-inventory.md` (the §3.1 1Click explainer is already there).

- [ ] **Step 12.1: Inventory every Zcash-related dependency in the project**

```bash
cd /home/kkang/anyone-pay && jq -r '.dependencies | keys[]' package.json
```

For each dependency, classify: `zcash-native | crypto-primitive (could be used for Zcash) | unrelated`. Reference the dependency lock for exact versions.

- [ ] **Step 12.2: Write §3.2 — how shielded tx execution is outsourced to 1Click**

Use §1.5 notes. Specifically describe:
- The user's deposit goes to a 1Click-controlled address (verify with §1.5 notes), not a self-custodied z-address.
- 1Click solvers handle the swap; they hold the ZEC.
- Pay Anyone Legend never holds, signs, or even constructs a Zcash transaction.

- [ ] **Step 12.3: Write §3.3 — how z-address generation is faked**

Use §1.3 notes. Show the verbatim code, explain why it produces a string that "looks like" a z-address, and explain why the system still functions (because the address is decorative — actual deposit identification happens via 1Click order id).

- [ ] **Step 12.4: Write §3.4 — inventory and "what they should have used"**

Two-part section:
1. **What's actually in the project:** the inventory table from Step 12.1.
2. **Catalog of native Zcash dev tools the project did not use** (one-liner each, with link):
   - `zcash_client_backend` (Rust) — full light-client + shielded-send
   - `zcash_primitives` (Rust) — primitives for note encryption, viewing keys, sapling/orchard
   - `zcashlc` C bindings — what `zcash_client_backend` exports for FFI
   - `ZcashLightClientKit` (Swift) — iOS reference (Zashi, Zapp use this)
   - `pirate-rust`, `librustzcash` — historical names; current state
   - `lightwalletd` — server they would have polled for real on-chain deposits
   - PCZT (`pczt` crate, ZIP-374) — for multi-party signing if relevant
   - `chainsig.js` — what they DID use, but for NEAR signing, not Zcash
   - Frame this as a normative recommendation to our team for Category E work.

- [ ] **Step 12.5: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/zcash-tool-inventory.md
git commit -m "Write §3 Zcash dev-tool inventory + outsourcing story"
```

---

## Task 13: §0 Big picture (write LAST)

**File (write):** `week3/pay-anyone-legend/README.md`

- [ ] **Step 13.1: Re-read every other file you wrote**

Skim `01-*.md` through `08-*.md`, `category-E-extraction.md`, `zcash-tool-inventory.md`. Make a one-sentence summary per file.

- [ ] **Step 13.2: Draft §0.1 — What Anyone-Pay is trying to be**

One paragraph (3–5 sentences) capturing: the product pitch, the underlying tech bet (x402 + AI + cross-chain), and the practical scope (mainnet-only, single happy path).

- [ ] **Step 13.3: Draft §0.2 — Five-step user story**

A numbered list, ≤ 10 lines total. Customer-side flow: search → intent match → QR shown → ZEC deposit → x402 unlocks content. Each step ends with `(see §1.X)`.

- [ ] **Step 13.4: Draft §0.3 — Architecture map (Mermaid)**

Mermaid `flowchart` with: Customer browser, Merchant UI, Next.js API routes, Supabase, NEAR contract, NEAR MPC (`v1.signer`), 1Click solver network, x402 facilitator, Zcash chain, OpenAI, NEAR AI. Arrows annotated with the data they carry. Keep it to ≤ 15 nodes.

If Mermaid doesn't render reliably in our viewing environment, fall back to ASCII boxes.

- [ ] **Step 13.5: Draft §0.4 — Reading guide**

A small table:

| If you want to know about… | Read |
|---|---|
| how the user's natural language becomes a service match | §1.1 + §1.2 |
| how a deposit address is generated | §1.3 |
| how a deposit gets confirmed | §1.4 |
| how Zcash becomes USDC | §1.5 |
| how the system signs an EVM tx without holding the key | §1.6 |
| how the paywall is unlocked | §1.7 |
| what the on-chain Rust does | §1.8 |
| should our team copy this for Category E? | §2 |
| what Zcash dev tools they used or didn't | §3 |

- [ ] **Step 13.6: Commit**

```bash
cd /home/kkang/pdm
git add week3/pay-anyone-legend/README.md
git commit -m "Write §0 big picture (README) for Pay Anyone Legend deep dive"
```

---

## Task 14: Final pass

- [ ] **Step 14.1: Verify every spec open-question is answered**

Open `week3/research-plan-pay-anyone-legend.md` §7. For each question, append a `> Answer (file:line):` line in the spec, pointing to the deep-dive section that answers it.

If any question is unanswered, do the additional investigation now — do not let an unanswered question slip.

- [ ] **Step 14.2: Cross-link the deep-dive files**

Inside each subsystem file, ensure:
- The Wiring section's "Dependencies (internal)" lists each linked subsystem with a relative link, e.g., `[§1.5 1Click bridge](./05-one-click-bridge.md)`.
- The Notes section, where it references findings from another subsystem, links to it.

- [ ] **Step 14.3: Resolve `_claims-to-verify.md`**

Every claim should be marked `[x]` with evidence, or annotated as "outside scope of this research" with a reason. Move the file's contents into a final "Appendix: Claim verification matrix" section in `README.md` (or delete the working file if you prefer to keep the appendix in README only).

- [ ] **Step 14.4: Spec-coverage scan**

Walk through `week3/research-plan-pay-anyone-legend.md` §3 (Deliverable structure). For every bullet under §0/§1/§2/§3, verify the corresponding text exists in the deep-dive files. Add a `> Spec coverage: complete` line at the top of `README.md`.

- [ ] **Step 14.5: Final commit**

```bash
cd /home/kkang/pdm
git add week3/
git commit -m "Finalize Pay Anyone Legend deep dive (cross-links, claim matrix, spec coverage)"
```

---

## Self-review of this plan

- **Spec coverage:** §0 → Task 13. §1.1–§1.8 → Tasks 1–8. §2 → Tasks 9 + 11. §3 → Tasks 10 + 12. Open questions → Task 14.1. ✓
- **No placeholders:** Every step has a concrete file path, a concrete grep, or a concrete write target. ✓
- **TDD note:** Standard TDD doesn't apply (research, not code). The substitute discipline is "every claim cited with file:line + every spec open-question explicitly tracked," enforced in Tasks 0 and 14.
- **One responsibility per file:** Each output file = one subsystem or one cross-cut; the README is the only entry point. ✓
- **Frequent commits:** Commit at the end of every task; that's 14 commits over the research. Matches week2 style.
