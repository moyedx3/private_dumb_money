# Clean-Wallet MVP — Design Spec

**Date:** 2026-05-26
**Status:** Draft, awaiting user review
**Scope:** One-week hackathon-grade MVP of the `week4/clean-wallet/` idea: a real Zcash testnet scan running inside a real Phala Cloud TEE, producing a screening artifact bound to a hardware attestation quote. ZK layer deferred to v2.

## 1. Problem and Claim

Exchanges treat shielded-origin ZEC as high-risk because they cannot inspect the source of funds. The naive answer — "user submits a list of recipients and proves no intersection with a sanctioned set" — fails the *completeness* test: the user can omit records. See `week4/clean-wallet/README.md` for the full argument.

This MVP implements the narrow claim:

> Given a specific Zcash viewing scope, an attested scanner processed the complete block range `[start, end]`, derived all relevant records visible under that scope, and found no outgoing recipient matching the sanctioned ZEC address set. The result is bound to a specific screening policy and a specific deposit intent.

Completeness comes from the attested scan. Non-intersection privacy on the recipient set is documented as future work (the optional ZK layer).

## 2. Architecture

Single-CVM "scanner-as-service" — chosen over a split prover/verifier design (over-engineered) and a custodial-wallet design (breaks the user-controlled viewing-scope narrative).

```
┌─────────────────┐                   ┌─────────────────────────────────────┐
│  User UI        │                   │   Phala Cloud CVM (Intel TDX)       │
│  (Next.js)      │                   │   ┌──────────────────────────────┐  │
│                 │  GET /attestation │   │ Rust HTTP server (axum)      │  │
│  paste UFVK ────┼──────────────────►│   │   POST /screen               │  │
│  paste deposit  │  POST /screen     │   │     │                        │  │
│  intent         │  {ufvk, policy,   │   │     ▼                        │  │
│                 │   intent}         │   │   zcash_client_backend       │  │
│                 │                   │   │   sync over [start,end]      │──┼──► public testnet
│                 │                   │   │     │                        │  │   lightwalletd (gRPC)
│                 │                   │   │     ▼                        │  │
│                 │                   │   │   derive outgoing recipients │  │
│                 │                   │   │   ∩ sanctioned set check     │  │
│                 │                   │   │     │                        │  │
│                 │                   │   │     ▼                        │  │
│                 │  {artifact, quote}│   │   dstack-sdk getQuote(       │  │
│                 │◄──────────────────┤   │     sha256(JCS(artifact)))   │  │
└─────────────────┘                   │   └──────────────────────────────┘  │
                                      └────────────────────────────────────-┘
┌─────────────────┐
│  Exchange UI    │
│  (Next.js)      │   paste {artifact, quote}
│                 │◄───────── user
│   verify quote ─┼─► dstack-verifier
│   match hash    │
│   show PASS/FAIL│
└─────────────────┘
```

### Trust view per block

| Block | Must trust | If compromised |
|---|---|---|
| User UI | Nothing (re-verifiable) | User might paste UFVK into a fake CVM — caught by attestation-before-upload check |
| Phala CVM | Intel TDX root + the published code measurement | All guarantees collapse — this is the load-bearing block |
| lightwalletd | Liveness only (returns blocks for the heights we ask) | Could lie about block contents; detection (header-chain verification) is future work |
| Exchange UI | Nothing (re-verifies everything from scratch) | Display bug at worst |

## 3. Decisions (locked)

- **Demo target:** live Zcash testnet scan + real Phala Cloud TEE attestation
- **Scanner stack:** Rust + `zcash_client_backend` + `lightwalletd` (Sapling + Orchard pools, full UFVK)
- **Lightwalletd endpoint:** `testnet.zec.rocks:443` (ECC LightWalletD, TLS-secured gRPC) as primary; a backup endpoint must be configured in the scanner because primary uptime is uneven (~69% over 7d, ~85% over 30d at time of spec). See §9 for failover semantics.
- **ZK layer:** deferred to v2; the artifact is signed JSON bound to a hardware quote
- **Demo data:** two pre-funded testnet UFVKs (one clean → PASS, one with a sanctioned-recipient hit → FAIL) + a curated sanctioned-address set
- **Timeline:** ~1 week sprint; polish on the exchange-verifier UI (the demo punchline); user UI is minimal
- **Canonical JSON:** RFC 8785 JCS, with cross-language golden vectors

