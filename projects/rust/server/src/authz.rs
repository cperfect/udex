use std::collections::HashMap;
use std::str::FromStr;
use std::time::Duration;

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
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

#[derive(Clone)]
enum KeySource {
    Static(DecodingKey),
    /// Each entry stores the decoding key and the algorithm for each `kid`.
    Jwks(HashMap<String, (DecodingKey, Algorithm)>),
}

#[derive(Clone)]
pub struct AuthzInterceptor {
    key_source: KeySource,
    expected_issuer: String,
    expected_audience: String,
    scope_claim_name: String,
    mask_subject_in_logs: bool,
}

impl AuthzInterceptor {
    pub async fn new(config: AuthzConfig) -> Result<Self, Error> {
        config.validate()?;
        // TODO If the identity provider rotates signing keys, a server restart would be required to pick up the new JWKS. This is acceptable for test/demo purposes (which is the case for now) but for production use, consider implementing periodic JWKS refresh (e.g., background task with configurable interval, or refresh on kid miss with rate limiting).
        let key_source = match (config.jwks_url, config.jwt_public_key) {
            (Some(url), None) => {
                let url_for_err = url.clone();
                let jwks_text = reqwest::Client::builder()
                    .timeout(Duration::from_secs(10))
                    .build()
                    .map_err(|e| {
                        Error::ConfigValidation(format!(
                            "Failed to build HTTP client for JWKS fetch from '{url_for_err}': {e}"
                        ))
                    })?
                    .get(&url)
                    .send()
                    .await
                    .map_err(|e| {
                        Error::ConfigValidation(format!(
                            "Failed to fetch JWKS from '{url_for_err}': {e}"
                        ))
                    })?
                    .error_for_status()
                    .map_err(|e| {
                        Error::ConfigValidation(format!(
                            "Failed to fetch JWKS from '{url_for_err}': {e}"
                        ))
                    })?
                    .text()
                    .await
                    .map_err(|e| {
                        Error::ConfigValidation(format!(
                            "Failed to fetch JWKS from '{url_for_err}': {e}"
                        ))
                    })?;

                let jwks: JwkSet = serde_json::from_str(&jwks_text).map_err(|e| {
                    Error::ConfigValidation(format!(
                        "Invalid JWKS response from '{url_for_err}': {e}"
                    ))
                })?;

                let mut key_map = HashMap::new();
                for jwk in &jwks.keys {
                    if let Some(kid) = &jwk.common.key_id {
                        if key_map.contains_key(kid) {
                            return Err(Error::ConfigValidation(format!(
                                "JWKS at '{url_for_err}' contains duplicate kid '{kid}'"
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
                                    "JWK (kid='{kid}') at '{url_for_err}' has no 'alg' field \
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
                        "JWKS at '{url_for_err}' contains no usable keys with a kid"
                    )));
                }

                KeySource::Jwks(key_map)
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

        Ok(Self {
            key_source,
            expected_issuer,
            expected_audience,
            scope_claim_name,
            mask_subject_in_logs: config.mask_subject_in_logs,
        })
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
    fn decoding_key_for<'a>(&'a self, token: &str) -> Result<(&'a DecodingKey, Algorithm), Status> {
        match &self.key_source {
            KeySource::Static(key) => Ok((key, Algorithm::ES256)),
            KeySource::Jwks(map) => {
                let header = decode_header(token)
                    .map_err(|_| Status::unauthenticated("Invalid JWT header"))?;
                let kid = header
                    .kid
                    .ok_or_else(|| Status::unauthenticated("JWT missing kid claim"))?;
                map.get(&kid).map(|(key, alg)| (key, *alg)).ok_or_else(|| {
                    tracing::warn!(kid = %kid, "JWT kid not found in JWKS");
                    Status::unauthenticated("Unknown JWT kid")
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    fn validate_jwt(&self, token: &str) -> Result<Claims, Status> {
        let (key, alg) = self.decoding_key_for(token)?;

        let mut validation = Validation::new(alg);
        validation.set_issuer(&[&self.expected_issuer]);
        validation.set_audience(&[&self.expected_audience]);
        // Require sub in addition to the library's default (exp).
        // An empty sub is caught later by custom_validate_public().
        validation.required_spec_claims.insert("sub".to_string());

        // Decode as a raw JSON map so we can extract scope from the configured
        // claim name without being coupled to a fixed field name in Claims.
        // Signature, exp, iss, and aud are all validated by jsonwebtoken before
        // the payload is deserialised.
        let payload: serde_json::Map<String, serde_json::Value> = decode(token, key, &validation)
            .map_err(|err| {
                tracing::warn!(error = ?err, "JWT validation error");
                Status::unauthenticated("Invalid JWT token")
            })?
            .claims;

        // Extract scope from the configured claim; accept a space-delimited
        // string (RFC 8693) or a JSON array (e.g. Hydra's "scp" claim).
        let scope = match payload.get(&self.scope_claim_name) {
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
                let claims = self.validate_jwt(token)?;
                let subject = if self.mask_subject_in_logs {
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
        let result = interceptor.validate_jwt("this.is.not.a.valid.jwt");
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
        let result = interceptor.validate_jwt(&token);
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

        let result = test_interceptor().await.validate_jwt(&token);
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

        let result = test_interceptor().await.validate_jwt(&token);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }
}
