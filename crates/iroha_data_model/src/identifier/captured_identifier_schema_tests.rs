//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::IdentifierNormalization>(
        "iroha_data_model::identifier::IdentifierNormalization",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::IdentifierPolicyId>(
        "iroha_data_model::identifier::IdentifierPolicyId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::IdentifierPolicy>(
        "iroha_data_model::identifier::IdentifierPolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::IdentifierClaimRecord>(
        "iroha_data_model::identifier::IdentifierClaimRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::IdentifierResolutionReceipt>(
        "iroha_data_model::identifier::IdentifierResolutionReceipt",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::IdentifierResolutionReceiptPayload>(
        "iroha_data_model::identifier::IdentifierResolutionReceiptPayload",
    );
}
