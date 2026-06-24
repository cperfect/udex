---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-07
title: "Docs"
scope: Small
status: Not Started
---

# WP-07: Docs

## Objective

Document the observability system as built: architecture, local + k8s usage, test
conventions, and secrets inventory.

## Deliverables

- `docs/ARCHITECTURE.md`: new Observability section with mermaid diagrams (signal
  topology + deployment), covering the open-standard boundary, hybrid logs, OTLP
  push metrics, and the `udex-telemetry` / SDK provider-free split.
- `projects/observability/README.md`: stack overview, up/down/rebuild usage,
  component endpoints, Grafana access.
- README/compose docs updates for the layering and devcontainer wiring.
- `projects/k8s/README.md`: observability in the cluster deployment.
- `projects/rust/CONTRIBUTING.md` + ARCHITECTURE.md test tables: new `test_obs_` /
  `test_obs_k8s_` prefixes and how to run them.
- `docs/SECRETS.md`: new credentials/certs (OTLP certs, Grafana admin).

## Acceptance Criteria

- [ ] ARCHITECTURE.md observability section is accurate to the as-built system,
  with rendering mermaid diagrams.
- [ ] Every fenced code block has a language identifier; markdown not manually
  wrapped.
- [ ] All cross-references resolve; the doc-reviewer agent finds no staleness or
  broken links.
- [ ] SECRETS.md reflects all new secret material and where it is generated.

## Dependencies

- WP01-WP06 (documents the finished system; captures as-built decisions from each).
