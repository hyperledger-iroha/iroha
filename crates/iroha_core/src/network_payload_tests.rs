//! Core network prefixes preserve envelope boundaries and live protocol-version admission.

use std::sync::Arc;

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::consensus_v2 as wire;
use norito::{
    DeserializePayload, SerializePayload,
    core::{self as ncore, DecodeFromSlice},
};

use crate::{
    NetworkMessage,
    sumeragi::message::{BlockMessage, BlockMessageWire},
};

fn block_message() -> BlockMessage {
    BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(wire::PayloadChunk {
            manifest_hash: HashOf::from_untyped_unchecked(Hash::new(b"prefix-test-manifest")),
            index: 0,
            bytes: vec![0x71],
            sender: 3,
            signature: vec![0x72],
        }),
    ))
}

fn payload<T: SerializePayload>(value: &T) -> Vec<u8> {
    let mut bytes = Vec::new();
    value
        .serialize(&mut ncore::Encoder::new(&mut bytes))
        .unwrap();
    bytes
}

fn assert_prefix<T>(value: &T)
where
    T: SerializePayload + for<'de> DeserializePayload<'de> + for<'de> DecodeFromSlice<'de>,
{
    for flags in (0..=ncore::supported_header_flags())
        .filter(|flags| ncore::validate_header_flags(*flags).is_ok())
    {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let bytes = payload(value);
        let mut trailing = bytes.clone();
        trailing.extend_from_slice(&[0xa5, 0x5a]);
        let (decoded, used) = T::decode_from_slice(&trailing).expect("one Core payload prefix");
        assert_eq!(used, bytes.len());
        assert_eq!(&trailing[used..], &[0xa5, 0x5a]);
        assert_eq!(payload(&decoded), bytes);
        assert!(matches!(
            ncore::decode_field_canonical::<T>(&trailing),
            Err(ncore::Error::LengthMismatch)
        ));
        for end in 0..bytes.len() {
            assert!(
                T::decode_from_slice(&bytes[..end]).is_err(),
                "truncated at {end}"
            );
        }
        assert!(T::decode_from_slice(&bytes).is_ok());
        assert_eq!(ncore::get_decode_flags(), flags);
    }
}

#[test]
fn core_network_payload_prefix_preserves_nested_canonical_block_frames() {
    let wire = BlockMessageWire::try_preencoded(Arc::new(block_message())).unwrap();
    assert_prefix(&NetworkMessage::Health);
    assert_prefix(&NetworkMessage::SumeragiBlock(Arc::new(wire)));
}

#[test]
fn core_network_payload_prefix_preserves_container_boundaries() {
    assert_prefix(&Some(NetworkMessage::Health));
    assert_prefix(&vec![NetworkMessage::Health, NetworkMessage::Health]);
}

#[test]
fn core_block_payload_prefix_preserves_live_message_boundaries() {
    assert_prefix(&block_message());
}

#[test]
fn core_block_payload_prefix_rejects_unsupported_versions_without_losing_context() {
    for flags in [
        0,
        ncore::header_flags::COMPACT_LEN,
        ncore::header_flags::PACKED_STRUCT,
    ] {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let mut bad = block_message();
        let BlockMessage::V2(ref mut message) = bad else {
            unreachable!()
        };
        message.protocol_version = message.protocol_version.saturating_sub(1);
        let mut bad = payload(&bad);
        bad.extend_from_slice(&[0xa5, 0x5a]);
        assert!(matches!(
            BlockMessage::decode_from_slice(&bad),
            Err(ncore::Error::Message(message)) if message.starts_with("unsupported Sumeragi v2 message version:")
        ));
        assert_eq!(ncore::get_decode_flags(), flags);
        let live = block_message();
        let bytes = payload(&live);
        let (decoded, used) = BlockMessage::decode_from_slice(&bytes).unwrap();
        assert_eq!(used, bytes.len());
        assert_eq!(payload(&decoded), bytes);
    }
}
