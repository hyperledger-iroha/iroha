#[test]
#[allow(clippy::too_many_lines)]
fn complete_certified_serve_episode_cannot_veto_pacemaker() {
    let calls = Cell::new(0_u8);
    for older_runtime_episode_claimed in [true, false] {
        service_certified_serve_barrier_pacemaker_turn(
            false,
            older_runtime_episode_claimed,
            || {
                calls.set(calls.get().saturating_add(1));
                Ok::<(), ()>(())
            },
        )
        .expect("live certified Serve barrier services one pacemaker turn");
    }
    assert_eq!(
        calls.get(),
        2,
        "a Complete predecessor episode must service the pacemaker exactly like a newly claimed episode"
    );

    service_certified_serve_barrier_pacemaker_turn(false, false, || {
        calls.set(calls.get().saturating_add(1));
        Err::<(), _>("typed pacemaker failure")
    })
    .expect_err("live runner propagates a typed pacemaker failure");
    assert_eq!(calls.get(), 3);

    service_certified_serve_barrier_pacemaker_turn(true, false, || {
        calls.set(calls.get().saturating_add(1));
        Ok::<(), ()>(())
    })
    .expect("interrupted-tip recovery does not arm a fresh pacemaker");
    assert_eq!(calls.get(), 3);

    #[cfg(feature = "bls")]
    {
        let mut recovery =
            super::super::v2_worker::tests::SelectedServeTimeoutRecoveryFixture::new();
        for _ in 0..16 {
            let older_runtime_episode_claimed = recovery
                .service_exact_serve_runtime_prefix()
                .expect("service the exact selected-Serve runtime prefix");
            service_certified_serve_barrier_liveness_turn(
                false,
                older_runtime_episode_claimed,
                |action| match action {
                    CertifiedServeBarrierLivenessAction::TimeoutVoteEpisode => {
                        recovery.service_timeout_vote_episode()
                    }
                    CertifiedServeBarrierLivenessAction::TimeoutRecoveryPrefix => {
                        recovery.service_timeout_recovery_prefix()
                    }
                    CertifiedServeBarrierLivenessAction::Pacemaker => recovery.service_pacemaker(),
                },
            )
            .expect("the selected-Serve suffix retains typed timeout recovery");
            if recovery.entered_view_one() {
                break;
            }
        }
        recovery.assert_complete();

        let mut late_passive_fetch =
            super::super::v2_worker::tests::SelectedServeTimeoutRecoveryFixture::new_late_passive_fetch();
        late_passive_fetch.assert_late_passive_fetch_completion_reopens_selected_serve();
    }
}

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
fn dormant_live_serve_debt_latches_restart_instead_of_waiting_for_requester() {
    let (mut services, _) = super::super::v2_worker::tests::fixture();
    assert!(!services.exact_output_restart_required_for_test());
    let reason = services.fail_closed_dormant_certified_serve(41);
    assert!(reason.contains("41"));
    assert!(reason.contains("restart is required for local discharge"));
    assert!(
        services.exact_output_restart_required_for_test(),
        "a carrierless live Serve lifecycle must restart into local startup discharge"
    );
}

#[test]
fn committed_lane_status_publisher_retries_revision_drift_without_publication() {
    let _guard = super::super::status::rbc_status_test_guard();
    super::super::status::clear_v2_status();
    let revision = Cell::new((1_u64, 1_u64, 1_u64));
    let mut publisher = CommittedLaneStatusPublisher::default();

    assert!(publisher.publish_if_changed_with(|| revision.get(), Vec::new));
    assert_eq!(publisher.published_revision, Some((1, 1, 1)));

    revision.set((2, 1, 1));
    let projection_ran = Cell::new(false);
    assert!(
        !publisher.publish_if_changed_with(
            || revision.get(),
            || {
                projection_ran.set(true);
                revision.set((3, 1, 1));
                Vec::new()
            },
        ),
        "a projection spanning two revisions must not replace the global status root"
    );
    assert!(projection_ran.get());
    assert_eq!(
        publisher.published_revision,
        Some((1, 1, 1)),
        "revision drift must retain the prior acknowledgement for retry"
    );
    assert!(
        super::super::status::committed_lane_blocks_snapshot().is_empty(),
        "revision drift must retain the prior global status root"
    );

    assert!(
        publisher.publish_if_changed_with(|| revision.get(), Vec::new),
        "the next stable runner edge must retry the newer revision"
    );
    assert_eq!(publisher.published_revision, Some((3, 1, 1)));
    super::super::status::clear_v2_status();
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
            chain_id: ChainId::from("v2-runner-test"),
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
            signers: (0..context.roster.len())
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
    super::super::fair_v2_ingress_admit_with_roster_for_test(
        InboundBlockMessage::from_transport(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request.clone()),
            )),
            requester.clone(),
            requester,
        ),
        context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
    )
}

