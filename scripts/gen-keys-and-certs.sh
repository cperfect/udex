#!/bin/bash
# Generates all dev key and certificate material by calling the per-component scripts.
#
# Usage:
#   scripts/gen-keys-and-certs.sh           # skips if key material already exists
#   scripts/gen-keys-and-certs.sh --force   # regenerates and rotates everything
#
# Run once when first cloning the repo. Re-run with --force to rotate keys.
# The devcontainer post-create script calls this automatically.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

FORCE=false
for arg in "$@"; do
  case "$arg" in
    --force) FORCE=true ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

# Files required by the test suite (not every intermediate artifact produced by
# the sub-scripts — e.g. server.csr and ca.srl are intentionally omitted).
# Every listed file must exist for the guard to pass; a partial state (e.g.
# interrupted run or manual deletion) triggers full regeneration.
JWT_DIR="${WORKSPACE_DIR}/projects/rust/server/tests/jwt"
TLS_DIR="${WORKSPACE_DIR}/projects/rust/server/tests/certs"
EDGE_TLS_DIR="${WORKSPACE_DIR}/projects/k8s/traefik/certs"
OTLP_TLS_DIR="${WORKSPACE_DIR}/projects/observability/certs"

ALL_EXIST=true
for f in \
  "${JWT_DIR}/signing_private_key.pem" \
  "${JWT_DIR}/signing_public_key.pem" \
  "${JWT_DIR}/bad_signing_private_key.pem" \
  "${JWT_DIR}/bad_signing_public_key.pem" \
  "${TLS_DIR}/ca.key" \
  "${TLS_DIR}/ca.crt" \
  "${TLS_DIR}/server.key" \
  "${TLS_DIR}/server.crt" \
  "${EDGE_TLS_DIR}/ca.key" \
  "${EDGE_TLS_DIR}/ca.crt" \
  "${EDGE_TLS_DIR}/tls.key" \
  "${EDGE_TLS_DIR}/tls.crt" \
  "${OTLP_TLS_DIR}/ca.key" \
  "${OTLP_TLS_DIR}/ca.crt" \
  "${OTLP_TLS_DIR}/collector.key" \
  "${OTLP_TLS_DIR}/collector.crt"; do
  [[ -f "$f" ]] || { ALL_EXIST=false; break; }
done

if [[ "${ALL_EXIST}" == true && "${FORCE}" == false ]]; then
  echo "Key material already exists — skipping generation."
  echo "Run with --force to rotate keys and certificates."
  exit 0
fi

echo "==> Generating TLS certificates..."
bash "${WORKSPACE_DIR}/projects/rust/server/tests/certs/regenerate_certs.sh"

echo ""
echo "==> Generating Traefik edge TLS certificates..."
bash "${WORKSPACE_DIR}/projects/k8s/traefik/certs/regenerate_certs.sh"

echo ""
echo "==> Generating JWT signing key pairs..."
bash "${WORKSPACE_DIR}/projects/rust/server/tests/jwt/regenerate_jwt_signing_key_pair.sh"

echo ""
echo "==> Generating OTLP collector TLS certificates..."
bash "${WORKSPACE_DIR}/projects/observability/certs/regenerate_certs.sh"

echo ""
echo "All key material generated successfully."
echo "These files are gitignored and must not be committed."
