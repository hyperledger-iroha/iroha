// Actual authenticated opening, shared reducer, bounded worker and outbox tests.
// Included with lane_process_tests so the canonical four-validator fixture and
// its independent physical custody controls remain the common setup owner.

fn native_driver_limits_for_test() -> crate::sumeragi::v2_lane_driver::NativeLaneDriverLimits {
    crate::sumeragi::v2_lane_driver::NativeLaneDriverLimits {
        voting_enabled: true,
        process: native_process_limits_for_test(),
        ingress: nonzero!(128_usize),
        outbound: nonzero!(32_usize),
        maximum_message_bytes: nonzero!(1_048_576_usize),
    }
}

include!("native_lane_candidate_tests.rs");

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
    // The same bounded cleanup phase used by NativeRunnerProcess must never
    // consume a live Decision or its original unapplied economic effect.
    for driver in &mut drivers {
        let decision = driver.process().instance(id).unwrap().native_decision().unwrap();
        for _ in 0..12 {
            driver.prepare_one_retirement().unwrap();
            driver.poll(&observed, due).unwrap();
        }
        assert_eq!(driver.process().instance(id).unwrap().native_decision().unwrap(), decision);
        assert!(driver.process().instance(id).unwrap().held_effects()
            .any(|effect| matches!(effect, core::Effect::Apply { .. })));
    }
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
state_test! { sync native_driver_full_effect_reservation_does_not_spin_on_overdue_clock
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_lane_driver::NativeLaneDriver,
    };
    use std::time::Instant;
    let start = Instant::now();
    let fixture = native_process_fixture(false, start);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let signer = lane.reducer_context().roster().iter()
        .position(|validator| validator.id() != lane.reducer_context().leader(0)).unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, signer), native_driver_limits_for_test()).unwrap();
    let until = Instant::now() + Duration::from_secs(15);
    while driver.process().instance(id).is_none() {
        driver.poll(&observed, start).unwrap();
        assert!(Instant::now() < until, "physical lane opening must complete");
        std::thread::sleep(Duration::from_millis(1));
    }
    let due = driver.next_deadline().expect("fresh active lane has a clock");
    let tag = driver.process().instance(id).unwrap().tag();
    driver.restrict_effect_capacity_to_retained_for_test(id);
    assert_eq!(driver.next_deadline(), None,
        "an unserviceable overdue clock must use the runner's bounded idle wake");
    driver.poll(&observed, due + Duration::from_secs(1)).unwrap();
    assert_eq!(driver.process().instance(id).unwrap().tag(), tag);
    assert_eq!(driver.next_deadline(), None);
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
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
        let (prepared, candidate) = std::thread::spawn(move || (
            handoff.prepare_groups().unwrap(), handoff.prepare_candidate().unwrap(),
        )).join().unwrap();
        if index + 1 < observed.contexts().len() {
            assert!(prepared.groups.is_empty());
            assert!(matches!(prepared.waits.as_slice(), [LaneDecisionGroupPreparationV1::MissingDecisions(_)]));
            assert!(candidate.work.is_none());
            assert!(matches!(candidate.waits.as_slice(), [LaneDecisionGroupPreparationV1::MissingDecisions(_)]));
        } else {
            assert!(prepared.waits.is_empty());
            assert_eq!(prepared.groups.len(), 1);
            assert_eq!(prepared.groups[0].body().canonical_bytes(), body.canonical_bytes());
            assert_eq!(prepared.groups[0].decisions().len(), 3);
            assert!(candidate.waits.is_empty());
            assert_eq!(candidate.work.unwrap().batch().groups, vec![prepared.groups[0].to_wire()]);
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

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_retains_retired_body_capacity_and_exact_closed_handoff
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_core as core,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput},
        v2_lane_wire::LaneWalRecordV1,
    };
    use std::{sync::mpsc, time::Instant};

    let now = Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(observed.contexts().len(), 3);
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let other = observed.contexts()[1].instance_id();
    let leader = lane.reducer_context().roster().iter()
        .position(|validator| validator.id() == lane.reducer_context().leader(0)).unwrap();
    let FirstLaneAdmittedInputReadV1::Ready(source) = fixture.state.first_lane_admitted_input(&observed, lane).unwrap()
        else { panic!("original complete first-carrier source"); };
    let LaneInputBodyPreparationV1::Ready(body) = fixture.state.prepare_lane_input_body(&observed, lane, &source).unwrap()
        else { panic!("actual all-route body"); };
    let guard = ConsensusOutputGuard::isolated();
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, leader), native_driver_limits_for_test()).unwrap();
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    driver.hold_next_body_completion_for_test(id, move || {
        entered.send(()).unwrap();
        // Dropping the sender on a failed assertion releases the real worker.
        let _ = release_rx.recv();
    });
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        driver.poll(&observed, now).unwrap();
        if entered_rx.try_recv().is_ok() { break; }
        assert!(Instant::now() < until, "genuine body completion must enter the held boundary");
        std::thread::sleep(Duration::from_millis(1));
    }
    let original = std::ptr::from_ref(driver.process().instance(id).unwrap());
    let original_tag = driver.process().instance(id).unwrap().tag();
    // Three actual BLS timeout shares advance only this same reducer. The
    // physical body operation retains its original view-zero purpose/result.
    for signer in 0..3 {
        assert!(matches!(driver.admit(&observed,
            NativeLaneInput::Control(native_driver_control_for_test(&fixture, lane, signer))),
            NativeLaneAdmission::Accepted));
    }
    while driver.process().instance(id).unwrap().tag().view() != 1
        || driver.process().instance(id).unwrap().timeout_deadline() != Some(now + Duration::from_secs(2))
    {
        driver.poll(&observed, now).unwrap();
        assert!(Instant::now() < until, "real WAL completion must install and enter certified view one");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert!(driver.process().instance(id).unwrap().native_records().iter()
        .any(|row| matches!(row.record, LaneWalRecordV1::InstallTimeout(_))));
    release.send(()).unwrap();
    while driver.process().instance(id).unwrap().retirement_count() == 0 {
        driver.poll(&observed, now).unwrap();
        assert!(Instant::now() < until, "returned body must remain owned after its exact tag becomes obsolete");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(std::ptr::from_ref(driver.process().instance(id).unwrap()), original);
    assert_ne!(driver.process().instance(id).unwrap().tag(), original_tag);
    assert!(!driver.process().instance(id).unwrap().held_effects()
        .any(|effect| matches!(effect, core::Effect::Apply { .. })));
    assert!(driver.take_retirement(other).is_none(), "another instance cannot consume this body");
    driver.restrict_effect_capacity_to_retained_for_test(id);
    let held = driver.process().instance(id).unwrap().held_effects().cloned().collect::<Vec<_>>();
    let retained = driver.process().instance(id).unwrap().retirement_count();
    let records = driver.process().instance(id).unwrap().native_records().to_vec();
    let due = now + Duration::from_secs(3);
    while driver.process().instance(other).is_none_or(|owner| owner.native_records().is_empty()) {
        driver.poll(&observed, due).unwrap();
        assert!(Instant::now() < until, "one full retirement owner cannot block another lane's WAL");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(driver.process().instance(id).unwrap().held_effects().cloned().collect::<Vec<_>>(), held);
    assert_eq!(driver.process().instance(id).unwrap().retirement_count(), retained);
    assert_eq!(driver.process().instance(id).unwrap().tag().view(), 1);
    assert_eq!(driver.process().instance(id).unwrap().native_records(), records);
    assert!(!guard.restart_required());
    while observed.contexts().iter().any(|lane| driver.process().instance(lane.instance_id()).is_none()) {
        driver.poll(&observed, due).unwrap();
        assert!(Instant::now() < until, "all three original opens must complete before closure");
        std::thread::sleep(Duration::from_millis(1));
    }

    // The authentic State closure carries the same escrow through the original
    // physical drain and LaneClosedInstance transfer, without Ready or Apply.
    native_process_advance(&fixture, true);
    let closed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(closed.contexts().is_empty());
    while driver.process().occupancy().closed != 3 {
        driver.poll(&closed, due).unwrap();
        assert!(Instant::now() < until, "actual physical owners must drain");
        std::thread::sleep(Duration::from_millis(1));
    }
    let mut owner = driver.take_closed(id).expect("same original owner transfers");
    assert_eq!(std::ptr::from_ref(owner.instance()), original);
    assert!(driver.take_closed(id).is_none());
    assert!(driver.take_retirement(id).is_none(), "Driver cannot consume from the transferred owner");
    let retirement = owner.take_retirement().expect("actual returned body is still owned");
    assert_eq!(retirement.instance(), id);
    assert!(retirement.belongs_to(&fixture.state));
    assert_eq!(retirement.body_bytes(), Some(body.canonical_bytes()));
    assert!(!retirement.requires_recovery(), "the real TC already obsoleted this body tag before closure");
    assert!(retirement.effect().is_none() && retirement.proposal().is_none());
    assert!(owner.take_retirement().is_none(), "the exact body transfers once");
    assert!(!owner.instance().held_effects().any(|effect| matches!(effect, core::Effect::Apply { .. })));
    assert!(!guard.restart_required());
    drop(owner);
    assert!(guard.restart_required(), "closed-owner discard never acknowledges unfinished obligations");
    driver.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_taken_unfinished_closed_body_keeps_original_output_fence
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_core as core,
        v2_lane_driver::NativeLaneDriver,
    };
    use std::{sync::mpsc, time::Instant};
    let now = Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let leader = lane.reducer_context().roster().iter()
        .position(|validator| validator.id() == lane.reducer_context().leader(0)).unwrap();
    let FirstLaneAdmittedInputReadV1::Ready(source) = fixture.state.first_lane_admitted_input(&observed, lane).unwrap()
        else { panic!("original complete carrier"); };
    let LaneInputBodyPreparationV1::Ready(body) = fixture.state.prepare_lane_input_body(&observed, lane, &source).unwrap()
        else { panic!("actual all-route body"); };
    let guard = ConsensusOutputGuard::isolated();
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, leader), native_driver_limits_for_test()).unwrap();
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    driver.hold_next_body_completion_for_test(id, move || {
        entered.send(()).unwrap();
        let _ = release_rx.recv();
    });
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        driver.poll(&observed, now).unwrap();
        if entered_rx.try_recv().is_ok() { break; }
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    let original_tag = driver.process().instance(id).unwrap().tag();
    let original_records = driver.process().instance(id).unwrap().native_records().to_vec();
    let original_effects = driver.process().instance(id).unwrap().held_effects().cloned().collect::<Vec<_>>();
    assert_eq!(driver.process().instance(id).unwrap().retirement_count(), 0);
    // The real completed job still owns its single original descriptor. There is
    // no spare slot for the productive reserve_ingress path at completion return.
    driver.restrict_effect_capacity_to_retained_for_test(id);
    native_process_advance(&fixture, true);
    let closed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(closed.contexts().is_empty());
    release.send(()).unwrap();
    while driver.process().instance(id).is_none_or(|owner| owner.retirement_count() == 0) {
        driver.poll(&closed, now).unwrap();
        assert!(Instant::now() < until, "original return must enter retained closure custody");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(driver.process().instance(id).unwrap().retirement_count(), 1,
        "the original completed descriptor moves once into retirement without headroom");
    assert_eq!(driver.process().instance(id).unwrap().native_records(), original_records);
    assert_eq!(driver.process().instance(id).unwrap().held_effects().cloned().collect::<Vec<_>>(), original_effects);
    // A closure-body result is not an obsolete control/result. Even repeated
    // production cleanup turns must leave its original output fence armed.
    for _ in 0..12 {
        driver.prepare_one_retirement().unwrap();
        driver.poll(&closed, now).unwrap();
    }
    // Closing moves the actual owner to a physical drain worker. Polling a
    // fixed number of times cannot promise that its exact result has returned.
    while driver.process().instance(id).is_none() {
        driver.poll(&closed, now).unwrap();
        assert!(Instant::now() < until, "the original closed owner must return from its physical drain");
        std::thread::sleep(Duration::from_millis(1));
    }
    assert_eq!(driver.process().instance(id).unwrap().retirement_count(), 1);
    let retirement = driver.take_retirement(id).expect("Driver transfers the original completion once");
    assert!(driver.take_retirement(id).is_none());
    assert_eq!(retirement.instance(), id);
    assert!(retirement.belongs_to(&fixture.state));
    assert!(retirement.requires_recovery());
    assert_eq!(retirement.body_bytes(), Some(body.canonical_bytes()));
    assert_eq!(driver.process().instance(id).unwrap().tag(), original_tag);
    assert!(!driver.process().instance(id).unwrap().held_effects()
        .any(|effect| matches!(effect, core::Effect::Apply { .. })));
    assert!(!guard.restart_required(), "taking custody is not loss");
    drop(retirement);
    assert!(guard.restart_required(), "dropping the taken unfinished body still fences original output");
    driver.shutdown().join().unwrap();
}

