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
        _ => Err(anyhow::Error::from(tonic::Status::unavailable(format!(
            "server is {status}"
        )))),
    }
}
