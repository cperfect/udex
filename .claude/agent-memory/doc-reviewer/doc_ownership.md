---
name: doc-ownership
description: Highlander source-of-truth map for Udex docs — which doc owns which topic, to catch duplication/contradiction
metadata:
  type: reference
---

Udex repo doc ownership (Highlander). Cross-reference these rather than duplicate:

- Data model / operations / security model / design principles → `docs/ARCHITECTURE.md`
- Design rationale / "why" questions → `docs/FAQ.md` (project-wide) and `projects/protobuf/FAQ.md` (API-specific, e.g. why hash_algorithm immutable)
- Secrets/keys/cert inventory → `docs/SECRETS.md` (must update same commit as any credential change)
- Core domain concepts (Index/Context/Key/Entry, 1:1 invariant) → top-level `README.md#core-concepts`
- API/proto contract → `projects/protobuf/README.md` + the `.proto` files (source of truth for all API types). Only 2 protos exist: udex.index.v1, udex.entry.v1 (a healthz proto was removed).
- JWT claims / scope/permission format → `projects/rust/api/README.md#jwt-claims`
- Authorization runtime behaviour (key sources, JWKS refresh) → `projects/rust/server/README.md`
- CLI command surface → `projects/rust/cli/README.md`
- DB schema/tables → `projects/rust/datastore/README.md`
- Rust contribution / pre-commit / version pin table → `projects/rust/CONTRIBUTING.md`
- k8s/Helm deploy → `projects/k8s/README.md`
- Pinned-version table authority → `projects/rust/CONTRIBUTING.md` "Developing" section names the defining file for each version.

Authz fact (verify against `projects/rust/api/src/authz/entry.rs`): LookupOrCreate requires BOTH read AND write (`Operation::LookupOrCreate(_) => &["read","write"]`). Docs that say "write only" are wrong.

Index name validation (`projects/rust/server/src/index.rs::invalid_name_char`): allows Unicode letters/digits, hyphen, underscore. NOT lowercased/lowercase-only. README.md saying "lowercase strings" is wrong.
