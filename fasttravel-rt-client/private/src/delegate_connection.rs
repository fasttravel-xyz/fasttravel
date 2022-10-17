use log::{error, trace};
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use fasttravel_rt_proto::{helpers as proto_helpers, realtime, RealtimeService};

use crate::{
    message_broker::RealtimeMessageBroker, realtime_module::ServiceDelegatePrivate,
    MessageDispatcher,
};

/// Wrapper around the ConnectionServiceKernel. We send this wrapper to JS
/// that it uses to communicate wwith the kernel.
#[wasm_bindgen]
pub struct ConnectionServiceDelegate {
    _private: Rc<ConnectionServiceKernel>,
}

#[wasm_bindgen]
impl ConnectionServiceDelegate {}

impl ServiceDelegatePrivate<ConnectionServiceKernel> for ConnectionServiceDelegate {
    fn new(private: Rc<ConnectionServiceKernel>) -> Self {
        Self { _private: private }
    }
}

///
/// Kernel responsible for the functionalities of the
/// connection service on the client side.
///
pub(crate) struct ConnectionServiceKernel {
    broker: Rc<RealtimeMessageBroker>,
    _js_dispatcher: MessageDispatcher,
}

impl ConnectionServiceKernel {
    pub(crate) fn new(broker: Rc<RealtimeMessageBroker>, js_dispatcher: MessageDispatcher) -> Self {
        Self {
            broker,
            _js_dispatcher: js_dispatcher,
        }
    }

    /// Send the ticket as first message to perform a ticket-handshake.
    pub(crate) async fn perform_websocket_ticket_handshake(&self, ticket: String) -> bool {
        trace!("perform_websocket_ticket_handshake_ticket {}", ticket);

        let req = proto_helpers::connection::create_ticket_handshake_request(ticket);
        trace!("perform_websocket_ticket_handshake_req_size {}", req.len());

        self.broker
            .send_proto_request_to_server(&RealtimeService::Connection, req)
            .await
            .map(|res| {
                trace!("send_proto_request_to_server_res_size: {}", res.len());

                if let Some(realtime::connection::message::Payload::HandshakeRes(res)) =
                    proto_helpers::connection::decode_connection_message_and_extract_payload(res)
                {
                    return res.success;
                } else {
                    error!("send_proto_request_to_server_res_decode_error");
                }

                false
            })
            .unwrap_or_else(|| {
                error!("send_proto_request_to_server_error");
                false
            })
    }

    pub(crate) async fn recv_proto_message_from_server(&self, _bytes: Vec<u8>) {
        trace!("ConnectionServiceKernel_recv_proto_message_from_server");
    }

    pub(crate) async fn answer_proto_req_from_server(&self, _bytes: Vec<u8>) -> Option<Vec<u8>> {
        trace!("ConnectionServiceKernel_answer_proto_req_from_server");
        None
    }
}
