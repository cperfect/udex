#!/bin/bash
# Bring up the local observability stack (Collector, Tempo, Prometheus, Loki,
# Grafana, Vector) and attach it to the running base/devcontainer network so the
# app can reach the collector and the collector can reach postgres.
#
# Idempotent. Starts the base stack (postgres + hydra) first if it is not running.
#
#   bash projects/observability/scripts/up.sh
#   FORCE_RECREATE=1 bash projects/observability/scripts/up.sh   # recreate containers

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OBS_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
WORKSPACE_DIR="$(cd "${OBS_DIR}/../.." && pwd)"

ENV_FILE="${WORKSPACE_DIR}/.env"
OBS_COMPOSE="${OBS_DIR}/docker-compose.observability.yml"
BASE_COMPOSE="${WORKSPACE_DIR}/projects/compose/docker-compose.yml"
PROJECT="udex-observability"
PROFILE="observability"
RETRIES="${RETRIES:-60}"

# --- Preconditions ---------------------------------------------------------
[[ -f "${ENV_FILE}" ]] || {
  echo "ERROR: ${ENV_FILE} not found - run scripts/gen-env.sh" >&2; exit 1; }

for f in ca.crt collector.crt collector.key; do
  [[ -f "${OBS_DIR}/certs/${f}" ]] || {
    echo "ERROR: OTLP cert missing (certs/${f}) - run scripts/gen-keys-and-certs.sh" >&2
    exit 1; }
done

# --- Ensure the base stack is up and find its network ----------------------
# Scope container discovery to THIS checkout's compose project when we can identify
# our own container (i.e. running inside the devcontainer), so a second checkout's
# `postgres`/`devcontainer` services on the same daemon are not matched by mistake.
# On a bare host (no self-container) we fall back to a daemon-wide match, which is
# unambiguous for a single running stack.
SELF_PROJECT="$(docker inspect "$(hostname)" \
  --format '{{index .Config.Labels "com.docker.compose.project"}}' 2>/dev/null || true)"
PROJECT_FILTER=()
if [[ -n "${SELF_PROJECT}" ]]; then
  PROJECT_FILTER=(--filter "label=com.docker.compose.project=${SELF_PROJECT}")
  echo "==> Scoping discovery to compose project: ${SELF_PROJECT}"
fi

PG_CID="$(docker ps -q "${PROJECT_FILTER[@]}" --filter "label=com.docker.compose.service=postgres" | head -n1)"
if [[ -z "${PG_CID}" ]]; then
  echo "==> Base stack not running; starting postgres + hydra..."
  docker compose -f "${BASE_COMPOSE}" --env-file "${ENV_FILE}" up -d
  PG_CID="$(docker ps -q "${PROJECT_FILTER[@]}" --filter "label=com.docker.compose.service=postgres" | head -n1)"
fi
[[ -n "${PG_CID}" ]] || { echo "ERROR: could not start/find the base postgres container" >&2; exit 1; }

OBS_NETWORK="$(docker inspect -f '{{range $k,$_ := .NetworkSettings.Networks}}{{$k}}{{"\n"}}{{end}}' "${PG_CID}" | head -n1)"
[[ -n "${OBS_NETWORK}" ]] || { echo "ERROR: could not determine the stack network" >&2; exit 1; }
export OBS_NETWORK
echo "==> Attaching observability stack to network: ${OBS_NETWORK}"

# Bind-mount sources must be DAEMON-HOST paths. When running inside the
# devcontainer (docker-outside-of-docker), the daemon is the host's and does not
# know our in-container /workspace path, so translate it to the host path via the
# devcontainer's /workspace mount source. On the host, OBS_DIR is already a valid
# daemon path.
DEVC_CID="$(docker ps -q "${PROJECT_FILTER[@]}" --filter "label=com.docker.compose.service=devcontainer" | head -n1)"
HOST_WORKSPACE=""
if [[ -n "${DEVC_CID}" ]]; then
  HOST_WORKSPACE="$(docker inspect "${DEVC_CID}" --format '{{range .Mounts}}{{if eq .Destination "/workspace"}}{{.Source}}{{end}}{{end}}' 2>/dev/null || true)"
fi
if [[ -n "${HOST_WORKSPACE}" ]]; then
  export OBS_HOST_DIR="${HOST_WORKSPACE}/projects/observability"
else
  export OBS_HOST_DIR="${OBS_DIR}"
fi
echo "==> Bind-mount host dir: ${OBS_HOST_DIR}"

# --- Bring up the observability stack --------------------------------------
docker compose -p "${PROJECT}" -f "${OBS_COMPOSE}" --env-file "${ENV_FILE}" \
  --profile "${PROFILE}" up -d ${FORCE_RECREATE:+--force-recreate}

# --- Readiness -------------------------------------------------------------
# Try the in-network service name (works inside the devcontainer) and the
# host-published port (works on the host); either succeeding means ready.
probe() {
  local name="$1" internal="$2" localport="$3" path="$4"
  local u1="http://${internal}${path}" u2="http://localhost:${localport}${path}"
  for _ in $(seq 1 "${RETRIES}"); do
    if curl -fsS --max-time 3 "${u1}" >/dev/null 2>&1 || curl -fsS --max-time 3 "${u2}" >/dev/null 2>&1; then
      printf "  [ready] %s\n" "${name}"; return 0
    fi
    sleep 2
  done
  printf "  [TIMEOUT] %s (%s | %s)\n" "${name}" "${u1}" "${u2}" >&2
  return 1
}

echo "==> Waiting for components to become ready..."
rc=0
probe "otel-collector" "otel-collector:13133" "13133" "/"          || rc=1
probe "tempo"          "tempo:3200"           "3200"  "/ready"     || rc=1
probe "prometheus"     "prometheus:9090"      "9090"  "/-/ready"   || rc=1
probe "loki"           "loki:3100"            "3100"  "/ready"     || rc=1
probe "grafana"        "grafana:3000"         "3000"  "/api/health" || rc=1

echo ""
if [[ "${rc}" -eq 0 ]]; then
  echo "Observability stack is up."
  echo "  Grafana:    http://localhost:3000  (admin / see GRAFANA_ADMIN_PASSWORD in .env)"
  echo "  Prometheus: http://localhost:9090"
  echo "  Loki:       http://localhost:3100"
  echo "  Tempo:      http://localhost:3200"
  echo "  OTLP (TLS): otel-collector:4317 (in-network) / localhost:4317 (host)"
else
  echo "Some components did not become ready in time - check 'docker compose -p ${PROJECT} logs'." >&2
fi
exit "${rc}"
