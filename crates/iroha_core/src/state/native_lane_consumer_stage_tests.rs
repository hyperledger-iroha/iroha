// Same native source/kernel, actual pristine block stage and shared start hooks.
// No test grants global acceptance, publication or a native Apply completion.

fn native_consumer_stage_fixture(due_asset_policy_hook: bool) -> Box<NativeEconomicFixture> {
    native_economic_fixture_with_world_initializer(
        &[NativeEconomicCase::Transfer(25)],
        false,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
        None,
        |world| {
            if due_asset_policy_hook {
                use iroha_data_model::asset::definition::{
                    AssetConfidentialPolicy, ConfidentialPolicyMode, ConfidentialPolicyTransition,
                };
                let domain = DomainId::try_new("native-economics", "universal").unwrap();
                let id = AssetDefinitionId::derive_from_components(domain, "coin".parse().unwrap());
                let mut definition = world.asset_definitions.view().get(&id).unwrap().clone();
                let mut policy = AssetConfidentialPolicy::convertible();
                // This is a supported scheduled transition. Supply may remain
                // before H; the H-effective hook must abort if it still remains.
                policy.pending_transition = Some(ConfidentialPolicyTransition {
                    new_mode: ConfidentialPolicyMode::ShieldedOnly,
                    previous_mode: ConfidentialPolicyMode::Convertible,
                    effective_height: 7,
                    transition_id: Hash::new(b"native H7 policy transition"),
                    conversion_window: Some(1),
                });
                assert!(policy.pending_transition_is_valid());
                definition.set_confidential_policy(policy);
                world.asset_definitions.insert(id, definition);
                world
                    .rebuild_confidential_policy_transition_index()
                    .unwrap();
            }
        },
    )
}

fn native_consumer_stage_carrier(fixture: &NativeEconomicFixture) -> SignedBlock {
    let mut carrier =
        empty_global_block_after(Some(&fixture.native.block)).canonical_resultless_proposal();
    carrier.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
        &fixture.native.state.nexus_snapshot(),
        carrier.header().height().get(),
    )));
    let groups = native_economic_groups(fixture);
    let batch = fixture
        .native
        .state
        .prepare_lane_decision_batch(&groups)
        .unwrap();
    carrier.set_execution_context(Some(
        BlockExecutionContextBundle::default().with_native_lane_decisions(batch),
    ));
    let key = merge_carrier_finality_fixture_keypair();
    carrier
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(key.private_key(), carrier.hash()),
            ),
        ]))
        .unwrap();
    carrier
}

