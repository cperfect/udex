use std::fmt;
use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::Mutex;

use crate::error::Error;

struct CachedToken {
    value: String,
    expires_at: Instant,
}

impl fmt::Debug for CachedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CachedToken")
            .field("value", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// Fetches and caches OAuth2 client-credentials tokens, refreshing transparently.
#[derive(Clone)]
pub struct TokenManager {
    inner: Arc<Inner>,
}

impl fmt::Debug for TokenManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TokenManager")
            .field("inner", &self.inner)
            .finish()
    }
}

struct Inner {
    http: reqwest::Client,
    token_url: String,
    client_id: String,
    client_secret: String,
    audience: Option<String>,
    scope: Option<String>,
    cached: Mutex<Option<CachedToken>>,
}

impl fmt::Debug for Inner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Inner")
            .field("token_url", &self.token_url)
            .field("client_id", &"[REDACTED]")
            .field("client_secret", &"[REDACTED]")
            .field("audience", &self.audience)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

/// Seconds before expiry at which the token is proactively refreshed.
const REFRESH_MARGIN_SECS: u64 = 30;

/// Total timeout for a single token-endpoint request (connect + response body).
const TOKEN_REQUEST_TIMEOUT_SECS: u64 = 10;

impl TokenManager {
    pub(crate) fn new(
        token_url: String,
        client_id: String,
        client_secret: String,
        audience: Option<String>,
        scope: Option<String>,
        danger_allow_non_tls: bool,
    ) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(TOKEN_REQUEST_TIMEOUT_SECS))
            .https_only(!danger_allow_non_tls)
            .build()
            .expect("failed to build HTTP client");
        TokenManager {
            inner: Arc::new(Inner {
                http,
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
        // Apply the refresh margin only when the token lives long enough to
        // accommodate it; otherwise cache for the full expires_in so a short-
        // lived token doesn't produce a zero-duration cache and a fetch loop.
        // The .max(1) floor prevents a tight loop if the server returns 0.
        let cache_secs = if response.expires_in > REFRESH_MARGIN_SECS {
            response.expires_in - REFRESH_MARGIN_SECS
        } else {
            response.expires_in.max(1)
        };
        let expires_at = Instant::now() + Duration::from_secs(cache_secs);
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
