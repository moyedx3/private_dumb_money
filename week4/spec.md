# Unlockable Drop — Technical Spec (v0)

- **Status**: Draft for team review.
- **Supersedes the implementation details in**: [`project-scope.md`](./project-scope.md). The "why" in scope still applies; this spec replaces the *baseline honest-but-curious* implementation with a **TEE-backed** implementation and adds the **NEAR Intents** cross-chain settlement path.
- **Hard requirement added since scope doc**: The Indexer **MUST NOT** be able to decrypt content blobs. Reason: if the Indexer can decrypt and redistribute (or sell at a lower price), the Creator's economics collapse and the platform loses meaning.

---

## 1. Goals and non-goals

### Must-haves for Phase 1 (the MVP we demo)

1. **Indexer cannot decrypt content.** Enforced by hardware (Intel TDX) + remote attestation, not by trust.
2. **Buyer pays in shielded ZEC.** Buyer-side privacy comes from Zcash's shielded protocol.
3. **Buyer identity is unlinkable** from the purchase event across the chain, the catalog, and the dispatch bucket.
4. **Creator receives shielded ZEC** at their own Zcash address. They manage it with their own wallet (Zashi). No cross-chain settlement, no TEE-held spending keys, no unshield.
5. **End-to-end live demo** with one creator (us) and audience members as buyers, in under 5 minutes per transaction.

### Phase 2 goal (stretch, only if Phase 1 is solid)