## 4. Repo Layout

```
week5/clean-wallet-mvp/
├── apps/
│   ├── scanner/                       # Rust binary; runs in the Phala CVM
│   │   ├── Cargo.toml
│   │   ├── Dockerfile                 # multi-stage; final image = what Phala measures
│   │   ├── docker-compose.yml         # what Phala Cloud deploys
│   │   └── src/
│   │       ├── main.rs                # axum: GET /attestation, POST /screen, GET /health
│   │       ├── scan.rs                # zcash_client_backend sync + recipient derivation
│   │       ├── lightwalletd.rs        # tonic gRPC client wrapper
│   │       ├── policy.rs              # Policy, SanctionedSet types + canonical hash
│   │       ├── artifact.rs            # ScreeningArtifact type + JCS serialize
│   │       └── attest.rs              # dstack-sdk wrapper, getQuote(sha256(artifact))
│   │
│   └── web/                           # ONE Next.js app, two routes
│       ├── package.json
│       ├── app/
│       │   ├── page.tsx               # landing + demo nav
│       │   ├── prover/page.tsx        # User UI: attestation check → UFVK form → submit
│       │   └── verifier/page.tsx      # Exchange UI: paste artifact+quote → 3 checks → PASS/FAIL
│       └── lib/
│           ├── canonical.ts           # JCS — MUST match Rust byte-for-byte
│           ├── verify-quote.ts        # calls server route that proxies dstack-verifier
│           └── policy.ts              # client-side policy/depositIntent hashing
│
├── demo-data/                         # checked in; reproducible
│   ├── ufvk-clean.txt                 # pre-funded testnet wallet A (PASS)
│   ├── ufvk-dirty.txt                 # pre-funded testnet wallet B (FAIL)
│   ├── sanctioned-set.json            # curated test addresses; one matches a wallet-B recipient
│   └── policy.demo.json               # the demo policy (range, sanctioned set, expected code measurement)
│
├── packages/
│   └── schemas/                       # JSON Schemas — single source of truth
│       ├── policy.schema.json
│       ├── deposit-intent.schema.json
│       ├── screening-artifact.schema.json
│       └── fixtures/                  # golden canonicalization vectors
│
├── scripts/
│   ├── fund-demo-wallets.sh           # one-time: receive testnet ZEC, send tx from B to sanctioned
│   ├── golden-vectors.ts              # generate cross-language canonicalization fixtures
│   └── deploy-cvm.sh                  # push image, deploy via Phala Cloud SDK
│
└── docs/
    ├── demo-script.md                 # narration for the 5-minute demo
    └── trust-model.md                 # explains checks #1/#2/#3 in plain terms
```

### Why this layout

- **One Next.js app, two routes.** Both UIs share `canonical.ts` and `policy.ts` — splitting them doubles the build setup with no benefit for a 1-week sprint.
- **`scanner/` is the only deployable.** Everything inside it gets measured by Intel TDX. Anything outside isn't trust-critical, so it doesn't need attestation discipline.
- **`packages/schemas/` is the contract.** Rust types and TS types must serialize identically; the JSON Schemas + golden fixtures keep them honest.
- **Demo data lives in the repo.** Testnet UFVKs are not secrets (testnet ZEC has no value); checking them in makes the demo reproducible by anyone who clones.

### Scanner runtime configuration

Environment variables passed to the scanner container via `docker-compose.yml`:

| Variable | Example | Notes |
|---|---|---|
| `LIGHTWALLETD_PRIMARY` | `https://testnet.zec.rocks:443` | TLS gRPC endpoint; primary data source |
| `LIGHTWALLETD_BACKUP` | `https://testnet.lightwalletd.com:9067` | Second endpoint; scanner falls over on connection failure or hash-link mismatch from primary |
| `NETWORK` | `testnet` | Hard-checked against `policy.network` on each request (rejects with 400 if mismatch) |
| `MAX_RANGE_BLOCKS` | `100000` | Caps the scan window; rejects oversized requests with 400 |
| `DSTACK_SOCKET` | `/var/run/dstack.sock` | Default for Phala Cloud; overrideable for the `dstack-simulator` in CI |

