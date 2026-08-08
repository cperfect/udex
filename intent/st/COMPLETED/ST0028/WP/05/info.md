---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
wp_id: WP-05
title: "Documentation"
scope: Small
status: Done
---

# WP-05: Documentation

## Objective

Bring every document that describes the observability fixture into line with what it actually is after WP04. The fixture's developer-facing UI is one of the main reasons it exists, so this work package is not a tidy-up pass -- if the docs still describe HyperDX, the capability is effectively lost even though it still exists.

## Deliverables

- `projects/compose/README.md` -- the largest single edit. The service table, the endpoint table (ClickHouse `:8123`, HyperDX `:8080`), the HyperDX login credentials, and the entire "Charting metrics in HyperDX" section.
- `docs/ARCHITECTURE.md` and `docs/DESIGN_DECISIONS.md` -- the observability backend narrative and the ST0027 decision record, which should gain the ST0028 supersession rather than being silently rewritten.
- `docs/SECRETS.md` -- new `OPENOBSERVE_*` entries, and an explicit note on the posture change: ClickHouse today is keyless, so the fixture gains a credential travelling in plaintext over the compose network. Dev-only and loopback-published, consistent with ST0027, but it should be stated rather than pass unremarked.
- `projects/k8s/README.md` and `projects/k8s/helm/udex/values.yaml` comments -- both describe the OTLP target as "the ClickHouse-backed collector". The endpoint is unchanged; only the description is wrong.
- `.devcontainer/README.md` and `.devcontainer/post-create.sh` -- both name the ClickHouse fixture and the HyperDX UI in their post-start guidance.
- `projects/rust/CONTRIBUTING.md` -- the `obs_k8s` test narrative.
- `intent/llm/MODULES.md` -- no module ownership changes (no production crate is touched), but the `udex-telemetry` note referencing the fixture should be checked.

## Implementation notes

**Do not simply delete the HyperDX charting recipes.** They exist because metrics charting is not self-evident: the udex and postgres counters are cumulative, so a raw value shows a running total and a developer needs to know to use a rate aggregation. That guidance is still true under OpenObserve; only the UI path changes. Replace the recipes, do not drop them.

This depends on someone having actually driven the OpenObserve UI. The spike deliberately did not -- it proved the API, not the ergonomics -- and it is flagged as an open question in `design.md`. Writing this work package is the point where that gets settled.

Markdown rules apply: every fenced code block carries a language identifier, tables are column-aligned, and `.md` files are never manually wrapped.

Run `/in-review` (doc-reviewer) before closing -- there is a known staleness hotspot file at `.claude/agent-memory/doc-reviewer/staleness_hotspots.md` that should be updated if these documents move in or out of it.

## Acceptance

Acceptance Criteria for this work package live in the steel thread's `acceptance.md`, under the `WP-05` heading (single source of truth). Do not restate ACs here.

## Dependencies

- WP-04 (documents the end state, so the end state should exist).
- Soft dependency on someone evaluating the OpenObserve UI, per the open question in `design.md`.
