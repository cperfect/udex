#!/bin/bash
# Recreate the observability stack from scratch: tear it down, then bring it up
# with fresh containers. Image versions are pinned in the compose file, so this
# does not change versions - it gives you a clean-slate stack.
#
#   bash projects/observability/scripts/rebuild.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "==> Rebuilding observability stack..."
# KEEP_VOLUMES=0 so the components' anonymous data volumes (Grafana, Loki, Tempo,
# Prometheus) are removed too - a true clean slate, not just fresh containers.
KEEP_VOLUMES=0 bash "${SCRIPT_DIR}/down.sh"
FORCE_RECREATE=1 bash "${SCRIPT_DIR}/up.sh"
