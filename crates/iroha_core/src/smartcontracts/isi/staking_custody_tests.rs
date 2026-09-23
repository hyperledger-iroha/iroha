// Exact custody protection through deposits, pending withdrawals and configuration drift.

fn register_custody_fixture(
    stx: &mut StateTransaction<'_, '_>,
    lane: LaneId,
    amount: u64,
) -> (AccountId, AccountId, AssetId) {
    let (validator, delegator, escrow, definition) = prepare_accounts(stx);
    RegisterPublicLaneValidator {
        monetary_plan: fixture_registration_plan(&stx, &validator, Quantity::from(amount)),
        lane_id: lane,
        peer_id: validator_peer_id(&validator),
        validator: validator.clone(),
        stake_account: validator.clone(),
        initial_stake: Quantity::from(amount),
        metadata: Metadata::default(),
    }
    .execute(&validator, stx)
    .expect("register backed staking custody");
    let asset = AssetId::new(definition, escrow);
    assert_eq!(
        stx.world
            .public_lane_stake_custody
            .get(&(lane, validator.clone())),
        Some(&(asset.clone(), Quantity::from(amount)))
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset),
        Some(&Quantity::from(amount))
    );
    (validator, delegator, asset)
}

#[test]
fn staking_custody_blocks_generic_debits_of_bonded_and_pending_funds() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xC4);
    let lane = LaneId::new(17);
    let (validator, recipient, asset) = register_custody_fixture(&mut stx, lane, 1_000);
    let custody = stx
        .world
        .public_lane_stake_custody
        .get(&(lane, validator.clone()))
        .cloned();
    for pending in [false, true] {
        if pending {
            SchedulePublicLaneUnbond {
                lane_id: lane,
                validator: validator.clone(),
                staker: validator.clone(),
                request_id: Hash::new("custody-protected-pending"),
                amount: Quantity::from(400_u64),
                release_at_ms: stx.block_unix_timestamp_ms(),
            }
            .execute(&validator, &mut stx)
            .unwrap();
        }
        let transfer = iroha_data_model::isi::Transfer::asset_quantity(
            asset.clone(),
            Quantity::one(),
            recipient.clone(),
        );
        for error in [
            transfer.execute(asset.account(), &mut stx).unwrap_err(),
            Burn::asset_quantity(1_u64, asset.clone())
                .execute(asset.account(), &mut stx)
                .unwrap_err(),
        ] {
            assert!(
                error.to_string().contains("reserved public-lane"),
                "{error}"
            );
        }
        assert_eq!(
            stx.world.assets.get(&asset).unwrap().as_ref(),
            &Quantity::from(1_000_u64)
        );
        assert_eq!(
            stx.world
                .public_lane_stake_custody
                .get(&(lane, validator.clone()))
                .cloned(),
            custody
        );
        assert_eq!(
            stx.world.public_lane_stake_reserves.get(&asset),
            Some(&Quantity::from(1_000_u64))
        );
    }
    let share = stx
        .world
        .public_lane_stake_shares
        .get(&stake_key(lane, &validator, &validator))
        .unwrap();
    assert_eq!(share.bonded, Quantity::from(600_u64));
    assert_eq!(
        share.pending_unbonds[&Hash::new("custody-protected-pending")].amount,
        Quantity::from(400_u64)
    );
    Mint::asset_quantity(25_u64, asset.clone())
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    iroha_data_model::isi::Transfer::asset_quantity(
        asset.clone(),
        Quantity::from(25_u64),
        recipient,
    )
    .execute(asset.account(), &mut stx)
    .expect("unreserved funds above the custody floor remain transferable");
    assert_eq!(
        stx.world.assets.get(&asset).unwrap().as_ref(),
        &Quantity::from(1_000_u64)
    );
}