enum RecordedPrepareOutcome {
    Admitted(u8),
    Backpressure,
    Rejected(&'static str),
    Service(&'static str),
}

struct RecordingDecidedLaneAuthorizer {
    calls: Vec<&'static str>,
    stage_error: Option<&'static str>,
    prepare_outcome: Option<RecordedPrepareOutcome>,
}

impl RecordingDecidedLaneAuthorizer {
    fn new(prepare_outcome: RecordedPrepareOutcome) -> Self {
        Self {
            calls: Vec::new(),
            stage_error: None,
            prepare_outcome: Some(prepare_outcome),
        }
    }
}

impl DecidedLaneRecoveryDrainAuthorizer for RecordingDecidedLaneAuthorizer {
    type Admission = u8;

    fn stage_negative(
        &mut self,
        _request_hash: HashOf<wire::CertifiedBodyRequest>,
        _outcome: CertifiedServeNegativeOutcome,
    ) -> Result<(), String> {
        self.calls.push("stage-negative");
        self.stage_error
            .map_or(Ok(()), |reason| Err(reason.to_owned()))
    }

    fn prepare_exact(
        &mut self,
        _authenticated_via: &PeerId,
        _request: AuthenticatedCertifiedBodyRequest,
    ) -> Result<Self::Admission, CertifiedServePrepareError> {
        self.calls.push("prepare-exact");
        match self
            .prepare_outcome
            .take()
            .expect("recording authorizer is called at most once")
        {
            RecordedPrepareOutcome::Admitted(admission) => Ok(admission),
            RecordedPrepareOutcome::Backpressure => Err(CertifiedServePrepareError::Backpressure),
            RecordedPrepareOutcome::Rejected(reason) => {
                Err(CertifiedServePrepareError::Rejected(reason.to_owned()))
            }
            RecordedPrepareOutcome::Service(reason) => {
                Err(CertifiedServePrepareError::Service(reason.to_owned()))
            }
        }
    }
}

struct RecordingDecidedLaneCommitter {
    calls: Vec<&'static str>,
    fail_on: Option<&'static str>,
}

impl RecordingDecidedLaneCommitter {
    fn new(fail_on: Option<&'static str>) -> Self {
        Self {
            calls: Vec::new(),
            fail_on,
        }
    }

    fn record(&mut self, operation: &'static str) -> Result<(), V2RunnerError> {
        self.calls.push(operation);
        if self.fail_on == Some(operation) {
            Err(V2RunnerError::Service(format!("{operation} failed")))
        } else {
            Ok(())
        }
    }
}

impl DecidedLaneRecoveryDrainCommitter for RecordingDecidedLaneCommitter {
    type Admission = u8;

    fn commit_lane_local(&mut self) -> Result<(), V2RunnerError> {
        self.record("lane-local")
    }

    fn commit_kura_replica_advert(&mut self) -> Result<(), V2RunnerError> {
        self.record("kura-replica-advert")
    }

    fn commit_current_serve(
        &mut self,
        current: DecidedLaneRecoveryCurrentDrain<Self::Admission>,
    ) -> Result<(), V2RunnerError> {
        match current {
            DecidedLaneRecoveryCurrentDrain::Admitted(7) => self.record("serve-exact-decided"),
            DecidedLaneRecoveryCurrentDrain::Rejected(_) => self.record("retire-staged-negative"),
            DecidedLaneRecoveryCurrentDrain::Admitted(other) => Err(V2RunnerError::Service(
                format!("unexpected test admission {other}"),
            )),
        }
    }

    fn bind_leader_wire(&mut self) -> Result<(), V2RunnerError> {
        self.record("bind-leader-wire")
    }

    fn commit_historical_serve(&mut self) -> Result<(), V2RunnerError> {
        self.record("serve-history")
    }

    fn commit_leader_wire_volatile(&mut self) -> Result<(), V2RunnerError> {
        self.record("retire-volatile")
    }
}

#[test]
fn decided_lane_checked_drain_requires_staged_or_prepared_outcome() {
    let (context, keys) = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 3,
    };
    let subject = proposal_subject(b"decided drain authorization subject");
    let decided_subject = proposal_subject(b"decided drain authorization winner");
    let request = decided_recovery_certified_request(&context, &keys[1], round, subject);
    let inbound = admitted_decided_recovery_request(&context, &request);
    let prepare = |winner| {
        prepare_decided_lane_recovery_ingress(
            &inbound,
            context.height,
            winner,
            |request, sender| {
                super::super::v2_transport::authenticate_certified_body_request(
                    &context,
                    request,
                    sender,
                    |_, _| Ok::<(), String>(()),
                )
                .map_err(|error| error.to_string())
            },
        )
    };

    let mut staged = RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Backpressure);
    let staged_decision =
        authorize_decided_lane_recovery_drain(prepare(decided_subject), &mut staged);
    assert!(matches!(
        staged_decision,
        DecidedLaneRecoveryDrainDecision::Authorized(
            DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                DecidedLaneRecoveryCurrentDrain::Rejected(_)
            )
        )
    ));
    staged.calls.push("checked-drain");
    assert_eq!(staged.calls, ["stage-negative", "checked-drain"]);

    let mut failed_stage =
        RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Backpressure);
    failed_stage.stage_error = Some("durable negative write failed");
    assert!(matches!(
        authorize_decided_lane_recovery_drain(
            prepare(decided_subject),
            &mut failed_stage,
        ),
        DecidedLaneRecoveryDrainDecision::FailClosed(reason)
            if reason == "durable negative write failed"
    ));
    assert_eq!(failed_stage.calls, ["stage-negative"]);

    let mut admitted = RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Admitted(7));
    assert!(matches!(
        authorize_decided_lane_recovery_drain(prepare(subject), &mut admitted),
        DecidedLaneRecoveryDrainDecision::Authorized(
            DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                DecidedLaneRecoveryCurrentDrain::Admitted(7)
            )
        )
    ));
    admitted.calls.push("checked-drain");
    assert_eq!(admitted.calls, ["prepare-exact", "checked-drain"]);

    let mut typed_rejection = RecordingDecidedLaneAuthorizer::new(
        RecordedPrepareOutcome::Rejected("typed negative was staged"),
    );
    assert!(matches!(
        authorize_decided_lane_recovery_drain(prepare(subject), &mut typed_rejection),
        DecidedLaneRecoveryDrainDecision::Authorized(
            DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                DecidedLaneRecoveryCurrentDrain::Rejected(reason)
            )
        ) if reason == "typed negative was staged"
    ));
    typed_rejection.calls.push("checked-drain");
    assert_eq!(typed_rejection.calls, ["prepare-exact", "checked-drain"]);

    let mut backpressured =
        RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Backpressure);
    assert!(matches!(
        authorize_decided_lane_recovery_drain(prepare(subject), &mut backpressured),
        DecidedLaneRecoveryDrainDecision::Retain
    ));
    assert_eq!(backpressured.calls, ["prepare-exact"]);

    let mut failed_prepare = RecordingDecidedLaneAuthorizer::new(RecordedPrepareOutcome::Service(
        "Serve preparation failed",
    ));
    assert!(matches!(
        authorize_decided_lane_recovery_drain(prepare(subject), &mut failed_prepare),
        DecidedLaneRecoveryDrainDecision::FailClosed(reason)
            if reason == "Serve preparation failed"
    ));
    assert_eq!(
        failed_prepare.calls,
        ["prepare-exact"],
        "service failure cannot authorize checked dequeue"
    );
}

