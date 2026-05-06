# Implementation - ST0009: One-to-one entry-context model

## Schema

Replaced the two-table model (`entry` + `context`) with a single `entry_context` table.
`UNIQUE(context_hash)` enforces the 1:1 constraint at the database level — no application-layer
deduplication loop needed. Write path uses `ON CONFLICT (context_hash) DO NOTHING`.

## Key Decisions

- `create_entry` returns the actual stored key (`Uuid`) so callers get the pre-existing key
  on idempotent creates, rather than discovering the collision via a separate lookup.
- `bulk_entry_write` returns `Vec<EntryWriteResult>` (one per operation) for the same reason.
- `get_entry_by_context` returns `Option<Entry>` (not an error) when no entry exists.
- `dek`/`kek_id` are stored in `entry_context` but are NOT part of the context hash. The hash
  covers only `pairs`. Callers that need DEK consistency must read back via `lookup_context_by_key`.
- Removed `DuplicateKey` and `ContextEncryptionConflict` error variants — both were artifacts of
  the old two-table model.

## Benchmark Baselines

Two Criterion baselines captured using `--save-baseline`:

| Baseline   | Schema          | Captured     |
|------------|-----------------|--------------|
| two-table  | entry + context | WP-01        |
| one-table  | entry_context   | WP-06        |

### Commands

```bash
# Capture one-table baseline (run after WP-04/05 merged)
cargo bench --bench entry_service -- --save-baseline one-table

# Compare against two-table baseline
cargo bench --bench entry_service -- --baseline two-table
```

### Results

Captured on 2026-05-06. All benchmarks use a real PostgreSQL instance with the full gRPC stack.

| Benchmark                  | two-table | one-table | Δ       |
|----------------------------|-----------|-----------|---------|
| grpc/entry/create          | 1.26 ms   | 0.91 ms   | -27.5%  |
| grpc/entry/get_by_key      | 356.2 µs  | 284.7 µs  | -20.1%  |
| grpc/entry/get_by_context  | 367.4 µs  | 286.7 µs  | -22.0%  |
| grpc/entry/delete          | 4.08 ms   | 0.89 ms   | **-78.1%** |
| grpc/bulk_write/10         | 2.49 ms   | 1.63 ms   | -34.4%  |
| grpc/bulk_write/100        | 12.75 ms  | 8.84 ms   | -30.7%  |
| grpc/bulk_write/1000       | 98.1 ms   | 109.2 ms  | +11.2%* |
| grpc/bulk_read/10          | 1.57 ms   | 1.33 ms   | -15.1%  |
| grpc/bulk_read/100         | 12.37 ms  | 11.74 ms  | -5.1%   |
| grpc/bulk_read/1000        | 110.7 ms  | 118.5 ms  | +7.1%** |

\* `bulk_write/1000` change is not statistically significant (p = 0.53).

\*\* `bulk_read/1000` apparent +7% is a benchmark artefact: the two-table seed used a
single repeated context (all 1000 reads hit the same cached row), while the one-table
seed uses 1000 distinct contexts (realistic cold-read pattern).

**Headline**: `delete` improved 78% (eliminated context-table GC scan); `create` and
reads improved 20–27% (single-table access, no JOIN); bulk writes improved 30–34% at
practical sizes (10–100 ops).
