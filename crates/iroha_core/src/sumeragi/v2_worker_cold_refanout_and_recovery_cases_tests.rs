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
    match service
        .capture_recovered_lifecycle_signed_broadcast_refanout(authority)
        .expect("cold Proposal re-enters its exact body-store service")
    {
        RecoveredLifecycleSignBroadcastOutputCaptureV1::Reserved(output) => {
            output.commit_after_publication();
        }
        RecoveredLifecycleSignBroadcastOutputCaptureV1::Unavailable => {
            panic!("empty aggregate corridor must reserve Proposal control and chunks")
        }
    }
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

#[test]
#[allow(clippy::too_many_lines)]
fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound() {
    use super::super::v2_lifecycle_coordinator::{
        RecoveredLifecycleSignClassV1, RecoveredLifecycleSignDispatchIdentityV1,
    };

    let (mut service, keys) = fixture_with_block_payload();
    let directory = TempDir::new().expect("temporary recovered Proposal output store");
    let body_store = V2BodyStore::open(directory.path(), service.context.clone())
        .expect("open exact recovered Proposal output store");
    let body_store_identity = body_store.instance_identity();
    let output_guard = ConsensusOutputGuard::isolated();
    let (_, payload, mut proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = service.active_tag;
    let request = super::super::v2::SignRequest::Proposal(proposal.clone());
    let dispatch_key = RecoveredLifecycleSignDispatchIdentityV1::for_test(
        91,
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
    let service_context = service.context.clone();
    let service_io = install_lifecycle_planner_io_for_validator_for_test(
        &mut service,
        service_context.clone(),
        tag,
        proposal.proposer,
        Arc::clone(&output_guard),
        body_store,
        body_store_identity.clone(),
        1,
    );
    install_local_signer_for_test(&mut service, &keys[proposer]);
    service
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install adversarial one-unit output corridor");

    let authority_context = service_context;
    let authority = |identity: V2BodyStoreInstanceIdentity,
                     guard: Arc<ConsensusOutputGuard>| {
        super::super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1::for_test(
            &authority_context,
            dispatch_key,
            tag,
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
                proposal.clone(),
            )),
            payload.clone(),
            identity,
            guard,
        )
        .expect("fixture Proposal output authority is structurally exact")
    };

    let foreign_directory = TempDir::new().expect("temporary foreign Proposal output store");
    let foreign_identity = V2BodyStore::open(foreign_directory.path(), service.context.clone())
        .expect("open foreign Proposal output store")
        .instance_identity();
    assert!(
        service
            .capture_recovered_lifecycle_proposal_exact_output(authority(
                foreign_identity,
                Arc::clone(&output_guard),
            ))
            .is_err(),
        "a same-context Proposal cannot cross a foreign body-store owner"
    );
    let foreign_guard = ConsensusOutputGuard::isolated();
    assert!(
        service
            .capture_recovered_lifecycle_proposal_exact_output(authority(
                body_store_identity.clone(),
                foreign_guard,
            ))
            .is_err(),
        "a same-store Proposal cannot cross a foreign output guard"
    );

    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let blocking_vote = routing_vote(&service, 0, wire::GlobalPhase::Prepare);
    assert_eq!(
        service
            .broadcast_consensus(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(blocking_vote),
            ))
            .expect("retain one exact blocking owner"),
        ConsensusBroadcastDisposition::ExactServiceAccepted
    );
    let before = {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect the exact blocking owner");
        (
            pending.fanouts.len(),
            pending.source_fifo_owners.clone(),
            pending.reservation_owner_counts.clone(),
            pending.ownership_units,
            pending.shared_ownership_units,
            pending.next_fanout_fifo_id,
        )
    };
    let expected_batch_first_fifo = before.5;
    let retry_authority = match service
        .capture_recovered_lifecycle_proposal_exact_output(authority(
            body_store_identity.clone(),
            Arc::clone(&output_guard),
        ))
        .expect("capacity pressure is a typed Proposal retry")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority) => authority,
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(_) => {
            panic!("aggregate Proposal demand must not fit behind the blocking owner")
        }
    };
    let after = {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect unchanged pressured corridor");
        (
            pending.fanouts.len(),
            pending.source_fifo_owners.clone(),
            pending.reservation_owner_counts.clone(),
            pending.ownership_units,
            pending.shared_ownership_units,
            pending.next_fanout_fifo_id,
        )
    };
    assert_eq!(
        after, before,
        "capacity failure cannot install either fanout"
    );
    assert!(service.fast_path_proposals.is_empty());
    assert!(!service.output_guard.restart_required());

    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("drain the exact blocking owner")
    );
    let retry_authority = match service
        .capture_recovered_lifecycle_proposal_exact_output(retry_authority)
        .expect("the unchanged Proposal authority reserves after capacity recovers")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(reservation) => {
            reservation.abort_before_publication()
        }
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty corridor must reserve the aggregate Proposal output")
        }
    };
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("typed abort leaves the corridor empty");
        assert!(pending.fanouts.is_empty());
        assert!(pending.source_fifo_owners.is_empty());
        assert!(pending.reservation_owner_counts.is_empty());
    }
    match service
        .capture_recovered_lifecycle_proposal_exact_output(retry_authority)
        .expect("retry the exact authority after typed abort")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(reservation) => {
            reservation.commit_after_publication();
        }
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty corridor must retain the complete Proposal batch")
        }
    }
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect committed atomic Proposal batch");
        let expected_peers = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
        assert_eq!(pending.fanouts.len(), 2);
        assert!(matches!(
            &pending.fanouts[0].rollover_claim,
            ExactOutputRolloverClaim::GlobalV2(_)
        ));
        assert!(matches!(
            &pending.fanouts[1].rollover_claim,
            ExactOutputRolloverClaim::PayloadChunks { .. }
        ));
        assert_eq!(
            pending
                .fanouts
                .iter()
                .map(|fanout| fanout.fifo_id)
                .collect::<Vec<_>>(),
            vec![
                Some(expected_batch_first_fifo),
                expected_batch_first_fifo.checked_add(1),
            ],
            "the inseparable control/chunk pair owns adjacent FIFO identities"
        );
        for fanout in &pending.fanouts {
            assert_eq!(
                fanout.peers.iter().cloned().collect::<BTreeSet<_>>(),
                expected_peers,
                "restart Proposal dissemination targets every remote voter"
            );
        }
        let control = &pending.fanouts[0];
        let chunks = &pending.fanouts[1];
        let [NetworkMessage::SumeragiBlock(control)] = control.messages.as_slice() else {
            panic!("the first fanout must retain one Proposal control")
        };
        assert!(matches!(
            control.as_message(),
            BlockMessage::V2(message)
                if message.payload
                    == wire::ConsensusMessageV2Payload::Proposal(proposal.clone())
        ));
        let manifest = payload.manifest();
        let signer = &service.context.roster[proposer].validator;
        let mut observed_chunk_indices = BTreeSet::new();
        for encoded in &chunks.messages {
            let NetworkMessage::SumeragiBlock(envelope) = encoded else {
                panic!("the second fanout must retain only payload chunks")
            };
            let BlockMessage::V2(message) = envelope.as_message() else {
                panic!("the recovered Proposal chunk changed protocol lane")
            };
            let wire::ConsensusMessageV2Payload::PayloadChunk(chunk) = &message.payload else {
                panic!("the recovered Proposal chunk fanout mixed message classes")
            };
            chunk
                .validate(&service.context, manifest)
                .expect("the retained chunk matches its canonical manifest");
            let signature = Signature::try_from_bytes(&chunk.signature)
                .expect("the retained chunk signature is canonical");
            signature
                .verify(
                    signer.public_key(),
                    &chunk
                        .signature_preimage(&service.context, manifest)
                        .expect("the retained chunk has a canonical signature preimage"),
                )
                .expect("the retained chunk is signed by the recovered proposer");
            assert!(observed_chunk_indices.insert(chunk.index));
        }
        assert_eq!(
            observed_chunk_indices.len(),
            manifest.chunk_hashes.len(),
            "the exact batch retains every canonical chunk once"
        );
        assert_eq!(
            pending.ownership_units,
            pending.reservation_owner_counts.values().sum::<usize>()
        );
        assert!(!pending.source_fifo_owners.is_empty());
    }
    assert!(
        service.fast_path_proposals.is_empty(),
        "recovered restart dissemination deliberately targets all voters without mutating live fast-path state"
    );
    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("drain both committed Proposal fanouts")
    );
    let retirement_authority = match service
        .capture_recovered_lifecycle_proposal_exact_output(authority(
            body_store_identity,
            Arc::clone(&output_guard),
        ))
        .expect("reserve one final exact Proposal before Decision")
    {
        RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(reservation) => {
            reservation.abort_before_publication()
        }
        RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(_) => {
            panic!("empty corridor must reserve the pre-Decision Proposal")
        }
    };
    service
        .retire_candidate_work_after_decision(proposal.round, proposal.subject)
        .expect("retire Proposal work at the durable Decision boundary");
    assert!(
        service
            .capture_recovered_lifecycle_proposal_exact_output(retirement_authority)
            .is_err(),
        "a pre-Decision Proposal authority cannot cross candidate retirement"
    );
    assert!(!service.output_guard.restart_required());
    service_io.detach(&mut service);
}

