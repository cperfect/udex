---
verblock: "21 May 2026:v0.1: Chris Perfect - Initial version"
wp_id: WP-03
title: "Documentation and FAQ"
scope: Small
status: Done
---

# WP-03: Documentation and FAQ

## Objective

Document hash algorithm immutability in the protobuf README and provide a FAQ entry so operators and API consumers understand the design decision and know what to do if they need a different algorithm.

## Deliverables

- `projects/protobuf/README.md` — new bullet in the "Key design points" section: `Index.hash_algorithm` is immutable after creation; attempts to change it are rejected
- `projects/protobuf/FAQ.md` (new file) — FAQ entry: "Why can't I change an index's hash algorithm?" explaining the three failure modes (stale cached hashes, concurrent-write race, no cache invalidation path) and the recommended workaround (delete and recreate the index)

## Acceptance Criteria

- [ ] `projects/protobuf/README.md` key design points section includes the hash algorithm immutability rule
- [ ] `projects/protobuf/FAQ.md` exists with the FAQ entry covering all three failure modes and the workaround
- [ ] Prose is clear enough for an operator unfamiliar with the internals to understand why and what to do

## Dependencies

- Independent of WP01 and WP02; can be done in parallel or after.