The lightwalletd endpoints are **not** in the Policy schema. Reason: with header-chain verification listed as future work, pinning the endpoint in the policy would imply a defense we don't yet have. Once header-chain verification ships, endpoint pinning becomes meaningful and can move into the policy. For MVP, the trust assumption "lightwalletd returns honest blocks" is documented at §9 as out-of-scope.

### What's deliberately out

- No database, no auth, no multi-tenant.
- No `circuits/` directory (ZK deferred).
- No separate `prover-ui/` and `verifier-ui/` apps.

## 5. Data Model

Three structured objects. Everything else flows from them.

### 5.1 Policy

Exchange-defined, fixed before the request.

```json
{
  "policyName": "demo-v1",
  "policyVersion": 1,
  "network": "testnet",
  "auditStartHeight": 2900000,
  "auditEndHeight": 2950000,
  "sanctionedAddressHashes": ["0x9f7c…", "0x12ab…"],
  "expectedScannerCodeMeasurement": "0x8c4f…",
  "createdAtUnix": 1716700000
}
```

`policyHash = sha256(JCS(policy))`

### 5.2 DepositIntent

Bound to a specific pending deposit.

```json
{
  "exchangeName": "demo-exchange",
  "exchangeDepositAddress": "ztestsapling1…",
  "depositAmountZat": "100000000",
  "nonce": "0x6f2a…",
  "expiryUnix": 1716800000
}
```

`depositIntentHash = sha256(JCS(intent))`

Amounts are strings to avoid JSON number-precision pitfalls under JCS.

### 5.3 ScreeningArtifact

Emitted by the scanner inside the CVM.

```json
{
  "schemaVersion": 1,
  "result": "PASS",
  "scanRange": { "network": "testnet", "startHeight": 2900000, "endHeight": 2950000 },
  "policyHash": "0xb1e3…",
  "depositIntentHash": "0x47d9…",
  "viewingScopeCommitment": "0xa085…",
  "recipientCount": 17,
  "sanctionedHitCount": 0,
  "scannerCodeMeasurement": "0x8c4f…",
  "scanCompletedAtUnix": 1716750000
}
```

`artifactHash = sha256(JCS(artifact))` → goes into the quote's `reportData` (32 of 64 bytes used, rest zero-padded).

The artifact carries no recipient addresses, no amounts, no txids, no memos. The only quantitative leak is `recipientCount`; `sanctionedHitCount` is just the FAIL counter.

### 5.4 viewingScopeCommitment

`sha256("clean-wallet-vsc-v1" || ivk_fingerprint_bytes)` where `ivk_fingerprint_bytes` is the SHA-256 of the canonical encoding of the UFVK's incoming-viewing-key components (Sapling + Orchard). Lets a user later prove a given artifact corresponds to a given UFVK, without the artifact leaking the UFVK itself.

## 6. Hash Chain

```
   ┌────────────────┐         ┌────────────────┐         ┌────────────────┐
   │     Policy     │         │ DepositIntent  │         │      UFVK      │
   │ (exchange-set) │         │ (per-deposit)  │         │ (user-supplied)│
   └───────┬────────┘         └───────┬────────┘         └───────┬────────┘
           │                          │                          │
           │ sha256(JCS)              │ sha256(JCS)              │ sha256(ivk fingerprint)
           ▼                          ▼                          ▼
       policyHash                depositIntentHash       viewingScopeCommitment
           │                          │                          │
           └──────────────────┬───────┴──────────────────────────┘
                              ▼
                  ScreeningArtifact (JSON)
                              │
                              │ sha256(JCS(artifact))
                              ▼
                       artifactHash (32 bytes)
                              │
                              │ goes into reportData
                              ▼
                  Phala attestation quote (signed by Intel TDX root)
```

