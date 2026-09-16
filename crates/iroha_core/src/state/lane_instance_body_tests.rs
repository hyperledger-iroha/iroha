// Real issued body work around the same native reducer; no runner/P2P/economic Apply claim.

fn service_native_lane_control_for_body_test(
    owner: &mut crate::sumeragi::v2_lane_instance::LaneInstance,
    state: &State,
    observed: &VerifiedLaneContexts,
    now: std::time::Instant,
) {
    use crate::sumeragi::v2_lane_instance::LaneService;
    for _ in 0..32 {
        match owner.service_one(state, observed, now).unwrap() {
            LaneService::PersistedAwaitingAck
            | LaneService::SignedAwaitingAck
            | LaneService::Completion(_)
            | LaneService::EnteredView(_) => {}
            LaneService::Idle | LaneService::NeedsBodyAdapter => return,
            other => panic!("unexpected control gate: {other:?}"),
        }
    }
    panic!("bounded native control pump did not quiesce");
}

fn run_native_lane_body_worker_for_test(
    job: crate::sumeragi::v2_lane_instance::LaneBodyJob,
    state: &State,
) -> crate::sumeragi::v2_lane_instance::LaneBodyCompletion {
    std::thread::scope(|scope| scope.spawn(move || job.run(state)).join().unwrap())
}

fn run_one_native_lane_body_job_for_test(
    owner: &mut crate::sumeragi::v2_lane_instance::LaneInstance,
    state: &State,
    observed: &VerifiedLaneContexts,
) -> crate::sumeragi::v2_lane_instance::LaneBodyProgress {
    use crate::sumeragi::v2_lane_instance::{LaneBodyLaunch, LaneBodyProgress};
    match owner.take_body_job(state, observed).unwrap() {
        LaneBodyLaunch::Job(job) => owner
            .finish_body_job(
                run_native_lane_body_worker_for_test(job, state),
                state,
                observed,
            )
            .unwrap(),
        LaneBodyLaunch::Wait(wait) => LaneBodyProgress::Waiting(wait),
    }
}

