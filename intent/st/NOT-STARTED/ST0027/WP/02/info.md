---
verblock: "30 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "HyperDX + Mongo dev UI"
scope: Small
status: Not Started
---

# WP-02: HyperDX + Mongo dev UI

## Objective

Add HyperDX (reader-only) and its MongoDB backing as always-on services so developers get a UI over the ClickHouse telemetry, without making tests or CI depend on it.

## Deliverables

- `hyperdx` + `mongo` services (pinned images) in the base compose stack.
- HyperDX pointed at ClickHouse with the datasource pre-provisioned (or scripted) so the UI is usable with minimal/no manual first-run setup.
- Readiness wiring such that HyperDX/Mongo do **not** gate the stack's "ready" signal used by tests/CI.

## Acceptance Criteria

- [ ] HyperDX renders traces/metrics/logs from ClickHouse for a generated request.
- [ ] Tests and CI pass whether or not HyperDX has completed setup (it never gates readiness).
- [ ] Images are pinned; no per-team ingestion key or OpAMP involved.

## Dependencies

- WP01 (ClickHouse must exist and hold queryable data; the schema decision affects HyperDX readability).
