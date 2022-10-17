use serde::{Deserialize, Serialize};

mod cospace_actor;
mod cospace_manager;
mod service_actor;
mod service_pool;

use factor;
use fasttravel_rt_proto::RealtimeService;
use fasttravel_rt_services::*;

pub(crate) use cospace_actor::*;
pub(crate) use cospace_manager::*;
pub(crate) use service_actor::*;
pub(crate) use service_pool::*;

use crate::ClientConnectionActor;

/// Message payload for client messages and service messages.
/// 
/// Binary payloads are protocol buffer encoded messages.
/// 
/// Text payloads are only used for logging and debug traces,
/// no functionality available through text messages.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum MessagePayload {
    /// protocol buffer encoded bytes.
    Binary(Vec<u8>),
    /// UTF-8 encoded string.
    Text(String),
}

/// Service message route defines the message and response transfer
/// protocol for a service message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum ServiceMessageRoute {
    /// Undefined route. Used when creating messages with payload
    /// but route information not yet available and will be provided
    /// later.
    Undefined,
    /// Tell pattern with no response route.
    Tell(ServiceMessageRecipient),
    /// Ask pattern with respose.
    Ask(ClientId),
}

/// Client message route defines the message and response transfer
/// protocol for a client message.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum ClientMessageRoute {
    /// Undefined route. Used when creating messages with payload
    /// but route information not yet available and will be provided
    /// later.
    Undefined,
    /// Tell pattern with no response route.
    Tell(ClientMessageRecipient),
    /// Ask pattern with response.
    Ask(RealtimeService),
}

/// A ServiceMessage is a message that a service sends to a client.
/// The binary/bytes payload is protocol buffer encoded.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ServiceMessage {
    pub(crate) sender: RealtimeService,
    pub(crate) route: ServiceMessageRoute,
    pub(crate) payload: MessagePayload,
}

impl factor::MessageCluster for ServiceMessage {
    type Result = Option<Vec<u8>>;
}

/// The message recipient(s) of a client message.
/// The message recipient could be a single service or a broadcast topic.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) enum ClientMessageRecipient {
    /// Message intended for one Service
    Service(RealtimeService),
    /// Message intended for broadcast to topics
    Broadcast(ServiceTopics),
}

/// Client connection message: a message sent to services
/// for every new client connection and disconnection.
#[derive(Clone, Serialize, Deserialize)]
pub(crate) enum ClientConnectionMessage {
    Connect {
        client: ClientId,
        addr: factor::ActorAddr<ClientConnectionActor>,
    },
    Disconnect(ClientId),
}

impl factor::MessageCluster for ClientConnectionMessage {
    type Result = ();
}

/// Message sent to services by clients connected to a collaborative space.
/// Binary payload of messages are protocol buffer encoded.
/// When sent to specific services the message is service specific (refer
/// to protocol buffer definitions for details.)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(crate) struct ClientMessage {
    pub(crate) route: ClientMessageRoute,
    pub(crate) client: ClientId,
    pub(crate) payload: MessagePayload,
}

// Send command to the client actors.
pub(crate) enum _ClientCommands {
    // e.g. if Ticket Authetication failed or HeartBeat Failed.
    Disconnect(ClientId),
}

impl factor::MessageCluster for ClientMessage {
    type Result = Option<Vec<u8>>;
}

/// Message requesting to generate a unique id for a new client connection.
/// Ids are unique within a collaborative space, not globally.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct GenerateClientIdMessage;
impl factor::MessageCluster for GenerateClientIdMessage {
    type Result = ClientId;
}
