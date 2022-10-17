use log::{error, info, trace};
use serde::{Deserialize, Serialize};
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Request, RequestInit, RequestMode, Response};

use crate::{
    delegate_connection::{
        ConnectionServiceDelegate, ConnectionServiceKernel,
    },
    delegate_core::{CoreServiceDelegate, CoreServiceKernel},
    message_broker::RealtimeMessageBroker,
    websocket_connection::WebSocketConnection,
    MessageDispatcher,
};

pub(crate) trait ServiceDelegatePrivate<K> {
    fn new(private: Rc<K>) -> Self;
}

/// The realtime module.
/// The core object that exposes the client-sdk functionalities to the JS-wrapper layer.
#[allow(dead_code)]
#[wasm_bindgen]
pub struct RealtimeModule {
    status: u32,
    config: RealtimeModuleConfig,
    broker: Rc<RealtimeMessageBroker>,
}

#[wasm_bindgen]
impl RealtimeModule {
    pub fn new(config: RealtimeModuleConfig) -> RealtimeModule {
        Self {
            status: 0,
            config,
            broker: Rc::new(RealtimeMessageBroker::new()),
        }
    }

    /// Get the running status of the realtime module.
    /// 
    /// Codes:
    /// 0: RUNNING_STATUS_OK
    pub fn status(&self) -> u32 {
        self.status
    }

    /// Create the core service delegate/kernel.
    pub fn init_core(&self, dispatcher: MessageDispatcher) -> CoreServiceDelegate {
        trace!("RealtimeModule_init_core");

        let kernel = Rc::new(CoreServiceKernel::new(self.broker.clone(), dispatcher));
        self.broker.set_kernel_core(kernel.clone());

        CoreServiceDelegate::new(kernel)
    }

    /// Create the connection service delegate/kernel.
    pub fn init_connection(&self, dispatcher: MessageDispatcher) -> ConnectionServiceDelegate {
        trace!("RealtimeModule_init_connection");

        let kernel = Rc::new(ConnectionServiceKernel::new(
            self.broker.clone(),
            dispatcher,
        ));
        self.broker.set_kernel_connection(kernel.clone());

        ConnectionServiceDelegate::new(kernel)
    }

    /// Request to join a session of a collaboration space.
    /// 1. Use accessToken and sessionUrl to send a host-cospace request.
    /// 2. Use statusTicket and statusUrl to check cospace hosting status.
    /// 3. Use queryTicket and realtimeUrl to start websocket connection.
    /// 4. Use messageTicket as the first message handshake over the socket.
    pub async fn join_session(&self, model_session: SessionModelRoot) -> Result<(), JsValue> {
        trace!("RealtimeModule_join_session");
        trace!("[namespace]: {}", model_session.namespace);
        trace!("[workspace]: {}", model_session.workspace);

        if let Ok(res) = self.host_cospace(model_session).await {
            // [todo]: check status of cospace before attempting connect.
            self.connect_realtime(&res.cospace_uuid, &res.ticket_query)
                .await
                .map_err(|e| {
                    error!("join_session_connect_realtime_error: {:?}", e);
                    e
                })?;

            if let Some(kernel) = self.broker.try_get_kernel_connection() {
                let success = kernel
                    .perform_websocket_ticket_handshake(res.ticket_message)
                    .await;

                info!("perform_websocket_ticket_handshake_status: {}", success);
            }
        }

        Ok(())
    }

    async fn host_cospace(
        &self,
        model_session: SessionModelRoot,
    ) -> Result<SessionJoinResponseBody, JsValue> {
        // Cospace.Modelroot.
        let session_join_req = SessionJoinRequestBody {
            namespace: model_session.namespace,
            workspace: model_session.workspace,
        };

        // Request.Body.Json.
        let body_obj = serde_wasm_bindgen::to_value::<SessionJoinRequestBody>(&session_join_req)?;
        let body_json = js_sys::JSON::stringify(&body_obj)?;
        let mut opts = RequestInit::new();
        opts.method("POST");
        opts.mode(RequestMode::Cors);
        opts.body(Some(&body_json));

        // Headers.
        let auth_token = "Bearer ".to_owned() + self.config.rt_access_token.as_str();
        let request = Request::new_with_str_and_init(self.config.rt_session_url.as_str(), &opts)?;
        request.headers().set("Accept", "application/json")?;
        request
            .headers()
            .set("content-type", "application/json; charset=utf-8")?;
        request
            .headers()
            .set("Authorization", auth_token.as_str())?;

        // Fetch.POST
        if let Some(window) = web_sys::window() {
            let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
            assert!(resp_value.is_instance_of::<Response>());
            let resp: Response = resp_value.dyn_into().unwrap();
            let json = JsFuture::from(resp.json()?).await?;

            return serde_wasm_bindgen::from_value::<SessionJoinResponseBody>(json)
                .map_err(|_| JsValue::from_str("response_body_json_parse_error"));
        }

        Err(JsValue::null())
    }

    async fn connect_realtime(
        &self,
        cospace_uuid: &str,
        ticket_query: &str,
    ) -> Result<(), JsValue> {
        let ws_url = self.config.rt_connect_url.as_str().to_owned()
            + cospace_uuid
            + "?ticket="
            + ticket_query;

        WebSocketConnection::new(ws_url.as_str(), self.broker.clone())
            .await
            .map(|connection| self.broker.set_connection(connection))?;

        Ok(())
    }
}


/// Session join request and collaboration space modelroot info.
#[derive(Debug, Serialize)]
struct SessionJoinRequestBody {
    namespace: String,
    workspace: String,
}

/// Session join response and collaboration space modelroot info.
#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct SessionJoinResponseBody {
    ticket_status: String,
    ticket_query: String,
    ticket_message: String,
    cospace_uuid: String,
    token_type: String,
}

/// Configuration settings for the realtime module.
#[allow(dead_code)]
#[wasm_bindgen]
pub struct RealtimeModuleConfig {
    rt_access_token: String,
    rt_session_url: String,
    rt_status_url: String,
    rt_connect_url: String,
}

#[wasm_bindgen]
impl RealtimeModuleConfig {
    pub fn new(
        rt_access_token: String,
        rt_session_url: String,
        rt_status_url: String,
        rt_connect_url: String,
    ) -> RealtimeModuleConfig {
        Self {
            rt_access_token,
            rt_session_url,
            rt_status_url,
            rt_connect_url,
        }
    }
}

/// The modelroot of the realtime session.
#[wasm_bindgen]
pub struct SessionModelRoot {
    namespace: String,
    workspace: String,
    #[allow(dead_code)]
    scopes: Vec<String>,
}

#[wasm_bindgen]
impl SessionModelRoot {
    pub fn new(namespace: String, workspace: String) -> SessionModelRoot {
        Self {
            namespace,
            workspace,
            scopes: Vec::with_capacity(0),
        }
    }
}
