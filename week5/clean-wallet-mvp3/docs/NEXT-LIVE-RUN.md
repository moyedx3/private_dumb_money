# Next Live Run Guide

This is the execution guide for the next operator or agent.

Read `docs/2026-06-01-mainnet-handoff-meeting-notes.md` first when you need
background. Use this file when you are ready to run the next Zcash mainnet work
session.

## Objective

Produce two real, attested Zcash mainnet screening artifacts:

| Wallet | Mainnet history | Required scanner result |
|---|---|---|
| Wallet A | No outgoing transfer to the fabricated sanctioned recipient | `PASS` |
| Wallet B | One outgoing transfer to the fabricated sanctioned recipient | `FAIL` |

Fabricated demo sanctioned recipient:

```text
t1Ss8dERcHbR9tQx6rN3tjhzK1vvAz4QgZu
```

This address is demo data. It is not an OFAC-listed address.

## Before You Start

The previous session intentionally stopped the local server, Docker Desktop,
and the paid Phala CVM. The existing CVM must be started, not redeployed.

Do not expose secrets:

- Never commit or paste seed phrases, mnemonic phrases, spending keys, or wallet
  backups.
- Keep Wallet A and Wallet B secrets outside this repository.
- Only place read-only UFVKs, public addresses, transaction IDs, block heights,
  hashes, and demo policy data in `demo-data/`.
- Any real ZEC transfer requires user approval immediately before broadcast.

Current known blocker:

```text
demo-data/ufvk-clean.txt  -> inherited uviewtest... fixture
demo-data/ufvk-dirty.txt  -> inherited uviewtest... fixture
```

These must be replaced with real mainnet `uview...` UFVKs before a live
mainnet `/screen` run.

## 1. Resume The Existing Infrastructure

### 1.1 Start Docker Desktop only if rebuilding the scanner

Docker is not needed merely to start the existing CVM or run the web app.
Start Docker Desktop only when rebuilding and pushing a new scanner image:

```bash
open -a Docker
```

### 1.2 Start the existing paid Phala CVM

```bash
phala cvms start clean-wallet-scanner-mvp3
```

Confirm status:

```bash
phala cvms get clean-wallet-scanner-mvp3 --json | jq '{name,status,vm_uuid,app_id,resource,endpoints}'
```

Expected:

```text
status: running
vm_uuid: c9ee88a6-323b-4847-9285-8e3b331dce5a
app_id: af483db7eae7054b05f9dd526f26f65b0e738448
```

Scanner endpoint:

```bash
export SCANNER_URL=https://af483db7eae7054b05f9dd526f26f65b0e738448-8080.dstack-pha-prod5.phala.network
```

Health check:

```bash
curl -fsS "$SCANNER_URL/health"
```

Expected:

```text
ok
```

### 1.3 Verify the live TDX quote

```bash
curl -fsS "$SCANNER_URL/attestation" \
  | jq '.quote' \
  | curl -fsS \
      -H 'content-type: application/json' \
      --data-binary @- \
      https://cloud-api.phala.com/api/v1/attestations/verify \
  | jq '{verified, checksum}'
```

Expected:

```text
verified: true
```

The deployed scanner measurement should remain:

```text
0xf06dfda6dce1cf904d4e2bab1dc370634cf95cefa2ceb2de2eee127c9382698090d7a4a13e14c536ec6c9c3c8fa87077
```

If a new image is deployed, retrieve the new measurement and update
`demo-data/policy.demo.json`.

## 2. Create Mainnet Wallet A And Wallet B

### 2.1 Choose the wallet operator workflow

Use the selected operator-controlled workflow in
`docs/wallet-operator-procedure.md`. In short: a human operator creates and
controls both mainnet wallets, handles seed backup and transaction broadcast,
and gives the agent only read-only UFVKs plus public metadata.

The repository has Rust helpers:

```text
apps/scanner/src/bin/gen-ufvk.rs
apps/scanner/src/bin/gen-taddr.rs
```

They are useful for controlled fixture derivation, but they do not replace a
safe transaction-broadcast workflow.

### 2.2 User action: create two wallets

- [ ] **USER:** Create Wallet A with a new secret.
- [ ] **USER:** Create Wallet B with a different new secret.
- [ ] **USER:** Back up both secrets outside the repository.
- [ ] **USER:** Export the read-only mainnet UFVK for each wallet.
- [ ] **USER:** Provide only the two read-only UFVKs and public receiving
  addresses to the agent.

Agent validation:

```bash
head -c 10 demo-data/ufvk-clean.txt && printf '\n'
head -c 10 demo-data/ufvk-dirty.txt && printf '\n'
```

Both values must begin with:

```text
uview
```

They must not begin with:

```text
uviewtest
```

### 2.3 Replace public demo fixtures

Replace:

```text
demo-data/ufvk-clean.txt
demo-data/ufvk-dirty.txt
```

Create `demo-data/wallet-meta.json` with public metadata only:

```json
{
  "network": "mainnet",
  "walletA": {
    "label": "clean",
    "receivingAddress": "<public-address>",
    "fundingTxId": "<fill-after-funding>",
    "fundingBlockHeight": 0
  },
  "walletB": {
    "label": "sanctioned-transfer",
    "receivingAddress": "<public-address>",
    "fundingTxId": "<fill-after-funding>",
    "fundingBlockHeight": 0,
    "sanctionedTransferTxId": "<fill-after-transfer>",
    "sanctionedTransferBlockHeight": 0
  }
}
```

