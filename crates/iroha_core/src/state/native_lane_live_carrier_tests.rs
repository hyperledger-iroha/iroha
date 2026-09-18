// Unfinalized applying-carrier replay still authenticates finalized first inputs
// and current native Decisions. These controls never open production admission.

fn proposed_native_batch_fixture(
    cases: &[NativeEconomicCase],
) -> (Box<NativeEconomicFixture>, SignedBlock) {
    let fixture = native_economic_fixture_with_genesis_layout(
        cases,
        false,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
    );
    let groups = native_economic_groups(&fixture);
    let mut carrier =
        empty_global_block_after(Some(&fixture.native.block)).canonical_resultless_proposal();
    carrier.set_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
        &fixture.native.state.nexus_snapshot(),
        carrier.header().height().get(),
    )));
    let batch = fixture
        .native
        .state
        .prepare_lane_decision_batch(&groups)
        .unwrap();
    carrier.set_execution_context(Some(
        BlockExecutionContextBundle::default().with_native_lane_decisions(batch),
    ));
    // A real header signature prevents accidental dependence on stale fixture
    // signature bytes. Global leader/context validation remains outside this API.
    let key = merge_carrier_finality_fixture_keypair();
    carrier
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(key.private_key(), carrier.hash()),
            ),
        ]))
        .unwrap();
    assert!(carrier.is_resultless_proposal());
    assert!(
        fixture
            .native
            .state
            .kura
            .v2_finality_artifact(carrier.header().height().get())
            .unwrap()
            .is_none()
    );
    (fixture, carrier)
}

