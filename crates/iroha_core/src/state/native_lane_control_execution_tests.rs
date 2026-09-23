// Real first admissions and finalized parent contexts feed the unpublished
// Native control producer. These controls never grant global acceptance/Apply.

struct NativeControlExecutionFixture {
    economic: Box<NativeEconomicFixture>,
    later: iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
    applying: crate::sumeragi::v2::VerifiedHeightContext,
    requested_beacon: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
}

// Finish economic fixture construction before reserving the later admission
// overlay frame. The same boxed State moves into the second phase.
#[inline(never)]
fn native_control_execution_fixture(
    atomic: bool,
    with_beacon: bool,
) -> NativeControlExecutionFixture {
    let economic = native_economic_fixture_with_genesis_layout(
        &[NativeEconomicCase::Transfer(25)],
        atomic,
        Some(DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 8192,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 2 * 1024 * 1024,
            max_chunk_count: 512,
        }),
    );
    native_control_execution_fixture_from_economic(economic, atomic, with_beacon)
}

// This continuation owns the original economic fixture; no State is cloned or
// reconstructed while the actual later admission acquires its writer guards.
#[inline(never)]
fn native_control_execution_fixture_from_economic(
    mut economic: Box<NativeEconomicFixture>,
    atomic: bool,
    with_beacon: bool,
) -> NativeControlExecutionFixture {
    let state = &economic.native.state;
    let original = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let original_contexts = original
        .contexts()
        .iter()
        .map(|context| context.frozen().clone())
        .collect::<Vec<_>>();
    assert_eq!(original_contexts.len(), if atomic { 2 } else { 1 });
    let first: iroha_data_model::block::lane_admission::LaneAdmittedInputV1 =
        norito::decode_canonical(
            &economic
                .native
                .block
                .execution_context()
                .unwrap()
                .queue_plan_admissions[0],
        )
        .unwrap();
    let routing = first.routing_plan().unwrap();
    let signer = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    assert_eq!(
        economic.source.account(),
        &AccountId::new(signer.public_key().clone())
    );
    let mut builder = TransactionBuilder::new(
        state.network_id,
        economic.source.account().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(2));
    builder.set_ttl(Duration::from_secs(1));
    let later_entrypoint = TransactionEntrypoint::External(
        builder
            .with_instructions([Transfer::asset_quantity(
                economic.source.clone(),
                7u32,
                economic.destination.account().clone(),
            )])
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            )
            .sign(signer.private_key()),
    );
    let (binding, control) = queue_plan_admission_certificate_for_entrypoint_state_test(
        state,
        routing.clone(),
        &economic.native.validators,
        economic.native.block.header().height().get(),
        0xB7,
        &later_entrypoint,
    );
    let later: iroha_data_model::block::lane_admission::LaneAdmittedInputV1 =
        norito::decode_canonical(&control).unwrap();
    assert_eq!(later.entrypoint, later_entrypoint);
    assert_eq!(later.certificate.binding, binding);
    assert_ne!(later.entrypoint.hash(), first.entrypoint.hash());
    let parent = state
        .kura
        .v2_finality_artifact(economic.native.block.header().height().get())
        .unwrap()
        .unwrap();
    parent.verify().unwrap();
    let opening = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        parent.height_context.nexus_amx_context_hash,
        None,
    )
    .unwrap();
    let mut block = empty_global_block_after(Some(&economic.native.block));
    let mut execution = block.execution_context().cloned().unwrap_or_default();
    execution.queue_plan_admissions = vec![control.clone()];
    block.set_execution_context(Some(execution));
    // The actual control admits the complete second input. It executes no
    // Network input; the pending input will execute only after the first head.
    assert_eq!(block.network_entrypoint_count(), 0);
    block
        .set_execution_outputs(
            Vec::new(),
            0,
            BTreeMap::new(),
            Vec::new(),
            AxtPolicySnapshot::default(),
            BTreeSet::new(),
            Vec::new(),
            &crate::execution_output_test_support::structural_output_limits(),
        )
        .unwrap();
    native_control_resign_carrier(&mut block);
    block.validate_proposal_commitments().unwrap();
    block.validate_execution_result_structure().unwrap();
    let mut overlay = state
        .block_with_queue_plan_admissions(block.header(), &[control])
        .unwrap();
    // Seed the exact pending pulse in this carrier's original World journal.
    // A later World-only commit would overwrite the genuine H-1 undo record,
    // making snapshot recovery pair old lane contexts with new admissions.
    let requested_beacon = with_beacon.then(|| {
        let validators = (0xD3_u8..=0xD6)
            .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
            .collect::<Vec<_>>();
        install_exact_merge_beacon_fixture(state, &mut overlay.world, &validators, &block)
    });
    overlay
        .finalize_lane_consensus_contexts(&block, Some(&opening))
        .unwrap();
    assert_eq!(
        overlay.lane_consensus_contexts.get().contexts,
        original_contexts
    );
    let mut witness = ExecWitness::default();
    overlay
        .capture_lane_consensus_contexts(&mut witness)
        .unwrap();
    overlay
        .stage_autoscale_sample_record_for_count(&block, 0)
        .unwrap();
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &block);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    let (artifact, receipt) =
        stage_lane_context_fixture_finality(state, &block, opening.clone(), witness.clone());
    state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    let published = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(
        published
            .contexts()
            .iter()
            .map(|context| context.frozen().clone())
            .collect::<Vec<_>>(),
        original_contexts,
        "later finalized admission preserves the original route head and its opening authority",
    );
    {
        let view = state.view();
        for input in [&first, &later] {
            assert!(
                State::pending_queue_plan_binding_for_execution(
                    &view,
                    &input.entrypoint,
                    &routing,
                    block.header().height().get() + 1,
                )
                .unwrap()
                .is_some()
            );
        }
    }
    let applying = native_control_verified_context(state, artifact.height);
    assert_eq!(applying.context().network_id, state.network_id);
    assert_eq!(applying.context().roster.len(), 4);
    assert_eq!(
        applying.context().da_layout.encoding,
        PayloadEncoding::ReedSolomon16
    );
    economic.native.block = block;
    economic.native.opening = opening;
    economic.native.witness = witness;
    NativeControlExecutionFixture {
        economic,
        later,
        applying,
        requested_beacon,
    }
}

