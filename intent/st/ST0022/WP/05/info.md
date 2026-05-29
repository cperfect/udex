---
verblock: "29 May 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Docs"
scope: Small
status: Not Started
---

# WP-05: Docs

## Objective

Update all user-facing documentation to reflect the new JWKS refresh
behaviour and configuration surface. Remove the stale "fetched once at
startup" language and describe both refresh triggers, the three new config
fields, and the DoS controls.

## Deliverables

- `projects/rust/server/README.md` — rewrite the Authorization / JWKS section:
  - Replace "fetches the JWKS document once at startup" with a description
    of both refresh mechanisms (cache-miss and configured expiry)
  - Document the three new config fields with their defaults and example
    TOML snippets
  - Describe the DoS controls (max failed refreshes, backoff)
- `README.md` (root) — review and update the Authorization table row if the
  brief description no longer matches the implemented behaviour
- `intent/st/ST0022/impl.md` — as-built notes covering all WPs: data model,
  key decisions made during implementation, any deviations from the design
- `intent/st/ST0022/tasks.md` — mark all WPs complete

## Acceptance Criteria

- [ ] `projects/rust/server/README.md` contains no reference to "once at
      startup" for the JWKS path
- [ ] All three new config fields are documented with their defaults
- [ ] `impl.md` is complete and accurately reflects the as-built state
- [ ] No broken links or formatting errors in updated docs

## Dependencies

- WP02, WP03, WP04 (docs written against the finished implementation)
