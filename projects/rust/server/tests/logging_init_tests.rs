/// Integration tests verifying that init_tracing() installs a global tracing subscriber.
///
/// These tests live in their own file so they compile into a separate test binary,
/// giving them an isolated global subscriber state — tests in other binaries
/// cannot interfere with `tracing::dispatcher::has_been_set()` here.
///
/// The "not initialised" check lives in logging_not_initialised_test.rs so it
/// gets its own binary with no competing tests that install a subscriber.
use udex_server::logging;

#[test]
fn test_init_tracing_installs_global_subscriber() {
    logging::init_tracing();
    assert!(
        tracing::dispatcher::has_been_set(),
        "init_tracing() must install a global tracing subscriber"
    );
}

#[test]
fn test_init_tracing_is_idempotent() {
    // Calling init_tracing() multiple times must never panic.
    // The first call sets the subscriber; subsequent calls are no-ops.
    logging::init_tracing();
    logging::init_tracing();
    logging::init_tracing();
}