fn native_control_resign_carrier(carrier: &mut SignedBlock) {
    let key = merge_carrier_finality_fixture_keypair();
    carrier
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                0,
                iroha_crypto::SignatureOf::from_hash(key.private_key(), carrier.hash()),
            ),
        ]))
        .unwrap();
}

fn native_control_verified_context(
    state: &State,
    parent_height: u64,
) -> crate::sumeragi::v2::VerifiedHeightContext {
    let (parent, receipt) = state
        .kura
        .v2_finality_artifact_with_receipt(parent_height)
        .unwrap()
        .unwrap();
    let next = crate::sumeragi::v2_context::build_successor_height_context(
        &parent,
        crate::sumeragi::v2_recovery::committed_nexus_amx_context_hash(state).unwrap(),
        None,
    )
    .unwrap();
    crate::sumeragi::v2::VerifiedHeightContext::successor(
        next,
        parent.validator_set_pops.clone(),
        &parent,
        &receipt,
        &parent.validator_set_pops,
    )
    .unwrap()
}

fn assert_native_control_suffix(
    fixture: &NativeControlExecutionFixture,
    recorded: &super::lane_decision_batch::RecordedNativeLaneBatchV1<'_>,
) {
    let prepared = recorded.prepared_for_test();
    assert_eq!(prepared.sources_for_test().len(), 1);
    assert_eq!(prepared.executions().len(), 1);
    assert!(prepared.executions()[0].result.is_ok());
    let source = &prepared.sources_for_test()[0];
    let overlay = prepared.overlay();
    let height = recorded.carrier().header().height().get();
    assert_native_economic_terminal(overlay, source, height);
    assert_eq!(
        overlay
            .world
            .assets
            .get(&fixture.economic.source)
            .unwrap()
            .0,
        Quantity::from(75u32)
    );
    assert_eq!(
        overlay
            .world
            .assets
            .get(&fixture.economic.destination)
            .unwrap()
            .0,
        Quantity::from(25u32)
    );
    assert!(
        State::pending_queue_plan_binding_for_execution(
            overlay,
            &fixture.later.entrypoint,
            &fixture.later.routing_plan().unwrap(),
            height,
        )
        .unwrap()
        .is_some(),
        "actual later admission remains pending, not executed or discarded"
    );
    assert!(
        overlay
            .transactions
            .get(&fixture.later.entrypoint.hash())
            .is_none()
    );
    let next = overlay.lane_consensus_contexts.get();
    assert_eq!(next.contexts.len(), source.contexts().len());
    for old in source.contexts() {
        let old = old.frozen();
        let successor = next
            .contexts
            .iter()
            .find(|next| next.lane_id == old.lane_id)
            .unwrap();
        assert_eq!(successor.lane_incarnation, old.lane_incarnation);
        assert_eq!(successor.dataspace_id, old.dataspace_id);
        assert_eq!(successor.predecessor_height, old.next_lane_height);
        assert_eq!(successor.next_lane_height, old.next_lane_height + 1);
        assert_eq!(
            successor.predecessor_hash,
            Some(source.body().payload().descriptor.canonical_hash().unwrap())
        );
        assert_eq!(successor.predecessor_applied_global_height, height);
        assert_eq!(
            successor.admitted_binding_hash,
            fixture.later.certificate.binding.canonical_hash()
        );
        assert_eq!(successor.opening_global_height, height);
        assert_eq!(
            successor.opening_global_context_id,
            fixture.applying.context().id()
        );
        assert_eq!(successor.committee.len(), 4);
        assert_eq!(successor.validator_set_pops.len(), 4);
        assert_eq!(successor.da_layout.encoding, PayloadEncoding::ReedSolomon16);
        assert_ne!(
            successor.canonical_hash().unwrap(),
            old.canonical_hash().unwrap()
        );
    }
    overlay
        .verify_execution_output_seal(recorded.carrier())
        .unwrap();
    let witness = overlay
        .exec_witness
        .as_ref()
        .expect("whole control+Native witness");
    overlay
        .verify_lane_consensus_contexts_witness(witness)
        .unwrap();
    overlay
        .verified_fastpq_source_inventory_for_capture()
        .unwrap()
        .verify_ordinary_witness_bundles(&witness.fastpq_transcripts)
        .unwrap();
    assert!(
        crate::block::ValidBlock::validate_inactive_native_carrier_for_test(recorded.carrier())
            .unwrap_err()
            .to_string()
            .contains("not active")
    );
}

