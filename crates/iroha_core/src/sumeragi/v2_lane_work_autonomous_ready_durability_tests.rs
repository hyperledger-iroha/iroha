#[test]
fn autonomous_ready_crosses_payload_and_certificate_durability_before_commit_vote() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, proposal) = planned_lane_candidate_block_at_view(&adapter, &keys, 0);
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("planned autonomous entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        AutonomousAuthorRule::Autonomous,
        b"autonomous-ready-queue-plan-admission-binding",
        b"autonomous-ready-reservation-owner",
        "deterministic autonomous producer",
        "producer key",
        "signed autonomous payload",
    );
    assert_ne!(
        adapter.local_peer, producer,
        "the receive-side READY fixture must not impersonate its authenticated producer"
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none(),
        "an unprotected global carrier must not make payload bytes durable"
    );
    let availability_body = lane_payload_availability_body(
        &payload,
        &proposal,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("derive exact READY body");
    assert!(matches!(
        adapter.kura.mint_lane_ready_authorization(
            &payload,
            &proposal,
            &availability_body,
            &adapter.local_peer,
            adapter.context.id(),
        ),
        Err("READY execution input is not durably readable")
    ));
    let (locked_round, _locked_subject) = mark_global_body_locked_for_block(&mut adapter, &block);
    let protected_hint = proposal
        .payload_block_hint
        .expect("autonomous proposal carries its candidate binding");
    adapter
        .locally_bound_lane_proposals
        .insert(proposal.proposal_hash, protected_hint);
    assert!(adapter.proposal_body_available(&proposal));
    // Model the live late-ingress race: the ingress precheck observed the
    // protected carrier, then a higher global transition retired that
    // in-memory witness before the durable READY boundary ran.
    adapter.locally_bound_lane_proposals.clear();
    assert_eq!(
        adapter.persist_and_authorize_autonomous_payload(&payload, &proposal),
        Ok(AutonomousPayloadDurabilityOutcome::DeferredUntilCarrierProtection),
        "lost carrier protection is a retryable deferral, not a durability failure"
    );
    assert!(
        !adapter.output_guard.restart_required(),
        "a late payload for an unprotected carrier must not stop consensus output"
    );
    assert!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none(),
        "deferred payload bytes must remain outside the durable READY boundary"
    );
    assert_ne!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected
    );
    let session_key = crate::lane_consensus::LaneBlockSessionKey {
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        proposal_hash: proposal.proposal_hash,
    };
    let local_prepare = adapter
        .lane_sessions
        .get(&session_key)
        .and_then(|session| session.prepare_votes.get(&adapter.local_peer))
        .expect("protected durable payload produces a local READY vote");
    assert!(local_prepare.payload_availability_vote.is_some());
    let durable_payload = autonomous_artifact(&adapter, &proposal, adapter.context.epoch)
        .expect("protected payload is durable before READY");
    assert_eq!(durable_payload.executable_payload, payload);
    assert!(durable_payload.availability_certificate.is_none());
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_some(),
        "READY authorization must follow durable execution input"
    );
    assert!(
        !adapter
            .lane_ready_authorizations
            .contains_key(&V2LaneWorkAdapter::lane_ready_session_key(&proposal)),
        "the local READY signer must consume its move-only authorization"
    );
    assert!(
        adapter
            .sign_lane_vote(&proposal, CertPhase::Prepare)
            .is_none(),
        "an authorized in-memory READY body cannot replay without a fresh durable capability"
    );
    let validator_set_pops = proposal
        .descriptor
        .validator_set
        .iter()
        .map(|validator| {
            let key = keys
                .iter()
                .find(|candidate| candidate.public_key() == validator.public_key())
                .expect("fixture validator key");
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("fixture validator proof of possession")
        })
        .collect::<Vec<_>>();
    let local_key = keys
        .iter()
        .find(|key| key.public_key() == adapter.local_peer.public_key())
        .expect("local READY key");
    let mut stale_incarnation = proposal.clone();
    stale_incarnation.descriptor.lane_incarnation = Hash::new(b"stale-ready-lane-incarnation");
    let mut proposal_drift = proposal.clone();
    proposal_drift.proposal_hash = Hash::new(b"drifted-ready-proposal");
    let wrong_signer_key = keys
        .iter()
        .find(|key| key.public_key() != adapter.local_peer.public_key())
        .expect("different committee signer");
    let wrong_signer = PeerId::new(wrong_signer_key.public_key().clone());
    let wrong_context_id = wire::HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"wrong-ready-height-context",
    )));
    for (candidate, signer, signing_key, context_id, authority_expect) in [
        (
            &stale_incarnation,
            adapter.local_peer.clone(),
            local_key.private_key(),
            adapter.context.id(),
            "exact durable input remints restart authority",
        ),
        (
            &proposal_drift,
            adapter.local_peer.clone(),
            local_key.private_key(),
            adapter.context.id(),
            "exact proposal authority",
        ),
        (
            &proposal,
            wrong_signer,
            wrong_signer_key.private_key(),
            adapter.context.id(),
            "exact signer authority",
        ),
        (
            &proposal,
            adapter.local_peer.clone(),
            local_key.private_key(),
            wrong_context_id,
            "exact session authority",
        ),
    ] {
        let authorization = adapter
            .kura
            .mint_lane_ready_authorization(
                &payload,
                &proposal,
                &availability_body,
                &adapter.local_peer,
                adapter.context.id(),
            )
            .expect(authority_expect);
        assert_eq!(
            LanePayloadAvailabilityVoteV1::new_signed_with_authorization(
                authorization,
                candidate,
                availability_body.clone(),
                signer,
                validator_set_pops.clone(),
                signing_key,
                context_id,
            ),
            Err(LaneAutonomousArtifactError::AvailabilityAuthorizationMismatch)
        );
    }
    let recovered_authorization = adapter
        .kura
        .mint_lane_ready_authorization(
            &payload,
            &proposal,
            &availability_body,
            &adapter.local_peer,
            adapter.context.id(),
        )
        .expect("restart recovery remints exact durable READY authority");
    let recovered_vote = LanePayloadAvailabilityVoteV1::new_signed_with_authorization(
        recovered_authorization,
        &proposal,
        availability_body.clone(),
        adapter.local_peer.clone(),
        validator_set_pops,
        local_key.private_key(),
        adapter.context.id(),
    )
    .expect("recovered exact authority signs the same READY body");
    assert_eq!(recovered_vote.body, availability_body);
    assert_eq!(recovered_vote.signer, adapter.local_peer);
    let ready_authority_rejected = |candidate| {
        adapter
            .kura
            .mint_lane_ready_authorization(
                candidate,
                &proposal,
                &availability_body,
                &adapter.local_peer,
                adapter.context.id(),
            )
            .is_err()
    };
    let mut payload_drift = payload.clone();
    payload_drift.payload_hash = Hash::new(b"drifted-ready-payload");
    assert!(
        ready_authority_rejected(&payload_drift),
        "payload drift must not mint READY authority"
    );
    let mut reservation_drift = payload.clone();
    reservation_drift.reservation_keys[0].reservation_owner_hash =
        Hash::new(b"drifted-ready-reservation-group");
    assert!(
        ready_authority_rejected(&reservation_drift),
        "reservation-group drift must not mint READY authority"
    );
    let _ = adapter.drain_effects(usize::MAX);
    adapter
        .schedule_lane_artifact_retransmissions()
        .expect("lane artifact retransmission should remain authorized");
    let retransmitted_payload_peers = adapter
        .drain_effects(usize::MAX)
        .into_iter()
        .filter_map(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock {
                peer,
                message: BlockMessage::LaneExecutablePayload(candidate),
            } if candidate == payload => Some(peer),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert!(
        retransmitted_payload_peers.is_empty(),
        "a receiving durable holder must not impersonate the producer during payload retransmission"
    );
    let remote_keys = keys
        .iter()
        .filter(|key| key.public_key() != adapter.local_peer.public_key())
        .collect::<Vec<_>>();
    let compatibility_vote = signed_lane_vote(&proposal, CertPhase::Prepare, remote_keys[2]);
    assert_eq!(
        adapter.lane_sessions.insert_vote(compatibility_vote, None),
        Err(crate::lane_consensus::LaneBlockSessionError::AvailabilityMismatch),
        "compatibility Prepare votes cannot mix with an authorized autonomous session"
    );
    for key in remote_keys.iter().take(2) {
        let vote = signed_autonomous_prepare_vote(&proposal, &payload, key, &keys);
        adapter
            .lane_sessions
            .insert_vote(vote, None)
            .expect("cache matching READY vote");
    }
    let prepare_qc = adapter
        .lane_sessions
        .get(&session_key)
        .and_then(|session| session.prepare_qc.clone())
        .expect("READY quorum seals autonomous PrepareQC");
    assert!(prepare_qc.payload_availability_qc.is_some());
    assert!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch)
            .is_some_and(|artifact| artifact.availability_certificate.is_none()),
        "quorum formation alone is not a durability boundary"
    );
    adapter.drive_lane_sessions();
    let session = adapter
        .lane_sessions
        .get(&session_key)
        .expect("autonomous session remains cached");
    assert!(
        session
            .commit_votes
            .get(&adapter.local_peer)
            .is_some_and(|vote| vote.payload_availability_vote.is_none()),
        "Commit vote follows durable READY publication and carries no second READY vote"
    );
    assert_eq!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch)
            .and_then(|artifact| artifact.availability_certificate),
        Some(DurableLanePayloadAvailabilityCertificateV1 {
            certificate: prepare_qc,
        })
    );
    // Materialize the exact certified source consumed by canonical merge
    // construction without advancing the live adapter's Commit session.
    // The terminal receipt helper must carry a production-valid
    // autonomous merge bundle, not synthetic payload bytes.
    let mut commit_votes = vec![
        adapter
            .lane_sessions
            .get(&session_key)
            .and_then(|session| session.commit_votes.get(&adapter.local_peer))
            .cloned()
            .expect("local Commit vote follows durable READY"),
    ];
    commit_votes.extend(
        remote_keys
            .iter()
            .take(2)
            .map(|key| signed_lane_vote(&proposal, CertPhase::Commit, key)),
    );
    let commit_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        commit_votes[0].body.clone(),
        proposal.descriptor.validator_set.clone(),
        &commit_votes,
    )
    .expect("terminal receipt fixture has a Commit quorum");
    let certified_session = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: adapter
            .lane_sessions
            .get(&session_key)
            .and_then(|session| session.prepare_qc.clone())
            .expect("terminal receipt fixture retains PrepareQC"),
        commit_qc,
    };
    let certified_pops = autonomous_lane_session_signer_pops(&certified_session)
        .expect("terminal receipt fixture validates autonomous signer PoPs")
        .expect("terminal receipt fixture has autonomous READY authority");
    adapter
        .kura
        .persist_committed_lane_block_session(&certified_session, &certified_pops)
        .expect("persist terminal receipt certified merge source");
    let terminal_receipt =
        crate::kura::tests::persist_merge_application_receipt_for_autonomous_payload_for_test(
            adapter.kura.as_ref(),
            &payload,
        );
    assert_eq!(
        terminal_receipt.format,
        LaneBlockApplicationReceiptArtifactFormat::MergeExecution,
        "the regression must exercise the terminal merge-frontier boundary"
    );
    assert!(
        adapter
            .kura
            .lane_block_application_receipt_available(&proposal),
        "terminal replay requires the exact durable application receipt"
    );
    let retained_artifact = autonomous_artifact(&adapter, &proposal, adapter.context.epoch)
        .expect("terminal authority retains the autonomous lifecycle attempt");
    assert_eq!(retained_artifact.executable_payload, payload);
    assert!(
        adapter.kura.lane_block_execution_input_available(&proposal),
        "terminal authority retains execution input until bounded indexed-history compaction"
    );
    let key = AutonomousLanePayloadKey::from(&proposal);
    let terminal_new_view_body = crate::lane_consensus::LaneBlockNewViewBodyV1::for_transition(
        &proposal,
        &payload,
        1,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("derive exact terminal NewView body");
    assert!(
        !adapter
            .autonomous_new_view_transition_is_current(&terminal_new_view_body, locked_round.view,),
        "an exact receipt must close the central NewView transition predicate"
    );
    let terminal_new_view_vote = crate::lane_consensus::LaneBlockNewViewVoteV1::new_signed(
        terminal_new_view_body,
        adapter.local_peer.clone(),
        local_key.private_key(),
    )
    .expect("sign cached terminal NewView vote");
    let mut terminal_votes = adapter.autonomous_new_view_votes.clone();
    let _ = terminal_votes
        .insert_and_maybe_seal(terminal_new_view_vote, &proposal.descriptor.validator_set)
        .expect("cache a valid pre-terminal NewView vote");
    adapter.autonomous_new_view_votes = terminal_votes;
    let _ = adapter.drain_effects(usize::MAX);
    adapter
        .schedule_lane_artifact_retransmissions()
        .expect("terminal NewView retransmission scan must remain valid");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockNewViewVote(vote),
                    ..
                } if vote.body.locked_proposal_hash == proposal.proposal_hash
            )),
        "a cached NewView vote must not retransmit after exact application"
    );
    assert_eq!(
        adapter.bind_locked_global_body(&block),
        V2LaneIngressOutcome::Rejected,
        "a locked carrier that still names an already applied lane proposal is stale"
    );
    assert!(
        !adapter.output_guard.restart_required(),
        "rejecting the stale locked carrier must not close consensus output"
    );
    assert_eq!(
        adapter.persist_and_authorize_autonomous_payload(&payload, &proposal),
        Ok(AutonomousPayloadDurabilityOutcome::AlreadyTerminalApplication),
        "the exact receipt must stop READY authorization before volatile state cleanup"
    );
    adapter.autonomous_new_view_started_at.insert(
        key,
        (
            proposal.descriptor.lane_block_view,
            Instant::now() - Duration::from_secs(1),
        ),
    );
    adapter
        .schedule_autonomous_new_view_timeouts(
            Instant::now(),
            locked_round.view,
            Duration::from_millis(1),
        )
        .expect("terminal NewView timeout must be discarded without a fail-stop");
    assert!(
        !adapter
            .pending_autonomous_anchor_payloads
            .contains_key(&key)
            && !adapter.autonomous_payloads.contains_key(&key)
            && !adapter.autonomous_payload_views.contains_key(&key)
            && !adapter.autonomous_new_view_started_at.contains_key(&key),
        "terminal cleanup must remove volatile payload and NewView state"
    );
    let restore_error = AutonomousPayloadDurabilityError::MissingLaneArtifact(
        "synthetic terminal missing-evidence race".to_owned(),
    );
    assert!(matches!(
        &restore_error,
        AutonomousPayloadDurabilityError::MissingLaneArtifact(_)
    ));
    assert!(
        adapter.missing_autonomous_artifact_became_terminal(&proposal, &restore_error),
        "a terminal missing-evidence race must downgrade only exact missing terminal evidence"
    );
    assert_eq!(
        adapter.persist_and_authorize_autonomous_payload(&payload, &proposal),
        Ok(AutonomousPayloadDurabilityOutcome::AlreadyTerminalApplication),
        "an exact receipt must stop READY authorization even when durable payload evidence remains"
    );
    assert!(
        !adapter.proposal_can_progress(&proposal),
        "an exact application receipt must terminally gate the retained lane session"
    );
    assert!(
        adapter.lane_sessions.get(&session_key).is_some(),
        "terminal gating preserves retained votes, QCs, and commit-lock evidence"
    );
    let _ = adapter.drain_effects(usize::MAX);
    adapter.drive_lane_sessions();
    adapter
        .schedule_lane_artifact_retransmissions()
        .expect("terminal retransmission scan must remain valid");
    let terminal_effects = adapter.drain_effects(usize::MAX);
    assert!(
        terminal_effects.iter().all(|effect| match effect {
            V2LaneWorkEffect::PostLaneBlock { message, .. } => match message {
                BlockMessage::LaneExecutablePayload(candidate) => {
                    candidate.origin_proposal != proposal
                }
                BlockMessage::LaneBlockProposal(candidate) => candidate != &proposal,
                BlockMessage::LaneBlockVote(vote) => {
                    vote.body.proposal_hash != proposal.proposal_hash
                }
                BlockMessage::LaneBlockQc(qc) => {
                    qc.body.proposal_hash != proposal.proposal_hash
                }
                BlockMessage::LaneBlockNewViewVote(vote) => {
                    vote.body.locked_proposal_hash != proposal.proposal_hash
                }
                BlockMessage::LaneBlockNewViewCertificate(certificate) => {
                    certificate.body.locked_proposal_hash != proposal.proposal_hash
                }
                _ => true,
            },
            _ => true,
        }),
        "terminal sessions must not produce READY, Prepare, QC, or payload rebroadcast effects"
    );
    let mut pending_payload = payload.clone();
    pending_payload.origin_proposal.payload_block_hint = None;
    adapter
        .pending_autonomous_anchor_payloads
        .insert(key, pending_payload);
    adapter.discard_volatile_autonomous_payload(key);
    assert!(
        !adapter
            .pending_autonomous_anchor_payloads
            .contains_key(&key)
            && !adapter.autonomous_payloads.contains_key(&key)
    );
    let _ = adapter.drain_effects(usize::MAX);
    adapter
        .schedule_lane_artifact_retransmissions()
        .expect("terminal pending-anchor cleanup keeps retransmission scanning valid");
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneExecutablePayload(candidate),
                    ..
                } if candidate.origin_proposal == proposal
            ))
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            payload.producer.clone(),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Duplicate,
        "a valid replay with an exact terminal receipt is idempotent at the terminal boundary"
    );
    assert!(
        !adapter
            .pending_autonomous_anchor_payloads
            .contains_key(&key)
            && !adapter.autonomous_payloads.contains_key(&key),
        "terminal ingress must not resurrect volatile autonomous payload state"
    );
    assert!(
        !adapter.output_guard.restart_required(),
        "terminal autonomous ingress must not stop consensus output"
    );
}
#[test]
fn recovered_autonomous_certificate_repairs_ready_before_certified_publication() {
    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (block, proposal) = globally_anchored_lane_block_fixture(&adapter, &keys);
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous recovery fixture entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        AutonomousAuthorRule::Lane,
        b"autonomous-recovery-queue-plan-admission-binding",
        b"autonomous-recovery-reservation-owner",
        "deterministic autonomous recovery producer",
        "autonomous recovery producer key",
        "signed autonomous recovery payload",
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    adapter
        .kura
        .store_block(block.clone())
        .expect("persist autonomous recovery carrier");
    let proposal_block = block.canonical_resultless_proposal();
    let (_locked_round, _locked_subject) =
        mark_global_body_locked_for_block(&mut adapter, &proposal_block);
    assert_ne!(
        adapter.bind_locked_global_body(&proposal_block),
        V2LaneIngressOutcome::Rejected
    );
    let prepare_votes = keys[..3]
        .iter()
        .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &keys))
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("recovered READY votes form PrepareQC");
    let recovered = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: prepare_qc.clone(),
        commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
    };
    adapter.pending_committed_lanes.push_back(recovered);
    assert!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch)
            .is_some_and(|artifact| artifact.availability_certificate.is_none()),
        "direct certificate recovery starts before local READY publication"
    );
    adapter
        .persist_autonomous_prepare_availability(&proposal, &prepare_qc)
        .expect("persist the first READY proof before the certified session");
    assert!(
        certified_artifact(&adapter, &proposal).is_none(),
        "fixture must stop at the sidecar-only crash boundary"
    );
    let mut conflicting_availability_body =
        lane_payload_availability_body(&payload, &proposal, payload.network_id, payload.epoch)
            .expect("derive a valid conflicting READY body fixture");
    conflicting_availability_body.executable_payload_hash =
        Hash::new(b"conflicting sidecar-only executable payload");
    let conflicting_votes = keys[..3]
        .iter()
        .map(|key| {
            signed_autonomous_prepare_vote_for_body(
                &proposal,
                conflicting_availability_body.clone(),
                key,
                &keys,
            )
        })
        .collect::<Vec<_>>();
    let conflicting_prepare = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &conflicting_votes,
    )
    .expect("conflicting READY body forms a cryptographically valid QC");
    assert!(!lane_qcs_certify_same_decision(
        &prepare_qc,
        &conflicting_prepare
    ));
    assert!(
        adapter
            .persist_autonomous_prepare_availability(&proposal, &conflicting_prepare)
            .is_err(),
        "sidecar-only recovery must reject a different READY decision subject"
    );
    assert!(
        certified_artifact(&adapter, &proposal).is_none(),
        "a conflicting READY subject must not publish a certified session"
    );
    let committed = ValidBlock::committed_from_replay_signed_block(block.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let historical_epoch = adapter.context.epoch;
    let historical_height = adapter.context.height;
    install_finalized_vrf_epoch(&adapter, historical_epoch, historical_height);
    let current_context = adapter.context.clone();
    let mut sidecar_only_successor = successor_context_for_parent(&adapter, &block);
    sidecar_only_successor.epoch = {
        let world = adapter.state.world_view();
        crate::sumeragi::epoch_for_height_from_world(&world, sidecar_only_successor.height)
    };
    adapter.context = sidecar_only_successor;
    let conflicting_session = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: conflicting_prepare,
        commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
    };
    let sidecar_conflict = adapter
        .persist_historical_recovery_session(&conflicting_session)
        .expect_err("historical sidecar-only recovery must reject a different READY subject");
    assert!(matches!(
        sidecar_conflict,
        V2LaneWorkError::Persistence(message)
            if message.contains("durable READY decision subject")
    ));
    assert!(
        certified_artifact(&adapter, &proposal).is_none(),
        "historical sidecar conflict must not publish a certified session"
    );
    adapter.context = current_context;
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("repair READY and publish recovered autonomous certificate"),
        1
    );
    assert_eq!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch)
            .and_then(|artifact| artifact.availability_certificate),
        Some(DurableLanePayloadAvailabilityCertificateV1 {
            certificate: prepare_qc,
        }),
        "READY durability must be repaired before recovered certified publication"
    );
    let durable = certified_artifact(&adapter, &proposal)
        .expect("the exact recovered autonomous certificate becomes durable");
    assert!(
        !durable.signer_pops.contains_key(keys[3].public_key()),
        "the retained 3-of-4 certificate deliberately omits one committee PoP"
    );
    let alternative_commit = lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit);
    adapter.lane_sessions = LaneBlockSessionCache::new(1);
    {
        let mut world = adapter.state.world.block();
        for key in &keys {
            world
                .consensus_keys_by_pk
                .insert(key.public_key().to_string(), Vec::new());
        }
        world.commit();
    }
    assert_eq!(
        validate_lane_block_qc_aggregate(
            &alternative_commit,
            &adapter.pops_for_lane_qc(&alternative_commit),
        ),
        Ok(()),
        "standalone CommitQC must recover PoPs from durable READY after cache and State pruning"
    );
    assert!(
        durable_historical_lane_output_source_hash(
            adapter.kura.as_ref(),
            &BlockMessage::LaneBlockQc(alternative_commit),
        )
        .expect("validate alternate autonomous quorum against durable READY authority")
        .is_some(),
        "a different valid 3-of-4 QC must survive rollover without mutable State PoPs"
    );
    let alternative_prepare_votes = keys[1..]
        .iter()
        .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &keys))
        .collect::<Vec<_>>();
    let alternative_prepare = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &alternative_prepare_votes,
    )
    .expect("alternate READY quorum forms a PrepareQC");
    let replayed = CommittedLaneBlockSession {
        proposal: proposal.clone(),
        prepare_qc: alternative_prepare,
        commit_qc: lane_qc_for_phase(&proposal, &keys[1..], CertPhase::Commit),
    };
    assert_ne!(
        replayed.prepare_qc.signers_bitmap, durable.prepare_qc.signers_bitmap,
        "historical replay must use a different valid READY quorum subset"
    );
    assert!(lane_qcs_certify_same_decision(
        &durable.prepare_qc,
        &replayed.prepare_qc
    ));
    assert!(lane_qcs_certify_same_decision(
        &durable.commit_qc,
        &replayed.commit_qc
    ));
    let mut missing_ready = replayed.prepare_qc.clone();
    missing_ready.payload_availability_qc = None;
    assert!(
        !lane_qcs_certify_same_decision(&durable.prepare_qc, &missing_ready),
        "READY presence is part of the certified decision subject"
    );
    let mut different_payload = replayed.prepare_qc.clone();
    different_payload
        .payload_availability_qc
        .as_mut()
        .expect("alternate PrepareQC carries READY")
        .body
        .executable_payload_hash = Hash::new(b"different autonomous executable payload");
    assert!(
        !lane_qcs_certify_same_decision(&durable.prepare_qc, &different_payload),
        "the READY executable-payload hash is part of the certified decision subject"
    );
    adapter.pending_committed_lanes.push_back(replayed.clone());
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("current-height alternate READY proof reuses the durable subject proof"),
        1
    );
    let mut successor_context = successor_context_for_parent(&adapter, &block);
    successor_context.epoch = {
        let world = adapter.state.world_view();
        crate::sumeragi::epoch_for_height_from_world(&world, successor_context.height)
    };
    adapter.context = successor_context;
    assert!(proposal.descriptor.proposal_height < adapter.context.height);
    adapter.historical_recovery_sessions.push_back(replayed);
    assert!(matches!(
        adapter
            .service_next_historical_recovery()
            .expect("alternate historical READY proof reuses durable certificate bytes"),
        HistoricalRecoveryServiceOutcome::Complete(_)
    ));
    let retained_availability = autonomous_artifact(&adapter, &proposal, payload.epoch)
        .and_then(|artifact| artifact.availability_certificate)
        .expect("the first durable READY certificate remains available");
    assert_eq!(retained_availability.certificate, durable.prepare_qc);
    let retained_certificate = certified_artifact(&adapter, &proposal)
        .expect("the first autonomous lane certificate remains durable");
    assert_eq!(retained_certificate.prepare_qc, durable.prepare_qc);
    assert_eq!(retained_certificate.commit_qc, durable.commit_qc);
    assert!(!adapter.output_guard.restart_required());
}
