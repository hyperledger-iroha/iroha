//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::RamLfeProgramId>(
        "iroha_data_model::ram_lfe::RamLfeProgramId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RamLfeProgramPolicy>(
        "iroha_data_model::ram_lfe::RamLfeProgramPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RamLfeExecutionReceiptPayload>(
        "iroha_data_model::ram_lfe::RamLfeExecutionReceiptPayload",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RamLfeOutputOpeningPayload>(
        "iroha_data_model::ram_lfe::RamLfeOutputOpeningPayload",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RamLfeOutputOpening>(
        "iroha_data_model::ram_lfe::RamLfeOutputOpening",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RamLfeReceiptAttestation>(
        "iroha_data_model::ram_lfe::RamLfeReceiptAttestation",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::RamLfeExecutionReceipt>(
        "iroha_data_model::ram_lfe::RamLfeExecutionReceipt",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
