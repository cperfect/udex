---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Validation + consistency fix"
scope: Small
status: Not Started
---

# WP-04: Validation + consistency fix

## Objective

Validate the full multi-instance path end to end and fix any cross-instance consistency bug surfaced (D5). (Docs are WP05.)

## Deliverables

- Verified deploy loop: `cluster-create → image-load → deploy` → 2/2 Ready.
- `validate-k8s-test.sh` green for both existing `test_sdk_k8s_*` and new `test_sdk_k8s_multi_*`.
- If a cross-instance bug surfaces: a fix in `projects/rust/server/src` so the suite is green (not expected per analysis, but in scope).

## Acceptance Criteria

- [ ] `bash scripts/validate-k8s-test.sh` → all k8s tests (single + multi instance) pass
- [ ] `cargo fmt --check` + `cargo clippy --tests -- -D warnings` clean
- [ ] No server (non-test) Rust change unless a real consistency bug required it (documented in impl.md)

## Dependencies

- WP03 (suite must exist to run/validate).
