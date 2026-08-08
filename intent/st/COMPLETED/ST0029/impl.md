# Implementation - ST0029: Remove cross-test ordering dependencies in the SDK integration suite

## Implementation

Changed file: `projects/rust/sdk/tests/integration_tests.rs`. Plus one inventory row in `docs/SECRETS.md`.

Both repaired tests now create and populate an index of their own instead of borrowing the shared fixture's. Two helpers were added, because index-level scopes in the test tokens are wildcarded but **entry**-level scopes are bound to a specific index — so a test can create its own index with the shared client but cannot put entries in it:

- `client_scoped_to_index(index_name)` — JWT path. Signs a token scoped to the given index using the same key, issuer and audience the shared fixture uses.
- `hydra_client_scoped_to_index(index_name)` — OAuth2 path. Registers a separate Hydra client carrying entry scopes for the given index.

The Hydra case deliberately registers a **separate** client rather than widening the shared one's scopes. Widening would have been a smaller diff but would quietly change what every other OAuth2 test exercises, which is the kind of change that makes a later failure hard to attribute.

## Why not the smaller fix

Seeding an entry into the *shared* index before asserting would also have removed the ordering dependency, and was the first option considered. It was rejected because it leaves the blast radius intact: if delete-on-non-empty ever regresses, a test operating on the shared index destroys it and nine unrelated tests fail with `Index ... not found` — burying the real defect under noise, exactly as it did here.

Owning the index means a regression fails one test, and that test names the actual problem.

## Challenges & Solutions

**The defect was mischaracterised as flaky.** It presented as intermittent — 10 failures once, clean on re-run — which invites waiting to see if it recurs. It was in fact deterministic in isolation and only *appeared* intermittent because whether it did damage depended on scheduling. Running the single test on its own was the cheapest possible diagnostic and settled it in seconds.

**A second instance was invisible from the failure.** `test_sdk_oauth2_delete_index_not_empty` has the same defect against the Hydra fixture and had never been implicated by an observed failure. It was found by running all 40 tests individually — worth keeping as a periodic check, since a suite that only ever runs whole cannot tell you which of its tests are load-bearing for others.

**Guarding against a vacuous fix.** A test that seeds its own precondition can pass for the wrong reason if the assertion stops biting. Verified by temporarily removing the seeding step: the test failed exactly as before, and passed again once restored.

## Technical Details

Leftover indexes are not cleaned up, deliberately: `init_postgres()` creates a throwaway database per run and drops it at exit, so per-test indexes cost nothing and cleanup code would be one more thing to get wrong.
