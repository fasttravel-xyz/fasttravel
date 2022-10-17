use dashmap::DashMap;
use futures::Future;
use std::{sync::Arc, pin::Pin};
use uuid::Uuid;

use factor;
use fasttravel_rt_proto::RealtimeService;
use fasttravel_rt_services::{
    ClientId, ClientTopics, ExecutionContext, ProtoBytes, ProtoResponse, ProtoResponseError,
    Service, ServiceMessageRecipient, Services,
};

use super::{
    ClientConnectionMessage, ClientMessage, ClientMessageRoute, CospaceActor, MessagePayload,
    ServiceMessage, ServiceMessageRoute,
};

/// Service allocation defines how collaborative spaces are going to
/// access the services. A single service could be dedicated
/// to a collaborative space or shared among multiple collaborative
/// spaces.
/// A single service-instance is backed by an factor::actor-pool
/// running on a dedicated threadpool.
#[derive(Clone)]
pub(crate) enum ServiceAllocation {
    /// service-instance dedicated to a cospace instance
    Dedicated(factor::ActorWeakAddr<CospaceActor>),

    /// service-instance shared by all cospace instances
    Shared(Arc<DashMap<Uuid, factor::MessageClusterAddr<ServiceMessage>>>),
}

/// Actor that owns and runs a Service.
/// Actors of this type are launched in an Actor-pool (same service running in
/// multiple actors and threads on a shared task-queue). A Service could be
/// dedicated to a single cospace or shared among multiple cospaces.
pub(crate) struct ServiceActor<S>
where
    S: Service + Send + 'static,
{
    service: S,
}

impl<S> ServiceActor<S>
where
    S: Service + Send + 'static,
{
    /// Create a new service actor (dedicated or shared depending on allocation)
    pub(crate) fn new(s_type: Services, allocation: ServiceAllocation, system: &factor::SystemRef) -> Self {
        Self {
            service: S::new(Box::new(ServiceExecutionContext::new(s_type, allocation, system))),
        }
    }
}

impl<S> factor::ActorReceiver for ServiceActor<S>
where
    S: Service + Send,
{
    type Context = factor::BasicContext<Self>;
}

// Handle ClientConnectionMessage.
impl<S> factor::MessageClusterHandler<ClientConnectionMessage> for ServiceActor<S>
where
    S: Service + Send,
{
    type Result =
        factor::MessageResponseType<<ClientConnectionMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ClientConnectionMessage, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ClientConnectionMessage::Connect { client, addr: _ } => {
                self.service.recv_connect(client)
            }
            ClientConnectionMessage::Disconnect(client) => self.service.recv_disconnect(client),
        }

        factor::MessageResponseType::Result(().into())
    }
}

// Handle ClientMessage.
impl<S> factor::MessageClusterHandler<ClientMessage> for ServiceActor<S>
where
    S: Service + Send,
{
    type Result = factor::MessageResponseType<<ClientMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ClientMessage, _ctx: &mut Self::Context) -> Self::Result {
        tracing::trace!(target: "server-event", "service_actor_handle_client_message");

        match msg.route {
            ClientMessageRoute::Tell(_) => match msg.payload {
                MessagePayload::Text(text) => self.service.recv_text(msg.client, &text),
                MessagePayload::Binary(bytes) => self.service.recv_encoded(msg.client, &bytes),
            },
            ClientMessageRoute::Ask(_) => match msg.payload {
                MessagePayload::Binary(bytes) => {
                    let proto_res = self.service.answer_encoded(msg.client, &bytes);
                    let t = async {
                        if let Ok(encoded_bytes_vec) = proto_res.await {
                            return Some(encoded_bytes_vec);
                        } else {
                            return None;
                        }
                    };
                    return factor::MessageResponseType::Future(Box::pin(t));
                }
                _ => {
                    tracing::warn!(target: "server-event", "service_actor_ask_only_supported_for_binary_proto_payload");
                }
            },
            _ => {
                tracing::warn!(target: "server-event", "service_actor_unhandled_client_message_route");
            }
        }

        factor::MessageResponseType::Result(None.into())
    }
}

/// The message dispatcher of a service. The uses the message
/// dispatcher to send messages to clients via the cospace.
/// [todo] Rename to ServiceExecutionContext.
pub(crate) struct ServiceExecutionContext {
    s_type: Services,
    allocation: ServiceAllocation,
    system: factor::SystemRef,
}

