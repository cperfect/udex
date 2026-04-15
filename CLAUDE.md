# workspace Project Guidelines

This is an Intent v2.8.0 project.

## Guidelines

Full guidelines are in [CONTRIBUTING.md](CONTRIBUTING.md) and [projects/rust/CONTRIBUTING.md](projects/rust/CONTRIBUTING.md). Key rules to always apply:

### General — see [CONTRIBUTING.md](CONTRIBUTING.md)

- Think of the Next Guy: write for the person reading this under pressure at 2am.
- State is hard: minimise it; prefer stateless design.
- If it isn't tested it doesn't work.
- Interfaces MUST be developed schema first.
- AI-generated files MUST have a comment on line 1 stating the tool and model.
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

### Project Conventions

- **The Highlander Rule**: Never duplicate code paths or logic for the same concern — check `intent/llm/MODULES.md` first.
- **Check before you create**: if a module owns a concern, use it; if creating a new one, register it in `MODULES.md` first.
- **No silent failures**: every error path must be handled explicitly.

## Project Structure

- `projects/rust/` - Rust workspace (three crates)
  - `api/` - `udex-api`: protobuf-generated types, authz, hashing — no I/O
  - `server/` - `udex-server`: gRPC handlers, authn, config, logging
  - `datastore/` - `udex-datastore`: `Datastore`/`Migrator` traits + PostgreSQL impl
- `intent/` - Project artifacts (steel threads, docs, work tracking)
  - `st/` - Steel threads organized as directories
  - `docs/` - Technical documentation
  - `llm/` - LLM-specific guidelines (MODULES.md, DECISION_TREE.md)
- `.intent/` - Configuration and metadata

## Key Reference Files

Read these on every session start and after every context reset:

- `CLAUDE.md` (this file) - Project rules and structure
- `intent/llm/MODULES.md` - Module registry (the Highlander enforcer)
- `intent/llm/DECISION_TREE.md` - Where does this code belong?
- `intent/wip.md` - Current work in progress
- `intent/restart.md` - Session restart context (if exists)

## Steel Threads

Steel threads are organized as directories under `intent/st/`:

- Each steel thread has its own directory (eg ST0001/)
- Minimum required file is `info.md` with metadata
- Optional files: design.md, impl.md, tasks.md

## Commands

### Core Commands

- `intent st new "Title"` - Create a new steel thread
- `intent st list` - List all steel threads
- `intent st show <id>` - Show steel thread details
- `intent wp new <STID> "Title"` - Create a new work package
- `intent wp list <STID>` - List work packages for a steel thread
- `intent wp start <STID/NN>` - Mark work package as WIP
- `intent wp done <STID/NN>` - Mark work package as Done
- `intent doctor` - Check configuration
- `intent help` - Get help

### Claude Commands

- `intent claude subagents <command>` - Manage Claude subagents
- `intent claude skills <command>` - Manage Claude skills
- `intent claude prime` - Synthesize project knowledge into MEMORY.md

## Session Workflow

### On session start

1. Read this file, MODULES.md, DECISION_TREE.md, wip.md, restart.md
2. Understand current state before making any changes
3. Ask clarifying questions if the task is ambiguous

### Before creating code

1. Check MODULES.md -- does a module already own this concern?
2. Check DECISION_TREE.md -- where does this code belong?
3. If creating a new module: register in MODULES.md first

### On session end

1. Update intent/wip.md with current state
2. Update intent/restart.md with context for next session
3. Commit with descriptive message

## Author

vscode

## Directives
-- Ignore [THOUGHTS.md](./THOUGHTS.md) unless specificially told otherwise
-- Use the intent wp commands to create/start/finish work packages 
-- When committing on intent Work Packages make sure the updates to the steel thread docs are committed with the changes to the work packages
