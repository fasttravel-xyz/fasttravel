mod client_connection_actor_creator;
mod client_connection_actor;
mod client_connection_service_actor;

pub(crate) use client_connection_actor_creator::*;
pub(crate) use client_connection_actor::*;
pub(crate) use client_connection_service_actor::*;

use crate::{axum, CospaceActor};
use fasttravel_rt_services::ClientId;

/// Create client connection actor request message.
pub(crate) struct CreateClientConnectionActorMessage {
    pub(crate) socket: axum::WebSocket,
    pub(crate) cospace_addr: factor::ActorAddr<CospaceActor>,
}
impl factor::Message for CreateClientConnectionActorMessage {
    type Result = Option<(ClientId, factor::ActorAddr<ClientConnectionActor>)>;
}

/// Incoming and Outgoing socket message.
pub(crate) struct SocketMessage(pub(crate) axum::Message);

impl factor::Message for SocketMessage {
    type Result = ();
}
