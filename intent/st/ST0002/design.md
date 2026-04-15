---
verblock: "15 Apr 2026:v0.1: vscode - Initial design"
---

# ST0002: CLI Design

## Overview

A single `udex` binary with subcommands, implemented as the `udex-cli` crate at `projects/rust/cli/`. The crate scaffold (Cargo.toml, README) already exists and is excluded from the workspace pending implementation.

The binary serves two distinct modes:
1. **Server mode** (`udex serve`) — starts the Udex gRPC server in the foreground, embedding `udex-server`
2. **Client mode** (all other commands) — connects to a running server via gRPC and issues operations

## Design Decisions

### D1: Single binary, subcommand structure

One `udex` binary replaces the notion of a separate server binary. The server crate remains a library; `udex serve` is the entry point for running the server.

```bash
udex serve                      # start the server (foreground)
udex config init                # generate a default config file
udex config validate            # validate an existing config file
udex index list|create|get|update|delete
udex entry create|get|lookup|delete
udex token inspect              # inspect a JWT (offline)
udex context hash               # compute a context hash (offline)
```

**Rationale**: One binary to install and document; simpler for operators. `udex serve` in foreground is idiomatic for containerised deployments (systemd/Docker manage the lifecycle).

Server stop is not a CLI concern for v1 — the process is managed by the OS/container runtime (Ctrl+C / SIGTERM). This is deferred.

### D2: CLI framework — clap v4 with derive macros

clap is already declared as a workspace dependency in the scaffolded Cargo.toml. Use the `derive` feature for subcommand structs. This is the canonical choice in the Rust ecosystem.

### D3: Authentication — bearer token via flag or environment variable

The server validates JWTs. The CLI accepts a pre-obtained token:

| Precedence | Source |
|---|---|
| 1 (highest) | `--token <TOKEN>` flag |
| 2 | `UDEX_TOKEN` environment variable |

The CLI does **not** implement OAuth Client Credentials flow in v1 — obtaining the token is out of scope. The token must be provided by the caller (e.g. via `curl` against the authorisation server, or a script). This keeps the CLI independent of any specific authorisation server.

TLS CA certificate for verifying the server:

| Precedence | Source |
|---|---|
| 1 | `--ca-cert <PATH>` flag |
| 2 | `UDEX_CA_CERT` environment variable |
| 3 | System trust store |

Server address:

| Precedence | Source |
|---|---|
| 1 | `--server <URL>` flag |
| 2 | `UDEX_SERVER` environment variable |
| 3 | `https://localhost:50051` (default) |

### D4: Config file format — TOML

TOML is the idiomatic Rust config format. The existing `ServerConfig` and `DatastoreConfig` structs already derive `Serialize`/`Deserialize` and can be serialised directly.

Config file location resolution order:
1. `--config <PATH>` flag
2. `UDEX_CONFIG` environment variable
3. `./udex.toml`

`udex config init` writes a default config with inline comments explaining each field. `udex config validate` loads, resolves env var placeholders, and runs the existing `.validate()` methods.

### D5: Output format — table (default), JSON, YAML

All commands that return data support `--output table|json|yaml`. Table is the default (human-friendly); JSON/YAML are for scripting and piping.

`tabled` is already in the scaffolded Cargo.toml for table rendering.

### D6: Command terminology

Following the architecture's terminology:

- `entry` — the key↔context mapping (not "context", which has a specific meaning as the key/value pairs)
- `index` — the namespace of entries
- `context hash` — offline utility to compute the hash of a set of key/value pairs

This aligns with the gRPC service names (`EntryService`, `IndexService`).

### D7: `udex-server` dependency

The CLI crate depends on `udex-server` (for `udex serve`) and `udex-api` (for gRPC client stubs and type definitions). It does **not** depend on `udex-datastore` directly — the datastore is configured through the server.

## Command Reference (v1 scope)

### Server

```bash
udex serve [--config <PATH>]
```
Starts the gRPC server in the foreground. Reads config from the resolved config file (D4). Exits on SIGTERM/Ctrl+C.

### Config

```bash
udex config init [--output <PATH>]      # write default config to ./udex.toml (or --output)
udex config validate [--config <PATH>]  # load, resolve, validate; exit non-zero on error
```