state_test! { sync native_consumer_stage_applies_h_effective_asset_policy_before_economics
    use super::{NativeLaneBatchReplayV1, NativeLaneBatchSourcePreparationV1};
    use iroha_data_model::asset::definition::ConfidentialPolicyMode;
    let fixture = native_consumer_stage_fixture(true);
    let state = &fixture.native.state;
    let carrier = native_consumer_stage_carrier(&fixture);
    assert_eq!(carrier.header().height().get(), 7);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let old_policy = *state.world.asset_definitions.view().get(fixture.source.definition()).unwrap().confidential_policy();
    assert_eq!(old_policy.mode, ConfidentialPolicyMode::Convertible);
    assert_eq!(old_policy.effective_mode(7), ConfidentialPolicyMode::ShieldedOnly,
        "without the actual due hook this input would be rejected at H");
    assert!(state.world.confidential_policy_transition_index.view().get(&(7,fixture.source.definition().clone())).is_some());
    let NativeLaneBatchReplayV1::Ready(scratch) = state.replay_proposed_native_lane_batch(&carrier, &[]).unwrap()
        else { panic!("authentic four-key sources need no applying QC"); };
    assert!(scratch.overlay().start_of_block_effects_applied);
    assert_eq!(scratch.overlay().retained_execution_outputs_for_test().unwrap().iter().filter(|row| matches!(row, iroha_data_model::block::execution_output::ExecutionOutputV1::Network(_))).count(), 1, "the actual native source consumes its terminal plan under the common producer");
    scratch.overlay().validate_native_lane_execution().unwrap();
    let exact_batch = scratch.batch().clone();
    let exact_roots = scratch.prefix_roots_for_test();
    let exact_result = scratch.executions()[0].result.clone();
    let exact_completed_root = scratch.overlay().merge_execution_write_set_root();
    assert!(scratch.executions()[0].result.is_ok(), "H-effective cancellation permits this exact transfer");
    drop(scratch);
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state.prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("actual current first carriers/Decisions prepare source authority"); };
    let NativeLaneBatchReplayV1::Ready(staged) = source.stage_with_start_hooks().unwrap()
        else { panic!("same base remains current"); };
    assert_eq!(staged.batch(), &exact_batch, "scratch and canonical staging run identical ordered hooks+native writes");
    assert_eq!(staged.prefix_roots_for_test(), exact_roots);
    assert_eq!(staged.executions()[0].result, exact_result);
    let overlay = staged.overlay();
    let policy=overlay.world.asset_definitions.get(fixture.source.definition()).unwrap().confidential_policy();
    assert_eq!(policy.mode,ConfidentialPolicyMode::Convertible);
    assert!(policy.pending_transition.is_none());
    assert!(overlay.world.confidential_policy_transition_index.get(&(7,fixture.source.definition().clone())).is_none());
    assert_eq!(overlay.world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(overlay.world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    overlay.validate_merge_carrier_entrypoint_binding().unwrap();
    overlay.validate_native_lane_execution().unwrap();
    assert_eq!(overlay.merge_execution_write_set_root(), exact_completed_root,
        "independent reexecution reproduces the common tail as well as the native prefix");
    assert_ne!(exact_completed_root, exact_roots.1,
        "the common tail owns additional runtime metadata after the native prefix");
    drop(staged);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before, "hook and native economics roll back together");
    assert_eq!(*state.world.asset_definitions.view().get(fixture.source.definition()).unwrap().confidential_policy(),old_policy);
    // A later private-prefix failure must discard the already-applied H hook too.
    let NativeLaneBatchSourcePreparationV1::Ready(source)=state.prepare_proposed_native_lane_batch_source(&carrier,&[]).unwrap() else {panic!("source still authentic")};
    let NativeLaneBatchReplayV1::Ready(mut altered)=source.stage_with_start_hooks().unwrap() else {panic!("same pre-State")};
    let path: iroha_model_base::state_path::StatePath="unbound_after_native_hook".parse().unwrap();
    altered.overlay_mut_for_test().world.smart_contract_state.insert(path,vec![1]);
    assert!(altered.overlay().validate_native_lane_execution().is_err());
    drop(altered);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
    assert_eq!(*state.world.asset_definitions.view().get(fixture.source.definition()).unwrap().confidential_policy(),old_policy);
    assert!(crate::block::ValidBlock::validate_inactive_native_carrier_for_test(&carrier).unwrap_err().to_string().contains("not active"));
}

state_test! { sync native_consumer_stage_rejects_membership_carrier_and_prefix_write_mutation
    let fixture = native_consumer_stage_fixture(false);
    let state = &fixture.native.state;
    let carrier = native_consumer_stage_carrier(&fixture);
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    for mutation in 0..5 {
        let mut prepared = state.prepare_native_batch_on_carrier(carrier.header(), groups.clone()).unwrap();
        let overlay = prepared.overlay_mut_for_test();
        overlay.validate_native_lane_execution().unwrap();
        match mutation {
            0 => { overlay.merge_carrier_entrypoints.insert(HashOf::from_untyped_unchecked(Hash::new(b"foreign membership"))); },
            1 => {
                let header = overlay._curr_block.clone();
                overlay._curr_block = BlockHeader::new(header.height(), header.prev_block_hash(), None,
                    u64::try_from(header.creation_time().as_millis()).unwrap()+1, header.view_change_index());
            },
            2 => {
                let key: iroha_model_base::state_path::StatePath = "native_stage_unbound_write".parse().unwrap();
                overlay.world.smart_contract_state.insert(key, vec![7]);
            },
            3 => overlay.staged_queue_plan_admissions.push(vec![7]),
            4 => overlay.applied_npos_consensus_effects_hash = Some(HashOf::from_untyped_unchecked(Hash::new(b"foreign pristine control"))),
            _ => unreachable!(),
        }
        assert!(overlay.validate_native_lane_execution().is_err(), "mutation {mutation}");
        if mutation != 2 { assert!(overlay.validate_merge_carrier_entrypoint_binding().is_err()); }
        drop(prepared);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
    }
}

