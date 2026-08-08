//! Shared scaffolding for the `udex-sdk` integration test binaries
//! (`integration_tests.rs` and `obs.rs`).
//!
//! This lives in a `tests/common/` subdirectory so Cargo compiles it as a module
//! of each test binary (via `mod common;`) rather than as its own test binary.
//! Some items are used by only one of the binaries, hence the crate-level
//! `allow(dead_code)` — per-binary, the unused ones would otherwise warn.

#![allow(dead_code)]

use jsonwebtoken::{encode, EncodingKey, Header};
use time::OffsetDateTime;
use tokio::time::{sleep, Duration};
use tonic::transport::{Certificate, Channel, ClientTlsConfig};
use tonic_health::pb::{health_client::HealthClient, HealthCheckRequest};
use udex_sdk::{ContextInput, KeyValuePair, Value};

// ── Cert / JWT key paths ──────────────────────────────────────────────────────
// CARGO_MANIFEST_DIR points to the sdk/ package root at compile time.
// The server's test fixtures live one level up under server/tests/.

const CERTS_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/certs");
const JWT_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../server/tests/jwt");

pub fn server_cert_path(file: &str) -> String {
    format!("{CERTS_DIR}/{file}")
}

pub fn jwt_key_path(file: &str) -> String {
    format!("{JWT_DIR}/{file}")
}

// ── Server readiness ──────────────────────────────────────────────────────────

/// Poll the healthz endpoint over TLS until the server responds or 3 seconds elapse.
pub async fn wait_for_server(addr: &str, ca_pem: &[u8]) {
    let tls = ClientTlsConfig::new()
        .ca_certificate(Certificate::from_pem(ca_pem))
        .domain_name("localhost");
    for _ in 0..30 {
        sleep(Duration::from_millis(100)).await;
        let Ok(ch) = Channel::from_shared(format!("https://{addr}"))
            .unwrap()
            .tls_config(tls.clone())
            .unwrap()
            .connect()
            .await
        else {
            continue;
        };
        if HealthClient::new(ch)
            .check(HealthCheckRequest {
                service: "".to_string(),
            })
            .await
            .is_ok()
        {
            return;
        }
    }
    panic!("server at {addr} did not become ready within 3 seconds");
}

// ── JWT / request helpers ─────────────────────────────────────────────────────

/// Signs a short-lived JWT with the given signing key.
pub fn make_token(
    signing_key: &EncodingKey,
    issuer: &str,
    audience: &str,
    index_name: &str,
    scope_override: Option<&str>,
) -> String {
    let now = OffsetDateTime::now_utc().unix_timestamp() as usize;
    let default_scope;
    let scope = if let Some(s) = scope_override {
        s
    } else {
        default_scope = format!(
            "udex:index:v1:list \
             udex:index:v1:create \
             udex:index:v1:{index_name}:read \
             udex:index:v1:*:delete \
             udex:entry:v1:{index_name}:create \
             udex:entry:v1:{index_name}:read \
             udex:entry:v1:{index_name}:write \
             udex:entry:v1:{index_name}:delete"
        );
        &default_scope
    };
    let claims = udex_api::authz::claims::Claims::new(
        "sdk-test-subject".to_string(),
        issuer.to_string(),
        audience.to_string(),
        now + 3600,
        now,
    )
    .with_scope(scope.to_string());
    encode(
        &Header::new(jsonwebtoken::Algorithm::ES256),
        &claims,
        signing_key,
    )
    .expect("JWT encode")
}

pub fn context_input(pairs: &[(&str, &str)]) -> ContextInput {
    ContextInput {
        pairs: pairs
            .iter()
            .map(|(k, v)| KeyValuePair {
                key: k.to_string(),
                value: Some(Value {
                    value: Some(udex_sdk::value::Value::StringValue(v.to_string())),
                }),
                kek_id: None,
                dek: None,
            })
            .collect(),
    }
}

pub fn now_unix_nanos() -> u128 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos()
}

// ── OpenObserve query helpers (observability fixture) ─────────────────────────
//
// The observability tests assert that telemetry lands in OpenObserve — the
// unified store the collector exports to (ST0028). It is an always-on fixture
// like Hydra: these helpers FAIL LOUDLY if it is unreachable, they never skip.
// Reachable via the `openobserve` service name in-network; override with
// OPENOBSERVE_URL for other environments (CI uses the runner's localhost).

