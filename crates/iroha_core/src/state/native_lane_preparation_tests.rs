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
    let fixture = native_control_execution_fixture(atomic, true);
    assert_native_preparation_success_in_fixture(fixture, atomic);
}

// Fixture construction acquires real genesis State journals. Finish that phase
// before reserving the assertion frame's later candidate/journal result slots;
// neither phase needs a larger thread stack or a different ownership contract.
#[inline(never)]
fn assert_native_preparation_success_in_fixture(
    fixture: NativeControlExecutionFixture,
    atomic: bool,
) {
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

// Keep the real fixture constructor off the later journal/authentication frame.
fn assert_native_durable_source_authentication(atomic: bool) {
    let fixture = native_control_execution_fixture(atomic, true);
    assert_native_durable_source_authentication_in_fixture(fixture);
}

#[inline(never)]
fn assert_native_durable_source_authentication_in_fixture(fixture: NativeControlExecutionFixture) {
    let state = &fixture.economic.native.state;
    let carrier = native_preparation_carrier(
        &fixture,
        Vec::new(),
        Some(native_control_requested_beacon(&fixture)),
        Duration::ZERO,
        false,
    );
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let super::NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[])
        .unwrap()
    else {
        panic!("real original Native sources must be ready");
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
    let block = prepared.block().clone();
    let commitment = prepared.execution_prefix_commitment();
    let journals = prepared
        .prepare_journals(None, None, |_| Ok::<_, std::convert::Infallible>(()))
        .unwrap();
    let prefix = journals.source_prefix();
    let native = journals.native_source_for_test().unwrap();
    let groups = native.sources_for_test().as_ptr();
    let source_body = native.sources_for_test()[0].body();
    let bytes = source_body.canonical_bytes().as_ptr();
    let decisions = native.sources_for_test()[0].decisions().as_ptr();
    let contexts = native.sources_for_test()[0].contexts().as_ptr();
    let original = source_body.source();
    let source_height = original.priority().carrier_height;
    let source_hash = original.carrier_hash();
    let inventory = Arc::clone(prefix.inventory());
    let writes = prefix.witness().writes.as_ptr();

    // Sign the actual retained execution; a fixture-only execution commitment or
    // old Native participant receipt cannot substitute for the real witness.
    let context = fixture.applying.context();
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
    let mut qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let keys = native_preparation_global_keys(context);
    let preimage = qc.signer_preimage(context, 0).unwrap();
    let shares = keys
        .iter()
        .take(3)
        .map(|key| {
            Signature::try_new(key.private_key(), &preimage)
                .unwrap()
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(
        &shares.iter().map(Vec::as_slice).collect::<Vec<_>>(),
    )
    .unwrap();
    let finality = V2FinalityArtifact::new(
        context.clone(),
        subject,
        qc,
        keys.iter()
            .map(|key| bls_normal_pop_prove(key.private_key()).unwrap())
            .collect(),
    );
    finality.verify().unwrap();
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    let _ = state.kura.store_v2_finality_artifact(&finality).unwrap();
    let authenticate = || {
        let lease = state.kura.try_publication_lease().unwrap();
        prefix.authenticate_durable_carrier(&block, context, &commitment, &lease)
    };
    authenticate().unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let path = state
        .kura
        .v2_finality_artifact_path_for_testing(source_height);
    let original_finality = std::fs::read(&path).unwrap();
    for corrupt in [false, true] {
        if corrupt {
            std::fs::write(&path, b"corrupt original first-carrier finality").unwrap();
        } else {
            std::fs::remove_file(&path).unwrap();
        }
        let occupied = exact_test_tree_fingerprint(&state.kura.store_root());
        let error = authenticate().unwrap_err();
        assert!(
            std::error::Error::source(&error).is_some(),
            "storage provenance: {error}"
        );
        assert_eq!(
            exact_test_tree_fingerprint(&state.kura.store_root()),
            occupied
        );
        std::fs::write(&path, &original_finality).unwrap();
        authenticate().unwrap();
        assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    }
    state
        .kura
        .evict_first_admission_body_for_testing(
            NonZeroUsize::new(usize::try_from(source_height).unwrap()).unwrap(),
            source_hash,
        )
        .unwrap();
    let absent = exact_test_tree_fingerprint(&state.kura.store_root());
    authenticate()
        .expect("original privately verified source survives authenticated local absence");
    assert_eq!(
        exact_test_tree_fingerprint(&state.kura.store_root()),
        absent
    );
    assert_eq!(native.sources_for_test().as_ptr(), groups);
    assert_eq!(
        native.sources_for_test()[0]
            .body()
            .canonical_bytes()
            .as_ptr(),
        bytes
    );
    assert_eq!(native.sources_for_test()[0].decisions().as_ptr(), decisions);
    assert_eq!(native.sources_for_test()[0].contexts().as_ptr(), contexts);
    assert!(Arc::ptr_eq(prefix.inventory(), &inventory));
    assert_eq!(prefix.witness().writes.as_ptr(), writes);
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
    drop(state.world.block());
    drop(state.transactions.block());

    // Historical source absence is not permission to publish without the actual
    // result-bearing applying carrier authenticated by this execution seal.
    state
        .kura
        .evict_first_admission_body_for_testing(
            NonZeroUsize::new(usize::try_from(context.height).unwrap()).unwrap(),
            block.hash(),
        )
        .unwrap();
    let absent_current = exact_test_tree_fingerprint(&state.kura.store_root());
    let error = authenticate().unwrap_err();
    assert!(
        error
            .to_string()
            .contains("exact body recovery is required"),
        "{error}"
    );
    assert_eq!(
        exact_test_tree_fingerprint(&state.kura.store_root()),
        absent_current
    );
    assert_eq!(
        crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
        before
    );
}

state_test! { sync native_preparation_single_authenticates_original_durable_sources_under_lease
    assert_native_durable_source_authentication(false);
}

state_test! { sync native_preparation_atomic_authenticates_original_durable_sources_under_lease
    assert_native_durable_source_authentication(true);
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
        matches!(
            error,
            crate::block::valid::NativeCandidatePreparationError::Preflight(_)
        ),
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
    let fixture = native_control_execution_fixture(false, false);
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::from_millis(1), false);
    assert_native_preparation_refusal(&fixture, carrier, "canonical logical time");
}

state_test! { sync native_preparation_rejects_signed_confidential_policy_substitution
    let fixture = native_control_execution_fixture(false, false);
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, true);
    assert_native_preparation_refusal(&fixture, carrier, "confidential");
}

state_test! { sync native_preparation_rejects_wrong_and_multiple_origin_signatures
    let fixture = native_control_execution_fixture(true, false);
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
    let fixture = native_control_execution_fixture(false, false);
    let state = &fixture.economic.native.state;
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, false);
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("original source"); };
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    {
        let mut publication_notice = state.state_view_publication();
        drop(publication_notice.begin());
    }
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

