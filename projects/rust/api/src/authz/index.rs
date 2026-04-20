use std::sync::Arc;

use crate::authz::{
    claims::Claims,
    permissions::{is_permitted, Permissable},
};
use crate::index::{
    index_service_server::IndexService, CreateIndexRequest, CreateIndexResponse, DescribeRequest,
    DescribeResponse, ListIndicesRequest, ListIndicesResponse, UpdateIndexRequest,
    UpdateIndexResponse,
};

/// Authorizor for IndexService that checks permissions based on Claims for each method call.
/// This exists because getting hold of the message type & content in an interceptor is painful.
/// The authorizor wraps the actual IndexService implementation and checks permissions before delegating the call.
pub struct IndexServiceAuthorizor<I>
where
    I: IndexService + Send + Sync + 'static,
{
    inner: Arc<I>,
}

impl<I> IndexServiceAuthorizor<I>
where
    I: IndexService + Send + Sync + 'static,
{
    pub fn new(inner: Arc<I>) -> Self {
        IndexServiceAuthorizor { inner }
    }
}

#[tonic::async_trait]
impl<I> IndexService for IndexServiceAuthorizor<I>
where
    I: IndexService + Send + Sync + 'static,
{
    /// Describe returns information about the index
    async fn describe(
        &self,
        request: tonic::Request<DescribeRequest>,
    ) -> std::result::Result<tonic::Response<DescribeResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims)
            .map_err(|e| tonic::Status::internal(format!("Permission check failed: {}", e)))?
        {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.describe(request).await
    }

    /// CreateIndex creates a new index
    async fn create_index(
        &self,
        request: tonic::Request<CreateIndexRequest>,
    ) -> std::result::Result<tonic::Response<CreateIndexResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims)
            .map_err(|e| tonic::Status::internal(format!("Permission check failed: {}", e)))?
        {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.create_index(request).await
    }

    /// UpdateIndex updates an existing index's mutable fields
    async fn update_index(
        &self,
        request: tonic::Request<UpdateIndexRequest>,
    ) -> std::result::Result<tonic::Response<UpdateIndexResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims)
            .map_err(|e| tonic::Status::internal(format!("Permission check failed: {}", e)))?
        {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.update_index(request).await
    }

    /// ListIndices returns a list of all indices
    async fn list_indices(
        &self,
        request: tonic::Request<ListIndicesRequest>,
    ) -> std::result::Result<tonic::Response<ListIndicesResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims)
            .map_err(|e| tonic::Status::internal(format!("Permission check failed: {}", e)))?
        {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.list_indices(request).await
    }
}

impl Permissable<DescribeRequest> for DescribeRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:index:v1:{}:read", self.name)]
    }
}

impl Permissable<CreateIndexRequest> for CreateIndexRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![
            format!("udex:index:v1:create"),
            // format!("udex:index:v1:{}:write", self.name)
        ]
    }
}

impl Permissable<UpdateIndexRequest> for UpdateIndexRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:index:v1:{}:write", self.name)]
    }
}

