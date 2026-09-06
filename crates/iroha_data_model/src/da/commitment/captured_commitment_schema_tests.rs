//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::DaProofScheme>(
        "iroha_data_model::da::commitment::DaProofScheme",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaProofPolicy>(
        "iroha_data_model::da::commitment::DaProofPolicy",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaProofPolicyBundle>(
        "iroha_data_model::da::commitment::DaProofPolicyBundle",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaCommitmentRecord>(
        "iroha_data_model::da::commitment::DaCommitmentRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaCommitmentBundle>(
        "iroha_data_model::da::commitment::DaCommitmentBundle",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaCommitmentKey>(
        "iroha_data_model::da::commitment::DaCommitmentKey",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaCommitmentLocation>(
        "iroha_data_model::da::commitment::DaCommitmentLocation",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaCommitmentWithLocation>(
        "iroha_data_model::da::commitment::DaCommitmentWithLocation",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::MerkleDirection>(
        "iroha_data_model::da::commitment::MerkleDirection",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::MerklePathItem>(
        "iroha_data_model::da::commitment::MerklePathItem",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::DaCommitmentProof>(
        "iroha_data_model::da::commitment::DaCommitmentProof",
    );
}
