# Implementation - ST0023: Move to yaml config

## Implementation

### WP-01 — Adopt serde-saphyr, retire serde_yaml (done)

As-built:

- Workspace dep `serde_yaml = "0.9"` replaced with `serde-saphyr = "0.0.27"` (`projects/rust/Cargo.toml`). `0.0.x` is an exact pin under Cargo semantics.
- `serde_yaml` was a dead dependency in `udex-datastore` and `udex-server` (declared, never used) — both lines removed outright.
- `udex-cli` is the only real user: the seven `serde_yaml::to_string(&x)?` call sites for `-o yaml` output (`commands/index.rs` ×2, `entry.rs` ×4, `token.rs` ×1) became `serde_saphyr::to_string(&x)?`. Same by-reference signature and `?`-into-`anyhow` error conversion — no other changes needed.
- **No `SerializerOptions` required:** `serde_saphyr::to_string` default output is already clean/idiomatic (unquoted keys, block nesting, `- item` sequences). Verified with a throwaway unit test serializing a representative nested `serde_json::Value`; the test was removed after confirming the format.
- Verified: `cargo fmt --check`, `cargo clippy --all-targets`, and the full suite via `scripts/validate-test-rust.sh` (all unit + Postgres + Hydra/OAuth2 integration tests pass). `serde_yaml` gone from `Cargo.lock`.

## Code Examples

[Key code snippets and examples]

## Technical Details

[Specific technical details and considerations]

## Challenges & Solutions

[Challenges encountered during implementation and how they were resolved]
