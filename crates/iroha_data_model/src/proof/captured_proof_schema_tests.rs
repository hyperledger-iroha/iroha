//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::serialize::<super::ProofBox>(
        "iroha_data_model::proof::ProofBox",
    ),
    crate::captured_schema_tests::Case::serialize::<super::VerifyingKeyBox>(
        "iroha_data_model::proof::VerifyingKeyBox",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::VerifyingKeyId>(
        "iroha_data_model::proof::VerifyingKeyId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::VerifyingKeyRecord>(
        "iroha_data_model::proof::VerifyingKeyRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofId>(
        "iroha_data_model::proof::ProofId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofStatus>(
        "iroha_data_model::proof::ProofStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofRecord>(
        "iroha_data_model::proof::ProofRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ProofedCommittedTransaction>(
        "iroha_data_model::proof::ProofedCommittedTransaction",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
