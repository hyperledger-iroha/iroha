//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayBoundaryV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayBoundaryV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpTonAccountV1>(
        "iroha_data_model::bridge::sccp_replay::SccpTonAccountV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayActorV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayActorV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayPrincipalV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayPrincipalV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayDomainV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayDomainV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayAccumulatorIdV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayAccumulatorIdV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayRecordV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayRecordV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpSparseMerkleWitnessV1>(
        "iroha_data_model::bridge::sccp_replay::SccpSparseMerkleWitnessV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayForestV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayForestV1",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::SccpReplayDeltaV1>(
        "iroha_data_model::bridge::sccp_replay::SccpReplayDeltaV1",
    );
}
