// Process-lived owner controls using actual finalized State, Kura, RS16 and WAL.

// Tests explicitly run the real move-owned disk job on a worker thread. This
// convenience is fixture-only; production service_one never appends or fsyncs.
trait LaneInstanceWorkerTestExt {
    fn service_with_worker(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
        now: std::time::Instant,
    ) -> std::result::Result<
        crate::sumeragi::v2_lane_instance::LaneService,
        crate::sumeragi::v2_lane_instance::LaneInstanceError,
    >;
}
impl LaneInstanceWorkerTestExt for crate::sumeragi::v2_lane_instance::LaneInstance {
    fn service_with_worker(
        &mut self,
        state: &State,
        observed: &VerifiedLaneContexts,
        now: std::time::Instant,
    ) -> std::result::Result<
        crate::sumeragi::v2_lane_instance::LaneService,
        crate::sumeragi::v2_lane_instance::LaneInstanceError,
    > {
        use crate::sumeragi::v2_lane_instance::{LanePersistenceLaunch, LaneService};
        let service = self.service_one(state, observed, now)?;
        if !matches!(service, LaneService::NeedsPersistenceWorker) {
            return Ok(service);
        }
        let LanePersistenceLaunch::Job(job) = self.take_persistence_job()? else {
            panic!("fixture reserved a physical worker slot for the exact issued effect");
        };
        let completed = std::thread::scope(|scope| scope.spawn(move || job.run()).join().unwrap());
        self.finish_persistence_job(completed)
            .map_err(|(error, completed)| {
                drop(completed);
                error
            })
    }
}

fn open_lane_instance_for_test(
    fixture: &LaneContextVerifiedFixture,
    observed: &VerifiedLaneContexts,
    lane: &VerifiedLaneContext,
    signer: usize,
    now: std::time::Instant,
) -> crate::sumeragi::v2_lane_instance::LaneInstance {
    use crate::sumeragi::{
        output_guard::ConsensusOutputGuard, v2_core, v2_lane_instance::LaneInstance,
    };
    let key = fixture
        .validators
        .iter()
        .find(|key| key.public_key() == lane.frozen().committee[signer].public_key())
        .unwrap()
        .clone();
    LaneInstance::open_with_worker_for_test(
        &fixture.state,
        observed,
        lane,
        key,
        ConsensusOutputGuard::isolated(),
        now,
        std::time::Duration::from_secs(1),
        std::time::Duration::from_millis(100),
        3 * v2_core::MAX_EFFECTS_PER_STEP,
    )
    .unwrap()
}

