# Implementation - ST0002: Command Line Interface

## Implementation

`udex-cli` is a single `udex` binary (`projects/rust/cli/`) with two modes:

- **Server mode** (`udex serve`) — starts the embedded gRPC server in the foreground
- **Client mode** (all other commands) — dials the server over mTLS and issues RPCs

Subcommands mirror the gRPC services: `index` wraps `IndexServiceClient`, `entry` wraps `EntryServiceClient`. Two offline utilities (`token inspect`, `context hash`) require no server connection.

### Source layout

```
cli/
  src/
    main.rs           — entry point: installs rustls provider, parses CLI, maps errors to exit codes
    cli.rs            — clap derive structs (Cli, Commands, *Args)
    client.rs         — ClientConfig: channel() + interceptor() shared by all client commands
    commands/
      serve.rs        — udex serve: delegates to udex_server::serve()
      config.rs       — udex config init|validate
      index.rs        — udex index list|create|get|update|delete
      entry.rs        — udex entry create|get|lookup|delete
      token.rs        — udex token inspect (offline)
      context.rs      — udex context hash (offline)
  tests/
    cli_tests.rs      — assert_cmd integration tests (no server required)
    serve_tests.rs    — serve smoke tests
```

### Exit codes

`main.rs::grpc_exit_code()` walks the `anyhow` source chain and maps causes to distinct codes so callers can branch without parsing error messages:

| Exit | Cause |
|------|-------|
| 0 | success |
| 1 | unclassified / internal error |
| 2 | gRPC `NOT_FOUND` |
| 3 | gRPC `ALREADY_EXISTS` |
| 4 | gRPC `INVALID_ARGUMENT`, `FAILED_PRECONDITION`, or `OUT_OF_RANGE` |
| 5 | gRPC `UNAUTHENTICATED` |
| 6 | gRPC `PERMISSION_DENIED` |
| 7 | gRPC `UNAVAILABLE` or `DEADLINE_EXCEEDED` |
| 8 | Transport failure (connection refused, TLS error, DNS failure) |

## Code Examples

### Channel + interceptor pattern (client.rs)

Every online command builds a channel and wraps it with a bearer-token interceptor in two lines:

```rust
let channel = client.channel().await?;
let mut grpc = IndexServiceClient::with_interceptor(channel, client.interceptor());
```

`channel()` reads `--ca-cert` for a custom PEM trust anchor; falls back to the system trust store. `interceptor()` injects `Authorization: Bearer <token>` when `--token` or `UDEX_TOKEN` is set.

### context hash (entry.rs / context.rs)

`udex entry lookup` computes the SHA-1 context hash **locally** before sending it to the server, matching the server's own hashing logic:

```rust
let context = build_context_input(&args.context)?;
let context_hash = sha1_context_hash(&context).context("failed to compute context hash")?;
grpc.lookup_keys_by_context(LookupKeysByContextRequest { index_name, context_hash }).await?;
```

The offline `udex context hash` command does the same thing without a connection — useful for pre-computing hashes or debugging mismatches.

### exit code chain-walk (main.rs)

`anyhow::Error` does not implement `std::error::Error`, so `downcast_ref` on `anyhow::Error` itself does not work. The trick is `e.as_ref()` which yields `&(dyn StdError + 'static)`, giving a `source()` chain that supports `downcast_ref::<tonic::Status>()`:

```rust
let root: &(dyn StdError + 'static) = e.as_ref();
let mut src = root.source();
while let Some(cause) = src {
    if let Some(status) = cause.downcast_ref::<tonic::Status>() { ... }
    if cause.downcast_ref::<tonic::transport::Error>().is_some() { return 8; }
    src = cause.source();
}
```

### CLI session example

