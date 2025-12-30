use eyre::Result;
pub use hickory_proto::op::Message as DnsMessage;
use hickory_proto::op::{Message, MessageType, ResponseCode};

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
    let mut response = Message::new();
    response.set_id(request.id());
    response.set_message_type(MessageType::Response);
    response.set_op_code(request.op_code());
    response.set_recursion_desired(request.recursion_desired());
    response.set_recursion_available(true);
    response.set_authoritative(authoritative);
    response.set_response_code(code);
    for query in request.queries() {
        response.add_query(query.clone());
    }
    response
}
