/// Verifies that no global tracing subscriber is set in a fresh process.
///
/// This test MUST be the only test in this binary — it relies on no other
/// test having initialised telemetry or installed a subscriber first.
/// Keeping it isolated in its own file guarantees a fresh binary with no
/// competing tests.
#[test]
fn test_logging_not_initialised_before_telemetry_init() {
    assert!(
        !tracing::dispatcher::has_been_set(),
        "No global subscriber should be set before telemetry init is called"
    );
}