```bash
# Start server (needs udex.toml):
udex serve --config /etc/udex/udex.toml

# Create an index:
udex index create users --description "user directory" --bulk-limit 200

# Create an entry:
udex entry create users --context name=alice --context role=admin
# → prints key (UUID) + context_hash

# Look up the entry by context (hash computed locally):
udex entry lookup users --context name=alice --context role=admin
# → prints matching key UUIDs

# Inspect a JWT without a server:
udex token inspect eyJhbGci...
# → prints header + claims JSON; flags expiry

# Compute context hash offline:
udex context hash --context name=alice --context role=admin
# → 40-char hex SHA-1 (order-independent)

# Scripting: check exit code
udex index get nonexistent; echo "exit: $?"
# → exit: 2  (NOT_FOUND)
```

## Technical Details

### TLS is mandatory

`ClientTlsConfig` is always applied to the channel. There is no plaintext mode — the server only accepts TLS connections. The `rustls` `aws_lc_rs` crypto provider must be installed before any channel is built:

```rust
rustls::crypto::aws_lc_rs::default_provider()
    .install_default()
    .expect("failed to install rustls CryptoProvider");
```

This call is the first thing in `main()`. Forgetting it causes a panic at channel-connect time with `no process-level CryptoProvider available`.

### Context hash is order-independent

`sha1_context_hash()` (from `udex-api`) sorts `KeyValuePair`s by key before hashing, so
`--context a=1 --context b=2` and `--context b=2 --context a=1` produce the same hash. The integration test `test_context_hash_is_deterministic` in `cli/tests/cli_tests.rs` asserts this property explicitly.

### `udex index delete` is stubbed

`IndexServiceClient` has no `delete_index` RPC in the current protobuf definition. The subcommand is registered but marked `#[command(hide = true)]` in `cli.rs` so it does not appear in `--help`. The handler still calls `anyhow::bail!` and exits 1 if invoked. Once the RPC is added to `udex.index.v1.proto` and the server handler is implemented, remove the `hide` attribute and implement the client call in `commands/index.rs`.

### `u32` → `i32` conversions for proto fields

The proto fields for index limits are `int32` (signed), but the CLI args are `u32` for user-facing clarity. Each create/update handler does an explicit `i32::try_from(args.bulk_limit)` that produces a clear error if the value overflows — rather than silent truncation.

### `--verbose` must be set before the logger is initialised

The `--verbose` flag sets `RUST_LOG=debug` only when `RUST_LOG` is not already set. It **must** be evaluated before `tracing_subscriber` initialisation inside `udex_server::serve()`. The current ordering in `main()` guarantees this.

## Challenges & Solutions

- **`anyhow::Error` does not implement `std::error::Error`**: Casting `e` as `&dyn StdError` fails at compile time. Fixed with `e.as_ref()` which returns `&(dyn StdError + Send + Sync + 'static)` — an entry point into the source chain that supports `downcast_ref`.

- **rustls CryptoProvider panic at runtime**: Integration tests that ran `udex index list` against a live server panicked with `no process-level CryptoProvider available`. Fixed by calling `rustls::crypto::aws_lc_rs::default_provider().install_default()` as the first statement in `main()` and adding `rustls` as an explicit dependency in `cli/Cargo.toml`.

- **`tonic::transport::Error` cannot be constructed in unit tests**: Attempted to write a unit test for exit code 8 by constructing a `tonic::transport::Error` from `std::io::Error`, but the `From` impl is not public. Removed the unit test; transport-failure exit code 8 is covered by the integration test `test_index_list_fails_without_server` which expects `.code(8)` from a real connection-refused error.

- **`IndexServiceClient` has no delete RPC**: The proto for `IndexService` does not define a `DeleteIndex` method. Rather than removing it from the CLI surface, the subcommand is hidden with `#[command(hide = true)]` so it is absent from `--help` but the argument surface is stable for when the RPC is added. The handler still `bail!`s if invoked directly.

- **Transport errors vs. gRPC UNAVAILABLE confusion**: Early versions mapped both transport failures (connection refused) and gRPC `UNAVAILABLE` to exit 7. This made it impossible for callers to distinguish "server is overloaded" from "cannot reach the server at all". Resolved by splitting: gRPC status codes → 2–7, transport-level `tonic::transport::Error` → 8.
