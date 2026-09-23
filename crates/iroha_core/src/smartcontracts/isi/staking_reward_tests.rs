// Same-scope tests for treasury authorization and conserved reward obligations.

fn reward_distribution(
    lane_id: LaneId,
    epoch: u64,
    asset: &AssetId,
    recipient: &AccountId,
    amount: u64,
) -> RecordPublicLaneRewards {
    RecordPublicLaneRewards {
        lane_id,
        epoch,
        reward_asset: asset.clone(),
        total_reward: Quantity::from(amount),
        shares: vec![PublicLaneRewardShare {
            account: recipient.clone(),
            role: PublicLaneRewardRole::Validator,
            amount: Quantity::from(amount),
        }],
        metadata: Metadata::default(),
    }
}

#[test]
fn reward_distribution_requires_the_exact_treasury_authority() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    let distribution = reward_distribution(lane, 0, &asset, &validator, 25);
    let error = distribution.clone().execute(&validator, &mut stx).unwrap_err();
    assert!(error.to_string().contains("authorized"));
    assert!(stx.world.public_lane_rewards.get(&(lane, 0)).is_none());
    distribution.execute(&sink, &mut stx).unwrap();
    assert_eq!(rewards::outstanding_rewards(&stx.world, &asset).unwrap(), Quantity::from(25_u64));
}

#[test]
fn reward_epoch_zero_is_claimable_once() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xB0);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, definition) = configure_reward_fixture(&mut stx, lane, 100);
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    reward_distribution(lane, 0, &asset, &validator, 25).execute(&sink, &mut stx).unwrap();
    let claim = ClaimPublicLaneRewards { lane_id: lane, account: validator.clone(), upto_epoch: Some(0) };
    claim.clone().execute(&validator, &mut stx).unwrap();
    claim.execute(&validator, &mut stx).unwrap();
    assert_eq!(stx.world.public_lane_reward_claims.get(&(lane, validator.clone(), asset.clone())), Some(&0));
    assert_eq!(stx.world.assets.get(&AssetId::new(definition, validator)).unwrap().as_ref(), &Quantity::from(25_u64));
    assert!(rewards::outstanding_rewards(&stx.world, &asset).unwrap().is_zero());
}

#[test]
fn reward_dust_accumulates_until_paid() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xB1);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, definition) = configure_reward_fixture(&mut stx, lane, 100);
    stx.nexus.staking.reward_dust_threshold = Quantity::from(50_u64);
    reward_distribution(lane, 0, &asset, &validator, 25).execute(&sink, &mut stx).unwrap();
    let claim = ClaimPublicLaneRewards { lane_id: lane, account: validator.clone(), upto_epoch: None };
    claim.clone().execute(&validator, &mut stx).unwrap();
    assert!(stx.world.public_lane_reward_claims.get(&(lane, validator.clone(), asset.clone())).is_none());
    assert_eq!(rewards::outstanding_rewards(&stx.world, &asset).unwrap(), Quantity::from(25_u64));
    reward_distribution(lane, 1, &asset, &validator, 25).execute(&sink, &mut stx).unwrap();
    claim.execute(&validator, &mut stx).unwrap();
    assert_eq!(stx.world.assets.get(&AssetId::new(definition, validator)).unwrap().as_ref(), &Quantity::from(50_u64));
    assert!(rewards::outstanding_rewards(&stx.world, &asset).unwrap().is_zero());
}

#[test]
fn reward_distributions_cannot_reuse_promised_funds() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 200);
    // The fee sink owns 200 independently of the validator's bonded custody.
    reward_distribution(lane, 1, &asset, &validator, 150).execute(&sink, &mut stx).unwrap();
    let error = reward_distribution(lane, 2, &asset, &validator, 100).execute(&sink, &mut stx).unwrap_err();
    assert!(error.to_string().contains("unreserved"));
    assert!(stx.world.public_lane_rewards.get(&(lane, 2)).is_none());
    assert_eq!(rewards::outstanding_rewards(&stx.world, &asset).unwrap(), Quantity::from(150_u64));
}

