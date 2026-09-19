// Canonical global checks feed the actual Native execution owner into common
// carrier preparation. Every candidate remains unpublished and inactive live.

fn native_preparation_global_keys(context: &HeightContext) -> Vec<KeyPair> {
    let mut keys = (0xD3_u8..=0xD6)
        .map(|seed| KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).unwrap())
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    assert_eq!(keys.len(), 4);
    assert!(
        keys.iter()
            .zip(&context.roster)
            .all(|(key, member)| { key.public_key() == member.validator.public_key() })
    );
    keys
}

fn native_preparation_carrier(
    fixture: &NativeControlExecutionFixture,
    admissions: Vec<Vec<u8>>,
    pulse: Option<iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1>,
    time_offset: Duration,
    corrupt_confidential: bool,
) -> SignedBlock {
    let state = &fixture.economic.native.state;
    let parent = &fixture.economic.native.block;
    let creation_time =
        parent.header().creation_time() + state.sumeragi_block_cadence() + time_offset;
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(creation_time);
    let groups = native_economic_groups(&fixture.economic);
    let batch = state.prepare_lane_decision_batch(&groups).unwrap();
    let view = state.view();
    let mut confidential = compute_confidential_feature_digest(
        view.world(),
        view.zk(),
        view.sccp_registry(),
        fixture.applying.context().height,
    );
    drop(view);
    if corrupt_confidential {
        confidential.conf_rules_version = Some(
            confidential
                .conf_rules_version
                .unwrap_or_default()
                .wrapping_add(1),
        );
    }
    let keys = native_preparation_global_keys(fixture.applying.context());
    let leader = fixture.applying.context().leader(0);
    let carrier: SignedBlock = crate::block::BlockBuilder::new_with_time_source(Vec::new(), clock)
        .chain(0, Some(parent))
        .with_confidential_features((!confidential.is_empty()).then_some(confidential))
        .with_da_proof_policies(Some(crate::da::active_proof_policy_bundle_at_height(
            &state.nexus_snapshot(),
            fixture.applying.context().height,
        )))
        .with_execution_context(Some(
            BlockExecutionContextBundle::default()
                .with_native_lane_decisions(batch)
                .with_queue_plan_admissions(admissions),
        ))
        .with_npos_consensus_effects(pulse.map(|pulse| {
            iroha_data_model::consensus::NposConsensusEffects {
                finalized_global_beacon_pulse: Some(pulse),
                ..Default::default()
            }
        }))
        .sign_with_index(
            keys[usize::try_from(leader).unwrap()].private_key(),
            u64::from(leader),
        )
        .unpack(|_| {})
        .into();
    assert_eq!(carrier.header().creation_time(), creation_time);
    assert_eq!(
        carrier.header().height().get(),
        fixture.applying.context().height
    );
    assert!(carrier.is_resultless_proposal());
    carrier.validate_proposal_commitments().unwrap();
    carrier
}

fn native_preparation_new_admission(
    fixture: &NativeControlExecutionFixture,
) -> iroha_data_model::block::lane_admission::LaneAdmittedInputV1 {
    let state = &fixture.economic.native.state;
    let signer = KeyPair::try_from_seed(vec![0x71; 32], Algorithm::Ed25519).unwrap();
    let mut transaction = TransactionBuilder::new(
        state.network_id,
        fixture.economic.source.account().clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    );
    transaction.set_creation_time(Duration::from_millis(3));
    transaction.set_ttl(Duration::from_secs(1));
    let entrypoint = TransactionEntrypoint::External(
        transaction
            .with_instructions([Transfer::asset_quantity(
                fixture.economic.source.clone(),
                11u32,
                fixture.economic.destination.account().clone(),
            )])
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            )
            .sign(signer.private_key()),
    );
    let (binding, bytes) = queue_plan_admission_certificate_for_entrypoint_state_test(
        state,
        fixture.later.routing_plan().unwrap(),
        &fixture.economic.native.validators,
        fixture.economic.native.block.header().height().get(),
        0xB9,
        &entrypoint,
    );
    let input: iroha_data_model::block::lane_admission::LaneAdmittedInputV1 =
        norito::decode_canonical(&bytes).unwrap();
    assert_eq!(input.entrypoint, entrypoint);
    assert_eq!(input.certificate.binding, binding);
    assert_ne!(entrypoint.hash(), fixture.later.entrypoint.hash());
    input
}

