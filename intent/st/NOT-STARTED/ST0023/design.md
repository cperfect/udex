# Design - ST0023: Move to yaml config

## Approach

The config types (`UdexConfig` → `ServerConfig` / `DatastoreConfig`) in `projects/rust/cli/src/config.rs` are plain serde-derived structs and are **format-agnostic**. The only TOML-coupled points are the parse calls; everything downstream (validation, secret binding, the server handoff) operates on the deserialized struct. This makes the migration a narrow, low-risk swap.

This thread also retires the archived `serde_yaml` (0.9, dtolnay, unmaintained) in favour of `serde-saphyr`, so YAML is handled by a single, maintained library across the whole project. That work is isolated in its own work package (see `tasks.md`).

Planned steps:

1. **Adopt `serde-saphyr`; retire `serde_yaml` (own WP).** Add `serde-saphyr` to the workspace. Migrate the existing `-o yaml` CLI output — `serde_yaml::to_string` in `commands/index.rs`, `entry.rs`, `token.rs` — to `serde_saphyr::to_string` (tuning `SerializerOptions` for clean, idiomatic output). Remove the `serde_yaml` workspace dependency (`projects/rust/Cargo.toml:44`) and the per-crate uses (`cli`, `datastore`, `server`). This WP is self-contained and a prerequisite for the config swap.
2. **Swap the config parser.** Replace `toml::from_str` with `serde_saphyr::from_str` at the two load seams: `UdexConfig::load` (`config.rs:115`) and the datastore-only `Wrapper` load (`config.rs:276`). No struct/field changes — YAML keys are the existing snake_case field names, and TOML tables (`[server]`, `[server.tls]`, `[datastore]`) become the corresponding nested YAML mappings serde already expects.
3. **Secret binding is untouched.** `secrets-rs` binds secrets from URN strings (`urn:secrets-rs:file:…`, `urn:secrets-rs:env:…`) *after* deserialization, via `SourceRegistry` / `FileSource` — it has no dependency on the config file format. The URNs stay byte-for-byte identical.
4. **Rename the default config.** `udex.toml` → `udex.yaml` and the `UDEX_CONFIG` env default at `cli.rs:97,115,123`.
5. **Update tests.** The config round-trip and parse tests (`config.rs:473–543`) currently use `toml::to_string_pretty` / `toml::from_str`; port them to `serde_saphyr`.
6. **Drop the TOML dependency.** `config.rs` is the sole user of the `toml` crate, so remove it from `projects/rust/cli/Cargo.toml:26` and the workspace `Cargo.toml:58` (replacement, not addition).
7. **Rewrite the deploy path.** Helm `configmap.yaml` renders the server config as TOML (`data: config.toml`); rewrite it as `config.yaml` (nested mappings instead of `[tables]`), and update `deployment.yaml` mount paths (`/etc/udex/config.toml` → `…/config.yaml`, `subPath`, comments at lines 66–109).
8. **Update docs & examples.** `README.md`, `SECURITY.md`, `projects/k8s/README.md`, `projects/rust/server/README.md`, `projects/rust/cli/README.md`, `docs/FAQ.md` reference `config.toml` / `udex.toml` / `[datastore]` blocks. Convert all config snippets to YAML.
9. **Per the dependency-change directive**, review `scripts/dev-doctor.sh` for any config-file reference and update it plus docs as needed.
10. **Migration note.** This is a breaking change for existing deployments — provide a short TOML→YAML conversion note (the field names are unchanged; only the syntax differs).

## Design Decisions

- **Single YAML library: `serde-saphyr`.** Replace the archived `serde_yaml` (0.9, unmaintained) entirely rather than keep it for output and add a second crate for parsing. `serde-saphyr` (saphyr/yaml-rust2 lineage, actively maintained — released within the last month) handles **both** directions with configurable serialization (`SerializerOptions`, block-string styles), so it covers config parsing *and* the existing `-o yaml` output without the non-idiomatic-output problem that rules out `serde_yaml2`. Its stated emphasis on **panic-free parsing and good error reporting** aligns with the project's **No Silent Errors** rule and reliability principle. ⚠️ Caveat: it is pre-1.0 (0.0.x), so the API may churn — accepted, and pinned to a specific minor version.
- **Replacement, not addition (×2).** Both `toml` (config format) and `serde_yaml` (YAML library) are removed; there is no dual-format config loader, no format auto-detection, and no second YAML crate. One config format, one YAML library, one code path.
- **The serde-library migration is its own work package**, sequenced first. It is independently shippable (migrates `-o yaml` output, retires `serde_yaml`) and de-risks the config swap, which then simply targets the already-adopted `serde-saphyr`.
- **Keep field/key names unchanged.** YAML keys equal the current serde field names, so existing configs translate line-for-line (`bind_address`, `request_timeout_secs`, …). This minimises churn and keeps the migration note trivial.
- **Secret URNs unchanged.** The `urn:secrets-rs:…` indirection is independent of file format and stays as-is, including in the Helm ConfigMap.

## Architecture

The format seam is tiny and well isolated:

```text
udex.yaml ──serde_saphyr::from_str──▶ UdexConfig ──validate()──▶ bind secrets (URN/FileSource) ──▶ server
            (only format-coupled line)    (format-agnostic from here on)
```

- **YAML library:** after WP1, `serde-saphyr` is the sole YAML crate — used for config `from_str` (CLI) and for `-o yaml` `to_string` (CLI commands). `serde_yaml` and `toml` are gone from the dependency tree.
- **Code:** the only format-coupled config lines live in `projects/rust/cli/src/config.rs` (two `from_str` calls + tests). The server crate consumes already-deserialized config types and needs no change.
- **Deploy:** Helm `configmap.yaml` (renders the file) and `deployment.yaml` (mounts it) are the only k8s touch-points; the mounted filename and the rendered body change, the `values.yaml` inputs do not.

## Alternatives Considered

- **Reuse the archived `serde_yaml` for parsing.** Rejected — it is unmaintained (archived 2024); building new config loading on it entrenches a dead dependency.
- **`serde_yaml2` + `yaml-rust2`.** Rejected — although it deserializes fine, its serializer emits non-idiomatic YAML (quoted keys, scalars on separate indented lines), which would visibly degrade the existing `-o yaml` CLI output. `serde-saphyr` offers configurable, clean serialization and more recent maintenance.
- **Support both TOML and YAML / auto-detect by extension.** Rejected — the objective is explicitly a replacement; a dual-format loader doubles the parse/test surface and creates "which file wins" ambiguity for no benefit.
- **Keep `serde_yaml` for `-o yaml` output and use a new crate only for config.** Rejected — leaves two YAML libraries in the tree (one of them archived); consolidating on `serde-saphyr` is cleaner (Highlander).
