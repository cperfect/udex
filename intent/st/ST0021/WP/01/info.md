---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-01
title: "SDK: add health() method to UdexClient"
scope: Small
status: Done
---

# WP-01: SDK — add health() method to UdexClient

## Objective

Add a `health()` method to `UdexClient` in `projects/rust/sdk/` that calls `grpc.health.v1.Health/Check` and returns a typed result indicating whether the server is serving.

## Deliverables

- `projects/rust/sdk/src/health.rs` — thin wrapper around `tonic_health::proto::health_client::HealthClient`
- `UdexClient::health() -> Result<HealthStatus, Error>` on the existing client
- `HealthStatus` type representing `ServingStatus` values meaningfully
- `health` module exposed on the `udex-sdk` public surface in `lib.rs`

## Acceptance Criteria

- [ ] `client.health()` returns `Ok(HealthStatus::Serving)` against a live server
- [ ] Returns a typed error (not panic) when the server is unreachable
- [ ] Uses the existing channel — no separate connection or auth path

## Dependencies

- None — `tonic-health` is already a workspace dependency present in `sdk/Cargo.toml`
