//!
//! Protocol buffer helpers for connection service used by both the server and the client-sdk.
//!

#![allow(dead_code)]

use log::{error, trace};
use prost::Message;

use super::ProtoBytes;
use crate::realtime;

/// Create the handshake request to be always sent as first message from client.
pub fn create_ticket_handshake_request(ticket: String) -> ProtoBytes {
    trace!("create_ticket_handshake_request TICKET: {}", ticket);

    let ticket_req = realtime::connection::TicketHandshakeRequest { ticket };
    let connection_payload = realtime::connection::message::Payload::HandshakeReq(ticket_req);
    let connection_msg = realtime::connection::Message {
        payload: Some(connection_payload),
    };

    connection_msg.encode_to_vec()
}

/// Create the handshake response to be always sent as first message from  
/// server as response to client's first ticket handshake request message.
pub fn create_ticket_handshake_response(success: bool) -> ProtoBytes {
    trace!("create_ticket_handshake_response SUCCESS: {}", success);

    let ticket_res = realtime::connection::TicketHandshakeResponse { success };
    let connection_payload = realtime::connection::message::Payload::HandshakeRes(ticket_res);
    let connection_msg = realtime::connection::Message {
        payload: Some(connection_payload),
    };

    connection_msg.encode_to_vec()
}

/// Decode connection service messages.
pub fn decode_connection_message_and_extract_payload(
    bytes: ProtoBytes,
) -> Option<realtime::connection::message::Payload> {
    trace!("decode_connection_message BYTES_LEN: {}", bytes.len());

    realtime::connection::Message::decode(&bytes[..])
        .map(|msg| msg.payload)
        .map_err(|e| error!("decode_connection_message_and_extract_payload: {}", e))
        .unwrap_or(None)
}
