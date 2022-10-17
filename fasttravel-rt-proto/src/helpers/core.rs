//!
//! Protocol buffer helpers for core service used by both the server and the client-sdk.
//!

#![allow(dead_code)]

use log::{error, trace};
use prost::Message;

use super::ProtoBytes;
use crate::realtime;

/// Decode core service messages.
pub fn decode_core_message_and_extract_payload(
    bytes: ProtoBytes,
) -> Option<realtime::core::message::Payload> {
    trace!("decode_core_message BYTES_LEN: {}", bytes.len());

    realtime::core::Message::decode(&bytes[..])
        .map(|msg| msg.payload)
        .map_err(|e| error!("decode_core_message_and_extract_payload: {}", e))
        .unwrap_or(None)
}