#[test]
fn decided_lane_commit_orders_bind_before_history_or_volatile_retirement() {
    let mut exact = RecordingDecidedLaneCommitter::new(None);
    assert_eq!(
        commit_decided_lane_recovery_drain(
            DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                DecidedLaneRecoveryCurrentDrain::Admitted(7),
            ),
            &mut exact,
        )
        .expect("commit exact decided Serve"),
        DecidedLaneRecoveryDrainCommitOutcome::CurrentServe
    );
    assert_eq!(exact.calls, ["serve-exact-decided"]);

    let mut negative = RecordingDecidedLaneCommitter::new(None);
    assert_eq!(
        commit_decided_lane_recovery_drain(
            DecidedLaneRecoveryDrainAuthorization::CurrentServe(
                DecidedLaneRecoveryCurrentDrain::Rejected("durable negative staged".to_owned(),),
            ),
            &mut negative,
        )
        .expect("retire staged negative"),
        DecidedLaneRecoveryDrainCommitOutcome::CurrentServe
    );
    assert_eq!(negative.calls, ["retire-staged-negative"]);

    let mut kura_replica_advert = RecordingDecidedLaneCommitter::new(None);
    assert_eq!(
        commit_decided_lane_recovery_drain(
            DecidedLaneRecoveryDrainAuthorization::KuraReplicaAdvert,
            &mut kura_replica_advert,
        )
        .expect("route Kura replica advert"),
        DecidedLaneRecoveryDrainCommitOutcome::KuraReplicaAdvert
    );
    assert_eq!(kura_replica_advert.calls, ["kura-replica-advert"]);

    let mut historical = RecordingDecidedLaneCommitter::new(None);
    assert_eq!(
        commit_decided_lane_recovery_drain(
            DecidedLaneRecoveryDrainAuthorization::HistoricalServe,
            &mut historical,
        )
        .expect("route historical Serve"),
        DecidedLaneRecoveryDrainCommitOutcome::HistoricalServe
    );
    assert_eq!(historical.calls, ["bind-leader-wire", "serve-history"]);

    let mut volatile = RecordingDecidedLaneCommitter::new(None);
    assert_eq!(
        commit_decided_lane_recovery_drain(
            DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire,
            &mut volatile,
        )
        .expect("retire non-Serve terminal traffic"),
        DecidedLaneRecoveryDrainCommitOutcome::LeaderWireVolatile
    );
    assert_eq!(volatile.calls, ["bind-leader-wire", "retire-volatile"]);

    let mut failed_bind = RecordingDecidedLaneCommitter::new(Some("bind-leader-wire"));
    assert!(
        commit_decided_lane_recovery_drain(
            DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire,
            &mut failed_bind,
        )
        .is_err()
    );
    assert_eq!(
        failed_bind.calls,
        ["bind-leader-wire"],
        "a failed bind cannot publish VolatileTerminal"
    );
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
fn drain_decided_lane_recovery_ingress_prepares_exact_and_typed_negative_branches() {
    let (context, keys) = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 3,
    };
    let subject = proposal_subject(b"decided recovery exact subject");
    let request = decided_recovery_certified_request(&context, &keys[1], round, subject);
    let inbound = admitted_decided_recovery_request(&context, &request);
    let authenticate = |request, sender: &PeerId| {
        super::super::v2_transport::authenticate_certified_body_request(
            &context,
            request,
            sender,
            |_, _| Ok::<(), String>(()),
        )
        .map_err(|error| error.to_string())
    };
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(
            &inbound,
            context.height,
            subject,
            authenticate,
        ),
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Authenticated { request: authenticated, .. }
        ) if authenticated.request_hash() == HashOf::new(&request)
    ));

    assert!(matches!(
        prepare_decided_lane_recovery_ingress(
            &inbound,
            context.height,
            subject,
            |_, _| Err("invalid aggregate signature".to_owned()),
        ),
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Negative {
                request_hash,
                outcome: CertifiedServeNegativeOutcome::InvalidCertificate,
                ..
            }
        ) if request_hash == HashOf::new(&request)
    ));

    let decided_subject = proposal_subject(b"decided recovery winning subject");
    assert_ne!(decided_subject, subject);
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(
            &inbound,
            context.height,
            decided_subject,
            |request, sender| {
                super::super::v2_transport::authenticate_certified_body_request(
                    &context,
                    request,
                    sender,
                    |_, _| Ok::<(), String>(()),
                )
                .map_err(|error| error.to_string())
            },
        ),
        DecidedLaneRecoveryIngressPreparation::CurrentServe(
            DecidedLaneRecoveryCurrentServe::Negative {
                request_hash,
                outcome:
                    CertifiedServeNegativeOutcome::SupersededByDurableDecision(decided),
                ..
            }
        ) if request_hash == HashOf::new(&request) && decided == decided_subject
    ));
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
        prepare_decided_lane_recovery_ingress(
            &historical_inbound,
            context.height,
            subject,
            |_, _| panic!("historical classification precedes active Serve authentication"),
        ),
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
    let ordinary_non_serve =
        InboundBlockMessage::new(BlockMessage::V2(non_serve.clone()), Some(peer.clone()));
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(
            &ordinary_non_serve,
            context.height,
            subject,
            |_, _| panic!("non-Serve traffic never reaches Serve authentication"),
        ),
        DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
    ));

    let mut wrong_version = non_serve;
    wrong_version.protocol_version = wrong_version.protocol_version.saturating_sub(1);
    let wrong_version =
        InboundBlockMessage::new(BlockMessage::V2(wrong_version), Some(peer.clone()));
    assert!(matches!(
        prepare_decided_lane_recovery_ingress(
            &wrong_version,
            context.height,
            subject,
            |_, _| panic!("non-Serve traffic never reaches Serve authentication"),
        ),
        DecidedLaneRecoveryIngressPreparation::LeaderWireRetire
    ));
}