// Find a real representable Instant at which every positive future interval
// overflows, without relying on the platform's clock epoch/range.
fn maximal_lane_clock_instant_for_test(now: std::time::Instant) -> std::time::Instant {
    use std::time::Duration;
    let mut low = 0_u64;
    let mut high = u64::MAX;
    while low < high {
        let middle = low + (high - low) / 2 + 1;
        if now.checked_add(Duration::from_secs(middle)).is_some() {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let seconds = now.checked_add(Duration::from_secs(low)).unwrap();
    let mut low = 0_u64;
    let mut high = 999_999_999_u64;
    while low < high {
        let middle = low + (high - low) / 2 + 1;
        if seconds.checked_add(Duration::from_nanos(middle)).is_some() {
            low = middle;
        } else {
            high = middle - 1;
        }
    }
    let maximum = seconds.checked_add(Duration::from_nanos(low)).unwrap();
    assert!(maximum.checked_add(Duration::from_nanos(1)).is_none());
    maximum
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_prepayload_failover_survives_fsync_restart_and_full_outbox
    use crate::sumeragi::{v2_core as core, v2_lane_instance::{LaneInputOutcome, LaneService}, v2_lane_wire::LaneWalRecordV1};
    use iroha_data_model::block::lane_consensus::{LaneMessageV1, LaneTcV1};
    use std::{sync::mpsc, time::{Duration, Instant}};
    let fixture = finalized_lane_wal_fixture();
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let context = lane.reducer_context();
    let silent = context.leader(0);
    let survivors = context.roster().iter().enumerate().filter_map(|(index, validator)| (validator.id() != silent).then_some(index)).collect::<Vec<_>>();
    assert_eq!(survivors.len(), 3);
    let start = Instant::now();
    let due = start + Duration::from_secs(1);
    let mut owners = Vec::new();
    let mut votes = Vec::new();
    for (position, signer) in survivors.into_iter().enumerate() {
        let mut owner = open_lane_instance_for_test(&fixture, &observed, lane, signer, start);
        assert_eq!(owner.timeout_deadline(), Some(due), "the same owner starts before any payload exists");
        assert!(matches!(owner.poll_clock(state, &observed, due).unwrap(), LaneInputOutcome::Stepped(receipt) if receipt.disposition == core::StepDisposition::Applied));
        assert!(matches!(owner.held_effects().collect::<Vec<_>>().as_slice(), [core::Effect::Persist { .. }]));
        assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::PersistedAwaitingAck));
        assert_eq!(owner.native_records().len(), 1);
        let exact = owner.native_records()[0].clone();
        assert!(matches!(&exact.record, LaneWalRecordV1::TimeoutIntent { body, .. } if body.highest_prepare.is_none()));
        if position == 0 {
            drop(owner); // Real fsync happened; the reducer never received its ack.
            owner = open_lane_instance_for_test(&fixture, &observed, lane, signer, due);
            assert_eq!(owner.native_records(), std::slice::from_ref(&exact));
        } else {
            assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::Completion(receipt) if receipt.disposition == core::StepDisposition::Applied));
        }
        assert!(matches!(owner.held_effects().collect::<Vec<_>>().as_slice(), [core::Effect::Sign { message: core::SignableMessage::TimeoutVote(_), .. }]));
        assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::SignedAwaitingAck));
        assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::Completion(receipt) if receipt.disposition == core::StepDisposition::Applied));
        let (sender, receiver) = mpsc::sync_channel(1);
        assert!(matches!(owner.flush_one(state, &observed, &sender).unwrap(), LaneService::Sent));
        let packet = receiver.recv().unwrap();
        assert_eq!(packet.canonical_bytes, norito::encode_canonical(&packet.envelope).unwrap());
        assert_eq!(packet.destinations, lane.frozen().committee);
        let LaneMessageV1::TimeoutVote(vote) = &packet.envelope.message else { panic!("actual signed timeout") };
        votes.push(vote.clone());
        sender.try_send(packet).unwrap(); // Keep the actual intended channel full.
        if position != 0 {
            let held = owner.held_effects().cloned().collect::<Vec<_>>();
            let records = owner.native_records().to_vec();
            assert!(owner.poll_clock(state, &observed, maximal_lane_clock_instant_for_test(start)).is_err());
            assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), held, "retransmit deadline overflow does not offer an event");
            assert_eq!(owner.native_records(), records);
        }
        let retransmit = due + Duration::from_millis(100);
        assert!(matches!(owner.poll_clock(state, &observed, retransmit).unwrap(), LaneInputOutcome::Stepped(_)));
        assert!(matches!(owner.flush_one(state, &observed, &sender).unwrap(), LaneService::OutboxFull));
        let retained = owner.held_effects().cloned().collect::<Vec<_>>();
        for tick in 2..10 {
            owner.poll_clock(state, &observed, due + Duration::from_millis(tick * 100)).unwrap();
            assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), retained, "exact retransmission coalesces into the existing owned output");
        }
        owners.push((signer, owner, sender, receiver));
    }
    votes.sort_by_key(|vote| vote.share.signer);
    let tc = LaneTcV1 { round: votes[0].body.round, votes: votes.clone() };
    let mut recovered_tc = None;
    let installed = due + Duration::from_secs(1);
    for (signer, mut owner, sender, receiver) in owners {
        assert_eq!(owner.tag().view(), 0);
        let mut retained_inputs = Vec::new();
        for vote in &votes {
            let message = LaneMessageV1::TimeoutVote(vote.clone());
            match owner.offer(state, &observed, &message).unwrap() {
                LaneInputOutcome::Stepped(_) => {},
                LaneInputOutcome::Backpressured => {
                    assert!(owner.held_effects().any(|effect| matches!(effect, core::Effect::Persist { .. })),
                        "only the actual in-flight TC acknowledgement can defer this vote; the full outbox cannot");
                    assert_eq!(owner.native_records().len(), 1, "no TC acknowledgement has occurred yet");
                    retained_inputs.push(message);
                },
                other => panic!("full output cannot block authenticated timeout admission: {other:?}"),
            }
        }
        assert!(owner.held_effects().any(|effect| matches!(effect, core::Effect::Persist { .. })));
        assert!(matches!(owner.service_with_worker(state, &observed, installed).unwrap(), LaneService::PersistedAwaitingAck),
            "a retained Broadcast cannot hide the issued fsync");
        assert_eq!(owner.tag().view(), 0);
        let overflow = maximal_lane_clock_instant_for_test(start);
        assert!(owner.service_with_worker(state, &observed, overflow).is_err());
        assert_eq!(owner.tag().view(), 0, "deadline preflight cannot consume the held fsync acknowledgement");
        let LaneService::Completion(receipt) = owner.service_with_worker(state, &observed, installed).unwrap() else { panic!("durable TC ack") };
        assert_eq!(owner.tag().view(), 1);
        assert_eq!(receipt.disposition, core::StepDisposition::Applied);
        assert_eq!(owner.retirement_count(), 1);
        let retirement = owner.take_retirement().expect("original retained timeout packet");
        assert_eq!(retirement.instance(), lane.instance_id());
        assert!(retirement.belongs_to(state));
        assert!(!retirement.requires_recovery(), "the shared reducer retired this exact obsolete timeout");
        assert!(owner.take_retirement().is_none(), "one consuming handoff");
        assert!(matches!(retirement.effect(), Some(core::Effect::Broadcast(core::ConsensusMessageV2::TimeoutVote(_)))),
            "the obsolete local timeout is explicitly retired only by the reducer's durable TC transition");
        let retired_packet = retirement.packet().expect("full channel retained the exact native bytes");
        assert_eq!(retired_packet.canonical_bytes, norito::encode_canonical(&retired_packet.envelope).unwrap());
        let exact_enter = owner.held_effects().find(|effect| matches!(effect, core::Effect::EnterView { .. })).unwrap().clone();
        assert!(owner.service_with_worker(state, &observed, overflow).is_err());
        assert!(owner.held_effects().any(|effect| effect == &exact_enter), "clock overflow cannot consume EnterView");
        assert!(matches!(owner.service_with_worker(state, &observed, installed).unwrap(), LaneService::EnteredView(tag) if tag.view() == 1));
        let durable_records = owner.native_records().to_vec();
        let held_after_install = owner.held_effects().cloned().collect::<Vec<_>>();
        for message in retained_inputs {
            assert!(matches!(owner.offer(state, &observed, &message).unwrap(), LaneInputOutcome::Stepped(_)),
                "the retained old-view vote is retried after the real TC acknowledgement");
            assert_eq!(owner.native_records(), durable_records, "retry cannot add another durable transition");
            assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), held_after_install,
                "the stale vote cannot invent an acknowledgement or output");
        }
        assert_ne!(context.leader(1), silent);
        assert_eq!(owner.timeout_deadline(), Some(installed + Duration::from_secs(2)));
        assert!(matches!(owner.flush_one(state, &observed, &sender).unwrap(), LaneService::OutboxFull));
        assert!(matches!(receiver.recv().unwrap().envelope.message, LaneMessageV1::TimeoutVote(_)), "healing drains the actual preexisting packet first");
        assert!(matches!(owner.flush_one(state, &observed, &sender).unwrap(), LaneService::Sent));
        let packet = receiver.recv().unwrap();
        assert_eq!(packet.envelope.message, LaneMessageV1::TimeoutCertificate(tc.clone()));
        assert_eq!(packet.canonical_bytes, norito::encode_canonical(&packet.envelope).unwrap());
        assert!(owner.held_effects().next().is_none());
        assert_eq!(owner.native_records().len(), 2);
        let expected_records = owner.native_records().to_vec();
        drop(owner);
        let restart = installed + Duration::from_secs(3);
        let mut reopened = open_lane_instance_for_test(&fixture, &observed, lane, signer, restart);
        assert_eq!(reopened.tag().view(), 1);
        assert_eq!(reopened.native_records(), expected_records);
        reopened.poll_clock(state, &observed, restart + Duration::from_millis(100)).unwrap();
        assert!(matches!(reopened.flush_one(state, &observed, &sender).unwrap(), LaneService::Sent));
        let replayed = receiver.recv().unwrap();
        assert_eq!(replayed.canonical_bytes, packet.canonical_bytes, "actual physical replay retains full native TC bytes");
        recovered_tc = Some(replayed.envelope.message);
    }
    let late_signer = context.roster().iter().position(|validator| validator.id() == silent).unwrap();
    let mut late = open_lane_instance_for_test(&fixture, &observed, lane, late_signer, start);
    assert!(matches!(late.offer(state, &observed, &recovered_tc.unwrap()).unwrap(), LaneInputOutcome::Stepped(_)));
    assert!(matches!(late.service_with_worker(state, &observed, installed).unwrap(), LaneService::PersistedAwaitingAck));
    assert!(matches!(late.service_with_worker(state, &observed, installed).unwrap(), LaneService::Completion(_)));
    assert!(matches!(late.service_with_worker(state, &observed, installed).unwrap(), LaneService::EnteredView(tag) if tag.view() == 1));
    assert!(late.held_effects().next().is_none());
    assert_eq!(late.native_records().len(), 1, "the initially silent fourth validator needs only the authenticated recovered TC");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_retains_clock_and_durable_completion_across_global_publication
    use crate::sumeragi::{v2_core as core, v2_lane_instance::{LaneCurrentGate, LaneInputOutcome, LaneService}};
    use std::time::{Duration, Instant};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let start = Instant::now();
    let due = start + Duration::from_secs(1);
    let mut owner = open_lane_instance_for_test(&fixture, &observed, lane, 0, start);
    owner.poll_clock(state, &observed, due).unwrap();
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::PersistedAwaitingAck));
    let tag = owner.tag();
    let records = owner.native_records().to_vec();
    let parent = state.kura.v2_finality_artifact(fixture.block.header().height().get()).unwrap().unwrap();
    let opening = crate::sumeragi::v2_context::build_successor_height_context(&parent, parent.height_context.nexus_amx_context_hash, None).unwrap();
    let later = empty_global_block_after(Some(&fixture.block));
    let mut overlay = state.block(later.header());
    overlay.finalize_lane_consensus_contexts(&later, Some(&opening)).unwrap();
    let mut witness = ExecWitness::default();
    overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.block_hashes.push(later.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay, &later);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(later.clone())).unwrap();
    let (artifact, receipt) = stage_lane_context_fixture_finality(state, &later, opening, witness);
    state.kura.promote_kagemusha_finality_sidecar(&artifact, &receipt).unwrap();
    assert!(!observed.is_current(state));
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::Completion(_)), "already-fsynced custody completes despite a stale global observation");
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::Gate(LaneCurrentGate::ObservationChanged)), "signing must use a fresh State lease");
    assert!(matches!(owner.held_effects().collect::<Vec<_>>().as_slice(), [core::Effect::Sign { .. }]));
    assert_eq!(owner.tag(), tag);
    assert_eq!(owner.native_records(), records);
    assert_eq!(owner.timeout_deadline(), None, "the consumed deadline does not reset on unrelated global progress");
    let current = state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(current.contexts()[0].instance_id(), lane.instance_id());
    assert!(matches!(owner.poll_clock(state, &observed, due).unwrap(), LaneInputOutcome::Gate(LaneCurrentGate::ObservationChanged)));
    assert!(matches!(owner.service_with_worker(state, &current, due).unwrap(), LaneService::SignedAwaitingAck));
    assert!(matches!(owner.service_with_worker(state, &current, due).unwrap(), LaneService::Completion(_)));
    assert_eq!(owner.tag(), tag);
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_replayed_body_sign_keeps_exact_effect_without_fabricating_ready
    use crate::sumeragi::{v2_core as core, v2_lane_instance::LaneService, v2_lane_payload::encode_lane_input,
        v2_lane_wal::LaneSafetyWal, v2_lane_wire::{LaneWalEnvelopeV1, LaneWalRecordV1}};
    use iroha_data_model::block::lane_consensus::{LANE_MESSAGE_VERSION_V1, LaneJustificationV1, LaneProposalBodyV1, LaneRoundV1};
    use std::time::Instant;
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("actual source") };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("actual all-route eligibility") };
    let manifest = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    let signer = manifest.value.origin_producer as usize;
    let mut wal = LaneSafetyWal::open(state.kura(), lane, signer as u32).unwrap();
    let mut store = wal.open_body_store().unwrap();
    let durable = store.persist(&body, &manifest).unwrap();
    store.validate_receipt(durable.receipt()).unwrap();
    let mut reducer = wal.recover(core::Generation::new(0)).unwrap();
    reducer.step(core::Event::ResumeAfterReplay { tag: reducer.current_tag() }).unwrap();
    // This fixture has the actual canonical body, exact all-route token and
    // physical fsync receipt. No invented Ready event feeds the process owner.
    let outcome = {
        let _lease = state.consensus_publication_lease();
        assert!(observed.is_current(state));
        reducer.step(core::Event::LocalProposalReady { tag: reducer.current_tag(), manifest: core::PayloadManifest::new(
            core::Subject::new(manifest.value.subject_hash().unwrap().into()), core::Digest::new(manifest.value.payload_hash.into()),
            core::Digest::new(manifest.chunk_root.into()), manifest.byte_len, manifest.chunk_count) }).unwrap()
    };
    let [effect @ core::Effect::Persist { entry, .. }] = outcome.effects() else { panic!("proposal persistence") };
    let native = LaneWalEnvelopeV1 { version: LANE_MESSAGE_VERSION_V1, persistence_id: entry.id().get(), record: LaneWalRecordV1::ProposalIntent(LaneProposalBodyV1 {
        round: LaneRoundV1 { instance_id: manifest.value.instance_id, lane_height: lane.reducer_context().height(), voting_view: 0 },
        proposer: signer as u32, manifest, justification: LaneJustificationV1::Opening }) };
    drop(wal.append_issued(effect, &native).unwrap());
    drop(durable); drop(store); drop(reducer); drop(wal);
    let now = Instant::now();
    let mut owner = open_lane_instance_for_test(&fixture, &observed, lane, signer, now);
    assert_eq!(owner.native_records(), std::slice::from_ref(&native));
    let exact = owner.held_effects().cloned().collect::<Vec<_>>();
    assert!(matches!(exact.as_slice(), [core::Effect::Sign { message: core::SignableMessage::Proposal(_), .. }]));
    for _ in 0..3 {
        assert!(matches!(owner.service_with_worker(state, &observed, now).unwrap(), LaneService::NeedsBodyAdapter));
        assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), exact);
    }
    let recovered = owner.body_store().read_for_manifest(&manifest).unwrap().unwrap();
    owner.body_store().validate_receipt(recovered.receipt()).unwrap();
    assert_eq!(recovered.canonical_bytes(), body.canonical_bytes());
    assert_eq!(owner.held_effects().cloned().collect::<Vec<_>>(), exact, "physical readback alone cannot acknowledge/sign the held body effect");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_timeout_preserves_high_prepare_while_body_effect_stays_owned
    use crate::sumeragi::{v2_core as core, v2_lane_instance::{LaneInputOutcome, LaneService}, v2_lane_payload::encode_lane_input, v2_lane_wire::LaneWalRecordV1};
    use iroha_data_model::block::lane_consensus::{LaneMessageV1, LanePhaseV1, LaneQcV1, LaneRoundV1,
        LaneSignatureShareV1, LaneTcV1, LaneTimeoutBodyV1, LaneTimeoutVoteV1, LaneVoteStatementV1};
    use std::{sync::mpsc, time::{Duration, Instant}};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed, lane).unwrap() else { panic!("source") };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed, lane, &source).unwrap() else { panic!("all routes") };
    let manifest = *encode_lane_input(lane, &body, 0).unwrap().manifest();
    let round = LaneRoundV1 { instance_id: manifest.value.instance_id, lane_height: lane.reducer_context().height(), voting_view: 0 };
    let statement = LaneVoteStatementV1 { round, phase: LanePhaseV1::Prepare, value: manifest.value };
    let signature_for = |signer: usize, bytes: &[u8]| {
        let key = fixture.validators.iter().find(|key| key.public_key() == lane.frozen().committee[signer].public_key()).unwrap();
        Signature::try_new(key.private_key(), bytes).unwrap().payload().to_vec()
    };
    let prepare = LaneQcV1 { statement, shares: (0..3).map(|signer| LaneSignatureShareV1 {
        signer: signer as u32, signature: signature_for(signer, &statement.signature_preimage().unwrap()),
    }).collect() };
    // These real remote signatures authenticate the exact prepared value. The
    // receiving owner has no body receipt and must retain Fetch, never fake Ready.
    let timeout = LaneTimeoutBodyV1 { round, highest_prepare: Some(prepare.clone()) };
    let votes = (0..3).map(|signer| LaneTimeoutVoteV1 { body: timeout.clone(), share: LaneSignatureShareV1 {
        signer: signer as u32, signature: signature_for(signer, &timeout.signature_preimage().unwrap()),
    }}).collect::<Vec<_>>();
    let now = Instant::now();
    let mut owner = open_lane_instance_for_test(&fixture, &observed, lane, 3, now);
    assert!(owner.body_store().read_for_manifest(&manifest).unwrap().is_none());
    let before = owner.tag();
    let mut corrupt = LaneTcV1 { round, votes: votes.clone() };
    corrupt.votes[0].share.signature[0] ^= 1;
    assert!(owner.offer(state, &observed, &LaneMessageV1::TimeoutCertificate(corrupt)).is_err());
    assert_eq!(owner.tag(), before);
    assert!(owner.native_records().is_empty());
    assert!(owner.held_effects().next().is_none());
    for (position, vote) in votes.into_iter().enumerate() {
        assert!(matches!(owner.offer(state, &observed, &LaneMessageV1::TimeoutVote(vote)).unwrap(), LaneInputOutcome::Stepped(_)));
        if position == 0 {
            let conflicting = LaneTimeoutBodyV1 { round, highest_prepare: None };
            let conflicting = LaneTimeoutVoteV1 { share: LaneSignatureShareV1 { signer: 0,
                signature: signature_for(0, &conflicting.signature_preimage().unwrap()) }, body: conflicting };
            assert!(matches!(owner.offer(state, &observed, &LaneMessageV1::TimeoutVote(conflicting)).unwrap(), LaneInputOutcome::Stepped(_)));
            assert!(matches!(owner.take_diagnostic(), Some(core::Effect::ReportEquivocation { evidence: core::EquivocationEvidence::Timeout { .. } })));
            assert!(owner.take_diagnostic().is_none());
        }
    }
    assert!(matches!(owner.service_with_worker(state, &observed, now).unwrap(), LaneService::PersistedAwaitingAck));
    assert!(matches!(owner.service_with_worker(state, &observed, now).unwrap(), LaneService::Completion(_)));
    assert!(matches!(owner.service_with_worker(state, &observed, now).unwrap(), LaneService::EnteredView(tag) if tag.view() == 1));
    assert!(owner.held_effects().any(|effect| matches!(effect, core::Effect::FetchBody { .. })));
    let due = owner.timeout_deadline().unwrap();
    assert_eq!(due, now + Duration::from_secs(2));
    let exact_fetch = owner.held_effects().find(|effect| matches!(effect, core::Effect::FetchBody { .. })).unwrap().clone();
    for tick in 1..20 {
        owner.poll_clock(state, &observed, now + Duration::from_millis(tick * 100)).unwrap();
        let fetches = owner.held_effects().filter(|effect| matches!(effect, core::Effect::FetchBody { .. })).cloned().collect::<Vec<_>>();
        assert_eq!(fetches, vec![exact_fetch.clone()], "one unlaunched exact Fetch owns repeated shared requests without exhausting control capacity");
    }
    owner.poll_clock(state, &observed, due).unwrap();
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::PersistedAwaitingAck));
    let LaneWalRecordV1::TimeoutIntent { body: retained, .. } = &owner.native_records().last().unwrap().record else { panic!("actual new timeout intent") };
    assert_eq!(retained.highest_prepare, Some(prepare.clone()));
    assert_eq!(retained.round.voting_view, 1);
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::Completion(_)));
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::SignedAwaitingAck));
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::Completion(_)));
    let (sender, receiver) = mpsc::sync_channel(1);
    let mut exact_timeout = None;
    while matches!(owner.flush_one(state, &observed, &sender).unwrap(), LaneService::Sent) {
        let packet = receiver.recv().unwrap();
        if let LaneMessageV1::TimeoutVote(vote) = packet.envelope.message { exact_timeout = Some(vote); }
    }
    let signed = exact_timeout.expect("timeout signing remains serviceable while the exact body adapter is missing");
    assert_eq!(signed.body.highest_prepare, Some(prepare));
    assert!(owner.held_effects().any(|effect| matches!(effect, core::Effect::FetchBody { .. })));
    assert!(!owner.held_effects().any(|effect| matches!(effect, core::Effect::ValidateBody { .. } | core::Effect::Apply { .. })));
    assert!(matches!(owner.service_with_worker(state, &observed, due).unwrap(), LaneService::NeedsBodyAdapter));
    assert!(owner.body_store().read_for_manifest(&manifest).unwrap().is_none());
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_frozen_key_and_process_output_gate_are_fail_closed
    use crate::sumeragi::{output_guard::ConsensusOutputGuard, v2_core as core,
        v2_lane_instance::{LaneInputOutcome, LaneInstance}};
    use std::time::{Duration, Instant};
    let fixture = finalized_lane_wal_fixture();
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let now = Instant::now();
    assert!(LaneInstance::open_with_worker_for_test(state, &observed, lane, KeyPair::random(), ConsensusOutputGuard::isolated(), now,
        Duration::from_secs(1), Duration::from_millis(100), 3 * core::MAX_EFFECTS_PER_STEP).is_err());
    let key = fixture.validators.iter().find(|key| key.public_key() == lane.frozen().committee[0].public_key()).unwrap().clone();
    assert!(LaneInstance::open_with_worker_for_test(state, &observed, lane, key.clone(), ConsensusOutputGuard::isolated(), now,
        Duration::ZERO, Duration::from_millis(100), 3 * core::MAX_EFFECTS_PER_STEP).is_err());
    assert!(LaneInstance::open_with_worker_for_test(state, &observed, lane, key.clone(), ConsensusOutputGuard::isolated(), now,
        Duration::from_secs(1), Duration::from_millis(100), 3 * core::MAX_EFFECTS_PER_STEP - 1).is_err());
    assert!(LaneInstance::open_with_worker_for_test(state, &observed, lane, key.clone(), ConsensusOutputGuard::isolated(), now,
        Duration::MAX, Duration::from_millis(100), 3 * core::MAX_EFFECTS_PER_STEP).is_err(), "configured timeout overflow fails before physical open");
    assert!(LaneInstance::open_with_worker_for_test(state, &observed, lane, key.clone(), ConsensusOutputGuard::isolated(), now,
        Duration::from_secs(1), Duration::MAX, 3 * core::MAX_EFFECTS_PER_STEP).is_err(), "configured retransmit overflow fails before physical open");
    let guard = ConsensusOutputGuard::isolated();
    let mut owner = LaneInstance::open_with_worker_for_test(state, &observed, lane, key, Arc::clone(&guard), now,
        Duration::from_secs(1), Duration::from_millis(100), 3 * core::MAX_EFFECTS_PER_STEP).unwrap();
    assert!(matches!(owner.poll_clock(state, &observed, now).unwrap(), LaneInputOutcome::NotDue));
    let deadline = owner.timeout_deadline().unwrap();
    let tag = owner.tag();
    guard.close_admission_for_restart();
    assert!(owner.poll_clock(state, &observed, deadline).is_err());
    assert_eq!(owner.tag(), tag);
    assert_eq!(owner.timeout_deadline(), Some(deadline));
    assert!(owner.native_records().is_empty());
    assert!(owner.held_effects().next().is_none());
}
