use flume;
use futures::{channel::oneshot, Future};
use std::collections::HashMap;
use std::sync::Arc;

use factor::{self, ActorReceiverContext};

use fasttravel_rt_proto::{
    helpers::{self as proto_helpers, ProtoMessage, ProtoPayloadResponse, ProtoPayloadTell},
    RealtimeService,
};
use fasttravel_rt_services::{ClientId, ServiceTopics};

use super::SocketMessage;
use crate::{
    axum, ClientConnectionServiceActor, ClientMessage, ClientMessageRecipient, ClientMessageRoute,
    MessagePayload, ServiceMessage, ServiceMessageRoute,
};

// ====================================================================
// NOTE: REQUEST-RESPONSE PATTERN SUPPORT
// ====================================================================
// When we receive a Request from the client with a request_id we create a
// promise (backed by a future chain). On completion of that promise we return the
// response with the appropriate response_id.
// In this case, instead of creating a future-chain we could have passed the request_id
// (inside the ClientMessage) to the services and wait for a response from the service
// with the response_id (inside the ServiceMessage). This would have avoided the
// future-chain, but now the services have to deal with request_id/response_id, instead
// we want services to simply reply with a future when "asked". If this decision has
// memory or performance implications we could go with the approach of passing the
// request_id to the services, but only after perf-testing.
// ====================================================================

/// Response promise which sends the response on complete.
#[derive(Debug)]
struct ResponsePromise {
    tx: oneshot::Sender<Vec<u8>>,
}

impl ResponsePromise {
    fn complete(self, encoded_bytes: Vec<u8>) {
        let _ = self.tx.send(encoded_bytes);
    }
}

/// Client connection actor representing a single client connection to the realtime server.
#[derive(Clone)]
pub(crate) struct ClientConnectionActor {
    client_id: ClientId,
    cospace_addr: factor::MessageClusterAddr<ClientMessage>,
    conn_service_addr: factor::ActorAddr<ClientConnectionServiceActor>,
    socket_tx_feeder: flume::Sender<axum::Message>,
    request_id_counter: u32,
    service_promises: HashMap<u32, Arc<ResponsePromise>>,
}

impl factor::ActorReceiver for ClientConnectionActor {
    type Context = factor::BasicContext<Self>;
}

// Handle incoming socket messages.
impl factor::MessageHandler<SocketMessage> for ClientConnectionActor {
    type Result = factor::MessageResponseType<<SocketMessage as factor::Message>::Result>;

    fn handle(&mut self, msg: SocketMessage, ctx: &mut Self::Context) -> Self::Result {
        tracing::trace!(target: "server-event", "client_conn_actor_handle_socket_message");

        self.recv_socket_msg_from_client(msg, ctx);

        factor::MessageResponseType::Result(().into())
    }
}

// Handle outgoing service messages.
impl factor::MessageClusterHandler<ServiceMessage> for ClientConnectionActor {
    type Result = factor::MessageResponseType<<ServiceMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ServiceMessage, ctx: &mut Self::Context) -> Self::Result {
        // return the promise, if promise is None then request was Tell not Ask
        if let Some(promise_fut) = self.recv_msg_from_service(msg, ctx) {
            return factor::MessageResponseType::Future(Box::pin(promise_fut));
        }

        factor::MessageResponseType::Result(None.into())
    }
}

impl ClientConnectionActor {
    pub(crate) fn new(
        client_id: ClientId, cospace_addr: factor::MessageClusterAddr<ClientMessage>,
        conn_service_addr: factor::ActorAddr<ClientConnectionServiceActor>,
        tx: flume::Sender<axum::Message>,
    ) -> Self {
        Self {
            client_id,
            cospace_addr,
            conn_service_addr,
            socket_tx_feeder: tx,
            request_id_counter: 0,
            service_promises: HashMap::new(),
        }
    }

