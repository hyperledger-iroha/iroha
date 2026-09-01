#[derive(Clone, Copy)]
struct AutonomousTestRouter {
    route: RoutingDecision,
}

impl crate::queue::LaneRouter for AutonomousTestRouter {
    fn try_route(
        &self,
        _transaction: &dyn crate::queue::TransactionRoutingView,
    ) -> Result<RoutingDecision, crate::queue::RoutingResolveError> {
        Ok(self.route)
    }
}

fn prepare_autonomous_test_lane(
    adapter: &mut V2LaneWorkAdapter,
    keys: &[KeyPair],
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
) {
    enable_multilane_nexus(adapter, keys, lane_id, dataspace_id);
}

fn autonomous_test_fixture(
    mode: wire::ConsensusMode,
    author: bool,
) -> (V2LaneWorkAdapter, Vec<KeyPair>) {
    let local_validator_index = if author { 0 } else { 1 };
    fixture_at_height_inner_with_kura_and_local_index(
        mode,
        9,
        true,
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY),
        Some(local_validator_index),
        true,
    )
}

fn assert_autonomous_test_role(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    author: bool,
) {
    let slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        lane_id,
        dataspace_id,
    )
    .expect("plan deterministic autonomous lane slot");
    assert!(
        keys.iter()
            .any(|key| key.public_key() == adapter.local_peer.public_key()),
        "the local fixture peer belongs to the frozen validator keys"
    );
    assert_eq!(
        adapter.local_peer == slot.author,
        author,
        "the fixture must bind its local peer before claiming the autonomous lifecycle generation"
    );
}

fn install_autonomous_test_queue(
    adapter: &mut V2LaneWorkAdapter,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    journal_path: &std::path::Path,
) -> Arc<Queue> {
    let queue = Arc::new(Queue::test_with_router_for_routes(
        iroha_config::parameters::actual::Queue::default(),
        &iroha_primitives::time::TimeSource::new_system(),
        Arc::new(AutonomousTestRouter {
            route: RoutingDecision::new(lane_id, dataspace_id),
        }),
        &[(lane_id, dataspace_id)],
    ));
    let manifests = adapter.state.lane_manifests.read().clone();
    queue.install_lane_manifests(&manifests);
    // Retain the explicit test router across the same generation check
    // performed before production reservation selection.
    queue.install_test_router_metadata_for_nexus(&adapter.state.nexus_snapshot());
    queue
        .install_lane_reservation_journal(journal_path, 1024 * 1024)
        .expect("install autonomous queue reservation journal");
    let plan_journal_path = journal_path.with_extension("plans.norito");
    queue
        .install_plan_journal(&plan_journal_path, 1024 * 1024, true)
        .expect("install autonomous queue plan journal");
    queue
        .replay_plan_journal(adapter.state.as_ref())
        .expect("replay autonomous queue plan journal");
    adapter
        .install_lane_drain_queue(Arc::clone(&queue))
        .expect("install autonomous production queue");
    queue
}

fn enqueue_autonomous_test_transactions(
    adapter: &V2LaneWorkAdapter,
    queue: &Queue,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    count: usize,
) -> Vec<TransactionEntrypoint> {
    (0..count)
        .map(|index| {
            let seed = u8::try_from(index)
                .expect("autonomous fixture index fits u8")
                .saturating_add(0xA0);
            let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                .expect("deterministic autonomous transaction key");
            let authority = AccountId::new(key.public_key().clone());
            if adapter
                .state
                .view()
                .world()
                .accounts()
                .get(&authority)
                .is_none()
            {
                let mut world = adapter.state.world.block();
                world.accounts.insert(
                    authority.clone(),
                    AccountValue::new(AccountDetails::default()),
                );
                world.commit();
            }
            let transaction = TransactionBuilder::new(
                adapter.context.network_id,
                AccountId::new(key.public_key().clone()),
                iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
            )
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            )
            .with_instructions([Log::new(
                Level::INFO,
                format!("autonomous lane fixture {index}"),
            )])
            .with_admission_intent(
                iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
            )
            .sign(key.private_key());
            let accepted =
                crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(transaction));
            let entrypoint = accepted.entrypoint().clone();
            let routing_plan = queue
                .route_plan_with_state(&accepted, adapter.state.as_ref())
                .expect("resolve autonomous fixture routing plan");
            assert_eq!(
                routing_plan.coordinator_route(),
                RoutingDecision::new(lane_id, dataspace_id),
                "the exact committed Nexus generation must retain the autonomous test router"
            );
            let admission_context = queue
                .plan_admission_context_with_state(adapter.state.as_ref(), &routing_plan)
                .expect("capture autonomous fixture admission context");
            let binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
                adapter.state.network_id_ref(),
                accepted.entrypoint(),
                &routing_plan,
                admission_context,
                queue.queue_plan_admission_timestamp_ms(),
            )
            .expect("build autonomous fixture global admission binding");
            queue
                .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                    accepted,
                    adapter.state.as_ref(),
                    routing_plan,
                    &binding,
                )
                .expect("durably enqueue globally bound autonomous lane transaction");
            install_autonomous_fixture_queue_plan_registry_value(adapter.state.as_ref(), &binding);
            entrypoint
        })
        .collect()
}

fn install_autonomous_fixture_queue_plan_registry_value(
    state: &State,
    binding: &crate::torii_proxy::QueuePlanAdmissionBindingV1,
) {
    state
        .install_queue_plan_pending_binding_for_test(binding)
        .expect("install complete autonomous fixture QueuePlan owner evidence");
}

fn autonomous_test_candidate_limits(
    max_transactions: usize,
    max_queue_scan: usize,
) -> CandidateLimits {
    autonomous_test_candidate_limits_with_payload(max_transactions, 4 * 1024 * 1024, max_queue_scan)
}

fn autonomous_test_candidate_limits_with_payload(
    max_transactions: usize,
    max_payload_bytes: usize,
    max_queue_scan: usize,
) -> CandidateLimits {
    CandidateLimits::new(
        NonZeroUsize::new(max_transactions).expect("non-zero transaction limit"),
        NonZeroUsize::new(max_payload_bytes).expect("non-zero payload limit"),
        NonZeroUsize::new(max_queue_scan).expect("non-zero queue scan limit"),
    )
    .expect("valid autonomous candidate limits")
}

fn autonomous_route_quota_for_test(
    adapter: &V2LaneWorkAdapter,
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    total: usize,
) -> usize {
    let routes = adapter
        .state
        .consensus_lane_routes_at_height(adapter.context.height)
        .into_keys()
        .collect::<Vec<_>>();
    let route_index = routes
        .iter()
        .position(|route| *route == (lane_id, dataspace_id))
        .expect("independent route is active");
    let rotation = usize::try_from(adapter.context.height.saturating_sub(1)).unwrap_or(usize::MAX)
        % routes.len();
    V2LaneWorkAdapter::autonomous_route_quota(total, routes.len(), route_index, rotation)
}

#[test]
fn autonomous_ready_crosses_payload_and_certificate_durability_before_commit_vote() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let (block, proposal) = planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("planned autonomous entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
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
            .expect("READY replay check should not fail")
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
fn commit_certified_autonomous_payload_replay_is_idempotent_before_application_receipt() {
    let (mut adapter, keys) =
        fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (source_block, mut unanchored_proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    unanchored_proposal.payload_block_hint = None;
    unanchored_proposal.proposal_hash = unanchored_proposal.computed_proposal_hash();
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("planned autonomous entrypoint");
    let (unanchored_payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &unanchored_proposal,
        entrypoint,
        b"autonomous-ready-queue-plan-admission-binding",
        b"autonomous-ready-reservation-owner",
        "deterministic autonomous producer",
        "producer key",
        "signed autonomous payload",
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(unanchored_payload.clone()),
            producer.clone(),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    let carrier = autonomous_carrier_block(&adapter, &keys, &unanchored_payload);
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist the locked autonomous carrier");
    let proposal_block = carrier.canonical_resultless_proposal();
    let (locked_round, locked_subject) =
        mark_global_body_locked_for_block(&mut adapter, &proposal_block);
    assert_ne!(
        adapter.bind_locked_global_body(&proposal_block),
        V2LaneIngressOutcome::Rejected
    );
    let payload = unanchored_payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: adapter.context.height,
                proposal_view: locked_round.view,
                proposal_block_hash: locked_subject.block_hash,
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach exact locked carrier hint");
    let proposal = payload.origin_proposal.clone();
    let prepare_votes = keys[..3]
        .iter()
        .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &keys))
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("three READY votes form the autonomous PrepareQC");
    assert_eq!(
        adapter.insert_lane_qc(prepare_qc, locked_round.view),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.insert_lane_qc(
            lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    let mut committed = adapter.lane_sessions.drain_committed_sessions_up_to(1);
    assert_eq!(
        committed.len(),
        1,
        "the PrepareQC and CommitQC must complete exactly one lane session"
    );
    let committed = committed.pop().expect("one completed lane session");
    adapter
        .committed_lane_outputs
        .push_back(PendingCommittedLaneOutput {
            session: committed.clone(),
            next_validator: 0,
        });
    adapter.pending_committed_lanes.push_back(committed);
    assert_eq!(
        adapter
            .lane_sessions
            .retain_sessions_for_admissible_lanes(|_, _, _, _, _| false),
        1,
        "the regression evicts the drained live-session cache entry"
    );
    assert!(
        !adapter.lane_sessions.contains_proposal(&proposal),
        "CommitQC handoff must drain the mutable lane session"
    );
    assert!(
        adapter
            .pending_committed_lanes
            .iter()
            .any(|session| session.proposal == proposal),
        "the exact certified session must remain retained until global application"
    );
    assert!(
        !adapter
            .kura
            .lane_block_application_receipt_available(&proposal),
        "the regression must stop in the CommitQC-before-application window"
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer.clone(),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Duplicate,
        "a byte-identical payload replay is already certified by the retained READY quorum"
    );
    adapter.pending_committed_lanes.clear();
    assert!(
        adapter
            .committed_lane_outputs
            .iter()
            .any(|output| output.session.proposal == proposal),
        "the exact-output corridor independently retains the certified session"
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload),
            producer,
            locked_round.view,
        ),
        V2LaneIngressOutcome::Duplicate,
        "the exact-output retained owner must classify the replay without mutable session state"
    );
    assert!(
        !adapter.output_guard.restart_required(),
        "an idempotent CommitQC-bound payload replay must not fail-stop consensus"
    );
}

#[test]
fn committee_payload_replay_defers_until_decided_carrier_recovery_binds_session() {
    let (mut adapter, keys) = fixture_at_height_inner_with_kura_and_local_index(
        wire::ConsensusMode::Permissioned,
        2,
        true,
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY),
        Some(0),
        true,
    );
    let (source_block, mut proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        b"direct-decision-queue-plan-admission-binding",
        b"direct-decision-reservation-owner",
        "deterministic autonomous producer",
        "autonomous producer key",
        "signed hint-free autonomous payload",
    );
    assert!(
        adapter.local_can_own_autonomous_payload(&proposal),
        "the regression requires a local lane-committee member"
    );
    let carrier = autonomous_carrier_block(&adapter, &keys, &payload);
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist canonical autonomous carrier");
    let (locked_round, decided) = global_lock_for_block(&adapter, &carrier);
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &carrier);
    let receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist exact canonical finality before receipt-bound recovery");
    let stale_lock = wire::BlockSubject {
        parent_block_hash: decided.parent_block_hash,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"pre-bind-replay-stale-local-lock",
        )),
        payload_hash: Hash::new(b"pre-bind-replay-stale-local-lock-payload"),
    };
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, stale_lock),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    adapter
        .retain_merge_sidecars_for_global_view(locked_round.view, Some(stale_lock), Some(decided))
        .expect("install direct same-view Decision");
    let committed = ValidBlock::committed_from_replay_signed_block(carrier.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let payload = payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: adapter.context.height,
                proposal_view: carrier.header().view_change_index(),
                proposal_block_hash: carrier.hash(),
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach exact canonical carrier hint");
    let proposal = payload.origin_proposal.clone();
    assert!(
        adapter
            .canonical_finalized_autonomous_payload_for_proposal(&proposal)
            .expect("validate finalized autonomous carrier")
            .is_some(),
        "public finality must already expose the exact carrier"
    );
    assert!(!adapter.lane_sessions.contains_proposal(&proposal));
    assert!(adapter.pending_committed_lanes.is_empty());
    assert!(adapter.committed_lane_outputs.is_empty());
    assert!(
        !adapter
            .kura
            .lane_block_application_receipt_available(&proposal)
    );
    assert!(autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none());
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_none()
    );
    assert!(
        adapter.proposal_body_available(&proposal),
        "public finality exposes the carrier body but not committee-local READY authority"
    );
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted,
        "the exact replay remains volatile until trusted canonical recovery binds the session"
    );
    assert!(!adapter.lane_sessions.contains_proposal(&proposal));
    assert!(autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none());
    assert!(adapter.lane_ready_authorizations.is_empty());
    assert!(!adapter.output_guard.restart_required());
    assert_ne!(
        adapter
            .recover_decided_canonical_lane_body(&receipt, &finality)
            .expect("recover exact receipt-authorized canonical carrier"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(adapter.lane_sessions.contains_proposal(&proposal));
    assert!(autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_some());
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_some(),
        "trusted recovery must persist execution input before READY"
    );
    assert!(!adapter.output_guard.restart_required());
}

