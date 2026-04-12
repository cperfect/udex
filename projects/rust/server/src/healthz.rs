use std::sync::Arc;
use tonic::{Request, Response, Status};
use udex_api::healthz::{
    healthz_service_server::HealthzService as HealthzServiceTrait, HealthzRequest, HealthzResponse,
};

use crate::HealthCheck;

/// Server implementation for the Healthz service.
/// Handles all health check-related operations.
#[derive(Clone)]
pub struct HealthzService {
    entry_service: Arc<dyn HealthCheck + Send + Sync>,
    index_service: Arc<dyn HealthCheck + Send + Sync>,
}

impl HealthzService {
    pub fn new(
        entry_service: Arc<dyn HealthCheck + Send + Sync>,
        index_service: Arc<dyn HealthCheck + Send + Sync>,
    ) -> Self {
        Self {
            entry_service,
            index_service,
        }
    }
}

#[tonic::async_trait]
impl HealthzServiceTrait for HealthzService {
    /// Handles health check requests.
    async fn healthz(
        &self,
        _request: Request<HealthzRequest>,
    ) -> Result<Response<HealthzResponse>, Status> {
        // Check health of entry service
        let entry_healthy =
            self.entry_service.is_healthy().await.map_err(|e| {
                Status::internal(format!("Entry service health check failed: {}", e))
            })?;

        // Check health of index service
        let index_healthy =
            self.index_service.is_healthy().await.map_err(|e| {
                Status::internal(format!("Index service health check failed: {}", e))
            })?;

        Ok(Response::new(HealthzResponse {
            server_time: Some(udex_api::now_timestamp()),
            is_healthy: entry_healthy && index_healthy,
            status_messages: vec![
                if entry_healthy {
                    "Entry service is healthy".to_string()
                } else {
                    "Entry service is unhealthy".to_string()
                },
                if index_healthy {
                    "Index service is healthy".to_string()
                } else {
                    "Index service is unhealthy".to_string()
                },
            ],
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Error;
    use mockall::mock;
    use std::sync::Arc;
    use tonic::Request;
    use udex_api::healthz::HealthzRequest;

    // Create a mock implementation of HealthCheck
    mock! {
        HealthChecker {}

        #[tonic::async_trait]
        impl HealthCheck for HealthChecker {
            async fn is_healthy(&self) -> Result<bool, Error>;
        }
    }

    #[tokio::test]
    async fn test_healthz_both_services_healthy() {
        // Arrange
        let mut mock_entry_service = MockHealthChecker::new();
        let mut mock_index_service = MockHealthChecker::new();

        mock_entry_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(true));

        mock_index_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(true));

        let healthz_service =
            HealthzService::new(Arc::new(mock_entry_service), Arc::new(mock_index_service));

        let request = Request::new(HealthzRequest {});

        // Act
        let response = healthz_service.healthz(request).await;

        // Assert
        assert!(response.is_ok());
        let response = response.unwrap();
        let healthz_response = response.into_inner();

        assert!(healthz_response.is_healthy);
        assert_eq!(healthz_response.status_messages.len(), 2);
        assert_eq!(
            healthz_response.status_messages[0],
            "Entry service is healthy"
        );
        assert_eq!(
            healthz_response.status_messages[1],
            "Index service is healthy"
        );
        assert!(healthz_response.server_time.is_some());
    }

    #[tokio::test]
    async fn test_healthz_entry_service_unhealthy() {
        // Arrange
        let mut mock_entry_service = MockHealthChecker::new();
        let mut mock_index_service = MockHealthChecker::new();

        mock_entry_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(false));

        mock_index_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(true));

        let healthz_service =
            HealthzService::new(Arc::new(mock_entry_service), Arc::new(mock_index_service));

        let request = Request::new(HealthzRequest {});

        // Act
        let response = healthz_service.healthz(request).await;

        // Assert
        assert!(response.is_ok());
        let response = response.unwrap();
        let healthz_response = response.into_inner();

        assert!(!healthz_response.is_healthy); // Overall health should be false
        assert_eq!(healthz_response.status_messages.len(), 2);
        assert_eq!(
            healthz_response.status_messages[0],
            "Entry service is unhealthy"
        );
        assert_eq!(
            healthz_response.status_messages[1],
            "Index service is healthy"
        );
        assert!(healthz_response.server_time.is_some());
    }

    #[tokio::test]
    async fn test_healthz_index_service_unhealthy() {
        // Arrange
        let mut mock_entry_service = MockHealthChecker::new();
        let mut mock_index_service = MockHealthChecker::new();

        mock_entry_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(true));

