use futures::channel::oneshot;
use log::{error, trace};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use wasm_bindgen_futures::spawn_local;

use fasttravel_rt_proto::{
    helpers::{
        self as proto_helpers, ProtoMessage, ProtoPayloadRequest, ProtoPayloadResponse,
        ProtoPayloadTell,
    },
    RealtimeService,
};

use crate::delegate_connection::ConnectionServiceKernel;
use crate::delegate_core::CoreServiceKernel;
use crate::websocket_connection::{SocketMessage, WebSocketConnection};

/// Response promise which sends the response on complete.
struct ResponsePromise {
    tx: oneshot::Sender<Vec<u8>>,
}

impl ResponsePromise {
    fn complete(self, encoded_bytes: Vec<u8>) {
        let _ = self.tx.send(encoded_bytes);
    }
}

/// The realtime message broker that brokers messages between the
/// websocket connection and the service delegates.
/// The broker also manages the request-response protocol over
/// the socket stream.
pub(crate) struct RealtimeMessageBroker {
    connection: RefCell<Option<Rc<WebSocketConnection>>>,
    kernel_core: RefCell<Option<Rc<CoreServiceKernel>>>,
    kernel_connection: RefCell<Option<Rc<ConnectionServiceKernel>>>,
    service_promises: RefCell<HashMap<u32, ResponsePromise>>,
    request_id_counter: RefCell<u32>,
}

// The realtime message broker is referenced through multiple Rc(s) by
// the realtime module and the service delegates. Instead of a single
// RefCell<Inner>, we use granular internal mutability.
impl RealtimeMessageBroker {
    pub(crate) fn new() -> Self {
        Self {
            connection: RefCell::new(None),
            kernel_core: RefCell::new(None),
            kernel_connection: RefCell::new(None),
            service_promises: RefCell::new(HashMap::new()),
            request_id_counter: RefCell::new(0),
        }
    }

    pub(crate) fn set_connection(&self, connection: WebSocketConnection) {
        self.connection.replace(Some(Rc::new(connection)));
    }

    pub(crate) fn set_kernel_core(&self, kernel_core: Rc<CoreServiceKernel>) {
        self.kernel_core.replace(Some(kernel_core));
    }

    pub(crate) fn set_kernel_connection(&self, kernel_connection: Rc<ConnectionServiceKernel>) {
        self.kernel_connection.replace(Some(kernel_connection));
    }

    // internal method, panic on misuse (unwrap) is intentional.
    fn get_connection(&self) -> Rc<WebSocketConnection> {
        self.connection.borrow().as_ref().unwrap().clone()
    }

    // internal method, panic on misuse (unwrap) is intentional.
    fn get_kernel_core(&self) -> Rc<CoreServiceKernel> {
        self.kernel_core.borrow().as_ref().unwrap().clone()
    }

    // internal method, panic on misuse (unwrap) is intentional.
    fn get_kernel_connection(&self) -> Rc<ConnectionServiceKernel> {
        self.kernel_connection.borrow().as_ref().unwrap().clone()
    }

    pub(crate) fn try_get_kernel_connection(&self) -> Option<Rc<ConnectionServiceKernel>> {
        if let Ok(cnx) = self.kernel_connection.try_borrow() {
            return cnx.as_ref().map(|kernel| kernel.clone());
        };

        None
    }

    pub(crate) async fn send_text_message_to_server(&self, msg: &str) {
        // Our public send-api to JS is async, but WebSocket.send() returns immediately
        // after pushing msg to buffer, if buffer is full socket is closed with exception.
        // REF: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/send

        self.get_connection()
            .socket
            .send_with_str(msg)
            .map_err(|e| error!("send_text_message_to_server_error {:#?}", e))
            .ok();
    }

    pub(crate) async fn _send_proto_message_to_server(
        &self,
        service: &RealtimeService,
        payload: Vec<u8>,
    ) {
        // Our public send-api to JS is async, but WebSocket.send() returns immediately
        // after pushing msg to buffer, if buffer is full socket is closed with exception.
        // REF: https://developer.mozilla.org/en-US/docs/Web/API/WebSocket/send

        proto_helpers::create_tell_message_from_service_payload(service, payload)
            .and_then(|rt_msg| {
                self.get_connection()
                    .socket
                    .send_with_u8_array(&rt_msg[..])
                    .map_err(|e| error!("send_proto_message_to_server_error {:#?}", e))
                    .ok();

                Some(rt_msg)
            })
            .or_else(|| {
                error!("create_tell_message_from_service_payload_error");
                None
            });
    }

