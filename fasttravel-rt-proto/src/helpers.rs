//!
//! Protocol buffer helpers used by both the server and the client-sdk.
//!

#![allow(dead_code)]

pub mod connection;
pub mod core;

use log::{error, trace};
use prost::Message;

use crate::{
    realtime::{self, realtime_message, realtime_message::Body, RealtimeMessage},
    RealtimeService,
};

/// Message types based on the RealtimeMessage.header request_id and response_id fields.
/// The payload is the service specific body extracted after striping the header.
pub enum ProtoMessage {
    /// undefined message type, either uninitialized or error.
    Undefined,
    /// a tell message
    Tell(ProtoPayloadTell),
    /// a request initiated through an ask.
    Request(ProtoPayloadRequest),
    /// the response to a previous ask request.
    Response(ProtoPayloadResponse),
}

pub type ProtoBytes = Vec<u8>;

/// Extracted payload of a TELL message.
pub struct ProtoPayloadTell {
    pub service: RealtimeService,
    pub bytes: ProtoBytes,
}

/// Extracted payload of a REQUEST message.
pub struct ProtoPayloadRequest {
    pub request_id: u32,
    pub service: RealtimeService,
    pub bytes: ProtoBytes,
}

/// Extracted payload of a RESPONSE message.
pub struct ProtoPayloadResponse {
    pub response_id: u32,
    pub service: RealtimeService,
    pub bytes: ProtoBytes,
}

///
/// Process the protocol buffers received over the socket.
/// 1. Decode the bytes to RealtimeMessage.
/// 2. Check header and body fields of the message.
/// 3. Determine whether the message is TELL, REQUEST, or RESPONSE.
/// 4. Extract service specific payload.
/// 5. Return a ProtoMessage with appropriate payload.
///
pub fn process_realtime_message(msg: ProtoBytes) -> ProtoMessage {
    trace!("process_realtime_message");

    realtime::RealtimeMessage::decode(&msg[..])
        .map(|rt_msg| process_header_and_message(rt_msg))
        .map_err(|e| error!("process_realtime_message_ERROR: {}", e))
        .unwrap_or(ProtoMessage::Undefined)
}

/// Create a TELL message from a service payload.
pub fn create_tell_message_from_service_payload(
    service: &RealtimeService,
    payload: ProtoBytes,
) -> Option<ProtoBytes> {
    trace!("create_tell_message_from_service_payload");

    create_rt_message_from_service_payload(service, payload)
        .and_then(|rt_msg| Some(rt_msg.encode_to_vec()))
}

/// Create a RESPONSE message from a service payload and the RESPONSE_ID
pub fn create_response_message_from_service_payload(
    response_id: u32,
    service: &RealtimeService,
    payload: ProtoBytes,
) -> Option<ProtoBytes> {
    trace!("create_response_message_from_service_payload");

    create_rt_message_from_service_payload(service, payload).and_then(|mut rt_msg| {
        let mut response_header = realtime_message::Header::default();
        response_header.response_id = response_id;
        rt_msg.header = Some(response_header);

        Some(rt_msg.encode_to_vec())
    })
}

/// Create a REQUEST message from a service payload and the REQUEST_ID
pub fn create_request_message_from_service_payload(
    request_id: u32,
    service: &RealtimeService,
    payload: ProtoBytes,
) -> Option<ProtoBytes> {
    trace!("create_request_message_from_service_payload");

    create_rt_message_from_service_payload(service, payload).and_then(|mut rt_msg| {
        let mut request_header = realtime_message::Header::default();
        request_header.request_id = request_id;
        rt_msg.header = Some(request_header);

        Some(rt_msg.encode_to_vec())
    })
}