state_test! { sync native_preparation_preserves_local_recorder_conflict
    use super::NativeLaneBatchSourcePreparationV1;
    use crate::block::valid::NativeCandidatePreparationError;
    let fixture = native_control_execution_fixture(false, false);
    let state = &fixture.economic.native.state;
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, false);
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
        else { panic!("original source"); };
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(carrier.header().creation_time());
    let recorder = crate::sumeragi::witness::begin_exec_witness_capture().unwrap();
    let error = source.prepare_candidate(
        fixture.applying, &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
        &clock, state.sumeragi_block_cadence(),
    ).err().expect("existing recorder refuses before State execution");
    assert!(matches!(error, NativeCandidatePreparationError::Execution(
        MergeLedgerCommitError::ExecutionRecorderConflict(_)
    )), "{error}");
    drop(recorder);
    drop(state.block(carrier.header()));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_preparation_retained_prefix_does_not_authorize_raw_state_commit
    let fixture = native_control_execution_fixture(false, false);
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
    assert!(matches!(
        overlay.commit().unwrap_err(),
        TransactionsBlockError::MergeAdmission
    ));
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
    let fixture = native_control_execution_fixture(false, false);
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
    assert_eq!(
        state
            .native_proposal_superseded(carrier.header().height().get())
            .unwrap(),
        Some(false)
    );
    let published = native_preparation_publish_later_admission(&fixture);
    assert_eq!(state.view().height(), original_height + 1);
    assert_eq!(published.header().height(), carrier.header().height());
    assert_ne!(published.hash(), carrier.hash());
    assert_ne!(state.state_view_generation(), original_generation);
    assert_eq!(state.state_view_generation() % 2, 0);
    assert_eq!(
        state
            .native_proposal_superseded(carrier.header().height().get())
            .unwrap(),
        Some(true)
    );
    assert!(matches!(
        state
            .prepare_proposed_native_lane_batch_source(&carrier, &[])
            .unwrap(),
        NativeLaneBatchSourcePreparationV1::Superseded
    ));
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
    native_publication_fixture_from_control(native_control_execution_fixture(atomic, true))
}