state_test! { sync native_consumer_stage_compares_actual_alias_and_both_prefix_roots
    use super::{NativeLaneBatchSourcePreparationV1, NativeLaneBatchReplayV1};
    let fixture = native_economic_fixture_with_genesis_layout(&[NativeEconomicCase::Reveal(0)], false,
        Some(DataAvailabilityLayout {encoding: PayloadEncoding::ReedSolomon16, chunk_size_bytes: 8192,
            data_shards: 1, parity_shards: 1, max_payload_size_bytes: 2 * 1024 * 1024, max_chunk_count: 512}));
    let state = &fixture.native.state;
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let mut expected = None;
    for mutation in 0..3 {
        let NativeLaneBatchSourcePreparationV1::Ready(source) = state.prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap() else {panic!("ready")};
        let NativeLaneBatchReplayV1::Ready(mut staged) = source.stage_with_start_hooks().unwrap() else {panic!("staged")};
        let alias = staged.executions()[0].authenticated_signed_replay_alias.expect("actual sealed authentication owns its alias");
        let input = &staged.batch().groups[0].payload.input.entrypoint;
        let TransactionEntrypoint::SealedReveal(reveal) = input else {panic!("sealed fixture")};
        assert_eq!(alias,Hash::from(reveal.signed_transaction().hash()));
        let roots = staged.prefix_roots_for_test();
        assert_ne!(roots.0,roots.1,"actual replay markers follow the actual economic writes");
        if let Some(expected) = expected { assert_eq!(roots,expected); } else { expected=Some(roots); }
        let alias = HashOf::from_untyped_unchecked(alias);
        assert!(staged.overlay().merge_carrier_entrypoints.contains(&alias));
        staged.overlay().validate_merge_carrier_entrypoint_binding().unwrap();
        match mutation {
            0 => { staged.overlay_mut_for_test().merge_carrier_entrypoints.remove(&alias); },
            1 => { staged.overlay_mut_for_test().merge_carrier_entrypoints.insert(HashOf::from_untyped_unchecked(Hash::new(b"foreign alias"))); },
            2 => { let key:iroha_model_base::state_path::StatePath="foreign_native_suffix".parse().unwrap(); staged.overlay_mut_for_test().world.smart_contract_state.insert(key,vec![1]); },
            _ => unreachable!(),
        }
        assert!(staged.overlay().validate_native_lane_execution().is_err());
        if mutation<2 {assert!(staged.overlay().validate_merge_carrier_entrypoint_binding().is_err());}
        drop(staged);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
    }
}

state_test! { sync native_consumer_stage_cannot_publish_through_empty_old_merge_authorization
    use std::sync::atomic::{AtomicBool,Ordering};
    struct EmptyOldMergeAuthorization(Arc<AtomicBool>);
    impl StateBlockCommitAuthorization for EmptyOldMergeAuthorization {
        fn validate_for_state_commit(&self, _: HashOf<BlockHeader>, entry: Option<&MergeLedgerEntry>) -> Result<(),String> {
            assert!(entry.is_none()); self.0.store(true,Ordering::SeqCst); Ok(())
        }
    }
    let fixture = native_consumer_stage_fixture(true);
    let state = &fixture.native.state; let carrier = native_consumer_stage_carrier(&fixture);
    let groups = native_economic_groups(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let mut overlay = state.prepare_native_batch_on_carrier(carrier.header(), groups.clone()).unwrap().into_overlay_for_test();
    assert!(overlay.start_of_block_effects_applied);
    overlay.stage_canonical_carrier_membership(Vec::new(),NonZeroUsize::new(carrier.header().height().get() as usize).unwrap()).unwrap();
    let called=Arc::new(AtomicBool::new(false));
    assert!(matches!(
        overlay.commit_with_state_commit_authorization(Box::new(EmptyOldMergeAuthorization(
            Arc::clone(&called)
        ))),
        Err(TransactionsBlockError::MergeAdmission)
    ));
    assert!(!called.load(Ordering::SeqCst),"native stage is rejected before an empty old binding can authorize publication");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before,"hooks and all native writes roll back together");
}

state_test! { sync native_consumer_stage_prepared_authority_refuses_changed_publication
    use super::{NativeLaneBatchReplayV1,NativeLaneBatchSourcePreparationV1};
    let fixture = native_consumer_stage_fixture(false); let state=&fixture.native.state;
    let carrier=native_consumer_stage_carrier(&fixture);
    let NativeLaneBatchSourcePreparationV1::Ready(source)=state.prepare_proposed_native_lane_batch_source(&carrier,&[]).unwrap() else {panic!("ready")};
    state.append_committed_block_header_for_tests(carrier.header());
    let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    assert!(matches!(source.stage_with_start_hooks().unwrap(),NativeLaneBatchReplayV1::ObservationChanged));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
    assert_eq!(state.world.assets.view().get(&fixture.source).unwrap().0,Quantity::from(100u32));
}

