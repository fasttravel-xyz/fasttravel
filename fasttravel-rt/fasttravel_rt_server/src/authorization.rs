use jsonwebtoken::{decode, Algorithm, Validation};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashSet;

use crate::axum;
use crate::*;

/// the claim used during {realtime-server-url}/realtime/host/ request
#[derive(Debug, Serialize, Deserialize)]
pub struct HostWorkspaceClaims {
    sub: String,
    aud: String,
    exp: usize,
}
impl HostWorkspaceClaims {
    // easy helper for the mockers.
    // [todo] expose this only for "mocker" feature.
    pub fn expected(exp: usize) -> Self {
        Self {
            sub: "certificate".to_owned(),
            aud: "realtime".to_owned(),
            exp,
        }
    }

    fn validation() -> Validation {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.sub = Some("certificate".to_string());
        validation.aud = Some(HashSet::from(["realtime".to_string()]));
        validation
    }
}

#[axum::async_trait]
impl axum::FromRequestParts<RealtimeServerState> for HostWorkspaceClaims {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::Parts, state: &RealtimeServerState,
    ) -> Result<Self, Self::Rejection> {
        // extract the token from the authorization header
        let axum::TypedHeader(axum::Authorization(bearer)) =
            axum::TypedHeader::<axum::Authorization<axum::Bearer>>::from_request_parts(
                parts, state,
            )
            .await
            .map_err(|_| AuthError::InvalidToken)?;

        // validate and decode the token
        let validation = HostWorkspaceClaims::validation();

        let decoding = &state.public_keys.session_request_decoding;
        let token_data = decode::<HostWorkspaceClaims>(bearer.token(), decoding, &validation)
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(token_data.claims)
    }
}

/// claim for the status request: {realtime-server-url}/realtime/status/:cospace_id
#[derive(Debug, Serialize, Deserialize)]
pub struct TicketClaimsStatus {
    sub: String,
    aud: String,
    exp: usize,
}
impl TicketClaimsStatus {
    // easy helper for the mockers.
    // [todo] expose this only for "mocker" feature.
    pub fn expected(exp: usize) -> Self {
        Self {
            sub: "sdk".to_owned(),
            aud: "status".to_owned(),
            exp,
        }
    }

    fn validation() -> Validation {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.sub = Some("sdk".to_string());
        validation.aud = Some(HashSet::from(["status".to_string()]));
        validation
    }
}

/// claim for the websocket connect query parameter.
/// const socket = new WebSocket(
///      "wss://{url}/realtime/connect/:cospace_id?ticket={TicketClaimsQuery token}",
///      "proto-cospace-ws-v01");
#[derive(Debug, Serialize, Deserialize)]
pub struct TicketClaimsQuery {
    sub: String,
    aud: String,
    exp: usize,
}
impl TicketClaimsQuery {
    // easy helper for the mockers.
    // [todo] expose this only for "mocker" feature.
    pub fn expected(exp: usize) -> Self {
        Self {
            sub: "sdk".to_owned(),
            aud: "query".to_owned(),
            exp,
        }
    }

    pub(super) fn validation() -> Validation {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.sub = Some("sdk".to_string());
        validation.aud = Some(HashSet::from(["query".to_string()]));
        validation
    }
}

/// claim for the first message from client after the websocket connection success
#[derive(Debug, Serialize, Deserialize)]
pub struct TicketClaimsMessage {
    sub: String,
    aud: String,
    exp: usize,
    // jti: String, // id to check that ticket is used only once
}
impl TicketClaimsMessage {
    // easy helper for the mockers.
    // [todo] expose this only for "mocker" feature.
    pub fn expected(exp: usize) -> Self {
        Self {
            sub: "sdk".to_owned(),
            aud: "message".to_owned(),
            exp,
        }
    }

    pub(crate) fn validation() -> Validation {
        let mut validation = Validation::new(Algorithm::ES256);
        validation.sub = Some("sdk".to_string());
        validation.aud = Some(HashSet::from(["message".to_string()]));
        validation
    }
}

#[axum::async_trait]
impl axum::FromRequestParts<RealtimeServerState> for TicketClaimsStatus {
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::Parts, state: &RealtimeServerState,
    ) -> Result<Self, Self::Rejection> {
        // extract the token from the authorization header
        let axum::TypedHeader(axum::Authorization(bearer)) =
            axum::TypedHeader::<axum::Authorization<axum::Bearer>>::from_request_parts(
                parts, state,
            )
            .await
            .map_err(|_| AuthError::InvalidToken)?;

        // validate and decode the token
        let validation = TicketClaimsStatus::validation();

        let decoding = &state.public_keys.session_request_decoding;
        let token_data = decode::<TicketClaimsStatus>(bearer.token(), decoding, &validation)
            .map_err(|_| AuthError::InvalidToken)?;

        Ok(token_data.claims)
    }
}

/// Realtime server authetication errors.
#[derive(Debug)]
pub enum AuthError {
    WrongCredentials,
    MissingCredentials,
    TicketCreation,
    TokenCreation,
    InvalidToken,
}

impl axum::IntoResponse for AuthError {
    fn into_response(self) -> axum::Response {
        let (status, error_message) = match self {
            AuthError::WrongCredentials => (axum::StatusCode::UNAUTHORIZED, "Wrong credentials"),
            AuthError::MissingCredentials => (axum::StatusCode::BAD_REQUEST, "Missing credentials"),
            AuthError::TicketCreation => (
                axum::StatusCode::INTERNAL_SERVER_ERROR,
                "Ticket creation error",
            ),
            AuthError::TokenCreation => (
                axum::StatusCode::INTERNAL_SERVER_ERROR,
                "Token creation error",
            ),
            AuthError::InvalidToken => (axum::StatusCode::BAD_REQUEST, "Invalid token"),
        };

        let body = axum::Json(json!({
            "auth_error": error_message,
        }));
        (status, body).into_response()
    }
}
