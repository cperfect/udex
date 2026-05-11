---
verblock: "08 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Core client struct, TLS, and connection management"
scope: Small
status: Done
---

# WP-02: Core client struct, TLS, and connection management

## Objective

Implement `UdexClient` and `ClientOptions` with TLS configuration and gRPC channel construction so callers can establish a secure connection to the server.

## Deliverables

- `ClientOptions` / builder: endpoint URL, TLS PEM/path, optional per-call timeout
- `UdexClient::connect(opts: ClientOptions) -> Result<Self, Error>` — builds tonic channel with TLS
- `sdk/src/error.rs` with `thiserror`-based `Error` type
- All public items documented with rustdoc

## Acceptance Criteria

- [ ] `UdexClient::connect` can reach the compose-stack server
- [ ] Connecting with an invalid CA cert returns a typed error (not panic)
- [ ] `cargo doc -p udex-sdk --no-deps` produces no warnings

## Dependencies

- WP-01