#[test]
fn canonical_decision_rebinds_quarantined_higher_view_autonomous_payload() {
    let (mut adapter, keys) = fixture_at_height_inner_with_kura_and_local_index(
        wire::ConsensusMode::Permissioned,
        2,
        true,
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY),
        Some(0),
        true,
    );
    let (source_block, mut proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous entrypoint");
    let (hint_free, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        b"canonical-rebind-queue-plan-admission-binding",
        b"canonical-rebind-reservation-owner",
        "deterministic autonomous producer",
        "autonomous producer key",
        "signed hint-free autonomous payload",
    );
    assert!(adapter.local_can_own_autonomous_payload(&proposal));
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(hint_free.clone()),
            producer,
            0,
        ),
        V2LaneIngressOutcome::Inserted,
        "the committee must retain the producer-authenticated hint-free payload before binding"
    );

    let first_carrier = autonomous_carrier_block_at_view(&adapter, &keys, &hint_free, 0)
        .canonical_resultless_proposal();
    let (first_round, _) = mark_global_body_locked_for_block(&mut adapter, &first_carrier);
    assert_ne!(
        adapter.bind_locked_global_body(&first_carrier),
        V2LaneIngressOutcome::Rejected,
        "the first protected carrier must establish durable local custody"
    );
    let first_hint = LaneBlockProposalPayloadHintV1 {
        proposal_height: adapter.context.height,
        proposal_view: first_round.view,
        proposal_block_hash: first_carrier.hash(),
    };
    let first_anchored = hint_free
        .attach_global_hint_exact(
            first_hint,
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach first protected carrier hint");
    let lane_id = proposal.descriptor.lane_id;
    let lane_block_height = proposal.descriptor.lane_block_height;
    assert_eq!(
        adapter
            .kura
            .current_autonomous_lane_payload(
                lane_id,
                lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("recover first durable payload")
            .0,
        first_anchored
    );

    let canonical_view = first_round.view.saturating_add(1);
    let canonical_carrier =
        autonomous_carrier_block_at_view(&adapter, &keys, &hint_free, canonical_view);
    adapter
        .kura
        .store_block(canonical_carrier.clone())
        .expect("persist higher-view canonical carrier");
    let (canonical_round, decided) = global_lock_for_block(&adapter, &canonical_carrier);
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &canonical_carrier);
    let receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist higher-view canonical finality");
    let stale_lock = wire::BlockSubject {
        parent_block_hash: decided.parent_block_hash,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"canonical-rebind-stale-local-lock",
        )),
        payload_hash: Hash::new(b"canonical-rebind-stale-local-lock-payload"),
    };
    assert_eq!(
        adapter.mark_global_body_locked(canonical_round, stale_lock),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    assert_eq!(
        adapter
            .pending_autonomous_anchor_payloads
            .values()
            .next(),
        Some(&hint_free),
        "the higher-view lock must quarantine the old advisory hint"
    );
    assert!(
        adapter
            .kura
            .read_autonomous_lane_slot_retirement(
                lane_id,
                lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("read pre-Decision retirement state")
            .is_none()
    );
    adapter
        .retain_merge_sidecars_for_global_view(
            canonical_round.view,
            Some(stale_lock),
            Some(decided),
        )
        .expect("install higher-view canonical Decision");
    let committed = ValidBlock::committed_from_replay_signed_block(canonical_carrier.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let canonical_hint = LaneBlockProposalPayloadHintV1 {
        proposal_height: adapter.context.height,
        proposal_view: canonical_view,
        proposal_block_hash: canonical_carrier.hash(),
    };
    let canonical_payload = hint_free
        .attach_global_hint_exact(
            canonical_hint,
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach canonical higher-view hint");
    let canonical_proposal = canonical_payload.origin_proposal.clone();
    assert_eq!(canonical_payload.payload_hash, first_anchored.payload_hash);
    assert_eq!(
        canonical_payload.reservation_keys,
        first_anchored.reservation_keys
    );
    assert!(
        adapter
            .canonical_finalized_autonomous_payload_for_proposal(&canonical_proposal)
            .expect("validate higher-view finalized carrier")
            .is_some()
    );

    assert_ne!(
        adapter
            .recover_decided_canonical_lane_body(&receipt, &finality)
            .expect("repair custody from the exact canonical Decision"),
        V2LaneIngressOutcome::Rejected,
        "canonical repair must rebind the advisory hint without retiring the stable payload"
    );
    assert_eq!(
        adapter
            .kura
            .current_autonomous_lane_payload(
                lane_id,
                lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("recover canonically rebound payload")
            .0,
        canonical_payload
    );
    assert!(
        adapter
            .kura
            .read_autonomous_lane_slot_retirement(
                lane_id,
                lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("read post-repair retirement state")
            .is_none()
    );
    assert!(adapter.lane_sessions.contains_proposal(&canonical_proposal));
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(lane_id, lane_block_height)
            .is_some()
    );
    assert!(!adapter.output_guard.restart_required());
}

#[test]
fn voting_validator_outside_lane_committee_skips_private_new_view_cursor_restore() {
    let (mut adapter, keys) = fixture_with_durable_parent(wire::ConsensusMode::Permissioned);
    let (block, proposal) = planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("planned autonomous entrypoint");
    let (payload, _) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        b"outside-committee-queue-plan-admission-binding",
        b"outside-committee-reservation-owner",
        "deterministic autonomous producer",
        "autonomous producer key",
        "signed autonomous payload",
    );
    let (locked_round, _) = mark_global_body_locked_for_block(&mut adapter, &block);
    let protected_hint = proposal
        .payload_block_hint
        .expect("autonomous proposal carries its candidate binding");
    adapter
        .locally_bound_lane_proposals
        .insert(proposal.proposal_hash, protected_hint);
    assert!(adapter.proposal_body_available(&proposal));
    assert!(matches!(
        adapter.restore_autonomous_new_view_state(&payload, locked_round.view, Instant::now()),
        Err(AutonomousPayloadDurabilityError::MissingLaneArtifact(_))
    ));
    let key = AutonomousLanePayloadKey::from(&proposal);
    adapter
        .autonomous_new_view_started_at
        .insert(key, (proposal.descriptor.lane_block_view, Instant::now()));

    let outside_committee_key = KeyPair::try_from_seed(vec![0xF1; 32], Algorithm::BlsNormal)
        .expect("deterministic outside-committee BLS key");
    adapter.local_peer = PeerId::new(outside_committee_key.public_key().clone());
    assert!(adapter.voting_enabled);
    assert_eq!(
        adapter.local_peer.public_key().try_algorithm().ok(),
        Some(Algorithm::BlsNormal)
    );
    assert!(
        !proposal
            .descriptor
            .validator_set
            .contains(&adapter.local_peer)
    );
    assert!(!adapter.local_can_own_autonomous_payload(&proposal));
    assert!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none(),
        "a validator outside the lane committee must not own its private durable artifact"
    );

    assert_eq!(
        adapter.persist_and_authorize_autonomous_payload(&payload, &proposal),
        Ok(AutonomousPayloadDurabilityOutcome::Authorized),
        "a non-member still verifies the globally protected carrier without minting READY authority"
    );
    adapter
        .restore_autonomous_new_view_state(&payload, locked_round.view, Instant::now())
        .expect("a non-member has no committee-local NewView cursor to restore");

    assert!(
        !adapter.autonomous_new_view_started_at.contains_key(&key),
        "committee removal must clear any stale local timeout"
    );
    assert!(
        autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none(),
        "verification-only handling must not synthesize private committee custody"
    );
}

#[test]
fn voting_validator_outside_lane_committee_retires_transport_only_loser_without_queue() {
    let (mut adapter, global_keys) = fixture_at_height_inner_with_kura_and_local_index(
        wire::ConsensusMode::Permissioned,
        9,
        true,
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY),
        Some(3),
        true,
    );
    let extra_lane_key = KeyPair::try_from_seed(vec![0xF1; 32], Algorithm::BlsNormal)
        .expect("deterministic extra lane validator key");
    {
        let id = ConsensusKeyId::new(ConsensusKeyRole::Validator, "transport-only-lane-validator");
        let record = ConsensusKeyRecord {
            id: id.clone(),
            public_key: extra_lane_key.public_key().clone(),
            pop: Some(
                iroha_crypto::bls_normal_pop_prove(extra_lane_key.private_key())
                    .expect("extra lane validator proof of possession"),
            ),
            activation_height: 0,
            expiry_height: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        let mut world = adapter.state.world.block();
        world.consensus_keys.insert(id.clone(), record.clone());
        world
            .consensus_keys_by_pk
            .insert(record.public_key.to_string(), vec![id]);
        world.commit();
    }

    let mut lane_keys = global_keys
        .iter()
        .filter(|key| key.public_key() != adapter.local_peer.public_key())
        .cloned()
        .collect::<Vec<_>>();
    lane_keys.push(extra_lane_key);
    lane_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &lane_keys, lane_id, dataspace_id);

    let (source_block, mut proposal) = planned_autonomous_lane_candidate_block_for_route_at_view(
        &adapter,
        &global_keys,
        0,
        lane_id,
        dataspace_id,
    );
    proposal.payload_block_hint = None;
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("transport-only losing payload entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &lane_keys,
        &proposal,
        entrypoint,
        b"transport-only-loser-queue-plan-admission-binding",
        b"transport-only-loser-reservation-owner",
        "deterministic transport-only autonomous producer",
        "transport-only autonomous producer key",
        "signed transport-only autonomous payload",
    );
    let key = AutonomousLanePayloadKey::from(&proposal);
    assert!(adapter.voting_enabled);
    assert!(adapter.autonomous_lifecycle_process_generation.is_some());
    assert!(
        adapter
            .context
            .roster
            .iter()
            .any(|entry| entry.validator == adapter.local_peer)
    );
    assert_eq!(proposal.descriptor.validator_set.len(), 4);
    assert!(
        !proposal
            .descriptor
            .validator_set
            .contains(&adapter.local_peer)
    );
    assert!(!adapter.local_can_own_autonomous_payload(&proposal));
    assert!(adapter.lane_drain_queue.is_none());

    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            0,
        ),
        V2LaneIngressOutcome::Inserted,
        "a globally voting non-member must retain the authenticated transport payload"
    );
    assert_eq!(
        adapter.pending_autonomous_anchor_payloads.get(&key),
        Some(&payload)
    );
    assert!(autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none());

    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("empty-winner leader index");
    let winner = BlockBuilder::new(
        adapter
            .merge_carrier_context_header(0)
            .expect("transport-only empty-winner header"),
    )
    .build_with_signature(
        u64::try_from(leader_index).expect("leader index fits u64"),
        global_keys[leader_index].private_key(),
    )
    .canonical_resultless_proposal();
    let (_round, _subject) = mark_global_body_locked_for_block(&mut adapter, &winner);
    assert_eq!(
        adapter.pending_autonomous_anchor_payloads.get(&key),
        Some(&payload),
        "lock publication alone must retain the transport payload"
    );
    assert_ne!(
        adapter.bind_locked_global_body(&winner),
        V2LaneIngressOutcome::Rejected,
        "binding an empty winner must discard a non-member transport copy without private retirement"
    );

    assert!(
        !adapter
            .pending_autonomous_anchor_payloads
            .contains_key(&key)
    );
    assert!(adapter.lane_drain_queue.is_none());
    assert!(autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none());
    let descriptor = &proposal.descriptor;
    assert_eq!(
        adapter
            .kura
            .read_autonomous_lane_slot_retirement(
                descriptor.lane_id,
                descriptor.lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("read transport-only losing-slot retirement"),
        None,
        "a non-member must not mint committee-local retirement authority"
    );
    assert!(!adapter.output_guard.restart_required());
    assert!(adapter.output_guard.acquire().is_some());
}

#[test]
fn authorized_ready_session_without_one_shot_token_fails_stop() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Npos);
    let (block, proposal) = planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    let entrypoint = block
        .external_entrypoints_cloned()
        .next()
        .expect("planned autonomous entrypoint");
    let (payload, _) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        b"missing-ready-token-queue-plan-admission-binding",
        b"missing-ready-token-reservation-owner",
        "deterministic autonomous producer",
        "producer key",
        "signed autonomous payload",
    );
    let availability = lane_payload_availability_body(
        &payload,
        &proposal,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("derive exact READY body");
    assert_eq!(
        adapter
            .lane_sessions
            .insert_recovered_proposal_replacing_uncommitted_conflict(proposal.clone()),
        Ok(crate::lane_consensus::LaneBlockSessionInsertOutcome::Inserted)
    );
    adapter
        .lane_sessions
        .authorize_payload_availability(&proposal, availability)
        .expect("install the body which normally follows durable authorization");
    adapter.locally_bound_lane_proposals.insert(
        proposal.proposal_hash,
        proposal
            .payload_block_hint
            .expect("planned proposal carries its global-body hint"),
    );
    assert!(
        adapter
            .lane_sessions
            .local_prepare_vote_needed_for(&proposal, &adapter.local_peer),
        "the regression fixture must still require the local READY vote"
    );
    assert!(
        !adapter
            .lane_ready_authorizations
            .contains_key(&V2LaneWorkAdapter::lane_ready_session_key(&proposal)),
        "the regression fixture deliberately models the consumed-token failure window"
    );

    adapter.drive_lane_sessions();

    assert!(
        adapter.output_guard.restart_required(),
        "an authorized READY body without its one-shot signer token must fail-stop instead of hanging"
    );
}

#[test]
fn single_custom_lane_still_produces_autonomous_payload() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    let validators = enable_single_custom_lane_nexus(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("single-lane reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);

    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(1, 1))
        .expect("run single custom-lane producer tick");

    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .find(|payload| {
            payload.origin_proposal.descriptor.lane_id == lane_id
                && payload.origin_proposal.descriptor.dataspace_id == dataspace_id
        })
        .expect("enabled single custom-lane author publishes its durable payload");
    assert_eq!(payload.origin_proposal.payload_block_hint, None);
    assert_eq!(
        payload.origin_proposal.descriptor.validator_set, validators,
        "the payload must bind the custom lane authority, not the global roster mode"
    );
    assert_eq!(payload.producer, adapter.local_peer);
    assert_eq!(queue.live_lane_reservations(), payload.reservation_keys);
}

