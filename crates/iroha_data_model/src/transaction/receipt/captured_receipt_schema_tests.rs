//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::TransactionSubmissionReceiptPayload>(
        "iroha_data_model::transaction::receipt::TransactionSubmissionReceiptPayload",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::TransactionSubmissionReceipt>(
        "iroha_data_model::transaction::receipt::TransactionSubmissionReceipt",
    );
}