fn height_ingress_bindings_fixture(
    owner_marker: u8,
) -> (
    TempDir,
    Arc<FairV2Ingress>,
    Arc<AtomicBool>,
    HeightIngressBindings,
    CertifiedServeIngressGate,
    Arc<LeaderWireLifecycleStoreGate>,
) {
    let (context, _) = context();
    let roster = context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<BTreeSet<_>>();
    let directory = TempDir::new().expect("temporary joint height-ingress directory");
    let ingress = Arc::new(
        FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
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
        ),
    );
    ingress
        .configure_roster_for_context(roster.iter().cloned(), &context.chain_id, context.da_layout)
        .expect("configure joint height ingress");
    ingress.require_certified_serve_gate();
    ingress.require_leader_wire_lifecycle_gate();

    let ingress_ready = Arc::new(AtomicBool::new(false));
    let (serve_gate, lifecycle_ordinals) =
        super::super::v2_worker::tests::certified_serve_ingress_gate_fixture();
    let certified_serve = CertifiedServeIngressBinding::bind(
        Arc::clone(&ingress_ready),
        Arc::clone(&ingress),
        serve_gate.clone(),
    )
    .expect("bind joint height Serve gate");

    let capacity = LeaderWireLifecycleStoreGate::derived_capacity(
        roster.len(),
        context.da_layout.max_chunk_count,
    )
    .expect("derive joint height leader-wire capacity");
    let owner = [owner_marker; 32];
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            context.id(),
            context.height,
            owner,
            0,
            false,
        );
    let (leader_gate, restore) = LeaderWireLifecycleStoreGate::open(
        &directory.path().join("joint-height-ingress.wal"),
        context.id(),
        context.height,
        owner,
        roster,
        capacity,
        context.da_layout.max_chunk_count,
        recovery_authority,
        &[],
        &[],
    )
    .expect("open joint height leader-wire gate");
    let leader_wire = LeaderWireIngressBinding::bind(
        Arc::clone(&ingress_ready),
        Arc::clone(&ingress),
        Arc::clone(&leader_gate),
        restore,
        lifecycle_ordinals,
        context.id(),
        context.height,
    )
    .expect("bind joint height leader-wire gate");
    let bindings = HeightIngressBindings::new(certified_serve, leader_wire);
    ingress.open().expect("open joint height ingress");
    ingress_ready.store(true, Ordering::Release);
    (
        directory,
        ingress,
        ingress_ready,
        bindings,
        serve_gate,
        leader_gate,
    )
}