state_test! { sync native_consumer_source_preparation_retains_exact_recovery_positions_then_stages
    use super::{NativeLaneBatchReplayV1,NativeLaneBatchSourcePreparationV1};
    let (fixture,carrier)=proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25),NativeEconomicCase::Transfer(30)]);
    let state=&fixture.native.state;let first=&fixture.native.block;
    let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    state.kura.evict_first_admission_body_for_testing(NonZeroUsize::new(first.header().height().get() as usize).unwrap(),first.hash()).unwrap();
    let NativeLaneBatchSourcePreparationV1::FirstInputRecoveryRequired{execution_index,source}=state.prepare_proposed_native_lane_batch_source(&carrier,&[]).unwrap() else {panic!("exact first body required")};
    assert_eq!(execution_index,0);
    let (request,response,outstanding)=authenticated_native_batch_body_response_for_test(&fixture.native.validators[0],source.finality(),first);
    let recovered=source.complete_from_authenticated_response(&request,&response).unwrap();
    let mut retained=vec![(0,recovered)];
    let NativeLaneBatchSourcePreparationV1::FirstInputRecoveryRequired{execution_index,source}=state.prepare_proposed_native_lane_batch_source(&carrier,&retained).unwrap() else {panic!("retain first completion while recovering second input")};
    assert_eq!(execution_index,1);assert_eq!(source.carrier_hash(),first.hash());
    let second=source.complete_from_authenticated_response(&request,&response).unwrap();
    assert!(state.prepare_proposed_native_lane_batch_source(&carrier,&[(0,second.clone())]).is_err());
    retained.push((1,second));
    let NativeLaneBatchSourcePreparationV1::Ready(source)=state.prepare_proposed_native_lane_batch_source(&carrier,&retained).unwrap() else {panic!("all private inputs retained")};
    let NativeLaneBatchReplayV1::Ready(staged)=source.stage_with_start_hooks().unwrap() else {panic!("same pre-State")};
    assert_eq!(staged.overlay().world.assets.get(&fixture.destination).unwrap().0,Quantity::from(55u32));
    staged.overlay().validate_merge_carrier_entrypoint_binding().unwrap();
    drop(staged);assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
    assert_eq!(outstanding.len(),1,"stage cannot release existing global recovery transport custody");
}

state_test! { sync native_consumer_source_refuses_authentically_resigned_first_carrier_substitution
    use super::NativeLaneBatchSourcePreparationV1;
    let fixture=native_consumer_stage_fixture(false);let state=&fixture.native.state;
    let carrier=native_consumer_stage_carrier(&fixture);
    let before=crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let mut changed=carrier.clone();let mut bundle=changed.execution_context().unwrap().clone();
    let source=&mut bundle.native_lane_decisions.as_mut().unwrap().groups[0];
    source.payload.descriptor.admission_carrier_hash=HashOf::from_untyped_unchecked(Hash::new(b"foreign finalized source"));
    resign_changed_native_group_payload_for_test(&fixture.native,source);
    changed.set_execution_context(Some(bundle));
    assert!(state.prepare_proposed_native_lane_batch_source(&changed,&[]).is_err(),
        "even valid native signatures cannot substitute the canonical first carrier");
    assert!(matches!(state.prepare_proposed_native_lane_batch_source(&carrier,&[]).unwrap(),NativeLaneBatchSourcePreparationV1::Ready(_)));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"),before);
}

