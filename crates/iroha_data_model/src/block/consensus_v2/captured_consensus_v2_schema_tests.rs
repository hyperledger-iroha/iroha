//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::ValidatorPower>(
        "iroha_data_model::block::consensus_v2::ValidatorPower",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DualQuorum>(
        "iroha_data_model::block::consensus_v2::DualQuorum",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::DataAvailabilityLayout>(
        "iroha_data_model::block::consensus_v2::DataAvailabilityLayout",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PayloadEncoding>(
        "iroha_data_model::block::consensus_v2::PayloadEncoding",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SumeragiV2GenesisContextParameters>(
        "iroha_data_model::block::consensus_v2::SumeragiV2GenesisContextParameters",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SnapshotBootstrapAnchor>(
        "iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::SnapshotV2BootstrapRecord>(
        "iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::HeightContext>(
        "iroha_data_model::block::consensus_v2::HeightContext",
    ),
    crate::captured_schema_tests::Case::serialize::<super::HeightContextIdentity>(
        "iroha_data_model::block::consensus_v2::HeightContextIdentity",
    ),
    crate::captured_schema_tests::Case::serialize::<super::ParentCommitIdentity>(
        "iroha_data_model::block::consensus_v2::ParentCommitIdentity",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::HeightContextId>(
        "iroha_data_model::block::consensus_v2::HeightContextId",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ConsensusRound>(
        "iroha_data_model::block::consensus_v2::ConsensusRound",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::GlobalPhase>(
        "iroha_data_model::block::consensus_v2::GlobalPhase",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::BlockSubject>(
        "iroha_data_model::block::consensus_v2::BlockSubject",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::ExecutionCommitment>(
        "iroha_data_model::block::consensus_v2::ExecutionCommitment",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::Vote>(
        "iroha_data_model::block::consensus_v2::Vote",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::VoteSignaturePayload>(
        "iroha_data_model::block::consensus_v2::VoteSignaturePayload",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::QuorumCertificateRef>(
        "iroha_data_model::block::consensus_v2::QuorumCertificateRef",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::QuorumCertificate>(
        "iroha_data_model::block::consensus_v2::QuorumCertificate",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}

crate::captured_schema_tests::native_capture::owner_printer!(CASES);
