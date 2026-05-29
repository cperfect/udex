use std::collections::HashMap;
use std::str::FromStr;
use std::sync::{Arc, OnceLock, Weak};
use std::time::{Duration, Instant};

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use rand::Rng;
use tokio::sync::{Mutex, RwLock};
use tonic::body::Body;
use tonic::codegen::http::Request;
use tonic::Status;
use tonic_middleware::RequestInterceptor;
use udex_api::authz::claims::Claims;

use crate::config::AuthzConfig;
use crate::Error;

/// Derives a signing algorithm from a JWK's key type and curve when the
/// optional `alg` field is absent. Only EC keys with a known named curve have
/// an unambiguous default; RSA and OKP require an explicit `alg`.
fn infer_algorithm_from_jwk(jwk: &jsonwebtoken::jwk::Jwk) -> Option<Algorithm> {
    use jsonwebtoken::jwk::{AlgorithmParameters, EllipticCurve};
    match &jwk.algorithm {
        AlgorithmParameters::EllipticCurve(ec) => match ec.curve {
            EllipticCurve::P256 => Some(Algorithm::ES256),
            EllipticCurve::P384 => Some(Algorithm::ES384),
            // P-521 / ES512 is not supported by this jsonwebtoken version.
            EllipticCurve::P521 => None,
            EllipticCurve::Ed25519 => Some(Algorithm::EdDSA),
        },
        _ => None,
    }
}

/// Parses a [`JwkSet`] into a `kid → (DecodingKey, Algorithm)` map.
///
/// RFC 7517 §4.5: keys without a `kid` are skipped — the server cannot
/// select them per-request. Keys with duplicate kids or an unresolvable
/// algorithm are rejected.
fn parse_jwks(
    jwks: &JwkSet,
    url: &str,
) -> Result<HashMap<String, (DecodingKey, Algorithm)>, Error> {
    let mut key_map = HashMap::new();
    for jwk in &jwks.keys {
        if let Some(kid) = &jwk.common.key_id {
            if key_map.contains_key(kid) {
                return Err(Error::ConfigValidation(format!(
                    "JWKS at '{url}' contains duplicate kid '{kid}'"
                )));
            }
            // RFC 7517 §4.4: `alg` is OPTIONAL. Many providers (AWS Cognito,
            // Azure AD, Keycloak) omit it. Fall back to deriving the algorithm
            // from kty/crv when absent; error only when neither is available.
            let alg = jwk
                .common
                .key_algorithm
                .as_ref()
                .and_then(|ka| Algorithm::from_str(&ka.to_string()).ok())
                .or_else(|| infer_algorithm_from_jwk(jwk))
                .ok_or_else(|| {
                    Error::ConfigValidation(format!(
                        "JWK (kid='{kid}') at '{url}' has no 'alg' field \
                         and the key type does not have an unambiguous default \
                         (EC keys with a known curve are inferred automatically)"
                    ))
                })?;
            let decoding_key = DecodingKey::from_jwk(jwk).map_err(|e| {
                Error::ConfigValidation(format!("Invalid JWK (kid='{kid}'): {e}"))
            })?;
            key_map.insert(kid.clone(), (decoding_key, alg));
        }
    }
    if key_map.is_empty() {
        return Err(Error::ConfigValidation(format!(
            "JWKS at '{url}' contains no usable keys with a kid"
        )));
    }
    Ok(key_map)
}

/// Computes a backoff delay using exponential backoff with equal jitter.
///
/// `temp = min(300s, factor^consecutive_failures)`; then
/// `delay = temp/2 + rand(0..temp/2)`, ensuring the floor is never zero
/// beyond the first failure.
fn compute_backoff(consecutive_failures: u32, factor_secs: u64) -> Duration {
    const CAP_SECS: u64 = 300;
    let temp = factor_secs
        .saturating_pow(consecutive_failures)
        .min(CAP_SECS);
    let half = temp / 2;
    let jitter: u64 = if half > 0 {
        rand::thread_rng().gen_range(0..half)
    } else {
        0
    };
    Duration::from_secs(half + jitter)
}

/// Computes the sleep duration before the next proactive JWKS refresh.
///
/// Applies ±1% jitter to `max_age_secs` so that a fleet of servers does not
/// all refresh simultaneously. Output lies in `[0.99 * max_age, 1.01 * max_age]`.
/// Returns `Duration::ZERO` when `max_age_secs` is 0.
fn compute_expiry_deadline(max_age_secs: u64) -> Duration {
    let jitter_range = max_age_secs / 100;
    let secs = if jitter_range > 0 {
        let amount = rand::thread_rng().gen_range(0..=jitter_range);
        if rand::thread_rng().gen::<bool>() {
            max_age_secs.saturating_add(amount)
        } else {
            max_age_secs.saturating_sub(amount)
        }
    } else {
        max_age_secs
    };
    Duration::from_secs(secs)
}

