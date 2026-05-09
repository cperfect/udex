use std::fmt;
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
    pub(crate) auth: AuthConfig,
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

/// Authentication configuration supplied via [`ClientOptionsBuilder`].
#[derive(Clone, Default)]
pub(crate) enum AuthConfig {
    /// No `Authorization` header injected.
    #[default]
    None,
    /// A pre-fetched bearer token injected verbatim on every request.
    StaticToken(String),
    /// OAuth2 client-credentials: tokens fetched and refreshed transparently.
    ClientCredentials(ClientCredentials),
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthConfig::None => write!(f, "None"),
            AuthConfig::StaticToken(_) => write!(f, "StaticToken([REDACTED])"),
            AuthConfig::ClientCredentials(creds) => {
                f.debug_tuple("ClientCredentials").field(creds).finish()
            }
        }
    }
}

/// OAuth2 client-credentials configuration.
#[derive(Clone)]
pub(crate) struct ClientCredentials {
    pub(crate) token_url: String,
    pub(crate) client_id: String,
    pub(crate) client_secret: String,
    pub(crate) audience: Option<String>,
    pub(crate) scope: Option<String>,
}

impl fmt::Debug for ClientCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientCredentials")
            .field("token_url", &self.token_url)
            .field("client_id", &"[REDACTED]")
            .field("client_secret", &"[REDACTED]")
            .field("audience", &self.audience)
            .field("scope", &self.scope)
            .finish()
    }
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
    auth: AuthConfig,
}

impl ClientOptionsBuilder {
    /// Sets the server endpoint URL (e.g. `https://localhost:50051`).
    pub fn endpoint(mut self, url: impl Into<String>) -> Self {
        self.endpoint = Some(url.into());
        self
    }

    /// Reads the CA certificate from a PEM file at the given path.
    ///
    /// **Development only.** In production the system trust store is used. Set
    /// this only when connecting to a dev/test deployment with a self-signed or
    /// private-CA certificate.
    pub fn ca_cert_pem_file(mut self, path: impl Into<PathBuf>) -> Self {
        self.ca_cert = CaCert::PemFile(path.into());
        self
    }

    /// Uses the provided PEM-encoded CA certificate bytes directly.
    ///
    /// **Development only.** See [`Self::ca_cert_pem_file`] for guidance.
    pub fn ca_cert_pem_bytes(mut self, pem: impl Into<Vec<u8>>) -> Self {
        self.ca_cert = CaCert::PemBytes(pem.into());
        self
    }

    /// Injects a pre-fetched bearer token on every request.
    ///
    /// **Development only.** Use this in tests or local scripts where bypassing
    /// the auth server is intentional. For production workloads use
    /// [`Self::client_credentials`] instead.
    pub fn static_bearer_token(mut self, token: impl Into<String>) -> Self {
        self.auth = AuthConfig::StaticToken(token.into());
        self
    }

    /// Configures OAuth2 client-credentials authentication.
    ///
    /// Tokens are fetched from `token_url` and refreshed automatically.
    /// Chain [`Self::audience`] and [`Self::scope`] to add those parameters.
    pub fn client_credentials(
        mut self,
        token_url: impl Into<String>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Self {
        self.auth = AuthConfig::ClientCredentials(ClientCredentials {
            token_url: token_url.into(),
            client_id: client_id.into(),
            client_secret: client_secret.into(),
            audience: None,
            scope: None,
        });
        self
    }

    /// Adds an `audience` claim to the OAuth2 token request.
    ///
    /// Has no effect if [`Self::client_credentials`] has not been called.
    pub fn audience(mut self, audience: impl Into<String>) -> Self {
        if let AuthConfig::ClientCredentials(creds) = &mut self.auth {
            creds.audience = Some(audience.into());
        }
        self
    }

    /// Adds a `scope` to the OAuth2 token request.
    ///
    /// Has no effect if [`Self::client_credentials`] has not been called.
    pub fn scope(mut self, scope: impl Into<String>) -> Self {
        if let AuthConfig::ClientCredentials(creds) = &mut self.auth {
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
            auth: self.auth,
        })
    }
}

/// Runtime auth state held by a connected [`UdexClient`].
pub(crate) enum AuthState {
    None,
    Static(String),
    Dynamic(TokenManager),
}

impl fmt::Debug for AuthState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AuthState::None => write!(f, "None"),
            AuthState::Static(_) => write!(f, "Static([REDACTED])"),
            AuthState::Dynamic(tm) => f.debug_tuple("Dynamic").field(tm).finish(),
        }
    }
}

/// A connected Udex client.
///
/// Instantiate with [`UdexClient::connect`].
#[derive(Clone, Debug)]
pub struct UdexClient {
    pub(crate) channel: Channel,
    pub(crate) auth: std::sync::Arc<AuthState>,
}

impl Clone for AuthState {
    fn clone(&self) -> Self {
        match self {
            AuthState::None => AuthState::None,
            AuthState::Static(t) => AuthState::Static(t.clone()),
            AuthState::Dynamic(tm) => AuthState::Dynamic(tm.clone()),
        }
    }
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

        let auth = match opts.auth {
            AuthConfig::None => AuthState::None,
            AuthConfig::StaticToken(t) => AuthState::Static(t),
            AuthConfig::ClientCredentials(creds) => {
                let tm = TokenManager::new(
                    creds.token_url,
                    creds.client_id,
                    creds.client_secret,
                    creds.audience,
                    creds.scope,
                );
                // Eagerly fetch so the first RPC does not pay the token-fetch latency.
                tm.token().await?;
                AuthState::Dynamic(tm)
            }
        };

        Ok(UdexClient {
            channel,
            auth: std::sync::Arc::new(auth),
        })
    }

    /// Returns the current bearer token string, or `None` if unauthenticated.
    pub(crate) async fn bearer_token(&self) -> Result<Option<String>, Error> {
        match self.auth.as_ref() {
            AuthState::None => Ok(None),
            AuthState::Static(t) => Ok(Some(t.clone())),
            AuthState::Dynamic(tm) => Ok(Some(tm.token().await?)),
        }
    }
}
