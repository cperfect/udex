use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;

/// Server-related configuration for gRPC.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Address to bind the gRPC server
    pub bind_address: SocketAddr,
    /// Request timeout duration
    pub request_timeout: Duration,
    /// Maximum concurrent connections
    pub max_connections: u32,
    /// Maximum message size in bytes
    pub max_message_size: usize,
    /// TLS configuration
    pub tls: TlsConfig,
    /// authz configuration
    pub authz: AuthzConfig,
    /// statically defined indexes
    pub init_indexes: Vec<udex_api::index::UpdateIndexRequest>,
}

/// TLS configuration for the gRPC server.
///
/// Holds PEM-encoded content directly. Callers (e.g. the CLI) are responsible
/// for loading file contents before constructing this struct.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    /// PEM-encoded server certificate
    pub cert_pem: String,
    /// PEM-encoded server private key
    pub key_pem: String,
}

// Authentication and Authorization configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AuthzConfig {
    /// PEM-encoded ECDSA public key for JWT validation — either this or jwks_url must be provided
    pub jwt_public_key_pem: Option<String>,
    /// JWKS endpoint for public key for token validation — either this or jwt_public_key_pem must be provided
    pub jwks_url: Option<String>,
    /// JWT issuer for token validation
    pub jwt_issuer: Option<String>,
    /// JWT audience for token validation
    pub jwt_audience: Option<String>,
    /// Allow plain HTTP for jwks_url. MUST NOT be set in production; intended for
    /// local development environments (e.g. Hydra without TLS).
    #[serde(default)]
    pub danger_allow_non_tls: bool,
    /// Name of the JWT claim that carries the OAuth 2.0 scope list.
    ///
    /// RFC 8693 §4.2 defines this as `"scope"` (a space-delimited string).
    /// Some identity providers use a different name — for example, Hydra uses
    /// `"scp"` with an array value. Set this field to match your IdP.
    ///
    /// The claim value may be a space-delimited string or a JSON array of
    /// strings; both forms are accepted regardless of which name is used.
    ///
    /// Defaults to `"scope"` when not set.
    #[serde(default)]
    pub scope_claim_name: Option<String>,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            bind_address: "127.0.0.1:50051"
                .parse()
                .expect("hardcoded default bind address is always valid"),
            request_timeout: Duration::from_secs(30),
            max_connections: 1000,
            max_message_size: 4 * 1024 * 1024, // 4MB
            tls: TlsConfig::default(),
            init_indexes: Vec::new(),
            authz: AuthzConfig::default(),
        }
    }
}

impl ServerConfig {
    /// Validate the server configuration.
    pub fn validate(&self) -> Result<(), crate::Error> {
        self.tls.validate()?;
        self.authz.validate()?;
        Ok(())
    }
}

