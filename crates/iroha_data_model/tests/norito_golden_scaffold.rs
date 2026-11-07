//! Norito golden scaffolding for data-model types.
//!
//! These tests pin stable encodings for core data-model types so future changes
//! surface deterministic diffs instead of silent codec drift.
use hex_literal::hex;
use iroha_data_model::block::BlockHeader;
use iroha_primitives::nonzero;
use norito::codec::{DecodeAll, Encode};
#[test]
fn block_header_roundtrip() {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 12345, 0);
    let bytes = norito::to_bytes(&header).expect("encode");
    let decoded: BlockHeader = norito::decode_from_bytes(&bytes).expect("decode");
    assert_eq!(decoded, header);
}
#[test]
fn block_header_golden_bytes() {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 12345, 0);
    let bytes = header.encode();
    // The default policy is pinned against Core's compiled ZK/SCCP inputs.
    // The current layout includes the execution-context and beacon commitment slots.
    let expected: &[u8] = &hex!(
        "08010000000000000001000100010001000100010008393000000000000008000000000000000052015001000100010006010401000000420140019101d301b101b10108016301400151019001a401ab012d0178013801bc01a2012b01d90184010801a0013f01e001f801de01ff01dc013501c2014d01df017a01000100"
    );
    assert_eq!(bytes.as_slice(), expected);
    let mut cursor = expected;
    assert_eq!(
        BlockHeader::decode_all(&mut cursor).expect("decode complete header golden"),
        header,
    );
    let mut without_beacon = &expected[..expected.len() - 2];
    assert!(
        BlockHeader::decode_all(&mut without_beacon).is_err(),
        "a header without the required beacon commitment slot must be rejected",
    );
}
