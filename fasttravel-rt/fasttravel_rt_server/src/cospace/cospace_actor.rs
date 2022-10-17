use std::collections::HashMap;

use factor;
use fasttravel_rt_proto::RealtimeService;
use fasttravel_rt_services::{ClientId, CospaceId, ServiceMessageRecipient, Services};

use super::{
    service_pool::{ServicePool, ServicePoolMessenger},
    ClientConnectionMessage, ClientMessage, ClientMessageRecipient, ClientMessageRoute,
    GenerateClientIdMessage, ModelRoot, ServiceMessage, ServiceMessageRoute,
};

/// An actor representing a collaborative space.
/// A collaborative space host multiple clients and a model-root.
/// It depends on the realtime service pool to handle client requests.
#[derive(Clone)]
pub(crate) struct CospaceActor {
    cospace: CospaceId,
    #[allow(dead_code)]
    model_root: ModelRoot,
    client_id_counter: u32,
    services: ServicePool,
    clients: HashMap<u32, factor::MessageClusterAddr<ServiceMessage>>,
}

impl CospaceActor {
    pub(crate) fn new(cospace: CospaceId, model_root: ModelRoot, services: ServicePool) -> Self {
        Self {
            cospace,
            model_root,
            client_id_counter: 1,
            services,
            clients: HashMap::new(),
        }
    }
}

impl factor::ActorReceiver for CospaceActor {
    type Context = factor::BasicContext<Self>;
}

// Handle GenerateClientIdMessage requests.
impl factor::MessageClusterHandler<GenerateClientIdMessage> for CospaceActor {
    type Result =
        factor::MessageResponseType<<GenerateClientIdMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, _msg: GenerateClientIdMessage, _ctx: &mut Self::Context) -> Self::Result {
        let id = ClientId {
            id: self.client_id_counter,
            cospace: self.cospace.clone(),
        };
        self.client_id_counter += 1;

        factor::MessageResponseType::Result(id.into())
    }
}

// Handle ServiceMessage requests.
impl factor::MessageClusterHandler<ServiceMessage> for CospaceActor {
    type Result = factor::MessageResponseType<<ServiceMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ServiceMessage, _ctx: &mut Self::Context) -> Self::Result {
        tracing::trace!(target: "server-event", "cospace_actor_handle_outgoing_service_message_to_clients");

        // forward ServiceMessage to clients
        match &msg.route {
            ServiceMessageRoute::Tell(recipient) => {
                match recipient {
                    ServiceMessageRecipient::Client(client) => {
                        if let Some(addr) = self.clients.get(&client.id) {
                            addr.tell(msg).map_err(|e| tracing::error!(target: "server-event", "cospace_tell_to_client_failed: {}", e)).err();
                        } else {
                            tracing::error!(target: "server-event", "receipient_client_id_not_found_in_cospace");
                        }
                    }
                    ServiceMessageRecipient::Broadcast(_topic) => {
                        // [todo] add role-based authorization control to both
                        // subscription and dispatch of broascast events and streams.
                        for addr in self.clients.values() {
                            let _ = addr.tell(msg.clone());
                        }
                    }
                }
            }
            ServiceMessageRoute::Ask(client) => {
                let ret = self
                    .clients
                    .get(&client.id)
                    .map(|addr| {
                        let addr_moved = addr.clone();

                        let response = Box::pin(async move {
                            addr_moved
                                .ask(msg)
                                .await
                                .map_err(|e| {
                                    tracing::error!(target: "server-event", "cospace_actor_handle_service_msg_ask_client_error: {:?}", e)
                                })
                                .unwrap_or(None)
                        });

                        factor::MessageResponseType::Future(response)
                    })
                    .unwrap_or(factor::MessageResponseType::Result(None.into()));

                return ret;
            }
            _ => {}
        }

        factor::MessageResponseType::Result(None.into())
    }
}

// Handle ClientConnectionMessage requests.
impl factor::MessageClusterHandler<ClientConnectionMessage> for CospaceActor {
    type Result =
        factor::MessageResponseType<<ClientConnectionMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ClientConnectionMessage, _ctx: &mut Self::Context) -> Self::Result {
        match &msg {
            ClientConnectionMessage::Connect { client, addr } => {
                self.clients.insert(client.id, addr.message_cluster_addr());
                self.services.broadcast(msg);
            }
            ClientConnectionMessage::Disconnect(client_id) => {
                self.clients.remove(&client_id.id);
            }
        }

        factor::MessageResponseType::Result(().into())
    }
}

// Handle ClientMessage requests.
impl factor::MessageClusterHandler<ClientMessage> for CospaceActor {
    type Result = factor::MessageResponseType<<ClientMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ClientMessage, _ctx: &mut Self::Context) -> Self::Result {
        match &msg.route {
            ClientMessageRoute::Tell(recipient) => {
                match recipient {
                    ClientMessageRecipient::Service(service) => {
                        let service = get_server_service_from_realtime_service(service);
                        self.services.tell(&service, msg);
                    }
                    ClientMessageRecipient::Broadcast(_topic) => {
                        // [todo] do not broadcast directly to services. push to topic-streams.
                        // topic-streams not implemented yet.
                        self.services.broadcast(msg);
                    }
                }
            }
            ClientMessageRoute::Ask(service) => {
                let service = get_server_service_from_realtime_service(service);
                if let Some(addr) = self.services.message_addr(&service) {
                    let response = Box::pin(async move {
                        addr.ask(msg)
                            .await
                            .map_err(|e| {
                                tracing::error!(target: "server-event", "cospace_actor_handle_client_msg_ask_service_error: {:?}", e)
                            })
                            .unwrap_or(None)
                    });

                    return factor::MessageResponseType::Future(response);
                }
            }
            _ => {}
        }

        factor::MessageResponseType::Result(None.into())
    }
}

/// Get the server services from the proto service enum.
fn get_server_service_from_realtime_service(proto_service: &RealtimeService) -> Services {
    match proto_service {
        RealtimeService::Core => Services::Core,
        RealtimeService::Activity => Services::Activity,
        RealtimeService::Presence => Services::Presence,
        RealtimeService::Model => Services::Model,
        _ => Services::Logging,
    }
}
