---
verblock: "15 May 2026:v0.1: vscode - Initial version"
wp_id: WP-04
title: "Update docs (README, FAQ, deployment)"
scope: Small
status: Not Started
---

# WP-04: Update docs (README, FAQ, deployment)

## Objective

Update project documentation to reflect the new migration control behaviour so operators understand how to deploy safely and what to do when the server refuses to start due to a schema mismatch.

## Deliverables

- README: document `apply_migrations` config option and the startup version check; add a "Database migrations" section covering the intended deployment workflow
- FAQ: add entries for "How do I apply migrations?" and "Why is the server refusing to start with a schema mismatch error?"
- Any relevant deployment/ops docs updated to describe the `migrate check` / `migrate apply` CLI commands

## Acceptance Criteria

- [ ] README covers `apply_migrations` flag and expected startup behaviour
- [ ] FAQ covers the two new common operator questions
- [ ] CLI commands documented with example invocations

## Dependencies

- WP-02 (startup behaviour)
- WP-03 (CLI commands)