fn native_driver_recovery_exchange_for_test(
    fixture: &NativeProcessFixture,
) -> (
    crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyRequest,
    crate::sumeragi::v2_transport::AuthenticatedCertifiedBodyResponse,
    crate::sumeragi::v2_transport::OutstandingCertifiedBodyRequests,
) {
    use crate::sumeragi::{v2_chunks, v2_transport};
    use iroha_data_model::block::consensus_v2 as wire;
    let finality = fixture
        .state
        .kura
        .v2_finality_artifact(fixture.block.header().height().get())
        .unwrap()
        .unwrap();
    let keys = native_preparation_global_keys(&finality.height_context);
    let key = &keys[0];
    let peer = PeerId::new(key.public_key().clone());
    let mut request = wire::CertifiedBodyRequest {
        round: finality.commit_qc.proposal_round,
        subject: finality.subject,
        certificate: finality.commit_qc.clone(),
        requester: peer.clone(),
        signature: Vec::new(),
    };
    request.signature = Signature::new(key.private_key(), &request.signature_preimage())
        .payload()
        .to_vec();
    let request = v2_transport::authenticate_certified_body_request_with_validator_pops(
        &finality.height_context,
        &finality.validator_set_pops,
        request,
        &peer,
    )
    .unwrap();
    let body = fixture
        .block
        .canonical_resultless_proposal()
        .encode_wire()
        .unwrap();
    let (manifest, _) = v2_chunks::encode_payload(
        &finality.height_context,
        request.request().round,
        finality.subject,
        &body,
    )
    .unwrap()
    .into_parts();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: request.request_hash(),
        manifest,
        body,
        responder: peer.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(key.private_key(), &response.signature_preimage())
        .payload()
        .to_vec();
    let mut outstanding = v2_transport::OutstandingCertifiedBodyRequests::new(1).unwrap();
    outstanding.register(request.clone()).unwrap();
    let response = outstanding
        .authenticate_response(&finality.height_context, response, &peer)
        .unwrap();
    (request, response, outstanding)
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_source_recovery_rejoins_original_owner_after_foreign_refusal
    use crate::sumeragi::{output_guard::ConsensusOutputGuard,
        v2_core::Effect, v2_lane_driver::NativeLaneDriver};
    use std::time::Instant;
    let now = Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let leader = lane.reducer_context().roster().iter()
        .position(|member| member.id() == lane.reducer_context().leader(0)).unwrap();
    let (request, response, outstanding) = native_driver_recovery_exchange_for_test(&fixture);
    let foreign = native_process_fixture(false, now);
    let (foreign_request, foreign_response, foreign_outstanding) = native_driver_recovery_exchange_for_test(&foreign);
    let foreign_observed = foreign.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let foreign_id = foreign_observed.contexts()[0].instance_id();
    assert_ne!(foreign_id, id);
    fixture.state.kura.evict_first_admission_body_for_testing(
        NonZeroUsize::new(fixture.block.header().height().get() as usize).unwrap(),
        fixture.block.hash()).unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, leader), native_driver_limits_for_test()).unwrap();
    let until = Instant::now() + Duration::from_secs(15);
    loop {
        driver.poll(&observed, now).unwrap();
        if driver.process().instance(id).is_some_and(|owner| owner.source_recovery_requirement().is_some()) {break;}
        assert!(Instant::now() < until, "real source-recovery job must complete");
        std::thread::sleep(Duration::from_millis(1));
    }
    let original = driver.process().instance(id).unwrap();
    let original_address = std::ptr::from_ref(original);
    let required = original.source_recovery_requirement().unwrap().clone();
    let records = original.native_records().len();
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    assert_eq!(required.carrier_hash(), fixture.block.hash());
    for (target, asked, received) in [
        (foreign_id, &request, &response),
        (id, &foreign_request, &foreign_response),
        (id, &request, &foreign_response),
    ] {
        assert!(driver.complete_source_recovery(target, asked, received).is_err());
        let owner = driver.process().instance(id).unwrap();
        assert_eq!(std::ptr::from_ref(owner), original_address);
        assert_eq!(owner.source_recovery_requirement().unwrap().carrier_hash(), required.carrier_hash());
        assert_eq!(owner.source_recovery_requirement().unwrap().priority(), required.priority());
        assert_eq!(owner.native_records().len(), records);
        assert!(!guard.restart_required(), "wrong source is retryable, not lost custody");
    }
    driver.complete_source_recovery(id, &request, &response).unwrap();
    let owner = driver.process().instance(id).unwrap();
    assert_eq!(std::ptr::from_ref(owner), original_address);
    assert!(owner.source_recovery_requirement().is_none());
    assert_eq!(owner.native_records().len(), records);
    assert!(owner.durable_decision_certificate().is_none());
    assert!(!owner.held_effects().any(|effect| matches!(effect,
        Effect::StoreBody {..} | Effect::ValidateBody {..} | Effect::Apply {..})),
        "historical resultless recovery cannot manufacture native Ready or Apply");
    assert!(driver.complete_source_recovery(id, &request, &response).is_err(),
        "completed recovery cannot create another source owner");
    assert_eq!(outstanding.len(), 1, "the global transport still owns its exact request");
    assert_eq!(foreign_outstanding.len(), 1);
    assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before);
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_observer_role_cannot_open_or_admit_voting_control
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput},
    };
    let now = std::time::Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let guard = ConsensusOutputGuard::isolated();
    let mut limits = native_driver_limits_for_test();
    limits.voting_enabled = false;
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, 0), limits).unwrap();
    driver.poll(&observed, now).unwrap();
    assert_eq!(driver.process().occupancy().instances, 0);
    let input = NativeLaneInput::Control(native_driver_control_for_test(&fixture, lane, 1));
    assert!(matches!(driver.admit(&observed, input), NativeLaneAdmission::Rejected { .. }));
    assert!(driver.take_outbound().unwrap().is_none());
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_owned_capacity_retry_retains_original_payload_and_fair_evidence
    use crate::sumeragi::{
        message::BlockMessage,
        output_guard::ConsensusOutputGuard,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput,
            NativeLaneOwnedAdmission, native_driver_owned_ingress_for_test},
    };
    use iroha_data_model::block::lane_consensus::LaneMessageV1;
    let now = std::time::Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let guard = ConsensusOutputGuard::isolated();
    let mut limits = native_driver_limits_for_test();
    limits.ingress = nonzero!(1_usize);
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, 0), limits).unwrap();
    assert!(matches!(driver.admit(&observed,
        NativeLaneInput::Control(native_driver_control_for_test(&fixture, lane, 1))),
        NativeLaneAdmission::Accepted));
    let original = native_driver_owned_ingress_for_test(
        BlockMessage::NativeLane(native_driver_control_for_test(&fixture, lane, 2)),
        lane.frozen().committee[2].clone());
    let signature = match original.message() {
        BlockMessage::NativeLane(envelope) => match &envelope.message {
            LaneMessageV1::TimeoutVote(vote) => vote.share.signature.as_ptr(),
            _ => panic!("fixture timeout vote"),
        },
        _ => panic!("fixture Native envelope"),
    };
    let evidence = original.ingress_ownership().unwrap();
    let projection = evidence.process_local_projection_hash();
    let ordinal = evidence.physical_admission_ordinal();
    let mut retained = original;
    for _ in 0..3 {
        retained = match driver.admit_owned(retained).unwrap() {
            NativeLaneOwnedAdmission::Retry(original) => original,
            _ => panic!("full process must return the original physical occurrence"),
        };
        let evidence = retained.ingress_ownership().unwrap();
        assert!(evidence.validate_exact());
        assert_eq!(evidence.process_local_projection_hash(), projection);
        assert_eq!(evidence.physical_admission_ordinal(), ordinal);
        assert_eq!(evidence.runtime_lifecycle_ordinal(), None);
        match retained.message() {
            BlockMessage::NativeLane(envelope) => match &envelope.message {
                LaneMessageV1::TimeoutVote(vote) => assert_eq!(vote.share.signature.as_ptr(), signature),
                _ => panic!("original timeout vote"),
            },
            _ => panic!("original Native envelope"),
        }
    }
    assert!(!guard.restart_required());
    drop(retained);
    driver.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_owned_rejection_returns_original_payload_without_poisoning_output
    use crate::sumeragi::{
        message::BlockMessage,
        output_guard::ConsensusOutputGuard,
        v2_lane_driver::{NativeLaneDriver, NativeLaneOwnedAdmission,
            native_driver_owned_ingress_for_test},
    };
    let now = std::time::Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let guard = ConsensusOutputGuard::isolated();
    let mut limits = native_driver_limits_for_test();
    limits.voting_enabled = false;
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, 0), limits).unwrap();
    let inbound = native_driver_owned_ingress_for_test(
        BlockMessage::NativeLane(native_driver_control_for_test(&fixture, lane, 1)),
        lane.frozen().committee[1].clone());
    let original = inbound.ingress_ownership().unwrap().process_local_projection_hash();
    let rejected = match driver.admit_owned(inbound).unwrap() {
        NativeLaneOwnedAdmission::Rejected { inbound, reason } => {
            assert!(reason.contains("voting signer"));
            inbound
        }
        _ => panic!("observer cannot admit voting control"),
    };
    assert_eq!(rejected.ingress_ownership().unwrap().process_local_projection_hash(), original);
    assert!(!guard.restart_required());
    driver.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_owned_ingress_without_original_fair_evidence_fails_closed
    use crate::sumeragi::{
        InboundBlockMessage, message::BlockMessage,
        output_guard::ConsensusOutputGuard, v2_lane_driver::NativeLaneDriver,
    };
    let now = std::time::Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let guard = ConsensusOutputGuard::isolated();
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, 0), native_driver_limits_for_test()).unwrap();
    let unowned = InboundBlockMessage::from_authenticated_peer(
        BlockMessage::NativeLane(native_driver_control_for_test(&fixture, lane, 1)),
        lane.frozen().committee[1].clone());
    assert!(driver.admit_owned(unowned).is_err());
    assert!(guard.restart_required());
    driver.shutdown().join().unwrap();
}

