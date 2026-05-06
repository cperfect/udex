---
verblock: "06 May 2026:v0.1: vscode - Initial version"
wp_id: WP-07
title: "Update docs and README data model"
scope: Small
status: Done
---

# WP-07: Update docs and README data model

## Objective

Update all documentation that describes the entry API or data model to reflect the 1:1 contract and the new `entry_context` table. This includes the datastore README (tables, indexes, ER diagram), `ARCHITECTURE.md`, and any other files that reference many-entries-per-context or the old two-table schema.

## Deliverables

- `projects/rust/datastore/README.md` data model section updated: replace `entry` + `context` tables with `entry_context`; update ER diagram
- `ARCHITECTURE.md` updated where it references many-entries-per-context or the old schema
- Any other docs referencing `lookup_keys_by_context` returning a list updated to reflect single-key return

## Acceptance Criteria

- [ ] `datastore/README.md` tables section describes only `index` and `entry_context`
- [ ] ER diagram updated to show `index ||--o{ entry_context : "scopes"` (1:1 context enforced by UNIQUE)
- [ ] No remaining references to a `context` table or `entry` table (other than in git history)
- [ ] No remaining docs stating `lookup_keys_by_context` returns a list

## Dependencies

- WP-03: Schema must be finalised before docs accurately describe it
- WP-04/WP-05: API behaviour must be settled before API docs are updated
