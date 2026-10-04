// Signed exact fee claims and additive custody regressions on committed network XOR.

pub(super) fn network_xor_claim_fixture(
    stx: &mut StateTransaction<'_, '_>,
    policy: ValidationFeePolicyV1,
) -> (ValidationFeePolicyV1, ValidationFeeTreasuryPayoutBindingV1) {
    let binding = active_bindings(stx).unwrap().remove(0);
    crate::state::validate_network_xor_asset(&stx.world, &binding.xor_asset_id)
        .expect("shared fee fixture uses the committed network XOR definition");
    (policy, binding)
}

pub(super) fn claim_current_fee_credit(
    stx: &mut StateTransaction<'_, '_>,
    account: &AccountId,
    lane: LaneId,
) -> Result<(), Error> {
    let plan = fee_reward_claim_plan(&stx.world, stx.block_height(), account, lane)
        .map_err(|error| stx.world.attempt_error_to_instruction_error(error))?;
    if let Some(prepared) = prepare_fee_reward_claim(stx, account, lane, plan.as_ref())? {
        claim_fee_rewards(stx, prepared)?;
    }
    Ok(())
}

fn seed_claim_credit(
    stx: &mut StateTransaction<'_, '_>,
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    claimant: &AccountId,
    amount: u128,
    balance: u128,
) {
    use iroha_data_model::IntoKeyValue;
    let original = beneficiary::ensure(stx, binding, claimant).unwrap();
    write(stx, claimable_key(binding, &original).unwrap(), &amount).unwrap();
    save_state(
        stx,
        binding,
        &ValidationFeeRewardsState {
            reserved_xor: amount,
            ..Default::default()
        },
    )
    .unwrap();
    let asset = AssetId::new(
        binding.xor_asset_id.clone(),
        binding.reward_pool_account_id.clone(),
    );
    let (_, value) = Asset::new(asset.clone(), quantity(balance, 9).unwrap()).into_key_value();
    stx.world.assets.insert(asset, value);
}

fn signed_claim_instruction(
    stx: &StateTransaction<'_, '_>,
    claimant: &AccountId,
    lane: LaneId,
    fee_claim: Option<PublicLaneFeeRewardClaimV1>,
) -> iroha_data_model::isi::staking::ClaimPublicLaneRewards {
    iroha_data_model::isi::staking::ClaimPublicLaneRewards {
        lane_id: lane,
        account: claimant.clone(),
        claim_plan: iroha_data_model::nexus::PublicLaneRewardClaimPlanV1 {
            network_scope: iroha_data_model::nexus::PublicLaneMonetaryScopeV1::Network(
                stx.network_id,
            ),
            valid_until_height: stx.block_height(),
            expected_state: None,
            records: vec![],
            sources: vec![],
            fee_claim,
        },
    }
}

