//! Single original staking authority for generated and custom signed test networks.

use super::*;
use color_eyre::eyre::ensure;
use iroha_data_model::block::consensus::PublicLaneStakingPolicyError;

pub(super) fn original_policy<'a>(
    instructions: impl Iterator<Item = &'a InstructionBox>,
) -> Result<Option<PublicLaneStakingPolicy>> {
    let mut slots = instructions
        .filter_map(|instruction| instruction.as_any().downcast_ref::<SetParameter>())
        .filter_map(|set| match set.inner() {
            Parameter::Custom(custom)
                if custom.id() == &PublicLaneStakingPolicy::parameter_id() =>
            {
                Some(custom)
            }
            _ => None,
        });
    let Some(original) = slots.next() else {
        return Ok(None);
    };
    ensure!(
        slots.next().is_none(),
        "genesis must contain exactly one original public_lane_staking_v1 policy"
    );
    Ok(Some(PublicLaneStakingPolicy::from_custom_parameter(
        original,
    )?))
}

pub(super) fn resolve_policy<'a>(
    authored: Option<PublicLaneStakingPolicy>,
    custom: Option<&GenesisBlock>,
    overrides: impl Iterator<Item = &'a InstructionBox>,
    asset: &AssetDefinitionId,
    custody: &AccountId,
) -> Result<PublicLaneStakingPolicy> {
    ensure!(
        original_policy(overrides)?.is_none(),
        "staking authority must be authored once with with_genesis_staking_policy, never as a parameter override"
    );
    let policy = if let Some(custom) = custom {
        ensure!(
            authored.is_none(),
            "custom signed genesis owns its original staking policy"
        );
        original_policy(
            custom
                .0
                .external_transactions()
                .filter_map(|transaction| match transaction.instructions() {
                    Executable::Instructions(instructions) => Some(instructions.iter()),
                    _ => None,
                })
                .flatten(),
        )?
        .ok_or(PublicLaneStakingPolicyError::MissingParameter)?
    } else if let Some(policy) = authored {
        policy
    } else {
        // The generated fixture profile is an explicit genesis author. This branch
        // is never reached for custom signed or retained original genesis.
        iroha_genesis::default_staking_policy(asset.clone(), custody.clone(), custody.clone())
    };
    policy.validate()?;
    Ok(policy)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn policy() -> PublicLaneStakingPolicy {
        let mut policy = config::default_test_network_staking_policy();
        policy.min_validator_stake = 37_u64.into();
        policy.max_stake_shares_per_validator = 13_u32.try_into().unwrap();
        policy.max_pending_unbonds_per_share = 9_u32.try_into().unwrap();
        policy.unbonding_delay_ms = 987_654;
        policy.max_slash_bps = 997;
        policy.stake_escrow_account_id = AccountId::new(
            checked_key_pair_from_seed(vec![0x72; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        policy.slash_sink_account_id = AccountId::new(
            checked_key_pair_from_seed(vec![0x73; 32], Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        policy
    }

    #[test]
    fn original_policy_requires_one_slot_and_retains_typed_failure() {
        let expected = policy();
        let instruction: InstructionBox = SetParameter::new(Parameter::Custom(
            expected.clone().into_custom_parameter().unwrap(),
        ))
        .into();
        assert_eq!(original_policy(std::iter::empty()).unwrap(), None);
        assert_eq!(
            original_policy(std::iter::once(&instruction)).unwrap(),
            Some(expected.clone())
        );
        assert!(
            original_policy([&instruction, &instruction].into_iter())
                .unwrap_err()
                .to_string()
                .contains("exactly one original")
        );
        let mut invalid = expected;
        invalid.max_slash_bps = 10_001;
        let invalid: InstructionBox = SetParameter::new(Parameter::Custom(CustomParameter::new(
            PublicLaneStakingPolicy::parameter_id(),
            Json::new(invalid),
        )))
        .into();
        assert!(matches!(
            original_policy(std::iter::once(&invalid))
                .unwrap_err()
                .downcast_ref::<PublicLaneStakingPolicyError>(),
            Some(PublicLaneStakingPolicyError::InvalidSlashFraction)
        ));
    }

    #[test]
    fn authored_policy_is_complete_and_parameter_overrides_are_rejected() {
        let expected = policy();
        let selected = resolve_policy(
            Some(expected.clone()),
            None,
            std::iter::empty(),
            &expected.stake_asset_id,
            &ALICE_ID,
        )
        .unwrap();
        assert_eq!(
            selected, expected,
            "neither default custody nor default scalar terms may replace caller policy"
        );
        let instruction: InstructionBox = SetParameter::new(Parameter::Custom(
            expected.clone().into_custom_parameter().unwrap(),
        ))
        .into();
        assert!(
            resolve_policy(
                None,
                None,
                std::iter::once(&instruction),
                &expected.stake_asset_id,
                &ALICE_ID
            )
            .unwrap_err()
            .to_string()
            .contains("never as a parameter override")
        );
        let mut invalid = expected.clone();
        invalid.max_slash_bps = 10_001;
        assert!(matches!(
            resolve_policy(
                Some(invalid),
                None,
                std::iter::empty(),
                &expected.stake_asset_id,
                &ALICE_ID
            )
            .unwrap_err()
            .downcast_ref::<PublicLaneStakingPolicyError>(),
            Some(PublicLaneStakingPolicyError::InvalidSlashFraction)
        ));
        let default = resolve_policy(
            None,
            None,
            std::iter::empty(),
            &expected.stake_asset_id,
            &BOB_ID,
        )
        .unwrap();
        assert_eq!(
            default,
            iroha_genesis::default_staking_policy(
                expected.stake_asset_id,
                BOB_ID.clone(),
                BOB_ID.clone()
            )
        );
    }

    #[test]
    fn requested_bootstrap_stake_is_not_increased_to_repair_an_original_minimum() {
        let policy = policy();
        let npos = SumeragiNposParameters {
            min_self_bond: 23_u64.into(),
            ..SumeragiNposParameters::default()
        };
        assert_eq!(
            resolve_npos_bootstrap_stake(&npos, &policy, 41_u64.into()),
            Quantity::from(41_u64)
        );
        for requested in [22_u64, 36] {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    resolve_npos_bootstrap_stake(&npos, &policy, requested.into())
                }))
                .is_err()
            );
        }
    }

    #[test]
    fn generated_preview_and_final_genesis_retain_all_policy_terms_and_plans() {
        let expected = policy();
        let network = crate::tests::build_with_isolated_permit(
            NetworkBuilder::new()
                .with_peers(4)
                .with_genesis_staking_policy(expected.clone())
                .with_npos_genesis_bootstrap(1_000_u64.into()),
        );
        let genesis = network.genesis();
        assert_eq!(network.generated_staking_policy.as_ref(), Some(&expected));
        assert_eq!(
            original_policy(
                genesis
                    .0
                    .external_transactions()
                    .filter_map(|tx| match tx.instructions() {
                        Executable::Instructions(instructions) => Some(instructions.iter()),
                        _ => None,
                    })
                    .flatten()
            )
            .unwrap(),
            Some(expected.clone())
        );
        let raw = network
            .generated_genesis_manifest
            .get()
            .expect("final original manifest");
        assert_eq!(
            PublicLaneStakingPolicy::from_parameters(&raw.effective_parameters().unwrap()).unwrap(),
            expected
        );
        assert!(
            genesis
                .0
                .output_results()
                .all(|result| result.as_ref().is_ok())
        );
        let mut registrations = 0;
        for instruction in raw.instructions() {
            if let Some(register) = instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
            {
                registrations += 1;
                assert_eq!(register.initial_stake, Quantity::from(1_000_u64));
                assert_eq!(
                    register.monetary_plan,
                    iroha_data_model::nexus::PublicLaneMonetaryPlanV1::genesis_registration(
                        AssetId::new(
                            expected.stake_asset_id.clone(),
                            register.stake_account.clone()
                        ),
                        AssetId::new(
                            expected.stake_asset_id.clone(),
                            expected.stake_escrow_account_id.clone()
                        ),
                        register.initial_stake.clone()
                    )
                );
            }
        }
        assert_eq!(registrations, 4);
        let second = network.genesis();
        assert_eq!(genesis.0.hash(), second.0.hash());
    }

    #[test]
    fn custom_signed_genesis_retains_original_policy_and_refuses_authoring_overrides() {
        let expected = policy();
        let original = expected.clone();
        let network = crate::tests::build_with_isolated_permit(
            NetworkBuilder::new()
                .with_peers(4)
                .with_genesis_block(move |topology, entries| {
                    config::genesis_unexecuted_with_keypair_and_post_topology(
                        Vec::new(),
                        Vec::new(),
                        topology,
                        entries,
                        SAMPLE_GENESIS_ACCOUNT_KEYPAIR.clone(),
                        original.clone(),
                    )
                }),
        );
        assert!(network.generated_staking_policy.is_none());
        let genesis = network.genesis();
        let bytes = genesis.0.encode_wire().unwrap();
        assert_eq!(
            resolve_policy(
                None,
                Some(&genesis),
                std::iter::empty(),
                &expected.stake_asset_id,
                &ALICE_ID
            )
            .unwrap(),
            expected
        );
        assert!(
            resolve_policy(
                Some(expected.clone()),
                Some(&genesis),
                std::iter::empty(),
                &expected.stake_asset_id,
                &ALICE_ID
            )
            .unwrap_err()
            .to_string()
            .contains("custom signed genesis owns")
        );
        assert_eq!(
            genesis.0.encode_wire().unwrap(),
            bytes,
            "reading original authority cannot normalize or resign it"
        );
        assert!(
            genesis
                .0
                .output_results()
                .all(|result| result.as_ref().is_ok())
        );
    }

    #[test]
    fn policy_builder_rejects_second_author_and_both_custom_orderings() {
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| NetworkBuilder::new()
                .with_genesis_staking_policy(policy())
                .with_genesis_staking_policy(policy())))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| NetworkBuilder::new()
                .with_genesis_staking_policy(policy())
                .with_genesis_block(|_, _| unreachable!("refuse before callback"))))
            .is_err()
        );
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| NetworkBuilder::new()
                .with_genesis_block(|_, _| unreachable!("refuse before callback"))
                .with_genesis_staking_policy(policy())))
            .is_err()
        );
    }
}
