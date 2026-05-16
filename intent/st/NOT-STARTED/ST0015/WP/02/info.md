---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-02
title: "Update migration and datastore docs"
scope: Small
status: Not Started
---

# WP-02: Update migration and datastore docs

## Objective

Update the existing migration file to reflect the new JSONB envelope structure, and update
`datastore/README.md` to document that the `pairs` column stores a versioned envelope rather
than a bare array.

## Deliverables

- `datastore/migrations/postgres/01_initial_schema.sql` — update the inline comment on the
  `pairs` column to describe the envelope format:
  `-- { "app_version": "<semver>", "pairs": [KeyValuePair…] }`

- `datastore/README.md` — update the Data Model section:
  - The `pairs` column description in the `entry_context` table should document the envelope
    structure and explain that `app_version` is the `udex-datastore` crate version at write
    time, present to support future migration authoring.
  - Add a note explaining that `app_version` does not correspond directly to a breaking
    change in the pairs schema — it is purely a version marker.
  - Update the Mermaid diagram comment for the `pairs` field accordingly.

## Acceptance Criteria

- [ ] Migration file comment accurately describes the JSONB envelope
- [ ] README Data Model section documents the envelope and `app_version` semantics
- [ ] README is clear that `app_version` is an internal datastore concern, not exposed to the API

## Dependencies

- WP-01 must be complete so the envelope structure is finalised before docs are written
