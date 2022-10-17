use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use uuid::Uuid;

use factor::{self, ActorReceiverContext, SystemRef};
use fasttravel_rt_services::{CospaceId, ModelRoot};

use crate::{
    CospaceActor, ServiceAllocation, ServiceMessage, ServicePool, ServicesConfig, WorkerNodesConfig,
};

#[derive(Clone)]
pub(crate) struct CospaceManager {
    inner: Arc<CospaceManagerInner>,
}

impl CospaceManager {
    pub(crate) async fn new(
        config_workers: WorkerNodesConfig, config_services: ServicesConfig,
        system: &factor::SystemRef,
    ) -> Self {
        Self {
            inner: Arc::new(
                CospaceManagerInner::new(config_workers, config_services, system).await,
            ),
        }
    }

    pub(crate) fn hosted_cospaces(&self) -> &HostedCospaces {
        &self.inner.hosted_cospaces
    }

    pub(crate) fn spawn_cospace_in_dedicated_node(
        &self, model_root: ModelRoot,
    ) -> Result<CospaceId, ()> {
        let cospace_id = CospaceId::generate();

        tracing::debug!(target: "server-event",
            "dedicated_node_path: {}",
            self.inner.config_workers.dedicated_node_bin_path
        );

        let config_node = factor::NodeCreationConfig {
            exec_path: self.inner.config_workers.dedicated_node_bin_path.clone(),
        };
        let cospace_id_moved = cospace_id.clone();
        let hosted_cospaces_handle = self.inner.hosted_cospaces.clone();
        let alloc = ResourceAllocation::Dedicated;
        let system = self.inner.system.clone();

        let task = async move {
            let node_id = system.spawn_worker_node(config_node).await.map_err(|_| {
                tracing::error!(target: "server-event", "spawn_dedicated_worker_node_failed");
                hosted_cospaces_handle.failed(cospace_id_moved.clone());
            })?;

            if let Some(node_mgr_addr) = system
                .get_remote_addr::<CospaceNodeManager>(node_id, crate::NODE_MANAGER_TAG)
                .await
            {
                Self::spawn_cospace_in_node(
                    cospace_id_moved,
                    model_root,
                    node_mgr_addr,
                    alloc,
                    hosted_cospaces_handle,
                )
                .await;
            } else {
                tracing::error!(target: "server-event", "get_remote_addr_dedicated_node_mgr_failed");

                // handle failure
                system.shutdown_worker_node(&node_id).await;
                hosted_cospaces_handle.failed(cospace_id_moved.clone());

                return Err(());
            }

            Result::<(), ()>::Ok(())
        };

        // mark cospace_id as scheduled for creation.
        self.inner.hosted_cospaces.scheduled(cospace_id.clone());
        let handle = self
            .inner
            .system
            .spawn_with_handle(task)
            .map_err(|e| tracing::error!(target: "server-event", "dedicated_node_spawn_task_scheduling_failed: {:?}", e))?;

        // let the task run, we are not interested in the result, task internally handles failure.
        handle.forget();

        Ok(cospace_id)
    }

    pub(crate) fn spawn_cospace_in_main_node(
        &self, model_root: ModelRoot,
    ) -> Result<CospaceId, ()> {
        let cospace_id = CospaceId::generate();
        let cospace_id_moved = cospace_id.clone();
        let hosted_cospaces_handle = self.inner.hosted_cospaces.clone();
        let node_mgr_addr = self.inner.main_node_mgr.clone();
        let alloc = ResourceAllocation::Shared;

        let task = async move {
            Self::spawn_cospace_in_node(
                cospace_id_moved,
                model_root,
                node_mgr_addr,
                alloc,
                hosted_cospaces_handle,
            )
            .await;
        };

        // mark cospace_id as scheduled for creation.
        self.inner.hosted_cospaces.scheduled(cospace_id.clone());
        self.inner.system.spawn_ok(task);

        Ok(cospace_id)
    }

