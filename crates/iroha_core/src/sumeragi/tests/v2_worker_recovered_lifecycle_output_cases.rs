#[test]
fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let fanout = || {
        PendingExactFanout::new(
            vec![lane_commit_qc_message(peer.clone())],
            vec![peer.clone()],
        )
        .expect("one exact topology fanout")
    };
    let mut tight = PendingExactOutput::new(1, 1, 1, &[])
        .expect("one individual fanout fits the exact corridor");
    assert!(
        tight
            .prepare_atomic_fanout_batch(vec![fanout()])
            .expect("one fanout is structurally exact")
            .is_some(),
        "each child fits independently"
    );
    tight.next_fanout_fifo_id = ExactFanoutFifoId::MAX;
    assert!(
        tight
            .prepare_atomic_fanout_batch(vec![fanout(), fanout()])
            .expect("the pair is structurally exact")
            .is_none(),
        "aggregate demand must be checked before admitting either child"
    );
    assert!(tight.fanouts.is_empty());
    assert!(tight.source_fifo_owners.is_empty());
    assert!(tight.reservation_owner_counts.is_empty());
    assert_eq!(tight.next_fanout_fifo_id, ExactFanoutFifoId::MAX);
    let mut roomy =
        PendingExactOutput::new(2, 1, 1, &[]).expect("the exact pair fits the aggregate corridor");
    roomy.next_fanout_fifo_id = ExactFanoutFifoId::MAX;
    let plan = roomy
        .prepare_atomic_fanout_batch(vec![fanout(), fanout()])
        .expect("prepare the exact pair")
        .expect("aggregate capacity retains both children");
    assert!(
        roomy.fanouts.is_empty(),
        "preflight cannot publish the pair"
    );
    assert_eq!(roomy.next_fanout_fifo_id, ExactFanoutFifoId::MAX);
    roomy.commit_atomic_fanout_batch(plan);
    assert_eq!(roomy.fanouts.len(), 2);
    assert_eq!(roomy.fanouts[0].fifo_id, Some(0));
    assert_eq!(roomy.fanouts[1].fifo_id, Some(1));
    assert_eq!(roomy.next_fanout_fifo_id, 2);
    assert_eq!(roomy.ownership_units, 2);
    assert_eq!(roomy.shared_ownership_units, 2);
}
#[test]
fn armed_recovered_proposal_output_reservation_fails_stop_on_drop() {
    use super::super::v2_lifecycle_coordinator::{
        RecoveredLifecycleSignClassV1, RecoveredLifecycleSignDispatchIdentityV1,
    };
    let (mut service, keys) = fixture_with_block_payload();
    let (_, payload, mut proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = service.active_tag;
    let request = super::super::v2::SignRequest::Proposal(proposal.clone());
    let dispatch_key = RecoveredLifecycleSignDispatchIdentityV1::for_test(
        92,
        tag,
        &request,
        RecoveredLifecycleSignClassV1::ControlProposal,
    )
    .expect("mint exact recovered Proposal dispatch identity")
    .key();
    let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
    proposal.signature =
        Signature::new(keys[proposer].private_key(), &proposal.signature_preimage())
            .payload()
            .to_vec();
    set_local_validator(&mut service, &keys, proposal.proposer);
    let directory = TempDir::new().expect("temporary armed Proposal output store");
    let identity = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open armed Proposal output store")
        .instance_identity();
    let wal_append = RecoveredLifecycleProposalPrepareWalAppendSealV1 {
        dispatch_key,
        body_store_identity: identity.clone(),
        output_guard: Arc::clone(&service.output_guard),
        attempted: false,
    };
    let authority = super::super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1::for_test(
        &service.context,
        dispatch_key,
        tag,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal)),
        payload,
        identity,
        Arc::clone(&service.output_guard),
    )
    .expect("mint exact armed Proposal authority");
    let operation = service
        .output_guard
        .begin_fail_stop_operation()
        .expect("arm the exact Proposal output cut");
    let pending = service
        .lock_pending_exact_output()
        .expect("retain the exact Proposal corridor mutex");
    drop(RecoveredLifecycleProposalExactOutputReservationV1 {
        operation: Some(operation),
        pending: Some(pending),
        batch: None,
        authority: Some(authority),
        wal_append,
    });
    assert!(
        service.output_guard.restart_required(),
        "dropping an armed Proposal reservation must close process output"
    );
}
#[test]
fn durable_recovered_broadcast_capture_owns_and_retries_one_exact_fanout() {
    let (mut service, _) = fixture();
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let vote = routing_vote(&service, 0, wire::GlobalPhase::Commit);
    let authority = super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignedBroadcastOutputAuthorityV1::for_test(
        &service.context,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote)),
    );
    let RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(output) = service
        .capture_recovered_lifecycle_signed_broadcast_refanout(authority)
        .expect("exact durable Broadcast authority enters the service cut")
    else {
        panic!("an empty exact-output corridor must reserve the durable Broadcast")
    };
    output.commit_after_publication();
    assert!(
        service
            .has_pending_exact_output()
            .expect("inspect the retained recovered Broadcast fanout")
    );
    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("the exact-output owner retries the durable Broadcast")
    );
    assert!(
        !service
            .has_pending_exact_output()
            .expect("the admitted recovered Broadcast leaves no pending suffix")
    );
    assert!(!service.output_guard.restart_required());
}
#[test]
fn recovered_lifecycle_signing_is_exact_and_class_sensitive_for_all_three_families() {
    use super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1;
    let phase_key = RecoveredLifecycleSignDispatchKeyV1::for_test(
        7,
        9,
        RecoveredLifecycleSignClassV1::PhaseVote,
    );
    let proposal_key = RecoveredLifecycleSignDispatchKeyV1::for_test(
        7,
        9,
        RecoveredLifecycleSignClassV1::ControlProposal,
    );
    let timeout_key = RecoveredLifecycleSignDispatchKeyV1::for_test(
        7,
        9,
        RecoveredLifecycleSignClassV1::ControlTimeout,
    );
    assert_ne!(phase_key, proposal_key);
    assert_ne!(phase_key, timeout_key);
    assert_ne!(proposal_key, timeout_key);
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let directory = TempDir::new().expect("temporary recovered Sign body store");
    let mut body_store = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open recovered Sign body store");
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let _ = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("store exact recovered Proposal body");
    let proposal_tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(1),
    );
    let proposal_signer =
        usize::try_from(proposal.proposer).expect("fixture proposer index is representable");
    let proposal_result = sign_recovered_lifecycle_task(
        &body_store,
        &service.context,
        &keys[proposal_signer],
        RecoveredLifecycleSignTaskV1::for_test(
            11,
            proposal_tag,
            super::super::v2::SignRequest::Proposal(proposal.clone()),
            RecoveredLifecycleSignClassV1::ControlProposal,
        ),
    )
    .expect("sign exact recovered Proposal");
    assert!(proposal_result.is_exact());
    assert_eq!(
        proposal_result
            .outbound_payload
            .as_ref()
            .expect("Proposal restores its exact outbound body"),
        &payload
    );
    let mut vote = routing_vote(&service, proposal.round.view, wire::GlobalPhase::Prepare);
    vote.signature.clear();
    let vote_request = super::super::v2::SignRequest::Vote(vote);
    let vote_result = sign_recovered_lifecycle_task(
        &body_store,
        &service.context,
        &keys[usize::try_from(service.local_validator.expect("local voter"))
            .expect("local voter index")],
        RecoveredLifecycleSignTaskV1::for_test(
            11,
            proposal_tag,
            vote_request.clone(),
            RecoveredLifecycleSignClassV1::PhaseVote,
        ),
    )
    .expect("sign exact recovered phase vote");
    assert!(vote_result.is_exact());
    assert!(vote_result.outbound_payload.is_none());
    assert_eq!(
        vote_result.task.prepared_candidate,
        Some(PreparedCandidateBody {
            tag: proposal_tag,
            subject: match &vote_request {
                super::super::v2::SignRequest::Vote(vote) => vote.subject,
                _ => unreachable!("fixture retains one Prepare vote"),
            },
        }),
        "opaque PhaseVote task retains its future Prepare-body successor marker"
    );
    let timeout = wire::TimeoutVote {
        round: proposal.round,
        highest_prepare_qc: None,
        signer: service.local_validator.expect("local timeout voter"),
        signature: Vec::new(),
    };
    let timeout_request = super::super::v2::SignRequest::TimeoutVote(timeout);
    let timeout_result = sign_recovered_lifecycle_task(
        &body_store,
        &service.context,
        &keys[usize::try_from(service.local_validator.expect("local voter"))
            .expect("local voter index")],
        RecoveredLifecycleSignTaskV1::for_test(
            11,
            proposal_tag,
            timeout_request.clone(),
            RecoveredLifecycleSignClassV1::ControlTimeout,
        ),
    )
    .expect("sign exact recovered timeout vote");
    assert!(timeout_result.is_exact());
    assert!(timeout_result.outbound_payload.is_none());
    assert_ne!(proposal_result.dispatch_key(), vote_result.dispatch_key());
    assert_ne!(vote_result.dispatch_key(), timeout_result.dispatch_key());
    assert!(
        RecoveredLifecycleSignDispatchIdentityV1::for_test(
            11,
            proposal_tag,
            &vote_request,
            RecoveredLifecycleSignClassV1::ControlProposal,
        )
        .is_none(),
        "a phase vote cannot alias the Proposal key class"
    );
    assert!(
        RecoveredLifecycleSignDispatchIdentityV1::for_test(
            11,
            proposal_tag,
            &timeout_request,
            RecoveredLifecycleSignClassV1::PhaseVote,
        )
        .is_none(),
        "a timeout vote cannot alias the PhaseVote key class"
    );
    let changed_tag = EventTag::new(
        proposal_tag.height(),
        proposal_tag.view(),
        Generation::new(proposal_tag.generation().get() + 1),
    );
    let identity = RecoveredLifecycleSignDispatchIdentityV1::for_test(
        12,
        proposal_tag,
        &vote_request,
        RecoveredLifecycleSignClassV1::PhaseVote,
    )
    .expect("mint exact vote identity");
    assert!(
        RecoveredLifecycleSignTaskV1::from_registry_projection(
            identity,
            changed_tag,
            vote_request,
        )
        .is_none(),
        "carrier-to-task projection pins exact tag and request transitively"
    );
    let mut historical_commit = routing_vote(&service, 0, wire::GlobalPhase::Commit);
    historical_commit.signature.clear();
    let historical_request = super::super::v2::SignRequest::Vote(historical_commit);
    let later_tag = EventTag::new(
        service.context.height,
        3,
        Generation::new(proposal_tag.generation().get() + 5),
    );
    let historical_identity = RecoveredLifecycleSignDispatchIdentityV1::for_test(
        13,
        later_tag,
        &historical_request,
        RecoveredLifecycleSignClassV1::PhaseVote,
    )
    .expect("historical Commit request remains exact under its later retained tag");
    assert!(
        RecoveredLifecycleSignTaskV1::from_registry_projection(
            historical_identity,
            later_tag,
            historical_request.clone(),
        )
        .is_some(),
        "PhaseVote exactness must not invent tag-view equality with the intrinsic vote round"
    );
    let changed_later_tag = EventTag::new(
        later_tag.height(),
        later_tag.view(),
        Generation::new(later_tag.generation().get() + 1),
    );
    let historical_identity = RecoveredLifecycleSignDispatchIdentityV1::for_test(
        14,
        later_tag,
        &historical_request,
        RecoveredLifecycleSignClassV1::PhaseVote,
    )
    .expect("mint the unchanged historical Commit identity");
    assert!(
        RecoveredLifecycleSignTaskV1::from_registry_projection(
            historical_identity,
            changed_later_tag,
            historical_request,
        )
        .is_none(),
        "changing the retained tag must still change the complete effect identity"
    );
}
#[test]
fn recovered_lifecycle_sign_queue_retains_exact_owner_through_opaque_extraction() {
    use super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1;
    let (mut service, keys) = fixture();
    let directory = TempDir::new().expect("temporary recovered Sign body store");
    let body_store = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open recovered Sign body store");
    let tag = EventTag::new(service.context.height, 0, Generation::new(1));
    let mut vote = routing_vote(&service, 0, wire::GlobalPhase::Prepare);
    vote.signature.clear();
    let task = RecoveredLifecycleSignTaskV1::for_test(
        31,
        tag,
        super::super::v2::SignRequest::Vote(vote),
        RecoveredLifecycleSignClassV1::PhaseVote,
    );
    let key = task.dispatch_key();
    let admission = Arc::new(V2IoAdmission::new(2, 2).expect("bounded Sign admission"));
    let (command_tx, command_rx) = v2_io_command_channel(2, 1, 1, 1, Arc::clone(&admission));
    let output_guard = ConsensusOutputGuard::isolated();
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("reserve under an open output guard");
    let RecoveredLifecycleSignCapacityCaptureV1::Reserved(reservation) = command_tx
        .queue
        .capture_recovered_lifecycle_sign_capacity(operation, key)
        .expect("capture one dedicated recovered Sign position")
    else {
        panic!("empty Consensus lane must reserve recovered Sign capacity");
    };
    reservation.commit_for_test(task);
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_lifecycle_signs
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::Queued)
    );
    let task = match command_rx
        .try_recv()
        .expect("activate the exact recovered Sign command")
    {
        V2IoCommand::RecoveredLifecycleSign(task) => task,
        _ => panic!("dedicated reservation published another command family"),
    };
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_lifecycle_signs
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::Active)
    );
    let result = sign_recovered_lifecycle_task(
        &body_store,
        &service.context,
        &keys[usize::try_from(service.local_validator.expect("local voter"))
            .expect("local voter index")],
        task,
    )
    .expect("sign the exact recovered phase vote");
    command_rx
        .complete_recovered_lifecycle_sign(key, &result)
        .expect("seal the exact worker result under its dedicated key");
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_lifecycle_signs
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::CompletionPending)
    );
    let (completion_tx, completion_rx) = mpsc::sync_channel(2);
    send_tracked_completion_with_lifecycle_ordinal(
        &completion_tx,
        admission.as_ref(),
        V2IoCompletion::RecoveredLifecycleSign(Box::new(
            GuardedRecoveredLifecycleSignWorkerResultV1::new(result, Arc::clone(&output_guard)),
        )),
        Some(key.lifecycle_ordinal()),
    )
    .expect("publish tracked recovered Sign completion");
    send_tracked_completion_with_lifecycle_ordinal(
        &completion_tx,
        admission.as_ref(),
        V2IoCompletion::AuxiliaryNoop,
        Some(key.lifecycle_ordinal() + 1),
    )
    .expect("publish unrelated completion behind recovered Sign");
    service.output_guard = Arc::clone(&output_guard);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission: Arc::clone(&admission),
    });
    let generic = service.take_io_completion(true);
    assert!(generic.completion.is_none() && generic.retained_runtime);
    let retained = service
        .drain_recovered_lifecycle_sign_completion()
        .expect("extract only the opaque recovered Sign owner")
        .into_completion()
        .expect("the parked Sign head belongs to this lifecycle owner");
    {
        let owned = admission
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        assert_eq!(owned.owned.len(), 1);
        assert!(owned.owned[0].recovered_lifecycle_sign.is_none());
    }
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_lifecycle_signs
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::CompletionPending),
        "opaque extraction must retain the dedicated command index"
    );
    let unrelated = service.take_io_completion(true);
    let Some(PendingServiceCompletion::Io {
        completion: V2IoCompletion::AuxiliaryNoop,
        ownership_position,
    }) = unrelated.completion
    else {
        panic!("the unrelated completion must remain aligned behind extracted Sign");
    };
    service
        .io
        .as_ref()
        .expect("test I/O remains installed")
        .acknowledge_completion_at(V2IoCompletionAcknowledgement::Untracked, ownership_position)
        .expect("acknowledge only the unrelated completion");
    assert!(
        admission
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .is_empty()
    );
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_lifecycle_signs
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::CompletionPending)
    );
    let duplicate_guard = ConsensusOutputGuard::isolated();
    let duplicate_operation = duplicate_guard
        .begin_fail_stop_operation()
        .expect("open duplicate-dispatch probe");
    assert!(matches!(
        command_rx
            .queue
            .capture_recovered_lifecycle_sign_capacity(duplicate_operation, key),
        Err(RecoveredLifecycleSignCapacityCaptureErrorV1::AlreadyDispatched)
    ));
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_lifecycle_signs
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::CompletionPending),
        "duplicate dispatch coalesces on the retained exact owner"
    );
    assert!(
        !duplicate_guard.restart_required(),
        "duplicate preflight releases its uncommitted fail-stop operation"
    );
    let adapter_authority = retained
        .project_adapter_completion_authority()
        .expect("the exact parked result projects one sealed adapter preview authority");
    drop(adapter_authority);
    assert!(
        !output_guard.restart_required(),
        "dropping only the cloned preview authority cannot acknowledge the parked owner"
    );
    drop(retained);
    assert!(output_guard.restart_required());
}
#[test]
fn recovered_decision_fetch_queue_transitions_and_parks_until_dedicated_extraction() {
    let (mut service, keys) = fixture_with_block_payload();
    let (canonical_wire, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let request = authenticated_serve_request(
        &service.context,
        &keys[0],
        proposal.round,
        proposal.subject,
        wire::GlobalPhase::Commit,
    );
    let response = certified_serve_response(
        &request,
        proposal.manifest.clone(),
        canonical_wire,
        &keys[0],
    );
    let authenticated = request
        .authenticate_response(
            &service.context,
            response,
            &service.context.roster[0].validator,
        )
        .expect("authenticate the exact recovered Fetch response fixture");
    let key = RecoveredDecisionFetchDispatchKeyV1::for_test(37, 0xB1);
    let target =
        LifecycleIngressIoTargetSeal::for_recovered_decision_fetch_test(&service.context, key, 23);
    let task = RecoveredDecisionFetchBodyPersistenceTaskV1::for_test(&target, key, authenticated);
    let response_hash = task.response_hash();
    let admission = Arc::new(V2IoAdmission::new(2, 2).expect("bounded Fetch admission"));
    let (command_tx, command_rx) = v2_io_command_channel(2, 1, 1, 1, Arc::clone(&admission));
    let output_guard = ConsensusOutputGuard::isolated();
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("reserve under an open output guard");
    let V2IoLifecycleCapacityCapture::Reserved(reservation) = command_tx
        .queue
        .capture_lifecycle_capacity(operation, Arc::clone(&output_guard), target)
        .expect("capture one dedicated recovered Fetch persistence position")
    else {
        panic!("empty Consensus lane must reserve recovered Fetch capacity");
    };
    assert!(reservation.preflight_recovered_decision_fetch_body_persistence(&task));
    reservation.commit_recovered_decision_fetch_body_persistence(task);
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_decision_fetch_bodies
            .get(&key)
            .map(|tracked| (tracked.state, tracked.response_hash)),
        Some((V2IoWorkState::Queued, response_hash))
    );
    let task = match command_rx
        .try_recv()
        .expect("activate the exact recovered Fetch persistence command")
    {
        V2IoCommand::PersistRecoveredDecisionFetchBody(task) => task,
        _ => panic!("dedicated reservation published another command family"),
    };
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_decision_fetch_bodies
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::Active)
    );
    let directory = TempDir::new().expect("temporary recovered Fetch body store");
    let mut body_store = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open recovered Fetch body store");
    let completion = task
        .persist(&mut body_store)
        .map_err(|(error, _)| error)
        .expect("persist the exact authenticated recovered Fetch response");
    command_rx
        .complete_recovered_decision_fetch_body(key, &completion)
        .expect("seal the exact durable response under its dedicated key");
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_decision_fetch_bodies
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::CompletionPending)
    );
    let ordinary_tag = EventTag::new(service.context.height, 0, Generation::new(2));
    let mut ordinary_vote = routing_vote(&service, 0, wire::GlobalPhase::Prepare);
    ordinary_vote.signature.clear();
    let ordinary_task = ConsensusSignTask::for_test(
        36,
        ordinary_tag,
        super::super::v2::SignRequest::Vote(ordinary_vote),
    );
    let ordinary_id = ordinary_task.id();
    let ordinary_ordinal = ordinary_task.lifecycle_ordinal();
    command_tx
        .try_send(V2IoCommand::Sign {
            task: ordinary_task,
            restore_outbound_payload: false,
        })
        .expect("queue an ordinary runtime-producing predecessor completion");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::Sign { task, .. }) if task.id() == ordinary_id
    ));
    command_rx.complete_work(ordinary_id);
    let (completion_tx, completion_rx) = mpsc::sync_channel(2);
    send_tracked_completion_with_lifecycle_ordinal(
        &completion_tx,
        admission.as_ref(),
        V2IoCompletion::Signature {
            work_id: ordinary_id,
            signature: vec![0x51],
            outbound_payload: None,
        },
        Some(ordinary_ordinal),
    )
    .expect("publish the ordinary predecessor completion");
    send_tracked_completion_with_lifecycle_ordinal(
        &completion_tx,
        admission.as_ref(),
        V2IoCompletion::RecoveredDecisionFetchBodyPersisted(Box::new(
            GuardedRecoveredDecisionFetchBodyPersistenceCompletionV1::new(
                completion,
                Arc::clone(&output_guard),
            ),
        )),
        Some(key.lifecycle_ordinal()),
    )
    .expect("publish tracked recovered Fetch completion");
    service.output_guard = Arc::clone(&output_guard);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission: Arc::clone(&admission),
    });
    let generic = service.take_io_completion(false);
    assert!(generic.completion.is_none() && generic.retained_runtime);
    let still_blocked = service.take_io_completion(false);
    assert!(still_blocked.completion.is_none() && still_blocked.retained_runtime);
    assert_eq!(
        admission
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .len(),
        2,
        "the recovered Fetch payload remains in-channel behind the held runtime result"
    );
    let ordinary = service.take_io_completion(true);
    let Some(PendingServiceCompletion::Io {
        completion: V2IoCompletion::Signature { work_id, .. },
        ownership_position,
    }) = ordinary.completion
    else {
        panic!("available runtime capacity must service the held ordinary predecessor");
    };
    assert_eq!(work_id, ordinary_id);
    service
        .io
        .as_ref()
        .expect("test I/O remains installed")
        .acknowledge_completion_at(
            V2IoCompletionAcknowledgement::Work(work_id),
            ownership_position,
        )
        .expect("acknowledge the ordinary predecessor only");
    let retained = service
        .drain_recovered_decision_fetch_body_completion()
        .expect("extract only the dedicated recovered Fetch completion")
        .into_completion()
        .expect("the parked completion retains its exact queue owner");
    assert_eq!(
        command_rx
            .queue
            .lock()
            .recovered_decision_fetch_bodies
            .get(&key)
            .map(|tracked| tracked.state),
        Some(V2IoWorkState::CompletionPending),
        "opaque extraction must retain the dedicated persistence index"
    );
    assert!(
        admission
            .completion_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .owned
            .is_empty(),
        "dedicated extraction transfers only completion metadata"
    );
    drop(retained);
    assert!(output_guard.restart_required());
}
#[test]
fn recovered_lifecycle_sign_capacity_unavailable_leaves_no_dedicated_index() {
    use super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignClassV1;
    let admission = Arc::new(V2IoAdmission::new(1, 1).expect("bounded Sign admission"));
    let (command_tx, command_rx) = v2_io_command_channel(1, 1, 1, 1, Arc::clone(&admission));
    command_tx
        .try_send(V2IoCommand::Shutdown)
        .expect("fill the sole physical queue position");
    let key = RecoveredLifecycleSignDispatchKeyV1::for_test(
        41,
        5,
        RecoveredLifecycleSignClassV1::ControlTimeout,
    );
    let output_guard = ConsensusOutputGuard::isolated();
    let operation = output_guard
        .begin_fail_stop_operation()
        .expect("probe capacity under an open output guard");
    assert!(matches!(
        command_tx
            .queue
            .capture_recovered_lifecycle_sign_capacity(operation, key),
        Ok(RecoveredLifecycleSignCapacityCaptureV1::Unavailable)
    ));
    assert!(!output_guard.restart_required());
    assert!(
        command_rx.queue.lock().recovered_lifecycle_signs.is_empty(),
        "unavailable capacity cannot publish a dedicated Sign index"
    );
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 1);
    assert!(matches!(command_rx.try_recv(), Ok(V2IoCommand::Shutdown)));
    assert_eq!(admission.queued.load(AtomicOrdering::Acquire), 0);
}
#[test]
#[allow(clippy::too_many_lines)]
fn cold_durable_proposal_refanout_atomically_owns_control_and_chunks() {
    let (mut service, keys) = fixture_with_block_payload();
    let directory = TempDir::new().expect("temporary cold Proposal output store");
    let body_store = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open exact cold Proposal output store");
    let body_store_identity = body_store.instance_identity();
    let output_guard = ConsensusOutputGuard::isolated();
    let (_, payload, mut proposal) = proposal_body_and_payload(&service.context, &keys);
    let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
    proposal.signature =
        Signature::new(keys[proposer].private_key(), &proposal.signature_preimage())
            .payload()
            .to_vec();
    set_local_validator(&mut service, &keys, proposal.proposer);
    let service_context = service.context.clone();
    let active_tag = service.active_tag;
    let _service_io = install_lifecycle_planner_io_for_validator_for_test(
        &mut service,
        service_context.clone(),
        active_tag,
        proposal.proposer,
        Arc::clone(&output_guard),
        body_store,
        body_store_identity.clone(),
        1,
    );
    install_local_signer_for_test(&mut service, &keys[proposer]);
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let cold_output = super::super::v2::RecoveredLifecycleColdProposalOutputV1::for_test(
        payload.clone(),
        body_store_identity,
    );
    let authority = super::super::v2_lifecycle_coordinator::RecoveredLifecycleSignedBroadcastOutputAuthorityV1::for_cold_proposal_test(
        &service_context,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
            proposal.clone(),
        )),
        cold_output,
    );
    let RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(output) = service
        .capture_recovered_lifecycle_signed_broadcast_refanout(authority)
        .expect("cold Proposal re-enters its exact body-store service")
    else {
        panic!("empty aggregate corridor must reserve Proposal control and chunks")
    };
    output.commit_after_publication();
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect the atomic cold Proposal fanouts");
        assert_eq!(pending.fanouts.len(), 2);
        assert!(matches!(
            &pending.fanouts[0].rollover_claim,
            ExactOutputRolloverClaim::GlobalV2(_)
        ));
        assert!(matches!(
            &pending.fanouts[1].rollover_claim,
            ExactOutputRolloverClaim::PayloadChunks { .. }
        ));
        assert_eq!(pending.fanouts[0].fifo_id, Some(0));
        assert_eq!(pending.fanouts[1].fifo_id, Some(1));
    }
    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("retry the atomic cold Proposal fanout")
    );
    assert!(!service.output_guard.restart_required());
}