#[test]
fn canonical_network_xor_is_required_before_fee_reward_state_changes() {
    use iroha_data_model::{
        Registrable,
        asset::{AssetBalancePolicy, AssetDefinition},
    };
    for variant in 0..4 {
        crate::retail_fee_tests::fixture(1_793_451_600_000, |stx, policy| {
            let (mut policy, mut binding) = network_xor_claim_fixture(stx, policy);
            if variant == 1 {
                binding.xor_asset_id = AssetDefinitionId::derive_from_components(
                    DomainId::try_new("fees", "paynet").unwrap(),
                    "substitute_xor".parse().unwrap(),
                );
            }
            let definition = AssetDefinition::new(
                binding.xor_asset_id.clone(),
                "XOR".to_owned(),
                iroha_primitives::numeric::NumericSpec::fractional(if variant == 2 {
                    2
                } else {
                    9
                }),
                if variant == 3 {
                    AssetBalancePolicy::DataspaceRestricted
                } else {
                    AssetBalancePolicy::Global
                },
                None,
            )
            .build(&account(2));
            stx.world
                .asset_definitions
                .insert(binding.xor_asset_id.clone(), definition);
            policy.reward_custody = binding.custody();
            let registry =
                crate::validation_fee::tests::policy_registry(&[policy], &[binding.clone()]);
            crate::validation_fee::tests::install_policy_registry_fixture(&registry, stx);
            let period = earning_month(stx.block_unix_timestamp_ms() - 1).unwrap();
            save_state(
                stx,
                &binding,
                &ValidationFeeRewardsState {
                    pending_sbd_total: 100,
                    ..Default::default()
                },
            )
            .unwrap();
            write(stx, pending_key(&binding, period).unwrap(), &100_u64).unwrap();
            write(
                stx,
                service_key(&binding, period).unwrap(),
                &BTreeMap::from([(account(2), 1_u64)]),
            )
            .unwrap();
            let before = stx
                .world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>();
            let offer = ConversionOffer {
                earning_period_start_ms: period,
                sbd_minor: 100,
                min_xor_minor: 100,
                sequence: 0,
            };
            let result = reserve_conversion(stx, &binding, &offer, 100);
            if variant == 0 {
                result.expect("canonical network XOR may reserve authenticated conversion output");
                assert_eq!(read_state(stx, &binding).unwrap().reserved_xor, 100);
                assert_eq!(read_state(stx, &binding).unwrap().next_allocation, 1);
                crate::validation_fee::active_policy(stx).unwrap().unwrap();
            } else {
                let error = result.expect_err(
                    "substitute identity, precision and scope cannot fund validator rewards",
                );
                assert!(error.to_string().contains("network XOR"), "{error}");
                assert_eq!(
                    stx.world
                        .smart_contract_state
                        .iter()
                        .map(|(key, value)| (key.clone(), value.clone()))
                        .collect::<Vec<_>>(),
                    before
                );
                assert!(conversion_offer(stx, &binding).is_err());
                assert!(crate::validation_fee::active_policy(stx).is_err());
                assert!(
                    crate::validation_fee::validate_persisted_policy_registry_governance_v1(
                        &stx.world
                    )
                    .is_err()
                );
            }
        });
    }
}

#[test]
fn signed_fee_reward_claim_rejects_every_changed_binding_before_mutation() {
    crate::retail_fee_tests::fixture(1_793_451_600_000, |stx, policy| {
        let (_, binding) = network_xor_claim_fixture(stx, policy);
        let claimant = account(2);
        seed_claim_credit(stx, &binding, &claimant, 100, 200);
        let plan = fee_reward_claim_plan(
            &stx.world,
            stx.block_height(),
            &claimant,
            binding.validator_lane_id,
        )
        .unwrap()
        .unwrap();
        let before = stx
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let pool_before = stx.world.assets.get(&plan.source_asset).cloned();
        for change in 0..8 {
            let mut changed = plan.clone();
            match change {
                0 => changed.lifecycle_seal[0] ^= 1,
                1 => changed.beneficiary_id = account(3),
                2 => changed.beneficiary_revision += 1,
                3 => changed.source_asset = AssetId::new(binding.xor_asset_id.clone(), account(3)),
                4 => {
                    changed.destination_asset =
                        AssetId::new(binding.xor_asset_id.clone(), account(3))
                }
                5 => changed.amount = quantity(99, 9).unwrap(),
                6 => changed.expected_claim_sequence += 1,
                _ => {
                    changed.source_asset = AssetId::with_scope(
                        binding.xor_asset_id.clone(),
                        binding.reward_pool_account_id.clone(),
                        iroha_data_model::asset::AssetBalanceScope::Dataspace(DataSpaceId::new(1)),
                    )
                }
            }
            let instruction =
                signed_claim_instruction(stx, &claimant, binding.validator_lane_id, Some(changed));
            let error = instruction
                .execute(&claimant, stx)
                .expect_err("every fee claim coordinate is signed");
            assert!(
                error
                    .to_string()
                    .contains("exact current signed monetary plan"),
                "{error}"
            );
            assert_eq!(
                stx.world
                    .smart_contract_state
                    .iter()
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect::<Vec<_>>(),
                before
            );
            assert_eq!(
                stx.world.assets.get(&plan.source_asset),
                pool_before.as_ref()
            );
            assert!(
                stx.world
                    .public_lane_reward_claims
                    .get(&(binding.validator_lane_id, claimant.clone()))
                    .is_none()
            );
        }
        let instruction = signed_claim_instruction(
            stx,
            &claimant,
            binding.validator_lane_id,
            Some(plan.clone()),
        );
        instruction.clone().execute(&claimant, stx).unwrap();
        assert_eq!(read_state(stx, &binding).unwrap().reserved_xor, 0);
        assert_eq!(
            stx.world
                .assets
                .get(&plan.destination_asset)
                .unwrap()
                .as_ref(),
            &quantity(100, 9).unwrap()
        );
        assert!(
            instruction
                .execute(&claimant, stx)
                .unwrap_err()
                .to_string()
                .contains("no eligible reserved credit")
        );
    });
}

