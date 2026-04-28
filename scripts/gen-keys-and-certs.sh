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

JWT_KEY="${WORKSPACE_DIR}/projects/rust/server/tests/jwt/signing_private_key.pem"
TLS_KEY="${WORKSPACE_DIR}/projects/rust/server/tests/certs/server.key"

if [[ -f "${JWT_KEY}" && -f "${TLS_KEY}" && "${FORCE}" == false ]]; then
  echo "Key material already exists — skipping generation."
  echo "Run with --force to rotate keys and certificates."
  exit 0
fi

echo "==> Generating TLS certificates..."
bash "${WORKSPACE_DIR}/projects/rust/server/tests/certs/regenerate_certs.sh"

echo ""
echo "==> Generating JWT signing key pairs..."
bash "${WORKSPACE_DIR}/projects/rust/server/tests/jwt/regenerate_jwt_signing_key_pair.sh"

echo ""
echo "All key material generated successfully."
echo "These files are gitignored and must not be committed."
