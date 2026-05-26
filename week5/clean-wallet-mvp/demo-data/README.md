# Demo data

Reproducible inputs for the clean-wallet MVP demo. **Testnet only.** None of these
addresses or UFVKs correspond to mainnet funds.

## Files

- `ufvk-clean.txt` — Wallet A UFVK. No outgoing tx to a sanctioned address. (Populated during Task 15.)
- `ufvk-dirty.txt` — Wallet B UFVK. Sent one shielded tx to the address in `sanctioned-set.json[0]`. (Populated during Task 15.)
- `sanctioned-set.json` — Curated demo sanctioned address set (NOT real OFAC data).
- `policy.demo.json` — Policy bound to the demo block range + sanctioned set + Phala code measurement.
- `wallet-meta.json` — Block heights and other provisioning metadata. (Populated during Task 15.)

## Regenerating

```bash
./scripts/fund-demo-wallets.sh
# Then update demo-data/policy.demo.json auditStart/EndHeight to bracket
# the sanctioned tx block.
```

The script is currently a runbook — the inline TODOs document the exact commands the Task 15 operator should run.

## Updating expectedScannerCodeMeasurement

After deploying to Phala Cloud (Task 15), record the code measurement of the
deployed image and update `demo-data/policy.demo.json`. Commit the change.
