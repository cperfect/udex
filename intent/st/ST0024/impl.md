# Implementation - ST0024: K8s ingress tls

## Implementation

### WP01 — Edge cert generation (as built)

- Added `projects/k8s/traefik/certs/regenerate_certs.sh`, modelled on `projects/rust/server/tests/certs/regenerate_certs.sh`. Generates a self-contained edge CA (`ca.key`/`ca.crt`) and edge server cert (`tls.key`/`tls.crt`, plus `tls.csr`/`ca.srl` intermediates). CN `host.docker.internal`; SANs `localhost`, `host.docker.internal`, `127.0.0.1`, `::1`. `chmod 600 *.key`, `644 *.crt *.csr`.
- `.gitignore`: added `projects/k8s/traefik/certs/*.{key,crt,csr,srl}` mirroring the existing per-path server-cert entries.
- `scripts/gen-keys-and-certs.sh`: new `EDGE_TLS_DIR`, the four edge files added to the `ALL_EXIST` guard, and a `==> Generating Traefik edge TLS certificates...` step between the server-cert and JWT steps.
- `scripts/dev-doctor.sh`: edge files added to the key-material check; PASS label now reads "TLS certs + Traefik edge certs + JWT signing keys".

Verified: `gen-keys-and-certs.sh --force` runs all three generation steps; `openssl x509` confirms the four SANs; `openssl verify` confirms the chain; all key material is `git check-ignore`d; `dev-doctor.sh` reports the edge certs present. `bash -n` clean (shellcheck unavailable in env).

## Code Examples

[Key code snippets and examples]

## Technical Details

[Specific technical details and considerations]

## Challenges & Solutions

[Challenges encountered during implementation and how they were resolved]