/// OpenObserve organisation. The fixture never creates a second one.
const OPENOBSERVE_ORG: &str = "default";

/// How far back a search window reaches. OpenObserve requires an explicit window
/// and returns *nothing* — with no error — when it does not bracket the data, so
/// this is deliberately generous. Assertions are made run-specific by resource
/// attribute (`udex.test.run`) and by baseline-then-poll, never by this window.
const SEARCH_LOOKBACK_SECS: u64 = 3600;

fn openobserve_url() -> String {
    std::env::var("OPENOBSERVE_URL").unwrap_or_else(|_| "http://openobserve:5080".to_string())
}

/// Basic-auth credentials for the search API — the same pair the collector uses
/// to ingest, generated into `.env` by `scripts/gen-env.sh`.
///
/// `.env` is loaded here rather than relying on another fixture having done it
/// first: several helpers call `dotenvy` incidentally, and depending on which
/// one happens to run earlier is exactly the kind of ordering coupling that
/// breaks when a test is run on its own.
fn openobserve_credentials() -> (String, String) {
    // Read the process environment BEFORE touching dotenv. CI sets these
    // directly and has no .env; a developer has a .env and no exported vars.
    // `dotenv_override` would let a stale .env win over an explicitly exported
    // value, which is the wrong precedence — the more specific source should.
    let mut email = std::env::var("OPENOBSERVE_ROOT_EMAIL").ok();
    let mut password = std::env::var("OPENOBSERVE_ROOT_PASSWORD_SECRET").ok();

    if password.is_none() {
        // Only fall back to .env when the process environment did not supply it.
        // Loaded here rather than relying on another fixture having done it
        // first: several helpers call dotenvy incidentally, and depending on
        // which one happens to run earlier is exactly the kind of ordering
        // coupling that breaks when a test is run on its own.
        static LOAD_ENV: std::sync::Once = std::sync::Once::new();
        LOAD_ENV.call_once(|| {
            dotenvy::dotenv().ok();
        });
        email = email.or_else(|| std::env::var("OPENOBSERVE_ROOT_EMAIL").ok());
        password = std::env::var("OPENOBSERVE_ROOT_PASSWORD_SECRET").ok();
    }

    let password = password.expect(
        "OPENOBSERVE_ROOT_PASSWORD_SECRET is not set — run scripts/gen-env.sh, or export it \
         directly in CI. The observability fixture is an always-on dev/CI dependency, like Hydra.",
    );
    (
        email.unwrap_or_else(|| "root@udex.local".to_string()),
        password,
    )
}

/// Run a SQL query against an OpenObserve stream type (`logs` | `traces` |
/// `metrics`) and return the `hits` array. Panics on any API error.
///
/// `stream_type` is not optional decoration: the stream name `default` exists
/// independently under both `logs` and `traces`, so getting it wrong silently
/// searches the wrong signal.
pub async fn openobserve_search(sql: &str, stream_type: &str) -> Vec<serde_json::Value> {
    openobserve_try_search(sql, stream_type)
        .await
        .unwrap_or_else(|reason| panic!("{reason}"))
}

/// True when an API error means "this data has not been ingested yet" rather
/// than "this query is wrong".
///
/// Both shapes appear on a cold fixture: a stream does not exist until its first
/// datapoint arrives (`Search stream not found`), and a column does not exist
/// until a datapoint carrying it arrives (`unknown field`). Neither is
/// distinguishable from a genuine typo by the response alone, which is why only
/// the metrics helpers below are allowed to treat it as transient.
fn is_schema_not_ready(reason: &str) -> bool {
    reason.contains("unknown field") || reason.contains("stream not found")
}