#[test]
fn height_ingress_bindings_retire_both_gates_in_one_closed_cut() {
    let (_directory, ingress, ingress_ready, mut bindings, serve_gate, leader_gate) =
        height_ingress_bindings_fixture(0xD5);
    {
        let state = ingress.state.lock();
        assert!(state.open);
        assert!(
            state
                .certified_serve_gate
                .as_ref()
                .is_some_and(|bound| bound.ptr_eq(&serve_gate))
        );
        assert!(
            state
                .leader_wire_lifecycle_gate
                .as_ref()
                .is_some_and(|bound| LeaderWireLifecycleStoreGate::ptr_eq(bound, &leader_gate))
        );
    }

    bindings
        .retire()
        .expect("retire both per-height ingress gates atomically");
    assert!(!ingress_ready.load(Ordering::Acquire));
    assert!(bindings.certified_serve.gate.is_none());
    assert!(bindings.leader_wire.gate.is_none());
    {
        let state = ingress.state.lock();
        assert!(!state.open);
        assert!(state.certified_serve_gate.is_none());
        assert!(state.leader_wire_lifecycle_gate.is_none());
        assert!(state.leader_wire_lifecycle_ordinals.is_none());
        assert!(state.leader_wire_context.is_none());
    }
    bindings
        .retire()
        .expect("joint height retirement remains idempotent");
}