// A signed replacement remains valid Native evidence but cannot replace the
// exact physical occurrence authenticated by the independent fair queue.
#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_owned_control_and_decision_require_exact_message_bytes
    use crate::sumeragi::{
        message::BlockMessage,
        output_guard::ConsensusOutputGuard,
        v2_core::{EventTag, Generation},
        v2_lane_driver::{NativeLaneDriver, NativeLaneOwnedAdmission,
            native_driver_owned_ingress_for_test},
        v2_lane_wire::LaneAuthenticator,
    };
    let now = std::time::Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) =
        fixture.state.first_lane_admitted_input(&observed, lane).unwrap()
    else { panic!("real authenticated first input"); };
    let LaneInputBodyPreparationV1::Ready(body) =
        fixture.state.prepare_lane_input_body(&observed, lane, &source).unwrap()
    else { panic!("original complete input body"); };
    let control = native_driver_control_for_test(&fixture, lane, 1);
    let replacement_control = native_driver_control_for_test(&fixture, lane, 2);
    let decision = sign_native_group_decision_for_test(lane, &fixture.keys, &body, 0, 0);
    let replacement_decision = sign_native_group_decision_for_test(lane, &fixture.keys, &body, 0, 1);
    let auth = LaneAuthenticator::new(lane);
    let tag = EventTag::new(lane.reducer_context().height(), 0, Generation::INITIAL);
    auth.event(&control.message, tag).unwrap();
    auth.event(&replacement_control.message, tag).unwrap();
    auth.decision_certificate(&decision).unwrap();
    auth.decision_certificate(&replacement_decision).unwrap();
    let control = BlockMessage::NativeLane(control);
    let decision = BlockMessage::NativeLaneDecision(Box::new(decision));
    let before = crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap();
    for (original, replacement, other_family) in [
        (control.clone(), BlockMessage::NativeLane(replacement_control), decision.clone()),
        (decision, BlockMessage::NativeLaneDecision(Box::new(replacement_decision)), control),
    ] {
        let inbound = native_driver_owned_ingress_for_test(
            original, lane.frozen().committee[1].clone());
        let ownership = inbound.ingress_ownership().unwrap();
        assert!(ownership.validate_exact());
        assert!(ownership.matches_message(inbound.message()),
            "both Native families must retain their original canonical body");
        assert!(!ownership.matches_message(&replacement),
            "another correctly signed message cannot replace the admitted bytes");
        assert!(!ownership.matches_message(&other_family),
            "Native control and Decision owners are not interchangeable");
        assert!(ownership.matches_semantic_origin(inbound.sender()));
        assert!(ownership.matches_reply_routes(inbound.reply_routes()));
        let guard = ConsensusOutputGuard::isolated();
        let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
            native_process_key(&fixture, lane, 0), native_driver_limits_for_test()).unwrap();
        assert!(matches!(driver.admit_owned(inbound).unwrap(), NativeLaneOwnedAdmission::Accepted),
            "the exact original must also cross the actual driver ownership seam");
        assert!(!guard.restart_required());
        assert_eq!(crate::snapshot::canonical_state_snapshot_hash(&fixture.state).unwrap(), before,
            "fair ownership and Native admission do not authorize economic Apply");
        driver.shutdown().join().unwrap();
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_driver_production_cleanup_releases_repeated_obsolete_control_capacity
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_core as core,
        v2_lane_driver::{NativeLaneAdmission, NativeLaneDriver, NativeLaneInput},
    };
    use iroha_data_model::block::lane_consensus::LaneMessageV1;
    use std::time::Instant;
    let now = Instant::now();
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let signer = lane.reducer_context().roster().iter()
        .position(|validator| validator.id() != lane.reducer_context().leader(0)).unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let mut limits = native_driver_limits_for_test();
    limits.outbound = nonzero!(1_usize);
    limits.process.effect_limit = 3 * core::MAX_EFFECTS_PER_STEP;
    let rounds = limits.process.effect_limit + 8;
    let mut driver = NativeLaneDriver::new(Arc::clone(&fixture.state), Arc::clone(&guard),
        native_process_key(&fixture, lane, signer), limits).unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    while driver.process().instance(id).is_none() {
        driver.poll(&observed, now).unwrap();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    }
    let original = std::ptr::from_ref(driver.process().instance(id).unwrap());
    let mut retired_controls = 0;
    // Keep the real outbox full. Each genuine certified view advance obsoletes
    // retained control output, so this exceeds the complete descriptor limit.
    // No test removes a retirement or enlarges that limit.
    for view in 0..u64::try_from(rounds).unwrap() {
        for signer in 0..3 {
            let mut envelope = native_driver_control_for_test(&fixture, lane, signer);
            let LaneMessageV1::TimeoutVote(vote) = &mut envelope.message else { unreachable!() };
            vote.body.round.voting_view = view;
            vote.share.signature = Signature::try_new(
                native_process_key(&fixture, lane, signer).private_key(),
                &vote.body.signature_preimage().unwrap(),
            ).unwrap().payload().to_vec();
            assert!(matches!(driver.admit(&observed, NativeLaneInput::Control(envelope)),
                NativeLaneAdmission::Accepted));
        }
        while driver.process().instance(id).unwrap().tag().view() != view + 1 {
            driver.prepare_one_retirement().unwrap();
            driver.poll(&observed, now).unwrap();
            assert!(Instant::now() < until, "retired control capacity cannot strand a later timeout certificate");
            std::thread::sleep(Duration::from_millis(1));
        }
        let retained = driver.process().instance(id).unwrap().retirement_count();
        retired_controls += retained;
        if retained != 0 {
            // Queueing cleanup only moves custody; it cannot release the original
            // instance descriptor before the exact physical completion returns.
            for _ in 0..driver.process().occupancy().instances {
                driver.prepare_one_retirement().unwrap();
            }
            assert_eq!(driver.process().instance(id).unwrap().retirement_count(), retained);
        }
        while driver.process().instance(id).unwrap().retirement_count() != 0 {
            driver.prepare_one_retirement().unwrap();
            driver.poll(&observed, now).unwrap();
            assert!(Instant::now() < until, "existing bounded Body worker must return cleanup custody");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert_eq!(std::ptr::from_ref(driver.process().instance(id).unwrap()), original);
        assert!(!driver.process().instance(id).unwrap().held_effects()
            .any(|effect| matches!(effect, core::Effect::Apply { .. })));
        assert!(!guard.restart_required());
    }
    assert!(retired_controls >= rounds - 2, "each later view retires real retained output");
    assert_eq!(driver.process().instance(id).unwrap().tag().view(), u64::try_from(rounds).unwrap());
    driver.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_cleanup_exact_completion_keeps_capacity_and_closed_body_authority
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard, v2_core as core,
        v2_lane_instance::{LaneInputOutcome, LanePhysicalPool, LaneProcessOwner, LaneWorkerClass},
    };
    use std::{sync::mpsc, time::Instant};
    let now = Instant::now();
    let due = now + Duration::from_secs(1);
    let fixture = native_process_fixture(false, now);
    let observed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let signer = lane.reducer_context().roster().iter()
        .position(|validator| validator.id() == lane.reducer_context().leader(1)).unwrap();
    assert_ne!(lane.reducer_context().leader(0), lane.reducer_context().leader(1));
    let guard = ConsensusOutputGuard::isolated();
    let limits = native_process_limits_for_test();
    let pool = LanePhysicalPool::new(Arc::clone(&fixture.state), Arc::clone(&guard), limits).unwrap();
    let mut table = LaneProcessOwner::new(Arc::clone(&fixture.state), Arc::clone(&guard), limits).unwrap();
    native_process_open_for_test(&mut table, &pool, &fixture, &observed, lane, signer, now);
    let until = Instant::now() + Duration::from_secs(30);
    let pump = |table: &mut LaneProcessOwner| {
        if let Some(completed) = pool.try_completion().unwrap() {
            table.accept_completion(completed, &observed).unwrap();
        }
        table.service_one(id, &observed, due).unwrap();
        table.prepare_persistence(id).unwrap();
        table.dispatch_one(&pool, LaneWorkerClass::Wal).unwrap();
        assert!(Instant::now() < until);
        std::thread::sleep(Duration::from_millis(1));
    };
    table.poll_clock(id, &observed, due).unwrap();
    while !table.instance(id).unwrap().held_effects().any(|effect|
        matches!(effect, core::Effect::Broadcast(core::ConsensusMessageV2::TimeoutVote(_)))) {
        pump(&mut table);
    }
    // These actual signed shares retire the held local timeout only after the
    // matching InstallTimeout record is fsynced and acknowledged by the reducer.
    for signer in 0..3 {
        let envelope = native_driver_control_for_test(&fixture, lane, signer);
        loop {
            if !matches!(table.offer(id, &observed, &envelope.message).unwrap(), LaneInputOutcome::Backpressured) { break; }
            pump(&mut table);
        }
    }
    while table.instance(id).unwrap().tag().view() != 1
        || table.instance(id).unwrap().timeout_deadline() != Some(due + Duration::from_secs(2)) {
        pump(&mut table);
    }
    let retained = table.instance(id).unwrap().retirement_count();
    assert!(retained > 0);
    let records = table.instance(id).unwrap().native_records().to_vec();
    table.prepare_body(id, &observed).unwrap();
    assert!(table.prepare_retirement(id).unwrap());
    assert!(!table.prepare_retirement(id).unwrap(), "one original cleanup job per instance");
    assert_eq!(table.instance(id).unwrap().retirement_count(), retained);
    let (entered, entered_rx) = mpsc::channel();
    let (release, release_rx) = mpsc::channel();
    table.hold_next_completion_for_test(id, LaneWorkerClass::Body, move || {
        entered.send(()).unwrap();
        let _ = release_rx.recv();
    }).unwrap();
    table.dispatch_one(&pool, LaneWorkerClass::Body).unwrap();
    entered_rx.recv_timeout(Duration::from_secs(10)).unwrap();
    assert!(table.queued_context_for_test(id, LaneWorkerClass::Body).is_none(),
        "the genuine body job wins before cleanup on the same physical class");
    assert!(table.has_queued_job_for_test(id, LaneWorkerClass::Body), "cleanup still owns its queued job");
    table.dispatch_one(&pool, LaneWorkerClass::Body).unwrap();
    assert_eq!(table.occupancy().transferred, 2,
        "the body worker is held and the one-slot Body queue retains cleanup");
    assert_eq!(table.instance(id).unwrap().retirement_count(), retained);
    native_process_advance(&fixture, true);
    let closed = fixture.state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(closed.contexts().is_empty());
    table.reconcile(&closed);
    table.prepare_closed_drain(id).unwrap();
    assert!(table.take_closed(id).is_none(), "physical closure cannot bypass in-flight cleanup");
    assert!(!guard.restart_required());
    release.send(()).unwrap();
    let forged = Arc::new(());
    let (mut completed, original) = loop {
        let mut completed = native_process_receive(&pool);
        if let Some(original) = completed.swap_retirement_ticket_for_test(Arc::clone(&forged)) {
            break (completed, original);
        }
        table.accept_completion(completed, &closed).unwrap();
    };
    let charged = table.instance(id).unwrap().retirement_count();
    let (_, returned) = table.accept_completion(completed, &closed).err().expect("foreign inner cleanup ticket must fail");
    completed = returned;
    assert_eq!(table.instance(id).unwrap().retirement_count(), charged,
        "even a real outer completion cannot release another cleanup receipt");
    assert!(guard.restart_required(), "substituted physical completion fences output");
    assert!(Arc::ptr_eq(&completed.swap_retirement_ticket_for_test(original).unwrap(), &forged));
    table.accept_completion(completed, &closed).unwrap();
    assert_eq!(table.instance(id).unwrap().retirement_count(), charged - 1);
    assert!(!table.prepare_retirement(id).unwrap(), "closure body still requires genuine publication");
    assert_eq!(table.instance(id).unwrap().native_records(), records);
    table.prepare_closed_drain(id).unwrap();
    table.dispatch_one(&pool, LaneWorkerClass::Body).unwrap();
    table.accept_completion(native_process_receive(&pool), &closed).unwrap();
    let mut owner = table.take_closed(id).expect("only exact physical completions release closed custody");
    let body = owner.take_retirement().expect("original closed-body result remains owned");
    assert!(body.requires_recovery());
    assert!(body.body_bytes().is_some());
    assert!(!owner.instance().held_effects().any(|effect| matches!(effect, core::Effect::Apply { .. })));
    drop(body);
    drop(owner);
    drop(table);
    pool.shutdown().join().unwrap();
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_source_closure_releases_exact_request_before_response
    native_source_retirement_fixture(false, false);
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_source_closure_releases_exact_buffered_response
    native_source_retirement_fixture(true, false);
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_source_closure_preserves_original_physical_body_and_output_fence
    native_source_retirement_fixture(false, true);
}