#[test]
fn staking_custody_and_rewards_share_one_additive_reserve_floor() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xC0);
    let lane = LaneId::SINGLE;
    let (sink, validator, asset, _) = configure_reward_fixture(&mut stx, lane, 100);
    let key = (lane, validator.clone());
    let (old_asset, held) = stx
        .world
        .public_lane_stake_custody
        .get(&key)
        .unwrap()
        .clone();
    let old_balance = stx.world.assets.remove(old_asset.clone()).unwrap();
    let total = stx
        .world
        .assets
        .get(&asset)
        .unwrap()
        .as_ref()
        .checked_add(old_balance.as_ref())
        .unwrap();
    **stx.world.assets.get_mut(&asset).unwrap() = total;
    stx.world.public_lane_stake_reserves.remove(old_asset);
    stx.world
        .public_lane_stake_reserves
        .insert(asset.clone(), held.clone());
    stx.world
        .public_lane_stake_custody
        .insert(key.clone(), (asset.clone(), held));
    stx.nexus.staking.stake_escrow_account_id = sink.to_string();
    stx.nexus.staking.reward_dust_threshold = Quantity::zero();
    reward_distribution(lane, 0, &asset, &validator, 100)
        .execute(&sink, &mut stx)
        .unwrap();
    let transfer = iroha_data_model::isi::Transfer::asset_quantity(
        asset.clone(),
        Quantity::one(),
        validator.clone(),
    );
    assert!(
        transfer
            .execute(&sink, &mut stx)
            .unwrap_err()
            .to_string()
            .contains("reserved public-lane")
    );
    assert!(
        Burn::asset_quantity(1_u64, asset.clone())
            .execute(&sink, &mut stx)
            .unwrap_err()
            .to_string()
            .contains("reserved public-lane")
    );
    ClaimPublicLaneRewards {
        claim_plan: fixture_reward_claim_plan(&stx, lane, &(validator), None),
        lane_id: lane,
        account: validator,
    }
    .execute(&key.1, &mut stx)
    .unwrap();
    assert_eq!(
        stx.world.assets.get(&asset).unwrap().as_ref(),
        &Quantity::from(100_u64)
    );
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&key),
        Some(&(asset.clone(), Quantity::from(100_u64)))
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset),
        Some(&Quantity::from(100_u64))
    );
    assert!(stx.world.public_lane_reward_reserves.get(&asset).is_none());
}

#[test]
fn staking_same_account_bond_cannot_reuse_held_custody() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let lane = LaneId::new(17);
    let (validator, asset, before, share_before, nexus) = {
        let mut stx = state_block.transaction();
        let (validator, _, _, definition) = prepare_accounts(&mut stx);
        stx.nexus.staking.stake_escrow_account_id = validator.to_string();
        RegisterPublicLaneValidator {
            monetary_plan: fixture_registration_plan(&stx, &validator, Quantity::from(10_000_u64)),
            lane_id: lane,
            peer_id: validator_peer_id(&validator),
            validator: validator.clone(),
            stake_account: validator.clone(),
            initial_stake: Quantity::from(10_000_u64),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let asset = AssetId::new(definition, validator.clone());
        let before = stx
            .world
            .public_lane_validators
            .get(&(lane, validator.clone()))
            .unwrap()
            .clone();
        let share_before = stx
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane, &validator, &validator))
            .unwrap()
            .clone();
        let nexus = stx.nexus.clone();
        stx.apply();
        (validator, asset, before, share_before, nexus)
    };
    state_block.drain_transfer_transcripts();
    let key = (lane, validator.clone());
    let share_key = stake_key(lane, &validator, &validator);
    let instruction = {
        let mut stx = state_block.transaction();
        stx.nexus = nexus.clone();
        BondPublicLaneStake {
            monetary_plan: fixture_bond_plan(&stx, lane, &validator, &validator, Quantity::one()),
            lane_id: lane,
            validator: validator.clone(),
            staker: validator.clone(),
            amount: Quantity::one(),
            metadata: Metadata::default(),
        }
    };
    {
        let mut stx = state_block.transaction();
        stx.nexus = nexus.clone();
        seed_test_call_hash(&mut stx, 0xC5);
        let error = instruction
            .clone()
            .execute(&validator, &mut stx)
            .unwrap_err();
        assert!(error.to_string().contains("unreserved custody"), "{error}");
        assert_eq!(
            stx.world.public_lane_validators.get(&key).unwrap().status,
            PublicLaneValidatorStatus::Active,
            "the rejected overlay includes the eligible lifecycle promotion"
        );
        // Native instruction errors discard the enclosing transaction, including
        // lifecycle changes that ran before the custody check rejected the bond.
    }
    assert!(state_block.drain_transfer_transcripts().is_empty());
    let mut stx = state_block.transaction();
    stx.nexus = nexus;
    assert_eq!(stx.world.public_lane_validators.get(&key), Some(&before));
    assert_eq!(
        stx.world.public_lane_stake_shares.get(&share_key),
        Some(&share_before)
    );
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&key),
        Some(&(asset.clone(), Quantity::from(10_000_u64)))
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset),
        Some(&Quantity::from(10_000_u64))
    );
    assert_eq!(
        stx.world.assets.get(&asset).unwrap().as_ref(),
        &Quantity::from(10_000_u64)
    );
    assert_eq!(stx.pending_transfer_transcript_count_for_testing(), 0);

    seed_test_call_hash(&mut stx, 0xC6);
    Mint::asset_quantity(1_u64, asset.clone())
        .execute(&ALICE_ID, &mut stx)
        .unwrap();
    instruction
        .execute(&validator, &mut stx)
        .expect("same-account bonding may reserve newly supplied free funds");
    let after = stx.world.public_lane_validators.get(&key).unwrap();
    assert_eq!(after.total_stake, Quantity::from(10_001_u64));
    assert_eq!(after.self_stake, Quantity::from(10_001_u64));
    assert_eq!(
        stx.world
            .public_lane_stake_shares
            .get(&share_key)
            .unwrap()
            .bonded,
        Quantity::from(10_001_u64)
    );
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&key),
        Some(&(asset.clone(), Quantity::from(10_001_u64)))
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset),
        Some(&Quantity::from(10_001_u64))
    );
    assert_eq!(
        stx.world.assets.get(&asset).unwrap().as_ref(),
        &Quantity::from(10_001_u64)
    );
}

