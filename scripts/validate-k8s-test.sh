#!/bin/bash
# Mirrors the "Run k8s integration tests" step in 01-Validation.yml (k8s-test job).
# Run from any directory. Requires:
#   - A live udex deployment in k3d (run cluster-create.sh, image-build.sh,
#     image-load.sh, deploy.sh first)
#   - DATABASE_URL pointing at a running PostgreSQL instance
#   - HYDRA_PUBLIC_URL and HYDRA_ADMIN_URL pointing at a running Hydra instance
# The devcontainer provides DATABASE_URL and Hydra URLs automatically via .env.
#
# K8S_SERVER_URL defaults to https://host.docker.internal:8443 for local dev
# (the devcontainer must reach the k3d load balancer via the Docker host).
# CI sets it explicitly to https://localhost:8443.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

export K8S_SERVER_URL="${K8S_SERVER_URL:-https://host.docker.internal:8443}"

cd "${SCRIPT_DIR}/../projects/rust"
cargo test -p udex-sdk --test integration_tests -- k8s --nocapture