#[test]
fn prepare_and_commit_votes_reach_every_remote_voter_across_views() {
    let (mut service, _) = fixture();
    let observations = install_consensus_route_observer(&mut service);
    let roster_len =
        u64::try_from(service.context.roster.len()).expect("fixture roster length");
    let expected = service.remote_voters().into_iter().collect::<BTreeSet<_>>();

    for view in 0..roster_len {
        let round = wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view,
        };
        for phase in [wire::GlobalPhase::Prepare, wire::GlobalPhase::Commit] {
            let vote = routing_vote(&service, view, phase);
            service
                .broadcast_consensus(wire::ConsensusMessageV2::new(
                    wire::ConsensusMessageV2Payload::Vote(vote),
                ))
                .expect("route phase vote to every remote voter");
            let routed = take_consensus_route_observations(&observations);
            let targets = routed
                .iter()
                .filter_map(|(peer, message)| match &message.payload {
                    wire::ConsensusMessageV2Payload::Vote(vote)
                        if vote.round == round && vote.phase == phase =>
                    {
                        Some(peer.clone())
                    }
                    _ => None,
                })
                .collect::<BTreeSet<_>>();
            assert_eq!(
                targets, expected,
                "phase vote fanout differs in view {view}"
            );
            assert_eq!(routed.len(), expected.len());
        }
    }
}