A dangling thread breaks the chain:
- Change one byte of the artifact → hash no longer matches `reportData` → exchange rejects.
- Re-sign the quote without the hardware root → exchange rejects.
- Submit yesterday's PASS for today's deposit → `depositIntentHash` mismatch → exchange rejects.

## 7. End-to-End Flow

```
EXCHANGE              USER UI                  PHALA CVM                LIGHTWALLETD       EXCHANGE UI
   │                     │                         │                          │                  │
   │ (1) publish policy  │                         │                          │                  │
   ├────────────────────►│                         │                          │                  │
   │                     │                         │                          │                  │
   │ (2) create deposit  │                         │                          │                  │
   │     intent          │                         │                          │                  │
   ├────────────────────►│                         │                          │                  │
   │                     │ (3) GET /attestation    │                          │                  │
   │                     ├────────────────────────►│                          │                  │
   │                     │◄────────────────────────┤  quote (no userdata)     │                  │
   │                     │                         │                          │                  │
   │                     │ (4) compare quote.code- │                          │                  │
   │                     │     Measurement against │                          │                  │
   │                     │     policy.expected-    │                          │                  │
   │                     │     ScannerCodeMeasure  │                          │                  │
   │                     │                         │                          │                  │
   │                     │ (5) POST /screen        │                          │                  │
   │                     │     {ufvk, policy,      │                          │                  │
   │                     │      depositIntent}     │                          │                  │
   │                     ├────────────────────────►│                          │                  │
   │                     │                         │ (6) gRPC stream blocks   │                  │
   │                     │                         │     [start, end]         │                  │
   │                     │                         ├─────────────────────────►│                  │
   │                     │                         │◄─────────────────────────┤                  │
   │                     │                         │ (7) decrypt outputs      │                  │
   │                     │                         │     under UFVK           │                  │
   │                     │                         │ (8) ∩ sanctioned set     │                  │
   │                     │                         │ (9) build artifact       │                  │
   │                     │                         │ (10) getQuote(           │                  │
   │                     │                         │       sha256(artifact))  │                  │
   │                     │◄────────────────────────┤ {artifact, quote}        │                  │
   │                     │                         │                          │                  │
   │                     │ (11) user copies blob to exchange ────────────────────────────────────►│
   │                                                                                              │
   │                                                                          (12) verify 3 checks:│
   │                                                                              quote, hash, intent│
   │                                                                          (13) render PASS/FAIL │
```

### Exchange UI verification (step 12)

Three checks, all client-side or via a thin server proxy to dstack-verifier:

1. **Quote is genuine.** `dstack-verifier` walks the quote back to the Intel TDX root. `quote.codeMeasurement` must equal `policy.expectedScannerCodeMeasurement`.
2. **Quote binds this artifact.** Recompute `sha256(JCS(artifact))` and check it equals `quote.reportData[0..32]`.
3. **Artifact binds this deposit and this policy.** Recompute `sha256(JCS(localDepositIntent))` → must equal `artifact.depositIntentHash`. Recompute `sha256(JCS(localPolicy))` → must equal `artifact.policyHash`. `artifact.scanRange` must match `policy.{network,start,end}`.

Only if all three pass does the UI render PASS/FAIL from `artifact.result`.

## 8. Canonical JSON

RFC 8785 JCS:
- Sorted object keys
- No whitespace
- Strict number canonicalization (and we sidestep float issues by encoding amounts as strings)
- Deterministic UTF-8

Rust: `serde_jcs`. TypeScript: `canonicalize` npm package.

`scripts/golden-vectors.ts` emits ~10 fixtures into `packages/schemas/fixtures/`. Both Rust and TS tests load these fixtures, re-canonicalize, and assert byte-equality + sha256 match.

## 9. Error Handling

### Core principle: fail closed

Anywhere upstream of artifact emission, if anything goes wrong, the scanner emits **no artifact at all**. No partial-scan PASS, no "best effort" result. Either a full clean scan + a real quote, or an error and the user retries.

### Error table

