//! Verifies that telemetry init installs a global tracing subscriber (the JSON
//! stdout floor) even when OTLP observability is disabled, and that repeated
//! initialisation is a no-op rather than a panic.
//!
//! These tests live in their own file so they compile into a separate test
//! binary, giving them isolated global subscriber state - tests in other
//! binaries cannot interfere with `tracing::dispatcher::has_been_set()` here.
//!
//! The "not initialised" check lives in logging_not_initialised_test.rs so it
//! gets its own binary with no competing tests that install a subscriber.
use udex_telemetry::{init, ServiceIdentity, TelemetryConfig};

fn test_identity() -> ServiceIdentity {
    ServiceIdentity {
        name: "udex-server-test".to_string(),
        version: "0.0.0".to_string(),
        instance_id: "test".to_string(),
    }
}

#[test]
fn test_telemetry_init_installs_global_subscriber() {
    // A disabled telemetry config must still install the JSON stdout floor.
    let _guard = init(&TelemetryConfig::default(), test_identity())
        .expect("disabled telemetry init must succeed");
    assert!(
        tracing::dispatcher::has_been_set(),
        "telemetry init must install a global tracing subscriber (stdout floor)"
    );
}

#[test]
fn test_telemetry_init_is_idempotent() {
    // Calling init multiple times must never panic; the first call sets the
    // subscriber, subsequent calls are no-ops.
    let _g1 = init(&TelemetryConfig::default(), test_identity()).expect("init");
    let _g2 = init(&TelemetryConfig::default(), test_identity()).expect("init");
    let _g3 = init(&TelemetryConfig::default(), test_identity()).expect("init");
}
