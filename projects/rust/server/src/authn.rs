use jsonwebtoken::{decode, Algorithm, DecodingKey, Validation};
use tonic::body::Body;
use tonic::codegen::http::Request;
use tonic::Status;
use tonic_middleware::RequestInterceptor;
use udex_api::authz::claims::Claims;

use crate::config::AuthNzConfig;
use crate::Error;

#[derive(Clone)]
pub struct AuthnInterceptor {
    public_key: DecodingKey,
    expected_issuer: String,
    expected_audience: String,
}

impl AuthnInterceptor {
    pub fn new(config: AuthNzConfig) -> Result<Self, Error> {
        let public_key =
            std::fs::read_to_string(config.jwt_public_key_path.as_ref().ok_or_else(|| {
                Error::ConfigValidation(
                    "jwt_public_key_path is required when using JWT authentication".to_string(),
                )
            })?)
            .map_err(|e| {
                Error::ConfigValidation(format!("Failed to read jwt_public_key_path: {}", e))
            })
            .map(|key| {
                DecodingKey::from_ec_pem(key.as_bytes()).map_err(|e| {
                    Error::ConfigValidation(format!("Failed to create decoding key: {}", e))
                })
            })??;
        let jwt_issuer = config
            .jwt_issuer
            .as_ref()
            .ok_or_else(|| {
                Error::ConfigValidation(
                    "jwt_issuer is required when using JWT authentication".to_string(),
                )
            })?
            .clone();
        let jwt_audience = config
            .jwt_audience
            .as_ref()
            .ok_or_else(|| {
                Error::ConfigValidation(
                    "jwt_audience is required when using JWT authentication".to_string(),
                )
            })?
            .clone();

        Ok(Self {
            public_key,
            expected_issuer: jwt_issuer,
            expected_audience: jwt_audience,
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
    fn validate_jwt(&self, token: &str) -> Result<Claims, Status> {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.set_issuer(&[&self.expected_issuer]);
        validation.set_audience(&[&self.expected_audience]);

        let claims = match decode::<Claims>(token, &self.public_key, &validation) {
            Ok(token_data) => Ok(token_data.claims),
            Err(err) => {
                tracing::warn!(error = ?err, "JWT validation error");
                Err(Status::unauthenticated("Invalid JWT token"))
            }
        }?;

        // validate the public claims
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
            jwt_public_key_path: Some("tests/jwt/signing_public_key.pem".to_string()),
            jwt_issuer: Some("test-issuer".to_string()),
            jwt_audience: Some("test-audience".to_string()),
        })
        .expect("Failed to create test AuthnInterceptor")
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

        // Build a JWT signed with the real key but with the wrong issuer.
        // This passes signature validation but fails issuer validation,
        // confirming the warn fires for that specific code path.
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
