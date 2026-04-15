---
verblock: "15 Apr 2026:v0.1: Matthew Sinclair - Initial version; 15 Apr 2026:v0.2: vscode - Updated with current state"
---

# Work In Progress

## Current Focus

**ST0002: Command Line Interface** — design complete, ready to implement

- Branch: `feat/cli`
- Design decisions resolved (D1–D7): single `udex` binary, clap derive, bearer token auth, TOML config, table/json/yaml output
- Testing strategy agreed: `assert_cmd` for offline, in-process server + `assert_cmd` for online (DATABASE_URL from devcontainer)
- 9 work packages defined in `intent/st/NOT-STARTED/ST0002/tasks.md`
- Next: start WP-01 (wire cli crate into workspace)

## Active Steel Threads

- ST0002: Command Line Interface — Not Started (design done, parked)

## Upcoming Work

- ST0002/WP-01: Re-enable `"cli"` in workspace Cargo.toml, add minimal `src/main.rs`
- ST0002/WP-02: clap command skeleton
- ST0002/WP-03: `udex config init` and `udex config validate`

## Notes

Two branches currently open:
- `chore/re-org-docs` — documentation reorganisation (CONTRIBUTING.md, ARCHITECTURE.md, README, CLAUDE.md, MODULES.md, DECISION_TREE.md). Ready to PR.
- `feat/cli` — ST0002 CLI steel thread. Design done, implementation not started.
