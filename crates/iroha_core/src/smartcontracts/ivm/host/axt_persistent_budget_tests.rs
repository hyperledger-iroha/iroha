#[test]
fn enforce_axt_policy_rejects_zero_snapshot_manifest_root() {
    crate::test_alias::ensure();
    let dsid = DataSpaceId::new(20);
    let lane = LaneId::new(1);
    let handle_manifest_root = [0x90; 32];
    let snapshot = make_policy_snapshot(dsid, [0; 32], 10);
    let authority: AccountId = fixture_account("alice");
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("canonical policy snapshot")
        .with_axt_policy(Arc::new(axt::AllowAllAxtPolicy));
    let asset_definition_id = fixture_axt_asset_definition_id();
    let usage = axt::HandleUsage {
        handle: AssetHandle {
            asset_definition_id: asset_definition_id.clone(),
            scope: vec!["transfer".into()],
            subject: HandleSubject {
                account: authority.to_string(),
                origin_dsid: Some(dsid),
            },
            budget: HandleBudget {
                remaining: Quantity::from(10_u64),
                per_use: Some(Quantity::from(10_u64)),
            },
            handle_era: 1,
            sub_nonce: 1,
            group_binding: GroupBinding {
                composability_group_id: vec![0xAA; 32],
                epoch_id: 1,
            },
            target_lane: lane,
            axt_binding: vec![0xAB; 32],
            manifest_view_root: handle_manifest_root.to_vec(),
            expiry_slot: 20,
            max_clock_skew_ms: Some(0),
            issuer_context: Default::default(),
            issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
        },
        intent: RemoteSpendIntent {
            asset_dsid: dsid,
            op: SpendOp {
                asset_definition_id,
                kind: "transfer".into(),
                from: authority.to_string(),
                to: fixture_account_literal("bob"),
                amount: Some(Quantity::from(5_u64)),
            },
        },
        proof: None,
        amount: Quantity::from(5_u64),
        amount_commitment: None,
    };

    assert_eq!(
        host.enforce_axt_policy(&usage),
        Err(VMError::PermissionDenied)
    );
    let rejection = host
        .take_axt_reject_for_tests()
        .expect("zero policy root rejection context");
    assert_eq!(rejection.reason, AxtRejectReason::Manifest);
    assert_eq!(rejection.dataspace, Some(dsid));
    assert_eq!(rejection.lane, Some(lane));
    assert_eq!(rejection.snapshot_version, Some(snapshot.version));
    assert_eq!(rejection.detail, "policy or handle manifest root is zeroed");
}
#[test]
fn snapshot_policy_accepts_base_plus_one_across_envelopes_but_rejects_gap() {
    crate::test_alias::ensure();
    let dsid = DataSpaceId::new(20);
    let lane = LaneId::new(2);
    let manifest_root = [0x90; 32];
    let base_counter = 5;
    let mut snapshot = make_policy_snapshot(dsid, manifest_root, 10);
    snapshot.entries[0].policy.target_lane = lane;
    snapshot.entries[0].policy.active_handle_era = 3;
    snapshot.entries[0].policy.next_handle_counter = base_counter;
    snapshot.version = AxtPolicySnapshot::compute_version(&snapshot.entries);

    let authority: AccountId = fixture_account("alice");
    let descriptor = axt::AxtDescriptor {
        dsids: vec![dsid],
        touches: Vec::new(),
    };
    let binding = axt::compute_binding(&descriptor).expect("AXT binding");
    let mut host = CoreHost::new(authority.clone())
        .with_axt_policy_snapshot(&snapshot)
        .expect("canonical policy snapshot");
    host.axt_state = Some(Arc::new(axt::HostAxtState::new(
        descriptor.clone(),
        binding,
    )));

    let base_handle = AssetHandle {
        asset_definition_id: fixture_axt_asset_definition_id(),
        scope: vec!["transfer".into()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: Quantity::from(20_u64),
            per_use: Some(Quantity::from(10_u64)),
        },
        handle_era: 3,
        sub_nonce: base_counter,
        group_binding: GroupBinding {
            composability_group_id: vec![0; 32],
            epoch_id: 1,
        },
        target_lane: lane,
        axt_binding: binding.to_vec(),
        manifest_view_root: manifest_root.to_vec(),
        expiry_slot: 20,
        max_clock_skew_ms: Some(0),
        issuer_context: Default::default(),
        issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
    };
    let usage_for = |sub_nonce| {
        let mut handle = base_handle.clone();
        handle.sub_nonce = sub_nonce;
        axt::HandleUsage {
            handle,
            intent: RemoteSpendIntent {
                asset_dsid: dsid,
                op: SpendOp {
                    asset_definition_id:
                        iroha_data_model::asset::AssetDefinitionId::from_uuid_bytes([
                            0, 0, 0, 0, 0, 0, 0x40, 0, 0x80, 0, 0, 0, 0, 0, 0, 1,
                        ])
                        .expect("valid AXT fixture asset id"),
                    kind: "transfer".into(),
                    from: authority.to_string(),
                    to: fixture_account_literal("bob"),
                    amount: Some(Quantity::from(5_u64)),
                },
            },
            proof: None,
            amount: Quantity::from(5_u64),
            amount_commitment: None,
        }
    };

    let base_usage = usage_for(base_counter);
    host.enforce_axt_policy(&base_usage)
        .expect("snapshot base counter must be accepted");
    Arc::make_mut(host.axt_state.as_mut().expect("active AXT state"))
        .record_handle(base_usage)
        .expect("record accepted base handle");

    let gap_usage = usage_for(base_counter + 2);
    assert_eq!(
        host.enforce_axt_policy(&gap_usage),
        Err(VMError::PermissionDenied),
        "snapshot counter gaps must remain rejected"
    );
    assert_eq!(
        host.take_axt_reject_for_tests()
            .expect("gap reject context")
            .reason,
        AxtRejectReason::SubNonce
    );

    let completed = host.axt_state.take().expect("completed AXT state");
    host.completed_axt
        .push(Arc::try_unwrap(completed).unwrap_or_else(|state| state.as_ref().clone()));
    host.axt_state = Some(Arc::new(axt::HostAxtState::new(descriptor, binding)));
    let next_usage = usage_for(base_counter + 1);
    host.enforce_axt_policy(&next_usage)
        .expect("snapshot base+1 counter must include a prior completed envelope");
}