#[test]
fn staking_failed_slash_restores_exact_custody_preimages() {
    let state = setup_state();
    let block = new_block();
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    seed_test_call_hash(&mut stx, 0xC1);
    let lane = LaneId::new(17);
    let (validator, sink, asset) = register_custody_fixture(&mut stx, lane, 1_000);
    RegisterPublicLaneValidator {
        monetary_plan: fixture_registration_plan(&stx, &sink, Quantity::from(500_u64)),
        lane_id: lane,
        peer_id: validator_peer_id(&sink),
        validator: sink.clone(),
        stake_account: sink.clone(),
        initial_stake: Quantity::from(500_u64),
        metadata: Metadata::default(),
    }
    .execute(&sink, &mut stx)
    .expect("second validator shares the same exact escrow asset");
    stx.nexus.staking.slash_sink_account_id = sink.to_string();
    stx.nexus.staking.max_slash_bps = 10_000;
    let key = (lane, validator.clone());
    let share_key = stake_key(lane, &validator, &validator);
    let validator_before = stx.world.public_lane_validators.get(&key).unwrap().clone();
    let share_before = stx
        .world
        .public_lane_stake_shares
        .get(&share_key)
        .unwrap()
        .clone();
    let custody_before = stx.world.public_lane_stake_custody.get(&key).cloned();
    let reserve_before = stx.world.public_lane_stake_reserves.get(&asset).cloned();
    let funding = stx.world.assets.remove(asset.clone()).unwrap();
    let instruction = SlashPublicLaneValidator {
        monetary_plan: fixture_slash_plan(&stx, lane, &(validator), 1, Quantity::from(100_u64)),
        lane_id: lane,
        validator,
        offence_height: 1,
        slash_id: Hash::new("failed-custody-slash"),
        amount: Quantity::from(100_u64),
        reason_code: "custody-regression".to_owned(),
        metadata: Metadata::default(),
    };
    let error = instruction
        .clone()
        .execute(&ALICE_ID, &mut stx)
        .unwrap_err();
    assert!(matches!(error, Error::Find(FindError::Asset(_))), "{error}");
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&key).cloned(),
        custody_before
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset).cloned(),
        reserve_before
    );
    assert_eq!(
        stx.world.public_lane_validators.get(&key),
        Some(&validator_before)
    );
    assert_eq!(
        stx.world.public_lane_stake_shares.get(&share_key),
        Some(&share_before)
    );
    stx.world.assets.insert(asset.clone(), funding);
    instruction.execute(&ALICE_ID, &mut stx).unwrap();
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&key),
        Some(&(asset.clone(), Quantity::from(900_u64)))
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset),
        Some(&Quantity::from(1_400_u64))
    );
    assert_eq!(
        stx.world.assets.get(&asset).unwrap().as_ref(),
        &Quantity::from(1_400_u64)
    );
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&(lane, sink)),
        Some(&(asset, Quantity::from(500_u64)))
    );
}

