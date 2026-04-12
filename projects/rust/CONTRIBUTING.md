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

Requires Rust stable. A dev container configuration is provided for VS Code and compatible editors — this is the recommended way to get a consistent environment.

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

### Pre-commit Checklist

Run these before every commit and fix any issues found:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

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
- `cargo fmt --check`
- `cargo clippy`
- `cargo test`

and fix any issues found
