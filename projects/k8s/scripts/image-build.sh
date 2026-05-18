#!/usr/bin/env bash
# Builds the udex Docker image.
#
# Usage:
#   bash projects/k8s/scripts/image-build.sh           # optimised (release) build
#   bash projects/k8s/scripts/image-build.sh --dev     # development (debug) build
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
WORKSPACE_DIR="$(cd "${SCRIPT_DIR}/../../.." && pwd)"

PROFILE="release"
TAG="udex:latest"

for arg in "$@"; do
  case "$arg" in
    --dev) PROFILE="dev"; TAG="udex:dev" ;;
    *) echo "Unknown argument: $arg" >&2; exit 1 ;;
  esac
done

if ! command -v docker &>/dev/null; then
  echo "ERROR: docker is required but not installed." >&2
  exit 1
fi

DOCKERFILE="${WORKSPACE_DIR}/projects/rust/cli/Dockerfile"
CONTEXT="${WORKSPACE_DIR}/projects"

echo "Building image '${TAG}' (PROFILE=${PROFILE})..."
docker build \
  -f "${DOCKERFILE}" \
  --build-arg "PROFILE=${PROFILE}" \
  -t "${TAG}" \
  "${CONTEXT}"

echo "Image '${TAG}' built successfully."
