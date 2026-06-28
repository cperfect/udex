# Contributing — Rust
> This guide applies equally to AI agents and humans, unless otherwise stated.

> The key words "MUST", "SHOULD", "MAY", etc. are used as defined in [RFC 2119](https://www.ietf.org/rfc/rfc2119.txt).

See also [CONTRIBUTING.md](../../CONTRIBUTING.md) for project-wide guidelines that apply to all contributors.

## Code Style

1. **MUST follow the [Rust Style Guide](https://doc.rust-lang.org/style-guide/index.html)** — `rustfmt` handles most of this automatically.
2. **SHOULD follow the [Rust API Design Guidelines](https://rust-lang.github.io/api-guidelines/)** for internal libraries and APIs.

Run `cargo fmt --all` before committing. CI will reject unformatted code.

## Comments

Doc-comments (`///`) **MUST** be provided for all public functions, types, and modules.

Additional inline comments **SHOULD** be added for clarity when needed — e.g. complex algorithms, non-obvious choices, or use of a library where constraints are not self-evident. Such comments should explain *why*, not just *what*.

## Developing

A dev container configuration is provided for VS Code and compatible editors — this is the recommended way to get a consistent environment. Run `bash scripts/dev-doctor.sh` from the workspace root at any time to verify that all tools, services, and fixtures are in place.

The container pins the following versions:

| Component | Version | Defined in |
|-----------|---------|------------|
| Rust | 1.95.0 | `.devcontainer/Dockerfile` (`ARG RUST_VERSION`) |
| protoc | v34.1 | `.devcontainer/devcontainer.json` (`features.protoc.version`) |
| Ory Hydra | v26.2.0 | `projects/compose/docker-compose.yml` |
| PostgreSQL | 16 | `projects/compose/docker-compose.yml` |

To update any of these, change the version in the file listed above. For Hydra, also update `HYDRA_VERSION` and `HYDRA_TARBALL` in `.devcontainer/post-create.sh` and fetch the new SHA-256 from the corresponding `checksums.txt` release asset.

```bash
# Build the workspace
cargo build

# Run the full test suite (requires DATABASE_URL for integration tests)
DATABASE_URL=postgres://postgres:postgres@localhost:5432/postgres cargo test

# Check formatting (must produce no output)
cargo fmt --all -- --check

# Run static analysis (must produce no warnings)
cargo clippy --all-targets -- -D warnings
```

> **OAuth2 integration tests** — Tests prefixed with `test_*_oauth2_` require a live
> Hydra instance. In the devcontainer `HYDRA_PUBLIC_URL` and `HYDRA_ADMIN_URL`
> are set automatically (the devcontainer compose file points them at the `hydra`
> service). Just run `cargo test` — no filter needed.
>
> **k8s integration tests** — Tests prefixed with `test_sdk_k8s_` require a live
> k3d cluster with Udex deployed. Set `K8S_SERVER_URL=https://host.docker.internal:8443`
> (devcontainer default) or `https://localhost:8443` (CI). When `K8S_SERVER_URL` is not
> set the fixture returns `None` and every `test_sdk_k8s_*` test exits early — they are
> silently skipped in a normal `cargo test` run. Use `bash scripts/validate-k8s-test.sh`
> to run them locally; see [projects/k8s/README.md](../../projects/k8s/README.md) for
> cluster setup.
>
> **Multi-instance k8s tests** — Tests prefixed with `test_sdk_k8s_multi_` are a
> subset of the k8s tests that require the default 2-replica deployment. They
> address each server pod **directly** via `kubectl port-forward` (bypassing the
> round-robin load balancer) so a request can be pinned to a specific instance,
> proving the server is stateless across replicas (a write through one instance is
> visible through the other). Direct hops trust the pod cert (server CA, SNI
> `localhost`); the load-balanced path uses the Traefik edge cert.
>
> **Observability tests** — Tests prefixed with `test_obs_k8s_` assert that
> telemetry from the k3d deployment lands in the local observability stack (traces
> in Tempo, metrics in Prometheus, logs in Loki). They reuse the k8s fixture, so
> they skip when `K8S_SERVER_URL` is unset, and additionally skip (with a printed
> message) when the observability stack is not reachable. Bring the stack up with
> `bash projects/observability/scripts/up.sh` first; backend URLs default to the
> devcontainer service names and are overridable via `TEMPO_URL` / `PROMETHEUS_URL`
> / `LOKI_URL`.
>
> **Integration test naming** — Every integration test function must be prefixed
> with a layer indicator: `test_sdk_`, `test_sdk_oauth2_`, `test_sdk_k8s_`,
> `test_sdk_k8s_multi_`, `test_obs_k8s_`, `test_server_`, `test_server_oauth2_`,
> `test_index_service_`, `test_entry_service_`, `test_datastore_`, or `test_cli_`.
> This makes it immediately obvious from output which layer a failing test covers.
> Shared fixture helpers live in `udex-test-utils` — check there before duplicating
> fixture code.

### Pre-commit Checklist

Run these before every commit and fix any issues found:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

### Benchmarks

The project has a Criterion benchmark suite covering two layers of the Entry API hot path:

| Benchmark file | Layer | What it measures |
|---|---|---|
| `server/benches/entry_service.rs` | gRPC end-to-end | Full stack: client → auth → server → DB |
| `datastore/benches/postgres_datastore.rs` | Datastore direct | DB queries + connection pool only |

Running both and comparing the numbers quantifies the overhead added by gRPC, auth, and middleware.

#### Running benchmarks locally

```bash
# Requires DATABASE_URL pointing at a running PostgreSQL instance.

# Run all benchmarks (both crates)
cargo bench

# Run only the gRPC layer benchmarks
cargo bench --bench entry_service

# Run only the datastore layer benchmarks
cargo bench --bench postgres_datastore

# Compile-check benchmarks without running them (no DB required — same as CI)
cargo bench --no-run
```

Criterion writes HTML reports to `target/criterion/` after each run. Open `target/criterion/report/index.html` in a browser to view them.

#### Saving and comparing baselines

Baselines are local-only — `target/criterion/` is gitignored and no baseline is committed to the repo. To capture a named baseline for explicit comparison on your machine:

```bash
# Save the current results as a named baseline
cargo bench -- --save-baseline main

# Run benchmarks and compare against the saved baseline
cargo bench -- --baseline main
```

To update the baseline after an intentional performance change, re-run `--save-baseline` with the same name.

### Claude Skills

Two Claude Code skills are available to assist with code quality:

- **`/review-code`** — Reviews code against the guidelines in this file. On `main` it reviews all source files; on a feature branch it reviews the diff against `main`; pass a PR number to review a specific PR.

- **`/rust-guide`** — Applies Rust style, idiomatic patterns, and Udex-specific guidelines when writing or reviewing Rust code.

---

## Rust Development Guidelines

> The key words "MUST", "MUST NOT", "REQUIRED", "SHALL", "SHALL NOT", "SHOULD", "SHOULD NOT", "RECOMMENDED", "MAY", and "OPTIONAL" in this document are to be interpreted as described in [RFC 2119](https://www.ietf.org/rfc/rfc2119.txt).

1. **Udex MUST follow the Rust Style Guide**: https://doc.rust-lang.org/style-guide/index.html (`rustfmt` should take care of most of this).
1. **Errors SHOULD follow the NRC Error Design Guidelines**: https://nrc.github.io/error-docs/error-design/index.html
2. **Internal API/Libraries SHOULD follow the Rust Lang API Design Guidelines**: https://rust-lang.github.io/api-guidelines/

### Udex Specific Guidelines

### Errors
1. **thiserror crate SHOULD be used for errors declared by the code**
1. **APIs SHOULD NOT expose errors from 3rd party libraries or services** — these should be wrapped or converted to error types exposed by the API.
1. **errors SHOULD be called `<SomethingUseful>Error`** i.e. explicitly have the word Error at the end of the name

### Perform Local Checks before committing
(unless no rust code was changed)
- `cargo fmt --check`
- `cargo clippy`
- `cargo test`

and fix any issues found