#[test]
fn autonomous_producer_retries_after_predecessor_application_receipt_arrives() {
    use super::super::lane_planner::AutonomousLaneReservationSlotPlanError;

    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    let kura =
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY);
    let (mut parent, keys) = fixture_at_height_inner_with_kura_and_local_index(
        wire::ConsensusMode::Permissioned,
        1,
        true,
        Arc::clone(&kura),
        None,
        false,
    );
    enable_single_custom_lane_nexus(&mut parent, &keys, lane_id, dataspace_id);
    let (mut predecessor_block, provisional_proposal) =
        planned_lane_candidate_block_for_route_at_view(&parent, &keys, 0, lane_id, dataspace_id);
    let predecessor_ownership = ownership_from_proposal(&provisional_proposal);
    let entrypoint_hashes = predecessor_block
        .external_entrypoints_cloned()
        .map(|entrypoint| entrypoint.hash())
        .collect::<Vec<_>>();
    assert_eq!(entrypoint_hashes.len(), 1);
    predecessor_block
        .set_transaction_results(
            Vec::new(),
            &entrypoint_hashes,
            vec![TransactionResultInner::Ok(DataTriggerSequence::default())],
        )
        .expect("attach the predecessor's canonical transaction result");
    let leader_index =
        usize::try_from(parent.context.leader(0)).expect("predecessor leader index fits usize");
    let signature = SignatureOf::try_from_hash(
        keys[leader_index].private_key(),
        predecessor_block.header().hash(),
    )
    .expect("sign the result-bearing predecessor");
    predecessor_block
        .replace_signatures(
            [BlockSignature::new(
                u64::try_from(leader_index).expect("predecessor leader index fits u64"),
                signature,
            )]
            .into_iter()
            .collect(),
        )
        .expect("replace the predecessor signature after attaching results");
    let predecessor_proposal =
        proposal_from_ownership(&predecessor_ownership, predecessor_block.hash())
            .expect("bind the exact result-bearing predecessor");
    parent
        .kura
        .store_block(predecessor_block.clone())
        .expect("persist the raw canonical predecessor");
    let committed_predecessor =
        ValidBlock::committed_from_replay_signed_block(predecessor_block.clone());
    commit_test_block_to_state(
        parent.state.as_ref(),
        &committed_predecessor,
        &parent.context,
    );
    assert_eq!(
        parent
            .state
            .unapplied_lane_block_artifact_heights_snapshot_cached()
            .get(&(lane_id, dataspace_id)),
        Some(&predecessor_proposal.descriptor.lane_block_height),
        "the raw predecessor must block a successor reservation until its exact receipt exists"
    );
    assert!(
        !parent
            .kura
            .lane_block_application_receipt_available(&predecessor_proposal)
    );

    let successor_context = successor_context_for_parent(&parent, &predecessor_block);
    let validator_set = parent
        .state
        .resolve_lane_committee_at_height(
            crate::state::LaneAuthorityRoute::new(lane_id, dataspace_id),
            successor_context.height,
        )
        .expect("successor lane committee remains active")
        .into_validators();
    let successor_lane_height = predecessor_proposal
        .descriptor
        .lane_block_height
        .checked_add(1)
        .expect("successor lane height fits u64");
    let producer = deterministic_lane_author(&validator_set, successor_lane_height)
        .expect("successor lane has a deterministic producer")
        .clone();
    let producer_key = keys
        .iter()
        .find(|key| key.public_key() == producer.public_key())
        .expect("fixture owns the successor producer key")
        .clone();
    let state = Arc::clone(&parent.state);
    let limits = parent.limits;
    drop(parent);
    let mut adapter = V2LaneWorkAdapter::new_with_output_guard(
        successor_context,
        producer.clone(),
        producer_key,
        true,
        Arc::clone(&state),
        Arc::clone(&kura),
        limits,
        None,
        None,
        ConsensusOutputGuard::isolated(),
    )
    .expect("open the autonomous successor while its predecessor remains unapplied");
    let journal_dir = tempfile::tempdir().expect("retry reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    let fifo_before = queue.fifo_snapshot_for_test();
    let route = (lane_id, dataspace_id);
    assert!(matches!(
        plan_autonomous_lane_reservation_slot(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &adapter.context,
            lane_id,
            dataspace_id,
        ),
        Err(AutonomousLaneReservationSlotPlanError::BlockedPredecessor {
            lane_id: blocked_lane,
            dataspace_id: blocked_dataspace,
        }) if blocked_lane == lane_id && blocked_dataspace == dataspace_id
    ));

    adapter.next_autonomous_producer_tick = Instant::now();
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(1, 1))
        .expect("the blocked producer tick remains non-fatal");
    assert!(
        !adapter
            .autonomous_production_attempted_routes
            .contains(&route),
        "a transient predecessor wait must not terminally suppress the route"
    );
    assert_eq!(queue.fifo_snapshot_for_test(), fifo_before);
    assert!(queue.live_lane_reservations().is_empty());
    assert!(adapter.pending_autonomous_anchor_payloads.is_empty());

    let predecessor_session = committed_lane_session(&predecessor_proposal, &keys);
    let predecessor_pops = adapter.pops_for_lane_session(&predecessor_session);
    adapter
        .kura
        .persist_committed_lane_block_session(&predecessor_session, &predecessor_pops)
        .expect("persist the predecessor's exact certificate");
    assert!(
        adapter
            .kura
            .persist_lane_block_application_receipt_if_ready(&predecessor_proposal)
            .expect("persist the predecessor's exact application receipt")
    );
    assert!(
        !adapter
            .state
            .unapplied_lane_block_artifact_heights_snapshot_cached()
            .contains_key(&route)
    );
    let recovered_slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        lane_id,
        dataspace_id,
    )
    .expect("the exact predecessor receipt unblocks reservation planning");
    assert_eq!(recovered_slot.lane_block_height, successor_lane_height);
    assert_eq!(recovered_slot.author, producer);

    adapter.next_autonomous_producer_tick = Instant::now();
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(1, 1))
        .expect("the next producer tick retries the recovered route");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .find(|payload| {
            payload.origin_proposal.descriptor.lane_id == lane_id
                && payload.origin_proposal.descriptor.dataspace_id == dataspace_id
        })
        .expect("the recovered route publishes its hint-free payload");
    assert_eq!(
        payload.origin_proposal.descriptor.lane_block_height,
        successor_lane_height
    );
    assert!(
        adapter
            .autonomous_production_attempted_routes
            .contains(&route)
    );
    assert_eq!(queue.live_lane_reservations(), payload.reservation_keys);
    assert!(queue.fifo_snapshot_for_test().is_empty());
    assert!(adapter.effects.iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneExecutablePayload(posted),
                ..
            } if posted.payload_hash == payload.payload_hash
        )
    }));
}

