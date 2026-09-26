// Authenticated lane Decisions through the existing global candidate assembler.
// Economic execution uses the original source consumer; no test acknowledges
// reducer Apply or activates the still-incomplete production runner cutover.

struct NativeCandidateFixture {
    state: Arc<State>,
    parent: SignedBlock,
    work: crate::sumeragi::v2_lane_driver::NativeLaneCandidateBatch,
    handoff: crate::sumeragi::v2_lane_driver::NativeLaneDecisionHandoff,
}

#[inline(never)]
fn native_candidate_fixture(cases: &[NativeEconomicCase], atomic: bool) -> NativeCandidateFixture {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput},
    };
    let fixture = native_economic_fixture_with_genesis_layout(
        cases,
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
    let groups = native_economic_groups(&fixture);
    let state: Arc<State> = Arc::from(fixture.native.state);
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let outsider = KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::BlsNormal).unwrap();
    let mut driver = NativeLaneDriver::new(
        Arc::clone(&state),
        Arc::clone(&guard),
        outsider,
        native_driver_limits_for_test(),
    )
    .unwrap();
    for group in &groups {
        for decision in group.decisions() {
            assert!(matches!(
                driver.admit(&observed, NativeLaneInput::Decision(decision.clone())),
                NativeLaneAdmission::Accepted
            ));
        }
    }
    let handoff = driver.capture_decisions(&observed).unwrap().unwrap();
    let (handoff, prepared) = std::thread::spawn(move || {
        let prepared = handoff.prepare_candidate().unwrap();
        (handoff, prepared)
    })
    .join()
    .unwrap();
    assert!(prepared.waits.is_empty());
    let work = prepared
        .work
        .expect("every exact route has a certified Decision");
    assert_eq!(
        work.batch().groups,
        groups
            .iter()
            .map(VerifiedLaneDecisionGroupV1::to_wire)
            .collect::<Vec<_>>()
    );
    assert_eq!(driver.process().occupancy().instances, 0);
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
    NativeCandidateFixture {
        state,
        parent: fixture.native.block,
        work,
        handoff,
    }
}

fn assemble_native_handoff_for_test(
    fixture: &NativeCandidateFixture,
    max_transactions: usize,
    max_bytes: usize,
    attachments: crate::sumeragi::v2_candidate::CandidateAttachments,
) -> Result<
    crate::sumeragi::v2_candidate::NativeCandidateAssembly,
    crate::sumeragi::v2_candidate::CandidateError,
> {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2::LocalProposalDirective,
        v2_candidate::{CandidateLimits, CandidateParent, CandidateRequest, V2CandidateAssembler},
        v2_core::{EventTag, Generation},
    };
    let verified =
        native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
    let context = verified.context();
    let local = context.leader(0);
    // Global and lane committees are distinct fixture authorities. Sign with
    // the actual global leader, never a key selected from the native committee.
    let keys = native_preparation_global_keys(context);
    let key = &keys[local as usize];
    let (_, time_source) = iroha_primitives::time::TimeSource::new_mock(
        fixture.parent.header().creation_time() + fixture.state.sumeragi_block_cadence(),
    );
    let queue = Arc::new(crate::queue::Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &time_source,
    ));
    let assembler = V2CandidateAssembler::new(
        CandidateLimits::new(
            NonZeroUsize::new(max_transactions).unwrap(),
            NonZeroUsize::new(max_bytes).unwrap(),
            NonZeroUsize::new(max_transactions).unwrap(),
        )
        .unwrap(),
        time_source,
    );
    let guard = ConsensusOutputGuard::isolated();
    let result = assembler.assemble_native(CandidateRequest {
        context,
        directive: LocalProposalDirective::for_test(
            EventTag::new(context.height, 0, Generation::new(0)),
            local,
            None,
            None,
            None,
        ),
        local_validator: local,
        parent: CandidateParent::Block(&fixture.parent),
        state: &fixture.state,
        queue: &queue,
        key_pair: key,
        output_guard: &guard,
        attachments,
        work_provider: &fixture.handoff,
    });
    assert!(
        !guard.restart_required(),
        "unsigned selection failures preserve the output owner"
    );
    assert_eq!(queue.queued_len(), 0);
    result
}

