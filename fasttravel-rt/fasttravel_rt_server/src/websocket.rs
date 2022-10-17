mod websocket_actor;
mod websocket_server;

use crate::CospaceActor;

pub(crate) use websocket_actor::*;
pub use websocket_server::HostSessionRequest;
pub(crate) use websocket_server::*;

/// Message notifying new client websocket connection.
pub(crate) struct WebsocketOnUpgradeMessage {
    pub(crate) socket: axum::WebSocket,
    pub(crate) cospace_addr: factor::ActorAddr<CospaceActor>,
}

impl factor::Message for WebsocketOnUpgradeMessage {
    type Result = ();
}