    pub(crate) fn spawn_cospace_in_shared_node(
        &self, model_root: ModelRoot,
    ) -> Result<CospaceId, ()> {
        let cospace_id = CospaceId::generate();
        let cospace_id_moved = cospace_id.clone();
        let hosted_cospaces_handle = self.inner.hosted_cospaces.clone();
        let node_mgr_addr = self.inner.shared_node_mgr.clone();
        let alloc = ResourceAllocation::Shared;

        let task = async move {
            Self::spawn_cospace_in_node(
                cospace_id_moved,
                model_root,
                node_mgr_addr,
                alloc,
                hosted_cospaces_handle,
            )
            .await;
        };

        // mark cospace_id as scheduled for creation.
        self.inner.hosted_cospaces.scheduled(cospace_id.clone());
        self.inner.system.spawn_ok(task);

        Ok(cospace_id)
    }

    async fn spawn_cospace_in_node(
        cospace_id: CospaceId, model_root: ModelRoot,
        node_mgr_addr: factor::ActorAddr<CospaceNodeManager>, alloc: ResourceAllocation,
        hosted_cospaces_handle: HostedCospaces,
    ) {
        let msg = CreateCospaceActorMessage::new(cospace_id.clone(), model_root);

        // request cospace actor creation
        match node_mgr_addr.ask_addr(msg).await {
            Ok(Some(addr_actor)) => {
                let cospace = match alloc {
                    ResourceAllocation::Dedicated => HostedCospace::new_dedicated(addr_actor),
                    ResourceAllocation::Shared => HostedCospace::new_shared(addr_actor),
                };
                hosted_cospaces_handle.insert_cospace(cospace_id, cospace);
            }
            Ok(None) => {
                tracing::error!(target: "server-event", "error_in_ask_create_cospace: ask_returns_None");
            }
            Err(e) => {
                tracing::error!(target: "server-event", "error_in_ask_create_cospace: {}", e);
            }
        };
    }

    pub(crate) fn _terminate_cospace(&self, cospace_id: &CospaceId) {
        // [todo] (1) inform the actor, the node_mgr.
        // [todo] (2) terminate the worker_node if cospace is dedicated.
        self.inner.hosted_cospaces._remove_cospace(cospace_id)
    }
}

struct CospaceManagerInner {
    config_workers: WorkerNodesConfig,
    system: SystemRef,
    main_node_mgr: factor::ActorAddr<CospaceNodeManager>,
    shared_node_mgr: factor::ActorAddr<CospaceNodeManager>,
    hosted_cospaces: HostedCospaces,
}

impl CospaceManagerInner {
    async fn new(
        config_workers: WorkerNodesConfig, config_services: ServicesConfig,
        system: &factor::SystemRef,
    ) -> Self {
        // main_node manager
        let config = factor::ActorBuilderConfig::default();
        let system_moved = system.clone();
        let factory = move |_| CospaceNodeManager::shared(config_services.clone(), &system_moved);
        let spawn_item = factor::ActorBuilder::create(factory, &system, config);
        let main_node_mgr = system.run_actor(spawn_item.expect("FATAL: main_node_mgr_creation_failed."));

        // shared_node manager
        tracing::debug!(target: "server-event",
            "shared_node_bin_path: {}",
            config_workers.shared_node_bin_path
        );

        let config_node = factor::NodeCreationConfig {
            exec_path: config_workers.shared_node_bin_path.clone(),
        };
        let node_id = system
            .spawn_worker_node(config_node)
            .await
            .expect("FATAL: shared_node_worker_node_spawn_failed");

        tracing::debug!(target: "server-event", "shared_node_generated_id: {}", node_id);

        let shared_node_mgr = system
            .get_remote_addr::<CospaceNodeManager>(node_id, crate::NODE_MANAGER_TAG)
            .await
            .expect("FATAL: get_remote_addr_shared_node_mgr_failed");

        Self {
            config_workers,
            system: system.clone(),
            main_node_mgr,
            shared_node_mgr,
            hosted_cospaces: HostedCospaces::new(),
        }
    }
}