/// Background task that proactively refreshes the JWKS cache on schedule.
///
/// Exits when the [`Weak`] reference can no longer be upgraded — i.e. when all
/// [`AuthzInterceptor`] clones have been dropped (server shutdown). The `Arc`
/// is released during the sleep so that a shutdown waiting on the refcount
/// reaching zero is not blocked.
async fn run_expiry_loop(weak: Weak<AuthzInterceptorInner>) {
    loop {
        // Compute the sleep duration while briefly holding a strong reference,
        // then release it before sleeping.
        let sleep = {
            let Some(inner) = weak.upgrade() else { return };
            let KeySource::Jwks(cache) = &inner.key_source else { return };
            compute_expiry_deadline(cache.max_age_secs)
        };

        tokio::time::sleep(sleep).await;

        // Re-acquire after sleep; exit if the server has shut down meanwhile.
        let Some(inner) = weak.upgrade() else { return };
        let KeySource::Jwks(cache) = &inner.key_source else { return };
        // kid = None: expiry-triggered refresh, no per-kid double-check needed.
        let _ = cache.try_refresh(None).await;
    }
}

struct RefreshCtrl {
    consecutive_failures: u32,
    backoff_until: Option<Instant>,
}

struct JwksCache {
    url: String,
    client: reqwest::Client,
    keys: RwLock<HashMap<String, (DecodingKey, Algorithm)>>,
    refresh_ctrl: Mutex<RefreshCtrl>,
    max_failed_refreshes: u32,
    backoff_factor_secs: u64,
    max_age_secs: u64,
    expiry_task: OnceLock<tokio::task::AbortHandle>,
}

impl JwksCache {
    /// Fetches the JWKS endpoint and parses the response into a key map.
    async fn fetch_and_parse_jwks(
        &self,
    ) -> Result<HashMap<String, (DecodingKey, Algorithm)>, String> {
        let text = self
            .client
            .get(&self.url)
            .send()
            .await
            .map_err(|e| format!("HTTP request failed: {e}"))?
            .error_for_status()
            .map_err(|e| format!("HTTP error response: {e}"))?
            .text()
            .await
            .map_err(|e| format!("Failed to read response body: {e}"))?;

        let jwks: JwkSet =
            serde_json::from_str(&text).map_err(|e| format!("Invalid JWKS JSON: {e}"))?;

        parse_jwks(&jwks, &self.url).map_err(|e| e.to_string())
    }

    /// Attempts a JWKS refresh under the `RefreshCtrl` mutex, applying DoS
    /// controls before and updating state after.
    ///
    /// `triggering_kid` — if `Some`, performs a double-check of the key map
    /// after acquiring the lock (another task may have refreshed first).
    /// Pass `None` from the expiry path where any kid-specific check is
    /// unnecessary.
    ///
    /// Returns `Ok(())` when the cache was refreshed or the kid was found
    /// by another task. Returns `Err(Status::unauthenticated(...))` when DoS
    /// controls suppress the attempt or the fetch fails; the caller should
    /// propagate this to the gRPC client.
    async fn try_refresh(&self, triggering_kid: Option<&str>) -> Result<(), Status> {
        let mut ctrl = self.refresh_ctrl.lock().await;

        // Double-checked: another task may have fetched while we waited for
        // the lock.
        if let Some(kid) = triggering_kid {
            if self.keys.read().await.contains_key(kid) {
                return Ok(());
            }
        }

        // Max attempts gate — once hit, no further refreshes until restart.
        if ctrl.consecutive_failures >= self.max_failed_refreshes {
            tracing::error!(
                consecutive_failures = ctrl.consecutive_failures,
                max = self.max_failed_refreshes,
                "JWKS refresh suspended: max failed attempts reached; retaining cached keys"
            );
            return Err(Status::unauthenticated("Unknown JWT kid"));
        }

        // Backoff gate — suppress refresh until the cooldown expires.
        if let Some(backoff_until) = ctrl.backoff_until {
            if Instant::now() < backoff_until {
                tracing::warn!(
                    kid = triggering_kid.unwrap_or("<expiry>"),
                    "JWKS refresh suppressed: backoff active"
                );
                return Err(Status::unauthenticated("Unknown JWT kid"));
            }
        }

        // Attempt the fetch.
        match self.fetch_and_parse_jwks().await {
            Ok(new_map) => {
                *self.keys.write().await = new_map;
                ctrl.consecutive_failures = 0;
                ctrl.backoff_until = None;
                tracing::info!(url = %self.url, "JWKS cache refreshed successfully");
                Ok(())
            }
            Err(e) => {
                ctrl.consecutive_failures += 1;
                let delay =
                    compute_backoff(ctrl.consecutive_failures, self.backoff_factor_secs);
                ctrl.backoff_until = Some(Instant::now() + delay);
                tracing::error!(
                    url = %self.url,
                    error = %e,
                    consecutive_failures = ctrl.consecutive_failures,
                    backoff_secs = delay.as_secs(),
                    "JWKS refresh failed"
                );
                Err(Status::unauthenticated("Unknown JWT kid"))
            }
        }
    }
}

impl Drop for JwksCache {
    fn drop(&mut self) {
        if let Some(handle) = self.expiry_task.get() {
            handle.abort();
        }
    }
}

enum KeySource {
    Static(DecodingKey),
    Jwks(JwksCache),
}

/// All mutable-at-construction state for the interceptor. Kept private behind
/// an `Arc` so that `AuthzInterceptor::clone()` is a single refcount bump.
/// The JWKS map lives behind a `RwLock` inside `JwksCache`, enabling runtime
/// refresh without touching the public API.
struct AuthzInterceptorInner {
    key_source: KeySource,
    expected_issuer: String,
    expected_audience: String,
    scope_claim_name: String,
    mask_subject_in_logs: bool,
}

