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

// ── ClickHouse query helpers (observability fixture) ──────────────────────────
//
// The observability tests assert that telemetry lands in ClickHouse — the
// unified store the collector exports to (ST0027). ClickHouse is an always-on
// fixture like Hydra: these helpers FAIL LOUDLY if it is unreachable, they never
// skip. Reachable via the `clickhouse` service name in-network; override with
// CLICKHOUSE_URL for other environments.

fn clickhouse_url() -> String {
    std::env::var("CLICKHOUSE_URL").unwrap_or_else(|_| "http://clickhouse:8123".to_string())
}

/// POST a SQL query to ClickHouse (default user, no password) and return the
/// trimmed response body. Panics on transport error — the obs fixture must be up.
pub async fn clickhouse_query(sql: &str) -> String {
    let http = reqwest::Client::new();
    http.post(clickhouse_url())
        .body(sql.to_string())
        .timeout(Duration::from_secs(5))
        .send()
        .await
        .expect(
            "ClickHouse query failed — is the observability fixture (ClickHouse) reachable? \
             It is an always-on dev/CI dependency, like Hydra.",
        )
        // Surface a 4xx/5xx (e.g. a SQL error) immediately rather than reading the
        // error page as if it were a result and wasting the poll budget.
        .error_for_status()
        .expect("ClickHouse returned an error status")
        .text()
        .await
        .expect("ClickHouse response body")
        .trim()
        .to_string()
}

/// Polls for a trace one of whose spans carries the entry `key` attribute,
/// returning the sorted, de-duplicated span names of that trace (empty if none
/// appear within the budget). Used for run-specific trace assertions.
pub async fn clickhouse_trace_span_names(key: &str) -> Vec<String> {
    let sql = format!(
        "SELECT DISTINCT SpanName FROM otel.otel_traces \
         WHERE TraceId IN (SELECT TraceId FROM otel.otel_traces \
                           WHERE SpanAttributes['key'] = '{key}') \
         ORDER BY SpanName FORMAT TabSeparated"
    );
    for _ in 0..30u32 {
        let body = clickhouse_query(&sql).await;
        if !body.is_empty() {
            return body
                .lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect();
        }
        sleep(Duration::from_secs(2)).await;
    }
    Vec::new()
}

/// Polls a scalar `count()` query, returning the first value > 0 (or 0 after the
/// budget expires).
pub async fn clickhouse_count(sql: &str) -> u64 {
    for _ in 0..45u32 {
        if let Ok(n) = clickhouse_query(sql).await.parse::<u64>() {
            if n > 0 {
                return n;
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    0
}

/// Single-shot scalar query parsed as f64 (None when the result is empty or
/// unparseable, e.g. a metric series that does not exist yet).
pub async fn clickhouse_scalar_f64(sql: &str) -> Option<f64> {
    clickhouse_query(sql).await.parse::<f64>().ok()
}
