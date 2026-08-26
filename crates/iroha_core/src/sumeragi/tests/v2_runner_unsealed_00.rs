#[test]
fn canonical_body_recovery_batches_all_ordered_heights_before_gate_close() {
    let need = |height: u64| {
        let executed_block_wire_hash = Hash::new(&height.to_le_bytes());
        CanonicalExecutedBlockNeedV1 {
            height,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(
                &[b"block".as_slice(), &height.to_le_bytes()].concat(),
            )),
            finality_artifact_hash: HashOf::from_untyped_unchecked(Hash::new(
                &[b"finality".as_slice(), &height.to_le_bytes()].concat(),
            )),
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"parent state"),
                Hash::new(b"post state"),
                Hash::new(b"writes"),
                1,
                executed_block_wire_hash,
            ),
            executed_block_wire_len: 1,
            executed_block_wire_hash,
        }
    };
    let needs = (1..=8).map(need).collect::<Vec<_>>();
    let mut startup_gate_pending = true;
    let mut observed_heights = Vec::new();
    let batches = canonical_executed_block_recovery_batches(&needs, 3)
        .expect("ordered distinct recovery plan is batchable");
    for batch in batches {
        assert!(
            startup_gate_pending,
            "the Queue startup gate remains closed for every recovery batch"
        );
        assert!(batch.len() <= 3);
        observed_heights.extend(batch.iter().map(|need| need.height));
    }
    assert_eq!(observed_heights, (1..=8).collect::<Vec<_>>());
    startup_gate_pending = false;
    assert!(
        !startup_gate_pending,
        "the gate opens only after all batches"
    );
    let mut duplicated = needs;
    duplicated[4] = duplicated[3];
    assert!(canonical_executed_block_recovery_batches(&duplicated, 3).is_err());
}
#[test]
fn canonical_body_recovery_only_inserted_responses_advance_the_requester() {
    assert!(
        !canonical_recovery_ingress_advances_requester(false, V2LaneIngressOutcome::Inserted,),
        "serving another peer's valid request is not progress on our outstanding request"
    );
    assert!(
        !canonical_recovery_ingress_advances_requester(true, V2LaneIngressOutcome::Rejected,),
        "a poisoned exact response must wait for the ordinary retry cadence"
    );
    assert!(!canonical_recovery_ingress_advances_requester(
        true,
        V2LaneIngressOutcome::Duplicate,
    ));
    assert!(canonical_recovery_ingress_advances_requester(
        true,
        V2LaneIngressOutcome::Inserted,
    ));
}
#[test]
fn canonical_body_recovery_flushes_owned_effects_after_local_completion() {
    assert!(!canonical_recovery_source_work_remains(false, 0));
    assert!(
        canonical_recovery_source_work_remains(false, 1),
        "a source-owned response must flush after the final local body arrives"
    );
    assert!(canonical_recovery_source_work_remains(true, 0));
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    canonical_body_recovery_dispatch_drains_old_output_before_new_reservations
);
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    historical_recovery_cancels_completed_requests_before_exact_output_retry
);
#[test]
fn canonical_body_recovery_successor_request_gets_a_fresh_retry_deadline() {
    let started = Instant::now();
    let interval = Duration::from_secs(5);
    let inherited = deadline_after(started, interval);
    let sent_at = deadline_after(started, Duration::from_millis(4_999));
    let mut deadline = inherited;
    refresh_canonical_recovery_retry_deadline(&mut deadline, sent_at, interval, false);
    assert_eq!(
        deadline, inherited,
        "a no-op must not consume a retry interval"
    );
    refresh_canonical_recovery_retry_deadline_after_progress(
        &mut deadline,
        sent_at,
        interval,
        false,
    );
    assert_eq!(
        deadline, sent_at,
        "progress blocked by effect capacity must wake the next service turn"
    );
    refresh_canonical_recovery_retry_deadline_after_progress(
        &mut deadline,
        sent_at,
        interval,
        true,
    );
    assert_eq!(deadline, deadline_after(sent_at, interval));
    assert!(
        deadline > inherited,
        "a successor chunk cannot inherit the expiring prefix deadline"
    );
    let handed_off_at = deadline_after(sent_at, Duration::from_millis(1));
    refresh_canonical_recovery_retry_deadline(&mut deadline, handed_off_at, interval, true);
    assert_eq!(
        deadline,
        deadline_after(handed_off_at, interval),
        "transport handoff owns the full retry interval"
    );
    assert_eq!(
        canonical_recovery_idle_wait(started, started),
        IDLE_POLL,
        "an expired retry under transport backpressure must poll instead of spinning"
    );
    assert_eq!(
        canonical_recovery_idle_wait(deadline_after(started, Duration::from_millis(1)), started,),
        Duration::from_millis(1)
    );
}
#[test]
fn pending_tip_recovery_gate_precedes_lane_work_construction() {
    let constructed = Cell::new(false);
    let error = construct_after_pending_tip_application_recovery(true, false, || {
        constructed.set(true);
        Ok(())
    })
    .expect_err("incomplete pending-tip recovery must block construction");
    assert!(matches!(error, V2RunnerError::PendingTipRecoveryIncomplete));
    assert!(
        !constructed.get(),
        "the lane-work constructor must not run before strict tip repair completes"
    );
    let value = construct_after_pending_tip_application_recovery(true, true, || {
        constructed.set(true);
        Ok(7_u8)
    })
    .expect("completed pending-tip recovery may construct lane work");
    assert_eq!(value, 7);
    assert!(constructed.get());
}
#[test]
fn pending_tip_recovery_deadline_is_bounded_and_fail_closed() {
    let _status_guard = super::super::status::rbc_status_test_guard();
    super::super::status::clear_v2_status();
    let started_at = Instant::now();
    let round_timeout = Duration::from_secs(10);
    let deadline = PendingTipRecoveryDeadline::new(started_at, round_timeout)
        .expect("derive recovery deadline");
    assert_eq!(deadline.timeout, Duration::from_secs(30));
    assert!(!deadline.expired(started_at + Duration::from_secs(30) - Duration::from_nanos(1)));
    assert!(deadline.expired(started_at + Duration::from_secs(30)));
    assert_eq!(
        deadline.remaining(started_at + Duration::from_secs(29)),
        Duration::from_secs(1)
    );
    let output_guard = ConsensusOutputGuard::isolated();
    let error = pending_tip_recovery_deadline_error(
        output_guard.as_ref(),
        deadline.timeout,
        17,
        Some(PendingKuraApplyRecoveryStage::ApplicationDispatched),
    );
    assert!(output_guard.restart_required());
    assert!(matches!(
        error,
        V2RunnerError::PendingTipRecoveryDeadlineExceeded {
            timeout,
            attempts: 17,
            stage: Some(PendingKuraApplyRecoveryStage::ApplicationDispatched),
        } if timeout == Duration::from_secs(30)
    ));
    super::super::status::clear_v2_status();
}
#[test]
fn bounded_sidecar_admission_turn_applies_only_its_budget() {
    let mut queued = VecDeque::from([1_u8, 2, 3]);
    let mut applied = Vec::new();
    let count = apply_bounded_sidecar_admissions(
        1,
        || Ok::<_, ()>(queued.pop_front()),
        |item| {
            applied.push(item);
            Ok::<_, ()>(())
        },
    )
    .expect("bounded admission turn");
    assert_eq!(count, 1);
    assert_eq!(applied, vec![1]);
    assert_eq!(queued, VecDeque::from([2, 3]));
    let result = apply_bounded_sidecar_admissions(
        2,
        || Ok::<_, &'static str>(queued.pop_front()),
        |_item| Err("fail-stop acknowledgement"),
    );
    assert_eq!(result, Err("fail-stop acknowledgement"));
    assert_eq!(queued, VecDeque::from([3]));
}
#[test]
fn empty_drain_after_peek_is_restart_required_without_panicking() {
    assert!(matches!(
        require_peeked_lane_work_effect(None),
        Err(V2RunnerError::RestartRequired)
    ));
}
fn context() -> (wire::HeightContext, Vec<KeyPair>) {
    let mut keys = (1_u8..=4)
        .map(|seed| {
            KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal).expect("deterministic key")
        })
        .collect::<Vec<_>>();
    keys.sort_by(|left, right| left.public_key().cmp(right.public_key()));
    let roster = keys
        .iter()
        .map(|key| wire::ValidatorPower {
            validator: PeerId::new(key.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    (
        wire::HeightContext {
            network_id: crate::sumeragi::synthetic_network_id("v2-runner-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            snapshot_bootstrap: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"runner-test-nexus-amx"),
            execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::ReedSolomon16,
                chunk_size_bytes: 1024,
                data_shards: 1,
                parity_shards: 1,
                max_payload_size_bytes: 4096,
                max_chunk_count: 8,
            },
            leader_seed: [0x42; 32],
        },
        keys,
    )
}
fn decided_recovery_certified_request(
    context: &wire::HeightContext,
    requester_key: &KeyPair,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> wire::CertifiedBodyRequest {
    let mut request = wire::CertifiedBodyRequest {
        round,
        subject,
        certificate: wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"runner recovery parent state"),
                Hash::new(b"runner recovery post state"),
                Hash::new(b"runner recovery ordinary writes"),
                1,
                Hash::new(b"runner recovery executed block"),
            ),
            signers: (0..super::super::network_topology::commit_quorum_from_len(
                context.roster.len(),
            ))
                .map(|index| u32::try_from(index).expect("small runner roster index"))
                .collect(),
            aggregate_signature: vec![0xA5; 48],
        },
        requester: PeerId::new(requester_key.public_key().clone()),
        signature: Vec::new(),
    };
    request.signature = Signature::new(requester_key.private_key(), &request.signature_preimage())
        .payload()
        .to_vec();
    request
}
fn admitted_decided_recovery_request(
    context: &wire::HeightContext,
    request: &wire::CertifiedBodyRequest,
) -> InboundBlockMessage {
    let requester = request.requester.clone();
    let mut routes = NetworkReplyRouteTestFixture::new(requester.clone());
    let reply_route = routes.mint(requester.clone());
    super::super::fair_v2_ingress_admit_with_roster_for_test(
        InboundBlockMessage::try_from_transport_with_reply_route(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
            )),
            requester.clone(),
            requester,
            reply_route,
        )
        .expect("decided recovery fixture retains an authenticated reply route"),
        context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
    )
}
#[test]
fn kura_replica_advert_error_classification_retires_only_invalid_remote_claims() {
    assert!(matches!(
        classify_kura_replica_advert_admission_error(
            crate::kura::Error::InvalidKuraReplicaAdvert("forged advert".to_owned())
        ),
        KuraReplicaAdvertAdmissionError::InvalidAdvert(reason)
            if reason == "forged advert"
    ));
    assert!(matches!(
        classify_kura_replica_advert_admission_error(crate::kura::Error::CanonicalStoragePoisoned),
        KuraReplicaAdvertAdmissionError::Fatal(crate::kura::Error::CanonicalStoragePoisoned)
    ));
}
#[test]
fn drain_decided_lane_recovery_ingress_authorizes_terminal_current_serve() {
    let (context, keys) = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 3,
    };
    let subject = proposal_subject(b"decided recovery exact subject");
    let request = decided_recovery_certified_request(&context, &keys[1], round, subject);
    let inbound = admitted_decided_recovery_request(&context, &request);
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(&inbound, context.height),
        DecidedLaneRecoveryIngressPreparation::CurrentServe
    ));
    assert!(matches!(
        authorize_decided_lane_recovery_drain(DecidedLaneRecoveryIngressPreparation::CurrentServe),
        DecidedLaneRecoveryDrainAuthorization::CurrentServe
    ));
    assert!(DecidedLaneRecoveryServeScope::Current.permits_height(context.height, context.height));
    assert!(
        !DecidedLaneRecoveryServeScope::Current
            .permits_height(context.height.saturating_sub(1), context.height)
    );
    assert!(
        !DecidedLaneRecoveryServeScope::Current
            .permits_height(context.height.saturating_add(1), context.height)
    );
    assert!(
        DecidedLaneRecoveryServeScope::Historical
            .permits_height(context.height.saturating_sub(1), context.height)
    );
    assert!(
        !DecidedLaneRecoveryServeScope::Historical.permits_height(context.height, context.height)
    );
    assert!(DecidedLaneRecoveryServeScope::Current.permits_subject(subject, subject));
    assert!(!DecidedLaneRecoveryServeScope::Current.permits_subject(
        proposal_subject(b"losing decided recovery subject"),
        subject
    ));
    assert!(
        DecidedLaneRecoveryServeScope::Historical
            .permits_subject(proposal_subject(b"historical recovery subject"), subject)
    );

    let future_round = wire::ConsensusRound {
        height: context.height.saturating_add(1),
        ..round
    };
    let future = decided_recovery_certified_request(&context, &keys[1], future_round, subject);
    let future = InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyRequest(future),
        )),
        PeerId::new(keys[1].public_key().clone()),
    );
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(&future, context.height),
        DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
    ));
}

