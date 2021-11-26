use std::sync::mpsc::{channel, Sender};
use rand::Rng;
use anyhow::Result;
use crate::env::Env;
use crate::oauth2::server::start_actix;
use log::{info, debug};
use serde::Serialize;

mod server;
mod port;
mod google_api;

pub fn do_oauth(env: Env) -> Result<String> {
    debug!("Generating code verified & challenge and state for OAuth2 login");
    let (verifier, challenge) = generate_code();
    let state: String = rand::thread_rng().sample_iter(rand::distributions::Alphanumeric).take(32).map(char::from).collect();
    let port = get_port();

    let (tx_endpoint, rx_endpoint) = channel();
    let actix_data = WebData {
        tx_endpoint,
        state: state.clone()
    };

    let (tx_actix, rx_actix) = channel();
    debug!("Starting Actix server");
    std::thread::spawn(move || {
        match start_actix(actix_data, tx_actix, port) {
            Ok(_) => {},
            Err(e) => panic!("Failed to start Actix web server: {:?}", e)
        };
    });

    debug!("Waiting for the Actix server to be started");
    let actix_server = rx_actix.recv()?;
    let auth_uri = create_authentication_uri(&env, &challenge, &state, &format!("http://localhost:{}", port));

    info!("Please open the following URL to log in: {}", &auth_uri);

    debug!("Waiting for user to complete login flow");
    let code = rx_endpoint.recv()?;
    debug!("User has completed the login flow");

    debug!("Stopping Actix web server");
    actix_web::rt::System::new("").block_on(actix_server.stop(true));

    debug!("Exchanging received code for access token");
    let resp = google_api::exchange_access_token(&env, &code, &verifier, &format!("http://localhost:{}", port))?;
    Ok(resp.access_token)
}

#[derive(Clone)]
pub struct WebData {
    state: String,
    tx_endpoint: Sender<String>,
}

/// Generate a code_verifier and code_challenge
fn generate_code() -> (String, String) {
    loop {
        let code_verifier: String = rand::thread_rng().sample_iter(rand::distributions::Alphanumeric).take(96).map(char::from).collect();
        let code_challenge = {
            use sha2::digest::Digest;

            let mut hasher = sha2::Sha256::new();
            hasher.update(code_verifier.as_bytes());
            let digest = hasher.finalize();
            base64::encode(digest.as_slice())
        };

        if code_challenge.contains('+') || code_challenge.contains('/') {
            continue;
        }

        return (code_verifier, code_challenge.replace("=", ""))
    }
}

/// Struct describing an authentication request
#[derive(Serialize)]
struct AuthenticationRequest<'a> {
    /// Application's client ID
    client_id:              &'a str,

    /// The original redirect URI
    redirect_uri:           &'a str,

    /// The response type
    response_type:          &'static str,

    /// The scopes requested
    scope:                  &'static str,

    /// The challenge halve of the code challenge
    code_challenge:         &'a str,

    /// The method of code challenge
    code_challenge_method:  &'static str,

    /// State parameter
    state:                  &'a str,
}

/// Create an authentication URL used for step 1 in the OAuth2 flow
pub fn create_authentication_uri(env: &Env, code_challenge: &str, state: &str, redirect_uri: &str) -> String {
    let auth_request = AuthenticationRequest {
        client_id:              &env.google_client_id,
        redirect_uri,
        response_type:          "code",
        scope:                  "https://www.googleapis.com/auth/youtube",
        code_challenge:         &code_challenge,
        code_challenge_method:  "S256",
        state:                  &state
    };

    let qstring = serde_qs::to_string(&auth_request).unwrap();
    format!("https://accounts.google.com/o/oauth2/v2/auth?{}", qstring)
}

fn get_port() -> u16 {
    let port = {
        let mut port = rand::thread_rng().gen_range(4000..8000) as u16;
        while !port::is_free(port) {
            port = rand::thread_rng().gen_range(4000..8000) as u16;
        }

        port
    };

    port
}
