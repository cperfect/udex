use anyhow::{Context, Result};
use secrets_rs::{bind_all, sources::file::FileSource, Secret, SourceRegistry};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use udex_server::config::{AuthzConfig, TlsConfig};
use udex_telemetry::TelemetryConfig;

/// Top-level Udex configuration, covering the server and datastore.
#[derive(Debug, Serialize, Deserialize)]
pub struct UdexConfig {
    pub server: ServerConfig,
    pub datastore: DatastoreConfig,
}

/// gRPC server configuration.
///
/// `tls` and `authz` reuse the server crate's types directly — no field
/// duplication between the CLI config and the types passed to the server.
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
    /// OpenTelemetry observability (traces/metrics/logs). Absent/disabled by
    /// default; configure `enabled`, `otlp_endpoint`, `sample_ratio`, etc.
    #[serde(default)]
    pub observability: Option<TelemetryConfig>,
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
    /// Allow the server to apply outstanding migrations on startup. Defaults to `false`.
    /// Prefer `udex migrate apply` as an explicit pre-deploy step in production.
    #[serde(default)]
    pub apply_migrations: bool,
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
                    cert: Secret::new("urn:secrets-rs:file:certs/server.crt")
                        .expect("hardcoded URN is always valid"),
                    key: Secret::new("urn:secrets-rs:file:certs/server.key")
                        .expect("hardcoded URN is always valid"),
                },
                authz: AuthzConfig {
                    jwks_url: None,
                    jwt_public_key: Some(
                        Secret::new("urn:secrets-rs:file:certs/jwt_public_key.pem")
                            .expect("hardcoded URN is always valid"),
                    ),
                    jwt_issuer: Some("https://auth.example.com".to_string()),
                    jwt_audience: Some("udex".to_string()),
                    danger_allow_non_tls: false,
                    scope_claim_name: None,
                    mask_subject_in_logs: false,
                    jwks_max_failed_refreshes: None,
                    jwks_backoff_factor_secs: None,
                    jwks_max_age_secs: None,
                },
                observability: None,
            },
            datastore: DatastoreConfig {
                connection_url: Secret::new("urn:secrets-rs:env:DATABASE_URL")
                    .expect("hardcoded URN is always valid"),
                max_connections: 10,
                min_connections: 1,
                connection_timeout_secs: 10,
                query_timeout_secs: 30,
                dangerous_allow_non_tls: false,
                apply_migrations: false,
            },
        }
    }
}