#[test]
fn terminal_current_serve_binds_leader_wire_before_guarded_service() {
    #[derive(Default)]
    struct CommitProbe(Vec<&'static str>);

    impl DecidedLaneRecoveryDrainCommitter for CommitProbe {
        fn commit_lane_local(&mut self) -> Result<(), V2RunnerError> {
            self.0.push("lane");
            Ok(())
        }

        fn commit_kura_replica_advert(&mut self) -> Result<(), V2RunnerError> {
            self.0.push("advert");
            Ok(())
        }

        fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError> {
            self.0.push("bind");
            Ok(())
        }

        fn commit_current_serve(&mut self) -> Result<(), V2RunnerError> {
            self.0.push("current");
            Ok(())
        }

        fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError> {
            self.0.push("historical");
            Ok(())
        }

        fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError> {
            self.0.push("volatile");
            Ok(())
        }
    }

    let mut probe = CommitProbe::default();
    let outcome = commit_decided_lane_recovery_drain(
        DecidedLaneRecoveryDrainAuthorization::CurrentServe,
        &mut probe,
    )
    .expect("terminal current Serve follows the checked commit corridor");
    assert_eq!(outcome, DecidedLaneRecoveryDrainCommitOutcome::CurrentServe);
    assert_eq!(probe.0, ["bind", "current"]);
}

#[test]
fn terminal_current_serve_source_retention_retries_without_reopening_runtime() {
    let mut source_retained = true;
    let mut retry_attempts = 0_u8;
    let runtime_turns = Cell::new(0_u8);

    let still_pending = super::lifecycle_run_inner::retry_decided_lane_recovery_exact_output(
        LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion
            .decided_lane_recovery_permit()
            .expect("Apply completion mints decided-lane recovery authority"),
        || {
            retry_attempts = retry_attempts.saturating_add(1);
            Ok(source_retained)
        },
    )
    .expect("an Apply completion barrier may retry its owned CurrentServe response");
    assert!(still_pending, "source retention remains explicit");

    source_retained = false;
    let still_pending = super::lifecycle_run_inner::retry_decided_lane_recovery_exact_output(
        LifecycleProducerClaimDispositionV1::ApplyTerminalSettled
            .decided_lane_recovery_permit()
            .expect("settled Apply mints decided-lane recovery authority"),
        || {
            retry_attempts = retry_attempts.saturating_add(1);
            Ok(source_retained)
        },
    )
    .expect("the settled Apply barrier must release the retained response owner");
    assert!(!still_pending);
    assert_eq!(retry_attempts, 2);
    assert_eq!(
        runtime_turns.get(),
        0,
        "the exact-output retry has no reducer/runtime callback"
    );

    assert!(
        LifecycleProducerClaimDispositionV1::AwaitingValidateSidecar
            .decided_lane_recovery_permit()
            .is_none(),
        "a non-Apply lane barrier cannot mint terminal response authority"
    );
    assert_eq!(
        runtime_turns.get(),
        0,
        "unauthorized retry must fail before touching the output owner"
    );
}

#[test]
fn drain_decided_lane_recovery_ingress_routes_history_and_volatile_terminal_traffic() {
    let (context, keys) = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height.saturating_sub(1),
        view: 0,
    };
    let subject = proposal_subject(b"decided recovery historical subject");
    let historical = decided_recovery_certified_request(&context, &keys[1], round, subject);
    let historical_inbound = admitted_decided_recovery_request(&context, &historical);
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(&historical_inbound, context.height),
        DecidedLaneRecoveryIngressPreparation::HistoricalServe
    ));
    let peer = context.roster[0].validator.clone();
    let non_serve = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(
        wire::PayloadChunk {
            manifest_hash: HashOf::from_untyped_unchecked(Hash::new(
                b"wrong-version recovery chunk",
            )),
            index: 0,
            bytes: vec![0xA5],
            sender: 0,
            signature: vec![0x5A],
        },
    ));
    let ordinary_non_serve = InboundBlockMessage::from_authenticated_peer(
        BlockMessage::V2(non_serve.clone()),
        peer.clone(),
    );
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(&ordinary_non_serve, context.height),
        DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
    ));
    let mut wrong_version = non_serve;
    wrong_version.protocol_version = wrong_version.protocol_version.saturating_sub(1);
    let wrong_version =
        InboundBlockMessage::from_authenticated_peer(BlockMessage::V2(wrong_version), peer.clone());
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(&wrong_version, context.height),
        DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
    ));
}
#[test]
fn drain_decided_lane_recovery_ingress_authorizes_lane_local_qc() {
    let (context, _) = context();
    let sender = context.roster[0].validator.clone();
    let inbound = InboundBlockMessage::from_authenticated_peer(valid_ingress_probe(), sender);
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(&inbound, context.height),
        DecidedLaneRecoveryIngressPreparation::LaneLocal
    ));
    assert!(matches!(
        authorize_decided_lane_recovery_drain(prepare_decided_lane_recovery_ingress(
            &inbound,
            context.height,
        )),
        DecidedLaneRecoveryDrainAuthorization::LaneLocal
    ));
}
#[test]
fn lifecycle_lane_local_selector_bypasses_only_productive_global_barrier() {
    let (_directory, ingress, gate, _global, semantic_origin) =
        queued_leader_wire_ingress_fixture();
    let global_scheduler_ordinal = gate
        .earliest_ingress_scheduler_ordinal()
        .expect("read the productive global ingress owner")
        .expect("the queued Proposal owns the durable leader-wire barrier");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            super::super::v2_worker::tests::lane_commit_qc_block_message(semantic_origin.clone()),
            semantic_origin,
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));

    assert!(
        ingress
            .try_recv_if_checked(|inbound| inbound.message().is_lane_local())
            .expect("ordinary selection preserves the productive gate")
            .is_none(),
        "the leader-wire barrier blocks a later lane-local occurrence on the ordinary path"
    );
    let selected = ingress
        .try_recv_lifecycle_lane_local_checked(
            LifecycleProducerClaimDispositionV1::AwaitingCompletion
                .blocked_ordinary_lane_local_ingress_permit()
                .expect("Broadcast worker ownership blocks only ordinary global ingress"),
        )
        .expect("lifecycle lane-local selection preserves checked dequeue failures")
        .expect("the later lane-local occurrence remains selectable");
    assert!(selected.message().is_lane_local());
    let selected_ownership = selected
        .ingress_ownership()
        .expect("selected lane occurrence retains ingress ownership");
    assert_eq!(selected_ownership.first.physical_admission_ordinal, 2);
    assert!(selected_ownership.validate_exact());
    assert!(selected_ownership.leader_wire_token().is_none());
    assert!(selected_ownership.leader_wire_runtime_receipt().is_none());
    assert_eq!(ingress.len(), 1);
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("read the retained productive global ingress owner"),
        Some(global_scheduler_ordinal),
        "lane-local service must not retire or runtime-bind the global barrier"
    );

    let remaining = ingress
        .try_recv()
        .expect("the blocked global occurrence remains queued exactly once");
    assert!(!remaining.message().is_lane_local());
    let remaining_ownership = remaining
        .ingress_ownership()
        .expect("remaining global occurrence retains ingress ownership");
    assert_eq!(remaining_ownership.first.physical_admission_ordinal, 1);
    assert!(remaining_ownership.leader_wire_token().is_some());
    assert!(remaining_ownership.leader_wire_runtime_receipt().is_some());
    assert_eq!(ingress.len(), 0);
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("read the runtime-bound productive owner"),
        None
    );
}

