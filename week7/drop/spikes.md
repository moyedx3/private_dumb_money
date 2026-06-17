# Pre-commit spikes — how to test the 3 things that decide the spec

These are the three legs that, if any fails, change the design. Run them **before** writing an implementation plan. Each says: goal → setup → steps → **pass/fail criteria** → what failure means.

Order: do **#1** first (cheapest, kills the buyer flow), then **#2** (the TEE core), then **#3** (creator onboarding).

---

## Spike #1 — Zashi honors a ZIP-321 QR memo to a shielded address

**Goal:** prove that *the exact Zashi build + device you'll demo with* will scan a ZIP-321 QR pointing at a **shielded** address with a `memo` param and produce a shielded tx that actually carries that memo. (External research says yes, but with historical gaps in `zcash:` handling — so verify, don't trust.)

**Setup:**
- The phone(s) you'll demo with, with the production **Zashi** installed (note the version + OS).
- Two shielded addresses you control: **A** = Zashi's own wallet (sender), **B** = a receiver whose memo you can read. Easiest **B**: a *second* Zashi wallet/account — Zashi displays received memos. (Mainnet, ~$0.10.)

**Steps:**
1. Base64url-encode a recognizable memo payload that mimics the real one, e.g. the bytes `drop_id=42|` followed by 32 random bytes standing in for `e_pub`. Keep it < 512 bytes.
2. Build the URI: `zcash:<B_shielded_addr>?amount=0.001&memo=<base64url>` (ZIP-321 form).
3. Turn it into a QR (any generator, or a one-file HTML page with a QR lib).
4. On phone **A**: scan the QR in Zashi. **Watch the compose screen** — does it pre-fill *address, amount, AND memo*? Send.
5. On wallet **B**: open the received tx and read the memo.

**PASS:** the memo on **B** byte-matches what you encoded in step 1.
**FAIL signals:** Zashi won't accept ZIP-321 from a QR; fills address/amount but **drops the memo**; disables the memo because it misreads the recipient as transparent; or mangles the base64url.

**If it fails:** the buyer UX as specced is dead. Fallbacks to evaluate: a custom buyer wallet/SDK that builds the shielded+memo tx directly (`zcash-swift/kotlin-payment-uri` + the mobile SDK), or carry `e_pub` out-of-band (not in the memo) — which changes §4.3 and the privacy story. Decide before planning.

---

## Spike #2 — IVK *incoming* detection + memo recovery on mainnet (the TEE core)

**Goal:** prove a server holding **only an IVK** can (a) detect an incoming shielded payment and (b) recover its 512-byte memo — via the mandatory full-tx fetch. This is `[C1]`+`[C2]`, the heart of the indexer. **This is the spike I can build for you** — it reuses `clean-wallet-mvp`'s lightwalletd client; the only new logic is *incoming* decryption (clean-wallet does *outgoing*) and *keeping* the memo.

**Setup:**
- A reliable **mainnet** lightwalletd (`zec.rocks:443`, or your own — testnet was ~70% uptime).
- A UFVK + its shielded unified address that you control (reuse `apps/scanner/src/bin/gen-ufvk.rs`). Derive the **IVK** from it.
- Send a small shielded payment to that address **with a known memo** — you can reuse Spike #1's send (point **B** at this address).

**Steps (new probe binary, e.g. `apps/scanner/src/bin/ivk-incoming-probe.rs`):**
1. Connect to lightwalletd; `GetLatestBlock` for the tip (reuse `LightwalletdClient`).
2. `GetBlockRange(tip-N .. tip)` → stream **compact** blocks.
3. For each compact Sapling output / Orchard action: **IVK-trial-decrypt the 52-byte compact** to *detect* a note addressed to you → collect candidate `txid`s. (52 bytes is enough to detect; **not** to read the memo.)
4. For each candidate: `GetTransaction(txid)` → deserialize the full tx.
5. **IVK-decrypt the full `enc_ciphertext`** with `try_sapling_note_decryption` (Sapling) / the Orchard incoming-decryption equivalent — using the *Incoming* Viewing Key, **not** `to_ovk()`/`try_*_output_recovery` (that's the outgoing path clean-wallet wrote). Keep the returned `(note, recipient, memo)` — **do not** drop `memo`.
6. Print: detected value, recipient, and the recovered **memo** (utf8 + hex). Log timestamps for latency.

**PASS:** recovered memo byte-matches what Zashi sent, and value matches. (That's §4.3 ①②③ proven end-to-end.)
**FAIL signals:** note never detected (IVK/address mismatch, or wrong pool); detection works but memo comes back empty/garbage (you decrypted the compact instead of the full ciphertext, or used OVK not IVK); decryption fails ZIP-212 enforcement (match Canopy/NU handling as in `scan.rs`).

**Bonus (feeds Spike #4):** record broadcast→detection wall-clock to sanity-check the spec's ~30s unlock promise.

**If it fails:** the indexer's core doesn't work as specced — but this is the most standard Zcash op there is, so failure almost certainly means a fixable key/pool/ciphertext-source mistake, not an impossibility.

---

## Spike #3 — Seal a secret INTO the Phala enclave (creator onboarding)

**Goal:** prove you can deliver `K_drop` so that **only the attested, measured enclave binary** can read it — not the Phala operator, not a swapped image. This is `[C3]`, the one attestation direction clean-wallet never built (it only sent results *out*). Bonus: this spike also makes the `[C4]` rebuild footgun visible firsthand.

**Setup:** a minimal dstack/Phala CVM you can deploy + redeploy; a client script (stand-in for the creator dashboard) that can verify a TDX quote (`@phala/dcap-qvl-web`, or reuse your t16z verifier).

**Path A — encrypt-to-enclave (most explicit):**
1. In the enclave, derive a keypair (or use a dstack KMS-derived key). Put the **public key (or its hash) into the quote's `report_data`**; serve quote + pubkey at `/attest`.
2. Client: fetch `/attest` → **verify the quote** (chains to Intel; measurement matches your open-source build) → extract the attested pubkey.
3. Client: encrypt a test secret to that pubkey (e.g. libsodium `sealed_box`) → POST the ciphertext.
4. Enclave: decrypt with its private key → reply with a possession proof that doesn't leak the secret (e.g. `sha256(secret || server_nonce)`).
5. **Negative test (operator):** try to decrypt the same ciphertext from the host/operator side → **must fail**.
6. **Rebuild test (`[C4]`):** change one line, redeploy → observe the measurement change → confirm the old ciphertext **no longer decrypts**. That is the fund-loss footgun in miniature; decide your key-lifecycle policy now (spend-to-zero before redeploy, or wire state migration).

**Path B — RA-TLS (less code, more magic):** use dstack's RA-HTTPS / `<app-id>.dstack.host` so the TLS cert is bound to the enclave's attested key; client verifies the binding, then sends `K_drop` over that TLS channel.

**PASS:** secret reaches the enclave, operator cannot read it, quote verification gates the whole thing.
**FAIL signals:** can't bind a value into `report_data`; can't verify the quote client-side without a trusted server (re-introduces a trusted party — note it); KMS key isn't stable enough to rely on.

**If it fails:** creator onboarding as specced doesn't hold — fall back to a weaker provisioning model (and downgrade the "operator can't see K_drop" claim) before planning.

---

## After the spikes

- **All three pass** → the spec is real; turn it into a task-by-task implementation plan (Phase 1) with `superpowers:writing-plans`.
- **#1 or #3 fails** → redesign that leg in `spec.md` first, then plan.
- **#2 fails** → almost certainly a key/pool/ciphertext bug, not an impossibility; fix and re-run.
