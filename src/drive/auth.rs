use std::cell::RefCell;
use std::env;
use std::io::{BufRead, BufReader, Write};
use std::ops::Add;
use std::rc::Rc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::net::TcpListener;
use std::fmt::{Display, Formatter};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Result};
use oauth2::{AuthUrl, ClientSecret, CsrfToken, RedirectUrl, RevocationUrl, TokenUrl, Scope, PkceCodeChallenge, TokenResponse, AccessToken, AuthorizationCode, basic, revocation, RefreshToken};
use oauth2::{basic::BasicClient, ClientId};
use oauth2::basic::BasicTokenResponse;
use oauth2::reqwest::http_client;
use serde::{Deserialize, Serialize};
use url::Url;
use crate::config;
use crate::drive::helper::FileCredentials;

// This is a installed app, client secret for OAuth2 is an extension of client id
// So, we can embed it
const DEFAULT_CLIENT_ID: &str = "308241855989-j1avgc71ptfakdihs3uj7pbjjric3bpj.apps.googleusercontent.com";
const DEFAULT_CLIENT_SECRET: &str = "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";

const CLIENT_ID_ENV_KEY: &str = "CLIENT_ID";
const CLIENT_SECRET_ENV_KEY: &str = "CLIENT_SECRET";

const OVERRIDE_CLIENT_ID: Option<&str> = option_env!("CLIENT_ID");
const OVERRIDE_CLIENT_SECRET: Option<&str> = option_env!("CLIENT_SECRET");

const GOOGLE_AUTH_URL: &str = "https://accounts.google.com/o/oauth2/v2/auth";
const GOOGLE_TOKEN_URL: &str = "https://www.googleapis.com/oauth2/v3/token";
const GOOGLE_REVOKE_URL: &str = "https://oauth2.googleapis.com/revoke";
const GOOGLE_DRIVE_SCOPE: &str = "https://www.googleapis.com/auth/drive";

const DEFAULT_LISTEN_PORT: i32 = 18080;

#[derive(Serialize, Deserialize, Debug)]
pub struct Token {
    client_id: String,
    client_secret: String,
    token_response: BasicTokenResponse,
    created_at: u64,
}

impl Token {
    fn new(client_id: &str, client_secret: &str, token_response: BasicTokenResponse, created_at: SystemTime) -> Self {
        Self {
            client_id: String::from(client_id),
            client_secret: String::from(client_secret),
            token_response,
            created_at: created_at.duration_since(UNIX_EPOCH).unwrap().as_secs(),
        }
    }

    fn created_at(&self) -> SystemTime {
        UNIX_EPOCH + Duration::from_secs(self.created_at)
    }
}

type Client = oauth2::Client<basic::BasicErrorResponse, BasicTokenResponse, basic::BasicTokenType, basic::BasicTokenIntrospectionResponse, revocation::StandardRevocableToken, basic::BasicRevocationErrorResponse>;

#[allow(dead_code)]
pub enum CredPath<'a> {
    Path(&'a Path),
    DefaultPath,
}

impl<'a> CredPath<'a> {
    fn path(&self) -> Box<Path> {
        match self {
            CredPath::Path(p) => {
                p.to_path_buf().into_boxed_path()
            }
            CredPath::DefaultPath => {
                let default_path = config::default_path();
                let default_cred_path = Rc::clone(&default_path.cred_path());
                default_cred_path.to_path_buf().into_boxed_path()
            }
        }
    }
}

#[allow(dead_code)]
pub enum ListenPort {
    Port(i32),
    DefaultPort,
}

impl Display for ListenPort {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            ListenPort::Port(port) => write!(f, "{}", port),
            ListenPort::DefaultPort => write!(f, "{}", DEFAULT_LISTEN_PORT)
        }
    }
}

impl ListenPort {
    fn port(&self) -> i32 {
        match self {
            ListenPort::Port(port) => *port,
            ListenPort::DefaultPort => DEFAULT_LISTEN_PORT
        }
    }
}

pub struct GoogleAuthenticator {
    client_id: String,
    client_secret: String,
    client: Client,
    token: Rc<RefCell<Option<Token>>>,
    cred_path: Rc<RefCell<PathBuf>>,
    listen_port: i32,
}

