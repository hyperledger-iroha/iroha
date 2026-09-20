// Authenticated lane Decisions through the existing global candidate assembler.
// Economic execution uses the original source consumer; no test acknowledges
// reducer Apply or activates the still-incomplete production runner cutover.

struct NativeCandidateFixture {
    state: Arc<State>,
    parent: SignedBlock,
    work: crate::sumeragi::v2_lane_driver::NativeLaneCandidateBatch,
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
    let prepared = std::thread::spawn(move || handoff.prepare_candidate())
        .join()
        .unwrap()
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
    }
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
        assert!(candidate_block_has_proposal_work(block, &fixture.state, false));
        block.validate_proposal_commitments().unwrap();
        let (block, bytes, encoded, _, _, _lease) = candidate.into_parts();
        assert_eq!(block.encode_wire().unwrap(), bytes);
        assert!(!encoded.into_parts().1.is_empty());
        let NativeLaneBatchSourcePreparationV1::Ready(source) = fixture.state.prepare_proposed_native_lane_batch_source(&block, &[]).unwrap()
        else { panic!("actual candidate rejoins original certified input"); };
        let context = native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
        let recorded = source.record_execution(block, context).unwrap().unwrap();
        assert!(recorded.prepared_for_test().executions()[0].result.is_ok());
        recorded.prepared_for_test().overlay().verify_execution_output_seal(recorded.carrier()).unwrap();
        drop(recorded);
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    }
}

state_test! { sync native_candidate_fits_whole_priority_prefix_before_signing
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments, CandidateError};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25), NativeEconomicCase::Transfer(10)], false);
    assert_eq!(fixture.work.batch().groups.len(), 2);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let CandidateAssemblyOutcome::Assembled(one) = assemble_native_candidate_for_test(
        &fixture, 1, 2 * 1024 * 1024, CandidateAttachments::default()).unwrap()
    else { panic!("count-bound prefix"); };
    let one_bytes = one.block().encode_wire().unwrap().len();
    assert_eq!(one.scan_report().native_selected, 1);
    assert_eq!(one.scan_report().native_deferred, 1);
    let expected = one.block().execution_context().unwrap().native_lane_decisions.clone();
    assert_eq!(expected.as_ref().unwrap().groups, fixture.work.batch().groups[..1]);
    drop(one);
    let CandidateAssemblyOutcome::Assembled(fitted) = assemble_native_candidate_for_test(
        &fixture, 16, one_bytes, CandidateAttachments::default()).unwrap()
    else { panic!("exact complete wire budget must fit the same one-group prefix"); };
    assert_eq!(fitted.block().encode_wire().unwrap().len(), one_bytes);
    assert_eq!(fitted.block().execution_context().unwrap().native_lane_decisions, expected);
    assert_eq!(fitted.scan_report().native_selected, 1);
    assert_eq!(fitted.scan_report().native_deferred, 1);
    drop(fitted);
    let error = assemble_native_candidate_for_test(&fixture, 16, one_bytes - 1, CandidateAttachments::default()).unwrap_err();
    assert!(matches!(error, CandidateError::ProposalFramingExceedsPayloadLimits { encoded_bytes, .. } if encoded_bytes == one_bytes), "{error}");
    let CandidateAssemblyOutcome::Assembled(all) = assemble_native_candidate_for_test(
        &fixture, 16, 2 * 1024 * 1024, CandidateAttachments::default()).unwrap()
    else { panic!("deferred groups must still be available"); };
    assert_eq!(all.scan_report().native_selected, 2);
    assert_eq!(all.block().execution_context().unwrap().native_lane_decisions.as_deref(), Some(fixture.work.batch()));
    assert_eq!(fixture.work.batch().groups.len(), 2, "fitting never mutates original evidence");
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
}

state_test! { sync native_candidate_stale_observation_waits_without_signing_or_custody_loss
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments, CandidateWorkDeferral};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let publication = fixture.state.begin_state_view_write();
    drop(publication);
    assert!(matches!(assemble_native_candidate_for_test(&fixture, 16, 2 * 1024 * 1024,
        CandidateAttachments::default()).unwrap(), CandidateAssemblyOutcome::WorkDeferred {
            reason: CandidateWorkDeferral::NativeLaneSource, ..
        }));
    assert_eq!(fixture.work.batch().groups.len(), 1);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
}

state_test! { sync native_candidate_controls_fit_without_displacing_or_duplicating_economic_input
    use crate::sumeragi::v2_candidate::{CandidateAssemblyOutcome, CandidateAttachments};
    let fixture = native_candidate_fixture(&[NativeEconomicCase::Transfer(25)], false);
    let input = &fixture.work.batch().groups[0].payload.input;
    let control = norito::encode_canonical(input).unwrap();
    let attachments = CandidateAttachments { queue_plan_admissions: vec![control.clone()], ..CandidateAttachments::default() };
    let CandidateAssemblyOutcome::Assembled(complete) = assemble_native_candidate_for_test(
        &fixture, 16, 2 * 1024 * 1024, attachments.clone()).unwrap()
    else { panic!("input and independent control fit"); };
    assert_eq!(complete.block().network_entrypoint_count(), 1);
    assert_eq!(complete.block().execution_context().unwrap().queue_plan_admissions(), &[control]);
    assert!(complete.block().external_entrypoints_slice().is_empty());
    let NativeLaneBatchSourcePreparationV1::Ready(source) = fixture.state.prepare_proposed_native_lane_batch_source(complete.block(), &[]).unwrap()
    else { panic!("complete source with admission control"); };
    let context = native_control_verified_context(&fixture.state, fixture.parent.header().height().get());
    let recorded = source.record_execution(complete.block().clone(), context).unwrap().unwrap();
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
