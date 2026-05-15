---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Documentation"
scope: Small
status: Not Started
---

# WP-05: Documentation

## Objective

Update user-facing documentation to describe `LookupKeyByContextOrCreate`, its use case, and how to use it from the CLI.

## Deliverables

- `README.md`: mention `LookupKeyByContextOrCreate` in the entry operations section; note it requires write permission.
- `docs/FAQ.md`: add "When should I use `lookup-or-create` instead of `lookup` + `create`?" FAQ entry explaining the Id Permanence use case and the read-before-write it avoids.

## Acceptance Criteria

- [ ] Documentation is clear, consistent with existing style, and all code blocks have language identifiers.
- [ ] The FAQ explains both the use case (Id Permanence) and the permission requirement (write).

## Dependencies

- WP-04 must be complete (CLI command name/flags must be final before writing docs).
