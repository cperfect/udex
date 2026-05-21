use tonic_health::pb::health_check_response::ServingStatus;
use tonic_health::pb::health_client::HealthClient;
use tonic_health::pb::HealthCheckRequest;

use crate::client::UdexClient;
use crate::error::Error;

/// The serving status reported by the server's health endpoint.
///
/// Returned by [`UdexClient::health`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// The server is ready to accept requests.
    Serving,
    /// The server is running but not ready to accept requests.
    NotServing,
    /// The health status could not be determined.
    Unknown,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            HealthStatus::Serving => write!(f, "SERVING"),
            HealthStatus::NotServing => write!(f, "NOT_SERVING"),
            HealthStatus::Unknown => write!(f, "UNKNOWN"),
        }
    }
}

impl UdexClient {
    /// Checks the server's overall health status.
    ///
    /// Uses the standard [gRPC Health Checking Protocol](https://github.com/grpc/grpc-proto/blob/master/grpc/health/v1/health.proto).
    /// No authentication is required.
    pub async fn health(&self) -> Result<HealthStatus, Error> {
        let mut client = HealthClient::new(self.channel.clone());
        let resp = client
            .check(HealthCheckRequest {
                service: String::new(),
            })
            .await?
            .into_inner();
        let status = match ServingStatus::try_from(resp.status) {
            Ok(ServingStatus::Serving) => HealthStatus::Serving,
            Ok(ServingStatus::NotServing) => HealthStatus::NotServing,
            _ => HealthStatus::Unknown,
        };
        Ok(status)
    }
}
