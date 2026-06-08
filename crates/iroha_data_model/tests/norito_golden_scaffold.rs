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
        "08010000000000000001000100010001000100010001000100010008393000000000000008000000000000000052015001000100010006010401000000420140010701c80117019101880187014f011401100118016f0126010201aa01e101be01df0199013601b501d001ec01b2016001cb015201b80164013501c9018301190100"
    );
    assert_eq!(bytes.as_slice(), expected);
}
