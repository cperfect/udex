//! Handler for `udex health`.

use anyhow::Result;
use udex_sdk::{HealthStatus, UdexClient};

/// Check the server health status and exit non-zero if not SERVING.
pub async fn run(client: &UdexClient) -> Result<()> {
    let status = client.health().await?;
    match status {
        HealthStatus::Serving => {
            println!("Server is SERVING");
            Ok(())
        }
        // The server is reachable but not in the required state — distinct from
        // a transport failure (exit 20). Maps to exit 9 (FAILED_PRECONDITION).
        _ => Err(anyhow::Error::from(tonic::Status::failed_precondition(
            format!("server is {status}"),
        ))),
    }
}
