---
verblock: "18 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "Deploy + cluster scripts"
scope: Small
status: Done
---

# WP-03: Deploy + cluster scripts

## Objective

Update the deploy and cluster-bootstrap scripts for the new ingress: ship the edge cert, wait on the new CRDs, and lint the new required values.

## Deliverables

- `scripts/deploy.sh` — `--set-file secrets.traefikTlsCrt=…/traefik/certs/tls.crt` and `…tls.key`; existence guard for the edge cert files.
- `scripts/cluster-create.sh` — CRD readiness wait targets `ingressroutes.traefik.io` / `serverstransports.traefik.io` instead of `ingressroutetcps.traefik.io`.
- `scripts/validate-lint-helm.sh` — add `--set secrets.traefikTlsCrt=placeholder --set secrets.traefikTlsKey=placeholder`.

## Acceptance Criteria

- [x] `cluster-create.sh` polls and passes on the new CRD names (saw "Traefik CRDs ready" for `ingressroutes` + `serverstransports`)
- [x] `deploy.sh` fails fast with an actionable error when edge cert files are absent (guard extended with `traefik/certs/tls.{crt,key}`)
- [x] `bash scripts/validate-lint-helm.sh` passes with the new placeholders
- [x] Live deploy succeeds: rollout complete; IngressRoute + ServersTransport created in `traefik.io` group; Traefik serves the edge cert at `host.docker.internal:8443`

## Dependencies

- WP01 (edge cert), WP02 (chart values/templates) for end-to-end lint.