#[test]
fn autonomous_local_author_reserves_fifo_before_durable_hint_free_publication() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    let expected_entrypoints =
        enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 4);
    let original_fifo = queue.fifo_snapshot_for_test();
    assert_eq!(original_fifo.len(), expected_entrypoints.len());
    let exact_slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        lane_id,
        dataspace_id,
    )
    .expect("plan the exact producer Kura activation slot");
    let journal_len_before = std::fs::metadata(&journal_path)
        .expect("stat empty reservation journal")
        .len();
    let limits = autonomous_test_candidate_limits(6, 6);
    let expected_count = autonomous_route_quota_for_test(&adapter, lane_id, dataspace_id, 6);
    assert_eq!(expected_count, 3, "fixture gives the independent lane half");
    adapter
        .schedule_autonomous_lane_production(0, limits)
        .expect("run autonomous producer tick");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .find(|payload| {
            payload.origin_proposal.descriptor.lane_id == lane_id
                && payload.origin_proposal.descriptor.dataspace_id == dataspace_id
        })
        .expect("local producer publishes one pending autonomous payload")
        .clone();
    assert_eq!(payload.origin_proposal.payload_block_hint, None);
    assert!(lane_executable_payload_carries_lane(
        &payload,
        lane_id,
        dataspace_id,
        payload.origin_proposal.descriptor.lane_incarnation,
    ));
    assert!(
        !lane_executable_payload_carries_lane(
            &payload,
            lane_id,
            dataspace_id,
            Hash::new(b"recreated autonomous lane incarnation"),
        ),
        "incarnation-A payload ownership must not ABA-block incarnation B"
    );
    let mut misaligned_payload = payload.clone();
    misaligned_payload.native_amx_receipts.clear();
    assert!(
        lane_executable_payload_carries_lane(
            &misaligned_payload,
            lane_id,
            dataspace_id,
            Hash::new(b"recreated autonomous lane incarnation"),
        ),
        "malformed routing/receipt alignment must fail closed"
    );
    assert_eq!(
        payload.entrypoints,
        expected_entrypoints[..expected_count],
        "the deterministic author must retain exact FIFO order"
    );
    assert_eq!(payload.reservation_keys.len(), expected_count);
    assert_eq!(queue.live_lane_reservations().len(), expected_count);
    assert_eq!(
        queue.queued_len(),
        expected_entrypoints.len() - expected_count
    );
    assert!(
        std::fs::metadata(&journal_path)
            .expect("stat durable reservation journal")
            .len()
            > journal_len_before,
        "queue ownership must be appended durably before publication"
    );
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("derive the exact published lifecycle reservation group");
    let lifecycle_binding = AutonomousLifecycleAttemptBindingV1::from_payload(
        exact_slot.height_context_id,
        exact_slot.lane_block_height,
        &payload,
        reservation_group,
        &adapter.local_peer,
    )
    .expect("rederive the exact signed producer lifecycle binding");
    let process_generation = adapter
        .autonomous_lifecycle_process_generation
        .clone()
        .expect("validator fixture owns one process-lifetime lifecycle claim");
    let lifecycle_cursor_hash = {
        let lifecycle_read = adapter
            .kura
            .read_autonomous_lifecycle_cursor(&payload, &lifecycle_binding, &process_generation)
            .expect("read the durably completed producer lifecycle cursor");
        let lifecycle_cursor = lifecycle_read
            .cursor()
            .expect("producer bootstrap publishes its exact Live cursor");
        assert!(matches!(
            lifecycle_cursor.phase(),
            AutonomousLifecycleCursorPhaseV1::Live { .. }
        ));
        assert_eq!(
            lifecycle_cursor.owner_generation(),
            process_generation.generation()
        );
        lifecycle_cursor.cursor_hash()
    };
    assert!(
        adapter
            .kura
            .autonomous_lifecycle_bootstrap_recovery_inventory(
                &process_generation,
                lane_id,
                dataspace_id,
                payload.origin_proposal.descriptor.lane_incarnation,
            )
            .expect("inventory completed producer bootstraps")
            .is_empty(),
        "exact Live readback must precede synced bootstrap deletion"
    );
    let mut substituted_proposal = payload.origin_proposal.clone();
    substituted_proposal
        .descriptor
        .qc_mode_tag
        .push_str(":substituted");
    substituted_proposal.descriptor.descriptor_hash =
        substituted_proposal.descriptor.computed_descriptor_hash();
    substituted_proposal.proposal_hash = substituted_proposal.computed_proposal_hash();
    let substituted_payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        adapter.native_network_id(),
        adapter.context.epoch,
        substituted_proposal,
        payload.entrypoints.clone(),
        payload.reservation_keys.clone(),
        payload.routing_plans.clone(),
        payload.native_amx_receipts.clone(),
        adapter.local_peer.clone(),
        adapter.key_pair.private_key(),
    )
    .expect("construct an internally valid payload with a substituted slot identity");
    assert!(
        AutonomousLifecycleAttemptBindingV1::from_payload(
            exact_slot.height_context_id,
            exact_slot.lane_block_height,
            &substituted_payload,
            reservation_group,
            &adapter.local_peer,
        )
        .is_err(),
        "the signed lifecycle binding must reject a substituted proposal identity"
    );
    assert_eq!(queue.live_lane_reservations().len(), expected_count);
    assert_eq!(
        adapter
            .kura
            .read_autonomous_lane_block_artifact(
                lane_id,
                payload.origin_proposal.descriptor.lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .expect("read durable autonomous payload")
            .executable_payload,
        payload,
        "the published payload must already be recoverable from Kura"
    );
    let duplicate_effect = adapter
        .effects
        .iter()
        .find(|effect| {
            matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneExecutablePayload(published),
                    ..
                } if published == &payload
            )
        })
        .expect("fresh producer fanout retains one exact remote effect")
        .clone();
    let effect_count = adapter.effect_count();
    assert_eq!(
        adapter
            .push_effect_with_fresh_authorization(
                duplicate_effect,
                |_| -> Result<FirstReleaseServeLateBodyAuthorization, V2LaneWorkError> {
                    panic!("an exact queued duplicate must not mint another transition")
                },
            )
            .expect("duplicate preflight is infallible"),
        LaneWorkEffectInsertionOutcome::Duplicate
    );
    assert_eq!(
        adapter.effect_count(),
        effect_count,
        "an exact queued fanout duplicate is a transport stutter"
    );
    assert!(adapter.drain_effects(usize::MAX).iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneExecutablePayload(published),
                ..
            } if published == &payload
        )
    }));
    let exact_reservations = queue.live_lane_reservations();
    let queued_after_reservation = queue.queued_len();
    let context = adapter.context.clone();
    for _ in 0..3 {
        let recovery_work = (&mut adapter)
            .prepare(&context, 0, &[])
            .expect("non-empty retry carries durable autonomous ownership");
        assert_eq!(recovery_work.autonomous_lane_payloads.len(), 1);
        adapter.next_autonomous_producer_tick = Instant::now();
        adapter
            .schedule_autonomous_lane_production(0, limits)
            .expect("non-empty retry drives the idempotent lane producer");
        assert_eq!(
            queue.live_lane_reservations(),
            exact_reservations,
            "non-empty/global retry must neither release nor duplicate exact reservations"
        );
        assert_eq!(queue.queued_len(), queued_after_reservation);
    }
    let repeated_lifecycle_read = adapter
        .kura
        .read_autonomous_lifecycle_cursor(&payload, &lifecycle_binding, &process_generation)
        .expect("re-read lifecycle cursor after idempotent producer heartbeats");
    assert_eq!(
        repeated_lifecycle_read
            .cursor()
            .expect("idempotent heartbeat retains the Live lifecycle cursor")
            .cursor_hash(),
        lifecycle_cursor_hash,
        "duplicate producer heartbeats must not re-bootstrap or replace the Live cursor"
    );
    let mut wrong_context = context.clone();
    wrong_context.height = wrong_context.height.saturating_add(1);
    let wrong_context_error = adapter
        .prepare_certified_execution_carrier(&wrong_context, 0, &[])
        .expect_err("a certified execution carrier must reject another height context");
    assert!(wrong_context_error.indices().is_empty());
    assert_eq!(
        wrong_context_error.reason(),
        "certified execution carrier requires its exact height and an empty ordinary batch"
    );
    assert!(
        !adapter.output_guard.restart_required() && adapter.output_guard.acquire().is_some(),
        "context rejection must happen before a fail-stop operation can latch the output guard"
    );
    let ordinary_entrypoint = payload
        .entrypoints
        .first()
        .expect("autonomous payload contains a reserved entrypoint")
        .clone();
    let ordinary_transaction = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
        std::borrow::Cow::Owned(ordinary_entrypoint),
    );
    let ordinary_routing_plan = payload
        .routing_plans
        .first()
        .expect("autonomous payload contains a reserved routing plan")
        .clone();
    let ordinary_candidate =
        CandidateDescriptor::new(&ordinary_transaction, &ordinary_routing_plan);
    let nonempty_error = adapter
        .prepare_certified_execution_carrier(&context, 0, &[ordinary_candidate])
        .expect_err("a certified execution carrier must reject ordinary candidates");
    assert_eq!(nonempty_error.indices(), &BTreeSet::from([0]));
    assert_eq!(
        nonempty_error.reason(),
        "certified execution carrier requires its exact height and an empty ordinary batch"
    );
    assert!(
        !adapter.output_guard.restart_required() && adapter.output_guard.acquire().is_some(),
        "ordinary-candidate rejection must happen before a fail-stop operation can latch the output guard"
    );
    let execution_carrier_work = adapter
        .prepare_certified_execution_carrier(&context, 0, &[])
        .expect("certified execution carrier reserves an exact-empty lane-work surface");
    assert!(execution_carrier_work.native_amx_receipts.is_empty());
    assert!(execution_carrier_work.lane_payload_ownerships.is_empty());
    assert!(execution_carrier_work.autonomous_lane_payloads.is_empty());
    assert_eq!(
        adapter.pending_autonomous_anchor_payloads.len(),
        1,
        "dedicated carrier preparation must not release a durable losing slot before lock"
    );
    assert_eq!(
        queue.live_lane_reservations(),
        exact_reservations,
        "dedicated carrier preparation preserves Queue ownership until a winner is locked"
    );
    let leader_index =
        usize::try_from(adapter.context.leader(0)).expect("execution-carrier leader index");
    let carrier_header = adapter
        .merge_carrier_context_header(0)
        .expect("exact execution-carrier round header");
    let carrier = BlockBuilder::new(carrier_header)
        .build_with_signature(
            u64::try_from(leader_index).expect("leader index fits u64"),
            keys[leader_index].private_key(),
        )
        .canonical_resultless_proposal();
    let (_round, _subject) = mark_global_body_locked_for_block(&mut adapter, &carrier);
    assert_eq!(
        queue.live_lane_reservations(),
        exact_reservations,
        "lock publication alone cannot release pending durable ownership"
    );
    assert_ne!(
        adapter.bind_locked_global_body(&carrier),
        V2LaneIngressOutcome::Rejected,
        "the exact empty carrier body must authenticate losing-slot release"
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(
        queue.fifo_snapshot_for_test(),
        original_fifo,
        "losing multi-transaction reservations must return in their original global FIFO order"
    );
}
#[test]
fn remote_hint_free_loser_without_queue_reservation_binds_empty_winner() {
    for retain_ordinary_fifo_copy in [true, false] {
        let observer_disposition = if retain_ordinary_fifo_copy {
            "exact ordinary FIFO"
        } else {
            "strict absence"
        };
        let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, false);
        let lane_id = LaneId::new(1);
        let dataspace_id = DataSpaceId::new(7);
        prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
        assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, false);
        let journal_dir =
            tempfile::tempdir().expect("remote observer reservation journal directory");
        let journal_path = journal_dir.path().join("lane-reservations.norito");
        let queue =
            install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);

        let (source_block, mut proposal) =
            planned_autonomous_lane_candidate_block_for_route_at_view(
                &adapter,
                &keys,
                0,
                lane_id,
                dataspace_id,
            );
        proposal.payload_block_hint = None;
        proposal.proposal_hash = proposal.computed_proposal_hash();
        let entrypoint = source_block
            .external_entrypoints_cloned()
            .next()
            .expect("remote hint-free autonomous entrypoint");
        {
            let mut world = adapter.state.world.block();
            world.accounts.insert(
                entrypoint.authority().clone(),
                AccountValue::new(AccountDetails::default()),
            );
            world.commit();
        }
        let accepted = crate::tx::AcceptedTransaction::new_unchecked_entrypoint(
            std::borrow::Cow::Owned(entrypoint.clone()),
        );
        let routing_plan = queue
            .route_plan_with_state(&accepted, adapter.state.as_ref())
            .expect("resolve the remote observer routing plan");
        let admission_context = queue
            .plan_admission_context_with_state(adapter.state.as_ref(), &routing_plan)
            .expect("capture the remote observer admission context");
        let admission_binding = crate::torii_proxy::QueuePlanAdmissionBindingV1::new(
            adapter.state.network_id_ref(),
            accepted.entrypoint(),
            &routing_plan,
            admission_context,
            queue.queue_plan_admission_timestamp_ms(),
        )
        .expect("build the remote observer QueuePlan binding");
        if retain_ordinary_fifo_copy {
            queue
                .push_with_lane_with_state_and_routing_plan_strict_global_admission_claim(
                    accepted,
                    adapter.state.as_ref(),
                    routing_plan.clone(),
                    &admission_binding,
                )
                .expect("enqueue the exact ordinary FIFO replica");
            install_autonomous_fixture_queue_plan_registry_value(
                adapter.state.as_ref(),
                &admission_binding,
            );
        }
        let original_fifo = queue.fifo_snapshot_for_test();
        if retain_ordinary_fifo_copy {
            assert_eq!(
                original_fifo,
                vec![entrypoint.hash()],
                "the observer fixture must retain the exact ordinary FIFO copy"
            );
        } else {
            assert!(
                original_fifo.is_empty(),
                "the strict-absence observer fixture must begin without the entrypoint"
            );
        }
        assert!(
            queue.live_lane_reservations().is_empty(),
            "a remote observer must not impersonate the producer's Queue reservation"
        );

        let mut reservation = LaneQueueReservationKeyV1 {
            version: LaneQueueReservationKeyV1::VERSION,
            entrypoint_hash: entrypoint.hash(),
            queue_plan_admission_binding_hash: admission_binding.canonical_hash(),
            routing_plan_digest: routing_plan.digest(),
            coordinator_leg: routing_plan.coordinator_leg(),
            lane_id,
            dataspace_id,
            lane_incarnation: proposal.descriptor.lane_incarnation,
            proposal_height: proposal.descriptor.proposal_height,
            lane_block_height: proposal.descriptor.lane_block_height,
            lane_block_view: proposal.descriptor.lane_block_view,
            reservation_owner_hash: Hash::new(b"remote-observer-reservation-owner"),
            proposal_identity_hash: proposal.proposal_hash,
        };
        let producer = adapter
            .expected_autonomous_lane_author(&proposal)
            .expect("deterministic remote autonomous producer")
            .clone();
        bind_canonical_autonomous_reservation_identity(
            &adapter,
            &proposal,
            &producer,
            &mut reservation,
        );
        let producer_key = keys
            .iter()
            .find(|key| key.public_key() == producer.public_key())
            .expect("remote autonomous producer key");
        let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
            adapter.native_network_id(),
            adapter.context.epoch,
            proposal,
            vec![entrypoint],
            vec![reservation],
            vec![routing_plan],
            vec![None],
            producer.clone(),
            producer_key.private_key(),
        )
        .expect("signed remote hint-free autonomous payload");
        assert_ne!(
            adapter.local_peer, producer,
            "the receiving validator must not impersonate the payload producer"
        );
        assert_eq!(
            accept_lane_message_from(
                &mut adapter,
                BlockMessage::LaneExecutablePayload(payload.clone()),
                producer,
                0,
            ),
            V2LaneIngressOutcome::Inserted,
            "the authenticated remote hint-free payload must enter pending ownership"
        );
        assert_eq!(adapter.pending_autonomous_anchor_payloads.len(), 1);
        assert!(queue.live_lane_reservations().is_empty());

        let leader_index =
            usize::try_from(adapter.context.leader(0)).expect("empty-winner leader index");
        let winner_header = adapter
            .merge_carrier_context_header(0)
            .expect("exact empty-winner round header");
        let empty_winner = BlockBuilder::new(winner_header)
            .build_with_signature(
                u64::try_from(leader_index).expect("leader index fits u64"),
                keys[leader_index].private_key(),
            )
            .canonical_resultless_proposal();
        let (_round, _subject) = mark_global_body_locked_for_block(&mut adapter, &empty_winner);
        assert_eq!(
            adapter.pending_autonomous_anchor_payloads.len(),
            1,
            "lock publication alone must retain the remote losing payload"
        );
        assert_ne!(
            adapter.bind_locked_global_body(&empty_winner),
            V2LaneIngressOutcome::Rejected,
            "an empty winner must retire a non-Queue remote loser observed as {observer_disposition}"
        );
        assert!(
            !adapter.output_guard.restart_required() && adapter.output_guard.acquire().is_some(),
            "non-Queue losing retirement must not fail-stop the receiving validator"
        );
        assert!(adapter.pending_autonomous_anchor_payloads.is_empty());
        assert!(queue.live_lane_reservations().is_empty());
        assert_eq!(
            queue.fifo_snapshot_for_test(),
            original_fifo,
            "non-Queue retirement must preserve the {observer_disposition} disposition"
        );
        let descriptor = &payload.origin_proposal.descriptor;
        assert_eq!(
            adapter
                .kura
                .read_autonomous_lane_slot_retirement(
                    descriptor.lane_id,
                    descriptor.lane_block_height,
                    adapter.native_network_id(),
                    adapter.context.epoch,
                )
                .expect("read remote losing-slot retirement"),
            Some(crate::kura::AutonomousLaneSlotRetirementV1::from_payload(
                &payload
            )),
            "the receiver must durably retire the exact losing slot"
        );
        let terminal_outcome_path = adapter
            .kura
            .autonomous_lifecycle_terminal_outcome_path_for_test(
                descriptor.lane_id,
                descriptor.lane_block_height,
                descriptor.proposal_height,
            )
            .expect("derive the remote losing terminal-outcome path");
        assert!(
            terminal_outcome_path.is_file(),
            "remote losing retirement must publish exact terminal lifecycle evidence"
        );
        assert!(
            adapter
                .kura
                .pending_autonomous_lifecycle_terminal_outcome_inventory()
                .expect("inspect remote losing terminal outcomes")
                .is_empty(),
            "remote losing retirement must complete rather than strand Pending recovery"
        );
    }
}
#[test]
fn autonomous_non_author_does_not_take_queue_ownership() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, false);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, false);
    let slot = plan_autonomous_lane_reservation_slot(
        adapter.state.as_ref(),
        adapter.kura.as_ref(),
        &adapter.context,
        lane_id,
        dataspace_id,
    )
    .expect("plan non-author autonomous slot");
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("run non-author autonomous producer tick");
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.queued_len(), 1);
    assert!(adapter.pending_autonomous_anchor_payloads.is_empty());
    assert!(
        adapter
            .kura
            .read_autonomous_lane_block_artifact(
                lane_id,
                slot.lane_block_height,
                adapter.native_network_id(),
                adapter.context.epoch,
            )
            .is_none()
    );
}
#[test]
fn generic_fanout_cannot_publish_an_autonomous_producer_payload() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("produce one Queue-fenced autonomous payload");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .next()
        .expect("local author publishes one autonomous payload")
        .clone();
    let validators = adapter.frozen_validator_set();
    let _ = adapter.drain_effects(usize::MAX);
    adapter.fanout_lane_message(BlockMessage::LaneExecutablePayload(payload), &validators);
    assert!(adapter.effects.is_empty());
    assert!(
        adapter.output_guard.restart_required(),
        "generic transport must fail closed before a producer payload effect is inserted"
    );
}
#[test]
fn autonomous_restart_hydrates_durable_hint_free_payload_and_queue_owner() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("produce durable autonomous payload");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .next()
        .expect("pending autonomous payload")
        .clone();
    assert_eq!(payload.origin_proposal.payload_block_hint, None);
    let context = adapter.context.clone();
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    drop(queue);
    let mut recovered = restart
        .reopen(context, true)
        .expect("reopen autonomous lane adapter");
    assert_eq!(
        recovered.pending_autonomous_anchor_payloads.values().next(),
        Some(&payload),
        "startup hydration must recover the exact hint-free Kura payload"
    );
    recovered
        .hydrate_canonical_lane_artifacts()
        .expect("repeated hydration must accept the exact durable payload idempotently");
    assert_eq!(
        recovered.pending_autonomous_anchor_payloads.values().next(),
        Some(&payload),
        "repeated hydration must retain the exact same pending payload"
    );
    let recovered_queue =
        install_autonomous_test_queue(&mut recovered, lane_id, dataspace_id, &journal_path);
    assert_eq!(
        recovered_queue.live_lane_reservations(),
        payload.reservation_keys,
        "queue journal replay must retain the payload's exact durable owner"
    );
}
#[test]
fn autonomous_restart_rejects_conflicting_in_memory_payload_for_durable_slot() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 1);
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("produce durable autonomous payload");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .next()
        .expect("pending autonomous payload")
        .clone();
    let payload_key = AutonomousLanePayloadKey::from(&payload.origin_proposal);
    let context = adapter.context.clone();
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    drop(queue);
    let mut recovered = restart
        .reopen(context, true)
        .expect("reopen autonomous lane adapter");
    recovered
        .pending_autonomous_anchor_payloads
        .get_mut(&payload_key)
        .expect("startup hydration installed the exact durable payload")
        .producer_signature
        .push(0xA5);
    let error = recovered
        .hydrate_canonical_lane_artifacts()
        .expect_err("same-slot payload substitution must fail closed");
    assert!(
        matches!(
            &error,
            V2LaneWorkError::InvalidContext(reason)
                if reason.contains("conflicting bytes for the current slot")
        ),
        "unexpected conflicting-payload error: {error}"
    );
    assert!(
        recovered.output_guard.restart_required(),
        "conflicting same-slot hydration must close authoritative admission"
    );
}
#[test]
fn autonomous_small_payload_and_scan_limits_cannot_over_reserve() {
    let (mut adapter, keys) = autonomous_test_fixture(wire::ConsensusMode::Permissioned, true);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    assert_autonomous_test_role(&adapter, &keys, lane_id, dataspace_id, true);
    let journal_dir = tempfile::tempdir().expect("autonomous reservation journal directory");
    let journal_path = journal_dir.path().join("lane-reservations.norito");
    let queue = install_autonomous_test_queue(&mut adapter, lane_id, dataspace_id, &journal_path);
    let expected_entrypoints =
        enqueue_autonomous_test_transactions(&adapter, &queue, lane_id, dataspace_id, 3);
    assert_eq!(
        autonomous_route_quota_for_test(&adapter, lane_id, dataspace_id, 2),
        1
    );
    adapter
        .schedule_autonomous_lane_production(
            0,
            autonomous_test_candidate_limits_with_payload(
                2,
                adapter
                    .limits
                    .autonomous_carrier_headroom_bytes
                    .get()
                    .saturating_add(2),
                2,
            ),
        )
        .expect("run byte-bounded autonomous producer tick");
    assert!(
        adapter.pending_autonomous_anchor_payloads.is_empty(),
        "an entrypoint larger than the route's one-byte budget cannot be published"
    );
    assert!(queue.live_lane_reservations().is_empty());
    assert_eq!(queue.queued_len(), expected_entrypoints.len());
    adapter.next_autonomous_producer_tick = Instant::now();
    adapter
        .schedule_autonomous_lane_production(0, autonomous_test_candidate_limits(2, 2))
        .expect("run tightly bounded autonomous producer tick");
    let payload = adapter
        .pending_autonomous_anchor_payloads
        .values()
        .next()
        .expect("bounded producer publishes one payload");
    assert_eq!(payload.entrypoints, expected_entrypoints[..1]);
    assert_eq!(payload.reservation_keys.len(), 1);
    assert_eq!(queue.live_lane_reservations().len(), 1);
    assert_eq!(queue.queued_len(), 2);
}
fn autonomous_carrier_block(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    payload: &LaneExecutablePayloadV1,
) -> SignedBlock {
    autonomous_carrier_block_at_view(adapter, keys, payload, 0)
}
fn autonomous_carrier_block_at_view(
    adapter: &V2LaneWorkAdapter,
    keys: &[KeyPair],
    payload: &LaneExecutablePayloadV1,
    view: wire::View,
) -> SignedBlock {
    let envelope = autonomous_lane_payload_envelope(
        payload,
        adapter.native_network_id(),
        adapter.context.epoch,
    )
    .expect("encode autonomous carrier envelope");
    let header = BlockHeader::new(
        NonZeroU64::new(adapter.context.height).expect("non-zero carrier height"),
        adapter
            .context
            .parent_commit_qc
            .as_ref()
            .map(|qc| qc.subject.block_hash),
        None,
        None,
        adapter.context.height,
        view,
    );
    let mut builder = BlockBuilder::new(header);
    builder.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new()).with_autonomous_lane_payloads(vec![envelope]),
    ));
    let leader = usize::try_from(adapter.context.leader(view)).expect("global leader index");
    builder.build_with_signature(
        u64::try_from(leader).expect("global leader index fits u64"),
        keys[leader].private_key(),
    )
}
/// Exact record-backed autonomous lane certificate shared with worker handoff tests.
pub(in crate::sumeragi) struct HistoricalAutonomousLaneCertificateFixture {
    /// Kura containing the immutable historical autonomous recovery record.
    pub(in crate::sumeragi) kura: Arc<Kura>,
    /// Full autonomous Prepare/Commit certificate covered by that record.
    pub(in crate::sumeragi) certificate: LaneBlockCertificateV1,
    /// Historical global context which owns the recovery record.
    pub(in crate::sumeragi) context: wire::HeightContext,
    /// Validator keys used only by deterministic tests.
    pub(in crate::sumeragi) validators: Vec<KeyPair>,
}
/// Persist one record-backed autonomous certificate without an application receipt.
pub(in crate::sumeragi) fn historical_autonomous_lane_certificate_fixture()
-> HistoricalAutonomousLaneCertificateFixture {
    let (adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (source_block, mut proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("historical autonomous fixture entrypoint");
    let (payload, _) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
        b"historical-autonomous-queue-plan-admission-binding",
        b"historical-autonomous-reservation-owner",
        "deterministic historical autonomous producer",
        "historical autonomous producer key",
        "signed historical autonomous payload",
    );
    let carrier = autonomous_carrier_block(&adapter, &keys, &payload);
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist historical autonomous carrier");
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &carrier);
    let finality_receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist historical autonomous carrier finality");
    assert_eq!(finality_receipt.height(), adapter.context.height);
    assert_eq!(finality_receipt.block_hash(), carrier.hash());
    let committed = ValidBlock::committed_from_replay_signed_block(carrier.clone());
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    let payload = payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: adapter.context.height,
                proposal_view: carrier.header().view_change_index(),
                proposal_block_hash: carrier.hash(),
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach exact historical autonomous carrier hint");
    let proposal = payload.origin_proposal.clone();
    let execution_commitment = finality.commit_qc.execution_commitment;
    let mut install = HistoricalAutonomousReservationInstallV1 {
        version: HistoricalAutonomousReservationInstallV1::VERSION,
        recovery_id: Hash::prehashed([0; Hash::LENGTH]),
        canonical_body: CanonicalExecutedBlockNeedV1 {
            height: adapter.context.height,
            block_hash: carrier.hash(),
            finality_artifact_hash: HashOf::new(&finality),
            execution_commitment,
            executed_block_wire_len: execution_commitment.executed_block_wire_len,
            executed_block_wire_hash: execution_commitment.executed_block_wire_hash,
        },
        historical_context: adapter.context.clone(),
        historical_context_id: adapter.context.id(),
        historical_context_hash: HashOf::new(&adapter.context),
        carrier_view: carrier.header().view_change_index(),
        payload: payload.clone(),
        reservation_group: LaneQueueReservationReconciliationGroupV1 {
            identity: LaneQueueReservationGroupIdentityV1::from_key(
                payload
                    .reservation_keys
                    .first()
                    .expect("historical autonomous reservation group is non-empty"),
            ),
            ordered_keys: payload.reservation_keys.clone(),
        },
    };
    install.recovery_id = install.computed_recovery_id();
    assert_eq!(
        install_historical_autonomous_lane_recovery(
            adapter.state.as_ref(),
            adapter.kura.as_ref(),
            &install,
        )
        .expect("persist exact historical autonomous recovery record"),
        HistoricalAutonomousLaneRecoveryInstallOutcome::Installed,
    );
    assert!(
        adapter
            .kura
            .read_lane_block_application_receipt(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "autonomous record authority must not borrow an ordinary application receipt"
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
    .expect("historical autonomous READY votes form PrepareQC");
    let certificate = LaneBlockCertificateV1 {
        proposal: proposal.clone(),
        prepare_qc,
        commit_qc: lane_qc_for_phase(&proposal, &keys[..3], CertPhase::Commit),
    };
    HistoricalAutonomousLaneCertificateFixture {
        kura: Arc::clone(&adapter.kura),
        certificate,
        context: adapter.context.clone(),
        validators: keys,
    }
}
fn exercise_canonical_autonomous_carrier_after_direct_decision(
    mode: wire::ConsensusMode,
    local_signer_quorum: bool,
) {
    let (mut adapter, keys) = fixture_at_height_inner_with_kura_and_local_index(
        mode,
        2,
        true,
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY),
        Some(0),
        true,
    );
    let quorum_keys = if local_signer_quorum {
        &keys[..3]
    } else {
        &keys[1..]
    };
    let (source_block, mut proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous entrypoint");
    let routing_plan = RoutingPlan::single(RoutingDecision::new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
    ));
    let mut reservation = crate::queue::LaneQueueReservationKeyV1 {
        version: crate::queue::LaneQueueReservationKeyV1::VERSION,
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new(
            b"direct-decision-queue-plan-admission-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"direct-decision-reservation-owner"),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let producer = adapter
        .expected_lane_author(&proposal)
        .expect("deterministic autonomous producer")
        .clone();
    let producer_key = keys
        .iter()
        .find(|candidate| candidate.public_key() == producer.public_key())
        .expect("autonomous producer key");
    bind_canonical_autonomous_reservation_identity(
        &adapter,
        &proposal,
        &producer,
        &mut reservation,
    );
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        adapter.native_network_id(),
        adapter.context.epoch,
        proposal.clone(),
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        producer.clone(),
        producer_key.private_key(),
    )
    .expect("signed hint-free autonomous payload");
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    let carrier = autonomous_carrier_block(&adapter, &keys, &payload);
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist canonical autonomous carrier");
    let (locked_round, decided) = global_lock_for_block(&adapter, &carrier);
    let finality = verified_finality_artifact_for_block(&adapter, &keys, &carrier);
    let receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist exact canonical finality before receipt-bound recovery");
    let stale_label: &[u8] = if local_signer_quorum {
        b"stale-single-validator-local-lock"
    } else {
        b"stale-four-validator-local-lock"
    };
    let stale_payload_label: &[u8] = if local_signer_quorum {
        b"stale-single-validator-local-lock-payload"
    } else {
        b"stale-four-validator-local-lock-payload"
    };
    let stale_lock = wire::BlockSubject {
        parent_block_hash: decided.parent_block_hash,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(stale_label)),
        payload_hash: Hash::new(stale_payload_label),
    };
    assert_eq!(
        adapter.mark_global_body_locked(locked_round, stale_lock),
        Ok(GlobalBodyLockOutcome::Inserted)
    );
    adapter
        .retain_merge_sidecars_for_global_view(locked_round.view, Some(stale_lock), Some(decided))
        .expect("install direct same-view Decision");
    assert_eq!(
        adapter.globally_locked_body.map(|lock| lock.subject),
        Some(stale_lock)
    );
    let committed = ValidBlock::committed_from_replay_signed_block(carrier);
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    assert!(
        !adapter
            .durable_completion_matches_finality(&finality)
            .expect("inspect the direct-Decision lane durability boundary"),
        "the finalized autonomous anchor must keep rollover open before canonical recovery"
    );
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "Decision arrived before the live locked-body binding path"
    );
    assert_ne!(
        adapter
            .recover_decided_canonical_lane_body(&receipt, &finality)
            .expect("recover exact receipt-authorized canonical carrier"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_some(),
        "receipt-bound recovery must persist execution input before READY"
    );
    assert!(
        !adapter
            .durable_completion_matches_finality(&finality)
            .expect("inspect receipt-bound recovery before the READY quorum"),
        "local canonical recovery cannot close ingress before the READY quorum returns"
    );
    let emitted_local_ready = adapter.drain_effects(usize::MAX).into_iter().any(|effect| {
        matches!(
            effect,
            V2LaneWorkEffect::PostLaneBlock {
                message: BlockMessage::LaneBlockVote(vote),
                ..
            } if vote.body.phase == CertPhase::Prepare
                && vote.signer == adapter.local_peer
                && vote.payload_availability_vote.is_some()
        )
    });
    assert!(
        emitted_local_ready,
        "receipt-bound recovery must emit the local READY vote after durable input"
    );
    let prepare_votes = quorum_keys
        .iter()
        .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &keys))
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("exact three-of-four READY votes form PrepareQC");
    let commit_qc = lane_qc_for_phase(&proposal, quorum_keys, CertPhase::Commit);
    let quorum_sender = PeerId::new(
        quorum_keys
            .first()
            .expect("the fixed quorum is non-empty")
            .public_key()
            .clone(),
    );
    let admit_quorum_message = |message| {
        fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
            message,
            quorum_sender.clone(),
        ))
    };
    let prepare_outcome = if local_signer_quorum {
        adapter.insert_lane_qc(prepare_qc, locked_round.view)
    } else {
        adapter.accept_lane_message_with_ingress_ownership(
            admit_quorum_message(BlockMessage::LaneBlockQc(prepare_qc)),
            locked_round.view,
        )
    };
    assert_eq!(prepare_outcome, V2LaneIngressOutcome::Inserted);
    assert!(
        !adapter
            .durable_completion_matches_finality(&finality)
            .expect("inspect the autonomous boundary after READY quorum"),
        "READY durability alone cannot release rollover before CommitQC"
    );
    let commit_outcome = if local_signer_quorum {
        adapter.insert_lane_qc(commit_qc, locked_round.view)
    } else {
        adapter.accept_lane_message_with_ingress_ownership(
            admit_quorum_message(BlockMessage::LaneBlockQc(commit_qc)),
            locked_round.view,
        )
    };
    assert_eq!(commit_outcome, V2LaneIngressOutcome::Inserted);
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("bind canonical carrier and finish lane consensus"),
        1
    );
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_some(),
        "canonical fallback must persist execution input before READY"
    );
    let durable = adapter
        .kura
        .read_certified_lane_block_artifact(
            proposal.descriptor.lane_id,
            proposal.descriptor.lane_block_height,
        )
        .expect("READY and Commit votes produce a durable certificate");
    assert!(durable.prepare_qc.payload_availability_qc.is_some());
    assert!(
        adapter
            .durable_completion_matches_finality(&finality)
            .expect("validate completed autonomous durability"),
        "the authenticated READY/Commit quorum must release the finalized preflight"
    );
    assert!(
        adapter
            .durable_lane_rollover_authority(&finality)
            .expect("validate lane rollover")
            .is_some(),
        "durable availability and Commit certificates release rollover"
    );
}
#[test]
fn canonical_autonomous_carrier_binds_after_direct_single_validator_decision() {
    exercise_canonical_autonomous_carrier_after_direct_decision(
        wire::ConsensusMode::Permissioned,
        true,
    );
}
#[test]
fn canonical_autonomous_carrier_binds_after_direct_four_validator_decision() {
    exercise_canonical_autonomous_carrier_after_direct_decision(wire::ConsensusMode::Npos, false);
}

