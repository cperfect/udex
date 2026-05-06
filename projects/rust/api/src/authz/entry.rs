use std::sync::Arc;

use crate::authz::{
    claims::Claims,
    permissions::{is_permitted, Permissable},
};
use crate::entry::{
    entry_service_server::EntryService, BulkReadEntryOperationRequest,
    BulkReadEntryOperationResponse, BulkWriteEntryOperationRequest,
    BulkWriteEntryOperationResponse, CreateEntryRequest, CreateEntryResponse, DeleteEntryRequest,
    DeleteEntryResponse, LookupContextByKeyRequest, LookupContextByKeyResponse,
    LookupKeyByContextRequest, LookupKeyByContextResponse,
};

/// Authorizor for EntryService that checks permissions based on Claims for each method call.
/// This exists because getting hold of the message type & content in an interceptor is painful.
/// The authorizor wraps the actual EntryService implementation and checks permissions before delegating the call.
pub struct EntryServiceAuthorizor<E>
where
    E: EntryService + Send + Sync + 'static,
{
    inner: Arc<E>,
}

impl<E> EntryServiceAuthorizor<E>
where
    E: EntryService + Send + Sync + 'static,
{
    pub fn new(inner: Arc<E>) -> Self {
        EntryServiceAuthorizor { inner }
    }
}

#[tonic::async_trait]
impl<E> EntryService for EntryServiceAuthorizor<E>
where
    E: EntryService + Send + Sync + 'static,
{
    /// CreateEntry creates a new entry between a key and context
    async fn create_entry(
        &self,
        request: tonic::Request<CreateEntryRequest>,
    ) -> std::result::Result<tonic::Response<CreateEntryResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims).map_err(tonic::Status::from)? {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.create_entry(request).await
    }

    /// DeleteEntry removes an entry by key.
    async fn delete_entry(
        &self,
        request: tonic::Request<DeleteEntryRequest>,
    ) -> std::result::Result<tonic::Response<DeleteEntryResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims).map_err(tonic::Status::from)? {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.delete_entry(request).await
    }

    /// LookupContextByKey retrieves the context for a given key
    async fn lookup_context_by_key(
        &self,
        request: tonic::Request<LookupContextByKeyRequest>,
    ) -> std::result::Result<tonic::Response<LookupContextByKeyResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims).map_err(tonic::Status::from)? {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.lookup_context_by_key(request).await
    }

    /// LookupKeyByContext retrieves the single key for a given context, if one exists.
    async fn lookup_key_by_context(
        &self,
        request: tonic::Request<LookupKeyByContextRequest>,
    ) -> std::result::Result<tonic::Response<LookupKeyByContextResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims).map_err(tonic::Status::from)? {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.lookup_key_by_context(request).await
    }

    /// BulkWriteEntryOperation performs multiple write operations in a single transaction
    /// If any operation fails, all operations are rolled back
    async fn bulk_write_entry_operation(
        &self,
        request: tonic::Request<BulkWriteEntryOperationRequest>,
    ) -> std::result::Result<tonic::Response<BulkWriteEntryOperationResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims).map_err(tonic::Status::from)? {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.bulk_write_entry_operation(request).await
    }

    /// BulkReadEntryOperation performs multiple read operations
    async fn bulk_read_entry_operation(
        &self,
        request: tonic::Request<BulkReadEntryOperationRequest>,
    ) -> std::result::Result<tonic::Response<BulkReadEntryOperationResponse>, tonic::Status> {
        // Extract claims from request extensions
        let claims = request
            .extensions()
            .get::<Claims>()
            .ok_or_else(|| tonic::Status::unauthenticated("No claims found in request"))?;

        // Check permissions
        if !is_permitted(request.get_ref(), claims).map_err(tonic::Status::from)? {
            return Err(tonic::Status::permission_denied("Insufficient permissions"));
        }

        self.inner.bulk_read_entry_operation(request).await
    }
}

impl Permissable<CreateEntryRequest> for CreateEntryRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:entry:v1:{}:create", self.index_name)]
    }
}

