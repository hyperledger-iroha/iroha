//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::BlockReceiptProof>(
        "iroha_data_model::block::proofs::BlockReceiptProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ExecutionReceiptProof>(
        "iroha_data_model::block::proofs::ExecutionReceiptProof",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BlockProofs>(
        "iroha_data_model::block::proofs::BlockProofs",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
