---
verblock: "18 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Helm chart: terminate + re-encrypt"
scope: Small
status: Done
---

# WP-02: Helm chart: terminate + re-encrypt

## Objective

Replace the L4 TLS-passthrough ingress with an L7 IngressRoute that terminates client TLS with the edge cert and re-encrypts to the pod via a ServersTransport. No server/Rust changes.

## Deliverables

- Delete `templates/ingressroutetcp.yaml`.
- `templates/ingressroute.yaml` — `kind: IngressRoute`, `entryPoints: [websecure]`, catch-all match, service `port: 443` with `scheme: https` + `serversTransport` ref, `tls.secretName` → edge cert secret.
- `templates/serverstransport.yaml` — `kind: ServersTransport`, `insecureSkipVerify: true` (D4).
- `templates/ingress-tls-secret.yaml` — `type: kubernetes.io/tls`, `required` guards on cert/key.
- `templates/service.yaml` — comment update (no longer passthrough).
- `values.yaml` — add `secrets.traefikTlsCrt` / `secrets.traefikTlsKey`.

## Acceptance Criteria

- [x] `helm template` renders IngressRoute, ServersTransport, and the kubernetes.io/tls Secret (verified with `helm v4.0.0`)
- [x] `helm lint --strict` passes with all required secrets (validate-lint-helm placeholders added in WP03)
- [x] No `IngressRouteTCP` / `tls.passthrough` remains in the chart

## Dependencies

- WP01 (edge cert exists for deploy/lint).