// End the sizeable genesis/admission setup frames before retaining the complete
// prepared journals and running their assertions on the default test stack.
#[inline(never)]
fn assert_native_preparation_success(atomic: bool) {
    let fixture = native_control_execution_fixture(atomic);
    assert_native_preparation_with_fixture(atomic, fixture);
}

#[inline(never)]
fn assert_native_preparation_with_fixture(atomic: bool, fixture: NativeControlExecutionFixture) {
    use super::NativeLaneBatchSourcePreparationV1;
    let pulse = native_control_requested_beacon(&fixture);
    let state = &fixture.economic.native.state;
    let admission = native_preparation_new_admission(&fixture);
    let admission_bytes = norito::encode_canonical(&admission).unwrap();
    let carrier = native_preparation_carrier(
        &fixture,
        vec![admission_bytes.clone()],
        Some(pulse),
        Duration::ZERO,
        false,
    );
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[])
        .unwrap()
    else {
        panic!("real finalized first input and exact four-validator Decisions");
    };
    assert_eq!(source.groups_for_test().len(), 1);
    assert_eq!(
        source.groups_for_test()[0].contexts().len(),
        if atomic { 2 } else { 1 }
    );
    let original_groups = source.groups_for_test().as_ptr();
    let original_body = source.groups_for_test()[0]
        .body()
        .canonical_bytes()
        .as_ptr();
    let original_decisions = source.groups_for_test()[0].decisions().as_ptr();
    let original_contexts = source.groups_for_test()[0].contexts().as_ptr();
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(carrier.header().creation_time());
    let prepared = source
        .prepare_candidate(
            fixture.applying.clone(),
            &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
            &clock,
            state.sumeragi_block_cadence(),
        )
        .unwrap()
        .expect("same exact applying predecessor");
    assert_eq!(prepared.context(), fixture.applying.context());
    let custody = prepared
        .native_source_for_test()
        .expect("actual Native custody");
    assert_eq!(custody.context().context(), fixture.applying.context());
    assert_eq!(custody.sources_for_test().as_ptr(), original_groups);
    assert_eq!(
        custody.sources_for_test()[0]
            .body()
            .canonical_bytes()
            .as_ptr(),
        original_body
    );
    assert_eq!(
        custody.sources_for_test()[0].decisions().as_ptr(),
        original_decisions
    );
    assert_eq!(
        custody.sources_for_test()[0].contexts().as_ptr(),
        original_contexts
    );
    assert_eq!(prepared.block().canonical_resultless_proposal(), carrier);
    prepared.block().validate_proposal_commitments().unwrap();
    prepared
        .block()
        .validate_execution_result_structure()
        .unwrap();
    prepared.block().validate_output_merkle_cache().unwrap();
    assert_eq!(prepared.block().network_entrypoint_count(), 1);
    assert_eq!(prepared.block().execution_outputs().len(), 1);
    assert!(prepared.block().execution_outputs()[0].result().is_ok());
    let wire = prepared.block().encode_wire().unwrap();
    let commitment = prepared.execution_prefix_commitment();
    commitment.validate().unwrap();
    assert_eq!(
        commitment.executed_block_wire_len,
        u64::try_from(wire.len()).unwrap()
    );
    assert_eq!(commitment.executed_block_wire_hash, Hash::new(wire));
    assert_eq!(commitment.native_amx_application_manifest_count, 0);
    assert!(
        prepared.native_amx_manifest().entries().is_empty(),
        "no legacy participant receipt is fabricated for Native Decisions"
    );
    let overlay = prepared.state();
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
    assert_eq!(
        overlay.world.global_beacon_pulses.get(&pulse.pulse_id),
        Some(&pulse)
    );
    assert_eq!(
        prepared
            .block()
            .execution_context()
            .unwrap()
            .queue_plan_admissions,
        vec![admission_bytes]
    );
    let height = carrier.header().height().get();
    for pending in [&fixture.later, &admission] {
        let binding = State::pending_queue_plan_binding_for_execution(
            overlay,
            &pending.entrypoint,
            &pending.routing_plan().unwrap(),
            height,
        )
        .unwrap()
        .expect("later inputs remain pending after the earlier head executes");
        assert_eq!(
            binding.canonical_hash(),
            pending.certificate.binding.canonical_hash()
        );
        assert!(
            overlay
                .transactions
                .get(&pending.entrypoint.hash())
                .is_none()
        );
    }
    let next = overlay.lane_consensus_contexts.get();
    assert_eq!(next.contexts.len(), if atomic { 2 } else { 1 });
    for context in &next.contexts {
        assert_eq!(context.opening_global_height, height);
        assert_eq!(
            context.opening_global_context_id,
            fixture.applying.context().id()
        );
        assert_eq!(
            context.admitted_binding_hash,
            fixture.later.certificate.binding.canonical_hash()
        );
        assert_eq!(context.committee.len(), 4);
        assert_eq!(context.da_layout.encoding, PayloadEncoding::ReedSolomon16);
    }
    assert!(
        crate::block::ValidBlock::validate_inactive_native_carrier_for_test(prepared.block())
            .unwrap_err()
            .to_string()
            .contains("not active")
    );
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, std::convert::Infallible>(()))
        .unwrap();
    assert_eq!(journals.execution_prefix_commitment(), commitment);
    assert_eq!(
        journals
            .native_source_for_test()
            .unwrap()
            .sources_for_test()
            .as_ptr(),
        original_groups
    );
    assert_eq!(
        journals
            .native_source_for_test()
            .unwrap()
            .context()
            .context(),
        fixture.applying.context()
    );
    // Actual detachment releases the original writers while retaining every
    // source; it does not reconstruct State to send the result across threads.
    drop(state.block(carrier.header()));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert!(state.kura.v2_finality_artifact(height).unwrap().is_none());
    assert_native_economic_relay_recorder_released();
    drop(fixture);
    let journals = std::thread::spawn(move || journals).join().unwrap();
    let custody = journals.native_source_for_test().unwrap();
    assert_eq!(custody.sources_for_test().as_ptr(), original_groups);
    assert_eq!(
        custody.sources_for_test()[0]
            .body()
            .canonical_bytes()
            .as_ptr(),
        original_body
    );
    assert_eq!(
        custody.sources_for_test()[0].decisions().as_ptr(),
        original_decisions
    );
    assert_eq!(
        custody.sources_for_test()[0].contexts().as_ptr(),
        original_contexts
    );
    drop(journals);
}

