use eyre::Result;
pub use hickory_proto::op::Message as DnsMessage;
use hickory_proto::op::{Message, ResponseCode};

/// Decode a DNS message from a byte slice.
pub fn decode_message(bytes: &[u8]) -> Result<Message> {
    Ok(Message::from_vec(bytes)?)
}

/// Encode a DNS message into bytes.
pub fn encode_message(message: &Message) -> Result<Vec<u8>> {
    Ok(message.to_vec()?)
}

/// Build a SERVFAIL response mirroring the request metadata.
pub fn build_servfail_response(request: &Message) -> Message {
    build_basic_response(request, ResponseCode::ServFail, false)
}

/// Build an NXDOMAIN response for unknown names.
pub fn build_nxdomain_response(request: &Message) -> Message {
    build_basic_response(request, ResponseCode::NXDomain, true)
}

fn build_basic_response(request: &Message, code: ResponseCode, authoritative: bool) -> Message {
    let mut response = Message::response(request.metadata.id, request.metadata.op_code);
    response.metadata.recursion_desired = request.metadata.recursion_desired;
    response.metadata.recursion_available = true;
    response.metadata.authoritative = authoritative;
    response.metadata.response_code = code;
    for query in &request.queries {
        response.add_query(query.clone());
    }
    response
}