/// Fallible core of [`openobserve_search`]. `Err` carries the fully formatted
/// failure reason, ready to panic with.
pub async fn openobserve_try_search(
    sql: &str,
    stream_type: &str,
) -> Result<Vec<serde_json::Value>, String> {
    let (email, password) = openobserve_credentials();
    let now_micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_micros() as i64;

    let body = serde_json::json!({
        "query": {
            "sql": sql,
            "start_time": now_micros - (SEARCH_LOOKBACK_SECS as i64 * 1_000_000),
            // A little ahead of now, so a record ingested during this call cannot
            // fall outside the window.
            "end_time": now_micros + 60_000_000,
            "from": 0,
            // Fixed page cap. Every caller either selects a single aggregate or
            // reads the span names of one trace, so 100 rows is ample — but it IS
            // a cap: a query returning more would be silently truncated. Any
            // future caller needing a larger result set should parameterise this
            // rather than assume completeness.
            "size": 100,
        }
    });

    let response = reqwest::Client::new()
        .post(format!(
            "{}/api/{OPENOBSERVE_ORG}/_search?type={stream_type}",
            openobserve_url()
        ))
        .basic_auth(email, Some(password))
        .json(&body)
        .timeout(Duration::from_secs(10))
        .send()
        .await
        .expect(
            "OpenObserve query failed — is the observability fixture reachable? \
             It is an always-on dev/CI dependency, like Hydra.",
        );

    // Read the body BEFORE reacting to the status. `error_for_status` alone would
    // throw away the part that actually helps: OpenObserve puts the reason in
    // `message` and often a repair suggestion in `hint`, and losing those leaves
    // a bare "400 Bad Request" to debug.
    let status = response.status();
    let raw = response
        .text()
        .await
        .expect("could not read the OpenObserve response body");
    let payload: serde_json::Value = serde_json::from_str(&raw).unwrap_or(serde_json::Value::Null);

    // A rejected query must FAIL LOUDLY rather than read as "no telemetry".
    //
    // A malformed query is answered with HTTP 400 and a `message`; some
    // conditions instead answer 200 with `message` set and `hits` null. Both are
    // treated as failure here, because the alternative is that a mistyped column
    // — easy to get wrong, since resource attributes are prefixed `service_` in
    // the traces stream but bare in logs and metrics — looks exactly like
    // telemetry that never arrived, and surfaces as a poll-budget timeout
    // blaming the pipeline (IN-AG-NO-SILENT-001).
    let message = payload.get("message").and_then(|m| m.as_str());
    if !status.is_success() || message.is_some() {
        let reason = message.unwrap_or(if raw.is_empty() {
            "<empty response body>"
        } else {
            &raw
        });
        let hint = payload
            .get("hint")
            .and_then(|h| h.as_str())
            .map(|h| format!("\n  hint: {h}"))
            .unwrap_or_default();
        return Err(format!(
            "OpenObserve rejected the query ({status}): {reason}{hint}\n  \
             stream_type: {stream_type}\n  SQL: {sql}"
        ));
    }

    Ok(payload
        .get("hits")
        .and_then(|h| h.as_array())
        .cloned()
        .unwrap_or_default())
}

/// Polls for a trace one of whose spans carries the entry `key` attribute,
/// returning the sorted, de-duplicated span names of that trace (empty if none
/// appear within the budget). Used for run-specific trace assertions.
///
/// `key` needs SQL quoting: it is a bare flattened column here, not a map lookup
/// as it was under ClickHouse's `SpanAttributes['key']`.
pub async fn openobserve_trace_span_names(key: &str) -> Vec<String> {
    let sql = format!(
        "SELECT DISTINCT operation_name FROM \"default\" \
         WHERE trace_id IN (SELECT trace_id FROM \"default\" WHERE \"key\" = '{key}') \
         ORDER BY operation_name"
    );
    let mut pending = PendingReason::default();
    for _ in 0..30u32 {
        match openobserve_try_search(&sql, "traces").await {
            Ok(hits) => {
                pending.record_success();
                if !hits.is_empty() {
                    return hits
                        .iter()
                        .filter_map(|h| h.get("operation_name").and_then(|v| v.as_str()))
                        .map(|s| s.to_string())
                        .collect();
                }
            }
            Err(reason) if is_schema_not_ready(&reason) => pending.record(reason),
            Err(reason) => panic!("{reason}"),
        }
        sleep(Duration::from_secs(2)).await;
    }
    pending.panic_if_persistent("traces for this entry key never arrived");
    Vec::new()
}

