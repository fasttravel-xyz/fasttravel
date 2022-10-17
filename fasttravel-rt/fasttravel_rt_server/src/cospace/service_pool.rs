use super::{service_actor::*, ClientConnectionMessage, ClientMessage};
use fasttravel_rt_services::*;

/// Static service configurations
#[derive(Clone)]
pub struct ServicesConfig {
    pub pool_size_core: u8,
    pub pool_size_presence: u8,
    pub pool_size_activity: u8,
    pub pool_size_model: u8,
}

const DEAFULT_POOL_SIZE: u8 = 1;
impl Default for ServicesConfig {
    /// Default static service configurations
    fn default() -> Self {
        Self {
            pool_size_core: DEAFULT_POOL_SIZE,
            pool_size_presence: DEAFULT_POOL_SIZE,
            pool_size_activity: DEAFULT_POOL_SIZE,
            pool_size_model: DEAFULT_POOL_SIZE,
        }
    }
}

/// Realtime service pool. Each service runs in its own actor-pool.
/// A service pool could be dedicated to a collaborative space or
/// shared among multiple collabprative spaces.
#[derive(Clone)]
pub(crate) struct ServicePool {
    core: factor::ActorAddr<ServiceActor<ServiceCore>>,
    presence: factor::ActorAddr<ServiceActor<ServicePresence>>,
    activity: factor::ActorAddr<ServiceActor<ServiceActivity>>,
    model: factor::ActorAddr<ServiceActor<ServiceModel>>,
}

pub(crate) trait ServicePoolMessenger<M: factor::MessageCluster + Send + Clone + 'static> {
    fn message_addr(&self, service: &Services) -> Option<factor::MessageClusterAddr<M>>;

    fn tell(&self, service: &Services, msg: M) {
        // calling unwrap() without check as with None should panic, which is what we want.
        let _ = self.message_addr(service).unwrap().tell(msg);
    }

    fn broadcast(&self, msg: M) {
        // calling unwrap() without check as with None should panic, which is what we want.
        let _ = self
            .message_addr(&Services::Core)
            .unwrap()
            .tell(msg.clone());
        let _ = self
            .message_addr(&Services::Presence)
            .unwrap()
            .tell(msg.clone());
        let _ = self
            .message_addr(&Services::Activity)
            .unwrap()
            .tell(msg.clone());
        let _ = self
            .message_addr(&Services::Model)
            .unwrap()
            .tell(msg.clone());
    }
}

impl ServicePoolMessenger<ClientMessage> for ServicePool {
    #[inline(always)]
    fn message_addr(
        &self, service: &Services,
    ) -> Option<factor::MessageClusterAddr<ClientMessage>> {
        match service {
            Services::Core => Some(self.core.message_cluster_addr::<ClientMessage>()),
            Services::Presence => Some(self.presence.message_cluster_addr::<ClientMessage>()),
            Services::Activity => Some(self.activity.message_cluster_addr::<ClientMessage>()),
            Services::Model => Some(self.model.message_cluster_addr::<ClientMessage>()),
            _ => None,
        }
    }
}

impl ServicePoolMessenger<ClientConnectionMessage> for ServicePool {
    #[inline(always)]
    fn message_addr(
        &self, service: &Services,
    ) -> Option<factor::MessageClusterAddr<ClientConnectionMessage>> {
        match service {
            Services::Core => Some(self.core.message_cluster_addr::<ClientConnectionMessage>()),
            Services::Presence => Some(
                self.presence
                    .message_cluster_addr::<ClientConnectionMessage>(),
            ),
            Services::Activity => Some(
                self.activity
                    .message_cluster_addr::<ClientConnectionMessage>(),
            ),
            Services::Model => Some(self.model.message_cluster_addr::<ClientConnectionMessage>()),
            _ => None,
        }
    }
}

impl ServicePool {
    // Create a new service pool.
    pub(crate) fn new_pool(
        system: &factor::SystemRef, allocation: ServiceAllocation, config_services: &ServicesConfig,
    ) -> Self {
        let mut config_core = factor::ActorBuilderConfig::default();
        let mut config_presence = factor::ActorBuilderConfig::default();
        let mut config_activity = factor::ActorBuilderConfig::default();
        let mut config_model = factor::ActorBuilderConfig::default();

        config_core.pool_size = Some(config_services.pool_size_core as usize);
        config_presence.pool_size = Some(config_services.pool_size_presence as usize);
        config_activity.pool_size = Some(config_services.pool_size_activity as usize);
        config_model.pool_size = Some(config_services.pool_size_model as usize);

        // Create and run the Core Service.
        let s_alloc = allocation.clone();
        let system_moved = system.clone();
        let mut spawn_item = factor::ActorBuilder::create_pool(
            move |_| {
                ServiceActor::<ServiceCore>::new(Services::Core, s_alloc.clone(), &system_moved)
            },
            &system,
            config_core,
        );
        let core = system.run_actor(spawn_item.unwrap());

        // Create and run the Presence Service.
        let s_alloc = allocation.clone();
        let system_moved = system.clone();
        spawn_item = factor::ActorBuilder::create_pool(
            move |_| {
                ServiceActor::<ServicePresence>::new(
                    Services::Presence,
                    s_alloc.clone(),
                    &system_moved,
                )
            },
            &system,
            config_presence,
        );
        let presence = system.run_actor(spawn_item.unwrap());

        // Create and run the Activity Service.
        let s_alloc = allocation.clone();
        let system_moved = system.clone();
        spawn_item = factor::ActorBuilder::create_pool(
            move |_| {
                ServiceActor::<ServiceActivity>::new(
                    Services::Activity,
                    s_alloc.clone(),
                    &system_moved,
                )
            },
            &system,
            config_activity,
        );
        let activity = system.run_actor(spawn_item.unwrap());

        // Create and run the Model Service.
        let s_alloc = allocation.clone();
        let system_moved = system.clone();
        spawn_item = factor::ActorBuilder::create_pool(
            move |_| {
                ServiceActor::<ServiceModel>::new(Services::Model, s_alloc.clone(), &system_moved)
            },
            &system,
            config_model,
        );
        let model = system.run_actor(spawn_item.unwrap());

        Self {
            core,
            presence,
            activity,
            model,
        }
    }
}
