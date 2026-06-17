# Clean Wallet MVP3 Mainnet Handoff Meeting Notes

- Date: 2026-06-01 KST
- Topic: MVP3 current state and the next Zcash mainnet demo milestone
- Working directory: `week5/clean-wallet-mvp3`
- Status: Phala TDX deployment and guided UI demo are working. Real Zcash
  mainnet wallet fixtures and live PASS/FAIL `/screen` runs are the next task.

## 1. Goal

Build a real Zcash mainnet demo with two wallets:

| Wallet | Story | Required live result |
|---|---|---|
| Wallet A | Clean wallet. It may have ordinary outgoing transfers, but it never pays the fabricated sanctioned recipient. | `PASS` |
| Wallet B | Wallet with one outgoing transfer to the fabricated sanctioned recipient during the selected audit window. | `FAIL` |

The sanctioned recipient is a demo-only address. It is not an OFAC-listed
address and must never be presented as one.

## 2. What Was Completed

### Repository and implementation

- Created `week5/clean-wallet-mvp3` from the earlier clean-wallet MVP.
- Reviewed the earliest UI proof of concept in `../clean-wallet-poc`.
- Kept the real Rust scanner and real verifier APIs. Added a guided UI layer so
  the product story can be demonstrated before mainnet wallets are provisioned.
- Added a click-driven guided demo:
  - `Clean wallet` scenario: preview result `PASS`.
  - `Wallet with sanctioned transfer` scenario: preview result `FAIL`.
  - Private outgoing transfers are visualized on the user side.
  - The exchange verifier explains trusted scanner, untampered artifact, deposit
    context binding, and sanctions decision separately.
- Moved raw JSON forms under `Advanced live mode`. They remain available for
  real mainnet UFVK submission and real Phala quote verification.
- Upgraded Next.js from `14.2.0` to `14.2.35`.

### Real Phala Cloud deployment

| Field | Value |
|---|---|
| CVM name | `clean-wallet-scanner-mvp3` |
| CVM ID | `c9ee88a6-323b-4847-9285-8e3b331dce5a` |
| App ID | `af483db7eae7054b05f9dd526f26f65b0e738448` |
| Instance | `tdx.small` |
| Billing | `$0.058/hour` while running; CVM stopped after validation |
| Image | `ghcr.io/soarewee/clean-wallet-scanner:mvp3-dev` |
| Endpoint | `https://af483db7eae7054b05f9dd526f26f65b0e738448-8080.dstack-pha-prod5.phala.network` |
| Dashboard | `https://cloud.phala.com/dashboard/cvms/c9ee88a6-323b-4847-9285-8e3b331dce5a` |

The deployed compose configuration uses:

```text
LIGHTWALLETD_PRIMARY=https://zec.rocks:443
NETWORK=mainnet
```

### Attestation verification

- Live scanner `/health` returned `ok`.
- Live `/attestation` returned a real TDX quote.
- The quote verified successfully through the current Phala API:

```text
POST https://cloud-api.phala.com/api/v1/attestations/verify
```

- Verified code measurement:

```text
0xf06dfda6dce1cf904d4e2bab1dc370634cf95cefa2ceb2de2eee127c9382698090d7a4a13e14c536ec6c9c3c8fa87077
```

- Updated `apps/web/app/api/verify-quote/route.ts` to use the current Phala API.
  The older `https://proof.t16z.com/api/v1/verify` endpoint returned `404`.
- Confirmed quote-to-local-verifier flow returns `ok: true`.

### Validation completed

```text
Web tests:             19/19 passing
Next production build: passing
Deploy script syntax:  passing
Live scanner health:   ok
Real TDX quote verify: ok: true
```

Host Rust tests were not rerun in this workspace because host `cargo` was not
installed. The scanner was compiled successfully as a Linux `amd64` release
binary during the Docker image build used for deployment.

## 3. Current UI Usage

Run the web app:

```bash
cd week5/clean-wallet-mvp3
pnpm --filter clean-wallet-web dev --hostname 127.0.0.1
```

Open:

```text
http://127.0.0.1:3000/prover
```

### Guided demo path

1. Click `Clean wallet` or `Wallet with sanctioned transfer`.
2. Review the private outgoing transfer visualization.
3. Click `Run guided screening`.
4. Review the preview `PASS` or `FAIL` artifact.
5. Click `Send artifact to exchange verifier`.
6. Click `Verify Wallet artifact`.

This guided path is intentionally labeled as a preview. It explains the product
flow but does not claim that those visualized rows came from Zcash mainnet.

### Real path

Expand `Advanced live mode` in `/prover` and `/verifier`. This path calls the
deployed Phala CVM and the real Phala attestation verification API. It requires
provisioned mainnet UFVKs, a current policy, and an unexpired deposit intent.

## 4. Known Gaps and Warnings

### Existing wallet files are not usable for the mainnet demo

`demo-data/ufvk-clean.txt` and `demo-data/ufvk-dirty.txt` currently begin with
`uviewtest...`. They are inherited testnet fixtures. The deployed scanner is
configured for mainnet and correctly rejects them:

