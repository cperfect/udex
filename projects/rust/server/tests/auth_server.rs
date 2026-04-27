// Test-only helper module (ST0007/WP-03).
// Provides Hydra client registration and client_credentials token exchange.
// Not compiled into the server binary.

use oauth2::{basic::BasicClient, AuthUrl, ClientId, ClientSecret, Scope, TokenResponse, TokenUrl};
use ory_hydra_client::apis::{configuration::Configuration, o_auth2_api};
use ory_hydra_client::models::o_auth2_client::OAuth2Client as HydraClient;

/// Configuration for an OAuth2 client registered in Hydra.
pub struct OAuthClientConfig {
    pub name: String,
    pub id: String,
    pub secret: String,
    /// Full set of scopes the client is permitted to request.
    pub scopes: Vec<String>,
    pub audience: String,
}

fn hydra_admin_config(admin_url: &str) -> Configuration {
    Configuration {
        base_path: admin_url.to_string(),
        ..Configuration::default()
    }
}

/// Register an OAuth2 client in Hydra via the admin API.
///
/// `admin_url` — base URL of the Hydra admin endpoint (e.g. `http://hydra:4445`).
pub async fn create_oauth2_client(
    admin_url: &str,
    client: OAuthClientConfig,
) -> anyhow::Result<()> {
    let config = hydra_admin_config(admin_url);

    let mut body = HydraClient::new();
    body.access_token_strategy = Some("jwt".to_string());
    body.audience = Some(vec![client.audience]);
    body.client_id = Some(client.id);
    body.client_name = Some(client.name);
    body.client_secret = Some(client.secret);
    body.grant_types = Some(vec!["client_credentials".to_string()]);
    body.scope = Some(client.scopes.join(" "));
    body.token_endpoint_auth_method = Some("client_secret_post".to_string());

    o_auth2_api::create_o_auth2_client(&config, body)
        .await
        .map_err(|e| anyhow::anyhow!("Hydra create_client failed: {e}"))?;

    Ok(())
}

/// Exchange client credentials for a JWT access token.
///
/// `public_url` — base URL of the Hydra public endpoint (e.g. `http://hydra:4444`).
/// `scopes`     — subset of the client's registered scopes to request.
///
/// Returns the raw access token string.
pub async fn authenticate(
    public_url: &str,
    client: OAuthClientConfig,
    scopes: Vec<String>,
) -> anyhow::Result<String> {
    let token_url = format!("{public_url}/oauth2/token");
    let auth_url = format!("{public_url}/oauth2/auth");

    let auth_client = BasicClient::new(ClientId::new(client.id))
        .set_client_secret(ClientSecret::new(client.secret))
        .set_auth_uri(AuthUrl::new(auth_url)?)
        .set_token_uri(TokenUrl::new(token_url)?);

    let http_client = oauth2::reqwest::ClientBuilder::new()
        .redirect(oauth2::reqwest::redirect::Policy::none())
        .build()?;

    let mut token_request = auth_client.exchange_client_credentials();
    for scope in scopes {
        token_request = token_request.add_scope(Scope::new(scope));
    }

    let token_response = token_request
        .request_async(&http_client)
        .await
        .map_err(|e| anyhow::anyhow!("Token exchange failed: {e}"))?;

    Ok(token_response.access_token().secret().to_string())
}
