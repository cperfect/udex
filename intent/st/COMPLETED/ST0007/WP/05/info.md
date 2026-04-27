---
verblock: "27 Apr 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Add token fetch subcommand to CLI: client_credentials flow with decoded output"
scope: Small
status: Done
---

# WP-05: Add token fetch subcommand to CLI: client_credentials flow with decoded output

## Objective

Add `udex token fetch` to the CLI — a command that performs an OAuth2
`client_credentials` token exchange, then prints both the raw (encoded) JWT
and the decoded header + claims, respecting `--output` (table/json/yaml).

## Deliverables

- `TokenFetchArgs` and `TokenCommands::Fetch` in `cli.rs`
- `commands::token::fetch` async handler in `commands/token.rs`
- `oauth2 = { version = "5", features = ["reqwest"] }` added to `cli/Cargo.toml`
- MODULES.md updated with CLI OAuth2 token-fetch concern
- `udex-cli` section added to MODULES.md

## CLI interface

```text
udex token fetch \
  --client-id <ID>       [env: UDEX_CLIENT_ID]
  --client-secret <SEC>  [env: UDEX_CLIENT_SECRET]
  --url <TOKEN_URL>      [env: UDEX_TOKEN_URL]   # full token endpoint URL
  --scope <SCOPE>        # repeatable; optional
```

Default (table) output:
```text
=== Token (encoded) ===
eyJ...

=== Token (decoded) ===
--- Header ---
{ "alg": "ES256", ... }

--- Claims ---
{ "sub": "...", "scope": "...", ... }

exp: valid for 3598 more seconds (2026-04-27 11:00:00 +00:00:00)
```

JSON output: `{ "token": "...", "header": {...}, "claims": {...} }`

## Acceptance Criteria

- [x] `udex token fetch --client-id … --client-secret … --url …` obtains a
      token from a live Hydra instance and prints it.
- [x] Both the raw encoded token and the decoded claims are shown by default.
- [x] `--output json` emits a single JSON object with `token`, `header`, and
      `claims` keys.
- [x] `--output yaml` emits equivalent YAML.
- [x] Repeated `--scope` args are each added to the token request.
- [x] `UDEX_CLIENT_ID`, `UDEX_CLIENT_SECRET`, and `UDEX_TOKEN_URL` env vars
      are honoured.
- [x] If the server returns a non-JWT (opaque) token, a clear message is
      printed and the raw token is still shown.
- [x] `cargo fmt --check`, `cargo clippy --all-targets -- -D warnings`, and
      `cargo test -p udex-cli` pass.

## Dependencies

Depends on WP-01 through WP-03 being complete (they are).
