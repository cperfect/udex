---
verblock: "18 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Test repoint + end-to-end validation"
scope: Small
status: Done
---

# WP-04: Test repoint + end-to-end validation

## Objective

Repoint the k8s integration test to trust the new edge CA, then validate the full terminate+re-encrypt path end to end against a live k3d deployment.

## Deliverables

- `sdk/tests/integration_tests.rs` — k8s fixture CA load (`server_cert_path("ca.crt")`, ~line 1507) and `wait_for_k8s_server` probe (~line 1381) point at `projects/k8s/traefik/certs/ca.crt`; `domain_name`/SNI stays `host.docker.internal`.
- Verified deploy loop: `cluster-create → image-build → image-load → deploy`.

## Acceptance Criteria

- [x] `bash scripts/validate-k8s-test.sh` → all 6 `test_sdk_k8s_*` pass (38.12s) against the re-encrypting ingress
- [x] Rollout completes; IngressRoute + ServersTransport applied in-cluster (`traefik.io` group)
- [x] `bash scripts/dev-doctor.sh` reports edge cert material present (verified in WP01)
- [x] No server (non-test) Rust code changed — only `sdk/tests/integration_tests.rs`
- [x] `cargo fmt --check` + `cargo clippy --tests -D warnings` clean

## Dependencies

- WP01, WP02, WP03 (certs + chart + scripts in place before deploy/test).
