# Wallet Operator Procedure

Use this procedure to provision the two Zcash mainnet wallets needed for the
MVP3 live PASS / FAIL run. It is written for a human wallet operator and an
agent working together without exposing wallet secrets.

## Scope

The selected operational path is:

- A human operator creates and controls Wallet A and Wallet B in a
  mainnet-capable Zcash wallet.
- The operator handles seed backup, funding, and transaction broadcast.
- The agent receives only public or read-only data: UFVKs, receiving addresses,
  transaction IDs, block heights, and screenshots if needed.

Do not use repository helper binaries as a replacement for wallet custody or
safe broadcast. The helpers are useful for controlled address derivation and
fixtures, but the live demo requires an operator-controlled wallet workflow.

## Secret Handling Rules

Never put these values in the repository, chat, screenshots, shell history, or
logs:

- Seed phrases or mnemonic phrases.
- Spending keys.
- Wallet backup files.
- Device recovery material.
- Exchange or funding-source credentials.

The agent may work with these values:

- Read-only mainnet UFVKs beginning with `uview`.
- Public receiving addresses.
- Public transaction IDs and confirmation block heights.
- The fabricated demo sanctioned recipient.
- Public policy and deposit-intent JSON.

## Wallet Setup

1. Create Wallet A with a fresh secret.
2. Create Wallet B with a different fresh secret.
3. Back up both secrets outside this repository before funding either wallet.
4. Export or derive a read-only mainnet UFVK for each wallet.
5. Record one public receiving address for each wallet.

Acceptance checks before funding:

```bash
head -c 10 demo-data/ufvk-clean.txt && printf '\n'
head -c 10 demo-data/ufvk-dirty.txt && printf '\n'
```

Both files must begin with:

```text
uview
```

They must not begin with:

```text
uviewtest
```

## Public Metadata

After the operator provides public values, create `demo-data/wallet-meta.json`
with this shape:

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

Do not add private notes or wallet identifiers that could reveal custody
details. This file is intentionally public demo metadata.

## Funding Checkpoint

Before any funding transaction is broadcast, get explicit operator approval for:

- Source wallet or account.
- Wallet A receiving address and exact amount.
- Wallet B receiving address and exact amount.
- Fee expectations.

After funding confirms, record transaction IDs and confirmation block heights in
`demo-data/wallet-meta.json`.

## Wallet B FAIL Transaction Checkpoint

Wallet B must send one tiny mainnet amount to the fabricated demo sanctioned
recipient:

```text
t1Ss8dERcHbR9tQx6rN3tjhzK1vvAz4QgZu
```

Before broadcast, get explicit operator approval for:

- Source: Wallet B.
- Destination: the exact address above.
- Exact amount.
- Fee expectations.

After confirmation, record the transaction ID and confirmation block height in
`demo-data/wallet-meta.json`.

Wallet A must never send to this recipient.

## Policy And Live Run Handoff

Once Wallet B's fabricated sanctioned transfer is confirmed:

1. Update `demo-data/policy.demo.json` so `auditStartHeight` is before the
   Wallet B transfer block.
2. Set `auditEndHeight` after the Wallet B transfer block.
3. Keep the deployed scanner measurement unless a new CVM image was deployed.
4. Create fresh unexpired deposit intents for Wallet A and Wallet B.
5. Follow `docs/NEXT-LIVE-RUN.md` to run the live `/screen` and verifier flow.

Definition of done for wallet provisioning:

- Wallet A and Wallet B UFVK files are real mainnet `uview...` values.
- Wallet B has one confirmed transfer to the fabricated demo recipient.
- Public transaction metadata is documented.
- No secrets were introduced into the repository.
