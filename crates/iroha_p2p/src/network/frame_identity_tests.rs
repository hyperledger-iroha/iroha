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