| Stage | Failure | HTTP status | User-facing message |
|---|---|---|---|
| Scanner — validation | Malformed UFVK | 400 | "Viewing key could not be parsed." |
| | Policy/intent fails JSON Schema | 400 | "Policy or deposit intent is malformed." |
| | `policy.network ≠ "testnet"` | 400 | "Scanner is configured for testnet only." |
| | `depositIntent.expiryUnix < now` | 400 | "Deposit intent has expired." |
| | `auditEndHeight - auditStartHeight > 100_000` | 400 | "Scan range too large for this scanner." |
| | `auditEndHeight > currentTip + 1` | 400 | "Audit range exceeds current chain tip." |
| | Another scan in flight | 429 | "Scanner busy, retry in a moment." |
| Scanner — scan | Primary lightwalletd unreachable | (internal) | Fall over to `LIGHTWALLETD_BACKUP`; no error surfaced |
| | Both lightwalletd endpoints unreachable | 503 | "Block source unreachable, retry." |
| | gRPC stream interrupted partway (after failover attempt) | 503 | "Scan interrupted, retry." (no artifact emitted) |
| | Block hash chain doesn't link | 502 | "Block source returned inconsistent data." |
| | Zero outputs under UFVK | 200 (legitimate) | Artifact with `recipientCount: 0`. Not an error. |
| Scanner — attestation | `dstack-sdk.getQuote()` fails | 503 | "Attestation hardware unavailable, retry." |
| Verifier — quote | Signature invalid / not Intel-rooted | reject | "Attestation is not genuine." |
| | `quote.codeMeasurement` mismatch | reject | "Scanner code does not match the policy's expected version." |
| Verifier — binding | `sha256(JCS(artifact)) ≠ quote.reportData` | reject | "Attestation seal does not match this report." |
| | `artifact.policyHash` mismatch | reject | "Report is for a different screening policy." |
| | `artifact.depositIntentHash` mismatch | reject | "Report is not for this deposit." |
| | `artifact.scanRange` mismatch | reject | "Report covers a different scan range than the policy requires." |
| | `now > depositIntent.expiryUnix` | reject | "Deposit intent expired before verification." |

### Not errors

- **`result: "FAIL"` is a valid signed artifact**, not a system failure. Exchange treats it as a policy decision. The user can't retry their way out — the chain doesn't change.
- **`recipientCount: 0`** is a legitimate PASS for wallets with no outgoing payments in the range.

### Resource limits

- One scan at a time per CVM (second concurrent request returns 429).
- `maxRangeBlocks = 100_000`; demo policy will use ~5k blocks.
- Request body cap 16 KB.
- No persistent storage; logs to stdout.

### Out of MVP scope

- **lightwalletd lying about block contents.** Defense (header-chain verification against trusted checkpoints, or running our own zebrad) → future work.
- **Side-channel resistance.** No timing obfuscation, no PIR over the lightwalletd connection. Lightwalletd sees which block range we asked about; for a testnet demo this is acknowledged.
- **CVM operator running modified code.** Mitigated by the exchange's `expectedScannerCodeMeasurement` check, not by the TEE alone.

## 10. Testing

Five layers, smallest scope first.

### Layer 1 — Golden vectors

The first tests written, before any business logic. Rust and TS both load the same fixtures and assert byte-identical canonicalization. If these pass, the trust chain is byte-safe across languages.

### Layer 2 — Scanner unit tests (`scanner/src/`)

Pure logic, no network: policy/intent/viewing-scope hashing, all fail-closed branches, lightwalletd decode via tonic mock server.

### Layer 3 — Scanner integration vs Zcash regtest (`scanner/tests/`)

`docker-compose.test.yml` spins up zebrad + lightwalletd in regtest. Two regtest wallets mirror the demo wallets; B sends to a known "sanctioned" address. Tests:
- PASS path: scan wallet A → `result: "PASS"`, `recipientCount > 0`, `sanctionedHitCount == 0`
- FAIL path: scan wallet B → `result: "FAIL"`, `sanctionedHitCount >= 1`
- Range mismatch → fail closed, no artifact
- lightwalletd mid-scan kill → 503, no artifact

Heaviest suite, but the only place shielded-pool decryption gets exercised end-to-end.

### Layer 4 — Attestation binding (without TDX hardware)

