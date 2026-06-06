---
verblock: "06 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Update docs and add TOML to YAML migration note"
scope: Small
status: Not Started
---

# WP-04: Update docs and add TOML to YAML migration note

## Objective

Update all documentation to YAML config and provide a TOML→YAML migration note for existing deployments (breaking change).

## Deliverables

- Config snippets converted from TOML to YAML in `README.md`, `SECURITY.md`, `projects/k8s/README.md`, `projects/rust/server/README.md`, `projects/rust/cli/README.md`, `docs/FAQ.md` (`[datastore]`/`[server]` blocks, `config.toml`/`udex.toml` references).
- `docs/DESIGN_DECISIONS.md` "Why YAML for configuration (not TOML)?" entry confirmed in sync with the shipped state (the decision was recorded ahead of implementation; ensure no remaining "original TOML" wording reads as still-current).
- TOML→YAML migration note (field names unchanged; only syntax differs) — placed where deployment/upgrade guidance lives (e.g. README database-migrations / a CHANGELOG/UPGRADING note).
- `scripts/dev-doctor.sh` reviewed for config-file references and updated if needed (per the CLAUDE.md dependency-change directive); relevant docs updated alongside.

## Acceptance Criteria

- [ ] No remaining `config.toml` / `udex.toml` / `[datastore]` / `[server]` TOML references in non-`intent/` docs (grep-clean).
- [ ] Migration note exists and is linked from the appropriate doc.
- [ ] Every fenced config block uses a language identifier (`yaml`).

## Dependencies

- **WP-02** (final YAML shape known). Pairs with WP-03.
