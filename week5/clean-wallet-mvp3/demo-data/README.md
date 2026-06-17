# Demo data

Reproducible inputs for the clean-wallet MVP demo. **Mainnet.** Demo funds are
intentionally tiny (sub-cent). The "sanctioned" address is fabricated (generated
by `apps/scanner/src/bin/gen-taddr.rs`) — it does not correspond to any real
OFAC listing.

## Files

- `ufvk-clean.txt` — Wallet A UFVK. Must be replaced with a real mainnet
  `uview...` value before a live run.
- `ufvk-dirty.txt` — Wallet B UFVK. Must be replaced with a real mainnet
  `uview...` value after Wallet B is provisioned.
- `sanctioned-set.json` — Curated demo sanctioned address set (NOT real OFAC data).
- `policy.demo.json` — Policy bound to the demo block range + sanctioned set + Phala code measurement.
- `wallet-meta.json` — Public block heights and transaction metadata created
  after Wallet A and Wallet B are provisioned.

## Provisioning

Follow `docs/wallet-operator-procedure.md` and `docs/NEXT-LIVE-RUN.md`.

The previous testnet-era `scripts/fund-demo-wallets.sh` runbook was removed.
Do not recreate it for mainnet funding. Mainnet wallet creation, funding, and
transaction broadcast require explicit human operator approval.

## Updating expectedScannerCodeMeasurement

After deploying a new Phala Cloud image, record the code measurement of the
deployed image and update `demo-data/policy.demo.json`. If no new image was
deployed, keep the current measurement.
