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
