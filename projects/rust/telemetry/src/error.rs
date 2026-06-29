//! Telemetry error type. Never exposes raw opentelemetry/exporter error types.

use thiserror::Error;

/// Errors raised while validating telemetry config or initialising exporters.
#[derive(Debug, Error)]
pub enum TelemetryError {
    /// The telemetry configuration is invalid.
    #[error("invalid telemetry configuration: {0}")]
    Config(String),

    /// The configured OTLP CA certificate could not be read.
    #[error("failed to read OTLP CA certificate '{path}': {source}")]
    CaRead {
        /// Path that could not be read.
        path: String,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },

    /// An OTLP exporter could not be built (endpoint/TLS/transport problem).
    #[error("failed to build OTLP {signal} exporter: {detail}")]
    Exporter {
        /// Which signal's exporter failed (`traces`, `metrics`, `logs`).
        signal: &'static str,
        /// Human-readable detail (wrapped; not a third-party error type).
        detail: String,
    },

    /// The global tracing subscriber could not be installed, so the OTLP layers
    /// were not attached and telemetry would silently not export.
    #[error("failed to install the telemetry subscriber (one is already set): {0}")]
    Subscriber(String),
}
