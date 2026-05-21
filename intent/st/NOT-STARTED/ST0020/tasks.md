# Tasks - ST0020: Immutable Index Hash Algorithms

## WP01 — Proto and generated code

- [ ] Remove `optional HashAlgorithm hash_algorithm = 7;` from `IndexUpdate` in `udex.index.v1.proto`; add a reserved comment for field 7
- [ ] Run `cargo build` to regenerate `api/src/generated/udex.index.v1.rs`
- [ ] Confirm compilation errors identify all affected call sites (do not fix yet — fix in WP02)

## WP02 — Server changes

- [ ] Change `ServerConfig.init_indexes` type from `Vec<UpdateIndexRequest>` to `Vec<CreateIndexRequest>` in `server/src/config.rs`
- [ ] Rewrite `IndexService::init()` to accept `Vec<CreateIndexRequest>`:
  - Create index if it does not exist
  - If it exists: error on hash_algorithm mismatch; update mutable fields otherwise
- [ ] Remove `hash_algorithm` from the empty-field guard in the `update_index` handler
- [ ] Fix all remaining compilation errors (authorizor tests, integration test fixtures, etc.)
- [ ] Validate: `cargo fmt --check`, `cargo clippy`, `cargo test`
- [ ] Commit WP01+WP02 together (proto and server are a single logical change)

## WP03 — Documentation and FAQ

- [ ] Add hash_algorithm immutability bullet to `projects/protobuf/README.md` key design points
- [ ] Create or extend a FAQ doc with the "Why can't I change the hash algorithm?" entry
- [ ] Commit docs

## Dependencies

WP02 depends on WP01 (generated types must compile). WP03 is independent and can be done any time.
