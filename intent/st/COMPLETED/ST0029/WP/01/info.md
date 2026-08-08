---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Isolation sweep in CI and locally"
scope: Small
status: Done
---

# WP-01: Isolation sweep in CI and locally

## Objective

Make the property this thread established -- every test passes on its own -- something CI enforces rather than something that happened to be true on the day it was fixed. A suite that only ever runs whole cannot tell you which of its tests are load-bearing for others.

## Deliverables

- `scripts/validate-test-isolation.sh` -- runs every test in a target binary alone, in a fresh process, and fails naming any that cannot pass without another test having run first. Named to match the existing `validate-*.sh` scripts, which exist so a developer can run exactly what CI runs.
- A `Test isolation` step in the Build & Test job of `.github/workflows/01-Validation.yml`, kept separate from `Build & Test` so a failure reads as an ordering problem rather than a test failure.
- The rule and the command documented in `projects/rust/CONTRIBUTING.md`, where somebody writing a new test will meet them.

## Implementation notes

**Gated on every run, not periodic.** The original suggestion was a scheduled check, on the assumption it would be slow. Measured first: **~20s**, because the script drives the already-compiled test binary directly rather than invoking `cargo test` once per test, where cargo's own startup dominated. At that cost there is no argument for a schedule, which would only let a regression sit undetected until the next sweep.

**The test list is discovered, never hardcoded** (`--list` against the built binary), so a new test is covered the moment it exists. A hardcoded list would quietly stop covering new tests -- the exact failure mode this thread is about.

**Serial by design.** The fixtures bind fixed ports, so parallel test processes would fight over them and report port conflicts as test failures: noise indistinguishable from the defect being hunted.

**Every test is attempted even after a failure**, so one run gives the whole picture. That is how the second instance of the defect was found.

## Acceptance

Acceptance Criteria for this work package live in the steel thread's `acceptance.md`, under the `WP-01` heading (single source of truth). Do not restate ACs here.

## Dependencies

- The ST-level work (both `delete_index_not_empty` tests made self-sufficient) must land first, or the gate is red on arrival.