impl Permissable<ListIndicesRequest> for ListIndicesRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec!["udex:index:v1:list".to_string()]
    }
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use crate::index::HashAlgorithm;
    use mockall::mock;
    use serde_json::json;
    use tonic::{Request, Response, Status};

    // Mock the IndexService trait
    mock! {
        pub IndexServiceImpl {}

        #[tonic::async_trait]
        impl IndexService for IndexServiceImpl {
            async fn describe(
                &self,
                request: Request<DescribeRequest>,
            ) -> Result<Response<DescribeResponse>, Status>;

            async fn create_index(
                &self,
                request: Request<CreateIndexRequest>,
            ) -> Result<Response<CreateIndexResponse>, Status>;

            async fn update_index(
                &self,
                request: Request<UpdateIndexRequest>,
            ) -> Result<Response<UpdateIndexResponse>, Status>;

            async fn list_indices(
                &self,
                request: Request<ListIndicesRequest>,
            ) -> Result<Response<ListIndicesResponse>, Status>;
        }
    }

    fn create_test_claims_with_permissions(permissions: Vec<&str>) -> Claims {
        let mut claims = Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1234567890 + 3600, // exp: 1 hour from now
            1234567890,        // iat: now
        );

        let permissions_json = json!(permissions);
        let mut extras = std::collections::HashMap::new();
        extras.insert("permissions".to_string(), permissions_json);
        claims.add_extras(extras);

        claims
    }

    fn create_test_claims_without_permissions() -> Claims {
        Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1234567890 + 3600, // exp: 1 hour from now
            1234567890,        // iat: now
        )
    }

    #[tokio::test]
    async fn test_describe_with_valid_permissions() {
        let mut mock_service = MockIndexServiceImpl::new();
        mock_service
            .expect_describe()
            .times(1)
            .returning(|_| Ok(Response::new(DescribeResponse { index: None })));

        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:index:v1:{}:read",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(DescribeRequest {
            name: "test-index".to_string(),
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.describe(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_describe_without_permissions() {
        let mock_service = MockIndexServiceImpl::new();
        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_without_permissions();

        let mut request = Request::new(DescribeRequest {
            name: "test-index".to_string(),
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.describe(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn test_describe_without_claims() {
        let mock_service = MockIndexServiceImpl::new();
        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));

        let request = Request::new(DescribeRequest {
            name: "test-index".to_string(),
        });

        let result = authorizor.describe(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_create_index_with_valid_permissions() {
        let mut mock_service = MockIndexServiceImpl::new();
        mock_service
            .expect_create_index()
            .times(1)
            .returning(|_| Ok(Response::new(CreateIndexResponse { index: None })));

        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec!["udex:index:v1:create"]);

        let mut request = Request::new(CreateIndexRequest {
            name: "test-index".to_string(),
            description: "Test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_index(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_create_index_without_permissions() {
        let mock_service = MockIndexServiceImpl::new();
        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:index:v1:{}:write",
            "test-index"
        )
        .as_str()]); // wrong permission

        let mut request = Request::new(CreateIndexRequest {
            name: "test-index".to_string(),
            description: "Test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_index(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn test_update_index_with_valid_permissions() {
        let mut mock_service = MockIndexServiceImpl::new();
        mock_service
            .expect_update_index()
            .times(1)
            .returning(|_| Ok(Response::new(UpdateIndexResponse { index: None })));

        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:index:v1:{}:write",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(UpdateIndexRequest {
            name: "test-index".to_string(),
            update: None,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.update_index(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_list_indices_with_valid_permissions() {
        let mut mock_service = MockIndexServiceImpl::new();
        mock_service
            .expect_list_indices()
            .times(1)
            .returning(|_| Ok(Response::new(ListIndicesResponse { indices: vec![] })));

        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec!["udex:index:v1:list"]);

        let mut request = Request::new(ListIndicesRequest {});
        request.extensions_mut().insert(claims);

        let result = authorizor.list_indices(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_permissions() {
        let mut mock_service = MockIndexServiceImpl::new();
        mock_service
            .expect_create_index()
            .times(1)
            .returning(|_| Ok(Response::new(CreateIndexResponse { index: None })));

        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![
            format!("udex:index:v1:{}:read", "test-index").as_str(),
            "udex:index:v1:create",
            format!("udex:index:v1:{}:write", "test-index").as_str(),
        ]);

        let mut request = Request::new(CreateIndexRequest {
            name: "test-index".to_string(),
            description: "Test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_index(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wrong_permission_type() {
        let mock_service = MockIndexServiceImpl::new();
        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:read",
            "test-index"
        )
        .as_str()]); // wrong service

        let mut request = Request::new(CreateIndexRequest {
            name: "test-index".to_string(),
            description: "Test index".to_string(),
            max_bulk_operations: 100,
            max_key_length: 256,
            max_value_length: 1024,
            max_kv_pairs_per_context: 50,
            hash_algorithm: HashAlgorithm::Xxh3 as i32,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_index(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn test_wrong_index_in_permissions() {
        let mock_service = MockIndexServiceImpl::new();
        let authorizor = IndexServiceAuthorizor::new(Arc::new(mock_service));

        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:index:v1:{}:read",
            "wrong-index"
        )
        .as_str()]);

        let mut request = Request::new(DescribeRequest {
            name: "test-index".to_string(),
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.describe(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }
}
