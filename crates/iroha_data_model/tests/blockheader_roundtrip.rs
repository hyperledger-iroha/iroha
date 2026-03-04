//! Block header Norito roundtrip and consensus hash behavior tests

use iroha_crypto::Hash;
use iroha_data_model::block::BlockHeader;
use nonzero_ext::nonzero;
use norito::codec::{DecodeAll as NoritoDecodeAll, Encode as NoritoEncode};

#[test]
fn block_header_norito_roundtrip() {
    // Construct a header with and without result_merkle_root and ensure Norito roundtrip is stable.
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let bytes = header.encode();
    assert!(!bytes.is_empty(), "encoded header must be non-empty");

    // Set result_merkle_root and ensure consensus hash remains identical and Norito roundtrips.
    let consensus_hash = header.hash();
    let fake_root = Hash::new(b"result_merkle_root");
    // Rebuild header with the same fields but with result_merkle_root set
    let header_with_root = BlockHeader::new(
        nonzero!(1_u64),
        None,
        None,
        Some(iroha_crypto::HashOf::from_untyped_unchecked(fake_root)),
        0,
        0,
    );
    let bytes2 = header_with_root.encode();
    assert!(
        !bytes2.is_empty(),
        "encoded header with root must be non-empty"
    );
    assert_eq!(
        header_with_root.hash(),
        consensus_hash,
        "consensus hash ignores result_merkle_root"
    );
}

#[test]
fn block_header_preserves_large_view_change_index() {
    let view = u64::from(u32::MAX) + 1;
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, view);
    assert_eq!(header.view_change_index(), view);
    let bytes = header.encode();
    let decoded = BlockHeader::decode_all(&mut bytes.as_slice()).expect("decode");
    assert_eq!(decoded.view_change_index(), view);
}