```text
HTTP 400
{"error":"Viewing key could not be parsed."}
```

Replace these files with `uview...` mainnet UFVKs before claiming a real
end-to-end mainnet demo.

### Stale testnet-era operational files were removed

The inherited `docs/task-15-runbook.md`, `docs/status-kr.md`,
`docs/demo-script.md`, and `scripts/fund-demo-wallets.sh` contained
pre-deployment or testnet-era instructions. They were removed to prevent
accidental execution. Use this meeting note and `docs/mvp3-deployment.md` as the
operational handoff.

### Phala CLI account API instability

After successful deployment, `phala status` and `phala cvms get` intermittently
returned `Unknown API error`. The deployed HTTPS endpoint remained healthy and
served real attestations. Use the Dashboard when CLI status calls fail.

### Billing

The CVM is a paid resource. It was stopped after validation. Start it before the
next live run:

```bash
phala cvms start clean-wallet-scanner-mvp3
```

Stop it again when it is not needed:

```bash
phala cvms stop clean-wallet-scanner-mvp3
```

For image updates, update the existing CVM instead of creating another paid
instance:

```bash
CVM_ID=c9ee88a6-323b-4847-9285-8e3b331dce5a \
IMAGE=ghcr.io/soarewee/clean-wallet-scanner:mvp3-dev \
./scripts/deploy-cvm.sh
```

## 5. Mainnet Demo Plan

### Safety constraints

- Use only tiny amounts of real ZEC required for the demo.
- Treat Wallet A and Wallet B seeds and spending keys as secrets.
- Never commit seeds, spending keys, wallet backups, or mnemonic phrases.
- Never paste seeds or spending keys into issues, chat logs, or screenshots.
- Commit only read-only UFVKs, public addresses, transaction IDs, block heights,
  hashes, and policy data intentionally used as demo fixtures.
- Clearly label the sanctioned recipient as fabricated demo data, not an actual
  sanctions-list entry.
- Any real ZEC transfer requires explicit operator approval immediately before
  broadcast.

### Proposed mainnet transaction graph

```text
Funding source
  ├── tiny ZEC ──► Wallet A (clean)
  │                 └── optional ordinary outgoing transfer only
  └── tiny ZEC ──► Wallet B (sanctioned scenario)
                    └── tiny ZEC ──► fabricated sanctioned recipient
```

Existing fabricated sanctioned recipient:

```text
t1Ss8dERcHbR9tQx6rN3tjhzK1vvAz4QgZu
```

It is already recorded in `demo-data/sanctioned-set.json` with its SHA-256 hash.

## 6. Mainnet TODO Checklist

### A. Prepare wallet tooling

- [x] **NEXT AGENT:** Decide the operational wallet path: use a human
  operator-controlled mainnet wallet workflow for seed backup, funding, and
  transaction broadcast. The agent receives only read-only UFVKs and public
  metadata. See `docs/wallet-operator-procedure.md`.
- [ ] **NEXT AGENT:** Verify the chosen tooling can export a mainnet UFVK and
  broadcast the Wallet B outgoing transfer.
- [x] **NEXT AGENT:** Add a short operator procedure under `docs/` for the chosen
  tooling. Do not include secrets.

Acceptance criteria:

- Wallet operator can create a wallet, back up its secret offline, obtain a
  `uview...` UFVK, obtain a receiving address, and send a tiny mainnet transfer.

### B. Create Wallet A and Wallet B

- [ ] **USER:** Create two new Zcash mainnet wallets with distinct secrets.
- [ ] **USER:** Store both secrets outside the repository.
- [ ] **NEXT AGENT:** Export and validate the read-only `uview...` UFVK for
  Wallet A.
- [ ] **NEXT AGENT:** Export and validate the read-only `uview...` UFVK for
  Wallet B.
- [ ] **NEXT AGENT:** Replace:
  - `demo-data/ufvk-clean.txt`
  - `demo-data/ufvk-dirty.txt`
- [ ] **NEXT AGENT:** Create or update `demo-data/wallet-meta.json` with public
  labels and addresses only.

Acceptance criteria:

- Both UFVK files begin with `uview...`, not `uviewtest...`.
- Neither file contains spending authority.
- The mainnet scanner accepts both UFVK formats.

### C. Fund the wallets with tiny mainnet amounts

- [ ] **USER:** Approve the exact funding amounts before broadcasting.
- [ ] **USER:** Send a tiny mainnet amount to Wallet A.
- [ ] **USER:** Send a tiny mainnet amount to Wallet B.
- [ ] **NEXT AGENT:** Record public funding transaction IDs and confirmation
  block heights in `demo-data/wallet-meta.json`.
- [ ] **NEXT AGENT:** Wait for sufficient confirmations before scanning.

Acceptance criteria:

- Wallet A and Wallet B each have confirmed mainnet funds.
- Public transaction metadata is documented without secrets.

### D. Create the fabricated sanctioned transfer