#[test]
fn first_proposal_routes_manifest_control_to_all_and_chunks_to_set_a() {
    let (mut service, keys) = fixture_with_block_payload();
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    set_local_validator(&mut service, &keys, proposal.proposer);
    let manifest = payload.manifest().clone();
    service
        .register_outbound_payload(service.active_tag, payload)
        .expect("retain first proposal chunks");
    let committee = service
        .committee_for_round(proposal.round)
        .expect("project first proposal committee");
    let expected_control = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    let expected_chunks = service
        .remote_voters_for_indices(committee.set_a())
        .expect("resolve first proposal Set A")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let observations = install_consensus_route_observer(&mut service);

    service
        .broadcast_consensus(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
        ))
        .expect("broadcast first proposal");

    let routed = take_consensus_route_observations(&observations);
    assert_eq!(
        proposal_route_targets(&routed, proposal.round, &manifest),
        expected_control
    );
    assert_eq!(chunk_route_targets(&routed, &manifest), expected_chunks);
    assert!(routed.iter().all(|(_, message)| matches!(
        &message.payload,
        wire::ConsensusMessageV2Payload::Proposal(routed)
            if routed.manifest == manifest
    ) || matches!(
        &message.payload,
        wire::ConsensusMessageV2Payload::PayloadChunk(_)
    )));
}

#[test]
fn same_round_proposal_retransmission_expands_chunks_to_set_b_and_all_voters() {
    let (mut service, keys) = fixture_with_block_payload();
    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    set_local_validator(&mut service, &keys, proposal.proposer);
    let manifest = payload.manifest().clone();
    service
        .register_outbound_payload(service.active_tag, payload)
        .expect("retain proposal chunks");
    let committee = service
        .committee_for_round(proposal.round)
        .expect("project proposal committee");
    let expected_fast = service
        .remote_voters_for_indices(committee.set_a())
        .expect("resolve Set A")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_set_b = service
        .remote_voters_for_indices(committee.set_b())
        .expect("resolve Set B")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_all = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    assert!(!expected_set_b.is_empty());
    let observations = install_consensus_route_observer(&mut service);
    let message = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
        proposal.clone(),
    ));

    service
        .broadcast_consensus(message.clone())
        .expect("broadcast first proposal occurrence");
    let first = take_consensus_route_observations(&observations);
    assert_eq!(chunk_route_targets(&first, &manifest), expected_fast);

    service
        .broadcast_consensus(message)
        .expect("broadcast same-round proposal retransmission");
    let retransmission = take_consensus_route_observations(&observations);
    let retransmitted_chunks = chunk_route_targets(&retransmission, &manifest);
    assert_eq!(retransmitted_chunks, expected_all);
    assert!(expected_set_b.is_subset(&retransmitted_chunks));
    assert_eq!(
        proposal_route_targets(&retransmission, proposal.round, &manifest),
        expected_all
    );
}

