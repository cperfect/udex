---
verblock: "16 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Introduce PairsEnvelope and update write/read paths"
scope: Small
status: WIP
---

# WP-01: Introduce PairsEnvelope and update write/read paths

## Objective

Define a private `PairsEnvelope` struct in `udex-datastore` that wraps `Vec<KeyValuePair>`
with an `app_version: String` field. Update every place in `postgres.rs` that serialises or
deserialises `pairs` to go through this envelope instead of serialising the `Vec` directly.

## Deliverables

- `datastore/src/postgres.rs` (or a new private `datastore/src/pairs.rs` module) — define:

  ```rust
  #[derive(serde::Serialize, serde::Deserialize)]
  struct PairsEnvelope {
      app_version: String,
      pairs: Vec<KeyValuePair>,
  }
  ```

  with a constructor `PairsEnvelope::new(pairs)` that captures `env!("CARGO_PKG_VERSION")`.

- `datastore/src/postgres.rs` — update all `serde_json::to_value(&entry.context.pairs)`
  calls to serialise a `PairsEnvelope` instead (affects `create_entry`, `lookup_or_create`,
  and any other write paths).

- `datastore/src/postgres.rs` — update the read path (`row.try_get("pairs")` +
  `serde_json::from_value`) to deserialise a `PairsEnvelope` and extract `.pairs`.

- `datastore/tests/postgres_integration_tests.rs` — add a test that writes an entry and
  then queries the raw JSONB column to assert that the stored object contains an
  `app_version` key with a non-empty string value, proving the envelope is persisted.

## Acceptance Criteria

- [ ] All write paths store `{ "app_version": "...", "pairs": [...] }` in the `pairs` column
- [ ] All read paths deserialise the envelope and return `Vec<KeyValuePair>` to the caller unchanged
- [ ] `app_version` never appears in any type outside `udex-datastore`
- [ ] Integration test confirms the `app_version` field is present in the stored JSONB
- [ ] `cargo fmt --check`, `cargo clippy --all-targets`, `cargo test` all pass

## Dependencies

- None; WP-02 should follow this so the migration and docs reflect the final shape
