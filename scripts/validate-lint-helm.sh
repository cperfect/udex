#!/bin/bash
# Mirrors the "Helm lint" step in 01-Validation.yml (k8s-lint job).
# Run from any directory. Requires helm to be installed.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"

CHART_DIR="${WORKSPACE_DIR}/projects/k8s/helm/udex"

if ! command -v helm &>/dev/null; then
  echo "ERROR: helm is required but not installed." >&2
  exit 1
fi

# Pass non-empty placeholder values for the required secrets so helm can render
# the templates. These are never deployed; they satisfy the required() guards
# in secret.yaml so lint can validate the full chart structure.
helm lint "${CHART_DIR}" \
  --strict \
  --set secrets.databaseUrl=placeholder \
  --set secrets.tlsCrt=placeholder \
  --set secrets.tlsKey=placeholder
