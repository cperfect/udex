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

## What future features might Udex support?
* Optional REST interface
* Optional UI (which would mean supporting OIDC)
* Tracing/APM
* Support for other datastores, as long as they are transactional
* SDKs for other languages
* Alternate hash algorithms
* Alternate key generation algorithms/formats - as long as they are opaque and globally unique

## What won't Udex support?
* Non-transactional datastores
* Complex/aggregate cross-context queries
