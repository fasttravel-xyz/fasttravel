use crate::{ClientConnectionMessage, ClientMessage, MessagePayload, ServiceMessage};
use fasttravel_rt_services::ClientId;

use fasttravel_rt_proto::{
    helpers::{self as proto_helpers},
    realtime::{self, connection::TicketHandshakeRequest},
};
use jsonwebtoken;

use crate::TicketClaimsMessage;

pub(crate) struct ClientConnectionServiceActor {
    client_id: ClientId,
    client_service_msg_addr: Option<factor::MessageClusterAddr<ServiceMessage>>,
    session_request_decoding: jsonwebtoken::DecodingKey,
    // heartbeat_interval: Duration,
}

impl factor::ActorReceiver for ClientConnectionServiceActor {
    type Context = factor::BasicContext<Self>;
}

// Handle ClientConnectionMessage requests.
impl factor::MessageClusterHandler<ClientConnectionMessage> for ClientConnectionServiceActor {
    type Result =
        factor::MessageResponseType<<ClientConnectionMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ClientConnectionMessage, _ctx: &mut Self::Context) -> Self::Result {
        match msg {
            ClientConnectionMessage::Connect { client, addr } => {
                assert_eq!(self.client_id, client);
                self.client_service_msg_addr = Some(addr.message_cluster_addr());
            }
            ClientConnectionMessage::Disconnect(client_id) => {
                assert_eq!(self.client_id, client_id);
                self.client_service_msg_addr = None;
            }
        }

        factor::MessageResponseType::Result(().into())
    }
}

// Handle ClientMessage requests.
impl factor::MessageClusterHandler<ClientMessage> for ClientConnectionServiceActor {
    type Result = factor::MessageResponseType<<ClientMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: ClientMessage, _ctx: &mut Self::Context) -> Self::Result {
        tracing::debug!(target: "server-event", "client_conn_service_actor_handle_client_conn_message");

        match msg.payload {
            MessagePayload::Binary(bytes) => {
                if let Some(payload) =
                    proto_helpers::connection::decode_connection_message_and_extract_payload(bytes)
                {
                    match payload {
                        realtime::connection::message::Payload::HandshakeReq(req) => {
                            let response = self.handle_handshake_request(req);
                            return factor::MessageResponseType::Result(response.into());
                        }
                        _ => {
                            tracing::warn!(target: "server-event", "client_conn_service_actor_unhandled_payload_received");
                        }
                    }
                }
            }
            _ => {
                tracing::warn!(target: "server-event", "client_conn_service_actor_unhandled_payload_received");
            }
        }

        factor::MessageResponseType::Result(None.into())
    }
}

impl ClientConnectionServiceActor {
    pub(crate) fn new(
        client_id: ClientId, session_request_decoding: jsonwebtoken::DecodingKey,
    ) -> Self {
        Self {
            client_id,
            client_service_msg_addr: None,
            session_request_decoding,
        }
    }

    fn handle_handshake_request(&self, req: TicketHandshakeRequest) -> Option<Vec<u8>> {
        let validation = TicketClaimsMessage::validation();
        let mut success = false;

        tracing::debug!(target: "server-event", "client_conn_service_actor_handle_handshake_request");

        match jsonwebtoken::decode::<TicketClaimsMessage>(
            req.ticket.as_str(),
            &self.session_request_decoding,
            &validation,
        ) {
            Ok(_data) => success = true,
            Err(e) => {
                tracing::error!(target: "server-event", "client_conn_service_actor_handshake_ticket_auth_error: {}", e);
            }
        }

        let response = proto_helpers::connection::create_ticket_handshake_response(success);
        Some(response)
    }
}
