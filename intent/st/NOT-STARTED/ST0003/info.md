---
verblock: "15 Apr 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Not Started
slug: implement-security-scanning-of-repository
created: 20260415
completed:
---

# ST0003: Implement Security Scanning of repository

## Objective

Introduce automated security scanning using [Trivy](https://trivy.dev) to detect vulnerabilities, misconfigurations, and secret exposure across the repository. Scanning must be runnable locally (same config as CI) and must block GitHub PR merges when Trivy reports issues at WARN severity or higher.

## Context

The project has no automated security scanning today. As the codebase grows (Rust dependencies, Docker/container assets, GitHub Actions, TOML/YAML config), it needs a consistent baseline that:
- Surfaces CVEs in Cargo dependencies
- Detects secrets accidentally committed
- Flags misconfigured IaC/config files
- Runs identically locally and in CI (no "works on my machine" gap)

Trivy was chosen as the tool because it covers all of these scan types in a single binary, is widely adopted, and integrates natively with GitHub Actions.

## Related Steel Threads

- ST0002: Command Line Interface — the CLI ships Cargo dependencies that must be scanned

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