struct NonmemberCanonicalReplicaPreQcFixture {
    adapter: V2LaneWorkAdapter,
    global_keys: Vec<KeyPair>,
    lane_keys: Vec<KeyPair>,
    lane_id: LaneId,
    proposal: LaneBlockProposalV1,
    payload: LaneExecutablePayloadV1,
    key: AutonomousLanePayloadKey,
    locked_round: wire::ConsensusRound,
    decided: wire::BlockSubject,
    finality: wire::finality::V2FinalityArtifact,
    successor_context: wire::HeightContext,
}

fn nonmember_canonical_replica_pre_qc_fixture() -> NonmemberCanonicalReplicaPreQcFixture {
    let (mut adapter, global_keys) = fixture_at_height_inner_with_kura_and_local_index(
        wire::ConsensusMode::Permissioned,
        2,
        true,
        locked_lane_work_test_kura(iroha_config::parameters::defaults::kura::BLOCKS_IN_MEMORY),
        Some(3),
        true,
    );
    let extra_lane_key = KeyPair::try_from_seed(vec![0xF2; 32], Algorithm::BlsNormal)
        .expect("deterministic canonical-replica lane validator key");
    {
        let id = ConsensusKeyId::new(
            ConsensusKeyRole::Validator,
            "canonical-replica-lane-validator",
        );
        let record = ConsensusKeyRecord {
            id: id.clone(),
            public_key: extra_lane_key.public_key().clone(),
            pop: Some(
                iroha_crypto::bls_normal_pop_prove(extra_lane_key.private_key())
                    .expect("canonical-replica validator proof of possession"),
            ),
            activation_height: 0,
            expiry_height: None,
            replaces: None,
            status: ConsensusKeyStatus::Active,
        };
        let mut world = adapter.state.world.block();
        world.consensus_keys.insert(id.clone(), record.clone());
        world
            .consensus_keys_by_pk
            .insert(record.public_key.to_string(), vec![id]);
        world.commit();
    }
    let mut lane_keys = global_keys
        .iter()
        .filter(|key| key.public_key() != adapter.local_peer.public_key())
        .cloned()
        .collect::<Vec<_>>();
    lane_keys.push(extra_lane_key);
    lane_keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &lane_keys, lane_id, dataspace_id);

    let (source_block, mut proposal) = planned_autonomous_lane_candidate_block_for_route_at_view(
        &adapter,
        &global_keys,
        0,
        lane_id,
        dataspace_id,
    );
    proposal.payload_block_hint = None;
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("canonical-replica autonomous entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &lane_keys,
        &proposal,
        entrypoint,
        b"canonical-replica-queue-plan-admission-binding",
        b"canonical-replica-reservation-owner",
        "deterministic canonical-replica producer",
        "canonical-replica producer key",
        "signed canonical-replica autonomous payload",
    );
    let key = AutonomousLanePayloadKey::from(&proposal);
    assert!(adapter.voting_enabled);
    assert!(
        adapter
            .context
            .roster
            .iter()
            .any(|entry| entry.validator == adapter.local_peer)
    );
    assert_eq!(proposal.descriptor.validator_set.len(), 4);
    assert!(
        !proposal
            .descriptor
            .validator_set
            .contains(&adapter.local_peer)
    );
    assert!(!adapter.local_can_own_autonomous_payload(&proposal));
    assert_eq!(
        accept_lane_message_from(
            &mut adapter,
            BlockMessage::LaneExecutablePayload(payload.clone()),
            producer,
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );

    let carrier = autonomous_carrier_block(&adapter, &global_keys, &payload);
    adapter
        .kura
        .store_block(carrier.clone())
        .expect("persist canonical non-member carrier");
    let (locked_round, decided) = mark_global_body_locked_for_block(&mut adapter, &carrier);
    adapter
        .retain_merge_sidecars_for_global_view(locked_round.view, Some(decided), Some(decided))
        .expect("install exact canonical non-member Decision");
    let finality = verified_finality_artifact_for_block(&adapter, &global_keys, &carrier);
    let receipt = adapter
        .kura
        .store_v2_finality_artifact(&finality)
        .expect("persist canonical non-member finality");
    let successor_context = successor_context_for_parent(&adapter, &carrier);
    let committed = ValidBlock::committed_from_replay_signed_block(carrier);
    commit_test_block_to_state(adapter.state.as_ref(), &committed, &adapter.context);
    assert_ne!(
        adapter
            .recover_decided_canonical_lane_body(&receipt, &finality)
            .expect("recover canonical carrier on a non-member global validator"),
        V2LaneIngressOutcome::Rejected
    );
    assert!(autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none());
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(lane_id, proposal.descriptor.lane_block_height)
            .is_none(),
        "canonical carrier recovery must not synthesize committee-local execution input"
    );
    assert!(
        adapter
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote.signer == adapter.local_peer
                    && vote.payload_availability_vote.is_some()
            )),
        "a non-member global validator must not sign committee-local READY"
    );
    NonmemberCanonicalReplicaPreQcFixture {
        adapter,
        global_keys,
        lane_keys,
        lane_id,
        proposal,
        payload,
        key,
        locked_round,
        decided,
        finality,
        successor_context,
    }
}

