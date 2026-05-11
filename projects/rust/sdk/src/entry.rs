use udex_api::entry::entry_service_client::EntryServiceClient;
use udex_api::entry::{
    BulkReadEntryOperation, BulkReadEntryOperationRequest, BulkReadEntryOperationResult,
    BulkWriteEntryOperation, BulkWriteEntryOperationRequest, BulkWriteEntryOperationResult,
    Context, ContextInput, CreateEntryRequest, CreateEntryResponse, DeleteEntryRequest,
    LookupContextByKeyRequest, LookupKeyByContextRequest,
};

use crate::client::{make_auth_interceptor, UdexClient};
use crate::error::Error;

impl UdexClient {
    /// Creates a new entry for `context` in `index_name`.
    ///
    /// Idempotent: if an entry already exists for the given context within the
    /// index, the existing key and hash are returned unchanged.
    pub async fn create_entry(
        &self,
        index_name: &str,
        context: ContextInput,
    ) -> Result<CreateEntryResponse, Error> {
        let mut client = self.entry_client().await?;
        let resp = client
            .create_entry(CreateEntryRequest {
                index_name: index_name.to_owned(),
                context: Some(context),
            })
            .await?
            .into_inner();
        Ok(resp)
    }

    /// Deletes the entry identified by `key` from `index_name`.
    pub async fn delete_entry(&self, index_name: &str, key: &str) -> Result<(), Error> {
        let mut client = self.entry_client().await?;
        client
            .delete_entry(DeleteEntryRequest {
                index_name: index_name.to_owned(),
                key: key.to_owned(),
            })
            .await?;
        Ok(())
    }

    /// Retrieves the [`Context`] associated with `key` in `index_name`.
    ///
    /// Returns [`Error::Rpc`] with status `NOT_FOUND` if no entry exists for
    /// `key`, or [`Error::InvalidResponse`] if the server omits the context
    /// payload in a successful response.
    pub async fn lookup_context_by_key(
        &self,
        index_name: &str,
        key: &str,
    ) -> Result<Context, Error> {
        let mut client = self.entry_client().await?;
        let resp = client
            .lookup_context_by_key(LookupContextByKeyRequest {
                index_name: index_name.to_owned(),
                key: key.to_owned(),
            })
            .await?
            .into_inner();
        resp.context
            .ok_or_else(|| Error::InvalidResponse("server returned empty context".to_owned()))
    }

    /// Reverse-looks up the entry key for the given pre-computed `context_hash` in `index_name`.
    ///
    /// Returns `None` when no entry exists for the hash.
    pub async fn lookup_key_by_context(
        &self,
        index_name: &str,
        context_hash: &str,
    ) -> Result<Option<String>, Error> {
        let mut client = self.entry_client().await?;
        let resp = client
            .lookup_key_by_context(LookupKeyByContextRequest {
                index_name: index_name.to_owned(),
                context_hash: context_hash.to_owned(),
            })
            .await?
            .into_inner();
        Ok(resp.key)
    }

    /// Executes multiple write operations in a single transaction.
    ///
    /// All operations share `index_name`. Results are returned in the same
    /// order as the input `operations`.
    pub async fn bulk_write(
        &self,
        index_name: &str,
        operations: Vec<BulkWriteEntryOperation>,
    ) -> Result<Vec<BulkWriteEntryOperationResult>, Error> {
        let mut client = self.entry_client().await?;
        let resp = client
            .bulk_write_entry_operation(BulkWriteEntryOperationRequest {
                index_name: index_name.to_owned(),
                operations,
            })
            .await?
            .into_inner();
        Ok(resp.results)
    }

    /// Executes multiple read operations, returning results in input order.
    pub async fn bulk_read(
        &self,
        index_name: &str,
        operations: Vec<BulkReadEntryOperation>,
    ) -> Result<Vec<BulkReadEntryOperationResult>, Error> {
        let mut client = self.entry_client().await?;
        let resp = client
            .bulk_read_entry_operation(BulkReadEntryOperationRequest {
                index_name: index_name.to_owned(),
                operations,
            })
            .await?
            .into_inner();
        Ok(resp.results)
    }

    /// Returns a tonic entry-service client with the Bearer token injected.
    async fn entry_client(
        &self,
    ) -> Result<
        EntryServiceClient<
            tonic::service::interceptor::InterceptedService<
                tonic::transport::Channel,
                impl tonic::service::Interceptor,
            >,
        >,
        Error,
    > {
        let token = self.bearer_token().await?;
        let client = EntryServiceClient::with_interceptor(
            self.channel.clone(),
            make_auth_interceptor(token),
        );
        Ok(client)
    }
}
