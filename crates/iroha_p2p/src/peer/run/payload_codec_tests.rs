//! Message variants preserve prefix consumption and enclosing decode limits.

use super::Message;
use crate::payload_codec_tests::{allocation_limit, prefix};

#[test]
fn message_payload_prefix_preserves_boundaries_in_every_layout() {
    for message in [Message::Data(0x12345678_u32), Message::Ping, Message::Pong] {
        prefix(&message);
        prefix(&Some(message.clone()));
        prefix(&vec![message.clone(), message]);
    }
}

#[test]
fn message_payload_prefix_inherits_decode_limits() {
    allocation_limit(&Message::Data("peer message".to_owned()));
}

#[test]
fn message_payload_prefix_accepts_a_payload_only_child() {
    prefix(&Message::Data(crate::payload_codec_tests::PayloadOnly(42)));
}
