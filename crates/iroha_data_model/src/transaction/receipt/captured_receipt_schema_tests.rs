//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::TransactionSubmissionReceiptPayload>(
        "iroha_data_model::transaction::receipt::TransactionSubmissionReceiptPayload",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::TransactionSubmissionReceipt>(
        "iroha_data_model::transaction::receipt::TransactionSubmissionReceipt",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
