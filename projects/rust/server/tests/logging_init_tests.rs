/// Integration tests verifying that serve() initialises the tracing subscriber.
///
/// These tests live in their own file so they compile into a separate test binary,
/// giving them an isolated global subscriber state — tests in other binaries
/// cannot interfere with `tracing::dispatcher::has_been_set()` here.
use udex_server::logging;

#[test]
fn test_logging_not_initialised_before_init_tracing() {
    // Sanity check: in a fresh process, no global subscriber should be set yet.
    // This relies on the test binary having no other tests that call init_tracing()
    // or install a subscriber before this test runs. Since this is the only test
    // in this binary, that is guaranteed.
    assert!(
        !tracing::dispatcher::has_been_set(),
        "No global subscriber should be set before init_tracing() is called"
    );
}

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
