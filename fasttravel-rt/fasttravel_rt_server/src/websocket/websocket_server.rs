pub(crate) mod axum {
    pub(crate) use axum::extract::ws::Message;
    pub(crate) use axum::extract::ws::WebSocket;
    pub(crate) use axum::extract::ws::WebSocketUpgrade;
    pub(crate) use axum::extract::FromRequestParts;
    pub(crate) use axum::extract::State;
    pub(crate) use axum::extract::{Path, Query, TypedHeader};
    pub(crate) use axum::http::request::Parts;
    pub(crate) use axum::http::StatusCode;
    pub(crate) use axum::response::{IntoResponse, Response};
    pub(crate) use axum::routing::{get, post};
    pub(crate) use axum::Json;
    pub(crate) use axum::Router;
    pub(crate) use axum::Server;
    pub(crate) use axum::{
        async_trait,
        headers::{authorization::Bearer, Authorization},
    };
}

mod tower {
    pub(crate) use tower_http::trace::DefaultMakeSpan;
    pub(crate) use tower_http::trace::TraceLayer;
}

use jsonwebtoken::decode;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

use fasttravel_rt_services::ModelRoot;

use crate::{
    AuthError, HostWorkspaceClaims, RealtimeServerState, TicketClaimsQuery, TicketClaimsStatus,
    WebsocketOnUpgradeMessage,
};

const WS_PROTOCOL: &'static str = "realtime-proto-v01";

/// Run the realtime websocket server.
///
/// example socket_addr: ([0, 0, 0, 0], 27000)
/// example ws url: const socket = new WebSocket(
///      "wss://realtime.fasttravel.xyz/realtime/connect/67e55044-10b1-426f-9247-bb680e5fe0c8?ticket=gTbhgat...",
///      "realtime-proto-v01");
pub(crate) async fn run_ws_server(socket_addr: ([u8; 4], u16), state: RealtimeServerState) {
    let app = axum::Router::with_state(state)
        .route("/realtime/host/", axum::post(realtime_host))
        .route("/realtime/status/:cospace", axum::get(realtime_status))
        .route("/realtime/connect/:cospace", axum::get(realtime_connect))
        // logging
        .layer(
            tower::TraceLayer::new_for_http()
                .make_span_with(tower::DefaultMakeSpan::default().include_headers(true)),
        );

    // run the server
    let addr = std::net::SocketAddr::from(socket_addr);
    tracing::debug!(target: "server-event", "rt_server_websocket_listening_on {}", addr);

    axum::Server::bind(&addr)
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Debug, Serialize, Deserialize)]
struct CospaceData {
    uuid: String,
    // routing: String,

