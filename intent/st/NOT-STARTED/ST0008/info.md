---
verblock: "28 Apr 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: inject-keys-and-secrets
created: 20260428
completed:
---

# ST0008: Inject keys and secrets

## Objective

Ensure no keys or secrets are committed to the repository. Secrets are injected
at runtime via environment variables; key/cert files are generated locally and
never committed. A systematic naming convention and config-loading guard prevent
secrets from ever appearing in committed config files.

## Context

Several private keys, hardcoded passwords, and OAuth2 client secrets are
currently tracked in git (see `SECRETS.md`). This steel thread removes them,
establishes a durable injection pattern for both dev and prod, and wires up
developer tooling so the setup is frictionless.

Closely related public artefacts (usernames, client IDs, certificates, public
keys) follow the same management pattern so that all credential-adjacent material
is handled consistently.

## Scope

- Remove all committed private keys and hardcoded secrets from the repo
- Simple string secrets → environment variables; `.env` file for dev (gitignored)
- Key/cert files → generated locally by setup scripts; gitignored
- Public artefacts (certs, public keys, usernames, client IDs) → same flow,
  generated/configured alongside secrets
- `_secret` naming convention on config struct fields carrying secrets; config
  loader errors if a secret key appears in a TOML/config file
- CLI: secrets must not be accepted as command-line arguments (prevent `ps` leakage)
- Developer scripts: `scripts/gen-env.sh` and `scripts/gen-keys-and-certs.sh`
- devcontainer post-create script runs both automatically
- Update CONTRIBUTING.md, SECRETS.md, SECURITY.md

## Out of Scope

- Secrets manager integration (Vault, AWS Secrets Manager) — prod infra concern
- Rotation automation beyond the dev setup scripts

## Related Steel Threads

- ST0007: Auth server integration (introduces JWKS/OAuth2 config this thread secures)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