fn create_rt_message_from_service_payload(
    service: &RealtimeService,
    payload: ProtoBytes,
) -> Option<RealtimeMessage> {
    let mut body = None;

    match service {
        RealtimeService::Connection => {
            let _ = realtime::connection::Message::decode(&payload[..])
                .map(|msg| body = Some(Body::ConnectionMsg(msg)))
                .map_err(|e| error!("realtime_connection_Message_decode {}", e));
        }
        RealtimeService::Core => {
            let _ = realtime::core::Message::decode(&payload[..])
                .map(|msg| body = Some(Body::CoreMsg(msg)))
                .map_err(|e| error!("realtime_core_Message_decode {}", e));
        }
        _ => {
            error!("create_rt_message_from_service_payload_ERROR_service_not_supported_yet");
        }
    }

    body.and_then(|b| {
        let mut rt_msg = realtime::RealtimeMessage::default();
        rt_msg.body = Some(b);

        Some(rt_msg)
    })
}

///
/// Extracts the service specific payload from the realtime message.
/// 1. Check if payload is valid.
/// 2. Check the audience or issuer service.
/// 3. get the service specifc payload.
/// 4. Return service and service-payload tuple.
///
fn extract_service_payload_from_rt_message(
    body: realtime_message::Body,
) -> Option<(RealtimeService, ProtoBytes)> {
    trace!("extract_service_payload_from_rt_message");

    let service;
    let proto_bytes;

    // This does a re-encoding, as we want the services to handle their own
    // payload and to easily move encoded-bytes instead of structs.
    // [todo] performance tests (for realtime message sizes this may not be an issue).
    match body {
        Body::ConnectionMsg(service_msg) => {
            service = RealtimeService::Connection;
            proto_bytes = Some(service_msg.encode_to_vec());
        }
        Body::CoreMsg(service_msg) => {
            service = RealtimeService::Core;
            proto_bytes = Some(service_msg.encode_to_vec());
        }
        Body::PresenceMsg(service_msg) => {
            service = RealtimeService::Presence;
            proto_bytes = Some(service_msg.encode_to_vec());
        }
        Body::ActivityMsg(service_msg) => {
            service = RealtimeService::Activity;
            proto_bytes = Some(service_msg.encode_to_vec());
        }
        Body::ModelMsg(service_msg) => {
            service = RealtimeService::Model;
            proto_bytes = Some(service_msg.encode_to_vec());
        }
    }

    // currently check redundant, but we might need when we add more services.
    if proto_bytes.is_none() || service == RealtimeService::Undefined {
        return None;
    }

    Some((service, proto_bytes.unwrap()))
}

///
/// Process the header and body fields of the realtime message.
/// 1. Check if header and body fields are valid.
/// 2. Check header request_id and response_id fields.
/// 3. Determine if message is TELL, REQUEST, or RESPONSE.
/// 4. Extract service specific payload.
/// 5. Return the ProtoMessage with appropriate payload.
///
fn process_header_and_message(rt_msg: RealtimeMessage) -> ProtoMessage {
    // check for undefined message
    if rt_msg.header.is_none() || rt_msg.body.is_none() {
        error!("process_header_and_message_ERROR_undefined_header_or_body");
        return ProtoMessage::Undefined;
    }

    let header = rt_msg.header.unwrap();
    let body = rt_msg.body.unwrap();

    if header.request_id != 0 && header.response_id != 0 {
        error!("process_header_and_message_ERROR_req_res_id_both_set");
        return ProtoMessage::Undefined;
    }

    extract_service_payload_from_rt_message(body)
        .map(|payload| {
            if header.request_id != 0 {
                let request_payload = ProtoPayloadRequest {
                    request_id: header.request_id,
                    service: payload.0,
                    bytes: payload.1,
                };

                return ProtoMessage::Request(request_payload);
            } else if header.response_id != 0 {
                let response_payload = ProtoPayloadResponse {
                    response_id: header.response_id,
                    service: payload.0,
                    bytes: payload.1,
                };

                return ProtoMessage::Response(response_payload);
            } else {
                let tell_payload = ProtoPayloadTell {
                    service: payload.0,
                    bytes: payload.1,
                };

                return ProtoMessage::Tell(tell_payload);
            }
        })
        .or_else(|| {
            error!("extract_service_payload_from_rt_message_ERROR");
            Some(ProtoMessage::Undefined)
        })
        .expect("extract_service_payload_from_rt_message_EXPECT_ERROR")
}