state_test! { sync native_preparation_single_retains_real_suffix_controls_and_unpublished_outputs
    assert_native_preparation_success(false);
}

state_test! { sync native_preparation_atomic_retains_real_suffix_controls_and_unpublished_outputs
    assert_native_preparation_success(true);
}

fn assert_native_preparation_refusal(
    fixture: &NativeControlExecutionFixture,
    carrier: SignedBlock,
    diagnostic: &str,
) {
    use super::NativeLaneBatchSourcePreparationV1;
    let state = &fixture.economic.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[])
        .unwrap()
    else {
        panic!("global defect does not fabricate or alter Native source Decisions");
    };
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(carrier.header().creation_time());
    let error = source
        .prepare_candidate(
            fixture.applying.clone(),
            &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
            &clock,
            state.sumeragi_block_cadence(),
        )
        .err()
        .expect("global validation refuses before producing a prepared owner");
    assert!(
        matches!(error, MergeLedgerCommitError::ExecutionBatchInvalid(_)),
        "{error}"
    );
    assert!(
        error.to_string().to_lowercase().contains(diagnostic),
        "{error}"
    );
    drop(state.block(carrier.header()));
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_preparation_rejects_signed_noncanonical_time
    let fixture = native_control_execution_fixture(false);
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::from_millis(1), false);
    assert_native_preparation_refusal(&fixture, carrier, "canonical logical time");
}

