---
verblock: "18 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Dockerfile for udex-cli"
scope: Small
status: Done
---

# WP-01: Dockerfile for udex-cli

## Objective

Produce a multi-stage Dockerfile at `projects/rust/cli/Dockerfile` that builds the `udex` binary and packages it into a minimal runtime image. The image is the deployable unit for the k8s environment.

## Deliverables

- `projects/rust/cli/Dockerfile`

## Acceptance Criteria

- [ ] `docker build -f projects/rust/cli/Dockerfile projects/ -t udex:latest` succeeds (release build)
- [ ] `docker build -f projects/rust/cli/Dockerfile projects/ -t udex:dev --build-arg PROFILE=dev` succeeds (dev build)
- [ ] `docker run --rm udex:latest --help` exits cleanly (binary is functional)
- [ ] Runtime image contains only the binary and ca-certificates (no Rust toolchain, no protoc)

## Dependencies

- None