impl GoogleAuthenticator {
    pub fn new(listen_port: ListenPort, cred_path: CredPath) -> Self {
        let auth_url = AuthUrl::new(GOOGLE_AUTH_URL.to_string()).unwrap();
        let token_url = TokenUrl::new(GOOGLE_TOKEN_URL.to_string()).unwrap();

        let revocation_url = RevocationUrl::new(GOOGLE_REVOKE_URL.to_string()).unwrap();
        let redirect_url = RedirectUrl::new(
            format!("http://127.0.0.1:{}", listen_port))
            .unwrap();

        // try to read client_id and client_secret from environment
        let mut client_id = match env::var(CLIENT_ID_ENV_KEY) {
            Ok(val) => val,
            _ => String::from(OVERRIDE_CLIENT_ID.unwrap_or(DEFAULT_CLIENT_ID)),
        };

        let mut client_secret = match env::var(CLIENT_SECRET_ENV_KEY) {
            Ok(val) => val,
            _ => String::from(OVERRIDE_CLIENT_SECRET.unwrap_or(DEFAULT_CLIENT_SECRET)),
        };

        // try to read cred from file
        let mut token = None;
        if let Ok(t) = FileCredentials::read_file(&cred_path.path()) {
            client_id = t.client_id.clone();
            client_secret = t.client_secret.clone();
            token = Some(t);
        }

        // create a http client
        let client = BasicClient::new(
            ClientId::new(client_id.clone()),
            Some(ClientSecret::new(client_secret.clone())),
            auth_url.clone(),
            Some(token_url.clone()),
        )
            .set_redirect_uri(redirect_url)
            .set_revocation_uri(revocation_url.clone());

        Self {
            client_id,
            client_secret,
            client,
            token: Rc::new(RefCell::new(token)),
            cred_path: Rc::new(RefCell::new(cred_path.path().to_path_buf())),
            listen_port: listen_port.port(),
        }
    }

    pub fn access_token(&self) -> Result<AccessToken> {
        let mut refresh = false;

        loop {
            let token = Rc::clone(&self.token);
            let token = RefCell::borrow(&token);

            if let Some(t) = token.as_ref() {
                // check access token expiration
                let now = SystemTime::now();
                let expires_at = t.created_at().add(t.token_response.expires_in().unwrap());

                if now > expires_at {
                    refresh = true;
                    break;
                }

                let ac = t.token_response.access_token();
                return Ok(ac.clone());
            }

            break;
        }

        if refresh {
            self.refresh_token()
        } else {
            self.authenticate()
        }
    }

    fn refresh_token(&self) -> Result<AccessToken> {
        let mut refresh_token: Option<RefreshToken> = None;

        loop {
            let token = Rc::clone(&self.token);
            let token = RefCell::borrow(&token);

            if let Some(t) = token.as_ref() {
                if let Some(ref_refresh_token) = t.token_response.refresh_token() {
                    refresh_token = Some(ref_refresh_token.clone());
                }
            }

            break;
        }

        match refresh_token {
            Some(refresh_token) => {
                let token_response = self.client.exchange_refresh_token(&refresh_token)
                    .request(http_client);

                match token_response {
                    Ok(mut token_response) => {
                        let ac = token_response.access_token().clone();
                        token_response.set_refresh_token(Some(refresh_token));  // set refresh token, because current token_response was missed it

                        self.set_token(token_response);

                        Ok(ac)
                    }
                    Err(e) => {
                        Err(anyhow!("Failed to exchange refresh token to access token: {}", e.to_string()))
                    }
                }
            }
            None => {
                Err(anyhow!("never reached!: refresh token is missing"))
            }
        }
    }

    fn authenticate(&self) -> Result<AccessToken> {
        // create a PKCE code verifier and SHA-256 encode it as a code challenge
        let (pkce_code_challenge, pkce_code_verifier) =
            PkceCodeChallenge::new_random_sha256();

        // generate authorization url
        let (authorize_url, csrf_state) = self.client.authorize_url(CsrfToken::new_random)
            .add_scope(Scope::new(GOOGLE_DRIVE_SCOPE.to_string()))
            .set_pkce_challenge(pkce_code_challenge)
            .url();

        // open browser
        if let Err(e) = opener::open(authorize_url.to_string()) {
            return Err(anyhow!("Failed to open browser to authenticate: {}", e.to_string()));
        }

        // start simple redirect server to receive token information from OAuth2 server
        match serve_redirect_oauth2(self.listen_port) {
            Ok((code, state)) => {
                if state.secret() != csrf_state.secret() {
                    return Err(anyhow!("Not matched state '{}' != '{}'", state.secret(), csrf_state.secret()));
                }

                // Exchange the code with a token.
                let token_response = self.client
                    .exchange_code(code)
                    .set_pkce_verifier(pkce_code_verifier)
                    .request(http_client);

                match token_response {
                    Ok(token_response) => {
                        let ac = token_response.access_token().clone();
                        self.set_token(token_response);

                        Ok(ac)
                    }
                    Err(e) => {
                        Err(anyhow!("Failed to exchange to access code to access token: {}", e.to_string()))
                    }
                }
            }
            Err(e) => {
                Err(anyhow!("Failed to serve redirect for OAuth2: {}", e.to_string()))
            }
        }
    }