state_test! { sync native_preparation_rejects_signed_confidential_policy_substitution
    let fixture = native_control_execution_fixture(false);
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, true);
    assert_native_preparation_refusal(&fixture, carrier, "confidential");
}

state_test! { sync native_preparation_rejects_wrong_and_multiple_origin_signatures
    let fixture = native_control_execution_fixture(true);
    let context = fixture.applying.context();
    let keys = native_preparation_global_keys(context);
    let original = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, false);
    let leader = context.leader(original.header().view_change_index());
    let other = (leader + 1) % u32::try_from(keys.len()).unwrap();
    for multiple in [false, true] {
        let mut carrier = original.clone();
        let mut signatures = BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                u64::from(other), iroha_crypto::SignatureOf::from_hash(
                    keys[usize::try_from(other).unwrap()].private_key(), carrier.hash(),
                ),
            ),
        ]);
        if multiple {
            signatures.extend(original.signatures().cloned());
        }
        carrier.replace_signatures(signatures).unwrap();
        assert_eq!(carrier.signatures().len(), if multiple { 2 } else { 1 });
        assert_native_preparation_refusal(&fixture, carrier, "signature");
    }
    // Keep a same-index bad signature distinct from an honestly signed wrong leader.
    let mut carrier = original;
    carrier.replace_signatures(BTreeSet::from([
        iroha_data_model::block::BlockSignature::new(
            u64::from(leader), iroha_crypto::SignatureOf::from_hash(
                keys[usize::try_from(other).unwrap()].private_key(), carrier.hash(),
            ),
        ),
    ])).unwrap();
    assert_native_preparation_refusal(&fixture, carrier, "signature");
}

state_test! { sync native_preparation_rejects_stale_source_without_execution_or_publication
    use super::NativeLaneBatchSourcePreparationV1;
    let fixture = native_control_execution_fixture(false);
    let state = &fixture.economic.native.state;
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, false);
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("original source"); };
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    drop(state.begin_state_view_write());
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(carrier.header().creation_time());
    assert!(source.prepare_candidate(
        fixture.applying, &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
        &clock, state.sumeragi_block_cadence(),
    ).unwrap().is_none(), "equal data does not renew an obsolete source observation");
    drop(state.block(carrier.header()));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_preparation_retained_prefix_does_not_authorize_raw_state_commit
    let fixture = native_control_execution_fixture(false);
    assert_native_preparation_raw_commit_refusal(fixture);
}

