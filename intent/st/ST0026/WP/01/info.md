---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Observability runtime stack"
scope: Small
status: Not Started
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
- `projects/observability/scripts/up.sh`, `down.sh`, `rebuild.sh` that layer the
  observability compose onto the base compose behind a compose `profile`
  (`docker compose -f base -f observability ...`).
- Devcontainer wiring: symlink mirroring the existing
  `.devcontainer/docker-compose.yml -> projects/compose/...` pattern; forward
  ports as needed (Grafana etc.).
- `scripts/dev-doctor.sh` checks for the new artefacts (certs, env keys, optional
  stack health).
- `projects/observability/README.md`.

## Acceptance Criteria

- [ ] `bash projects/observability/scripts/up.sh` brings all six components up
  healthy; `down.sh` / `rebuild.sh` work.
- [ ] A plain `docker compose up` of the base stack still starts only
  postgres + hydra (observability behind a profile, off by default).
- [ ] Each component is reachable on its API/UI (Grafana, Prometheus, Loki,
  Tempo, Collector health/zpages) and Grafana has the datasources provisioned.
- [ ] OTLP certs and Grafana credential are generated idempotently and gitignored;
  `git status` shows no secret material.
- [ ] `scripts/dev-doctor.sh` reports the new artefacts.

## Dependencies

- None (first WP). Reuses the cert-generation and compose-layering patterns from
  the existing infra.
