//! The open-standard telemetry configuration contract.
//!
//! This is the YAML-facing shape embedded by the server (and CLI) as their
//! `observability` section. It speaks only OpenTelemetry concepts (OTLP
//! endpoint, sampling, per-signal toggles) - never a specific backend.

use crate::TelemetryError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

fn default_true() -> bool {
    true
}

fn default_sample_ratio() -> f64 {
    1.0
}

/// Telemetry configuration. Defaults to disabled - when `enabled` is false (or no
/// `otlp_endpoint` is set) the binary runs with JSON stdout logging only and no
/// OTLP exporters.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TelemetryConfig {
    /// Master switch. When false, no OTLP exporters are created.
    #[serde(default)]
    pub enabled: bool,

    /// OTLP endpoint, e.g. `https://otel-collector:4317`. Required when enabled.
    #[serde(default)]
    pub otlp_endpoint: Option<String>,

    /// Path to a PEM CA bundle used to verify the OTLP endpoint's TLS cert. When
    /// omitted for an `https://` endpoint, the system/native roots are used.
    #[serde(default)]
    pub otlp_ca: Option<String>,

    /// Head trace sampling ratio in `0.0..=1.0` (applied to root spans;
    /// parent-based otherwise). Defaults to 1.0 (sample everything).
    #[serde(default = "default_sample_ratio")]
    pub sample_ratio: f64,

    /// Export traces over OTLP. Defaults to true (gated by `enabled`).
    #[serde(default = "default_true")]
    pub traces: bool,

    /// Export metrics over OTLP. Defaults to true (gated by `enabled`).
    #[serde(default = "default_true")]
    pub metrics: bool,

    /// Export logs over OTLP. Defaults to true (gated by `enabled`). The JSON
    /// stdout log layer is ALWAYS on regardless of this flag (hybrid design).
    #[serde(default = "default_true")]
    pub logs: bool,

    /// Extra OpenTelemetry resource attributes (e.g.
    /// `deployment.environment: local`).
    #[serde(default)]
    pub resource_attributes: BTreeMap<String, String>,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            otlp_endpoint: None,
            otlp_ca: None,
            sample_ratio: default_sample_ratio(),
            traces: true,
            metrics: true,
            logs: true,
            resource_attributes: BTreeMap::new(),
        }
    }
}

impl TelemetryConfig {
    /// Validate the telemetry configuration. A disabled config always validates.
    pub fn validate(&self) -> Result<(), TelemetryError> {
        if !self.enabled {
            return Ok(());
        }

        let endpoint = self
            .otlp_endpoint
            .as_deref()
            .map(str::trim)
            .unwrap_or_default();
        if endpoint.is_empty() {
            return Err(TelemetryError::Config(
                "observability.enabled is true but otlp_endpoint is not set".to_string(),
            ));
        }
        if !(endpoint.starts_with("http://") || endpoint.starts_with("https://")) {
            return Err(TelemetryError::Config(format!(
                "otlp_endpoint '{endpoint}' must start with http:// or https://"
            )));
        }

        if !(0.0..=1.0).contains(&self.sample_ratio) {
            return Err(TelemetryError::Config(format!(
                "sample_ratio {} must be between 0.0 and 1.0",
                self.sample_ratio
            )));
        }

        // The CA is only consulted for TLS (https) endpoints by tls_config(); a
        // plaintext (http) endpoint never reads it, so don't reject a config that
        // names a CA it will never use.
        if endpoint.starts_with("https://") {
            if let Some(ca) = &self.otlp_ca {
                // Open for reading (not just stat) so the check matches the "not
                // readable" message and init()'s std::fs::read of the CA.
                if std::fs::File::open(ca).is_err() {
                    return Err(TelemetryError::Config(format!(
                        "otlp_ca '{ca}' is not readable"
                    )));
                }
            }
        }

        if !self.traces && !self.metrics && !self.logs {
            return Err(TelemetryError::Config(
                "observability.enabled is true but all signals (traces, metrics, logs) \
                 are disabled - set enabled=false instead"
                    .to_string(),
            ));
        }

        Ok(())
    }

    /// True when telemetry should actually export (enabled with an endpoint).
    pub(crate) fn active(&self) -> bool {
        self.enabled
            && self
                .otlp_endpoint
                .as_deref()
                .map(|e| !e.trim().is_empty())
                .unwrap_or(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn enabled_cfg() -> TelemetryConfig {
        TelemetryConfig {
            enabled: true,
            otlp_endpoint: Some("https://otel-collector:4317".to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn disabled_always_valid() {
        assert!(TelemetryConfig::default().validate().is_ok());
    }

    #[test]
    fn enabled_without_endpoint_is_err() {
        let cfg = TelemetryConfig {
            enabled: true,
            otlp_endpoint: None,
            ..Default::default()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("otlp_endpoint"), "got: {err}");
    }

    #[test]
    fn enabled_bad_scheme_is_err() {
        let cfg = TelemetryConfig {
            otlp_endpoint: Some("otel-collector:4317".to_string()),
            ..enabled_cfg()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("http"), "got: {err}");
    }

    #[test]
    fn sample_ratio_out_of_range_is_err() {
        let cfg = TelemetryConfig {
            sample_ratio: 1.5,
            ..enabled_cfg()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("sample_ratio"), "got: {err}");
    }

    #[test]
    fn unreadable_ca_is_err() {
        let cfg = TelemetryConfig {
            otlp_ca: Some("/nonexistent/ca.crt".to_string()),
            ..enabled_cfg()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("otlp_ca"), "got: {err}");
    }

    #[test]
    fn unreadable_ca_ignored_for_plaintext_endpoint() {
        // tls_config() never reads the CA for an http:// endpoint, so an
        // unreadable CA must not fail validation in that case.
        let cfg = TelemetryConfig {
            otlp_endpoint: Some("http://otel-collector:4317".to_string()),
            otlp_ca: Some("/nonexistent/ca.crt".to_string()),
            ..enabled_cfg()
        };
        assert!(cfg.validate().is_ok());
    }

    #[test]
    fn all_signals_disabled_is_err() {
        let cfg = TelemetryConfig {
            traces: false,
            metrics: false,
            logs: false,
            ..enabled_cfg()
        };
        let err = cfg.validate().unwrap_err().to_string();
        assert!(err.contains("all signals"), "got: {err}");
    }

    #[test]
    fn valid_enabled_ok() {
        assert!(enabled_cfg().validate().is_ok());
    }

    #[test]
    fn active_requires_enabled_and_endpoint() {
        assert!(!TelemetryConfig::default().active());
        assert!(enabled_cfg().active());
        let no_ep = TelemetryConfig {
            enabled: true,
            otlp_endpoint: None,
            ..Default::default()
        };
        assert!(!no_ep.active());
    }
}