impl Permissable<DeleteEntryRequest> for DeleteEntryRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:entry:v1:{}:delete", self.index_name)]
    }
}

impl Permissable<LookupContextByKeyRequest> for LookupContextByKeyRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:entry:v1:{}:read", self.index_name)]
    }
}

impl Permissable<LookupKeyByContextRequest> for LookupKeyByContextRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:entry:v1:{}:read", self.index_name)]
    }
}

impl Permissable<BulkWriteEntryOperationRequest> for BulkWriteEntryOperationRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:entry:v1:{}:write", self.index_name)]
    }
}

impl Permissable<BulkReadEntryOperationRequest> for BulkReadEntryOperationRequest {
    fn required_permissions(&self) -> Vec<String> {
        vec![format!("udex:entry:v1:{}:read", self.index_name)]
    }
}

#[cfg(test)]
#[allow(clippy::result_large_err)]
mod tests {
    use super::*;
    use mockall::mock;
    use tonic::{Request, Response, Status};

    // Mock the EntryService trait
    mock! {
        pub EntryServiceImpl {}

        #[tonic::async_trait]
        impl EntryService for EntryServiceImpl {
            async fn create_entry(
                &self,
                request: Request<CreateEntryRequest>,
            ) -> Result<Response<CreateEntryResponse>, Status>;

            async fn delete_entry(
                &self,
                request: Request<DeleteEntryRequest>,
            ) -> Result<Response<DeleteEntryResponse>, Status>;

            async fn lookup_context_by_key(
                &self,
                request: Request<LookupContextByKeyRequest>,
            ) -> Result<Response<LookupContextByKeyResponse>, Status>;

            async fn lookup_key_by_context(
                &self,
                request: Request<LookupKeyByContextRequest>,
            ) -> Result<Response<LookupKeyByContextResponse>, Status>;

            async fn bulk_write_entry_operation(
                &self,
                request: Request<BulkWriteEntryOperationRequest>,
            ) -> Result<Response<BulkWriteEntryOperationResponse>, Status>;

            async fn bulk_read_entry_operation(
                &self,
                request: Request<BulkReadEntryOperationRequest>,
            ) -> Result<Response<BulkReadEntryOperationResponse>, Status>;
        }
    }

