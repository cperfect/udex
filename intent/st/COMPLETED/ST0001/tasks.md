# Tasks - ST0001: Add structured logging

## Tasks

All tasks are tracked as work packages under `intent/st/ST0001/WP/`. Summary:

| WP    | Title                                              | Status   |
|-------|----------------------------------------------------|----------|
| WP-01 | Add tracing-subscriber init                        | Done     |
| WP-02 | Replace println! in production code                | Done     |
| WP-03 | Add request/response and error logging             | Done     |
| WP-04 | Add log capture to direct service tests            | Done     |
| WP-05 | init_tracing is never called in tests              | Done     |
| WP-06 | Guard init_tracing against duplicate init          | Done     |
| WP-07 | Add AI generation comment to logging.rs            | Done     |
| WP-08 | Move tower-http to workspace dependencies          | Done     |
| WP-09 | Move tracing-test to workspace dependencies        | Done     |
| WP-10 | Move test-only server deps to dev-dependencies     | Done     |
| WP-11 | Return opaque messages to gRPC clients             | Done     |
| WP-12 | Remove or document unused options                  | Done     |
| WP-13 | Review RUST_LOG=debug default                      | Done     |
| WP-14 | Remove duplicate JWT warning test                  | Done     |
| WP-15 | Remove dead or_else in validate_jwt                | Done     |
| WP-16 | Consider tracing::instrument on handlers           | Deferred |
| WP-17 | Review structured field usage                      | Done     |
| WP-18 | Enable optional log viewing in cargo tests         | Done     |

## Dependencies

WP-16 is deferred to the distributed tracing steel thread so that span design (field names,
PII exclusions, OTLP export) is handled consistently across the system.
