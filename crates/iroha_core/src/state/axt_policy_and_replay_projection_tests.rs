state_test! { sync axt_policy_refresh_clears_stale_entries_when_snapshot_missing
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    let dsid = DataSpaceId::new(31);
    let_row! { policy = AxtPolicyEntry { manifest_root: [0x77; 32], target_lane: LaneId::new(2), active_handle_era: 1, next_handle_counter: 1, current_slot: 1, } };
    state.set_axt_policy(dsid, policy);
    let snapshot = state.refresh_axt_policies_from_directory();
    assert!(
        snapshot.is_none(),
        "no snapshot should be derived without manifests"
    );
    let view = state.world.axt_policies.view();
    assert!(
        view.get(&dsid).is_none(),
        "stale policy entries must be cleared"
    );
}
state_test! { sync state_block_axt_policy_snapshot_reads_block_scope
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let dsid = DataSpaceId::new(13);
    let_row! { entry = AxtPolicyEntry { manifest_root: [0x66; 32], target_lane: LaneId::new(2), active_handle_era: 5, next_handle_counter: 4, current_slot: 99, } };
    {
        let_row! { lane_catalog = LaneCatalog::new( nonzero!(3_u32), vec![LaneConfig { id: entry.target_lane, dataspace_id: dsid, alias: "block-scope-axt".into(), ..LaneConfig::default() }], ) .expect("block-scope AXT lane catalog") };
        install_test_nexus_lane_catalog(state.nexus.get_mut(), lane_catalog);
    }
    let mut block = state.block(header);
    block.world.axt_policies.insert(dsid, entry);
    let expected_slot = block.block_hashes().len() as u64;
    let snapshot = block.axt_policy_snapshot();
    let_row! { binding = snapshot .entries .iter() .find(|binding| binding.dsid == dsid) .expect("policy from block scope available") };
    assert_eq!(binding.policy.manifest_root, entry.manifest_root);
    assert_eq!(binding.policy.target_lane, entry.target_lane);
    assert_eq!(binding.policy.active_handle_era, entry.active_handle_era);
    assert_eq!(
        binding.policy.next_handle_counter,
        entry.next_handle_counter
    );
    assert_eq!(binding.policy.current_slot, expected_slot);
    let expected_version = AxtPolicySnapshot::compute_version(&snapshot.entries);
    assert_eq!(snapshot.version, expected_version);
}
state_test! { sync axt_replay_ledger_overlay_applies
    let dsid = DataSpaceId::new(41);
    let lane = LaneId::new(0);
    let_row! { lane_catalog = LaneCatalog::new( nonzero!(1_u32), vec![public_lane!(lane, dsid, "primary".to_owned())], ) .expect("lane catalog") };
    let_row! { mut nexus = iroha_config::parameters::actual::Nexus { lane_catalog: lane_catalog.clone(), lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog), dataspace_catalog: dataspace_catalog_for_lane_catalog(&lane_catalog), routing_policy: LaneRoutingPolicy { default_lane: lane, default_dataspace: dsid, ..Default::default() }, ..Default::default() } };
    nexus.axt.slot_length_ms = NonZeroU64::new(1).expect("slot length");
    nexus.axt.replay_retention_slots = NonZeroU64::new(2).expect("retention");
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_with_nexus_for_testing(World::new(), nexus, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(lane);
    let key = AxtHandleReplayKey::from_parts(
        dsid,
        axt_replay_incarnation_for_test(0xAA),
        [0xAA; 32],
        3,
        7,
        lane,
    );
    let_row! { record = axt_replay_record_for_key(&key, 1, 4) };
    stx.world.axt_replay_ledger.insert(key, record.clone());
    stx.apply();
    assert_eq!(
        block.world.axt_replay_ledger.get(&key).cloned(),
        Some(record)
    );
}
state_test! { sync ordinary_block_apply_defers_axt_replay_pruning_until_commit
    let dsid = DataSpaceId::new(42);
    let lane = LaneId::new(0);
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.axt.slot_length_ms = NonZeroU64::new(1).expect("slot length");
    nexus.axt.replay_retention_slots = NonZeroU64::new(2).expect("retention");
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(World::new(), kura, query_handle);
    state
        .set_nexus(nexus)
        .expect("apply Nexus config for replay ledger pruning test");
    let key = AxtHandleReplayKey::from_parts(
        dsid,
        axt_replay_incarnation_for_test(0xAB),
        [0xAB; 32],
        3,
        7,
        lane,
    );
    let_row! { stale = axt_replay_record_for_key(&key, 1, 2) };
    {
        let mut block = state.world.axt_replay_ledger.block();
        block.insert(key, stale.clone());
        block.commit();
    }
    let keypair = crate::state::checked_keypair();
    let_row! { signed: SignedBlock = BlockBuilder::new(vec![dummy_accepted_transaction()]) .chain(0, state.view().latest_block().as_deref()) .sign(keypair.private_key()) .unpack(|_| {}) .into() };
    assert!(
        signed.axt_envelopes().is_none(),
        "test block must not carry AXT envelopes"
    );
    let mut state_block = state.block(signed.header());
    let valid = ValidBlock::validate_unchecked(signed, &mut state_block).unpack(|_| {});
    let committed = valid.commit_unchecked().unpack(|_| {});
    let _ = state_block.apply_without_execution(&committed, Vec::new());
    assert_eq!(
        state_block.world.axt_replay_ledger.get(&key).cloned(),
        Some(stale),
        "ordinary block apply should leave AXT replay pruning to commit"
    );
    state_block.commit().expect("ordinary block should commit");
    assert!(
        state.world.axt_replay_ledger.view().get(&key).is_none(),
        "ordinary block commit should prune expired AXT replay entries"
    );
}
