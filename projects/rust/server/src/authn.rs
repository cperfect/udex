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
            }).map(|key| {
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

    fn extract_bearer_token<'a>(&self, auth_header: &'a str) -> Result<&'a str, Status> {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            Ok(token.trim())
        } else {
            Err(Status::unauthenticated(
                "Authorization header must start with 'Bearer '",
            ))
        }
    }

    fn validate_jwt(&self, token: &str) -> Result<Claims, Status> {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.set_issuer(&[&self.expected_issuer]);
        validation.set_audience(&[&self.expected_audience]);

        let claims = match decode::<Claims>(token, &self.public_key, &validation) {
            Ok(token_data) => Ok(token_data.claims),
            Err(err) => {
                println!("JWT validation error: {:?}", err);
                Err(Status::unauthenticated("Invalid JWT token"))
            }
        }.or_else(|_| {
            Err(Status::unauthenticated("Failed to decode JWT token"))
        })?;

        // validate the public claims
        claims.custom_validate_public().map_err(|e| {
            Status::unauthenticated(format!("JWT validation failed: {}", e))
        })?;

        Ok(claims)
    }

    // fn validate_api_key(&self, token: &str) -> Result<(), Status> {
    //     let api_key = self.config.api_key.as_ref()
    //         .ok_or_else(|| Status::unauthenticated("API key not configured"))?;

    //     if token == format!("Bearer {}", api_key).as_str() {
    //         Ok(())
    //     } else {
    //         Err(Status::unauthenticated("Invalid API key"))
    //     }
    // }
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
                println!("JWT validation successful for token");

                //add the claims to the request extensions
                req.extensions_mut().insert(claims);
                println!("Claims added to request extensions");
                Ok(req)
            }
            None => Err(Status::unauthenticated("Authorization header missing")),
        }
    }
}
