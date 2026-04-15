---
verblock: "15 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Document local usage in repo README / devcontainer docs"
scope: Small
status: Done
---

# WP-04: Document local usage in repo README / devcontainer docs

## Objective

Ensure developers know how to run Trivy locally (inside and outside the devcontainer) and understand the blocking behaviour, without needing to read the CI config.

## Deliverables

- `README.md` — Security scanning section with the local run command, explanation of the MEDIUM+ blocking threshold, suppression instructions, and install steps for non-devcontainer users (macOS and Debian/Ubuntu)
- `README.md` — Security badge linking to the `02-Security` workflow

## Acceptance Criteria

- [x] `README.md` has a Security scanning section under Getting Started
- [x] Local run command (`trivy fs --config .trivy.yaml .`) documented
- [x] Blocking threshold (MEDIUM+) explained
- [x] `.trivyignore` suppression workflow explained
- [x] Install instructions provided for non-devcontainer developers
- [x] Security badge added to README header

## Dependencies

- WP-01, WP-02, WP-03