fn drain_native_lane_messages_for_body_test(
    owner: &mut crate::sumeragi::v2_lane_instance::LaneInstance,
    state: &State,
    observed: &VerifiedLaneContexts,
) -> Vec<iroha_data_model::block::lane_consensus::LaneMessageV1> {
    use crate::sumeragi::v2_lane_instance::LaneService;
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    let mut messages = Vec::new();
    for _ in 0..32 {
        match owner.flush_one(state, observed, &sender).unwrap() {
            LaneService::Sent => {
                let packet = receiver.recv().unwrap();
                assert_eq!(
                    packet.canonical_bytes,
                    norito::encode_canonical(&packet.envelope).unwrap()
                );
                messages.push(packet.envelope.message);
            }
            LaneService::Idle | LaneService::NeedsBodyAdapter => return messages,
            other => panic!("unexpected output gate: {other:?}"),
        }
    }
    panic!("bounded native output pump did not quiesce");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_silent_initial_leader_reaches_decision_with_real_body_receipts
    use crate::sumeragi::{v2_core as core, v2_lane_instance::{LaneInputOutcome, LaneBodyProgress}, v2_lane_wire::{LaneAuthenticator, LaneWalRecordV1}};
    use iroha_data_model::block::lane_consensus::{LaneMessageV1, LanePhaseV1};
    use std::{collections::VecDeque, time::{Duration, Instant}};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let context = lane.reducer_context();
    let silent = context.leader(0);
    let survivors = context.roster().iter().enumerate().filter_map(|(index, validator)|
        (validator.id() != silent).then_some(index)).collect::<Vec<_>>();
    assert_eq!(survivors.len(), 3);
    let start = Instant::now();
    let due = start + Duration::from_secs(1);
    let mut owners = survivors.iter().map(|&signer| open_lane_instance_for_test(&fixture, &observed, lane, signer, start)).collect::<Vec<_>>();
    let mut packets = VecDeque::new();
    for (sender, owner) in owners.iter_mut().enumerate() {
        assert!(owner.native_records().is_empty());
        assert!(matches!(owner.poll_clock(state, &observed, due).unwrap(), LaneInputOutcome::Stepped(_)));
        service_native_lane_control_for_body_test(owner, state, &observed, due);
        assert!(matches!(&owner.native_records()[0].record, LaneWalRecordV1::TimeoutIntent {body,..} if body.highest_prepare.is_none()));
        for message in drain_native_lane_messages_for_body_test(owner, state, &observed) {
            for recipient in 0..3 {
                if recipient != sender { packets.push_back((recipient, message.clone())); }
            }
        }
    }
    // Each packet remains owned in this bounded fixture until the instance accepts
    // it. These are real native signatures transported through actual outboxes.
    for _ in 0..32 {
        while let Some((recipient, message)) = packets.pop_front() {
            match owners[recipient].offer(state, &observed, &message).unwrap() {
                LaneInputOutcome::Backpressured => {
                    service_native_lane_control_for_body_test(&mut owners[recipient], state, &observed, due);
                    packets.push_front((recipient, message));
                },
                LaneInputOutcome::Stepped(_) => {},
                other => panic!("unexpected timeout input: {other:?}"),
            }
        }
        for (sender, owner) in owners.iter_mut().enumerate() {
            service_native_lane_control_for_body_test(owner, state, &observed, due);
            for message in drain_native_lane_messages_for_body_test(owner, state, &observed) {
                for recipient in 0..3 { if recipient != sender { packets.push_back((recipient, message.clone())); } }
            }
        }
        if owners.iter().all(|owner| owner.tag().view() == 1) && packets.is_empty() { break; }
    }
    assert!(owners.iter().all(|owner| owner.tag().view() == 1));
    assert!(owners.iter().all(|owner| owner.native_records().iter().all(|record| matches!(record.record,
        LaneWalRecordV1::TimeoutIntent {..} | LaneWalRecordV1::InstallTimeout(_)))));
    assert_ne!(context.leader(1), silent);
    let now = due + Duration::from_millis(100);
    let mut saw_proposal = false;
    let mut saw_prepare = false;
    let mut saw_commit = false;
    for tick in 0..80 {
        let now = now + Duration::from_millis(tick * 10);
        for (sender, owner) in owners.iter_mut().enumerate() {
            service_native_lane_control_for_body_test(owner, state, &observed, now);
            let _ = owner.service_body_completion(state, &observed).unwrap();
            match run_one_native_lane_body_job_for_test(owner, state, &observed) {
                LaneBodyProgress::Stepped(_) | LaneBodyProgress::SignReady | LaneBodyProgress::Waiting(_) => {},
                other => panic!("unexpected body result {other:?}"),
            }
            service_native_lane_control_for_body_test(owner, state, &observed, now);
            // The real shared fallback activates the Set B survivor without
            // fabricating body readiness or changing the committee geometry.
            owner.poll_clock(state, &observed, now).unwrap();
            service_native_lane_control_for_body_test(owner, state, &observed, now);
            for message in drain_native_lane_messages_for_body_test(owner, state, &observed) {
                match &message {
                    LaneMessageV1::Proposal(proposal) => { saw_proposal = true; assert_eq!(proposal.body.round.voting_view, 1); },
                    LaneMessageV1::Vote(vote) => match vote.statement.phase { LanePhaseV1::Prepare => saw_prepare = true, LanePhaseV1::Commit => saw_commit = true },
                    _ => {},
                }
                for recipient in 0..3 { if recipient != sender { packets.push_back((recipient, message.clone())); } }
            }
        }
        let count = packets.len();
        for _ in 0..count {
            let (recipient, message) = packets.pop_front().unwrap();
            match owners[recipient].offer(state, &observed, &message).unwrap() {
                LaneInputOutcome::Backpressured => packets.push_back((recipient, message)),
                LaneInputOutcome::BodyPreparationQueued | LaneInputOutcome::Stepped(_) => {},
                other => panic!("unexpected native body input: {other:?}"),
            }
        }
        if owners.iter().all(|owner| owner.native_decision().unwrap().is_some() && owner.held_effects().any(|effect| matches!(effect, core::Effect::Apply {..}))) { break; }
    }
    assert!(saw_proposal && saw_prepare && saw_commit);
    let mut exact_subject = None;
    for owner in &owners {
        let decision = owner.native_decision().unwrap().expect("same physical owner reaches native Decision");
        assert_eq!(decision.commit_qc.shares.len(), 3);
        assert_eq!(decision.commit_qc.statement.round.voting_view, 1);
        assert_eq!(decision.manifest.value.origin_view, 1);
        let qc = LaneAuthenticator::new(lane).decision_certificate(&decision).unwrap();
        assert!(exact_subject.replace(qc.subject()).is_none_or(|old| old == qc.subject()));
        assert_eq!(owner.durable_decision_certificate(), Some(&decision.commit_qc));
        assert!(owner.held_effects().any(|effect| matches!(effect, core::Effect::Apply {subject,..} if *subject == qc.subject())),
            "Decision is exposed immediately; group/global execution never acknowledges this Apply in the instance fixture");
        assert_eq!(owner.body_state_for_test(qc.proposal_round(), qc.subject()), core::BodyState::Validated);
    }
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_set_b_retains_native_candidate_without_effect_until_fallback
    use crate::sumeragi::{v2_core as core, v2_lane_instance::{LaneInputOutcome, LaneBodyProgress}, v2_lane_wire::LaneWalRecordV1};
    use iroha_data_model::block::lane_consensus::{LaneMessageV1, LanePhaseV1};
    use std::time::{Duration, Instant};
    let fixture = all_route_input_fixture(false);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let roles = core::Committee::project(lane.reducer_context(), 0).unwrap();
    let now = Instant::now();
    let leader_index = lane.reducer_context().roster().iter().position(|v| v.id() == lane.reducer_context().leader(0)).unwrap();
    let mut leader = open_lane_instance_for_test(&fixture, &observed, lane, leader_index, now);
    assert!(matches!(run_one_native_lane_body_job_for_test(&mut leader, state, &observed), LaneBodyProgress::Stepped(_)));
    service_native_lane_control_for_body_test(&mut leader, state, &observed, now);
    let proposal = drain_native_lane_messages_for_body_test(&mut leader, state, &observed).into_iter().find_map(|message| match message {
        LaneMessageV1::Proposal(proposal) => Some(proposal), _ => None,
    }).expect("actual native proposal from fsynced input and ProposalIntent");
    let subject = core::Subject::new(proposal.body.manifest.value.subject_hash().unwrap().into());
    let round = core::Round::new(lane.frozen().next_lane_height, 0);
    let mut member = open_lane_instance_for_test(&fixture, &observed, lane, roles.set_b()[0] as usize, now);
    assert!(matches!(member.offer(state, &observed, &LaneMessageV1::Proposal(proposal)).unwrap(), LaneInputOutcome::BodyPreparationQueued));
    assert!(matches!(run_one_native_lane_body_job_for_test(&mut member, state, &observed), LaneBodyProgress::Stepped(_)));
    assert_eq!(member.body_state_for_test(round, subject), core::BodyState::Missing);
    assert!(member.held_effects().next().is_none(), "Set B has no issued Fetch while its candidate stays reducer-owned");
    assert!(member.native_records().is_empty(), "no prematurely invented Prepare intent");
    assert!(matches!(member.poll_clock(state, &observed, now + Duration::from_millis(100)).unwrap(), LaneInputOutcome::Stepped(_)));
    for _ in 0..3 { // The shared fallback actually issues Fetch -> Store -> Validate.
        assert!(matches!(run_one_native_lane_body_job_for_test(&mut member, state, &observed), LaneBodyProgress::Stepped(_)));
    }
    assert_eq!(member.body_state_for_test(round, subject), core::BodyState::Validated);
    service_native_lane_control_for_body_test(&mut member, state, &observed, now + Duration::from_millis(100));
    assert!(member.native_records().iter().any(|record| matches!(record.record, LaneWalRecordV1::PrepareIntent {..})),
        "native witness survived the no-issued-effect interval and the actual fallback authored a durable vote");
    assert!(drain_native_lane_messages_for_body_test(&mut member, state, &observed).iter().any(|message| matches!(message,
        LaneMessageV1::Vote(vote) if vote.statement.phase == LanePhaseV1::Prepare && core::Subject::new(vote.statement.value.subject_hash().unwrap().into()) == subject)));
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_commit_qc_progresses_while_actual_body_job_is_in_flight
    use crate::sumeragi::{v2_core as core, v2_lane_instance::{LaneBodyLaunch,LaneBodyProgress,LaneBodyWait,LaneInputOutcome,LaneService}, v2_lane_payload::encode_lane_input};
    use iroha_data_model::block::lane_consensus::{LaneMessageV1,LaneQcV1,LaneRoundV1,LanePhaseV1,LaneVoteStatementV1,LaneSignatureShareV1};
    use std::time::Instant;
    let (fixture, _) = first_lane_input_fixture(0xA4);
    let state = &fixture.state;
    let observed = state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane = &observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source) = state.first_lane_admitted_input(&observed,lane).unwrap() else { panic!("exact source") };
    let LaneInputBodyPreparationV1::Ready(body) = state.prepare_lane_input_body(&observed,lane,&source).unwrap() else { panic!("exact body") };
    let manifest = *encode_lane_input(lane,&body,0).unwrap().manifest();
    let statement = LaneVoteStatementV1 { round:LaneRoundV1 { instance_id:manifest.value.instance_id,lane_height:lane.frozen().next_lane_height,voting_view:0 }, phase:LanePhaseV1::Commit,value:manifest.value };
    let qc = LaneQcV1 { statement, shares:(0..3).map(|signer| {
        let key = fixture.validators.iter().find(|key| key.public_key()==lane.frozen().committee[signer].public_key()).unwrap();
        LaneSignatureShareV1 { signer:signer as u32, signature:Signature::try_new(key.private_key(),&statement.signature_preimage().unwrap()).unwrap().payload().to_vec() }
    }).collect() };
    let now = Instant::now();
    let mut owner = open_lane_instance_for_test(&fixture,&observed,lane,manifest.value.origin_producer as usize,now);
    let LaneBodyLaunch::Job(job) = owner.take_body_job(state,&observed).unwrap() else { panic!("leader owns real local input job") };
    let completed = run_native_lane_body_worker_for_test(job,state); // Actual fsync/readback done, result deliberately held.
    assert!(owner.native_records().is_empty());
    assert!(matches!(owner.offer(state,&observed,&LaneMessageV1::QuorumCertificate(qc.clone())).unwrap(),LaneInputOutcome::Stepped(_)));
    assert!(matches!(owner.service_one(state,&observed,now).unwrap(),LaneService::PersistedAwaitingAck));
    assert_eq!(owner.durable_decision_certificate(),Some(&qc),"native Decision custody is visible before the worker completion");
    assert!(owner.native_decision().unwrap().is_none(),"a QC alone cannot manufacture the missing manifest");
    assert!(!owner.held_effects().any(|effect| matches!(effect,core::Effect::Apply {..})),"no body Ready was inferred from CommitQC");
    assert!(matches!(owner.finish_body_job(completed,state,&observed).unwrap(),LaneBodyProgress::Waiting(LaneBodyWait::ControlCompletion)));
    let read=owner.body_store().read_for_manifest(&manifest).unwrap().unwrap();
    assert_eq!(read.canonical_bytes(),body.canonical_bytes(),"physical custody returns before the blocked reducer acknowledgement");
    service_native_lane_control_for_body_test(&mut owner,state,&observed,now);
    assert!(matches!(owner.service_body_completion(state,&observed).unwrap(),LaneBodyProgress::Stepped(_)));
    let decision=owner.native_decision().unwrap().unwrap();
    assert_eq!(decision.commit_qc,qc);
    assert_eq!(decision.manifest,manifest);
    assert!(owner.held_effects().any(|effect| matches!(effect,core::Effect::Apply {..})),"the real returned durable/validated local body can now satisfy the already-decided body");
    assert_eq!(owner.body_state_for_test(core::Round::new(lane.frozen().next_lane_height,0),core::Subject::new(manifest.value.subject_hash().unwrap().into())),core::BodyState::Validated);
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_closed_worker_completion_restores_store_without_productive_ack
    use crate::sumeragi::{v2_lane_instance::{LaneBodyLaunch,LaneBodyProgress,LaneCurrentGate,LaneInputOutcome},v2_lane_payload::encode_lane_input};
    use std::time::Instant;
    let (fixture,_) = first_lane_input_fixture(0xA5);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let FirstLaneAdmittedInputReadV1::Ready(source)=state.first_lane_admitted_input(&observed,lane).unwrap() else { panic!("first source") };
    let LaneInputBodyPreparationV1::Ready(body)=state.prepare_lane_input_body(&observed,lane,&source).unwrap() else { panic!("all routes") };
    let manifest=*encode_lane_input(lane,&body,0).unwrap().manifest();
    let now=Instant::now();
    let mut owner=open_lane_instance_for_test(&fixture,&observed,lane,manifest.value.origin_producer as usize,now);
    let tag=owner.tag();
    let LaneBodyLaunch::Job(job)=owner.take_body_job(state,&observed).unwrap() else { panic!("local real worker") };
    let completed=run_native_lane_body_worker_for_test(job,state); // The real physical write survives authenticated closure.
    let parent=state.kura.v2_finality_artifact(fixture.block.header().height().get()).unwrap().unwrap();
    let opening=crate::sumeragi::v2_context::build_successor_height_context(&parent,parent.height_context.nexus_amx_context_hash,None).unwrap();
    let block=empty_global_block_after(Some(&fixture.block));
    let mut overlay=state.block(block.header());
    assert!(State::resolve_queue_plan_pending_obligation_in_storage(&mut overlay.world.smart_contract_state,fixture.binding.network_id_digest,fixture.binding.entrypoint_hash).unwrap());
    overlay.finalize_lane_consensus_contexts(&block,Some(&opening)).unwrap();
    let mut witness=ExecWitness::default();
    overlay.capture_lane_consensus_contexts(&mut witness).unwrap();
    overlay.block_hashes.push(block.hash());
    insert_empty_transaction_block_for_state_commit(&mut overlay,&block);
    overlay.commit().unwrap();
    state.kura.store_block(Arc::new(block.clone())).unwrap();
    let (artifact,receipt)=stage_lane_context_fixture_finality(state,&block,opening,witness);
    state.kura.promote_kagemusha_finality_sidecar(&artifact,&receipt).unwrap();
    let closed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert!(closed.contexts().is_empty());
    assert!(matches!(owner.finish_body_job(completed,state,&closed).unwrap(),LaneBodyProgress::Retired {effect:None,proposal:None}));
    assert_eq!(owner.tag(),tag);
    assert!(owner.native_records().is_empty());
    assert!(owner.held_effects().next().is_none(),"returning physical custody did not synthesize ProposalIntent/Ready/Apply");
    let read=owner.body_store().read_for_manifest(&manifest).unwrap().unwrap();
    assert_eq!(read.canonical_bytes(),body.canonical_bytes());
    owner.body_store().validate_receipt(read.receipt()).unwrap();
    assert!(matches!(owner.poll_clock(state,&closed,owner.timeout_deadline().unwrap()).unwrap(),LaneInputOutcome::Gate(LaneCurrentGate::InstanceClosed)));
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_rejects_signed_wrong_input_before_body_admission_and_keeps_timer
    use crate::sumeragi::{v2_lane_instance::{LaneBodyProgress,LaneInputOutcome},v2_lane_wire::LaneWalRecordV1};
    use iroha_data_model::block::lane_consensus::LaneMessageV1;
    use std::time::{Duration,Instant};
    let (fixture,_) = first_lane_input_fixture(0xA6);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let leader_index=lane.reducer_context().roster().iter().position(|v| v.id()==lane.reducer_context().leader(0)).unwrap();
    let now=Instant::now();
    let mut leader=open_lane_instance_for_test(&fixture,&observed,lane,leader_index,now);
    assert!(matches!(run_one_native_lane_body_job_for_test(&mut leader,state,&observed),LaneBodyProgress::Stepped(_)));
    service_native_lane_control_for_body_test(&mut leader,state,&observed,now);
    let mut proposal=drain_native_lane_messages_for_body_test(&mut leader,state,&observed).into_iter().find_map(|message| match message { LaneMessageV1::Proposal(proposal)=>Some(proposal),_=>None }).unwrap();
    let exact_manifest=proposal.body.manifest;
    proposal.body.manifest.value.payload_hash=Hash::new(b"validly signed wrong immutable input");
    let key=fixture.validators.iter().find(|key| key.public_key()==lane.frozen().committee[leader_index].public_key()).unwrap();
    proposal.signature=Signature::try_new(key.private_key(),&proposal.body.signature_preimage().unwrap()).unwrap().payload().to_vec();
    let mut receiver=open_lane_instance_for_test(&fixture,&observed,lane,(leader_index+1)%4,now);
    assert!(matches!(receiver.offer(state,&observed,&LaneMessageV1::Proposal(proposal.clone())).unwrap(),LaneInputOutcome::BodyPreparationQueued));
    assert_eq!(receiver.deferred_proposal(),Some(&proposal));
    let LaneBodyProgress::RejectedProposal {proposal:rejected,..}=run_one_native_lane_body_job_for_test(&mut receiver,state,&observed) else { panic!("deterministic mismatch rejected before core body admission") };
    assert_eq!(rejected,proposal);
    assert!(receiver.deferred_proposal().is_none());
    assert!(receiver.native_records().is_empty());
    assert!(receiver.held_effects().next().is_none());
    assert!(receiver.body_store().read_for_manifest(&exact_manifest).unwrap().is_none());
    assert!(matches!(receiver.poll_clock(state,&observed,now+Duration::from_secs(1)).unwrap(),LaneInputOutcome::Stepped(_)));
    service_native_lane_control_for_body_test(&mut receiver,state,&observed,now+Duration::from_secs(1));
    assert!(matches!(&receiver.native_records()[0].record,LaneWalRecordV1::TimeoutIntent {body,..} if body.highest_prepare.is_none()));
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_missing_first_body_retains_global_recovery_and_timeout_custody
    use crate::sumeragi::{v2_chunks,v2_transport,v2_lane_instance::{LaneBodyProgress,LaneBodyWait,LaneInputOutcome},v2_lane_wire::LaneWalRecordV1};
    use iroha_data_model::block::consensus_v2 as wire;
    use std::time::{Duration,Instant};
    let (fixture,_) = first_lane_input_fixture(0xA7);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=&observed.contexts()[0];
    let leader_index=lane.reducer_context().roster().iter().position(|v| v.id()==lane.reducer_context().leader(0)).unwrap();
    let now=Instant::now();
    let mut owner=open_lane_instance_for_test(&fixture,&observed,lane,leader_index,now);
    state.kura.evict_first_admission_body_for_testing(std::num::NonZeroUsize::new(fixture.block.header().height().get() as usize).unwrap(),fixture.block.hash()).unwrap();
    assert!(matches!(run_one_native_lane_body_job_for_test(&mut owner,state,&observed),LaneBodyProgress::Waiting(LaneBodyWait::FirstCarrierRecovery)));
    let required=owner.source_recovery_requirement().unwrap().clone();
    assert_eq!(required.carrier_hash(),fixture.block.hash());
    assert_eq!(required.priority(),lane.frozen().admission_priority);
    assert!(owner.held_effects().next().is_none(),"first-source wait did not manufacture core body availability");
    assert!(matches!(owner.poll_clock(state,&observed,now+Duration::from_secs(1)).unwrap(),LaneInputOutcome::Stepped(_)));
    service_native_lane_control_for_body_test(&mut owner,state,&observed,now+Duration::from_secs(1));
    assert!(matches!(&owner.native_records()[0].record,LaneWalRecordV1::TimeoutIntent {body,..} if body.highest_prepare.is_none()));
    assert_eq!(owner.source_recovery_requirement().unwrap().carrier_hash(),required.carrier_hash(),"timeout service preserves the exact historical source owner");
    let finality=required.finality();
    let key=&fixture.validators[0];
    let peer=PeerId::new(key.public_key().clone());
    let mut request=wire::CertifiedBodyRequest {round:finality.commit_qc.proposal_round,subject:finality.subject,certificate:finality.commit_qc.clone(),requester:peer.clone(),signature:vec![]};
    request.signature=Signature::new(key.private_key(),&request.signature_preimage()).payload().to_vec();
    let request=v2_transport::authenticate_certified_body_request_with_validator_pops(&finality.height_context,&finality.validator_set_pops,request,&peer).unwrap();
    let bytes=fixture.block.canonical_resultless_proposal().encode_wire().unwrap();
    let (manifest,_)=v2_chunks::encode_payload(&finality.height_context,request.request().round,finality.subject,&bytes).unwrap().into_parts();
    let mut response=wire::CertifiedBodyResponse {request_hash:request.request_hash(),manifest,body:bytes,responder:peer.clone(),signature:vec![]};
    response.signature=Signature::new(key.private_key(),&response.signature_preimage()).payload().to_vec();
    let mut outstanding=v2_transport::OutstandingCertifiedBodyRequests::new(1).unwrap();
    outstanding.register(request.clone()).unwrap();
    let response=outstanding.authenticate_response(&finality.height_context,response,&peer).unwrap();
    owner.complete_source_recovery(&request,&response).unwrap();
    assert!(owner.source_recovery_requirement().is_none());
    assert_eq!(outstanding.len(),1,"source projection cannot acknowledge the transport owner");
    assert!(owner.durable_decision_certificate().is_none());
    assert!(!owner.held_effects().any(|effect| matches!(effect,crate::sumeragi::v2_core::Effect::StoreBody {..}|crate::sumeragi::v2_core::Effect::ValidateBody {..}|crate::sumeragi::v2_core::Effect::Apply {..})),"recovered resultless global bytes confer no executed-body or native Ready authority");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_earlier_route_dependency_does_not_block_pre_payload_timeout
    use crate::sumeragi::{v2_lane_instance::{LaneBodyProgress,LaneBodyWait,LaneInputOutcome},v2_lane_wire::LaneWalRecordV1};
    use std::time::{Instant,Duration};
    let fixture=all_route_input_fixture(true);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    let lane=observed.contexts().iter().find(|lane| lane.frozen().lane_id==LaneId::SINGLE).unwrap();
    let leader=lane.reducer_context().roster().iter().position(|v| v.id()==lane.reducer_context().leader(0)).unwrap();
    let now=Instant::now();
    let mut owner=open_lane_instance_for_test(&fixture,&observed,lane,leader,now);
    let LaneBodyProgress::Waiting(LaneBodyWait::EarlierHeads(dependencies))=run_one_native_lane_body_job_for_test(&mut owner,state,&observed) else { panic!("actual earlier secondary head must own the wait") };
    assert_eq!(dependencies.len(),1);
    assert_eq!(owner.blocked_route_dependencies(),dependencies);
    assert!(dependencies[0].priority<lane.frozen().admission_priority);
    assert_eq!(dependencies[0].route.0,LaneId::new(1));
    assert!(owner.native_records().is_empty());
    assert!(owner.held_effects().next().is_none());
    assert!(matches!(owner.poll_clock(state,&observed,now+Duration::from_secs(1)).unwrap(),LaneInputOutcome::Stepped(_)));
    service_native_lane_control_for_body_test(&mut owner,state,&observed,now+Duration::from_secs(1));
    assert!(matches!(&owner.native_records()[0].record,LaneWalRecordV1::TimeoutIntent {body,..} if body.highest_prepare.is_none()));
    assert!(owner.durable_decision_certificate().is_none());
    assert_eq!(owner.blocked_route_dependencies(),dependencies,"control service does not erase the named earlier-head dependency");
}

