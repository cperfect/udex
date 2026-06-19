---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Documentation updates"
scope: Small
status: Not Started
---

# WP-05: Documentation updates

## Objective

Bring the docs in line with the multi-replica dev deployment and the direct-addressing test path.

## Deliverables

- `projects/k8s/README.md`: 2-replica default; how the LB round-robins across instances; the direct-addressing (port-forward) test path; refresh the architecture diagram/prose if affected.
- `docs/ARCHITECTURE.md`: note multi-instance statelessness coverage and how cross-instance consistency is exercised (link to the test suite).
- `projects/rust/CONTRIBUTING.md`: document the `test_sdk_k8s_multi_*` naming/convention alongside the existing `test_sdk_k8s_*` note (if that file documents the convention).
- `docs/SECRETS.md`: update only if WP01–WP04 introduced new key material (none expected).

## Acceptance Criteria

- [ ] README describes the 2-replica default + direct-addressing test path; no stale single-replica references
- [ ] ARCHITECTURE/CONTRIBUTING reflect the multi-instance tests and convention
- [ ] Repo-wide sweep: no doc claims the deployment is single-replica
- [ ] Markdown fenced blocks all have language identifiers (project rule)

## Dependencies

- WP01–WP04 (document the as-built behaviour; run after the implementation settles).
