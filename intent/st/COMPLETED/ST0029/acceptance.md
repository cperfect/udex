---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
st_id: ST0029
title: "Remove cross-test ordering dependencies in the SDK integration suite -- acceptance contract"
---

# ST0029 Remove cross-test ordering dependencies -- Acceptance

> Canonical acceptance contract for ST0029. Acceptance Criteria (AC) are the ratified completeness boundary; Acceptance Tests (AT) are the small red-to-green tests that prove them. Real test code lives in the suite (paths cited below); this file is the contract plus the AC-to-AT coverage map plus live status.
>
> Done = every AC is covered by a GREEN AT, or (for a non-test AC) its named evidence is satisfied, AND the AC set is the ratified full boundary.
>
> AT status vocabulary: to-write (red-first) | red | green | n/a (non-test: doc / eyeball / gate).

## Acceptance Criteria

### ST-level

- AC-00.1 Every test in `sdk/tests/integration_tests.rs` passes when run on its own, so no test depends on another having run first
- AC-00.2 No test deletes or empties shared fixture state that other tests read
- AC-00.3 The repaired tests still detect the defect they exist to catch -- they must not pass vacuously
- AC-00.4 (non-test) The new Hydra client secret is recorded in the secrets inventory -- evidence: `docs/SECRETS.md` -- satisfied: yes

### WP-01 -- Isolation sweep in CI and locally (status: Done)

- AC-01.1 The sweep **detects** a reintroduced ordering dependency, naming the offending test and exiting non-zero -- a check that cannot fail is worthless
- AC-01.2 (non-test) The same command runs locally and in CI, so a developer can reproduce a CI failure exactly -- evidence: `scripts/validate-test-isolation.sh`, invoked by the `Test isolation` step in `01-Validation.yml`, matching the existing `validate-*.sh` convention -- satisfied: yes
- AC-01.3 (non-test) The sweep is cheap enough to gate every run rather than being relegated to a schedule -- evidence: ~20s, by driving the compiled test binary directly instead of invoking `cargo` per test -- satisfied: yes
- AC-01.4 (non-test) The test list is discovered at runtime, not hardcoded, so the check cannot silently stop covering new tests -- evidence: `--list --format terse` against the built binary -- satisfied: yes
- AC-01.5 (non-test) The rule and the command are documented where a developer writing a test will meet them -- evidence: `projects/rust/CONTRIBUTING.md` testing section -- satisfied: yes

## Acceptance Tests

- AT-00.1 each of the 40 tests in `integration_tests.rs` run individually with `-- --exact` -- covers AC-00.1 -- status: green (40/40)
- AT-00.2 `test_sdk_delete_index_not_empty` and `test_sdk_oauth2_delete_index_not_empty` operate on indexes they create themselves -- covers AC-00.2 -- status: green
- AT-00.3 removing the seeding step from `test_sdk_delete_index_not_empty` makes it fail -- covers AC-00.3 -- status: green (verified by temporarily deleting the seed; the test failed, then passed again once restored)
- Coverage: AC-00.4 is non-test and carries evidence on the AC line.

Evidence trail, in the order it was established:

1. **Deterministic reproduction before the fix.** `cargo test test_sdk_delete_index_not_empty` failed 100% of the time in isolation -- `called Result::unwrap_err() on an Ok value`. The defect was never really intermittent; only its *collateral damage* was.
2. **Isolation sweep found a second instance.** Running all 40 tests individually surfaced `test_sdk_oauth2_delete_index_not_empty`, which no observed failure had ever implicated.
3. **Both pass in isolation after the fix**, and the sweep is clean at 40/40.
4. **Vacuity check.** Removing the seeding step reproduced the original failure, confirming the assertion still bites.
5. **Two consecutive full-package runs** (`cargo test --package udex-sdk` with `K8S_SERVER_URL` set): 60 tests, 0 failed, exit 0 each time -- the concurrent conditions under which the cascade originally appeared.

### WP-01

- AT-01.1 reintroduce the ordering dependency in `test_sdk_delete_index_not_empty`, run `scripts/validate-test-isolation.sh` -- covers AC-01.1 -- status: green

AT-01.1 observed: `39/40 tests pass in isolation`, the offending test named, a copy-pasteable reproduction command printed, and exit code 1. The regression was then reverted and the sweep returned to 40/40. Verifying this mattered more than the passing run: a gate that cannot go red would have given false assurance for the rest of the project's life.
- Coverage: AC-01.2 through AC-01.5 are non-test and carry evidence on their AC lines.