state_test! { sync native_recorded_control_suffix_requires_opening_after_real_later_admission
    for atomic in [false, true] {
        let fixture = native_control_execution_fixture(atomic, false);
        let state = &fixture.economic.native.state;
        let mut carrier = native_consumer_stage_carrier(&fixture.economic);
        let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
        let files = exact_test_tree_fingerprint(&state.kura.store_root());
        let groups = native_economic_groups(&fixture.economic);
        let mut scratch = state.prepare_native_batch_on_carrier(carrier.header(), groups).unwrap();
        attach_actual_native_prefix_results_for_test(&mut carrier, &scratch);
        let contexts = scratch.overlay().lane_consensus_contexts.get().clone();
        // This real scratch has settled the first head, so the later admission
        // genuinely requires a new context; no fabricated pending row is used.
        assert!(State::pending_queue_plan_binding_for_execution(
            scratch.overlay(), &fixture.later.entrypoint,
            &fixture.later.routing_plan().unwrap(), carrier.header().height().get(),
        ).unwrap().is_some());
        let error = scratch.overlay_mut_for_test()
            .finalize_lane_consensus_contexts(&carrier, None).unwrap_err();
        assert!(error.contains("requires authenticated carrier context"), "{error}");
        assert_eq!(scratch.overlay().lane_consensus_contexts.get(), &contexts);
        assert!(scratch.overlay().lane_consensus_contexts_seal.is_none());
        drop(scratch);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
        assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
        assert_native_economic_relay_recorder_released();
        assert!(carrier.has_results(), "actual prefix outputs precede the context projection");
    }
}

