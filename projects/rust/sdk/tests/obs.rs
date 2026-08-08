//! Always-run, non-k8s observability test.
//!
//! Stands up an in-process udex server with observability **enabled**, pointed at
//! the compose collector (`http://otel-collector:4317`, plaintext OTLP/gRPC), drives a few
//! requests, and asserts the resulting trace / metric / log land in OpenObserve.
//! Unlike the `test_obs_k8s_*` tests this needs no cluster, so it exercises the
//! full server -> collector -> OpenObserve path on every `cargo test`.
//!
//! It lives in its OWN test binary on purpose: `udex_telemetry::init` installs a
//! PROCESS-GLOBAL tracing subscriber + OpenTelemetry providers, so it must be the
//! sole installer. The shared `integration_tests` binary pre-installs a test
//! subscriber (`init_test_tracing`), which would make the enabled-path `try_init`
//! here fail — hence the separation.
//!
//! OpenObserve is an always-on fixture like Hydra: the `common::openobserve_*`
//! helpers FAIL (not skip) if it is unreachable.

use std::net::SocketAddr;

use jsonwebtoken::EncodingKey;
use tokio::time::{sleep, Duration};
use udex_datastore::integration_test::init_postgres;
use udex_sdk::{ClientOptions, CreateIndexRequest, HashAlgorithm, UdexClient};
use udex_telemetry::TelemetryConfig;
use udex_test_utils::bind_file_secret;

mod common;
use common::{
    context_input, jwt_key_path, make_token, now_unix_nanos, openobserve_metric_scalar_f64,
    openobserve_scalar_f64, openobserve_search, openobserve_trace_span_names, server_cert_path,
    wait_for_server,
};

// Distinct from the integration_tests binary's bind addresses to avoid clashes
// when both run concurrently.
const OBS_BIND_ADDR: &str = "127.0.0.1:50056";

const LIST_METHOD: &str = "/udex.index.v1.IndexService/ListIndices";

/// A rejected query must FAIL LOUDLY, not read as "no telemetry".
///
/// OpenObserve answers a bad query with **HTTP 200** and the reason in a
/// `message` field, leaving `hits` null. If the helper treated that as an empty
/// result set, a mistyped column would be indistinguishable from telemetry never
/// arriving — and the mistake is easy to make, because resource attributes are
/// prefixed `service_` in the traces stream but bare in logs and metrics. The
/// failure would surface as a poll-budget timeout blaming the pipeline, minutes
/// away from the actual cause.
///
/// This guards `IN-AG-NO-SILENT-001` for the whole observability suite, so it is
/// tested directly rather than trusted.
///
/// A panic backtrace in this test's output is expected — it is the behaviour
/// being asserted.
#[tokio::test]
async fn obs_search_surfaces_query_errors() {
    let result = tokio::spawn(async {
        openobserve_search("SELECT no_such_column_at_all FROM \"default\"", "logs").await
    })
    .await;

    let panic_payload = result
        .expect_err(
            "a query OpenObserve rejects must panic; returning an empty result set would make \
             a typo look like missing telemetry",
        )
        .into_panic();

    let message = panic_payload
        .downcast_ref::<String>()
        .map(String::as_str)
        .or_else(|| panic_payload.downcast_ref::<&str>().copied())
        .unwrap_or("<non-string panic payload>");

    assert!(
        message.contains("OpenObserve rejected the query"),
        "panic did not identify the query rejection; got: {message}"
    );
    // The API's own reason must reach the developer, not just our wrapper text —
    // that reason is what names the offending column.
    assert!(
        message.contains("no_such_column_at_all"),
        "panic did not carry the API's reason naming the bad column; got: {message}"
    );
}

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

    // A unique resource attribute tags THIS run's telemetry, so the metric/log
    // assertions match only this server's exports (not concurrent k8s traffic or a
    // previous run) in the shared OpenObserve.
    let run_id = format!("obs-run-{}", now_unix_nanos());

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
            // Tag every signal from this run so the assertions are run-specific.
            resource_attributes: std::collections::BTreeMap::from([(
                "udex.test.run".to_string(),
                run_id.clone(),
            )]),
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
            dangerous_allow_non_tls: false,
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

    // Baselines BEFORE the ListIndices traffic. Both queries are scoped to THIS
    // run's resource tag (`udex.test.run`), so only this server's exports can
    // advance the count — concurrent k8s traffic or a prior run cannot.
    // `udex.rpc.requests` becomes its own stream, dots as underscores. Taking
    // max(value) per series is the argMax equivalent (OpenObserve's DataFusion
    // build has no max_by) and is CORRECT ONLY BECAUSE the counter is monotonic
    // cumulative — so the latest value of a series is also its largest. Summing
    // those per-series maxima gives a total that rises by one per request.
    //
    // Note the attribute naming: in the metrics stream, resource attributes are
    // flattened bare (`udex_test_run`, `service_instance_id`). In the *traces*
    // stream the same attributes carry a `service_` prefix. Getting this wrong
    // returns HTTP 200 with an error body, which openobserve_search panics on.
    let metric_query = format!(
        "SELECT sum(m) AS total FROM ( \
           SELECT max(value) AS m FROM \"udex_rpc_requests\" \
           WHERE rpc_method = '{LIST_METHOD}' \
             AND udex_test_run = '{run_id}' \
           GROUP BY service_instance_id, rpc_grpc_status_code )"
    );
    let log_query = format!(
        "SELECT count(*) AS n FROM \"default\" \
         WHERE service_name = 'udex-server' AND udex_test_run = '{run_id}'"
    );
    let metric_baseline = openobserve_metric_scalar_f64(&metric_query)
        .await
        .unwrap_or(0.0);
    let log_baseline = openobserve_scalar_f64(&log_query, "logs")
        .await
        .unwrap_or(0.0);

    // Drive request traffic: each authed ListIndices increments the request
    // counter and emits a "request authenticated" audit log.
    for _ in 0..3 {
        client.list_indices().await.expect("list_indices");
    }

    let spans = openobserve_trace_span_names(&key).await;
    assert!(
        !spans.is_empty(),
        "no trace in OpenObserve for entry key {key}"
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
        if let Some(v) = openobserve_metric_scalar_f64(&metric_query).await {
            if v > metric_baseline {
                metric_increased = true;
                break;
            }
        }
        sleep(Duration::from_secs(3)).await;
    }
    assert!(
        metric_increased,
        "udex.rpc.requests (this run) for {LIST_METHOD} did not increase (baseline \
         {metric_baseline}). If the value never appeared at all, check the column names in the \
         query against the metrics stream schema — a misspelled column reads as an absent series \
         here, which is the cost of tolerating metric cold starts."
    );

    // ── logs ── the audit logs export via the batch log processor (sub-second).
    let mut log_increased = false;
    for _ in 0..30u32 {
        if let Some(v) = openobserve_scalar_f64(&log_query, "logs").await {
            if v > log_baseline {
                log_increased = true;
                break;
            }
        }
        sleep(Duration::from_secs(2)).await;
    }
    assert!(
        log_increased,
        "udex-server logs in OpenObserve did not increase (baseline {log_baseline})"
    );
}