#[test]
fn self_custody_fee_claim_releases_only_its_exact_reserve_without_debiting_balance() {
    crate::retail_fee_tests::fixture(1_793_451_600_000, |stx, policy| {
        let (_, binding) = network_xor_claim_fixture(stx, policy);
        let claimant = binding.reward_pool_account_id.clone();
        seed_claim_credit(stx, &binding, &claimant, 100, 250);
        let source = AssetId::new(binding.xor_asset_id.clone(), claimant.clone());
        stx.world
            .public_lane_stake_reserves
            .insert(source.clone(), quantity(80, 9).unwrap());
        stx.world
            .public_lane_reward_reserves
            .insert(source.clone(), quantity(70, 9).unwrap());
        let plan = fee_reward_claim_plan(
            &stx.world,
            stx.block_height(),
            &claimant,
            binding.validator_lane_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(plan.source_asset, plan.destination_asset);
        let instruction =
            signed_claim_instruction(stx, &claimant, binding.validator_lane_id, Some(plan));
        instruction.clone().execute(&claimant, stx).unwrap();
        assert_eq!(
            stx.world.assets.get(&source).unwrap().as_ref(),
            &quantity(250, 9).unwrap(),
            "self-custody claims release a liability without changing the balance"
        );
        assert_eq!(read_state(stx, &binding).unwrap().reserved_xor, 0);
        assert_eq!(read_state(stx, &binding).unwrap().next_claim, 1);
        assert_eq!(
            stx.world.public_lane_stake_reserves.get(&source),
            Some(&quantity(80, 9).unwrap())
        );
        assert_eq!(
            stx.world.public_lane_reward_reserves.get(&source),
            Some(&quantity(70, 9).unwrap())
        );
        assert!(
            read::<u128>(stx, &claimable_key(&binding, &claimant).unwrap())
                .unwrap()
                .is_none()
        );
        instruction
            .execute(&claimant, stx)
            .expect_err("released credit cannot be claimed again");
    });
}

#[test]
fn absent_fee_claim_leaves_even_unreadable_credit_and_dust_untouched() {
    crate::retail_fee_tests::fixture(1_793_451_600_000, |stx, policy| {
        let (_, binding) = network_xor_claim_fixture(stx, policy);
        let claimant = account(2);
        seed_claim_credit(stx, &binding, &claimant, 100, 200);
        let key = claimable_key(&binding, &claimant).unwrap();
        stx.world.smart_contract_state.insert(key, vec![0xff]);
        let before = stx
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        signed_claim_instruction(stx, &claimant, binding.validator_lane_id, None)
            .execute(&claimant, stx)
            .unwrap();
        assert_eq!(
            stx.world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>(),
            before
        );
    });
}

#[test]
fn signed_fee_reward_claim_rolls_back_public_payout_when_fee_transfer_is_refused() {
    use iroha_data_model::{IntoKeyValue, nexus::PublicLaneRewardClaimSourceV1};
    crate::retail_fee_tests::fixture_block(1_793_451_600_000, |block, policy| {
        let claimant = account(2);
        let (binding, mut instruction, source, before) = {
            let mut stx = block.transaction();
            let (_, binding) = network_xor_claim_fixture(&mut stx, policy);
            seed_claim_credit(&mut stx, &binding, &claimant, 100, 200);
            let fee = fee_reward_claim_plan(
                &stx.world,
                stx.block_height(),
                &claimant,
                binding.validator_lane_id,
            )
            .unwrap()
            .unwrap();
            let mut instruction =
                signed_claim_instruction(&stx, &claimant, binding.validator_lane_id, Some(fee));
            let source = AssetId::new(binding.xor_asset_id.clone(), account(3));
            let (_, value) = Asset::new(source.clone(), quantity(50, 9).unwrap()).into_key_value();
            stx.world.assets.insert(source.clone(), value);
            stx.world.public_lane_reward_accruals.insert(
                (binding.validator_lane_id, claimant.clone(), source.clone()),
                quantity(50, 9).unwrap(),
            );
            stx.world
                .public_lane_reward_reserves
                .insert(source.clone(), quantity(50, 9).unwrap());
            instruction
                .claim_plan
                .sources
                .push(PublicLaneRewardClaimSourceV1 {
                    source_asset: source.clone(),
                    destination_asset: AssetId::new(binding.xor_asset_id.clone(), claimant.clone()),
                    expected_accrued: Some(quantity(50, 9).unwrap()),
                    payout: quantity(50, 9).unwrap(),
                });
            // Put a valid blacklist transfer control on the fee pool. Public reward
            // execution completes first; the exact fee transfer then fails.
            use iroha_data_model::asset::transfer_control::{
                ASSET_TRANSFER_CONTROL_METADATA_KEY, AssetTransferControlRecord,
                AssetTransferControlStoreV1,
            };
            let mut control = AssetTransferControlRecord::new(binding.xor_asset_id.clone());
            control.blacklisted = true;
            let mut store = AssetTransferControlStoreV1::default();
            store.upsert(control);
            stx.world
                .accounts
                .get_mut(&binding.reward_pool_account_id)
                .unwrap()
                .metadata_mut()
                .insert(
                    ASSET_TRANSFER_CONTROL_METADATA_KEY.parse().unwrap(),
                    iroha_primitives::json::Json::new(store),
                );
            let before = stx
                .world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>();
            stx.apply();
            (binding, instruction, source, before)
        };
        {
            let mut stx = block.transaction_for_fastpq_testing(Hash::new(b"exact fee rollback"));
            stx.nexus.staking.reward_dust_threshold = Quantity::zero();
            assert!(instruction.clone().execute(&claimant, &mut stx).is_err());
            assert!(
                stx.world
                    .public_lane_reward_accruals
                    .get(&(binding.validator_lane_id, claimant.clone(), source.clone()))
                    .is_none(),
                "public payout must have executed before the fee refusal"
            );
            // Drop the complete transaction; there is no manual ledger repair.
        }
        let mut stx = block.transaction_for_fastpq_testing(Hash::new(b"explicitly skip fee"));
        stx.nexus.staking.reward_dust_threshold = Quantity::zero();
        assert_eq!(
            stx.world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>(),
            before
        );
        assert_eq!(
            stx.world.public_lane_reward_reserves.get(&source),
            Some(&quantity(50, 9).unwrap())
        );
        assert_eq!(
            stx.world.assets.get(&source).unwrap().as_ref(),
            &quantity(50, 9).unwrap()
        );
        assert!(
            stx.world
                .assets
                .get(&AssetId::new(
                    binding.xor_asset_id.clone(),
                    claimant.clone()
                ))
                .is_none()
        );
        instruction.claim_plan.fee_claim = None;
        instruction.execute(&claimant, &mut stx).unwrap();
        assert_eq!(read_state(&stx, &binding).unwrap().reserved_xor, 100);
    });
}

#[test]
fn shared_fee_stake_reward_custody_is_additive() {
    use iroha_data_model::{
        IntoKeyValue,
        isi::staking::RecordPublicLaneRewards,
        nexus::{PublicLaneRewardRole, PublicLaneRewardShare},
    };
    crate::retail_fee_tests::fixture(1_793_451_600_000, |stx, policy| {
        let (_, binding) = network_xor_claim_fixture(stx, policy);
        let claimant = account(2);
        seed_claim_credit(stx, &binding, &claimant, 100, 300);
        let source = AssetId::new(
            binding.xor_asset_id.clone(),
            binding.reward_pool_account_id.clone(),
        );
        stx.world
            .public_lane_stake_reserves
            .insert(source.clone(), quantity(80, 9).unwrap());
        stx.world
            .public_lane_reward_reserves
            .insert(source.clone(), quantity(70, 9).unwrap());
        ensure_reward_custody_debit(stx, &source, &quantity(50, 9).unwrap()).unwrap();
        assert!(
            ensure_reward_custody_debit(stx, &source, &quantity(51, 9).unwrap()).is_err(),
            "distinct liabilities must add, not overlap"
        );
        let scoped = AssetId::with_scope(
            binding.xor_asset_id.clone(),
            binding.reward_pool_account_id.clone(),
            iroha_data_model::asset::AssetBalanceScope::Dataspace(DataSpaceId::new(9)),
        );
        assert_eq!(
            reserved_fee_custody(&stx.world, &scoped).unwrap(),
            Quantity::zero(),
            "global reservations cannot attach to another scope"
        );
        assert!(
            crate::smartcontracts::isi::staking::prepare_stake_custody_credit(
                &stx.world,
                binding.validator_lane_id,
                &claimant,
                &source,
                &quantity(51, 9).unwrap(),
                &quantity(300, 9).unwrap()
            )
            .is_err()
        );
        crate::smartcontracts::isi::staking::prepare_stake_custody_credit(
            &stx.world,
            binding.validator_lane_id,
            &claimant,
            &source,
            &quantity(50, 9).unwrap(),
            &quantity(300, 9).unwrap(),
        )
        .unwrap();
        stx.nexus.fees.fee_asset_id = binding.xor_asset_id.to_string();
        stx.nexus.fees.fee_sink_account_id = binding.reward_pool_account_id.to_string();
        let record = RecordPublicLaneRewards {
            lane_id: binding.validator_lane_id,
            epoch: 0,
            reward_asset: source.clone(),
            total_reward: quantity(51, 9).unwrap(),
            shares: vec![PublicLaneRewardShare {
                account: claimant.clone(),
                role: PublicLaneRewardRole::Nominator,
                amount: quantity(51, 9).unwrap(),
            }],
            metadata: iroha_model_base::metadata::Metadata::default(),
        };
        let error = record
            .execute(&binding.reward_pool_account_id, stx)
            .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("insufficient unreserved balance"),
            "{error}"
        );
        assert!(
            stx.world
                .public_lane_rewards
                .get(&(binding.validator_lane_id, 0))
                .is_none()
        );
        // Restoration and signed claims also reject a balance that backs each
        // ledger alone but cannot back their sum.
        let (_, underfunded) =
            Asset::new(source.clone(), quantity(200, 9).unwrap()).into_key_value();
        stx.world.assets.insert(source.clone(), underfunded);
        assert!(validate_fee_custody_backing(&stx.world).is_err());
        assert!(
            fee_reward_claim_plan(
                &stx.world,
                stx.block_height(),
                &claimant,
                binding.validator_lane_id
            )
            .is_err()
        );
        let (_, funded) = Asset::new(source.clone(), quantity(300, 9).unwrap()).into_key_value();
        stx.world.assets.insert(source.clone(), funded);
        validate_fee_custody_backing(&stx.world).unwrap();
        Transfer::asset_quantity(source.clone(), quantity(51, 9).unwrap(), account(3))
            .execute(&binding.reward_pool_account_id, stx)
            .expect_err("generic debit cannot consume the same funds twice");
        Transfer::asset_quantity(source.clone(), quantity(50, 9).unwrap(), account(3))
            .execute(&binding.reward_pool_account_id, stx)
            .unwrap();
        let fee = fee_reward_claim_plan(
            &stx.world,
            stx.block_height(),
            &claimant,
            binding.validator_lane_id,
        )
        .unwrap();
        signed_claim_instruction(stx, &claimant, binding.validator_lane_id, fee)
            .execute(&claimant, stx)
            .unwrap();
        assert_eq!(
            stx.world.assets.get(&source).unwrap().as_ref(),
            &quantity(150, 9).unwrap()
        );
        assert_eq!(
            stx.world.public_lane_stake_reserves.get(&source),
            Some(&quantity(80, 9).unwrap())
        );
        assert_eq!(
            stx.world.public_lane_reward_reserves.get(&source),
            Some(&quantity(70, 9).unwrap())
        );
    });
}

