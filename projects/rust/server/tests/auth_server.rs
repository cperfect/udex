use std::error::Error;

use oauth2::basic::BasicClient;
use oauth2::{self, reqwest};
use oauth2::{
    AuthUrl,
    ClientId,
    ClientSecret,
    Scope,
    // TokenResponse,
    TokenUrl,
};
use ory_hydra_client::apis::{configuration, o_auth2_api};
use ory_hydra_client::models::{o_auth2_client as hydra_oauth2_client};

const API_CONFIG: configuration::Configuration = configuration::Configuration {
    base_path: "http://hydra:4444".to_string(),
    user_agent: None(),
    client: todo!(),
    basic_auth: todo!(),
    oauth_access_token: todo!(),
    bearer_access_token: todo!(),
    api_key: todo!(),
};

pub struct OAuth2Client {
    name: String,
    id: String,
    secret: String,
    scopes: Vec<String>,
    audience: String,
}

pub async fn authenticate(client: OAuth2Client, scopes: Vec<String>) -> Result<String, Error> {
    // note this isn't hydra specific - it should just be

    // see https://docs.rs/oauth2/5.0.0/oauth2/#example-2
    let auth_client = BasicClient::new(ClientId::new(client.id.to_string()))
        .set_client_secret(ClientSecret::new(client.secret.to_string()))
        .set_auth_uri(AuthUrl::new("http://hydra:4444/authorize".to_string())?)
        .set_token_uri(TokenUrl::new("http://hydra:4444/token".to_string())?);

    let http_client = reqwest::ClientBuilder::new()
        // Following redirects opens the client up to SSRF vulnerabilities.
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .expect("Client should build");

    // just give back the result from the API - we don't need
    // to get fancy for this test code
    auth_client
        .exchange_client_credentials()
        .add_scope(Scope::new(client.scopes.join(" ")))
        .request_async(&http_client).await
}

pub async fn create_oauth2_client(client: OAuth2Client) -> Result<(), Error> {
    let req = hydra_oauth2_client::OAuth2Client::new(); // srart with a default object (basically all props -> None())

    //override the things we need to be something
    req.access_token_strategy = Some("jwt".to_string());
    req.audience = Some(vec![client.audience]);
    req.client_id = Some(client.id);
    req.client_name = Some(client.name);
    req.client_secret = Some(client.secret);
    req.grant_types = Some(vec!["client_credentials".to_string()]);
    req.scope = Some(client.scopes.join(" "));
    req.token_endpoint_auth_method = Some("client_secret_post".to_string());

    // just give back the result from the API - we don't need
    // to get fancy for this test code
    let result = o_auth2_api::create_o_auth2_client(&API_CONFIG, req).await;

    match result {
      Ok(client) => Ok({}),
      Err(e) => Err(e)
    }

    
}