fn queued_leader_wire_ingress_fixture() -> (
    TempDir,
    FairV2Ingress,
    Arc<LeaderWireLifecycleStoreGate>,
    wire::ConsensusMessageV2,
    PeerId,
) {
    let (context, keys) = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let body = b"runner authenticated-Coalesce defense".to_vec();
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"runner authenticated-Coalesce block",
        )),
        payload_hash: Hash::new(&body),
    };
    let manifest = encode_payload(&context, round, subject, &body)
        .expect("encode runner leader-wire fixture payload")
        .manifest()
        .clone();
    let proposer = context.leader(round.view);
    let mut proposal = wire::Proposal {
        round,
        proposer,
        subject,
        manifest,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    proposal.signature = Signature::new(
        keys[usize::try_from(proposer).expect("small runner proposer index")].private_key(),
        &proposal.signature_preimage(),
    )
    .payload()
    .to_vec();
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal));
    let semantic_origin = context.roster
        [usize::try_from(proposer).expect("small runner proposer index")]
    .validator
    .clone();
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let directory = TempDir::new().expect("temporary runner leader-wire directory");
    let ingress = FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
        64,
        512 * 1024 * 1024,
        64 * 1024 * 1024,
        super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        8 * 1024 * 1024,
        8 * 1024 * 1024,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        None,
    );
    ingress
        .configure_roster_for_context(roster.clone(), &context.network_id, context.da_layout)
        .expect("configure runner leader-wire ingress");
    ingress.require_leader_wire_lifecycle_gate();
    let capacity = LeaderWireLifecycleStoreGate::derived_capacity(
        roster.len(),
        context.da_layout.max_chunk_count,
    )
    .expect("derive runner leader-wire capacity");
    let owner = [0xD4; 32];
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            owner,
            round.view,
            false,
        );
    let (gate, restore) = LeaderWireLifecycleStoreGate::open(
        &directory.path().join("runner-leader-wire.wal"),
        context.id(),
        context.height,
        owner,
        roster.iter().cloned().collect(),
        capacity,
        context.da_layout.max_chunk_count,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open runner leader-wire gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(64),
            context.id(),
            context.height,
        )
        .expect("bind runner leader-wire gate");
    ingress.open().expect("open runner leader-wire ingress");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message.clone()),
            semantic_origin.clone(),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
    (directory, ingress, gate, message, semantic_origin)
}

