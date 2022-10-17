//! Implements a mocker that emulates the session_lambda cloud function
//! and provides the below endpoint(s):
//!
//! * POST /session/join/
//!
//! example join request for the mocker (client-sdk will make this call):
//! curl --request POST \
//!      --url "http://localhost:27001/session/join/" \
//!      --header 'content-type: application/json; charset=utf-8' \
//!      --header 'Authorization: Bearer {access_token_from_auth_provider}' \
//!      --data-binary @- << EOF
//!      {
//!         "workspace": "public_workspace",
//!         "namespace": "organization_name",
//!      }
//! EOF
//!
//! =============================================================================
//! This server is provided to debug and develope the REALTIME SERVER locally.
//! Better local mockers could be created using below tools:
//! https://github.com/dherault/serverless-offline
//! https://github.com/jagregory/cognito-local
//! https://github.com/localstack/localstack
//! =============================================================================

use axum::{
    async_trait,
    extract::{FromRequestParts, State, TypedHeader},
    headers::{authorization::Bearer, Authorization},
    http::request::Parts,
    routing::post,
    Json, Router, Server,
};
use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use tower_http::trace::{DefaultMakeSpan, TraceLayer};

use fasttravel_rt_server::{
    AuthError, HostSessionRequest, HostWorkspaceClaims, TicketClaimsMessage, TicketClaimsQuery,
    TicketClaimsStatus,
};

// =============================================================================
// Below debug keys generated using:
// > openssl ecparam -genkey -noout -name prime256v1 | openssl pkcs8 -topk8 -nocrypt -out private.pem
// > openssl ec -in private.pem -pubout -out public.pem
// =============================================================================

// Mocker auth-public-key for the private-key with which the access token was
// signed by auth_provider. The mocker private-key is in auth_provider.rs
const AUTH_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAErSGEjwB09a60VLIGMzUCaE5Qpgw6
f/X+AUEt5b2hv/Dpjr28n688fqOWuVrPOw7kbi0WQokH/paL201sJl3I4g==
-----END PUBLIC KEY-----";

// Mocker ticket-private-key using which this lambda will generate the three
// JWT tickets for the cient-sdk using which the sdk will connect to the
// realtime server.
const TICKET_PRIVATE_KEY: &str = "-----BEGIN PRIVATE KEY-----
MIGHAgEAMBMGByqGSM49AgEGCCqGSM49AwEHBG0wawIBAQQgbqQKK3GZezZB1j5F
MiATO+7xAXwUqNjjtZAmO56nhLGhRANCAAQvhVDFyvR947IF6r2YBPS/P+qgHDR6
kyygmF4SHhyym72azugUvjasYrjW3QebHWPg0lvHeU7mjOKob7+uDxIz
-----END PRIVATE KEY-----";

static AUTH_KEY_DECODING: Lazy<DecodingKey> =
    Lazy::new(|| DecodingKey::from_ec_pem(AUTH_PUBLIC_KEY.as_bytes()).unwrap());

static TICKET_KEY_ENCODING: Lazy<EncodingKey> =
    Lazy::new(|| EncodingKey::from_ec_pem(TICKET_PRIVATE_KEY.as_bytes()).unwrap());

#[derive(Clone)]
pub struct SessionServerState {
    // REST end-point to ask a COSPACE to host a WORKSPACE
    endpoint: String,
}

pub fn run_session_lambda(socket_addr: ([u8; 4], u16)) -> impl futures::Future<Output = ()> {
    let state = SessionServerState {
        // endpoint: "https://realtime.fasttravel.xyz/realtime/host/".to_owned(),
        endpoint: "http://localhost:27000/realtime/host/".to_owned(),
    };

    run_session_api(socket_addr, state)
}

