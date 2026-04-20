---
verblock: "20 Apr 2026:v0.1: vscode - Initial version"
intent_version: 2.4.0
status: Completed
slug: better-hash-algorithm
created: 20260420
completed:
---

# ST0005: Better hash algorithm

## Objective

Replace the SHA-1 context hashing algorithm with a fast, non-cryptographic hash (xxHash) that is better suited to creating stable data identities and minimising collisions at scale.

## Context

Context hashes are the primary identity mechanism in Udex: identical key-value pairs must always produce the same hash, and different pairs must produce different hashes. SHA-1 was chosen as the initial implementation but has several drawbacks for this use case:

- **Wrong tool**: SHA-1 is a cryptographic hash designed for tamper detection, not identity. Its security properties (preimage resistance, avalanche effect) are unnecessary overhead here.
- **Speed**: SHA-1 is significantly slower than purpose-built non-cryptographic hashes at the same collision resistance for short inputs.
- **Collision risk**: SHA-1 has known theoretical weaknesses and is considered broken for cryptographic purposes; while practical collisions in this context are unlikely, using a stronger algorithm by design is preferable.
- **Algorithm field exists**: The `hash_algorithm` column is already present in the schema and proto definitions, so the infrastructure for multi-algorithm support is in place.

### Algorithm choice: xxHash (xxh3)

| Algorithm | Speed | Collision resistance | Cryptographic | Notes |
|-----------|-------|---------------------|---------------|-------|
| SHA-1 | Slow | Excellent for crypto | Yes (broken) | Current implementation |
| xxHash (xxh3) | Very fast | Excellent | No | Recommended |
| FNV | Fast | Moderate | No | Simpler but weaker |
| SipHash | Moderate | Good | No (MAC) | Std default, DoS-resistant |

xxHash (xxh3) is the recommended replacement:
- Industry-standard non-cryptographic hash designed for fast, stable data identity
- Excellent collision resistance for arbitrary-length inputs
- Widely used for content-addressable storage and deduplication
- Available via the `xxhash` crate

## Scope

- Replace `SHA1` with `XXH3` in the `HashAlgorithm` protobuf enum and regenerate
- Replace `sha1_context_hash` with `xxh3_context_hash` in `udex-api`; remove the SHA-1 implementation
- Update the server, CLI, and all call-sites to use xxh3
- Update all tests, benchmarks, and documentation

## Related Steel Threads

- ST0004 (Benchmark testing) — benchmarks should be re-run after the switch to validate speed improvement

## Context for LLM

This document represents a single steel thread - a self-contained unit of work focused on implementing a specific piece of functionality. When working with an LLM on this steel thread, start by sharing this document to provide context about what needs to be done.

### How to update this document

1. Update the status as work progresses
2. Update related documents (design.md, impl.md, etc.) as needed
3. Mark the completion date when finished

The LLM should assist with implementation details and help maintain this document as work progresses.