fn assemble_native_candidate_for_test(
    fixture: &NativeCandidateFixture,
    max_transactions: usize,
    max_bytes: usize,
    attachments: crate::sumeragi::v2_candidate::CandidateAttachments,
) -> Result<
    crate::sumeragi::v2_candidate::CandidateAssemblyOutcome,
    crate::sumeragi::v2_candidate::CandidateError,
> {
    let assembly =
        assemble_native_handoff_for_test(fixture, max_transactions, max_bytes, attachments)?;
    assert!(assembly.source.waits.is_empty());
    assert_eq!(
        assembly.source.work.as_ref().unwrap().batch(),
        fixture.work.batch(),
        "success, fitting, deferral and refusal retain every original prepared group",
    );
    assembly.outcome
}

fn assemble_prepared_native_candidate_for_test(
    fixture: &NativeCandidateFixture,
    max_transactions: usize,
    max_bytes: usize,
    attachments: crate::sumeragi::v2_candidate::CandidateAttachments,
) -> Result<
    crate::sumeragi::v2_candidate::CandidateAssemblyOutcome,
    crate::sumeragi::v2_candidate::CandidateError,
> {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2::LocalProposalDirective,
        v2_candidate::{CandidateLimits, CandidateParent, CandidateRequest, V2CandidateAssembler},
        v2_core::{EventTag, Generation},
    };
    let verified =
        native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
    let context = verified.context();
    let local = context.leader(0);
    // Global and lane committees are distinct fixture authorities. Sign with
    // the actual global leader, never a key selected from the native committee.
    let keys = native_preparation_global_keys(context);
    let key = &keys[local as usize];
    let (_, time_source) = iroha_primitives::time::TimeSource::new_mock(
        fixture.parent.header().creation_time() + fixture.state.sumeragi_block_cadence(),
    );
    let queue = Arc::new(crate::queue::Queue::test(
        iroha_config::parameters::actual::Queue::default(),
        &time_source,
    ));
    let assembler = V2CandidateAssembler::new(
        CandidateLimits::new(
            NonZeroUsize::new(max_transactions).unwrap(),
            NonZeroUsize::new(max_bytes).unwrap(),
            NonZeroUsize::new(max_transactions).unwrap(),
        )
        .unwrap(),
        time_source,
    );
    let guard = ConsensusOutputGuard::isolated();
    let result = assembler.assemble(CandidateRequest {
        context,
        directive: LocalProposalDirective::for_test(
            EventTag::new(context.height, 0, Generation::new(0)),
            local,
            None,
            None,
            None,
        ),
        local_validator: local,
        parent: CandidateParent::Block(&fixture.parent),
        state: &fixture.state,
        queue: &queue,
        key_pair: key,
        output_guard: &guard,
        attachments,
        work_provider: &fixture.work,
    });
    assert!(
        !guard.restart_required(),
        "unsigned selection failures preserve the output owner"
    );
    assert_eq!(queue.queued_len(), 0);
    result
}

