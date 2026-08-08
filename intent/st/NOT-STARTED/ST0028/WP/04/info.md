---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Retire ClickHouse, HyperDX and Mongo; CI and dev-doctor"
scope: Small
status: Not Started
---

# WP-04: Retire ClickHouse, HyperDX and Mongo; CI and dev-doctor

## Objective

Delete the old backend now that nothing reads it, and update the pipeline and diagnostics that name it. This is where the simplification the thread exists for actually shows up: five fixture services become three.

## Deliverables

- Remove from `projects/compose/docker-compose.yml`: the `clickhouse` service (with its `ulimits` block and `CLICKHOUSE_DB` bootstrap), `hyperdx`, `mongo`, `hyperdx-init`, and the `clickhouse` exporter from all three collector pipelines. That also deletes the ~1.5KB inline `DEFAULT_SOURCES` JSON blob and the curl-based registration one-shot.
- `.github/workflows/01-Validation.yml`: update the two `docker compose up` service lists (currently `postgres hydra clickhouse otel-collector vector`) and replace `CLICKHOUSE_URL` with the OpenObserve equivalents in both jobs. The comment explaining why HyperDX/Mongo are omitted in CI becomes obsolete -- delete it rather than leave it describing services that no longer exist.
- `scripts/dev-doctor.sh`: add an OpenObserve reachability check. Note this is **net-new** -- dev-doctor currently has no observability checks at all, so there is nothing to edit. Per the project directive, ask whether the version check should be exact or major-version-only before writing it. The binary has no `--version` on the default `PATH`; invoke `/openobserve --version`.

## Implementation notes

Removal order matters: confirm WP02 and WP03 are green against OpenObserve **before** deleting ClickHouse, since the whole point of the strangler sequence is that this deletion is boring.

Record the actual CI startup time in `impl.md`. The thread assumes OpenObserve is lighter than ClickHouse + Mongo + HyperDX, and that assumption has not been measured -- CI is precisely where the fixture has to be reliable, so a number beats a belief. If it turns out slower, that is worth knowing before the thread closes rather than after.

Grep for stragglers once the services are gone: `clickhouse`, `hyperdx`, `8123`, and `8080` appear across compose, CI, scripts, tests and docs, and a stale port reference in a diagnostic script is exactly the kind of thing that survives a rename and wastes somebody's afternoon.

## Acceptance

Acceptance Criteria for this work package live in the steel thread's `acceptance.md`, under the `WP-04` heading (single source of truth). Do not restate ACs here.

## Dependencies

- WP-02 and WP-03 (both must be green against OpenObserve before the old backend is removed).
