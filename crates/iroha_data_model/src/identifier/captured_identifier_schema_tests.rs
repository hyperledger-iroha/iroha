//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::IdentifierNormalization>(
        "iroha_data_model::identifier::IdentifierNormalization",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::IdentifierPolicyId>(
        "iroha_data_model::identifier::IdentifierPolicyId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::IdentifierPolicy>(
        "iroha_data_model::identifier::IdentifierPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::IdentifierClaimRecord>(
        "iroha_data_model::identifier::IdentifierClaimRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::IdentifierResolutionReceipt>(
        "iroha_data_model::identifier::IdentifierResolutionReceipt",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::IdentifierResolutionReceiptPayload>(
        "iroha_data_model::identifier::IdentifierResolutionReceiptPayload",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
