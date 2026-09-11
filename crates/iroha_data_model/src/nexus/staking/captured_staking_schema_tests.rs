//! Immutable compiler-captured identities for this source owner’s existing codecs.

const CASES: &[crate::captured_schema_tests::Case] = &[
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLaneValidatorRecord>(
        "iroha_data_model::nexus::staking::PublicLaneValidatorRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLaneValidatorStatus>(
        "iroha_data_model::nexus::staking::PublicLaneValidatorStatus",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLaneStakeShare>(
        "iroha_data_model::nexus::staking::PublicLaneStakeShare",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLaneUnbonding>(
        "iroha_data_model::nexus::staking::PublicLaneUnbonding",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLaneRewardShare>(
        "iroha_data_model::nexus::staking::PublicLaneRewardShare",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLaneRewardRole>(
        "iroha_data_model::nexus::staking::PublicLaneRewardRole",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLaneRewardRecord>(
        "iroha_data_model::nexus::staking::PublicLaneRewardRecord",
    ),
    crate::captured_schema_tests::Case::bidirectional::<super::PublicLanePendingReward>(
        "iroha_data_model::nexus::staking::PublicLanePendingReward",
    ),
];

#[test]
fn captured_codec_schema_identities() {
    for case in CASES {
        case.check();
    }
}
