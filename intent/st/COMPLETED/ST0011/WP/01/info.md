---
verblock: "12 May 2026:v0.1: vscode - Initial version"
wp_id: WP-01
title: "Proto and generated Rust types"
scope: Small
status: Done
---

# WP-01: Proto and generated Rust types

## Objective

Add `rpc DeleteIndex` to the `IndexService` service definition in the proto and update the hand-maintained generated Rust file to expose the new RPC on both the client and the server trait.

## Deliverables

- `projects/protobuf/udex.index.v1.proto`: add `rpc DeleteIndex(DeleteIndexRequest) returns (DeleteIndexResponse);` to the `IndexService` service block (messages are already defined)
- `projects/rust/api/src/generated/udex.index.v1.rs`:
  - Add `delete_index` to `IndexServiceClient<T>` (follow the `list_indices` pattern)
  - Add `async fn delete_index` to the `IndexService` server trait in `index_service_server`
  - Add the routing arm for `/udex.index.v1.IndexService/DeleteIndex` in the `Service` impl

## Acceptance Criteria

- [ ] `rpc DeleteIndex` appears in the proto service block
- [ ] `IndexServiceClient::delete_index` is callable
- [ ] The `IndexService` server trait requires `delete_index` to be implemented
- [ ] `cargo build -p udex-api` passes

## Dependencies

- None