#[cfg(all(unix, not(target_os = "espidf")))]
state_test! { sync native_lane_instance_foreign_worker_completion_keeps_both_physical_owners
    use crate::sumeragi::v2_lane_instance::{LaneBodyLaunch,LaneBodyProgress};
    use std::time::Instant;
    let fixture=all_route_input_fixture(false);
    let state=&fixture.state;
    let observed=state.verified_lane_consensus_contexts().unwrap().unwrap();
    assert_eq!(observed.contexts().len(),2);
    let now=Instant::now();
    let open=|lane:&VerifiedLaneContext| {
        let leader=lane.reducer_context().roster().iter().position(|v|v.id()==lane.reducer_context().leader(0)).unwrap();
        open_lane_instance_for_test(&fixture,&observed,lane,leader,now)
    };
    let mut first=open(&observed.contexts()[0]);
    let mut second=open(&observed.contexts()[1]);
    let LaneBodyLaunch::Job(first_job)=first.take_body_job(state,&observed).unwrap() else {panic!("first exact job");};
    let LaneBodyLaunch::Job(second_job)=second.take_body_job(state,&observed).unwrap() else {panic!("second exact job");};
    let completion=run_native_lane_body_worker_for_test(first_job,state);
    let (_,completion)=second.finish_body_job(completion,state,&observed).unwrap_err();
    assert!(first.native_records().is_empty());
    assert!(second.native_records().is_empty());
    assert!(matches!(first.finish_body_job(completion,state,&observed).unwrap(),LaneBodyProgress::Stepped(_)),"foreign rejection returns the complete exact result to its true owner");
    let completion=run_native_lane_body_worker_for_test(second_job,state);
    assert!(matches!(second.finish_body_job(completion,state,&observed).unwrap(),LaneBodyProgress::Stepped(_)),"the receiving instance retained its own original job/handle");
    service_native_lane_control_for_body_test(&mut first,state,&observed,now);
    service_native_lane_control_for_body_test(&mut second,state,&observed,now);
    assert!(!first.native_records().is_empty());
    assert!(!second.native_records().is_empty());
}
