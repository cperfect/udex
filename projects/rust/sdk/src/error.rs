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
    Rpc(Box<tonic::Status>),

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
        Error::Rpc(Box::new(s))
    }
}