state_test! { sync native_candidate_uses_exact_decisions_and_canonical_recorded_execution
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments, candidate_block_has_proposal_work};
    for atomic in [false, true] {
        let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], atomic);
        let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
        let CandidateAssemblyOutcome::Assembled(candidate) = assemble_native_candidate_for_test(
            &fixture, 16, 2 * 1024 * 1024, CandidateAttachments::default()).unwrap()
        else { panic!("a decided input is proposal work without any ordinary transaction"); };
        let block = candidate.block();
        assert!(block.is_resultless_proposal());
        assert!(block.external_entrypoints_slice().is_empty());
        assert_eq!(block.network_entrypoint_count(), 1);
        assert_eq!(block.execution_context().unwrap().native_lane_decisions.as_deref(), Some(fixture.work.batch()));
        assert_eq!(candidate.scan_report().native_selected, 1);
        assert_eq!(candidate.scan_report().native_deferred, 0);
        assert!(candidate_block_has_proposal_work(block, &fixture.state, false).unwrap());
        block.validate_proposal_commitments().unwrap();
        let (block, bytes, encoded, _, _, _lease) = candidate.into_parts();
        assert_eq!(block.encode_wire().unwrap(), bytes);
        assert!(!encoded.into_parts().1.is_empty());
        let NativeLaneBatchSourcePreparationV1::Ready(source) = fixture.state.prepare_proposed_native_lane_batch_source(&block, &[]).unwrap()
        else { panic!("actual candidate rejoins original certified input"); };
        let context = native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
        drop(block);
        let recorded = source.record_execution(context).unwrap().unwrap();
        assert!(recorded.prepared_for_test().executions()[0].result.is_ok());
        recorded.prepared_for_test().overlay().verify_execution_output_seal(recorded.carrier()).unwrap();
        drop(recorded);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    }
}

state_test! { sync native_candidate_after_idle_uses_input_time_in_full_preparation
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments};
    for atomic in [false, true] {
        let fixture = native_candidate_fixture(&[NativeEconomicCase::TransferAfterParent(25, 10_000)], atomic);
        let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
        let CandidateAssemblyOutcome::Assembled(candidate) = assemble_native_candidate_for_test(
            &fixture, 16, 2 * 1024 * 1024, CandidateAttachments::default()).unwrap()
        else { panic!("one finite input after an idle parent must produce a carrier"); };
        let block = candidate.block();
        let input_time = fixture.work.batch().groups[0].payload.input.entrypoint.creation_time_ms().unwrap();
        assert_eq!(block.header().creation_time_ms, input_time + 1);
        assert!(block.header().creation_time() > fixture.parent.header().creation_time()
            + fixture.state.sumeragi_block_cadence());
        let NativeLaneBatchSourcePreparationV1::Ready(source) = fixture.state
            .prepare_proposed_native_lane_batch_source(block, &[]).unwrap()
        else { panic!("original authenticated input remains available"); };
        let context = native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
        // The actual preflight must derive the same time from the Native source,
        // independently of the validator's local wall clock.
        let (_, clock) = iroha_primitives::time::TimeSource::new_mock(Duration::from_millis(1));
        let prepared = source.prepare_candidate(context,
            &iroha_test_samples::SAMPLE_GENESIS_ACCOUNT_ID, &clock,
            fixture.state.sumeragi_block_cadence()).unwrap().unwrap();
        assert_eq!(prepared.block().canonical_resultless_proposal(), *block);
        assert_eq!(prepared.block().execution_outputs().len(), 1);
        assert!(prepared.block().execution_outputs()[0].result().is_ok());
        prepared.block().validate_execution_result_structure().unwrap();
        drop(prepared);
        drop(candidate);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    }
}