fn attach_finalized_nonmember_public_payload(
    adapter: &V2LaneWorkAdapter,
    proposal: &LaneBlockProposalV1,
    payload: LaneExecutablePayloadV1,
    locked_round: &wire::ConsensusRound,
    decided: &wire::BlockSubject,
) -> LaneExecutablePayloadV1 {
    payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: proposal.descriptor.proposal_height,
                proposal_view: locked_round.view,
                proposal_block_hash: decided.block_hash,
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach exact finalized nonmember carrier hint")
}

fn install_saturated_nonmember_observer_conflict(
    adapter: &mut V2LaneWorkAdapter,
    canonical_proposal: &LaneBlockProposalV1,
    canonical_block_hash: HashOf<BlockHeader>,
    conflict_label: &[u8],
) -> (LaneBlockProposalV1, LaneBlockSessionCache) {
    let mut conflicting_ownership = ownership_from_proposal(canonical_proposal);
    conflicting_ownership.accepted_transaction_hashes = vec![Hash::new(conflict_label)];
    let conflicting_replay = conflicting_ownership
        .compute_replay_hashes()
        .expect("derive structurally valid saturated observer conflict");
    conflicting_ownership.subject_hash = conflicting_replay.subject_hash;
    conflicting_ownership.payload_ownership_hash = conflicting_replay.payload_ownership_hash;
    conflicting_ownership.rbc_instance_hash = conflicting_replay.rbc_instance_hash;
    conflicting_ownership.lane_block_descriptor_hash =
        Some(conflicting_replay.lane_block_descriptor_hash);
    let conflicting_proposal =
        proposal_from_ownership(&conflicting_ownership, canonical_block_hash)
            .expect("construct saturated same-slot observer conflict");
    assert_ne!(
        conflicting_proposal.proposal_hash, canonical_proposal.proposal_hash,
        "the saturated cache fixture must contain a different proposal identity"
    );

    adapter.limits.session_capacity =
        std::num::NonZeroUsize::new(1).expect("capacity-one observer cache");
    adapter.lane_sessions = LaneBlockSessionCache::new(1);
    adapter
        .lane_sessions
        .insert_proposal(conflicting_proposal.clone())
        .expect("fill the capacity-one observer cache with an uncommitted conflict");
    assert_eq!(adapter.lane_sessions.len(), 1);
    let saturated_cache = adapter.lane_sessions.clone();
    (conflicting_proposal, saturated_cache)
}

fn finalized_nonmember_prepare_qc(
    proposal: &LaneBlockProposalV1,
    payload: &LaneExecutablePayloadV1,
    lane_keys: &[KeyPair],
) -> LaneBlockQcV1 {
    let prepare_votes = lane_keys[..3]
        .iter()
        .map(|key| signed_autonomous_prepare_vote(proposal, payload, key, lane_keys))
        .collect::<Vec<_>>();
    crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("three-of-four READY votes form finalized nonmember PrepareQC")
}

fn globally_connected_lane_quorum_key<'a>(
    global_keys: &[KeyPair],
    lane_keys: &'a [KeyPair],
) -> &'a KeyPair {
    lane_keys[..3]
        .iter()
        .find(|lane_key| {
            global_keys
                .iter()
                .any(|global_key| global_key.public_key() == lane_key.public_key())
        })
        .expect("READY quorum includes a globally connected lane validator")
}

#[test]
fn finalized_carrier_nonmember_cache_saturated_capacity_replaces_uncommitted_conflict() {
    let NonmemberCanonicalReplicaPreQcFixture {
        mut adapter,
        global_keys,
        lane_keys,
        proposal,
        payload,
        locked_round,
        decided,
        ..
    } = nonmember_canonical_replica_pre_qc_fixture();
    let public_payload = attach_finalized_nonmember_public_payload(
        &adapter,
        &proposal,
        payload,
        &locked_round,
        &decided,
    );
    let public_proposal = public_payload.origin_proposal.clone();
    let (conflicting_proposal, saturated_cache) = install_saturated_nonmember_observer_conflict(
        &mut adapter,
        &public_proposal,
        decided.block_hash,
        b"capacity-one finalized observer conflict",
    );
    let prepare_qc = finalized_nonmember_prepare_qc(&public_proposal, &public_payload, &lane_keys);
    let sender = PeerId::new(
        globally_connected_lane_quorum_key(&global_keys, &lane_keys)
            .public_key()
            .clone(),
    );

    assert_eq!(adapter.lane_sessions, saturated_cache);
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::LaneBlockQc(prepare_qc.clone()),
                sender,
            )),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted,
        "canonical finalized hydration must replace, rather than count in addition to, an uncommitted same-slot conflict"
    );
    assert_eq!(
        adapter.lane_sessions.len(),
        1,
        "replacement must stay within the configured capacity-one bound"
    );
    assert_eq!(
        adapter
            .lane_sessions
            .proposal_for_vote_body(&prepare_qc.body),
        Some(public_proposal.clone())
    );
    assert!(adapter.lane_sessions.contains_proposal(&public_proposal));
    assert!(
        !adapter
            .lane_sessions
            .contains_proposal(&conflicting_proposal)
    );
    assert!(!adapter.output_guard.restart_required());
}

#[test]
fn finalized_carrier_nonmember_cache_invalid_ready_vote_rolls_back_hydration() {
    let NonmemberCanonicalReplicaPreQcFixture {
        mut adapter,
        global_keys,
        lane_keys,
        proposal,
        payload,
        locked_round,
        decided,
        ..
    } = nonmember_canonical_replica_pre_qc_fixture();
    let public_payload = attach_finalized_nonmember_public_payload(
        &adapter,
        &proposal,
        payload,
        &locked_round,
        &decided,
    );
    let public_proposal = public_payload.origin_proposal.clone();
    let (conflicting_proposal, original_cache) = install_saturated_nonmember_observer_conflict(
        &mut adapter,
        &public_proposal,
        decided.block_hash,
        b"invalid READY vote observer conflict",
    );
    let signing_key = globally_connected_lane_quorum_key(&global_keys, &lane_keys);
    let valid_vote =
        signed_autonomous_prepare_vote(&public_proposal, &public_payload, signing_key, &lane_keys);
    let sender = valid_vote.signer.clone();
    let mut invalid_vote = valid_vote.clone();
    *invalid_vote
        .bls_signature
        .first_mut()
        .expect("READY signature fixture is non-empty") ^= 0x01;

    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::LaneBlockVote(invalid_vote),
                sender.clone(),
            )),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.lane_sessions, original_cache,
        "invalid signed READY must roll back finalized-carrier hydration byte-for-byte"
    );
    assert!(
        adapter
            .lane_sessions
            .contains_proposal(&conflicting_proposal)
    );

    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::LaneBlockVote(valid_vote.clone()),
                sender,
            )),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted,
        "the corresponding valid READY vote must remain admissible after rollback"
    );
    assert!(adapter.lane_sessions.contains_vote(&valid_vote));
    assert!(adapter.lane_sessions.contains_proposal(&public_proposal));
    assert!(
        !adapter
            .lane_sessions
            .contains_proposal(&conflicting_proposal)
    );
    assert_eq!(adapter.lane_sessions.len(), 1);
    assert!(!adapter.output_guard.restart_required());
}