    // If we have multiple instances of the REALTIME SERVER running (scaling)
    // and a model-root is already hosted, we would need some kind of routing info
    // to route the user to the appropriate instance already hosting the model-root.
    // The exact flow and info will depend on the ingress-proxy and load-balancer
    // setup. Adding this comment to help design the session-datastore keeping
    // this use case in mind.
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HostSessionRequest {
    pub model_workspace: String,
    pub model_namespace: String,
}

/// Schedule a cospace for creation that will host the model-root.
async fn realtime_host(
    _claims: HostWorkspaceClaims, axum::State(state): axum::State<RealtimeServerState>,
    axum::Json(session_req): axum::Json<HostSessionRequest>,
) -> Result<axum::Json<CospaceData>, ()> {
    // NOTE: We don't check whether the model-root is already hosted, as it is
    // business dependent whether to allow this or not. The session_lambda with
    // access to session_store and the model_service are responsible for
    // establishing that policy.

    tracing::debug!(target: "server-event", "realtime_host_session_request_received");

    let cospace_host_node = resolve_cospace_host_node_from_session_request(&session_req);

    let model_root = ModelRoot {
        namespace: session_req.model_workspace,
        workspace: session_req.model_namespace,
    };

    let cospace_id = match cospace_host_node {
        CospaceHostNode::Main => state
            .cospace_mgr
            .spawn_cospace_in_main_node(model_root)
            .map_err(|_| tracing::error!("cospace_scheduleing_failed_for_main_node"))?,
        CospaceHostNode::Shared => state
            .cospace_mgr
            .spawn_cospace_in_shared_node(model_root)
            .map_err(|_| tracing::error!("cospace_scheduleing_failed_for_shared_node"))?,
        CospaceHostNode::Dedicated => state
            .cospace_mgr
            .spawn_cospace_in_dedicated_node(model_root)
            .map_err(|_| tracing::error!("cospace_scheduleing_failed_for_dedicated_node"))?,
    };

    Ok(axum::Json(CospaceData {
        uuid: cospace_id.uuid.simple().to_string(),
    }))
}

/// Retrieve the status of a cospace.
async fn realtime_status(
    _claims: TicketClaimsStatus, axum::State(state): axum::State<RealtimeServerState>,
    axum::Path(cospace_uuid): axum::Path<Uuid>,
) -> impl axum::IntoResponse {
    // check cospace status (NOT_FOUND, SCHEDULED, HOSTED, FAILED)
    if state.cospace_mgr.hosted_cospaces().is_hosted(&cospace_uuid) {
        return "HOSTED".to_string();
    } else if state
        .cospace_mgr
        .hosted_cospaces()
        .is_scheduled(&cospace_uuid)
    {
        return "SCHEDULED".to_string();
    } else if state.cospace_mgr.hosted_cospaces().is_failed(&cospace_uuid) {
        return "FAILED".to_string();
    }

    "NOT_FOUND".to_string()
}

/// Handle new websocket client connections
async fn realtime_connect(
    ws: axum::WebSocketUpgrade, axum::Path(uuid): axum::Path<Uuid>,
    params: Option<axum::Query<HashMap<String, String>>>,
    user_agent: Option<axum::TypedHeader<headers::UserAgent>>,
    axum::State(state): axum::State<RealtimeServerState>,
) -> Result<axum::Response, AuthError> {
    if let Some(axum::TypedHeader(user_agent)) = user_agent {
        tracing::trace!(target: "server-event", "`{}`_connected", user_agent.as_str());
    }

    // get ticket from query parameter and validate
    if !check_connect_authorization(&params, &state) {
        return Err(AuthError::InvalidToken);
    }

    // Make sure the cospace is already created
    if let Some(cospace_addr) = state.cospace_mgr.hosted_cospaces().get_cospace_addr(&uuid) {
        let addr_moved = cospace_addr.clone();

        // inform the websocket service of new client connection.
        let wss = state.ws_addr.clone();
        let res = ws.protocols([WS_PROTOCOL]).on_upgrade(|socket| async move {
            let _ = wss.tell(WebsocketOnUpgradeMessage {
                socket,
                cospace_addr: addr_moved,
            });
        });

        return Ok(res);
    }

    Err(AuthError::WrongCredentials)
}

/// check connect request authorization
fn check_connect_authorization(
    params: &Option<axum::Query<HashMap<String, String>>>, state: &RealtimeServerState,
) -> bool {
    if let Some(axum::Query(params)) = params {
        if let Some(query_ticket) = params.get("ticket") {
            let validation = TicketClaimsQuery::validation();

            let decoding = &state.public_keys.session_request_decoding;
            match decode::<TicketClaimsQuery>(query_ticket, decoding, &validation) {
                Ok(_data) => {
                    tracing::debug!(target: "server-event", "check_connect_auth_ok");

                    return true;
                }
                Err(e) => {
                    tracing::error!(target: "server_event", "connect_ticket_auth_error: {}", e)
                }
            }
        }
    }

    false
}

#[allow(dead_code)]
enum CospaceHostNode {
    Main,
    Shared,
    Dedicated,
}

fn resolve_cospace_host_node_from_session_request(
    _session_req: &HostSessionRequest,
) -> CospaceHostNode {
    // [future use]
    CospaceHostNode::Dedicated
}
