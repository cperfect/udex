---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
wp_id: WP-03
title: "New coverage: log floor and postgres receiver metric"
scope: Small
status: Done
---

# WP-03: New coverage: log floor and postgres receiver metric

## Objective

Close two coverage gaps ST0027 left open. Unlike WP02 this is genuinely net-new assertion, not a port.

Worth stating plainly, because it is easy to assume otherwise: **"do traces, metrics and logs from udex-server land?" is already tested** -- by `obs.rs` on the always-run path and by the three `test_obs_k8s_*` tests against the cluster. WP02 ports those. What follows is what is actually missing.

## Deliverables

- **Vector log floor assertion.** ST0027 built postgres/hydra container-log shipping and nothing asserts it arrives. Add an always-run test that drives traffic at hydra and asserts its container log reaches the store with `service_name = 'hydra'`, sitting in the same logs stream as `udex-server` application telemetry. Baseline-then-poll, run-scoped, matching the existing tests' shape so a stale record cannot satisfy it.
- **`postgresql.backends` on the always-run path.** The collector's `postgresqlreceiver` metric is asserted only in the k8s test today, so a receiver regression is invisible to anyone not running the cluster suite. Add a presence check to the non-k8s path. Presence (not increase) is the right assertion: the collector scrapes continuously, so there is no run-scoped increment to anchor to.

## Implementation notes

The floor test earns its place in this thread specifically because WP01 changes the floor's transport -- it stops being a direct sink write and becomes an OTLP hop through the collector, with a hand-built VRL envelope and one-event batching. That is exactly the kind of change that silently stops working, and until now nothing would have caught it. "If it isn't tested it doesn't work" applies with force here.

Watch the severity column: ST0027 deliberately leaves severity unset for floor records because postgres and hydra both log to stderr regardless of level, so stream is not severity. OpenObserve renders that as `"0"` rather than blank. Assert on `service_name` and body presence, **not** on severity, or the test will encode a value that is an artifact rather than a property.

Hydra is always running per project directive -- a hydra-dependent test must never skip. If it fails, something is broken and needs fixing.

## Acceptance

Acceptance Criteria for this work package live in the steel thread's `acceptance.md`, under the `WP-03` heading (single source of truth). Do not restate ACs here.

## Dependencies

- WP-02 (needs the `openobserve_*` helpers).
- WP-01 (needs Vector routing through the collector, which is what the floor test exercises).
