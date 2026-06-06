# AGENTS.md

This is the primary tool-agnostic config file for AI coding agents working on this project. Every major agentic CLI (Claude Code, Codex, Cursor, Copilot, Aider, Continue, Cline, Gemini CLI) reads AGENTS.md as the canonical project contract. Spec: https://agents.md.

## Project Overview

workspace -- an Intent project. See `CLAUDE.md` for Claude-specific overlay.

## Development Environment

### Prerequisites

- Bash 4.0+, POSIX-compliant shell

### Setup

```bash
intent doctor     # verify Intent configuration
```

## Build and Test Commands

### Testing

_No automated tests configured yet._

### Building

_No build step required._

### Validation

```bash
intent doctor       # check Intent configuration
intent st list      # list steel threads
```

## Code Style

- Shell scripts: 2-space indentation, POSIX-compliant where practical.
- Markdown: no manual line wrapping; verblock frontmatter on persistent docs.
- See `usage-rules.md` for the terse "DO / NEVER" contract.
- See `intent/docs/working-with-llms.md` for the canon tech note on the LLM-facing layout and how hooks + critics + skills compose.

## Steel Thread + Work Package Process

Work is organised into numbered **steel threads** (`ST####`), each containing **work packages** (`WP`). Every steel thread has its own directory under `intent/st/<ID>/` with `info.md` as the entry point.

```bash
intent st list               # list steel threads
intent st new "Title"        # create new
intent st show <id>          # inspect
intent wp list <STID>        # list work packages
intent wp new <STID> "..."   # create work package
intent wp start <STID/NN>    # mark WIP
intent wp done <STID/NN>     # mark done
```

Never create steel thread or work package directories by hand -- always use the CLI.

## Installed Skills

- **review-code** -- Launch the code-reviewer agent. Optional argument controls scope: a PR number (#123), branch name, commit SHA, file path/glob, or * for the full project. Defaults to current branch vs main, or HEAD commit if on main.
- **rust-guide** -- Understands the Rust programming language, idiomatic Rust and the Udex Rust Development Guidelines.
- **trivy-triage** -- Run a Trivy security scan, compile an ordered list of findings, then triage each one interactively: fetch advisory details, analyse exploitability in context, propose a fix or suppression, and commit after user approval. Stops after each finding for review.

## Installed Subagents

- **code-reviewer** -- Use this agent when code has been written or modified and needs to be reviewed for bugs, style violations, deviations from project guidelines, or general quality issues. This agent should be invoked after completing a logical unit of work such as a new feature, bug fix, or refactor.\\n\\n<example>\\nContext: The user has just written a new code function implementing a datastore query.\\nuser: \"I've implemented the new context lookup function in the datastore crate\"\\nassistant: \"Great! Let me use the code-reviewer agent to review the implementation.\"\\n<commentary>\\nSince new code was written, launch the code-reviewer agent to check for bugs, style issues, and adherence to the Udex development guidelines and code style guide.\\n</commentary>\\n</example>\\n\\n<example>\\nContext: The user has added a new gRPC endpoint handler in the core crate.\\nuser: \"I've added the bulk entry creation handler to the server\"\\nassistant: \"I'll now invoke the code-reviewer agent to review the new handler for correctness and guideline compliance.\"\\n<commentary>\\nA significant piece of code was added. Use the code-reviewer agent to review it before considering the task complete.\\n</commentary>\\n</example>\\n\\n<example>\\nContext: The user has refactored the CLI command structure.\\nuser: \"Can you review the changes I made to the CLI?\"\\nassistant: \"Absolutely. I'll use the code-reviewer agent to perform a thorough review.\"\\n<commentary>\\nUser explicitly requested a code review. Use the code-reviewer agent.\\n</commentary>\\n</example>

## Critic Family

Per-language rule enforcement via thin subagents that read the rule library at invocation time. Available: `critic-elixir`, `critic-rust`, `critic-swift`, `critic-lua`, `critic-shell`. Invoke:

```
Task(subagent_type="critic-<lang>", prompt="review <targets>")
```

A headless runner at `bin/intent_critic` (Greppable-proxy rules only; no LLM required) powers the pre-commit gate. Contract: `intent/docs/critics.md`. Exit codes: `0` clean, `1` findings, `2` error.

## Rule Library

The coding-rule library is served by the installed Intent tool, not vendored into this project. Access rules through the CLI below -- each rule carries YAML frontmatter, a Detection heuristic, and bad/good examples. Skills cite rule IDs; critics enforce them.

```bash
intent claude rules list        # enumerate
intent claude rules show <id>   # inspect
intent claude rules validate    # schema check
```

## Extensions

User extensions live at `~/.intent/ext/<name>/` and contribute subagents, skills, or rule packs without modifying canon. Reference extension: `worker-bee`.

```bash
intent ext list
intent ext show <name>
intent ext new <name>
```

Authoring guide: `intent/docs/writing-extensions.md`.

## Session Hooks

`.claude/settings.json` wires three Claude Code lifecycle hooks: **SessionStart** (inject context + `/in-session` reminder), **UserPromptSubmit** (strict gate -- block first prompt until `/in-session` runs), **Stop** (remind `/in-finish` at wrap-up). Full architecture: `intent/docs/working-with-llms.md#session-hook-architecture`.

## Socrates vs Diogenes FAQ

Two distinct subagents for two distinct concerns:

- **Socrates** -- CTO Review Mode. Architectural and strategic technical decision-making via Socratic dialog.
- **Diogenes** -- Elixir Test Architect. Test-specification generation and gap analysis.

They have never been the same agent. FAQ + forensic detail: `intent/docs/working-with-llms.md#socrates-vs-diogenes-faq`.

## Testing Instructions

1. Run the test suite before making changes -- establish a baseline.
2. Make changes.
3. Run the suite again; confirm no regressions.
4. Add new tests for new functionality.
5. Before commit, the pre-commit critic gate checks staged files for `CRITICAL` + `WARNING` findings. See `intent/docs/pre-commit-hook.md`.

## Commit Guidelines

- Descriptive commit messages; reference steel thread or work package IDs where applicable.
- No AI attribution in commit messages (no `Co-Authored-By`, no tool signatures).
- Small, coherent commits preferred over large batches.

## Security Considerations

- Never commit secrets or credentials.
- Review external inputs for injection-class bugs.
- Follow the project's security policy if one exists.

## Additional Resources

- `intent/docs/working-with-llms.md` -- canon tech note (narrative).
- `intent/docs/critics.md` -- critic contract.
- `intent/docs/rules.md` -- rule library authoring guide.
- `intent/docs/writing-extensions.md` -- extension author guide.
- `intent/docs/pre-commit-hook.md` -- pre-commit critic gate install + configure.
- `intent/docs/migration-v2.10.0.md` -- v2.9.0 -> v2.10.0 migration guide (directory move + recovery).
- `intent/llm/MODULES.md` -- module registry (Highlander enforcement).
- `CLAUDE.md` -- Claude-specific overlay.
- `usage-rules.md` -- DO / NEVER contract for this project.

---

_Generated by Intent v2.11.11 on 2026-06-06_
