# Tasks - ST0002: Command Line Interface

## Work Packages

| WP    | Title                                              | Status      |
|-------|----------------------------------------------------|-------------|
| WP-01 | Wire cli crate into workspace                      | Not Started |
| WP-02 | clap command skeleton                              | Not Started |
| WP-03 | `udex config init` and `udex config validate`      | Not Started |
| WP-04 | `udex serve`                                       | Not Started |
| WP-05 | gRPC client connection helper                      | Not Started |
| WP-06 | `udex index` commands                              | Not Started |
| WP-07 | `udex entry` commands                              | Not Started |
| WP-08 | `udex token inspect` and `udex context hash`       | Not Started |
| WP-09 | Output formatting (`--output table\|json\|yaml`)   | Not Started |

## Dependencies

```
WP-01 (workspace)
  └── WP-02 (skeleton)
        ├── WP-03 (config)
        ├── WP-04 (serve)       depends on WP-03
        ├── WP-05 (gRPC client) 
        │     ├── WP-06 (index) depends on WP-09
        │     └── WP-07 (entry) depends on WP-09
        └── WP-08 (offline utils)
        WP-09 (output formatting) — can be done alongside WP-05
```

## Work Package Detail

### WP-01: Wire cli crate into workspace
Re-enable `"cli"` in the workspace `Cargo.toml` members list. Verify `cargo build` succeeds with no `src/` yet (add a minimal `src/main.rs`). Add `assert_cmd` and `predicates` to `[dev-dependencies]`.

### WP-02: clap command skeleton
Implement the full command/subcommand structure using clap derive macros — all commands, all flags — with stubbed handlers that print "not implemented". Includes the global flags: `--server`, `--token`, `--ca-cert`, `--output`, `--verbose`. No business logic yet. Tests: help text renders correctly, unknown subcommands exit non-zero.

### WP-03: `udex config init` and `udex config validate`
`config init` serialises `ServerConfig::default()` + `DatastoreConfig::default()` to TOML and writes to `--output` path (default `./udex.toml`). `config validate` reads the file, resolves `${VAR}` placeholders, and calls `.validate()`. Tests: offline, no server needed.

### WP-04: `udex serve`
Implement `udex serve`: load config from the resolved path (D4), initialise the datastore, call `server::serve()`. Integrates with `init_tracing()` / `init_test_tracing()` via `--verbose`. Tests: server starts and responds to a health check.

### WP-05: gRPC client connection helper
Shared internal helper that builds a `tonic::Channel` from `--server`, `--token`, and `--ca-cert` (following D3 resolution order). Used by all online command handlers. Tests: connection refused and invalid token produce correct error messages and non-zero exit codes.

### WP-06: `udex index` commands
Implement `list`, `create`, `get`, `update`, `delete` against `IndexServiceClient`. Tests: integration tests using the in-process server pattern (see Testing Strategy in design.md), covering the happy path and key error cases for each subcommand.

### WP-07: `udex entry` commands
Implement `create`, `get`, `lookup`, `delete` against `EntryServiceClient`. Context key/value pairs parsed from repeated `--context key=value` flags. Tests: same in-process server pattern as WP-06.

### WP-08: `udex token inspect` and `udex context hash`
`token inspect` decodes a JWT without signature verification and prints header + claims. `context hash` parses `--context` flags and calls `udex_api::hash`. Both are fully offline. Tests: assert_cmd with known inputs and expected outputs.

### WP-09: Output formatting (`--output table|json|yaml`)
Implement the `--output` flag for all commands that return data. Table rendering via `tabled`; JSON via `serde_json`; YAML via `serde_yaml`. Can be developed in parallel with WP-05. Tests: same command invoked with each output flag produces correctly formatted output.