// Native sources have no physical external transactions. These controls exercise
// the actual virtual Network projection, common internal phases and consuming
// result seal rather than constructing output rows in a fixture.
state_test! { sync native_common_owner_executes_and_seals_pipeline_and_time_once
    use iroha_data_model::block::execution_output::ExecutionOutputV1;
    for nested in [false, true] {
        let (fixture, parent, child) = pipeline_receipt_fixture(nested);
        let authority = fixture.source.account().clone();
        let mut metadata = iroha_model_base::metadata::Metadata::default();
        metadata.insert("__registered_block_height".parse::<Name>().unwrap(), Json::new(0u64));
        let time_id: TriggerId = "native_common_time".parse().unwrap();
        let trigger = Trigger::new(time_id.clone(), Action::new(
            [InstructionBox::from(SetKeyValue::account(authority.clone(), "native_time_effect".parse().unwrap(), Json::new(1u32)))],
            Repeats::Exactly(1), authority.clone(), TimeEventFilter::new(ExecutionTime::PreCommit),
        ).unwrap().with_metadata(metadata));
        {
            let mut block = fixture.native.state.world.triggers.block();
            let mut transaction = block.transaction();
            assert!(transaction.add_time_trigger(trigger.try_into().unwrap()).unwrap());
            transaction.apply();
            block.commit();
        }
        let state = &fixture.native.state;
        let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
        let groups = native_economic_groups(&fixture);
        let mut carrier = native_consumer_stage_carrier(&fixture);
        let proposal = carrier.hash();
        let signatures = carrier.signatures().cloned().collect::<Vec<_>>();
        assert!(carrier.external_entrypoints_slice().is_empty());
        assert_eq!(carrier.network_entrypoint_count(), 1);
        let mut prepared = state.prepare_native_batch_on_carrier(carrier.header(), groups.clone()).unwrap();
        let rows = prepared.overlay().retained_execution_outputs_for_test().unwrap().to_vec();
        assert!(matches!(rows.as_slice(), [ExecutionOutputV1::Network(_), ExecutionOutputV1::Pipeline(_), ExecutionOutputV1::Time(_)]));
        assert!(rows.iter().all(|row| row.result().is_ok()), "{rows:?}");
        assert_eq!(prepared.executions()[0].result, *rows[0].result());
        assert_eq!(rows[1].completions().len(), if nested { 2 } else { 1 });
        assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(72u32), "native25 + one actual Pipeline batch3");
        assert_eq!(prepared.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(28u32));
        assert_eq!(prepared.overlay().world.account(&authority).unwrap().metadata().get("native_time_effect"), Some(&Json::new(1u32)));
        assert!(prepared.overlay().world.triggers.pipeline_triggers().get(&parent).is_none());
        if nested { assert!(prepared.overlay().world.triggers.by_call_triggers().get(&child).is_none()); }
        assert!(prepared.overlay().world.triggers.time_triggers().get(&time_id).is_none());
        prepared.overlay().validate_native_lane_execution().unwrap();
        assert_native_economic_terminal(prepared.overlay(), &groups[0], carrier.header().height().get());
        prepared.overlay_mut_for_test().seal_execution_outputs(&mut carrier, |overlay, _, routes| {
            assert_eq!(routes, &[groups[0].body().payload().input.routing_plan().unwrap().coordinator_route()]);
            Ok::<_, String>(super::ExecutionOutputSealMetadata {
                committed_fragment_count: u64::try_from(overlay.committed_fragment_count()).unwrap(),
                lane_finality_statements: Vec::new(),
            })
        }).unwrap();
        assert_eq!(carrier.execution_outputs(), rows);
        assert_eq!(carrier.hash(), proposal);
        assert_eq!(carrier.signatures().cloned().collect::<Vec<_>>(), signatures);
        assert_eq!(crate::block::native_lane_batch_for_scratch(&carrier).unwrap(), prepared.batch());
        prepared.overlay().verify_execution_output_seal(&carrier).unwrap();
        assert_eq!(prepared.overlay().verified_fastpq_source_inventory_for_capture().unwrap().entries().len(), 3);
        let overlay = prepared.into_overlay_for_test();
        assert!(
            matches!(
                overlay.commit(),
                Err(TransactionsBlockError::MergeAdmission)
            ),
            "completed output attachment is not State/native Apply authority"
        );
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before, "all actual phases and metadata remain atomic on discard");
    }
}

state_test! { sync native_common_owner_refuses_foreign_carrier_before_finalization
    let fixture = native_consumer_stage_fixture(false);
    let state = &fixture.native.state;
    let groups = native_economic_groups(&fixture);
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot");
    let mut prepared = state.prepare_native_batch_on_carrier(carrier.header(), groups.clone()).unwrap();
    let mut foreign = carrier.clone();
    let mut batch = prepared.batch().clone();
    batch.base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"foreign native economic predecessor"));
    foreign.set_execution_context(Some(BlockExecutionContextBundle::default().with_native_lane_decisions(batch)));
    assert_eq!(foreign.network_entrypoints().map(TransactionEntrypoint::hash).collect::<Vec<_>>(), carrier.network_entrypoints().map(TransactionEntrypoint::hash).collect::<Vec<_>>());
    assert!(matches!(prepared.overlay_mut_for_test().seal_execution_outputs::<String>(&mut foreign, |_, _, _| panic!("foreign proposal must not run the finalizer")), Err(super::ExecutionOutputSealError::Owner(_))));
    assert!(!foreign.has_results());
    assert!(matches!(prepared.overlay().execution_output_plan, Some(super::output_capacity::ExecutionOutputPlanState::Poisoned)));
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).expect("stable valid fixture snapshot"), before);
}

