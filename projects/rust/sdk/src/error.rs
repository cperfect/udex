use std::fmt;

/// Standard gRPC status codes.
///
/// See <https://grpc.github.io/grpc/core/md_doc_statuscodes.html>.
pub mod grpc_code {
    pub const INVALID_ARGUMENT: u32 = 3;
    pub const DEADLINE_EXCEEDED: u32 = 4;
    pub const NOT_FOUND: u32 = 5;
    pub const ALREADY_EXISTS: u32 = 6;
    pub const PERMISSION_DENIED: u32 = 7;
    pub const FAILED_PRECONDITION: u32 = 9;
    pub const OUT_OF_RANGE: u32 = 11;
    pub const INTERNAL: u32 = 13;
    pub const UNAVAILABLE: u32 = 14;
    pub const UNAUTHENTICATED: u32 = 16;
}

/// The gRPC status returned by the server for a failed RPC.
#[derive(Debug, Clone)]
pub struct RpcStatus {
    code: u32,
    message: String,
}

impl RpcStatus {
    /// The raw gRPC status code (see [`grpc_code`] for named constants).
    pub fn code(&self) -> u32 {
        self.code
    }

    /// The error message from the server.
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for RpcStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "status {} {:?}", self.code, self.message)
    }
}

/// All errors that can be returned by the SDK.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The `ClientOptions` builder was given an invalid or incomplete configuration.
    #[error("invalid client options: {0}")]
    InvalidOptions(String),

    /// A transport-level or TLS error occurred when connecting to the server.
    #[error("transport error: {0}")]
    Transport(String),

    /// An OAuth2 token could not be acquired or refreshed.
    #[error("authentication error: {0}")]
    Auth(String),

    /// An RPC returned a non-OK status.
    #[error("RPC error: {0}")]
    Rpc(RpcStatus),

    /// An I/O error (e.g. reading a certificate file).
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
}

impl From<tonic::transport::Error> for Error {
    fn from(e: tonic::transport::Error) -> Self {
        Error::Transport(e.to_string())
    }
}

impl From<tonic::Status> for Error {
    fn from(s: tonic::Status) -> Self {
        Error::Rpc(RpcStatus {
            code: i32::from(s.code()) as u32,
            message: s.message().to_owned(),
        })
    }
}