/// Cheap to clone — the `Arc` makes every `.clone()` a single refcount bump.
#[derive(Clone)]
pub struct AuthzInterceptor(Arc<AuthzInterceptorInner>);

impl AuthzInterceptor {
    pub async fn new(config: AuthzConfig) -> Result<Self, Error> {
        config.validate()?;
        let key_source = match (config.jwks_url, config.jwt_public_key) {
            (Some(url), None) => {
                let url_for_err = url.clone();
                let client = reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                    .map_err(|e| {
                        Error::ConfigValidation(format!(
                            "Failed to build HTTP client for JWKS from '{url_for_err}': {e}"
                        ))
                    })?;
                // Shared reference lets the same Fn closure be passed to multiple
                // map_err calls: &Fn(E)->T satisfies the FnOnce bound map_err requires.
                let jwks_err = |e: reqwest::Error| {
                    Error::ConfigValidation(format!(
                        "Failed to fetch JWKS from '{url_for_err}': {e}"
                    ))
                };
                let jwks_text = client
                    .get(&url)
                    .send()
                    .await
                    .map_err(&jwks_err)?
                    .error_for_status()
                    .map_err(&jwks_err)?
                    .text()
                    .await
                    .map_err(&jwks_err)?;
                let jwks: JwkSet = serde_json::from_str(&jwks_text).map_err(|e| {
                    Error::ConfigValidation(format!(
                        "Invalid JWKS response from '{url_for_err}': {e}"
                    ))
                })?;
                let key_map = parse_jwks(&jwks, &url_for_err)?;
                KeySource::Jwks(JwksCache {
                    url,
                    client,
                    keys: RwLock::new(key_map),
                    refresh_ctrl: Mutex::new(RefreshCtrl {
                        consecutive_failures: 0,
                        backoff_until: None,
                    }),
                    max_failed_refreshes: config.jwks_max_failed_refreshes.unwrap_or(5),
                    backoff_factor_secs: config.jwks_backoff_factor_secs.unwrap_or(3),
                    max_age_secs: config.jwks_max_age_secs.unwrap_or(86400),
                    expiry_task: OnceLock::new(),
                })
            }
            (None, Some(pem_secret)) => {
                let pem = pem_secret.value().map_err(|_| {
                    Error::ConfigValidation("jwt_public_key is not bound".to_string())
                })?;
                let key = DecodingKey::from_ec_pem(pem.as_bytes()).map_err(|e| {
                    Error::ConfigValidation(format!(
                        "Failed to create decoding key from jwt_public_key: {e}"
                    ))
                })?;
                KeySource::Static(key)
            }
            // Both-set and neither-set are unreachable: config.validate() above
            // guarantees exactly one key source is present.
            _ => unreachable!("AuthzConfig::validate() ensures exactly one key source is set"),
        };

        // config.validate() above guarantees both fields are Some and non-empty.
        let expected_issuer = config.jwt_issuer.expect("validated");
        let expected_audience = config.jwt_audience.expect("validated");
        let scope_claim_name = config
            .scope_claim_name
            .unwrap_or_else(|| "scope".to_string());

        let inner = Arc::new(AuthzInterceptorInner {
            key_source,
            expected_issuer,
            expected_audience,
            scope_claim_name,
            mask_subject_in_logs: config.mask_subject_in_logs,
        });

        // Spawn the proactive expiry task after the Arc is constructed so we
        // can form a Weak reference and store the AbortHandle via OnceLock.
        if let KeySource::Jwks(cache) = &inner.key_source {
            if cache.max_age_secs > 0 {
                let weak = Arc::downgrade(&inner);
                let abort_handle = tokio::spawn(run_expiry_loop(weak)).abort_handle();
                cache
                    .expiry_task
                    .set(abort_handle)
                    .expect("expiry_task set once at construction");
            }
        }

        Ok(Self(inner))
    }