#[test]
fn proposal_broadcast_reports_source_retained_until_corridor_acceptance() {
    let (mut service, keys) = fixture_with_block_payload();
    service
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install one-unit adversarial output corridor");
    service.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });

    let blocking_vote = routing_vote(&service, 0, wire::GlobalPhase::Prepare);
    assert_eq!(
        service
            .broadcast_consensus(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::Vote(blocking_vote),
            ))
            .expect("the first control transfers into the exact corridor"),
        ConsensusBroadcastDisposition::ExactServiceAccepted
    );
    assert!(
        service
            .has_pending_exact_output()
            .expect("inspect actor-backpressured control")
    );

    let (_, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    set_local_validator(&mut service, &keys, proposal.proposer);
    service
        .register_outbound_payload(service.active_tag, payload)
        .expect("retain proposal chunks before broadcast");
    let message = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(
        proposal.clone(),
    ));
    assert_eq!(
        service
            .broadcast_consensus(message.clone())
            .expect("corridor pressure is a typed ownership disposition"),
        ConsensusBroadcastDisposition::SourceRetained,
        "a full same-class corridor must not masquerade as Proposal acceptance"
    );
    {
        let pending = service
            .lock_pending_exact_output()
            .expect("inspect atomic Proposal rejection");
        assert_eq!(
            pending.fanouts.len(),
            1,
            "capacity pressure must retain only the pre-existing owner"
        );
        assert!(pending.fanouts.iter().all(|fanout| !matches!(
            &fanout.rollover_claim,
            ExactOutputRolloverClaim::PayloadChunks { .. }
        )));
    }
    assert!(
        !service.fast_path_proposals.contains(&proposal.round),
        "failed aggregate admission cannot consume the first-send marker"
    );
    assert!(!service.output_guard.restart_required());

    service.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    assert!(
        !service
            .retry_pending_exact_output()
            .expect("network recovery drains the previously accepted exact suffix")
    );
    assert_eq!(
        service
            .broadcast_consensus(message)
            .expect("the retained Proposal source retries after corridor recovery"),
        ConsensusBroadcastDisposition::ExactServiceAccepted
    );
    assert!(
        !service
            .has_pending_exact_output()
            .expect("accepted retransmission drains immediately")
    );
    assert!(service.fast_path_proposals.contains(&proposal.round));
    assert!(!service.output_guard.restart_required());
}

#[test]
fn certified_view_transition_resets_fast_path_before_new_set_a_fanout() {
    let (mut service, keys) = fixture_with_block_payload();
    let old_round = wire::ConsensusRound {
        context_id: service.context.id(),
        height: service.context.height,
        view: service.active_tag.view(),
    };
    assert!(service.fast_path_proposals.insert(old_round));
    let new_tag = EventTag::new(
        service.active_tag.height(),
        service.active_tag.view() + 1,
        Generation::new(service.active_tag.generation().get() + 1),
    );
    service
        .entered_view(
            new_tag,
            timeout_certificate_at_view(&service, old_round.view),
        )
        .expect("install certified successor view");
    assert!(service.fast_path_proposals.is_empty());

    let (_, payload) =
        proposal_body_and_payload_at_view(&service.context, &keys, new_tag.view());
    let manifest = payload.manifest().clone();
    let proposal = wire::Proposal {
        round: manifest.round,
        proposer: service.context.leader(manifest.round.view),
        subject: manifest.subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::Timeout(wire::TimeoutJustification {
            timeout_certificate: timeout_certificate_at_view(&service, old_round.view),
            highest_prepare_qc: None,
        }),
        signature: vec![0xA5; 48],
    };
    set_local_validator(&mut service, &keys, proposal.proposer);
    service
        .register_outbound_payload(new_tag, payload)
        .expect("retain new-view proposal chunks");
    let committee = service
        .committee_for_round(proposal.round)
        .expect("project new-view committee");
    let expected_set_a = service
        .remote_voters_for_indices(committee.set_a())
        .expect("resolve new-view Set A")
        .into_iter()
        .collect::<BTreeSet<_>>();
    let expected_all = service.remote_voters().into_iter().collect::<BTreeSet<_>>();
    assert_ne!(expected_set_a, expected_all);
    let observations = install_consensus_route_observer(&mut service);

    service
        .broadcast_consensus(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal.clone()),
        ))
        .expect("broadcast first proposal in certified successor view");

    let routed = take_consensus_route_observations(&observations);
    assert_eq!(chunk_route_targets(&routed, &manifest), expected_set_a);
    assert_eq!(
        proposal_route_targets(&routed, proposal.round, &manifest),
        expected_all
    );
    assert_eq!(
        service.fast_path_proposals,
        BTreeSet::from([proposal.round])
    );
}

