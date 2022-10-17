use wasm_bindgen::prelude::*;

mod delegate_connection;
mod delegate_core;
mod message_broker;
mod realtime_module;
mod websocket_connection;

pub use delegate_connection::ConnectionServiceDelegate;
pub use delegate_core::CoreServiceDelegate;
pub use realtime_module::{RealtimeModule, RealtimeModuleConfig, SessionModelRoot};

// Import development and debug helpers.
#[wasm_bindgen]
extern "C" {
    pub(crate) fn alert(msg: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub(crate) fn console_log(msg: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub(crate) fn console_log_pair(msg_a: &str, msg_b: &str);

    #[wasm_bindgen(js_namespace = console, js_name = log)]
    pub(crate) fn console_log_u32(num: u32);
}

#[wasm_bindgen(module = "/src/lib/Events.ts")]
extern "C" {
    pub type EventEnvelope;

    pub type EventMessage;

    #[wasm_bindgen(extends = EventMessage)]
    pub type CoreEventMessage;

    #[wasm_bindgen(constructor)]
    pub(crate) fn new(payload: EventMessage) -> EventEnvelope;

    #[wasm_bindgen(constructor)]
    pub(crate) fn new(text: &str) -> CoreEventMessage;

    pub type MessageDispatcher;
    #[wasm_bindgen(method, catch)]
    pub(crate) async fn recv_message(this: &MessageDispatcher, env: EventEnvelope) -> Result<(), JsValue>;
}