state_test! { sync native_candidate_fits_whole_priority_prefix_before_signing
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments, CandidateError};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::TransferAfterParent(25, 10_000), NativeEconomicCase::TransferAfterParent(10, 20_000)], false);
    assert_eq!(fixture.work.batch().groups.len(), 2);
    let full_time = fixture.work.batch().groups.iter()
        .map(|group| group.payload.input.entrypoint.creation_time_ms().unwrap() + 1)
        .max().unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let CandidateAssemblyOutcome::Assembled(one) = assemble_native_candidate_for_test(
        &fixture, 1, 2 * 1024 * 1024, CandidateAttachments::default()).unwrap()
    else { panic!("count-bound prefix"); };
    let one_bytes = one.block().encode_wire().unwrap().len();
    assert_eq!(one.scan_report().native_selected, 1);
    assert_eq!(one.scan_report().native_deferred, 1);
    let expected = one.block().execution_context().unwrap().native_lane_decisions.clone();
    assert_eq!(expected.as_ref().unwrap().groups, fixture.work.batch().groups[..1]);
    let first_time = expected.as_ref().unwrap().groups[0].payload.input.entrypoint.creation_time_ms().unwrap() + 1;
    assert_eq!(one.block().header().creation_time_ms, first_time);
    assert!(first_time < full_time, "admission order retains the earlier input first");
    drop(one);
    let CandidateAssemblyOutcome::Assembled(fitted) = assemble_native_candidate_for_test(
        &fixture, 16, one_bytes, CandidateAttachments::default()).unwrap()
    else { panic!("exact complete wire budget must fit the same one-group prefix"); };
    assert_eq!(fitted.block().encode_wire().unwrap().len(), one_bytes);
    assert_eq!(fitted.block().execution_context().unwrap().native_lane_decisions, expected);
    assert_eq!(fitted.block().header().creation_time_ms, first_time,
        "byte trimming recomputes time without the deferred later input");
    assert_eq!(fitted.scan_report().native_selected, 1);
    assert_eq!(fitted.scan_report().native_deferred, 1);
    drop(fitted);
    let error = assemble_native_candidate_for_test(&fixture, 16, one_bytes - 1, CandidateAttachments::default()).unwrap_err();
    assert!(matches!(error, CandidateError::ProposalFramingExceedsPayloadLimits { encoded_bytes, .. } if encoded_bytes == one_bytes), "{error}");
    let CandidateAssemblyOutcome::Assembled(all) = assemble_native_candidate_for_test(
        &fixture, 16, 2 * 1024 * 1024, CandidateAttachments::default()).unwrap()
    else { panic!("deferred groups must still be available"); };
    assert_eq!(all.scan_report().native_selected, 2);
    assert_eq!(all.block().header().creation_time_ms, full_time);
    assert_eq!(all.block().execution_context().unwrap().native_lane_decisions.as_deref(), Some(fixture.work.batch()));
    assert_eq!(fixture.work.batch().groups.len(), 2, "fitting never mutates original evidence");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
}

state_test! { sync native_candidate_stale_observation_waits_without_signing_or_custody_loss
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments, CandidateWorkDeferral};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let mut publication_notice = fixture.state.state_view_publication();
    let publication = publication_notice.begin();
    drop(publication);
    drop(publication_notice);
    assert!(matches!(assemble_prepared_native_candidate_for_test(&fixture, 16, 2 * 1024 * 1024,
        CandidateAttachments::default()).unwrap(), CandidateAssemblyOutcome::WorkDeferred {
            reason: CandidateWorkDeferral::NativeLaneSource, ..
        }));
    assert_eq!(fixture.work.batch().groups.len(), 1);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
}

