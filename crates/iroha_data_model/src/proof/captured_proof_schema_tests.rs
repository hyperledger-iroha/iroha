//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_serialize::<super::ProofBox>(
        "iroha_data_model::proof::ProofBox",
    );
    crate::captured_schema_tests::assert_serialize::<super::VerifyingKeyBox>(
        "iroha_data_model::proof::VerifyingKeyBox",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::VerifyingKeyId>(
        "iroha_data_model::proof::VerifyingKeyId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::VerifyingKeyRecord>(
        "iroha_data_model::proof::VerifyingKeyRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ProofId>(
        "iroha_data_model::proof::ProofId",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ProofStatus>(
        "iroha_data_model::proof::ProofStatus",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ProofRecord>(
        "iroha_data_model::proof::ProofRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::ProofedCommittedTransaction>(
        "iroha_data_model::proof::ProofedCommittedTransaction",
    );
}
