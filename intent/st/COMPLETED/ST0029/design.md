# Design - ST0029: Remove cross-test ordering dependencies in the SDK integration suite

## Approach

Two steps, in order: make the offending tests self-sufficient, then make the property enforceable so it cannot regress.

1. **Repair.** Both `delete_index_not_empty` tests create and populate an index of their own instead of borrowing the shared fixture's.
2. **Enforce.** A sweep that runs every test alone, wired into CI as its own step.

The second step is what makes the first durable. Without it the fix is a snapshot; the defect was latent for as long as the tests existed and nothing would have noticed it returning.

## Design Decisions

### Own the index, do not seed the shared one

Seeding an entry into the *shared* index before asserting would also have removed the ordering dependency, and was the first option considered. Rejected: it leaves the blast radius intact. If delete-on-non-empty ever regresses, a test operating on the shared index destroys it and nine unrelated tests fail with `Index ... not found` -- burying the real defect under noise, exactly as happened here. Owning the index means a regression fails one test, and that test names the actual problem.

### Scoped clients, because index and entry scopes differ

Index-level scopes in the test tokens are wildcarded, but **entry**-level scopes are bound to a named index. A test can therefore create its own index with the shared client but cannot put entries in it. Hence `client_scoped_to_index` (JWT path) and `hydra_client_for_owned_index` (OAuth2 path).

For the OAuth2 case a *separate* Hydra client is registered rather than widening the shared client's scopes. Widening would have been a smaller diff but would quietly change what every other OAuth2 test exercises, which is the kind of change that makes a later failure hard to attribute.

### Constant registration, so concurrent callers cannot race

`register_hydra_client` upserts (it PUTs on a 409) and Hydra persists clients in PostgreSQL across runs. A fixed `client_id` registered with a *per-index* scope would therefore be rewritten by every caller: two tests using the helper concurrently could overwrite each other's scope between registration and token fetch, and one would fail with a permission error instead of its real assertion -- a flake of precisely the kind this thread exists to remove. The scopes are wildcarded at the index-name position instead, so every registration is byte-identical and ordering stops mattering.

### Drive the binary, not cargo

The sweep invokes the compiled test binary directly. Going through `cargo test` once per test spent most of the wall clock in cargo's startup; direct invocation brought the sweep to ~20s, which is what makes gating every run affordable rather than a scheduled afterthought.

The cost is that this does not reproduce cargo's full runtime environment. The working directory is matched to cargo's (the package root, taken from cargo's own `manifest_path`), but variables cargo sets -- notably the dynamic library search path -- are not. Verified that no divergence is reachable with the suite as it stands; the residual gap and its remedy are documented in the script itself.

## Architecture

```text
scripts/validate-test-isolation.sh
  └─ cargo test --no-run --message-format=json   → executable path + manifest_path
       └─ <binary> --list                        → discovered test names + declared count
            └─ for each: (cd <package root> && <binary> <name> --exact)
                 └─ collect failures, report all, exit 1 if any
```

The declared count from `--list` is checked against the names parsed. A truncated listing would otherwise under-report coverage silently -- "20/20 pass" while skipping 23 -- which is the same class of failure the sweep exists to catch, so it must not be able to commit it itself.

## Alternatives Considered

**`cargo nextest`**, which runs process-per-test natively. Rejected: it is a new binary dependency needing a `dev-doctor` entry and a version-pin decision, and its parallelism would collide on the fixtures' fixed ports anyway -- so it would need `-j1`, at which point it is the same thing with an extra tool.

**Seeding the shared index** (see above) -- smaller diff, leaves the blast radius.

**Widening the shared Hydra client's scopes** instead of registering a second one -- smaller diff, silently changes what other OAuth2 tests exercise.

**A periodic sweep rather than a gate** -- the original suggestion, discarded once measurement showed the sweep costs ~20s.
