---
verblock: "18 May 2026:v0.1: Chris Perfect - Initial version"
intent_version: 2.4.0
status: Completed
slug: local-k8s-and-helm-development
created: 20260518
completed: 20260520
---

# ST0018: Local k8s and helm development

## Objective

Enable local development and integration testing of the Udex server on Kubernetes using k3d, and produce a Helm chart as the primary deployment artefact. The deliverables are:

- A multi-stage Dockerfile for the `udex` CLI binary (which embeds the server), located in `projects/rust/cli/`.
- A Helm chart and supporting k8s manifests under `projects/k8s/`.
- Scripts to create/destroy the k3d cluster, build and load the Docker image, and deploy/undeploy via Helm.
- A k8s-targeted test harness that re-uses the existing SDK integration tests against the deployed cluster, using OAuth2 (Hydra) auth exclusively.
- Integration of the k8s test run into the GitHub Actions validation workflow.
- A `projects/k8s/README.md` and updates to `CONTRIBUTING.md` and `scripts/dev-doctor.sh`.

## Context

The project already runs its services (PostgreSQL + Hydra) via Docker Compose. The Rust workspace builds a single `udex` binary from the `cli` crate, which embeds `udex-server` and is therefore the natural deployable unit. k3d, kubectl, and Helm are already installed in the devcontainer.

The k8s deployment does **not** attempt to move PostgreSQL or Hydra onto the cluster — they continue to run in the Compose environment. Pods reach them via `host.k3d.internal`, which k3d injects into every pod's `/etc/hosts`.

Auth in the k8s deployment is OAuth2 (client credentials via Hydra) exclusively. Static-JWT auth is not configured in the cluster, so JWT-fixture-only tests are not run in the k8s harness. As part of this steel thread, existing SDK test cases that live solely under the static-JWT fixture will be audited and ported to the OAuth2 fixture where appropriate, increasing real-world auth coverage independently of k8s.

## Related Steel Threads

- None

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
