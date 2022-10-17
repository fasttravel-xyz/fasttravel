use flume;
use futures::{sink::SinkExt, stream::StreamExt};

use factor::{self, ActorReceiverContext};
use fasttravel_rt_services::*;

use super::{CreateClientConnectionActorMessage, SocketMessage};
use crate::{
    axum, ClientConnectionActor, ClientConnectionMessage, ClientConnectionServiceActor,
    GenerateClientIdMessage,
};

/// Client connection actor creator.
pub(crate) struct ClientConnectionActorCreator {
    session_request_decoding: jsonwebtoken::DecodingKey,
}

impl factor::ActorReceiver for ClientConnectionActorCreator {
    type Context = factor::BasicContext<Self>;
}

// Handle CreateClientConnectionActorMessage request.
impl factor::MessageHandler<CreateClientConnectionActorMessage> for ClientConnectionActorCreator {
    type Result = factor::MessageResponseType<
        <CreateClientConnectionActorMessage as factor::Message>::Result,
    >;

    fn handle(
        &mut self, msg: CreateClientConnectionActorMessage, ctx: &mut Self::Context,
    ) -> Self::Result {
        let session_request_decoding = self.session_request_decoding.clone();
        let fut = Self::create_client_actor(msg, ctx.system(), session_request_decoding);

        factor::MessageResponseType::Future(Box::pin(fut))
    }
}

impl ClientConnectionActorCreator {
    pub(crate) fn new(session_request_decoding: jsonwebtoken::DecodingKey) -> Self {
        Self {
            session_request_decoding,
        }
    }

    /// Create the cllient connection actor for a new client connection.
    async fn create_client_actor(
        msg: CreateClientConnectionActorMessage, sys: factor::SystemRef,
        session_request_decoding: jsonwebtoken::DecodingKey,
    ) -> Option<(ClientId, factor::ActorAddr<ClientConnectionActor>)> {
        // create the channels
        let (mut socket_tx, mut socket_rx) = msg.socket.split();
        let (tx, rx) = flume::unbounded::<axum::Message>();

        // generate the client id
        let client_id;
        if let Ok(generated_id) = msg.cospace_addr.ask_addr(GenerateClientIdMessage).await {
            client_id = generated_id;
        } else {
            return None;
        }

        // create the connection service actor
        let config = factor::ActorBuilderConfig::default();
        let mut client_id_moved = client_id.clone();
        let item = factor::ActorBuilder::create(
            move |_| {
                ClientConnectionServiceActor::new(
                    client_id_moved.clone(),
                    session_request_decoding.clone(),
                )
            },
            &sys,
            config,
        );
        let conn_service_addr = sys.run_actor(item.unwrap());
        let moved_conn_service_addr = conn_service_addr.clone();

        // create the client connection actor.
        client_id_moved = client_id.clone();
        let factory = move |_| {
            ClientConnectionActor::new(
                client_id_moved.clone(),
                msg.cospace_addr.message_cluster_addr(),
                moved_conn_service_addr.clone(),
                tx.clone(),
            )
        };
        let config = factor::ActorBuilderConfig::default();
        let spawn_item = factor::ActorBuilder::create(factory, &sys, config);
        let client_addr = sys.run_actor(spawn_item.unwrap());

        // create the outgoing socket message looper.
        let fut_socket_tx = async move {
            while let Ok(msg) = rx.recv_async().await {
                // forward outgoing messages from client connection actor to socket.
                if let Err(e) = socket_tx.send(msg).await {
                    tracing::error!(target: "server-event", "socket_outgoing_message_send_failed: {}", e);
                }
            }
        };
        sys.spawn_ok(fut_socket_tx);

        // create the incoming socket message looper.
        let msg_addr = client_addr.message_addr::<SocketMessage>();
        let fut_socket_rx = async move {
            while let Some(msg) = socket_rx.next().await {
                if let Ok(msg) = msg {
                    // forward incoming socket messages to client connection actor.
                    tracing::trace!(target: "server-event", "socket_message_received");

                    let _ = msg_addr.tell(SocketMessage(msg));
                } else {
                    tracing::debug!(target: "server-event", "client_disconnected");
                    // [todo] inform client_connection_actor
                    return;
                }
            }
        };
        sys.spawn_ok(fut_socket_rx);

        // notify the connection service actor
        let _ = conn_service_addr.tell_addr(ClientConnectionMessage::Connect {
            client: client_id.clone(),
            addr: client_addr.clone(),
        });

        Some((client_id, client_addr))
    }
}