    fn create_test_claims_with_permissions(permissions: Vec<&str>) -> Claims {
        Claims::new(
            "test-user".to_string(),
            "test-issuer".to_string(),
            "test-audience".to_string(),
            1234567890 + 3600,
            1234567890,
        )
        .with_scope(permissions.join(" "))
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
    async fn test_create_entry_with_valid_permissions() {
        let mut mock_service = MockEntryServiceImpl::new();
        mock_service.expect_create_entry().times(1).returning(|_| {
            Ok(Response::new(CreateEntryResponse {
                key: "test-key".to_string(),
                context_hash: String::new(),
            }))
        });

        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:create",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(CreateEntryRequest {
            index_name: "test-index".to_string(),
            context: None,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_entry(request).await;
        assert!(result.is_ok());
        assert_eq!(result.unwrap().into_inner().key, "test-key");
    }

    #[tokio::test]
    async fn test_create_entry_without_permissions() {
        let mock_service = MockEntryServiceImpl::new();
        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_without_permissions();

        let mut request = Request::new(CreateEntryRequest {
            index_name: "test-index".to_string(),
            context: None,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_entry(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn test_create_entry_without_claims() {
        let mock_service = MockEntryServiceImpl::new();
        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));

        let request = Request::new(CreateEntryRequest {
            index_name: "test-index".to_string(),
            context: None,
        });

        let result = authorizor.create_entry(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::Unauthenticated);
    }

    #[tokio::test]
    async fn test_delete_entry_with_valid_permissions() {
        let mut mock_service = MockEntryServiceImpl::new();
        mock_service
            .expect_delete_entry()
            .times(1)
            .returning(|_| Ok(Response::new(DeleteEntryResponse {})));

        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:delete",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(DeleteEntryRequest {
            index_name: "test-index".to_string(),
            key: "test-key".to_string(),
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.delete_entry(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_delete_entry_without_permissions() {
        let mock_service = MockEntryServiceImpl::new();
        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec!["udex:entry:v1:read"]); // wrong permission

        let mut request = Request::new(DeleteEntryRequest {
            index_name: "test-index".to_string(),
            key: "test-key".to_string(),
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.delete_entry(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn test_lookup_context_by_key_with_valid_permissions() {
        let mut mock_service = MockEntryServiceImpl::new();
        mock_service
            .expect_lookup_context_by_key()
            .times(1)
            .returning(|_| {
                Ok(Response::new(LookupContextByKeyResponse {
                    index_name: String::new(),
                    context: None,
                }))
            });

        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:read",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(LookupContextByKeyRequest {
            index_name: "test-index".to_string(),
            key: "test-key".to_string(),
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.lookup_context_by_key(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_lookup_key_by_context_with_valid_permissions() {
        let mut mock_service = MockEntryServiceImpl::new();
        mock_service
            .expect_lookup_key_by_context()
            .times(1)
            .returning(|_| Ok(Response::new(LookupKeyByContextResponse { key: None })));

        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:read",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(LookupKeyByContextRequest {
            index_name: "test-index".to_string(),
            context_hash: "test-hash".to_string(),
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.lookup_key_by_context(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_bulk_write_entry_operation_with_valid_permissions() {
        let mut mock_service = MockEntryServiceImpl::new();
        mock_service
            .expect_bulk_write_entry_operation()
            .times(1)
            .returning(|_| {
                Ok(Response::new(BulkWriteEntryOperationResponse {
                    results: vec![],
                }))
            });

        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:write",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(BulkWriteEntryOperationRequest {
            index_name: "test-index".to_string(),
            operations: vec![],
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.bulk_write_entry_operation(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_bulk_read_entry_operation_with_valid_permissions() {
        let mut mock_service = MockEntryServiceImpl::new();
        mock_service
            .expect_bulk_read_entry_operation()
            .times(1)
            .returning(|_| {
                Ok(Response::new(BulkReadEntryOperationResponse {
                    results: vec![],
                }))
            });

        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:read",
            "test-index"
        )
        .as_str()]);

        let mut request = Request::new(BulkReadEntryOperationRequest {
            index_name: "test-index".to_string(),
            operations: vec![],
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.bulk_read_entry_operation(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_multiple_permissions() {
        let mut mock_service = MockEntryServiceImpl::new();
        mock_service.expect_create_entry().times(1).returning(|_| {
            Ok(Response::new(CreateEntryResponse {
                key: "test-key".to_string(),
                context_hash: String::new(),
            }))
        });

        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![
            format!("udex:entry:v1:{}:read", "test-index").as_str(),
            format!("udex:entry:v1:{}:create", "test-index").as_str(),
            format!("udex:entry:v1:{}:write", "test-index").as_str(),
        ]);

        let mut request = Request::new(CreateEntryRequest {
            index_name: "test-index".to_string(),
            context: None,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_entry(request).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_wrong_permission_type() {
        let mock_service = MockEntryServiceImpl::new();
        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:index:v1:{}:read",
            "test-index"
        )
        .as_str()]); // wrong service

        let mut request = Request::new(CreateEntryRequest {
            index_name: "test-index".to_string(),
            context: None,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_entry(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }

    #[tokio::test]
    async fn test_wrong_index_in_permission() {
        let mock_service = MockEntryServiceImpl::new();
        let authorizor = EntryServiceAuthorizor::new(Arc::new(mock_service));
        let claims = create_test_claims_with_permissions(vec![format!(
            "udex:entry:v1:{}:read",
            "wrong-index"
        )
        .as_str()]);

        let mut request = Request::new(CreateEntryRequest {
            index_name: "test-index".to_string(),
            context: None,
        });
        request.extensions_mut().insert(claims);

        let result = authorizor.create_entry(request).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code(), tonic::Code::PermissionDenied);
    }
}