impl ServiceExecutionContext {
    /// Create new service message dispatcher.
    pub(crate) fn new(s_type: Services, allocation: ServiceAllocation, system: &factor::SystemRef) -> Self {
        Self { s_type, allocation, system: system.clone() }
    }

    /// Get the cospace message address to send messages.
    /// A service could dedicated or shared, so we need to find the
    /// correct cospace address.
    /// We won't need this if we make all services dedicated. Want to have some usage
    /// data before making that decision. Also if we decide to launch each cospace
    /// in its dedicated process, we won't need this as by default services will be dedicated.
    fn cospace_addr(
        &self, recipient: &ServiceMessageRecipient,
    ) -> Option<factor::MessageClusterAddr<ServiceMessage>> {
        match &self.allocation {
            ServiceAllocation::Dedicated(addr) => {
                addr.upgrade().map(|addr| addr.message_cluster_addr())
            }
            ServiceAllocation::Shared(map) => {
                let key = match recipient {
                    ServiceMessageRecipient::Client(client) => client.cospace.uuid,
                    ServiceMessageRecipient::Broadcast(topic) => match topic {
                        ClientTopics::Cospace(cospace) => cospace.uuid,
                    },
                };

                if let Some(addr) = map.get(&key) {
                    return Some(addr.clone());
                } else {
                    tracing::error!(target: "server-event", "cospace_id_not_found_in_shared_service");
                    return None;
                }
            }
        }
    }
}

impl ExecutionContext for ServiceExecutionContext {
    fn tell_text(&self, recipient: ServiceMessageRecipient, text: &str) {
        tracing::debug!(target: "server-event", "service_context_tell_text: {}", text);

        if let Some(addr) = self.cospace_addr(&recipient) {
            let msg = ServiceMessage {
                sender: get_realtime_service_from_server_service(&self.s_type),
                route: ServiceMessageRoute::Tell(recipient),
                payload: MessagePayload::Text(text.to_string()),
            };

            addr.tell(msg).map_err(|e| tracing::error!(target: "server-event", "service_context_tell_text_to_client_failed: {}", e)).err();
        } else {
            tracing::error!(target: "server-event", "cospace_recipient_not_found_in_service");
        }
    }

    fn tell_encoded(&self, recipient: ServiceMessageRecipient, bytes: &ProtoBytes) {
        if let Some(addr) = self.cospace_addr(&recipient) {
            let msg = ServiceMessage {
                sender: get_realtime_service_from_server_service(&self.s_type),
                route: ServiceMessageRoute::Tell(recipient),
                payload: MessagePayload::Binary(bytes.to_vec()),
            };

            addr.tell(msg).map_err(|e| tracing::error!(target: "server-event", "service_context_tell_encoded_to_client_failed: {}", e)).err();
        } else {
            tracing::error!(target: "server-event", "cospace_recipient_not_found_in_service");
        }
    }

    fn ask_encoded(&self, client: ClientId, bytes: &ProtoBytes) -> ProtoResponse {
        if let Some(addr) = self.cospace_addr(&ServiceMessageRecipient::Client(client.clone())) {
            let msg = ServiceMessage {
                sender: get_realtime_service_from_server_service(&self.s_type),
                route: ServiceMessageRoute::Ask(client),
                payload: MessagePayload::Binary(bytes.to_vec()),
            };

            let response = Box::pin(async move {
                addr.ask(msg)
                    .await
                    .and_then(|result| result.ok_or(factor::MessageSendError::ErrDefault))
                    .map_err(|_e| ProtoResponseError)
            });

            return response;
        } else {
            tracing::error!(target: "server-event", "cospace_recipient_not_found_in_service");
        }

        Box::pin(async { Err(ProtoResponseError) })
    }

    fn spawn_ok(&self, task: Pin<Box<dyn Future<Output = ()> + Send + 'static>>) {
        self.system.spawn_ok(task);
    }
}

/// Get the proto service enum from server service enum.
fn get_realtime_service_from_server_service(service: &Services) -> RealtimeService {
    match service {
        Services::Core => RealtimeService::Core,
        Services::Presence => RealtimeService::Presence,
        Services::Activity => RealtimeService::Activity,
        Services::Model => RealtimeService::Model,
        _ => RealtimeService::Undefined,
    }
}
