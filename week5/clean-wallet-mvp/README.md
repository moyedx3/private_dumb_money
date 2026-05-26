# Clean-Wallet MVP

> A Zcash off-ramp screening tool. The user uploads a read-only viewing key to an attested TEE; the TEE scans the chain itself, derives outgoing recipients via Sapling+Orchard OVK trial decryption, intersects against a sanctioned set, and emits a canonical-JSON `ScreeningArtifact` bound to a hardware attestation quote. The exchange verifies three hash checks without ever seeing wallet history.

## The lab analogy (TL;DR)

Picture a forensics lab that tests blood samples for court:

```
┌────────────────┐    ┌─────────────────┐    ┌────────────────┐
│   the patient  │    │   the lab       │    │   the court    │
│  (sample +     │───►│  (does the test │───►│  (reads result │
│  request form) │    │   + stamps it)  │    │   + verifies   │
│                │    │                 │    │   the stamp)   │
└────────────────┘    └─────────────────┘    └────────────────┘
```

Mapped onto this project:

| Lab analogy | This project |
|---|---|
| Patient + request form | User pasting UFVK + policy + deposit intent into `/prover` |
| Sample library (where blood goes) | Zcash blockchain (we read it via `lightwalletd`) |
| The lab equipment doing the test | Rust scanner inside a Phala Cloud TEE; OVK-decrypts outgoing notes, intersects against the sanctioned set |
| Tamper-evident wax seal | TDX attestation quote; binds `sha256(JCS(artifact))` into the quote's `reportData` |
| Court / prosecutor reading the result | Exchange `/verifier` running three hash checks |

## Architecture

```
┌─────────────┐   GET /attestation     ┌───────────────────────────────────┐
│ User UI     │ ─────────────────────► │  Scanner (Rust + axum)            │
│ /prover     │                        │  inside TEE (Phala Cloud TDX VM)  │
│             │   POST /screen         │                                   │
│  paste UFVK ├───────────────────────►│  → fetch compact blocks from      │ ──► testnet.zec.rocks:443
│  + policy   │   {ufvk,policy,intent} │    lightwalletd                   │     (real public testnet)
│  + intent   │                        │  → fetch full txs (GetTransaction)│
│             │ ◄──────────────────────│  → OVK-decrypt Sapling + Orchard  │
└─────────────┘   {artifact, quote}    │    outputs, derive recipients     │
                                       │  → intersect sanctioned set       │
                                       │  → emit canonical-JSON artifact   │
                                       │  → dstack.GetQuote(sha256(art))   │ ──► dstack-sdk
┌─────────────┐                        │    binds artifact hash into       │     unix socket
│ Exchange UI │   paste {bundle,       │    quote's reportData             │
│ /verifier   │   policy, intent}      └───────────────────────────────────┘
│             │
│ → check 1: quote signature chains up to Intel TDX root
│ → check 2: sha256(JCS(artifact)) == quote.reportData[0..32]
│ → check 3: policyHash + depositIntentHash + scanRange match locally
│ → render PASS / FAIL
└─────────────┘
```

## What's actually implemented vs simulated

### Real (verified end-to-end)

| Layer | Implementation |
|---|---|
| Zcash scan | `zcash_client_backend` + `zcash_primitives` doing real Sapling+Orchard OVK trial decryption over full txs fetched via `GetTransaction` RPC |
| Lightwalletd client | tonic gRPC over TLS (`webpki-roots`), verified live against `testnet.zec.rocks:443`; primary→backup failover |
| UFVK handling | Real bech32m parse via `zcash_keys::UnifiedFullViewingKey::decode` |
| Canonical JSON | RFC 8785 JCS, byte-identical between Rust (`serde_jcs`) and TS (`canonicalize`); 10 golden fixtures both sides pass |
| Hash chain | `policyHash`, `depositIntentHash`, `artifactHash`, `viewingScopeCommitment = sha256("clean-wallet-vsc-v1" ‖ ivk_fp)` — all spec-compliant, all tested |
| HTTP server | axum with CORS layer, 16 KB body cap, 1-scan-at-a-time mutex, fail-closed error mapping per spec §9 (26 unit tests) |
| Attestation wrapper | Direct unix-socket HTTP-over-UDS to dstack; correctly parses real `/Info` (incl. nested `tcb_info` JSON-string quirk) and `/GetQuote` |
| TDX quote parsing | Local extraction of `mr_td` (offset 184..232) and `report_data` (offset 568..632) from raw quote bytes; binding checks work even without Trust Center cooperation |
| Both UIs | Next.js with correct server/client boundary (`node:crypto`-using libs only reachable via API routes) |

