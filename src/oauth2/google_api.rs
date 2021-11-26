use crate::env::Env;
use anyhow::Result;
use serde::{Serialize, Deserialize};

/// Struct describing the request to exchange an access code for an access token
#[derive(Serialize)]
struct ExchangeAccessTokenRequest<'a> {
    /// The application's client ID
    client_id:          &'a str,

    /// The application's client secret
    client_secret:      &'a str,

    /// The access code
    code:               &'a str,

    /// The verifier halve of the code challenge
    code_verifier:      &'a str,

    /// The grant type
    grant_type:         &'static str,

    /// The original redirect URI
    redirect_uri:       &'a str
}

/// Struct describing the response to an access token exchange request
#[derive(Deserialize)]
pub struct ExchangeAccessTokenResponse {
    /// The access token
    pub access_token:   String,
}

/// Exchange an access code for an access token
///
/// ## Errors
/// - Google API error
/// - Reqwest error
pub fn exchange_access_token(env: &Env, access_token: &str, code_verifier: &str, redirect_uri: &str) -> Result<ExchangeAccessTokenResponse> {

    //We can now exchange this token for a refresh_token and the likes
    let exchange_request = ExchangeAccessTokenRequest {
        client_id: &env.google_client_id,
        client_secret: &env.google_client_secret,
        code: access_token,
        code_verifier,
        grant_type: "authorization_code",
        redirect_uri
    };

    // Send a request to Google to exchange the code for the necessary codes
    let response = reqwest::blocking::Client::new().post("https://oauth2.googleapis.com/token")
        .json(&exchange_request)
        .send()?;

    // Deserialize from JSON
    let exchange_response: ExchangeAccessTokenResponse = response.json()?;

    Ok(exchange_response)
}