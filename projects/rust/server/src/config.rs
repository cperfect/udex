use secrets_rs::Secret;
use serde::{Deserialize, Serialize};
use std::net::SocketAddr;
use std::time::Duration;
use udex_telemetry::TelemetryConfig;

/// Server-related configuration for gRPC.
#[derive(Debug, Serialize, Deserialize)]
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
    pub init_indexes: Vec<udex_api::index::CreateIndexRequest>,
    /// OpenTelemetry observability configuration. Absent/disabled by default;
    /// when enabled the server exports OTLP traces/metrics/logs (see
    /// `udex-telemetry`). JSON stdout logging is always on regardless.
    #[serde(default)]
    pub observability: Option<TelemetryConfig>,
}

/// TLS configuration for the gRPC server.
///
/// Both fields are [`Secret<String>`] holding PEM-encoded content. The CLI
/// loads them from `urn:secrets-rs:file:` URNs before constructing this struct;
/// integration tests bind them via `FileSource`. Neither field is cloneable —
/// the secrets are moved into the server at startup.
#[derive(Debug, Serialize, Deserialize)]
pub struct TlsConfig {
    /// PEM-encoded server certificate (bound `Secret<String>`)
    pub cert: Secret<String>,
    /// PEM-encoded server private key (bound `Secret<String>`)
    pub key: Secret<String>,
}

/// Authentication and authorization configuration.
///
/// Exactly one of `jwt_public_key` or `jwks_url` must be set. All fields are
/// validated by [`AuthzConfig::validate`] before the server accepts requests.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthzConfig {
    /// PEM-encoded ECDSA public key for JWT validation (bound `Secret<String>`).
    /// Either this or `jwks_url` must be provided, not both.
    pub jwt_public_key: Option<Secret<String>>,
    /// JWKS endpoint for public key discovery.
    /// Either this or `jwt_public_key` must be provided, not both.
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
    /// When `true`, the JWT `sub` claim is replaced with `[masked]` in audit log lines.
    ///
    /// Enable this when your identity provider places PII (e.g. an email address) in
    /// the subject claim. The preferred alternative is to configure the IdP to issue
    /// an opaque, non-PII identifier as the subject instead — masking is a last resort.
    ///
    /// Defaults to `false`.
    #[serde(default)]
    pub mask_subject_in_logs: bool,
    /// Maximum consecutive failed JWKS refresh attempts before refreshing is
    /// suspended until restart. Only consulted when `jwks_url` is set.
    /// Defaults to 5 when absent.
    #[serde(default)]
    pub jwks_max_failed_refreshes: Option<u32>,
    /// Exponential backoff base in seconds between failed JWKS refresh attempts.
    /// Only consulted when `jwks_url` is set. Defaults to 3 when absent.
    #[serde(default)]
    pub jwks_backoff_factor_secs: Option<u64>,
    /// JWKS cache lifetime in seconds; a background task proactively refreshes
    /// after this interval. Set to 0 to disable time-based refresh (cache-miss
    /// refresh still applies). Only consulted when `jwks_url` is set.
    /// Defaults to 86400 (1 day) when absent.
    #[serde(default)]
    pub jwks_max_age_secs: Option<u64>,
}

impl ServerConfig {
    /// Validate the server configuration.
    pub fn validate(&self) -> Result<(), crate::Error> {
        self.tls.validate()?;
        self.authz.validate()?;
        if let Some(observability) = &self.observability {
            observability
                .validate()
                .map_err(|e| crate::Error::ConfigValidation(e.to_string()))?;
        }
        Ok(())
    }
}

impl TlsConfig {
    pub fn validate(&self) -> Result<(), crate::Error> {
        let cert = self
            .cert
            .value()
            .map_err(|_| crate::Error::ConfigValidation("tls.cert is not bound".to_string()))?;
        if cert.trim().is_empty() {
            return Err(crate::Error::ConfigValidation(
                "tls.cert must not be empty".to_string(),
            ));
        }
        let key = self
            .key
            .value()
            .map_err(|_| crate::Error::ConfigValidation("tls.key is not bound".to_string()))?;
        if key.trim().is_empty() {
            return Err(crate::Error::ConfigValidation(
                "tls.key must not be empty".to_string(),
            ));
        }
        Ok(())
    }
}

