---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
intent_version: 2.4.0
status: Completed
slug: grpc-health-check-support-in-sdk-and-cli
created: 20260521
completed: 20260521
---

# ST0021: gRPC Health Check Support in SDK and CLI

## Objective

Expose the standard `grpc.health.v1.Health` service to SDK users and CLI operators so that health can be checked programmatically or interactively without relying on external tooling.

## Context

The Udex server already registers the standard gRPC health protocol via `tonic-health` (added in ST0019). However neither the SDK nor the CLI expose this to callers.

**SDK rationale**: cannot predict all client workflows — a caller may want to verify server health before a bulk write or as part of a startup readiness check.

**CLI rationale**: operators debugging connectivity have no way to probe a gRPC-only server without specialised tools (`grpc_health_probe`). A `udex health` subcommand fills that gap.

Health is unauthenticated — the CLI command must not require a JWT or OAuth2 config.

## Related Steel Threads

- ST0019: gRPC standard health check migration (server-side implementation)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
