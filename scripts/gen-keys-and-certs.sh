#!/bin/bash
# Generates all dev key and certificate material by calling the per-component scripts.
# Re-running regenerates and rotates everything.
#
# Usage:
#   scripts/gen-keys-and-certs.sh
#
# The devcontainer post-create script calls this automatically.

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

echo "==> Generating TLS certificates..."
bash "${WORKSPACE_DIR}/projects/rust/server/tests/certs/regenerate_certs.sh"

echo ""
echo "==> Generating JWT signing key pairs..."
bash "${WORKSPACE_DIR}/projects/rust/server/tests/jwt/regenerate_jwt_signing_key_pair.sh"

echo ""
echo "All key material generated successfully."
echo "These files are gitignored and must not be committed."
