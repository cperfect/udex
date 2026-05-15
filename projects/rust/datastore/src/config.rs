use secrets_rs::Secret;
use serde::{Deserialize, Serialize};
use std::time::Duration;

/// Datastore configuration.
///
/// `connection_url` is a [`Secret<String>`] — the value is masked in Debug/Display
/// output and must be explicitly accessed via [`.value()`](Secret::value).
/// The caller is responsible for binding it (via [`secrets_rs::bind_all`]) before
/// passing this config to [`DatastoreConfig::validate`] or
/// [`DatastoreConfig::resolved_connection_url`].
#[derive(Debug, Serialize, Deserialize, secrets_rs::Bindable)]
pub struct DatastoreConfig {
    /// PostgreSQL connection URL, resolved from the secret source at startup.
    pub connection_url: Secret<String>,
    /// Maximum number of connections in the pool
    pub max_connections: u32,
    /// Minimum number of connections in the pool
    pub min_connections: u32,
    /// Connection timeout duration
    pub connection_timeout: Duration,
    /// Query timeout duration
    pub query_timeout: Duration,
    /// Disable the TLS enforcement check on the connection URL.
    ///
    /// **DANGER**: Setting this to `true` allows plaintext database connections.
    /// Only use this in local development or test environments. Production
    /// deployments MUST leave this `false` and include `sslmode=require` (or
    /// stronger) in the connection URL.
    #[serde(default)]
    pub dangerous_allow_non_tls: bool,
    /// Allow the server to apply outstanding database migrations on startup.
    ///
    /// Defaults to `false`. When `false` the server will refuse to start if the
    /// database schema is not already at the expected version. Set to `true` only
    /// in environments where the server process is authorised to mutate the schema
    /// (e.g. development, CI). In production, prefer running `udex migrate apply`
    /// as a dedicated pre-deploy step.
    #[serde(default)]
    pub apply_migrations: bool,
}

/// Returns `true` if the URL contains a TLS-enforcing `sslmode` value
/// (`require`, `verify-ca`, or `verify-full`).
fn url_enforces_tls(url: &str) -> bool {
    let query = url.split_once('?').map(|(_, q)| q).unwrap_or("");
    for param in query.split('&') {
        if let Some(value) = param.strip_prefix("sslmode=") {
            return matches!(value, "require" | "verify-ca" | "verify-full");
        }
    }
    false
}

impl DatastoreConfig {
    /// Validate the datastore configuration.
    ///
    /// Must be called after the secret has been bound; returns an error if
    /// `connection_url` is unbound.
    pub fn validate(&self) -> Result<(), DatastoreConfigError> {
        let url = self
            .connection_url
            .value()
            .map_err(|e| DatastoreConfigError::Validation(format!("connection_url: {e}")))?;

        if url.is_empty() {
            return Err(DatastoreConfigError::Validation(
                "connection_url cannot be empty".to_string(),
            ));
        }

        if self.max_connections == 0 {
            return Err(DatastoreConfigError::Validation(
                "max_connections must be greater than 0".to_string(),
            ));
        }

        if self.min_connections > self.max_connections {
            return Err(DatastoreConfigError::Validation(
                "min_connections cannot be greater than max_connections".to_string(),
            ));
        }

        if !self.dangerous_allow_non_tls && !url_enforces_tls(url) {
            return Err(DatastoreConfigError::NonTlsConnection);
        }

        Ok(())
    }

    /// Returns the bound connection URL after running all validation rules.
    ///
    /// Calls [`Self::validate`] before returning the URL so callers cannot
    /// accidentally bypass TLS enforcement or other invariants (e.g. by
    /// skipping `validate()` and calling this directly).
    ///
    /// Must be called after the secret has been bound via [`secrets_rs::bind_all`].
    pub fn resolved_connection_url(&self) -> Result<String, DatastoreConfigError> {
        self.validate()?;
        self.connection_url
            .value()
            .map(|s| s.to_string())
            .map_err(|e| DatastoreConfigError::Validation(format!("connection_url: {e}")))
    }
}

/// Datastore configuration error types.
#[derive(Debug, thiserror::Error)]
pub enum DatastoreConfigError {
    /// Validation error
    #[error("Datastore config validation failed: {0}")]
    Validation(String),
    /// Non-TLS connection URL without the override flag set
    #[error(
        "connection_url does not enforce TLS (sslmode=require/verify-ca/verify-full required); \
         set dangerous_allow_non_tls=true to override in non-production environments"
    )]
    NonTlsConnection,
}

