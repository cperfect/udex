# Udex Frequently Asked Questions

For the rationale behind Udex's core design choices, see [Design Decisions](DESIGN_DECISIONS.md).

## How do I handle key migrations?
Because Udex enforces a strict 1:1 mapping (see [Why are keys:contexts 1:1?](DESIGN_DECISIONS.md#why-are-keyscontexts-11)) you cannot create a second entry for the same context. Instead, encode the migration intent in the context using a version pair.

**Example — re-enrolling a user after a credential rotation:**

```text
# Original entry
create_entry index=users context={user_id: alice, version: 1}
→ key: 550e8400-e29b-41d4-a716-446655440000

# Migrate: bump the version to produce a distinct context fingerprint
create_entry index=users context={user_id: alice, version: 2}
→ key: 6ba7b810-9dad-11d1-80b4-00c04fd430c8   ← fresh key

# Optionally retire the old entry
delete_entry key=550e8400-e29b-41d4-a716-446655440000
```

The `version` pair is just a convention — any pair that changes the fingerprint works (`epoch`, `tenant`, `rotation_id`, etc.). Callers that need to look up the current entry include the expected version in their context and let the hash do the rest. No out-of-band state is needed to track "which version is current" beyond what is already in the context pairs themselves.

## What future features might Udex support?
* Optional REST interface
* Optional UI (which would mean supporting OIDC)
* Tracing/APM
* Support for other datastores, as long as they are transactional
* SDKs for other languages
* Alternate hash algorithms
* Alternate key generation algorithms/formats - as long as they are opaque and globally unique
* Separate db connection Pools per index - this would allow for separation of connection resources and/or allow for different schemas/dbs/servers per index.
* A dedicated non-TLS health port — this would allow Kubernetes native `grpc` probes to exercise the full gRPC stack rather than only confirming the TCP port is open, without requiring an extra binary in the container image (see [Why does Udex use the gRPC Health Checking Protocol?](DESIGN_DECISIONS.md#why-does-udex-use-the-grpc-health-checking-protocol-instead-of-a-custom-healthz-endpoint)).

## How do I apply database migrations?

Use the `udex migrate` subcommands to inspect and advance the database schema independently of the server process.

```bash
# Check whether the database is at the expected schema version.
# Exits 0 if up to date; exits non-zero with a descriptive message if behind.
udex migrate check --config udex.yaml

# Apply all outstanding migrations, then confirm the version.
udex migrate apply --config udex.yaml
```

Both commands read only the `datastore` section of the config file — TLS certificate files are not required. The recommended approach for production is to run `udex migrate apply` as a pre-deploy step before starting the new server binary.

If you want the server to apply migrations automatically on startup (e.g. in development or CI), set `apply_migrations: true` under `datastore`:

```yaml
datastore:
  apply_migrations: true
```

See [Database migrations](../README.md#database-migrations) in the README for the full deployment workflow.

## Why is the server refusing to start with a schema mismatch error?

The server always checks that the database schema version matches the version expected by the running binary. If they differ, the server logs an error and exits rather than starting against an incompatible schema.

The log will include the current and expected version numbers, for example:

```text
ERROR Database schema version mismatch — server cannot start; run `udex migrate apply` or set apply_migrations=true to resolve
  error: Database not initialized: Current version 0 does not match latest version 1
```

To resolve, run `udex migrate apply --config udex.yaml` to bring the schema up to date, then restart the server. Alternatively, set `apply_migrations: true` under `datastore` to allow the server to migrate automatically on startup (not recommended for production).

## How do I migrate my config from TOML to YAML?

Udex configuration is now YAML (previously TOML) — see [Why YAML for configuration (not TOML)?](DESIGN_DECISIONS.md#why-yaml-for-configuration-not-toml). This is a breaking change: an existing `udex.toml` will not load. The conversion is mechanical because **the field names and secret URNs are unchanged** — only the syntax differs:

- Rename the file `udex.toml` → `udex.yaml` (the default config path and the container mount are now `config.yaml`).
- TOML tables become nested YAML mappings: `[server]` → `server:`, `[server.tls]` → `tls:` under `server`, `[server.authz]` → `authz:` under `server`, `[datastore]` → `datastore:`.
- `key = value` becomes `key: value`; indent nested keys two spaces.

For example:

```toml
[server.tls]
cert = "urn:secrets-rs:file:certs/server.crt"
```

becomes:

```yaml
server:
  tls:
    cert: "urn:secrets-rs:file:certs/server.crt"
```

If you prefer to start fresh, `udex config init` writes a fully commented `udex.yaml` template; run `udex config validate` to check it.

## When should I use `lookup-or-create` instead of `lookup` + `create`?

Use `lookup_key_by_context_or_create` (CLI: `udex entry lookup-or-create`) when you cannot know in advance whether an entry exists for a context and you do not want to perform a read-before-write.

The canonical case is **Id Permanence**: an Indexer receives an entity — say, a customer or transaction — and must return a stable key for it, regardless of whether this is the first time it has seen that entity. A two-step approach (`lookup` → if missing, `create`) works but burns an extra round trip and introduces a TOCTOU window in concurrent systems. `lookup-or-create` removes both problems in a single call.

```bash
# First call — entry does not exist yet
udex entry lookup-or-create customers --context id=alice --context version=1
# → key: 550e8400-..., context_hash: abc123, created: true

# Second call — same context, different process or retry
udex entry lookup-or-create customers --context id=alice --context version=1
# → key: 550e8400-..., context_hash: abc123, created: false
```

**Permission requirement**: `lookup-or-create` requires **both** the `udex:entry:v1:{index_name}:read` and `udex:entry:v1:{index_name}:write` permissions, because it both reads and writes. A token missing either is rejected. In bulk operations it must appear in `BulkWriteEntryOperation`, not `BulkReadEntryOperation`.

**Hash verification**: the client must supply a pre-computed `context_hash` alongside the full context pairs. The server recomputes the hash and returns `INVALID_ARGUMENT` if they disagree — even before touching the database. Always use the SDK to compute the hash; it guarantees algorithm stability.
