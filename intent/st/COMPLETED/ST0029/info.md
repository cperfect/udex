---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
intent_version: 2.15.0
status: Completed
slug: remove-cross-test-ordering-dependencies-in-the-sdk-integration-suite
created: 20260808
completed: 2026-08-08T09:22:12Z
---

# ST0029: Remove cross-test ordering dependencies in the SDK integration suite

## Objective

Make every test in `projects/rust/sdk/tests/integration_tests.rs` self-sufficient: each must establish the preconditions it asserts on, rather than assuming another test has already produced them. No test may destroy shared fixture state that other tests depend on.

## Context

Surfaced during ST0028. A full-package run failed with **10 of 40** integration tests down, then passed on a re-run. The intermittency made it look like flakiness; it was not.

`test_sdk_delete_index_not_empty` asserts that deleting a **non-empty** index is refused, and its own comment stated the assumption plainly: *"The shared index has entries from other tests."* Nothing enforced that ordering. When it ran before any entry-creating test, the shared `sdk-integration-test-index` was still empty, so the delete **succeeded**, `unwrap_err()` panicked, and the fixture index was gone — failing the nine tests that depend on it with a misleading `Index 'sdk-integration-test-index' not found`.

Two things made this worse than a normal flake:

- **It could not pass in isolation at all.** `cargo test test_sdk_delete_index_not_empty` failed 100% of the time, because a fresh test database gives it an empty shared index. Anyone running the test alone to debug the suite met a guaranteed failure that told them nothing.
- **A second instance existed and was invisible.** `test_sdk_oauth2_delete_index_not_empty` is the OAuth2 twin of the same defect against the Hydra fixture. It was found only by running all 40 tests individually — no observed failure had ever pointed at it.

The dependency predates ST0028. What changed was scheduling pressure: ST0028 added two concurrently-running tests to the `obs` binary, and cargo runs test binaries in parallel, which made the losing interleaving reachable.

This violates the project's rule that flakey tests are broken tests, and it is a latent CI failure independent of observability — hence its own steel thread rather than absorption into ST0028.

## Acceptance

Acceptance Criteria and Acceptance Tests for this steel thread live in `acceptance.md` (the single source of truth). Do not restate ACs here -- see that file for the ratified completeness boundary and live status.

## Related Steel Threads

- ST0028 (OpenObserve as the dev observability backend) — surfaced the defect; its added concurrency is what made the latent ordering dependency reachable.

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
