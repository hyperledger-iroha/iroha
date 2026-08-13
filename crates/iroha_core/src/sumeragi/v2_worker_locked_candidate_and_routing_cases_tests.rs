#[test]
fn closed_network_actor_fails_stop_before_later_output() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    service
        .lock_pending_exact_output()
        .expect("lock output corridor")
        .enqueue(
            PendingExactFanout::new(vec![lane_commit_qc_message(peer.clone())], vec![peer])
                .expect("non-empty final QC fanout"),
        )
        .expect("retain final QC before actor admission");

    let error = service
        .retry_pending_exact_output()
        .expect_err("a permanently closed network actor must fail stop");

    assert!(error.contains("network actor closed"));
    assert!(service.output_guard.restart_required());
    let pending = service
        .lock_pending_exact_output()
        .expect("inspect fail-stop output ownership");
    assert_eq!(pending.fanouts.len(), 1);
    let retained = pending.fanouts[0].targets[0]
        .current
        .as_ref()
        .expect("closed actor returned the exact final QC post");
    assert!(matches!(&retained.data, NetworkMessage::SumeragiBlock(_)));
}

#[test]
fn full_exact_output_corridor_does_not_disguise_non_progress_routes_as_backpressure() {
    let (service, _) = fixture();
    let peer = service.context.roster[1].validator.clone();
    let mut pending = PendingExactOutput::new(1, 1, 1, &[]).expect("one-fanout corridor");
    assert_eq!(
        pending
            .enqueue(
                PendingExactFanout::new(
                    vec![lane_commit_qc_message(peer.clone())],
                    vec![peer.clone()],
                )
                .expect("valid progress fanout"),
            )
            .expect("valid fanout enters corridor"),
        ExactFanoutOwnership::Owned
    );
    let error = PendingExactFanout::classified_with_routes(
        vec![NetworkMessage::Health],
        vec![peer],
        vec![ExactTargetRoute::Topology],
    )
    .expect_err("a non-progress route has no reliable scheduler class");

    assert!(error.contains("no reliable progress class"));
    assert_eq!(pending.fanouts.len(), 1);
    assert!(!service.output_guard.restart_required());
}

fn locked_candidate_subject(label: &[u8]) -> wire::BlockSubject {
    wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        payload_hash: Hash::new(label),
    }
}

fn locked_candidate_tag(view: u64) -> EventTag {
    EventTag::new(1, view, Generation::new(view + 1))
}

fn locked_candidate_round(service: &ProductionV2Services, view: u64) -> wire::ConsensusRound {
    wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view,
    }
}

fn attach_locked_candidate_io(
    service: &mut ProductionV2Services,
    capacity: usize,
) -> V2IoCommandReceiver {
    let (command_tx, command_rx, admission) = test_io_command_channel(capacity);
    let (_completion_tx, completion_rx) = mpsc::sync_channel(capacity);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    command_rx
}

fn detach_locked_candidate_io(service: &mut ProductionV2Services) {
    drop(service.io.take());
}

#[test]
fn locked_candidate_requests_coalesce_by_immutable_subject() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"coalesced locked candidate");

    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue the one physical acquisition");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("coalesce an exact retransmission");
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("rebind the same acquisition to a later view");
    let same_view_rebound = EventTag::new(1, 1, Generation::new(3));
    service
        .request_locked_candidate(
            same_view_rebound,
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("rebind the same acquisition to a newer same-view generation");

    let commands = command_rx.try_iter().collect::<Vec<_>>();
    assert!(matches!(
        commands.as_slice(),
        [V2IoCommand::LoadCandidate { subject: queued, .. }] if *queued == subject
    ));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("one acquisition owner");
    assert_eq!(acquisition.subject, subject);
    assert_eq!(acquisition.consumer, same_view_rebound);
    assert_eq!(acquisition.pending_count(), 1);
    detach_locked_candidate_io(&mut service);
}

