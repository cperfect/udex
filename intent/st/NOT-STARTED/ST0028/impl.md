# Implementation - ST0028: OpenObserve as the dev observability backend

## Implementation

### WP-01 -- Stand up OpenObserve beside ClickHouse (complete)

Changed files: `projects/compose/docker-compose.yml`, `scripts/gen-env.sh`. No Rust source touched, as designed.

The fixture now runs both backends. The collector exports every pipeline to `clickhouse` and `otlphttp/openobserve` simultaneously, and Vector's log floor goes through the collector rather than writing to a store directly. Verified end to end: `obs.rs` still passes green against ClickHouse while the identical telemetry is queryable in OpenObserve.

## Technical Details

Live verification, all against the running fixture rather than inferred:

| Signal   | Observed in OpenObserve                                                                   |
| -------- | ----------------------------------------------------------------------------------------- |
| traces   | 8 `udex-server` span names incl. `db.create_entry`, `/udex.entry.v1.EntryService/CreateEntry` |
| metrics  | `udex_rpc_requests` + 19 `postgresql_*` receiver streams; counter query returned 3          |
| logs     | `udex-server` app logs plus `postgres` and `hydra` floor records in one stream               |

`GET /config` on the running service confirms `data_retention_days: 3` and `telemetry_enabled: false`, so the retention mapping from ClickHouse's `ttl: 72h` is applied rather than merely declared.

## Challenges & Solutions

**The OpenObserve image is fully distroless.** No shell, no curl, no wget, not even busybox -- probed directly rather than assumed. Compose healthchecks exec inside the container, so this service cannot have one at all, even though it does serve `/healthz`. Dependents gate on `service_started`; readiness is absorbed by the collector's `retry_on_failure` and by the tests' existing poll budgets. The one export failure observed at startup was exactly this race, and it self-healed on retry with no data loss. If a future image ships a shell, add the healthcheck and tighten the conditions.

**OpenObserve rejects hex passwords and panics on it.** The rule is 8-128 characters with at least one lowercase, one uppercase, one digit and one special character, and the failure mode is a hard panic on first boot (`ZO_ROOT_USER_PASSWORD is too weak`) -- the container exits rather than degrading. This collided with a documented constraint in `gen-env.sh`: its values are expanded inside an *unquoted* heredoc, and the file explicitly warns that anything outside `[0-9a-f]` risks silent shell expansion.

Resolved by keeping the value inside `[A-Za-z0-9-]`: a fixed `Ux1-` prefix supplies the four required character classes while all the entropy stays in the 48 hex characters that follow. That satisfies OpenObserve without reopening the heredoc-safety question, and also avoids `#`, which docker compose's `.env` parser can read as a comment. The reasoning is written into the script so the next person does not "improve" it into a random symbol soup.

A related trap caught during review rather than in production: composing the password inline as `"Ux1-$(openssl rand -hex 24)"` would yield a non-empty `"Ux1-"` if openssl failed, sailing straight past the existing emptiness guard. The entropy is now captured into its own variable so the guard can actually see it.

**Vector's `opentelemetry` sink rejected `healthcheck`.** Unknown field under `protocol`. Removed. This is the fourth distinct configuration wart in that sink, after the ones the spike found (hand-built OTLP envelope, `to_unix_timestamp!` fallible form, mandatory `Content-Type` or a 415). All are commented in the compose config.

**Vector did not recreate on a config-only change.** `docker compose up -d vector` reported it unchanged despite the inline `configs` content differing; `--force-recreate` was needed. Worth knowing for WP04's CI work -- a pipeline that relies on `up -d` picking up a config edit could silently run the old config.

## Notes for later work packages

- The `.env` on this machine was appended to by hand rather than regenerated, because the rotation guard correctly refuses to rewrite it while a compose Postgres is live. A clean clone gets the same values from `gen-env.sh`; this only affects existing developer machines, and belongs in the WP05 upgrade note.
- Both backends are running, so the fixture is temporarily *heavier*, not lighter. The simplification only materialises at WP04. Anyone measuring CI footprint before then will get a misleading number.