Do not include secrets in this file.

## 3. Create The Mainnet Transaction History

### 3.1 User approval checkpoint: fund both wallets

Before broadcasting, ask the user to approve:

- Exact amount sent to Wallet A.
- Exact amount sent to Wallet B.
- Funding source.
- Public receiving addresses.

Then:

- [ ] **USER:** Send a tiny mainnet amount to Wallet A.
- [ ] **USER:** Send a tiny mainnet amount to Wallet B.
- [ ] **AGENT:** Record transaction IDs and confirmation block heights in
  `demo-data/wallet-meta.json`.

### 3.2 User approval checkpoint: create Wallet B FAIL history

Before broadcasting, ask the user to approve:

- Exact tiny amount.
- Source: Wallet B.
- Destination:

```text
t1Ss8dERcHbR9tQx6rN3tjhzK1vvAz4QgZu
```

Then:

- [ ] **USER:** Send the approved tiny amount from Wallet B to the fabricated
  sanctioned recipient.
- [ ] **AGENT:** Record the transaction ID and confirmation block height.
- [ ] **AGENT:** Wait for confirmation.

Wallet A must never send to this recipient.

## 4. Update The Mainnet Policy

`demo-data/sanctioned-set.json` already contains the fabricated sanctioned
recipient and its hash.

Update `demo-data/policy.demo.json`:

- Keep `"network": "mainnet"`.
- Keep the deployed scanner measurement unless a new image was deployed.
- Set `auditStartHeight` before Wallet B's sanctioned-transfer confirmation
  block.
- Set `auditEndHeight` after that block.
- Keep:

```text
0x5789cca3a3cc4906ff4b9061d95a3218f52daabf758bdb5b683f62b6b3c431a9
```

in `sanctionedAddressHashes`.

Create two unexpired deposit-intent fixtures:

```text
demo-data/deposit-intent-clean.json
demo-data/deposit-intent-dirty.json
```

Use the current schema:

```json
{
  "exchangeName": "demo-exchange",
  "exchangeDepositAddress": "<public-demo-deposit-address>",
  "depositAmountZat": "<string-in-zatoshis>",
  "nonce": "<unique-value>",
  "expiryUnix": 4102444800
}
```

Use a unique nonce for each intent.

## 5. Run The Real PASS And FAIL Screens

### 5.1 Start the local UI

```bash
pnpm --filter clean-wallet-web dev --hostname 127.0.0.1
```

Open:

```text
http://127.0.0.1:3000/prover
```

Expand:

```text
Advanced live mode: call the deployed Phala scanner
```

### 5.2 Wallet A: real mainnet PASS

Use:

```text
demo-data/ufvk-clean.txt
demo-data/policy.demo.json
demo-data/deposit-intent-clean.json
```

Expected attested artifact:

```text
result: PASS
sanctionedHitCount: 0
```

Save the returned public bundle as:

```text
demo-data/bundle-clean.json
```

### 5.3 Wallet B: real mainnet FAIL

Use:

```text
demo-data/ufvk-dirty.txt
demo-data/policy.demo.json
demo-data/deposit-intent-dirty.json
```

Expected attested artifact:

```text
result: FAIL
sanctionedHitCount: >= 1
```

Save the returned public bundle as:

```text
demo-data/bundle-dirty.json
```

## 6. Verify Both Artifacts As The Exchange

Open:

```text
http://127.0.0.1:3000/verifier
```

Expand:

```text
Advanced live mode: verify a real Phala artifact
```

For each Wallet A and Wallet B bundle:

1. Paste the bundle.
2. Paste the same policy.
3. Paste the matching deposit intent.
4. Click `Verify real artifact`.
5. Confirm all three integrity checks pass:
   - TDX quote authenticity.
   - Quote binds artifact.
   - Artifact binds policy, audit range, and deposit intent.

Expected exchange decisions:

| Wallet | Integrity checks | Decision |
|---|---|---|
| Wallet A | All pass | Deposit approved: `PASS` |
| Wallet B | All pass | Deposit rejected: `FAIL` |

Wallet B returning `FAIL` is not a verifier failure. It proves the trusted
scanner found the fabricated sanctioned recipient.

## 7. Finish The Session

Stop the local web server with `Ctrl-C`.

Stop the paid CVM:

```bash
phala cvms stop clean-wallet-scanner-mvp3
```

Confirm:

```bash
phala cvms get clean-wallet-scanner-mvp3 --json | jq '{name,status,resource}'
```

Expected:

```text
status: stopped
```

Quit Docker Desktop if it was started and no other work needs it:

```bash
osascript -e 'tell application "Docker" to quit'
```

## 8. Definition Of Done

- [ ] Wallet A and Wallet B use real mainnet `uview...` UFVKs.
- [ ] Wallet B has one confirmed tiny outgoing transfer to the fabricated
  sanctioned recipient.
- [ ] Policy audit range includes that confirmation block.
- [ ] Live Wallet A `/screen` returns attested `PASS`.
- [ ] Live Wallet B `/screen` returns attested `FAIL`.
- [ ] Exchange verifier accepts all integrity checks for both artifacts.
- [ ] Public metadata and screenshots are documented.
- [ ] Paid CVM is stopped after the session.