#[test]
fn finalized_carrier_nonmember_cache_invalid_commit_certificate_rolls_back_hydration() {
    let NonmemberCanonicalReplicaPreQcFixture {
        mut adapter,
        global_keys,
        lane_keys,
        proposal,
        payload,
        locked_round,
        decided,
        ..
    } = nonmember_canonical_replica_pre_qc_fixture();
    let public_payload = attach_finalized_nonmember_public_payload(
        &adapter,
        &proposal,
        payload,
        &locked_round,
        &decided,
    );
    let public_proposal = public_payload.origin_proposal.clone();
    let (conflicting_proposal, original_cache) = install_saturated_nonmember_observer_conflict(
        &mut adapter,
        &public_proposal,
        decided.block_hash,
        b"invalid CommitQC certificate observer conflict",
    );
    let prepare_qc = finalized_nonmember_prepare_qc(&public_proposal, &public_payload, &lane_keys);
    let commit_qc = lane_qc_for_phase(&public_proposal, &lane_keys[..3], CertPhase::Commit);
    let valid_certificate = LaneBlockCertificateV1 {
        proposal: public_proposal.clone(),
        prepare_qc: prepare_qc.clone(),
        commit_qc,
    };
    let mut invalid_certificate = valid_certificate.clone();
    *invalid_certificate
        .commit_qc
        .bls_aggregate_signature
        .first_mut()
        .expect("CommitQC aggregate signature fixture is non-empty") ^= 0x01;
    let sender = PeerId::new(
        globally_connected_lane_quorum_key(&global_keys, &lane_keys)
            .public_key()
            .clone(),
    );

    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::LaneBlockCertificate(Box::new(invalid_certificate)),
                sender.clone(),
            )),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.lane_sessions, original_cache,
        "a certificate that hydrates and inserts a valid PrepareQC before rejecting its CommitQC must roll back the whole cache"
    );
    assert!(
        adapter
            .lane_sessions
            .contains_proposal(&conflicting_proposal)
    );

    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
                BlockMessage::LaneBlockCertificate(Box::new(valid_certificate)),
                sender,
            )),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted,
        "the valid complete certificate must remain admissible after transactional rollback"
    );
    assert_eq!(
        adapter
            .lane_sessions
            .proposal_for_vote_body(&prepare_qc.body),
        Some(public_proposal.clone())
    );
    assert!(adapter.lane_sessions.contains_proposal(&public_proposal));
    assert!(
        !adapter
            .lane_sessions
            .contains_proposal(&conflicting_proposal)
    );
    assert_eq!(adapter.lane_sessions.len(), 1);
    assert!(!adapter.output_guard.restart_required());
}