fn establish_authenticated_axt_ledger_time(state: &State, creation_time_ms: u64) {
    let header = BlockHeader::new(
        nonzero_ext::nonzero!(1_u64),
        None,
        None,
        None,
        creation_time_ms,
        0,
    );
    let mut block_hashes = state.block_hashes.block();
    block_hashes.push_for_tests(header.hash());
    block_hashes.commit_for_tests();
    state.update_latest_block_header_cache_for_tests(header);
}

#[test]
fn hydrate_axt_state_installs_one_consistent_state_snapshot() {
    crate::test_alias::ensure();
    let dsid = DataSpaceId::UNIVERSAL;
    let lane = LaneId::new(0);
    let manifest_root = [0x6A; 32];
    let timing = iroha_config::parameters::actual::NexusAxt {
        slot_length_ms: NonZeroU64::new(7).expect("slot length"),
        max_clock_skew_ms: 3,
        proof_cache_ttl_slots: NonZeroU64::new(2).expect("proof cache ttl"),
        replay_retention_slots: NonZeroU64::new(4).expect("replay retention"),
    };
    let mut nexus = iroha_config::parameters::actual::Nexus::default();
    nexus.axt = timing;
    let state =
        State::new_with_nexus_for_testing(World::new(), nexus, LiveQueryStore::start_test());
    establish_authenticated_axt_ledger_time(&state, 7);
    let live_key = AxtHandleReplayKey::from_parts(
        dsid,
        fixture_axt_asset_incarnation(0x11),
        [0x11; 32],
        1,
        1,
        lane,
    );
    let expired_key = AxtHandleReplayKey::from_parts(
        dsid,
        fixture_axt_asset_incarnation(0x22),
        [0x22; 32],
        1,
        2,
        lane,
    );
    let live_record = AxtReplayRecord {
        dataspace: dsid,
        budget_key: fixture_axt_budget_key_for_replay_key(&live_key),
        used_slot: 1,
        retain_until_slot: 9,
    };
    let expired_record = AxtReplayRecord {
        dataspace: dsid,
        budget_key: fixture_axt_budget_key_for_replay_key(&expired_key),
        used_slot: 0,
        retain_until_slot: 0,
    };
    {
        let mut world = state.world.block();
        world.axt_policies.insert(
            dsid,
            AxtPolicyEntry {
                manifest_root,
                target_lane: lane,
                active_handle_era: 3,
                next_handle_counter: 5,
                current_slot: u64::MAX,
            },
        );
        world
            .axt_replay_ledger
            .insert(live_key, live_record.clone());
        world
            .axt_replay_ledger
            .insert(expired_key, expired_record.clone());
        world.commit();
    }
    let view = state.view();
    assert_eq!(
        view.world().axt_replay_ledger().get(&expired_key),
        Some(&expired_record),
        "the source view must retain the expired control so host-side filtering is tested"
    );
    let expected_snapshot = view.axt_policy_snapshot();
    assert_eq!(expected_snapshot.entries.len(), 1);
    assert_eq!(expected_snapshot.entries[0].policy.current_slot, 1);
    let authority: AccountId = fixture_account("alice");
    let (descriptor, binding) = axt::AxtDescriptor::builder()
        .dataspace(dsid)
        .build_with_binding()
        .expect("active AXT descriptor");
    let mut host = CoreHost::new(authority);
    host.axt_state = Some(Arc::new(axt::HostAxtState::new(descriptor, binding)));
    host.cache_proof_entry(
        dsid,
        Hash::new(b"stale proof cache").into(),
        Some(20),
        Some(1),
        Some(manifest_root),
        true,
        AXT_PROOF_CACHE_HIT,
    );
    assert!(!host.axt_proof_cache.is_empty());
    host.hydrate_axt_state(&view)
        .expect("hydrate canonical AXT state");
    assert_eq!(host.axt_timing, timing);
    assert_eq!(host.axt_policy_snapshot.as_ref(), Some(&expected_snapshot));
    assert_eq!(
        host.current_axt_policy_version(),
        Some(expected_snapshot.version)
    );
    assert_eq!(host.axt_replay_ledger.get(&live_key), Some(&live_record));
    assert!(!host.axt_replay_ledger.contains_key(&expired_key));
    assert!(
        host.axt_state.is_none(),
        "hydration must abort an active envelope"
    );
    assert!(
        host.axt_proof_cache.is_empty(),
        "hydration must clear proofs admitted under the prior snapshot"
    );
    assert_eq!(
        host.axt_proof_cache_slot,
        Some(1),
        "the cleared cache must be rebound to the installed policy slot"
    );
    assert!(matches!(
        host.axt_handle_budget_base,
        AxtBudgetBase::Owned(_)
    ));
}

