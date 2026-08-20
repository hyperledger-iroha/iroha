// State-backed AXT envelope and replay regressions included by the parent test crate.

#[cfg(feature = "app_api")]
#[test]
fn core_host_exports_axt_envelopes_to_state_block() {
    let authority = fixture_authority();
    let lane = LaneId::new(3);
    let dsid = DataSpaceId::new(21);
    let manifest_root = [0xCC; 32];
    let mut vm = IVM::new(1_000_000);
    let mut host = host_with_policy(authority.clone(), dsid, manifest_root, lane, 4);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders".into()],
            write: vec!["ledger".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let touch_manifest = TouchManifest {
        read: vec!["orders/1".into()],
        write: vec!["ledger/1".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &touch_manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle = signed_abi_handle(
        AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(50_u64),
                per_use: Some(Quantity::from(50_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: binding.to_vec(),
            manifest_view_root: manifest_root.to_vec(),
            expiry_slot: 10,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid,
    );
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle);
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1]).expect("valid AXT fixture asset id"),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let proof = proof_blob_for_remote_spend(
        dsid,
        manifest_root,
        vec![0xAA, 0xBB, 0xCC],
        20,
        &handle,
        &intent,
        &Quantity::from(5_u64),
    );
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof");
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect("use handle");
    host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm)
        .expect("commit");
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(lane);
    let queued = host
        .apply_queued(&mut stx, &authority)
        .expect("apply queued");
    assert!(queued.is_empty());
    stx.apply();
    let envelopes = block.axt_envelopes();
    assert_eq!(envelopes.len(), 1);
    let record = &envelopes[0];
    assert_eq!(record.lane, lane);
    assert_eq!(record.commit_height, 1);
    assert_eq!(record.descriptor.dsids, descriptor.dsids);
    assert_eq!(record.touches.len(), 1);
    assert_eq!(record.proofs.len(), 1);
    assert_eq!(record.handles.len(), 1);
    assert_eq!(
        record.handles[0].intent.op.amount,
        Some(Quantity::from(5_u64))
    );
    let drained = block.drain_axt_envelopes();
    assert_eq!(drained.len(), 1);
    assert!(block.axt_envelopes().is_empty());
}
#[test]
fn core_host_rejects_cached_proof_after_manifest_rotation() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(55);
    let entries_current = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root: [0x11; 32],
            target_lane: LaneId::new(1),
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 7,
        },
    }];
    let snapshot_current = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries_current),
        entries: entries_current,
    };
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot_current)
        .expect("current AXT policy snapshot should be canonical");
    let mut vm = IVM::new(1_000_000);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/cache".into()],
            write: vec!["ledger/cache".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/cache".into()],
        write: vec!["ledger/cache".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let proof_current = proof_blob_for(dsid, [0x11; 32], b"manifest-v1".to_vec(), 20);
    let proof_v1_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_current);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_v1_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof matches manifest v1");
    let entries_v2 = vec![AxtPolicyBinding {
        dsid,
        policy: AxtPolicyEntry {
            manifest_root: [0x22; 32],
            target_lane: LaneId::new(1),
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 7,
        },
    }];
    let snapshot_v2 = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries_v2),
        entries: entries_v2,
    };
    let mut noncanonical_snapshot_v2 = snapshot_v2.clone();
    let expected_v2 = noncanonical_snapshot_v2.version;
    noncanonical_snapshot_v2.version = expected_v2.wrapping_add(1);
    let advertised_v2 = noncanonical_snapshot_v2.version;
    assert!(matches!(
        host.refresh_axt_policy_snapshot(&noncanonical_snapshot_v2),
        Err(AxtPolicySnapshotValidationError::VersionMismatch {
            expected,
            actual,
        }) if expected == expected_v2 && actual == advertised_v2
    ));
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_v1_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("rejected refresh must preserve the current policy and proof cache");
    assert!(
        host.axt_recorded_proof_payload(dsid).is_some(),
        "rejected refresh must preserve the active envelope"
    );
    host.refresh_axt_policy_snapshot(&snapshot_v2)
        .expect("rotated AXT policy snapshot should be canonical");
    assert!(
        host.axt_recorded_proof_payload(dsid).is_none(),
        "successful refresh must abort proofs recorded under the prior policy"
    );
    assert!(
        host.axt_cached_proof_status(dsid).is_none(),
        "successful refresh must clear proofs cached under the prior policy"
    );
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Err(VMError::PermissionDenied),
        "an envelope accepted under the prior policy must not commit"
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("restart envelope after policy refresh");
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch after policy refresh");
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_v1_ptr);
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm),
        Err(VMError::PermissionDenied)
    );
    let proof_v2 = proof_blob_for(dsid, [0x22; 32], b"manifest-v2".to_vec(), 20);
    let proof_v2_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_v2);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_v2_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm));
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm));
}
#[test]
fn core_host_timing_change_aborts_active_envelope() {
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(56);
    let manifest_root = [0x33; 32];
    let snapshot = make_policy_snapshot(dsid, manifest_root, LaneId::new(1), 1, 1, 7);
    let mut host = CoreHost::new(authority)
        .with_axt_policy_snapshot(&snapshot)
        .expect("AXT policy snapshot should be canonical");
    let mut vm = IVM::new(1_000_000);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/timing".into()],
            write: vec!["ledger/timing".into()],
        }],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/timing".into()],
        write: vec!["ledger/timing".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let proof = proof_blob_for(dsid, manifest_root, b"timing-change".to_vec(), 20);
    let proof_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof matches current timing");
    assert!(host.axt_recorded_proof_payload(dsid).is_some());
    let timing = ActualAxtTiming {
        slot_length_ms: NonZeroU64::new(2).expect("slot length"),
        max_clock_skew_ms: 1,
        proof_cache_ttl_slots: NonZeroU64::new(1).expect("ttl slots"),
        replay_retention_slots: NonZeroU64::new(1).expect("replay slots"),
    };
    host.set_axt_timing(timing)
        .expect("replacement timing should accept the installed snapshot");
    assert!(
        host.axt_recorded_proof_payload(dsid).is_none(),
        "timing replacement must abort proofs recorded under the prior timing"
    );
    assert!(
        host.axt_cached_proof_status(dsid).is_none(),
        "timing replacement must clear proofs cached under the prior timing"
    );
    assert_eq!(
        host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm),
        Err(VMError::PermissionDenied),
        "an envelope accepted under the prior timing must not commit"
    );
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("restart envelope after timing replacement");
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch after timing replacement");
    vm.set_register(10, ds_ptr);
    vm.set_register(11, proof_ptr);
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm));
    assert_ok_gas!(host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm));
}
#[cfg(feature = "app_api")]
#[test]
fn core_host_records_multi_dataspace_envelope() {
    let authority = fixture_authority();
    let dsid_a = DataSpaceId::new(31);
    let dsid_b = DataSpaceId::new(32);
    let entries = vec![
        AxtPolicyBinding {
            dsid: dsid_a,
            policy: AxtPolicyEntry {
                manifest_root: [0xA1; 32],
                target_lane: LaneId::new(1),
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 0,
            },
        },
        AxtPolicyBinding {
            dsid: dsid_b,
            policy: AxtPolicyEntry {
                manifest_root: [0xB2; 32],
                target_lane: LaneId::new(2),
                active_handle_era: 1,
                next_handle_counter: 1,
                current_slot: 0,
            },
        },
    ];
    let snapshot = AxtPolicySnapshot {
        version: AxtPolicySnapshot::compute_version(&entries),
        entries,
    };
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("fixture AXT policy snapshot should be canonical");
    configure_axt_test_host(&mut host, [(dsid_a, [0xA1; 32]), (dsid_b, [0xB2; 32])]);
    let mut vm = IVM::new(1_000_000);
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid_a, dsid_b],
        touches: vec![
            axt::AxtTouchSpec {
                dsid: dsid_a,
                read: vec!["orders/a".into()],
                write: vec!["ledger/a".into()],
            },
            axt::AxtTouchSpec {
                dsid: dsid_b,
                read: vec!["orders/b".into()],
                write: vec!["ledger/b".into()],
            },
        ],
    };
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_a_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid_a);
    let ds_b_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid_b);
    let manifest_a = TouchManifest {
        read: vec!["orders/a/touch".into()],
        write: vec!["ledger/a/touch".into()],
    };
    let manifest_b = TouchManifest {
        read: vec!["orders/b/touch".into()],
        write: vec!["ledger/b/touch".into()],
    };
    let manifest_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest_a);
    let manifest_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest_b);
    vm.set_register(10, ds_a_ptr);
    vm.set_register(11, manifest_a_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch a");
    vm.set_register(10, ds_b_ptr);
    vm.set_register(11, manifest_b_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch b");
    let binding = axt::compute_binding(&descriptor).expect("binding");
    let handle_a = signed_abi_handle(
        AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid_a),
            },
            budget: HandleBudget {
                remaining: Quantity::from(80_u64),
                per_use: Some(Quantity::from(80_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(1),
            axt_binding: binding.to_vec(),
            manifest_view_root: vec![0xA1; 32],
            expiry_slot: 50,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid_a,
    );
    let handle_b = signed_abi_handle(
        AssetHandle {
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid_b),
            },
            budget: HandleBudget {
                remaining: Quantity::from(60_u64),
                per_use: Some(Quantity::from(60_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0; 32],
                epoch_id: 1,
            },
            target_lane: LaneId::new(2),
            axt_binding: binding.to_vec(),
            manifest_view_root: vec![0xB2; 32],
            expiry_slot: 50,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        dsid_b,
    );
    let intent_a = RemoteSpendIntent {
        asset_dsid: dsid_a,
        op: SpendOp {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1]).expect("valid AXT fixture asset id"),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(10_u64)),
        },
    };
    let intent_b = RemoteSpendIntent {
        asset_dsid: dsid_b,
        op: SpendOp {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1]).expect("valid AXT fixture asset id"),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_VENDOR_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(15_u64)),
        },
    };
    let proof_a = proof_blob_for_remote_spend(
        dsid_a,
        [0xA1; 32],
        b"multi-ds-a".to_vec(),
        50,
        &handle_a,
        &intent_a,
        &Quantity::from(10_u64),
    );
    let proof_b = proof_blob_for_remote_spend(
        dsid_b,
        [0xB2; 32],
        b"multi-ds-b".to_vec(),
        50,
        &handle_b,
        &intent_b,
        &Quantity::from(15_u64),
    );
    let proof_a_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_a);
    let proof_b_ptr = store_tlv_norito(&mut vm, PointerType::ProofBlob, &proof_b);
    vm.set_register(10, ds_a_ptr);
    vm.set_register(11, proof_a_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof a");
    vm.set_register(10, ds_b_ptr);
    vm.set_register(11, proof_b_ptr);
    host.syscall(ivm::syscalls::SYSCALL_VERIFY_DS_PROOF, &mut vm)
        .expect("proof b");
    let handle_a_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_a);
    let intent_a_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_a);
    vm.set_register(10, handle_a_ptr);
    vm.set_register(11, intent_a_ptr);
    vm.set_register(12, proof_a_ptr);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect("use handle a");
    let handle_b_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &handle_b);
    let intent_b_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent_b);
    vm.set_register(10, handle_b_ptr);
    vm.set_register(11, intent_b_ptr);
    vm.set_register(12, proof_b_ptr);
    host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm)
        .expect("use handle b");
    host.syscall(ivm::syscalls::SYSCALL_AXT_COMMIT, &mut vm)
        .expect("commit");
    let kura = Kura::blank_kura_for_testing();
    let query_handle = LiveQueryStore::start_test();
    let state = State::new_for_testing(World::new(), kura, query_handle);
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(LaneId::new(1));
    let queued = host
        .apply_queued(&mut stx, &authority)
        .expect("apply queued");
    assert!(queued.is_empty());
    stx.apply();
    let envelopes = block.axt_envelopes();
    assert_eq!(envelopes.len(), 1);
    let record = &envelopes[0];
    assert_eq!(record.descriptor.dsids.len(), 2);
    assert_eq!(record.touches.len(), 2);
    assert_eq!(record.proofs.len(), 2);
    assert_eq!(record.handles.len(), 2);
}
#[cfg(feature = "app_api")]
#[test]
fn axt_sub_nonce_floor_persists_across_restart() {
    use iroha_data_model::nexus::{
        AssetHandle as ModelAssetHandle, AxtEnvelopeRecord as ModelAxtEnvelopeRecord,
        AxtHandleFragment as ModelAxtHandleFragment, AxtProofFragment as ModelAxtProofFragment,
        AxtTouchFragment as ModelAxtTouchFragment, GroupBinding as ModelGroupBinding,
        HandleBudget as ModelHandleBudget, HandleSubject as ModelHandleSubject, LaneConfig,
        RemoteSpendIntent as ModelRemoteSpendIntent, SpendOp as ModelSpendOp,
        TouchManifest as ModelTouchManifest,
    };
    let authority = fixture_authority();
    let dsid = DataSpaceId::new(44);
    let world = World::new();
    let lane_meta = LaneConfig {
        id: LaneId::new(0),
        dataspace_id: dsid,
        alias: "primary".to_owned(),
        ..LaneConfig::default()
    };
    let lane_catalog = LaneCatalog::new(nonzero!(1_u32), vec![lane_meta]).expect("catalog");
    let nexus = nexus_with_lane_catalog(lane_catalog);
    let kura = Kura::blank_kura_for_testing();
    let query = LiveQueryStore::start_test();
    let mut state = State::new_for_testing(world, kura, query);
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root: [0x44; 32],
            target_lane: LaneId::new(0),
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    state
        .set_nexus(nexus)
        .expect("apply Nexus catalog for policy refresh");
    state.set_axt_policy(
        dsid,
        AxtPolicyEntry {
            manifest_root: [0x44; 32],
            target_lane: LaneId::new(0),
            active_handle_era: 1,
            next_handle_counter: 1,
            current_slot: 0,
        },
    );
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: vec![axt::AxtTouchSpec {
            dsid,
            read: vec!["orders/replay".into()],
            write: vec!["ledger/replay".into()],
        }],
    };
    let manifest_root = [0x44; 32];
    let binding_bytes = axt::compute_binding(&descriptor).expect("binding");
    let binding = iroha_data_model::nexus::AxtBinding::new(binding_bytes);
    let envelope = ModelAxtEnvelopeRecord {
        binding,
        lane: LaneId::new(0),
        descriptor: iroha_data_model::nexus::AxtDescriptor {
            dsids: descriptor.dsids.clone(),
            touches: descriptor
                .touches
                .iter()
                .map(|t| iroha_data_model::nexus::AxtTouchSpec {
                    dsid: t.dsid,
                    read: t.read.clone(),
                    write: t.write.clone(),
                })
                .collect(),
        },
        touches: vec![ModelAxtTouchFragment {
            dsid,
            manifest: ModelTouchManifest {
                read: vec!["orders/replay".into()],
                write: vec!["ledger/replay".into()],
            },
        }],
        proofs: vec![ModelAxtProofFragment {
            dsid,
            proof: model_proof_blob_for(dsid, manifest_root, b"sub-nonce-floor", 10),
        }],
        handles: vec![ModelAxtHandleFragment {
            handle: ModelAssetHandle {
                scope: vec!["transfer".into()],
                subject: ModelHandleSubject {
                    account: authority.to_string(),
                    origin_dsid: Some(dsid),
                },
                budget: ModelHandleBudget {
                    remaining: Quantity::from(50_u64),
                    per_use: Some(Quantity::from(50_u64)),
                },
                handle_era: 2,
                sub_nonce: 5,
                group_binding: ModelGroupBinding {
                    composability_group_id: vec![0; 32],
                    epoch_id: 2,
                },
                target_lane: LaneId::new(0),
                axt_binding: binding,
                manifest_view_root: manifest_root,
                expiry_slot: 50,
                max_clock_skew_ms: Some(0),
                issuer_context: Default::default(),
                issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
            },
            intent: ModelRemoteSpendIntent {
                asset_dsid: dsid,
                op: ModelSpendOp {
                    asset_definition_id: iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1]).expect("valid AXT fixture asset id"),
                    kind: "transfer".into(),
                    from: authority.to_string(),
                    to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
                    amount: Some(Quantity::from(10_u64)),
                },
            },
            proof: None,
            amount: Some(Quantity::from(10_u64)),
            amount_commitment: None,
        }],
        commit_height: 1,
    };
    let header = BlockHeader::new(nonzero!(1_u64), None, None, None, 0, 0);
    let mut block = state.block(header);
    let mut stx = block.transaction();
    stx.current_lane_id = Some(LaneId::new(0));
    stx.record_axt_envelope(envelope)
        .expect("exact replay-ledger AXT sequence should stage");
    stx.apply();
    block
        .commit()
        .expect("commit replay envelope before restart");
    let view = state.view();
    let cached_policy = view
        .world()
        .axt_policies()
        .get(&dsid)
        .expect("policy cached");
    assert_eq!(cached_policy.next_handle_counter, 6);
    assert_eq!(cached_policy.active_handle_era, 2);
    let mut vm = IVM::new(1_000_000);
    let mut host = CoreHost::from_state(authority.clone(), &state)
        .expect("fixture state should produce a valid CoreHost");
    let desc_ptr = store_tlv_codec(&mut vm, PointerType::AxtDescriptor, &descriptor);
    vm.set_register(10, desc_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_BEGIN, &mut vm)
        .expect("begin");
    let ds_ptr = store_tlv_codec(&mut vm, PointerType::DataSpaceId, &dsid);
    let manifest = TouchManifest {
        read: vec!["orders/replay".into()],
        write: vec!["ledger/replay".into()],
    };
    let manifest_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &manifest);
    vm.set_register(10, ds_ptr);
    vm.set_register(11, manifest_ptr);
    host.syscall(ivm::syscalls::SYSCALL_AXT_TOUCH, &mut vm)
        .expect("touch");
    let stale_handle = AssetHandle {
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: Quantity::from(50_u64),
            per_use: Some(Quantity::from(50_u64)),
        },
        handle_era: 2,
        sub_nonce: 5,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 2,
        },
        target_lane: LaneId::new(0),
        axt_binding: binding_bytes.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 100,
        max_clock_skew_ms: Some(0),
        issuer_context: Default::default(),
        issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
    };
    let intent = RemoteSpendIntent {
        asset_dsid: dsid,
        op: SpendOp {
            asset_definition_id: iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1]).expect("valid AXT fixture asset id"),
            kind: "transfer".into(),
            from: authority.to_string(),
            to: FIXTURE_MERCHANT_ACCOUNT_LITERAL.into(),
            amount: Some(Quantity::from(5_u64)),
        },
    };
    let handle_ptr = store_tlv_norito(&mut vm, PointerType::AssetHandle, &stale_handle);
    let intent_ptr = store_tlv_norito(&mut vm, PointerType::NoritoBytes, &intent);
    vm.set_register(10, handle_ptr);
    vm.set_register(11, intent_ptr);
    vm.set_register(12, 0);
    assert!(matches!(
        host.syscall(ivm::syscalls::SYSCALL_USE_ASSET_HANDLE, &mut vm),
        Err(VMError::PermissionDenied)
    ));
}
