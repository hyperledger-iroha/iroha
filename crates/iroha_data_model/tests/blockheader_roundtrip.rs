//! Block header Norito roundtrip and consensus hash behavior tests
use iroha_data_model::block::BlockHeader;
use nonzero_ext::nonzero;
use norito::codec::{DecodeAll as NoritoDecodeAll, Encode as NoritoEncode};
#[test]
fn block_header_norito_roundtrip() {
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, 0);
    let bytes = header.encode();
    assert!(!bytes.is_empty());
    let decoded = BlockHeader::decode_all(&mut bytes.as_slice()).unwrap();
    assert_eq!(decoded, header);
    assert_eq!(decoded.hash(), header.hash());
    let json = norito::json::to_value(&header).unwrap();
    assert!(!json.as_object().unwrap().contains_key("result_merkle_root"));
}
#[test]
fn block_header_preserves_large_view_change_index() {
    let view = u64::from(u32::MAX) + 1;
    let header = BlockHeader::new(nonzero!(1_u64), None, None, 0, view);
    assert_eq!(header.view_change_index(), view);
    let bytes = header.encode();
    let decoded = BlockHeader::decode_all(&mut bytes.as_slice()).expect("decode");
    assert_eq!(decoded.view_change_index(), view);
}