#[test]
fn reward_reserve_blocks_transfer_and_burn_but_releases_paid_rewards() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xB2);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 200);
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    reward_distribution(lane, 1, &asset, &validator, 150).execute(&sink, &mut stx).unwrap();
    let transfer = iroha_data_model::isi::Transfer::asset_quantity(asset.clone(), Quantity::from(51_u64), validator.clone());
    assert!(transfer.execute(&sink, &mut stx).unwrap_err().to_string().contains("reserved public-lane rewards"));
    assert!(Burn::asset_quantity(51_u64, asset.clone()).execute(&sink, &mut stx).unwrap_err().to_string().contains("reserved public-lane rewards"));
    assert_eq!(stx.world.assets.get(&asset).unwrap().as_ref(), &Quantity::from(200_u64));
    ClaimPublicLaneRewards { lane_id: lane, account: validator.clone(), upto_epoch: None }.execute(&validator, &mut stx).unwrap();
    assert_eq!(stx.world.assets.get(&asset).unwrap().as_ref(), &Quantity::from(50_u64));
    ensure_public_lane_reserves_after_debit(&stx.world, &asset, &Quantity::zero()).unwrap();
}

#[test]
fn reward_failed_payment_restores_claim_and_reserve() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xB3);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    reward_distribution(lane, 1, &asset, &validator, 25).execute(&sink, &mut stx).unwrap();
    // Simulate an unavailable funding balance; payment must not consume entitlement.
    stx.world.assets.remove(asset.clone());
    let claim = ClaimPublicLaneRewards { lane_id: lane, account: validator.clone(), upto_epoch: None };
    assert!(claim.execute(&validator, &mut stx).is_err());
    assert!(stx.world.public_lane_reward_claims.get(&(lane, validator, asset.clone())).is_none());
    assert_eq!(rewards::outstanding_rewards(&stx.world, &asset).unwrap(), Quantity::from(25_u64));
}

#[test]
fn reward_obligation_audit_rejects_corrupt_record_keys() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    reward_distribution(lane, 1, &asset, &validator, 25).execute(&sink, &mut stx).unwrap();
    stx.world.public_lane_rewards.get_mut(&(lane, 1)).unwrap().epoch = 2;
    assert!(rewards::outstanding_rewards(&stx.world, &asset).is_err());
    let error = ClaimPublicLaneRewards {
        lane_id: lane,
        account: validator.clone(),
        upto_epoch: None,
    }.execute(&validator, &mut stx).unwrap_err();
    assert!(error.to_string().contains("non-canonical reward record"));
    assert!(stx.world.public_lane_reward_claims.get(&(lane, validator, asset.clone())).is_none());
    assert_eq!(stx.world.public_lane_reward_reserves.get(&asset), Some(&Quantity::from(25_u64)));
}

#[test]
fn reward_claim_uses_recorded_custody_after_fee_policy_changes() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xB4);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, definition) = configure_reward_fixture(&mut stx, lane, 100);
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    reward_distribution(lane, 1, &asset, &validator, 25).execute(&sink, &mut stx).unwrap();
    stx.nexus.fees.fee_sink_account_id = ALICE_ID.to_string();
    stx.nexus.fees.fee_asset_id = "retired-fee-selector".to_owned();
    stx.nexus.staking.public_validator_mode = iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
    ClaimPublicLaneRewards { lane_id: lane, account: validator.clone(), upto_epoch: None }.execute(&validator, &mut stx).unwrap();
    assert_eq!(stx.world.assets.get(&AssetId::new(definition, validator)).unwrap().as_ref(), &Quantity::from(25_u64));
    assert!(stx.world.public_lane_reward_reserves.get(&asset).is_none());
}

#[test]
fn reward_recording_excludes_bonded_custody_from_a_shared_fee_sink() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xB5);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    // Model the default shared custody layout: 100 fees plus 100 bonded tokens.
    let old_escrow = AccountId::parse_encoded(&stx.nexus.staking.stake_escrow_account_id).unwrap();
    let old_escrow_asset = AssetId::new(asset.definition().clone(), old_escrow);
    let bonded = stx.world.assets.remove(old_escrow_asset.clone()).unwrap();
    let total = stx.world.assets.get(&asset).unwrap().as_ref().checked_add(bonded.as_ref()).unwrap();
    **stx.world.assets.get_mut(&asset).unwrap() = total;
    let held = stx.world.public_lane_stake_reserves.remove(old_escrow_asset).unwrap();
    stx.world.public_lane_stake_reserves.insert(asset.clone(), held.clone());
    stx.world.public_lane_stake_custody.insert((lane, validator.clone()), (asset.clone(), held));
    stx.nexus.staking.stake_escrow_account_id = sink.to_string();
    let error = reward_distribution(lane, 1, &asset, &validator, 101).execute(&sink, &mut stx).unwrap_err();
    assert!(error.to_string().contains("unreserved"));
    assert!(stx.world.public_lane_rewards.get(&(lane, 1)).is_none());
    reward_distribution(lane, 1, &asset, &validator, 100).execute(&sink, &mut stx).unwrap();
    ClaimPublicLaneRewards { lane_id: lane, account: validator.clone(), upto_epoch: None }.execute(&validator, &mut stx).unwrap();
    assert_eq!(stx.world.assets.get(&asset).unwrap().as_ref(), &Quantity::from(100_u64), "bonded custody must remain after every fee is paid");
}

