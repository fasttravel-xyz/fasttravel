//! This mocker emulates the auth_provider and provides the below endpoint:
//!
//! * POST /authorize/
//!
//! example authorization request (client-sdk will make this call over https):
//! curl --request POST \
//!      --url "http://localhost:27002/authorize/" \
//!      --header 'content-type: application/json; charset=utf-8' \
//!      --data-binary @- << EOF
//!      {
//!         "user_id": "user_id_01",
//!         "user_secret": "user_secret_01",
//!         "client_id": "client_id_01",
//!         "audience": "realtime:api"
//!      }
//! EOF
//!
//! This is a mocker, refer to your auth-provider's doc for auth-flow.
//! Example auth-flows:
//! 1. https://auth0.com/docs/get-started/authentication-and-authorization-flow/call-your-api-using-the-authorization-code-flow-with-pkce
//! 2. https://auth0.com/docs/quickstart/backend/python/interactive
//!

use axum::{extract::State, routing::post, Json, Router, Server};
use jsonwebtoken::{encode, Algorithm, EncodingKey, Header};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::{collections::HashMap, sync::Arc};
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use fasttravel_rt_server::AuthError;

// Debug auth-private-key with which the access token was signed by auth_provider.
// The corresponding PUBLIC_KEY is in session_lambda.rs
const AUTH_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgQTRFM6ANTfZfQWPh
J343d5uHiBPeihnCXGTFzLSxDtGhRANCAAStIYSPAHT1rrRUsgYzNQJoTlCmDDp/
9f4BQS3lvaG/8OmOvbyfrzx+o5a5Ws87DuRuLRZCiQf+lovbTWwmXcji
-----END PRIVATE KEY-----";

static AUTH_KEY_ENCODING: Lazy<EncodingKey> =
    Lazy::new(|| EncodingKey::from_ec_pem(AUTH_PRIVATE_KEY.as_bytes()).unwrap());

// default issuer example
const ISS_STR: &str = "https://login.fasttravel.xyz";

// Keep the secrets and profile datastores separate.
#[derive(Clone)]
pub struct AuthServerState {
    datastore_namespace_users_secrets: Arc<HashMap<String, HashMap<String, String>>>,
    datastore_userprofile: Arc<HashMap<String, UserProfile>>,
}

struct UserProfile {
    name: String,
    _email: String,
}

// fill some pre-existing fake-data.
// [todo]: implement a userpool api and remove fake data (low priority).
impl Default for AuthServerState {
    fn default() -> Self {
        let mut user_map = HashMap::new();
        user_map.insert("user_id_01".to_owned(), "user_secret_01".to_owned());
        user_map.insert("user_id_02".to_owned(), "user_secret_02".to_owned());
        user_map.insert("user_id_03".to_owned(), "user_secret_03".to_owned());
        user_map.insert("user_id_04".to_owned(), "user_secret_04".to_owned());
        let mut namespace_map = HashMap::new();
        namespace_map.insert("client_id_01".to_owned(), user_map);

        let mut profile_map = HashMap::new();
        profile_map.insert(
            "user_id_01".to_owned(),
            UserProfile {
                name: "name_01".to_owned(),
                _email: "1@ft.xyz".to_owned(),
            },
        );
        profile_map.insert(
            "user_id_02".to_owned(),
            UserProfile {
                name: "name_02".to_owned(),
                _email: "2@ft.xyz".to_owned(),
            },
        );
        profile_map.insert(
            "user_id_03".to_owned(),
            UserProfile {
                name: "name_03".to_owned(),
                _email: "3@ft.xyz".to_owned(),
            },
        );
        profile_map.insert(
            "user_id_04".to_owned(),
            UserProfile {
                name: "name_04".to_owned(),
                _email: "4@ft.xyz".to_owned(),
            },
        );

        Self {
            datastore_namespace_users_secrets: Arc::new(namespace_map),
            datastore_userprofile: Arc::new(profile_map),
        }
    }
}

pub fn run_auth_provider(socket_addr: ([u8; 4], u16)) -> impl futures::Future<Output = ()> {
    let state = AuthServerState::default();

    run_authapi(socket_addr, state)
}

