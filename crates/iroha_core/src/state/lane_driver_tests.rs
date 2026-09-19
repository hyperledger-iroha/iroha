// Actual authenticated opening, shared reducer, bounded worker and outbox tests.
// Included with lane_process_tests so the canonical four-validator fixture and
// its independent physical custody controls remain the common setup owner.

fn native_driver_limits_for_test() -> crate::sumeragi::v2_lane_driver::NativeLaneDriverLimits {
    crate::sumeragi::v2_lane_driver::NativeLaneDriverLimits {
        process: native_process_limits_for_test(),
        ingress: nonzero!(128_usize),
        outbound: nonzero!(32_usize),
        maximum_message_bytes: nonzero!(1_048_576_usize),
    }
}

fn native_driver_control_for_test(
    fixture: &NativeProcessFixture,
    lane: &VerifiedLaneContext,
    signer: usize,
) -> iroha_data_model::block::lane_consensus::LaneMessageEnvelopeV1 {
    use iroha_data_model::block::lane_consensus::{
        LANE_MESSAGE_VERSION_V1, LaneMessageEnvelopeV1, LaneMessageV1, LaneRoundV1,
        LaneSignatureShareV1, LaneTimeoutBodyV1, LaneTimeoutVoteV1,
    };
    let body = LaneTimeoutBodyV1 {
        round: LaneRoundV1 {
            instance_id: Hash::from(lane.instance_id().0),
            lane_height: lane.frozen().next_lane_height,
            voting_view: 0,
        },
        highest_prepare: None,
    };
    let key = native_process_key(fixture, lane, signer);
    let signature = Signature::try_new(key.private_key(), &body.signature_preimage().unwrap())
        .unwrap()
        .payload()
        .to_vec();
    LaneMessageEnvelopeV1 {
        version: LANE_MESSAGE_VERSION_V1,
        message: LaneMessageV1::TimeoutVote(LaneTimeoutVoteV1 {
            body,
            share: LaneSignatureShareV1 {
                signer: u32::try_from(signer).unwrap(),
                signature,
            },
        }),
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_silent_initial_author_reaches_real_decision_without_global_view_input
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_core as core,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput},
        v2_lane_wire::{LaneAuthenticator, LaneWalRecordV1},
    };
    use iroha_data_model::block::lane_consensus::LaneMessageV1;
    use std::{collections::VecDeque, time::Instant};
    let start = Instant::now();
    let fixture = native_process_fixture(false, start);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let context = lane.reducer_context();
    let silent = context.leader(0);
    let survivors = context.roster().iter().enumerate()
        .filter_map(|(index, validator)| (validator.id() != silent).then_some(index))
        .collect::<Vec<_>>();
    assert_eq!(survivors.len(), 3);
    assert_ne!(context.leader(1), silent);
    let guards = (0..3).map(|_| ConsensusOutputGuard::isolated()).collect::<Vec<_>>();
    let mut drivers = survivors.iter().zip(&guards).map(|(&signer, guard)| {
        NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(guard),
            native_process_key(&fixture, lane, signer), native_driver_limits_for_test()).unwrap()
    }).collect::<Vec<_>>();
    let until = Instant::now() + Duration::from_secs(30);
    while drivers.iter().any(|driver| driver.process().instance(id).is_none()) {
        for driver in &mut drivers { driver.poll(&observed, start).unwrap(); }
        assert!(Instant::now() < until, "all real physical opens must complete");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(drivers.iter().all(|driver| driver.process().instance(id).unwrap().native_records().is_empty()));
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    let due = start + Duration::from_secs(1);
    let mut pending = VecDeque::new();
    let mut saw_successor_proposal = false;
    // These are genuine native signed messages from the actual driver outboxes.
    // There is no global-view argument, fake TC, payload injection or Ready event.
    while drivers.iter().any(|driver| {
        let instance = driver.process().instance(id).unwrap();
        instance.native_decision().unwrap().is_none()
            || !instance.held_effects().any(|effect| matches!(effect, core::Effect::Apply { .. }))
    }) {
        for driver in &mut drivers { driver.poll(&observed, due).unwrap(); }
        for (sender, driver) in drivers.iter().enumerate() {
            while let Some(packet) = driver.take_outbound().unwrap() {
                assert_eq!(packet.canonical_bytes, norito::encode_canonical(&packet.envelope).unwrap());
                if let LaneMessageV1::Proposal(proposal) = &packet.envelope.message
                    && proposal.body.round.instance_id == Hash::from(id.0)
                {
                    assert_eq!(proposal.body.round.voting_view, 1);
                    saw_successor_proposal = true;
                }
                for (recipient, &signer) in survivors.iter().enumerate() {
                    if recipient != sender && packet.destinations.contains(&lane.frozen().committee[signer]) {
                        pending.push_back((recipient, NativeLaneInput::Control(packet.envelope.clone())));
                    }
                }
            }
        }
        let count = pending.len();
        for _ in 0..count {
            let (recipient, input) = pending.pop_front().unwrap();
            match drivers[recipient].admit(&observed, input) {
                NativeLaneAdmission::Accepted => {},
                NativeLaneAdmission::Retry(input) => pending.push_back((recipient, input)),
                NativeLaneAdmission::Rejected { reason, .. } => panic!("real native input rejected: {reason}"),
            }
        }
        assert!(pending.len() <= 3 * 128, "bounded transport fixture");
        assert!(Instant::now() < until, "silent initial author must not own all future payload creation");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(saw_successor_proposal);
    for driver in &drivers {
        let instance = driver.process().instance(id).unwrap();
        assert!(matches!(&instance.native_records()[0].record,
            LaneWalRecordV1::TimeoutIntent { body, .. } if body.highest_prepare.is_none()));
        assert!(instance.native_records().iter().any(|row| matches!(row.record, LaneWalRecordV1::InstallTimeout(_))));
        let decision = instance.native_decision().unwrap().unwrap();
        assert_eq!(decision.commit_qc.shares.len(), 3);
        LaneAuthenticator::new(lane).decision_certificate(&decision).unwrap();
        assert!(instance.held_effects().any(|effect| matches!(effect, core::Effect::Apply { .. })),
            "the candidate consumer still owes exact economic Apply");
    }
    assert!(guards.iter().all(|guard| !guard.restart_required()));
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    for driver in drivers { driver.shutdown().join().unwrap(); }
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_retains_exact_ingress_and_instance_across_global_carrier_change
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput}};
    use std::time::Instant;
    let start = Instant::now();
    let fixture = native_process_fixture(false, start);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let guard = ConsensusOutputGuard::isolated();
    let mut limits = native_driver_limits_for_test();
    limits.ingress = nonzero!(1_usize);
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, 0), limits).unwrap();
    let control = native_driver_control_for_test(&fixture, lane, 1);
    assert!(matches!(driver.admit(&observed, NativeLaneInput::Control(control.clone())), NativeLaneAdmission::Accepted));
    let second = native_driver_control_for_test(&fixture, lane, 2);
    let NativeLaneAdmission::Retry(NativeLaneInput::Control(retained)) = driver.admit(&observed, NativeLaneInput::Control(second.clone())) else { panic!("full queue retains exact input"); };
    assert_eq!(retained, second);
    let until = Instant::now() + Duration::from_secs(15);
    while driver.process().instance(id).is_none() {
        driver.poll(&observed, start).unwrap();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    let original = std::ptr::from_ref(driver.process().instance(id).unwrap());
    let deadline = driver.next_deadline();
    native_process_advance(&fixture, false);
    assert!(!observed.is_current(&fixture.state));
    assert!(matches!(driver.admit(&observed, NativeLaneInput::Control(second.clone())), NativeLaneAdmission::Retry(_)));
    assert!(driver.capture_decisions(&observed).unwrap().is_none());
    driver.poll(&observed, start).unwrap();
    assert_eq!(driver.next_deadline(), deadline);
    let current = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(current.contexts().iter().any(|lane| lane.instance_id() == id));
    driver.poll(&current, start).unwrap();
    assert_eq!(std::ptr::from_ref(driver.process().instance(id).unwrap()), original,
        "global rollover does not reconstruct the native owner");
    let mut bad = control;
    bad.version += 1;
    let NativeLaneAdmission::Rejected { input: NativeLaneInput::Control(returned), reason } =
        driver.admit(&current, NativeLaneInput::Control(bad.clone()))
    else { panic!("invalid revision must return the original rejected control"); };
    assert_eq!(returned, bad);
    assert_eq!(reason, "unsupported native lane envelope revision");
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_transfers_exact_diagnostic_and_closed_custody
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_core as core,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput},
        v2_lane_payload::encode_lane_input,
    };
    use iroha_data_model::block::lane_consensus::{
        LaneMessageV1, LanePhaseV1, LaneQcV1, LaneSignatureShareV1, LaneVoteStatementV1,
    };
    use std::time::Instant;

    let now = Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let local = lane.reducer_context().roster().iter()
        .position(|validator| validator.id() != lane.reducer_context().leader(0)).unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let mut limits = native_driver_limits_for_test();
    limits.process.instances = nonzero!(1_usize);
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, local), limits).unwrap();
    let until = Instant::now() + Duration::from_secs(15);
    while driver.process().instance(id).is_none() {
        driver.poll(&observed, now).unwrap();
        assert!(Instant::now() < until, "original native opening must complete");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(driver.take_closed(id).is_none(), "an active signer cannot transfer as closed");
    assert!(driver.take_diagnostic(id).is_none());

    let FirstLaneAdmittedInputReadV1::Ready(source) = fixture.state.first_lane_admitted_input(&observed, lane).unwrap()
    else { panic!("authenticated first carrier"); };
    let LaneInputBodyPreparationV1::Ready(body) = fixture.state.prepare_lane_input_body(&observed, lane, &source).unwrap()
    else { panic!("canonical all-route body"); };
    let manifest = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    let original = native_driver_control_for_test(&fixture, lane, 0);
    let mut conflicting = original.clone();
    let LaneMessageV1::TimeoutVote(vote) = &mut conflicting.message else { unreachable!() };
    let statement = LaneVoteStatementV1 {
        round: vote.body.round,
        phase: LanePhaseV1::Prepare,
        value: manifest.value,
    };
    let preimage = statement.signature_preimage().unwrap();
    let shares = (0..3).map(|signer| LaneSignatureShareV1 {
        signer: signer as u32,
        signature: Signature::try_new(native_process_key(&fixture, lane, signer).private_key(), &preimage)
            .unwrap().payload().to_vec(),
    }).collect();
    vote.body.highest_prepare = Some(LaneQcV1 { statement, shares });
    vote.share.signature = Signature::try_new(native_process_key(&fixture, lane, 0).private_key(),
        &vote.body.signature_preimage().unwrap()).unwrap().payload().to_vec();
    for envelope in [original, conflicting] {
        assert!(matches!(driver.admit(&observed, NativeLaneInput::Control(envelope)), NativeLaneAdmission::Accepted));
        driver.poll(&observed, now).unwrap();
    }
    let held = driver.process().instance(id).unwrap().held_effects().cloned().collect::<Vec<_>>();
    assert!(held.iter().any(|effect| matches!(effect, core::Effect::ReportEquivocation { .. })));
    let original_obligations = held.into_iter()
        .filter(|effect| !matches!(effect, core::Effect::ReportEquivocation { .. }))
        .collect::<Vec<_>>();
    assert!(matches!(driver.take_diagnostic(id), Some(core::Effect::ReportEquivocation {
        evidence: core::EquivocationEvidence::Timeout { .. }
    })));
    assert!(driver.take_diagnostic(id).is_none(), "the exact diagnostic transfers once");
    assert_eq!(driver.process().instance(id).unwrap().held_effects().cloned().collect::<Vec<_>>(), original_obligations);
    let original_records = driver.process().instance(id).unwrap().native_records().to_vec();

    native_process_advance(&fixture, true);
    let closed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(closed.contexts().is_empty());
    while driver.process().occupancy().closed == 0 {
        driver.poll(&closed, now).unwrap();
        assert!(Instant::now() < until, "physical closure must complete");
        std::thread::sleep(Duration::from_millis(1));
    }
    let retired = driver.take_closed(id).expect("original drained owner remains available");
    assert!(driver.take_closed(id).is_none());
    assert_eq!(driver.process().occupancy().instances, 0);
    assert_eq!(retired.instance().native_records(), original_records);
    assert_eq!(retired.instance().held_effects().cloned().collect::<Vec<_>>(), original_obligations);
    assert!(!guard.restart_required(), "explicit transfer preserves protocol custody");
    // Observe custody loss before shutdown, which independently closes output.
    drop(retired);
    assert!(guard.restart_required(), "discarding closed custody never acknowledges its effects");
    driver.shutdown().join().unwrap();
    assert!(guard.restart_required(), "shutdown cannot reopen abandoned protocol custody");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_nonmember_decision_handoff_requires_every_exact_route_and_original_body
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput}};
    use std::time::Instant;
    let fixture = native_process_fixture(false, Instant::now());
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let first = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, first).unwrap() else { panic!("canonical first carrier"); };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, first, &source).unwrap() else { panic!("complete route body"); };
    let guard = ConsensusOutputGuard::isolated();
    let outsider = KeyPair::try_from_seed(vec![0xE9; 32], Algorithm::BlsNormal).unwrap();
    assert!(observed.contexts().iter().all(|lane| lane.frozen().committee.iter().all(|peer| peer.public_key() != outsider.public_key())));
    let mut driver = NativeLaneDriver::new(Arc::clone(state), Arc::clone(&guard), outsider, native_driver_limits_for_test()).unwrap();
    let before = crate::snapshot::canonical_state_snapshot_hash(state).unwrap();
    for (index, lane) in observed.contexts().iter().enumerate() {
        let decision = sign_native_group_decision_for_test(lane, &fixture.keys, &body, 0, 0);
        let mut damaged = decision.clone();
        damaged.commit_qc.shares[0].signature[0] ^= 1;
        assert!(matches!(driver.admit(&observed, NativeLaneInput::Decision(damaged)), NativeLaneAdmission::Rejected { .. }));
        assert!(matches!(driver.admit(&observed, NativeLaneInput::Decision(decision.clone())), NativeLaneAdmission::Accepted));
        assert!(matches!(driver.admit(&observed, NativeLaneInput::Decision(decision)), NativeLaneAdmission::Accepted));
        let handoff = driver.capture_decisions(&observed).unwrap().unwrap();
        let prepared = std::thread::spawn(move || handoff.prepare_groups()).join().unwrap().unwrap();
        if index + 1 < observed.contexts().len() {
            assert!(prepared.groups.is_empty());
            assert!(matches!(prepared.waits.as_slice(), [LaneDecisionGroupPreparationV1::MissingDecisions(_)]));
        } else {
            assert!(prepared.waits.is_empty());
            assert_eq!(prepared.groups.len(), 1);
            assert_eq!(prepared.groups[0].body().canonical_bytes(), body.canonical_bytes());
            assert_eq!(prepared.groups[0].decisions().len(), 3);
        }
    }
    assert_eq!(driver.process().occupancy().instances, 0, "a global consumer outside the committee receives no signing instance");
    assert!(!guard.restart_required());
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(state).unwrap(), before);
    driver.shutdown().join().unwrap();
}

state_test! { sync native_driver_admits_complete_control_envelope_before_opening_any_signer
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_lane_driver::NativeLaneDriver,v2_lane_frame_bounds};
    let start = std::time::Instant::now();
    let fixture = native_process_fixture(false,start);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let required = v2_lane_frame_bounds::maximum_message_bytes(lane.frozen().committee.len()).unwrap();
    let mut limits = native_driver_limits_for_test();
    limits.maximum_message_bytes = NonZeroUsize::new(required-1).unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state),Arc::clone(&guard),
        native_process_key(&fixture,lane,0),limits).unwrap();
    let error = driver.poll(&observed,start).unwrap_err();
    assert!(error.contains("before opening"),"{error}");
    assert_eq!(driver.process().occupancy().instances,0);
    assert_eq!(driver.process().occupancy().queued,0);
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
}
