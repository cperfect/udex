---
verblock: "24 Jun 2026:v0.1: vscode - Initial version"
wp_id: WP-07
title: "Docs"
scope: Small
status: Done
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

- [x] ARCHITECTURE.md observability section is accurate to the as-built system,
  with a mermaid signal-topology diagram.
- [x] Every fenced code block has a language identifier; markdown not manually
  wrapped.
- [x] All cross-references resolve; the doc-reviewer agent verified accuracy and
  links (approved; its three minor findings were applied).
- [x] SECRETS.md reflects all new secret material and where it is generated.

## As-built notes

- **`docs/ARCHITECTURE.md`**: new `## Observability` section (open-standard
  boundary, signals, mermaid topology, configuration, deployment) + a Development
  Components bullet + a `test_obs_k8s_` row in the Test Strategy table.
- **`docs/SECRETS.md`**: OTLP CA/collector cert rows, a new Observability section
  for the Grafana admin credential, the OTLP `regenerate_certs.sh` row, and
  updated `gen-env.sh`/`gen-keys-and-certs.sh` descriptions.
- **`projects/rust/CONTRIBUTING.md`**: an "Observability tests" note +
  `test_obs_k8s_` in the naming list.
- **`projects/k8s/README.md`**: an Observability section (pods export to the local
  stack via `host.k3d.internal`; best-effort; how to view).
- **`projects/compose/README.md`**: an "Optional: observability stack" pointer.
- **`projects/observability/README.md`**: forward-references updated now that the
  app emits telemetry.
- **`projects/rust/telemetry/README.md`** (new): crate README for `udex-telemetry`.
- **`README.md`**: project-table rows for `projects/observability/` and
  `projects/rust/telemetry/`.
- **doc-reviewer**: ran on the changeset; verdict "approve with minor changes".
  Applied all three: telemetry README `init` signature now shows the `Result`,
  the rustdoc `[`init`]` link is plain code, and the mermaid node uses `<br/>`.

## Dependencies

- WP01-WP06 (documents the finished system; captures as-built decisions from each).
