---
verblock: "22 Apr 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Completed
slug: rfc-8693-scope-claim-for-permissions
created: 20260422
completed:
---

# ST0006: RFC 8693 scope claim for permissions

## Objective

Replace the custom `permissions` extra claim in JWTs with the standard RFC 8693
`scope` claim. Udex permissions (`udex:<action>`) are carried as a
space-separated list in `scope`; non-`udex:` scopes are silently ignored.

## Context

Currently the server expects permissions in a non-standard structure: an `extra`
map containing a `permissions` key whose value is a JSON array of
`udex:<action>` strings. This is bespoke and incompatible with standard OAuth
2.0 / OpenID Connect token issuers.

RFC 8693 §4.2 defines the `scope` claim as a space-delimited string of scope
values. Using it means:

- Any standards-compliant OAuth 2.0 / OIDC identity provider can issue tokens
  that Udex accepts without custom claim mapping.
- Tokens may carry unrelated scopes (e.g. `openid profile email`) alongside
  `udex:*` scopes — these must be silently discarded, not treated as an error.

## Scope

- Update `udex_api::authz::claims::Claims` to store `scope` as a raw `String`
  (defaulting to empty) instead of the `permissions` extra claim.
- Update permission extraction logic in `udex_api::authz::permissions` to split
  the raw `scope` string on whitespace and filter for only `udex:`-prefixed values;
  non-`udex:` entries are silently discarded there, not in `Claims`.
- Update `AuthnInterceptor` / permission evaluation to use the new claim.
- Update integration tests and JWT fixture generation to issue tokens with
  `scope` instead of `permissions`.
- Update CLI token-related commands / documentation if they reference the old
  claim format.

## Related Steel Threads

- ST0003 — original AuthNZ implementation (permissions claim origin)

## Acceptance Criteria

- [ ] `Claims` struct carries a `scope` field (space-delimited string, optional
      or empty-default).
- [ ] Permission extraction splits `scope` on whitespace and retains only values
      prefixed with `udex:`.
- [ ] Non-`udex:` scope values are silently discarded (no error, no warning).
- [ ] Existing integration tests pass with tokens carrying `scope`.
- [ ] A token with `scope = "openid profile email udex:entry:read"` grants only
      `udex:entry:read`.
- [ ] The old `permissions` extra-claim path is removed.
- [ ] `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, and all tests
      pass.