/// Seed the canonical genesis authority before any signed admission, so the
/// genuine published Native carrier can enter the production replay consumer.
#[inline(never)]
pub(super) fn native_publication_replay_fixture(atomic: bool) -> Box<NativePublicationFixture> {
    let economic = native_economic_fixture_with_world_initializer(
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
        None,
        |world| {
            let authority = iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID.clone();
            let (id, account) = Account::new(authority.clone())
                .build(&authority)
                .into_key_value();
            world.accounts.insert(id, account);
            let domain_id = iroha_genesis::GENESIS_DOMAIN_ID.clone();
            world
                .domains
                .insert(domain_id.clone(), Domain::new(domain_id).build(&authority));
        },
    );
    native_publication_fixture_from_control(native_control_execution_fixture_from_economic(
        economic, atomic, true,
    ))
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
    /// Borrow the genuine fixture's configured physical limits.
    pub(super) fn process_limits() -> crate::sumeragi::v2_lane_instance::LaneProcessLimits {
        native_process_limits_for_test()
    }

    /// Select a real key from the exact frozen four-validator committee.
    pub(super) fn key_for(&self, lane: &VerifiedLaneContext, signer: usize) -> KeyPair {
        self.original
            .economic
            .native
            .validators
            .iter()
            .find(|key| key.public_key() == lane.frozen().committee[signer].public_key())
            .unwrap()
            .clone()
    }

    /// Move the same State family after actual journal detachment, without
    /// introducing a second instance, physical pool or publication authority.
    pub(super) fn into_shared_state(self: Box<Self>) -> Arc<State> {
        Arc::from(self.original.economic.native.state)
    }

    /// Open real local WAL/body owners for the already authenticated Decisions.
    /// Alternate quorum signers prove that publication settles the value, while
    /// preserving the exact local certificate/effect rather than replacing it.
    pub(super) fn local_apply_owners(
        &self,
    ) -> Vec<(
        VerifiedLaneContext,
        crate::sumeragi::v2_lane_instance::LaneInstance,
    )> {
        use crate::sumeragi::{
            output_guard::ConsensusOutputGuard,
            v2_core as core,
            v2_lane_instance::{
                LaneBodyLaunch, LaneBodyProgress, LaneBodyWait, LaneInputOutcome, LaneInstance,
                LaneService,
            },
        };
        use iroha_data_model::block::lane_consensus::LaneMessageV1;
        let state = self.state();
        let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
        let batch = self
            .carrier
            .execution_context()
            .unwrap()
            .native_lane_decisions
            .as_ref()
            .unwrap();
        batch
            .groups
            .iter()
            .flat_map(|group| &group.decisions)
            .map(|published| {
                let lane = observed
                    .contexts()
                    .iter()
                    .find(|lane| {
                        Hash::from(lane.instance_id().0) == published.manifest.value.instance_id
                    })
                    .unwrap();
                let key = self
                    .original
                    .economic
                    .native
                    .validators
                    .iter()
                    .find(|key| {
                        key.public_key()
                            == lane.frozen().committee
                                [published.manifest.value.origin_producer as usize]
                                .public_key()
                    })
                    .unwrap()
                    .clone();
                let now = std::time::Instant::now();
                let mut owner = LaneInstance::open_with_worker_for_test(
                    state,
                    &observed,
                    lane,
                    key,
                    ConsensusOutputGuard::isolated(),
                    now,
                    Duration::from_secs(1),
                    Duration::from_millis(100),
                    3 * core::MAX_EFFECTS_PER_STEP,
                )
                .unwrap();
                let LaneBodyLaunch::Job(job) = owner.take_body_job(state, &observed).unwrap()
                else {
                    panic!("actual original producer must own its physical source job");
                };
                let completed = run_native_lane_body_worker_for_test(job, state);
                let mut original = published.clone();
                self.resign_local_decision(lane, &mut original);
                assert_ne!(original.commit_qc.shares, published.commit_qc.shares);
                assert_eq!(original.manifest, published.manifest);
                assert!(matches!(
                    owner
                        .offer(
                            state,
                            &observed,
                            &LaneMessageV1::QuorumCertificate(original.commit_qc.clone())
                        )
                        .unwrap(),
                    LaneInputOutcome::Stepped(_)
                ));
                assert!(matches!(
                    owner.service_with_worker(state, &observed, now).unwrap(),
                    LaneService::PersistedAwaitingAck
                ));
                assert!(matches!(
                    owner.finish_body_job(completed, state, &observed).unwrap(),
                    LaneBodyProgress::Waiting(LaneBodyWait::ControlCompletion)
                ));
                service_native_lane_control_for_body_test(&mut owner, state, &observed, now);
                assert!(matches!(
                    owner.service_body_completion(state, &observed).unwrap(),
                    LaneBodyProgress::Stepped(_)
                ));
                assert_eq!(owner.native_decision().unwrap().as_ref(), Some(&original));
                // The earlier local preparation is not the Commit's issued
                // Fetch/Store/Validate chain. Finish those actual physical jobs
                // before asking the reducer to expose its original Apply.
                for _ in 0..8 {
                    if owner
                        .held_effects()
                        .any(|effect| matches!(effect, core::Effect::Apply { .. }))
                    {
                        break;
                    }
                    service_native_lane_control_for_body_test(&mut owner, state, &observed, now);
                    let _ = owner.service_body_completion(state, &observed).unwrap();
                    let _ = run_one_native_lane_body_job_for_test(&mut owner, state, &observed);
                }
                assert_eq!(owner.native_decision().unwrap().as_ref(), Some(&original));
                assert!(
                    owner
                        .held_effects()
                        .any(|effect| matches!(effect, core::Effect::Apply { .. })),
                    "actual committed body work must produce its original Apply; retained: {:?}",
                    owner.held_effects().collect::<Vec<_>>()
                );
                (lane.clone(), owner)
            })
            .collect()
    }

    /// Open a real matching instance from a separately restored State family.
    /// The exact snapshot, Kura and frozen context match; the original State
    /// owner does not. A different genuine committee signer avoids sharing the
    /// original instance's physical WAL/body writer.
    #[inline(never)]
    pub(super) fn foreign_apply_owner(
        &self,
    ) -> Box<crate::sumeragi::v2_lane_instance::LaneInstance> {
        use crate::sumeragi::{
            output_guard::ConsensusOutputGuard,
            v2_core as core,
            v2_lane_instance::{LaneInputOutcome, LaneInstance},
        };
        use iroha_data_model::block::lane_consensus::LaneMessageV1;
        let foreign = self.restored_foreign_state();
        let observed = foreign.verified_lane_consensus_contexts().unwrap().unwrap();
        let published = &self
            .carrier
            .execution_context()
            .unwrap()
            .native_lane_decisions
            .as_ref()
            .unwrap()
            .groups[0]
            .decisions[0];
        let lane = observed
            .contexts()
            .iter()
            .find(|lane| Hash::from(lane.instance_id().0) == published.manifest.value.instance_id)
            .unwrap();
        let original = self
            .state()
            .verified_lane_consensus_contexts()
            .unwrap()
            .unwrap();
        assert!(
            original
                .contexts()
                .iter()
                .any(|context| context.frozen() == lane.frozen())
        );
        let signer =
            (published.manifest.value.origin_producer as usize + 2) % lane.frozen().committee.len();
        let key = self
            .original
            .economic
            .native
            .validators
            .iter()
            .find(|key| key.public_key() == lane.frozen().committee[signer].public_key())
            .unwrap()
            .clone();
        let now = std::time::Instant::now();
        let mut owner = Box::new(
            LaneInstance::open_with_worker_for_test(
                &foreign,
                &observed,
                lane,
                key,
                ConsensusOutputGuard::isolated(),
                now,
                Duration::from_secs(1),
                Duration::from_millis(100),
                3 * core::MAX_EFFECTS_PER_STEP,
            )
            .unwrap(),
        );
        assert!(!owner.state_owner_for_test().matches_state(self.state()));
        assert!(owner.state_owner_for_test().matches_state(&foreign));
        let mut decision = published.clone();
        self.resign_local_decision(lane, &mut decision);
        assert!(matches!(
            owner
                .offer(
                    &foreign,
                    &observed,
                    &LaneMessageV1::QuorumCertificate(decision.commit_qc.clone())
                )
                .unwrap(),
            LaneInputOutcome::Stepped(_)
        ));
        for _ in 0..8 {
            service_native_lane_control_for_body_test(&mut owner, &foreign, &observed, now);
            let _ = owner.service_body_completion(&foreign, &observed).unwrap();
            let _ = run_one_native_lane_body_job_for_test(&mut owner, &foreign, &observed);
            if owner
                .held_effects()
                .any(|effect| matches!(effect, core::Effect::Apply { .. }))
            {
                assert_eq!(owner.native_decision().unwrap().as_ref(), Some(&decision));
                return owner;
            }
        }
        panic!("actual foreign State WAL/body work must produce its original Apply");
    }

    // End snapshot decoding before opening any physical lane owner. No snapshot
    // field or context is rewritten to make the foreign family appear current.
    #[inline(never)]
    fn restored_foreign_state(&self) -> Box<State> {
        let state = self.state();
        let restored = deserialize::KuraSeed {
            kura: Arc::clone(&state.kura),
            lane_manifests: state.lane_manifests.read().clone(),
            query_handle: LiveQueryStore::start_test(),
            #[cfg(feature = "telemetry")]
            telemetry: crate::telemetry::StateTelemetry::default(),
        }
        .into_state_from_json(norito::json::to_value(state).unwrap())
        .expect("restore the genuine original snapshot into a distinct State family");
        assert!(restored.matches_kura_instance(&state.kura));
        assert_eq!(restored.network_id, state.network_id);
        assert_eq!(
            restored.latest_block_hash_fast(),
            state.latest_block_hash_fast()
        );
        restored
    }

    /// Genuine alternate 2f+1 certificate from this exact four-validator fixture.
    pub(super) fn resign_local_decision(
        &self,
        lane: &VerifiedLaneContext,
        decision: &mut iroha_data_model::block::lane_consensus::LaneDecisionV1,
    ) {
        use iroha_data_model::block::lane_consensus::LaneSignatureShareV1;
        decision.commit_qc.statement.value = decision.manifest.value;
        let preimage = decision.commit_qc.statement.signature_preimage().unwrap();
        decision.commit_qc.shares = [1, 2, 3]
            .into_iter()
            .map(|index| {
                let key = self
                    .original
                    .economic
                    .native
                    .validators
                    .iter()
                    .find(|key| key.public_key() == lane.frozen().committee[index].public_key())
                    .unwrap();
                LaneSignatureShareV1 {
                    signer: index as u32,
                    signature: Signature::try_new(key.private_key(), &preimage)
                        .unwrap()
                        .payload()
                        .to_vec(),
                }
            })
            .collect();
        crate::sumeragi::v2_lane_wire::LaneAuthenticator::new(lane)
            .decision_certificate(decision)
            .unwrap();
    }

    /// Actual unopened-work instance: publication alone cannot mint local Ready.
    #[inline(never)]
    pub(super) fn local_unready_owner(
        &self,
    ) -> Box<crate::sumeragi::v2_lane_instance::LaneInstance> {
        let state = self.state();
        let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
        let lane = &observed.contexts()[0];
        let origin = self
            .carrier
            .execution_context()
            .unwrap()
            .native_lane_decisions
            .as_ref()
            .unwrap()
            .groups[0]
            .decisions[0]
            .manifest
            .value
            .origin_producer as usize;
        let other = (origin + 1) % lane.frozen().committee.len();
        let key = self
            .original
            .economic
            .native
            .validators
            .iter()
            .find(|key| key.public_key() == lane.frozen().committee[other].public_key())
            .unwrap()
            .clone();
        Box::new(
            crate::sumeragi::v2_lane_instance::LaneInstance::open_with_worker_for_test(
                state,
                &observed,
                lane,
                key,
                crate::sumeragi::output_guard::ConsensusOutputGuard::isolated(),
                std::time::Instant::now(),
                Duration::from_secs(1),
                Duration::from_millis(100),
                3 * crate::sumeragi::v2_core::MAX_EFFECTS_PER_STEP,
            )
            .unwrap(),
        )
    }

    /// Move the same State family into the actual process-lived driver after
    /// borrowed preparation has released every writer into owned journals.
    pub(super) fn into_shared_driver(
        self: Box<Self>,
    ) -> (
        Arc<State>,
        crate::sumeragi::v2_lane_driver::NativeLaneDriver,
    ) {
        let key = self.original.economic.native.validators[0].clone();
        let state: Arc<State> = Arc::from(self.original.economic.native.state);
        let driver = crate::sumeragi::v2_lane_driver::NativeLaneDriver::new(
            Arc::clone(&state),
            crate::sumeragi::output_guard::ConsensusOutputGuard::isolated(),
            key,
            native_driver_limits_for_test(),
        )
        .unwrap();
        (state, driver)
    }

    /// Original State whose authenticated admission and sources are retained.
    pub(super) fn state(&self) -> &State {
        &self.original.economic.native.state
    }

    /// Exact applying context verified from the original parent's durable QC.
    pub(super) fn context(&self) -> &HeightContext {
        self.original.applying.context()
    }

    /// Original authenticated applying context for the real service adapter.
    pub(super) fn verified_context(&self) -> crate::sumeragi::v2::VerifiedHeightContext {
        self.original.applying.clone()
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

fn native_preparation_snapshot_record(
    fixture: &NativeControlExecutionFixture,
) -> iroha_data_model::block::consensus_v2::SnapshotV2BootstrapRecord {
    use iroha_data_model::block::consensus_v2::{
        SnapshotBootstrapAnchor, SnapshotV2BootstrapRecord,
    };
    let state = &fixture.economic.native.state;
    let parent = fixture.economic.native.block.header();
    let mut context = fixture.applying.context().clone();
    context.parent_commit_qc = None;
    context.snapshot_bootstrap = Some(SnapshotBootstrapAnchor {
        snapshot_height: parent.height().get(),
        snapshot_block_hash: parent.hash(),
        snapshot_block_creation_time_ms: parent.creation_time_ms,
        snapshot_state_hash: crate::snapshot::canonical_state_snapshot_hash(state).unwrap(),
    });
    let record = SnapshotV2BootstrapRecord {
        version: SnapshotV2BootstrapRecord::VERSION,
        context,
        validator_set_pops: fixture.applying.proofs_of_possession().to_vec(),
    };
    record.validate().unwrap();
    record
}

state_test! { sync native_preparation_snapshot_anchor_retains_source_owned_execution
    let mut fixture = native_control_execution_fixture(false, false);
    let record = native_preparation_snapshot_record(&fixture);
    let state = &mut fixture.economic.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    state.kura.force_hash_only_block_for_testing(
        std::num::NonZeroUsize::new(state.committed_height()).unwrap(),
    ).expect("snapshot startup retains the authenticated parent hash without its body");
    assert!(state.view().latest_block().is_none());
    state.set_snapshot_v2_bootstrap_candidate_for_testing(record.clone());
    state.authenticate_snapshot_v2_bootstrap_candidate(
        crate::snapshot::SnapshotBootstrapLineageAuthority::normally_signed_for_testing(),
    ).expect("authenticate exact snapshot State, history, roster and live BLS proofs");
    assert_eq!(state.authenticated_snapshot_v2_bootstrap(), Some(&record));
    fixture.applying = crate::sumeragi::v2::VerifiedHeightContext::snapshot_bootstrap(&record).unwrap();
    let carrier = native_preparation_carrier(&fixture, Vec::new(), None, Duration::ZERO, false);
    let state = &fixture.economic.native.state;
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    let (_, clock) = iroha_primitives::time::TimeSource::new_mock(carrier.header().creation_time());
    let super::NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
    else { panic!("snapshot successor retains genuine finalized first sources"); };
    let prepared = source.prepare_candidate(
        fixture.applying.clone(), &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
        &clock, state.sumeragi_block_cadence(),
    ).unwrap().expect("shared global preflight and Native controls accept the authenticated anchor");
    assert!(prepared.block().has_results());
    assert_eq!(prepared.block().hash(), carrier.hash());
    assert_eq!(prepared.context(), &record.context);
    assert!(prepared.native_source_for_test().is_some());
    assert_eq!(prepared.state().world.assets.get(&fixture.economic.source).unwrap().0, Quantity::from(75u32));
    assert_eq!(prepared.state().world.assets.get(&fixture.economic.destination).unwrap().0, Quantity::from(25u32));
    drop(prepared);

    let mut wrong = record.clone();
    wrong.context.snapshot_bootstrap.as_mut().unwrap().snapshot_block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign Native snapshot parent"));
    let wrong = crate::sumeragi::v2::VerifiedHeightContext::snapshot_bootstrap(&wrong).unwrap();
    let error = crate::block::ValidBlock::prepare_native_execution_controls(
        &carrier, state, wrong.clone(),
    ).err().expect("control admission must reject another snapshot parent");
    assert!(error.to_string().contains("exact carrier pre-State"), "{error}");
    let super::NativeLaneBatchSourcePreparationV1::Ready(source) = state
        .prepare_proposed_native_lane_batch_source(&carrier, &[]).unwrap()
    else { panic!("bad global context cannot alter the original source"); };
    let error = source.prepare_candidate(
        wrong, &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID,
        &clock, state.sumeragi_block_cadence(),
    ).err().expect("shared preflight must reject another snapshot parent before recording");
    assert!(error.to_string().contains("verified context"), "{error}");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
    assert_native_economic_relay_recorder_released();
}

state_test! { sync native_preparation_snapshot_anchor_rejects_wrong_state_authentication
    let mut fixture = native_control_execution_fixture(false, false);
    let mut record = native_preparation_snapshot_record(&fixture);
    record.context.snapshot_bootstrap.as_mut().unwrap().snapshot_state_hash =
        Hash::new(b"foreign Native snapshot State");
    record.validate().unwrap();
    let state = &mut fixture.economic.native.state;
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    let files = exact_test_tree_fingerprint(&state.kura.store_root());
    state.set_snapshot_v2_bootstrap_candidate_for_testing(record);
    let error = state.authenticate_snapshot_v2_bootstrap_candidate(
        crate::snapshot::SnapshotBootstrapLineageAuthority::normally_signed_for_testing(),
    ).expect_err("a structurally valid anchor cannot authenticate a different State");
    assert!(error.contains("canonical snapshot WSV"), "{error}");
    assert!(state.authenticated_snapshot_v2_bootstrap().is_none());
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    assert_eq!(exact_test_tree_fingerprint(&state.kura.store_root()), files);
}