#[test]
fn staking_failed_mature_unbond_restores_exact_custody_preimages() {
    for withdrawal in [400_u64, 1_000_u64] {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let lane = LaneId::new(17);
        let request_id = Hash::new("failed-custody-unbond");
        let (validator, asset, release_at_ms, release_height, nexus) = {
            let block = new_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let (validator, _, asset) = register_custody_fixture(&mut stx, lane, 1_000);
            let release_at_ms = stx.block_unix_timestamp_ms();
            SchedulePublicLaneUnbond {
                lane_id: lane,
                validator: validator.clone(),
                staker: validator.clone(),
                request_id,
                amount: Quantity::from(withdrawal),
                release_at_ms,
            }
            .execute(&validator, &mut stx)
            .unwrap();
            let release_height = stx
                .world
                .public_lane_stake_shares
                .get(&stake_key(lane, &validator, &validator))
                .unwrap()
                .pending_unbonds[&request_id]
                .liability_release_height;
            let nexus = stx.nexus.clone();
            stx.apply();
            state_block.commit_world_overlay_for_testing().unwrap();
            (validator, asset, release_at_ms, release_height, nexus)
        };
        let block = new_block_with_height_and_time(release_height, release_at_ms);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus = nexus;
        seed_test_call_hash(&mut stx, 0xC2);
        let key = (lane, validator.clone());
        let share_key = stake_key(lane, &validator, &validator);
        let share_before = stx
            .world
            .public_lane_stake_shares
            .get(&share_key)
            .unwrap()
            .clone();
        let custody_before = stx.world.public_lane_stake_custody.get(&key).cloned();
        let reserve_before = stx.world.public_lane_stake_reserves.get(&asset).cloned();
        let funding = stx.world.assets.remove(asset.clone()).unwrap();
        let instruction = FinalizePublicLaneUnbond {
            monetary_plan: fixture_unbond_plan(&stx, lane, &validator, &validator, request_id),
            lane_id: lane,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
        };
        let error = instruction
            .clone()
            .execute(&validator, &mut stx)
            .unwrap_err();
        assert!(matches!(error, Error::Find(FindError::Asset(_))), "{error}");
        assert_eq!(
            stx.world.public_lane_stake_custody.get(&key).cloned(),
            custody_before
        );
        assert_eq!(
            stx.world.public_lane_stake_reserves.get(&asset).cloned(),
            reserve_before
        );
        assert_eq!(
            stx.world.public_lane_stake_shares.get(&share_key),
            Some(&share_before)
        );
        stx.world.assets.insert(asset.clone(), funding);
        instruction.execute(&validator, &mut stx).unwrap();
        let remaining = Quantity::from(1_000_u64 - withdrawal);
        if remaining.is_zero() {
            assert!(stx.world.public_lane_stake_custody.get(&key).is_none());
            assert!(stx.world.public_lane_stake_reserves.get(&asset).is_none());
            assert!(stx.world.assets.get(&asset).is_none());
        } else {
            assert_eq!(
                stx.world.public_lane_stake_custody.get(&key),
                Some(&(asset.clone(), remaining.clone()))
            );
            assert_eq!(
                stx.world.public_lane_stake_reserves.get(&asset),
                Some(&remaining)
            );
            assert_eq!(stx.world.assets.get(&asset).unwrap().as_ref(), &remaining);
        }
    }
}

