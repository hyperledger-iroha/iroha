//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::BlockReceiptProof>(
        "iroha_data_model::block::proofs::BlockReceiptProof",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ExecutionReceiptProof>(
        "iroha_data_model::block::proofs::ExecutionReceiptProof",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::BlockProofs>(
        "iroha_data_model::block::proofs::BlockProofs",
    );
}
