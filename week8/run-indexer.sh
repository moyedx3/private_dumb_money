#!/usr/bin/env bash
# Launch the drop-indexer as a plain web server (NO TEE): dev seed instead of dstack KMS,
# real A1 mainnet scanner ON. Run on the EC2 box.
set -euo pipefail

: "${SEED_HEX:?set SEED_HEX (32-byte hex; also used by seed.mjs)}"

export A2_DEV_PROVISIONING_SEED_HEX="$SEED_HEX"   # dev seed; refused automatically if a dstack socket exists
export A1_SCAN_ENABLE=1                            # real mainnet payment scanner
export LIGHTWALLETD_URL="${LIGHTWALLETD_URL:-https://zec.rocks:443}"
export BUCKET_DIR="${BUCKET_DIR:-$HOME/drop-bucket}"
export PORT="${PORT:-8080}"
export RUST_LOG="${RUST_LOG:-info}"

BIN="${INDEXER_BIN:-../week7/drop/indexer/target/release/drop-indexer}"
echo "starting $BIN on :$PORT (bucket=$BUCKET_DIR, lightwalletd=$LIGHTWALLETD_URL)"
exec "$BIN"