state_test! { sync live_native_batch_replays_without_applying_carrier_finality_or_publication
    use super::NativeLaneBatchReplayV1;
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let expected = carrier.execution_context().unwrap().native_lane_decisions.as_deref().unwrap();
    assert_eq!(carrier.da_proof_policies(), Some(&crate::da::active_proof_policy_bundle_at_height(
        &state.nexus_snapshot(), carrier.header().height().get(),
    )));
    let groups = native_economic_groups(&fixture);
    let NativeLaneBatchReplayV1::Ready(prepared) = state.replay_proposed_native_lane_batch(&carrier, &[]).unwrap()
        else { panic!("current sources and exact pre-State need no future CommitQC"); };
    assert_eq!(prepared.batch(), expected);
    assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(prepared.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    assert_native_economic_terminal(prepared.overlay(), &groups[0], carrier.header().height().get());
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    assert!(state.kura.v2_finality_artifact(carrier.header().height().get()).unwrap().is_none());
    assert!(state.kura.read_finalized_native_lane_batch(
        NonZeroUsize::new(carrier.header().height().get() as usize).unwrap(), carrier.hash(),
    ).is_err(), "successful live scratch replay cannot mint historical canonical inclusion");
    assert!(crate::block::ValidBlock::validate_inactive_native_carrier_for_test(&carrier).unwrap_err().to_string().contains("not active"));
}

state_test! { sync live_native_batch_rejects_base_and_authentically_resigned_source_substitution
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    for mutation in 0..5 {
        let mut changed = carrier.clone();
        let mut bundle = changed.execution_context().unwrap().clone();
        let batch = bundle.native_lane_decisions.as_mut().unwrap();
        match mutation {
            0 => batch.base_state_height += 1,
            1 => batch.base_state_hash = HashOf::from_untyped_unchecked(Hash::new(b"another WSV base")),
            2 => batch.groups[0].payload.descriptor.admission_priority.carrier_height += 1,
            3 => {
                let source = &mut batch.groups[0];
                source.payload.descriptor.admission_carrier_hash = HashOf::from_untyped_unchecked(Hash::new(b"different finalized first carrier"));
                resign_changed_native_group_payload_for_test(&fixture.native, source);
            },
            4 => {
                // The binding can name a network, but only the actual State and
                // independently authenticated first source can authorize it.
                batch.groups[0].payload.input.certificate.binding.network_id_digest = Hash::new(b"foreign network");
            },
            _ => unreachable!(),
        }
        changed.set_execution_context(Some(bundle));
        assert!(state.replay_proposed_native_lane_batch(&changed, &[]).is_err(), "mutation {mutation}");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    }
}

state_test! { sync live_native_batch_rejects_mixed_unbound_and_unsupported_actual_carriers
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let input = &carrier.execution_context().unwrap().native_lane_decisions.as_deref().unwrap().groups[0].payload.input;
    for mutation in 0..11 {
        let mut changed = carrier.clone();
        match mutation {
            0 => changed.set_execution_context(None),
            1 => {
                let mut header = changed.header();
                header.set_execution_context_hash(Some(HashOf::from_untyped_unchecked(Hash::new(b"unbound bundle"))));
                changed.replace_header_for_testing(header);
            },
            2 => {
                // Source-only proposals do not duplicate an economic header.
                // This boundary rejects zero time; global cadence is ValidBlock-owned.
                let mut header = changed.header(); header.creation_time_ms = 0;
                changed.replace_header_for_testing(header);
            },
            3 => {
                let mut bundle = changed.execution_context().unwrap().clone();
                bundle.external.push(iroha_data_model::block::ExternalExecutionContext::new(input.entrypoint.hash(), LaneId::SINGLE, DataSpaceId::UNIVERSAL));
                changed.set_execution_context(Some(bundle));
            },
            4 => changed.set_external_entrypoints(vec![input.entrypoint.clone()]),
            5 => {
                let mut bundle = changed.execution_context().unwrap().clone();
                bundle.queue_plan_admissions = fixture.native.block.execution_context().unwrap().queue_plan_admissions.clone();
                changed.set_execution_context(Some(bundle));
            },
            6 => changed.set_sccp_commitment_root(Some(*Hash::new(b"additional global SCCP work").as_ref())),
            7 => {
                let mut header = changed.header();
                header.set_prev_block_hash(Some(HashOf::from_untyped_unchecked(Hash::new(b"another actual parent"))));
                // Source-only proposals have no duplicate application header;
                // the exact State parent gate rejects this substituted base.
                changed.replace_header_for_testing(header);
            },
            8 => changed.set_da_proof_policies(None),
            9 => changed.set_da_proof_policies(Some(crate::da::proof_policy_bundle(
                &iroha_config::parameters::actual::LaneConfig::default(),
            ))),
            10 => {
                let mut header = changed.header();
                header.set_da_proof_policies_hash(Some(HashOf::from_untyped_unchecked(Hash::new(b"unbound DA policy"))));
                changed.replace_header_for_testing(header);
            },
            _ => unreachable!(),
        }
        let error = state.replay_proposed_native_lane_batch(&changed, &[]).err()
            .unwrap_or_else(|| panic!("mutation {mutation} must reject an invalid or unsupported carrier"));
        if matches!(mutation, 5 | 6) { assert!(error.contains("additional carrier controls"), "{error}"); }
        if matches!(mutation, 8 | 9) { assert!(error.contains("active pre-State policy"), "{error}"); }
        if mutation == 10 { assert!(error.contains("unbound DA proof-policy"), "{error}"); }
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    }
    let mut executed = carrier.clone();
    let groups = native_economic_groups(&fixture);
    let prepared = state.prepare_native_batch_on_carrier(
        carrier.header(),
        &groups,
    ).unwrap();
    attach_actual_native_prefix_results_for_test(&mut executed, &prepared);
    drop(prepared);
    assert!(executed.has_results());
    assert!(state.replay_proposed_native_lane_batch(&executed, &[]).err().unwrap().contains("resultless"));
}

state_test! { sync live_native_batch_later_time_keeps_exact_source_and_owns_actual_overlay_header
    use super::NativeLaneBatchReplayV1;
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let mut later = carrier.clone();
    let mut header = later.header();
    header.creation_time_ms = header.creation_time_ms.checked_add(1).unwrap();
    later.replace_header_for_testing(header);
    let signer = merge_carrier_finality_fixture_keypair();
    later.replace_signatures(BTreeSet::from([
        iroha_data_model::block::BlockSignature::new(0,
            iroha_crypto::SignatureOf::from_hash(signer.private_key(), later.hash())),
    ])).unwrap();
    later.signatures().next().unwrap().signature().verify_hash(signer.public_key(), later.hash()).unwrap();
    assert_ne!(later.hash(), carrier.hash());
    assert_eq!(later.execution_context(), carrier.execution_context());
    let batch = carrier.execution_context().unwrap().native_lane_decisions.as_deref().unwrap();
    // A valid signature is supplied, but this scratch API does not perform the
    // enclosing global leader/signature/cadence checks owned by ValidBlock.
    let NativeLaneBatchReplayV1::Ready(prepared) = state.replay_proposed_native_lane_batch(&later, &[]).unwrap()
        else { panic!("same authenticated source can execute under a later actual header"); };
    assert_eq!(prepared.batch(), batch);
    assert_eq!(prepared.overlay()._curr_block, later.header());
    assert!(prepared.executions()[0].result.is_ok());
    assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(prepared.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(25u32));
    let batch_hash = batch.canonical_hash().unwrap();
    let identity = super::lane_decision_batch::native_application_identity(&later.header(), batch_hash);
    assert_ne!(identity, super::lane_decision_batch::native_application_identity(&carrier.header(), batch_hash));
    for slot in &batch.groups[0].payload.descriptor.slots {
        let path: iroha_model_base::state_path::StatePath = format!("native_lane_applied_instance_{}", hex::encode(slot.instance_id.as_ref())).parse().unwrap();
        let bytes = prepared.overlay().world.smart_contract_state.get(&path).expect("actual instance marker");
        assert_eq!(norito::decode_canonical::<Hash>(bytes).unwrap(), identity);
    }
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    assert!(state.kura.v2_finality_artifact(later.header().height().get()).unwrap().is_none());
    assert!(state.kura.read_finalized_native_lane_batch(
        NonZeroUsize::new(later.header().height().get() as usize).unwrap(), later.hash(),
    ).is_err(), "scratch replay cannot publish the applying carrier");
    assert!(crate::block::ValidBlock::validate_inactive_native_carrier_for_test(&later).unwrap_err().to_string().contains("not active"));
}

state_test! { sync live_native_batch_retains_exact_completed_first_sources_across_two_recovery_positions
    use super::NativeLaneBatchReplayV1;
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25), NativeEconomicCase::Transfer(30)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let first = &fixture.native.block;
    state.kura.evict_first_admission_body_for_testing(NonZeroUsize::new(first.header().height().get() as usize).unwrap(), first.hash()).unwrap();
    let NativeLaneBatchReplayV1::FirstInputRecoveryRequired { execution_index, source } = state.replay_proposed_native_lane_batch(&carrier, &[]).unwrap()
        else { panic!("the sender's complete source is not first-carrier authority"); };
    assert_eq!(execution_index, 0); assert_eq!(source.carrier_hash(), first.hash());
    let (request, response, outstanding) = authenticated_native_batch_body_response_for_test(&fixture.native.validators[0], source.finality(), first);
    let recovered = source.complete_from_authenticated_response(&request, &response).unwrap();
    assert!(state.replay_proposed_native_lane_batch(&carrier, &[(2, recovered.clone())]).is_err());
    assert!(state.replay_proposed_native_lane_batch(&carrier, &[(0, recovered.clone()), (0, recovered.clone())]).is_err());
    let mut retained = vec![(0, recovered)];
    let NativeLaneBatchReplayV1::FirstInputRecoveryRequired { execution_index, source } = state.replay_proposed_native_lane_batch(&carrier, &retained).unwrap()
        else { panic!("keep the first completed input while recovering the next position"); };
    assert_eq!(execution_index, 1); assert_eq!(source.carrier_hash(), first.hash());
    let second = source.complete_from_authenticated_response(&request, &response).unwrap();
    assert!(state.replay_proposed_native_lane_batch(&carrier, &[(0, second.clone())]).is_err(), "a shared carrier does not collapse exact input positions");
    retained.push((1, second));
    let NativeLaneBatchReplayV1::Ready(prepared) = state.replay_proposed_native_lane_batch(&carrier, &retained).unwrap()
        else { panic!("exact completions re-enter the same source/economic kernel"); };
    assert_eq!(prepared.batch(), carrier.execution_context().unwrap().native_lane_decisions.as_deref().unwrap());
    assert_eq!(prepared.overlay().world.assets.get(&fixture.source).unwrap().0, Quantity::from(45u32));
    assert_eq!(prepared.overlay().world.assets.get(&fixture.destination).unwrap().0, Quantity::from(55u32));
    drop(prepared);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
    assert_eq!(outstanding.len(), 1, "scratch replay cannot acknowledge global transport custody");
    assert!(matches!(state.replay_proposed_native_lane_batch(&carrier, &[]).unwrap(), NativeLaneBatchReplayV1::FirstInputRecoveryRequired { execution_index: 0, .. }), "resultless completion never populates executed-wire storage");
}