`dstack-simulator` produces structurally-valid quotes signed with a dev key. Tests:
- Round-trip: `sha256(JCS(artifact)) == quote.reportData`
- Code measurement match → verifier accepts
- Code measurement mismatch → verifier rejects
- Tampered artifact (one byte flipped) → verifier rejects

Runs in CI. Real Intel TDX attestation is only exercised in Layer 5.

### Layer 5 — Phala Cloud dry run (manual)

Must complete ≥24h before demo. Deploys actual Docker image to Phala Cloud, runs both wallet flows through the live UIs against real testnet, confirms quote at `proof.t16z.com`, records cold-start and worst-case scan times.

### What we don't test

- Intel TDX cryptography (Intel's job)
- `zcash_client_backend` correctness (upstream crate's job)
- JCS spec correctness (library's job)
- Throughput / load (single-CVM MVP has nothing meaningful to load-test)

### Demo rehearsal checklist (`docs/demo-script.md`)

- [ ] Both demo UFVKs still have expected on-chain history
- [ ] `testnet.zec.rocks:443` reachable (check hosh.zec.rocks status page right before demo — 7d uptime ~69% means a same-day check is mandatory)
- [ ] Backup lightwalletd URL configured in scanner env; manually verified reachable
- [ ] Phala CVM deployed and warm; policy `expectedScannerCodeMeasurement` matches deployed image
- [ ] Both PASS and FAIL flows complete in <60s
- [ ] proof.t16z.com renders the quote as genuine
- [ ] Screen-recording fallback prepared

## 11. What This MVP Does Not Prove

These limits are part of the design and must be stated in the demo:

- It does not prove that every wallet controlled by the user was scanned.
- It does not prove that another undisclosed viewing scope is clean.
- It does not prove the full upstream history of the ZEC is clean.
- It does not prove full OFAC, AML, or exchange compliance.
- It does not prove the sanctioned address set is complete.
- It does not remove Intel TDX hardware trust assumptions.

## 12. Future Work (not in MVP)

- ZK non-intersection circuit over the scanner's recipient hashes (hide `recipientCount`)
- Real OFAC SDN list ingestion + Merkle non-membership for large sanctioned sets
- Header-chain verification of lightwalletd's block responses
- ra-tls (attested TLS) for the User UI → CVM channel
- Multi-tenant exchange-side persistence + policy versioning
- Mainnet support
- Multiple viewing scopes per request
- Replace `viewingScopeCommitment` with a proof of UFVK derivation from a published unified address

## 13. Decision Log

| Decision | Choice | Alternative considered | Reason |
|---|---|---|---|
| Demo target | Live testnet + real TEE | regtest + real TEE; testnet + mock TEE | Most convincing single end-to-end demo |
| Scanner stack | Rust + zcash_client_backend + lightwalletd | TS + zcashd RPC (Sapling-only); Rust + zebrad direct | Best code reuse from week2 references; full UFVK Sapling+Orchard; light bootstrap |
| ZK layer | Deferred | Build now; stub | YAGNI for 1-week sprint; attestation alone tells full trust story |
| Demo data | Two pre-funded UFVKs + curated set | User UFVK + curated set; real OFAC SDN | Reproducible PASS *and* FAIL convincingly |
| Timeline | ~1 week sprint | 2-3 weeks; open-ended | Stated user constraint |
| Architecture | Single-CVM scanner-as-service | Split prover+verifier CVMs; custodial CVM | Smallest moving-parts count; correct trust narrative |
| Canonical JSON | RFC 8785 JCS | CBOR; bespoke ordering | Spec exists in both languages; easy to test |
| `viewingScopeCommitment` | Keep | Drop for MVP | Cheap to include; enables future UFVK↔artifact association |
| `reportData` packing | Just `sha256(artifact)` | `sha256(artifact) ‖ nonce` | `depositIntentHash` already provides replay protection |
| Lightwalletd endpoint | `testnet.zec.rocks:443` primary + configured backup | Run our own zebrad+lightwalletd; mainnet | Cheapest, most reliable for a 1-week sprint; mixed uptime mitigated by failover |
| Endpoint location | Scanner env vars, NOT in policy | In policy | Endpoint pinning without header-chain verification would be theater; once verification ships, endpoint moves into policy |
