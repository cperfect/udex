---
verblock: "15 Apr 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: command-line-interface
created: 20260415
completed:
---

# ST0002: Command Line Interface

## Objective

Implement a `udex` CLI binary that supports server lifecycle management, configuration generation/validation, index admin operations, and operational use as a test client against a running Udex server.

## Context

The CLI was explicitly deferred in the architecture but is the intended mechanism for:
- Managing server configuration (generate, validate)
- Starting and stopping the server
- Admin operations on indices (create, update, delete, list) — currently only possible via static config
- Operational and test use against a live server (entry and index operations)

The architecture states Udex is not intended to be used directly by humans _except_ for specific admin operations — the CLI is that designed exception.

Open design questions to be resolved in design.md:
- Binary structure: standalone `udex` binary, or subcommands on the server binary?
- Authentication: how does the CLI obtain and present credentials to the server?
- Config format: what does the generated/validated config look like?
- Scope of index operations: admin-only, or full entry operations too?
- CLI framework choice (clap is the canonical Rust option)

## Related Steel Threads

- ST0001: Add structured logging — logging conventions the CLI should follow

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
