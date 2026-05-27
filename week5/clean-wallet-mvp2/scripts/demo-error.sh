#!/bin/sh
set -eu
python3 -m clean_wallet.cli build-blacklist \
  --commitments fixtures/blacklist_commitments.txt \
  --output artifacts/blacklist.json \
  --network regtest \
  --pool orchard \
  --issuer demo-issuer \
  --version v0 >/dev/null
set +e
python3 -m clean_wallet.cli request-proof \
  --fixture fixtures/error_scan.json \
  --blacklist artifacts/blacklist.json \
  --output artifacts/error-report.json \
  --viewing-scope-id alice-orchard-account-0 \
  --network regtest \
  --pool orchard \
  --start-block 100 \
  --end-block 110
status=$?
set -e
if [ "$status" -ne 2 ]; then
  echo "expected scanner error exit 2, got $status" >&2
  exit 1
fi
python3 -m clean_wallet.cli verify-report \
  --report artifacts/error-report.json \
  --blacklist artifacts/blacklist.json
