//! Udex CLI — `udex` binary entry point.

mod cli;
mod commands;
mod config;

use clap::Parser;
use udex_sdk::{ClientOptions, UdexClient};

use cli::{
    Cli, Commands, ConfigCommands, ContextCommands, EntryCommands, IndexCommands, MigrateCommands,
    TokenCommands,
};
use std::path::PathBuf;

fn main() {
    // Parse args and set RUST_LOG *before* creating the Tokio runtime.
    // set_var is UB when called from a multi-threaded process; doing it here
    // guarantees no runtime threads exist yet.
    let cli = Cli::parse();
    if cli.verbose && std::env::var("RUST_LOG").is_err() {
        // SAFETY: single-threaded at this point — Tokio runtime not yet started.
        unsafe { std::env::set_var("RUST_LOG", "debug") };
    }

    // Install the rustls crypto provider before any TLS connections are made.
    rustls::crypto::aws_lc_rs::default_provider()
        .install_default()
        .expect("failed to install rustls CryptoProvider");

    let result = tokio::runtime::Runtime::new()
        .expect("failed to create Tokio runtime")
        .block_on(run(cli));

    if let Err(e) = result {
        eprintln!("error: {e:#}");
        std::process::exit(grpc_exit_code(&e));
    }
}

/// Map an error to a distinct exit code.
///
/// Walks the `anyhow` source chain looking for a `udex_sdk::Error` (SDK errors),
/// a bare `tonic::Status` (gRPC errors surfaced outside the SDK), or a
/// `tonic::transport::Error` (transport-level failures). Falls back to 1 for
/// anything else.
///
/// gRPC status codes map directly: exit N = gRPC code N (1–16).
/// Non-gRPC causes use codes ≥ 20 to keep the two ranges distinct.
///
/// | Exit | Cause |
/// |------|-------|
/// |  1   | gRPC CANCELLED |
/// |  2   | gRPC UNKNOWN |
/// |  3   | gRPC INVALID_ARGUMENT |
/// |  4   | gRPC DEADLINE_EXCEEDED |
/// |  5   | gRPC NOT_FOUND |
/// |  6   | gRPC ALREADY_EXISTS |
/// |  7   | gRPC PERMISSION_DENIED |
/// |  8   | gRPC RESOURCE_EXHAUSTED |
/// |  9   | gRPC FAILED_PRECONDITION |
/// | 10   | gRPC ABORTED |
/// | 11   | gRPC OUT_OF_RANGE |
/// | 12   | gRPC UNIMPLEMENTED |
/// | 13   | gRPC INTERNAL |
/// | 14   | gRPC UNAVAILABLE |
/// | 15   | gRPC DATA_LOSS |
/// | 16   | gRPC UNAUTHENTICATED |
/// | 20   | transport failure (connection refused, TLS error, DNS failure) |
fn grpc_exit_code(e: &anyhow::Error) -> i32 {
    use std::error::Error as StdError;

    let root: &(dyn StdError + 'static) = e.as_ref();
    let mut src: Option<&(dyn StdError + 'static)> = Some(root);
    while let Some(cause) = src {
        if let Some(sdk_err) = cause.downcast_ref::<udex_sdk::Error>() {
            return match sdk_err {
                udex_sdk::Error::Rpc(s) => exit_code_for_rpc(s.code()),
                udex_sdk::Error::Transport(_) => 20,
                _ => 1,
            };
        }
        if let Some(status) = cause.downcast_ref::<tonic::Status>() {
            return exit_code_for_rpc(i32::from(status.code()) as u32);
        }
        if cause.downcast_ref::<tonic::transport::Error>().is_some() {
            return 20;
        }
        src = cause.source();
    }
    1
}

fn exit_code_for_rpc(code: u32) -> i32 {
    // gRPC status codes 1–16 map directly to exit codes 1–16.
    if (1..=16).contains(&code) {
        return code as i32;
    }
    1
}

/// Build a `UdexClient` with no authentication — for use by commands that do
/// not require a token (e.g. `health`). Avoids an unnecessary OAuth2 token
/// fetch even when auth env vars are set.
async fn build_unauthenticated_client(
    server: &str,
    ca_cert: &Option<PathBuf>,
) -> anyhow::Result<UdexClient> {
    let mut builder = ClientOptions::builder().endpoint(server);
    if let Some(ca) = ca_cert {
        builder = builder.ca_cert_pem_file(ca);
    }
    if server.starts_with("http://") {
        builder = builder.danger_allow_non_tls();
    }
    Ok(UdexClient::connect(builder.build()?).await?)
}

/// Build a `UdexClient` from the global CLI flags and environment variables.
///
/// Auth priority:
/// 1. `UDEX_TOKEN` — static bearer token (development only)
/// 2. `UDEX_TOKEN_URL` + `UDEX_CLIENT_ID` + `UDEX_CLIENT_SECRET` — OAuth2 client-credentials
/// 3. No auth
async fn build_client(
    server: &str,
    ca_cert: &Option<std::path::PathBuf>,
) -> anyhow::Result<UdexClient> {
    let mut builder = ClientOptions::builder().endpoint(server);

    if let Some(ca) = ca_cert {
        builder = builder.ca_cert_pem_file(ca);
    }

    if let Ok(token) = std::env::var("UDEX_TOKEN") {
        builder = builder.static_bearer_token(token);
    } else if let (Ok(url), Ok(id), Ok(secret)) = (
        std::env::var("UDEX_TOKEN_URL"),
        std::env::var("UDEX_CLIENT_ID"),
        std::env::var("UDEX_CLIENT_SECRET"),
    ) {
        builder = builder.client_credentials(url, id, secret);
        if let Ok(aud) = std::env::var("UDEX_AUDIENCE") {
            builder = builder.audience(aud);
        }
        if let Ok(scope) = std::env::var("UDEX_SCOPE") {
            builder = builder.scope(scope);
        }
    }

    Ok(UdexClient::connect(builder.build()?).await?)
}

/// Initialise client-side telemetry for non-`serve` CLI commands.
///
/// Opt-in: returns `Ok(None)` (no subscriber installed, current behaviour
/// preserved) unless `UDEX_OTLP_ENDPOINT` is set. When set, the CLI emits OTLP
/// traces/metrics/logs as `udex-cli` so commands can be traced end to end against
/// the observability stack. `serve` is excluded — it initialises its own
/// telemetry from the server config.
fn init_cli_telemetry() -> anyhow::Result<Option<udex_telemetry::TelemetryGuard>> {
    let Ok(endpoint) = std::env::var("UDEX_OTLP_ENDPOINT") else {
        return Ok(None);
    };
    // Fail fast on a malformed UDEX_TRACE_SAMPLE_RATIO rather than silently
    // enabling full sampling.
    let sample_ratio: f64 = match std::env::var("UDEX_TRACE_SAMPLE_RATIO") {
        Ok(s) => s.parse().map_err(|_| {
            anyhow::anyhow!("invalid UDEX_TRACE_SAMPLE_RATIO '{s}': expected a number in 0.0..=1.0")
        })?,
        Err(_) => 1.0,
    };
    let cfg = udex_telemetry::TelemetryConfig {
        enabled: true,
        otlp_endpoint: Some(endpoint),
        otlp_ca: std::env::var("UDEX_OTLP_CA").ok(),
        sample_ratio,
        ..Default::default()
    };
    let guard = udex_telemetry::init(
        &cfg,
        udex_telemetry::ServiceIdentity {
            name: "udex-cli".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            instance_id: format!("pid-{}", std::process::id()),
        },
    )
    .map_err(|e| anyhow::anyhow!("telemetry init failed: {e}"))?;
    Ok(Some(guard))
}

async fn run(cli: Cli) -> anyhow::Result<()> {
    let Cli {
        command,
        server,
        ca_cert,
        output,
        ..
    } = cli;

    // Optional client telemetry (opt-in via UDEX_OTLP_ENDPOINT). Held for the
    // duration of the command so spans/metrics/logs are flushed on completion.
    let _telemetry_guard = if matches!(command, Commands::Serve(_)) {
        None
    } else {
        init_cli_telemetry()?
    };

    match command {
        Commands::Serve(args) => commands::serve::run(args).await,

        Commands::Config { command } => match command {
            ConfigCommands::Init(args) => commands::config::init(args),
            ConfigCommands::Validate(args) => commands::config::validate(args),
        },

        Commands::Token { command } => match command {
            TokenCommands::Inspect(args) => commands::token::inspect(args),
            TokenCommands::Fetch(args) => commands::token::fetch(args, &output).await,
        },

        Commands::Context { command } => match command {
            ContextCommands::Hash(args) => commands::context::hash(args),
        },

        Commands::Migrate { command } => match command {
            MigrateCommands::Check(args) => commands::migrate::check(args).await,
            MigrateCommands::Apply(args) => commands::migrate::apply(args).await,
        },

        Commands::Index { command } => {
            let client = build_client(&server, &ca_cert).await?;
            match command {
                IndexCommands::List => commands::index::list(&client, &output).await,
                IndexCommands::Create(args) => {
                    commands::index::create(&client, args, &output).await
                }
                IndexCommands::Get(args) => commands::index::get(&client, args, &output).await,
                IndexCommands::Update(args) => {
                    commands::index::update(&client, args, &output).await
                }
                IndexCommands::Delete(args) => {
                    commands::index::delete(&client, args, &output).await
                }
            }
        }

        Commands::Entry { command } => {
            let client = build_client(&server, &ca_cert).await?;
            match command {
                EntryCommands::Create(args) => {
                    commands::entry::create(&client, args, &output).await
                }
                EntryCommands::Get(args) => commands::entry::get(&client, args, &output).await,
                EntryCommands::Lookup(args) => {
                    commands::entry::lookup(&client, args, &output).await
                }
                EntryCommands::LookupOrCreate(args) => {
                    commands::entry::lookup_or_create(&client, args, &output).await
                }
                EntryCommands::Delete(args) => {
                    commands::entry::delete(&client, args, &output).await
                }
            }
        }

        Commands::Health => {
            let client = build_unauthenticated_client(&server, &ca_cert).await?;
            commands::health::run(&client).await
        }

        Commands::Version(args) => commands::version::run(args),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn anyhow_from_status(code: tonic::Code) -> anyhow::Error {
        anyhow::Error::from(tonic::Status::new(code, "test")).context("rpc failed")
    }

    // Bare status — no .context() wrapper — root IS the tonic::Status.
    fn bare_status(code: tonic::Code) -> anyhow::Error {
        anyhow::Error::from(tonic::Status::new(code, "test"))
    }

    fn sdk_rpc_error(code: tonic::Code) -> anyhow::Error {
        anyhow::Error::from(udex_sdk::Error::from(tonic::Status::new(code, "test")))
            .context("rpc failed")
    }

    #[test]
    fn exit_code_not_found() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::NotFound)),
            5
        );
    }

    #[test]
    fn exit_code_already_exists() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::AlreadyExists)),
            6
        );
    }

    #[test]
    fn exit_code_invalid_argument() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::InvalidArgument)),
            3
        );
    }

    #[test]
    fn exit_code_unauthenticated() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::Unauthenticated)),
            16
        );
    }

    #[test]
    fn exit_code_permission_denied() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::PermissionDenied)),
            7
        );
    }

    #[test]
    fn exit_code_unavailable() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::Unavailable)),
            14
        );
    }

    #[test]
    fn exit_code_deadline_exceeded() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::DeadlineExceeded)),
            4
        );
    }

    #[test]
    fn exit_code_non_grpc_error() {
        let e = anyhow::anyhow!("something went wrong");
        assert_eq!(grpc_exit_code(&e), 1);
    }

    // Bare tonic::Status (no .context() wrapper) — root itself must be matched.
    #[test]
    fn exit_code_bare_status_not_found() {
        assert_eq!(grpc_exit_code(&bare_status(tonic::Code::NotFound)), 5);
    }

    #[test]
    fn exit_code_bare_status_unauthenticated() {
        assert_eq!(
            grpc_exit_code(&bare_status(tonic::Code::Unauthenticated)),
            16
        );
    }

    // SDK-wrapped RPC errors are mapped correctly.
    #[test]
    fn exit_code_sdk_rpc_not_found() {
        assert_eq!(grpc_exit_code(&sdk_rpc_error(tonic::Code::NotFound)), 5);
    }

    #[test]
    fn exit_code_sdk_rpc_permission_denied() {
        assert_eq!(
            grpc_exit_code(&sdk_rpc_error(tonic::Code::PermissionDenied)),
            7
        );
    }
}
