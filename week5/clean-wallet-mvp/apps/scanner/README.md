# Scanner

Rust binary that runs inside a Phala Cloud CVM. Implements the screening API
(`/health`, `/attestation`, `/screen`) backed by `zcash_client_backend` (lightwalletd)
and `dstack-sdk` (TEE attestation).

## Build the Docker image

From the `week5/clean-wallet-mvp/` root:

```bash
docker build -f apps/scanner/Dockerfile -t clean-wallet-scanner:dev .
```

## Deploy to Phala Cloud

```bash
./scripts/deploy-cvm.sh
```

Requires `docker login ghcr.io` and `phala auth login` first.

Environment variables (set in `docker-compose.yml`):

| Variable | Default | Notes |
|---|---|---|
| `LIGHTWALLETD_PRIMARY` | `https://testnet.zec.rocks:443` | TLS gRPC endpoint |
| `LIGHTWALLETD_BACKUP` | (empty) | Optional second endpoint for failover |
| `NETWORK` | `testnet` | Hard-checked against policy.network |
| `MAX_RANGE_BLOCKS` | `100000` | (Not currently consumed at runtime; reserved.) |
| `DSTACK_SOCKET` | `/var/run/dstack.sock` | Phala dstack unix socket |
| `RUST_LOG` | `info,clean_wallet_scanner=debug` | Tracing filter |
