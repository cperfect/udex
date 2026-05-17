use secrets_rs::{sources::file::FileSource, Secret, SourceRegistry};

/// Binds a file-sourced `Secret<String>` from an absolute path.
pub fn bind_file_secret(abs_path: &str) -> Secret<String> {
    let mut s = Secret::new(&format!("urn:secrets-rs:file:{abs_path}")).expect("valid file URN");
    let mut reg = SourceRegistry::new();
    reg.register("file", FileSource::new())
        .expect("register file source");
    s.bind(&reg).expect("bind file secret");
    s
}

/// Returns the Hydra public URL from `HYDRA_PUBLIC_URL` env var, defaulting to `http://localhost:4444`.
pub fn hydra_public_url() -> String {
    dotenvy::dotenv_override().ok();
    std::env::var("HYDRA_PUBLIC_URL").unwrap_or_else(|_| "http://localhost:4444".to_string())
}

/// Returns the Hydra admin URL from `HYDRA_ADMIN_URL` env var, defaulting to `http://localhost:4445`.
pub fn hydra_admin_url() -> String {
    dotenvy::dotenv_override().ok();
    std::env::var("HYDRA_ADMIN_URL").unwrap_or_else(|_| "http://localhost:4445".to_string())
}

/// Upserts an OAuth2 client in Hydra (create or replace on 409).
pub async fn register_hydra_client(
    admin_url: &str,
    client_id: &str,
    client_secret: &str,
    audience: &str,
    scopes: &str,
) {
    use ory_hydra_client::apis::{configuration::Configuration, o_auth2_api};
    use ory_hydra_client::models::o_auth2_client::OAuth2Client as HydraClient;

    let config = Configuration {
        base_path: admin_url.to_string(),
        ..Configuration::default()
    };

    let mut body = HydraClient::new();
    body.access_token_strategy = Some("jwt".to_string());
    body.audience = Some(vec![audience.to_string()]);
    body.client_id = Some(client_id.to_string());
    body.client_name = Some(client_id.to_string());
    body.client_secret = Some(client_secret.to_string());
    body.grant_types = Some(vec!["client_credentials".to_string()]);
    body.scope = Some(scopes.to_string());
    body.token_endpoint_auth_method = Some("client_secret_post".to_string());

    match o_auth2_api::create_o_auth2_client(&config, body.clone()).await {
        Ok(_) => {}
        Err(e)
            if matches!(
                &e,
                ory_hydra_client::apis::Error::ResponseError(r) if r.status.as_u16() == 409
            ) =>
        {
            o_auth2_api::set_o_auth2_client(&config, client_id, body)
                .await
                .expect("Hydra set_client failed");
        }
        Err(e) => panic!("Hydra create_client failed: {e}"),
    }
}
