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
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 12345, 0);
    let bytes = norito::to_bytes(&header).expect("encode");
    let decoded: BlockHeader = norito::decode_from_bytes(&bytes).expect("decode");
    assert_eq!(decoded, header);
}
#[test]
fn block_header_golden_bytes() {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 12345, 0);
    let bytes = header.encode();
    // `BlockHeader::new` commits the default confidential feature digest.
    let expected: &[u8] = &hex!(
        "080100000000000000010001000100010001000100010008393000000000000008000000000000000052015001000100010006010401000000420140019301760191013401d001a3014d014c0193017a019501bb01c3014001050177011b019d018201ef010f01cf01df01f00169015701f2010701e201160189016f0100"
    );
    assert_eq!(bytes.as_slice(), expected);
}
