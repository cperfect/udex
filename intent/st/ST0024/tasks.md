# Tasks - ST0024: K8s ingress tls

## Tasks

### WP01 — Edge cert generation

- [ ] Add `projects/k8s/traefik/certs/regenerate_certs.sh` (own CA + edge cert; SANs `host.docker.internal`, `localhost`, `127.0.0.1`, `::1`; `chmod` keys/certs; dev-only banner)
- [ ] Gitignore the generated edge cert material
- [ ] Wire the new script into `scripts/gen-keys-and-certs.sh` (generation step + `ALL_EXIST` guard)
- [ ] Add the four edge cert files to the key-material check in `scripts/dev-doctor.sh`

### WP02 — Helm chart: terminate + re-encrypt

- [ ] Delete `templates/ingressroutetcp.yaml`
- [ ] Add `templates/ingressroute.yaml` (L7 IngressRoute, websecure, `scheme: https` + `serversTransport` ref, `tls.secretName`)
- [ ] Add `templates/serverstransport.yaml` (`insecureSkipVerify: true`)
- [ ] Add `templates/ingress-tls-secret.yaml` (`kubernetes.io/tls`, `required` guards)
- [ ] Update `templates/service.yaml` comment (no longer passthrough)
- [ ] Add `secrets.traefikTlsCrt` / `secrets.traefikTlsKey` to `values.yaml`

### WP03 — Deploy + cluster scripts

- [ ] `scripts/deploy.sh`: `--set-file` edge cert/key + existence guard
- [ ] `scripts/cluster-create.sh`: wait on `ingressroutes.traefik.io` / `serverstransports.traefik.io` CRDs (not `ingressroutetcps`)
- [ ] `scripts/validate-lint-helm.sh`: add `traefikTlsCrt`/`traefikTlsKey` placeholders

### WP04 — Test repoint + end-to-end validation

- [ ] Repoint k8s fixture CA in `sdk/tests/integration_tests.rs` (`server_cert_path("ca.crt")` ~1507 and `wait_for_k8s_server` probe ~1381) to `projects/k8s/traefik/certs/ca.crt`
- [ ] Full loop: `cluster-create → image-build → image-load → deploy`
- [ ] `bash scripts/validate-k8s-test.sh` → `test_sdk_k8s_*` pass
- [ ] `bash scripts/dev-doctor.sh` → edge cert material reported present

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
