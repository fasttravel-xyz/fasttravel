use log::{error, trace};
use std::rc::Rc;
use wasm_bindgen::prelude::*;

use crate::{
    message_broker::RealtimeMessageBroker, realtime_module::ServiceDelegatePrivate,
    CoreEventMessage, MessageDispatcher, EventEnvelope,
};

/// Wrapper around the CoreServiceKernel. We send this wrapper to JS.
#[wasm_bindgen]
pub struct CoreServiceDelegate {
    private: Rc<CoreServiceKernel>,
}

/// All communications from JS are through the delegate.
#[wasm_bindgen]
impl CoreServiceDelegate {
    /// Send a message to the server and broadcast to all services.
    pub async fn send_text_message(&self, msg: &str) {
        self.private.send_text_message_to_server(msg).await
    }
}

impl ServiceDelegatePrivate<CoreServiceKernel> for CoreServiceDelegate {
    fn new(private: Rc<CoreServiceKernel>) -> Self {
        Self { private }
    }
}

///
/// Kernel responsible for the functionalities of the core-service on the client side.
/// Receives server-messages from the MessageBroker in recv_server(), answer_server() and
/// after processing and needed computation passes required message to the JS-CoreService object.
/// Receives client-messagse from the JS-CoreService object in various fetch/send-methods
/// and after processing and needed computation passes required message to the MessageBroker.
///
pub(crate) struct CoreServiceKernel {
    broker: Rc<RealtimeMessageBroker>,
    js_dispatcher: MessageDispatcher,
}

impl CoreServiceKernel {
    pub(crate) fn new(broker: Rc<RealtimeMessageBroker>, js_dispatcher: MessageDispatcher) -> Self {
        Self {
            broker,
            js_dispatcher,
        }
    }

    /// Send a text message to the server.
    pub async fn send_text_message_to_server(&self, msg: &str) {
        self.broker.send_text_message_to_server(msg).await
    }

    /// Receive a text message from the server.
    pub(crate) async fn recv_text_message_from_server(&self, text: String) {
        let msg = CoreEventMessage::new(text.as_str());
        let env = EventEnvelope::new(msg.into());

        self.js_dispatcher.recv_message(env).await
            .map_err(|e| error!("js_dispatcher_recv_message_error {:#?}", e))
            .err();
    }

    pub(crate) async fn recv_proto_message_from_server(&self, _service_payload: Vec<u8>) {
        trace!("CoreServiceKernel_recv_proto_message_from_server");
    }

    pub(crate) async fn answer_proto_req_from_server(
        &self,
        _service_payload: Vec<u8>,
    ) -> Option<Vec<u8>> {
        trace!("CoreServiceKernel_answer_proto_req_from_server");
        None
    }
}