// Keep the large State move out of the race test frame on the default stack.
fn proposed_native_batch_shared_state_fixture() -> (Arc<State>, SignedBlock) {
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    (Arc::new(fixture.native.state), carrier)
}

state_test! { sync live_native_batch_discards_a_changed_publication_during_finality_io
    use super::NativeLaneBatchReplayV1;
    let (state, carrier) = proposed_native_batch_shared_state_fixture();
    let writer = Arc::clone(&state);
    let header = carrier.header();
    let result = lane_consensus_verified::io_observer::observe(move || {
        let writer = Arc::clone(&writer);
        let (done, received) = std::sync::mpsc::channel();
        let task = std::thread::spawn(move || {
            let _publication = writer.consensus_publication_lease();
            // Same actual publication/header owner used by the verified-reader
            // race test. No old proof may authorize the newly observed prefix.
            writer.append_committed_block_header_for_tests(header);
            let world = writer.world.block(); world.commit();
            done.send(()).unwrap();
        });
        received.recv_timeout(Duration::from_secs(5)).expect("replay must release State/MV guards before Kura I/O");
        task.join().unwrap();
    }, || state.replay_proposed_native_lane_batch(&carrier, &[]));
    assert!(matches!(result.unwrap(), NativeLaneBatchReplayV1::ObservationChanged));
    assert!(state.kura.v2_finality_artifact(carrier.header().height().get()).unwrap().is_none(), "a changed prefix does not mint finality or a prepared application");
}

