//! Original relay-envelope and nested P2P frame fixture regressions.
use super::*;
use crate::frame_identity_tests::{preimage, shapes};
fn relay<T>(owner: &str, nested: &str, value: T)
where
    T: Clone + ncore::NoritoSerialize + for<'de> ncore::NoritoDeserialize<'de>,
{
    let key = KeyPair::try_from_seed(vec![0x13; 32], Algorithm::BlsNormal).unwrap();
    let target_key = KeyPair::try_from_seed(vec![0x53; 32], Algorithm::BlsNormal).unwrap();
    for (variant, target) in [
        ("broadcast", RelayTarget::Broadcast),
        (
            "direct",
            RelayTarget::Direct(PeerId::from(target_key.public_key().clone())),
        ),
    ] {
        let frame = RelayMessage::try_new(&key, target, 7, value.clone()).unwrap();
        frame.verify_origin_signature().unwrap();
        shapes(owner, variant, &frame);
        let digest = relay_origin_signature_digest(&frame.origin, &frame.target, &frame.payload);
        preimage(owner, variant, digest.as_ref());
        crate::peer::assert_captured_p2p_message(nested, variant, frame);
    }
}
#[test]
fn captured_original_p2p_relay_frames() {
    relay("relay_u32", "message_relay_u32", 0x12345678_u32);
    relay("relay_u64", "message_relay_u64", 0x0123456789abcdef_u64);
}

#[test]
fn relay_payload_prefix_preserves_boundaries_in_every_layout() {
    let key = KeyPair::try_from_seed(vec![0x13; 32], Algorithm::BlsNormal).unwrap();
    let target = PeerId::from(key.public_key().clone());
    for target in [RelayTarget::Broadcast, RelayTarget::Direct(target)] {
        let relay = RelayMessage::try_new(&key, target, 7, 0x12345678_u32).unwrap();
        crate::payload_codec_tests::prefix(&relay);
        crate::payload_codec_tests::prefix(&Some(relay.clone()));
        crate::payload_codec_tests::prefix(&vec![relay.clone(), relay]);
    }
}

#[test]
fn relay_payload_prefix_inherits_decode_limits() {
    let key = KeyPair::try_from_seed(vec![0x13; 32], Algorithm::BlsNormal).unwrap();
    let relay = RelayMessage::try_new(&key, RelayTarget::Broadcast, 7, 42_u32).unwrap();
    crate::payload_codec_tests::allocation_limit(&relay);
}

#[test]
fn relay_payload_prefix_accepts_a_payload_only_child() {
    let key = KeyPair::try_from_seed(vec![0x13; 32], Algorithm::BlsNormal).unwrap();
    let relay = RelayMessage::try_new(
        &key,
        RelayTarget::Broadcast,
        7,
        crate::payload_codec_tests::PayloadOnly(42),
    )
    .unwrap();
    crate::payload_codec_tests::prefix(&relay);
}
