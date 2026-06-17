# Unlockable Drop — Feasibility Review (reality check on `spec.md`)

- **Status**: Sober teardown before committing to implementation.
- **Method**: Cross-checked `spec.md` against (a) the code you actually shipped in `week5/clean-wallet-mvp/`, (b) your week-3/5 notes, and (c) the live Zcash / NEAR / Phala specs. Every verdict below is cited.
- **One-line answer**: **Phase 1 is feasible** — and the scariest-looking piece (the attested TEE scanner) is the part you've *already built*. **Phase 2 is the danger zone.** There is **no clean-wallet-style "oh no it's impossible" landmine in Phase 1**, but there are five corrections the spec needs before you write a line of code.

---

## 0. The plain-language verdict (analogy first)

Think of the drop as a **6-step relay handoff** between buyer → chain → TEE → bucket → buyer. Five of those handoffs are ordinary baton passes you can practice. The one handoff that **does not exist in nature** is "the chain forces the runner to hand off the baton when paid" — Zcash has **no referee** (no smart-contract layer) to make payment-for-key atomic. That's the wall you hit in week 3.

**But you already decided to run the race without a referee** — that's the entire point of the "honest-but-curious TEE" design. Once you accept that, Phase 1 is a race you can win, because:

- The hardest leg — *a TEE that scans shielded Zcash and proves to a stranger it can't cheat* — is **exactly what clean-wallet is.** You bored that tunnel from the other side already.
- The remaining legs are standard crypto plumbing.

**Phase 2 adds a baton made of ice**: a Zcash *spending* key living inside the enclave. It works — until you rebuild the image, at which point the key changes and any funds sitting at the old address become **unspendable**. Most teams that try this drown there.

```
   BUYER ──①──▶ build ZIP-321 QR ──②──▶ Zashi pays (memo) ──③──▶ ZCASH CHAIN
   🟢 trivial            🟢 trivial         🟡 VERIFY Zashi build      │
                                                                       │ ④ TEE scans w/ IVK
                                                                       ▼   + reads memo
   CREATOR ──⑤──▶ seal K_drop to enclave        ┌──────────────────────────┐
   🟡 attested "secret-IN" (new)  ───────────────▶│   TEE INDEXER (Phala)    │
                                                 │ 🟡 IVK *incoming* scan + │
                                                 │    full-tx memo fetch    │
                                                 │    (NEW code — you built │
                                                 │     the OVK *outgoing*   │
                                                 │     path, not this one)  │
                                                 │ 🟢 attestation: DONE     │
                                                 └────────────┬─────────────┘
                                                  ⑥ ECIES-wrap K_drop, publish blob 🟢
                                                              ▼
   BUYER ──⑦──▶ poll bucket, trial-decrypt, render 🟢
   ─────────────────────── PHASE 1 ENDS HERE (feasible) ───────────────────────
   ══════════ PHASE 2: unshield → NEAR Intents → target chain ══════════
   🔴 in-enclave spending key = fund-loss footgun   🟡 Intents = transparent-only (unshield mandatory)
```

🟢 done/trivial 🟡 real but known work 🔴 hazard

---

## 1. Bottom line by component

