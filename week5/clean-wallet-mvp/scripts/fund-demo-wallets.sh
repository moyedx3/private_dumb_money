#!/usr/bin/env bash
set -euo pipefail

# Provisions two Zcash testnet wallets, funds them from the testnet faucet,
# and sends a tx from wallet B to one of the sanctioned demo addresses.
#
# Requires:
#   - zcashd-wallet-tool or zcashd built with testnet support
#   - Network access to a testnet faucet (https://faucet.zecpages.com or similar)
#   - jq
#
# Outputs (filled in by manual run during Task 15):
#   demo-data/ufvk-clean.txt       — UFVK for wallet A (no outgoing sanctioned recipients)
#   demo-data/ufvk-dirty.txt       — UFVK for wallet B (sent to sanctioned-set.json[0])
#   demo-data/wallet-meta.json     — { walletA: {firstReceiveBlock,...}, walletB: {...}, sanctionedTxBlock }
#   demo-data/sanctioned-set.json  — populated with actual demo sanctioned address + hash
#   demo-data/policy.demo.json     — updated with auditStartHeight/auditEndHeight bracketing the sanctioned tx

DEMO="$(dirname "$0")/../demo-data"
mkdir -p "$DEMO"

echo "Step 1: Generate UFVKs for wallet A (clean) and wallet B (will send to sanctioned)…"
# Option A (recommended): use zcashd-wallet-tool on a testnet-synced node:
#   z_getnewaccount    -> account
#   z_getaddressforaccount <account> -> unified address (UA)
#   z_exportviewingkey <UA> -> UFVK
#
# Option B: use the `zcash_keys` Rust crate via a small helper binary to derive
# UFVKs from a fresh seed. This avoids needing a running zcashd.
#
# Whichever path you take, write the UFVK strings to ufvk-clean.txt and ufvk-dirty.txt.

echo "Step 2: Fund both wallets from a testnet faucet…"
# https://faucet.zecpages.com (or any active testnet faucet — verify currency)
# Each wallet needs at least one shielded incoming transaction.
# Wait for confirmation (3 blocks typical on testnet).

echo "Step 3: From wallet B, send a small shielded tx to the demo sanctioned address."
# The sanctioned address is initially a freshly-generated testnet UA we control.
# After this send, wallet B's UFVK can derive that as an outgoing recipient.
#   zcash-cli z_sendmany <walletB-ua> '[{"address":"<sanctioned-ua>","amount":0.0001}]'
# Record the block height the tx confirms in.

echo "Step 4: Compute sanctioned address hash (matches what the scanner will compute)."
# Hash is sha256 of the address-as-bytes; the scanner uses the same encoding.
# SANCTIONED_HASH=$(printf %s "$SANCTIONED_ADDR" | sha256sum | awk '{print "0x"$1}')

echo "Step 5: Populate demo-data files."
# echo "$WALLET_A_UFVK" > "$DEMO/ufvk-clean.txt"
# echo "$WALLET_B_UFVK" > "$DEMO/ufvk-dirty.txt"
# jq -n --arg addr "$SANCTIONED_ADDR" --arg hash "$SANCTIONED_HASH" \
#   '{description:"Curated demo sanctioned ZEC address set. NOT a real OFAC list.",version:1,
#     entries:[{label:"Demo Sanctioned Address (testnet)",address:$addr,hash:$hash}]}' \
#   > "$DEMO/sanctioned-set.json"
# Then edit policy.demo.json: set sanctionedAddressHashes to [$SANCTIONED_HASH],
# and set auditStartHeight/auditEndHeight to bracket the wallet B sanctioned tx.

echo "Done. Verify by inspecting $DEMO/."
echo "NOTE: This script is currently a RUNBOOK (manual Task 15 step). Inline TODOs document the exact commands."