state_test! { sync native_recorded_control_suffix_opens_exact_next_context_and_retains_pending_work
    use super::NativeLaneBatchSourcePreparationV1;
    for atomic in [false, true] {
        let fixture = native_control_execution_fixture(atomic, false);
        let state = &fixture.economic.native.state;
        let carrier = native_consumer_stage_carrier(&fixture.economic);
        let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
        let files = exact_test_tree_fingerprint(&state.kura.store_root());
        let NativeLaneBatchSourcePreparationV1::Ready(source) = state
            .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
            else { panic!("exact original first-source owners"); };
        let recorded = source.record_execution(carrier, fixture.applying.clone())
            .unwrap().expect("same applying pre-State");
        assert_native_control_suffix(&fixture, &recorded);
        drop(recorded);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
        assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
        assert_native_economic_relay_recorder_released();
    }
}

state_test! { sync native_recorded_control_rejects_changed_opening_and_stale_verified_height
    use super::NativeLaneBatchSourcePreparationV1;
    let fixture = native_control_execution_fixture(true, false);
    let state = &fixture.economic.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let parent_height = fixture.economic.native.block.header().height().get();
    let (parent, receipt) = state.kura.v2_finality_artifact_with_receipt(parent_height)
        .unwrap().unwrap();
    for alteration in 0..3 {
        let mut changed = fixture.applying.context().clone();
        match alteration {
            0 => changed.network_id = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(b"foreign Native opening network"))),
            1 => changed.height += 1,
            2 => changed.execution_policy_hash = Hash::new(b"foreign Native opening execution policy"),
            _ => unreachable!(),
        }
        assert!(crate::sumeragi::v2::VerifiedHeightContext::successor(
            changed, parent.validator_set_pops.clone(), &parent, &receipt,
            &parent.validator_set_pops,
        ).is_err(), "real parent QC/PoPs/receipt reject changed opening field {alteration}");
    }
    let stale = native_control_verified_context(state, parent_height - 1);
    assert_eq!(stale.context().height + 1, fixture.applying.context().height);
    // Nexus policy is selected from the applying pre-State, independently from
    // the parent QC's roster/policy proof. Its exact local join must still hold.
    let mut wrong_nexus = fixture.applying.context().clone();
    wrong_nexus.nexus_amx_context_hash = Hash::new(b"foreign Native opening Nexus policy");
    let wrong_nexus = crate::sumeragi::v2::VerifiedHeightContext::successor(
        wrong_nexus, parent.validator_set_pops.clone(), &parent, &receipt,
        &parent.validator_set_pops,
    ).unwrap();
    for other in [stale, wrong_nexus] {
        let carrier = native_consumer_stage_carrier(&fixture.economic);
        let NativeLaneBatchSourcePreparationV1::Ready(source) = state
            .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
            else { panic!("exact original first-source owners"); };
        let error = source.record_execution(carrier, other).err().expect("verified foreign context is not applying authority");
        assert!(matches!(error, MergeLedgerCommitError::NativeControlValidation(_)), "{error}");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
        assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
        assert_native_economic_relay_recorder_released();
    }
}

