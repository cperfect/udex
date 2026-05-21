---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-03
title: "Tests and docs"
scope: Small
status: Done
---

# WP-03: Tests and docs

## Objective

Add integration test coverage for the SDK health method and CLI health command, and update all affected documentation.

## Deliverables

### Tests
- SDK integration test: `test_sdk_health_serving` — call `client.health()` against a live test server, assert SERVING
- CLI integration test: `udex health` exits 0 and prints SERVING against a live server

### Docs
- `intent/llm/MODULES.md` — add `udex_sdk::health` and `udex_cli::commands::health` entries
- Any SDK doc comments or README referencing available operations

## Acceptance Criteria

- [ ] SDK integration test passes against both JWT and OAuth2 fixtures
- [ ] CLI integration test passes
- [ ] MODULES.md updated with new modules

## Dependencies

- WP01, WP02
