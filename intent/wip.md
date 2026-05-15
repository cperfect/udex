---
verblock: "15 Apr 2026:v0.1: Matthew Sinclair - Initial version; 15 Apr 2026:v0.3: vscode - Cleared; ST0002 complete; 09 May 2026:v0.4: vscode - ST0010 complete; 12 May 2026:v0.5: vscode - ST0011 complete; 15 May 2026:v0.6: vscode - ST0012 complete; 15 May 2026:v0.7: vscode - ST0013 complete; 15 May 2026:v0.8: vscode - ST0013 post-review fixes complete"
---

# Work In Progress

Nothing in progress. ST0013 (Lookup Or Create Entry) post-review fixes complete — branch `feat/lookup-or-create` ready for final review / PR.

## Session summary (15 May 2026 — post-review)

All fixes were made on `feat/lookup-or-create`. No open items remain.

| Commit | Change |
|---|---|
| `106aca2` | Benchmark coverage for `lookup_or_create` in datastore and server bench suites (create path, found path, bulk at 10/100/1000) |
| `179f1f6` | SDK: map hash-computation failure to `Error::InvalidArgument` (was incorrectly `Error::InvalidResponse`) |
| `212ddcd` | SDK: re-export `LookupKeyByContextOrCreateRequest` and `LookupKeyByContextOrCreateResponse` from `udex_sdk` so consumers need no direct `udex_api` dependency |
| `c30e26c` | Datastore: replace two-statement TOCTOU race with a single atomic `INSERT … ON CONFLICT DO UPDATE … RETURNING` upsert; add immutability regression test |
| `09f67c7` | Server: add integration tests for all `lookup_key_by_context_or_create` paths (create, found, validation errors, hash mismatch, bulk write) |
