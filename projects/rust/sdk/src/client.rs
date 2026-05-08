use std::path::PathBuf;

use tonic::transport::{Certificate, Channel, ClientTlsConfig};

use crate::auth::TokenManager;
use crate::error::Error;

/// Options for connecting to a Udex server.
///
/// Build with [`ClientOptions::builder()`].
#[derive(Debug, Clone)]
pub struct ClientOptions {
    pub(crate) endpoint: String,
    pub(crate) ca_cert: CaCert,
    pub(crate) credentials: Option<ClientCredentials>,
}

#[derive(Debug, Clone, Default)]
pub(crate) enum CaCert {
    /// Use the system trust store.
    #[default]
    System,
    /// PEM bytes loaded from a file path.
    PemFile(PathBuf),
    /// PEM bytes provided directly.
    PemBytes(Vec<u8>),
}

/// OAuth2 client-credentials configuration.
#[derive(Debug, Clone)]
pub(crate) struct ClientCredentials {
    pub(crate) token_url: String,
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) audience: Option<String>,
    pub(crate) scope: Option<String>,
}

impl ClientOptions {
    /// Returns a builder for `ClientOptions`.
    pub fn builder() -> ClientOptionsBuilder {
        ClientOptionsBuilder::default()
    }
}

/// Builder for [`ClientOptions`].
#[derive(Debug, Default)]
pub struct ClientOptionsBuilder {
    endpoint: Option<String>,
    ca_cert: CaCert,
    credentials: Option<ClientCredentials>,
}

impl ClientOptionsBuilder {
    /// Sets the server endpoint URL (e.g. `https://localhost:50051`).
    pub fn endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint = Some(url.into());
        self
    }

    /// Reads the CA certificate from a PEM file at the given path.
    pub fn ca_cert_pem_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_cert = CaCert::PemFile(path.into());
        self
    }

    /// Uses the provided PEM-encoded CA certificate bytes directly.
    pub fn ca_cert_pem_bytes(mut self, pem: impl Into<Vec<u8>>) -> Self {
        self.ca_cert = CaCert::PemBytes(pem.into());
        self
    }

    /// Configures OAuth2 client-credentials authentication.
    pub fn client_credentials(
        mut self,
        token_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        self.credentials = Some(ClientCredentials {
            token_url: token_url.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            audience: None,
            scope: None,
        });
        self
    }

    /// Adds an `audience` claim to the token request.
    pub fn audience(mut self, audience: impl Into<String>) -> Self {
        if let Some(creds) = &mut self.credentials {
            creds.audience = Some(audience.into());
        }
        self
    }

    /// Adds a `scope` to the token request.
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        if let Some(creds) = &mut self.credentials {
            creds.scope = Some(scope.into());
        }
        self
    }

    /// Builds [`ClientOptions`], returning an error if required fields are missing.
    pub fn build(self) -> Result<ClientOptions, Error> {
        let endpoint = self
            .endpoint
            .ok_or_else(|| Error::InvalidOptions("endpoint is required".into()))?;
        Ok(ClientOptions {
            endpoint,
            ca_cert: self.ca_cert,
            credentials: self.credentials,
        })
    }
}

/// A connected Udex client.
///
/// Instantiate with [`UdexClient::connect`].
#[derive(Clone)]
pub struct UdexClient {
    // Used by entry/index service wrappers added in WP-04.
    #[allow(dead_code)]
    pub(crate) channel: Channel,
    #[allow(dead_code)]
    pub(crate) token_manager: Option<TokenManager>,
}

impl UdexClient {
    /// Connects to the Udex server described by `opts`.
    ///
    /// Builds a TLS channel and, if OAuth2 credentials are configured, performs
    /// the initial token fetch so the first RPC does not pay the latency cost.
    pub async fn connect(opts: ClientOptions) -> Result<Self, Error> {
        let tls = match &opts.ca_cert {
            CaCert::System => ClientTlsConfig::new(),
            CaCert::PemFile(path) => {
                let pem = tokio::fs::read(path).await?;
                ClientTlsConfig::new().ca_certificate(Certificate::from_pem(pem))
            }
            CaCert::PemBytes(bytes) => {
                ClientTlsConfig::new().ca_certificate(Certificate::from_pem(bytes))
            }
        };

        let channel = Channel::from_shared(opts.endpoint.clone())
            .map_err(|_| Error::InvalidOptions(format!("invalid endpoint URL: {}", opts.endpoint)))?
            .tls_config(tls)?
            .connect()
            .await?;

        let token_manager = match opts.credentials {
            Some(creds) => {
                let tm = TokenManager::new(
                    creds.token_url,
                    creds.client_id,
                    creds.client_secret,
                    creds.audience,
                    creds.scope,
                );
                // Eagerly fetch so first RPC is not delayed.
                tm.token().await?;
                Some(tm)
            }
            None => None,
        };

        Ok(UdexClient {
            channel,
            token_manager,
        })
    }
}
