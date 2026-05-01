use anyhow::{Context, Result};
use secrets_rs::{bind_all, EnvSource, Secret, SourceRegistry};
use serde::{Deserialize, Serialize};

/// Top-level Udex configuration, covering the server and datastore.
#[derive(Debug, Serialize, Deserialize)]
pub struct UdexConfig {
    pub server: ServerConfig,
    pub datastore: DatastoreConfig,
}

/// gRPC server configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Address and port to bind the gRPC server (e.g. "0.0.0.0:50051")
    pub bind_address: String,
    /// Request timeout in seconds
    pub request_timeout_secs: u64,
    /// Maximum number of concurrent connections
    pub max_connections: u32,
    /// Maximum message size in bytes
    pub max_message_size_bytes: usize,
    pub tls: TlsConfig,
    pub authz: AuthzConfig,
}

/// TLS certificate configuration.
#[derive(Debug, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Path to the server TLS certificate (PEM)
    pub cert_path: String,
    /// Path to the server TLS private key (PEM)
    pub key_path: String,
}

/// Authentication and authorisation configuration.
#[derive(Debug, Serialize, Deserialize, Default)]
pub struct AuthzConfig {
    /// JWKS endpoint for token validation — either this or jwt_public_key_path must be set
    pub jwks_url: Option<String>,
    /// Path to the JWT public key for token validation (ECDSA PEM)
    pub jwt_public_key_path: Option<String>,
    /// Expected JWT issuer claim
    pub jwt_issuer: Option<String>,
    /// Expected JWT audience claim
    pub jwt_audience: Option<String>,
    /// Allow plain HTTP for jwks_url. MUST NOT be set in production; intended for
    /// local development environments (e.g. Hydra without TLS).
    #[serde(default)]
    pub danger_allow_non_tls: bool,
    /// Name of the JWT claim that carries the OAuth 2.0 scope list.
    /// RFC 8693 §4.2 default is `"scope"`. Set to e.g. `"scp"` for Hydra.
    #[serde(default)]
    pub scope_claim_name: Option<String>,
}

/// PostgreSQL datastore configuration.
///
/// `connection_url` is a [`Secret<String>`] — its real value is masked in
/// Debug/Display/serde output. In a loaded config it is bound from the env source
/// declared in the URN (e.g. `DATABASE_URL`). [`UdexConfig::load`] handles
/// binding automatically; [`UdexConfig::validate`] skips the URL check when the
/// secret is not yet bound.
#[derive(Debug, Serialize, Deserialize, secrets_rs::Bindable)]
pub struct DatastoreConfig {
    /// PostgreSQL connection URL, referenced as a secret URN.
    /// Example: `urn:secrets-rs:env:DATABASE_URL`
    pub connection_url: Secret<String>,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    /// Connection timeout in seconds
    pub connection_timeout_secs: u64,
    /// Query timeout in seconds
    pub query_timeout_secs: u64,
    /// Disable TLS enforcement on the connection URL. MUST NOT be set in production.
    #[serde(default)]
    pub dangerous_allow_non_tls: bool,
}

impl Default for UdexConfig {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                bind_address: "127.0.0.1:50051".to_string(),
                request_timeout_secs: 30,
                max_connections: 1000,
                max_message_size_bytes: 4 * 1024 * 1024,
                tls: TlsConfig {
                    cert_path: "certs/server.crt".to_string(),
                    key_path: "certs/server.key".to_string(),
                },
                authz: AuthzConfig {
                    jwks_url: None,
                    jwt_public_key_path: Some("certs/jwt_public_key.pem".to_string()),
                    jwt_issuer: Some("https://auth.example.com".to_string()),
                    jwt_audience: Some("udex".to_string()),
                    danger_allow_non_tls: false,
                    scope_claim_name: None,
                },
            },
            datastore: DatastoreConfig {
                connection_url: Secret::new("urn:secrets-rs:env:DATABASE_URL")
                    .expect("hardcoded URN is always valid"),
                max_connections: 10,
                min_connections: 1,
                connection_timeout_secs: 10,
                query_timeout_secs: 30,
                dangerous_allow_non_tls: false,
            },
        }
    }
}

