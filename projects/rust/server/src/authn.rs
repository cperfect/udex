use std::collections::HashMap;

use jsonwebtoken::jwk::JwkSet;
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use tonic::body::Body;
use tonic::codegen::http::Request;
use tonic::Status;
use tonic_middleware::RequestInterceptor;
use udex_api::authz::claims::Claims;

use crate::config::AuthNzConfig;
use crate::Error;

#[derive(Clone)]
enum KeySource {
    Static(DecodingKey),
    Jwks(HashMap<String, DecodingKey>),
}

#[derive(Clone)]
pub struct AuthnInterceptor {
    key_source: KeySource,
    expected_issuer: String,
    expected_audience: String,
}

impl AuthnInterceptor {
    pub fn new(config: AuthNzConfig) -> Result<Self, Error> {
        let key_source = match (config.jwks_url, config.jwt_public_key_path) {
            (Some(url), None) => {
                let url_for_err = url.clone();
                let jwks_text = std::thread::spawn(move || -> Result<String, String> {
                    reqwest::blocking::get(&url)
                        .map_err(|e| e.to_string())?
                        .error_for_status()
                        .map_err(|e| e.to_string())?
                        .text()
                        .map_err(|e| e.to_string())
                })
                .join()
                .map_err(|_| Error::ConfigValidation("JWKS fetch panicked".to_string()))?
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
                        let decoding_key = DecodingKey::from_jwk(jwk).map_err(|e| {
                            Error::ConfigValidation(format!("Invalid JWK (kid='{kid}'): {e}"))
                        })?;
                        key_map.insert(kid.clone(), decoding_key);
                    }
                }

                if key_map.is_empty() {
                    return Err(Error::ConfigValidation(format!(
                        "JWKS at '{url_for_err}' contains no usable keys with a kid"
                    )));
                }

                KeySource::Jwks(key_map)
            }
            (None, Some(path)) => {
                let pem = std::fs::read_to_string(&path).map_err(|e| {
                    Error::ConfigValidation(format!(
                        "Failed to read jwt_public_key_path '{path}': {e}"
                    ))
                })?;
                let key = DecodingKey::from_ec_pem(pem.as_bytes()).map_err(|e| {
                    Error::ConfigValidation(format!(
                        "Failed to create decoding key from '{path}': {e}"
                    ))
                })?;
                KeySource::Static(key)
            }
            (Some(_), Some(_)) => {
                return Err(Error::ConfigValidation(
                    "Exactly one of jwks_url or jwt_public_key_path must be set, not both"
                        .to_string(),
                ));
            }
            (None, None) => {
                return Err(Error::ConfigValidation(
                    "One of jwks_url or jwt_public_key_path must be set".to_string(),
                ));
            }
        };

        let expected_issuer = config.jwt_issuer.ok_or_else(|| {
            Error::ConfigValidation(
                "jwt_issuer is required when using JWT authentication".to_string(),
            )
        })?;
        let expected_audience = config.jwt_audience.ok_or_else(|| {
            Error::ConfigValidation(
                "jwt_audience is required when using JWT authentication".to_string(),
            )
        })?;

        Ok(Self {
            key_source,
            expected_issuer,
            expected_audience,
        })
    }

    #[allow(clippy::result_large_err)]
    fn extract_bearer_token<'a>(&self, auth_header: &'a str) -> Result<&'a str, Status> {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            Ok(token.trim())
        } else {
            Err(Status::unauthenticated(
                "Authorization header must start with 'Bearer '",
            ))
        }
    }

    #[allow(clippy::result_large_err)]
    fn decoding_key_for<'a>(&'a self, token: &str) -> Result<&'a DecodingKey, Status> {
        match &self.key_source {
            KeySource::Static(key) => Ok(key),
            KeySource::Jwks(map) => {
                let header = decode_header(token)
                    .map_err(|_| Status::unauthenticated("Invalid JWT header"))?;
                let kid = header
                    .kid
                    .ok_or_else(|| Status::unauthenticated("JWT missing kid claim"))?;
                map.get(&kid).ok_or_else(|| {
                    tracing::warn!(kid = %kid, "JWT kid not found in JWKS");
                    Status::unauthenticated("Unknown JWT kid")
                })
            }
        }
    }

    #[allow(clippy::result_large_err)]
    fn validate_jwt(&self, token: &str) -> Result<Claims, Status> {
        let key = self.decoding_key_for(token)?;

        let mut validation = Validation::new(Algorithm::ES256);
        validation.set_issuer(&[&self.expected_issuer]);
        validation.set_audience(&[&self.expected_audience]);

        let claims = match decode::<Claims>(token, key, &validation) {
            Ok(token_data) => Ok(token_data.claims),
            Err(err) => {
                tracing::warn!(error = ?err, "JWT validation error");
                Err(Status::unauthenticated("Invalid JWT token"))
            }
        }?;

        claims
            .custom_validate_public()
            .map_err(|e| Status::unauthenticated(format!("JWT validation failed: {}", e)))?;

        Ok(claims)
    }
}

#[tonic::async_trait]
impl RequestInterceptor for AuthnInterceptor {
    async fn intercept(&self, mut req: Request<Body>) -> Result<Request<Body>, Status> {
        match req
            .headers()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
        {
            Some(auth_header) => {
                let token = self.extract_bearer_token(auth_header)?;
                let claims = self.validate_jwt(token)?;
                tracing::debug!("JWT validation successful");

                req.extensions_mut().insert(claims);
                tracing::debug!("Claims added to request extensions");
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
    use crate::config::AuthNzConfig;
    use tracing_test::traced_test;

    fn test_interceptor() -> AuthnInterceptor {
        AuthnInterceptor::new(AuthNzConfig {
            jwks_url: None,
            jwt_public_key_path: Some("tests/jwt/signing_public_key.pem".to_string()),
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
        })
        .expect("Failed to create test AuthnInterceptor")
    }

    #[test]
    fn test_new_rejects_both_key_sources() {
        let result = AuthnInterceptor::new(AuthNzConfig {
            jwks_url: Some("http://localhost:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_path: Some("tests/jwt/signing_public_key.pem".to_string()),
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
        });
        assert!(result.is_err());
    }

    #[test]
    fn test_new_rejects_no_key_source() {
        let result = AuthnInterceptor::new(AuthNzConfig {
            jwks_url: None,
            jwt_public_key_path: None,
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
        });
        assert!(result.is_err());
    }

    #[traced_test]
    #[test]
    fn test_invalid_jwt_emits_warn() {
        let interceptor = test_interceptor();
        let result = interceptor.validate_jwt("this.is.not.a.valid.jwt");
        assert!(result.is_err());
        assert!(logs_contain("JWT validation error"));
    }

    #[traced_test]
    #[test]
    fn test_jwt_wrong_issuer_emits_warn() {
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

        let interceptor = test_interceptor();
        let result = interceptor.validate_jwt(&token);
        assert!(result.is_err());
        assert!(logs_contain("JWT validation error"));
    }
}
