//! Norito golden scaffolding for data-model types.
//!
//! These tests pin stable encodings for core data-model types so future changes
//! surface deterministic diffs instead of silent codec drift.

use hex_literal::hex;
use iroha_data_model::block::BlockHeader;
use nonzero_ext::nonzero;
use norito::codec::Encode;

#[test]
fn block_header_roundtrip() {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 12345, 0);
    let bytes = norito::to_bytes(&header).expect("encode");
    let decoded: BlockHeader = norito::decode_from_bytes(&bytes).expect("decode");
    assert_eq!(decoded, header);
}

#[test]
fn block_header_golden_bytes() {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 12345, 0);
    let bytes = header.encode();
    let expected: &[u8] = &hex!(
        "08000000000000000100000000000000010000000000000000010000000000000000010000000000000000010000000000000000010000000000000000010000000000000000080000000000000039300000000000000800000000000000000000000000000039000000000000000130000000000000000100000000000000000100000000000000000100000000000000000d0000000000000001040000000000000001000000"
    );
    assert_eq!(bytes.as_slice(), expected);
}
