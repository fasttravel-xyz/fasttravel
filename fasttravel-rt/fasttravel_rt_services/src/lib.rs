#![deny(unreachable_pub, private_in_public)]
#![forbid(unsafe_code)]

//! Package provides the realtime services to build a collaborative
//! space.

use core::pin::Pin;
use futures::future::Future;

use serde::{Deserialize, Serialize};

mod ids;
mod services;

pub use ids::{ClientId, CospaceId, ModelRoot};
pub use services::*;

/// streams and events that services publish and clients subscribe to.
#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub enum ClientTopics {
    /// all clients connected to the cospace receive the broadcast.
    Cospace(CospaceId),
}

/// streams and events that clients publish and services subscribe to.
#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub enum ServiceTopics {
    /// all services receive the broadcast.
    Default,
}

#[derive(Serialize, Deserialize)]
#[derive(Clone, Debug)]
pub enum ServiceMessageRecipient {
    /// send to a specific client
    Client(ClientId),
    /// send to clients passing policy (e.g. certain messages only for cospace admins/hosts)
    Broadcast(ClientTopics),
}

/// Pre-defined realtime services available to a collaborative space
/// hosting multiple clients.
///
// DEV-NOTE: Currently the services package provides pre-defined services. This
// creates a depedency on the cospace layer related to the services that
// it could use. Later we could add a ServicesProvider layer that could do
// the decoupling. The ServicesProvider could provide their own realtime-message
// proto-definitions. Currently, we don't want to invest more effort
// towards the ServicesProvider as predefined-services are enough for
// current needs.
#[derive(Clone, Debug)]
pub enum Services {
    /// Service managing Logging: Logs any message received.
    /// This is different from observability traces/logs.
    Logging,
    /// Service managing Function/Lambda/Async-Task framework.
    Function,
    /// Service managing core functionalities.
    Core,
    /// Service managing presence of a user in a cospace.
    /// Deals with avatar, status, handle, availability-for-activity.
    Presence,
    /// Service managing activities available to a client based
    /// on model-root and client-role.
    Activity,
    /// Service managing realtime models opened in the cospace.
    Model,
}

/// ProtoMessage: protocol buffer encoded bytes
pub type ProtoBytes = [u8];

/// ProtoResponse: future that resolves to protocol buffer encoded bytes
pub type ProtoResponse = Pin<Box<dyn Future<Output = Result<Vec<u8>, ProtoResponseError>> + Send>>;

/// Message Sender Error.
#[derive(Debug)]
pub struct ProtoResponseError;

/// Execution context that the service depends on to send messages to host
/// collaboration space and clients.
/// Allows dependency injection into the Service.
/// [todo]: change this to ServiceContext and add spawn() method.
pub trait ExecutionContext: Send {
    fn spawn_ok(&self, task: Pin<Box<dyn Future<Output = ()> + Send + 'static>>);
    fn tell_text(&self, recipient: ServiceMessageRecipient, text: &str);
    fn tell_encoded(&self, recipient: ServiceMessageRecipient, bytes: &ProtoBytes);
    fn ask_encoded(&self, client: ClientId, bytes: &ProtoBytes) -> ProtoResponse;
}

/// Trait Object of ExecutionContext
pub type ExecutionContextObj = Box<dyn ExecutionContext>;

/// Trait representing a Service
pub trait Service {
    fn new(dispatcher: ExecutionContextObj) -> Self;
    fn recv_connect(&mut self, client: ClientId);
    fn recv_disconnect(&mut self, client: ClientId);
    fn recv_text(&mut self, client: ClientId, text: &str);
    fn recv_encoded(&mut self, client: ClientId, bytes: &ProtoBytes);
    fn answer_encoded(&mut self, client: ClientId, bytes: &ProtoBytes) -> ProtoResponse;
}
