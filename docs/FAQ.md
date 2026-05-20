# Udex Frequently Asked Questions

## Why is this not just a KV-store?
Firstly I wanted it to be highly opinionated about keys, contexts, hashes and the mappings between them to fit the use cases without it becoming a free-for-all. Secondly because most KV-stores aren't transactional (which is really just a special case of the former).

## Why is it transactional?
Transactions enforce data quality and make reasoning and implementation of state management much easier, especially in the kind of distributed systems that integration scenarios involve. Ultimately [state is *hard*](https://istheenemyofgood.medium.com/001-state-is-hard-18fa3b1812ae). Modern transactional datastores are highly-performant and I don't think the tradeoff with non-transactional implementations is worth it for Udex.

## Why are keys:contexts 1:1?
The driving use case is cross-party key resolution. If Party A sends the same entity twice — because of a network retry, a restart, or a race — Udex must return the same stable key both times. Returning a different key would break any downstream system that cached or stored the first one.

The 1:1 invariant provides that guarantee at the database level: one context fingerprint maps to exactly one entry key, always. No application-layer deduplication is needed, and `create_entry` is safe to call from retry-prone systems without coordination.

## Why are contexts immutable?
If a context could be updated, its fingerprint would change silently — any caller holding the old key could get back a different context, breaking the stable-key guarantee that is the point of Udex. Immutability makes the key permanent: whatever was true when the entry was created stays true, and callers that need a different mapping encode that intent in a new context (see [How do I handle key migrations?](#how-do-i-handle-key-migrations)).

## How do I handle key migrations?
Because Udex enforces 1:1 you cannot create a second entry for the same context. Instead, encode the migration intent in the context using a version pair.

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

## Why is this all driven by a CLI?
The CLI serves two purposes: operational control (starting and stopping the server, managing indices and entries) and a scriptable, auditable client surface for development and testing that does not require building a custom client.

## Why Rust?
As a high-performance, high-reliability back-end system I wanted it to be something that handled concurrency well and was memory safe. My preferred language for this would have been Go but I wanted an excuse to learn Rust.

## Why RPC and not REST?
I think the RPC model suits Udex better than REST, especially for the bulk scenarios. gRPC in particular is more network efficient than HTTP REST. I also think that, as a service involved in integration and data governance, it would be better for the API to avoid breaking changes and keep backwards compatibility as far as possible and the protobuf model suits this, which in turn leads to RPC.

## Why does Udex use the gRPC Health Checking Protocol instead of a custom healthz endpoint?

Udex implements the [standard gRPC Health Checking Protocol](https://github.com/grpc/grpc/blob/master/doc/health-checking.md) (`grpc.health.v1.Health`) rather than a bespoke healthz service for four reasons:

- **Broad tooling support** — the standard is understood natively by Kubernetes, Envoy, Istio, `grpc-health-probe`, and any gRPC-aware load balancer. No Udex-specific client code or configuration is required.
- **Per-service granularity** — the protocol carries a service name (`""` for overall server, `"udex.entry.v1.EntryService"`, `"udex.index.v1.IndexService"`). This is built in; a custom proto would duplicate the concern.
- **Enables native Kubernetes `grpc` probes** — Kubernetes has supported native `grpc` probe type since 1.24 (GA). These probes actually exercise the gRPC stack, whereas `tcpSocket` probes only confirm the port is open. The current deployment uses `tcpSocket` because the server is TLS-only on port 443 and native `grpc` probes do not support TLS; a future non-TLS health port would remove that constraint.
- **The old `udex.healthz.v1` proto was bespoke** — it had no tooling support outside this repository and imposed a custom client on every caller that needed to check server health.

## What future features might Udex support?
* Optional REST interface
* Optional UI (which would mean supporting OIDC)
* Tracing/APM
* Support for other datastores, as long as they are transactional
* SDKs for other languages
* Alternate hash algorithms
* Alternate key generation algorithms/formats - as long as they are opaque and globally unique
* Separate db connection Pools per index - this would allow for separation of connection resources and/or allow for different schemas/dbs/servers per index.
* A dedicated non-TLS health port — this would allow Kubernetes native `grpc` probes to exercise the full gRPC stack rather than only confirming the TCP port is open, without requiring an extra binary in the container image (see [Why does Udex use the gRPC Health Checking Protocol?](#why-does-udex-use-the-grpc-health-checking-protocol-instead-of-a-custom-healthz-endpoint)).

## How do I apply database migrations?

Use the `udex migrate` subcommands to inspect and advance the database schema independently of the server process.

```bash
# Check whether the database is at the expected schema version.
# Exits 0 if up to date; exits non-zero with a descriptive message if behind.
udex migrate check --config udex.toml

# Apply all outstanding migrations, then confirm the version.
udex migrate apply --config udex.toml
```

Both commands read only the `[datastore]` section of the config file — TLS certificate files are not required. The recommended approach for production is to run `udex migrate apply` as a pre-deploy step before starting the new server binary.

If you want the server to apply migrations automatically on startup (e.g. in development or CI), set `apply_migrations = true` in `[datastore]`:

```toml
[datastore]
apply_migrations = true
```

See [Database migrations](../README.md#database-migrations) in the README for the full deployment workflow.

## Why is the server refusing to start with a schema mismatch error?

The server always checks that the database schema version matches the version expected by the running binary. If they differ, the server logs an error and exits rather than starting against an incompatible schema.

The log will include the current and expected version numbers, for example:

```text
ERROR Database schema version mismatch — server cannot start; run `udex migrate apply` or set apply_migrations=true to resolve
  error: Database not initialized: Current version 0 does not match latest version 1
```

To resolve, run `udex migrate apply --config udex.toml` to bring the schema up to date, then restart the server. Alternatively, set `apply_migrations = true` in `[datastore]` to allow the server to migrate automatically on startup (not recommended for production).

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

**Permission requirement**: `lookup-or-create` requires the `udex:entry:v1:{index_name}:write` permission because it may write. A token with only `read` permission will be rejected. In bulk operations it must appear in `BulkWriteEntryOperation`, not `BulkReadEntryOperation`.

**Hash verification**: the client must supply a pre-computed `context_hash` alongside the full context pairs. The server recomputes the hash and returns `INVALID_ARGUMENT` if they disagree — even before touching the database. Always use the SDK to compute the hash; it guarantees algorithm stability.

## What won't Udex support?
* Non-transactional datastores
* Complex/aggregate cross-context queries