- [ ] **USER:** Approve the exact Wallet B transfer amount and destination
  immediately before broadcast.
- [ ] **USER:** Send a tiny amount from Wallet B to:

```text
t1Ss8dERcHbR9tQx6rN3tjhzK1vvAz4QgZu
```

- [ ] **NEXT AGENT:** Record the transaction ID and confirmation block height.
- [ ] **NEXT AGENT:** Confirm Wallet A never paid this recipient.
- [ ] **NEXT AGENT:** Confirm `demo-data/sanctioned-set.json` still describes
  this recipient as fabricated demo data.

Acceptance criteria:

- Wallet B has one confirmed outgoing mainnet transfer to the fabricated
  sanctioned recipient.
- Wallet A has no outgoing transfer to that recipient.

### E. Bind policy to the confirmed mainnet transaction

- [ ] **NEXT AGENT:** Update `demo-data/policy.demo.json`:
  - Keep `"network": "mainnet"`.
  - Keep the deployed scanner measurement unless a new image is deployed.
  - Set `auditStartHeight` before Wallet B's sanctioned transaction.
  - Set `auditEndHeight` after its confirmation block.
  - Keep the fabricated sanctioned-address hash in `sanctionedAddressHashes`.
- [ ] **NEXT AGENT:** Create fresh unexpired `DepositIntent` JSON fixtures for
  Wallet A and Wallet B demo runs.
- [ ] **NEXT AGENT:** Document the selected audit range and why it includes the
  Wallet B transfer.

Acceptance criteria:

- The policy range includes Wallet B's sanctioned transfer.
- The same range can be used to demonstrate Wallet A `PASS` and Wallet B `FAIL`.

### F. Execute real live runs

- [ ] **NEXT AGENT:** Start or confirm the existing Phala CVM.
- [ ] **NEXT AGENT:** Call `/attestation` and verify the quote through the Phala
  attestation API.
- [ ] **NEXT AGENT:** Run `/screen` with Wallet A UFVK, current policy, and an
  unexpired intent.
- [ ] **NEXT AGENT:** Confirm Wallet A returns an attested artifact with:

```text
result: PASS
sanctionedHitCount: 0
```

- [ ] **NEXT AGENT:** Run `/screen` with Wallet B UFVK, the same policy, and an
  unexpired intent.
- [ ] **NEXT AGENT:** Confirm Wallet B returns an attested artifact with:

```text
result: FAIL
sanctionedHitCount: >= 1
```

- [ ] **NEXT AGENT:** Paste both bundles into `/verifier` Advanced live mode.
- [ ] **NEXT AGENT:** Confirm all integrity checks pass for both artifacts.
- [ ] **NEXT AGENT:** Confirm the exchange decision is approved for Wallet A and
  rejected for Wallet B.

Acceptance criteria:

- Both live artifacts carry real verified TDX quotes.
- Wallet A produces real mainnet `PASS`.
- Wallet B produces real mainnet `FAIL`.
- The exchange verifier accepts the integrity of both reports and applies the
  report result correctly.

### G. Reconcile UI with real fixtures

- [ ] **NEXT AGENT:** Add one-click loading of the real Wallet A and Wallet B
  demo fixtures into Advanced live mode, without embedding any secret.
- [ ] **NEXT AGENT:** Keep the guided preview clearly labeled so it is never
  mistaken for a mainnet result.
- [ ] **NEXT AGENT:** Record the final real transaction IDs, block heights,
  policy range, and screenshots in a mainnet demo report.

## 7. Key Files

| File | Purpose |
|---|---|
| `docs/NEXT-LIVE-RUN.md` | Step-by-step guide for the next mainnet live session |
| `docs/wallet-operator-procedure.md` | Secret-safe human wallet operator workflow |
| `docs/mvp3-deployment.md` | Live Phala deployment record |
| `apps/scanner/docker-compose.yml` | Mainnet runtime config |
| `scripts/deploy-cvm.sh` | Build, push, create/update CVM |
| `apps/web/app/prover/page.tsx` | Guided preview and live scanner UI |
| `apps/web/app/verifier/page.tsx` | Guided exchange decision and live verifier UI |
| `apps/web/lib/guided-demo.ts` | Click-driven preview scenarios |
| `apps/web/app/api/verify-quote/route.ts` | Real Phala quote verification proxy |
| `demo-data/sanctioned-set.json` | Fabricated demo sanctioned recipient |
| `demo-data/policy.demo.json` | Mainnet policy and deployed measurement |
| `demo-data/ufvk-clean.txt` | Must be replaced with Wallet A mainnet UFVK |
| `demo-data/ufvk-dirty.txt` | Must be replaced with Wallet B mainnet UFVK |

## 8. Workspace State

- The `week5/clean-wallet-mvp3` tree is currently new and uncommitted.
- Root `.gitignore` is also new and uncommitted.
- Generated local paths such as `.env.local`, `.next`, and `node_modules` are
  ignored.
- Before committing, inspect `git status` and commit only intended source,
  documentation, and public fixture files.