### Index (requires running server + auth)

```bash
udex index list
udex index create <name> [--bulk-limit <n>] [--description <text>]
udex index get <name>
udex index update <name> [--bulk-limit <n>] [--description <text>]
# udex index delete <name>  — hidden; DeleteIndex RPC not yet defined in the proto
```

### Entry (requires running server + auth)

```bash
udex entry create <index> --context <key=value>...
udex entry get <index> --key <uuid>
udex entry lookup <index> --context <key=value>...
udex entry delete <index> --key <uuid>
```

Context key/value pairs are passed as repeated `--context key=value` flags. Parsing produces a `Context` proto message.

### Offline utilities

```bash
udex token inspect <TOKEN>                    # decode JWT header+claims (no signature verification)
udex context hash --context <key=value>...    # compute and print context hash
```

## Error Handling

Errors are printed to stderr; data output goes to stdout. `--verbose` enables tracing output.

Both gRPC status codes and transport-level errors produce distinct exit codes so callers can branch without parsing error messages:

| Exit | Cause |
|------|-------|
| 0 | success |
| 1 | unclassified / internal error |
| 2 | gRPC `NOT_FOUND` |
| 3 | gRPC `ALREADY_EXISTS` |
| 4 | gRPC `INVALID_ARGUMENT`, `FAILED_PRECONDITION`, or `OUT_OF_RANGE` |
| 5 | gRPC `UNAUTHENTICATED` |
| 6 | gRPC `PERMISSION_DENIED` |
| 7 | gRPC `UNAVAILABLE` or `DEADLINE_EXCEEDED` (server reachable but overloaded/timing out) |
| 8 | Transport connection failure (connection refused, TLS error, DNS failure) |

The implementation walks the `anyhow` error-source chain looking for a `tonic::Status` (gRPC codes → 2–7) or `tonic::transport::Error` (transport failures → 8). Non-gRPC errors exit 1.

## Testing Strategy

### Offline commands

`assert_cmd` spawns the `udex` binary as a subprocess and asserts on exit code, stdout, and stderr. No infrastructure needed.

### Online commands

Same pattern as `server_integration_tests.rs`:

1. `init_postgres()` — creates a fresh isolated database from `DATABASE_URL` (provided by devcontainer docker-compose)
2. Start the server in-process via `server::serve()` as a background tokio task, using the test TLS certs (`tests/certs/`) and JWT keys (`tests/jwt/`) already present in the server crate
3. Generate a test JWT with `generate_test_claims()` + `EncodingKey::from_ec_pem()`
4. Invoke the `udex` binary via `assert_cmd` with env vars set:
   - `UDEX_SERVER=https://127.0.0.1:<test_port>`
   - `UDEX_TOKEN=<generated_jwt>`
   - `UDEX_CA_CERT=<path_to_test_ca_cert>`

The `init_server()` helper will be duplicated into `cli/tests/` rather than extracted to a shared crate — the project guideline explicitly favours copying over premature abstraction. Extract to a shared test-support crate only if a third consumer appears.

### Test infrastructure summary

| Test type | Tool | Infrastructure |
|---|---|---|
| Offline commands | `assert_cmd` | None |
| Online commands | `assert_cmd` + in-process server | `DATABASE_URL` (devcontainer) |
| Help / usage errors | `assert_cmd` | None |

`assert_cmd` and `predicates` are the only new test dependencies required.

## Alternatives Considered

**Subcommands on the server binary**: Rejected — the server crate is a library with no `main.rs`. Adding CLI concerns there would conflate two responsibilities. A dedicated crate is cleaner.

**OAuth Client Credentials flow in v1**: Deferred — adds a dependency on a specific token endpoint and credential management. Pre-obtained token keeps the CLI simple and auth-server-agnostic for now.

**`context` as the entry subcommand name**: Rejected — "context" has a specific meaning in the data model (the key/value pairs). Using `entry` matches the gRPC service name and the architecture docs.

## Out of Scope for v1

- OAuth Client Credentials flow (D3)
- `udex serve stop` / `udex serve status` — lifecycle managed externally
- Bulk operations via CLI
- SDK generation
- Shell completion generation
