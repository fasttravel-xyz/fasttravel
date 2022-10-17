use factor::{self, ActorReceiverContext};

use super::WebsocketOnUpgradeMessage;
use crate::{
    ClientConnectionActorCreator, ClientConnectionMessage, CreateClientConnectionActorMessage,
};

/// Websocket service actor handling new client connections.
/// Creates a client connection actor on every new client connection.
pub(crate) struct WebsocketServiceActor {
    cnx_creator: factor::ActorAddr<ClientConnectionActorCreator>,
}

impl WebsocketServiceActor {
    pub(crate) fn new(
        system: &factor::SystemRef, session_request_decoding: jsonwebtoken::DecodingKey,
    ) -> Self {
        let config = factor::ActorBuilderConfig::default();
        let factory = move |_| ClientConnectionActorCreator::new(session_request_decoding.clone());
        let spawn_item = factor::ActorBuilder::create(factory, system, config);
        let cnx_creator = system.run_actor(spawn_item.unwrap());
        Self { cnx_creator }
    }
}

impl factor::ActorReceiver for WebsocketServiceActor {
    type Context = factor::BasicContext<Self>;
}

// Handle new client websocket connections
impl factor::MessageHandler<WebsocketOnUpgradeMessage> for WebsocketServiceActor {
    type Result = factor::MessageResponseType<()>;

    fn handle(&mut self, msg: WebsocketOnUpgradeMessage, ctx: &mut Self::Context) -> Self::Result {
        let fut = Self::handle_on_upgrade(msg, ctx.system(), self.cnx_creator.clone());
        ctx.spawn_ok(async {
            fut.await;
        });

        factor::MessageResponseType::Result(().into())
    }
}

impl WebsocketServiceActor {
    async fn handle_on_upgrade(
        msg: WebsocketOnUpgradeMessage, _sys: factor::SystemRef,
        cnx_creator: factor::ActorAddr<ClientConnectionActorCreator>,
    ) {
        match cnx_creator
            .ask(CreateClientConnectionActorMessage {
                socket: msg.socket,
                cospace_addr: msg.cospace_addr.clone(),
            })
            .await
        {
            Ok(Some((id, addr))) => {
                // inform cospace of new client connection.
                let _ = msg
                    .cospace_addr
                    .tell_addr(ClientConnectionMessage::Connect { client: id, addr });
            }
            Ok(None) => {
                tracing::error!(target: "server-event", "error_in_ask_create_client_connection: ask_returns_None");
            }
            Err(e) => {
                tracing::error!(target: "server-event", "error_in_ask_create_client_connection: {}", e);
            }
        };
    }
}
