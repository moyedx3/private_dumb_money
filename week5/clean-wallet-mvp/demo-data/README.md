# Demo data

Reproducible inputs for the clean-wallet MVP demo. **Mainnet.** Demo funds are
intentionally tiny (sub-cent). The "sanctioned" address is fabricated (a fresh
mainnet **shielded unified address** minted by `apps/scanner/src/bin/gen-watchlist.rs`)
— it does not correspond to any real OFAC listing.

> **Why shielded, not transparent?** The scanner only recovers Sapling/Orchard
> outputs via OVK trial decryption (`scan.rs::extract_outgoing_recipients`). A
> transparent `t1…` recipient is invisible to it, so a payment there would
> wrongly return PASS. The earlier `gen-taddr.rs` (transparent) address was the
> wrong tool for this; `gen-watchlist.rs` mints a shielded unified address and
> prints the exact `sha256` the scanner computes for the matching pool. Zashi is
> Orchard-first, so the Orchard-receiver hash is the one that triggers the hit.

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
