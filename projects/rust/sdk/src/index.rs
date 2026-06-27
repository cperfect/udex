use udex_api::index::index_service_client::IndexServiceClient;
use udex_api::index::{
    CreateIndexRequest, CreateIndexResponse, DeleteIndexRequest, DescribeRequest, Index,
    IndexUpdate, ListIndicesRequest, UpdateIndexRequest,
};

use crate::client::{make_auth_interceptor, UdexClient};
use crate::error::Error;

impl UdexClient {
    /// Retrieves the [`Index`] definition for `name`.
    ///
    /// Returns [`Error::Rpc`] with status `NOT_FOUND` if the index does not
    /// exist, or [`Error::InvalidResponse`] if the server omits the index
    /// payload in a successful response.
    #[tracing::instrument(name = "sdk.describe_index", skip_all, fields(index = %name))]
    pub async fn describe_index(&self, name: &str) -> Result<Index, Error> {
        let mut client = self.index_client().await?;
        let resp = client
            .describe(DescribeRequest {
                name: name.to_owned(),
            })
            .await?
            .into_inner();
        resp.index
            .ok_or_else(|| Error::InvalidResponse("server returned empty index".to_owned()))
    }

    /// Creates a new index from `req` and returns the created [`Index`].
    ///
    /// Returns [`Error::InvalidResponse`] if the server omits the index payload
    /// in a successful response.
    #[tracing::instrument(name = "sdk.create_index", skip_all, fields(index = %req.name))]
    pub async fn create_index(&self, req: CreateIndexRequest) -> Result<Index, Error> {
        let mut client = self.index_client().await?;
        let resp: CreateIndexResponse = client.create_index(req).await?.into_inner();
        resp.index
            .ok_or_else(|| Error::InvalidResponse("server returned empty index".to_owned()))
    }

    /// Applies `update` to the index named `name` and returns the updated [`Index`].
    ///
    /// Returns [`Error::InvalidResponse`] if the server omits the index payload
    /// in a successful response.
    #[tracing::instrument(name = "sdk.update_index", skip_all, fields(index = %name))]
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
            .ok_or_else(|| Error::InvalidResponse("server returned empty index".to_owned()))
    }

    /// Deletes the index named `name`.
    ///
    /// Returns [`Error::Rpc`] with status `FAILED_PRECONDITION` if the index
    /// still has entries, or `NOT_FOUND` if the index does not exist.
    #[tracing::instrument(name = "sdk.delete_index", skip_all, fields(index = %name))]
    pub async fn delete_index(&self, name: &str) -> Result<(), Error> {
        let mut client = self.index_client().await?;
        client
            .delete_index(DeleteIndexRequest {
                name: name.to_owned(),
            })
            .await?;
        Ok(())
    }

    /// Lists all indices accessible to the caller.
    #[tracing::instrument(name = "sdk.list_indices", skip_all)]
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