state_test! { sync native_candidate_controls_fit_without_displacing_or_duplicating_economic_input
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::TransferAfterParent(25, 10_000)], false);
    let input = &fixture.work.batch().groups[0].payload.input;
    let control = norito::encode_canonical(input).unwrap();
    let attachments = CandidateAttachments { queue_plan_admissions: vec![control.clone()], ..CandidateAttachments::default() };
    let CandidateAssemblyOutcome::Assembled(complete) = assemble_native_candidate_for_test(
        &fixture, 16, 2 * 1024 * 1024, attachments.clone()).unwrap()
    else { panic!("input and independent control fit"); };
    assert_eq!(complete.block().network_entrypoint_count(), 1);
    assert_eq!(complete.block().header().creation_time_ms,
        input.entrypoint.creation_time_ms().unwrap() + 1);
    assert_eq!(complete.block().execution_context().unwrap().queue_plan_admissions(), &[control]);
    assert!(complete.block().external_entrypoints_slice().is_empty());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = fixture.state.prepare_proposed_native_lane_batch_source(complete.block(), &[]).unwrap()
    else { panic!("complete source with admission control"); };
    let context = native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
    let recorded = source.record_execution(context).unwrap().unwrap();
    assert!(recorded.prepared_for_test().executions()[0].result.is_ok());
    drop(recorded);
    drop(complete);
    let CandidateAssemblyOutcome::Assembled(native_only) = assemble_native_candidate_for_test(
        &fixture, 16, 2 * 1024 * 1024, CandidateAttachments::default()).unwrap()
    else { panic!("native-only budget"); };
    let budget = native_only.block().encode_wire().unwrap().len();
    drop(native_only);
    let CandidateAssemblyOutcome::Assembled(fitted) = assemble_native_candidate_for_test(
        &fixture, 16, budget, attachments).unwrap()
    else { panic!("defer the control while preserving the complete decided input"); };
    assert_eq!(fitted.scan_report().admission_deferred, 1);
    assert_eq!(fitted.scan_report().native_selected, 1);
    assert_eq!(fitted.scan_report().native_deferred, 0);
    assert!(fitted.block().execution_context().unwrap().queue_plan_admissions().is_empty());
    assert_eq!(fitted.block().execution_context().unwrap().native_lane_decisions.as_deref(), Some(fixture.work.batch()));
    drop(fitted);
    // The native input plus Decisions exceeds this budget, but its complete
    // admission control alone fits. Preserve that feasible independent work.
    let control = norito::encode_canonical(input).unwrap();
    let CandidateAssemblyOutcome::Assembled(admission_only) = assemble_native_candidate_for_test(
        &fixture, 16, budget - 1, CandidateAttachments {
            queue_plan_admissions: vec![control.clone()], ..CandidateAttachments::default()
        }).unwrap()
    else { panic!("an oversized decided group must not erase feasible admission work"); };
    assert_eq!(admission_only.scan_report().native_selected, 0);
    assert_eq!(admission_only.scan_report().native_deferred, 1);
    assert_eq!(admission_only.scan_report().admission_deferred, 0);
    let context = admission_only.block().execution_context().unwrap();
    assert!(context.native_lane_decisions.is_none());
    assert_eq!(context.queue_plan_admissions(), &[control]);
    assert!(admission_only.block().external_entrypoints_slice().is_empty());
    assert_eq!(admission_only.block().header().creation_time(),
        fixture.parent.header().creation_time() + fixture.state.sumeragi_block_cadence(),
        "an admission control carries no execution clock floor");
    assert!(admission_only.block().encode_wire().unwrap().len() < budget);
}

state_test! { sync native_candidate_refuses_unsupported_carrier_controls_before_signing
    use crate::sumeragi::v2_candidate::{CandidateAttachments, CandidateError};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    for attachments in [
        CandidateAttachments { da_commitments: Some(Default::default()), ..CandidateAttachments::default() },
        CandidateAttachments { da_pin_intents: Some(Default::default()), ..CandidateAttachments::default() },
        CandidateAttachments { sccp_commitment_root: Some([1; 32]), ..CandidateAttachments::default() },
    ] {
        assert!(matches!(assemble_native_candidate_for_test(&fixture, 16, 2 * 1024 * 1024,
            attachments).unwrap_err(), CandidateError::NativeLaneDecisionInvalid(reason)
            if reason.contains("additional carrier controls")));
        assert_eq!(fixture.work.batch().groups.len(), 1);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    }
}

state_test! { sync native_candidate_proof_rejects_foreign_state_and_network
    let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let foreign = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let context = native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
    assert!(fixture.work.is_current(&fixture.state, context.context()));
    assert!(!fixture.work.is_current(&foreign.state, context.context()));
    let mut wrong_network = context.context().clone();
    wrong_network.network_id = crate::sumeragi::synthetic_network_id("foreign-native-candidate");
    assert!(!fixture.work.is_current(&fixture.state, &wrong_network));
    let mut wrong_height = context.context().clone();
    wrong_height.height += 1;
    assert!(!fixture.work.is_current(&fixture.state, &wrong_height));
}