/// Polls a single-column aggregate, returning the first value > 0 (or 0 after
/// the budget expires). Tolerates a cold fixture while polling — see
/// [`openobserve_pending_scalar_f64`] and [`PendingReason`].
pub async fn openobserve_pending_count(sql: &str, stream_type: &str) -> u64 {
    openobserve_await(sql, stream_type, 0.0, 45)
        .await
        .unwrap_or(0.0) as u64
}

/// Baseline-then-poll: waits for a single-column aggregate to rise above
/// `baseline`, returning the value that did so.
///
/// This is the one owner of the "capture a baseline, drive traffic, poll for an
/// increase" shape that every run-scoped observability assertion needs. It was
/// four hand-rolled copies across two test binaries before ST0028 WP04, and they
/// did not agree on cold-start handling — which is exactly how the bug this
/// replaces got in.
pub async fn openobserve_await(
    sql: &str,
    stream_type: &str,
    baseline: f64,
    attempts: u32,
) -> Option<f64> {
    let mut pending = PendingReason::default();
    for _ in 0..attempts {
        match openobserve_try_search(sql, stream_type).await {
            Ok(hits) => {
                pending.record_success();
                if let Some(v) = scalar_from_hits(&hits) {
                    if v > baseline {
                        return Some(v);
                    }
                }
            }
            Err(reason) if is_schema_not_ready(&reason) => pending.record(reason),
            Err(reason) => panic!("{reason}"),
        }
        sleep(Duration::from_secs(2)).await;
    }
    pending.panic_if_persistent("the value never rose above the baseline");
    None
}

/// Remembers why a poll kept coming back empty, so a cold start and a wrong
/// query can be told apart *at the end* even though they look identical at each
/// individual attempt.
///
/// While polling, a not-yet-ingested response is tolerated. If the budget runs
/// out with the **most recent** attempt still rejected that way, this fails
/// rather than letting the caller report a vague "telemetry did not arrive" —
/// and it distinguishes two cases that deserve different diagnoses:
///
/// - **Nothing ever resolved.** The stream or column almost certainly does not
///   exist; the name is probably wrong.
/// - **Something resolved earlier, then stopped.** Weaker evidence — the schema
///   was readable at least once, so a name is less likely to be the culprit.
///
/// A trailing success clears the memory entirely, so a genuine "ingested but did
/// not increase" still reports as the caller intends. This is what keeps
/// `IN-AG-NO-SILENT-001` intact while still tolerating a cold fixture.
///
/// The distinction exists because the message is read by someone debugging a
/// failure: claiming every attempt was rejected when some succeeded sends them
/// looking for a typo that is not there.
#[derive(Default)]
struct PendingReason {
    /// Reason from the latest not-yet-ingested response — `None` when the most
    /// recent attempt resolved.
    last: Option<String>,
    /// Whether any attempt in this poll ever resolved against the schema.
    resolved_at_least_once: bool,
}

impl PendingReason {
    fn record(&mut self, reason: String) {
        self.last = Some(reason);
    }

    /// The query resolved against the schema. Clears the pending reason and
    /// remembers that the names were valid at least once.
    fn record_success(&mut self) {
        self.last = None;
        self.resolved_at_least_once = true;
    }

    fn panic_if_persistent(&self, context: &str) {
        if let Some(reason) = &self.last {
            let diagnosis = if self.resolved_at_least_once {
                "an earlier attempt resolved against the schema, so the names were valid at \
                 least once — but the final attempts came back not-yet-ingested"
            } else {
                "no attempt ever resolved against the schema, so the stream or column name is \
                 probably wrong"
            };
            panic!("{context}, and {diagnosis}:\n{reason}");
        }
    }
}

// ── PendingReason behaviour ───────────────────────────────────────────────────
// The whole point of this type is producing an accurate diagnosis, so the two
// branches are pinned rather than trusted.
//
// These live in `common/`, which is compiled into BOTH test binaries, so they run
// twice per suite — once under `integration_tests` and once under `obs`. That is
// intentional and costs nothing: they are pure and sub-millisecond, and moving
// them to a single binary would separate them from the code they describe.

#[test]
fn pending_reason_stays_silent_when_the_last_attempt_resolved() {
    let mut p = PendingReason::default();
    p.record("unknown field 'x'".to_string());
    p.record_success();
    // Must not panic: a trailing success means the caller's own assertion is the
    // right thing to report.
    p.panic_if_persistent("ctx");
}

