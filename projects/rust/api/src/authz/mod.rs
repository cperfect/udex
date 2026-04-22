pub mod claims;
pub mod entry;
pub mod glob;
pub mod index;
pub mod permissions;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid claims: {0}")]
    InvalidClaimsError(String),
    #[error("Invalid permission: {0}")]
    InvalidPermissionError(String),
}

/// Both error variants represent malformed client tokens — map them to
/// `UNAUTHENTICATED` so callers never accidentally surface an `INTERNAL` status
/// for what is fundamentally a bad-request from the client.
impl From<Error> for tonic::Status {
    fn from(e: Error) -> Self {
        match e {
            Error::InvalidClaimsError(_) | Error::InvalidPermissionError(_) => {
                tonic::Status::unauthenticated(e.to_string())
            }
        }
    }
}