#[test]
fn locked_candidate_completion_uses_latest_consumer_without_reloading() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"rebound locked candidate");
    let canonical_wire = b"exact durable body".to_vec();

    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue initial load");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the one exact-subject candidate load"),
    };
    service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject,
            canonical_wire: canonical_wire.clone(),
        })
        .expect("complete the physical load");
    service
        .request_locked_candidate(
            locked_candidate_tag(3),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("advance the ready result consumer");

    let first = service
        .take_loaded_candidate()
        .expect("deliver ready bytes to the latest view");
    assert_eq!(first.tag(), locked_candidate_tag(3));
    assert_eq!(first.round(), locked_candidate_round(&service, 0));
    assert_eq!(first.subject(), subject);
    assert_eq!(first.into_canonical_wire(), canonical_wire);
    assert!(service.take_loaded_candidate().is_none());

    service
        .request_locked_candidate(
            locked_candidate_tag(4),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("rebind retained ready bytes once more");
    let second = service
        .take_loaded_candidate()
        .expect("redeliver retained bytes without another read");
    assert_eq!(second.tag(), locked_candidate_tag(4));
    assert_eq!(second.subject(), subject);
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    detach_locked_candidate_io(&mut service);
}

#[test]
fn locked_candidate_consumer_rebind_rejects_stale_or_regressive_tags() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"monotonic locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(2),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue current-view acquisition");

    let stale = service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect_err("a stale consumer must not replace the latest binding");
    assert!(stale.contains("did not advance monotonically"));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("original acquisition remains owned");
    assert_eq!(acquisition.consumer, locked_candidate_tag(2));
    assert!(service.output_guard.restart_required());
    assert_eq!(command_rx.try_iter().count(), 1);
    detach_locked_candidate_io(&mut service);
}

#[test]
fn locked_candidate_duplicate_or_wrong_completion_is_rejected() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"owned locked candidate");
    let wrong = locked_candidate_subject(b"conflicting locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue owned acquisition");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the owned candidate load"),
    };

    let completion_error = service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject: wrong,
            canonical_wire: b"wrong body".to_vec(),
        })
        .expect_err("wrong-subject completion must be rejected");
    assert!(completion_error.contains("different acquisition subject"));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("exact acquisition remains owned");
    assert_eq!(acquisition.subject, subject);
    assert!(matches!(
        &acquisition.state,
        LockedCandidateAcquisitionState::Loading { .. }
    ));

    service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject,
            canonical_wire: b"exact body".to_vec(),
        })
        .expect("complete the exact acquisition");
    let duplicate = service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id,
            subject,
            canonical_wire: b"exact body".to_vec(),
        })
        .expect_err("duplicate completion must be rejected");
    assert!(duplicate.contains("completed more than once"));
    detach_locked_candidate_io(&mut service);
}

#[test]
fn locked_candidate_future_completion_is_rejected_without_replacing_owner() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"future completion owner");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue owned acquisition");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the owned candidate load"),
    };
    let future_id = LockedCandidateAcquisitionId(
        acquisition_id
            .0
            .checked_add(1)
            .expect("test acquisition ID has a successor"),
    );

    let future = service
        .complete_locked_candidate_load(LockedCandidateLoad {
            acquisition_id: future_id,
            subject,
            canonical_wire: b"forged future body".to_vec(),
        })
        .expect_err("an unissued future completion must fail closed");
    assert!(future.contains("unknown future acquisition ID"));
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("the issued acquisition remains owned");
    assert_eq!(acquisition.subject, subject);
    assert!(matches!(
        acquisition.state,
        LockedCandidateAcquisitionState::Loading {
            acquisition_id: owned,
            subject: owned_subject,
        } if owned == acquisition_id && owned_subject == subject
    ));
    assert!(service.take_loaded_candidate().is_none());
    detach_locked_candidate_io(&mut service);
}