#[inline(never)]
fn assert_native_preparation_raw_commit_refusal(fixture: NativeControlExecutionFixture) {
    use super::NativeLaneBatchSourcePreparationV1;
    let state = &fixture.economic.native.state;
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, false);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[])
        .unwrap()
    else {
        panic!("real first source");
    };
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(carrier.header().creation_time());
    let prepared = source
        .prepare_candidate(
            fixture.applying.clone(),
            &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
            &clock,
            state.sumeragi_block_cadence(),
        )
        .unwrap()
        .unwrap();
    assert!(prepared.native_source_for_test().is_some());
    let overlay = prepared.into_state_for_test();
    assert_eq!(
        overlay.commit().unwrap_err(),
        TransactionsBlockError::MergeAdmission
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

// This is a real later QueuePlan control publication with zero Network work,
// using the same State admission/finality owners as the established control
// fixture. It does not publish or simulate execution of the retained Native head.
#[inline(never)]
fn native_preparation_publish_later_admission(
    fixture: &NativeControlExecutionFixture,
) -> SignedBlock {
    let state = &fixture.economic.native.state;
    let admitted = native_preparation_new_admission(fixture);
    let encoded = norito::encode_canonical(&admitted).unwrap();
    let mut block = native_preparation_carrier(fixture, Vec::new(), None, Duration::ZERO, false);
    block.set_execution_context(Some(
        BlockExecutionContextBundle::default().with_queue_plan_admissions(vec![encoded.clone()]),
    ));
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
    let keys = native_preparation_global_keys(fixture.applying.context());
    let leader = fixture
        .applying
        .context()
        .leader(block.header().view_change_index());
    block
        .replace_signatures(BTreeSet::from([
            iroha_data_model::block::BlockSignature::new(
                u64::from(leader),
                iroha_crypto::SignatureOf::from_hash(
                    keys[usize::try_from(leader).unwrap()].private_key(),
                    block.hash(),
                ),
            ),
        ]))
        .unwrap();
    crate::sumeragi::v2_body_store::verify_origin_block_signature(
        fixture.applying.context(),
        &block,
        &crate::sumeragi::v2_body_store::BlockSignaturePolicy::RotatingLeader,
    )
    .unwrap();
    block.validate_proposal_commitments().unwrap();
    block.validate_execution_result_structure().unwrap();
    let original = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let heads = original
        .contexts()
        .iter()
        .map(|context| context.frozen().clone())
        .collect::<Vec<_>>();
    let mut overlay = state
        .block_with_queue_plan_admissions(block.header(), &[encoded])
        .unwrap();
    overlay
        .finalize_lane_consensus_contexts(&block, Some(fixture.applying.context()))
        .unwrap();
    assert_eq!(overlay.lane_consensus_contexts.get().contexts, heads);
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
    let (artifact, receipt) = stage_lane_context_fixture_finality(
        state,
        &block,
        fixture.applying.context().clone(),
        witness,
    );
    artifact.verify().unwrap();
    state
        .kura
        .promote_kagemusha_finality_sidecar(&artifact, &receipt)
        .unwrap();
    assert!(!original.is_current(state));
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(current.carrier_height(), block.header().height().get());
    assert_eq!(
        current
            .contexts()
            .iter()
            .map(|context| context.frozen().clone())
            .collect::<Vec<_>>(),
        heads
    );
    let view = state.view();
    for input in [&fixture.later, &admitted] {
        assert!(
            State::pending_queue_plan_binding_for_execution(
                &view,
                &input.entrypoint,
                &input.routing_plan().unwrap(),
                block.header().height().get() + 1,
            )
            .unwrap()
            .is_some()
        );
    }
    assert_eq!(
        view.world()
            .assets()
            .get(&fixture.economic.source)
            .unwrap()
            .0,
        Quantity::from(100u32)
    );
    assert!(
        view.world()
            .assets()
            .get(&fixture.economic.destination)
            .is_none()
    );
    block
}

state_test! { sync native_preparation_refreshes_source_after_actual_finalized_height_advance
    let fixture = native_control_execution_fixture(false);
    assert_native_preparation_after_height_advance(fixture);
}

#[inline(never)]
fn assert_native_preparation_after_height_advance(fixture: NativeControlExecutionFixture) {
    use super::NativeLaneBatchSourcePreparationV1;
    let state = &fixture.economic.native.state;
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, false);
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[])
        .unwrap()
    else {
        panic!("original source before actual publication");
    };
    let original_snapshot = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let original_height = state.view().height();
    let original_generation = state.state_view_generation();
    let published = native_preparation_publish_later_admission(&fixture);
    assert_eq!(state.view().height(), original_height + 1);
    assert_eq!(published.header().height(), carrier.header().height());
    assert_ne!(published.hash(), carrier.hash());
    assert_ne!(state.state_view_generation(), original_generation);
    assert_eq!(state.state_view_generation() % 2, 0);
    let after = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    assert_ne!(after, original_snapshot);
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let generation = state.state_view_generation();
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(carrier.header().creation_time());
    assert!(
        source
            .prepare_candidate(
                fixture.applying,
                &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
                &clock,
                state.sumeragi_block_cadence(),
            )
            .unwrap()
            .is_none(),
        "a finalized successor makes the retained source obsolete, not the proposal invalid"
    );
    assert_eq!(state.view().height(), original_height + 1);
    assert_eq!(state.state_view_generation(), generation);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        after
    );
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    drop(state.block(carrier.header()));
    assert_native_economic_relay_recorder_released();
}

/// Read-only bridge from the genuine Native control fixture to terminal tests.
pub(super) struct NativePublicationFixture {
    original: NativeControlExecutionFixture,
    carrier: SignedBlock,
    admission: iroha_data_model::block::lane_admission::LaneAdmittedInputV1,
    pulse: iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1,
}

