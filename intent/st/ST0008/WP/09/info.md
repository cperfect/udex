---
verblock: "28 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-09
title: "Fix CI: generate key material before tests"
scope: Small
status: Not Started
---

# WP-09: Fix CI: generate key material before tests

## Objective

Add a CI step to generate TLS certs and JWT key pairs before `cargo test` runs.
WP-01 removed all cert and key material from git tracking (including public artefacts
like `.crt` and `.pem` files), but the `01-Validation.yml` test job was not updated
to regenerate them. Integration tests that read from `tests/certs/` and `tests/jwt/`
will fail with file-not-found errors until this is fixed.

## Deliverables

- `01-Validation.yml` `test` job: new step running `scripts/gen-keys-and-certs.sh`
  immediately before `cargo build`

## Acceptance Criteria

- [ ] CI `test` job includes a "Generate key material" step that runs
  `scripts/gen-keys-and-certs.sh` before `cargo build`
- [ ] `cargo test` passes end-to-end in CI (no file-not-found errors for certs or keys)
- [ ] The step runs from the workspace root, not `projects/rust/`

## Dependencies

- WP-01 (completed — removed cert/key files from git, which created this gap)
