//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::LaneRelayEnvelope>(
        "iroha_data_model::nexus::relay::LaneRelayEnvelope",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneFinalityStatement>(
        "iroha_data_model::nexus::relay::LaneFinalityStatement",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneFinalityAuthorityV1>(
        "iroha_data_model::nexus::relay::LaneFinalityAuthorityV1",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneRelayFastpqMaterialStatus>(
        "iroha_data_model::nexus::relay::LaneRelayFastpqMaterialStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneRelayEnvelopeRef>(
        "iroha_data_model::nexus::relay::LaneRelayEnvelopeRef",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::VerifiedLaneRelayRecord>(
        "iroha_data_model::nexus::relay::VerifiedLaneRelayRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::VerifiedFeeSponsorVaultAllocation>(
        "iroha_data_model::nexus::relay::VerifiedFeeSponsorVaultAllocation",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneFastpqProofMaterial>(
        "iroha_data_model::nexus::relay::LaneFastpqProofMaterial",
    ),
    crate::captured_schema_tests::Case::serialize::<super::LaneRelayFastpqClaim>(
        "iroha_data_model::nexus::relay::LaneRelayFastpqClaim",
    ),
    crate::captured_schema_tests::Case::serialize::<super::LaneRelayMergeHint>(
        "iroha_data_model::nexus::relay::LaneRelayMergeHint",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::FeeSponsorVaultAllocationClaim>(
        "iroha_data_model::nexus::relay::FeeSponsorVaultAllocationClaim",
    ),
    crate::captured_schema_tests::Case::serialize::<super::FeeSponsorVaultSourceStateCommitment>(
        "iroha_data_model::nexus::relay::FeeSponsorVaultSourceStateCommitment",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneRelayEvidenceBundle>(
        "iroha_data_model::nexus::relay::LaneRelayEvidenceBundle",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::LaneRelayEmergencyValidatorSet>(
        "iroha_data_model::nexus::relay::LaneRelayEmergencyValidatorSet",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