impl UdexConfig {
    /// Load a [`UdexConfig`] from a YAML file and bind all secrets from
    /// their declared sources.
    ///
    /// Returns an error if the file cannot be read, the YAML is malformed, or
    /// any secret URN cannot be resolved from its declared source (e.g.
    /// `DATABASE_URL` is not set).
    pub fn load(path: &std::path::Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;
        let mut cfg: Self = serde_saphyr::from_str(&content)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        // EnvSource is pre-registered under "env" by SourceRegistry::new().
        // File URNs are resolved relative to the config file's directory so that
        // cert and key paths in the config are portable alongside the config file.
        let config_dir = path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .to_path_buf();
        let mut registry = SourceRegistry::new();
        registry
            .register("file", FileSource::with_base(config_dir))
            .context("failed to register file source")?;

        // Bind TLS secrets from file URNs.
        cfg.server
            .tls
            .cert
            .bind(&registry)
            .context("failed to bind server.tls.cert")?;
        cfg.server
            .tls
            .key
            .bind(&registry)
            .context("failed to bind server.tls.key")?;

        // Bind the JWT public key if using static key mode (not JWKS).
        if let Some(ref mut jwt_key) = cfg.server.authz.jwt_public_key {
            jwt_key
                .bind(&registry)
                .context("failed to bind server.authz.jwt_public_key")?;
        }

        // Bind datastore secrets from env URNs.
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

    /// Validate the configuration, returning a human-readable error if invalid.
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

        // authz: exactly one of jwks_url or jwt_public_key must be set
        let has_pem = self.server.authz.jwt_public_key.is_some();
        let has_jwks = self.server.authz.jwks_url.is_some();
        match (has_pem, has_jwks) {
            (true, true) => errors.push(
                "server.authz: only one of jwt_public_key or jwks_url may be set".to_string(),
            ),
            (false, false) => errors
                .push("server.authz: one of jwks_url or jwt_public_key must be set".to_string()),
            _ => {}
        }

        if let Some(key) = &self.server.authz.jwt_public_key {
            if let Ok(pem) = key.value() {
                if pem.trim().is_empty() {
                    errors.push("server.authz.jwt_public_key must not be empty".to_string());
                }
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

    /// Load only the `[datastore]` section from a config file and return a bound
    /// [`udex_datastore::config::DatastoreConfig`].
    ///
    /// Unlike [`UdexConfig::load`], this does **not** bind or validate any server
    /// TLS or auth secrets, making it safe to call in pre-deploy contexts where
    /// cert files may not yet be present.
    pub fn load_datastore_config(
        path: &std::path::Path,
    ) -> Result<udex_datastore::config::DatastoreConfig> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;

        #[derive(serde::Deserialize)]
        struct Wrapper {
            datastore: DatastoreConfig,
        }

        let wrapper: Wrapper = serde_saphyr::from_str(&content)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;
        let ds = wrapper.datastore;

        let urn_str = ds.connection_url.urn().to_string();
        let mut ds_config = udex_datastore::config::DatastoreConfig {
            connection_url: Secret::new(&urn_str)
                .expect("URN from a parsed Secret is always valid"),
            max_connections: ds.max_connections,
            min_connections: ds.min_connections,
            connection_timeout: Duration::from_secs(ds.connection_timeout_secs),
            query_timeout: Duration::from_secs(ds.query_timeout_secs),
            dangerous_allow_non_tls: ds.dangerous_allow_non_tls,
            apply_migrations: ds.apply_migrations,
        };

        let registry = SourceRegistry::new();
        bind_all(&mut ds_config, &registry).map_err(|errs| {
            anyhow::anyhow!(
                "failed to bind datastore secrets: {}",
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        ds_config
            .validate()
            .with_context(|| format!("datastore config is invalid: {}", path.display()))?;

        Ok(ds_config)
    }

    /// Convert into the server's [`udex_server::config::ServerConfig`] and the
    /// datastore's [`udex_datastore::config::DatastoreConfig`].
    ///
    /// Consumes `self` so that bound [`Secret<String>`] values in `tls` and
    /// `authz` are moved directly into the server config without cloning.
    /// Call [`UdexConfig::validate`] before this method.
    pub fn into_configs(
        self,
    ) -> Result<(
        udex_server::config::ServerConfig,
        udex_datastore::config::DatastoreConfig,
    )> {
        let Self { server, datastore } = self;

        let bind_address = server
            .bind_address
            .parse()
            .with_context(|| format!("invalid bind_address: {}", server.bind_address))?;

        let server_config = udex_server::config::ServerConfig {
            bind_address,
            request_timeout: Duration::from_secs(server.request_timeout_secs),
            max_connections: server.max_connections,
            max_message_size: server.max_message_size_bytes,
            tls: server.tls,
            authz: server.authz,
            init_indexes: vec![],
            observability: server.observability,
        };

        // URN copy pattern: reconstruct a fresh unbound Secret from the URN
        // string and bind it against a new registry. This is the formalized
        // pattern from secrets-rs 1.0.0 for sharing a secret across owners.
        let urn_str = datastore.connection_url.urn().to_string();
        let mut datastore_config = udex_datastore::config::DatastoreConfig {
            connection_url: Secret::new(&urn_str)
                .expect("URN from a parsed Secret is always valid"),
            max_connections: datastore.max_connections,
            min_connections: datastore.min_connections,
            connection_timeout: Duration::from_secs(datastore.connection_timeout_secs),
            query_timeout: Duration::from_secs(datastore.query_timeout_secs),
            dangerous_allow_non_tls: datastore.dangerous_allow_non_tls,
            apply_migrations: datastore.apply_migrations,
        };

        // EnvSource is pre-registered by SourceRegistry::new() in secrets-rs 1.0.0.
        let registry = SourceRegistry::new();
        bind_all(&mut datastore_config, &registry).map_err(|errs| {
            anyhow::anyhow!(
                "failed to bind datastore secrets: {}",
                errs.iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        })?;

        Ok((server_config, datastore_config))
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
        // Replace the default EnvSource with an in-memory source for isolation.
        registry
            .register("env", MapSource::with("DATABASE_URL", url))
            .unwrap();
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
        cfg.server.authz.jwt_public_key = None;
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
        cfg.server.authz.jwt_public_key = None;
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
        // round-tripping through serde_saphyr::from_str — config files must always be
        // hand-authored with bare URN strings such as "urn:secrets-rs:env:DATABASE_URL".
        let cfg = UdexConfig::default();
        let yaml_str = serde_saphyr::to_string(&cfg).expect("serialization should succeed");
        assert!(yaml_str.contains("bind_address"));
        assert!(yaml_str.contains("max_connections"));
    }

    // WP-05: Verify that Secret<String> deserialization acts as a file-injection guard.
    // Secret<T>::Deserialize only accepts a bare URN — plain secret values are rejected
    // at parse time, so a misconfigured YAML file with a raw connection string cannot
    // reach the application.

    #[test]
    fn test_plain_url_rejected_by_deserializer() {
        let yaml = r#"
server:
  bind_address: "0.0.0.0:50051"
  request_timeout_secs: 30
  max_connections: 1000
  max_message_size_bytes: 4194304
  tls:
    cert: "urn:secrets-rs:file:certs/server.crt"
    key: "urn:secrets-rs:file:certs/server.key"
  authz:
    jwt_public_key: "urn:secrets-rs:file:certs/jwt_public_key.pem"
    jwt_issuer: "https://auth.example.com"
    jwt_audience: "udex"
datastore:
  connection_url: "postgres://user:password@localhost:5432/db"
  max_connections: 10
  min_connections: 1
  connection_timeout_secs: 10
  query_timeout_secs: 30
"#;
        let result = serde_saphyr::from_str::<UdexConfig>(yaml);
        assert!(
            result.is_err(),
            "plain connection URL must be rejected at deserialization"
        );
    }

    #[test]
    fn test_valid_urn_accepted_by_deserializer() {
        let yaml = r#"
server:
  bind_address: "0.0.0.0:50051"
  request_timeout_secs: 30
  max_connections: 1000
  max_message_size_bytes: 4194304
  tls:
    cert: "urn:secrets-rs:file:certs/server.crt"
    key: "urn:secrets-rs:file:certs/server.key"
  authz:
    jwt_public_key: "urn:secrets-rs:file:certs/jwt_public_key.pem"
    jwt_issuer: "https://auth.example.com"
    jwt_audience: "udex"
datastore:
  connection_url: "urn:secrets-rs:env:DATABASE_URL"
  max_connections: 10
  min_connections: 1
  connection_timeout_secs: 10
  query_timeout_secs: 30
"#;
        let result = serde_saphyr::from_str::<UdexConfig>(yaml);
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

    #[test]
    fn test_observability_section_parses() {
        let yaml = r#"
server:
  bind_address: "0.0.0.0:50051"
  request_timeout_secs: 30
  max_connections: 1000
  max_message_size_bytes: 4194304
  tls:
    cert: "urn:secrets-rs:file:certs/server.crt"
    key: "urn:secrets-rs:file:certs/server.key"
  authz:
    jwt_public_key: "urn:secrets-rs:file:certs/jwt_public_key.pem"
    jwt_issuer: "https://auth.example.com"
    jwt_audience: "udex"
  observability:
    enabled: true
    otlp_endpoint: "https://otel-collector:4317"
    sample_ratio: 0.5
datastore:
  connection_url: "urn:secrets-rs:env:DATABASE_URL"
  max_connections: 10
  min_connections: 1
  connection_timeout_secs: 10
  query_timeout_secs: 30
"#;
        let cfg: UdexConfig =
            serde_saphyr::from_str(yaml).expect("config with observability must parse");
        let obs = cfg
            .server
            .observability
            .expect("observability section present");
        assert!(obs.enabled);
        assert_eq!(
            obs.otlp_endpoint.as_deref(),
            Some("https://otel-collector:4317")
        );
        assert_eq!(obs.sample_ratio, 0.5);
        // Per-signal flags default to true when omitted.
        assert!(obs.traces && obs.metrics && obs.logs);
    }

    #[test]
    fn test_observability_absent_is_none() {
        // A config without an observability section leaves it disabled (None).
        let cfg = UdexConfig::default();
        assert!(cfg.server.observability.is_none());
    }
}
