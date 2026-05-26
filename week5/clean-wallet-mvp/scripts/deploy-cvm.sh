#!/usr/bin/env bash
set -euo pipefail

# Push the scanner image to GHCR, then deploy via Phala Cloud CLI.
#
# Requires:
#   - docker logged in to ghcr.io (gh auth token-based via `docker login ghcr.io`)
#   - phala CLI installed and authenticated (`phala auth login`)
#   - TAG env (defaults to git short SHA)
#   - APP_NAME env (defaults to clean-wallet-scanner)

cd "$(dirname "$0")/.."  # cd into week5/clean-wallet-mvp

TAG="${TAG:-$(git rev-parse --short HEAD)}"
APP_NAME="${APP_NAME:-clean-wallet-scanner}"

# Derive the GHCR repo from `git remote get-url origin`:
REMOTE="$(git remote get-url origin)"
ORG_REPO="$(echo "$REMOTE" | sed -E 's#.*github\.com[:/]([^/]+/[^/]+?)(\.git)?$#\1#')"
IMAGE="ghcr.io/${ORG_REPO}/clean-wallet-scanner:${TAG}"

echo "==> Building image ${IMAGE}…"
docker build -f apps/scanner/Dockerfile -t "${IMAGE}" .

echo "==> Pushing to GHCR…"
docker push "${IMAGE}"

echo "==> Templating docker-compose.yml…"
COMPOSE_FILE="$(mktemp -t compose.XXXXXX.yml)"
trap 'rm -f "${COMPOSE_FILE}"' EXIT
IMAGE="${IMAGE}" envsubst < apps/scanner/docker-compose.yml > "${COMPOSE_FILE}"

echo "==> Deploying ${APP_NAME} to Phala Cloud…"
phala cvms create \
  --name "${APP_NAME}" \
  --compose "${COMPOSE_FILE}" \
  --vcpu 2 \
  --memory 4096

echo "==> Done. Useful follow-ups:"
echo "  phala cvms attestation ${APP_NAME}"
echo "  phala cvms env get ${APP_NAME}"
echo "  phala cvms logs ${APP_NAME}"
