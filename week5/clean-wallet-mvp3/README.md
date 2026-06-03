# Clean Wallet MVP3

Zcash mainnet off-ramp screening demo backed by a real Phala Cloud Intel TDX
CVM. A read-only UFVK is submitted to the scanner. The scanner checks outgoing
recipients against a policy-defined sanctioned set and returns an attested
`PASS` or `FAIL` artifact without sending wallet records to the exchange.

## Current State

- Real Rust scanner deployed to Phala Cloud.
- Real TDX `/attestation` quote verified through the Phala attestation API.
- Mainnet runtime configured with `https://zec.rocks:443`.
- Guided web UI provides click-driven `Clean wallet` and
  `Wallet with sanctioned transfer` preview scenarios.
- Advanced live UI remains available for real mainnet UFVK submission.
- Real mainnet Wallet A and Wallet B fixtures are the next milestone.

The inherited `demo-data/ufvk-*.txt` files are still testnet fixtures. Do not
claim a live mainnet PASS/FAIL demo until they are replaced with provisioned
`uview...` mainnet UFVKs and the policy audit window is updated.

## Run The Guided UI

```bash
pnpm --filter clean-wallet-web dev --hostname 127.0.0.1
```

Open:

```text
http://127.0.0.1:3000/prover
```

The guided flow is a labeled preview. Expand `Advanced live mode` for the real
Phala CVM path.

## Validate The Web App

```bash
CI=true pnpm --filter clean-wallet-web test --run
CI=true pnpm --filter clean-wallet-web build
```

## Deploy Or Update The Scanner

Use the existing paid CVM when updating the scanner image:

```bash
CVM_ID=c9ee88a6-323b-4847-9285-8e3b331dce5a \
IMAGE=ghcr.io/soarewee/clean-wallet-scanner:mvp3-dev \
./scripts/deploy-cvm.sh
```

Stop the CVM when it is not needed:

```bash
phala cvms stop clean-wallet-scanner-mvp3
```

## Read Next

- `docs/NEXT-LIVE-RUN.md` - operator checklist for the next real Zcash mainnet
  PASS / FAIL session. Start here when resuming live work.
- `docs/2026-06-01-mainnet-handoff-meeting-notes.md` - latest handoff meeting
  notes and the ordered Zcash mainnet Wallet A / Wallet B TODO checklist.
- `docs/wallet-operator-procedure.md` - human wallet operator procedure for
  creating Wallet A and Wallet B without exposing secrets.
- `docs/mvp3-deployment.md` - live Phala deployment record.
- `docs/trust-model.md` - trust model and verifier checks.
- `apps/scanner/README.md` - scanner-specific build and Docker notes.
