//! This package provides the implementation of the realtime server and
//! provides the below endpoints to initiate realtime sessions:
//!
//! * endpoint: Used by the session-lambda to request the realtime-server
//!             to host a model-root by spawning a collaboration space
//!             (cospace), when the session-lambda receives the
//!             POST /session/join/ request from an authorized client-sdk.
//!
//!         POST /realtime/host/
//!
//! * endpoint: Used by an authorized client-sdk to query the status of a
//!             collaboration space requested in the /realtime/host/ call.
//!
//!         GET /realtime/status/:cospace
//!
//! * endpoint: The websocket endpoint to start a websocket connection and
//!             finally join a session inside a collaboration space.
//!
//!         GET UPGRADE WEBSOCKET /realtime/connect/:cospace
//!
//!
//! The realtime client-sdk depends on the below endpoints from additional services:
//! (a) authorization_provider: user-pool management,
//!     authetication and authorization e.g. auth0, AWS Cognito, etc.
//!
//!     * endpoint: Get your user autheticated and authorized-access to raltime:api.
//!
//!         POST /authorize/
//!
//! (b) session_lambda: A cloud function/lambda with access to a private vault
//!     with the secrets (private keys) that initiates the realtime sessions.
//!
//!     * endpoint: Request to join a session and access to activities associated
//!                 with a model-root.
//!
//!         POST /session/join/
//!
//!
//! Though we recommend a managed auth-provider and a session-lambda,
//! it is upto you to decide how to provide these endpoints. If you want you could
//! run the auth and session management apis in your own server. We recommend
//! the above approach because this way your secrets are never stored
//! on servers or environment variables (even if the server is
//! managed by you) and are stored in a single managed vault.
//!
//! For local testing this repo provides mocker services that emulate
//! the endpoints (refer to the mocker files for example requests):
//!
//!     - /src/bin/mockers/auth_provider.rs
//!     - /src/bin/mockers/session_lambda.rs
//!     - /src/bin/mockers/realtime_main_node.rs
//!     - [todo] /src/bin/mockers/Dockerfile
//!
//! You could create your own mockers as well.
//!
//! The recommended flow for initiating a realtime session:
//! ... refer to ../README.md
//!

#![deny(unreachable_pub, private_in_public)]
#![forbid(unsafe_code)]

use clap::Parser;
use jsonwebtoken::DecodingKey;
use std::sync::Arc;

use factor::SystemRef;

mod authorization;
mod client;
mod cospace;
mod server;
mod websocket;

pub use authorization::*;
use client::*;
use cospace::*;
pub use server::*;
pub use websocket::HostSessionRequest;
use websocket::*;

//
// fasttravel realtime server has three different kind of nodes that run in
// separate processes depending on the configuration in which the realtime
// server is launched. The three nodes provide the configuration flexibility
// for how collaboration spaces (cospaces) are allocated resources.
//
// Cospaces could be spawned in three different manner based on their resource requirements.
// So, there are three different kind of nodes available to the server:
// (1) main_node:
//     a. the main node with the websocket server running that hosts the client-connections for all the cospaces.
//     b. the main node has a shared service pool and all cospaces spawned in this node share the resources.
//     c. fit for short-lived cospaces.
// (2) shared_node:
//     a. the shared node has a shared service pool where multiple cospaces can be spawned.
//     b. all cospaces spawned in this node share the resources.
//     c. fit for test/anonymous/demo cospaces.
//     d. currently, we have only one shared_node, but we could have mulltiple (e.g. one node per tenant.)
// (3) dedicated_node:
//     a. a dedicated node for a single cospace.
//     b. new node gets spawned every time a request comes for hosting a cospace with dediacted resources.
//     c. fit for a cospace hosting large number of participants and cpu bound tasks.
//
//
// Refer to the three mocker binaries for examples of how
// to create the three node binaries.
// (1) src/bin/mockers/realtime_main_node.rs
// (2) src/bin/mockers/realtime_shared_node.rs
// (3) src/bin/mockers/realtime_dedicated_node.rs
//

pub const NODE_MANAGER_TAG: &'static str = "node_mgr_root";

/// Worker node of the factor cluster.
#[derive(Parser, Debug)]
struct Args {
    /// Node id of this node.
    #[arg(long, default_value_t = 0)]
    node_id: factor::NodeId,
}

// Below are the convenience functions that initialize the required
// services and actors for the three different kind of nodes.

