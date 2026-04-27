// WP-03 (ST0007): Hydra client creation and client_credentials token exchange.
// This module is a stub; full implementation is in WP-03.

use std::error::Error;

/// Configuration for an OAuth2 client registered in Hydra.
pub struct OAuthClientConfig {
    pub name: String,
    pub id: String,
    pub secret: String,
    pub scopes: Vec<String>,
    pub audience: String,
}

/// Create an OAuth2 client in Hydra via the admin API.
///
/// `admin_url` — base URL of the Hydra admin endpoint (e.g. `http://hydra:4445`).
pub async fn create_oauth2_client(
    _admin_url: &str,
    _client: OAuthClientConfig,
) -> Result<(), Box<dyn Error>> {
    todo!("ST0007/WP-03: implement Hydra admin API call")
}

/// Exchange client credentials for a JWT access token.
///
/// `public_url` — base URL of the Hydra public endpoint (e.g. `http://hydra:4444`).
/// `scopes`     — subset of the client's registered scopes to request.
///
/// Returns the raw access token string.
pub async fn authenticate(
    _public_url: &str,
    _client: OAuthClientConfig,
    _scopes: Vec<String>,
) -> Result<String, Box<dyn Error>> {
    todo!("ST0007/WP-03: implement client_credentials token exchange")
}
