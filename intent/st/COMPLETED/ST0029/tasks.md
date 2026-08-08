---
verblock: "08 Aug 2026:v0.1: vscode - Initial version"
intent_version: 2.15.0
---

# Tasks - ST0029: Remove cross-test ordering dependencies in the SDK integration suite

Acceptance lives in `acceptance.md`; as-built detail in `impl.md`. This file is the sequencing view.

## Tasks

- [x] Reproduce the defect deterministically -- `cargo test test_sdk_delete_index_not_empty` in isolation failed 100% of the time, proving it was not flakiness
- [x] Make `test_sdk_delete_index_not_empty` self-sufficient (create and populate its own index via `client_scoped_to_index`)
- [x] Sweep all tests individually to find further instances -- surfaced `test_sdk_oauth2_delete_index_not_empty`, which no observed failure had ever implicated
- [x] Make `test_sdk_oauth2_delete_index_not_empty` self-sufficient (`hydra_client_for_owned_index`, wildcard scopes so concurrent callers cannot race)
- [x] Record the new Hydra client secret in `docs/SECRETS.md`
- [x] WP-01 -- `scripts/validate-test-isolation.sh` plus the `Test isolation` CI step and the rule in `projects/rust/CONTRIBUTING.md`

## Task Notes

The order mattered. Reproducing first settled what the defect actually was: it presented as intermittent, but the test could never pass alone, and only the *collateral damage* depended on scheduling. Sweeping before declaring victory is what found the second instance -- one fix would have looked complete and left half the defect in place.

Two verifications guard against a fix that passes for the wrong reason:

- **Vacuity.** Removing the seeding step reproduced the original failure, confirming the assertion still bites.
- **The gate can go red.** Reintroducing the ordering dependency made the sweep report `42/43`, name the test, print a reproduction command and exit 1. A gate that cannot fail is worse than no gate, because it reads as assurance.

## Dependencies

```text
reproduce → fix JWT test → sweep → fix OAuth2 test → WP-01 (gate)
```

WP-01 depends on both repairs landing first; wiring a gate before the suite is clean would make it red on arrival.

Evidence: 40/40 (later 43/43) individually, two consecutive full-package runs at 60 tests with `K8S_SERVER_URL` set, and the red-first checks above.
