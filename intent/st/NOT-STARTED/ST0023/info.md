---
verblock: "06 Jun 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: move-to-yaml-config
created: 20260606
completed:
---

# ST0023: Move to yaml config

## Objective

Replace the TOML configuration format with YAML across Udex — **a replacement, not an addition**. After this thread there is one config format (YAML); TOML config support is removed, not kept alongside.

## Context

Udex configuration is currently TOML (`udex.toml`), loaded by the CLI:

- `toml` crate dependency (workspace dep; used in `projects/rust/cli/Cargo.toml`).
- Config types and loading in `projects/rust/cli/src/config.rs` (`UdexConfig` → `ServerConfig` / `DatastoreConfig`), with secret binding via `secrets-rs` `FileSource`.
- CLI defaults `udex.toml` and the `UDEX_CONFIG` env var (`projects/rust/cli/src/cli.rs:97,115,123`).
- TOML config rendered/consumed in the deploy path — the k8s Helm `configmap.yaml` renders a server `config.toml`, and docs reference `[datastore]`/`config.toml` (FAQ, server/README, README database-migrations).

**Why YAML:** it is effectively a universal config standard and the default in Kubernetes land, where Udex is deployed (Helm/k8s). A single YAML format removes the TOML-vs-YAML mismatch between the app config and its surrounding k8s manifests.

Scope to confirm during design:
- Swap the parser (`toml` → a YAML crate, e.g. `serde_yaml`) while keeping the serde-derived config types.
- Confirm `secrets-rs` `FileSource` supports YAML (or adapt the secret-URN binding).
- Rename defaults `udex.toml` → `udex.yaml` and update `UDEX_CONFIG` default.
- Update Helm chart (`configmap.yaml` → render YAML), example configs, and all docs that show TOML config.
- Update `scripts/dev-doctor.sh` / fixtures if any reference config files (per CLAUDE.md dependency-change directive).
- Migration note for existing deployments (TOML → YAML) since this is a breaking change.

## Related Steel Threads

- [List any related steel threads here]

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