/// End original genesis/State construction before the terminal assertion frame.
#[inline(never)]
pub(super) fn native_publication_fixture(atomic: bool) -> Box<NativePublicationFixture> {
    native_publication_fixture_from_control(native_control_execution_fixture(atomic))
}

#[inline(never)]
fn native_publication_fixture_from_control(
    original: NativeControlExecutionFixture,
) -> Box<NativePublicationFixture> {
    let pulse = native_control_requested_beacon(&original);
    let admission = native_preparation_new_admission(&original);
    let carrier = native_preparation_carrier(
        &original,
        vec![norito::encode_canonical(&admission).unwrap()],
        Some(pulse),
        Duration::ZERO,
        false,
    );
    Box::new(NativePublicationFixture {
        original,
        carrier,
        admission,
        pulse,
    })
}

impl NativePublicationFixture {
    /// Original State whose authenticated admission and sources are retained.
    pub(super) fn state(&self) -> &State {
        &self.original.economic.native.state
    }

    /// Exact applying context verified from the original parent's durable QC.
    pub(super) fn context(&self) -> &HeightContext {
        self.original.applying.context()
    }

    /// Canonical signed proposal before recorded Native execution.
    pub(super) fn carrier(&self) -> &SignedBlock {
        &self.carrier
    }

    /// Actual source and destination of the single executed transfer.
    pub(super) fn assets(&self) -> (&AssetId, &AssetId) {
        (
            &self.original.economic.source,
            &self.original.economic.destination,
        )
    }

    /// Real later pending input plus this carrier's admitted input.
    pub(super) fn pending_inputs(
        &self,
    ) -> [&iroha_data_model::block::lane_admission::LaneAdmittedInputV1; 2] {
        [&self.original.later, &self.admission]
    }

    /// Mandatory authenticated beacon retained by the signed proposal.
    pub(super) fn pulse(
        &self,
    ) -> &iroha_data_model::consensus::FinalizedGlobalThresholdBeaconPulseV1 {
        &self.pulse
    }

    /// Execute through the real global preflight and sole Native source owner.
    pub(super) fn prepare(&self) -> super::carrier_preparation::PreparedCarrier<'_> {
        let super::NativeLaneBatchSourcePreparationV1::Ready(source) = self
            .state()
            .prepare_proposed_native_lane_batch_source(&self.carrier, &[])
            .unwrap()
        else {
            panic!("genuine finalized Native source must be ready");
        };
        let (_, clock) =
            iroha_primitives::time::TimeSource::new_mock(self.carrier.header().creation_time());
        source
            .prepare_candidate(
                self.original.applying.clone(),
                &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
                &clock,
                self.state().sumeragi_block_cadence(),
            )
            .unwrap()
            .expect("original applying generation remains current")
    }

    /// Authenticate the actual retained output commitment with this committee.
    pub(super) fn finality(
        &self,
        block: &SignedBlock,
        execution_commitment: ExecutionCommitment,
    ) -> crate::block::VerifiedV2FinalityArtifact {
        let context = self.context().clone();
        let keys = native_preparation_global_keys(&context);
        let subject = BlockSubject {
            parent_block_hash: block.header().prev_block_hash(),
            block_hash: block.hash(),
            payload_hash: block.canonical_proposal_wire_hash().unwrap(),
        };
        let round = ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: block.header().view_change_index(),
        };
        let vote = iroha_data_model::block::consensus_v2::Vote {
            round,
            proposal_round: round,
            phase: GlobalPhase::Commit,
            subject,
            execution_commitment,
            signer: 0,
            signature: Vec::new(),
        };
        let shares = keys[..3]
            .iter()
            .map(|key| {
                Signature::new(key.private_key(), &vote.signature_preimage())
                    .payload()
                    .to_vec()
            })
            .collect::<Vec<_>>();
        let artifact = V2FinalityArtifact::new(
            context,
            subject,
            QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(
                    &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
                )
                .unwrap(),
            },
            keys.iter()
                .map(|key| bls_normal_pop_prove(key.private_key()).unwrap())
                .collect(),
        );
        crate::block::VerifiedV2FinalityArtifact::verify(artifact)
            .expect("real three-of-four global finality joins the actual Native execution")
    }
}
