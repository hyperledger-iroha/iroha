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
    // `BlockHeader::new` commits the default confidential feature digest.
    let expected: &[u8] = &hex!(
        "080100000000000000010001000100010001000100010001000839300000000000000800000000000000005201500100010001000601040100000042014001ed011301e701db017c01fb01f0019201c1019a012601ef014a0103019d0109011c01b6016e010401ca0178015e01b801c301ed01a401b901a0012701c5015c0100"
    );
    assert_eq!(bytes.as_slice(), expected);
}
