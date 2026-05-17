---
verblock: "17 May 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Completed
slug: integration-test-consolidation-and-shared-test
created: 20260517
completed: 20260517
---

# ST0017: Integration test consolidation and shared test utilities

## Objective

Consolidate the integration test suite to reduce maintenance overhead and runtime cost without losing coverage or debug capability. Establish a consistent test naming convention, extract shared fixture code into a dedicated `udex-test-utils` crate, slim down redundant service-layer tests, rename `hydra` to `oauth2` in test function names, and document the test strategy in the architecture and contributing guides.

## Context

A duplication analysis of the integration tests across `server`, `sdk`, `cli`, and `datastore` revealed:

1. **Fixture code is copy-pasted across 5+ files** — `bind_file_secret`, `hydra_*_url`, `register_hydra_client`, and `ServerConfig` setup blocks are nearly identical in `server_integration_tests.rs`, `sdk/integration_tests.rs`, `cli/token_hydra_tests.rs`, `cli/index_hydra_tests.rs`, and `cli/entry_live_tests.rs`.

2. **Business-logic scenarios are tested at every layer** — create/lookup, delete, bulk write/read, lookup_or_create, and delete-index all appear in the direct service tests AND the SDK tests AND sometimes the CLI tests. The service-layer tests add little value over the SDK tests because they skip TLS, auth middleware, and the gRPC wire format — the exact failure modes that matter most.

3. **Every happy-path SDK scenario is run twice** — once with a static JWT, once with Hydra (JWKS). Only scope subsetting, JWKS fetch, and audience validation uniquely require Hydra; the rest of the Hydra test suite is redundant.

4. **Inconsistent naming makes it hard to tell at a glance what layer a test covers** — `test_create_entry`, `test_sdk_bulk_write_and_read`, and `test_hydra_sdk_lookup_nonexistent_returns_none` all follow different conventions.

## Test Strategy (canonical)

The SDK integration tests are the **primary end-to-end test suite**. They exercise the full stack: TLS, auth middleware, gRPC wire format, and the datastore. They are the closest to what real users do (SDK or equivalent in another language, or the CLI). Test coverage should be maximised here.

The other test suites cover specific scenarios that are hard to test or debug from the SDK level, or things the SDK itself does not cover:

| Suite | File(s) | Role | When to add tests here |
|---|---|---|---|
| `test_sdk_*` | `sdk/tests/integration_tests.rs` | Primary end-to-end | New features and behaviours |
| `test_sdk_oauth2_*` | `sdk/tests/integration_tests.rs` | OAuth2-specific SDK paths | Hydra-specific scenarios: JWKS fetch, scope subset, audience |
| `test_server_*` | `server/tests/server_integration_tests.rs` | Server-level behaviour | TLS config, JWT validation edge cases, init path |
| `test_server_oauth2_*` | `server/tests/server_integration_tests.rs` | OAuth2-specific server paths | Scope subset, wrong audience, token reuse |
| `test_index_service_*` | `server/tests/index_service_integration_tests.rs` | Index service contract | Validation logic that is hard to verify from SDK level |
| `test_entry_service_*` | `server/tests/entry_service_integration_tests.rs` | Entry service contract | Kept slim; useful for debugging entry service in isolation |
| `test_datastore_*` | `datastore/tests/postgres_integration_tests.rs` | Datastore contract | SQL/migration correctness, datastore-specific invariants |
| `test_cli_*` | `cli/tests/` | CLI behaviour | Output formatting, argument parsing, CLI-level integration |

## Naming Convention

All integration test functions **MUST** be prefixed to indicate the layer under test:

- `test_sdk_` — SDK integration tests
- `test_sdk_oauth2_` — SDK tests specific to the OAuth2/JWKS path
- `test_server_` — server-level integration tests (gRPC, TLS, JWT)
- `test_server_oauth2_` — server tests specific to the OAuth2/Hydra path
- `test_index_service_` — index service handler tests
- `test_entry_service_` — entry service handler tests
- `test_datastore_` — datastore integration tests
- `test_cli_` — CLI integration tests

Test functions referencing the Ory Hydra implementation artifact **MUST NOT** use `hydra` in the function name — use `oauth2` instead. Utility functions that are Hydra-specific (e.g. `register_hydra_client`, `hydra_admin_url`) MAY retain the `hydra` name.

## Related Steel Threads

- All previous STs — this is a quality/hygiene thread, not a feature thread

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
