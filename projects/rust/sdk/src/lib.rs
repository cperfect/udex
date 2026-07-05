//! `udex-sdk` — Rust client library for [Udex](https://github.com/cperfect/udex).
//!
//! Provides a high-level, idiomatic Rust API over the Udex gRPC service:
//! TLS channel construction, transparent OAuth2 client-credentials token
//! management, and strongly-typed wrappers for every entry and index
//! operation.
//!
//! # Quick start
//!
//! ```no_run
//! use udex_sdk::{UdexClient, ClientOptions};
//!
//! # #[tokio::main]
//! # async fn main() -> Result<(), udex_sdk::Error> {
//! let client = UdexClient::connect(
//!     ClientOptions::builder()
//!         .endpoint("https://localhost:50051")
//!         .ca_cert_pem_file("certs/ca.pem")
//!         .client_credentials(
//!             "https://auth.example.com/oauth2/token",
//!             "my-client-id",
//!             "my-client-secret",
//!         )
//!         .build()?,
//! )
//! .await?;
//! # Ok(())
//! # }
//! ```
//!
//! # Tracing and OpenTelemetry
//!
//! Every entry/index call is wrapped in a `tracing` span (`sdk.*`). The SDK is
//! **provider-free**: it never installs an OpenTelemetry tracer/meter provider —
//! the host application owns that. It uses only the vendor-neutral `opentelemetry`
//! API to read the ambient context and inject a W3C `traceparent` into each
//! outgoing request, so the server continues the caller's trace.
//!
//! As a result, SDK spans nest under whatever span is current when you call them.
//! A host application that has set up OpenTelemetry sees one connected trace from
//! its own span through the SDK call into the server:
//!
//! ```no_run
//! # async fn example(client: &udex_sdk::UdexClient) -> Result<(), udex_sdk::Error> {
//! // The host owns telemetry setup (tracer provider + W3C propagator). Within any
//! // of its spans, SDK calls become child spans and propagate `traceparent`:
//! let span = tracing::info_span!("checkout");
//! let _enter = span.enter();
//! let indices = client.list_indices().await?; // `sdk.list_indices` nested under `checkout`
//! # let _ = indices;
//! # Ok(())
//! # }
//! ```
//!
//! When no OpenTelemetry provider/propagator is installed, the spans are ordinary
//! `tracing` spans and no `traceparent` is emitted — non-OTel callers are
//! unaffected.

pub mod auth;
pub mod client;
pub mod entry;
pub mod error;
pub mod health;
pub mod index;
pub mod json;

pub use client::{ClientOptions, ClientOptionsBuilder, UdexClient};
pub use error::{grpc_code, Error, RpcStatus};
pub use health::HealthStatus;
pub use json::context_input_from_json;

// Re-export every proto type callers need to build requests and read responses.
// `udex-sdk` is meant to be a client's single Udex dependency: application and
// test code should import these from `udex_sdk`, never reach into `udex-api`
// directly. If a client operation needs a type that is not re-exported here, add
// it — that gap is a bug in this list.
pub use udex_api::entry::{
    bulk_read_entry_operation, bulk_read_entry_operation_result, bulk_write_entry_operation,
    bulk_write_entry_operation_result, value, BulkReadEntryOperation, BulkReadEntryOperationResult,
    BulkWriteEntryOperation, BulkWriteEntryOperationResult, Context, ContextInput,
    CreateEntryRequest, CreateEntryResponse, DeleteEntryRequest, DeleteEntryResponse, KeyValuePair,
    LookupContextByKeyRequest, LookupContextByKeyResponse, LookupKeyByContextOrCreateRequest,
    LookupKeyByContextOrCreateResponse, LookupKeyByContextRequest, LookupKeyByContextResponse,
    Value,
};
pub use udex_api::hash::xxh3_context_hash;
pub use udex_api::index::{CreateIndexRequest, HashAlgorithm, Index, IndexUpdate};
