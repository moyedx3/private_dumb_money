# Scanner

Rust binary that runs inside a Phala Cloud CVM. Implements the screening API
(`/health`, `/attestation`, `/screen`) backed by `zcash_client_backend` (lightwalletd)
and `dstack-sdk` (TEE attestation).

## Build the Docker image

From the `week5/clean-wallet-mvp3/` root:

```bash
docker build --platform linux/amd64 -f apps/scanner/Dockerfile -t clean-wallet-scanner:dev .
```

## Deploy to Phala Cloud

```bash
./scripts/deploy-cvm.sh
```

Requires `docker login ghcr.io` and `phala login` first.

Environment variables (set in `docker-compose.yml`):

| Variable | Default | Notes |
|---|---|---|
| `LIGHTWALLETD_PRIMARY` | `https://zec.rocks:443` | TLS gRPC endpoint |
| `LIGHTWALLETD_BACKUP` | (empty) | Optional second endpoint for failover |
| `NETWORK` | `mainnet` | Hard-checked against policy.network |
| `MAX_RANGE_BLOCKS` | `100000` | (Not currently consumed at runtime; reserved.) |
| `DSTACK_SOCKET` | `/var/run/dstack.sock` | Phala dstack unix socket |
| `RUST_LOG` | `info,clean_wallet_scanner=debug` | Tracing filter |

## Regtest integration suite

Prerequisites: docker, jq, zcash-cli (host), and ~5 GB free disk.

```bash
./tests/regtest_setup.sh                              # provisions wallets, mines blocks, crafts txs
cargo test -p clean-wallet-scanner --test regtest_scan -- --ignored --nocapture
```

The setup script is idempotent: re-running drops the existing `.regtest-state/` and starts fresh.

To exercise the lightwalletd-disconnect test, stop the container between tests:
```bash
docker compose -f apps/scanner/docker-compose.test.yml stop lightwalletd
cargo test -p clean-wallet-scanner --test regtest_scan fail_closed_on_lightwalletd_disconnect -- --ignored
docker compose -f apps/scanner/docker-compose.test.yml start lightwalletd
```

Note: wallet provisioning in `regtest_setup.sh` is currently marked TODO/BLOCKED pending Zebra wallet RPC support (see comments in the script). The docker-compose infrastructure (zebrad in Regtest mode + lightwalletd) stands up correctly; the four integration tests are `#[ignore]`'d until the state files are populated.
