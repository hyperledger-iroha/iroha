use super::*;
use iroha_crypto::{Algorithm, KeyPair};
use norito::codec::{Decode, Encode};
#[derive(Clone, Debug, Decode, Encode)]
struct Dummy {
    tag: u8,
}
#[derive(Clone, Debug, Decode, Encode)]
struct DynamicDummy {
    body: Vec<u8>,
}
impl message::ClassifyTopic for DynamicDummy {
    const HAS_INBOUND_DECODE_LIMITS: bool = true;
    fn topic(&self) -> message::Topic {
        message::Topic::Control
    }
    fn inbound_topic(payload: &[u8], _flags: u8) -> Result<Option<message::Topic>, ncore::Error> {
        if payload.is_empty() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(Some(message::Topic::Control))
    }
    fn inbound_decode_limits(
        payload: &[u8],
        _framed_len: usize,
        _flags: u8,
    ) -> Result<Option<norito::DecodeLimits>, ncore::Error> {
        if payload.is_empty() {
            return Err(ncore::Error::LengthMismatch);
        }
        Ok(Some(norito::DecodeLimits::new(8, 64, 16, 128, 8)))
    }
}
#[derive(Clone, Debug, Decode, Encode)]
struct DeniedDummy;
impl message::ClassifyTopic for DeniedDummy {
    fn is_outbound_allowed(&self) -> bool {
        false
    }
}
fn node_key_pair(seed_tag: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed_tag; 32], Algorithm::BlsNormal)
        .expect("derive deterministic BLS-normal node key")
}
fn assert_relay_origin_signature_roundtrip(seed_tag: u8) {
    let key_pair = node_key_pair(seed_tag);
    let target_key_pair = node_key_pair(seed_tag.wrapping_add(0x40));
    let target = PeerId::from(target_key_pair.public_key().clone());
    let payload = DynamicDummy {
        body: vec![seed_tag, 0xC0, 0xDE],
    };
    let frame = RelayMessage::try_new(
        &key_pair,
        RelayTarget::Direct(target.clone()),
        7,
        message::Priority::High,
        payload.clone(),
    )
    .expect("sign BLS-normal relay origin");
    assert_eq!(
        frame.origin_signature.len(),
        RELAY_ORIGIN_SIGNATURE_BYTES,
        "BLS-normal signature width changed"
    );
    frame
        .verify_origin_signature()
        .expect("verify fresh BLS-normal relay");
    let materialized_wire_len = crate::peer::materialized_data_message_wire_len(frame.clone())
        .expect("materialize signed relay frame");
    assert_eq!(
        data_frame_wire_len(
            &frame.origin,
            Some(&target),
            frame.ttl,
            frame.priority,
            &payload,
        ),
        materialized_wire_len,
        "estimated wire geometry must match the signed frame"
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len::<DynamicDummy>(
            &frame.origin,
            Some(&target),
            payload.encoded_len(),
        ),
        materialized_wire_len,
        "payload-length geometry must match the signed frame"
    );
    let encoded = frame.encode();
    let (decoded, used) =
        <RelayMessage<DynamicDummy> as ncore::DecodeFromSlice>::decode_from_slice(&encoded)
            .expect("decode BLS-normal relay");
    assert_eq!(used, encoded.len());
    assert_eq!(decoded.origin, frame.origin);
    assert_eq!(decoded.origin_signature, frame.origin_signature);
    assert_eq!(decoded.origin_signature.len(), RELAY_ORIGIN_SIGNATURE_BYTES);
    assert_eq!(decoded.ttl, frame.ttl);
    assert_eq!(decoded.priority, frame.priority);
    assert_eq!(decoded.payload.body, payload.body);
    match &decoded.target {
        RelayTarget::Direct(decoded_target) => assert_eq!(decoded_target, &target),
        RelayTarget::Broadcast => panic!("decoded relay lost its target"),
    }
    decoded
        .verify_origin_signature()
        .expect("verify round-tripped BLS-normal relay");
    let mut signature_truncated = decoded.clone();
    signature_truncated.origin_signature.pop();
    assert!(
        signature_truncated.verify_origin_signature().is_err(),
        "relay signatures must have the exact BLS-normal width"
    );
    let mut payload_tampered = decoded;
    payload_tampered.payload.body.push(0xFF);
    assert!(
        payload_tampered.verify_origin_signature().is_err(),
        "relay signature must bind the immutable payload"
    );
}
#[test]
fn relay_origin_signature_roundtrips_with_bls_normal() {
    assert_relay_origin_signature_roundtrip(0x13);
}
#[test]
fn relay_origin_signature_rejects_non_node_algorithms() {
    let node_key_pair = node_key_pair(0x20);
    for algorithm in [
        Algorithm::Ed25519,
        Algorithm::Secp256k1,
        Algorithm::BlsSmall,
        Algorithm::MlDsa,
    ] {
        let key_pair = KeyPair::try_from_seed(vec![0x21; 32], algorithm)
            .expect("derive non-node rejection key");
        assert!(
            RelayMessage::try_new(
                &key_pair,
                RelayTarget::Broadcast,
                1,
                message::Priority::Low,
                Dummy { tag: 7 },
            )
            .is_err(),
            "{algorithm:?} must not sign a node relay envelope"
        );
        let unsupported_target = PeerId::from(key_pair.public_key().clone());
        assert!(
            RelayMessage::try_new(
                &node_key_pair,
                RelayTarget::Direct(unsupported_target.clone()),
                1,
                message::Priority::Low,
                Dummy { tag: 7 },
            )
            .is_err(),
            "{algorithm:?} must not identify a direct relay target"
        );
        assert_eq!(
            data_frame_wire_len_from_payload_len::<Dummy>(
                &PeerId::from(node_key_pair.public_key().clone()),
                Some(&unsupported_target),
                Dummy { tag: 7 }.encoded_len(),
            ),
            usize::MAX,
            "unsupported target geometry must fail closed"
        );
    }
}
#[test]
fn data_frame_wire_len_matches_manual_envelope() {
    let origin = PeerId::from(node_key_pair(0x31).public_key().clone());
    let target = PeerId::from(node_key_pair(0x32).public_key().clone());
    let payload = Dummy { tag: 7 };
    let direct = data_frame_wire_len(&origin, Some(&target), 8, message::Priority::High, &payload);
    let direct_from_len = data_frame_wire_len_from_payload_len::<Dummy>(
        &origin,
        Some(&target),
        payload.encoded_len(),
    );
    assert_eq!(direct_from_len, direct);
    let direct_frame = RelayMessage::new(
        origin.clone(),
        RelayTarget::Direct(target.clone()),
        8,
        message::Priority::High,
        payload.clone(),
    );
    let direct_expected = crate::peer::materialized_data_message_wire_len(direct_frame)
        .expect("materialize direct comparator frame");
    assert_eq!(
        direct, direct_expected,
        "direct frame size should match envelope"
    );
    let broadcast = data_frame_wire_len(&origin, None, 8, message::Priority::Low, &payload);
    let broadcast_from_len =
        data_frame_wire_len_from_payload_len::<Dummy>(&origin, None, payload.encoded_len());
    assert_eq!(broadcast_from_len, broadcast);
    let broadcast_frame = RelayMessage::new(
        origin,
        RelayTarget::Broadcast,
        8,
        message::Priority::Low,
        payload,
    );
    let broadcast_expected = crate::peer::materialized_data_message_wire_len(broadcast_frame)
        .expect("materialize broadcast comparator frame");
    assert_eq!(
        broadcast, broadcast_expected,
        "broadcast frame size should match envelope"
    );
}
#[test]
fn data_frame_wire_len_from_payload_len_matches_varint_boundaries() {
    let key_pair = node_key_pair(0x51);
    let origin = PeerId::from(key_pair.public_key().clone());
    let target = origin.clone();
    for body_len in [0, 1, 127, 128, 16_383, 16_384] {
        let payload = DynamicDummy {
            body: vec![0xA5; body_len],
        };
        let payload_len = payload.encoded_len();
        let direct = data_frame_wire_len(
            &origin,
            Some(&target),
            u8::MAX,
            message::Priority::High,
            &payload,
        );
        let broadcast = data_frame_wire_len(&origin, None, 0, message::Priority::Low, &payload);
        assert_eq!(
            data_frame_wire_len_from_payload_len::<DynamicDummy>(
                &origin,
                Some(&target),
                payload_len,
            ),
            direct,
            "direct frame geometry diverged at body length {body_len}"
        );
        assert_eq!(
            direct_data_frame_wire_len_from_payload_len::<DynamicDummy>(payload_len),
            direct,
            "canonical direct peer geometry diverged at body length {body_len}"
        );
        assert_eq!(
            data_frame_wire_len_from_payload_len::<DynamicDummy>(&origin, None, payload_len,),
            broadcast,
            "broadcast frame geometry diverged at body length {body_len}"
        );
        assert_eq!(
            broadcast_data_frame_wire_len_from_payload_len::<DynamicDummy>(payload_len),
            broadcast,
            "canonical broadcast peer geometry diverged at body length {body_len}"
        );
        assert!(direct > broadcast);
    }
    assert_eq!(
        data_frame_wire_len_from_payload_len::<DynamicDummy>(&origin, Some(&target), usize::MAX,),
        usize::MAX,
        "payload-length overflow must fail closed"
    );
    assert_eq!(
        broadcast_data_frame_wire_len_from_payload_len::<DynamicDummy>(usize::MAX),
        usize::MAX,
        "canonical broadcast payload-length overflow must fail closed"
    );
}
#[test]
fn relay_message_decode_from_slice_roundtrip() {
    let origin = PeerId::from(node_key_pair(0x61).public_key().clone());
    let target = PeerId::from(node_key_pair(0x62).public_key().clone());
    let payload = Dummy { tag: 42 };
    let frame = RelayMessage::new(
        origin.clone(),
        RelayTarget::Direct(target.clone()),
        5,
        message::Priority::High,
        payload.clone(),
    );
    let bytes = frame.encode();
    let (decoded, used) =
        <RelayMessage<Dummy> as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
            .expect("decode relay message");
    assert_eq!(used, bytes.len(), "should consume full payload");
    assert_eq!(decoded.origin, origin);
    assert_eq!(decoded.ttl, 5);
    assert_eq!(decoded.priority, message::Priority::High);
    assert_eq!(decoded.payload.tag, payload.tag);
    match decoded.target {
        RelayTarget::Direct(peer_id) => assert_eq!(peer_id, target),
        RelayTarget::Broadcast => panic!("expected direct relay target"),
    }
}
#[test]
fn relay_message_decode_from_slice_roundtrip_with_dynamic_payload() {
    let origin = PeerId::from(node_key_pair(0x63).public_key().clone());
    let target = PeerId::from(node_key_pair(0x64).public_key().clone());
    let payload = DynamicDummy {
        body: vec![1u8, 2, 3, 4],
    };
    let frame = RelayMessage::new(
        origin.clone(),
        RelayTarget::Direct(target.clone()),
        6,
        message::Priority::Low,
        payload.clone(),
    );
    let bytes = frame.encode();
    let (decoded, used) =
        <RelayMessage<DynamicDummy> as ncore::DecodeFromSlice>::decode_from_slice(&bytes)
            .expect("decode relay message");
    assert_eq!(used, bytes.len(), "should consume full payload");
    assert_eq!(decoded.origin, origin);
    assert_eq!(decoded.ttl, 6);
    assert_eq!(decoded.priority, message::Priority::Low);
    assert_eq!(decoded.payload.body, payload.body);
    match decoded.target {
        RelayTarget::Direct(peer_id) => assert_eq!(peer_id, target),
        RelayTarget::Broadcast => panic!("expected direct relay target"),
    }
}
#[test]
fn relay_envelope_delegates_policy_to_exact_nested_payload() {
    let origin = PeerId::from(node_key_pair(0x65).public_key().clone());
    let target = PeerId::from(node_key_pair(0x66).public_key().clone());
    let nested = DynamicDummy {
        body: vec![1, 3, 3, 7],
    };
    let frame = RelayMessage::new(
        origin,
        RelayTarget::Direct(target),
        5,
        message::Priority::High,
        nested.clone(),
    );
    let (bare, flags) = norito::codec::encode_with_header_flags(&frame);
    let nested_bare = nested.encode();
    assert_eq!(
        relay_message_payload_field(&bare, flags).expect("extract nested relay payload"),
        nested_bare,
        "the predecode hook must inspect only the nested application payload"
    );
    assert_eq!(
        <RelayMessage<DynamicDummy> as message::ClassifyTopic>::inbound_decode_limits(
            &bare, 512, flags,
        )
        .expect("delegate nested policy"),
        Some(norito::DecodeLimits::new(8, 64, 16, 128, 8))
    );
    assert_eq!(
        <RelayMessage<DynamicDummy> as message::ClassifyTopic>::inbound_topic(&bare, flags)
            .expect("delegate nested raw topic"),
        Some(message::Topic::Control),
        "the relay wrapper must classify the exact nested application field"
    );
}
#[test]
fn relay_payload_extractor_rejects_truncated_or_trailing_layouts() {
    let origin = PeerId::from(node_key_pair(0x67).public_key().clone());
    let frame = RelayMessage::new(
        origin,
        RelayTarget::Broadcast,
        1,
        message::Priority::Low,
        DynamicDummy { body: vec![9] },
    );
    let (bare, flags) = norito::codec::encode_with_header_flags(&frame);
    assert!(
        relay_message_payload_field(&bare[..bare.len() - 1], flags).is_err(),
        "truncated relay layout must fail before typed decode"
    );
    assert!(
        <RelayMessage<DynamicDummy> as message::ClassifyTopic>::inbound_topic(
            &bare[..bare.len() - 1],
            flags,
        )
        .is_err(),
        "the raw topic wrapper must fail closed on a truncated relay"
    );
    let mut trailing = bare;
    trailing.push(0);
    assert!(
        relay_message_payload_field(&trailing, flags).is_err(),
        "trailing bytes must not be ignored by the raw envelope parser"
    );
    assert!(
        <RelayMessage<DynamicDummy> as message::ClassifyTopic>::inbound_topic(&trailing, flags,)
            .is_err(),
        "the raw topic wrapper must fail closed on relay trailing bytes"
    );
}
#[test]
fn relay_message_preserves_outbound_admission_policy() {
    let origin = PeerId::from(node_key_pair(0x68).public_key().clone());
    let frame = RelayMessage::new(
        origin,
        RelayTarget::Broadcast,
        1,
        message::Priority::High,
        DeniedDummy,
    );
    assert!(!message::ClassifyTopic::is_outbound_allowed(&frame));
}