// Session Lambda
pub(crate) async fn run_session_api(socket_addr: ([u8; 4], u16), state: SessionServerState) {
    let app = Router::with_state(state)
        .route("/session/join/", post(join_handler))
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

async fn join_handler(
    claims: AuthClaims,
    State(state): State<SessionServerState>,
    Json(session_join_payload): Json<SessionJoinPayload>,
    // _user_agent: Option<TypedHeader<headers::UserAgent>>,
) -> Result<Json<AuthResponseBody>, AuthError> {
    // check that the user_id has permissions to access the namespace and
    // the workspace (even if this is redundant)
    if !authorization_check(&claims.sub, &session_join_payload) {
        return Err(AuthError::WrongCredentials);
    }

    // Ask REALTIME SERVER TO HOST the model-rrot, and if already hosted get the
    // COSPACE_uuid from model-root-cospace session store.
    let expiration_utc: chrono::DateTime<chrono::Utc> =
        chrono::offset::Utc::now() + chrono::Duration::seconds(60);
    let exp = expiration_utc.timestamp() as usize;
    let claims = HostWorkspaceClaims::expected(exp);
    let certificate = encode(
        &Header::new(Algorithm::ES256),
        &claims,
        &TICKET_KEY_ENCODING,
    )
    .map_err(|e| {
        println!("TicketCreation: TICKET_CLAIMS_HOST: {}", e);
        AuthError::TicketCreation
    })?;

    let workspace_payload = HostSessionRequest {
        model_workspace: session_join_payload.workspace,
        model_namespace: session_join_payload.namespace,
    };

    // send host model-root request
    // [todo] handle error
    let client = reqwest::Client::new();
    let cospace_data: CospaceData = client
        .post(state.endpoint)
        .bearer_auth(certificate)
        .json(&workspace_payload)
        .send()
        .await
        .unwrap() // [todo] handlel the panic, if connection resets (not a priority as this is a mocker)
        .json::<CospaceData>()
        .await
        .unwrap();

    // send the tickets
    let response = AuthResponseBody::new(cospace_data.uuid)?;
    Ok(Json(response))
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct AuthClaims {
    sub: String, // "usr_123"
    iss: String, // "https://login.fasttravel.xyz"
    exp: usize,  //
    aud: String, // API-identifier from your auth-provider.
}

#[derive(Debug, Serialize, Deserialize)]
struct SessionJoinPayload {
    workspace: String,
    namespace: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct CospaceData {
    uuid: String,
    // routing: String,

    // if we have multiple instances of the REALTIME SERVER running and a
    // workspace is already hosted, we would need some kind to routing info
    // to route the user to the appropriate instance already hosting the workspace.
    // The exact flow and info will depend on the ingress-proxy and load-balancer
    // setup. Adding this comment to help build the session-datastore keeping
    // this use case in mind.
}

fn authorization_check(_user_id: &String, _payload: &SessionJoinPayload) -> bool {
    true
}

#[derive(Debug, Serialize)]
struct AuthResponseBody {
    ticket_status: String,
    ticket_query: String,
    ticket_message: String,
    cospace_uuid: String,
    token_type: String,
}

impl AuthResponseBody {
    fn new(cospace_uuid: String) -> Result<Self, AuthError> {
        // 5 minutes expiration
        let expiration_utc = chrono::offset::Utc::now() + chrono::Duration::seconds(300);
        let exp = expiration_utc.timestamp() as usize;

        // Once we finalize the handshake flow, we might not need all the tickets.
        let claims_status = TicketClaimsStatus::expected(exp);
        let claims_query = TicketClaimsQuery::expected(exp);
        let claims_message = TicketClaimsMessage::expected(exp);

        let ticket_status = encode(
            &Header::new(Algorithm::ES256),
            &claims_status,
            &TICKET_KEY_ENCODING,
        )
        .map_err(|e| {
            println!("TicketCreation: TICKET_CLAIMS_STATUS: {}", e);
            AuthError::TicketCreation
        })?;

        let ticket_query = encode(
            &Header::new(Algorithm::ES256),
            &claims_query,
            &TICKET_KEY_ENCODING,
        )
        .map_err(|e| {
            println!("TicketCreation: TICKET_CLAIMS_QUERY: {}", e);
            AuthError::TicketCreation
        })?;

        let ticket_message = encode(
            &Header::new(Algorithm::ES256),
            &claims_message,
            &TICKET_KEY_ENCODING,
        )
        .map_err(|e| {
            println!("TicketCreation: TICKET_CLAIMS_MESSAGE: {}", e);
            AuthError::TicketCreation
        })?;

        Ok(Self {
            ticket_status,
            ticket_query,
            ticket_message,
            cospace_uuid,
            token_type: "Ticket".to_string(),
        })
    }
}

// default issuer
const ISS_STR: &str = "https://login.fasttravel.xyz";

#[async_trait]
impl<S> FromRequestParts<S> for AuthClaims
where
    S: Send + Sync,
{
    type Rejection = AuthError;
    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        // extract the token from the authorization header
        let TypedHeader(Authorization(bearer)) =
            TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, _state)
                .await
                .map_err(|_| AuthError::InvalidToken)?;
        // validate and decode the token
        let mut validation = Validation::new(Algorithm::ES256);
        validation.iss = Some(HashSet::from([ISS_STR.to_string()]));
        validation.aud = Some(HashSet::from(["realtime:api".to_string()]));
        let token_data = decode::<AuthClaims>(bearer.token(), &AUTH_KEY_DECODING, &validation)
            .map_err(|e| {
                println!("InvalidToken: AUTH_CLAIMS: {}", e);
                AuthError::InvalidToken
            })?;
        Ok(token_data.claims)
    }
}

#[tokio::main]
async fn main() {
    let addr = ([0, 0, 0, 0], 27001);
    let fut = run_session_lambda(addr);

    fut.await;
}