#[test]
fn fee_reward_claim_refuses_currency_substitution_and_stale_credit() {
    use iroha_data_model::parameter::{Parameter, system::SumeragiNposParameters};
    crate::retail_fee_tests::fixture(1_793_451_600_000, |stx, policy| {
        let (_, binding) = network_xor_claim_fixture(stx, policy);
        let claimant = account(2);
        seed_claim_credit(stx, &binding, &claimant, 100, 200);
        let plan = fee_reward_claim_plan(
            &stx.world,
            stx.block_height(),
            &claimant,
            binding.validator_lane_id,
        )
        .unwrap();
        let mut params = SumeragiNposParameters::default();
        params.xor_asset_definition_id = binding.ds_asset_id.clone();
        stx.world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(params.into_custom_parameter()));
        let before = stx
            .world
            .smart_contract_state
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect::<Vec<_>>();
        let currency_error =
            crate::state::validate_network_xor_asset(&stx.world, &binding.xor_asset_id)
                .expect_err("claim custody must still name the committed network XOR");
        assert!(
            currency_error.to_string().contains("committed network XOR"),
            "{currency_error}"
        );
        assert!(
            prepare_fee_reward_claim(stx, &claimant, binding.validator_lane_id, plan.as_ref())
                .is_err(),
            "the claim must propagate its authenticated registry refusal"
        );
        assert_eq!(
            stx.world
                .smart_contract_state
                .iter()
                .map(|(key, value)| (key.clone(), value.clone()))
                .collect::<Vec<_>>(),
            before,
            "rejected currency cannot alter the credit, receipt sequence or reserve"
        );
        stx.world
            .parameters
            .get_mut()
            .set_parameter(Parameter::Custom(
                SumeragiNposParameters::default().into_custom_parameter(),
            ));
        seed_claim_credit(stx, &binding, &claimant, 101, 200);
        assert!(
            prepare_fee_reward_claim(stx, &claimant, binding.validator_lane_id, plan.as_ref())
                .err()
                .unwrap()
                .to_string()
                .contains("exact current signed monetary plan")
        );
        beneficiary::rekey_beneficiary(stx, &claimant, &account(3)).unwrap();
        assert!(
            prepare_fee_reward_claim(stx, &claimant, binding.validator_lane_id, plan.as_ref())
                .is_err()
        );
        let recovered = fee_reward_claim_plan(
            &stx.world,
            stx.block_height(),
            &account(3),
            binding.validator_lane_id,
        )
        .unwrap()
        .unwrap();
        assert_eq!(recovered.beneficiary_id, claimant);
        assert_eq!(recovered.beneficiary_revision, 1);
        assert_eq!(recovered.destination_asset.account(), &account(3));
    });
}
