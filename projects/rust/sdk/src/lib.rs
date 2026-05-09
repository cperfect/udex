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

pub mod auth;
pub mod client;
pub mod entry;
pub mod error;
pub mod index;

pub use client::{ClientOptions, ClientOptionsBuilder, UdexClient};
pub use error::Error;

// Re-export the proto types callers need to build requests and read responses.
pub use udex_api::entry::{
    bulk_read_entry_operation, bulk_write_entry_operation, value, BulkReadEntryOperation,
    BulkReadEntryOperationResult, BulkWriteEntryOperation, BulkWriteEntryOperationResult, Context,
    ContextInput, CreateEntryRequest, CreateEntryResponse, KeyValuePair, Value,
};
pub use udex_api::hash::xxh3_context_hash;
pub use udex_api::index::{CreateIndexRequest, Index, IndexUpdate};
