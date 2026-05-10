# Week 3 Research Plan — Pay Anyone Legend (#37, kurodenjiro)

**Date:** 2026-05-11
**Researcher:** kkang
**Reference repo (local clone):** `/home/kkang/anyone-pay`
**Upstream:** https://github.com/kurodenjiro/Anyone-pay

---

## 1. Goal

Produce a deep-dive document on Pay Anyone Legend that serves three purposes simultaneously, all with very deep detail:

1. **Architectural walkthrough** — explain how the whole system works end-to-end so a teammate who never opened the repo can understand it.
2. **Category-E reference extraction** — assume our team is going for **Category E (x402 + Zcash)**; pull out exactly what we'd reuse from this project, and where it leaves room for our differentiation.
3. **Zcash dev-tool / library inventory + outsourcing story** — what tools and libraries did they actually use for the Zcash side, what did they outsource (1Click), what did they fake (z-address generation), and what *should* have been used.

This research is **read-only**. Local execution is deferred to a later step ("test it later"). No code is written for our project as part of this task; the deliverable is a markdown document.

## 2. Framing assumption

Our team is committing (for the purpose of this research) to **Category E: x402 + Zcash**. Pay Anyone Legend was selected as the strongest single E-reference. Per the week2 ★★★ note, the project is dual-listed in D and E; this research treats it primarily as an E-reference and only secondarily as a D-reference where useful.

Key prior finding from week2 to verify in this research:

> "Pay Anyone Legend = x402 결제 이전에 ZEC가 USDC로 변환되는 funding 단계 (외부 swap에 위임). Zcash 측 구현은 얕다 (1Click API에 위임, Z-address 생성도 mock)."

If the prior finding holds, the differentiation angle for our team is "make Zcash itself the x402 settlement asset, not an upstream funding asset." The deep dive must validate or correct this finding with code-level evidence.

## 3. Deliverable

**File:** `week3/pay-anyone-legend-deep-dive.md` (single file; split if it exceeds ~3,000 lines)

**Language:** Korean prose with English technical terms — matches week2 deliverable style.

**Structure:**

```
§0. Big picture (read first)
    0.1 What Anyone-Pay is trying to be (1 paragraph)
    0.2 The user story as 5 numbered steps (one happy-path narrative)
    0.3 Architecture map — Mermaid diagram of subsystems + data/control arrows
    0.4 Reading guide — which §1.X to look at for each box on the diagram

§1. Subsystem walkthrough (deep detail)
    Each subsection follows a fixed template:
      - Purpose (2-3 sentences)
      - Files / functions (file:line pointers)
      - Wiring (inputs, outputs, who calls it, what it calls)
      - Libraries + versions
      - Notes / quirks / footguns

    1.1 Intent parser              (NEAR AI + OpenAI embeddings)
    1.2 Service registry           (Supabase + pgvector semantic search)
    1.3 Z-address generation       (week2 says it's a mock — verify in code)
    1.4 Deposit tracking           (Supabase polling / cron)
    1.5 1Click bridge integration  (cross-chain swap)
    1.6 NEAR Chain Signatures (MPC) (signing without exposing keys)
    1.7 x402 client                (HTTP 402 protocol execution)
    1.8 NEAR Rust contract         (what's in `contract/` and why)

§2. Category-E (x402 + Zcash) reference extraction
    2.1 What "x402 + Zcash" means in this codebase vs. Secure Legion / NLx402
    2.2 The exact 402 → quote → MPC sign → execute call sequence with code refs
    2.3 What's lift-and-use for our project; what we'd have to redo
    2.4 Where Pay Anyone Legend's design opens differentiation room for us

§3. Zcash dev-tool / library inventory + outsourcing story
    3.1 What 1Click actually is — origin (NEAR Intents / Defuse), what it does,
        who runs it, what chains/assets, API surface
    3.2 How they outsource shielded tx execution to 1Click — the exact API calls,
        what 1Click handles vs. what their app handles, where ZEC actually
        gets shielded/sent
    3.3 How z-address generation is faked — the `crypto.getRandomValues + 'zs1'`
        pattern, why it works at all (it doesn't, really), what breaks if you
        rely on it
    3.4 Inventory: every Zcash-related lib/import they actually use (likely
        zero native Zcash crypto), and what they *should* have used
        (zcash_client_backend, ZcashLightClientKit, lightwalletd, PCZT, etc.)
```

## 4. Research method

Bottom-up. Stop and write notes per subsystem before moving on.

1. **Read once, end-to-end:** `README.md`, `SETUP.md`, `DEPLOY*.md`, `SUPABASE*.md`. Capture every concrete claim into a "Claims to verify" working list.
2. **Map files:** index every file under `app/`, `lib/`, `components/`, `contract/`, `scripts/`. Tag each file with which subsystem it belongs to. This is the index for §1.
3. **Per subsystem, read the listed files and trace one happy path through them.** Write the §1.X notes as you go. Use `git grep` / `rg` to follow imports and call sites.
4. **Read `contract/src/*.rs`** to understand the on-chain side and what the Rust code authoritatively does vs. the TS app code.
5. **Re-read API routes (`app/api/**`)** with the subsystem map already in hand — these tie everything together.
6. **External-tool mini-research** for §3.1: pull the 1Click docs/API reference, find the company behind it, write the explainer. Same for x402 (Coinbase facilitator vs. NLx402 by PCEF).
7. **Synthesize §0 (big picture) last** — easier to write the diagram once you've seen all the parts.
8. **§2 and §3** get written after §1 is done — they are cross-cuts of the per-subsystem notes.

## 5. Artifacts and scope

**In-scope:**
- The `week3/pay-anyone-legend-deep-dive.md` document.
- Mermaid architecture diagram embedded in §0.3.
- File:line code pointers throughout §1.

**Out-of-scope:**
- Running the app locally (deferred — see §6).
- Proposing our own architecture (separate later doc).
- Comparing to Section A/B/C references at length — already done in week2.

## 6. Deferred (not this task)

- Local execution: README path requires NEAR mainnet account, OpenAI key, Supabase project, optional 1Click JWT. We will pick up B2-strict / B2-no-mainnet / B3 in a follow-up task once the deep-dive doc is written.
- Picking the team's actual category. This research feeds that decision but does not make it.

## 7. Open questions to answer in the dive

These are written in the deep-dive doc as we encounter the answers:

- Which x402 facilitator does Pay Anyone Legend actually call — Coinbase's Base facilitator, NLx402 (PCEF/Solana), or something else?
- Is `app/api/intent/` the place where the 402 challenge → MPC sign → execute happens, or is that split across `lib/chainSig.ts` + `lib/oneClick.ts`?
- Does the NEAR Rust contract in `contract/` participate in the payment flow, or is it just service-registry / metadata?
- Is the Supabase deposit tracking actually verifying chain state (via lightwalletd or RPC), or just trusting a webhook from 1Click?
- What does `lib/kdf.ts` do? Key derivation for what — chain-sig path derivation, or something Zcash-related?
- Is there any handling for 402 retries / refunds, or is it fire-and-hope?
- Verify the week2 claim that z-address generation is `crypto.getRandomValues + 'zs1' prefix` — find the exact file:line, or correct the claim if the code has moved on.
