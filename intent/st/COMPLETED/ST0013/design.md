# Design - ST0013: Lookup Or Create Entry

## Approach

ST0013 implements a lookup-or-create flow for entries identified by a stable business key. The operation first normalizes and validates the incoming attributes, then attempts to find an existing entry using the canonical lookup fields. If a matching entry is found, the existing record is returned unchanged. If no match is found, a new entry is created using the validated request data and the newly created record is returned.

The intended behavior is idempotent for repeated requests that carry the same identifying data. The design assumes that lookup is cheaper and less disruptive than creating duplicates and reconciling them later, so create is treated as a fallback path rather than the default path.

## Design Decisions

1. **Lookup before create.** The primary decision is to always attempt a read using the entry's natural key before inserting a new record. This reduces duplicate data and makes repeat requests safe.
2. **Canonical matching inputs.** Lookup fields should be normalized before comparison so that insignificant formatting differences do not produce duplicate entries.
3. **Create only on confirmed miss.** A new record is created only when the lookup returns no existing match. Validation errors stop the request before either lookup-dependent updates or creation occur.
4. **Database-enforced uniqueness.** Uniqueness must be enforced at persistence level for the lookup key so concurrent requests cannot create duplicate entries. If two requests race, one may succeed in creating while the other retries lookup and returns the existing row.
5. **No implicit merge on partial matches.** If the system finds ambiguous or conflicting candidates, the request should fail clearly rather than guessing which record to reuse.

## Architecture

The flow is:

1. Receive request to lookup or create an entry.
2. Normalize the identifying fields into a canonical form.
3. Query storage for an existing entry using the canonical key.
4. If found, return the existing entry.
5. If not found, attempt to insert a new entry protected by a uniqueness constraint.
6. If insert fails because another request created the same entry concurrently, perform the lookup again and return the now-existing entry.

This keeps responsibility split cleanly: request validation and normalization happen at the boundary, lookup-or-create orchestration happens in the application layer, and uniqueness guarantees live in the data store.

## Alternatives Considered

- **Always create, then deduplicate later.** Rejected because it increases operational cleanup cost and allows avoidable duplicate records.
- **Create first and rely only on unique-constraint errors.** Rejected as the primary strategy because it turns normal control flow into exception-driven flow and makes successful reads more expensive than necessary.
- **Manual resolution for every uncertain case.** Rejected because the common path should be automatic; only ambiguous matches should require intervention.

This design was chosen because it provides predictable behavior for callers, minimizes duplicate entry creation, and remains safe under concurrent requests.
