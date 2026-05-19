#!/usr/bin/env bash
# Deletes the local k3d Udex cluster.
#
# Usage: bash projects/k8s/scripts/cluster-delete.sh
set -euo pipefail

CLUSTER_NAME="udex"

if ! command -v k3d &>/dev/null; then
  echo "ERROR: k3d is required but not installed." >&2
  exit 1
fi

if ! k3d cluster list 2>/dev/null | grep -q "^${CLUSTER_NAME}\b"; then
  echo "Cluster '${CLUSTER_NAME}' does not exist — nothing to do."
  exit 0
fi

echo "Deleting k3d cluster '${CLUSTER_NAME}'..."
k3d cluster delete "${CLUSTER_NAME}"
echo "Cluster '${CLUSTER_NAME}' deleted."
