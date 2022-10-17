use uuid::Uuid;

use serde::{Deserialize, Serialize};

/// Collaborative space Id.
/// A collaborative space is a realtime-space/room that hosts multiple
/// clients and provides activities that the present clients could
/// perform on resources under a single model-root.
/// A cospace-id is unique globally, even for multiple instances of
/// the realtime-server.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct CospaceId {
    pub uuid: Uuid, // [todo] distributed-id
}

impl CospaceId {
    /// associated function to generate a new cospace id
    pub fn generate() -> Self {
        Self {
            uuid: Uuid::new_v4(),
        }
    }
}

/// Client Id of the clients connected to a collaborative space.
/// Client ids are unique inside a collaborative space, not globally.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct ClientId {
    pub id: u32,
    pub cospace: CospaceId,
}

/// Identifier/info of the persistent root in an applications data-model 
/// that could only be opened in a single collaboration space at any
/// given instant. This is persistent root-entity that the cospace is hosting.
/// This could be a document-root for a design app, a world-root for a
/// mini-game, etc. It is upto the application to map it appropriately.
/// 
/// The model-root hosted by the collaboration space.
/// A model-root usually maps to concepts workspaces and documents.
/// The "namespace" and "workspace" uniquely identifies a model-root.
/// The "namespace" and "workspace" could map to any node in your
/// data-model, when your application wants that node to be hosted by
/// only one collaborative space at any given instant.
/// In general, "namespace" is the organization identifier and
/// "workspaces" are top-level documents of that organization.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ModelRoot {
    pub namespace: String, // e.g. universe
    pub workspace: String, // e.g. world
}