    /// Process the message received over the socket from the client.
    fn recv_socket_msg_from_client(
        &mut self, msg: SocketMessage, ctx: &mut <Self as factor::ActorReceiver>::Context,
    ) {
        match msg.0 {
            axum::Message::Binary(proto_bytes) => {
                self.recv_proto_msg_from_client(proto_bytes, ctx);
            }
            axum::Message::Text(text) => self.recv_text_message_from_client(text),
            _ => {
                tracing::warn!(target: "server-event", "unhandled_socket_message_payload");
            }
        }
    }

    /// Process the service message received from the services.
    pub(crate) fn recv_msg_from_service(
        &mut self, msg: ServiceMessage, ctx: &mut <Self as factor::ActorReceiver>::Context,
    ) -> Option<impl Future<Output = Option<Vec<u8>>>> {
        match &msg.route {
            ServiceMessageRoute::Ask(_) => {
                return self.recv_req_from_service(msg, ctx);
            }
            ServiceMessageRoute::Tell(_) => {
                self.recv_tell_from_service(msg);
            }
            _ => {
                tracing::warn!(target: "server-event", "unhandled_service_message_route");
            }
        }

        None
    }

    #[inline(always)]
    fn recv_proto_tell_from_client(&mut self, payload: ProtoPayloadTell) {
        // create the client message
        let client_msg = ClientMessage {
            route: ClientMessageRoute::Tell(ClientMessageRecipient::Service(
                payload.service.clone(),
            )),
            client: self.client_id.clone(),
            payload: MessagePayload::Binary(payload.bytes),
        };

        // Route to proper services
        if payload.service == RealtimeService::Connection {
            let _ = self.conn_service_addr.tell_addr(client_msg);
        } else {
            let _ = self.cospace_addr.tell(client_msg);
        }
    }

    #[inline(always)]
    fn recv_proto_req_from_client(
        &mut self, payload: proto_helpers::ProtoPayloadRequest,
        ctx: &mut <Self as factor::ActorReceiver>::Context,
    ) {
        // create the client message
        let client_msg = ClientMessage {
            client: self.client_id.clone(),
            route: ClientMessageRoute::Ask(payload.service.clone()),
            payload: MessagePayload::Binary(payload.bytes),
        };

        // Route to proper services
        let addr_client_msg_handler;
        if payload.service == RealtimeService::Connection {
            addr_client_msg_handler = self.conn_service_addr.message_cluster_addr();
        } else {
            addr_client_msg_handler = self.cospace_addr.clone();
        }

        // send the ask-request to the service
        let response_promise = async move {
            // wait for the ask-response
            if let Ok(Some(response_bytes)) = addr_client_msg_handler.ask(client_msg).await {
                if let Some(res_msg) = proto_helpers::create_response_message_from_service_payload(
                    payload.request_id,
                    &payload.service,
                    response_bytes,
                ) {
                    return Some(SocketMessage(axum::Message::Binary(res_msg)));
                } else {
                    tracing::error!(target: "server-event", "proto_response_msg_creation_from_service_payload_failed");
                }
            };

            None
        };

        let tx = self.socket_tx_feeder.clone();

        let task = async move {
            if let Some(socket_msg) = response_promise.await {
                // tx.send() never blocks, so doesn't guarantee delivery.
                if let Err(e) = tx.send(socket_msg.0) {
                    tracing::error!(target: "server-event", "client_message_response_send_failed: {}", e);
                }
            }
        };

        ctx.spawn_ok(task);
    }

    #[inline(always)]
    fn recv_proto_res_from_client(&mut self, payload: ProtoPayloadResponse) {
        // send the response from client to the service
        if let Some(promise_handle) = self.service_promises.remove(&payload.response_id) {
            let promise = Arc::try_unwrap(promise_handle)
                .expect("expected_as_clone_is_just_to_satisfy_bound");
            promise.complete(payload.bytes);
        } else {
            tracing::error!(target: "server-event", "client_message_req_promise_not_found");
        }
    }

