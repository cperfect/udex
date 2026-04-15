---
verblock: "15 Apr 2026:v0.1: vscode - Initial restart context"
---

# Session Restart Context

## Branch: `feat/cli`

### What was done

ST0002 (Command Line Interface) steel thread created and fully designed:

- `intent/st/NOT-STARTED/ST0002/info.md` — objective, context, open questions
- `intent/st/NOT-STARTED/ST0002/design.md` — all design decisions resolved (D1–D7) + testing strategy
- `intent/st/NOT-STARTED/ST0002/tasks.md` — 9 work packages with dependency order

### Where to resume

WP-01: Re-enable `"cli"` in `projects/rust/Cargo.toml` (the `# "cli"` line is already there, commented out). Add a minimal `src/main.rs`. Add `assert_cmd` and `predicates` to `[dev-dependencies]` in `projects/rust/cli/Cargo.toml`. Verify `cargo build` passes.

The cli crate scaffold already exists at `projects/rust/cli/` with a `Cargo.toml` (clap, tabled, reqwest, tonic already listed as deps) and a `README.md`. No `src/` yet.

### Key design decisions to remember

- Single `udex` binary (`[[bin]] name = "udex"` already in cli/Cargo.toml)
- Bearer token via `--token` flag or `UDEX_TOKEN` env var (no OAuth flow in v1)
- TOML config; `ServerConfig`/`DatastoreConfig` already serialisable
- Test pattern: in-process server + `assert_cmd` (same as `server_integration_tests.rs`); `DATABASE_URL` from devcontainer

### Also open: `chore/re-org-docs`

Documentation reorganisation branch — CONTRIBUTING.md, ARCHITECTURE.md, README, CLAUDE.md, MODULES.md, DECISION_TREE.md all updated. Ready to PR against main.
