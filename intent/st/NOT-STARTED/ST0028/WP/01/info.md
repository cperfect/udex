---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Stand up OpenObserve beside ClickHouse"
scope: Small
status: Not Started
---

# WP-01: Stand up OpenObserve beside ClickHouse

## Objective

Add OpenObserve to the compose fixture and start feeding it, **without removing anything**. The collector dual-exports to both ClickHouse and OpenObserve, so the existing observability tests keep passing untouched while the new backend fills with identical telemetry. This is the strangler step that lets every later work package land green.

## Deliverables

- `openobserve` service in `projects/compose/docker-compose.yml`, image pinned to `public.ecr.aws/zinclabs/openobserve:v0.92.0`, configured with `ZO_LOCAL_MODE=true`, `ZO_LOCAL_MODE_STORAGE=disk`, `ZO_DATA_DIR`, and `ZO_TELEMETRY=false` (on by default -- a dev fixture must not phone home). Port 5080 published loopback-only, matching the ClickHouse posture. Healthcheck on `/healthz`. No volume: telemetry is ephemeral, exactly as ClickHouse is today.
- `otlphttp/openobserve` exporter added to all three collector pipelines alongside the existing `clickhouse` exporter.
- Vector's `clickhouse` sink replaced by an `opentelemetry` sink pointed at the collector's OTLP/HTTP receiver, so the log floor reaches both stores through the collector and the collector becomes the only backend-aware service.
- `scripts/gen-env.sh` generates `OPENOBSERVE_ROOT_EMAIL`, `OPENOBSERVE_ROOT_PASSWORD_SECRET` (must include punctuation -- OpenObserve rejects a trivial root password on first boot), and the pre-encoded `OPENOBSERVE_BASIC_AUTH_SECRET` consumed by the collector header and later by the tests.
- Retention set to the closest equivalent of the current `ttl: 72h` via `ZO_COMPACT_DATA_RETENTION_DAYS` (day-granular, so `3`).

## Implementation notes

All configuration stays **inline** (`configs:` content + env vars). ST0027's no-bind-mount rule is not stylistic: the base compose is consumed from two different project directories (`projects/compose/` standalone/CI and `.devcontainer/` via symlink), so any relative bind-mount path resolves to two different places and breaks in one of them. The spike hit exactly this when trying to mount a Vector config.

The Vector `opentelemetry` sink needs four things that are not obvious and were each a separate failure during the spike:

1. The full OTLP `resourceLogs` envelope hand-built in VRL -- the sink does no OTel mapping of its own.
2. `to_unix_timestamp!(.timestamp, unit: "nanoseconds")` -- the fallible form; the plain call fails to compile and the bare `.timestamp ?? now()` is rejected as unnecessary coalescing.
3. An explicit `Content-Type: application/json` request header, or the collector answers `415 Unsupported Media Type` and Vector drops the batch as non-retriable.
4. `batch.max_events: 1`, because the VRL builds one complete JSON document per event and batching would concatenate several into one invalid body.

Point 4 means one HTTP request per log line. That is acceptable for two low-volume dev containers but must carry a comment saying it is deliberate.

Keep ST0027's honest-severity decision: postgres and hydra both log to stderr regardless of level, so `stream` is not severity. Leave it unset rather than mislabel everything, and keep the original stream as a log attribute.

## Acceptance

Acceptance Criteria for this work package live in the steel thread's `acceptance.md`, under the `WP-01` heading (single source of truth). Do not restate ACs here.

## Dependencies

- None. This is the entry point for the thread.
- Reference: `FINDINGS.md` on branch `spike/openobserve-obs` for verified service config and the Vector sink shape.
