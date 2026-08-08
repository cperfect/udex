# Tasks - ST0028: OpenObserve as the dev observability backend

Work is tracked in work packages (`WP/01` .. `WP/05`); this file is the sequencing view. Acceptance lives in `acceptance.md`.

## Tasks

- [ ] WP-01 Stand up OpenObserve beside ClickHouse (compose service, collector dual-export, Vector via collector, `gen-env.sh` credentials)
- [ ] WP-02 Port the observability verification layer to OpenObserve (helpers, `obs.rs`, three k8s tests)
- [ ] WP-03 New coverage: Vector log floor assertion, `postgresql.backends` on the always-run path
- [ ] WP-04 Retire ClickHouse / HyperDX / Mongo; CI service lists and env; `dev-doctor.sh` check
- [ ] WP-05 Documentation across compose, docs, k8s, devcontainer and CONTRIBUTING

## Task Notes

The sequence is a strangler swap, not a big-bang cut. WP-01 adds without removing and WP-04 removes once nothing reads the old backend, so every work package leaves `cargo test` green. Deliberately no red window -- trunk-based development should not need one.

WP-02 is a port: the assertions stay semantically identical, only the query layer changes. WP-03 is the only work package adding genuinely new coverage. Anyone reading "add tests that traces, metrics and logs land" should note that already exists (`obs.rs` plus the three `test_obs_k8s_*` tests) and is being ported in WP-02, not written from scratch.

Two decisions to raise with the owner during implementation rather than assume:

- `dev-doctor.sh` version check: exact or major-version-only (project directive requires asking before changing a binary-dependency check).
- Whether the OpenObserve UI's charting story is good enough to close WP-05 without a HyperDX-equivalent recipe set. Unevaluated -- the spike proved the API, not the ergonomics.

Reference material: `FINDINGS.md` on branch `spike/openobserve-obs`. That branch is throwaway and must not be merged; if it is deleted before this thread completes, the schema map and verified queries in `design.md` are the surviving record.

## Dependencies

```text
WP-01 --> WP-02 --> WP-03 --> WP-04 --> WP-05
                \-----------/
          (WP-04 needs both WP-02 and WP-03 green)
```

- WP-01 has no dependencies; it is the entry point.
- WP-02 needs OpenObserve receiving telemetry.
- WP-03 needs the WP-02 helpers and the WP-01 Vector rewiring.
- WP-04 must not start until WP-02 and WP-03 are green, or the deletion stops being safe.
- WP-05 documents the end state, so it follows WP-04; it also carries a soft dependency on somebody evaluating the OpenObserve UI.
