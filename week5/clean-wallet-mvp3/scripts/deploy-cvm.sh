#!/usr/bin/env bash
set -euo pipefail

# Push the scanner image to a registry, then deploy via Phala Cloud CLI.
#
# Requires:
#   - docker logged in to the target registry
#   - phala CLI installed and authenticated (`phala login`)
#   - TAG env (defaults to git short SHA)
#   - APP_NAME env (defaults to clean-wallet-scanner-mvp3)
#   - CVM_ID env (optional; updates an existing CVM instead of creating one)
#   - IMAGE env (optional; defaults to this repo's GHCR package)
#   - PLATFORM env (optional; defaults to linux/amd64 for Phala TDX CVMs)

cd "$(dirname "$0")/.."  # cd into week5/clean-wallet-mvp3

if ! command -v docker >/dev/null && [[ -x /Applications/Docker.app/Contents/Resources/bin/docker ]]; then
  export PATH="/Applications/Docker.app/Contents/Resources/bin:${PATH}"
fi

TAG="${TAG:-$(git rev-parse --short HEAD)}"
APP_NAME="${APP_NAME:-clean-wallet-scanner-mvp3}"
INSTANCE_TYPE="${INSTANCE_TYPE:-tdx.small}"
PLATFORM="${PLATFORM:-linux/amd64}"

# Derive the GHCR repo from `git remote get-url origin`:
REMOTE="$(git remote get-url origin)"
ORG_REPO="$(echo "$REMOTE" | sed -E 's#.*github\.com[:/]([^/]+/[^/]+)$#\1#; s#\.git$##')"
IMAGE="${IMAGE:-ghcr.io/${ORG_REPO}/clean-wallet-scanner:${TAG}}"

echo "==> Building image ${IMAGE}..."
docker build --platform "${PLATFORM}" -f apps/scanner/Dockerfile -t "${IMAGE}" .

echo "==> Pushing image..."
docker push "${IMAGE}"

echo "==> Templating docker-compose.yml..."
COMPOSE_FILE="$(mktemp -t compose.XXXXXX.yml)"
trap 'rm -f "${COMPOSE_FILE}"' EXIT
IMAGE="${IMAGE}" envsubst < apps/scanner/docker-compose.yml > "${COMPOSE_FILE}"

echo "==> Deploying ${APP_NAME} to Phala Cloud..."
DEPLOY_ARGS=(
  --name "${APP_NAME}" \
  --compose "${COMPOSE_FILE}" \
  --instance-type "${INSTANCE_TYPE}" \
  --wait
)
if [[ -n "${CVM_ID:-}" ]]; then
  DEPLOY_ARGS+=(--cvm-id "${CVM_ID}")
fi
phala deploy "${DEPLOY_ARGS[@]}"

echo "==> Done. Useful follow-ups:"
echo "  phala apps"
echo "  phala cvms get ${APP_NAME}"
echo "  phala cvms logs ${APP_NAME}"