fn leader_wire_runtime_ingress_fixture() -> (
    TempDir,
    FairV2Ingress,
    Arc<LeaderWireLifecycleStoreGate>,
    FairV2IngressOwnershipEvidence,
    wire::ConsensusMessageV2,
    PeerId,
) {
    let (directory, ingress, gate, message, semantic_origin) = queued_leader_wire_ingress_fixture();
    let mut admitted = ingress
        .try_recv()
        .expect("drain runner leader-wire fixture");
    let mut ownership = admitted
        .take_ingress_ownership()
        .expect("runner leader-wire fixture retains ingress ownership");
    ingress
        .bind_leader_wire_runtime_ownership(&mut ownership)
        .expect("bind runner leader-wire runtime receipt");
    (
        directory,
        ingress,
        gate,
        ownership,
        message,
        semantic_origin,
    )
}
#[test]
fn fail_closed_authenticated_coalesce_releases_gate_and_suppresses_retry() {
    let (_directory, ingress, gate, ownership, message, semantic_origin) =
        leader_wire_runtime_ingress_fixture();
    assert!(
        ownership.leader_wire_runtime_receipt().is_some(),
        "the synthetic stale Coalesce result carries a fresh runtime receipt"
    );
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("read runner leader-wire minimum"),
        None,
        "a runtime-bound owner has already left the durable Ingress selector"
    );
    assert!(matches!(
        complete_control_ingress_admission(
            &ingress,
            &ownership,
            Err(NetworkIngressError::FailClosed),
        ),
        Err(V2RunnerError::RuntimeFailClosed)
    ));
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("read retired runner leader-wire minimum"),
        None
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message),
            semantic_origin,
        )),
        Ok(super::super::FairV2IngressPushDisposition::Coalesced)
    ));
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("retry retains the terminal tombstone"),
        None
    );
}
#[test]
fn authentication_rejection_volatile_terminalizes_exact_leader_wire() {
    let (_directory, ingress, gate, ownership, message, semantic_origin) =
        leader_wire_runtime_ingress_fixture();
    let token = ownership
        .leader_wire_token()
        .expect("the runner fixture owns one productive token")
        .clone();
    complete_control_ingress_admission(
        &ingress,
        &ownership,
        Err(NetworkIngressError::Authentication(
            super::super::v2::AdapterError::AuthenticatedTimeoutVoteOriginMismatch {
                signer: 1,
                semantic_origin: semantic_origin.clone(),
            },
        )),
    )
    .expect("remote authentication rejection is nonfatal");
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles[&token.slot].status,
        super::super::FairV2IngressLeaderWireStatus::VolatileTerminal
    );
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("read volatile-terminal runner leader-wire minimum"),
        None
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(message),
            semantic_origin,
        )),
        Ok(super::super::FairV2IngressPushDisposition::Coalesced)
    ));
}
fn test_predecessor(context: &wire::HeightContext, label: &[u8]) -> DurableV2PredecessorIdentity {
    DurableV2PredecessorIdentity::for_test(context.height, label)
}
fn test_successor_authority(
    predecessor: DurableV2PredecessorIdentity,
    successor_context_id: wire::HeightContextId,
) -> DurableSuccessorActivationAuthority {
    DurableSuccessorActivationAuthority::for_test(predecessor, successor_context_id)
}
fn test_recovered_complete_tip_authority(
    context: &wire::HeightContext,
    successor_context_id: wire::HeightContextId,
    label: &[u8],
    predecessor_root: &std::path::Path,
) -> RecoveredCompleteTipActivationAuthority {
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            [b"recovered complete-tip block", label].concat(),
        )),
        payload_hash: Hash::new([b"recovered complete-tip payload", label].concat()),
    };
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let artifact = wire::finality::V2FinalityArtifact::new(
        context.clone(),
        subject,
        wire::QuorumCertificate {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Commit,
            subject,
            execution_commitment: wire::ExecutionCommitment::without_topups_or_merge_carrier(
                Hash::new(b"recovered complete-tip parent state"),
                Hash::new(b"recovered complete-tip post state"),
                Hash::new(b"recovered complete-tip writes"),
                1,
                Hash::new(b"recovered complete-tip executed block"),
            ),
            signers: Vec::new(),
            aggregate_signature: Vec::new(),
        },
        Vec::new(),
    );
    let receipt = KuraV2CommitReceipt::for_test(&artifact);
    let predecessor = DurableV2PredecessorIdentity::authenticate(&artifact, &receipt)
        .expect("synthetic complete-tip artifact and receipt match exactly");
    RecoveredCompleteTipActivationAuthority::authenticate_for_lifecycle_test(
        artifact,
        receipt,
        successor_context_id,
        test_successor_authority(predecessor, successor_context_id),
        predecessor_root,
    )
    .expect("synthetic complete-tip activation matches its exact durable evidence")
}
fn valid_ingress_probe() -> BlockMessage {
    let validator = PeerId::new(
        KeyPair::try_from_seed(vec![0xD7; 32], Algorithm::BlsNormal)
            .expect("deterministic ingress probe key")
            .public_key()
            .clone(),
    );
    super::super::v2_worker::tests::lane_commit_qc_block_message(validator)
}
fn runner_status(context: &wire::HeightContext) -> wire::SumeragiV2Status {
    wire::SumeragiV2Status {
        protocol_version: wire::PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"runner status node"),
        build_fingerprint: Hash::new(b"runner status build"),
        config_fingerprint: Hash::new(b"runner status config"),
        restart_required: false,
        height_context_id: context.id(),
        height: context.height,
        view: 0,
        phase: wire::SumeragiV2StatusPhase::AwaitingProposal,
        leader: context.leader(0),
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: wire::SumeragiV2BodyState::Missing,
        pending_persistence_id: None,
        last_committed_height: context.height.saturating_sub(1),
        last_committed_subject: None,
        height_context: wire::SumeragiV2HeightContextStatus {
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            mode: context.mode,
            epoch_seed: context.leader_seed,
            validator_count: u32::try_from(context.roster.len()).expect("validator count"),
            quorum: context.quorum,
        },
        last_commit_qc: None,
        liveness: Default::default(),
    }
}
fn publish_applied_runner_status(context: &wire::HeightContext) {
    let mut status = runner_status(context);
    status.phase = wire::SumeragiV2StatusPhase::PendingApply;
    status.body_state = wire::SumeragiV2BodyState::Applied;
    status.liveness.generation = context.height;
    status.liveness.work.application = wire::SumeragiV2LocalWorkStage::Complete;
    status.liveness.work.successor_height = wire::SumeragiV2LocalWorkStage::Queued;
    status.liveness.last_progress = Some(wire::SumeragiV2ProgressTransitionStatus {
        generation: context.height,
        round: wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 0,
        },
        transition: wire::SumeragiV2ProgressTransition::Applied,
        age_ms: 0,
    });
    super::super::status::set_v2_status(status);
}
fn labelled_lane_qc_message(peer: PeerId, label: &[u8]) -> BlockMessage {
    let mut message = super::super::v2_worker::tests::lane_commit_qc_block_message(peer);
    let BlockMessage::LaneBlockQc(qc) = &mut message else {
        unreachable!("lane-QC fixture must return a lane CommitQC")
    };
    qc.body.proposal_hash = Hash::new(label);
    message
}
fn lane_qc_label(message: &NetworkMessage) -> Hash {
    let NetworkMessage::SumeragiBlock(wire) = message else {
        panic!("runner scheduler fixture emitted a non-block network message")
    };
    let BlockMessage::LaneBlockQc(qc) = wire.as_message() else {
        panic!("runner scheduler fixture emitted a non-lane-QC block message")
    };
    qc.body.proposal_hash.clone()
}
fn runner_sidecar_chunk(
    local: PeerId,
    requester: PeerId,
    label: &[u8],
) -> CertifiedMergeSidecarChunkV1 {
    CertifiedMergeSidecarChunkV1 {
        version: CERTIFIED_MERGE_SIDECAR_VERSION_V1,
        service_generation: CertifiedMergeSidecarServiceGenerationV1::INITIAL,
        stream_epoch: CertifiedMergeSidecarStreamEpochV1(
            NonZeroU64::new(1).expect("runner sidecar stream epoch is non-zero"),
        ),
        semantic_sequence: CertifiedMergeSidecarSemanticSequenceV1(
            NonZeroU64::new(1).expect("runner semantic sequence is non-zero"),
        ),
        request_id: Hash::new(label),
        entry_hash: HashOf::from_untyped_unchecked(Hash::new(b"runner sidecar entry")),
        encoded_len: 4,
        epoch_id: 7,
        reference_digest: Hash::new(b"runner sidecar reference"),
        requester,
        responder: local,
        chunk_index: 0,
        chunk_count: 1,
        bytes: vec![1, 2, 3, 4],
    }
}
#[test]
fn reserved_lane_output_bypasses_unserviceable_head_without_losing_owner() {
    let (mut services, keys) = super::super::v2_worker::tests::fixture();
    services
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install one shared slot plus frozen-validator reservations");
    let blocked = PeerId::new(keys[1].public_key().clone());
    let responsive = PeerId::new(keys[2].public_key().clone());
    let keep_blocked = Arc::new(AtomicBool::new(true));
    let keep_blocked_for_hook = Arc::clone(&keep_blocked);
    let blocked_for_hook = blocked.clone();
    let admitted = Arc::new(Mutex::new(Vec::new()));
    let admitted_for_hook = Arc::clone(&admitted);
    services.set_exact_output_admission_hook(move |post, ticket| {
        if post.peer_id == blocked_for_hook && keep_blocked_for_hook.load(Ordering::Acquire) {
            return Err(NetworkActorAdmissionError::Backpressured {
                message: post,
                ticket,
                rank: 13,
            });
        }
        admitted_for_hook
            .lock()
            .expect("record admitted lane output")
            .push((post.peer_id.clone(), lane_qc_label(&post.data)));
        Ok(())
    });
    let reserved_filler = Hash::new(b"runner reserved filler");
    let shared_filler = Hash::new(b"runner shared filler");
    for (label, expected) in [
        (
            b"runner reserved filler".as_slice(),
            reserved_filler.clone(),
        ),
        (b"runner shared filler".as_slice(), shared_filler.clone()),
    ] {
        services
            .post_lane_block(
                blocked.clone(),
                labelled_lane_qc_message(blocked.clone(), label),
            )
            .expect("blocked validator output remains exactly owned");
        assert!(
            services
                .has_pending_exact_output()
                .expect("inspect blocked exact output")
        );
        assert!(
            admitted
                .lock()
                .expect("inspect admitted output")
                .iter()
                .all(|(_, actual)| actual != &expected)
        );
    }
    let blocked_label = Hash::new(b"runner blocked effect A");
    let responsive_label = Hash::new(b"runner reserved effect B");
    let blocked_effect = V2LaneWorkEffect::PostLaneBlock {
        peer: blocked.clone(),
        message: labelled_lane_qc_message(blocked.clone(), b"runner blocked effect A"),
    };
    let responsive_effect = V2LaneWorkEffect::PostLaneBlock {
        peer: responsive.clone(),
        message: labelled_lane_qc_message(responsive.clone(), b"runner reserved effect B"),
    };
    let (mut lane_work, _) =
        super::super::v2_lane_work::tests::fixture(wire::ConsensusMode::Permissioned);
    assert!(lane_work.requeue_effect(blocked_effect));
    assert!(lane_work.requeue_effect(responsive_effect));
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("reserved work bypasses the unserviceable head");
    assert_eq!(lane_work.effect_count(), 1);
    match lane_work.next_effect() {
        Some(V2LaneWorkEffect::PostLaneBlock {
            peer,
            message: BlockMessage::LaneBlockQc(qc),
        }) => {
            assert_eq!(peer, blocked);
            assert_eq!(qc.body.proposal_hash, blocked_label);
        }
        other => panic!("blocked effect A must remain the exact queued owner: {other:?}"),
    }
    {
        let admitted = admitted.lock().expect("inspect admitted output");
        assert_eq!(
            admitted
                .iter()
                .filter(|(peer, label)| peer == &responsive && label == &responsive_label)
                .count(),
            1
        );
        assert!(admitted.iter().all(|(_, label)| label != &blocked_label));
    }
    keep_blocked.store(false, Ordering::Release);
    assert!(
        !services
            .retry_pending_exact_output()
            .expect("responsive retry drains both retained fillers")
    );
    dispatch_lane_work_effects(&mut lane_work, &services, 1)
        .expect("the retained head dispatches after capacity reopens");
    assert_eq!(lane_work.effect_count(), 0);
    assert!(
        !services
            .has_pending_exact_output()
            .expect("all exact lane output is admitted")
    );
    let admitted = admitted.lock().expect("inspect final admitted output");
    for (peer, label) in [
        (&blocked, &reserved_filler),
        (&blocked, &shared_filler),
        (&blocked, &blocked_label),
        (&responsive, &responsive_label),
    ] {
        assert_eq!(
            admitted
                .iter()
                .filter(|(actual_peer, actual_label)| {
                    actual_peer == peer && actual_label == label
                })
                .count(),
            1,
            "each semantic output must be admitted exactly once"
        );
    }
    assert_eq!(admitted.len(), 4);
}
#[test]
fn finalized_rollover_drains_source_effects_after_handoff_reopens_capacity() {
    let fixture = super::super::v2_lane_work::tests::certified_sidecar_server_fixture();
    let mut lane_work = fixture.adapter;
    let mut services =
        super::super::v2_worker::tests::service_for_history_context_with_local_validator(
            Arc::clone(&fixture.kura),
            fixture.context,
            &fixture.validators,
            fixture.local_validator,
        );
    services
        .set_exact_output_shared_unit_capacity_for_test(1)
        .expect("install one shared exact-output slot");
    services.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 17,
        })
    });
    let (receipt, artifact) =
        super::super::v2_worker::tests::durable_finality_fixture(&services, &fixture.validators);
    let lane_authority = DurableLaneRolloverAuthority::missing_winning_witness_for_test(
        &artifact,
        Hash::new(b"rollover source-effect capacity witness"),
    );
    let local = fixture.request.responder.clone();
    let remote = fixture.request.requester.clone();
    let outbound = |sequence: u64| {
        let mut request = fixture.request.clone();
        request.requester = local.clone();
        request.responder = remote.clone();
        request.semantic_sequence = CertifiedMergeSidecarSemanticSequenceV1(
            NonZeroU64::new(sequence).expect("non-zero rollover request sequence"),
        );
        request.request_id = request.canonical_request_id();
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: remote.clone(),
            reply_routes: None,
            message: Arc::new(CertifiedMergeSidecarMessage::Request(request)),
        }
    };
    let current = outbound(100);
    let mut retained = 0_u64;
    while services
        .can_retain_lane_work_effect(&current)
        .expect("inspect exact-output rollover capacity")
    {
        assert!(matches!(
            dispatch_lane_work_effect(&services, outbound(retained.saturating_add(1)))
                .expect("retain one actor-backpressured predecessor request"),
            LaneWorkEffectDispatch::Complete
        ));
        retained = retained.saturating_add(1);
        assert!(retained < 8, "the exact-output fixture remains bounded");
    }
    assert_ne!(retained, 0, "the fixture must retain predecessor output");
    assert!(
        services
            .has_pending_exact_output()
            .expect("inspect retained predecessor output")
    );
    assert!(lane_work.requeue_effect(current));

    drain_finalized_lane_work_output(
        &mut lane_work,
        &services,
        &receipt,
        &artifact,
        &lane_authority,
        1,
    )
    .expect("durable handoff frees capacity for every retained source effect");
    assert_eq!(lane_work.effect_count(), 0);
    assert!(
        !services
            .has_pending_exact_output()
            .expect("all finalized exact output crosses durable handoff")
    );
}
#[test]
fn runner_dispatch_preserves_durable_lane_certificate_reply_routes() {
    let history = super::super::v2_lane_work::tests::durable_lane_history_fixture();
    let requester = history
        .certificate
        .commit_qc
        .validator_set
        .iter()
        .find(|peer| peer.public_key() != history.validators[0].public_key())
        .cloned()
        .expect("durable lane fixture has a remote requester");
    let mut services = super::super::v2_worker::tests::service_for_history_context(
        Arc::clone(&history.kura),
        history.context,
        &history.validators,
    );
    let dispatch_attempts = Arc::new(AtomicUsize::new(0));
    let dispatch_attempts_for_hook = Arc::clone(&dispatch_attempts);
    services.set_exact_output_admission_hook(move |post, ticket| {
        dispatch_attempts_for_hook.fetch_add(1, Ordering::Relaxed);
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let route_a = route_fixture.mint_via(requester.clone(), hub_a.clone());
    let route_b = route_fixture.mint_via(requester.clone(), hub_b.clone());
    assert!(route_a.source_key() != route_b.source_key());
    let mut reply_routes = NetworkReplyRoutes::try_from_route(route_a.clone())
        .expect("first authenticated durable-response source");
    reply_routes
        .merge(
            &NetworkReplyRoutes::try_from_route(route_b.clone())
                .expect("second authenticated durable-response source"),
        )
        .expect("attach the independent durable-response source");
    let mut admitted_a = super::super::fair_v2_ingress_admit_for_test(
        InboundBlockMessage::try_from_transport_with_reply_route(
            BlockMessage::LaneBlockProposal(history.certificate.proposal.clone()),
            requester.clone(),
            hub_a,
            route_a.clone(),
        )
        .expect("first durable request route binds its fair-ingress occurrence"),
    );
    let mut ingress_ownership = admitted_a
        .take_ingress_ownership()
        .expect("fair ingress supplies first exact durable-request ownership");
    let mut admitted_b = super::super::fair_v2_ingress_admit_for_test(
        InboundBlockMessage::try_from_transport_with_reply_route(
            BlockMessage::LaneBlockProposal(history.certificate.proposal.clone()),
            requester.clone(),
            hub_b,
            route_b.clone(),
        )
        .expect("second durable request route binds its fair-ingress occurrence"),
    );
    assert!(
        ingress_ownership.merge_downstream(
            admitted_b
                .take_ingress_ownership()
                .expect("fair ingress supplies second exact durable-request ownership")
        ),
        "independent authenticated sources merge under one semantic request identity"
    );
    assert!(ingress_ownership.validate_exact());
    assert!(ingress_ownership.matches_reply_routes(Some(&reply_routes)));
    let mut effect = V2LaneWorkEffect::PostDurableLaneCertificate {
        peer: requester,
        reply_routes: Some(reply_routes),
        ingress_ownership: Some(ingress_ownership),
        certificate: history.certificate,
    };
    assert!(retain_active_owned_reply_routes_after_snapshot(
        &mut effect,
        || assert!(route_fixture.retire(&route_a))
    ));
    assert!(!route_a.is_active());
    assert!(route_b.is_active());
    match &effect {
        V2LaneWorkEffect::PostDurableLaneCertificate {
            reply_routes: Some(routes),
            ingress_ownership: Some(ownership),
            ..
        } => {
            assert_eq!(routes.len(), 2);
            assert!(routes.iter().any(|route| route.same_delivery(&route_a)));
            assert!(routes.iter().any(|route| route.same_delivery(&route_b)));
            assert!(ownership.validate_exact());
            assert!(ownership.matches_reply_routes(Some(routes)));
        }
        other => panic!("durable response lost exact route ownership: {other:?}"),
    }
    dispatch_lane_work_effect(&services, effect)
        .expect("runner hands the Kura-backed certificate to exact output");
    assert!(
        !services
            .retains_reply_route_for_test(&route_a)
            .expect("inspect retired durable certificate route")
    );
    assert!(
        services
            .retains_reply_route_for_test(&route_b)
            .expect("inspect retained sibling durable certificate route")
    );
    assert_eq!(
        dispatch_attempts.load(Ordering::Relaxed),
        1,
        "only the responsive authenticated source may reach exact-output dispatch"
    );
}
#[test]
fn runner_dispatch_preserves_certified_sidecar_chunk_reply_routes() {
    let (mut services, keys) = super::super::v2_worker::tests::fixture();
    services.set_exact_output_admission_hook(|post, ticket| {
        Err(NetworkActorAdmissionError::Backpressured {
            message: post,
            ticket,
            rank: 1,
        })
    });
    let local = PeerId::new(keys[0].public_key().clone());
    let requester = PeerId::new(keys[1].public_key().clone());
    let hub = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::new(hub);
    let route = route_fixture.mint(requester.clone());
    let reply_routes =
        NetworkReplyRoutes::try_from_route(route.clone()).expect("live reply route set");
    let chunk = runner_sidecar_chunk(local, requester.clone(), b"runner sidecar request");
    dispatch_lane_work_effect(
        &services,
        V2LaneWorkEffect::PostCertifiedMergeSidecar {
            peer: requester,
            reply_routes: Some(reply_routes),
            message: Arc::new(CertifiedMergeSidecarMessage::Chunk(chunk)),
        },
    )
    .expect("runner hands the certified chunk to exact output");
    assert!(
        services
            .retains_reply_route_for_test(&route)
            .expect("inspect retained sidecar route")
    );
}
