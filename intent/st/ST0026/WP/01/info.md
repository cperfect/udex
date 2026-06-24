---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Observability runtime stack"
scope: Small
status: Done
---

# WP-01: Observability runtime stack

## Objective

Stand up the local observability backend stack under a new `projects/observability/`
folder and layer it into the existing dev infrastructure (main compose +
devcontainer) as an opt-in profile, without changing application code yet. This
gives every later WP concrete backends to point telemetry at.

## Deliverables

- `projects/observability/docker-compose.observability.yml` defining: OpenTelemetry
  Collector (with `postgresqlreceiver`), Grafana Tempo, Prometheus, Grafana Loki,
  Grafana, and Vector.
- Per-component config trees: `collector/`, `tempo/`, `prometheus/`, `loki/`,
  `grafana/` (datasource + dashboard provisioning), `vector/`.
- OTLP cert generation script under `projects/observability/` (Collector server
  cert + CA), folded into `scripts/gen-keys-and-certs.sh`; outputs gitignored.
- Grafana admin credential surfaced via `.env` (added to `scripts/gen-env.sh`),
  gitignored.
- `projects/observability/scripts/up.sh`, `down.sh`, `rebuild.sh` that run the
  observability compose as its own project (`udex-observability`, profile
  `observability`) attached to the running base/devcontainer network.
- Devcontainer reachability: the app reaches the collector by service name on the
  shared network; host access via published ports. (See as-built note - the
  external-network approach replaces a compose-file symlink.)
- `scripts/dev-doctor.sh` checks for the new artefacts (OTLP certs, Grafana cred).
- `projects/observability/README.md`.

## Acceptance Criteria

- [x] `bash projects/observability/scripts/up.sh` brings all six components up
  healthy; `down.sh` / `rebuild.sh` work.
- [x] A plain `docker compose up` of the base stack still starts only
  postgres + hydra (observability is a separate, profile-gated project, off by
  default).
- [x] Each component is reachable on its API/UI (Grafana, Prometheus, Loki,
  Tempo, Collector health) and Grafana has the datasources provisioned.
- [x] OTLP certs and Grafana credential are generated idempotently and gitignored;
  `git status` shows no secret material.
- [x] `scripts/dev-doctor.sh` reports the new artefacts.

## As-built notes

- **Layering as a separate project, not a `-f` merge.** With multi-file
  `docker compose -f a -f b`, relative bind-mount paths resolve against the first
  file's directory - which would break the `./collector/config.yaml`-style mounts.
  So the stack runs as its own single-file project and attaches to the running
  base/devcontainer network (detected from the `postgres` container). This keeps
  bind mounts correct, gives a clean teardown, and still gives the in-devcontainer
  app full reachability (`otel-collector:4317`). No symlink was needed.
- **docker-outside-of-docker path translation.** The daemon is the host's, so
  bind-mount sources must be host paths. `up.sh` sets `OBS_HOST_DIR` by reading
  the devcontainer's `/workspace` mount source; the compose file uses
  `${OBS_HOST_DIR:-.}` for all bind sources (default `.` is correct on the host).
- **Verified live**: all six components ready; Grafana datasources provisioned;
  Prometheus scraping the collector (both targets up); PostgreSQL receiver metric
  `postgresql_backends` present; OTLP TLS endpoint serves a cert that validates
  against the OTLP CA (`Verify return code: 0`); an OTLP-over-TLS log POST landed
  in Loki and was queryable. `up.sh` is idempotent; teardown leaves the base
  stack intact.

## Dependencies

- None (first WP). Reuses the cert-generation and compose-layering patterns from
  the existing infra.