#[test]
fn staking_unbond_uses_original_custody_after_configuration_and_alias_changes() {
    for unresolved_aliases in [false, true] {
        let mut state = setup_state();
        set_epoch_length(&mut state, 3);
        let lane = LaneId::new(17);
        let request_id = Hash::new("custody-drift-unbond");
        let (validator, replacement, asset, release_at_ms, release_height, nexus) = {
            let block = new_block();
            let mut state_block = state.block(block.as_ref().header());
            let mut stx = state_block.transaction();
            let (validator, replacement, asset) = register_custody_fixture(&mut stx, lane, 1_000);
            let release_at_ms = stx.block_unix_timestamp_ms();
            SchedulePublicLaneUnbond {
                lane_id: lane,
                validator: validator.clone(),
                staker: validator.clone(),
                request_id,
                amount: Quantity::from(400_u64),
                release_at_ms,
            }
            .execute(&validator, &mut stx)
            .unwrap();
            let release_height = stx
                .world
                .public_lane_stake_shares
                .get(&stake_key(lane, &validator, &validator))
                .unwrap()
                .pending_unbonds[&request_id]
                .liability_release_height;
            let nexus = stx.nexus.clone();
            stx.apply();
            state_block.commit_world_overlay_for_testing().unwrap();
            (
                validator,
                replacement,
                asset,
                release_at_ms,
                release_height,
                nexus,
            )
        };
        let block = new_block_with_height_and_time(release_height, release_at_ms);
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        stx.nexus = nexus;
        seed_test_call_hash(&mut stx, 0xC3);
        stx.nexus.staking.stake_escrow_account_id = replacement.to_string();
        let key = (lane, validator.clone());
        let custody_before = stx.world.public_lane_stake_custody.get(&key).cloned();
        let error = BondPublicLaneStake {
            monetary_plan: fixture_bond_plan(&stx, lane, &validator, &validator, Quantity::one()),
            lane_id: lane,
            validator: validator.clone(),
            staker: validator.clone(),
            amount: Quantity::one(),
            metadata: Metadata::default(),
        }
        .execute(&validator, &mut stx)
        .unwrap_err();
        assert!(error.to_string().contains("retained custody"), "{error}");
        assert_eq!(
            stx.world.public_lane_stake_custody.get(&key).cloned(),
            custody_before
        );
        if unresolved_aliases {
            stx.nexus.staking.stake_escrow_account_id = "retired@nexus".to_owned();
            stx.nexus.staking.stake_asset_id = "retired#nexus".to_owned();
        }
        FinalizePublicLaneUnbond {
            monetary_plan: fixture_unbond_plan(&stx, lane, &validator, &validator, request_id),
            lane_id: lane,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
        }
        .execute(&validator, &mut stx)
        .unwrap();
        assert_eq!(
            stx.world.assets.get(&asset).unwrap().as_ref(),
            &Quantity::from(600_u64)
        );
        assert_eq!(
            stx.world
                .assets
                .get(&AssetId::new(asset.definition().clone(), validator))
                .unwrap()
                .as_ref(),
            &Quantity::from(9_400_u64)
        );
        assert_eq!(
            stx.world
                .assets
                .get(&AssetId::new(asset.definition().clone(), replacement))
                .unwrap()
                .as_ref(),
            &Quantity::from(10_000_u64)
        );
        assert_eq!(
            stx.world.public_lane_stake_custody.get(&key),
            Some(&(asset.clone(), Quantity::from(600_u64)))
        );
        assert_eq!(
            stx.world.public_lane_stake_reserves.get(&asset),
            Some(&Quantity::from(600_u64))
        );
    }
}