### Currently simulated locally

| Piece | Status | What "real" looks like |
|---|---|---|
| TDX hardware | `dstack-simulator` v0.5.3 (dev-key signed quotes) | Real Intel TDX VM via Phala Cloud → Intel-signed quotes |
| Verifier Check 1 ("signature genuine") | Always ❌ — Phala Trust Center correctly rejects sim quotes | ✅ once deployed to real Phala Cloud |
| Demo wallets | Two fresh testnet UFVKs from hardcoded seeds, **no funds yet** | Funded testnet wallets, one with a deliberate outgoing tx to a "sanctioned" address |
| Sanctioned set | `demo-data/sanctioned-set.json` has `FILL_IN_*` placeholders | Real OFAC SDN list (or exchange-curated set) populated by the funding script |
| Deploy target | Local scanner binary | Docker image on Phala Cloud (Dockerfile + deploy script ready) |

### Deferred (called out in spec §12)

- ZK non-intersection proof over the recipient set (would hide `recipientCount`)
- Header-chain verification of lightwalletd's block honesty
- Side-channel / traffic-shaping over lightwalletd queries
- Multi-wallet support per request
- Real OFAC ingestion + Merkle non-membership for large sanctioned sets

## Testing it locally — full walkthrough

### Prerequisites

- Rust ≥ 1.85 (`~/.cargo/bin/cargo --version`)
- pnpm ≥ 9 (`pnpm --version`)
- Node ≥ 20
- Phala CLI: `npm install -g phala` (use a user-local npm prefix if you'd rather avoid sudo)

### 1. Run the test suites first (~1 min)

```bash
cd week5/clean-wallet-mvp
~/.cargo/bin/cargo test -p clean-wallet-scanner       # 26 Rust unit tests
cd apps/web && pnpm install && pnpm exec vitest run   # 17 TS tests
```

Both should be green.

### 2. Verify the live testnet connection (~30 s)

```bash
~/.cargo/bin/cargo test -p clean-wallet-scanner --lib live_testnet_returns_a_tip -- --ignored --nocapture
```

This is an `#[ignore]`d test that does a real TLS handshake to `testnet.zec.rocks:443` and fetches the current chain tip. If it fails, the public lightwalletd may be in a downtime window (7-day uptime ~70%) — try again in a few minutes or configure a backup endpoint.

### 3. Start the dstack simulator (~30 s, downloads ~8 MB the first time)

```bash
phala simulator start
```

The CLI downloads the official `dstack-simulator` binary from `Dstack-TEE/dstack` releases (v0.5.3) and runs it. The unix socket lives at `~/.phala-cloud/simulator/0.5.3/dstack.sock`.

### 4. Start the scanner pointed at the simulator + testnet (~20 s build, persistent)

```bash
DSTACK_SOCKET=$HOME/.phala-cloud/simulator/0.5.3/dstack.sock \
LIGHTWALLETD_PRIMARY=https://testnet.zec.rocks:443 \
RUST_LOG=info,clean_wallet_scanner=debug \
  ~/.cargo/bin/cargo run --release -p clean-wallet-scanner
```

You should see (within a few seconds):

```
INFO clean_wallet_scanner: scanner starting with code measurement measurement=0xc68518a0ebb42136...0e91fd
INFO clean_wallet_scanner: listening on :8080
```

If the boot fails with `dstack info failed at startup`, the simulator socket isn't where it should be — verify `ls $DSTACK_SOCKET`.

### 5. Start the Next.js UI (~5 s)

In a second terminal:

```bash
cd week5/clean-wallet-mvp/apps/web
pnpm dev
```

Open http://localhost:3000.

### 6. Click through the demo

In the `/prover` tab:

1. Click **Fetch attestation**. You should see the simulator's code measurement (`0xc68518a0…0e91fd`).

2. Paste the UFVK from `demo-data/ufvk-clean.txt`:
   ```
   uviewtest1jm2gjnn3dc5sm7qcnl6ud46x46lq7a3qdhdlhxkqdwzy0e8pu7v48898g45xz8jgxv2ry6lnqdf37pp59we9pqfn0n7a5f947gq68nt687vtzr7tjpcdyc35nl2lrkfxn5gsxmywvw2r0lqttl2hhd90r2x65dpnm9mx9gl50zkm0vrgfwn2p3rlc3xnurjwnr4hmhucjxtxrr8ecjwd36pf8my72alsfvpp00t3v2vuruzvdhkxj280w2xlt3x5s9njff9780kz2ekpznmt8fgvgzwvzpnz097rkz7aqwf3mg6sh072nzxvyrgf2jyx5dqhza6yenk9w95t8yeddzkfjvvdp5mpgv5ng6cjmpmhacvgzlkj7mppfrt06ukf4j87pks3yn7r0
   ```

3. Paste the policy from `demo-data/policy.demo.json`:
   ```json
   {"policyName":"demo-v1","policyVersion":1,"network":"testnet","auditStartHeight":3340000,"auditEndHeight":3340010,"sanctionedAddressHashes":[],"expectedScannerCodeMeasurement":"0xc68518a0ebb42136c12b2275164f8c72f25fa9a34392228687ed6e9caeb9c0f1dbd895e9cf475121c029dc47e70e91fd","createdAtUnix":1716700000}
   ```

4. Paste a deposit intent:
   ```json
   {"exchangeName":"demo-exchange","exchangeDepositAddress":"ztestsapling1abcdef0123456789","depositAmountZat":"100000000","nonce":"0x6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f6f","expiryUnix":4000000000}
   ```

5. Click **Submit screening request** — wait ~4 seconds. A JSON bundle with `result: "PASS"` + a quote appears.

6. Copy that bundle. Switch to `/verifier`. Paste the bundle + the **same** policy + intent. Click **Verify**.

### Expected outcome locally

```
❌ Check 1: Attestation is not genuine.
✅ Check 2: Attestation seal matches this report.
✅ Check 3: Artifact binds this deposit, policy, and scan range.
```

Check 1 ❌ is the **correct, expected** outcome with the simulator — its quote is signed with a developer key, not Intel's TDX root. Checks 2 and 3 passing proves the entire trust chain works structurally: the artifact's hash is genuinely baked into the quote, and your local computation of the policy/deposit hashes matches what the scanner produced.

## What changes when we deploy to real Phala Cloud

**Only one thing.** The simulator's dev-key-signed quote becomes a real Intel-TDX-signed quote, and Check 1 flips ❌ → ✅. Same Rust code, same UFVK, same scan, same verification logic. The Phala Cloud deploy is the operational step at `docs/task-15-runbook.md` and takes ~15 minutes (Phala account auth + Docker push + capture real MRTD + paste into policy).

## Where to read more

- **Spec** (the why) — `../../docs/superpowers/specs/2026-05-26-clean-wallet-mvp-design.md`
- **Implementation plan** (how it was built, task-by-task) — `../../docs/superpowers/plans/2026-05-26-clean-wallet-mvp.md`
- **Operational runbook** (Phala Cloud deploy, wallet provisioning) — `docs/task-15-runbook.md`
- **Trust model** (the three checks in plain English) — `docs/trust-model.md`
- **5-min demo script** (live presentation narration) — `docs/demo-script.md`
- **Scanner README** (Docker, regtest integration suite) — `apps/scanner/README.md`

## Glossary

| Term | Meaning |
|---|---|
| **UFVK** | Unified Full Viewing Key — read-only key that decrypts a Zcash wallet's notes (incoming + outgoing) across Sapling and Orchard pools; cannot spend |
| **OVK** | Outgoing Viewing Key (part of the UFVK) — lets you decrypt your own outgoing transactions to see whom you paid |
| **TEE / TDX** | Trusted Execution Environment / Intel's Trust Domain Extensions — hardware-encrypted VM with attestation |
| **MRTD** | Measurement of the TD — a 48-byte hash of the exact code running in the TDX VM, signed by Intel |
| **reportData** | 64-byte slot inside a TDX attestation quote where the app commits arbitrary data (we put `sha256(artifact)` here) |
| **lightwalletd** | gRPC server that streams compact Zcash blocks for light clients — what `zcash_client_backend` talks to |
| **JCS** | JSON Canonicalization Scheme (RFC 8785) — sorted keys, no whitespace, deterministic UTF-8 — so both sides hash to the same bytes |
| **dstack** | Phala's open-source guest agent that exposes a unix socket inside a CVM for attestation requests |
