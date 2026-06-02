# Zcash Private Off-Ramp Screening

Prove that a Zcash wallet has not transacted with sanctioned addresses —
**without revealing its transaction history to the exchange.**

> MVP / hackathon demo. Mock chain + simulated attestation for local demo;
> real Zcash + real TDX (Phala) supported via Rust sidecar + RA-TLS.
> This is a narrow screening signal, **not** a compliance product.

> **팀원 시작점 → [`ONBOARDING.md`](ONBOARDING.md)** (셋업 · 테스트 · 배포 빠른 안내).

## The problem

Exchanges treat shielded-origin ZEC as high risk: they cannot inspect
source-of-funds. So users who chose privacy get their deposits rejected — or are
asked to hand over a viewing key, destroying that privacy.

## The approach

An **attested scanner** runs inside a TEE (Trusted Execution Environment):

1. The user submits their **viewing key + salt** directly to the enclave via
   **RA-TLS** (not to the exchange, not via env vars). Operators cannot see it.
2. The scanner pulls full blocks for the requested range, **verifies block-range
   completeness** (height + prev_hash chain), and derives every outgoing
   recipient visible under that scope (sapling/orchard/transparent).
3. It checks recipients against a sanctioned-address set.
4. It emits a **screening artifact** — `PASS`/`FAIL` + attestation, bound to a
   policy, deposit request, and **the chain source actually used**. No raw
   history leaves the TEE; no salt either.
5. The exchange verifies the artifact (7 checks).

The TEE solves *both* problems: scan **completeness** (which a ZK proof over
user-supplied records cannot) and recipient **privacy**. A ZK circuit was
deliberately dropped from the MVP — see [`docs/decisions.md`](docs/decisions.md).

## What it proves / does NOT prove

**Proves:** within the declared viewing scope and block range, no outgoing
recipient (shielded or transparent) matched the provided sanctioned set; the
result is bound to a specific policy, deposit request, and chain source.

**Does NOT prove:** that every wallet the user controls is clean; full OFAC/AML
compliance; the upstream provenance of the funds; that the lightwalletd operator
is honest (chain source is bound, and data correctness has opt-in PoW header-chain
verification — D12.2 — though that depends on the lightwalletd serving block
headers). It is a *narrow* signal.

## Quickstart

```bash
npm install
npm test          # 36 tests (core 17 + scanner 19)
npm run demo      # CLI: scan a clean wallet -> artifact -> verify
npm run dev       # web demo at http://localhost:3000
```

The FAIL path:

```bash
npm run demo:scan -- tainted   # scan a wallet with a sanctioned recipient
npm run demo:verify            # artifact verifies; trusted result = FAIL
```

Real-mode (Phala TDX + real UFVK via RA-TLS) — see
[`docs/deploy-phala.md`](docs/deploy-phala.md).

Results viewer (Phala scanner + AWS Amplify/DynamoDB) — UFVK stays CLI-only, the web
only stores and re-verifies the non-secret artifact — see
[`docs/deploy-web-amplify.md`](docs/deploy-web-amplify.md).

## Repository layout

```
clean-wallet/
├─ packages/core/    core library — types, scanner, attestation, artifact, verifier
├─ apps/web/         Next.js demo — Prover, Exchange Verifier, Results(DB) pages
├─ apps/scanner/     deployable scanner HTTP/HTTPS service
│  ├─ src/           server.ts (SCANNER_TRANSPORT: ratls/http), phala-attestation.ts
│  └─ tools/         submit-ufvk.ts — client to send UFVK via RA-TLS body
├─ apps/zcash-scanner-rs/   Rust sidecar — real Zcash scan (sapling+orchard+transparent)
└─ docs/             full documentation (Korean)
```

## Status

| Phase | Scope | State |
|---|---|---|
| 1–2 | Core pipeline, attestation, verifier, CLI | done |
| 3 | Next.js web demo | done |
| 4 | Real TEE (Phala) + real Zcash (Rust sidecar) | **deployed** — scanner live on Phala TDX, real-UFVK PASS/FAIL verified on mainnet (`docs/examples/`) |
| 5 | Web results viewer + DynamoDB + CLI `--save` | done — see [`docs/deploy-web-amplify.md`](docs/deploy-web-amplify.md) |

Phase 4 ships `Dockerfile`, `docker-compose.dstack.yml`, `PhalaAttestation` with
RA-TLS credentials, `apps/scanner/tools/submit-ufvk.ts` client, and the Rust
sidecar with block-range completeness + transparent-vout handling + Orchard
unified-address encoding + UFVK zeroize. The scanner is deployed on Phala Cloud
(dstack Gateway TLS passthrough → `SCANNER_TRANSPORT=ratls`, client verifies the
enclave quote directly + measurement pin; see [`docs/deploy-phala.md`](docs/deploy-phala.md) §1)
and real mainnet UFVK scans have been verified end-to-end (sample artifacts in `docs/examples/`).

## Documentation

Full docs live in [`docs/`](docs/) (Korean). Start with
[`docs/one-pager.md`](docs/one-pager.md), then `planning`, `architecture`,
`decisions` (D1–D12). Per-module notes are in
[`docs/implementation/`](docs/implementation/).

Korean version of this README: [`README.kr.md`](README.kr.md).