    fn set_token(&self, token_response: BasicTokenResponse) {
        let t = Rc::clone(&self.token);
        let mut t = RefCell::borrow_mut(&t);
        let now = SystemTime::now();

        // make token
        let token = Token::new(&self.client_id, &self.client_secret, token_response, now);

        // write to cred path
        let cred_path = Rc::clone(&self.cred_path);
        let cred_path = RefCell::borrow(&cred_path);

        if let Err(e) = FileCredentials::write_file(&token, cred_path.as_path()) {
            eprintln!("Failed to write cred file: {}", e);
        }

        // set token
        *t = Some(token);
    }
}

fn serve_redirect_oauth2(listen_port: i32) -> Result<(AuthorizationCode, CsrfToken)> {
    let listen_addr = format!("127.0.0.1:{}", listen_port);

    let listener = match TcpListener::bind(&listen_addr) {
        Ok(l) => l,
        Err(e) => {
            return Err(anyhow!("Failed to listen at '{}': {}", &listen_addr, e.to_string()));
        }
    };

    for stream in listener.incoming() {
        if let Ok(mut stream) = stream {
            let code;
            let state;
            {
                let mut reader = BufReader::new(&stream);

                let mut request_line = String::new();
                if let Err(e) = reader.read_line(&mut request_line) {
                    return Err(anyhow!("Failed to read line from stream: {}", e.to_string()));
                }

                let redirect_url = match request_line.split_whitespace().nth(1) {
                    Some(s) => s,
                    None => {
                        return Err(anyhow!("Invalid request line '{}'", request_line));
                    }
                };

                let url = Url::parse(&("http://localhost".to_string() + redirect_url)).unwrap();

                let code_pair = match url.query_pairs()
                    .find(|pair| {
                        let &(ref key, _) = pair;
                        key == "code"
                    }) {
                    Some(p) => p,
                    None => {
                        return Err(anyhow!("Can't find code pair on '{}'", url.to_string()));
                    }
                };

                let (_, value) = code_pair;
                code = AuthorizationCode::new(value.into_owned());

                let state_pair = match url.query_pairs()
                    .find(|pair| {
                        let &(ref key, _) = pair;
                        key == "state"
                    }) {
                    Some(p) => p,
                    None => {
                        return Err(anyhow!("Can't find state pair on '{}'", url.to_string()));
                    }
                };

                let (_, value) = state_pair;
                state = CsrfToken::new(value.into_owned());
            }

            // respond to browser
            let message = "Good! You turn off this window any time! :)";
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-length: {}\r\n\r\n{}",
                message.len(),
                message
            );

            if let Err(e) = stream.write_all(response.as_bytes()) {
                eprintln!("Failed to write to browser, but it's OK: {}", e.to_string());
            }

            // return authorize code
            return Ok((code, state));
        }
    }

    Err(anyhow!("never reached"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use oauth2::basic::BasicTokenType;
    use oauth2::EmptyExtraTokenFields;
    use crate::drive::helper::FileCredentials;

    #[test]
    fn google_oauth2() {
        let auth = GoogleAuthenticator::new(ListenPort::DefaultPort, CredPath::DefaultPath);

        // get access token with login
        let ac = auth.access_token().unwrap();
        println!("Access token from login: {}", ac.secret());

        // get access token using refresh token
        let ac = auth.refresh_token().unwrap();
        println!("Access token from refresh token: {}", ac.secret());
    }

    #[test]
    fn marshal_and_unmarshal_token() {
        let token_response =
            BasicTokenResponse::new(AccessToken::new(String::from("access_token")), BasicTokenType::Bearer, EmptyExtraTokenFields {});

        let token = Token::new("client_id", "client_secret", token_response, SystemTime::now());

        // marshal token
        let marshaled = FileCredentials::marshal(&token).unwrap();
        println!("marshaled = {}", marshaled);

        // unmarshal token
        let unmarshaled_token = FileCredentials::unmarshal(marshaled.into_bytes()).unwrap();

        // comparison values
        assert_eq!(token.client_id, "client_id");
        assert_eq!(token.client_secret, "client_secret");
        assert_eq!(token.token_response.access_token().secret(),
                   unmarshaled_token.token_response.access_token().secret());
    }
}