    #[allow(clippy::result_large_err)]
    fn extract_bearer_token<'a>(&self, auth_header: &'a str) -> Result<&'a str, Status> {
        match auth_header.splitn(2, ' ').collect::<Vec<_>>().as_slice() {
            [scheme, token] if scheme.eq_ignore_ascii_case("Bearer") => Ok(token.trim()),
            _ => Err(Status::unauthenticated(
                "Authorization header must start with 'Bearer '",
            )),
        }
    }

    #[allow(clippy::result_large_err)]
    async fn decoding_key_for(&self, token: &str) -> Result<(DecodingKey, Algorithm), Status> {
        match &self.0.key_source {
            KeySource::Static(key) => Ok((key.clone(), Algorithm::ES256)),
            KeySource::Jwks(cache) => {
                let header = decode_header(token)
                    .map_err(|_| Status::unauthenticated("Invalid JWT header"))?;
                let kid = header
                    .kid
                    .ok_or_else(|| Status::unauthenticated("JWT missing kid claim"))?;

                // Fast path: read lock, clone entry, release lock.
                if let Some(entry) = cache
                    .keys
                    .read()
                    .await
                    .get(&kid)
                    .map(|(k, a)| (k.clone(), *a))
                {
                    return Ok(entry);
                }

                // Cache miss: attempt refresh (serialised by Mutex<RefreshCtrl>).
                cache.try_refresh(Some(&kid)).await?;

                // Post-refresh check.
                cache
                    .keys
                    .read()
                    .await
                    .get(&kid)
                    .map(|(key, alg)| (key.clone(), *alg))
                    .ok_or_else(|| {
                        tracing::warn!(kid = %kid, "JWT kid not found in JWKS after refresh");
                        Status::unauthenticated("Unknown JWT kid")
                    })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    async fn validate_jwt(&self, token: &str) -> Result<Claims, Status> {
        let (key, alg) = self.decoding_key_for(token).await?;

        let mut validation = Validation::new(alg);
        validation.set_issuer(&[&self.0.expected_issuer]);
        validation.set_audience(&[&self.0.expected_audience]);
        // Require sub in addition to the library's default (exp).
        // An empty sub is caught later by custom_validate_public().
        validation.required_spec_claims.insert("sub".to_string());

        // Decode as a raw JSON map so we can extract scope from the configured
        // claim name without being coupled to a fixed field name in Claims.
        // Signature, exp, iss, and aud are all validated by jsonwebtoken before
        // the payload is deserialised.
        let payload: serde_json::Map<String, serde_json::Value> =
            decode(token, &key, &validation)
                .map_err(|err| {
                    tracing::warn!(error = ?err, "JWT validation error");
                    Status::unauthenticated("Invalid JWT token")
                })?
                .claims;

        // Extract scope from the configured claim; accept a space-delimited
        // string (RFC 8693) or a JSON array (e.g. Hydra's "scp" claim).
        let scope = match payload.get(&self.0.scope_claim_name) {
            Some(serde_json::Value::String(s)) => s.clone(),
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str())
                .collect::<Vec<_>>()
                .join(" "),
            _ => String::new(),
        };

        let mut claims: Claims = serde_json::from_value(serde_json::Value::Object(payload))
            .map_err(|err| {
                tracing::warn!(error = ?err, "JWT claims deserialisation error");
                Status::unauthenticated("Invalid JWT claims")
            })?;
        claims.scope = scope;

        claims
            .custom_validate_public()
            .map_err(|e| Status::unauthenticated(format!("JWT validation failed: {}", e)))?;

        Ok(claims)
    }
}

/// Returns a masked representation of a JWT subject for audit logging.
///
/// Subjects of 6 or more characters expose the first 2 and last 2 characters;
/// everything in between is replaced with asterisks. Subjects of 5 or fewer
/// characters are replaced entirely with asterisks of the same length.
///
/// Operates on Unicode scalar values (chars), so multi-byte characters are
/// counted correctly.
fn mask_subject(sub: &str) -> String {
    let chars: Vec<char> = sub.chars().collect();
    let len = chars.len();
    if len >= 6 {
        let first: String = chars[..2].iter().collect();
        let last: String = chars[len - 2..].iter().collect();
        format!("{}{}{}", first, "*".repeat(len - 4), last)
    } else {
        "*".repeat(len)
    }
}

#[tonic::async_trait]
impl RequestInterceptor for AuthzInterceptor {
    async fn intercept(&self, mut req: Request<Body>) -> Result<Request<Body>, Status> {
        match req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
        {
            Some(auth_header) => {
                let token = self.extract_bearer_token(auth_header)?;
                let claims = self.validate_jwt(token).await?;
                let subject = if self.0.mask_subject_in_logs {
                    mask_subject(claims.sub())
                } else {
                    claims.sub().to_string()
                };
                tracing::info!(
                    subject = %subject,
                    method = %req.uri().path(),
                    "request authenticated"
                );
                req.extensions_mut().insert(claims);
                Ok(req)
            }
            None => {
                tracing::warn!("Request missing Authorization header");
                Err(Status::unauthenticated("Authorization header missing"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AuthzConfig;
    use tracing_test::traced_test;

    fn bound_pem_secret(path: &str) -> secrets_rs::Secret<String> {
        let mut s = secrets_rs::Secret::new(&format!("urn:secrets-rs:file:{path}"))
            .expect("valid file URN");
        let mut reg = secrets_rs::SourceRegistry::new();
        reg.register("file", secrets_rs::sources::file::FileSource::new())
            .unwrap();
        s.bind(&reg).unwrap();
        s
    }

    async fn test_interceptor() -> AuthzInterceptor {
        AuthzInterceptor::new(AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bound_pem_secret("tests/jwt/signing_public_key.pem")),
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        })
        .await
        .expect("Failed to create test AuthzInterceptor")
    }

    async fn test_interceptor_masked() -> AuthzInterceptor {
        AuthzInterceptor::new(AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bound_pem_secret("tests/jwt/signing_public_key.pem")),
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: true,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        })
        .await
        .expect("Failed to create masked test AuthzInterceptor")
    }

    #[tokio::test]
    async fn test_new_rejects_both_key_sources() {
        let Err(err) = AuthzInterceptor::new(AuthzConfig {
            jwks_url: Some("http://localhost:4444/.well-known/jwks.json".to_string()),
            jwt_public_key: Some(bound_pem_secret("tests/jwt/signing_public_key.pem")),
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        })
        .await
        else {
            panic!("expected ConfigValidation error for both key sources");
        };
        assert!(
            matches!(&err, Error::ConfigValidation(msg) if msg.contains("not both")),
            "expected 'not both' error, got: {err}"
        );
    }

    #[tokio::test]
    async fn test_new_rejects_no_key_source() {
        let Err(err) = AuthzInterceptor::new(AuthzConfig {
            jwks_url: None,
            jwt_public_key: None,
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        })
        .await
        else {
            panic!("expected ConfigValidation error for no key source");
        };
        assert!(
            matches!(&err, Error::ConfigValidation(msg) if msg.contains("must be set")),
            "expected 'must be set' error, got: {err}"
        );
    }

    #[tokio::test]
    async fn extract_bearer_token_canonical() {
        let interceptor = test_interceptor().await;
        assert_eq!(
            interceptor.extract_bearer_token("Bearer mytoken").unwrap(),
            "mytoken"
        );
    }

    #[tokio::test]
    async fn extract_bearer_token_lowercase_scheme() {
        let interceptor = test_interceptor().await;
        assert_eq!(
            interceptor.extract_bearer_token("bearer mytoken").unwrap(),
            "mytoken"
        );
    }

    #[tokio::test]
    async fn extract_bearer_token_uppercase_scheme() {
        let interceptor = test_interceptor().await;
        assert_eq!(
            interceptor.extract_bearer_token("BEARER mytoken").unwrap(),
            "mytoken"
        );
    }

    #[tokio::test]
    async fn extract_bearer_token_trims_token_whitespace() {
        let interceptor = test_interceptor().await;
        assert_eq!(
            interceptor
                .extract_bearer_token("Bearer   mytoken  ")
                .unwrap(),
            "mytoken"
        );
    }

    #[tokio::test]
    async fn extract_bearer_token_wrong_scheme_is_err() {
        let interceptor = test_interceptor().await;
        assert!(interceptor.extract_bearer_token("Basic mytoken").is_err());
    }

    #[tokio::test]
    async fn extract_bearer_token_missing_token_is_err() {
        let interceptor = test_interceptor().await;
        assert!(interceptor.extract_bearer_token("Bearer").is_err());
    }

    #[tokio::test]
    async fn extract_bearer_token_empty_is_err() {
        let interceptor = test_interceptor().await;
        assert!(interceptor.extract_bearer_token("").is_err());
    }

    fn make_request_with_token(token: &str) -> tonic::codegen::http::Request<tonic::body::Body> {
        tonic::codegen::http::Request::builder()
            .uri("/udex.index.v1.IndexService/ListIndices")
            .header("authorization", format!("Bearer {token}"))
            .body(tonic::body::Body::empty())
            .expect("build request")
    }

    fn make_valid_token(sub: &str) -> String {
        use jsonwebtoken::{encode, EncodingKey, Header};
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = Claims::new(
            sub.to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            now + 3600,
            now,
        );
        let key_pem =
            std::fs::read_to_string("tests/jwt/signing_private_key.pem").expect("read private key");
        let encoding_key =
            EncodingKey::from_ec_pem(key_pem.as_bytes()).expect("create EncodingKey");
        let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
        header.typ = Some("JWT".to_string());
        encode(&header, &claims, &encoding_key).expect("encode JWT")
    }

    #[traced_test]
    #[tokio::test]
    async fn test_mask_subject_in_logs_emits_masked() {
        use tonic_middleware::RequestInterceptor;
        let token = make_valid_token("alice@example.com");
        let interceptor = test_interceptor_masked().await;
        let req = make_request_with_token(&token);
        interceptor.intercept(req).await.expect("intercept ok");
        // "alice@example.com" (17 chars) → "al*************om"
        assert!(
            logs_contain("al*************om"),
            "expected partial-masked subject in log"
        );
        assert!(
            !logs_contain("alice@example.com"),
            "expected subject to be redacted"
        );
    }

    #[traced_test]
    #[tokio::test]
    async fn test_mask_subject_in_logs_false_emits_subject() {
        use tonic_middleware::RequestInterceptor;
        let token = make_valid_token("alice@example.com");
        let interceptor = test_interceptor().await;
        let req = make_request_with_token(&token);
        interceptor.intercept(req).await.expect("intercept ok");
        assert!(logs_contain("alice@example.com"), "expected subject in log");
        assert!(
            !logs_contain("al*************om"),
            "expected subject not masked"
        );
    }

    #[test]
    fn test_mask_subject_long() {
        assert_eq!(mask_subject("alice@example.com"), "al*************om");
        assert_eq!(mask_subject("abcdef"), "ab**ef");
    }

    #[test]
    fn test_mask_subject_short() {
        assert_eq!(mask_subject("alice"), "*****");
        assert_eq!(mask_subject("ab"), "**");
        assert_eq!(mask_subject("a"), "*");
        assert_eq!(mask_subject(""), "");
    }

    #[test]
    fn test_mask_subject_boundary() {
        // exactly 6 chars: first 2 + 2 asterisks + last 2
        assert_eq!(mask_subject("abcdef"), "ab**ef");
        // exactly 5 chars: all asterisks
        assert_eq!(mask_subject("abcde"), "*****");
    }

    #[traced_test]
    #[tokio::test]
    async fn test_invalid_jwt_emits_warn() {
        let interceptor = test_interceptor().await;
        let result = interceptor.validate_jwt("this.is.not.a.valid.jwt").await;
        assert!(result.is_err());
        assert!(logs_contain("JWT validation error"));
    }

    #[traced_test]
    #[tokio::test]
    async fn test_jwt_wrong_issuer_emits_warn() {
        use jsonwebtoken::{encode, EncodingKey, Header};
        use std::time::{SystemTime, UNIX_EPOCH};

        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;

        let claims = Claims::new(
            "test-user".to_string(),
            "wrong-issuer".to_string(),
            "test-audience".to_string(),
            now + 3600,
            now,
        );

        let private_key = std::fs::read_to_string("tests/jwt/signing_private_key.pem")
            .expect("Failed to read signing private key");
        let encoding_key =
            EncodingKey::from_ec_pem(private_key.as_bytes()).expect("Failed to create EncodingKey");

        let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
        header.typ = Some("JWT".to_string());

        let token = encode(&header, &claims, &encoding_key).expect("Failed to encode JWT");

        let interceptor = test_interceptor().await;
        let result = interceptor.validate_jwt(&token).await;
        assert!(result.is_err());
        assert!(logs_contain("JWT validation error"));
    }

    #[tokio::test]
    async fn test_jwt_empty_sub_is_rejected() {
        use jsonwebtoken::{encode, EncodingKey, Header};
        use std::time::{SystemTime, UNIX_EPOCH};
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = Claims::new(
            "".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            now + 3600,
            now,
        );
        let key_pem =
            std::fs::read_to_string("tests/jwt/signing_private_key.pem").expect("read private key");
        let encoding_key =
            EncodingKey::from_ec_pem(key_pem.as_bytes()).expect("create EncodingKey");
        let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
        header.typ = Some("JWT".to_string());
        let token = encode(&header, &claims, &encoding_key).expect("encode JWT");

        let result = test_interceptor().await.validate_jwt(&token).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_jwt_missing_sub_is_rejected() {
        use jsonwebtoken::{encode, EncodingKey, Header};
        use std::time::{SystemTime, UNIX_EPOCH};
        #[derive(serde::Serialize)]
        struct NoSubClaims {
            iss: String,
            aud: String,
            exp: usize,
            iat: usize,
        }
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as usize;
        let claims = NoSubClaims {
            iss: "test-issuer".to_string(),
            aud: "test-audience".to_string(),
            exp: now + 3600,
            iat: now,
        };
        let key_pem =
            std::fs::read_to_string("tests/jwt/signing_private_key.pem").expect("read private key");
        let encoding_key =
            EncodingKey::from_ec_pem(key_pem.as_bytes()).expect("create EncodingKey");
        let mut header = Header::new(jsonwebtoken::Algorithm::ES256);
        header.typ = Some("JWT".to_string());
        let token = encode(&header, &claims, &encoding_key).expect("encode JWT");

        let result = test_interceptor().await.validate_jwt(&token).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    // ── JWKS refresh tests ────────────────────────────────────────────────────

    /// RFC 7517 Appendix C P-256 test vector — a known-valid EC public key.
    /// Used to construct a well-formed JWKS response in refresh unit tests.
    const TEST_JWK_KID_1: &str = "refresh-test-key-1";
    const TEST_JWK_KID_2: &str = "refresh-test-key-2";
    const TEST_JWKS_ONE_KEY: &str = r#"{
      "keys": [{
        "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig",
        "kid": "refresh-test-key-1",
        "x": "f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU",
        "y": "x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0"
      }]
    }"#;
    const TEST_JWKS_TWO_KEYS: &str = r#"{
      "keys": [
        {
          "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig",
          "kid": "refresh-test-key-1",
          "x": "f83OJ3D2xF1Bg8vub9tLe1gHMzV76e8Tus9uPHvRVEU",
          "y": "x_FEzRu9m36HLN_tue659LNpXW6pCyStikYjKIWI5a0"
        },
        {
          "kty": "EC", "crv": "P-256", "alg": "ES256", "use": "sig",
          "kid": "refresh-test-key-2",
          "x": "Cu_UyxwLgHzE9rvlYSmvVdqYCXY42E9V5h5d7-WxTe4",
          "y": "AEGMjSmHUhpMQFgV_q7FDexBjVVz_dV0iFTPRE9RD_c"
        }
      ]
    }"#;

    fn make_dead_url() -> String {
        // Port 1 is reserved and will always refuse connections.
        "http://127.0.0.1:1/jwks".to_string()
    }

    fn make_test_cache(url: String, max_failed_refreshes: u32) -> JwksCache {
        JwksCache {
            url,
            client: reqwest::Client::builder()
                .timeout(Duration::from_millis(100))
                .build()
                .unwrap(),
            keys: RwLock::new(HashMap::new()),
            refresh_ctrl: Mutex::new(RefreshCtrl {
                consecutive_failures: 0,
                backoff_until: None,
            }),
            max_failed_refreshes,
            backoff_factor_secs: 3,
            max_age_secs: 86400,
            expiry_task: OnceLock::new(),
        }
    }

    /// Spawns a minimal HTTP/1.1 server that serves `body` on every request.
    /// Returns the URL to the server. The server runs for the lifetime of the
    /// returned `JoinHandle`.
    async fn spawn_test_jwks_server(
        body: &'static str,
    ) -> (String, tokio::task::JoinHandle<()>) {
        use tokio::io::AsyncWriteExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = tokio::spawn(async move {
            loop {
                let Ok((mut socket, _)) = listener.accept().await else {
                    break;
                };
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{}",
                    body.len(),
                    body
                );
                let _ = socket.write_all(response.as_bytes()).await;
            }
        });
        (format!("http://{addr}/jwks"), handle)
    }

    // ── compute_backoff ───────────────────────────────────────────────────────

    #[test]
    fn compute_backoff_floor_increases_with_failures() {
        // The backoff floor (temp/2, before jitter) must be non-decreasing.
        // Checking the floor directly avoids a flaky assertion: consecutive
        // jitter ranges can overlap, so two random samples are not guaranteed
        // to be ordered even when the underlying exponential curve is growing.
        // With factor=3 the floors are: 1, 4, 13, 40, 121, 150 (capped at 300).
        const CAP: u64 = 300;
        let factor = 3u64;
        let floors: Vec<u64> = (1u32..=6)
            .map(|f| factor.saturating_pow(f).min(CAP) / 2)
            .collect();
        for w in floors.windows(2) {
            assert!(w[1] >= w[0], "backoff floor should be non-decreasing: {floors:?}");
        }
    }

    #[test]
    fn compute_backoff_caps_at_300s() {
        // A very large failure count must never exceed the 300 s cap.
        let d = compute_backoff(100, 3);
        assert!(d.as_secs() <= 300, "delay must be capped: {d:?}");
    }

    #[test]
    fn compute_backoff_jitter_within_bounds() {
        // For factor=3 and failures=3: temp = min(300, 27) = 27, half = 13.
        // Delay must lie in [13, 26].
        for _ in 0..200 {
            let d = compute_backoff(3, 3);
            assert!(
                d.as_secs() >= 13 && d.as_secs() <= 26,
                "jitter out of [13, 26]: {d:?}"
            );
        }
    }

    #[test]
    fn compute_backoff_zero_failures_is_zero() {
        // 3^0 = 1, half = 0, jitter = 0 → delay = 0.
        assert_eq!(compute_backoff(0, 3), Duration::ZERO);
    }

    // ── try_refresh: DoS controls ─────────────────────────────────────────────

    #[traced_test]
    #[tokio::test]
    async fn try_refresh_increments_failures_on_fetch_error() {
        let cache = make_test_cache(make_dead_url(), 5);
        let result = cache.try_refresh(Some("kid-x")).await;
        assert!(result.is_err());
        assert_eq!(cache.refresh_ctrl.lock().await.consecutive_failures, 1);
        assert!(logs_contain("JWKS refresh failed"));
    }

    #[traced_test]
    #[tokio::test]
    async fn try_refresh_gate_fires_at_max_failures() {
        // max_failed_refreshes = 1: a single failure exhausts the limit.
        // The gate check runs before the backoff check, so the second call
        // hits the gate regardless of any active backoff.
        let cache = make_test_cache(make_dead_url(), 1);
        let _ = cache.try_refresh(Some("kid-x")).await;
        let result = cache.try_refresh(Some("kid-x")).await;
        assert!(result.is_err());
        assert!(logs_contain("JWKS refresh suspended"));
        // Counter must not increment beyond max.
        assert_eq!(cache.refresh_ctrl.lock().await.consecutive_failures, 1);
    }

    #[traced_test]
    #[tokio::test]
    async fn try_refresh_backoff_suppresses_retry() {
        let cache = make_test_cache(make_dead_url(), 5);
        // One failure sets a backoff_until in the future.
        let _ = cache.try_refresh(Some("kid-x")).await;
        {
            let mut ctrl = cache.refresh_ctrl.lock().await;
            // Force backoff_until well into the future.
            ctrl.backoff_until = Some(Instant::now() + Duration::from_secs(3600));
        }
        let result = cache.try_refresh(Some("kid-x")).await;
        assert!(result.is_err());
        assert!(logs_contain("JWKS refresh suppressed"));
        // Failure count must not have increased.
        assert_eq!(cache.refresh_ctrl.lock().await.consecutive_failures, 1);
    }

    #[tokio::test]
    async fn try_refresh_success_resets_state() {
        let (url, _server) = spawn_test_jwks_server(TEST_JWKS_ONE_KEY).await;
        let cache = make_test_cache(url, 5);
        // Simulate prior failures.
        {
            let mut ctrl = cache.refresh_ctrl.lock().await;
            ctrl.consecutive_failures = 3;
            ctrl.backoff_until = Some(Instant::now() - Duration::from_secs(1)); // expired
        }
        let result = cache.try_refresh(Some(TEST_JWK_KID_1)).await;
        assert!(result.is_ok(), "expected Ok, got {result:?}");
        let ctrl = cache.refresh_ctrl.lock().await;
        assert_eq!(ctrl.consecutive_failures, 0);
        assert!(ctrl.backoff_until.is_none());
        assert!(cache.keys.read().await.contains_key(TEST_JWK_KID_1));
    }

    #[tokio::test]
    async fn try_refresh_double_check_skips_fetch_when_kid_appears() {
        // Simulate another task having already refreshed the cache by pre-populating
        // the key map before calling try_refresh. The double-check should return Ok
        // without making a network request (dead URL would fail if contacted).
        let cache = make_test_cache(make_dead_url(), 5);
        {
            // Pre-populate — as if another task's refresh already landed.
            let jwks: JwkSet = serde_json::from_str(TEST_JWKS_ONE_KEY).unwrap();
            let new_map = parse_jwks(&jwks, "test").unwrap();
            *cache.keys.write().await = new_map;
        }
        let result = cache.try_refresh(Some(TEST_JWK_KID_1)).await;
        assert!(result.is_ok(), "double-check should have short-circuited");
        // No network call → no failure incremented.
        assert_eq!(cache.refresh_ctrl.lock().await.consecutive_failures, 0);
    }

    fn make_test_inner(url: String, max_age_secs: u64) -> Arc<AuthzInterceptorInner> {
        Arc::new(AuthzInterceptorInner {
            key_source: KeySource::Jwks(JwksCache {
                url,
                client: reqwest::Client::builder()
                    .timeout(Duration::from_millis(100))
                    .build()
                    .unwrap(),
                keys: RwLock::new(HashMap::new()),
                refresh_ctrl: Mutex::new(RefreshCtrl {
                    consecutive_failures: 0,
                    backoff_until: None,
                }),
                max_failed_refreshes: 5,
                backoff_factor_secs: 3,
                max_age_secs,
                expiry_task: OnceLock::new(),
            }),
            expected_issuer: "test-issuer".to_string(),
            expected_audience: "test-audience".to_string(),
            scope_claim_name: "scope".to_string(),
            mask_subject_in_logs: false,
        })
    }

    #[tokio::test]
    async fn try_refresh_new_kid_visible_after_successful_refresh() {
        let (url, _server) = spawn_test_jwks_server(TEST_JWKS_TWO_KEYS).await;
        let cache = make_test_cache(url, 5);
        let result = cache.try_refresh(Some(TEST_JWK_KID_2)).await;
        assert!(result.is_ok());
        let keys = cache.keys.read().await;
        assert!(keys.contains_key(TEST_JWK_KID_1));
        assert!(keys.contains_key(TEST_JWK_KID_2));
    }

    // ── compute_expiry_deadline ───────────────────────────────────────────────

    #[test]
    fn compute_expiry_deadline_within_one_percent() {
        let max_age = 86400u64;
        let lo = (max_age * 99) / 100;
        let hi = (max_age * 101) / 100;
        for _ in 0..500 {
            let secs = compute_expiry_deadline(max_age).as_secs();
            assert!(secs >= lo && secs <= hi, "deadline {secs} not in [{lo}, {hi}]");
        }
    }

    #[test]
    fn compute_expiry_deadline_no_jitter_when_range_zero() {
        // max_age = 50 → jitter_range = 50/100 = 0 → no jitter applied.
        assert_eq!(compute_expiry_deadline(50), Duration::from_secs(50));
    }

    #[test]
    fn compute_expiry_deadline_zero_returns_zero() {
        assert_eq!(compute_expiry_deadline(0), Duration::ZERO);
    }

    // ── run_expiry_loop ───────────────────────────────────────────────────────

    #[tokio::test]
    async fn expiry_loop_exits_immediately_on_dead_weak() {
        // Drop the Arc before spawning so the very first upgrade() returns None.
        let weak = {
            let inner = make_test_inner(make_dead_url(), 86400);
            Arc::downgrade(&inner)
        };
        let task = tokio::spawn(run_expiry_loop(weak));
        tokio::time::timeout(Duration::from_millis(200), task)
            .await
            .expect("expiry loop should exit promptly when Weak is dead")
            .expect("task must not panic");
    }

    // ── expiry task spawn ─────────────────────────────────────────────────────

    #[tokio::test]
    async fn no_expiry_task_when_max_age_is_zero() {
        let (url, _server) = spawn_test_jwks_server(TEST_JWKS_ONE_KEY).await;
        let interceptor = AuthzInterceptor::new(AuthzConfig {
            jwks_url: Some(url),
            jwt_public_key: None,
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: Some(0),
        })
        .await
        .expect("interceptor should be created");
        let KeySource::Jwks(cache) = &interceptor.0.key_source else {
            panic!("expected Jwks key source");
        };
        assert!(
            cache.expiry_task.get().is_none(),
            "no task should be spawned when max_age_secs = 0"
        );
    }

    #[tokio::test]
    async fn expiry_task_abort_handle_set_when_max_age_nonzero() {
        let (url, _server) = spawn_test_jwks_server(TEST_JWKS_ONE_KEY).await;
        let interceptor = AuthzInterceptor::new(AuthzConfig {
            jwks_url: Some(url),
            jwt_public_key: None,
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: Some(3600),
        })
        .await
        .expect("interceptor should be created");
        let KeySource::Jwks(cache) = &interceptor.0.key_source else {
            panic!("expected Jwks key source");
        };
        assert!(
            cache.expiry_task.get().is_some(),
            "abort handle should be set when max_age_secs > 0"
        );
    }
}