/// fasttravel-rt main_node initialization function.
///
/// This function must be awaited on inside the context of a tokio-runtime.
///
/// @param session_request_key_str: The PUBLIC-KEY of the session-lambda service. This is the
///         key that is used to decode and authenticate the host-session
///         requests that the session-lambda will send to this server.
///         (1) Refer to fasttravel-rt/main/README.md to get details on
///         fasttravel's recommended deployment architecture and the
///         session-lambda.
///         (2) Refer to src/bin/mockers/realtime_main_node.rs for usage example.
///
pub async fn initialize_and_run_main_node(
    session_request_key_str: &str,
) -> Result<(), NodeInitializationError> {
    tracing::info!(target: "server_event", "initialize_and_run_main_node");

    let session_request_decoding = DecodingKey::from_ec_pem(session_request_key_str.as_bytes())
        .map_err(|e| {
            tracing::error!(target: "server_event", "FATAL_session_decoding_key_generation_failed {}", e);
            NodeInitializationError
        })?;

    let public_keys = PublicDecodingKeys {
        session_request_key_str: session_request_key_str.to_string(),
        session_request_decoding,
    };

    // configurations
    let mut config_server = ServerConfig::default();
    config_server.public_keys = Arc::new(public_keys);
    let config_workers = WorkerNodesConfig::default();

    // system
    let node_id = 0; // must always be 0 for the main_node.
    let provider = build_remote_types_provider();
    let system = factor::init_cluster(node_id, Some("main_node_system".to_owned()), provider).await;

    tracing::info!(target: "server_event", "initialize_and_run_main_node_init_cluster_OK");

    // realtime server
    let server = Server::build(system, config_server, None, config_workers).await?;
    let run = server.run_realtime();

    tracing::info!(target: "server_event", "initialize_and_run_main_node_run_realtime_OK");

    // run the server
    run.await;

    Ok(())
}

/// fasttravel-rt shared_node initialization function.
///
/// This function must be awaited on inside the context of a tokio-runtime.
///
pub async fn initialize_and_run_shared_node() -> Result<(), NodeInitializationError> {
    // configurations
    let config_services = ServicesConfig::default();

    // initialization closure
    let closure = move |system: SystemRef| {
        // [root] shared node manager actor
        let mut config = factor::ActorBuilderConfig::default();
        config.actor_tag = Some(NODE_MANAGER_TAG.to_string());
        config.discovery = factor::DiscoveryKey::Tag;
        let system_moved = system.clone();
        let factory = move |_| CospaceNodeManager::shared(config_services.clone(), &system_moved);
        if let Some(spawn_item) = factor::ActorBuilder::create(factory, &system, config) {
            let _mgr = system.run_actor(spawn_item);
        } else {
            tracing::error!(target: "server_event", "FATAL_shared_node_mgr_actor_build_failed");
        }
    };

    // system
    let args = Args::parse();
    let provider = build_remote_types_provider();

    let _system = factor::init_cluster_with_closure(
        args.node_id,
        Some("shared_node_system".to_owned()),
        provider,
        closure,
    )
    .await;

    tracing::info!(target: "server_event", "shared_node_initialization_OK");

    // [todo] wait on SIGKILL/STDINPUT rather that dummy channel.
    let (_tx, rx) = futures::channel::oneshot::channel::<()>();
    rx.await.ok();

    Ok(())
}

/// fasttravel-rt dedicated_node initialization function.
///
/// This function must be awaited on inside the context of a tokio-runtime.
///
pub async fn initialize_and_run_dedicated_node() -> Result<(), NodeInitializationError> {
    // configurations
    let config_services = ServicesConfig::default();

    // initialization closure
    let closure = move |system: SystemRef| {
        // [root] dedicated node manager
        let mut config = factor::ActorBuilderConfig::default();
        config.actor_tag = Some(NODE_MANAGER_TAG.to_string());
        config.discovery = factor::DiscoveryKey::Tag;
        let factory = move |_| CospaceNodeManager::dedicated(config_services.clone());

        if let Some(spawn_item) = factor::ActorBuilder::create(factory, &system, config) {
            let _mgr = system.run_actor(spawn_item);
        } else {
            tracing::error!(target: "server_event", "FATAL_dedicated_node_mgr_actor_build_failed");
        }
    };

    // system
    let args = Args::parse();
    let provider = build_remote_types_provider();
    let _system = factor::init_cluster_with_closure(
        args.node_id,
        Some("dedicated_node_system".to_owned()),
        provider,
        closure,
    )
    .await;

    // [todo] wait on SIGKILL/STDINPUT rather that dummy channel.
    let (_tx, rx) = futures::channel::oneshot::channel::<()>();
    rx.await.ok();

    Ok(())
}

pub fn build_remote_types_provider() -> factor::RemoteMessageTypeProvider {
    let mut provider = factor::RemoteMessageTypeProvider::new();

    // register only the actor-types those interact across the ipc-nodes,
    // and the message-types those are sent across ipc-nodes.
    provider.register::<CospaceActor, GenerateClientIdMessage>();
    provider.register::<CospaceActor, ClientConnectionMessage>();
    provider.register::<CospaceActor, ClientMessage>();
    provider.register::<ClientConnectionActor, ServiceMessage>();
    provider.register::<CospaceNodeManager, CreateCospaceActorMessage>();

    provider
}

#[derive(Debug)]
pub struct NodeInitializationError;
