---
name: doc-ownership
description: Highlander source-of-truth map for Udex docs — which doc owns which topic, to catch duplication/contradiction
metadata:
  type: reference
---

Udex repo doc ownership (Highlander). Cross-reference these rather than duplicate:

- Data model / operations / security model / design principles → `docs/ARCHITECTURE.md`
- Design rationale / "why" decisions → `docs/DESIGN_DECISIONS.md` (project-wide, grouped: data-model invariants, tech/interface choices, scope/non-goals) and `projects/protobuf/FAQ.md` (API-specific, e.g. why hash_algorithm immutable)
- Operational how-tos / usage guidance / troubleshooting / roadmap → `docs/FAQ.md` (split from DESIGN_DECISIONS.md; FAQ no longer holds "why" rationale)
- Secrets/keys/cert inventory → `docs/SECRETS.md` (must update same commit as any credential change)
- Core domain concepts (Index/Context/Key/Entry, 1:1 invariant) → top-level `README.md#core-concepts`
- API/proto contract → `projects/protobuf/README.md` + the `.proto` files (source of truth for all API types). Only 2 protos exist: udex.index.v1, udex.entry.v1 (a healthz proto was removed).
- JWT claims / scope/permission format → `projects/rust/api/README.md#jwt-claims`
- Authorization runtime behaviour (key sources, JWKS refresh) → `projects/rust/server/README.md`
- CLI command surface → `projects/rust/cli/README.md`
- DB schema/tables → `projects/rust/datastore/README.md`
- Rust contribution / pre-commit / version pin table → `projects/rust/CONTRIBUTING.md`
- k8s/Helm deploy → `projects/k8s/README.md`
- Observability / OTel design → `docs/ARCHITECTURE.md#observability` (system-level) + `projects/compose/README.md#observability` (local fixture: services, UI/credentials, where data lands, chart recipes — there is no `projects/observability/` any more). Telemetry crate boundary → `projects/rust/telemetry/README.md` (`udex-telemetry` is the ONLY crate that installs an OTel provider; SDK is provider-free, uses `opentelemetry` API only).
- Observability *rationale* → `docs/DESIGN_DECISIONS.md#observability`, kept as a layered record: the superseded ST0027 ClickHouse sections are retained deliberately, with an ST0028 "Why OpenObserve instead of ClickHouse + HyperDX?" section after them. ClickHouse/HyperDX/Mongo mentions there (and the posture comparison in `docs/SECRETS.md#observability`, and ClickStack/HyperDX as a third-party backend example in `projects/rust/telemetry/README.md`) are INTENTIONAL history, not staleness.
- Recurring duplication risk: the obs UI URL + login credential sentence tends to get copied into `projects/compose/README.md`, `.devcontainer/README.md` and `projects/k8s/README.md`. compose/README owns it; the other two should link.
- Pinned-version table authority → `projects/rust/CONTRIBUTING.md` "Developing" section names the defining file for each version.

Authz fact (verify against `projects/rust/api/src/authz/entry.rs`): LookupOrCreate requires BOTH read AND write (`Operation::LookupOrCreate(_) => &["read","write"]`). Docs that say "write only" are wrong.

Index name validation (`projects/rust/server/src/index.rs::invalid_name_char`): allows Unicode letters/digits, hyphen, underscore. NOT lowercased/lowercase-only. README.md saying "lowercase strings" is wrong.
