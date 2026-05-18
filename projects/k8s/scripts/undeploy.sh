#!/usr/bin/env bash
# Removes the udex Helm release from the local k3d cluster.
#
# Usage: bash projects/k8s/scripts/undeploy.sh
set -euo pipefail

RELEASE_NAME="udex"
NAMESPACE="default"

if ! command -v helm &>/dev/null; then
  echo "ERROR: helm is required but not installed." >&2
  exit 1
fi

if ! helm status "${RELEASE_NAME}" --namespace "${NAMESPACE}" &>/dev/null; then
  echo "Release '${RELEASE_NAME}' is not deployed — nothing to do."
  exit 0
fi

echo "Uninstalling release '${RELEASE_NAME}' from namespace '${NAMESPACE}'..."
helm uninstall "${RELEASE_NAME}" --namespace "${NAMESPACE}"
echo "Release '${RELEASE_NAME}' removed."
