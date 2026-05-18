#!/usr/bin/env bash
# Creates the local k3d cluster for Udex development.
# Idempotent — exits cleanly if the cluster already exists.
#
# Usage: bash projects/k8s/scripts/cluster-create.sh
set -euo pipefail

CLUSTER_NAME="udex"

for cmd in k3d docker; do
  if ! command -v "$cmd" &>/dev/null; then
    echo "ERROR: $cmd is required but not installed." >&2
    exit 1
  fi
done

if k3d cluster list 2>/dev/null | grep -q "^${CLUSTER_NAME}\b"; then
  echo "Cluster '${CLUSTER_NAME}' already exists — nothing to do."
  exit 0
fi

echo "Creating k3d cluster '${CLUSTER_NAME}'..."
k3d cluster create "${CLUSTER_NAME}" \
  --port "8443:443@loadbalancer" \
  --k3s-arg "--tls-san=host.docker.internal@server:*"

# k3d writes 0.0.0.0 as the API server address, which is unreachable from
# inside a devcontainer. Replace it with host.docker.internal (which is also
# covered by the --tls-san above so cert verification succeeds).
if grep -q "https://0.0.0.0:" "${HOME}/.kube/config" 2>/dev/null; then
  sed -i 's|https://0.0.0.0:|https://host.docker.internal:|g' "${HOME}/.kube/config"
  echo "Patched kubeconfig: replaced 0.0.0.0 with host.docker.internal"
fi

echo "Cluster '${CLUSTER_NAME}' ready."
echo "kubectl context is now: $(kubectl config current-context)"
