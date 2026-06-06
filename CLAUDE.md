# workspace

This project uses Intent v2.11.11. The primary config file for AI coding agents is `AGENTS.md` at the project root -- read that first. `CLAUDE.md` is a Claude Code-specific overlay that adds directives beyond the tool-agnostic contract.

## Required on every session

Run `/in-session` immediately after session start and after every `/compact` or context reset. It auto-detects the project language and loads the right skills (`/in-essentials`, `/in-standards`, plus language-specific). Rationale: `intent/docs/working-with-llms.md#skills-and-in-session-auto-load`.

## Persistent memory

Claude Code persists cross-session memories at `~/.claude/projects/<project-dir>/memory/`. Notes about user preferences, design decisions not derivable from code, and project context live there. See Claude Code's memory docs for management.

## Session hooks

`.claude/settings.json` wires Claude Code lifecycle hooks: `SessionStart` (inject project context + `/in-session` reminder), `UserPromptSubmit` (strict gate -- block first prompt until `/in-session` runs), `Stop` (remind `/in-finish` at wrap-up). Hook scripts live under `.claude/scripts/`. Full architecture: `intent/docs/working-with-llms.md#session-hook-architecture`.

## File map

- `AGENTS.md` -- primary tool-agnostic contract. Read first.
- `usage-rules.md` -- terse DO / NEVER rules (Elixir convention; honoured by `mix usage_rules.sync`).
- `intent/docs/working-with-llms.md` -- canon narrative on how AGENTS.md + CLAUDE.md + usage-rules.md + hooks + critics + skills compose.
- `intent/llm/MODULES.md` -- Highlander registry; check before creating new modules.
- `intent/llm/DECISION_TREE.md` -- code-placement flow chart.
- `intent/` -- steel threads (`st/`), project docs (`docs/`), work tracking (`wip.md`, `restart.md`).
- `intent/.config/` -- configuration and metadata.

## Rules of the road

Four cross-language principles govern all Intent projects:

- **Highlander** (`IN-AG-HIGHLANDER-001`) -- there can be only one; no divergent copies of the same concern.
- **PFIC** (`IN-AG-PFIC-001`) -- Pure-Functional-Idiomatic-Coordination; pattern match, pipe, tag, compose.
- **Thin Coordinator** (`IN-AG-THIN-COORD-001`) -- coordinators parse to call to render; business logic lives elsewhere.
- **No Silent Errors** (`IN-AG-NO-SILENT-001`) -- every failure surfaces; rescue-and-swallow is forbidden.

Rule files are served by the installed Intent tool, not vendored into this project -- read them with `intent claude rules show <id>` (`intent claude rules list` to enumerate, `--lang <lang>` to filter). The terse DO / NEVER contract for this project lives in `usage-rules.md`.

## Critic dispatch

Per-language rule enforcement via thin subagents that read the rule library at invocation:

```
Task(subagent_type="critic-<lang>", prompt="review <paths>")
Task(subagent_type="critic-<lang>", prompt="test-check <paths>")
```

`/in-review` auto-detects language and dispatches. Headless runner `bin/intent_critic` powers the pre-commit gate. Contract: `intent/docs/critics.md`.

## Project-specific

<!-- user:start -->
<!-- Author: vscode, created 2026-05-17. Add project-specific Claude directives below this line. Preserved across regeneration. -->

## Develop Guidelines
>These guidelines take precedence over intent or other external rules and guideline if there is a clash.

Full guidelines are in [CONTRIBUTING.md](CONTRIBUTING.md) and [projects/rust/CONTRIBUTING.md](projects/rust/CONTRIBUTING.md). Key rules to always apply:

### General — see [CONTRIBUTING.md](CONTRIBUTING.md)

- Think of the Next Guy: write for the person reading this under pressure at 2am.
- State is hard: minimise it; prefer stateless design.
- If it isn't tested it doesn't work.
- Interfaces MUST be developed schema first.
- Commits MUST follow Conventional Commits; Git workflow is Trunk Based.
- Minimise shell scripting.

### Architecture — see [CONTRIBUTING.md](CONTRIBUTING.md)

- Server MUST be stateless; all persistent state lives in the datastore.
- Datastore concerns (transactions, scaling, distribution) MUST be opaque to the application.
- Use internet/de-facto standards; minimise external dependencies (especially stateful ones).
- Datastores without TLS MUST NOT be supported.
- Configuration MUST NOT be mutated at runtime.

### Testing — see [CONTRIBUTING.md](CONTRIBUTING.md)

- Tests MUST be automated and reliable (flakey tests are broken tests).
- Prefer integration test coverage over unit tests (Test Diamond).

### Rust — see [projects/rust/CONTRIBUTING.md](projects/rust/CONTRIBUTING.md)

- Follow the Rust Style Guide (`rustfmt`) and Rust API Design Guidelines.
- Use `thiserror`; name error types with an `Error` suffix; never expose third-party error types.
- Before committing: `cargo fmt --check`, `cargo clippy`, `cargo test`.

### Markdown

- Every fenced code block MUST have a language identifier (e.g. ` ```rust `, ` ```bash `, ` ```yaml `, ` ```text `). Never write a bare ` ``` ` fence.

## Project structure
> Orientation map for AI agents — work in the narrowest project that fits the task to avoid context bloat. The canonical tree lives in [CONTRIBUTING.md](CONTRIBUTING.md) under "Repo Structure"; treat that as the source of truth if this drifts.

- `projects/protobuf/` — `.proto` definitions; **source of truth for all API types**. Changing an API starts here; it drives code generation for the server, SDK, and CLI.
- `projects/rust/` — Rust workspace. Dependency order is `api` → `datastore` / `server` / `sdk` → `cli`. See [projects/rust/CONTRIBUTING.md](projects/rust/CONTRIBUTING.md).
  - `api/` — `udex-api`: generated types, authz, hashing; no I/O.
  - `server/` — `udex-server`: gRPC handlers, authn, config, logging.
  - `datastore/` — `udex-datastore`: `Datastore` / `Migrator` traits + PostgreSQL impl.
  - `sdk/` — `udex-sdk`: client SDK.
  - `cli/` — `udex-cli`: CLI binary.
  - `test-utils/` — `udex-test-utils`: shared integration test fixtures (dev-only).
- `projects/compose/` — Docker Compose for local dev (PostgreSQL + Hydra).
- `projects/k8s/` — Helm chart and scripts for local k3d Kubernetes dev.
- `scripts/` — setup and diagnostics: `gen-env.sh`, `gen-keys-and-certs.sh`, `hydra-create-client.sh`, `dev-doctor.sh`.
- `docs/` — project documentation: `ARCHITECTURE.md`, `FAQ.md`, `SECRETS.md`.
- `intent/` — Intent project tracking (steel threads, work packages, LLM guidelines).

## Directives
-- Ignore [THOUGHTS.md](./THOUGHTS.md) unless specificially told otherwise
-- Use the intent wp commands to create/start/finish work packages 
-- When committing on intent Work Packages make sure the updates to the steel thread docs are committed with the changes to the work packages
-- When updating a binary dependency, generated fixture, or service dependency: also update `scripts/dev-doctor.sh` and the relevant docs. Ask the user whether the check should be an exact version or major-version-only before making the change.
-- Assume for integration tests that hydra is always running and tests that rely on hydra must never be skipped because - if they fail we need to fix something.

<!-- user:end -->

---

_Generated from `lib/templates/llm/_CLAUDE.md` on 2026-06-06 for Intent v2.11.11._
