# Implementation - ST0024: K8s ingress tls

## Implementation

### WP01 — Edge cert generation (as built)

- Added `projects/k8s/traefik/certs/regenerate_certs.sh`, modelled on `projects/rust/server/tests/certs/regenerate_certs.sh`. Generates a self-contained edge CA (`ca.key`/`ca.crt`) and edge server cert (`tls.key`/`tls.crt`, plus `tls.csr`/`ca.srl` intermediates). CN `host.docker.internal`; SANs `localhost`, `host.docker.internal`, `127.0.0.1`, `::1`. `chmod 600 *.key`, `644 *.crt *.csr`.
- `.gitignore`: added `projects/k8s/traefik/certs/*.{key,crt,csr,srl}` mirroring the existing per-path server-cert entries.
- `scripts/gen-keys-and-certs.sh`: new `EDGE_TLS_DIR`, the four edge files added to the `ALL_EXIST` guard, and a `==> Generating Traefik edge TLS certificates...` step between the server-cert and JWT steps.
- `scripts/dev-doctor.sh`: edge files added to the key-material check; PASS label now reads "TLS certs + Traefik edge certs + JWT signing keys".

Verified: `gen-keys-and-certs.sh --force` runs all three generation steps; `openssl x509` confirms the four SANs; `openssl verify` confirms the chain; all key material is `git check-ignore`d; `dev-doctor.sh` reports the edge certs present. `bash -n` clean (shellcheck unavailable in env).

### WP02 — Helm chart: terminate + re-encrypt (as built)

- Deleted `templates/ingressroutetcp.yaml`.
- Added `templates/ingressroute.yaml`: `kind: IngressRoute`, `entryPoints: [websecure]`, `match: PathPrefix(`/`)`, service `port: 443` with `scheme: https` (re-encrypt, HTTP/2 via ALPN for gRPC) and `serversTransport: udex-udex`; `tls.secretName: udex-udex-tls`.
- Added `templates/serverstransport.yaml`: `kind: ServersTransport`, `insecureSkipVerify: true` (D4 — Traefik reaches the pod by Service name/IP, not a pod-cert SAN). Header documents the `rootCAsSecrets` + `serverName` prod-hardening path.
- Added `templates/ingress-tls-secret.yaml`: `type: kubernetes.io/tls`, `tls.crt`/`tls.key` from `.Values.secrets.traefikTls{Crt,Key}` with `required` guards. Name `{{ fullname }}-tls`, matching the IngressRoute `secretName`.
- `templates/service.yaml`: comment updated (passthrough → terminate + re-encrypt).
- `values.yaml`: added `secrets.traefikTlsCrt` / `secrets.traefikTlsKey`; clarified pod-vs-edge TLS in comments.

Verification: with helm v4.0.0 / kubectl v1.36 / k3d v5.8.3 available, `helm lint --strict` passes (0 failed) and `helm template --show-only` renders all three new resources correctly (IngressRoute with `scheme: https` + `serversTransport: udex-udex` + `secretName: udex-udex-tls`; ServersTransport `insecureSkipVerify: true`; kubernetes.io/tls Secret `udex-udex-tls`). No `IngressRouteTCP`/`passthrough` remains in the chart (only an explanatory comment reference). Live deploy/test exercised in WP03/WP04.

## Code Examples

[Key code snippets and examples]

## Technical Details

[Specific technical details and considerations]

## Challenges & Solutions

[Challenges encountered during implementation and how they were resolved]