#[test]
fn pending_reason_blames_the_name_when_nothing_ever_resolved() {
    let mut p = PendingReason::default();
    p.record("unknown field 'udex_test_run'".to_string());
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        p.panic_if_persistent("ctx")
    }))
    .expect_err("a persistently unresolved query must panic");
    let msg = panic_message(&*err);
    assert!(
        msg.contains("no attempt ever resolved"),
        "expected the strong diagnosis; got: {msg}"
    );
    assert!(
        msg.contains("udex_test_run"),
        "reason must be carried: {msg}"
    );
}

#[test]
fn pending_reason_softens_the_claim_when_something_resolved_earlier() {
    let mut p = PendingReason::default();
    p.record_success();
    p.record("unknown field 'x'".to_string());
    let err = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        p.panic_if_persistent("ctx")
    }))
    .expect_err("a trailing not-yet-ingested response must still panic");
    let msg = panic_message(&*err);
    assert!(
        msg.contains("an earlier attempt resolved"),
        "expected the softened diagnosis; got: {msg}"
    );
    assert!(
        !msg.contains("no attempt ever resolved"),
        "must not claim nothing resolved when something did: {msg}"
    );
}

fn panic_message(err: &(dyn std::any::Any + Send)) -> String {
    err.downcast_ref::<String>()
        .cloned()
        .or_else(|| err.downcast_ref::<&str>().map(|s| s.to_string()))
        .unwrap_or_else(|| "<non-string panic payload>".to_string())
}

/// Single-shot scalar query (None when the result set is empty or the value is
/// null, e.g. a metric series that does not exist yet).
///
/// The SQL must select exactly one column; the alias is irrelevant, the sole
/// value of the first row is taken. A *malformed* query does not land here — it
/// panics inside `openobserve_search`, which is the point.
pub async fn openobserve_scalar_f64(sql: &str, stream_type: &str) -> Option<f64> {
    let hits = openobserve_search(sql, stream_type).await;
    scalar_from_hits(&hits)
}

/// Variant of [`openobserve_scalar_f64`] that tolerates a **cold fixture**:
/// telemetry that has not been ingested yet, rather than a wrong query.
///
/// OpenObserve derives a stream's schema from the data it has ingested, so
/// neither the stream nor a column exists until a matching datapoint arrives.
/// Querying too early yields `Search stream not found` or `unknown field '...'`
/// — a cold start, not a mistake. Returning `None` lets the caller keep polling.
///
/// Two situations genuinely need this, both of them races against ingestion
/// rather than logic errors:
///
/// - **Metrics**, where the OTel export interval (60s by default) means a
///   freshly started exporter has produced nothing yet, and a scraped receiver
///   stream like `postgresql_backends` does not exist for the first few seconds.
/// - **The container log floor** on a fresh fixture, where the `default` logs
///   stream does not exist until Vector has shipped its first record.
///
/// The tolerance is **opt-in at the call site, never the default**. A mistyped
/// column still fails instantly through [`openobserve_scalar_f64`], which is
/// what the run-scoped app-telemetry assertions use. The cost here is that a
/// typo fails when the poll budget expires rather than immediately, so callers
/// MUST word their assertion to say that a value which never appears may be
/// misspelled and not merely absent. Every other API error — bad syntax, unknown
/// function, auth, transport — still panics at once.
pub async fn openobserve_pending_scalar_f64(sql: &str, stream_type: &str) -> Option<f64> {
    match openobserve_try_search(sql, stream_type).await {
        Ok(hits) => scalar_from_hits(&hits),
        Err(reason) if is_schema_not_ready(&reason) => None,
        Err(reason) => panic!("{reason}"),
    }
}

/// Pulls the sole value out of a single-column aggregate result.
fn scalar_from_hits(hits: &[serde_json::Value]) -> Option<f64> {
    let row = hits.first()?.as_object()?;
    // Exactly one column, or nothing. Taking "the first value" of a multi-column
    // row would pick whichever key sorts first, silently answering a different
    // question than the caller asked — and every caller here is documented as
    // selecting a single aggregate.
    if row.len() != 1 {
        return None;
    }
    row.values().next()?.as_f64()
}