fn store_native_control_carrier_for_test(
    fixture: &NativeEconomicFixture,
    carrier: &mut SignedBlock,
) -> V2FinalityArtifact {
    // The actual native prefix supplies its outputs. Additional carrier controls
    // remain unsupported; authentic storage/finality is not their authorization.
    let state = &fixture.native.state;
    let groups = native_economic_groups(fixture);
    let prepared = state
        .prepare_native_batch_on_carrier(carrier.header(), &groups)
        .unwrap();
    attach_actual_native_prefix_results_for_test(carrier, &prepared);
    drop(prepared);
    state.kura.store_block(Arc::new(carrier.clone())).unwrap();
    let parent = state
        .kura
        .v2_finality_artifact(fixture.native.block.header().height().get())
        .unwrap()
        .unwrap();
    let opening = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        parent.height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap();
    let mut overlay = Box::new(state.block(carrier.header()));
    overlay
        .finalize_lane_consensus_contexts(carrier, Some(&opening))
        .unwrap();
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    drop(overlay);
    let (artifact, receipt) = stage_lane_context_fixture_finality(state, carrier, opening, witness);
    state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    artifact
}

state_test! { sync finalized_native_batch_cannot_erase_additional_controls_before_scratch_replay
    use crate::kura::NativeLaneBatchCarrierReadV1;
    let (fixture, mut carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::Transfer(25)]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let mut bundle = carrier.execution_context().unwrap().clone();
    bundle.queue_plan_admissions = fixture.native.block.execution_context().unwrap().queue_plan_admissions.clone();
    bundle.validate_native_lane_decisions_shape().unwrap();
    carrier.set_execution_context(Some(bundle));
    assert!(state.replay_proposed_native_lane_batch(&carrier, &[]).err().unwrap().contains("additional carrier controls"));
    let artifact = store_native_control_carrier_for_test(&fixture, &mut carrier);
    artifact.verify().unwrap();
    let height = NonZeroUsize::new(carrier.header().height().get() as usize).unwrap();
    let error = state.kura.read_finalized_native_lane_batch(height, carrier.hash()).unwrap_err();
    assert!(error.contains("additional carrier controls"), "{error}");
    state.kura.evict_first_admission_body_for_testing(height, carrier.hash()).unwrap();
    let NativeLaneBatchCarrierReadV1::CanonicalBodyRecoveryRequired(required) = state.kura.read_finalized_native_lane_batch(height, carrier.hash()).unwrap()
        else { panic!("an evicted body must be recovered before its full input shape is known"); };
    let (request, response, outstanding) = authenticated_native_batch_body_response_for_test(&fixture.native.validators[0], required.finality(), &carrier);
    let error = required.complete_from_authenticated_response(&request, &response).unwrap_err();
    assert!(error.contains("additional carrier controls"), "{error}");
    assert_eq!(outstanding.len(), 1, "rejected projection cannot discharge exact transport custody");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
}