fn native_control_requested_beacon(
    fixture: &NativeControlExecutionFixture,
) -> iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1 {
    let mut validators = (0xD3_u8..=0xD6)
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    validators.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert_eq!(
        validators
            .iter()
            .map(KeyPair::public_key)
            .collect::<Vec<_>>(),
        fixture
            .applying
            .context()
            .roster
            .iter()
            .map(|entry| entry.validator.public_key())
            .collect::<Vec<_>>(),
        "beacon authority is the authentic global committee, not a lane committee",
    );
    let pulse = fixture
        .requested_beacon
        .expect("beacon setup belongs to the original carrier commit");
    assert_eq!(pulse.height, fixture.applying.context().height);
    assert_eq!(pulse.network_id, fixture.applying.context().network_id);
    assert_eq!(
        pulse.finalized_chain_anchor.block_hash,
        fixture.economic.native.block.hash()
    );
    let view = fixture.economic.native.state.view();
    let slot = (
        iroha_data_model::governance::types::BeaconSessionId::for_network_v1(&pulse.network_id),
        pulse.height,
    );
    assert!(
        view.world()
            .parliament_required_beacon_pulse_slots()
            .get(&slot)
            .is_some_and(|attempts| !attempts.is_empty())
    );
    assert!(
        view.world()
            .global_beacon_pulses()
            .get(&pulse.pulse_id)
            .is_none()
    );
    pulse
}

fn native_control_attach_beacon(
    carrier: &mut SignedBlock,
    pulse: iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
) {
    carrier.set_npos_consensus_effects(Some(iroha_data_model::consensus::NposConsensusEffects {
        finalized_global_beacon_pulse: Some(pulse),
        ..Default::default()
    }));
    native_control_resign_carrier(carrier);
    carrier.validate_proposal_commitments().unwrap();
}

state_test! { sync native_recorded_control_beacon_preserves_complete_snapshot_and_predecessor
    for atomic in [false, true] {
        let fixture = native_control_execution_fixture(atomic, true);
        let state = &fixture.economic.native.state;
        let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
        let pulse = native_control_requested_beacon(&fixture);
        assert_eq!(native_control_requested_beacon(&fixture), pulse);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before,
            "reading the requested pulse cannot publish a replacement World undo");
        let restored = deserialize::KuraSeed {
            kura: Arc::clone(&state.kura),
            lane_manifests: state.lane_manifests.read().clone(),
            query_handle: LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }.into_state_from_json(norito::json::to_value(state).unwrap())
            .expect("restore the exact finalized beacon/admission carrier and its H-1 predecessor");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&restored).unwrap(), before);
        assert_eq!(restored.lane_consensus_contexts.view().get(), state.lane_consensus_contexts.view().get());
        assert_eq!(restored.lane_consensus_contexts.predecessor_view().get(), state.lane_consensus_contexts.predecessor_view().get());
    }
}

state_test! { sync native_recorded_control_executes_requested_beacon_before_suffix_and_retains_witness
    use super::NativeLaneBatchSourcePreparationV1;
    for atomic in [false, true] {
        let fixture = native_control_execution_fixture(atomic, true);
        let pulse = native_control_requested_beacon(&fixture);
        let state = &fixture.economic.native.state;
        let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
        let files = exact_test_tree_fingerprint(&state.kura.store_root());
        let mut carrier = native_consumer_stage_carrier(&fixture.economic);
        native_control_attach_beacon(&mut carrier, pulse);
        let NativeLaneBatchSourcePreparationV1::Ready(source) = state
            .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
            else { panic!("actual source with mandatory beacon control"); };
        let recorded = source.record_execution(carrier, fixture.applying.clone())
            .unwrap().expect("same complete original pre-State");
        assert_native_control_suffix(&fixture, &recorded);
        let overlay = recorded.prepared_for_test().overlay();
        assert_eq!(overlay.world.global_beacon_pulses.get(&pulse.pulse_id), Some(&pulse));
        let expected_link = crate::beacon::validate_persisted_global_threshold_beacon_pulse_v1(&pulse).unwrap();
        assert_eq!(overlay.world.global_beacon_latest_pulse.get(&GLOBAL_THRESHOLD_BEACON_SINGLETON_KEY), Some(&expected_link));
        let slot = (
            iroha_data_model::governance::types::BeaconSessionId::for_network_v1(&pulse.network_id),
            pulse.height,
        );
        assert_eq!(overlay.world.global_beacon_pulse_slots.get(&slot), Some(&pulse.pulse_id));
        assert!(!overlay.exec_witness.as_ref().unwrap().writes.is_empty());
        assert_eq!(recorded.carrier().npos_consensus_effects().unwrap().finalized_global_beacon_pulse, Some(pulse));
        drop(recorded);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
        assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
        assert_native_economic_relay_recorder_released();
    }
}

