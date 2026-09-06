//! Immutable compiler-captured identities for this source owner’s existing codecs.

#[test]
fn captured_codec_schema_identities() {
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLaneValidatorRecord>(
        "iroha_data_model::nexus::staking::PublicLaneValidatorRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLaneValidatorStatus>(
        "iroha_data_model::nexus::staking::PublicLaneValidatorStatus",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLaneStakeShare>(
        "iroha_data_model::nexus::staking::PublicLaneStakeShare",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLaneUnbonding>(
        "iroha_data_model::nexus::staking::PublicLaneUnbonding",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLaneRewardShare>(
        "iroha_data_model::nexus::staking::PublicLaneRewardShare",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLaneRewardRole>(
        "iroha_data_model::nexus::staking::PublicLaneRewardRole",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLaneRewardRecord>(
        "iroha_data_model::nexus::staking::PublicLaneRewardRecord",
    );
    crate::captured_schema_tests::assert_bidirectional::<super::PublicLanePendingReward>(
        "iroha_data_model::nexus::staking::PublicLanePendingReward",
    );
}
