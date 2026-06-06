# Udex Design Decisions

This document captures the rationale behind Udex's core design choices — why the system is shaped the way it is. For operational how-tos, usage guidance, and troubleshooting, see the [FAQ](FAQ.md).

## Data model and invariants

### Why is this not just a KV-store?
Firstly I wanted it to be highly opinionated about keys, contexts, hashes and the mappings between them to fit the use cases without it becoming a free-for-all. Secondly because most KV-stores aren't transactional (which is really just a special case of the former).

### Why is it transactional?
Transactions enforce data quality and make reasoning and implementation of state management much easier, especially in the kind of distributed systems that integration scenarios involve. Ultimately [state is *hard*](https://istheenemyofgood.medium.com/001-state-is-hard-18fa3b1812ae). Modern transactional datastores are highly-performant and I don't think the tradeoff with non-transactional implementations is worth it for Udex.

### Why are keys:contexts 1:1?
The driving use case is cross-party key resolution. If Party A sends the same entity twice — because of a network retry, a restart, or a race — Udex must return the same stable key both times. Returning a different key would break any downstream system that cached or stored the first one.

The 1:1 invariant provides that guarantee at the database level: one context fingerprint maps to exactly one entry key, always. No application-layer deduplication is needed, and `create_entry` is safe to call from retry-prone systems without coordination.

### Why are contexts immutable?
If a context could be updated, its fingerprint would change silently — any caller holding the old key could get back a different context, breaking the stable-key guarantee that is the point of Udex. Immutability makes the key permanent: whatever was true when the entry was created stays true, and callers that need a different mapping encode that intent in a new context (see [How do I handle key migrations?](FAQ.md#how-do-i-handle-key-migrations)).

## Technology and interface choices

### Why Rust?
As a high-performance, high-reliability back-end system I wanted it to be something that handled concurrency well and was memory safe. My preferred language for this would have been Go but I wanted an excuse to learn Rust.

### Why RPC and not REST?
I think the RPC model suits Udex better than REST, especially for the bulk scenarios. gRPC in particular is more network efficient than HTTP REST. I also think that, as a service involved in integration and data governance, it would be better for the API to avoid breaking changes and keep backwards compatibility as far as possible and the protobuf model suits this, which in turn leads to RPC.

### Why is this all driven by a CLI?
The CLI serves two purposes: operational control (starting and stopping the server, managing indices and entries) and a scriptable, auditable client surface for development and testing that does not require building a custom client.

### Why does Udex use the gRPC Health Checking Protocol instead of a custom healthz endpoint?

Udex implements the [standard gRPC Health Checking Protocol](https://github.com/grpc/grpc/blob/master/doc/health-checking.md) (`grpc.health.v1.Health`) rather than a bespoke healthz service for four reasons:

- **Broad tooling support** — the standard is understood natively by Kubernetes, Envoy, Istio, `grpc-health-probe`, and any gRPC-aware load balancer. No Udex-specific client code or configuration is required.
- **Per-service granularity** — the protocol carries a service name (`""` for overall server, `"udex.entry.v1.EntryService"`, `"udex.index.v1.IndexService"`). This is built in; a custom proto would duplicate the concern.
- **Enables native Kubernetes `grpc` probes** — Kubernetes has supported native `grpc` probe type since 1.24 (beta, enabled by default; GA in 1.27). These probes actually exercise the gRPC stack, whereas `tcpSocket` probes only establish a TCP connection (no TLS handshake, no gRPC). The current deployment uses `tcpSocket` because the server is TLS-only on port 443 and native `grpc` probes do not support TLS; a future non-TLS health port would remove that constraint.
- **The old `udex.healthz.v1` proto was bespoke** — it had no tooling support outside this repository and imposed a custom client on every caller that needed to check server health.

## Scope and non-goals

### What won't Udex support?
* Non-transactional datastores
* Complex/aggregate cross-context queries
