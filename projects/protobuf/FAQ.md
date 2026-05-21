# Udex API — Frequently Asked Questions

## Why can't I change an index's hash algorithm?

`Index.hash_algorithm` is fixed at creation time and cannot be changed via `UpdateIndex`. Attempting to do so is rejected.

The hash algorithm determines how every context stored in the index was fingerprinted. Changing it after entries exist would cause three classes of silent failure:

1. **Stale cached hashes.** Any client that computed or cached a context hash under the old algorithm would produce hashes that match no entry — lookups return empty rather than an error, making the failure invisible.

2. **Concurrent-write race.** During a change window, concurrent writers would hash the same context under different algorithms and produce two distinct entries with the same logical key. There is no mechanism to detect or reconcile this divergence.

3. **No server-side cache invalidation.** The server caches the hasher function for each index at first use. The cache has no invalidation path for algorithm changes; the only safe design is to make the algorithm immutable.

**What to do instead:** If you need an index with a different algorithm, delete the existing index (which requires it to be empty — delete all entries first) and create a new one with the desired algorithm, then re-ingest your entries.
