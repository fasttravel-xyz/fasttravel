use std::sync::Arc;
use std::time::Duration;

use factor;

use crate::{
    run_ws_server, CospaceManager, NodeInitializationError, ServicesConfig,
    WebsocketOnUpgradeMessage, WebsocketServiceActor,
};

/// Realtime server state that all message handlers receive to have access to
/// the server config and resources.
#[derive(Clone)]
pub(crate) struct RealtimeServerState {
    #[allow(dead_code)]
    pub(crate) system: factor::SystemRef,
    pub(crate) cospace_mgr: CospaceManager,
    pub(crate) ws_addr: factor::MessageAddr<WebsocketOnUpgradeMessage>,
    pub(crate) public_keys: Arc<PublicDecodingKeys>,
}

/// Configuration of the Workers. Paths and other details of the Worker nodes.
///
/// [todo]: currently, we have a restriction that the path has to be UTF-8 encoded.
///
pub struct WorkerNodesConfig {
    // path to the binary of the shared node.
    pub shared_node_bin_path: String,

    // path to the binary of the dedicated node.
    pub dedicated_node_bin_path: String,
}

// default realtime server config.
impl Default for WorkerNodesConfig {
    fn default() -> Self {
        let mut main_exec_path = std::env::current_exe().expect("path_to_main_node_exec_failed");
        let _ = main_exec_path.pop();

        let mut shared_node_bin_path = main_exec_path.clone();
        shared_node_bin_path.push("realtime_shared_node");

        let mut dedicated_node_bin_path = main_exec_path;
        dedicated_node_bin_path.push("realtime_dedicated_node");

        Self {
            shared_node_bin_path: shared_node_bin_path
                .to_str()
                .expect("non_utf8_paths_not_supported")
                .to_string(),

            dedicated_node_bin_path: dedicated_node_bin_path
                .to_str()
                .expect("non_utf8_paths_not_supported")
                .to_string(),
        }
    }
}

/// Public keys to decode requests and tickets issued by
/// the session_manager/session_lambda.
///
/// We store both the string and the decoding-key as decoding-key
/// is not serializable, in such scenarios we could use the string
/// and regenerate the decoding-key.
pub struct PublicDecodingKeys {
    pub session_request_key_str: String,
    pub session_request_decoding: jsonwebtoken::DecodingKey,
}

impl Default for PublicDecodingKeys {
    fn default() -> Self {
        Self {
            session_request_key_str: "".to_string(),
            session_request_decoding: jsonwebtoken::DecodingKey::from_secret(b"secret"),
        }
    }
}

/// Configuration to start the server.
pub struct ServerConfig {
    /// ip of the websocket server
    pub wss_ip: [u8; 4],

    /// the websocket port
    pub wss_port: u16,

    /// Timeout duration. Client will get disconnected if no heartbeat for specified duration.
    pub heartbeat_timeout: Duration,

    /// Duration between successive heartbeats.
    pub heartbeat_interval: Duration,

    /// public key to decode the tickets provided to the authorized client-sdk.
    pub public_keys: Arc<PublicDecodingKeys>,
}

// default realtime server config.
impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            wss_ip: [0, 0, 0, 0],
            wss_port: 27000,
            heartbeat_timeout: Duration::from_secs(180),
            heartbeat_interval: Duration::from_secs(30),
            public_keys: Arc::new(PublicDecodingKeys::default()),
        }
    }
}

/// The Realtime Server.
pub struct Server {
    /// Server configuration.
    config_server: ServerConfig,

    /// Actor system of the server actors.
    system: factor::SystemRef,

    /// Collaboration space manager.
    cospace_mgr: CospaceManager,

    /// The websocket service actor.
    ws: factor::ActorAddr<WebsocketServiceActor>,
}

impl Server {
    /// create a new realtime server.
    pub async fn build(
        system: factor::SystemRef, config_server: ServerConfig,
        config_services: Option<ServicesConfig>, config_workers: WorkerNodesConfig,
    ) -> Result<Self, NodeInitializationError> {
        // get the service configuration.
        let config_services = config_services.unwrap_or_default();

        let cospace_mgr = CospaceManager::new(config_workers, config_services, &system).await;

        // create and run the websocket service actor.
        let system_moved = system.clone();
        let config = factor::ActorBuilderConfig::default();
        let session_request_decoding = config_server.public_keys.session_request_decoding.clone();
        let factory =
            move |_| WebsocketServiceActor::new(&system_moved, session_request_decoding.clone());

        let spawn_item = factor::ActorBuilder::create(factory, &system, config)
            .ok_or(NodeInitializationError)?;
        let ws = system.run_actor(spawn_item);

        Ok(Self {
            config_server,
            system,
            cospace_mgr,
            ws,
        })
    }

    /// Run the realtime server.
    pub async fn run_realtime(&self) {
        // Create the server state.
        let state = RealtimeServerState {
            system: self.system.clone(),
            cospace_mgr: self.cospace_mgr.clone(),
            ws_addr: self.ws.message_addr(),
            public_keys: self.config_server.public_keys.clone(),
        };

        run_ws_server(
            (self.config_server.wss_ip, self.config_server.wss_port),
            state,
        )
        .await
    }
}
