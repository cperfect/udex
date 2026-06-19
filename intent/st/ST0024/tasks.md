# Tasks - ST0024: K8s ingress tls

## Tasks

### WP01 — Edge cert generation

- [x] Add `projects/k8s/traefik/certs/regenerate_certs.sh` (own CA + edge cert; SANs `host.docker.internal`, `localhost`, `127.0.0.1`, `::1`; `chmod` keys/certs; dev-only banner)
- [x] Gitignore the generated edge cert material
- [x] Wire the new script into `scripts/gen-keys-and-certs.sh` (generation step + `ALL_EXIST` guard)
- [x] Add the four edge cert files to the key-material check in `scripts/dev-doctor.sh`

### WP02 — Helm chart: terminate + re-encrypt

- [x] Delete `templates/ingressroutetcp.yaml`
- [x] Add `templates/ingressroute.yaml` (L7 IngressRoute, websecure, `scheme: https` + `serversTransport` ref, `tls.secretName`)
- [x] Add `templates/serverstransport.yaml` (`insecureSkipVerify: true`)
- [x] Add `templates/ingress-tls-secret.yaml` (`kubernetes.io/tls`, `required` guards)
- [x] Update `templates/service.yaml` comment (no longer passthrough)
- [x] Add `secrets.traefikTlsCrt` / `secrets.traefikTlsKey` to `values.yaml`

### WP03 — Deploy + cluster scripts

- [x] `scripts/deploy.sh`: `--set-file` edge cert/key + existence guard
- [x] `scripts/cluster-create.sh`: wait on `ingressroutes.traefik.io` / `serverstransports.traefik.io` CRDs (not `ingressroutetcps`)
- [x] `scripts/validate-lint-helm.sh`: add `traefikTlsCrt`/`traefikTlsKey` placeholders

### WP04 — Test repoint + end-to-end validation

- [x] Repoint k8s fixture CA in `sdk/tests/integration_tests.rs` to `projects/k8s/traefik/certs/ca.crt` (new `edge_cert_path` helper); also add `secrets.traefikTls{Crt,Key}` to `redeploy_k8s_server`'s `helm upgrade` (required by chart)
- [x] Full loop: `cluster-create → image-load → deploy` (image pre-built)
- [x] `bash scripts/validate-k8s-test.sh` → all 6 `test_sdk_k8s_*` pass
- [x] `bash scripts/dev-doctor.sh` → edge cert material reported present

### WP05 — Docs + security scan

- [ ] Update `projects/k8s/README.md` (mermaid diagram, prose passthrough → terminate+re-encrypt, chart-structure listing, health-probe note)
- [ ] Run the Trivy/KSV Helm misconfig scan; add a justified annotated suppression if `insecureSkipVerify` is flagged

## Task Notes

- Decision (18 Jun 2026): re-encrypt Traefik→pod, no server (Rust) changes; edge cert gets its own CA; test-code change to trust it is in scope. See `design.md` (D1–D5).
- D4 hardening (`rootCAsSecrets` + `serverName`) is deferred to prod; local dev uses `insecureSkipVerify`.

## Dependencies

- WP02, WP03 depend on WP01 (edge cert must exist for deploy/lint).
- WP04 depends on WP01–WP03 (chart + scripts + certs in place before deploy/test).
- WP05 (Trivy scan) depends on WP02 (chart templates rendered).