/// Message requesting to create a new collaboration space that will host
/// a requested model-root.
/// Subsequent client connections to join the model-root get connected
/// to already existing collaborative space.
#[derive(Serialize, Deserialize, Clone)]
pub(crate) struct CreateCospaceActorMessage {
    cospace_id: CospaceId,
    model_root: ModelRoot,
}

impl CreateCospaceActorMessage {
    pub(crate) fn new(cospace_id: CospaceId, model_root: ModelRoot) -> Self {
        Self {
            cospace_id,
            model_root,
        }
    }
}

impl factor::MessageCluster for CreateCospaceActorMessage {
    type Result = Option<factor::ActorAddr<CospaceActor>>;
}

#[derive(Clone)]
pub(crate) enum SpawnConfig {
    Dedicated,
    Shared { services: ServicePool },
}

/// The collaborative space creator actor.
/// Create a CospaceActor to host a new collaborative space.
#[derive(Clone)]
pub(crate) struct CospaceNodeManager {
    config_services: ServicesConfig,
    config_spawn: SpawnConfig,
    cospaces: Arc<DashMap<Uuid, factor::MessageClusterAddr<ServiceMessage>>>,
}
impl CospaceNodeManager {
    pub(crate) fn dedicated(config_services: ServicesConfig) -> Self {
        Self {
            config_services,
            config_spawn: SpawnConfig::Dedicated,
            cospaces: Arc::new(DashMap::new()),
        }
    }

    pub(crate) fn shared(config_services: ServicesConfig, system: &SystemRef) -> Self {
        let cospaces = Arc::new(DashMap::new());
        let allocation = ServiceAllocation::Shared(cospaces.clone());
        let services = ServicePool::new_pool(system, allocation, &config_services);
        let config_spawn = SpawnConfig::Shared { services };

        Self {
            config_services,
            config_spawn,
            cospaces,
        }
    }
}
impl factor::ActorReceiver for CospaceNodeManager {
    type Context = factor::BasicContext<Self>;
}

// [todo] TerminateCospaceActorMessage
impl factor::MessageClusterHandler<CreateCospaceActorMessage> for CospaceNodeManager {
    type Result =
        factor::MessageResponseType<<CreateCospaceActorMessage as factor::MessageCluster>::Result>;

    fn handle(&mut self, msg: CreateCospaceActorMessage, ctx: &mut Self::Context) -> Self::Result {
        tracing::debug!(target: "server-event", "cospace_node_mgr_handle_create_cospace_actor_msg: {:#?}", msg.cospace_id);

        let system = ctx.system();
        let cospace_uuid = msg.cospace_id.uuid.clone();

        let addr = match &self.config_spawn {
            SpawnConfig::Shared { services } => {
                let services = services.clone();
                let factory = move |_| {
                    CospaceActor::new(
                        msg.cospace_id.clone(),
                        msg.model_root.clone(),
                        services.clone(),
                    )
                };

                let builder_config = factor::ActorBuilderConfig::default();
                let spawn_item = factor::ActorBuilder::create(factory, &system, builder_config);
                system.run_actor(spawn_item.unwrap())
            }
            SpawnConfig::Dedicated => {
                let config_services = self.config_services.clone();
                let system_moved = system.clone();
                let factory = move |weak_addr| {
                    let allocation = ServiceAllocation::Dedicated(weak_addr);
                    let services =
                        ServicePool::new_pool(&system_moved, allocation, &config_services);

                    CospaceActor::new(msg.cospace_id.clone(), msg.model_root.clone(), services)
                };

                let builder_config = factor::ActorBuilderConfig::default();
                let spawn_item = factor::ActorBuilder::create(factory, &system, builder_config);
                system.run_actor(spawn_item.unwrap())
            }
        };

        self.cospaces
            .insert(cospace_uuid, addr.message_cluster_addr());

        factor::MessageResponseType::Result(Some(addr).into())
    }
}

