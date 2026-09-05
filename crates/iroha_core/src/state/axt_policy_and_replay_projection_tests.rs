#[test]
#[allow(clippy::too_many_lines)]
fn axt_replay_ledger_survives_state_restart() {
    let authority = ALICE_ID.clone();
    let dsid = DataSpaceId::UNIVERSAL;
    let lane = LaneId::new(0);
    let (world, issuer, issuer_uaid, manifest_root, asset_definition_id, incarnation) =
        authenticated_axt_replay_world(dsid, 0x44);
    let_row! { lane_catalog = LaneCatalog::new( nonzero!(1_u32), vec![public_lane!(lane, dsid, "primary".to_owned())], ) .expect("lane catalog") };
    let_row! { mut nexus = iroha_config::parameters::actual::Nexus { lane_catalog: lane_catalog.clone(), lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog), dataspace_catalog: dataspace_catalog_for_lane_catalog(&lane_catalog), routing_policy: LaneRoutingPolicy { default_lane: lane, default_dataspace: dsid, ..Default::default() }, ..Default::default() } };
    nexus.axt.slot_length_ms = NonZeroU64::new(1).expect("slot length");
    nexus.axt.replay_retention_slots = NonZeroU64::new(4).expect("retention");
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_nexus_for_testing(world, nexus.clone(), query_handle);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 2,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let active_policy = state
        .world
        .axt_policies
        .view()
        .get(&dsid)
        .copied()
        .expect("restart fixture policy is installed");
    assert_eq!(active_policy.active_handle_era, 2);
    let_row! { descriptor = AxtDescriptor { dsids: vec![dsid], touches: vec![AxtTouchSpec { dsid, read: vec!["orders/replay".into()], write: vec!["ledger/replay".into()], }], } };
    let binding = descriptor.binding().expect("compute binding");
    let_row! { touch_manifest = TouchManifest { read: vec!["orders/replay".into()], write: vec!["ledger/replay".into()], } };
    let issuer_context = AxtHandleIssuerContextV1 {
        network_id: state.network_id,
        asset_dsid: dsid,
        asset_definition_incarnation: incarnation,
        issuer: issuer_uaid,
        issuer_manifest_root: manifest_root,
        code_root: [0; 32],
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
    };
    let handle = signed_axt_handle_for_block_freeze(
        &issuer,
        issuer_context,
        lane,
        binding,
        active_policy.active_handle_era,
        active_policy.next_handle_counter,
    );
    assert_eq!(handle.asset_definition_id, asset_definition_id);
    let_row! { handle_fragment = AxtHandleFragment { handle, intent: RemoteSpendIntent { asset_dsid: dsid, op: SpendOp { asset_definition_id, kind: "transfer".into(), from: authority.to_string(), to: BOB_ID.to_string(), amount: Some(Quantity::from(10_u64)), }, }, proof: None, amount: Some(Quantity::from(10_u64)), amount_commitment: None, } };
    let_row! { proof_fragment = AxtProofFragment { dsid, proof: axt_proof_blob_for_remote_spend( dsid, manifest_root, b"state-restart", 200, &handle_fragment, &Quantity::from(10_u64), ), } };
    let_row! { ivm_descriptor = ivm::axt::AxtDescriptor { dsids: descriptor.dsids.clone(), touches: descriptor .touches .iter() .map(|touch| ivm::axt::AxtTouchSpec { dsid: touch.dsid, read: touch.read.clone(), write: touch.write.clone(), }) .collect(), } };
    let_row! { ivm_manifest = ivm::axt::TouchManifest { read: touch_manifest.read.clone(), write: touch_manifest.write.clone(), } };
    let_row! { ivm_proof = ivm::axt::ProofBlob { payload: proof_fragment.proof.payload.clone(), expiry_slot: proof_fragment.proof.expiry_slot, } };
    let_row! { ivm_handle = ivm::axt::AssetHandle { scope: handle_fragment.handle.scope.clone(), asset_definition_id: handle_fragment.handle.asset_definition_id.clone(), subject: ivm::axt::HandleSubject { account: handle_fragment.handle.subject.account.clone(), origin_dsid: handle_fragment.handle.subject.origin_dsid, }, budget: ivm::axt::HandleBudget { remaining: handle_fragment.handle.budget.remaining.clone(), per_use: handle_fragment.handle.budget.per_use.clone(), }, handle_era: handle_fragment.handle.handle_era, sub_nonce: handle_fragment.handle.sub_nonce, group_binding: ivm::axt::GroupBinding { composability_group_id: handle_fragment .handle .group_binding .composability_group_id .clone(), epoch_id: handle_fragment.handle.group_binding.epoch_id, }, target_lane: handle_fragment.handle.target_lane, axt_binding: handle_fragment.handle.axt_binding.as_bytes().to_vec(), manifest_view_root: handle_fragment.handle.manifest_view_root.to_vec(), expiry_slot: handle_fragment.handle.expiry_slot, max_clock_skew_ms: handle_fragment.handle.max_clock_skew_ms, issuer_context: handle_fragment.handle.issuer_context, issuer_signature: handle_fragment.handle.issuer_signature.clone(), } };
    let_row! { ivm_intent = ivm::axt::RemoteSpendIntent { asset_dsid: handle_fragment.intent.asset_dsid, op: ivm::axt::SpendOp { asset_definition_id: handle_fragment.intent.op.asset_definition_id.clone(), kind: handle_fragment.intent.op.kind.clone(), from: handle_fragment.intent.op.from.clone(), to: handle_fragment.intent.op.to.clone(), amount: handle_fragment.intent.op.amount.clone(), }, } };
    {
        let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
        let mut block = state.block(header);
        let mut stx = block.transaction();
        stx.current_lane_id = Some(lane);
        stx.record_axt_envelope(AxtEnvelopeRecord {
            binding,
            lane,
            descriptor: descriptor.clone(),
            touches: vec![AxtTouchFragment {
                dsid,
                manifest: touch_manifest.clone(),
            }],
            proofs: vec![proof_fragment.clone()],
            handles: vec![handle_fragment.clone()],
            commit_height: 1,
        })
        .expect("exact AXT sequence should stage");
        stx.apply();
        block
            .commit_empty_block_for_testing()
            .expect("commit recorded envelope");
    }
    let replay_key = AxtHandleReplayKey::from_handle(dsid, &handle_fragment.handle);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let world = mem::replace(&mut state.world, World::new());
    let mut restarted =
        State::new_with_nexus_for_testing(world, nexus, LiveQueryStore::start_test());
    assert!(
        restarted
            .world
            .axt_replay_ledger
            .view()
            .get(&replay_key)
            .is_some(),
        "restart must retain the exact persisted replay row"
    );
    let authenticated_tip = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
    restarted.push_block_hash_for_testing(authenticated_tip.hash());
    restarted.update_latest_block_header_cache_for_tests(authenticated_tip);
    let_row! { mut host = CoreHost::from_state(authority.clone(), &restarted).expect("canonical state snapshots") };
    let mut vm = IVM::new(100_000);
    let desc_ptr = store_tlv_norito(&mut vm, PointerType::AxtDescriptor, &ivm_descriptor);
    vm.set_register(10, desc_ptr);
    IVMHost::syscall(&mut host, syscalls::SYSCALL_AXT_BEGIN, &mut vm).expect("axt begin");
    let ds_ptr = store_tlv_norito(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &ivm_manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    IVMHost::syscall(&mut host, syscalls::SYSCALL_AXT_TOUCH, &mut vm).expect("touch");
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &ivm_proof);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &ivm_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &ivm_intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    let_row! { err = IVMHost::syscall(&mut host, syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm) .expect_err("the permanent counter must reject the already-used nonce after restart") };
    let_row! { reject = host .take_axt_reject_for_tests() .expect("reject context recorded") };
    assert!(matches!(err, ivm::VMError::PermissionDenied));
    assert_eq!(reject.reason, AxtRejectReason::SubNonce);
    assert_eq!(reject.dataspace, Some(dsid));
    assert_eq!(reject.lane, Some(lane));
    assert_eq!(
        reject.active_handle_era,
        Some(active_policy.active_handle_era)
    );
    assert_eq!(
        reject.next_handle_counter,
        active_policy.next_handle_counter.checked_add(1)
    );
}
#[test]
#[allow(clippy::too_many_lines)]
fn axt_replay_ledger_prunes_after_retention_window() {
    let authority = ALICE_ID.clone();
    let dsid = DataSpaceId::new(72);
    let lane = LaneId::new(0);
    let (world, issuer, issuer_uaid, manifest_root, asset_definition_id, incarnation) =
        authenticated_axt_replay_world(dsid, 0x55);
    let_row! { lane_catalog = LaneCatalog::new( nonzero!(1_u32), vec![public_lane!(lane, dsid, "primary".to_owned())], ) .expect("lane catalog") };
    let_row! { mut nexus = iroha_config::parameters::actual::Nexus { lane_catalog: lane_catalog.clone(), lane_config: iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog), dataspace_catalog: dataspace_catalog_for_lane_catalog(&lane_catalog), routing_policy: LaneRoutingPolicy { default_lane: lane, default_dataspace: dsid, ..Default::default() }, ..Default::default() } };
    nexus.axt.slot_length_ms = NonZeroU64::new(1).expect("slot length");
    nexus.axt.replay_retention_slots = NonZeroU64::new(2).expect("retention");
    let query_handle = LiveQueryStore::start_test();
    let mut state = State::new_with_nexus_for_testing(world, nexus, query_handle);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let_row! { descriptor = AxtDescriptor { dsids: vec![dsid], touches: vec![AxtTouchSpec { dsid, read: vec!["orders/replay".into()], write: vec!["ledger/replay".into()], }], } };
    let binding = descriptor.binding().expect("compute binding");
    let_row! { touch_manifest = TouchManifest { read: vec!["orders/replay".into()], write: vec!["ledger/replay".into()], } };
    let issuer_context = AxtHandleIssuerContextV1 {
        network_id: state.network_id,
        asset_dsid: dsid,
        asset_definition_incarnation: incarnation,
        issuer: issuer_uaid,
        issuer_manifest_root: manifest_root,
        code_root: [0; 32],
        abi_version: 1,
        abi_hash: ivm::syscalls::compute_abi_hash(ivm::SyscallPolicy::AbiV1),
    };
    let handle = signed_axt_handle_for_block_freeze(&issuer, issuer_context, lane, binding, 1, 1);
    assert_eq!(handle.asset_definition_id, asset_definition_id);
    let_row! { handle_fragment = AxtHandleFragment { handle, intent: RemoteSpendIntent { asset_dsid: dsid, op: SpendOp { asset_definition_id, kind: "transfer".into(), from: authority.to_string(), to: BOB_ID.to_string(), amount: Some(Quantity::from(5_u64)), }, }, proof: None, amount: Some(Quantity::from(5_u64)), amount_commitment: None, } };
    let_row! { proof_fragment = AxtProofFragment { dsid, proof: axt_proof_blob_for_remote_spend( dsid, manifest_root, b"state-retention", 200, &handle_fragment, &Quantity::from(5_u64), ), } };
    let_row! { ivm_descriptor = ivm::axt::AxtDescriptor { dsids: descriptor.dsids.clone(), touches: descriptor .touches .iter() .map(|touch| ivm::axt::AxtTouchSpec { dsid: touch.dsid, read: touch.read.clone(), write: touch.write.clone(), }) .collect(), } };
    let_row! { ivm_manifest = ivm::axt::TouchManifest { read: touch_manifest.read.clone(), write: touch_manifest.write.clone(), } };
    let_row! { ivm_proof = ivm::axt::ProofBlob { payload: proof_fragment.proof.payload.clone(), expiry_slot: proof_fragment.proof.expiry_slot, } };
    let_row! { ivm_handle = ivm::axt::AssetHandle { scope: handle_fragment.handle.scope.clone(), asset_definition_id: handle_fragment.handle.asset_definition_id.clone(), subject: ivm::axt::HandleSubject { account: handle_fragment.handle.subject.account.clone(), origin_dsid: handle_fragment.handle.subject.origin_dsid, }, budget: ivm::axt::HandleBudget { remaining: handle_fragment.handle.budget.remaining.clone(), per_use: handle_fragment.handle.budget.per_use.clone(), }, handle_era: handle_fragment.handle.handle_era, sub_nonce: handle_fragment.handle.sub_nonce, group_binding: ivm::axt::GroupBinding { composability_group_id: handle_fragment .handle .group_binding .composability_group_id .clone(), epoch_id: handle_fragment.handle.group_binding.epoch_id, }, target_lane: handle_fragment.handle.target_lane, axt_binding: handle_fragment.handle.axt_binding.as_bytes().to_vec(), manifest_view_root: handle_fragment.handle.manifest_view_root.to_vec(), expiry_slot: handle_fragment.handle.expiry_slot, max_clock_skew_ms: handle_fragment.handle.max_clock_skew_ms, issuer_context: handle_fragment.handle.issuer_context, issuer_signature: handle_fragment.handle.issuer_signature.clone(), } };
    let_row! { ivm_intent = ivm::axt::RemoteSpendIntent { asset_dsid: handle_fragment.intent.asset_dsid, op: ivm::axt::SpendOp { asset_definition_id: handle_fragment.intent.op.asset_definition_id.clone(), kind: handle_fragment.intent.op.kind.clone(), from: handle_fragment.intent.op.from.clone(), to: handle_fragment.intent.op.to.clone(), amount: handle_fragment.intent.op.amount.clone(), }, } };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 1, 0);
    let mut block = state.block(header);
    {
        let mut stx = block.transaction();
        stx.current_lane_id = Some(lane);
        stx.record_axt_envelope(AxtEnvelopeRecord {
            binding,
            lane,
            descriptor: descriptor.clone(),
            touches: vec![AxtTouchFragment {
                dsid,
                manifest: touch_manifest.clone(),
            }],
            proofs: vec![proof_fragment.clone()],
            handles: vec![handle_fragment.clone()],
            commit_height: 1,
        })
        .expect("exact AXT sequence should stage");
        stx.apply();
    }
    block
        .commit_world_overlay_for_testing()
        .expect("commit recorded envelope before pruning");
    let replay_key = AxtHandleReplayKey::from_handle(dsid, &handle_fragment.handle);
    let budget_key = AxtHandleBudgetKey::from_handle(&handle_fragment.handle);
    let counter_before_prune = state
        .world
        .axt_handle_counters
        .view()
        .get(&dsid)
        .copied()
        .expect("permanent handle counter recorded");
    let budget_before_prune = state
        .world
        .axt_handle_budget_ledger
        .view()
        .get(&budget_key)
        .cloned()
        .expect("permanent handle budget recorded");
    assert_eq!(counter_before_prune.authorization_generation(), 1);
    assert_eq!(counter_before_prune.next(), 2);
    assert_eq!(budget_before_prune.consumed(), &Quantity::from(5_u64));
    let retention_slots = state.nexus_snapshot().axt.replay_retention_slots.get();
    let_row! { entry = state .world .axt_replay_ledger .view() .iter() .next() .map(|(_, entry)| entry.clone()) .expect("replay ledger populated") };
    let_row! { prune_at = entry .retain_until_slot .max(entry.used_slot.saturating_add(retention_slots)) .saturating_add(1) };
    state.prune_axt_replay_ledger_for_tests(prune_at, retention_slots);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root,
            target_lane: lane,
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: prune_at,
        },
    );
    assert!(
        state
            .world
            .axt_replay_ledger
            .view()
            .get(&replay_key)
            .is_none(),
        "ledger entries should be pruned after retention window"
    );
    assert_eq!(
        state.world.axt_handle_counters.view().get(&dsid),
        Some(&counter_before_prune),
        "replay pruning must not reset the permanent handle counter"
    );
    assert_eq!(
        state.world.axt_handle_budget_ledger.view().get(&budget_key),
        Some(&budget_before_prune),
        "replay pruning must not reset the permanent family budget"
    );
    let authenticated_tip = BlockHeader::new(nonzero!(1_u64), None, None, None, prune_at, 0);
    state.push_block_hash_for_testing(authenticated_tip.hash());
    state.update_latest_block_header_cache_for_tests(authenticated_tip);
    let projected = state.axt_policy_snapshot();
    let projected_policy = projected
        .entries
        .iter()
        .find(|binding| binding.dsid == dsid)
        .expect("requested policy remains projected")
        .policy;
    assert_eq!(projected_policy.active_handle_era, 1);
    assert_eq!(projected_policy.next_handle_counter, 2);
    let_row! { mut host = CoreHost::from_state(authority.clone(), &state).expect("canonical state snapshots") };
    let mut vm = IVM::new(100_000);
    let desc_ptr = store_tlv_norito(&mut vm, PointerType::AxtDescriptor, &ivm_descriptor);
    vm.set_register(10, desc_ptr);
    IVMHost::syscall(&mut host, syscalls::SYSCALL_AXT_BEGIN, &mut vm).expect("axt begin");
    let ds_ptr = store_tlv_norito(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &ivm_manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    IVMHost::syscall(&mut host, syscalls::SYSCALL_AXT_TOUCH, &mut vm).expect("touch");
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &ivm_proof);
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &ivm_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &ivm_intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    let err = IVMHost::syscall(&mut host, syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect_err("replay pruning must not revive a stale sub-nonce");
    assert!(matches!(err, ivm::VMError::PermissionDenied));
    let reject = host
        .take_axt_reject_for_tests()
        .expect("structured stale-subnonce rejection recorded");
    assert_eq!(reject.reason, AxtRejectReason::SubNonce);
    assert_eq!(reject.dataspace, Some(dsid));
    assert_eq!(reject.lane, Some(lane));
    assert_eq!(reject.active_handle_era, Some(1));
    assert_eq!(reject.next_handle_counter, Some(2));
    assert_eq!(
        state.world.axt_handle_counters.view().get(&dsid),
        Some(&counter_before_prune)
    );
    assert_eq!(
        state.world.axt_handle_budget_ledger.view().get(&budget_key),
        Some(&budget_before_prune)
    );
    let header = BlockHeader::new(nonzero!(2_u64), None, None, None, prune_at, 0);
    let mut block = state.block(header);
    let mut transaction = block.transaction();
    transaction.current_lane_id = Some(lane);
    let error = transaction
        .record_axt_envelope(AxtEnvelopeRecord {
            binding,
            lane,
            descriptor,
            touches: vec![AxtTouchFragment {
                dsid,
                manifest: touch_manifest,
            }],
            proofs: vec![proof_fragment],
            handles: vec![handle_fragment],
            commit_height: 2,
        })
        .expect_err("state persistence must reject the same stale sub-nonce");
    assert!(error.to_string().contains("sub-nonce mismatch"));
    drop(transaction);
    assert_eq!(
        block.world.axt_handle_counters.get(&dsid),
        Some(&counter_before_prune)
    );
    assert_eq!(
        block.world.axt_handle_budget_ledger.get(&budget_key),
        Some(&budget_before_prune)
    );
}

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