state_test! { sync native_recorded_control_rejects_missing_corrupt_and_foreign_parent_beacon
    use super::NativeLaneBatchSourcePreparationV1;
    let fixture = native_control_execution_fixture(true, true);
    let pulse = native_control_requested_beacon(&fixture);
    let state = &fixture.economic.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    for alteration in 0..3 {
        let mut carrier = native_consumer_stage_carrier(&fixture.economic);
        if alteration != 0 {
            let mut changed = pulse;
            match alteration {
                1 => changed.signature[0] ^= 1,
                2 => changed.finalized_chain_anchor.block_hash = HashOf::from_untyped_unchecked(Hash::new(b"foreign Native beacon parent")),
                _ => unreachable!(),
            }
            native_control_attach_beacon(&mut carrier, changed);
        }
        let NativeLaneBatchSourcePreparationV1::Ready(source) = state
            .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
            else { panic!("first input and Decisions remain authentic independently of controls"); };
        let error = source.record_execution(carrier, fixture.applying.clone())
            .err().expect("real requested beacon must be exact");
        assert!(matches!(error, MergeLedgerCommitError::NativeControlValidation(_)), "{error}");
        assert!(error.to_string().contains("beacon"), "{error}");
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
        assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
        assert_native_economic_relay_recorder_released();
    }
}

state_test! { sync native_recorded_control_admits_same_carrier_input_without_executing_it
    use super::NativeLaneBatchSourcePreparationV1;
    let fixture = native_control_execution_fixture(true, false);
    let state = &fixture.economic.native.state;
    let signer = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    let mut builder = TransactionBuilder::new(
        state.network_id, fixture.economic.source.account().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    builder.set_creation_time(Duration::from_millis(3));
    builder.set_ttl(Duration::from_secs(1));
    let entrypoint = TransactionEntrypoint::External(builder.with_instructions([
        Transfer::asset_quantity(fixture.economic.source.clone(), 11u32, fixture.economic.destination.account().clone()),
    ]).with_admission_intent(iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced)
        .sign(signer.private_key()));
    let routing = fixture.later.routing_plan().unwrap();
    let (binding, control) = queue_plan_admission_certificate_for_entrypoint_state_test(
        state, routing.clone(), &fixture.economic.native.validators,
        fixture.economic.native.block.header().height().get(), 0xB8, &entrypoint,
    );
    assert_ne!(binding.canonical_hash(), fixture.later.certificate.binding.canonical_hash());
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let mut carrier = native_consumer_stage_carrier(&fixture.economic);
    let mut context = carrier.execution_context().unwrap().clone();
    context.queue_plan_admissions = vec![control.clone()];
    carrier.set_execution_context(Some(context));
    native_control_resign_carrier(&mut carrier);
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("earlier exact source remains independent from the fresh admission"); };
    let recorded = source.record_execution(carrier, fixture.applying.clone())
        .unwrap().expect("same applying pre-State");
    assert_native_control_suffix(&fixture, &recorded);
    let overlay = recorded.prepared_for_test().overlay();
    let admitted = State::pending_queue_plan_binding_for_execution(
        overlay, &entrypoint, &routing, recorded.carrier().header().height().get(),
    ).unwrap().expect("same carrier's actual complete input remains a pending obligation");
    assert_eq!(admitted.canonical_hash(), binding.canonical_hash());
    assert!(overlay.transactions.get(&entrypoint.hash()).is_none());
    assert_eq!(recorded.carrier().execution_outputs().len(), 1);
    assert_eq!(recorded.carrier().execution_context().unwrap().queue_plan_admissions, vec![control]);
    drop(recorded);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}
