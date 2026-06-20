# drop-indexer — deploy to Phala (A2 Task 8)

> The code is complete + tested (8 unit + 4 live-against-simulator). **This is the deploy step**,
> which needs Docker + the `gh`/`phala` CLIs. (The dev env where this was built had no Docker,
> so run these from a machine with Docker Desktop WSL integration on.) Same flow as
> [`../spike3/RUNBOOK.md`](../spike3/RUNBOOK.md), which already worked on real Phala.

## ✅ DEPLOYED & VERIFIED on real Intel TDX (2026-06-20)

Built (`rust:1.90`, dryoc 0.8) → pushed to GHCR (public) → deployed a `tdx.small` CVM. Verified end to end:

- **Container up:** logs show `drop-indexer listening on :8080` → `main.rs` derived its provisioning seed via dstack `get_key` **on real Phala** (same as the simulator) and serves.
- **Genuine TDX attestation:** `Status Online`, 3-cert chain, `Mrtd f06dfda6…826980` (shared dstack base), **`Rtmr3 4c3fe02fdc6bcdad…2ace8cb7`** (this image's app measurement — what Lane C compares against).
- **Live routes** (`https://88feb6bc3273df0186f16cfbccd3d3be661b776c-8080.dstack-pha-prod5.phala.network`): `/health`→`ok`, `/catalog`→`[]`, `/attest`→ real 10020-hex quote + provisioning pubkey.
- **Trust binding holds on hardware:** the quote's `report_data[0:32]` **==** `sha256(provisioning_pubkey)` (verified `f6a1e8a5…`). This is exactly the check Lane C runs before sealing `K_drop`.

App ID `app_88feb6bc3273df0186f16cfbccd3d3be661b776c` · image `ghcr.io/moyedx3/private_dumb_money/drop-indexer:5fdbf9a`. **Lane C:** compare the running `Rtmr3` against a reproducible rebuild of this commit before trusting a server.

---

## 1. Build + push the image — GHCR package MUST be public (Phala pulls anonymously)

```bash
cd week7/drop/indexer
OWNER_REPO=moyedx3/private_dumb_money            # from: git remote get-url origin
IMAGE=ghcr.io/$OWNER_REPO/drop-indexer:$(git rev-parse --short HEAD)

docker build -t "$IMAGE" .
docker login ghcr.io                            # gh PAT with write:packages
docker push "$IMAGE"
```
Then make the package **public** (one-time, manual web step):
GitHub → your packages → `drop-indexer` → Package settings → Danger Zone → Change visibility → **Public**.

## 2. Deploy the CVM (real Intel TDX — billable)

```bash
IMAGE="$IMAGE" envsubst < docker-compose.yml > /tmp/drop-compose.yml
phala deploy --name drop-indexer --compose /tmp/drop-compose.yml --instance-type tdx.small --wait
```

## 3. Verify the real attestation (Check 1 passes on real Phala, unlike the simulator)

```bash
phala cvms attestation --cvm-id drop-indexer    # capture the Mrtd
curl https://<cvm-url>/attest                   # → {quote_hex, provisioning_pubkey_hex}
```
Verify the quote at <https://proof.t16z.com>. The `/attest` `quote_hex` should carry
`report_data = sha256(provisioning_pubkey)` — exactly what Lane C checks before sealing `K_drop`.

## 4. Publish the measurement for creators (Lane C)

Put the `Mrtd` from step 3 in the public repo README — that's the reproducible-build hash Lane C
compares against. For true reproducibility, pin the Dockerfile base by `@sha256` digest captured
from your first build, rebuild, and confirm the **same** `Mrtd`.

## 5. Smoke-test the live routes

```bash
curl https://<cvm-url>/health        # ok
curl https://<cvm-url>/catalog       # []  (empty until a creator provisions)
# Lane C then verifies /attest and POSTs a sealed payload to /provision.
```

## 6. Stop billing when done

```bash
phala cvms stop drop-indexer         # or: phala cvms delete drop-indexer
```

---

**Reminder (C4):** rebuilding the image changes the measurement → changes the provisioning
keypair → creators who provisioned to the old build must re-`POST /provision`. Re-provisioning
is idempotent per `drop_id`.
