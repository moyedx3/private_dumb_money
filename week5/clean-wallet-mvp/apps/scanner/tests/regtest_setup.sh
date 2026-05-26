#!/usr/bin/env bash
set -euo pipefail

# Brings up regtest zebrad + lightwalletd, generates two wallets, mines blocks,
# sends a tx from wallet B to a "sanctioned" address, and exports:
#   tests/.regtest-state/wallet-a.ufvk        — clean wallet
#   tests/.regtest-state/wallet-b.ufvk        — wallet with sanctioned recipient
#   tests/.regtest-state/sanctioned.json      — { recipient_hash_hex }
#   tests/.regtest-state/range.json           — { start, end }
#
# Uses `zcash-cli` over RPC against the regtest zebrad node plus lightwalletd.
#
# PREREQUISITES (host):
#   - docker (compose v2)
#   - zcash-cli (any recent version; used for z_sendmany / z_getnewaddress RPCs)
#   - jq
#   - sha256sum (coreutils)
#   - nc (netcat, for health checks)
#
# BLOCKED (2026-05-26): The Zebra regtest node does not yet expose the full set
# of z_* wallet RPCs (z_sendmany, z_getnewaddress, z_exportviewingkey) — these
# are zcashd-specific.  Zebra's wallet support is tracked in:
#   https://github.com/ZcashFoundation/zebra/issues/4727
#
# Until Zebra ships wallet RPCs, this script cannot automate the shielded tx
# flow.  Two workarounds for a future implementer:
#
#   Option A — sidecar zcashd for wallet ops only:
#     Add a `zcashd` service to docker-compose.test.yml in wallet-only mode,
#     pointing its peers at zebrad.  Use zcashd's z_* RPCs to create/fund
#     wallets, then use lightwalletd (which talks to zebrad) for scanning.
#
#   Option B — zcash-wallet-tool (https://github.com/zcash/zcash-wallet-tool):
#     Generate UFVKs offline, fund addresses by constructing raw transactions
#     with `zcash-primitives` tooling, and submit via zebrad's
#     `sendrawtransaction` RPC.
#
# The remainder of this script documents the intended flow with inline TODOs.

COMPOSE="docker compose -f apps/scanner/docker-compose.test.yml"
STATE_DIR="tests/.regtest-state"

# Wipe and recreate state directory for idempotency
rm -rf "$STATE_DIR"
mkdir -p "$STATE_DIR"

# ---------------------------------------------------------------------------
# Step 1: Bring up the regtest cluster
# ---------------------------------------------------------------------------
echo "[regtest_setup] Starting docker-compose cluster…"
$COMPOSE up -d

echo "[regtest_setup] Waiting for lightwalletd gRPC on :9067…"
for i in $(seq 1 60); do
  if nc -z localhost 9067 2>/dev/null; then
    echo "[regtest_setup] lightwalletd is ready (attempt $i)"
    break
  fi
  sleep 1
done

# ---------------------------------------------------------------------------
# Step 2: Wallet creation
# ---------------------------------------------------------------------------
# TODO(task-10-wallet-provisioning): Once a wallet RPC source is available
# (either a sidecar zcashd or zcash-wallet-tool), replace the lines below.
#
# Intended flow:
#   ADDR_A=$(zcash-cli -regtest z_getnewaddress sapling)
#   ADDR_B=$(zcash-cli -regtest z_getnewaddress sapling)
#   SANCTIONED_ADDR=$(zcash-cli -regtest z_getnewaddress sapling)
#   UFVK_A=$(zcash-cli -regtest z_exportviewingkey "$ADDR_A" "uviewtest")
#   UFVK_B=$(zcash-cli -regtest z_exportviewingkey "$ADDR_B" "uviewtest")
#   echo "$UFVK_A" > "$STATE_DIR/wallet-a.ufvk"
#   echo "$UFVK_B" > "$STATE_DIR/wallet-b.ufvk"

echo "[regtest_setup] TODO: wallet creation (BLOCKED — see comments above)"

# ---------------------------------------------------------------------------
# Step 3: Mine blocks until coinbase matures (~100 blocks in regtest)
# ---------------------------------------------------------------------------
# TODO(task-10-mining):
#   MINING_ADDR=$(zcash-cli -regtest getnewaddress)
#   for i in $(seq 1 110); do
#     zcash-cli -regtest generatetoaddress 1 "$MINING_ADDR"
#   done
#   START_HEIGHT=$(zcash-cli -regtest getblockcount)

echo "[regtest_setup] TODO: mine blocks for coinbase maturity (BLOCKED)"

# ---------------------------------------------------------------------------
# Step 4: Fund wallets with shielded sends
# ---------------------------------------------------------------------------
# TODO(task-10-funding):
#   # Send to wallet A (clean — no sanctioned recipients)
#   zcash-cli -regtest z_sendmany "$MINING_ADDR" \
#     "[{\"address\":\"$ADDR_A\",\"amount\":0.5}]" 1 0.0001 "AllowRevealedAmounts"
#   zcash-cli -regtest generatetoaddress 10 "$MINING_ADDR"
#
#   # Send to wallet B
#   zcash-cli -regtest z_sendmany "$MINING_ADDR" \
#     "[{\"address\":\"$ADDR_B\",\"amount\":0.5}]" 1 0.0001 "AllowRevealedAmounts"
#   zcash-cli -regtest generatetoaddress 10 "$MINING_ADDR"
#
#   # From wallet B, send to sanctioned address (this is what the FAIL test detects)
#   zcash-cli -regtest z_sendmany "$ADDR_B" \
#     "[{\"address\":\"$SANCTIONED_ADDR\",\"amount\":0.1}]" 1 0.0001 "AllowRevealedAmounts"
#   zcash-cli -regtest generatetoaddress 10 "$MINING_ADDR"
#   END_HEIGHT=$(zcash-cli -regtest getblockcount)

echo "[regtest_setup] TODO: fund wallets and send sanctioned tx (BLOCKED)"

# ---------------------------------------------------------------------------
# Step 5: Export state files consumed by regtest_scan.rs
# ---------------------------------------------------------------------------
# TODO(task-10-state-export):
#   # Sanctioned address hash (SHA-256 of the address string, hex)
#   SANCTIONED_HASH=$(echo -n "$SANCTIONED_ADDR" | sha256sum | awk '{print $1}')
#   echo "{\"recipient_hash_hex\":\"0x${SANCTIONED_HASH}\"}" > "$STATE_DIR/sanctioned.json"
#   echo "{\"start\":${START_HEIGHT},\"end\":${END_HEIGHT}}" > "$STATE_DIR/range.json"

echo "[regtest_setup] TODO: export sanctioned.json + range.json (BLOCKED)"

echo ""
echo "[regtest_setup] Infrastructure is running but wallet provisioning is BLOCKED."
echo "  See inline TODOs for the full flow.  Current cluster status:"
$COMPOSE ps
