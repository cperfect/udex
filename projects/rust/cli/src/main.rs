//! Udex CLI — `udex` binary entry point.

mod cli;
mod commands;
mod config;

use clap::Parser;
use udex_sdk::{ClientOptions, UdexClient};

use cli::{
    Cli, Commands, ConfigCommands, ContextCommands, EntryCommands, IndexCommands, TokenCommands,
};

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
/// Walks the `anyhow` source chain looking for a `tonic::Status` (gRPC-level
/// errors), a `udex_sdk::Error` (SDK-wrapped gRPC errors), or a
/// `tonic::transport::Error` (transport-level failures). Falls back to 1 for
/// anything else.
///
/// | Exit | Cause |
/// |------|-------|
/// |  1   | unclassified / internal error |
/// |  2   | gRPC NOT_FOUND |
/// |  3   | gRPC ALREADY_EXISTS |
/// |  4   | gRPC INVALID_ARGUMENT / FAILED_PRECONDITION / OUT_OF_RANGE |
/// |  5   | gRPC UNAUTHENTICATED |
/// |  6   | gRPC PERMISSION_DENIED |
/// |  7   | gRPC UNAVAILABLE / DEADLINE_EXCEEDED |
/// |  8   | transport failure (connection refused, TLS error, DNS failure) |
fn grpc_exit_code(e: &anyhow::Error) -> i32 {
    use std::error::Error as StdError;

    let root: &(dyn StdError + 'static) = e.as_ref();
    let mut src: Option<&(dyn StdError + 'static)> = Some(root);
    while let Some(cause) = src {
        if let Some(status) = cause.downcast_ref::<tonic::Status>() {
            return status_exit_code(status);
        }
        if cause.downcast_ref::<tonic::transport::Error>().is_some() {
            return 8;
        }
        if let Some(sdk_err) = cause.downcast_ref::<udex_sdk::Error>() {
            return match sdk_err {
                udex_sdk::Error::Rpc(status) => status_exit_code(status),
                udex_sdk::Error::Transport(_) => 8,
                _ => 1,
            };
        }
        src = cause.source();
    }
    1
}

fn status_exit_code(status: &tonic::Status) -> i32 {
    use tonic::Code;
    match status.code() {
        Code::NotFound => 2,
        Code::AlreadyExists => 3,
        Code::InvalidArgument | Code::FailedPrecondition | Code::OutOfRange => 4,
        Code::Unauthenticated => 5,
        Code::PermissionDenied => 6,
        Code::Unavailable | Code::DeadlineExceeded => 7,
        _ => 1,
    }
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

async fn run(cli: Cli) -> anyhow::Result<()> {
    let Cli { command, server, ca_cert, output, .. } = cli;

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
                IndexCommands::Delete(args) => commands::index::delete(&client, args).await,
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
                EntryCommands::Delete(args) => commands::entry::delete(&client, args).await,
            }
        }
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
            2
        );
    }

    #[test]
    fn exit_code_already_exists() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::AlreadyExists)),
            3
        );
    }

    #[test]
    fn exit_code_invalid_argument() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::InvalidArgument)),
            4
        );
    }

    #[test]
    fn exit_code_unauthenticated() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::Unauthenticated)),
            5
        );
    }

    #[test]
    fn exit_code_permission_denied() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::PermissionDenied)),
            6
        );
    }

    #[test]
    fn exit_code_unavailable() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::Unavailable)),
            7
        );
    }

    #[test]
    fn exit_code_deadline_exceeded() {
        assert_eq!(
            grpc_exit_code(&anyhow_from_status(tonic::Code::DeadlineExceeded)),
            7
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
        assert_eq!(grpc_exit_code(&bare_status(tonic::Code::NotFound)), 2);
    }

    #[test]
    fn exit_code_bare_status_unauthenticated() {
        assert_eq!(
            grpc_exit_code(&bare_status(tonic::Code::Unauthenticated)),
            5
        );
    }

    // SDK-wrapped RPC errors are mapped correctly.
    #[test]
    fn exit_code_sdk_rpc_not_found() {
        assert_eq!(grpc_exit_code(&sdk_rpc_error(tonic::Code::NotFound)), 2);
    }

    #[test]
    fn exit_code_sdk_rpc_permission_denied() {
        assert_eq!(
            grpc_exit_code(&sdk_rpc_error(tonic::Code::PermissionDenied)),
            6
        );
    }
}
