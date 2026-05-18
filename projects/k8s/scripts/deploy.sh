#!/usr/bin/env bash
# Deploys udex to the local k3d cluster via Helm and waits for rollout.
#
# Reads credentials from the workspace .env file and TLS material from the
# generated test certs. Run cluster-create.sh and image-load.sh first.
#
# Usage: bash projects/k8s/scripts/deploy.sh
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

CHART_DIR="${WORKSPACE_DIR}/projects/k8s/helm/udex"
ENV_FILE="${WORKSPACE_DIR}/.env"
CERTS_DIR="${WORKSPACE_DIR}/projects/rust/server/tests/certs"
RELEASE_NAME="udex"
NAMESPACE="default"

for cmd in helm kubectl; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "ERROR: $cmd is required but not installed." >&2
    exit 1
  fi
done

if [[ ! -f "${ENV_FILE}" ]]; then
  echo "ERROR: ${ENV_FILE} not found. Run scripts/gen-env.sh first." >&2
  exit 1
fi

for cert_file in "${CERTS_DIR}/server.crt" "${CERTS_DIR}/server.key"; do
  if [[ ! -f "${cert_file}" ]]; then
    echo "ERROR: ${cert_file} not found. Run scripts/gen-keys-and-certs.sh first." >&2
    exit 1
  fi
done

# Load .env — export only the variables this script needs.
set -o allexport
# shellcheck source=/dev/null
source "${ENV_FILE}"
set +o allexport

if [[ -z "${POSTGRES_PASSWORD_SECRET:-}" ]]; then
  echo "ERROR: POSTGRES_PASSWORD_SECRET not set in ${ENV_FILE}." >&2
  exit 1
fi

# Build the k8s DATABASE_URL: postgres is on the host docker network,
# reachable from k3d pods via host.k3d.internal.
K8S_DATABASE_URL="postgres://postgres:${POSTGRES_PASSWORD_SECRET}@host.k3d.internal:5432/postgres"

TLS_CRT="$(cat "${CERTS_DIR}/server.crt")"
TLS_KEY="$(cat "${CERTS_DIR}/server.key")"

echo "Deploying release '${RELEASE_NAME}' to namespace '${NAMESPACE}'..."
helm upgrade --install "${RELEASE_NAME}" "${CHART_DIR}" \
  --namespace "${NAMESPACE}" \
  --set-string "secrets.databaseUrl=${K8S_DATABASE_URL}" \
  --set-string "secrets.tlsCrt=${TLS_CRT}" \
  --set-string "secrets.tlsKey=${TLS_KEY}"

echo "Waiting for rollout..."
kubectl rollout status deployment/"${RELEASE_NAME}-udex" \
  --namespace "${NAMESPACE}" \
  --timeout=120s

echo "Deploy complete. Server reachable at https://localhost:8443"