state_test! { sync axt_slot_uses_authenticated_time_for_hash_only_snapshot_parent
    fn hash_only_state() -> State {
        let state = blank_state();
        seed_committed_height_for_state_test(&state, 5);
        state
    }

    let unavailable = hash_only_state();
    assert!(unavailable.latest_block_header_fast().is_none());
    let unavailable_view = unavailable.view();
    assert_eq!(
        crate::smartcontracts::ivm::host::current_axt_slot_for_state(&unavailable_view),
        None,
        "a non-genesis hash-only view without authenticated time must fail closed"
    );
    assert!(matches!(
        crate::smartcontracts::ivm::host::CoreHost::from_state(
            ALICE_ID.clone(),
            &unavailable
        ),
        Err(crate::smartcontracts::ivm::host::CoreHostStateError::AxtPolicySnapshot(
            iroha_data_model::nexus::AxtPolicySnapshotValidationError::AuthenticatedLedgerTimeUnavailable
        ))
    ));
    drop(unavailable_view);

    let mut anchored = hash_only_state();
    anchored.nexus.get_mut().axt.slot_length_ms = nonzero!(10_u64);
    let parameters = crate::kagemusha_v1_test_fixtures::genesis_context_parameters();
    let mut mint_finality_voters = (1_u8..=4)
        .map(|seed| {
            let key_pair = iroha_crypto::KeyPair::try_from_seed(
                vec![seed; 32],
                iroha_crypto::Algorithm::BlsNormal,
            )
            .expect("derive deterministic snapshot mint-finality validator");
            iroha_data_model::block::consensus_v2::ValidatorPower {
                validator: iroha_data_model::peer::PeerId::new(key_pair.public_key().clone()),
                power: 1,
            }
        })
        .collect::<Vec<_>>();
    mint_finality_voters.sort_by(|left, right| left.validator.cmp(&right.validator));
    let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
        crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(
            anchored.network_id,
            0,
            &mint_finality_voters,
        );
    let snapshot_block_hash = anchored
        .latest_block_hash_fast()
        .expect("hash-only fixture has a committed tip");
    anchored.set_authenticated_snapshot_v2_bootstrap_for_testing(SnapshotV2BootstrapRecord {
        version: SnapshotV2BootstrapRecord::VERSION,
        context: HeightContext {
            network_id: anchored.network_id,
            protocol_version: PROTOCOL_VERSION,
            height: 6,
            epoch: 0,
            epoch_end_height: 6,
            next_epoch_snapshot: None,
            mode: ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: Some(
                iroha_data_model::block::consensus_v2::SnapshotBootstrapAnchor {
                    snapshot_height: 5,
                    snapshot_block_hash,
                    snapshot_block_creation_time_ms: 10_000,
                    snapshot_state_hash: Hash::new(b"hash-only-axt-time"),
                },
            ),
            roster: Vec::new(),
            quorum: DualQuorum {
                min_signers: 0,
                total_power: 0,
            },
            kagemusha_mint_finality_epoch_id,
            kagemusha_mint_finality_epoch_roster,
            nexus_amx_context_hash: Hash::prehashed(parameters.nexus_amx_context_hash),
            execution_policy_hash: Hash::prehashed(parameters.execution_policy_hash),
            da_layout: parameters.da_layout,
            leader_seed: [0; 32],
        },
        validator_set_pops: Vec::new(),
    });
    let stale_prefix_header = BlockHeader::new(nonzero!(1_u64), None, None, None, 99, 0);
    anchored.update_latest_block_header_cache_for_tests(stale_prefix_header);
    assert_eq!(anchored.latest_block_creation_time_ms_fast(), Some(10_000));
    let anchored_view = anchored.view();
    assert_eq!(anchored_view.query_ledger_time_ms(), 10_000);
    assert_eq!(anchored_view.authenticated_query_ledger_time_ms(), Some(10_000));
    assert_eq!(
        crate::smartcontracts::ivm::host::current_axt_slot_for_state(&anchored_view),
        Some(1_000),
        "AXT expiry must use the authenticated tip anchor, never height 5 or a stale cached prefix header"
    );
}