fn persistent_budget_usage(
    family_seed: u8,
    amount: u64,
) -> (axt::HostAxtState, axt::HandleBudgetKey) {
    let dsid = DataSpaceId::new(44);
    let (descriptor, binding) = axt::AxtDescriptor::builder()
        .dataspace(dsid)
        .build_with_binding()
        .expect("budget descriptor");
    let mut state = axt::HostAxtState::new(descriptor, binding);
    state
        .record_touch(
            dsid,
            TouchManifest {
                read: Vec::new(),
                write: Vec::new(),
            },
        )
        .expect("empty declared touch");
    let authority = fixture_account("alice");
    let asset_definition_id = fixture_axt_asset_definition_id();
    let mut issuer_context = AxtHandleIssuerContextV1::default();
    issuer_context.asset_dsid = dsid;
    issuer_context.asset_definition_incarnation = fixture_axt_asset_incarnation(family_seed);
    let handle = AssetHandle {
        asset_definition_id: asset_definition_id.clone(),
        scope: vec!["transfer".to_owned()],
        subject: HandleSubject {
            account: authority.to_string(),
            origin_dsid: Some(dsid),
        },
        budget: HandleBudget {
            remaining: Quantity::from(10_u64),
            per_use: Some(Quantity::from(10_u64)),
        },
        handle_era: 1,
        sub_nonce: 1,
        group_binding: GroupBinding {
            composability_group_id: vec![family_seed; 32],
            epoch_id: u64::from(family_seed).saturating_add(1),
        },
        target_lane: LaneId::new(0),
        axt_binding: binding.to_vec(),
        manifest_view_root: vec![0x5A; 32],
        expiry_slot: 100,
        max_clock_skew_ms: Some(0),
        issuer_context,
        issuer_signature: iroha_crypto::Signature::from_bytes(&[1_u8; 64]),
    };
    let key = axt::try_handle_budget_key(dsid, &handle).expect("canonical budget family");
    state
        .record_handle(axt::HandleUsage {
            handle,
            intent: RemoteSpendIntent {
                asset_dsid: dsid,
                op: SpendOp {
                    asset_definition_id,
                    kind: "transfer".to_owned(),
                    from: authority.to_string(),
                    to: fixture_account_literal("bob"),
                    amount: Some(Quantity::from(amount)),
                },
            },
            proof: None,
            amount: Quantity::from(amount),
            amount_commitment: None,
        })
        .expect("record budget usage");
    (state, key)
}

fn consumed_budget_record(key: &axt::HandleBudgetKey, consumed: u64) -> AxtHandleBudgetRecord {
    let mut record = AxtHandleBudgetRecord::empty();
    record
        .try_consume(key, &Quantity::from(consumed), 100)
        .expect("seed durable budget consumption");
    record
}

