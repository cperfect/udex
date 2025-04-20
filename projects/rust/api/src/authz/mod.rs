pub mod entry;
pub mod index;
pub mod claims;
pub mod permissions;
pub mod glob;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Invalid claims: {0}")]
    InvalidClaimsError(String),
    #[error("Invalid permission: {0}")]
    InvalidPermissionError(String),
}