#[test]
fn global_validator_outside_lane_committee_uses_canonical_replica_for_rollover() {
    let NonmemberCanonicalReplicaPreQcFixture {
        mut adapter,
        global_keys,
        lane_keys,
        lane_id: _,
        proposal,
        payload,
        key,
        locked_round,
        decided,
        finality,
        successor_context,
    } = nonmember_canonical_replica_pre_qc_fixture();

    let prepare_votes = lane_keys[..3]
        .iter()
        .map(|key| signed_autonomous_prepare_vote(&proposal, &payload, key, &lane_keys))
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("canonical-replica three-of-four READY votes form PrepareQC");
    let commit_qc = lane_qc_for_phase(&proposal, &lane_keys[..3], CertPhase::Commit);
    let quorum_sender_key = lane_keys[..3]
        .iter()
        .find(|lane_key| {
            global_keys
                .iter()
                .any(|global_key| global_key.public_key() == lane_key.public_key())
        })
        .expect("READY quorum includes a globally connected lane validator");
    let quorum_sender = PeerId::new(quorum_sender_key.public_key().clone());
    let admit_quorum_message = |message| {
        fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
            message,
            quorum_sender.clone(),
        ))
    };
    let recovered_proposal = adapter
        .lane_sessions
        .proposal_for_vote_body(&prepare_qc.body)
        .expect("canonical carrier recovery must hydrate the exact public proposal");
    assert!(
        recovered_proposal.same_consensus_identity(&proposal),
        "canonical carrier recovery must preserve the hint-free consensus identity"
    );
    assert_eq!(
        recovered_proposal.payload_block_hint,
        Some(LaneBlockProposalPayloadHintV1 {
            proposal_height: proposal.descriptor.proposal_height,
            proposal_view: locked_round.view,
            proposal_block_hash: decided.block_hash,
        }),
        "canonical carrier recovery must attach the exact decided global hint"
    );
    assert!(
        adapter.lane_message_is_allowed_after_decision(&BlockMessage::LaneBlockQc(
            prepare_qc.clone(),
        )),
        "the exact canonical-carrier QC must survive the post-Decision ingress gate"
    );
    assert!(
        adapter.lane_vote_body_available(&prepare_qc.body),
        "the exact canonical-carrier proposal must remain available to QC ingress"
    );
    assert!(
        adapter.lane_qc_authorized(&prepare_qc, locked_round.view),
        "the finalized carrier's lane committee must authorize its exact QC"
    );
    assert_eq!(
        adapter
            .lane_sessions
            .authorized_payload_availability_body_for(&proposal),
        prepare_qc
            .payload_availability_qc
            .as_ref()
            .map(|availability| availability.body.clone()),
        "a non-member observer must install the READY subject without receiving signing authority"
    );
    let mut conflicting_ownership = ownership_from_proposal(&recovered_proposal);
    conflicting_ownership.accepted_transaction_hashes =
        vec![Hash::new(b"uncommitted observer cache conflict")];
    let conflicting_replay = conflicting_ownership
        .compute_replay_hashes()
        .expect("derive structurally valid conflicting observer replay material");
    conflicting_ownership.subject_hash = conflicting_replay.subject_hash;
    conflicting_ownership.payload_ownership_hash = conflicting_replay.payload_ownership_hash;
    conflicting_ownership.rbc_instance_hash = conflicting_replay.rbc_instance_hash;
    conflicting_ownership.lane_block_descriptor_hash =
        Some(conflicting_replay.lane_block_descriptor_hash);
    let conflicting_proposal = proposal_from_ownership(&conflicting_ownership, decided.block_hash)
        .expect("construct an uncommitted same-slot observer conflict");
    assert_ne!(conflicting_proposal.proposal_hash, proposal.proposal_hash);
    adapter.lane_sessions = LaneBlockSessionCache::new(adapter.limits.session_capacity.get());
    adapter
        .lane_sessions
        .insert_proposal(conflicting_proposal.clone())
        .expect("install uncommitted same-slot observer conflict");
    let conflicting_cache = adapter.lane_sessions.clone();

    let mut bad_signature = prepare_qc.clone();
    *bad_signature
        .bls_aggregate_signature
        .first_mut()
        .expect("aggregate signature fixture is non-empty") ^= 0x01;
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            admit_quorum_message(BlockMessage::LaneBlockQc(bad_signature)),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.lane_sessions, conflicting_cache,
        "bad aggregate validation must roll back public observer hydration"
    );

    let mut bad_ready = prepare_qc.clone();
    bad_ready
        .payload_availability_qc
        .as_mut()
        .expect("autonomous PrepareQC carries READY")
        .body
        .executable_payload_hash = Hash::new(b"bad embedded observer READY");
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            admit_quorum_message(BlockMessage::LaneBlockQc(bad_ready)),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.lane_sessions, conflicting_cache,
        "bad embedded READY validation must roll back public observer hydration"
    );
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            admit_quorum_message(BlockMessage::LaneBlockQc(commit_qc.clone())),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Rejected
    );
    assert_eq!(
        adapter.lane_sessions, conflicting_cache,
        "Commit-before-Prepare must roll back public observer hydration"
    );
    assert!(adapter.lane_ready_authorizations.is_empty());
    assert!(!adapter.output_guard.restart_required());
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            admit_quorum_message(BlockMessage::LaneBlockQc(prepare_qc)),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter.accept_lane_message_with_ingress_ownership(
            admit_quorum_message(BlockMessage::LaneBlockQc(commit_qc)),
            locked_round.view,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        adapter
            .persist_anchored_sessions()
            .expect("persist canonical replica without private custody"),
        1
    );

    assert!(autonomous_artifact(&adapter, &proposal, adapter.context.epoch).is_none());
    assert!(
        adapter
            .kura
            .read_certified_lane_block_artifact(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "replica persistence must not publish the committee-owned certified slot"
    );
    assert!(
        adapter
            .kura
            .read_lane_block_execution_input(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "replica persistence must not publish committee-owned execution input"
    );
    assert!(!adapter.autonomous_new_view_started_at.contains_key(&key));
    assert!(adapter.lane_ready_authorizations.is_empty());
    assert!(adapter.lane_drain_queue.is_none());
    assert!(
        adapter
            .state
            .has_pending_merge_execution_sources(adapter.context.mode),
        "global merge selection must consume the replica without private certified/frontier state"
    );
    assert!(
        adapter
            .durable_completion_matches_finality(&finality)
            .expect("validate canonical replica completion"),
        "an exact canonical replica must close the global rollover durability boundary"
    );
    adapter
        .prepare_canonical_lane_rollover(&finality)
        .expect("canonicalize non-member replica rollover evidence");
    assert!(
        adapter
            .durable_lane_rollover_authority(&finality)
            .expect("validate canonical replica rollover authority")
            .is_some(),
        "canonical replica evidence must authorize successor rollover"
    );
    assert!(!adapter.output_guard.restart_required());

    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let recovered = restart
        .reopen(successor_context, true)
        .expect("restart non-member adapter from canonical replica");
    assert!(
        recovered
            .state
            .has_pending_merge_execution_sources(recovered.context.mode),
        "restart must recover the replica as an execution-capable merge source"
    );
    assert!(
        recovered
            .durable_completion_matches_finality(&finality)
            .expect("validate canonical replica completion after restart"),
        "restart must preserve the exact completed global rollover boundary"
    );
    assert!(recovered.lane_ready_authorizations.is_empty());
    assert!(recovered.lane_drain_queue.is_none());
    assert!(!recovered.output_guard.restart_required());
}

#[test]
fn restarted_nonmember_accepts_public_qcs_and_persists_only_canonical_replica() {
    let NonmemberCanonicalReplicaPreQcFixture {
        adapter,
        global_keys,
        lane_keys,
        lane_id,
        proposal,
        payload,
        key,
        locked_round,
        decided,
        finality,
        successor_context,
    } = nonmember_canonical_replica_pre_qc_fixture();
    let public_payload = payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: proposal.descriptor.proposal_height,
                proposal_view: locked_round.view,
                proposal_block_hash: decided.block_hash,
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach exact public carrier hint before restart");
    let public_proposal = public_payload.origin_proposal.clone();
    let prepare_votes = lane_keys[..3]
        .iter()
        .map(|key| {
            signed_autonomous_prepare_vote(&public_proposal, &public_payload, key, &lane_keys)
        })
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        public_proposal.vote_body(CertPhase::Prepare),
        public_proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("restart fixture READY votes form PrepareQC");
    let commit_qc = lane_qc_for_phase(&public_proposal, &lane_keys[..3], CertPhase::Commit);
    let quorum_sender = PeerId::new(
        lane_keys[..3]
            .iter()
            .find(|lane_key| {
                global_keys
                    .iter()
                    .any(|global_key| global_key.public_key() == lane_key.public_key())
            })
            .expect("restart quorum includes a globally connected lane validator")
            .public_key()
            .clone(),
    );
    assert!(
        adapter
            .kura
            .durable_canonical_autonomous_lane_replica(
                lane_id,
                public_proposal.descriptor.lane_block_height,
                public_payload.network_id,
                public_payload.epoch,
            )
            .expect("inspect pre-restart replica slot")
            .is_none(),
        "the crash boundary must precede every lane QC and replica write"
    );
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut recovered = restart
        .reopen(successor_context, true)
        .expect("reopen nonmember after carrier finality but before lane QCs");
    assert!(autonomous_artifact(&recovered, &public_proposal, public_payload.epoch).is_none());
    assert!(
        recovered
            .kura
            .read_lane_block_execution_input(lane_id, public_proposal.descriptor.lane_block_height,)
            .is_none()
    );
    let admit = |message| {
        fair_v2_ingress_admit_for_test(InboundBlockMessage::from_authenticated_peer(
            message,
            quorum_sender.clone(),
        ))
    };
    assert_eq!(
        recovered.accept_lane_message_with_ingress_ownership(
            admit(BlockMessage::LaneBlockQc(prepare_qc)),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert_eq!(
        recovered.accept_lane_message_with_ingress_ownership(
            admit(BlockMessage::LaneBlockQc(commit_qc)),
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert!(recovered.has_pending_historical_recovery());
    assert!(matches!(
        recovered
            .service_next_historical_recovery()
            .expect("persist historical public replica after restart"),
        HistoricalRecoveryServiceOutcome::Complete(_)
    ));
    assert!(autonomous_artifact(&recovered, &public_proposal, public_payload.epoch).is_none());
    assert!(
        recovered
            .kura
            .read_certified_lane_block_artifact(
                lane_id,
                public_proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "historical nonmember recovery must not publish the committee-certified slot"
    );
    assert!(
        recovered
            .kura
            .read_lane_block_execution_input(lane_id, public_proposal.descriptor.lane_block_height,)
            .is_none(),
        "historical nonmember recovery must not publish private execution input"
    );
    assert!(
        recovered
            .kura
            .durable_canonical_autonomous_lane_replica(
                lane_id,
                public_proposal.descriptor.lane_block_height,
                public_payload.network_id,
                public_payload.epoch,
            )
            .expect("read historical canonical replica")
            .is_some()
    );
    assert!(!recovered.autonomous_new_view_started_at.contains_key(&key));
    assert!(recovered.lane_ready_authorizations.is_empty());
    assert!(recovered.lane_drain_queue.is_none());
    assert!(
        recovered
            .drain_effects(usize::MAX)
            .iter()
            .all(|effect| !matches!(
                effect,
                V2LaneWorkEffect::PostLaneBlock {
                    message: BlockMessage::LaneBlockVote(vote),
                    ..
                } if vote.signer == recovered.local_peer
            )),
        "a restarted nonmember must not emit a lane vote"
    );
    assert!(
        recovered
            .durable_completion_matches_finality(&finality)
            .expect("validate historical replica completion")
    );
    assert!(!recovered.output_guard.restart_required());
}

#[test]
fn restarted_nonmember_accepts_complete_public_certificate_without_private_custody() {
    let NonmemberCanonicalReplicaPreQcFixture {
        adapter,
        global_keys,
        lane_keys,
        lane_id,
        proposal,
        payload,
        key,
        locked_round,
        decided,
        finality,
        successor_context,
    } = nonmember_canonical_replica_pre_qc_fixture();
    let public_payload = payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: proposal.descriptor.proposal_height,
                proposal_view: locked_round.view,
                proposal_block_hash: decided.block_hash,
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach exact public carrier hint for complete certificate");
    let public_proposal = public_payload.origin_proposal.clone();
    let prepare_votes = lane_keys[..3]
        .iter()
        .map(|key| {
            signed_autonomous_prepare_vote(&public_proposal, &public_payload, key, &lane_keys)
        })
        .collect::<Vec<_>>();
    let certificate = LaneBlockCertificateV1 {
        proposal: public_proposal.clone(),
        prepare_qc: crate::lane_consensus::aggregate_lane_block_votes_to_qc(
            public_proposal.vote_body(CertPhase::Prepare),
            public_proposal.descriptor.validator_set.clone(),
            &prepare_votes,
        )
        .expect("complete restart certificate has a valid READY PrepareQC"),
        commit_qc: lane_qc_for_phase(&public_proposal, &lane_keys[..3], CertPhase::Commit),
    };
    let sender = PeerId::new(
        lane_keys[..3]
            .iter()
            .find(|lane_key| {
                global_keys
                    .iter()
                    .any(|global_key| global_key.public_key() == lane_key.public_key())
            })
            .expect("certificate quorum includes a globally connected validator")
            .public_key()
            .clone(),
    );
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut recovered = restart
        .reopen(successor_context, true)
        .expect("reopen nonmember before complete certificate ingress");
    assert_eq!(
        accept_lane_message_from(
            &mut recovered,
            BlockMessage::LaneBlockCertificate(Box::new(certificate)),
            sender,
            0,
        ),
        V2LaneIngressOutcome::Inserted
    );
    assert!(matches!(
        recovered
            .service_next_historical_recovery()
            .expect("persist complete historical certificate as public replica"),
        HistoricalRecoveryServiceOutcome::Complete(_)
    ));
    assert!(autonomous_artifact(&recovered, &public_proposal, public_payload.epoch).is_none());
    assert!(
        recovered
            .kura
            .read_certified_lane_block_artifact(
                lane_id,
                public_proposal.descriptor.lane_block_height,
            )
            .is_none()
    );
    assert!(
        recovered
            .kura
            .read_lane_block_execution_input(lane_id, public_proposal.descriptor.lane_block_height,)
            .is_none()
    );
    assert!(
        recovered
            .kura
            .durable_canonical_autonomous_lane_replica(
                lane_id,
                public_proposal.descriptor.lane_block_height,
                public_payload.network_id,
                public_payload.epoch,
            )
            .expect("read complete-certificate canonical replica")
            .is_some()
    );
    assert!(!recovered.autonomous_new_view_started_at.contains_key(&key));
    assert!(recovered.lane_ready_authorizations.is_empty());
    assert!(recovered.lane_drain_queue.is_none());
    assert!(
        recovered
            .durable_completion_matches_finality(&finality)
            .expect("validate complete-certificate replica completion")
    );
    assert!(!recovered.output_guard.restart_required());
}

#[test]
fn restarted_nonmember_historical_response_persists_only_exact_public_replica() {
    let NonmemberCanonicalReplicaPreQcFixture {
        adapter,
        global_keys,
        lane_keys,
        lane_id,
        proposal,
        payload,
        key,
        locked_round,
        decided,
        finality,
        successor_context,
    } = nonmember_canonical_replica_pre_qc_fixture();
    let public_payload = payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: proposal.descriptor.proposal_height,
                proposal_view: locked_round.view,
                proposal_block_hash: decided.block_hash,
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach exact public carrier hint for historical response");
    let public_proposal = public_payload.origin_proposal.clone();
    let prepare_votes = lane_keys[..3]
        .iter()
        .map(|key| {
            signed_autonomous_prepare_vote(&public_proposal, &public_payload, key, &lane_keys)
        })
        .collect::<Vec<_>>();
    let prepare_qc = crate::lane_consensus::aggregate_lane_block_votes_to_qc(
        public_proposal.vote_body(CertPhase::Prepare),
        public_proposal.descriptor.validator_set.clone(),
        &prepare_votes,
    )
    .expect("historical response fixture has a valid READY PrepareQC");
    let commit_qc = lane_qc_for_phase(&public_proposal, &lane_keys[..3], CertPhase::Commit);
    let sender = PeerId::new(
        lane_keys[..3]
            .iter()
            .find(|lane_key| {
                global_keys
                    .iter()
                    .any(|global_key| global_key.public_key() == lane_key.public_key())
            })
            .expect("historical response quorum includes a connected lane validator")
            .public_key()
            .clone(),
    );
    let restart = LaneAdapterRestartParts::capture(&adapter);
    drop(adapter);
    let mut recovered = restart
        .reopen(successor_context, true)
        .expect("reopen nonmember before historical response import");
    let request = LaneHistoricalRecoveryRequestV1 {
        version: LANE_HISTORICAL_RECOVERY_VERSION_V1,
        requester: recovered.local_peer.clone(),
        certificate: Some(LaneBlockCertificateV1 {
            proposal: public_proposal.clone(),
            prepare_qc: prepare_qc.clone(),
            commit_qc: commit_qc.clone(),
        }),
        signer_pops: lane_signer_pops(&lane_keys[..3]),
        kind: LaneHistoricalRecoveryKindV1::AutonomousPayload {
            executable_payload_hash: public_payload.payload_hash,
            prepare_qc_hash: HashOf::new(&prepare_qc),
            commit_qc_hash: HashOf::new(&commit_qc),
        },
    };
    let identity = HistoricalRecoveryIdentity::from_proposal(&public_proposal)
        .expect("derive historical response request identity");
    let request_hash = HashOf::new(&request);
    recovered
        .historical_recovery_request_owners
        .insert(request_hash, identity);
    recovered.historical_recovery_requests.insert(
        identity,
        OutstandingHistoricalRecoveryRequest {
            request_hash,
            request,
            cadence: HistoricalRecoveryRequestCadence::immediate(
                HistoricalRecoveryWaitReason::AutonomousPayloadPending,
                Instant::now(),
            ),
            canonical_body_destinations: BTreeSet::new(),
        },
    );
    let response = LaneHistoricalRecoveryResponseV1 {
        version: LANE_HISTORICAL_RECOVERY_VERSION_V1,
        request_hash,
        payload: LaneHistoricalRecoveryPayloadV1::AutonomousPayload {
            payload: public_payload.clone(),
            prepare_qc,
            commit_qc,
        },
    };
    assert_eq!(
        recovered.accept_historical_recovery_response(response, Some(&sender)),
        V2LaneIngressOutcome::Inserted
    );
    assert!(recovered.historical_recovery_requests.is_empty());
    assert!(recovered.historical_recovery_request_owners.is_empty());
    assert!(autonomous_artifact(&recovered, &public_proposal, public_payload.epoch).is_none());
    assert!(
        recovered
            .kura
            .read_certified_lane_block_artifact(
                lane_id,
                public_proposal.descriptor.lane_block_height,
            )
            .is_none(),
        "a historical response must not publish the committee-certified slot"
    );
    assert!(
        recovered
            .kura
            .read_lane_block_execution_input(lane_id, public_proposal.descriptor.lane_block_height,)
            .is_none(),
        "a historical response must not publish private execution input"
    );
    let replica = recovered
        .kura
        .durable_canonical_autonomous_lane_replica(
            lane_id,
            public_proposal.descriptor.lane_block_height,
            public_payload.network_id,
            public_payload.epoch,
        )
        .expect("read historical-response canonical replica")
        .expect("historical response persists the public replica");
    assert_eq!(replica.bundle.executable_payload(), &public_payload);
    assert!(!recovered.autonomous_new_view_started_at.contains_key(&key));
    assert!(recovered.lane_ready_authorizations.is_empty());
    assert!(recovered.lane_drain_queue.is_none());
    assert!(
        recovered
            .durable_completion_matches_finality(&finality)
            .expect("validate historical-response replica completion")
    );
    assert!(!recovered.output_guard.restart_required());
}

#[test]
fn recovered_autonomous_certificate_repairs_ready_before_certified_publication() {
    let (mut adapter, keys) = fixture_at_height_inner(wire::ConsensusMode::Permissioned, 2, true);
    let (source_block, mut proposal) =
        planned_autonomous_lane_candidate_block_at_view(&adapter, &keys, 0);
    proposal.payload_block_hint = None;
    let entrypoint = source_block
        .external_entrypoints_cloned()
        .next()
        .expect("autonomous recovery fixture entrypoint");
    let (payload, producer) = signed_autonomous_payload_for_entrypoint(
        &adapter,
        &keys,
        &proposal,
        entrypoint,
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
    let block = autonomous_carrier_block(&adapter, &keys, &payload);
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
    let payload = payload
        .attach_global_hint_exact(
            LaneBlockProposalPayloadHintV1 {
                proposal_height: adapter.context.height,
                proposal_view: 0,
                proposal_block_hash: block.hash(),
            },
            adapter.native_network_id(),
            adapter.context.epoch,
        )
        .expect("attach the canonical autonomous carrier hint");
    let proposal = payload.origin_proposal.clone();
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
    let current_context = adapter.context.clone();
    let mut sidecar_only_successor = successor_context_for_parent(&adapter, &block);
    sidecar_only_successor.epoch = {
        let world = adapter.state.world_view();
        crate::sumeragi::epoch_for_height_from_world(
            &world,
            sidecar_only_successor.height,
            sidecar_only_successor.mode,
        )
        .expect("fixture has a valid committed epoch schedule")
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
    assert_eq!(
        durable_historical_lane_output_source_hash(
            adapter.kura.as_ref(),
            &BlockMessage::LaneBlockQc(alternative_commit),
        )
        .expect("classify autonomous output against ordinary historical durability"),
        None,
        "autonomous recovery must use its immutable record rather than ordinary certificate-and-application authority"
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
        crate::sumeragi::epoch_for_height_from_world(
            &world,
            successor_context.height,
            successor_context.mode,
        )
        .expect("fixture has a valid committed epoch schedule")
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
#[test]
fn repeated_non_empty_retries_never_make_autonomous_routes_ordinary_eligible() {
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(7);
    prepare_autonomous_test_lane(&mut adapter, &keys, lane_id, dataspace_id);
    let transaction_key = KeyPair::try_from_seed(vec![0xE1; 32], Algorithm::Ed25519)
        .expect("deterministic autonomous-route transaction key");
    let transaction = TransactionBuilder::new(
        adapter.context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .sign(transaction_key.private_key());
    let accepted =
        crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(transaction));
    let routing_plan = RoutingPlan::single(RoutingDecision::new(lane_id, dataspace_id));
    let candidate = CandidateDescriptor::new(&accepted, &routing_plan);
    let context = adapter.context.clone();
    let mut provider = &mut adapter;
    for _ in 0..3 {
        let recovery = provider
            .prepare(&context, 0, &[])
            .expect("a non-empty retry requires no ordinary lane ownership");
        assert!(recovery.native_amx_receipts.is_empty());
        let unavailable = provider
            .prepare(&context, 0, &[candidate])
            .expect_err("autonomous route must remain unavailable to ordinary execution");
        assert_eq!(unavailable.indices(), &BTreeSet::from([0]));
        assert_eq!(
            unavailable.reason(),
            "waiting for deterministic autonomous lane authors to publish durable FIFO reservations"
        );
    }

    // QueuePlan-synchronized ownership remains autonomous even when the
    // topology exposes only one route and the broader multi-lane exclusion is
    // therefore disabled.
    let (mut adapter, keys) = fixture(wire::ConsensusMode::Permissioned);
    enable_single_custom_lane_nexus(&mut adapter, &keys, lane_id, dataspace_id);
    let transaction_key = KeyPair::try_from_seed(vec![0xB7; 32], Algorithm::Ed25519)
        .expect("deterministic single-lane QueuePlan transaction key");
    let transaction = TransactionBuilder::new(
        adapter.context.network_id,
        AccountId::new(transaction_key.public_key().clone()),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    )
    .sign(transaction_key.private_key());
    let accepted =
        crate::tx::AcceptedTransaction::new_unchecked(std::borrow::Cow::Owned(transaction));
    let routing_plan = RoutingPlan::single(RoutingDecision::new(lane_id, dataspace_id));
    let candidate = CandidateDescriptor::new(&accepted, &routing_plan);
    let context = adapter.context.clone();
    let mut provider = &mut adapter;
    for _ in 0..3 {
        let unavailable = provider
            .prepare(&context, 0, &[candidate])
            .expect_err("single-route QueuePlan work must remain autonomous");
        assert_eq!(unavailable.indices(), &BTreeSet::from([0]));
        assert_eq!(
            unavailable.reason(),
            "waiting for deterministic autonomous lane authors to publish durable FIFO reservations"
        );
    }
}
