use tracing_subscriber::{fmt, EnvFilter};

/// Initializes the global tracing subscriber with JSON output.
///
/// Log level is read from `RUST_LOG` env var, defaulting to `info`.
/// Set `RUST_LOG=trace` for local development to get full trace output.
pub fn init_tracing() {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    fmt()
        .json()
        .with_env_filter(env_filter)
        .init();
}