state_test! { sync native_candidate_handoff_rejects_retired_merge_before_signing
    use crate::sumeragi::v2_candidate::{CandidateAttachments, CandidateError};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let error = assemble_native_handoff_for_test(&fixture, 16, 2 * 1024 * 1024,
        CandidateAttachments {
            certified_merge_carrier_header: Some(fixture.parent.header()),
            ..CandidateAttachments::default()
        }).err().expect("retired producer attachment must be refused");
    assert!(matches!(error, CandidateError::NativeLaneDecisionInvalid(reason)
        if reason.contains("retired certified merge")));
    assert!(fixture.handoff.belongs_to(&fixture.state));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
}

state_test! { sync native_candidate_handoff_rejects_foreign_original_state
    use crate::sumeragi::v2_candidate::{CandidateAttachments, CandidateError};
    let mut fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let foreign = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    assert!(!foreign.handoff.belongs_to(&fixture.state));
    fixture.handoff = foreign.handoff;
    let error = assemble_native_handoff_for_test(&fixture, 16, 2 * 1024 * 1024,
        CandidateAttachments::default()).err().expect("foreign State custody must be refused");
    assert!(matches!(error, CandidateError::NativeLaneDecisionInvalid(reason)
        if reason.contains("another State owner")));
}

state_test! { sync native_candidate_partial_atomic_handoff_retains_waits_and_independent_work
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments},
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput},
    };
    let mut fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], true);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let outsider = KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::BlsNormal).unwrap();
    let mut driver = NativeLaneDriver::new(
        Arc::clone(&fixture.state), Arc::clone(&guard), outsider,
        native_driver_limits_for_test(),
    ).unwrap();
    let full = fixture.handoff.prepare_groups().unwrap();
    assert_eq!(full.groups.len(), 1);
    let decisions = full.groups[0].decisions();
    assert_eq!(decisions.len(), 2, "real atomic input owns two route Decisions");
    assert!(matches!(driver.admit(&observed, NativeLaneInput::Decision(decisions[0].clone())),
        NativeLaneAdmission::Accepted));
    fixture.handoff = driver.capture_decisions(&observed).unwrap().unwrap();
    let assembly = assemble_native_handoff_for_test(&fixture, 16, 2 * 1024 * 1024,
        CandidateAttachments::default()).unwrap();
    assert!(assembly.source.work.is_none());
    assert!(matches!(assembly.source.waits.as_slice(),
        [LaneDecisionGroupPreparationV1::MissingDecisions(missing)] if missing.len() == 1));
    assert!(matches!(assembly.outcome, Ok(CandidateAssemblyOutcome::NoProposalWork(_))),
        "a partial atomic input must not manufacture a carrier");

    let control = norito::encode_canonical(&fixture.work.batch().groups[0].payload.input).unwrap();
    let assembly = assemble_native_handoff_for_test(&fixture, 16, 2 * 1024 * 1024,
        CandidateAttachments { queue_plan_admissions: vec![control.clone()],
            ..CandidateAttachments::default() }).unwrap();
    assert!(assembly.source.work.is_none());
    assert!(matches!(assembly.source.waits.as_slice(),
        [LaneDecisionGroupPreparationV1::MissingDecisions(missing)] if missing.len() == 1));
    let CandidateAssemblyOutcome::Assembled(candidate) = assembly.outcome.unwrap()
    else { panic!("complete admission work remains serviceable while a route Decision is missing"); };
    assert!(candidate.block().external_entrypoints_slice().is_empty());
    let context = candidate.block().execution_context().unwrap();
    assert!(context.native_lane_decisions.is_none());
    assert!(context.merge_entry.is_none());
    assert_eq!(context.queue_plan_admissions(), &[control]);
    drop(candidate);
    assert_eq!(driver.process().occupancy().instances, 0, "evidence cannot settle a local Apply");
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
}