    pub(crate) async fn send_proto_request_to_server(
        &self,
        service: &RealtimeService,
        payload: Vec<u8>,
    ) -> Option<Vec<u8>> {
        // increment the request counter.
        let mut req_id_counter = self.request_id_counter.borrow_mut();
        let next_id = *req_id_counter + 1;
        *req_id_counter = next_id;

        trace!("send_proto_request_to_server_req_id: {}", next_id);

        if let Some(rt_msg) =
            proto_helpers::create_request_message_from_service_payload(next_id, service, payload)
        {
            // send the request message
            if let Err(e) = self.get_connection().socket.send_with_u8_array(&rt_msg) {
                error!("socket.send_with_u8_array_error {:#?}", e);
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
                self.service_promises.borrow_mut().insert(next_id, promise);

                return promise_fut.await;
            }
        }

        None
    }

    #[inline(always)]
    pub(crate) fn recv_msg_from_server(&self, msg: SocketMessage) {
        match msg {
            SocketMessage::Text(text) => self.recv_text_msg_from_server(text),
            SocketMessage::Binary(bytes) => self.recv_proto_msg_from_server(bytes),
        }
    }

    #[inline(always)]
    fn recv_text_msg_from_server(&self, msg: String) {
        trace!("recv_text_msg_from_server");

        // no functionality, just forward the message to core-service-kernel.
        let kernel_core = self.get_kernel_core();
        let task = async move {
            kernel_core.recv_text_message_from_server(msg).await;
        };

        spawn_local(task);
    }

    #[inline(always)]
    fn recv_proto_msg_from_server(&self, bytes: Vec<u8>) {
        trace!("recv_proto_msg_from_server_msg_len: {}", bytes.len());

        let proto_msg = proto_helpers::process_realtime_message(bytes);

        match proto_msg {
            ProtoMessage::Tell(payload) => self.recv_proto_tell_from_server(payload),
            ProtoMessage::Request(payload) => self.recv_proto_req_from_server(payload),
            ProtoMessage::Response(payload) => self.recv_proto_res_from_server(payload),
            _ => {}
        }
    }

    #[inline(always)]
    fn recv_proto_req_from_server(&self, payload: ProtoPayloadRequest) {
        trace!("recv_proto_req_from_server");

        let connection = self.get_connection();
        let kernel_core = self.get_kernel_core();
        let kernel_connection = self.get_kernel_connection();

        let task = async move {
            let mut response = None;
            match payload.service {
                RealtimeService::Connection => {
                    response = kernel_connection
                        .answer_proto_req_from_server(payload.bytes)
                        .await;
                }
                RealtimeService::Core => {
                    response = kernel_core
                        .answer_proto_req_from_server(payload.bytes)
                        .await;
                }
                _ => {
                    error!("recv_proto_req_from_server_error_service_not_handled")
                }
            }

            response.map(|bytes| {
                proto_helpers::create_response_message_from_service_payload(
                    payload.request_id,
                    &payload.service,
                    bytes,
                )
                .map(|rt_msg| {
                    connection
                        .socket
                        .send_with_u8_array(&rt_msg[..])
                        .map_err(|e| {
                            error!("recv_proto_req_from_server_error_on_res_send {:#?}", e)
                        })
                        .ok();
                });
            });
        };

        spawn_local(task); // future will run on the next microtask tick.
    }

    #[inline(always)]
    fn recv_proto_tell_from_server(&self, payload: ProtoPayloadTell) {
        trace!("recv_proto_tell_from_server");

        let kernel_core = self.get_kernel_core();
        let kernel_connection = self.get_kernel_connection();

        match &payload.service {
            RealtimeService::Connection => {
                let task = async move {
                    kernel_connection
                        .recv_proto_message_from_server(payload.bytes)
                        .await;
                };

                spawn_local(task); // future will run on the next microtask tick.
            }
            RealtimeService::Core => {
                let task = async move {
                    kernel_core
                        .recv_proto_message_from_server(payload.bytes)
                        .await;
                };

                spawn_local(task); // future will run on the next microtask tick.
            }
            _ => {
                error!("recv_proto_tell_from_server_error_service_not_handled")
            }
        }
    }

    #[inline(always)]
    fn recv_proto_res_from_server(&self, payload: ProtoPayloadResponse) {
        trace!("recv_proto_res_from_server");

        let mut map = self.service_promises.borrow_mut();
        if let Some(promise) = map.remove(&payload.response_id) {
            promise.complete(payload.bytes);
        } else {
            error!("req_promise_already_dropped_req_id {}", payload.response_id);
        }
    }
}
