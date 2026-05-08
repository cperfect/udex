use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::Error;

#[derive(Debug)]
struct CachedToken {
    value: String,
    expires_at: Instant,
}

/// Fetches and caches OAuth2 client-credentials tokens, refreshing transparently.
#[derive(Debug, Clone)]
pub struct TokenManager {
    inner: Arc<Inner>,
}

#[derive(Debug)]
struct Inner {
    http: reqwest::Client,
    token_url: String,
    client_id: String,
    client_secret: String,
    audience: Option<String>,
    scope: Option<String>,
    cached: Mutex<Option<CachedToken>>,
}

/// Seconds before expiry at which the token is proactively refreshed.
const REFRESH_MARGIN_SECS: u64 = 30;

impl TokenManager {
    pub(crate) fn new(
        token_url: String,
        client_id: String,
        client_secret: String,
        audience: Option<String>,
        scope: Option<String>,
    ) -> Self {
        TokenManager {
            inner: Arc::new(Inner {
                http: reqwest::Client::new(),
                token_url,
                client_id,
                client_secret,
                audience,
                scope,
                cached: Mutex::new(None),
            }),
        }
    }

    /// Returns a valid bearer token, refreshing if necessary.
    pub async fn token(&self) -> Result<String, Error> {
        let mut guard = self.inner.cached.lock().await;
        if let Some(cached) = &*guard {
            if cached.expires_at > Instant::now() {
                return Ok(cached.value.clone());
            }
        }
        // Fetch a new token.
        let response = self.fetch_token().await?;
        let expires_at = Instant::now()
            + Duration::from_secs(response.expires_in.saturating_sub(REFRESH_MARGIN_SECS));
        *guard = Some(CachedToken {
            value: response.access_token.clone(),
            expires_at,
        });
        Ok(response.access_token)
    }

    async fn fetch_token(&self) -> Result<TokenResponse, Error> {
        let mut params = vec![
            ("grant_type", "client_credentials"),
            ("client_id", &self.inner.client_id),
            ("client_secret", &self.inner.client_secret),
        ];
        let audience = self.inner.audience.as_deref().unwrap_or_default();
        let scope = self.inner.scope.as_deref().unwrap_or_default();
        if !audience.is_empty() {
            params.push(("audience", audience));
        }
        if !scope.is_empty() {
            params.push(("scope", scope));
        }

        let resp = self
            .inner
            .http
            .post(&self.inner.token_url)
            .form(&params)
            .send()
            .await
            .map_err(|e| Error::Auth(e.to_string()))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::Auth(format!(
                "token endpoint returned {status}: {body}"
            )));
        }

        resp.json::<TokenResponse>()
            .await
            .map_err(|e| Error::Auth(format!("failed to parse token response: {e}")))
    }
}

#[derive(Debug, serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    expires_in: u64,
}
