#!/usr/bin/env bash
# Loads the udex Docker image into the local k3d cluster.
# Run after image-build.sh and before deploy.sh.
#
# Usage:
#   bash projects/k8s/scripts/image-load.sh           # loads udex:latest
#   bash projects/k8s/scripts/image-load.sh --dev     # loads udex:dev
set -euo pipefail

CLUSTER_NAME="udex"
TAG="udex:latest"

for arg in "$@"; do
  case "$arg" in
    --dev) TAG="udex:dev" ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

for cmd in k3d docker; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "ERROR: $cmd is required but not installed." >&2
    exit 1
  fi
done

if ! k3d cluster list 2>/dev/null | grep -q "^${CLUSTER_NAME}\b"; then
  echo "ERROR: cluster '${CLUSTER_NAME}' does not exist. Run cluster-create.sh first." >&2
  exit 1
fi

echo "Loading image '${TAG}' into cluster '${CLUSTER_NAME}'..."
k3d image import "${TAG}" -c "${CLUSTER_NAME}"
echo "Image '${TAG}' loaded."