impl UdexConfig {
    /// Load a [`UdexConfig`] from a TOML file and bind all secrets from
    /// environment variables.
    ///
    /// Returns an error if the file cannot be read, the TOML is malformed, or
    /// any secret URN cannot be resolved from its declared source (e.g.
    /// `DATABASE_URL` is not set).
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let mut cfg: Self = toml::from_str(&content)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        let mut registry = SourceRegistry::new();
        registry.register("env", EnvSource);
        bind_all(&mut cfg.datastore, &registry).map_err(|errs| {
            anyhow::anyhow!(
                "failed to bind config secrets: {}",
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        Ok(cfg)
    }

    /// Validate the configuration, returning a list of human-readable errors.
    ///
    /// The `connection_url` check is skipped when the secret is not yet bound
    /// (binding failures are caught by [`UdexConfig::load`]). All other checks
    /// are stateless and run unconditionally.
    pub fn validate(&self) -> Result<()> {
        let mut errors: Vec<String> = Vec::new();

        // server
        if self
            .server
            .bind_address
            .parse::<std::net::SocketAddr>()
            .is_err()
        {
            errors.push(format!(
                "invalid bind_address {:?}: must be a valid socket address (e.g. \"0.0.0.0:50051\")",
                self.server.bind_address
            ));
        }

        // datastore
        if let Ok(url) = self.datastore.connection_url.value() {
            if url.trim().is_empty() {
                errors.push("datastore.connection_url must not be empty".to_string());
            }
        }
        if self.datastore.max_connections == 0 {
            errors.push("datastore.max_connections must be greater than 0".to_string());
        }
        if self.datastore.min_connections > self.datastore.max_connections {
            errors.push(
                "datastore.min_connections must not exceed datastore.max_connections".to_string(),
            );
        }

        // authz: exactly one of jwks_url or jwt_public_key_path must be set
        let has_pem = self.server.authz.jwt_public_key_path.is_some();
        let has_jwks = self.server.authz.jwks_url.is_some();
        match (has_pem, has_jwks) {
            (true, true) => errors.push(
                "server.authz: only one of jwt_public_key_path or jwks_url may be set".to_string(),
            ),
            (false, false) => errors.push(
                "server.authz: one of jwks_url or jwt_public_key_path must be set".to_string(),
            ),
            _ => {}
        }

        if let Some(path) = &self.server.authz.jwt_public_key_path {
            if path.trim().is_empty() {
                errors.push("server.authz.jwt_public_key_path cannot be empty".to_string());
            }
        }
        if let Some(url) = &self.server.authz.jwks_url {
            if url.trim().is_empty() {
                errors.push("server.authz.jwks_url cannot be empty".to_string());
            }
        }

        let jwt_issuer = self.server.authz.jwt_issuer.as_deref().unwrap_or("").trim();
        let jwt_audience = self
            .server
            .authz
            .jwt_audience
            .as_deref()
            .unwrap_or("")
            .trim();
        if jwt_issuer.is_empty() {
            errors.push("server.authz.jwt_issuer is required".to_string());
        } else if jwt_issuer.len() > 255 {
            errors.push("server.authz.jwt_issuer must be 255 characters or fewer".to_string());
        }
        if jwt_audience.is_empty() {
            errors.push("server.authz.jwt_audience is required".to_string());
        } else if jwt_audience.len() > 255 {
            errors.push("server.authz.jwt_audience must be 255 characters or fewer".to_string());
        }
        if !jwt_issuer.is_empty() && !jwt_audience.is_empty() && jwt_issuer == jwt_audience {
            errors.push(
                "server.authz.jwt_issuer and server.authz.jwt_audience must be different"
                    .to_string(),
            );
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(anyhow::anyhow!(
                "configuration is invalid:\n  - {}",
                errors.join("\n  - ")
            ))
        }
    }

    /// Convert to the server's [`udex_server::config::ServerConfig`].
    pub fn to_server_config(&self) -> Result<udex_server::config::ServerConfig> {
        let bind_address = self
            .server
            .bind_address
            .parse()
            .with_context(|| format!("invalid bind_address: {}", self.server.bind_address))?;

        Ok(udex_server::config::ServerConfig {
            bind_address,
            request_timeout: std::time::Duration::from_secs(self.server.request_timeout_secs),
            max_connections: self.server.max_connections,
            max_message_size: self.server.max_message_size_bytes,
            tls: udex_server::config::TlsConfig {
                cert_path: self.server.tls.cert_path.clone(),
                key_path: self.server.tls.key_path.clone(),
            },
            authz: udex_server::config::AuthzConfig {
                jwks_url: self.server.authz.jwks_url.clone(),
                jwt_public_key_path: self.server.authz.jwt_public_key_path.clone(),
                jwt_issuer: self.server.authz.jwt_issuer.clone(),
                jwt_audience: self.server.authz.jwt_audience.clone(),
                danger_allow_non_tls: self.server.authz.danger_allow_non_tls,
                scope_claim_name: self.server.authz.scope_claim_name.clone(),
            },
            init_indexes: vec![],
        })
    }

    /// Convert to the datastore's [`udex_datastore::config::DatastoreConfig`].
    ///
    /// Copies the connection URL URN from this config and binds it against the
    /// environment, returning a fully-bound datastore config ready for use.
    ///
    /// # Double-bind note
    ///
    /// `Secret<T>` does not implement `Clone`, so we cannot move the already-bound
    /// value from `load()`. Instead we reconstruct a fresh `Secret` from the URN and
    /// bind it again, reading `DATABASE_URL` from the environment a second time. This
    /// also means this method is implicitly coupled to `EnvSource` — if `load()` ever
    /// gains a non-`env` source, `to_datastore_config()` will fail even though `load()`
    /// succeeded. Resolve this once `secrets-rs` implements `Clone` on `Secret<T>`: at
    /// that point, clone `self.datastore.connection_url` directly instead of re-binding.
    pub fn to_datastore_config(&self) -> Result<udex_datastore::config::DatastoreConfig> {
        let urn_str = self.datastore.connection_url.urn().to_string();
        let mut config = udex_datastore::config::DatastoreConfig {
            connection_url: Secret::new(&urn_str)
                .expect("URN from a parsed Secret is always valid"),
            max_connections: self.datastore.max_connections,
            min_connections: self.datastore.min_connections,
            connection_timeout: std::time::Duration::from_secs(
                self.datastore.connection_timeout_secs,
            ),
            query_timeout: std::time::Duration::from_secs(self.datastore.query_timeout_secs),
            dangerous_allow_non_tls: self.datastore.dangerous_allow_non_tls,
        };

        let mut registry = SourceRegistry::new();
        registry.register("env", EnvSource);
        bind_all(&mut config, &registry).map_err(|errs| {
            anyhow::anyhow!(
                "failed to bind datastore secrets: {}",
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrets_rs::{bind_all, SourceError, SourceRegistry};

    /// In-memory secret source — avoids env var access and test races.
    struct MapSource(std::collections::HashMap<String, String>);

    impl MapSource {
        fn with(key: &str, value: &str) -> Self {
            let mut m = std::collections::HashMap::new();
            m.insert(key.to_string(), value.to_string());
            Self(m)
        }
    }

    impl secrets_rs::Source for MapSource {
        fn get(&self, name: &str) -> Result<Vec<u8>, SourceError> {
            self.0
                .get(name)
                .map(|v| v.as_bytes().to_vec())
                .ok_or_else(|| SourceError::NotFound {
                    name: name.to_owned(),
                })
        }
    }

    /// Bind `cfg.datastore.connection_url` to `url` using an in-memory source.
    fn bind_url(cfg: &mut UdexConfig, url: &str) {
        let mut registry = SourceRegistry::new();
        registry.register("env", MapSource::with("DATABASE_URL", url));
        bind_all(&mut cfg.datastore, &registry).unwrap();
    }

    #[test]
    fn test_default_config_is_valid() {
        // connection_url is unbound on a default(); validate() skips that check,
        // so all other defaults must be valid.
        UdexConfig::default()
            .validate()
            .expect("default config should be valid");
    }

    #[test]
    fn test_invalid_bind_address_is_invalid() {
        let mut cfg = UdexConfig::default();
        cfg.server.bind_address = "not-an-address".to_string();
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_empty_connection_url_is_invalid() {
        let mut cfg = UdexConfig::default();
        bind_url(&mut cfg, "");
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_min_connections_exceeds_max_is_invalid() {
        let mut cfg = UdexConfig::default();
        cfg.datastore.min_connections = 20;
        cfg.datastore.max_connections = 5;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_jwks_url_only_is_valid() {
        let mut cfg = UdexConfig::default();
        cfg.server.authz.jwt_public_key_path = None;
        cfg.server.authz.jwks_url = Some("http://localhost:4444/.well-known/jwks.json".to_string());
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn test_both_key_sources_is_invalid() {
        let mut cfg = UdexConfig::default();
        cfg.server.authz.jwks_url = Some("http://localhost:4444/.well-known/jwks.json".to_string());
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_no_key_source_is_invalid() {
        let mut cfg = UdexConfig::default();
        cfg.server.authz.jwt_public_key_path = None;
        cfg.server.authz.jwks_url = None;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_identical_issuer_and_audience_is_invalid() {
        let mut cfg = UdexConfig::default();
        cfg.server.authz.jwt_issuer = Some("same".to_string());
        cfg.server.authz.jwt_audience = Some("same".to_string());
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn test_config_serializes_without_panicking() {
        // Verify that serialization succeeds. Note: Secret<String> serializes as
        // a masked string (not a bare URN), so the output is not suitable for
        // round-tripping through toml::from_str — config files must always be
        // hand-authored with bare URN strings such as "urn:secrets-rs:env:DATABASE_URL".
        let cfg = UdexConfig::default();
        let toml_str = toml::to_string_pretty(&cfg).expect("serialization should succeed");
        assert!(toml_str.contains("bind_address"));
        assert!(toml_str.contains("max_connections"));
    }

    // WP-05: Verify that Secret<String> deserialization acts as a file-injection guard.
    // Secret<T>::Deserialize only accepts a bare URN — plain secret values are rejected
    // at parse time, so a misconfigured TOML file with a raw connection string cannot
    // reach the application.

    #[test]
    fn test_plain_url_rejected_by_deserializer() {
        let toml = r#"
[server]
bind_address = "0.0.0.0:50051"
request_timeout_secs = 30
max_connections = 1000
max_message_size_bytes = 4194304

[server.tls]
cert_path = "certs/server.crt"
key_path = "certs/server.key"

[server.authz]
jwt_public_key_path = "certs/jwt_public_key.pem"
jwt_issuer = "https://auth.example.com"
jwt_audience = "udex"

[datastore]
connection_url = "postgres://user:password@localhost:5432/db"
max_connections = 10
min_connections = 1
connection_timeout_secs = 10
query_timeout_secs = 30
"#;
        let result = toml::from_str::<UdexConfig>(toml);
        assert!(
            result.is_err(),
            "plain connection URL must be rejected at deserialization"
        );
    }

    #[test]
    fn test_valid_urn_accepted_by_deserializer() {
        let toml = r#"
[server]
bind_address = "0.0.0.0:50051"
request_timeout_secs = 30
max_connections = 1000
max_message_size_bytes = 4194304

[server.tls]
cert_path = "certs/server.crt"
key_path = "certs/server.key"

[server.authz]
jwt_public_key_path = "certs/jwt_public_key.pem"
jwt_issuer = "https://auth.example.com"
jwt_audience = "udex"

[datastore]
connection_url = "urn:secrets-rs:env:DATABASE_URL"
max_connections = 10
min_connections = 1
connection_timeout_secs = 10
query_timeout_secs = 30
"#;
        let result = toml::from_str::<UdexConfig>(toml);
        assert!(
            result.is_ok(),
            "valid URN must deserialize successfully: {:?}",
            result.err()
        );
        assert_eq!(
            result.unwrap().datastore.connection_url.urn().to_string(),
            "urn:secrets-rs:env:DATABASE_URL"
        );
    }
}
