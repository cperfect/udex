---
verblock: "18 Jun 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: WIP
slug: k8s-ingress-tls
created: 20260618
completed:
---

# ST0024: K8s ingress tls

## Objective

Switch the local k8s (k3d) ingress from L4 TCP with TLS **passthrough** to L7 HTTP with TLS **termination at the Traefik proxy**, using a statically generated edge certificate (with its own CA) kept in `projects/k8s/traefik/certs/`. A new cert-generation script (mirroring the existing `regenerate_certs.sh` pattern) produces the edge CA + cert, and it is wired into the top-level `scripts/gen-keys-and-certs.sh`.

## Context

Today the chart ships a Traefik `IngressRouteTCP` with `tls.passthrough: true` (`projects/k8s/helm/udex/templates/ingressroutetcp.yaml`). Traefik forwards raw TLS bytes at L4 and the udex pod terminates TLS itself with its mounted cert. We want Traefik to terminate the client TLS at L7 so the ingress operates as a normal HTTP(S) proxy (enabling L7 routing/middleware and an edge-managed certificate).

Two constraints shape the design:

1. **The server's TLS is mandatory.** `udex-server` applies `tls_config` unconditionally (`projects/rust/server/src/server.rs:179`); `TlsConfig` cert/key are required and validated non-empty (`projects/rust/server/src/config.rs:18,107-127`). There is no plaintext/h2c listen mode. gRPC is HTTP/2 end-to-end, so the Traefik→pod hop must still carry HTTP/2.
2. **Decision (user, 18 Jun 2026): re-encrypt to the pod — no server (Rust) changes.** Traefik terminates the client TLS with the static edge cert, then opens a *new* TLS (h2) connection to the pod, which keeps its existing cert. The alternative (plaintext h2c to the pod) was rejected because it would require a new insecure server listen mode, against the project's TLS-everywhere posture.

Trust implication: the client (incl. the k8s integration test) now terminates against **Traefik's** cert, not the server's. The edge cert gets its **own CA** in `projects/k8s/traefik/certs/`, and the k8s integration test is updated to trust that new CA (test-code change is in scope, per user, 18 Jun 2026). The edge cert carries `host.docker.internal`, `localhost`, `127.0.0.1`, `::1` in its SANs so hostname verification against `K8S_SERVER_URL` (default `https://host.docker.internal:8443`) succeeds. The pod keeps its existing cert; the Traefik→pod hop uses a `ServersTransport` (re-encrypt).

See `design.md` for the full target architecture, the cert strategy, and the per-file change plan.

## Related Steel Threads

- ST0023 — YAML config plan (most recent server config work; merged in #37)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