fn install_temporary_chunk_root(service: &mut ProductionV2Services) -> TempDir {
    let directory = TempDir::new().expect("temporary chunk root");
    service.chunk_root = directory.path().to_path_buf();
    directory
}

fn certified_fetch_task(
    service: &ProductionV2Services,
    id: u64,
    tag: EventTag,
    manifest: Option<wire::PayloadManifest>,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> BodyFetchTask {
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"fetch fixture parent state"),
            Hash::new(b"fetch fixture post state"),
            Hash::new(b"fetch fixture writes"),
            1,
            Hash::new(b"fetch fixture block"),
        ),
        signers: vec![0],
        aggregate_signature: vec![1],
    };
    let request = wire::CertifiedBodyRequest {
        round,
        subject,
        certificate,
        requester: service.local_peer.clone(),
        signature: vec![1],
    };
    BodyFetchTask::certified_for_test(
        id,
        tag,
        manifest,
        vec![service.local_peer.clone()],
        request,
    )
}

#[test]
fn certified_fetch_fans_out_to_every_frozen_roster_archive() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let (_, _, proposal) = proposal_body_and_payload(&service.context, &keys);
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let request =
        certified_fetch_task(&service, 62, tag, None, proposal.round, proposal.subject)
            .certified_request()
            .expect("fixture certified request")
            .clone();
    assert_eq!(request.certificate.signers, vec![0]);
    let sources = service
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let expected_targets = sources
        .iter()
        .filter(|peer| *peer != &service.local_peer)
        .cloned()
        .collect::<Vec<_>>();
    let task = BodyFetchTask::certified_for_test(62, tag, None, sources.clone(), request);
    let admitted = Arc::new(Mutex::new(Vec::new()));
    let admitted_for_hook = Arc::clone(&admitted);
    service.set_exact_output_admission_hook(move |post, ticket| {
        assert!(ticket.is_none());
        let NetworkMessage::SumeragiBlock(envelope) = &post.data else {
            panic!("certified archive fanout emitted a non-Sumeragi message")
        };
        assert!(matches!(
            envelope.as_message(),
            BlockMessage::V2(message)
                if matches!(
                    &message.payload,
                    wire::ConsensusMessageV2Payload::CertifiedBodyRequest(_)
                )
        ));
        admitted_for_hook
            .lock()
            .expect("record frozen-roster archive target")
            .push(post.peer_id);
        Ok(())
    });

    service
        .enqueue_body_fetch(task.clone())
        .expect("fan out one certified request to every remote archive");

    assert_eq!(task.sources(), sources.as_slice());
    assert_eq!(
        admitted
            .lock()
            .expect("inspect frozen-roster archive targets")
            .as_slice(),
        expected_targets.as_slice()
    );
    assert_eq!(
        expected_targets,
        service.context.roster[1..]
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<Vec<_>>(),
        "every remote fixture archive is intentionally outside the one-signer QC"
    );
}

#[test]
fn replayed_proposal_signature_restores_exact_durable_payload() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    let directory = TempDir::new().expect("temporary body store");
    let mut body_store =
        V2BodyStore::open(directory.path(), service.context.clone()).expect("open body store");
    let (canonical_wire, payload, proposal) =
        proposal_body_and_payload(&service.context, &keys);
    let _receipt = body_store
        .store(payload.manifest().clone(), canonical_wire)
        .expect("store exact proposal body");
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    let proposer = usize::try_from(proposal.proposer).expect("fixture proposer index");
    let task =
        ConsensusSignTask::for_test(7, tag, super::super::v2::SignRequest::Proposal(proposal));
    let expected_work_id = task.id();
    let completion =
        sign_consensus_task(&body_store, &service.context, &keys[proposer], task, true)
            .expect("sign replayed proposal");

    let V2IoCompletion::Signature {
        work_id,
        signature,
        outbound_payload: Some(restored),
    } = completion
    else {
        panic!("proposal replay must restore its outbound payload");
    };
    assert_eq!(work_id, expected_work_id);
    assert!(!signature.is_empty());
    assert_eq!(restored, payload);
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
    let (canonical_wire, payload, proposal) =
        proposal_body_and_payload(&service.context, &keys);
    body_store
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
    let target = LifecycleIngressIoTargetSeal::for_recovered_decision_fetch_test(
        &service.context,
        key,
        23,
    );
    let task =
        RecoveredDecisionFetchBodyPersistenceTaskV1::for_test(&target, key, authenticated);
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