impl TlsConfig {
    pub fn validate(&self) -> Result<(), crate::Error> {
        if self.cert_pem.trim().is_empty() {
            return Err(crate::Error::ConfigValidation(
                "tls.cert must not be empty".to_string(),
            ));
        }
        if self.key_pem.trim().is_empty() {
            return Err(crate::Error::ConfigValidation(
                "tls.key must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

impl AuthzConfig {
    pub fn validate(&self) -> Result<(), crate::Error> {
        match (&self.jwks_url, &self.jwt_public_key_pem) {
            (Some(_), Some(_)) => {
                return Err(crate::Error::ConfigValidation(
                    "Exactly one of jwks_url or jwt_public_key_pem must be set, not both"
                        .to_string(),
                ));
            }
            (None, None) => {
                return Err(crate::Error::ConfigValidation(
                    "One of jwks_url or jwt_public_key_pem must be set".to_string(),
                ));
            }
            _ => {}
        }

        if let Some(pem) = &self.jwt_public_key_pem {
            if pem.trim().is_empty() {
                return Err(crate::Error::ConfigValidation(
                    "jwt_public_key must not be empty".to_string(),
                ));
            }
        }

        if let Some(url) = &self.jwks_url {
            let u = url.trim();
            if u.is_empty() {
                return Err(crate::Error::ConfigValidation(
                    "jwks_url cannot be empty".to_string(),
                ));
            }
            // Require HTTPS unless danger_allow_non_tls is explicitly set.
            if !self.danger_allow_non_tls && !u.starts_with("https://") {
                return Err(crate::Error::ConfigValidation(format!(
                    "jwks_url '{u}' must use HTTPS; set danger_allow_non_tls = true \
                     to permit plain HTTP (local/dev only, never in production)"
                )));
            }
        }

        let jwt_issuer = self.jwt_issuer.as_ref().ok_or_else(|| {
            crate::Error::ConfigValidation(
                "jwt_issuer is required when using JWT authentication".to_string(),
            )
        })?;
        let jwt_audience = self.jwt_audience.as_ref().ok_or_else(|| {
            crate::Error::ConfigValidation(
                "jwt_audience is required when using JWT authentication".to_string(),
            )
        })?;

        if jwt_issuer.trim().is_empty() {
            return Err(crate::Error::ConfigValidation(
                "jwt_issuer cannot be empty".to_string(),
            ));
        }
        if jwt_issuer.len() > 255 {
            return Err(crate::Error::ConfigValidation(
                "jwt_issuer must be no more than 255 characters long".to_string(),
            ));
        }

        if jwt_audience.trim().is_empty() {
            return Err(crate::Error::ConfigValidation(
                "jwt_audience cannot be empty".to_string(),
            ));
        }
        if jwt_audience.len() > 255 {
            return Err(crate::Error::ConfigValidation(
                "jwt_audience must be no more than 255 characters long".to_string(),
            ));
        }

        if jwt_issuer == jwt_audience {
            return Err(crate::Error::ConfigValidation(
                "jwt_issuer and jwt_audience should be different values".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_authz() -> AuthzConfig {
        AuthzConfig {
            jwks_url: None,
            jwt_public_key_pem: Some(
                "-----BEGIN PUBLIC KEY-----\nfake\n-----END PUBLIC KEY-----".to_string(),
            ),
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        }
    }

    fn valid_authz_jwks() -> AuthzConfig {
        AuthzConfig {
            jwks_url: Some("https://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_pem: None,
            jwt_issuer: Some("https://hydra:4444".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        }
    }

    #[test]
    fn tls_validate_ok() {
        let cfg = TlsConfig {
            cert_pem: "-----BEGIN CERTIFICATE-----\nfake\n-----END CERTIFICATE-----".to_string(),
            key_pem: "-----BEGIN EC PRIVATE KEY-----\nfake\n-----END EC PRIVATE KEY-----"
                .to_string(),
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn tls_validate_empty_cert() {
        let cfg = TlsConfig {
            cert_pem: String::new(),
            key_pem: "-----BEGIN EC PRIVATE KEY-----\nfake\n-----END EC PRIVATE KEY-----"
                .to_string(),
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("cert"), "expected cert in error: {err}");
    }

    #[test]
    fn tls_validate_empty_key() {
        let cfg = TlsConfig {
            cert_pem: "-----BEGIN CERTIFICATE-----\nfake\n-----END CERTIFICATE-----".to_string(),
            key_pem: String::new(),
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("key"), "expected key in error: {err}");
    }

    #[test]
    fn server_config_validate_calls_tls_validate() {
        let cfg = ServerConfig {
            tls: TlsConfig {
                cert_pem: String::new(),
                key_pem: String::new(),
            },
            authz: valid_authz(),
            ..ServerConfig::default()
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn authz_validate_static_key_ok() {
        assert!(valid_authz().validate().is_ok());
    }

    #[test]
    fn authz_validate_jwks_url_ok() {
        assert!(valid_authz_jwks().validate().is_ok());
    }

    #[test]
    fn authz_validate_both_key_sources_is_err() {
        let cfg = AuthzConfig {
            jwks_url: Some("https://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_pem: Some(
                "-----BEGIN PUBLIC KEY-----\nfake\n-----END PUBLIC KEY-----".to_string(),
            ),
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("not both"),
            "expected 'not both' in error: {err}"
        );
    }

    #[test]
    fn authz_validate_no_key_source_is_err() {
        let cfg = AuthzConfig {
            jwks_url: None,
            jwt_public_key_pem: None,
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("must be set"),
            "expected 'must be set' in error: {err}"
        );
    }

    #[test]
    fn authz_validate_empty_jwks_url_is_err() {
        let cfg = AuthzConfig {
            jwks_url: Some("   ".to_string()),
            jwt_public_key_pem: None,
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("jwks_url"),
            "expected 'jwks_url' in error: {err}"
        );
    }

    #[test]
    fn authz_validate_issuer_equals_audience_is_err() {
        let cfg = AuthzConfig {
            jwks_url: Some("https://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_pem: None,
            jwt_issuer: Some("udex".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn authz_validate_jwks_http_non_localhost_is_err() {
        let cfg = AuthzConfig {
            jwks_url: Some("http://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_pem: None,
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("must use HTTPS"),
            "expected HTTPS error: {err}"
        );
    }

    #[test]
    fn authz_validate_jwks_localhost_http_is_err_without_flag() {
        let cfg = AuthzConfig {
            jwks_url: Some("http://localhost:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_pem: None,
            jwt_issuer: Some("http://localhost:4444/".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("must use HTTPS"),
            "expected HTTPS error for localhost without flag: {err}"
        );
    }

    #[test]
    fn authz_validate_jwks_localhost_http_ok_with_flag() {
        let cfg = AuthzConfig {
            jwks_url: Some("http://localhost:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_pem: None,
            jwt_issuer: Some("http://localhost:4444/".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: None,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn authz_validate_jwks_danger_allow_non_tls_ok() {
        let cfg = AuthzConfig {
            jwks_url: Some("http://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key_pem: None,
            jwt_issuer: Some("http://hydra:4444/".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: None,
        };
        assert!(cfg.validate().is_ok());
    }
}
