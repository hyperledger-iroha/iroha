//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeProgramId>(
        "iroha_data_model::ram_lfe::RamLfeProgramId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeProgramPolicy>(
        "iroha_data_model::ram_lfe::RamLfeProgramPolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeExecutionReceiptPayload>(
        "iroha_data_model::ram_lfe::RamLfeExecutionReceiptPayload",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeOutputOpeningPayload>(
        "iroha_data_model::ram_lfe::RamLfeOutputOpeningPayload",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeOutputOpening>(
        "iroha_data_model::ram_lfe::RamLfeOutputOpening",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeReceiptAttestation>(
        "iroha_data_model::ram_lfe::RamLfeReceiptAttestation",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::RamLfeExecutionReceipt>(
        "iroha_data_model::ram_lfe::RamLfeExecutionReceipt",
    );
}
