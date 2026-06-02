#!/usr/bin/env bash
set -euo pipefail

# Push the scanner image to GHCR, then deploy via Phala Cloud CLI.
#
# Requires:
#   - docker logged in to ghcr.io (gh auth token-based via `docker login ghcr.io`)
#   - phala CLI installed and authenticated (`phala login`)
#   - TAG env (defaults to git short SHA)
#   - APP_NAME env (defaults to clean-wallet-scanner)
#   - INSTANCE_TYPE env (defaults to tdx.medium — 2 vCPU / 4 GB; see `phala instance-types`)
#
# NOTE: Phala dstack pulls the image anonymously — the Phala CLI exposes no
# private-registry credentials. The GHCR package MUST be public or the CVM will
# fail to pull. GitHub has no REST API to flip package visibility, so it is a
# one-time manual step in the package settings (URL printed after the push).

cd "$(dirname "$0")/.."  # cd into week5/clean-wallet-mvp

TAG="${TAG:-$(git rev-parse --short HEAD)}"
APP_NAME="${APP_NAME:-clean-wallet-scanner}"
INSTANCE_TYPE="${INSTANCE_TYPE:-tdx.medium}"

# Derive the GHCR repo from `git remote get-url origin`:
REMOTE="$(git remote get-url origin)"
ORG_REPO="$(echo "$REMOTE" | sed -E 's#.*github\.com[:/]##; s#\.git$##')"
OWNER="${ORG_REPO%%/*}"
REPO="${ORG_REPO##*/}"
IMAGE="ghcr.io/${ORG_REPO}/clean-wallet-scanner:${TAG}"

echo "==> Building image ${IMAGE}…"
docker build -f apps/scanner/Dockerfile -t "${IMAGE}" .

echo "==> Pushing to GHCR…"
docker push "${IMAGE}"

echo "==> Ensure the GHCR package is PUBLIC (Phala pulls anonymously):"
echo "    https://github.com/users/${OWNER}/packages/container/${REPO}%2Fclean-wallet-scanner/settings"
echo "    (Danger Zone → Change visibility → Public)"

echo "==> Templating docker-compose.yml…"
COMPOSE_FILE="$(mktemp -t compose.XXXXXX.yml)"
trap 'rm -f "${COMPOSE_FILE}"' EXIT
IMAGE="${IMAGE}" envsubst < apps/scanner/docker-compose.yml > "${COMPOSE_FILE}"

echo "==> Deploying ${APP_NAME} to Phala Cloud (${INSTANCE_TYPE})…"
# `phala cvms create --vcpu/--memory` is deprecated; `phala deploy` selects
# resources by instance type and waits for the CVM to reach `running`.
phala deploy \
  --name "${APP_NAME}" \
  --compose "${COMPOSE_FILE}" \
  --instance-type "${INSTANCE_TYPE}" \
  --wait

echo "==> Done. Useful follow-ups:"
echo "  phala cvms list"
echo "  phala cvms attestation --cvm-id ${APP_NAME}"
echo "  phala cvms get --cvm-id ${APP_NAME}"
echo "  phala cvms logs --cvm-id ${APP_NAME}"