fn state_with_persistent_budgets(
    rows: impl IntoIterator<Item = (axt::HandleBudgetKey, AxtHandleBudgetRecord)>,
) -> State {
    let state = State::new_for_testing(
        World::new(),
        Kura::blank_kura_for_testing(),
        LiveQueryStore::start_test(),
    );
    establish_authenticated_axt_ledger_time(&state, 1);
    {
        let mut world = state.world.block();
        for (key, record) in rows {
            world.axt_handle_budget_ledger.insert(key, record);
        }
        world.commit();
    }
    state
}

#[test]
fn axt_query_backed_hydration_loads_only_the_touched_budget_family() {
    let (first_usage, first_key) = persistent_budget_usage(0x31, 2);
    let (_, other_key) = persistent_budget_usage(0x32, 1);
    let state = state_with_persistent_budgets([
        (first_key.clone(), consumed_budget_record(&first_key, 7)),
        (other_key.clone(), consumed_budget_record(&other_key, 1)),
    ]);
    let view = state.view();
    let mut host: CoreHostImpl<QueryStateSlot<_>> = CoreHostImpl::new(fixture_account("alice"));
    host.hydrate_axt_state(&view)
        .expect("hydrate query-backed budget base");
    assert!(matches!(
        host.axt_handle_budget_base,
        AxtBudgetBase::QueryRequired
    ));
    assert!(host.axt_handle_budget_ledger.is_empty());
    host.set_query_state(&view);

    let updates = host
        .stage_axt_handle_budget_updates(&first_usage)
        .expect("seven plus two stays within the signed budget");
    assert_eq!(
        updates.get(&first_key).map(AxtHandleBudgetRecord::consumed),
        Some(&Quantity::from(9_u64))
    );
    host.commit_axt_handle_budget_updates(updates);
    assert_eq!(host.axt_handle_budget_ledger.len(), 1);
    assert!(host.axt_handle_budget_ledger.contains_key(&first_key));
    assert!(!host.axt_handle_budget_ledger.contains_key(&other_key));

    let (second_usage, second_key) = persistent_budget_usage(0x31, 2);
    assert_eq!(second_key, first_key);
    assert_eq!(
        host.stage_axt_handle_budget_updates(&second_usage),
        Err(VMError::PermissionDenied)
    );
    assert_eq!(
        host.axt_handle_budget_ledger[&first_key].consumed(),
        &Quantity::from(9_u64)
    );
}

#[test]
fn axt_query_required_budget_base_without_attached_state_fails_closed() {
    let (usage, key) = persistent_budget_usage(0x41, 1);
    let state = state_with_persistent_budgets([(key.clone(), consumed_budget_record(&key, 1))]);
    let view = state.view();
    let mut host: CoreHostImpl<QueryStateSlot<_>> = CoreHostImpl::new(fixture_account("alice"));
    host.hydrate_axt_state(&view)
        .expect("hydrate query-backed budget base");
    host.set_query_state(&view);
    host.query_state.state = None;

    assert_eq!(
        host.stage_axt_handle_budget_updates(&usage),
        Err(VMError::PermissionDenied)
    );
    assert_eq!(
        host.take_axt_reject_for_tests()
            .expect("missing query rejection context")
            .reason,
        AxtRejectReason::PolicyDenied
    );
    assert!(host.axt_handle_budget_ledger.is_empty());
}

#[test]
fn axt_no_query_hydration_owns_budget_base_and_view_base_is_not_an_effect() {
    let (usage, key) = persistent_budget_usage(0x51, 4);
    let state = state_with_persistent_budgets([(key.clone(), consumed_budget_record(&key, 7))]);
    let mut owned =
        CoreHost::from_state(fixture_account("alice"), &state).expect("hydrate owned budget base");
    assert!(matches!(
        &owned.axt_handle_budget_base,
        AxtBudgetBase::Owned(records) if records.len() == 1
    ));
    assert_eq!(
        owned.stage_axt_handle_budget_updates(&usage),
        Err(VMError::PermissionDenied)
    );
    assert!(owned.axt_handle_budget_ledger.is_empty());

    let view = state.view();
    let mut query_host: CoreHostImpl<QueryStateSlot<_>> =
        CoreHostImpl::new(fixture_account("alice"));
    query_host
        .hydrate_axt_state(&view)
        .expect("hydrate query-backed view base");
    query_host.set_query_state(&view);
    query_host.execution_class = HostExecutionClass::View;
    query_host
        .ensure_view_execution_has_no_effect_artifacts()
        .expect("an immutable durable budget base is not a mutable view artifact");
}
