//! Always-run, non-k8s observability test.
//!
//! Stands up an in-process udex server with observability **enabled**, pointed at
//! the compose collector (`http://otel-collector:4317`, plaintext OTLP/gRPC), drives a few
//! requests, and asserts the resulting trace / metric / log land in ClickHouse.
//! Unlike the `test_obs_k8s_*` tests this needs no cluster, so it exercises the
//! full server -> collector -> ClickHouse path on every `cargo test`.
//!
//! It lives in its OWN test binary on purpose: `udex_telemetry::init` installs a
//! PROCESS-GLOBAL tracing subscriber + OpenTelemetry providers, so it must be the
//! sole installer. The shared `integration_tests` binary pre-installs a test
//! subscriber (`init_test_tracing`), which would make the enabled-path `try_init`
//! here fail — hence the separation.
//!
//! ClickHouse is an always-on fixture like Hydra: the `common::clickhouse_*`
//! helpers FAIL (not skip) if it is unreachable.

use std::net::SocketAddr;

use jsonwebtoken::EncodingKey;
use tokio::time::{sleep, Duration};
use udex_api::index::{CreateIndexRequest, HashAlgorithm};
use udex_datastore::integration_test::init_postgres;
use udex_sdk::{ClientOptions, UdexClient};
use udex_telemetry::TelemetryConfig;
use udex_test_utils::bind_file_secret;

mod common;
use common::{
    clickhouse_scalar_f64, clickhouse_trace_span_names, context_input, jwt_key_path, make_token,
    now_unix_nanos, server_cert_path, wait_for_server,
};

// Distinct from the integration_tests binary's bind addresses to avoid clashes
// when both run concurrently.
const OBS_BIND_ADDR: &str = "127.0.0.1:50056";

const LIST_METHOD: &str = "/udex.index.v1.IndexService/ListIndices";

