# Unlockable Drop — Phase 1 task division (4 people)

> Companion to [`spec.md`](./spec.md) (v1) and [`spikes.md`](./spikes.md). This divides the **Phase 1 demo** (creator owns address, IVK only, shielded→shielded — no NEAR). Phase 2 is out of scope until Phase 1 is green.

## Team (confirmed: 2 Rust + 2 web)

Because the TEE indexer is the long pole, the **two Rust devs split it** — A1 (payment-flow) and A2 (platform) — and run in parallel after the interface freeze. The dedicated infra lane dissolves into **A2** (build/deploy/bucket) and **B** (wallets/demo). If you actually have a 3rd Rust dev, hand A2's bucket+CI to them and you parallelize deploy too.

## The big idea: freeze the seams on Day 1, then go parallel

The four components touch each other through exactly **six contracts**. Agree these in a 1-hour kickoff and write them into a one-page `interfaces.md`. After that, everyone codes against the contract (with mocks) and nobody waits on anybody.

| Contract | Producer → Consumer | Freeze it as |
|---|---|---|
| **Memo bytes** `drop_id ‖ e_pub` | Buyer (B) → TEE (A) | exact byte layout + base64url framing in ZIP-321 |
| **Dispatch blob** `ECIES(e_pub, K_drop)` | TEE (A) → Buyer (B) | curve (X25519), AEAD, wire format |
| **Catalog entry** `{drop_id, price_zec, H_content, deposit_zaddr}` | Creator (C) → TEE (A) → Buyer (B) | JSON schema |
| **Content blob** `AES-256-GCM(K_drop, plaintext)` | Creator (C) → Buyer (B) | nonce/tag layout |
| **Provisioning** seal `K_drop + IVK` to enclave | Creator (C) ↔ TEE (A) | RA-TLS or encrypt-to-enclave (spike #3 decides) |
| **Attestation** TDX quote + `report_data` binding | TEE (A) → Creator (C) | reuse clean-wallet's `attest.rs` format |

## The four lanes (2 Rust devs split the TEE core)

The two Rust devs split the long pole into **payment-flow (A1)** vs **platform/trust (A2)** — they share one crate with a clean module boundary, so they integrate without colliding. The old infra lane folds into A2 (build/deploy/bucket) and B (wallets/demo).

### Lane A1 — Payment-flow engine (Rust) · *clean-wallet scanner author*
The heart: see a payment, dispatch the key. **Most de-risked lane — can start now.**
- Productionize **spike #2**: `ivk-incoming-probe` → a **polling service** watching each creator's IVK, detecting payments, recovering the memo → `(drop_id, e_pub)`. *(probe already written + mainnet-verified; reuses `lightwalletd.rs`)*
- Verify `amount ≥ price`; nullifier tracking for replay (spec §7.3).
- **ECIES-wrap** `K_drop` for `e_pub` → dispatch blob → push to bucket.
- Stand up + own a **reliable mainnet lightwalletd** (you depend on it).
- **Owns:** memo format, dispatch-blob format. Crate modules: `scan`, `dispatch`.
- **First action:** finish the probe→service loop — you don't need the spikes to start.

### Lane A2 — Enclave platform: provisioning + attestation + deploy (Rust) · *second Rust dev*
The trust spine + the thing that actually ships.
- **Secret-IN provisioning endpoint** (enclave side): receive `K_drop + IVK` sealed so only the measured binary reads them — **spike #3 enclave side**, paired with C.
- Attestation endpoint + `report_data` binding *(reuse `attest.rs`)*; catalog store; axum server skeleton *(reuse `server.rs`)*.
- Docker image → **Phala deploy** *(reuse `deploy-cvm.sh`)* + the **reproducible build + CI image-hash** (load-bearing — without it C's verifier is meaningless, spec §7.1).
- **Stub the public bucket Day 1** (hash-addressed S3/Blossom) so nobody's blocked.
- **Owns:** provisioning endpoint, catalog schema, attestation format, bucket. Crate modules: `provision`, `attest`, `server`, deploy/CI.
- **First action:** spike #3 (enclave side) with C.

### Lane B — Buyer web app + demo/wallets (web) · *web dev*
The buyer experience, plus the Zashi-facing demo logistics. Build against mocks until A1/A2 are live.
- Catalog browse — **fetch the whole catalog once, client-side** (kills the fingerprint leak, spec §7.3).
- Ephemeral X25519 keypair per purchase; build **ZIP-321 URI + QR** (`memo = drop_id‖e_pub`).
- Poll bucket → **trial-decrypt** dispatch blobs with `e_priv` → `K_drop` → **AES-GCM decrypt** content → render.
- Fund tiny mainnet demo wallets + pre-load demo devices; own the **demo runbook + fallback screencast**.
- **Owns:** **spike #1** (Zashi memo-from-QR) — Day 1, cheapest kill-switch.
- **First action:** spike #1, then build against a mocked dispatch blob + catalog.

### Lane C — Creator dashboard + attestation/provisioning client (web) · *web dev, crypto-comfortable*
The trust UX + the client half of the enclave seam.
- Local content encryption (`AES-256-GCM` with `K_drop`) + upload ciphertext to the bucket.
- **Attestation verifier UI** — verify the TDX quote + measurement matches the repro build *(reuse clean-wallet's t16z / `@phala/dcap-qvl-web` verifier)*.
- **Secret-IN provisioning client** — verify quote → seal `K_drop + IVK` to the enclave. **Spike #3 client side**, paired with A2.
- **Owns:** content-blob format. **Spike #3** (client side).
- **First action:** spike #3 with A2.

## Timeline / parallelism

```
 DAY 1   ┌────────── 1-hr kickoff: freeze the 6 contracts → interfaces.md (all 4) ──────────┐
         └─────────────────────────────────────────────────────────────────────────────────┘
 SPIKES  │ B: spike#1 (Zashi) │ A1: finish spike#2 │ A2+C: spike#3 (secret-IN) │ A2: bucket+repro · A1: lwd
 (1-2d)  │   cheapest kill     │   probe→service     │   THE risky unbuilt one   │   unblock everyone
         │                    ── gate: all 3 spikes green before deep build (per feasibility review) ──
 BUILD   │ A1 payment-flow engine (scan → dispatch)          ┐
 (~1wk)  │ A2 enclave platform (provision · attest · deploy) ┘ two halves of the long pole, in parallel
         │ B  buyer web (zip321 → poll → decrypt → render)     ── on mocks, then wire to A1/A2
         │ C  creator dash + attest verify + provisioning     ── shares enclave seam with A2
 SHIP    │ all-hands: end-to-end run + demo rehearsal + fallback screencast
```

## Critical path & where it breaks

1. **The long pole is split, not removed.** A1 (payment-flow) and A2 (platform) run in parallel after the freeze — that's *why* 2 Rust devs tightens the schedule. They converge on one integration point: the indexer crate. Keep modules clean (`scan`/`dispatch` vs `provision`/`attest`/`server`) so they don't collide, and agree the in-crate API (how `dispatch` calls the bucket, how `server` invokes `scan`) at the kickoff.
2. **Spike #3 is the riskiest unbuilt thing** (secret-IN provisioning, `[C3]`). A2+C must prove it in days 1–2; if it fails, the "operator can't see K_drop" claim weakens and you redesign onboarding *before* building.
3. **Spike #1 is the cheapest kill-switch** — B runs it first; if Zashi drops the memo, rethink the buyer flow before building.
4. **A2 must stub the bucket Day 1** or it silently blocks B, C, and A1's dispatch.

## Definition of done (Phase 1 demo)
A teammate-creator pre-uploads one encrypted drop; an audience member scans a QR → pays ~0.01 ZEC shielded via Zashi → within ~30s their browser auto-unlocks the content; sanitized TEE logs show the indexer never saw the plaintext. (Spec §10.)

## After spikes are green
Each lane owner (or Claude) runs `superpowers:writing-plans` on their lane to get the bite-sized TDD task plan. This division is *who owns what*; that's *the step-by-step for each*.
