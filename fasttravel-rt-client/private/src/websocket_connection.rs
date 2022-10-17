use log::{error, info, trace};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{ErrorEvent, MessageEvent, WebSocket};

use crate::message_broker::RealtimeMessageBroker;

// [todo] get this from config.
const WS_PROTOCOL: &str = "realtime-proto-v01";

pub(crate) enum SocketMessage {
    Binary(Vec<u8>),
    Text(String),
}

#[allow(dead_code)]
pub(crate) struct WebSocketConnection {
    // The socket object.
    pub(crate) socket: WebSocket,

    // These closures are passed to JS and these handles need to be stored or
    // else the JS closure will get dropped causing exception.
    // REF: https://rustwasm.github.io/wasm-bindgen/examples/websockets.html
    on_message: Closure<dyn FnMut(MessageEvent)>,
    on_open: Closure<dyn FnMut()>,
    on_error: Closure<dyn FnMut(ErrorEvent)>,
}

impl WebSocketConnection {
    pub(crate) async fn new(url: &str, broker: Rc<RealtimeMessageBroker>) -> Result<Self, JsValue> {
        trace!("WebSocketConnection_new_url: {}", url);

        let socket = WebSocket::new_with_str(url, WS_PROTOCOL).map_err(|err| {
            error!("websocket_creation_error: {:?}", err);
            err
        })?;

        socket.set_binary_type(web_sys::BinaryType::Arraybuffer);

        // onmessage handler
        let on_message = Closure::<dyn FnMut(_)>::wrap(Box::new(move |e: MessageEvent| {
            e.data()
                .dyn_into::<js_sys::ArrayBuffer>()
                .map(|buf| {
                    let bytes = js_sys::Uint8Array::new(&buf).to_vec();
                    broker.recv_msg_from_server(SocketMessage::Binary(bytes));
                })
                .map_err(|_| {
                    e.data()
                        .dyn_into::<js_sys::JsString>()
                        .map(|text| {
                            broker.recv_msg_from_server(SocketMessage::Text(
                                text.as_string().unwrap(),
                            ));
                        })
                        .map_err(|_| error!("ws_on_message_unknown_format_error: {:?}", e.data()))
                        .ok();
                })
                .ok();
        }));

        // onopen handler
        let (tx, rx) = futures::channel::oneshot::channel::<bool>();
        let on_open = Closure::<dyn FnOnce()>::once(move || {
            tx.send(true)
                .map_err(|e| error!("websocket_onopen_promise_send_error: {}", e))
                .ok();
        });

        // onerror logger
        let on_error = Closure::<dyn FnMut(_)>::new(move |e: ErrorEvent| {
            error!("websocket_error_event: {:?}", e);
        });

        // set all the handlers
        socket.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
        socket.set_onopen(Some(on_open.as_ref().unchecked_ref()));
        socket.set_onerror(Some(on_error.as_ref().unchecked_ref()));

        // wait for the onopen event and then return the connection object
        rx.await
            .map(|status| {
                info!("websocket_connection_open_status: {}", status);
                WebSocketConnection {
                    socket,
                    on_message,
                    on_open,
                    on_error,
                }
            })
            .map_err(|e| {
                error!("websocket_on_open_promise_error: {}", e);
                JsValue::null()
            })
    }
}
