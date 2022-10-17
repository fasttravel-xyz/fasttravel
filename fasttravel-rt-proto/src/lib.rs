//!
//! Package contains the protocol buffer definitions of the fasttravel
//! realtime messages.
//!
//! Along with the protocol buffer definitions this package contains:
//! * The Rust crate that uses [`prost`] to generate the Rust bindings
//! for the protocol buffers.
//! * Helper functionalities used by both the server and the client-sdk.
//!
//! [`prost`]: https://github.com/tokio-rs/prost
//!
//! WARNING: The current version of this repository is 0.0.1-dev0 and is
//! undergoing development for the first release client 0.1.0-rc0, which
//! means that both the public interfaces and internal module structures
//! may change significantly.
//!

use serde::{Deserialize, Serialize};

/// Pre-defined realtime services.
/// The audience or the issuer service of a realtime message.
#[derive(PartialEq, Clone, Debug, Serialize, Deserialize)]
pub enum RealtimeService {
    Undefined,
    Connection,
    Core,
    Presence,
    Activity,
    Model,
}

/// Generated Rust bindings for the proto definitions.
#[rustfmt::skip]
pub mod realtime {
    pub mod connection { include!(concat!(env!("OUT_DIR"), "/fasttravel.realtime.connection.rs")); }
    pub mod core { include!(concat!(env!("OUT_DIR"), "/fasttravel.realtime.core.rs")); }
    pub mod presence { include!(concat!(env!("OUT_DIR"), "/fasttravel.realtime.presence.rs")); }
    pub mod activity { include!(concat!(env!("OUT_DIR"), "/fasttravel.realtime.activity.rs")); }
    pub mod model { include!(concat!(env!("OUT_DIR"), "/fasttravel.realtime.model.rs")); }

    include!(concat!(env!("OUT_DIR"), "/fasttravel.realtime.rs"));
}

/// Protocol buffer helpers used by both the server and the client-sdk.
pub mod helpers;