    #[inline(always)]
    fn recv_proto_msg_from_client(
        &mut self, proto_bytes: Vec<u8>, ctx: &mut <Self as factor::ActorReceiver>::Context,
    ) {
        tracing::debug!(target: "server-event", "client_conn_actor_recv_proto_msg_from_client");

        let proto_msg = proto_helpers::process_realtime_message(proto_bytes);

        match proto_msg {
            ProtoMessage::Tell(payload) => {
                tracing::trace!(target: "server-event", "client_conn_actor_recv_proto_tell_from_client_before");

                self.recv_proto_tell_from_client(payload);
            }
            ProtoMessage::Request(payload) => {
                tracing::trace!(target: "server-event", "client_conn_actor_recv_proto_req_from_client_before");

                self.recv_proto_req_from_client(payload, ctx);
            }
            ProtoMessage::Response(payload) => {
                tracing::trace!(target: "server-event", "client_conn_actor_recv_proto_res_from_client_before");

                self.recv_proto_res_from_client(payload);
            }
            _ => {
                tracing::error!(target: "server-event", "client_conn_actor_recv_proto_msg_from_client_error");
            }
        }
    }

    #[inline(always)]
    fn recv_text_message_from_client(&mut self, text: String) {
        tracing::debug!(target: "server-event",
            "ClientConnectionActor_recv_text_msg_from_client {}",
            text.as_str()
        );

        // create the client message.
        let client_msg = ClientMessage {
            // Text messages are always broadcast. Keeping it so that client-sdk
            // could use it for easy traces, no functionality through Text.
            route: ClientMessageRoute::Tell(ClientMessageRecipient::Broadcast(
                ServiceTopics::Default,
            )),
            client: self.client_id.clone(),
            payload: MessagePayload::Text(text),
        };

        let _ = self.cospace_addr.tell(client_msg);
    }

    #[inline(always)]
    fn recv_req_from_service(
        &mut self, msg: ServiceMessage, _ctx: &mut <Self as factor::ActorReceiver>::Context,
    ) -> Option<impl Future<Output = Option<Vec<u8>>>> {
        if let MessagePayload::Binary(proto_bytes) = msg.payload {
            let request_id = self.request_id_counter;
            self.request_id_counter += 1;

            if let Some(proto_msg) = proto_helpers::create_request_message_from_service_payload(
                request_id,
                &msg.sender,
                proto_bytes,
            ) {
                let a_msg = axum::Message::Binary(proto_msg);

                // socket_tx_feeder.send() never blocks so delivery not guaranteed.
                if let Err(e) = self.socket_tx_feeder.send(a_msg) {
                    tracing::error!(target: "server-event", "outgoing_service_message_req_send_failed: {}", e);
                } else {
                    // create the response promise.
                    let (tx, rx) = oneshot::channel::<Vec<u8>>();
                    let promise_fut = async move {
                        if let Ok(encoded_bytes) = rx.await {
                            return Some(encoded_bytes);
                        }
                        None
                    };

                    // [todo] schedule a task that will invalidate the promise after a duration.
                    let promise = ResponsePromise { tx };
                    self.service_promises.insert(request_id, Arc::new(promise));

                    return Some(promise_fut);
                }
            }
        } else {
            tracing::error!(target: "server-event", "service_message_ask_only_supported_for_binary_not_text");
        }

        None
    }

    #[inline(always)]
    fn recv_tell_from_service(&mut self, msg: ServiceMessage) {
        let a_msg = match msg.payload {
            MessagePayload::Text(t) => axum::Message::Text(t),
            MessagePayload::Binary(b) => axum::Message::Binary(b),
        };

        tracing::debug!(target: "server-event", "client_conn_actor_recv_tell_from_service: {}", self.client_id.id);

        // socket_tx_feeder.send() never blocks so delivery not guaranteed.
        if let Err(e) = self.socket_tx_feeder.send(a_msg) {
            tracing::error!(target: "server-event", "outgoing_service_msg_tell_send_failed: {}", e);
        }
    }
}
