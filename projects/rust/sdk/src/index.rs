use udex_api::index::index_service_client::IndexServiceClient;
use udex_api::index::{
    CreateIndexRequest, CreateIndexResponse, DescribeRequest, Index, IndexUpdate,
    ListIndicesRequest, UpdateIndexRequest,
};

use crate::client::UdexClient;
use crate::entry::make_auth_interceptor;
use crate::error::Error;

impl UdexClient {
    /// Retrieves the [`Index`] definition for `name`.
    ///
    /// Returns [`Error::Rpc`] with status `NOT_FOUND` if the index does not exist.
    pub async fn describe_index(&self, name: &str) -> Result<Index, Error> {
        let mut client = self.index_client().await?;
        let resp = client
            .describe(DescribeRequest {
                name: name.to_owned(),
            })
            .await?
            .into_inner();
        resp.index
            .ok_or_else(|| tonic::Status::internal("server returned empty index").into())
    }

    /// Creates a new index from `req` and returns the created [`Index`].
    pub async fn create_index(&self, req: CreateIndexRequest) -> Result<Index, Error> {
        let mut client = self.index_client().await?;
        let resp: CreateIndexResponse = client.create_index(req).await?.into_inner();
        resp.index
            .ok_or_else(|| tonic::Status::internal("server returned empty index").into())
    }

    /// Applies `update` to the index named `name` and returns the updated [`Index`].
    pub async fn update_index(&self, name: &str, update: IndexUpdate) -> Result<Index, Error> {
        let mut client = self.index_client().await?;
        let resp = client
            .update_index(UpdateIndexRequest {
                name: name.to_owned(),
                update: Some(update),
            })
            .await?
            .into_inner();
        resp.index
            .ok_or_else(|| tonic::Status::internal("server returned empty index").into())
    }

    /// Lists all indices accessible to the caller.
    pub async fn list_indices(&self) -> Result<Vec<Index>, Error> {
        let mut client = self.index_client().await?;
        let resp = client
            .list_indices(ListIndicesRequest {})
            .await?
            .into_inner();
        Ok(resp.indices)
    }

    /// Returns a tonic index-service client with the Bearer token injected.
    async fn index_client(
        &self,
    ) -> Result<
        IndexServiceClient<
            tonic::service::interceptor::InterceptedService<
                tonic::transport::Channel,
                impl tonic::service::Interceptor,
            >,
        >,
        Error,
    > {
        let token = self.bearer_token().await?;
        let client = IndexServiceClient::with_interceptor(
            self.channel.clone(),
            make_auth_interceptor(token),
        );
        Ok(client)
    }
}
