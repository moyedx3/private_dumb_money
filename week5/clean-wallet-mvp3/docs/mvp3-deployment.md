# MVP3 Phala Deployment

Live scanner deployment created on 2026-06-01 KST.

Current state: stopped on 2026-06-01 KST after validation to avoid unnecessary
hourly runtime charges.

## Scanner

- CVM name: `clean-wallet-scanner-mvp3`
- CVM ID: `c9ee88a6-323b-4847-9285-8e3b331dce5a`
- App ID: `af483db7eae7054b05f9dd526f26f65b0e738448`
- Instance type: `tdx.small`
- Dashboard: `https://cloud.phala.com/dashboard/cvms/c9ee88a6-323b-4847-9285-8e3b331dce5a`
- Endpoint: `https://af483db7eae7054b05f9dd526f26f65b0e738448-8080.dstack-pha-prod5.phala.network`
- Image: `ghcr.io/soarewee/clean-wallet-scanner:mvp3-dev`

## Attestation

- Code measurement: `0xf06dfda6dce1cf904d4e2bab1dc370634cf95cefa2ceb2de2eee127c9382698090d7a4a13e14c536ec6c9c3c8fa87077`
- Phala attestation API: `https://cloud-api.phala.com/api/v1/attestations/verify`
- Live `/attestation` quote verified successfully against the Phala API.

## Update The Existing CVM

Use `CVM_ID` for subsequent image updates so the deployment script updates this
paid instance instead of creating another one:

```bash
CVM_ID=c9ee88a6-323b-4847-9285-8e3b331dce5a \
IMAGE=ghcr.io/soarewee/clean-wallet-scanner:mvp3-dev \
./scripts/deploy-cvm.sh
```

Start the existing CVM before the next live run:

```bash
phala cvms start clean-wallet-scanner-mvp3
```

## Demo Inputs

The inherited `demo-data/ufvk-*.txt` files still contain early testnet keys.
They are not valid mainnet end-to-end fixtures. Replace them with provisioned
mainnet demo wallets before demonstrating `/screen`.

## Local Web App

```bash
cd apps/web
pnpm dev
```

Open `http://127.0.0.1:3000`. The ignored `.env.local` file points the prover
page at the live scanner endpoint. `.env.example` records the same value for
other environments.
