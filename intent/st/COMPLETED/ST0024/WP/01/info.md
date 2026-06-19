---
verblock: "18 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Edge cert generation"
scope: Small
status: Done
---

# WP-01: Edge cert generation

## Objective

Generate the static Traefik edge certificate (with its own CA) under `projects/k8s/traefik/certs/`, and wire generation + health checks into the workspace scripts.

## Deliverables

- `projects/k8s/traefik/certs/regenerate_certs.sh` — self-contained edge CA + cert, modelled on `projects/rust/server/tests/certs/regenerate_certs.sh`. SANs: `host.docker.internal`, `localhost`, `127.0.0.1`, `::1`. `chmod 600 *.key`, `644 *.crt`; dev-only banner; idempotent clean.
- Gitignore entry for the generated edge cert material.
- `scripts/gen-keys-and-certs.sh` — generation step + `ALL_EXIST` guard extended with the four edge files.
- `scripts/dev-doctor.sh` — key-material check extended with the four edge files.

## Acceptance Criteria

- [x] `bash scripts/gen-keys-and-certs.sh --force` produces `ca.key`, `ca.crt`, `tls.key`, `tls.crt` in `projects/k8s/traefik/certs/`
- [x] `openssl x509 -in tls.crt -text` shows the four required SANs
- [x] Edge cert material is gitignored (not staged by `git add -A`)
- [x] `bash scripts/dev-doctor.sh` reports edge cert material present

## Dependencies

- None (foundational; WP02–WP04 depend on this).