#[test]
fn higher_different_lock_replaces_load_and_retires_stale_completion() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let original = locked_candidate_subject(b"original locked candidate");
    let replacement = locked_candidate_subject(b"higher locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            original,
        )
        .expect("queue original acquisition");
    let original_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        }) if subject == original => acquisition_id,
        _ => panic!("expected original candidate load"),
    };

    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 1),
            replacement,
        )
        .expect("a higher lock replaces the desired subject");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: original_id,
                subject: original,
                canonical_wire: b"superseded body".to_vec(),
            })
            .expect("retire superseded physical result"),
        None
    );
    let replacement_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        }) if subject == replacement => acquisition_id,
        _ => panic!("expected one replacement candidate load"),
    };
    assert!(replacement_id > original_id);

    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: original_id,
                subject: original,
                canonical_wire: b"late duplicate".to_vec(),
            })
            .expect("late superseded completion is non-fatal"),
        None
    );
    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: replacement_id,
                subject: replacement,
                canonical_wire: b"replacement body".to_vec(),
            })
            .expect("complete replacement acquisition"),
        Some(locked_candidate_tag(1))
    );
    let loaded = service
        .take_loaded_candidate()
        .expect("deliver only the higher locked body");
    assert_eq!(loaded.tag(), locked_candidate_tag(1));
    assert_eq!(loaded.round(), locked_candidate_round(&service, 1));
    assert_eq!(loaded.subject(), replacement);
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    detach_locked_candidate_io(&mut service);
}

#[test]
fn superseded_locked_candidate_failure_starts_latest_acquisition() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let original = locked_candidate_subject(b"failing original candidate");
    let replacement = locked_candidate_subject(b"replacement after failure");
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 0),
            original,
        )
        .expect("queue original acquisition");
    let original_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject,
        }) if subject == original => acquisition_id,
        _ => panic!("expected original candidate load"),
    };
    service
        .request_locked_candidate(
            locked_candidate_tag(1),
            locked_candidate_round(&service, 1),
            replacement,
        )
        .expect("install higher same-incarnation lock");

    assert_eq!(
        service
            .locked_candidate_load_failed(
                original_id,
                original,
                "superseded read failure".to_owned(),
            )
            .expect("superseded failure must retire non-fatally"),
        None
    );
    assert!(!service.output_guard.restart_required());
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate { subject, .. }) if subject == replacement
    ));
    detach_locked_candidate_io(&mut service);
}

#[test]
fn unavailable_locked_candidate_waits_for_matching_durable_store() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"not-yet-durable locked candidate");
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue initial acquisition");
    let acquisition_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected initial candidate load"),
    };

    assert_eq!(
        service
            .locked_candidate_load_unavailable(acquisition_id, subject)
            .expect("local absence is a recoverable state"),
        None
    );
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("waiting acquisition remains owned");
    assert!(matches!(
        &acquisition.state,
        LockedCandidateAcquisitionState::Waiting { .. }
    ));
    assert_eq!(acquisition.pending_count(), 1);
    assert!(!service.output_guard.restart_required());

    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("same request coalesces while certified recovery runs");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    service
        .retry_locked_candidate_after_store(locked_candidate_subject(b"unrelated body"))
        .expect("unrelated store cannot steal retry ownership");
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    service
        .retry_locked_candidate_after_store(subject)
        .expect("matching durable store requeues exactly once");
    assert!(matches!(
        command_rx.try_recv(),
        Ok(V2IoCommand::LoadCandidate { subject: queued, .. }) if queued == subject
    ));
    detach_locked_candidate_io(&mut service);
}

