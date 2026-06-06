# Implementation - ST0023: Move to yaml config

## Implementation

### WP-01 — Adopt serde-saphyr, retire serde_yaml (done)

As-built:

- Workspace dep `serde_yaml = "0.9"` replaced with `serde-saphyr = "0.0.27"` (`projects/rust/Cargo.toml`). `0.0.x` is an exact pin under Cargo semantics.
- `serde_yaml` was a dead dependency in `udex-datastore` and `udex-server` (declared, never used) — both lines removed outright.
- `udex-cli` is the only real user: the seven `serde_yaml::to_string(&x)?` call sites for `-o yaml` output (`commands/index.rs` ×2, `entry.rs` ×4, `token.rs` ×1) became `serde_saphyr::to_string(&x)?`. Same by-reference signature and `?`-into-`anyhow` error conversion — no other changes needed.
- **No `SerializerOptions` required:** `serde_saphyr::to_string` default output is already clean/idiomatic (unquoted keys, block nesting, `- item` sequences). Verified with a throwaway unit test serializing a representative nested `serde_json::Value`; the test was removed after confirming the format.
- Verified: `cargo fmt --check`, `cargo clippy --all-targets`, and the full suite via `scripts/validate-test-rust.sh` (all unit + Postgres + Hydra/OAuth2 integration tests pass). `serde_yaml` gone from `Cargo.lock`.

### WP-02 — Convert config parsing from TOML to YAML (done)

As-built:

- Both parse seams swapped `toml::from_str` → `serde_saphyr::from_str`: `UdexConfig::load` and `load_datastore_config`'s `Wrapper` (`cli/src/config.rs`). No config struct/field changes; `secrets-rs` URN binding is unchanged.
- Default config path renamed `udex.toml` → `udex.yaml` (all four `default_value` sites in `cli/src/cli.rs`).
- `udex config init` template (`commands/config.rs`, `CONFIG_TEMPLATE`) rewritten from commented TOML to commented YAML — this was an extra serialization seam beyond the design's list (the template is a hand-authored string, not `toml::to_string`).
- `toml` dependency removed from `cli/Cargo.toml` and the workspace `Cargo.toml` (`config.rs` was its only user).
- Tests ported to YAML: the `config.rs` unit tests (`test_config_serializes_without_panicking`, `test_plain_url_rejected_by_deserializer`, `test_valid_urn_accepted_by_deserializer`) plus the **integration** tests that write config bodies — `cli/tests/config_tests.rs`, `serve_tests.rs`, `serve_live_tests.rs` (TOML bodies → YAML, `udex.toml` → `udex.yaml`, invalid-syntax fixtures → invalid YAML, `..._invalid_toml`/`..._udex_toml` test names updated).
- Note: `server/tests/server_integration_tests.rs` build config structs directly in Rust (no file parsing) and are unaffected; one flaky port-binding blip on those fixed-address tests under back-to-back full-suite load passed cleanly in isolation (pre-existing, unrelated to this WP).
- Verified: `cargo fmt --check`, `cargo clippy --all-targets`, full suite via `scripts/validate-test-rust.sh` green; no `toml` / `udex.toml` references remain in the Rust tree.

### WP-03 — Update k8s deploy path to config.yaml (done)

As-built:

- Helm `configmap.yaml` rewritten: `data` key `config.toml` → `config.yaml`, body from TOML `[tables]` to nested YAML mappings (`server` → `tls`/`authz`, `datastore`). All `urn:secrets-rs:…` references and Go-template value interpolation preserved.
- `deployment.yaml`: mount `mountPath`/`subPath` and the ConfigMap-volume/`DATABASE_URL` comments updated `config.toml` → `config.yaml`.
- **`projects/rust/cli/Dockerfile` `ENTRYPOINT`** changed `--config /etc/udex/config.toml` → `…/config.yaml` — a third path seam beyond the WP deliverable list; without it the container would look for the removed `config.toml`.
- Verified: `bash scripts/validate-lint-helm.sh` passes; rendered `config.yaml` parses as valid YAML with the correct struct shape (`helm template` + parse check). **Full k3d e2e** (image-build → image-load → deploy → `scripts/validate-k8s-test.sh`): deployment rolled out successfully (pod loaded the mounted `config.yaml`) and all 6 `test_sdk_k8s_*` SDK tests passed over TLS.

## Code Examples

[Key code snippets and examples]

## Technical Details

[Specific technical details and considerations]

## Challenges & Solutions

[Challenges encountered during implementation and how they were resolved]
