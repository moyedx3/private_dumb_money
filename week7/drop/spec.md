# Unlockable Drop — Technical Spec (v1, feasibility-corrected)

- **Status**: Reality-checked revision of `week4/drop/spec.md` (v0).
- **Supersedes**: `week4/drop/spec.md`. The architecture and security goals are unchanged; this version corrects six load-bearing technical claims that were wrong or understated in v0, based on (a) the code actually shipped in `week5/clean-wallet-mvp/`, (b) the live Zcash/NEAR/Phala specs, and (c) what the three pre-build spikes verified live (all passed — see `team/00-overview.md`).
- **Hard requirement (unchanged)**: The Indexer **MUST NOT** be able to decrypt content blobs. Enforced by Intel TDX + remote attestation, not trust.

---

## Changelog v0 → v1 (the six corrections)

| # | What v0 said | What's actually true | Where fixed |
|---|---|---|---|
| **C1** | "TEE polls the chain with IVK, decrypts the memo" — implied a single compact-block scan. | Compact blocks carry only the **first 52 bytes** of ciphertext — **no memo**. The TEE must detect the note in the compact block, then fetch the **full transaction** (`GetTransaction`) and trial-decrypt the full `enc_ciphertext` to recover the 512-byte memo. This also leaks *which tx* the TEE wants to the (untrusted) lightwalletd. | §2, §4.3, §5 |
| **C2** | Implied clean-wallet's scanner drops straight in. | clean-wallet scans **outgoing** payments (OVK) and **discards the memo** (`scan.rs:259,279`). The drop needs **incoming** detection (IVK + `try_sapling_note_decryption` / Orchard incoming) and must **keep** the memo. New code — but it's the most standard wallet operation, so it's low-risk. | §2, §4.3, §7.8, §9 |
| **C3** | "Creator opens an attested-TLS channel and submits K_drop, e2e-encrypted to the enclave." | This is a *secret-IN* attestation flow (seal a secret so only the measured binary reads it). clean-wallet only ever did *result-OUT* attestation. This is genuinely new and is the **real Phase-1 integration risk**. | §4.1, §7.8, §8 |
| **C4** | §7.6 framed the in-enclave spending key as a *trust* escalation. | It's also an **operational fund-loss footgun**: dstack/KMS derives enclave keys from the **code measurement (MRTD)**. Rebuild the image → key changes → **ZEC at the old shielded address becomes unspendable**. In-enclave Zcash key derivation is also DIY (Phala ships none). | §7.6, §8, §9 |
| **C5** | Phase 2: "shielded payment → … → NEAR Intents." | NEAR Intents touches **transparent `t`-addresses only**. "Private ZEC" UX is *wallet-side* auto-shielding, which Intents does not provide. The unshield is mandatory and exposes the creator's revenue at that hop. (v0 §7.5 already hinted this; v1 makes it unmissable.) | §7.5 |
| **C6** | (not in v0 — found live during spike #2) | A freshly-created mainnet tx used consensus branch `0x5437f330` (the current network upgrade); `zcash_primitives 0.27` **rejects** it ("invalid consensus branch id") while older txs parse. A scanner whose librustzcash lags a network upgrade **silently misses every payment after it**. Fix: keep the zcash crates current, or decode **branch-tolerantly** (the branch id is irrelevant to IVK note decryption) — `ivk-incoming-probe` rewrites the embedded branch to NU5 before parsing. | Lane A1 build (branch-tolerant read) |

---

## 1. Goals and non-goals

### Must-haves for Phase 1 (the MVP we demo)

1. **Indexer cannot decrypt content.** Enforced by hardware (Intel TDX) + remote attestation, not by trust.
2. **Buyer pays in shielded ZEC.** Buyer-side privacy comes from Zcash's shielded protocol. Payment is to a **shielded** address (required — memos only exist for shielded outputs).
3. **Buyer identity is unlinkable** from the purchase event across the chain, the catalog, and the dispatch bucket.
4. **Creator receives shielded ZEC** at their own Zcash address, managed with their own wallet (Zashi). No cross-chain settlement, no TEE-held spending keys, no unshield.
5. **End-to-end live demo** with one creator (us) and audience members as buyers, in under 5 minutes per transaction.

### Phase 2 goal (stretch, only if Phase 1 is solid)

6. **Creator can receive in any chain/asset they choose**, via NEAR Intents. **[C5]** NEAR Intents supports **transparent Zcash only** — `t1`/`t3` addresses, never Sapling/Orchard ([near-intents.org chain support](https://docs.near-intents.org/resources/chain-support#zcash)). So the Phase-2 path is *shielded payment → TEE-managed **unshield** → transparent ZEC → NEAR Intents → target chain*. The unshield step exposes the creator's revenue stream (not the buyer's identity); see §7.5 and §7.6.

### Non-goals

- **Subscriptions.** One-time content purchases only.
- **DRM / piracy prevention.** A paying buyer can leak their decryption key. Acknowledged, not solved.
- **Full network-layer privacy** (Tor / mixnets). Documented in the threat model; not implemented for the demo.
- **Recovery from a lost Buyer ephemeral key.** Session-scoped; closing the tab forfeits the purchase. (Mitigation in §9 / §7.3.)
- **Auditing or governance of which content gets sold.** Out of scope.
- **On-chain atomicity** of payment ⇆ key release. Impossible on Zcash (no script layer); the honest-but-curious TEE is the deliberate workaround. Never claimed.

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
              │                                          │ publish dispatch
              │ ① detect via IVK on COMPACT block        │
              │ ② GetTransaction(txid) → FULL tx         │
              │ ③ IVK-decrypt full enc_ciphertext → memo │
              ▼                                          │
   ┌──────────────────────────────────────────────────────────────────┐
   │                  TEE INDEXER  (Phala Cloud, Intel TDX)            │
   │                                                                   │
   │   Inside the enclave (operator CANNOT read):                      │
   │   - K_drops (per-drop AES-256 keys)                               │
   │   - Zcash IVK (Phase 1) / + SPENDING KEY (Phase 2 only)           │
   │   - Server dispatch keypair                                       │
   │   - NEAR account credentials (Phase 2 only)                       │
   │                                                                   │
   │   Code: open-source, reproducibly built. Image hash published.    │
   │   Attestation: TDX quote signed by Intel; published at /attest.   │
   │                                                                   │
   │   Logic (Phase 1):                                                │
   │   - [C2] INCOMING detection: trial-decrypt outputs with IVK       │
   │     (try_sapling_note_decryption / orchard incoming), NOT the     │
   │     OVK outgoing-recovery path clean-wallet uses.                 │
   │   - [C1] memo recovery requires the FULL tx, not the compact      │
   │     block (compact carries only 52 bytes of ciphertext).          │
   │   - decode memo → (drop_id, e_pub); verify amount ≥ price         │
   │   - wrap K_drop via ECIES → publish dispatch blob                 │
   └────────────────────────────┬──────────────────────────────────────┘
                                │ (Phase 2 only) NEAR RPC: submit Intent
                                ▼
   ┌───────────────────────────────────────────────────────────────────┐
   │     NEAR INTENTS (Phase 2)  — TRANSPARENT ZEC ONLY [C5]            │
   │  - TEE must UNSHIELD first: shielded → t-addr → Intents deposit    │
   │  - solvers compete to fill; bridge to target chain (Base, ETH…)   │
   └────────────────────────────┬──────────────────────────────────────┘
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
| 2 | **TEE Indexer** | Phala Cloud, Intel TDX | `K_drop`s, Zcash IVK (+ spending key, NEAR account in Phase 2), dispatch keypair | Yes — required for attestation to be meaningful |
| 3 | **Public bucket** | S3 + CloudFront (or Blossom/NIP-96) | Encrypted content blobs + dispatch blobs | n/a |
| 4 | **Creator dashboard** | Static frontend + small backend | Local upload/encrypt tool + attestation verifier + **[C3] secret-IN provisioning client** | Yes |
| 5 | **NEAR Intents (external)** | NEAR mainnet + solver network | Quotes, **transparent** deposit addresses, swap settlement | External dependency (Phase 2) |

**[C2] Reuse note:** Components that already exist in `week5/clean-wallet-mvp/` and transfer directly: the lightwalletd gRPC client (`lightwalletd.rs`), tx deserialization + Sapling/Orchard bundle iteration, the dstack attestation wrapper (`attest.rs`), and the Docker→Phala deploy pipeline. The **incoming-IVK scanner + memo extraction** and the **secret-IN provisioning** are new. See §7.8.

---

## 4. End-to-end flow

### 4.1 Onboarding (Phase 1 — creator owns the Zcash address)

1. Creator opens dashboard, fetches the TEE Indexer's published attestation, verifies the TDX quote against Intel's signing chain (via DCAP — in-browser `@phala/dcap-qvl-web` / t16z Trust Center) and verifies the code measurement matches the open-source repo's reproducible build.
2. Creator generates `K_drop = random_bytes(32)` locally in browser.
3. Creator encrypts content: `ciphertext = AES-256-GCM.encrypt(K_drop, plaintext)`.
4. Creator uploads `ciphertext` to the public bucket → gets `H_content`.
5. Creator submits to the TEE Indexer: `drop_id`, `price_zec`, `H_content`, `K_drop`, `creator_zcash_address`, `IVK_creator`.
   - **[C3] This is a *secret-IN* flow and is the real Phase-1 integration risk.** `K_drop` and `IVK_creator` must be sealed so that **only the attested, measured enclave binary can decrypt them** — not the Phala operator, not a swapped image. Mechanism options: (a) **RA-TLS** — the creator's browser verifies the TDX quote, then opens a TLS channel whose server key is bound to that quote, and sends the secrets over it; or (b) **encrypt-to-enclave** — the enclave derives a keypair whose public half is attested in the quote's `report_data`, and the creator encrypts `K_drop` to it. Unlike clean-wallet (which only sends an attested *result OUT*), nobody has built this *secret-IN* direction yet — budget a dedicated spike (see spike #3). Resolves Open-Q §8 #2.
   - **Note: only the IVK is given to the TEE. The spending key never leaves the Creator's wallet.**
6. TEE Indexer publishes the catalog entry: `{drop_id, price_zec, H_content, creator_zcash_address}`. The Creator continues to own this address and the funds that land on it.

### 4.2 Onboarding (Phase 2 — TEE owns the Zcash address)

Phase 2 reuses 4.1 steps 1–4 unchanged. Steps 5–6 differ:

5'. Creator submits via the §4.1-style secret-IN channel: `drop_id`, `price_zec`, `H_content`, `K_drop`, `payout_destination` (target chain + asset + address). **No Zcash address or IVK is submitted by the Creator.**

6'. **TEE generates a fresh Zcash shielded address + spending key inside the enclave** for this creator (or reuses an existing per-creator address). The IVK and spending key never leave the enclave. **[C4] Critical:** the seed backing this key is derived from the enclave's *measurement* — see §7.6 for the rebuild/fund-loss hazard this creates.

7'. TEE Indexer publishes the catalog entry: `{drop_id, price_zec, H_content, deposit_zcash_address}`. The deposit address belongs to the TEE, not the creator.

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
       │ user scans w/ Zashi  ──────────────────────────────────────────────── │
       │ [C2/buyer] memo only valid for a SHIELDED recipient; Zashi must honor  │
       │  the ZIP-321 memo param from the QR (VERIFY on demo build — spike #1)  │
       │                                     │                │                 │
       │ ── shielded tx (memo in note) ──▶│                 │                │   │
       │                   │                 │                │                 │
       │                   │ ◀─ ① GetBlockRange (COMPACT) ─   │                 │
       │                   │ ── compact blocks ──▶            │                 │
       │            [inside enclave: IVK trial-decrypt 52-byte compact          │
       │             outputs → DETECT incoming note → learn txid]               │
       │                   │ ◀─ ② GetTransaction(txid) ───    │                 │
       │                   │ ── FULL tx (enc_ciphertext) ──▶  │                 │
       │            [③ IVK-decrypt FULL enc_ciphertext → 512-byte memo          │
       │             → (drop_id, e_pub); verify amount ≥ price;                 │
       │             wrap K_drop via ECIES → blob]                              │
       │   [C1] memo is in bytes 52..564 of the FULL ciphertext — NOT in the    │
       │    compact block. ② is mandatory. ② also tells lightwalletd which tx   │
       │    the TEE cares about (TEE-side metadata leak; see §5).               │
       │                                     │ ── publish blob ▶                │
       │                                     │  (buyer can unlock NOW)          │
       │                                     │                │                 │
       │ ── poll new blobs ──────────────────────────────▶│                     │
       │ ◀── all dispatch blobs ─────────────────────────│                     │
       │ [try ECIES dec w/ e_priv on each → one yields K_drop]                  │
       │ ── fetch H_content ──▶│ ◀── ciphertext ──│                             │
       │ [AES-GCM dec(K_drop, ciphertext) → plaintext, render]                  │
       │                                                                        │
       │   ── Phase 1 flow ends here. Creator already received shielded ZEC. ── │
       │                                                                        │
       │  ════════════════════ PHASE 2 ADDITIONS ONLY ═══════════════════════   │
       │                                     │ [C5] UNSHIELD: shielded → t-addr │
       │                                     │ submit transparent tx           │
       │                                     │── transparent ZEC ──▶ NEAR Intents deposit (t-addr)
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
| **Dispatch blob unlinkable** | Blobs not tagged with buyer or drop ID; only the matching ephemeral private key decrypts | Network-layer caveat applies |
| **Buyer cannot be linked back through the bridge** | Unshield reveals only the *aggregate* TEE balance leaving the shielded pool | Batching strengthens; per-payment unshields weaken |

**Explicitly NOT guaranteed:**

- **[C1] TEE-side tx-interest metadata.** Because reading a memo requires `GetTransaction(specific_txid)`, the untrusted lightwalletd learns *which transactions the TEE fetched*. This is metadata about the **creator/TEE** side, not the buyer (the buyer's identity stays in the shielded set). Mitigation: decoy fetches, fetch all txs in a block, or run our own lightwalletd. Documented; for the demo we accept it.
- **Creator revenue privacy.** The Phase-2 unshield makes the creator's net revenue visible on transparent Zcash and the target chain. The creator opts into this by choosing a transparent destination chain. See §7.5.
- **TEE-custody-free flow.** In Phase 2 the TEE holds the spending key briefly. A TEE compromise during that window could drain pending funds. See §7.6.

---

## 6. Trust model

| Entity | Trust required | Why |
|---|---|---|
| **Intel** (TDX manufacturer) | High — hardware not backdoored | Unavoidable for production TEEs |
| **Phala Cloud** (operator) | Low — can DoS, cannot read enclave memory | TDX prevents read; attestation prevents code swap |
| **lightwalletd operator** | Low–Medium — sees TEE's tx-fetch pattern **[C1]**; could feed forged blocks (DoS / withhold) | Memo integrity is AEAD-protected (can't be forged); but availability + interest-metadata depend on it |
| **NEAR Intents solver network** (Phase 2) | Medium — settlement happens at fair price | If solvers misbehave, swaps fail/price poorly. Doesn't affect buyer privacy or content confidentiality. |
| **Reproducible build of our TEE image** | High — attestation is meaningless if reviewers can't verify what the measured hash does | Must be open source + reproducible. CI must publish artifact + hash. |
| **Buyer's browser environment** | High | Standard web trust |
| **Public bucket operator** | Low — sees encrypted blobs only | Could DoS by deleting blobs |

---

## 7. Tradeoffs and known security drawbacks

Listed in rough order of severity. (TEE failure history §7.1, NEAR §7.2, protocol §7.3, ops §7.4 are unchanged from v0 and abbreviated here; the corrected/expanded items are §7.5, §7.6, and the new §7.8.)

### 7.1 TEE-specific concerns (unchanged)

TEE security has a real failure history (SGX: Foreshadow, Plundervolt, LVI, SGAxe, ÆPIC Leak; SEV: CipherLeaks). TDX is newer (2023+) → fewer published attacks, partly because less-studied. Side-channels remain possible for co-located attackers. Hardware-vendor trust (Intel signing root) is unavoidable. **Attestation is only meaningful if the code is auditable** — publish source, ship a reproducible build, have an external party verify the image hash. Our design is stateless per request after `K_drop` provisioning, so rollback isn't a key risk. Operator can DoS.

### 7.2 NEAR Intents and cross-chain settlement concerns

**[C5] NEAR Intents supports transparent Zcash only** (`t1`/`t3`). Confirmed: [near-intents.org docs](https://docs.near-intents.org/resources/chain-support#zcash). This forces the unshield in §7.5. Solver settlement risk (fail to fill, bad price, latency — Creator sets slippage tolerance). Solver censorship (lower risk than a CEX bridge, not zero). **Atomicity gap:** the TEE releases the dispatch blob the moment it sees payment; unshield + settlement happen *after* and can fail. Mitigation: hold released ZEC briefly and verify Intents acceptance before publishing the blob — adds 10–60s buyer latency for atomicity.

### 7.3 Architectural and protocol concerns (unchanged + memo note)

Buyer ephemeral keypair is single-shot/unrecoverable (mitigate via `IndexedDB` 24h, blob retention window, recovery file). Network-layer correlation (buyer IP polling bucket + Zashi broadcast) — documented, needs Tor/mixnet. Catalog-browsing fingerprint — mitigate with client-side full-catalog fetch. **Memo size:** 512 bytes ([ZIP-302](https://zips.z.cash/zip-0302)); our payload (`drop_id` + `e_pub` + framing) is ~40 bytes — plenty of headroom. Replay / `drop_id` confusion handled via nullifier tracking.

### 7.4 Operational and economic concerns (unchanged)

TDX enclave cost ~$50/mo at demo scale, grows linearly. `K_drop` compromise → all past+future purchases for that drop exposed (no forward secrecy; rotate `K_drop` to protect future buyers). Code update → measurement change → re-attest + re-provision `K_drop`s.

### 7.5 Creator revenue privacy is lost at the unshield step

**[C5] Reframed for accuracy.** Because NEAR Intents accepts **only transparent** Zcash, the TEE must unshield before forwarding. **Do not picture "shielded in, shielded out via Intents" — that is structurally impossible.** The "private ZEC" experience seen in Zashi/Cake is the *wallet* auto-shielding *after* a transparent swap; Intents itself only ever touches `t`-addresses. So our TEE must replicate the de-shield: shielded note → transparent `t`-addr → Intents deposit address.

The unshield transaction publicly exposes: the amount, the destination `t`-address (the Intents deposit), and approximate timing. **Not** exposed: the buyer's wallet identity or any specific purchase↔buyer link (preserved by the shielded protocol upstream).

**Why acceptable to ship:** the creator has chosen a transparent destination chain (e.g. USDC on Base), where their revenue is already public. The transparent-Zcash hop only links Zcash-side numbers to already-visible target-chain receipts.

**Mitigation — batched unshielding.** Accumulate and unshield in batches (hourly, or per `$100` threshold), collapsing many purchases into one observable event. Cost: creator settlement latency = batch interval. Buyer experience unchanged. **Does not** protect against a patient observer watching the `t`-address for months — fundamental to a transparent destination.

### 7.6 TEE holds the Zcash spending key (Phase 2) — trust escalation **and** a fund-loss footgun

The TEE generates and holds a per-creator Zcash spending key inside the enclave to perform unshields. v0 treated this purely as a *trust* escalation. There is a second, more practical hazard:

**[C4] Measurement-bound key derivation → funds can become unspendable.** On dstack/Phala, the enclave's secrets are derived by the KMS from the **app measurement (MRTD / compose-hash)** — *not* from raw hardware sealing. Consequences:

- **Rebuild = new key.** Any code/image change alters the measurement → the derived seed changes → the Zcash spending key changes → **ZEC still sitting at the old enclave-derived shielded address is unspendable from the new build.** In a hackathon you redeploy constantly; this *will* strand funds unless handled.
- **In-enclave Zcash key derivation is DIY.** Phala ships no Zcash key logic. You take the KMS-derived seed and run ZIP-32 Sapling/Orchard derivation yourself (`librustzcash`/`zcash_keys`), pinning the derivation path + library version for determinism.

**Mitigations:**
- **Spend to zero before every redeploy.** Never redeploy with a non-empty enclave-derived balance.
- **Aggressive spend-out + batching policy** so steady-state balance ≤ one batch interval.
- **Wire dstack state migration / explicit key portability** if a stable address must survive rebuilds (extra work — do not assume it's automatic).
- **Per-creator key isolation** so one creator's compromise doesn't cascade.

**What this does NOT mitigate:** a TEE breach exactly while a high-value batch is assembled still costs that batch. Acceptable at demo scale (~$10–100/drop). Resolves/raises Open-Q §8 #8.

### 7.7 Demo-day concerns (unchanged)

Audience needs Zashi pre-installed (provide pre-loaded test devices). Recommend **mainnet with tiny amounts** (~$0.10/purchase) — testnet faucets + `testnet.zec.rocks` (~70% uptime, per `clean-wallet-mvp/README.md`) are too flaky; this matches the clean-wallet pivot to mainnet. Fall back to a pre-recorded screencast.

### 7.8 Build reality — what we reuse vs. what's new **[new in v1]**

Grounding the plan in the code that already exists (`week5/clean-wallet-mvp/`):

| Already built & reusable | New work for the drop |
|---|---|
| lightwalletd gRPC client w/ failover (`lightwalletd.rs`: `GetLatestBlock`/`GetBlockRange`/`GetTransaction`) | **[C2]** IVK **incoming** note decryption + **keep** the memo (clean-wallet does OVK *outgoing* + discards memo) |
| Full-tx fetch pattern (you already learned compact omits ciphertext) | **[C1]** Drive that pattern from memo-recovery instead of OVK recovery |
| dstack attestation wrapper + `report_data` binding (`attest.rs`) + quote verifier (t16z) | **[C3]** Secret-**IN** provisioning (seal `K_drop` to the measured enclave) |
| Docker → Phala CVM → MRTD-in-policy deploy pipeline (`task-15-runbook.md`) | ECIES dispatch-blob wrap/unwrap; buyer browser app; creator dashboard |
| — | **[C4]** (Phase 2 only) in-enclave ZIP-32 key derivation + unshield + key-portability handling |

**Implication:** the hardest *infrastructure* is in hand; the new work is drop-specific crypto plumbing plus the incoming-scan. No Phase-1 leg is impossible. The only *impossible* things (on-chain atomicity; shielded-through-Intents) are already designed around.

---

## 8. Open questions (research before commit)

1. ~~**Does NEAR Intents support ZEC as a source asset?**~~ **Resolved.** Transparent `t1`/`t3` only; unshield inside the enclave. See §7.5.
2. **[C3 — now the #1 Phase-1 risk] Secret-IN provisioning on Phala/Dstack.** Exactly how does the creator's browser verify the quote and seal `K_drop` so only the measured enclave reads it — RA-TLS, or encrypt-to-attested-enclave-key? Prototype before planning (spike #3).
3. **Public bucket choice.** S3 (easy, central) vs Blossom/NIP-96 (decentralized, less mature). Pick on team familiarity.
4. **Buyer browser key recovery.** Persist `e_priv` in `IndexedDB` 24h? UX choice? Drop on tab close?
5. **TEE provider redundancy.** Phala only, or AWS Nitro backup for the demo?
6. **Replay protection / dispatch blob retention window.** How long do blobs live in the bucket?
7. **Unshield batching policy** (Phase 2). Per-payment (low latency, worst privacy) vs hourly vs threshold. Default: hourly, creator-configurable threshold override.
8. **[C4] In-enclave key lifecycle** (Phase 2). Can we keep a *stable* per-creator Zcash address across enclave rebuilds (dstack state migration / key portability), or do we accept fresh addresses + spend-to-zero-before-redeploy? Answer decides whether Phase 2 is safe to iterate on.
9. **[C1/C2] Scanner mechanics.** Confirmed direction: detect on compact via IVK → `GetTransaction` → decrypt full ciphertext for memo. Open: decoy-fetch strategy to blunt the tx-interest leak, and acceptable poll cadence for ~30s unlock latency (spike #2 + spike #4).

---

## 9. Phased delivery

### Phase 0 — Software-only indexer (internal milestone, ~3 days)

Indexer runs as a normal server. Proves the full cryptographic flow end-to-end: **[C2]** IVK *incoming* detection, **[C1]** full-tx memo recovery, ECIES wrap, bucket dispatch, buyer-side polling and decrypt. `K_drop` is NOT confidential here (operator-visible). Integration only — not for demo.

Deliverables:
- Indexer server with **IVK incoming-payment detection + memo recovery via full-tx fetch** (new code; reuses `lightwalletd.rs`)
- Public bucket integration (S3 or Blossom)
- Buyer browser app: catalog → ZIP-321 QR → poll → decrypt
- End-to-end test: a teammate pays (real memo on mainnet), content unlocks in another teammate's browser

### Phase 1 — TEE-hosted indexer, Zcash-only (the demo target)

Move the Indexer into a Docker image, deploy on Phala Cloud (Intel TDX). Publish `/attest`. Build the **[C3] secret-IN provisioning** path and the creator-side attestation verifier.

**TEE holds only `K_drop`s and `IVK_creator` — no spending key, no NEAR account.** Creator owns their Zcash address; shielded → shielded; managed with Zashi.

Satisfies §1 must-haves 1–5. This is what we demo.

Deliverables:
- Reproducibly-built Docker image of the Indexer
- Phala Cloud deployment with public attestation endpoint
- **[C3] Secret-IN provisioning** (RA-TLS or encrypt-to-enclave) + creator-side attestation verifier UI
- All Phase 0 functionality preserved through the TEE boundary
- End-to-end live demo script with audience-as-buyers

### Phase 2 — TEE-mediated cross-chain settlement (stretch / bonus)

Extend the TEE to hold per-creator Zcash spending keys + a NEAR account. Adds §4.2 onboarding, §4.3 settlement (unshield + Intents), batched-unshield policy, updated attestation.

**Attempt only if Phase 1 is solid by end of week 2.** **[C4] Treat as a separate project with its own spike** — the in-enclave spending key, in-enclave ZIP-32 derivation, and unshield logic stack three hard things, one of which can strand funds on rebuild.

Deliverables (conditional):
- **[C4]** Per-creator ZIP-32 key derivation inside the TEE **+ a tested key-lifecycle/portability story** (resolves Open-Q #8)
- Unshield logic with configurable batching
- NEAR Intents integration: **unshield → transparent ZEC → target chain**
- Updated attestation, re-provisioning flow for existing creators

### Phase 3 — Post-hackathon, not committed

Multi-creator catalog UX, threshold dispatch (k-of-n TEEs), Tor/mixnet for buyer polling, mobile-native buyer app.

---

## 10. Demo plan (unchanged)

Three creators (us); audience as buyers. Each creator pre-uploads one drop. Attested addresses + catalog on the projector. Audience scans QR → opens catalog → picks a drop → scans the ZIP-321 QR with Zashi → pays (~0.01 ZEC). Within ~30s the dispatch blob lands and the browser auto-unlocks. Show sanitized TEE logs proving we *cannot* see the content. Fallback: pre-recorded screencast.

---

## 11. TEE tooling — recommendation (unchanged: Phala Cloud / Intel TDX / Dstack)

**Start with Phala Cloud.** Phase 0 runs anywhere; Phase 1 wraps the same code in Docker and pushes to Phala; Phase 2 extends the same container. One artifact, one deploy workflow, one set of logs. Alternative: AWS Nitro Enclaves (AWS-only, `vsock` overhead). Rejected for scope: Marlin Oyster, Azure/GCP confidential, Enarx, Gramine/Occlum, mobile enclaves. First day of Phase 1: a Phala "hello attested world" before swapping in the real Indexer.

---

## 12. References

- Reused code: [`../../week5/clean-wallet-mvp/`](../../week5/clean-wallet-mvp/) (`apps/scanner/src/{scan,lightwalletd,attest}.rs`).
- ZIP-321 payment URIs. [zips.z.cash/zip-0321](https://zips.z.cash/zip-0321)
- ZIP-302 memo field (512 bytes). [zips.z.cash/zip-0302](https://zips.z.cash/zip-0302)
- ZIP-307 light-client protocol (compact blocks omit memo). [zips.z.cash/zip-0307](https://zips.z.cash/zip-0307)
- lightwalletd compact formats (52-byte ciphertext). [github.com/zcash/lightwallet-protocol](https://github.com/zcash/lightwallet-protocol/blob/master/walletrpc/compact_formats.proto)
- NEAR Intents chain support (transparent Zcash only). [docs.near-intents.org/resources/chain-support#zcash](https://docs.near-intents.org/resources/chain-support#zcash)
- Phala dstack (TDX, RA-TLS, KMS-derived keys). [docs.phala.com/dstack/overview](https://docs.phala.com/dstack/overview)
- In-browser TDX quote verification. [github.com/Phala-Network/dcap-qvl](https://github.com/Phala-Network/dcap-qvl)
- Sean Bowe, "Tachyon: scaling Zcash via oblivious synchronization." [seanbowe.com](https://seanbowe.com/blog/tachyon-scaling-zcash-oblivious-synchronization/)
- Previous scope decision: [`../../week4/drop/project-scope.md`](../../week4/drop/project-scope.md)