#[test]
fn unavailable_locked_candidate_rebinds_latest_consumer_before_retry() {
    let (mut service, _) = fixture();
    let command_rx = attach_locked_candidate_io(&mut service, 4);
    let subject = locked_candidate_subject(b"waiting rebound candidate");
    let canonical_wire = b"recovered exact body".to_vec();
    service
        .request_locked_candidate(
            locked_candidate_tag(0),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("queue initial acquisition");
    let initial_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected initial candidate load"),
    };
    service
        .locked_candidate_load_unavailable(initial_id, subject)
        .expect("local absence waits for certified recovery");

    service
        .request_locked_candidate(
            locked_candidate_tag(7),
            locked_candidate_round(&service, 0),
            subject,
        )
        .expect("same lock rebinds while durable recovery is pending");
    let acquisition = service
        .locked_candidate_acquisition
        .as_ref()
        .expect("waiting acquisition remains owned");
    assert_eq!(acquisition.consumer, locked_candidate_tag(7));
    assert!(matches!(
        acquisition.state,
        LockedCandidateAcquisitionState::Waiting {
            acquisition_id,
            subject: waiting_subject,
        } if acquisition_id == initial_id && waiting_subject == subject
    ));
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));

    service
        .retry_locked_candidate_after_store(subject)
        .expect("matching durable store starts one replacement read");
    let retry_id = match command_rx.try_recv() {
        Ok(V2IoCommand::LoadCandidate {
            acquisition_id,
            subject: queued,
        }) if queued == subject => acquisition_id,
        _ => panic!("expected the matching durable retry"),
    };
    assert!(retry_id > initial_id);
    assert_eq!(
        service
            .complete_locked_candidate_load(LockedCandidateLoad {
                acquisition_id: retry_id,
                subject,
                canonical_wire: canonical_wire.clone(),
            })
            .expect("complete the recovered exact acquisition"),
        Some(locked_candidate_tag(7))
    );
    let loaded = service
        .take_loaded_candidate()
        .expect("deliver recovered bytes only to the latest consumer");
    assert_eq!(loaded.tag(), locked_candidate_tag(7));
    assert_eq!(loaded.round(), locked_candidate_round(&service, 0));
    assert_eq!(loaded.subject(), subject);
    assert_eq!(loaded.into_canonical_wire(), canonical_wire);
    assert!(service.take_loaded_candidate().is_none());
    assert!(matches!(
        command_rx.try_recv(),
        Err(mpsc::TryRecvError::Empty)
    ));
    detach_locked_candidate_io(&mut service);
}

fn proposal_body_and_payload(
    context: &wire::HeightContext,
    keys: &[KeyPair],
) -> (Vec<u8>, EncodedV2Payload, wire::Proposal) {
    let (canonical_wire, payload) = proposal_body_and_payload_at_view(context, keys, 0);
    let round = payload.manifest().round;
    let proposer = context.leader(round.view);
    let proposal = wire::Proposal {
        round,
        proposer,
        subject: payload.manifest().subject,
        manifest: payload.manifest().clone(),
        justification: wire::ProposalJustification::ParentCommit(
            wire::ParentCommitJustification { certificate: None },
        ),
        signature: Vec::new(),
    };
    (canonical_wire, payload, proposal)
}

fn proposal_body_and_payload_at_view(
    context: &wire::HeightContext,
    keys: &[KeyPair],
    view: u64,
) -> (Vec<u8>, EncodedV2Payload) {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let proposer = context.leader(round.view);
    let proposer_index = usize::try_from(proposer).expect("fixture proposer index");
    // The immutable body was created by the genesis authority in view 0;
    // `view` is the certified round in which that exact body is proposed
    // or reproposed after restart.
    let header = BlockHeader::new(
        NonZeroU64::new(round.height).expect("non-zero fixture height"),
        None,
        None,
        None,
        1_000,
        0,
    );
    let signature =
        SignatureOf::try_from_hash(keys[proposer_index].private_key(), header.hash())
            .expect("sign fixture block header");
    let block = SignedBlock::presigned(
        BlockSignature::new(u64::from(proposer), signature),
        header,
        Vec::new(),
    );
    let canonical_wire = block.encode_wire().expect("canonical fixture block");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let payload = encode_payload(context, round, subject, &canonical_wire)
        .expect("encode fixture proposal payload");
    (canonical_wire, payload)
}