// Regression for output/header circularity: proposal construction never executes
// registration; actual replay records the final source-bound proposal hash.
state_test! { sync native_asset_registration_replays_under_actual_proposal_hash
    use super::NativeLaneBatchReplayV1;
    let (fixture, carrier) = proposed_native_batch_fixture(&[NativeEconomicCase::RegisterAssetDefinition]);
    let state = &fixture.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state);
    let groups = native_economic_groups(&fixture);
    let id = AssetDefinitionId::derive_from_components(
        DomainId::try_new("native-economics", "universal").unwrap(), "registered".parse().unwrap(),
    );
    assert!(state.world.axt_asset_incarnations.view().get(&id).is_none(), "proposal source preparation cannot register the asset");
    let call = Hash::from(groups[0].body().payload().input.entrypoint.execution_call_hash());
    let NativeLaneBatchReplayV1::Ready(replayed) = state.replay_proposed_native_lane_batch(&carrier, &[]).expect(
        "valid registration must replay under the actual source-bound proposal, without output/header hash circularity",
    ) else { panic!("all authentic first sources are locally available"); };
    assert!(replayed.executions()[0].result.is_ok(), "registration must actually execute: {:?}", replayed.executions()[0].result);
    let expected = iroha_data_model::nexus::AxtAssetIncarnationV1::derive(
        &state.network_id, &id, &carrier.hash(), &call, 0,
    );
    assert_eq!(replayed.overlay().world.axt_asset_incarnations.get(&id), Some(&expected));
    let roots = replayed.prefix_roots_for_test();
    let mut executed = carrier.clone();
    attach_actual_native_prefix_results_for_test(&mut executed, &replayed);
    assert_eq!(executed.hash(), carrier.hash(), "actual result projection cannot change the executing hash");
    assert_eq!(executed.canonical_resultless_proposal(), carrier);
    drop(replayed);
    let NativeLaneBatchReplayV1::Ready(again) = state.replay_proposed_native_lane_batch(&carrier, &[]).unwrap() else { panic!("same source/base") };
    assert_eq!(again.prefix_roots_for_test(), roots);
    assert_eq!(again.overlay().world.axt_asset_incarnations.get(&id), Some(&expected));
    drop(again);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state), before);
}