fn native_source_retirement_fixture(buffered_response: bool, hold_body: bool) {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard,
        v2_lane_driver::NativeLaneDriver,
        v2_lane_instance::{LaneCurrentGate, LaneProcessOwner},
        v2_runner::NativeSourceRequestTestProbe,
    };
    use std::{sync::mpsc, time::Instant};
    let now = Instant::now();
    let mut fixture = native_process_fixture(false, now);
    let observed = fixture
        .state
        .verified_lane_consensus_contexts()
        .unwrap()
        .unwrap();
    let lane = &observed.contexts()[0];
    let id = lane.instance_id();
    let leader = lane
        .reducer_context()
        .roster()
        .iter()
        .position(|member| member.id() == lane.reducer_context().leader(0))
        .unwrap();
    let (request, response, outstanding) = native_driver_recovery_exchange_for_test(&fixture);
    fixture
        .state
        .kura
        .evict_first_admission_body_for_testing(
            NonZeroUsize::new(fixture.block.header().height().get() as usize).unwrap(),
            fixture.block.hash(),
        )
        .unwrap();
    let guard = ConsensusOutputGuard::isolated();
    let mut driver = NativeLaneDriver::new(
        Arc::clone(&fixture.state),
        Arc::clone(&guard),
        native_process_key(&fixture, lane, leader),
        native_driver_limits_for_test(),
    )
    .unwrap();
    let until = Instant::now() + Duration::from_secs(30);
    loop {
        driver.poll(&observed, now).unwrap();
        if driver
            .process()
            .instance(id)
            .is_some_and(|owner| owner.source_recovery_requirement().is_some())
        {
            break;
        }
        assert!(
            Instant::now() < until,
            "real Native body job returns its authenticated source need"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    let source = Arc::new(
        driver
            .process()
            .instance(id)
            .unwrap()
            .source_recovery_requirement()
            .unwrap()
            .clone(),
    );
    let source_keys = native_preparation_global_keys(&source.finality().height_context);
    #[cfg(feature = "bls")]
    NativeSourceRequestTestProbe::instance(
        driver
            .process()
            .source_recovery_target(id, &observed)
            .unwrap(),
        Arc::clone(&source),
        &source_keys[0],
        now,
    )
    .assert_observation_deadline(Arc::clone(&fixture.state), &source_keys[0], now);
    let target = driver
        .process()
        .source_recovery_target(id, &observed)
        .unwrap();
    assert_eq!(target.instance_id(), id);
    let foreign_fixture = native_process_fixture(false, now);
    let foreign = LaneProcessOwner::new(
        Arc::clone(&foreign_fixture.state),
        ConsensusOutputGuard::isolated(),
        native_process_limits_for_test(),
    )
    .unwrap();
    assert_eq!(
        foreign.source_recovery_target_gate(&target, &observed),
        LaneCurrentGate::ObservationChanged,
        "another original State family cannot consume this target"
    );
    let mut retained =
        NativeSourceRequestTestProbe::instance(target, Arc::clone(&source), &source_keys[0], now);
    let ticket = retained.backpressure();
    assert!(
        retained.admits_hash(request.request_hash()),
        "the real constructor signs the exact historical request"
    );
    assert_eq!(
        retained.retire(driver.process(), None),
        LaneCurrentGate::ObservationChanged
    );
    assert_eq!(
        retained.retire(driver.process(), Some(&observed)),
        LaneCurrentGate::Current
    );
    assert!(retained.retains_request());
    assert_eq!(ticket.waiter_count(), 1);
    assert_eq!(ticket.ticket_drop_cancellations(), 0);
    if buffered_response {
        retained.accept(response.response().clone(), &request.request().requester);
    }
    let mut validation =
        NativeSourceRequestTestProbe::non_instance(Arc::clone(&source), &source_keys[0], now, true);
    let mut candidate = NativeSourceRequestTestProbe::non_instance(
        Arc::clone(&source),
        &source_keys[0],
        now,
        false,
    );
    let mut release_body = None;
    if hold_body {
        driver
            .complete_source_recovery(id, &request, &response)
            .unwrap();
        let (entered, entered_rx) = mpsc::channel();
        let (release, release_rx) = mpsc::channel();
        driver.hold_next_body_completion_for_test(id, move || {
            entered.send(()).unwrap();
            let _ = release_rx.recv();
        });
        loop {
            driver.poll(&observed, now).unwrap();
            if entered_rx.try_recv().is_ok() {
                break;
            }
            assert!(
                Instant::now() < until,
                "the original body remains in a real physical worker"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(driver.process().occupancy().transferred > 0);
        release_body = Some(release);
    }
    // A new global carrier with the same immutable lane instance is not closure.
    fixture.block = native_process_advance(&fixture, false);
    let current = fixture
        .state
        .verified_lane_consensus_contexts()
        .unwrap()
        .unwrap();
    assert!(source.is_current_in(&current));
    assert_eq!(
        candidate.retire_closed_candidate(&fixture.state, Some(&current)),
        LaneCurrentGate::Current,
        "a new global carrier does not cancel the still-current candidate source"
    );
    assert_eq!(
        retained.retire(driver.process(), Some(&observed)),
        LaneCurrentGate::ObservationChanged
    );
    assert_eq!(
        retained.retire(driver.process(), Some(&current)),
        LaneCurrentGate::Current
    );
    assert!(retained.retains_request());
    assert_eq!(ticket.waiter_count(), 1);
    let original = std::ptr::from_ref(driver.process().instance(id).unwrap());
    let original_records = driver
        .process()
        .instance(id)
        .unwrap()
        .native_records()
        .to_vec();
    let original_effects = driver
        .process()
        .instance(id)
        .unwrap()
        .held_effects()
        .cloned()
        .collect::<Vec<_>>();
    let original_work = driver.process().occupancy().transferred;
    fixture.block = native_process_advance(&fixture, true);
    let closed = fixture
        .state
        .verified_lane_consensus_contexts()
        .unwrap()
        .unwrap();
    assert!(closed.contexts().is_empty());
    assert!(!source.is_current_in(&closed));
    if buffered_response {
        let mut completed_candidate = NativeSourceRequestTestProbe::non_instance(
            Arc::clone(&source), &source_keys[0], now, false,
        );
        completed_candidate.accept(response.response().clone(), &request.request().requester);
        assert_eq!(
            completed_candidate.retire_closed_candidate(&fixture.state, None),
            LaneCurrentGate::Current,
            "an authenticated body can settle while State publication moves"
        );
        assert_eq!(
            completed_candidate.retire_closed_candidate(&fixture.state, Some(&closed)),
            LaneCurrentGate::Current,
            "an authenticated buffered body remains available for another current route"
        );
        assert!(completed_candidate.retains_request());
    }
    #[cfg(feature = "bls")]
    if !buffered_response && !hold_body {
        NativeSourceRequestTestProbe::assert_candidate_source_pruning(
            Arc::clone(&fixture.state),
            Arc::clone(&source),
            &current,
            &closed,
            &source_keys[0],
        );
    }
    assert!(
        driver
            .process()
            .source_recovery_target(id, &current)
            .is_none(),
        "stale observation cannot issue a target"
    );
    assert!(
        driver
            .process()
            .source_recovery_target(id, &closed)
            .is_none(),
        "authenticated closure cannot issue a target even before table drain"
    );
    assert_eq!(
        retained.retire(driver.process(), Some(&closed)),
        LaneCurrentGate::InstanceClosed
    );
    assert!(
        !retained.retains_request(),
        "the sole source slot is available for another retained requirement"
    );
    assert!(
        !retained.admits_hash(request.request_hash()),
        "a late response cannot reacquire the retired slot"
    );
    assert_eq!(ticket.waiter_count(), 0);
    assert_eq!(
        ticket.ticket_drop_cancellations(),
        1,
        "only this exact actor wait was cancelled"
    );
    assert_eq!(
        retained.retire(driver.process(), Some(&closed)),
        LaneCurrentGate::Current,
        "retirement is consumed once"
    );
    assert_eq!(
        std::ptr::from_ref(driver.process().instance(id).unwrap()),
        original
    );
    assert_eq!(
        driver.process().instance(id).unwrap().native_records(),
        original_records
    );
    assert_eq!(
        driver
            .process()
            .instance(id)
            .unwrap()
            .held_effects()
            .cloned()
            .collect::<Vec<_>>(),
        original_effects
    );
    assert_eq!(driver.process().occupancy().transferred, original_work);
    if !hold_body {
        assert_eq!(
            driver
                .process()
                .instance(id)
                .unwrap()
                .source_recovery_requirement()
                .unwrap()
                .carrier_hash(),
            source.carrier_hash(),
            "request retirement cannot settle original body/source custody"
        );
    }
    for retained in [&mut validation, &mut candidate] {
        assert_eq!(
            retained.retire(driver.process(), Some(&closed)),
            LaneCurrentGate::Current
        );
        assert!(
            retained.retains_request(),
            "validation and candidate owners need their own completion/cancellation authority"
        );
    }
    let exact_validate = (source.finality().subject, 0, Arc::clone(&source));
    assert!(!validation.retire_released_validation(Some(&exact_validate)));
    let validation_ticket = validation.backpressure();
    let wrong_index = (exact_validate.0, 1, Arc::clone(&source));
    assert!(validation.retire_released_validation(Some(&wrong_index)));
    assert!(!validation.retains_request());
    assert_eq!(validation_ticket.waiter_count(), 0);
    assert_eq!(validation_ticket.ticket_drop_cancellations(), 1);
    let mut different_source = NativeSourceRequestTestProbe::non_instance(
        Arc::clone(&source), &source_keys[0], now, true,
    );
    let copied_source = (exact_validate.0, 0, Arc::new(source.as_ref().clone()));
    assert!(different_source.retire_released_validation(Some(&copied_source)));
    let mut superseded = NativeSourceRequestTestProbe::non_instance(
        Arc::clone(&source), &source_keys[0], now, true,
    );
    let superseded_ticket = superseded.backpressure();
    assert!(superseded.retire_released_validation(None));
    assert_eq!(superseded_ticket.waiter_count(), 0);
    assert_eq!(superseded_ticket.ticket_drop_cancellations(), 1);
    assert!(!candidate.retire_released_validation(None));
    assert!(candidate.retains_request());
    let candidate_ticket = candidate.backpressure();
    assert_eq!(
        candidate.retire_closed_candidate(&fixture.state, None),
        LaneCurrentGate::ObservationChanged,
        "missing current State cannot cancel an outstanding candidate request"
    );
    assert_eq!(
        candidate.retire_closed_candidate(&fixture.state, Some(&current)),
        LaneCurrentGate::ObservationChanged,
        "a stale observation cannot retire a candidate source"
    );
    assert!(candidate.retains_request());
    assert_eq!(
        candidate.retire_closed_candidate(&fixture.state, Some(&closed)),
        LaneCurrentGate::InstanceClosed,
        "authenticated lane closure frees the shared recovery slot"
    );
    assert_eq!(candidate_ticket.waiter_count(), 0);
    assert_eq!(candidate_ticket.ticket_drop_cancellations(), 1);
    assert_eq!(
        outstanding.len(),
        1,
        "unrelated global request custody is intact"
    );
    assert!(!guard.restart_required());
    if let Some(release) = release_body {
        release.send(()).unwrap();
        loop {
            driver.poll(&closed, now).unwrap();
            if driver
                .process()
                .instance(id)
                .is_some_and(|owner| owner.retirement_count() != 0)
            {
                break;
            }
            assert!(
                Instant::now() < until,
                "original closed body completion must remain owned"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(
            driver
                .process()
                .source_recovery_target(id, &closed)
                .is_none()
        );
        let body = driver
            .take_retirement(id)
            .expect("only the body owner transfers its original result");
        assert!(body.requires_recovery());
        assert!(body.body_bytes().is_some());
        assert!(!guard.restart_required());
        drop(body);
        assert!(
            guard.restart_required(),
            "the original unfinished body still fences output"
        );
    } else {
        for _ in 0..4 {
            driver.poll(&closed, now).unwrap();
        }
        assert!(
            driver
                .process()
                .source_recovery_target(id, &closed)
                .is_none(),
            "Closing/Closed inspection never admits another request"
        );
        assert!(!guard.restart_required());
    }
    driver.shutdown().join().unwrap();
}
