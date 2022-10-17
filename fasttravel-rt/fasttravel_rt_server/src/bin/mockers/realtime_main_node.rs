//! Implements a mocker realtime-server. Starts a server that provides the
//! below endpoints:
//!
//! * POST /realtime/host/
//! * GET /realtime/status/:cospace
//! * GET UPGRADE WEBSOCKET /realtime/connect/:cospace
//!
//! example "host" api call (the session_lambda makes this call, not the client-sdk):
//! curl --request POST \
//!      --url "http://localhost:27000/realtime/host/" \
//!      --header 'content-type: application/json; charset=utf-8' \
//!      --header 'Authorization: Bearer {session_lambda_certificate}' \
//!      --data-binary @- << EOF
//!      {
//!         "workspace": {workspace_received_from_client_sdk},
//!         "namespace": {namespace_received_from_client_sdk},
//!      }
//! EOF
//!
//! example "status" api call (client-sdk will make this call over https):
//! curl --request GET \
//!      --url "http://localhost:27000/realtime/status/:cospace_uuid_response_from_session_join_call" \
//!      --header 'content-type: application/json; charset=utf-8' \
//!      --header 'Authorization: Bearer {status_ticket_response_from_session_join_call}'
//!
//! example websocket connect (client-sdk will make this call over wss):
//! [todo] curl has recently added websocket support, add the curl call as well.
//! const socket = new WebSocket(
//!      "ws://localhost:27000/realtime/connect/:cospace_uuid_response_from_session_join_call?ticket={query_ticket_response_from_session_join_call}",
//!      "realtime-proto-v01");
//!

// Mocker ticket-public-key for the private-key with which the tickets were
// signed by session_lambda. The mocker private-key is in session_lambda.rs
// In a production setup get key from the JSON Web Key Sets (jwks.json) file.
const TICKET_PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAEL4VQxcr0feOyBeq9mAT0vz/qoBw0
epMsoJheEh4cspu9ms7oFL42rGK41t0Hmx1j4NJbx3lO5oziqG+/rg8SMw==
-----END PUBLIC KEY-----";

// we need tokio runtime for axum.
#[tokio::main]
async fn main() {
    fasttravel_rt_server::initialize_and_run_main_node(TICKET_PUBLIC_KEY)
        .await
        .ok();
}
