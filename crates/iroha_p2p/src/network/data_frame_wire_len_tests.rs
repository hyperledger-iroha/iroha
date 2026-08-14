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
fn assert_relay_origin_signature_roundtrip(
    algorithm: Algorithm,
    seed_tag: u8,
    expected_signature_len: usize,
) {
    let key_pair = KeyPair::try_from_seed(vec![seed_tag; 32], algorithm)
        .unwrap_or_else(|error| panic!("derive deterministic {algorithm:?} key pair: {error}"));
    assert_eq!(key_pair.algorithm(), algorithm);
    let target_key_pair =
        KeyPair::try_from_seed(vec![seed_tag.wrapping_add(0x40); 32], Algorithm::Ed25519)
            .expect("derive deterministic relay target");
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
    .unwrap_or_else(|error| panic!("sign {algorithm:?} relay origin: {error}"));
    assert_eq!(
        frame.origin_signature.len(),
        expected_signature_len,
        "{algorithm:?} signature width changed"
    );
    assert_eq!(
        relay_origin_signature_len(&frame.origin),
        Some(expected_signature_len),
        "{algorithm:?} transport geometry must use the exact signature width"
    );
    frame
        .verify_origin_signature()
        .unwrap_or_else(|error| panic!("verify fresh {algorithm:?} relay: {error}"));
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
        "{algorithm:?} estimated wire geometry must match the signed frame"
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len::<DynamicDummy>(
            &frame.origin,
            Some(&target),
            payload.encoded_len(),
        ),
        materialized_wire_len,
        "{algorithm:?} payload-length geometry must match the signed frame"
    );
    let encoded = frame.encode();
    let (decoded, used) =
        <RelayMessage<DynamicDummy> as ncore::DecodeFromSlice>::decode_from_slice(&encoded)
            .unwrap_or_else(|error| panic!("decode {algorithm:?} relay: {error}"));
    assert_eq!(used, encoded.len());
    assert_eq!(decoded.origin, frame.origin);
    assert_eq!(decoded.origin_signature, frame.origin_signature);
    assert_eq!(decoded.origin_signature.len(), expected_signature_len);
    assert_eq!(decoded.ttl, frame.ttl);
    assert_eq!(decoded.priority, frame.priority);
    assert_eq!(decoded.payload.body, payload.body);
    match &decoded.target {
        RelayTarget::Direct(decoded_target) => assert_eq!(decoded_target, &target),
        RelayTarget::Broadcast => panic!("decoded {algorithm:?} relay lost its target"),
    }
    decoded
        .verify_origin_signature()
        .unwrap_or_else(|error| panic!("verify round-tripped {algorithm:?} relay: {error}"));
    let mut payload_tampered = decoded;
    payload_tampered.payload.body.push(0xFF);
    assert!(
        payload_tampered.verify_origin_signature().is_err(),
        "{algorithm:?} relay signature must bind the immutable payload"
    );
}
#[test]
fn relay_origin_signature_roundtrips_with_ed25519() {
    assert_relay_origin_signature_roundtrip(Algorithm::Ed25519, 0x11, 64);
}
#[test]
fn relay_origin_signature_roundtrips_with_secp256k1() {
    assert_relay_origin_signature_roundtrip(Algorithm::Secp256k1, 0x12, 64);
}
#[test]
fn relay_origin_signature_roundtrips_with_bls_normal() {
    assert_relay_origin_signature_roundtrip(Algorithm::BlsNormal, 0x13, 96);
}
#[test]
fn relay_origin_signature_roundtrips_with_bls_small() {
    assert_relay_origin_signature_roundtrip(Algorithm::BlsSmall, 0x14, 48);
}
#[test]
fn relay_origin_signature_roundtrips_with_ml_dsa_65() {
    assert_relay_origin_signature_roundtrip(
        Algorithm::MlDsa,
        0x15,
        MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
    );
}
#[cfg(feature = "gost")]
#[test]
fn relay_origin_signature_roundtrips_with_gost_256_param_set_a() {
    assert_relay_origin_signature_roundtrip(Algorithm::Gost3410_2012_256ParamSetA, 0x21, 64);
}
#[cfg(feature = "gost")]
#[test]
fn relay_origin_signature_roundtrips_with_gost_256_param_set_b() {
    assert_relay_origin_signature_roundtrip(Algorithm::Gost3410_2012_256ParamSetB, 0x22, 64);
}
#[cfg(feature = "gost")]
#[test]
fn relay_origin_signature_roundtrips_with_gost_256_param_set_c() {
    assert_relay_origin_signature_roundtrip(Algorithm::Gost3410_2012_256ParamSetC, 0x23, 64);
}
#[cfg(feature = "gost")]
#[test]
fn relay_origin_signature_roundtrips_with_gost_512_param_set_a() {
    assert_relay_origin_signature_roundtrip(Algorithm::Gost3410_2012_512ParamSetA, 0x24, 128);
}
#[cfg(feature = "gost")]
#[test]
fn relay_origin_signature_roundtrips_with_gost_512_param_set_b() {
    assert_relay_origin_signature_roundtrip(Algorithm::Gost3410_2012_512ParamSetB, 0x25, 128);
}
#[cfg(feature = "sm")]
#[test]
fn relay_origin_signature_roundtrips_with_sm2() {
    assert_relay_origin_signature_roundtrip(Algorithm::Sm2, 0x31, 64);
}
#[test]
fn data_frame_wire_len_matches_manual_envelope() {
    let origin = PeerId::from(KeyPair::random().public_key().clone());
    let target = PeerId::from(KeyPair::random().public_key().clone());
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
fn data_frame_wire_len_from_payload_len_matches_varint_boundaries_and_large_peer_ids() {
    let key_pair = KeyPair::from_seed(vec![0x51; 32], Algorithm::MlDsa);
    let (_, raw_key) = key_pair
        .public_key()
        .try_to_bytes()
        .expect("generated ML-DSA key is canonical");
    let raw_key_bytes = raw_key.len();
    assert_eq!(raw_key_bytes, 1_952, "ML-DSA-65 key width changed");
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
            data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
                raw_key_bytes,
                Some(raw_key_bytes),
                MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
                payload_len,
            ),
            direct,
            "synthetic direct peer geometry diverged at body length {body_len}"
        );
        assert_eq!(
            data_frame_wire_len_from_payload_len::<DynamicDummy>(&origin, None, payload_len,),
            broadcast,
            "broadcast frame geometry diverged at body length {body_len}"
        );
        assert_eq!(
            data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
                raw_key_bytes,
                None,
                MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
                payload_len,
            ),
            broadcast,
            "synthetic broadcast peer geometry diverged at body length {body_len}"
        );
        assert!(direct > broadcast);
    }
    assert_eq!(
        data_frame_wire_len_from_payload_len::<DynamicDummy>(&origin, Some(&target), usize::MAX,),
        usize::MAX,
        "payload-length overflow must fail closed"
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
            usize::MAX,
            None,
            MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
            0,
        ),
        usize::MAX,
        "origin key-length overflow must fail closed"
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
            raw_key_bytes,
            None,
            MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
            usize::MAX,
        ),
        usize::MAX,
        "synthetic payload-length overflow must fail closed"
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
            raw_key_bytes,
            Some(usize::MAX),
            MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
            0,
        ),
        usize::MAX,
        "target key-length overflow must fail closed"
    );
    assert_ne!(
        data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
            iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
            Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES),
            MAX_RELAY_ORIGIN_SIGNATURE_BYTES,
            0,
        ),
        usize::MAX,
        "protocol-maximum public-key geometry must remain representable"
    );
}
#[cfg(feature = "sm")]
#[test]
fn data_frame_wire_len_matches_materialized_maximum_sm2_peer_ids() {
    let distid = "x".repeat(u16::MAX as usize / 8);
    let private = iroha_crypto::Sm2PrivateKey::from_seed(&distid, b"p2p-maximum-sm2-peer")
        .expect("maximum SM2 distinguishing identifier is accepted");
    let key_payload = iroha_crypto::sm::encode_sm2_public_key_payload(
        &distid,
        &private.public_key().to_sec1_bytes(false),
    )
    .expect("encode maximum canonical SM2 public key payload");
    let public_key = iroha_crypto::PublicKey::from_bytes(Algorithm::Sm2, &key_payload)
        .expect("maximum canonical SM2 public key is accepted");
    let (_, raw_key) = public_key
        .try_to_bytes()
        .expect("maximum SM2 public key has canonical bytes");
    assert_eq!(raw_key.len(), iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES);
    let origin = PeerId::from(public_key);
    let target = origin.clone();
    assert!(
        origin.encoded_len() > 16_384,
        "the real PeerId fixture must cross the compact-length boundary missed by ML-DSA"
    );
    let payload = DynamicDummy {
        body: vec![0x5A; 128],
    };
    let payload_len = payload.encoded_len();
    let direct_frame = RelayMessage::new(
        origin.clone(),
        RelayTarget::Direct(target.clone()),
        u8::MAX,
        message::Priority::High,
        payload.clone(),
    );
    let direct_materialized = crate::peer::materialized_data_message_wire_len(direct_frame)
        .expect("materialize maximum-SM2 direct comparator");
    assert_eq!(
        data_frame_wire_len(
            &origin,
            Some(&target),
            u8::MAX,
            message::Priority::High,
            &payload
        ),
        direct_materialized
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len::<DynamicDummy>(&origin, Some(&target), payload_len,),
        direct_materialized
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
            iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
            Some(iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES),
            64,
            payload_len,
        ),
        direct_materialized
    );
    let broadcast_frame = RelayMessage::new(
        origin.clone(),
        RelayTarget::Broadcast,
        0,
        message::Priority::Low,
        payload.clone(),
    );
    let broadcast_materialized = crate::peer::materialized_data_message_wire_len(broadcast_frame)
        .expect("materialize maximum-SM2 broadcast comparator");
    assert_eq!(
        data_frame_wire_len(&origin, None, 0, message::Priority::Low, &payload),
        broadcast_materialized
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len::<DynamicDummy>(&origin, None, payload_len),
        broadcast_materialized
    );
    assert_eq!(
        data_frame_wire_len_from_payload_len_with_peer_key_bytes::<DynamicDummy>(
            iroha_crypto::MAX_PUBLIC_KEY_PAYLOAD_BYTES,
            None,
            64,
            payload_len,
        ),
        broadcast_materialized
    );
}
#[test]
fn relay_message_decode_from_slice_roundtrip() {
    let origin = PeerId::from(KeyPair::random().public_key().clone());
    let target = PeerId::from(KeyPair::random().public_key().clone());
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
    let origin = PeerId::from(KeyPair::random().public_key().clone());
    let target = PeerId::from(KeyPair::random().public_key().clone());
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
    let origin = PeerId::from(KeyPair::random().public_key().clone());
    let target = PeerId::from(KeyPair::random().public_key().clone());
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
    let origin = PeerId::from(KeyPair::random().public_key().clone());
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
    let origin = PeerId::from(KeyPair::random().public_key().clone());
    let frame = RelayMessage::new(
        origin,
        RelayTarget::Broadcast,
        1,
        message::Priority::High,
        DeniedDummy,
    );
    assert!(!message::ClassifyTopic::is_outbound_allowed(&frame));
}
