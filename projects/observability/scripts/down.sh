#!/bin/bash
# Tear down the local observability stack. Leaves the base stack (postgres +
# hydra) and the external network untouched.
#
#   bash projects/observability/scripts/down.sh
#   KEEP_VOLUMES=0 bash projects/observability/scripts/down.sh   # also remove volumes

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
OBS_DIR="$(cd "${SCRIPT_DIR}/.." && pwd)"
WORKSPACE_DIR="$(cd "${OBS_DIR}/../.." && pwd)"

ENV_FILE="${WORKSPACE_DIR}/.env"
OBS_COMPOSE="${OBS_DIR}/docker-compose.observability.yml"
PROJECT="udex-observability"
PROFILE="observability"

# OBS_NETWORK is only needed so the compose file's `external` network reference
# parses; `down` neither creates nor removes the external network, so a best-
# effort value (the real one if postgres is up, else a placeholder) is fine.
PG_CID="$(docker ps -aq --filter "label=com.docker.compose.service=postgres" | head -n1)"
if [[ -n "${PG_CID}" ]]; then
  OBS_NETWORK="$(docker inspect -f '{{range $k,$_ := .NetworkSettings.Networks}}{{$k}}{{"\n"}}{{end}}' "${PG_CID}" | head -n1)"
fi
export OBS_NETWORK="${OBS_NETWORK:-bridge}"

DOWN_ARGS=(--profile "${PROFILE}" down --remove-orphans)
[[ "${KEEP_VOLUMES:-1}" == "0" ]] && DOWN_ARGS+=(--volumes)

ENV_ARG=()
if [[ -f "${ENV_FILE}" ]]; then
  ENV_ARG=(--env-file "${ENV_FILE}")
else
  # No env file present: provide a placeholder so the compose file's required
  # `GRAFANA_ADMIN_PASSWORD` interpolation parses. `down` never starts Grafana,
  # so the value is irrelevant — it only needs to be non-empty to tear down.
  export GRAFANA_ADMIN_PASSWORD="${GRAFANA_ADMIN_PASSWORD:-unused-for-down}"
fi

echo "==> Stopping observability stack..."
docker compose -p "${PROJECT}" -f "${OBS_COMPOSE}" "${ENV_ARG[@]}" "${DOWN_ARGS[@]}"
echo "Observability stack stopped."