| Component | Feasible? | Why / evidence |
|---|---|---|
| **TEE + remote attestation** (the scary part) | 🟢 **Already built** | `clean-wallet-mvp/apps/scanner/src/attest.rs` speaks real dstack protocol; binds artifact hash into TDX `report_data`; verifier checks the quote (t16z Trust Center). Real Intel-signed quotes work once deployed to Phala Cloud. |
| **lightwalletd plumbing** | 🟢 **Already built** | `lightwalletd.rs` does real gRPC `GetLatestBlock` / `GetBlockRange` / `GetTransaction` with failover; you already learned compact blocks omit ciphertext and you fetch full txs. Directly reusable. |
| **Docker → Phala deploy pipeline** | 🟢 **Already built** | `scripts/deploy-cvm.sh`, `task-15-runbook.md`, MRTD-in-policy flow. |
| **Buyer pays shielded ZEC w/ memo via ZIP-321 QR (Zashi)** | 🟡 **Feasible, must verify build** | ZIP-321 `memo` param is shielded-only and Zashi parses ZIP-321 QRs incl. memo ([ECC issue #1758](https://github.com/Electric-Coin-Company/zashi-android/issues/1758)). But `zcash:` deep-link handling / programmatic ZIP-321 *creation* had open gaps ([ECC #43](https://github.com/Electric-Coin-Company/zashi-android)). **Test on the exact Zashi build/devices you'll demo with.** |
| **TEE detects payment via IVK + reads memo** | 🟡 **Feasible, but NEW code** | This is **incoming** detection (IVK + `try_sapling_note_decryption`, keep the memo). Clean-wallet built the **outgoing** path (OVK + `try_*_output_recovery`, `_memo` discarded — `scan.rs:259,279`). ~80% of the plumbing reuses; the decrypt call + memo-keep is new. The good news: IVK incoming decryption is the *most standard* wallet op there is. |
| **Reading the memo at all** | 🟡 **Mandatory full-tx fetch** | Compact blocks carry only the **first 52 bytes** of ciphertext — **no memo** ([compact_formats.proto](https://github.com/zcash/lightwallet-protocol/blob/master/walletrpc/compact_formats.proto), [ZIP-307](https://zips.z.cash/zip-0307)). Memo lives in bytes 52–564 of the **full** 580-byte `enc_ciphertext`. You must `GetTransaction(txid)` and IVK-trial-decrypt the full ciphertext. You already do this for OVK — same pattern. |
| **Attested provisioning of `K_drop` into the enclave** | 🟡 **New, trickier than what you've done** | Clean-wallet gets an attested result *out*. The drop needs to seal a secret *in* (only the measured code can read `K_drop`). Feasible via dstack RA-TLS / enclave-bound key, but it's genuinely new integration. This is spec Open-Q #2 and the real Phase-1 integration risk. |
| **ECIES wrap + dispatch blob + buyer trial-decrypt** | 🟢 **Standard crypto** | X25519 + AES-GCM. No protocol risk. |
| **Memo capacity for `drop_id ‖ e_pub`** | 🟢 **Fits easily** | Memo = 512 bytes ([ZIP-302](https://zips.z.cash/zip-0302)); payload ~40 bytes. Confirmed in orchard `note_encryption.rs` (`type Memo = [u8; 512]`). |
| **Phase 2: NEAR Intents settlement** | 🟡 **Transparent-only, unshield mandatory** | [NEAR Intents docs](https://docs.near-intents.org/resources/chain-support): Zcash = "Transparent addresses only." ZEC is supported both as source and destination, but Intents only ever touches `t`-addresses. Spec already got this right. |
| **Phase 2: spending key inside the enclave** | 🔴 **Fund-loss footgun** | dstack KMS keys are derived from the **app measurement**. Rebuild the image → MRTD changes → derived key changes → **funds at the old shielded address become unspendable**. Plus in-enclave Orchard spend/unshield logic is DIY (Phala ships none). |
| **On-chain atomicity (payment ⇆ key)** | 🔴 **Impossible on Zcash** — but **designed around** | No script layer (week-3 finding, `project-scope.md` §1.1). The honest-but-curious TEE *is* the workaround. Not a blocker; just never claim atomicity. |

---

## 2. What clean-wallet already proved (your reusable foundation)

This is the reassuring half. The components people *assume* are the hard part of the drop are the components you've already de-risked:

1. **A TEE can scan shielded Zcash.** `scan.rs` decodes a UFVK, iterates Sapling shielded outputs and Orchard actions, and trial-decrypts them against a viewing key, against **live mainnet** lightwalletd. (It uses the *outgoing* OVK path — see §3.2 — but the machinery is the same.)
2. **The attestation story is real, not hand-waving.** `attest.rs` talks real dstack, packs your artifact's SHA-256 into the 64-byte `report_data`, and the web verifier chains the quote to Intel via the t16z Trust Center. Locally the simulator quote correctly *fails* signature check (dev-signed); real Phala Cloud gives Intel-signed quotes. This is the single most important thing you've proven, because "the indexer cannot decrypt content, enforced by hardware" is the drop's headline claim.
3. **You already hit, and solved, the compact-block ciphertext problem.** Your own code comment: *"Compact blocks omit `outCiphertext` so we must retrieve full txs via GetTransaction."* The drop needs the identical move for memos.
4. **You have a working Docker → Phala → MRTD-in-policy deploy loop** (`task-15-runbook.md`).

**Translation:** for the drop you are not starting at zero. You are starting with the lightwalletd client, the tx-deserialization + bundle-iteration, the attestation wrapper, and the deploy pipeline already in hand.

---

## 3. The five corrections the spec needs (the "sober" part)

These are the places where `spec.md` is wrong, understated, or quietly assumes something that bit you before.

### 3.1 "TEE polls the chain with IVK, decrypts the memo" hides a mandatory full-tx fetch

The spec (one-pager step 4, spec §4.3) makes memo-reading sound like a single scan. It isn't:

- **Detect** the incoming note from the 52-byte compact output (IVK trial-decryption) → learn the `txid`.
- **Then** `GetTransaction(txid)` and IVK-trial-decrypt the **full** `enc_ciphertext` to recover the 512-byte memo.

Two consequences the spec must state: (a) the TEE needs full-tx access, not just compact blocks; (b) calling `GetTransaction(specific_txid)` **tells the untrusted lightwalletd which tx the TEE cares about** — a minor metadata leak on the *creator/TEE* side (not the buyer). ZIP-307 suggests decoy fetches; for the demo, document it as accepted. *You already know this from clean-wallet — just don't let the spec pretend it's free.*

### 3.2 Your existing scanner is the OUTGOING path; the drop needs the INCOMING path

`spec.md` reads as if clean-wallet's scanner drops straight in. It does not:

- **clean-wallet** = "where did this wallet *send* money?" → OVK (outgoing viewing key), `try_sapling_output_recovery` / `try_output_recovery_with_ovk` (`scan.rs:213–290`), and it **throws the memo away** (`(_note, addr, _memo)`).
- **drop** = "who *paid* the creator, and what's in their memo?" → IVK (incoming viewing key), `try_sapling_note_decryption` / orchard incoming decryption, and you **keep** the memo.

This is new code. It's *more* standard than what you wrote (every wallet detects incoming funds this way), so it's not a risk — but scope it as new work, not a copy-paste.

### 3.3 Sealing `K_drop` INTO the enclave is harder than getting a result OUT

Clean-wallet's attestation flows one way: a result + quote come *out*, the client verifies. The drop's creator-onboarding flows the *other* way: a secret (`K_drop`) must go *in*, sealed so only the measured binary can read it. That's spec Open-Q #2 and the **real Phase-1 integration risk**. It's feasible (dstack RA-TLS, or encrypt to an attested enclave-derived public key), but you have **not** done this direction yet. Budget time for it; don't assume the existing verifier covers it.

### 3.4 Phase 2's in-enclave spending key is a fund-loss footgun, not just a "trust escalation"

Spec §7.6 frames the in-enclave spending key as a *trust* tradeoff. The bigger, unstated problem is **operational fund loss**:

> dstack/KMS derives the enclave's keys from the **app measurement (MRTD/compose-hash)**. Change the code → measurement changes → the derived seed changes → **the Zcash spending key changes** → any ZEC still sitting at the old enclave-derived shielded address is **unspendable from the new build.**

In a hackathon you rebuild constantly. This will eat funds unless you (a) spend out to zero before every redeploy, and/or (b) explicitly handle key portability/migration (dstack supports state migration, but you must wire it). Also: deriving Sapling/Orchard spending keys *inside* the enclave from that seed is **your code** (librustzcash) — Phala ships no Zcash key logic. **This is the part most likely to make you say "we thought it was possible…"** Treat Phase 2 as a separate project with its own spike.

### 3.5 "NEAR Intents = private shielded settlement" is structurally false

If anyone on the team pictures Intents delivering into the creator's Orchard address: it can't. **NEAR Intents only ever touches transparent `t`-addresses.** The "private ZEC" UX you see in Zashi/Cake is the *wallet* auto-shielding *after* a transparent swap. So Phase 2's real path is: enclave **unshields** → transparent ZEC → Intents deposit address → target chain. The spec's §7.5 already says this; just make sure the whole team internalizes that **the unshield is mandatory and the creator's revenue stream becomes transparent at that hop.**

---

## 4. Phase 1 verdict: 🟢 GREEN, with three conditions

The Phase-1 MVP (creator owns address, IVK only, shielded→shielded, no NEAR) is **buildable** and demo-able. Conditions:

1. **Verify the Zashi ZIP-321-QR-with-memo path on your actual demo devices this week** (§1, §3 corrections). If Zashi on the demo build strips the memo or won't honor ZIP-321 from a QR, the buyer flow is dead — so prove it before anything else.
2. **Write the IVK incoming-detection + memo-keep scanner** as new code (§3.2), reusing the lightwalletd client and full-tx pattern you already have.
3. **Build the attested "secret-IN" provisioning of `K_drop`** (§3.3) — the one genuinely new attestation direction.

Everything else (ECIES dispatch, buyer browser app, bucket) is standard.

## 5. Phase 2 verdict: 🟡→🔴 Treat as a separate, optional project

Feasible in principle, but it stacks three hard things (in-enclave spending key + unshield logic + Intents integration), one of which (§3.4) can silently burn funds. The spec's own gate — *"attempt only if Phase 1 is solid"* — is correct. Recommendation: **demo Phase 1; present Phase 2 as designed-and-spiked, not shipped**, unless Phase 1 lands with a week to spare.

---

## 6. Verify-before-you-commit checklist (do these first, cheaply)

| # | Spike | Kills the project if it fails? | Time |
|---|---|---|---|
| 1 | Zashi: scan a ZIP-321 QR to a shielded addr **with a memo** on your demo device → confirm the memo lands on-chain | **Yes** (buyer flow) | ½ day |
| 2 | Stand up IVK *incoming* detection on mainnet: send yourself a shielded payment with a memo, detect it + recover the memo via full-tx fetch | **Yes** (TEE core) | 1 day |
| 3 | Seal a secret into the Phala enclave and prove only the measured build can read it (RA-TLS or attested enclave key) | **Yes** (creator onboarding) | 1–2 days |
| 4 | Measure end-to-end latency: payment broadcast → TEE detects → blob published → buyer unlocks. Spec promises ~30s | No, but demo quality | ½ day |
| 5 | Pin a reliable mainnet lightwalletd (testnet.zec.rocks was ~70% uptime — `clean-wallet-mvp/README.md`) or run your own | No, but demo reliability | ½ day |

If spikes 1–3 pass, the spec is real and `writing-plans` can turn it into a task-by-task plan with confidence. If any of 1–3 fails, we redesign that leg *before* planning — which is the whole point of doing this now.

---

## Appendix — jargon, in one place

| Term | What it means here |
|---|---|
| **IVK** (Incoming Viewing Key) | Lets you *detect and decrypt notes sent TO an address*, incl. their memos. Cannot spend. What the TEE needs to see buyer payments. |
| **OVK** (Outgoing Viewing Key) | Lets you recover notes an address *SENT*. What clean-wallet used. Different operation from IVK. |
| **UFVK** (Unified Full Viewing Key) | Bundles IVK + OVK across pools; what `scan.rs` decodes. |
| **Compact block** | Bandwidth-saving block format from lightwalletd: carries only the first **52 bytes** of each output's ciphertext — enough to *detect* a note, **not** to read its **memo**. |
| **`enc_ciphertext`** | The full 580-byte encrypted output (52-byte note prefix + **512-byte memo** + 16-byte tag). Only in full transactions. |
| **`GetTransaction`** | lightwalletd RPC that returns a full tx — required to read memos. Reveals to lightwalletd which tx you wanted. |
| **TDX / attestation / MRTD** | Intel's confidential-compute (TEE). The *quote* cryptographically proves *which exact binary* ran; **MRTD** is that binary's measurement. Rebuild → MRTD changes → enclave-derived keys change. |
| **dstack / Phala / RA-TLS** | Phala's SDK for running Docker in TDX, issuing quotes, and binding TLS to the enclave's attested key. |
| **NEAR Intents** | Cross-chain swap/settlement network. For Zcash it speaks **transparent `t`-addresses only**. |
| **Unshield** | Moving ZEC from the shielded pool to a transparent `t`-address — makes the amount/destination **public**. Mandatory before NEAR Intents. |
| **ECIES** | Encrypt-to-a-public-key scheme (X25519 + AES-GCM here) used to wrap `K_drop` for the buyer's ephemeral key. |
| **"No script" problem** | Zcash has no smart contracts, so payment-for-key can't be atomic — the reason an off-chain TEE intermediary exists at all. |