#[test]
fn height_ingress_bindings_drop_fails_closed_on_mismatched_or_partial_ownership() {
    {
        let (_directory, ingress, ingress_ready, mut bindings, serve_gate, leader_gate) =
            height_ingress_bindings_fixture(0xD6);
        bindings.leader_wire.ingress_ready = Arc::new(AtomicBool::new(true));
        let error = bindings
            .retire()
            .expect_err("mismatched readiness ownership must reject joint retirement");
        assert!(matches!(
            error,
            V2RunnerError::Service(ref reason)
                if reason == "per-height ingress gates changed their shared queue"
        ));
        assert!(ingress_ready.load(Ordering::Acquire));
        assert!(ingress.state.lock().open);

        drop(bindings);
        assert!(!ingress_ready.load(Ordering::Acquire));
        let state = ingress.state.lock();
        assert!(!state.open);
        assert!(
            state
                .certified_serve_gate
                .as_ref()
                .is_some_and(|bound| bound.ptr_eq(&serve_gate)),
            "a failed joint validation cannot partially detach the Serve gate"
        );
        assert!(
            state
                .leader_wire_lifecycle_gate
                .as_ref()
                .is_some_and(|bound| LeaderWireLifecycleStoreGate::ptr_eq(bound, &leader_gate)),
            "a failed joint validation cannot partially detach the leader-wire gate"
        );
    }

    {
        let (_directory, ingress, ingress_ready, mut bindings, serve_gate, leader_gate) =
            height_ingress_bindings_fixture(0xD7);
        bindings.leader_wire.gate = None;
        let error = bindings
            .retire()
            .expect_err("partial child ownership must reject joint retirement");
        assert!(matches!(
            error,
            V2RunnerError::Service(ref reason)
                if reason == "per-height ingress gates changed joint ownership"
        ));
        assert!(ingress_ready.load(Ordering::Acquire));
        assert!(ingress.state.lock().open);

        drop(bindings);
        assert!(!ingress_ready.load(Ordering::Acquire));
        let state = ingress.state.lock();
        assert!(!state.open);
        assert!(
            state
                .certified_serve_gate
                .as_ref()
                .is_some_and(|bound| bound.ptr_eq(&serve_gate)),
            "partial child ownership cannot trigger split Serve teardown"
        );
        assert!(
            state
                .leader_wire_lifecycle_gate
                .as_ref()
                .is_some_and(|bound| LeaderWireLifecycleStoreGate::ptr_eq(bound, &leader_gate)),
            "partial child ownership cannot trigger split leader-wire teardown"
        );
    }
}

fn leader_wire_runtime_ingress_fixture() -> (
    TempDir,
    FairV2Ingress,
    Arc<LeaderWireLifecycleStoreGate>,
    FairV2IngressOwnershipEvidence,
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
        .configure_roster_for_context(roster.clone(), &context.chain_id, context.da_layout)
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
        ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message.clone()),
            Some(semantic_origin.clone()),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Enqueued)
    ));
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
        ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message),
            Some(semantic_origin),
        )),
        Ok(super::super::FairV2IngressPushDisposition::Coalesced)
    ));
    assert_eq!(
        gate.earliest_ingress_scheduler_ordinal()
            .expect("retry retains the terminal tombstone"),
        None
    );
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
