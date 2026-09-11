//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::DaProofScheme>(
        "iroha_data_model::da::commitment::DaProofScheme",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaProofPolicy>(
        "iroha_data_model::da::commitment::DaProofPolicy",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaProofPolicyBundle>(
        "iroha_data_model::da::commitment::DaProofPolicyBundle",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaCommitmentRecord>(
        "iroha_data_model::da::commitment::DaCommitmentRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaCommitmentBundle>(
        "iroha_data_model::da::commitment::DaCommitmentBundle",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaCommitmentKey>(
        "iroha_data_model::da::commitment::DaCommitmentKey",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaCommitmentLocation>(
        "iroha_data_model::da::commitment::DaCommitmentLocation",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaCommitmentWithLocation>(
        "iroha_data_model::da::commitment::DaCommitmentWithLocation",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::MerkleDirection>(
        "iroha_data_model::da::commitment::MerkleDirection",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::MerklePathItem>(
        "iroha_data_model::da::commitment::MerklePathItem",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DaCommitmentProof>(
        "iroha_data_model::da::commitment::DaCommitmentProof",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