#[cfg(test)]
mod tests {
    use super::*;
    use secrets_rs::{SourceError, SourceRegistry};

    /// In-memory secret source for tests — no env var access, no race conditions.
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

    fn bound_config(url: &str) -> DatastoreConfig {
        let mut cfg = DatastoreConfig {
            connection_url: Secret::new("urn:secrets-rs:test:url").unwrap(),
            max_connections: 10,
            min_connections: 1,
            connection_timeout: Duration::from_secs(10),
            query_timeout: Duration::from_secs(30),
            dangerous_allow_non_tls: false,
            apply_migrations: false,
        };
        let mut registry = SourceRegistry::new();
        registry
            .register("test", MapSource::with("url", url))
            .unwrap();
        cfg.connection_url.bind(&registry).unwrap();
        cfg
    }

    #[test]
    fn validate_empty_url_is_err() {
        assert!(bound_config("").validate().is_err());
    }

    #[test]
    fn validate_zero_max_connections_is_err() {
        let mut cfg = bound_config("postgres://host:5432/db?sslmode=require");
        cfg.max_connections = 0;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_min_exceeds_max_is_err() {
        let mut cfg = bound_config("postgres://host:5432/db?sslmode=require");
        cfg.max_connections = 5;
        cfg.min_connections = 10;
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn validate_valid_config_ok() {
        assert!(bound_config("postgres://host:5432/db?sslmode=require")
            .validate()
            .is_ok());
    }

    #[test]
    fn validate_unbound_secret_is_err() {
        let cfg = DatastoreConfig {
            connection_url: Secret::new("urn:secrets-rs:env:DATABASE_URL").unwrap(),
            max_connections: 10,
            min_connections: 1,
            connection_timeout: Duration::from_secs(10),
            query_timeout: Duration::from_secs(30),
            dangerous_allow_non_tls: false,
            apply_migrations: false,
        };
        assert!(cfg.validate().is_err());
    }

    #[test]
    fn tls_enforcement_no_sslmode() {
        assert!(bound_config("postgres://host:5432/db").validate().is_err());
    }

    #[test]
    fn tls_enforcement_sslmode_disable() {
        assert!(bound_config("postgres://host:5432/db?sslmode=disable")
            .validate()
            .is_err());
    }

    #[test]
    fn tls_enforcement_sslmode_prefer() {
        assert!(bound_config("postgres://host:5432/db?sslmode=prefer")
            .validate()
            .is_err());
    }

    #[test]
    fn tls_enforcement_sslmode_require() {
        assert!(bound_config("postgres://host:5432/db?sslmode=require")
            .validate()
            .is_ok());
    }

    #[test]
    fn tls_enforcement_sslmode_verify_ca() {
        assert!(bound_config("postgres://host:5432/db?sslmode=verify-ca")
            .validate()
            .is_ok());
    }

    #[test]
    fn tls_enforcement_sslmode_verify_full() {
        assert!(bound_config("postgres://host:5432/db?sslmode=verify-full")
            .validate()
            .is_ok());
    }

    #[test]
    fn tls_enforcement_dangerous_allow_override() {
        let mut cfg = bound_config("postgres://host:5432/db");
        cfg.dangerous_allow_non_tls = true;
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn apply_migrations_defaults_to_false_when_absent() {
        let json = r#"{
            "connection_url": "urn:secrets-rs:env:DATABASE_URL",
            "max_connections": 10,
            "min_connections": 1,
            "connection_timeout": { "secs": 10, "nanos": 0 },
            "query_timeout": { "secs": 30, "nanos": 0 }
        }"#;
        let cfg: DatastoreConfig = serde_json::from_str(json).unwrap();
        assert!(!cfg.apply_migrations);
    }

    #[test]
    fn apply_migrations_true_round_trips() {
        let json = r#"{
            "connection_url": "urn:secrets-rs:env:DATABASE_URL",
            "max_connections": 10,
            "min_connections": 1,
            "connection_timeout": { "secs": 10, "nanos": 0 },
            "query_timeout": { "secs": 30, "nanos": 0 },
            "apply_migrations": true
        }"#;
        let cfg: DatastoreConfig = serde_json::from_str(json).unwrap();
        assert!(cfg.apply_migrations);
    }
}
