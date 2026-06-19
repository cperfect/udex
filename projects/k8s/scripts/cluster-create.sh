#!/usr/bin/env bash
# Creates the local k3d cluster for Udex development.
# Idempotent — exits cleanly if the cluster already exists.
#
# Usage: bash projects/k8s/scripts/cluster-create.sh
set -euo pipefail

CLUSTER_NAME="udex"

for cmd in k3d docker kubectl; do
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

# k3d writes 0.0.0.0 as the API server address, which is not a valid
# connection target. Replace it with the appropriate address for the environment:
#   devcontainer — k3d runs on the Docker host; use host.docker.internal
#                  (also added to the k3s TLS SAN above so cert validation passes)
#   CI / bare metal — k3d runs on the same machine; use 127.0.0.1
if grep -q "https://0.0.0.0:" "${HOME}/.kube/config" 2>/dev/null; then
  if getent hosts host.docker.internal &>/dev/null; then
    NEW_ADDR="host.docker.internal"
  else
    NEW_ADDR="127.0.0.1"
  fi
  tmp_kubeconfig="$(mktemp)"
  sed "s|https://0.0.0.0:|https://${NEW_ADDR}:|g" "${HOME}/.kube/config" >"${tmp_kubeconfig}"
  mv "${tmp_kubeconfig}" "${HOME}/.kube/config"
  echo "Patched kubeconfig: replaced 0.0.0.0 with ${NEW_ADDR}"
fi

# k3s deploys Traefik asynchronously after the cluster API is ready. The
# IngressRoute and ServersTransport CRDs must be registered before helm install
# can apply them (the chart terminates TLS at Traefik and re-encrypts to the pod).
# Poll until both appear (typically 10–30 s on CI runners).
echo "Waiting for Traefik IngressRoute + ServersTransport CRDs..."
for i in $(seq 1 40); do
  if kubectl get crd ingressroutes.traefik.io serverstransports.traefik.io &>/dev/null; then
    echo "Traefik CRDs ready."; break
  fi
  echo "  attempt $i/40..."; sleep 3
done
kubectl get crd ingressroutes.traefik.io serverstransports.traefik.io || {
  echo "ERROR: Traefik IngressRoute/ServersTransport CRDs did not become ready."; exit 1
}

echo "Cluster '${CLUSTER_NAME}' ready."
echo "kubectl context is now: $(kubectl config current-context)"
