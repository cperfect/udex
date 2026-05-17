---
verblock: "17 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Update docs: architecture test strategy, contributing guides, MODULES.md"
scope: Small
status: Not Started
---

# WP-04: Update docs: architecture test strategy, contributing guides, MODULES.md

## Objective

Document the test strategy established in ST0017 so future contributors know where to add tests, how to name them, and where shared fixture code lives. Update every relevant guide so the decisions are discoverable and consistent.

## Deliverables

- `intent/docs/ARCHITECTURE.md` — add a "Test Strategy" section that describes the Test Diamond rationale, the canonical suite table (SDK as primary, others as supplementary), and the naming convention.

- `projects/rust/CONTRIBUTING.md` — replace any mention of `test_hydra_` prefix with the new `test_*_oauth2_` convention; add a "Integration Test Naming" section pointing to the canonical prefixes and explaining where to add new tests.

- `CONTRIBUTING.md` (workspace root) — add or update the testing section to reference the test strategy in `ARCHITECTURE.md` and note that integration tests should follow the naming convention.

- `intent/llm/MODULES.md` — register `udex-test-utils` under the Rust workspace section with its exported concerns (`bind_file_secret`, `hydra_public_url`, `hydra_admin_url`, `register_hydra_client`, `acquire_oauth2_token`).

- `intent/st/NOT-STARTED/ST0017/impl.md` — populate with the implementation decisions: why the SDK tests are primary, why service-layer tests were slimmed, the chosen naming convention and the rationale for `oauth2` over `hydra` in test names.

## Acceptance Criteria

- [ ] `ARCHITECTURE.md` contains a "Test Strategy" section with the canonical suite table
- [ ] `projects/rust/CONTRIBUTING.md` no longer references the old `test_hydra_` prefix and documents the canonical naming convention
- [ ] `intent/llm/MODULES.md` includes a `udex-test-utils` entry
- [ ] `ST0017/impl.md` is fully populated (no template placeholders remain)
- [ ] `cargo fmt --check`, `cargo clippy --all-targets` pass (no Rust changes, but verify nothing was inadvertently broken)

## Dependencies

- WP-01, WP-02, WP-03 should be complete so the impl.md can be written with accurate retrospective notes
