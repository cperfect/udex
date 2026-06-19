---
verblock: "19 Jun 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Completed
slug: cluster-integration-tests
created: 20260619
completed: 20260619
---

# ST0025: Cluster integration tests

## Objective

Default the dev k8s deployment to a **2-replica** server ReplicaSet, allow the individual instances to be **directly addressed** for tests (in addition to the round-robin load balancer), and add k8s integration tests that exercise **cross-instance** correctness — e.g. CRUD an index/entry via one instance and verify it via the other.

## Context

The server is required to be stateless with all persistent state in the datastore (CLAUDE.md: "Server MUST be stateless"). Today the Helm chart runs `replicas: 1` (`projects/k8s/helm/udex/templates/deployment.yaml:12`), so no test ever proves the multi-instance invariant holds. This steel thread raises the dev default to 2 replicas and adds tests that pin requests to specific instances to prove writes on one are visible on the other.

Design decisions (user, 19 Jun 2026):

1. **Direct addressing via `kubectl port-forward` per pod** (not per-pod Traefik routing). The chart change is minimal (just a configurable replica count); the test harness discovers the pods by label and port-forwards each to a distinct local port. These direct hops bypass Traefik and terminate against the **pod cert** (trusted via the server CA, SNI `localhost`), while the existing LB path (Traefik edge cert, round-robin) is unchanged. See [[k8s-ingress-tls]] (ST0024) for the ingress/TLS model.
2. **Fix any cross-instance consistency bug in scope.** If the new tests surface a real bug, fix it within this ST so the suite lands green.

Code reality (verified): the only per-instance state is an index→hasher cache (`projects/rust/server/src/entry.rs:33-38`) populated at startup, but request handlers fall back to the datastore on a cache miss (`entry.rs:94-141`); entries are never cached (straight to PostgreSQL). So an index/entry created on instance A is expected to be visible on B on first request — the tests should pass without a server change. A concrete harness gotcha: the existing `redeploy_k8s_server` rollout-wait loops until **≤1 pod** (`projects/rust/sdk/tests/integration_tests.rs`), which is wrong for 2 replicas and must be updated to wait for N ready pods.

See `design.md` for the full plan.

## Related Steel Threads

- ST0024 — K8s ingress TLS termination (current ingress/TLS model; edge vs pod certs)
- ST0018 — Local k8s and Helm dev environment (chart + scripts foundation)

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