fn allow_fixture_block_payload(context: &mut wire::HeightContext) {
    context.da_layout = wire::DataAvailabilityLayout {
        encoding: wire::PayloadEncoding::ReedSolomon16,
        chunk_size_bytes: 1_024,
        data_shards: 1,
        parity_shards: 1,
        max_payload_size_bytes: 16_384,
        max_chunk_count: 32,
    };
    context.validate().expect("widened fixture context");
}

fn fixture_with_block_payload() -> (ProductionV2Services, Vec<KeyPair>) {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    (service, keys)
}

type ConsensusRouteObservation = (PeerId, wire::ConsensusMessageV2);

fn install_consensus_route_observer(
    service: &mut ProductionV2Services,
) -> Arc<Mutex<Vec<ConsensusRouteObservation>>> {
    let observations = Arc::new(Mutex::new(Vec::new()));
    let observed = Arc::clone(&observations);
    service.set_exact_output_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        let NetworkMessage::SumeragiBlock(envelope) = &post.data else {
            panic!("consensus routing emitted a non-Sumeragi message");
        };
        let BlockMessage::V2(message) = envelope.as_message() else {
            panic!("consensus routing emitted a lane message");
        };
        observed
            .lock()
            .expect("lock consensus route observations")
            .push((post.peer_id, message.clone()));
        Ok(())
    });
    observations
}

fn take_consensus_route_observations(
    observations: &Mutex<Vec<ConsensusRouteObservation>>,
) -> Vec<ConsensusRouteObservation> {
    std::mem::take(
        &mut *observations
            .lock()
            .expect("inspect consensus route observations"),
    )
}

fn proposal_route_targets(
    observations: &[ConsensusRouteObservation],
    round: wire::ConsensusRound,
    manifest: &wire::PayloadManifest,
) -> BTreeSet<PeerId> {
    observations
        .iter()
        .filter_map(|(peer, message)| match &message.payload {
            wire::ConsensusMessageV2Payload::Proposal(proposal)
                if proposal.round == round && proposal.manifest == *manifest =>
            {
                Some(peer.clone())
            }
            _ => None,
        })
        .collect()
}

fn chunk_route_targets(
    observations: &[ConsensusRouteObservation],
    manifest: &wire::PayloadManifest,
) -> BTreeSet<PeerId> {
    let manifest_hash = HashOf::new(manifest);
    observations
        .iter()
        .filter_map(|(peer, message)| match &message.payload {
            wire::ConsensusMessageV2Payload::PayloadChunk(chunk)
                if chunk.manifest_hash == manifest_hash =>
            {
                Some(peer.clone())
            }
            _ => None,
        })
        .collect()
}

fn set_local_validator(
    service: &mut ProductionV2Services,
    keys: &[KeyPair],
    validator: wire::ValidatorIndex,
) {
    let index = usize::try_from(validator).expect("fixture validator index");
    service.local_validator = Some(validator);
    service.local_peer = service.context.roster[index].validator.clone();
    service.key_pair = keys[index].clone();
}

fn routing_vote(
    service: &ProductionV2Services,
    view: u64,
    phase: wire::GlobalPhase,
) -> wire::Vote {
    let round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view,
    };
    wire::Vote {
        round,
        proposal_round: round,
        phase,
        subject: wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"routing vote block")),
            payload_hash: Hash::new(b"routing vote payload"),
        },
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"routing vote parent state"),
            Hash::new(b"routing vote post state"),
            Hash::new(b"routing vote ordinary writes"),
            1,
            Hash::new(b"routing vote executed block wire"),
        ),
        signer: service
            .local_validator
            .expect("routing fixture is a voting validator"),
        signature: vec![0xA5; 48],
    }
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
    match service
        .capture_recovered_lifecycle_signed_broadcast_refanout(authority)
        .expect("exact durable Broadcast authority enters the service cut")
    {
        RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(output) => {
            output.commit_after_publication();
        }
        RecoveredLifecycleSignBroadcastOutputCaptureV1::Unavailable => {
            panic!("an empty exact-output corridor must reserve the durable Broadcast")
        }
    }
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