        mock_index_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(false));

        let healthz_service =
            HealthzService::new(Arc::new(mock_entry_service), Arc::new(mock_index_service));

        let request = Request::new(HealthzRequest {});

        // Act
        let response = healthz_service.healthz(request).await;

        // Assert
        assert!(response.is_ok());
        let response = response.unwrap();
        let healthz_response = response.into_inner();

        assert!(!healthz_response.is_healthy); // Overall health should be false
        assert_eq!(healthz_response.status_messages.len(), 2);
        assert_eq!(
            healthz_response.status_messages[0],
            "Entry service is healthy"
        );
        assert_eq!(
            healthz_response.status_messages[1],
            "Index service is unhealthy"
        );
        assert!(healthz_response.server_time.is_some());
    }

    #[tokio::test]
    async fn test_healthz_both_services_unhealthy() {
        // Arrange
        let mut mock_entry_service = MockHealthChecker::new();
        let mut mock_index_service = MockHealthChecker::new();

        mock_entry_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(false));

        mock_index_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(false));

        let healthz_service =
            HealthzService::new(Arc::new(mock_entry_service), Arc::new(mock_index_service));

        let request = Request::new(HealthzRequest {});

        // Act
        let response = healthz_service.healthz(request).await;

        // Assert
        assert!(response.is_ok());
        let response = response.unwrap();
        let healthz_response = response.into_inner();

        assert!(!healthz_response.is_healthy);
        assert_eq!(healthz_response.status_messages.len(), 2);
        assert_eq!(
            healthz_response.status_messages[0],
            "Entry service is unhealthy"
        );
        assert_eq!(
            healthz_response.status_messages[1],
            "Index service is unhealthy"
        );
        assert!(healthz_response.server_time.is_some());
    }

    #[tokio::test]
    async fn test_healthz_entry_service_error() {
        // Arrange
        let mut mock_entry_service = MockHealthChecker::new();
        let mut mock_index_service = MockHealthChecker::new();

        mock_entry_service
            .expect_is_healthy()
            .times(1)
            .returning(|| {
                Err(Error::ServerError(
                    "Entry service connection failed".to_string(),
                ))
            });

        // Index service should not be called if entry service fails
        mock_index_service.expect_is_healthy().times(0);

        let healthz_service =
            HealthzService::new(Arc::new(mock_entry_service), Arc::new(mock_index_service));

        let request = Request::new(HealthzRequest {});

        // Act
        let response = healthz_service.healthz(request).await;

        // Assert
        assert!(response.is_err());
        let error = response.unwrap_err();
        assert_eq!(error.code(), tonic::Code::Internal);
        assert!(error
            .message()
            .contains("Entry service health check failed"));
        assert!(error.message().contains("Entry service connection failed"));
    }

    #[tokio::test]
    async fn test_healthz_index_service_error() {
        // Arrange
        let mut mock_entry_service = MockHealthChecker::new();
        let mut mock_index_service = MockHealthChecker::new();

        mock_entry_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(true));

        mock_index_service
            .expect_is_healthy()
            .times(1)
            .returning(|| {
                Err(Error::ServerError(
                    "Index service database unavailable".to_string(),
                ))
            });

        let healthz_service =
            HealthzService::new(Arc::new(mock_entry_service), Arc::new(mock_index_service));

        let request = Request::new(HealthzRequest {});

        // Act
        let response = healthz_service.healthz(request).await;

        // Assert
        assert!(response.is_err());
        let error = response.unwrap_err();
        assert_eq!(error.code(), tonic::Code::Internal);
        assert!(error
            .message()
            .contains("Index service health check failed"));
        assert!(error
            .message()
            .contains("Index service database unavailable"));
    }

    #[tokio::test]
    async fn test_healthz_server_time_is_set() {
        // Arrange
        let mut mock_entry_service = MockHealthChecker::new();
        let mut mock_index_service = MockHealthChecker::new();

        mock_entry_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(true));

        mock_index_service
            .expect_is_healthy()
            .times(1)
            .returning(|| Ok(true));

        let healthz_service =
            HealthzService::new(Arc::new(mock_entry_service), Arc::new(mock_index_service));

        let request = Request::new(HealthzRequest {});
        let before_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();

        // Act
        let response = healthz_service.healthz(request).await;

        let after_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap();

        // Assert
        assert!(response.is_ok());
        let response = response.unwrap();
        let healthz_response = response.into_inner();

        assert!(healthz_response.server_time.is_some());
        let server_time = healthz_response.server_time.unwrap();

        // Check that server time is reasonable (between before and after)
        assert!(server_time.seconds >= before_time.as_secs() as i64);
        assert!(server_time.seconds <= after_time.as_secs() as i64);
        assert!(server_time.nanos >= 0);
        assert!(server_time.nanos < 1_000_000_000); // Less than 1 second in nanoseconds
    }
}