#[test]
fn staking_mode_owner_change_rejects_pending_custody_without_blocking_withdrawal() {
    let mut state = setup_state();
    set_epoch_length(&mut state, 3);
    let lane = LaneId::new(17);
    let request_id = Hash::new("custody-mode-change-unbond");
    let (validator, asset, before, share_before, release_at_ms, release_height, nexus) = {
        let block = new_block();
        let mut state_block = state.block(block.as_ref().header());
        let mut stx = state_block.transaction();
        let (validator, _, asset) = register_custody_fixture(&mut stx, lane, 1_000);
        let release_at_ms = stx.block_unix_timestamp_ms();
        SchedulePublicLaneUnbond {
            lane_id: lane,
            validator: validator.clone(),
            staker: validator.clone(),
            request_id,
            amount: Quantity::from(400_u64),
            release_at_ms,
        }
        .execute(&validator, &mut stx)
        .unwrap();
        let before = stx
            .world
            .public_lane_validators
            .get(&(lane, validator.clone()))
            .unwrap()
            .clone();
        let share_before = stx
            .world
            .public_lane_stake_shares
            .get(&stake_key(lane, &validator, &validator))
            .unwrap()
            .clone();
        let release_height = share_before.pending_unbonds[&request_id].liability_release_height;
        let nexus = stx.nexus.clone();
        stx.apply();
        state_block.commit_world_overlay_for_testing().unwrap();
        (
            validator,
            asset,
            before,
            share_before,
            release_at_ms,
            release_height,
            nexus,
        )
    };
    let mut attempted = state.nexus_snapshot();
    attempted.staking.public_validator_mode =
        iroha_config::parameters::actual::LaneValidatorMode::AdminManaged;
    let error = state
        .set_nexus(attempted)
        .expect_err("normal reconfiguration must preserve a live staking owner");
    assert!(
        matches!(
            &error,
            crate::state::LaneLifecycleError::UnsafeRetirement { lane: rejected, reason }
                if *rejected == lane && reason.contains("canonical owner reset or change")
        ),
        "{error}"
    );
    assert_eq!(
        state.nexus_snapshot().staking.public_validator_mode,
        iroha_config::parameters::actual::LaneValidatorMode::StakeElected
    );

    let block = new_block_with_height_and_time(release_height, release_at_ms);
    let mut state_block = state.block(block.as_ref().header());
    let mut stx = state_block.transaction();
    stx.nexus = nexus;
    seed_test_call_hash(&mut stx, 0xC7);
    let key = (lane, validator.clone());
    let share_key = stake_key(lane, &validator, &validator);
    let destination = AssetId::new(asset.definition().clone(), validator.clone());
    assert_eq!(stx.world.public_lane_validators.get(&key), Some(&before));
    assert_eq!(
        stx.world.public_lane_stake_shares.get(&share_key),
        Some(&share_before)
    );
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&key),
        Some(&(asset.clone(), Quantity::from(1_000_u64)))
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset),
        Some(&Quantity::from(1_000_u64))
    );
    assert_eq!(
        stx.world.assets.get(&asset).unwrap().as_ref(),
        &Quantity::from(1_000_u64)
    );
    assert_eq!(
        stx.world.assets.get(&destination).unwrap().as_ref(),
        &Quantity::from(9_000_u64)
    );
    FinalizePublicLaneUnbond {
        monetary_plan: fixture_unbond_plan(&stx, lane, &validator, &validator, request_id),
        lane_id: lane,
        validator: validator.clone(),
        staker: validator.clone(),
        request_id,
    }
    .execute(&validator, &mut stx)
    .expect("the retained staking mode must still allow matured withdrawals");
    assert_eq!(
        stx.world.public_lane_stake_custody.get(&key),
        Some(&(asset.clone(), Quantity::from(600_u64)))
    );
    assert_eq!(
        stx.world.public_lane_stake_reserves.get(&asset),
        Some(&Quantity::from(600_u64))
    );
    assert_eq!(
        stx.world.assets.get(&asset).unwrap().as_ref(),
        &Quantity::from(600_u64)
    );
    assert_eq!(
        stx.world.assets.get(&destination).unwrap().as_ref(),
        &Quantity::from(9_400_u64)
    );
    let share = stx.world.public_lane_stake_shares.get(&share_key).unwrap();
    assert_eq!(share.bonded, Quantity::from(600_u64));
    assert!(share.pending_unbonds.is_empty());
}