#[test]
fn reward_reserve_checks_aggregate_batch_debits() {
    use iroha_data_model::isi::{TransferAssetBatch, TransferAssetBatchEntry};
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xB6);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, definition) = configure_reward_fixture(&mut stx, lane, 200);
    reward_distribution(lane, 1, &asset, &validator, 150).execute(&sink, &mut stx).unwrap();
    let batch = TransferAssetBatch::new(vec![
        TransferAssetBatchEntry::with_leg_id("one", sink.clone(), validator.clone(), definition.clone(), 30_u32),
        TransferAssetBatchEntry::with_leg_id("two", sink.clone(), validator, definition, 30_u32),
    ]);
    let error = batch.execute(&sink, &mut stx).unwrap_err();
    assert!(error.to_string().contains("reserved public-lane rewards"), "{error}");
    assert_eq!(stx.world.assets.get(&asset).unwrap().as_ref(), &Quantity::from(200_u64));
    assert_eq!(stx.world.public_lane_reward_reserves.get(&asset), Some(&Quantity::from(150_u64)));
}

#[test]
fn reward_failed_second_asset_rolls_back_the_enclosing_transaction() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let lane = LaneId::SINGLE;
    let (validator, assets, nexus) = {
        let mut stx = state_block.transaction();
        let (sink, validator, first_asset, _) = configure_reward_fixture(&mut stx, lane, 100);
        stx.nexus.staking.reward_dust_threshold = Quantity::zero();
        reward_distribution(lane, 0, &first_asset, &validator, 25)
            .execute(&sink, &mut stx)
            .unwrap();
        let second_definition = AssetDefinitionId::derive_from_components(
            DomainId::try_new("wonderland", "universal").unwrap(),
            "other-reward".parse().unwrap(),
        );
        Register::asset_definition(AssetDefinition::numeric(
            second_definition.clone(),
            "Other reward",
            iroha_data_model::asset::AssetBalancePolicy::Global,
            None,
        ))
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
        let second_asset = AssetId::new(second_definition.clone(), sink.clone());
        Mint::asset_quantity(100_u64, second_asset.clone())
            .execute(&ALICE_ID, &mut stx)
            .unwrap();
        stx.nexus.fees.fee_asset_id = second_definition.to_string();
        reward_distribution(lane, 1, &second_asset, &validator, 25)
            .execute(&sink, &mut stx)
            .unwrap();
        let mut assets = vec![first_asset, second_asset];
        assets.sort();
        let nexus = stx.nexus.clone();
        stx.apply();
        (validator, assets, nexus)
    };
    state_block.drain_transfer_transcripts();
    {
        let mut stx = state_block.transaction();
        stx.nexus = nexus;
        seed_test_call_hash(&mut stx, 0xB7);
        // The later sorted custody asset becomes unavailable after the first payout.
        stx.world.assets.remove(assets[1].clone());
        let error = ClaimPublicLaneRewards {
            lane_id: lane,
            account: validator.clone(),
            upto_epoch: None,
        }
        .execute(&validator, &mut stx)
        .unwrap_err();
        assert!(matches!(error, Error::Find(FindError::Asset(_))), "{error}");
        assert_eq!(
            stx.world.assets.get(&assets[0]).unwrap().as_ref(),
            &Quantity::from(75_u64),
            "the first payout must execute before the second fails"
        );
        assert!(
            stx.world
                .public_lane_reward_reserves
                .get(&assets[0])
                .is_none()
        );
        assert_eq!(
            stx.world.public_lane_reward_reserves.get(&assets[1]),
            Some(&Quantity::from(25_u64))
        );
        // Production rejects and drops this whole overlay on any instruction error.
    }
    assert!(state_block.drain_transfer_transcripts().is_empty());
    let stx = state_block.transaction();
    for asset in assets {
        assert_eq!(
            stx.world.assets.get(&asset).unwrap().as_ref(),
            &Quantity::from(100_u64)
        );
        assert_eq!(
            stx.world.public_lane_reward_reserves.get(&asset),
            Some(&Quantity::from(25_u64))
        );
        assert!(
            stx.world
                .public_lane_reward_claims
                .get(&(lane, validator.clone(), asset.clone()))
                .is_none()
        );
        let destination = AssetId::new(asset.definition().clone(), validator.clone());
        assert!(stx.world.assets.get(&destination).is_none());
    }
    assert_eq!(stx.pending_transfer_transcript_count_for_testing(), 0);
}