6. **Creator can receive in any chain/asset they choose.** Realized via NEAR Intents (cross-chain swap and settlement). NEAR Intents supports **transparent Zcash only** ([near-intents.org docs](https://docs.near-intents.org/resources/chain-support#zcash)), so the Phase 2 path is *shielded payment → TEE-managed unshield → transparent ZEC → NEAR Intents → target chain*. The unshield step exposes the creator's revenue stream (not the buyer's identity); see §7.2 and §7.5.

### Non-goals

- **Subscriptions.** One-time content purchases only.
- **DRM / piracy prevention.** A paying buyer can leak their decryption key to others. Acknowledged, not solved.
- **Full network-layer privacy** (Tor / mixnets). Documented in the threat model; not implemented for the demo.
- **Recovery from a lost Buyer ephemeral key.** Ephemeral keys are session-scoped; closing the tab forfeits the purchase. (Mitigation discussed in §9.)
- **Auditing or governance of which content gets sold.** Out of scope.

---

## 2. High-level architecture

```
   ┌────────────────────────────────────────────────────────────────────┐
   │                       BUYER (browser)                              │
   │  - generates ephemeral keypair (e_priv, e_pub) per purchase        │
   │  - constructs ZIP-321 URI with memo = drop_id || e_pub             │
   │  - polls public bucket, trial-decrypts dispatch blobs              │
   └─────────┬──────────────────────────────────────┬───────────────────┘
             │ scans QR with Zashi                  │ HTTPS polls
             ▼                                      ▼
   ┌─────────────────────┐               ┌─────────────────────────────┐
   │     ZCASH CHAIN     │               │       PUBLIC BUCKET         │
   │  (shielded protocol │               │  - encrypted content blobs  │
   │   handles privacy   │               │  - dispatch blobs           │
   │   at protocol level)│               │  (S3 / Blossom / NIP-96)    │
   └──────────┬──────────┘               └──────────────▲──────────────┘
              │ trial-decrypt with IVK                   │ publish dispatch
              │                                          │
              ▼                                          │
   ┌──────────────────────────────────────────────────────────────────┐
   │                  TEE INDEXER  (Phala Cloud, Intel TDX)            │
   │                                                                   │
   │   Inside the enclave (operator CANNOT read):                      │
   │   - K_drops (per-drop AES-256 keys)                               │
   │   - Zcash IVK + SPENDING KEY (TEE-controlled deposit address)     │
   │   - Server dispatch keypair                                       │
   │   - NEAR account credentials (for triggering Intents settlement)  │
   │                                                                   │
   │   Code: open-source, reproducibly built. Image hash published.    │
   │   Attestation: TDX quote signed by Intel; published at /attest.   │
   │                                                                   │
   │   Logic:                                                          │
   │   - poll Zcash via IVK → decode memo → wrap K_drop via ECIES      │
   │   - publish dispatch blob to public bucket                        │
   │   - unshield received ZEC (batched) → transparent t-addr          │
   │   - submit NEAR Intent: transparent ZEC → creator's target asset  │
   └────────────────────────────┬──────────────────────────────────────┘
                                │ NEAR RPC: submit Intent
                                ▼
   ┌───────────────────────────────────────────────────────────────────┐
   │            NEAR INTENTS (One-Click API + solver network)          │
   │  - solvers compete to fill the swap                               │
   │  - bridges to creator's target chain (Base, Ethereum, etc.)       │
   └────────────────────────────┬──────────────────────────────────────┘
                                │ delivers funds
                                ▼
   ┌──────────────────────────────────────────┐
   │  CREATOR'S DESTINATION WALLET             │
   │  (USDC on Base, ETH on mainnet, etc.)     │
   └──────────────────────────────────────────┘
```

---

## 3. Components

| # | Component | Hosting | What it holds | Open-source? |
|---|---|---|---|---|
| 1 | **Buyer browser app** | Static frontend (Vercel/Cloudflare Pages) | Ephemeral keypair per purchase, in-memory only | Yes |
| 2 | **TEE Indexer** | Phala Cloud, Intel TDX | `K_drop`s, Zcash IVK + spending key (per-creator), dispatch keypair, NEAR account | Yes — required for attestation to be meaningful |
| 3 | **Public bucket** | S3 + CloudFront (or Blossom/NIP-96 if we want pure decentralization) | Encrypted content blobs + dispatch blobs | n/a |
| 4 | **Creator dashboard** | Static frontend + small backend | Local upload tool + attestation verifier | Yes |
| 5 | **NEAR Intents (external)** | NEAR mainnet + solver network | Quotes, deposit addresses, swap settlement | External dependency |

---

## 4. End-to-end flow

### 4.1 Onboarding (Phase 1 — creator owns the Zcash address)

1. Creator opens dashboard, fetches the TEE Indexer's published attestation, verifies the TDX quote against Intel's signing chain and verifies the code measurement matches the open-source repo's reproducible build.
2. Creator generates `K_drop = random_bytes(32)` locally in browser.
3. Creator encrypts content: `ciphertext = AES-256-GCM.encrypt(K_drop, plaintext)`.
4. Creator uploads `ciphertext` to the public bucket → gets `H_content`.
5. Creator opens an attested-TLS channel to the TEE Indexer and submits:
   - `drop_id`, `price_zec`, `H_content`, `K_drop`, `creator_zcash_address`, `IVK_creator`.
   - The `K_drop` and `IVK_creator` are end-to-end encrypted to the TEE enclave's public key. Only code measured by the attestation can decrypt them.
   - **Note: only the IVK is given to the TEE. The spending key never leaves the Creator's wallet.**
6. TEE Indexer publishes the catalog entry: `{drop_id, price_zec, H_content, creator_zcash_address}`. The Creator continues to own this address and the funds that land on it.

### 4.2 Onboarding (Phase 2 — TEE owns the Zcash address)

Phase 2 reuses 4.1 steps 1–4 unchanged. Step 5–6 differ:

5'. Creator submits via attested-TLS: `drop_id`, `price_zec`, `H_content`, `K_drop`, `payout_destination` (target chain + asset + address). **No Zcash address or IVK is submitted by the Creator.**

6'. **TEE generates a fresh Zcash shielded address + spending key inside the enclave** for this creator (or reuses an existing per-creator address). The IVK and spending key never leave the enclave.

7'. TEE Indexer publishes the catalog entry: `{drop_id, price_zec, H_content, deposit_zcash_address}`. The deposit address belongs to the TEE, not the creator. The Creator's only role going forward is to receive on their target chain.

### 4.3 Purchase (per buyer)

```
 BUYER (browser)      ZCASH CHAIN       TEE INDEXER         BUCKET        NEAR INTENTS
       │                   │                 │                │                 │
       │ [generate (e_priv, e_pub)]          │                │                 │
       │                                     │                │                 │
       │ build ZIP-321 URI:                  │                │                 │
       │   zcash:<deposit_zaddr>?amount=...& │                │                 │
       │   memo=drop_id||e_pub               │                │                 │
       │                                     │                │                 │
       │ user scans w/ Zashi                 │                │                 │
       │                                     │                │                 │
       │ ── shielded tx ──▶│                 │                │                 │
       │                   │                 │                │                 │
       │                   │ ◀── poll w/ TEE's IVK ──         │                 │
       │                   │ ── tx + memo ──▶                 │                 │
       │                                     │                │                 │
       │                          [inside enclave:            │                 │
       │                           decrypt memo → (drop_id, e_pub)              │
       │                           verify amount ≥ price                        │
       │                           wrap K_drop via ECIES → blob]                │
       │                                     │                │                 │
       │                                     │ ── publish blob ▶                │
       │                                     │  (buyer can unlock NOW)          │
       │                                     │                │                 │
       │ ── poll new blobs ──────────────────────────────▶│                     │
       │ ◀── all dispatch blobs ─────────────────────────│                     │
       │                                                                        │
       │ [try ECIES dec w/ e_priv on each → one yields K_drop]                  │
       │                                                                        │
       │ ── fetch H_content ──────────────────────────── ▶│                     │
       │ ◀── ciphertext ─────────────────────────────────│                     │
       │                                                                        │
       │ [AES-GCM dec(K_drop, ciphertext) → plaintext, render]                  │
       │                                                                        │
       │   ── Phase 1 flow ends here. Creator already received shielded ZEC. ── │
       │                                                                        │
       │  ════════════════════ PHASE 2 ADDITIONS ONLY ═══════════════════════   │
       │                                     │                                  │
       │                                     │ [unshield ZEC: shielded → t-addr]│
       │                                     │ submit transparent tx           │
       │                                     │── transparent ZEC ──▶ NEAR Intents deposit
       │                                     │                                  │
       │                                     │ solver settles → creator's       │
       │                                     │ destination wallet (e.g. Base)   │
```

---

## 5. Security properties (what we guarantee)

| Property | Guaranteed by | Notes |
|---|---|---|
| **Indexer cannot decrypt content** | Intel TDX memory isolation + remote attestation verifying open-source reproducible build | Hardware vendor trust required (Intel) |
| **Buyer wallet identity hidden** | Zcash shielded protocol (no sender info on-chain) | Independent of TEE; survives the unshield step |
| **Payment-to-buyer linkage broken** | Fresh ephemeral keypair per purchase; memo carries only ephemeral pubkey | Reused ephemeral keys would break this |
| **Dispatch blob unlinkable** | Blobs are not tagged with buyer or drop ID; only the matching ephemeral private key decrypts | Network-layer caveat applies |
| **Buyer cannot be linked back through the bridge** | Unshield reveals only the *aggregate* TEE balance leaving the shielded pool; sender side stays inside the shielded set | Batching strengthens this; per-payment unshields weaken it |

**Explicitly NOT guaranteed:**

- **Creator revenue privacy.** The unshield step makes the creator's net revenue visible on the transparent Zcash and target chain. The creator is opting into this by choosing a non-shielded destination chain. See §7.5.
- **TEE-custody-free flow.** The TEE holds the Zcash spending key briefly (between receive and unshield). A TEE compromise during that window could drain pending funds. See §7.6.

---

## 6. Trust model

What you need to trust, and to what degree:

| Entity | Trust required | Why |
|---|---|---|
| **Intel** (TDX manufacturer) | High — for hardware not to be backdoored | Currently unavoidable for production TEEs |
| **Phala Cloud** (operator) | Low — they can DoS us, cannot read enclave memory | TDX prevents read; attestation prevents code swap |
| **NEAR Intents solver network** | Medium — for settlement to actually happen and at fair price | If solvers misbehave, swaps may fail or price poorly. Doesn't affect buyer privacy or content confidentiality. |
| **Reproducible build of our TEE image** | High — attestation is meaningless if reviewers can't verify what the measured hash actually does | Must be open source + reproducible. CI must publish artifact + hash. |
| **Buyer's browser environment** | High — buyer must trust their own browser | Standard web trust |
| **Public bucket operator** | Low — sees encrypted blobs only | Could DoS by deleting blobs |

---

## 7. Tradeoffs and known security drawbacks

This is the section to read carefully. Every design choice has a cost. Listed in rough order of severity.

### 7.1 TEE-specific concerns

**TEE security has a real failure history.** Intel SGX has had multiple major vulnerability classes (Foreshadow, Plundervolt, LVI, SGAxe, ÆPIC Leak) over the past decade — most allowing key extraction or memory leak. AMD SEV has had similar (CipherLeaks, undeSErVed). Intel TDX is newer (2023+), so it has *fewer* published attacks — but that's at least partly because it's been studied less. Researchers have published TDX side-channel research and TDX-specific issues are likely to surface. **A TEE is "best-effort hardware confidentiality," not a mathematical guarantee.**

**Side-channel attacks remain possible.** Even when memory encryption holds, attackers with co-located access (a malicious cloud neighbor, or a malicious cloud operator) can extract secrets via timing, cache, power, or transient-execution channels. TDX has mitigations; none are complete.

**Hardware vendor trust is unavoidable.** TDX attestation is rooted in Intel's signing keys. A subpoena, a state-level compromise of Intel, or a future Intel firmware update can in principle break the attestation chain. This is the fundamental ceiling of TEE-based privacy claims.

**Attestation is only meaningful if the code is auditable.** A TDX quote certifies "this exact binary ran in this exact enclave on this CPU." If the binary is closed-source or built non-reproducibly, the certification is empty — auditors can't verify what the measured hash actually does. **Required mitigation:** publish the Indexer source publicly, ship a reproducible build pipeline (Docker buildkit with deterministic timestamps, etc.), and have at least one external party verify the published image hash matches the source.

**TEE rollback attacks.** A malicious host can roll back TEE persistent state (snapshot + restore from before a key was used). Mitigation: monotonic counters or stateless design. Our design is stateless per request (after initial `K_drop` provisioning), so rollback isn't a key risk.

**Operator can DoS.** Phala can shut us down. The Indexer's host can refuse to run the workload, drop network traffic, or rate-limit. We can't prevent this. Mitigation if it matters: multiple Phala regions, multiple TEE providers (Marlin, AWS Nitro), client-side failover.

### 7.2 NEAR Intents and cross-chain settlement concerns

**NEAR Intents supports transparent Zcash only.** Confirmed by [near-intents.org docs](https://docs.near-intents.org/resources/chain-support#zcash): `t1` and `t3` addresses are partially supported; Sapling and Orchard shielded pools are not. This forces an explicit unshield step inside the TEE before the funds can reach Intents. See §7.6 for the privacy implication.

**Solver settlement risk.** NEAR Intents solvers can fail to fill, fill at bad prices, or take a long time. Buyer doesn't care (they already got content), but Creator's payout may be delayed or under-quoted. The Creator should set slippage tolerance in the quote.

**Solver censorship.** A solver could decline to swap specific addresses. Lower risk than a CEX bridge but not zero.

**Atomicity gap between content release and payment settlement.** The TEE releases the dispatch blob the moment it sees a shielded payment land. The unshield + cross-chain settlement happens *after*. If settlement fails (Intent expires, solver rejects, ZEC price moves), the Creator may receive less than expected or nothing on the target chain. The buyer has already gotten the content. **Mitigation:** the TEE should hold the released ZEC briefly and verify Intents acceptance before publishing the dispatch blob — adds 10s–60s buyer-perceived latency in exchange for atomicity.

### 7.3 Architectural and protocol concerns

**Buyer's ephemeral keypair is single-shot and unrecoverable.** If the buyer closes the tab between paying and receiving the dispatch blob, they lose access. We can mitigate by: (a) persisting the keypair in `IndexedDB` for 24h, (b) re-publishing dispatch blobs for some retention window, (c) letting the buyer download a "recovery file" with `e_priv` after paying. None are perfect.

**Network-layer correlation.** The Buyer's browser polls the bucket from an IP address. The Buyer's Zashi wallet broadcasts from an IP. If the same network observer sees both, they can correlate the buyer to the purchase. **Documented but not addressed in v0.** Mitigation requires Tor or a mixnet.

**Catalog-browsing fingerprint.** If the Buyer's browser fetches `/drops/42` (specific drop page) and shortly after a payment for `drop_id=42` arrives, the Indexer can correlate. Mitigation: client-side catalog browsing (entire catalog fetched once, no per-drop endpoints).

**Memo size limit.** Zcash Orchard memos are 512 bytes. Our memo payload (`drop_id` + `e_pub` + framing) is ~40 bytes. Plenty of headroom, but watch for protocol changes.

**Replay / drop_id confusion.** A buyer who copies a previous transaction's memo and re-broadcasts (or tries to claim the same `e_pub` for two drops) can be handled by the Indexer tracking nullifiers and rejecting duplicates. Standard nullifier handling.

### 7.4 Operational and economic concerns

**Operating cost of a TDX-enabled enclave is non-trivial.** Phala Cloud pricing scales with vCPU + memory. For a single-creator demo it's small (~$50/mo); at scale it grows linearly with concurrent encryption operations.

**Key compromise → blast radius.** If the TEE is somehow breached and `K_drop`s leak, *all past and future purchases* for those drops are compromised. There is no forward-secrecy in this design. Mitigation: rotate `K_drop`s periodically (re-encrypt content), but this only protects future buyers.

**Code update / re-attestation.** Updating the Indexer's code changes its measurement → invalidates the previous attestation → Creator must re-attest and re-provision `K_drop`s. Manageable but a real operational burden.

### 7.5 Creator revenue privacy is lost at the unshield step

Because NEAR Intents accepts only transparent Zcash, the TEE must unshield received ZEC before forwarding. The unshield transaction publicly exposes:

- The amount being unshielded
- The destination transparent address (the NEAR Intents deposit)
- Approximate timing

What is **not** exposed: the buyer's wallet identity or the link between any specific purchase and any specific buyer. Buyer anonymity is preserved by Zcash's shielded protocol upstream of the unshield.

**Why this is acceptable to ship:** the creator has explicitly chosen to settle on a transparent target chain (e.g. USDC on Base). Their revenue on that chain is *already* publicly visible. Adding the transparent-Zcash hop only links Zcash-side numbers to those already-visible target-chain receipts — it doesn't create new exposure beyond what the creator's payout destination already implies.

**Mitigation — batched unshielding.** The TEE accumulates received ZEC and unshields in batches (hourly, or once a threshold like $100 is reached). This collapses many individual purchases into one observable unshield event, breaking the per-purchase amount/timing correlation. Cost: creator settlement latency increases to the batch interval. Buyer experience is unchanged (content unlocks immediately on payment).

**What batching does NOT protect against:** a sufficiently patient observer watching the TEE's transparent address for months can still infer aggregate revenue. This is fundamental to using a transparent-receiving destination chain and not solvable in this design.

### 7.6 TEE holds the Zcash spending key

The TEE generates and holds a Zcash spending key per creator inside the enclave to perform unshields. This is a meaningful trust escalation versus the IVK-only design: a TEE compromise during the receive-to-unshield window could let an attacker drain pending balances.

**Mitigations:**

- **Aggressive spend-out.** The TEE unshields as soon as it can (subject to batching policy). Steady-state balance held inside the TEE should be at most one batch interval's worth of revenue.
- **Per-creator key isolation.** Each creator's spending key is derived independently so a single creator's compromise doesn't cascade across the platform.
- **No long-lived balances.** The TEE is a router, not a wallet. It does not custody funds beyond the time needed to forward them.

**What this does NOT mitigate:** a TEE breach exactly during the moment a high-value batch is being assembled would still cost that batch. Acceptable for the scale we're targeting (~$10–100 per drop in the demo).

### 7.7 Demo-day concerns

**The audience needs Zashi or another Zcash wallet pre-installed.** Wallet onboarding is slow. Mitigation: provide test mobile devices with pre-loaded testnet wallets at the demo table.

**Testnet vs mainnet for the demo.** Mainnet is more impressive but requires real ZEC. Testnet is risk-free. Recommend: mainnet with tiny amounts (~$0.10/purchase) so the audience can keep what they unlock.

---

## 8. Open questions (research before commit)

These need to be answered in week 1 by the team:

1. ~~**Does NEAR Intents support ZEC as a source asset?**~~ **Resolved 2026-05-18.** NEAR Intents supports transparent Zcash (`t1`/`t3`) only — not Sapling, not Orchard. The TEE handles the unshield step inside the enclave; see §4 flow and §7.2/§7.5 for the privacy implication.
2. **What does the attested-TLS provisioning flow look like in practice on Phala Cloud / Dstack?** Specifically: how does the Creator's browser verify the attestation and seal `K_drop` to the enclave?
3. **Public bucket choice.** S3 is easy but central. Blossom/NIP-96 is decentralized but less mature. Pick based on team familiarity.
4. **Buyer browser key recovery.** Do we persist `e_priv` in `IndexedDB` for 24h? Make it a UX choice? Just drop it on tab close?
5. **TEE provider redundancy.** Should v0 ship on Phala only, or also have an AWS Nitro backup deployment for the demo?
6. **Replay protection / dispatch blob retention window.** How long do we keep dispatch blobs in the bucket?
7. **Unshield batching policy.** Per-payment unshield (low latency, worst revenue privacy) vs hourly batch (medium) vs threshold-triggered (best privacy, variable latency)? Default proposal: hourly batches, with creator-configurable threshold override.
8. **Zcash key generation inside the enclave.** Can we derive Sapling/Orchard spending keys from a TDX-sealed seed reproducibly across enclave restarts, or do we need an explicit re-provisioning flow?

---

## 9. Phased delivery

### Phase 0 — Software-only indexer (internal milestone, ~3 days)

Indexer runs as a normal server on Vercel/Fly/local. Proves the full cryptographic flow end-to-end: IVK detection, ECIES wrap, bucket dispatch, buyer-side polling and decrypt. The `K_drop` is NOT confidential — it sits in operator-visible memory. **This phase exists only to integrate the moving parts.** Not for any demo.

Deliverables:
- Working Indexer server with IVK-based payment detection
- Public bucket integration (S3 or Blossom; pick one)
- Buyer-side browser app: catalog → ZIP-321 QR → poll → decrypt
- End-to-end test: a teammate pays, content unlocks in another teammate's browser

### Phase 1 — TEE-hosted indexer, Zcash-only (the demo target)

Move the Indexer code into a Docker image, deploy on Phala Cloud (Intel TDX). Publish the attestation endpoint. Build the creator-side attestation verifier so creators can confirm the running binary matches the open-source repo.

**TEE holds only `K_drop`s and `IVK_creator` — no Zcash spending key, no NEAR account.** The Creator continues to own their own Zcash address. Payment flow is shielded → shielded; the Creator manages received funds with Zashi.

This is the version that satisfies the hard requirements (§1 must-haves 1–5) and is what we demo.

Deliverables:
- Reproducibly-built Docker image of the Indexer
- Phala Cloud deployment with public attestation endpoint
- Creator-side attestation verifier UI
- All Phase 0 functionality preserved through the TEE boundary
- End-to-end live demo script with audience-as-buyers

### Phase 2 — TEE-mediated cross-chain settlement (stretch / bonus)

Extend the TEE to hold per-creator Zcash spending keys and a NEAR account. Adds the §4.2 onboarding flow (TEE generates the deposit address), the §4.3 settlement flow (unshield + Intents), batched-unshield policy, and updated attestation showing the new measured code.

**Attempt only if Phase 1 is solid by end of week 2.** Phase 2 brings real new tradeoffs (creator revenue privacy, TEE-held spending keys) and is not required for the core demo story.

Deliverables (conditional):
- Per-creator Zcash key derivation inside the TEE (resolves Open Question §8 #8)
- Unshield logic with configurable batching
- NEAR Intents integration for transparent ZEC → target chain
- Updated attestation, re-provisioning flow for existing creators

### Phase 3 — Post-hackathon, not committed

Multi-creator catalog UX, threshold dispatch (k-of-n TEEs across providers), Tor/mixnet for buyer polling, mobile-native buyer app.

---

## 10. Demo plan

Three creators (us); audience members as buyers.

1. Each creator pre-uploads a single drop (1 image each: a cat photo, a recipe, a meme).
2. Creators' attested addresses + drop catalog displayed on the projector.
3. Audience scans a QR code → opens the catalog web page.
4. Audience picks a drop → browser shows a ZIP-321 QR → audience scans with Zashi → confirms payment (~0.01 ZEC).
5. Within ~30 seconds, dispatch blob lands; browser auto-unlocks the content.
6. Show on projector: the TEE Indexer's logs (sanitized) prove that we *cannot* see the content even though we just dispatched the key.

If anything goes wrong on demo day, fall back to a pre-recorded screencast.

---

## 11. TEE tooling comparison

For a small team in a hackathon timeframe, comfortable with Docker, and not committed to a specific cloud, the rankings work out roughly as follows.

### Recommended — Phala Cloud (Intel TDX, Dstack SDK)

- **Deployment model:** Docker container. Push your image, get a public URL and an `/attest` endpoint that serves the TDX quote. ~1–2 hours from zero to "hello attested world."
- **TEE hardware:** Intel TDX. Newer and stronger than SGX; fewer published attacks to date.
- **SDK:** [Dstack](https://github.com/Dstack-TEE/dstack), open-source, provides a local socket inside the enclave for sealing keys and accessing enclave services.
- **Attestation:** Built in. Phala publishes the TDX quote; clients verify against Intel's signing chain.
- **Pricing:** Pay-as-you-go USD. Small for our scale (one Docker container, low traffic).
- **Templates:** Multiple, including a (now-deprecated-but-still-instructive) NEAR Shade Agent template. The Docker structure transfers; we won't depend on the deprecated framework.
- **Docs:** [docs.phala.network](https://docs.phala.network), [cloud.phala.network/templates](https://cloud.phala.network/templates)
- **Watch out for:** Phala-specific deployment workflow (some lock-in). We trust Phala's infrastructure to not DoS us.

### Alternative — AWS Nitro Enclaves

- **Deployment model:** Launch a Nitro-eligible EC2 instance (m5n / c5n / r5n family), use `nitro-cli` to build an `.eif` image from a Docker image, run via the enclave allocator. Talk to the enclave from the parent EC2 over a `vsock` socket.
- **TEE hardware:** AWS-managed (AWS claims AMD SEV-equivalent guarantees + their own root of trust).
- **SDK:** Nitro Enclaves SDK (Rust); Python helpers also available.
- **Attestation:** AWS-signed quotes via KMS integration. Clients verify against AWS public keys.
- **Pricing:** You pay for the underlying EC2 instance ($90–200/month for a small Nitro-eligible type). No extra fee for running the enclave.
- **Hello-world path:** 3–5 hours of AWS plumbing the first time; faster after that.
- **Docs:** [docs.aws.amazon.com/enclaves](https://docs.aws.amazon.com/enclaves/latest/user/nitro-enclave.html)
- **Watch out for:** AWS-only (no cross-cloud), trust model includes "trust AWS," and `vsock` adds engineering overhead.

### Considered and rejected for our scope

| Option | Why we're skipping |
|---|---|
| **Marlin Oyster** | Decentralized TEE marketplace (SGX/TDX/SEV). Stronger trust story but harder setup. Worth knowing about; not the right hackathon choice. |
| **Azure Confidential Containers / GCP Confidential VMs** | Fine if you're already on Azure or GCP. We're not. |
| **Enarx** | WASM-based, cross-TEE. Cool idea but project maturity is uncertain and docs are sparse. |
| **Gramine / Occlum** | Wraps unmodified Linux binaries for Intel SGX. Useful if you have a binary to wrap; SGX itself has the weaker security record. |
| **Apple Secure Enclave / Android TrustZone** | Mobile-side. Too restrictive for our server-side Indexer. |

### Recommendation

**Start with Phala Cloud.** Phase 0 runs anywhere (it's a normal Node/Python/Rust server). Phase 1 wraps that exact same code in Docker and pushes it to Phala. Phase 2 (if we get there) extends the same container with additional capabilities. One artifact, one deployment workflow, one set of logs.

Concrete first day of Phase 1: spend it doing a Phala "hello world" — get *any* attested service running that prints its TDX attestation report. Once that loop works end-to-end, swap in the real Indexer code.

---

## 12. References

- Sean Bowe, "Tachyon: scaling Zcash via oblivious synchronization." [seanbowe.com](https://seanbowe.com/blog/tachyon-scaling-zcash-oblivious-synchronization/)
- NEAR Intents chain support (transparent Zcash only). [docs.near-intents.org/resources/chain-support#zcash](https://docs.near-intents.org/resources/chain-support#zcash)
- NEAR Shade Agents (deprecated April 19, 2026 — underlying Phala Cloud / Dstack stack still active). [docs.near.org/ai/shade-agents/getting-started/introduction](https://docs.near.org/ai/shade-agents/getting-started/introduction)
- Phala Cloud + NEAR Shade Agent template. [phala.com](https://phala.com/posts/near-shade-agent-template-phala-cloud)
- ZIP-321 (payment request URI). [zips.z.cash/zip-0321](https://zips.z.cash/zip-0321)
- Previous scope decision: [`project-scope.md`](./project-scope.md)
- Week 3 research: [`../week3/회의록_week3.md`](../week3/회의록_week3.md), [`../week3/pay-anyone-legend/`](../week3/pay-anyone-legend/)
