//! Core network prefixes preserve envelope boundaries and live protocol-version admission.

use std::sync::Arc;

use crate::{NetworkMessage, sumeragi::net::SumeragiFrame};
use iroha_sumeragi::{
    message::{BlockRequest, WireMessage},
    types::Hash32,
};
use norito::{
    DeserializePayload, SerializePayload,
    core::{self as ncore, DecodeFromSlice},
};

fn consensus_frame() -> SumeragiFrame {
    let instance = Hash32([0x51; 32]);
    let message = WireMessage::BlockRequest(BlockRequest {
        instance,
        height: 3,
        block_hash: Hash32([0x71; 32]),
    });
    SumeragiFrame::new(instance, message.encode().unwrap())
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
    assert_prefix(&NetworkMessage::Health);
    assert_prefix(&NetworkMessage::Sumeragi(Arc::new(consensus_frame())));
}

#[test]
fn core_network_payload_prefix_preserves_container_boundaries() {
    assert_prefix(&Some(NetworkMessage::Health));
    assert_prefix(&vec![NetworkMessage::Health, NetworkMessage::Health]);
}

#[test]
fn core_block_payload_prefix_preserves_live_message_boundaries() {
    assert_prefix(&NetworkMessage::Sumeragi(Arc::new(consensus_frame())));
}

#[test]
fn core_frame_preserves_exact_bytes_and_rejects_malformed_native_framing() {
    for flags in [0, ncore::header_flags::COMPACT_LEN] {
        let _flags = ncore::DecodeFlagsGuard::enter(flags);
        let live = consensus_frame();
        let bytes = payload(&live);
        let (decoded, consumed): (SumeragiFrame, usize) =
            ncore::decode_field_canonical(&bytes).unwrap();
        assert_eq!(consumed, bytes.len());
        assert_eq!(decoded, live);
        assert!(WireMessage::decode(decoded.bytes(), usize::MAX).is_ok());
        let mut invalid = live.bytes().to_vec();
        invalid[0] ^= 0xff;
        let bad = SumeragiFrame::new(live.instance(), invalid);
        assert!(WireMessage::decode(bad.bytes(), usize::MAX).is_err());
        assert!(bad.class().is_none());
        assert_eq!(ncore::get_decode_flags(), flags);
    }
}