#[tokio::test]
async fn obs_local_traces_metrics_logs_land() {
    let _ = rustls::crypto::aws_lc_rs::default_provider().install_default();

    // Shorten the OTel metric export interval so the metric assertion does not
    // wait the 60s default. Honoured by the SDK PeriodicReader when set before
    // init; if it is ignored the generous poll budget below still covers 60s.
    std::env::set_var("OTEL_METRIC_EXPORT_INTERVAL", "5000");

    let datastore = init_postgres().await.0;

    let index_name = "sdk-obs-index".to_string();
    let jwt_issuer = "sdk-obs-issuer".to_string();
    let jwt_audience = "sdk-obs-audience".to_string();
    let bind_address: SocketAddr = OBS_BIND_ADDR.parse().unwrap();

    let server_config = udex_server::config::ServerConfig {
        bind_address,
        request_timeout: std::time::Duration::from_secs(30),
        max_connections: 1000,
        max_message_size: 4 * 1024 * 1024,
        tls: udex_server::config::TlsConfig {
            cert: bind_file_secret(&server_cert_path("server.crt")),
            key: bind_file_secret(&server_cert_path("server.key")),
        },
        // The whole point of this test: OTLP export ON. The exporter speaks OTLP
        // over gRPC, so it targets 4317 (the gRPC port); `http://` = plaintext
        // (no CA — the local fixture's collector terminates no TLS). Reachable by
        // service name in the devcontainer; CI overrides OTEL_COLLECTOR_OTLP_ENDPOINT
        // to the runner-host published port (http://localhost:4317).
        observability: Some(TelemetryConfig {
            enabled: true,
            otlp_endpoint: Some(
                std::env::var("OTEL_COLLECTOR_OTLP_ENDPOINT")
                    .unwrap_or_else(|_| "http://otel-collector:4317".to_string()),
            ),
            // The local fixture's collector terminates no TLS — opt into plaintext.
            dangerous_allow_non_tls: true,
            ..Default::default()
        }),
        init_indexes: vec![CreateIndexRequest {
            name: index_name.clone(),
            display_name: index_name.clone(),
            description: index_name.clone(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        }],
        authz: udex_server::config::AuthzConfig {
            jwks_url: None,
            jwt_public_key: Some(bind_file_secret(&jwt_key_path("signing_public_key.pem"))),
            jwt_issuer: Some(jwt_issuer.clone()),
            jwt_audience: Some(jwt_audience.clone()),
            danger_allow_non_tls: false,
            scope_claim_name: None,
            mask_subject_in_logs: false,
            jwks_max_failed_refreshes: None,
            jwks_backoff_factor_secs: None,
            jwks_max_age_secs: None,
        },
    };

    let ca_pem = tokio::fs::read(server_cert_path("ca.crt"))
        .await
        .expect("read CA cert");

    tokio::spawn(async move {
        udex_server::server::serve(server_config, datastore)
            .await
            .expect("server failed");
    });

    wait_for_server(OBS_BIND_ADDR, &ca_pem).await;

    let private_key_pem = tokio::fs::read_to_string(jwt_key_path("signing_private_key.pem"))
        .await
        .expect("read JWT private key");
    let signing_key = EncodingKey::from_ec_pem(private_key_pem.as_bytes()).expect("EncodingKey");
    let token = make_token(&signing_key, &jwt_issuer, &jwt_audience, &index_name, None);

    let client = UdexClient::connect(
        ClientOptions::builder()
            .endpoint(format!("https://{OBS_BIND_ADDR}"))
            .ca_cert_pem_bytes(ca_pem)
            .static_bearer_token(token)
            .build()
            .unwrap(),
    )
    .await
    .expect("SDK connect failed");

    // ── traces ──────────────────────────────────────────────────────────────
    // A unique context yields an entry with a server-generated key, stamped onto
    // the db.create_entry span — a run-specific handle to this exact trace.
    let tag = format!("obs-local-{}", now_unix_nanos());
    let resp = client
        .create_entry(&index_name, context_input(&[("obs_tag", &tag)]))
        .await
        .expect("create_entry");
    let key = resp.key;

    // Baselines BEFORE the ListIndices traffic, scoped to NON-k3d so concurrent
    // k8s telemetry in the shared ClickHouse cannot satisfy the increase.
    let metric_query = format!(
        "SELECT sum(v) FROM ( \
           SELECT argMax(Value, TimeUnix) AS v FROM otel.otel_metrics_sum \
           WHERE MetricName = 'udex.rpc.requests' \
             AND Attributes['rpc.method'] = '{LIST_METHOD}' \
             AND ResourceAttributes['deployment.environment'] != 'k3d' \
           GROUP BY ResourceAttributes['service.instance.id'], \
                    Attributes['rpc.grpc.status_code'] )"
    );
    let log_query = "SELECT count() FROM otel.otel_logs WHERE ServiceName = 'udex-server'";
    let metric_baseline = clickhouse_scalar_f64(&metric_query).await.unwrap_or(0.0);
    let log_baseline = clickhouse_scalar_f64(log_query).await.unwrap_or(0.0);

    // Drive request traffic: each authed ListIndices increments the request
    // counter and emits a "request authenticated" audit log.
    for _ in 0..3 {
        client.list_indices().await.expect("list_indices");
    }

    let spans = clickhouse_trace_span_names(&key).await;
    assert!(
        !spans.is_empty(),
        "no trace in ClickHouse for entry key {key}"
    );
    assert!(
        spans.iter().any(|s| s.ends_with("/CreateEntry")),
        "trace missing the CreateEntry request span; got {spans:?}"
    );
    assert!(
        spans.iter().any(|s| s == "db.create_entry"),
        "trace missing the db.create_entry span; got {spans:?}"
    );

    // ── metrics ── poll for the cumulative counter to rise above baseline. Budget
    // is generous to cover the metric export interval (5s if the env var is
    // honoured, else the 60s default).
    let mut metric_increased = false;
    for _ in 0..40u32 {
        if let Some(v) = clickhouse_scalar_f64(&metric_query).await {
            if v > metric_baseline {
                metric_increased = true;
                break;
            }
        }
        sleep(Duration::from_secs(3)).await;
    }
    assert!(
        metric_increased,
        "udex.rpc.requests (non-k3d) for {LIST_METHOD} did not increase (baseline {metric_baseline})"
    );

    // ── logs ── the audit logs export via the batch log processor (sub-second).
    let mut log_increased = false;
    for _ in 0..30u32 {
        if let Some(v) = clickhouse_scalar_f64(log_query).await {
            if v > log_baseline {
                log_increased = true;
                break;
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    assert!(
        log_increased,
        "udex-server logs in ClickHouse did not increase (baseline {log_baseline})"
    );
}
