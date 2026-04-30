---
verblock: "28 Apr 2026:v0.2: vscode - Mark WP-01/02/03/07 done; add acceptance criteria; 28 Apr 2026:v0.3: vscode - Add WP-09: Fix CI key material generation; 30 Apr 2026:v0.4: vscode - Mark WP-04 done"
---

# ST0008: Tasks — Inject keys and secrets

## Work Packages

- [x] WP-01: Gitignore & remove committed secrets
- [x] WP-02: Developer setup scripts (`gen-env.sh`, `gen-keys-and-certs.sh`)
- [x] WP-03: Devcontainer post-create integration
- [x] WP-04: Config crate evaluation and `_secret` naming convention
- [ ] WP-05: File-injection guard in config loader
- [ ] WP-06: Remove secrets from CLI arguments
- [x] WP-07: Inject secrets into Compose and CI via env vars *(completed within WP-01)*
- [ ] WP-08: Update CONTRIBUTING.md, SECRETS.md, SECURITY.md
- [ ] WP-09: Fix CI — generate key material before tests

## Acceptance Criteria

### WP-01 ✓
- `git ls-files` shows no private keys or generated cert material
- `.env` and all `tests/certs/*.{key,crt,csr,srl}` and `tests/jwt/*.pem` are gitignored
- `docker-compose.yml` and CI workflow contain no hardcoded secret values
- Postgres init script fails fast with a clear error if `HYDRA_DB_PASSWORD_SECRET` is unset

### WP-02 ✓
- `scripts/gen-env.sh --force` creates `.env` with `POSTGRES_PASSWORD_SECRET`,
  `HYDRA_DB_PASSWORD_SECRET`, `HYDRA_SECRETS_SYSTEM_SECRET`, and derived `DATABASE_URL`
- `scripts/gen-keys-and-certs.sh` produces all TLS and JWT key material in gitignored
  paths with correct permissions (600 for private keys)
- Both scripts are idempotent: re-running does not break a working setup
- All generated files absent from `git status`

### WP-03 ✓
- Fresh devcontainer (no `.env`, no keys) auto-runs both scripts during post-create
- Rebuild with existing workspace volume skips generation and logs "already exists"

### WP-07 ✓
- `docker-compose.yml`: `POSTGRES_PASSWORD`, Hydra DSN password, `SECRETS_SYSTEM`
  all use `${..._SECRET}` env var references
- `01-Validation.yml`: postgres service and `DATABASE_URL` reference
  `secrets.POSTGRES_PASSWORD_SECRET`
- No hardcoded secret values remain in any committed compose or CI file

### WP-04 ✓
- [x] Decision documented in `design.md`: keep current TOML/serde loader; add secrets-rs on top
- [x] `_secret` naming convention rejected — `Secret<String>` type carries the intent instead
- [x] `DatastoreConfig.connection_url` is `Secret<String>` in both the datastore and CLI crates
- [x] Config files reference secrets by URN (`urn:secrets-rs:env:VAR_NAME`); real values never appear in TOML
- [x] `UdexConfig::load()` calls `bind_all()` after deserialisation; startup fails with a clear error if any env var is absent
- [x] `Secret<String>` masked in `Debug`, `Display`, and serde serialisation — `.value()` required for access
- [x] `config init` writes a hand-authored template with correct URN format and a comment explaining the secrets-rs pattern
- [x] `cargo fmt`, `cargo clippy`, and `cargo test --lib` all pass

### WP-05
- Config loader parses raw TOML before deserializing and errors if any `_secret`
  key is present in the file
- Error message names the offending key and the correct env var to use instead
- Tests cover: valid config file (no secrets) passes; config file containing a
  secret key is rejected with the expected message

### WP-06
- `--token` / `--client-secret` flags removed from the CLI; env vars only
- Attempting to pass the value as a flag produces a clear "use env var" error
- Missing required secret env var produces a clear error naming the variable
- `cargo clippy` and `cargo test` pass

### WP-08
- `CONTRIBUTING.md` includes first-time setup steps (run both gen scripts before
  building or starting the devcontainer)
- `SECRETS.md` updated: committed-file entries removed; source column reflects
  env-var injection and gitignored generated files
- `SECURITY.md` documents the injection model, `_secret` naming convention,
  file-injection guard, and CLI arg restriction

### WP-09
- `01-Validation.yml` `test` job includes a "Generate key material" step running
  `scripts/gen-keys-and-certs.sh` before `cargo build`
- The step runs from the workspace root (not `projects/rust/`)
- `cargo test` passes end-to-end in CI with no file-not-found errors for certs/keys

## Sequencing

```
WP-01 ✓  (remove secrets from repo)
  └─► WP-02 ✓  (gen scripts replace what was removed)
        └─► WP-03 ✓  (devcontainer runs gen scripts)

WP-07 ✓  (compose/CI — completed within WP-01)

WP-04 ✓  (secrets-rs; Secret<String> for connection_url)
  └─► WP-05  (file-injection guard)   ← next
        └─► WP-06  (remove secrets from CLI args)

WP-08  (docs — after all implementation WPs complete)

WP-09  (CI fix — can run in parallel with WP-04/05/06; depends only on WP-01 ✓)
```

## Dependencies

- WP-05 depends on WP-04 (naming convention must be decided before guard can be coded)
- WP-06 depends on WP-05 (CLI uses the same config loading path)
- WP-08 depends on all other WPs