impl AuthzConfig {
    pub fn validate(&self) -> Result<(), crate::Error> {
        match (&self.jwks_url, &self.jwt_public_key) {
            (Some(_), Some(_)) => {
                return Err(crate::Error::ConfigValidation(
                    "Exactly one of jwks_url or jwt_public_key must be set, not both".to_string(),
                ));
            }
            (None, None) => {
                return Err(crate::Error::ConfigValidation(
                    "One of jwks_url or jwt_public_key must be set".to_string(),
                ));
            }
            _ => {}
        }

        if let Some(pem_secret) = &self.jwt_public_key {
            if let Ok(pem) = pem_secret.value() {
                if pem.trim().is_empty() {
                    return Err(crate::Error::ConfigValidation(
                        "jwt_public_key must not be empty".to_string(),
                    ));
                }
            }
        }

        if let Some(url) = &self.jwks_url {
            let u = url.trim();
            if u.is_empty() {
                return Err(crate::Error::ConfigValidation(
                    "jwks_url cannot be empty".to_string(),
                ));
            }
            if !self.danger_allow_non_tls && !u.starts_with("https://") {
                return Err(crate::Error::ConfigValidation(format!(
                    "jwks_url '{u}' must use HTTPS; set danger_allow_non_tls = true \
                     to permit plain HTTP (local/dev only, never in production)"
                )));
            }
            if self.jwks_max_failed_refreshes == Some(0) {
                return Err(crate::Error::ConfigValidation(
                    "jwks_max_failed_refreshes must be at least 1 when set; \
                     0 would gate all refreshes before any attempt is made"
                        .to_string(),
                ));
            }
            if self.jwks_backoff_factor_secs == Some(0) {
                return Err(crate::Error::ConfigValidation(
                    "jwks_backoff_factor_secs must be at least 1 when set; \
                     0 produces no backoff between failed refresh attempts"
                        .to_string(),
                ));
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

        if let Some(name) = &self.scope_claim_name {
            if name.trim().is_empty() {
                return Err(crate::Error::ConfigValidation(
                    "scope_claim_name must not be empty or whitespace when set; \
                     omit it to use the default 'scope' claim"
                        .to_string(),
                ));
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrets_rs::{Source, SourceError, SourceRegistry};

    /// Creates a bound `Secret<String>` from a literal value, using an in-memory
    /// source so tests don't touch the filesystem.
    fn bound_secret(value: &str) -> Secret<String> {
        struct FixedSource(String);
        impl Source for FixedSource {
            fn get(&self, _name: &str) -> Result<Vec<u8>, SourceError> {
                Ok(self.0.as_bytes().to_vec())
            }
        }
        let mut s = Secret::new("urn:secrets-rs:fixed:_").expect("valid URN");
        let mut registry = SourceRegistry::new();
        registry
            .register("fixed", FixedSource(value.to_string()))
            .unwrap();
        s.bind(&registry).unwrap();
        s
    }

    fn valid_tls() -> TlsConfig {
        TlsConfig {
            cert: bound_secret("-----BEGIN CERTIFICATE-----\nfake\n-----END CERTIFICATE-----"),
            key: bound_secret("-----BEGIN EC PRIVATE KEY-----\nfake\n-----END EC PRIVATE KEY-----"),
        }
    }

    fn valid_authz() -> AuthzConfig {
        AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bound_secret(
                "-----BEGIN PUBLIC KEY-----\nfake\n-----END PUBLIC KEY-----",
            )),
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        }
    }

    fn valid_authz_jwks() -> AuthzConfig {
        AuthzConfig {
            jwks_url: Some("https://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key: None,
            jwt_issuer: Some("https://hydra:4444".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        }
    }

    #[test]
    fn tls_validate_ok() {
        assert!(valid_tls().validate().is_ok());
    }

    #[test]
    fn tls_validate_empty_cert() {
        let cfg = TlsConfig {
            cert: bound_secret("   "),
            key: bound_secret("-----BEGIN EC PRIVATE KEY-----\nfake\n-----END EC PRIVATE KEY-----"),
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("cert"), "expected cert in error: {err}");
    }

    #[test]
    fn tls_validate_empty_key() {
        let cfg = TlsConfig {
            cert: bound_secret("-----BEGIN CERTIFICATE-----\nfake\n-----END CERTIFICATE-----"),
            key: bound_secret(""),
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("key"), "expected key in error: {err}");
    }

    #[test]
    fn tls_validate_unbound_cert_is_err() {
        let cfg = TlsConfig {
            cert: Secret::new("urn:secrets-rs:file:nonexistent").expect("valid URN"),
            key: bound_secret("-----BEGIN EC PRIVATE KEY-----\nfake\n-----END EC PRIVATE KEY-----"),
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("not bound"),
            "expected not bound in error: {err}"
        );
    }

    #[test]
    fn server_config_validate_calls_tls_validate() {
        let cfg = ServerConfig {
            bind_address: "127.0.0.1:50051".parse().unwrap(),
            request_timeout: std::time::Duration::from_secs(30),
            max_connections: 1000,
            max_message_size: 4 * 1024 * 1024,
            tls: TlsConfig {
                cert: Secret::new("urn:secrets-rs:file:nonexistent").expect("valid URN"),
                key: Secret::new("urn:secrets-rs:file:nonexistent").expect("valid URN"),
            },
            authz: valid_authz(),
            init_indexes: vec![],
            observability: None,
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
            jwt_public_key: Some(bound_secret(
                "-----BEGIN PUBLIC KEY-----\nfake\n-----END PUBLIC KEY-----",
            )),
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
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
            jwt_public_key: None,
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
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
            jwt_public_key: None,
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
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
            jwt_public_key: None,
            jwt_issuer: Some("udex".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn authz_validate_jwks_http_non_localhost_is_err() {
        let cfg = AuthzConfig {
            jwks_url: Some("http://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key: None,
            jwt_issuer: Some("https://issuer.example.com".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
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
            jwt_public_key: None,
            jwt_issuer: Some("http://localhost:4444/".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
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
            jwt_public_key: None,
            jwt_issuer: Some("http://localhost:4444/".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn authz_validate_jwks_danger_allow_non_tls_ok() {
        let cfg = AuthzConfig {
            jwks_url: Some("http://hydra:4444/.well-known/jwks.json".to_string()),
            jwt_public_key: None,
            jwt_issuer: Some("http://hydra:4444/".to_string()),
            jwt_audience: Some("udex".to_string()),
            danger_allow_non_tls: true,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn authz_validate_scope_claim_name_none_ok() {
        assert!(valid_authz().validate().is_ok());
    }

    #[test]
    fn authz_validate_scope_claim_name_non_empty_ok() {
        let cfg = AuthzConfig {
            scope_claim_name: Some("scp".to_string()),
            ..valid_authz()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn authz_validate_scope_claim_name_empty_is_err() {
        let cfg = AuthzConfig {
            scope_claim_name: Some(String::new()),
            ..valid_authz()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("scope_claim_name"),
            "expected 'scope_claim_name' in error: {err}"
        );
    }

    #[test]
    fn authz_validate_scope_claim_name_whitespace_is_err() {
        let cfg = AuthzConfig {
            scope_claim_name: Some("   ".to_string()),
            ..valid_authz()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("scope_claim_name"),
            "expected 'scope_claim_name' in error: {err}"
        );
    }

    #[test]
    fn authz_validate_jwks_max_failed_refreshes_zero_is_err() {
        let cfg = AuthzConfig {
            jwks_max_failed_refreshes: Some(0),
            ..valid_authz_jwks()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("jwks_max_failed_refreshes"),
            "expected 'jwks_max_failed_refreshes' in error: {err}"
        );
    }

    #[test]
    fn authz_validate_jwks_max_failed_refreshes_nonzero_ok() {
        let cfg = AuthzConfig {
            jwks_max_failed_refreshes: Some(1),
            ..valid_authz_jwks()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn authz_validate_jwks_backoff_factor_secs_zero_is_err() {
        let cfg = AuthzConfig {
            jwks_backoff_factor_secs: Some(0),
            ..valid_authz_jwks()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(
            err.contains("jwks_backoff_factor_secs"),
            "expected 'jwks_backoff_factor_secs' in error: {err}"
        );
    }

    #[test]
    fn authz_validate_jwks_backoff_factor_secs_nonzero_ok() {
        let cfg = AuthzConfig {
            jwks_backoff_factor_secs: Some(1),
            ..valid_authz_jwks()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn authz_validate_jwks_max_age_secs_zero_ok() {
        // 0 is explicitly supported — disables the expiry task.
        let cfg = AuthzConfig {
            jwks_max_age_secs: Some(0),
            ..valid_authz_jwks()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn authz_validate_jwks_refresh_params_ignored_for_static_key() {
        // Invalid JWKS-specific values are not validated when jwt_public_key is used.
        let cfg = AuthzConfig {
            jwks_max_failed_refreshes: Some(0),
            jwks_backoff_factor_secs: Some(0),
            ..valid_authz()
        };
        assert!(cfg.validate().is_ok());
    }
}
