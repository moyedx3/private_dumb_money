#!/bin/sh
set -eu
python3 -m clean_wallet.cli build-blacklist \
  --commitments fixtures/blacklist_commitments.txt \
  --output artifacts/blacklist.json \
  --network regtest \
  --pool orchard \
  --issuer demo-issuer \
  --version v0 >/dev/null
python3 -m clean_wallet.cli request-proof \
  --fixture fixtures/pass_scan.json \
  --blacklist artifacts/blacklist.json \
  --output artifacts/pass-report.json \
  --viewing-scope-id alice-orchard-account-0 \
  --network regtest \
  --pool orchard \
  --start-block 100 \
  --end-block 110
python3 -m clean_wallet.cli verify-report \
  --report artifacts/pass-report.json \
  --blacklist artifacts/blacklist.json