// Session Lambda
pub(crate) async fn run_authapi(socket_addr: ([u8; 4], u16), state: AuthServerState) {
    let app = Router::with_state(state)
        .route("/authorize/", post(auth_handler))
        .layer(
            // logging
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::default().include_headers(true)),
        );

    // run the server
    let addr = std::net::SocketAddr::from(socket_addr);
    tracing::debug!("rest listening on {}", addr);
    Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

/// Authorization request handler.
async fn auth_handler(
    State(state): State<AuthServerState>,
    Json(payload): Json<AuthPayload>,
    // _user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> Result<Json<AuthResponseBody>, AuthError> {
    // verify user credential fields
    if payload.client_id.is_empty() || payload.user_id.is_empty() || payload.user_secret.is_empty()
    {
        return Err(AuthError::MissingCredentials);
    }

    // check if the credentials are correct
    let mut ceredential_ok = false;
    if let Some(users) = state
        .datastore_namespace_users_secrets
        .get(&payload.client_id)
    {
        if let Some(secret) = users.get(&payload.user_id) {
            if secret == &payload.user_secret {
                ceredential_ok = true;
            }
        }
    }
    if !ceredential_ok {
        return Err(AuthError::WrongCredentials);
    }

    // get the profile data
    let mut user_name = String::new();
    if let Some(profile) = state.datastore_userprofile.get(&payload.user_id) {
        user_name = profile.name.clone();
    }

    // token valid for 24 hours
    let expiration_utc: chrono::DateTime<chrono::Utc> =
        chrono::offset::Utc::now() + chrono::Duration::seconds(86400);
    let expiration_unix = expiration_utc.timestamp() as usize;

    // id token creation
    let id_claims = AuthIdClaims {
        sub: payload.user_id.clone(),
        iss: ISS_STR.to_string(),
        exp: expiration_unix,
        name: user_name,
    };
    let token_id = encode(
        &Header::new(Algorithm::ES256),
        &id_claims,
        &AUTH_KEY_ENCODING,
    )
    .map_err(|e| {
        println!("TokenCreation: ID_CLAIMS: {}", e);
        AuthError::TokenCreation
    })?;

    // api access token
    let access_claims = AuthAccessClaims {
        sub: payload.user_id,
        iss: ISS_STR.to_string(),
        exp: expiration_unix,
        aud: payload.audience,
    };
    let token_access = encode(
        &Header::new(Algorithm::ES256),
        &access_claims,
        &AUTH_KEY_ENCODING,
    )
    .map_err(|e| {
        println!("TokenCreation: ACCESS_CLAIMS: {}", e);
        AuthError::TokenCreation
    })?;

    // Send the authorized token
    Ok(Json(AuthResponseBody::new(token_id, token_access)))
}

#[derive(Debug, Deserialize)]
struct AuthPayload {
    user_id: String,
    user_secret: String,
    client_id: String,
    audience: String,
}

#[derive(Debug, Serialize)]
struct AuthResponseBody {
    token_type: String,
    id_token: String,
    access_token: String,
    // refresh_token: String,
}

impl AuthResponseBody {
    fn new(id_token: String, access_token: String) -> Self {
        Self {
            token_type: "Bearer".to_owned(),
            id_token,
            access_token,
        }
    }
}

// 1. When you authenticate an user using your auth-provider-sdk from a public
//    user-agent(e.g. a browser), request an API access JWT with audience
//    as the api-identifier e.g "realtime:api". Once your user is authenticated
//    get the token from the user object of the auth-client-sdk.
// 2. Use that access token to authorize the /session/join/ request to start
//    a realtime session.
#[derive(Debug, Serialize)]
struct AuthAccessClaims {
    sub: String, // e.g. "user_id_01"
    iss: String, // e.g. "https://login.fasttravel.xyz"
    exp: usize,  // expiry timestamp
    aud: String, // API-identifier from your auth-provider e.g. "realtime:api".
}

#[derive(Debug, Serialize)]
struct AuthIdClaims {
    sub: String,  // e.g. "user_id_01"
    iss: String,  // e.g. "https://login.fasttravel.xyz"
    exp: usize,   // expiry timestamp
    name: String, // unvalidated claim
}

#[tokio::main]
async fn main() {
    let addr = ([0, 0, 0, 0], 27002);
    let fut = run_auth_provider(addr);

    fut.await;
}