/// Details of the scheduled collaboration space creation request.
struct CospaceCreationRequest {
    _id: CospaceId,
    _timestamp: chrono::DateTime<chrono::Utc>,
}

impl CospaceCreationRequest {
    fn new(cospace_id: CospaceId) -> Self {
        Self {
            _id: cospace_id,
            _timestamp: chrono::offset::Utc::now(),
        }
    }
}

enum ResourceAllocation {
    Dedicated,
    Shared,
}

struct Tenant {}

struct HostedCospace {
    _tenant: Tenant,                     // future use
    _resource_alloc: ResourceAllocation, // will be used in terminate
    addr_actor: factor::ActorAddr<CospaceActor>,
}

impl HostedCospace {
    fn new_dedicated(addr_actor: factor::ActorAddr<CospaceActor>) -> Self {
        Self {
            _tenant: Tenant {},
            _resource_alloc: ResourceAllocation::Dedicated,
            addr_actor,
        }
    }

    fn new_shared(addr_actor: factor::ActorAddr<CospaceActor>) -> Self {
        Self {
            _tenant: Tenant {},
            _resource_alloc: ResourceAllocation::Shared,
            addr_actor,
        }
    }
}

/// Structure storing the hosted collaboration spaces details.
///
/// [todo]: This is currently stored only in memory. Make this info available
/// to the session_manager/session_lambda through persistent memory-store.
/// Memory-store will allow restart in case of sleep, recovery in case of failure,
/// proper routing in case on multi-instance deployment of this realtime server.
///
#[derive(Clone)]
pub(crate) struct HostedCospaces {
    inner: Arc<HostedCospacesInner>,
}

struct HostedCospacesInner {
    scheduled: DashMap<Uuid, CospaceCreationRequest>,
    failed: DashMap<Uuid, CospaceCreationRequest>,
    cospaces: DashMap<Uuid, HostedCospace>,
}

impl HostedCospaces {
    fn new() -> Self {
        Self {
            inner: Arc::new(HostedCospacesInner {
                scheduled: DashMap::new(),
                failed: DashMap::new(),
                cospaces: DashMap::new(),
            }),
        }
    }

    pub(crate) fn get_cospace_addr(&self, uuid: &Uuid) -> Option<factor::ActorAddr<CospaceActor>> {
        self.inner
            .cospaces
            .get(uuid)
            .map(|cospace| cospace.addr_actor.clone())
    }

    pub(crate) fn is_scheduled(&self, uuid: &Uuid) -> bool {
        self.inner.scheduled.contains_key(uuid)
    }

    pub(crate) fn is_failed(&self, uuid: &Uuid) -> bool {
        self.inner.failed.contains_key(uuid)
    }

    pub(crate) fn is_hosted(&self, uuid: &Uuid) -> bool {
        self.inner.cospaces.contains_key(uuid)
    }

    pub(crate) fn scheduled(&self, cospace_id: CospaceId) {
        self.inner
            .scheduled
            .insert(cospace_id.uuid, CospaceCreationRequest::new(cospace_id));
        // [todo] schedule a task to remove this and move to failed after timeout.
    }

    fn failed(&self, cospace_id: CospaceId) {
        if let Some(pair) = self.inner.scheduled.remove(&cospace_id.uuid) {
            self.inner.failed.insert(pair.0, pair.1);
        }
    }

    fn insert_cospace(&self, cospace_id: CospaceId, cospace: HostedCospace) {
        self.inner.cospaces.insert(cospace_id.uuid, cospace);
        self.inner.scheduled.remove(&cospace_id.uuid);
    }

    pub(crate) fn _remove_cospace(&self, cospace_id: &CospaceId) {
        self.inner.cospaces.remove(&cospace_id.uuid);
    }
}
