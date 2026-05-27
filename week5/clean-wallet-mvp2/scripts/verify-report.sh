#!/bin/sh
set -eu
report=${1:?usage: scripts/verify-report.sh artifacts/report.json}
blacklist=${2:-artifacts/blacklist.json}
python3 -m clean_wallet.cli verify-report --report "$report" --blacklist "$blacklist"
