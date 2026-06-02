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

- `ufvk-clean.txt` — Wallet A UFVK (mainnet). No outgoing tx to a sanctioned address → PASS.
- `ufvk-dirty.txt` — Wallet B UFVK (mainnet). Restore its mnemonic into Zashi, fund sub-cent, send one shielded tx to `sanctioned-set.json[0].address` → FAIL.

Both wallets are minted by `apps/scanner/src/bin/gen-dirty-wallet.rs`, which prints a
BIP39 mnemonic + the matching mainnet UFVK derived the same way Zashi does (BIP39
seed, empty passphrase, account 0) — so you can restore the mnemonic into a fresh
Zashi wallet and the scanner will recognise its outgoing payments. **Throwaway
only:** the default entropy is public; never hold more than sub-cent funds.
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