state_test! { sync native_consumer_source_custody_moves_original_all_route_owners
    use super::{NativeLaneBatchReplayV1, NativeLaneBatchSourcePreparationV1};
    let fixture = native_economic_fixture_with_genesis_layout(
        &[NativeEconomicCase::Transfer(25)], true,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16, chunk_size_bytes: 8192,
            data_shards: 1, parity_shards: 1, max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
    );
    let state = &fixture.native.state;
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("real four-validator first sources and all-route Decisions"); };
    let groups = source.groups_for_test();
    assert_eq!(groups.len(), 1);
    assert!(groups[0].contexts().len() > 1, "exercise every affected route, not only the coordinator");
    let original_groups = groups.as_ptr();
    let original = groups.iter().map(|group| (
        group.body().canonical_bytes().as_ptr(),
        group.body().source().canonical_control_bytes().as_ptr(),
        group.decisions().as_ptr(),
        group.contexts().as_ptr(),
        group.body().source().source().finality().clone(),
        group.to_wire(),
    )).collect::<Vec<_>>();
    let NativeLaneBatchReplayV1::Ready(staged) = source.stage_with_start_hooks().unwrap()
        else { panic!("same actual predecessor"); };
    assert_eq!(staged.sources_for_test().as_ptr(), original_groups,
        "the original verified Vec moves across execution, not a wire-derived replacement");
    for ((group, original), wire) in staged.sources_for_test().iter().zip(&original).zip(&staged.batch().groups) {
        assert_eq!(group.body().canonical_bytes().as_ptr(), original.0);
        assert_eq!(group.body().source().canonical_control_bytes().as_ptr(), original.1);
        assert_eq!(group.decisions().as_ptr(), original.2);
        assert_eq!(group.contexts().as_ptr(), original.3);
        assert_eq!(group.body().source().source().finality(), &original.4);
        assert_eq!(wire, &original.5);
        assert_eq!(group.body().payload(), &wire.payload);
        assert_eq!(group.decisions(), wire.decisions);
    }
    assert_eq!(staged.executions()[0].source, original[0].5);
    assert!(staged.executions()[0].result.is_ok());
    staged.overlay().validate_native_lane_execution().unwrap();
    assert_eq!(staged.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    drop(staged);
    // Reacquiring the original State writers proves the consumed stage released them.
    drop(state.block(carrier.header()));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert!(state.kura.v2_finality_artifact(carrier.header().height().get()).unwrap().is_none());
    assert!(crate::block::ValidBlock::validate_inactive_native_carrier_for_test(&carrier)
        .unwrap_err().to_string().contains("not active"));
}

state_test! { sync native_consumer_source_custody_refusal_keeps_state_and_storage_unchanged
    use super::{NativeLaneBatchReplayV1, NativeLaneBatchSourcePreparationV1};
    let fixture = native_consumer_stage_fixture(false);
    let state = &fixture.native.state;
    let carrier = native_consumer_stage_carrier(&fixture);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("current authenticated source"); };
    assert!(!source.groups_for_test().is_empty());
    let mut publication_notice = state.state_view_publication();
    let publication = publication_notice.begin();
    drop(publication);
    drop(publication_notice);
    assert!(matches!(source.stage_with_start_hooks().unwrap(), NativeLaneBatchReplayV1::ObservationChanged));
    drop(state.block(carrier.header()));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);

    let groups = native_economic_groups(&fixture);
    let mut substituted = state.prepare_lane_decision_batch(&groups).unwrap();
    substituted.groups[0].payload.descriptor.admission_carrier_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign first carrier after owned admission"));
    assert!(state.replay_lane_decision_batch(&carrier.header(), &substituted, groups).is_err());
    drop(state.block(carrier.header()));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert!(state.kura.v2_finality_artifact(carrier.header().height().get()).unwrap().is_none());
}
