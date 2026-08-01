// Status fixtures and validation tests included in the parent consensus-v2 test module.

fn status(context: &HeightContext) -> SumeragiV2Status {
    SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"status-node"),
        build_fingerprint: Hash::new(b"status-build"),
        config_fingerprint: Hash::new(b"status-config"),
        restart_required: false,
        height_context_id: context.id(),
        height: context.height,
        view: 3,
        phase: SumeragiV2StatusPhase::AwaitingProposal,
        leader: 0,
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Missing,
        pending_persistence_id: None,
        last_committed_height: 0,
        last_committed_subject: None,
        height_context: SumeragiV2HeightContextStatus {
            epoch: context.epoch,
            epoch_end_height: context.epoch_end_height,
            mode: context.mode,
            epoch_seed: context.leader_seed,
            validator_count: u32::try_from(context.roster.len())
                .expect("test roster fits status count"),
            quorum: context.quorum,
        },
        last_commit_qc: None,
        liveness: SumeragiV2LivenessStatus::default(),
    }
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the complete status rejection matrix documents the ordered scalar, frontier, and phase invariants in one canonical protocol vector"
)]
fn status_validation_rejects_impossible_scalar_and_phase_states() {
    use SumeragiV2StatusValidationError as Error;

    let context = context(&[1, 1, 1, 1]);
    let baseline = status(&context);
    assert_eq!(baseline.validate(), Ok(()));

    let mut wrong_protocol = baseline.clone();
    wrong_protocol.protocol_version += 1;
    assert!(matches!(
        wrong_protocol.validate(),
        Err(Error::UnsupportedProtocolVersion { .. })
    ));

    let mut wrong_body = baseline.clone();
    wrong_body.body_state = SumeragiV2BodyState::Validated;
    assert_eq!(wrong_body.validate(), Err(Error::PhaseBodyMismatch));

    let mut commit_without_lock = baseline.clone();
    commit_without_lock.phase = SumeragiV2StatusPhase::Commit;
    commit_without_lock.body_state = SumeragiV2BodyState::Validated;
    assert_eq!(
        commit_without_lock.validate(),
        Err(Error::CommitWithoutLock)
    );

    let mut zero_persistence = baseline.clone();
    zero_persistence.pending_persistence_id = Some(0);
    assert_eq!(zero_persistence.validate(), Err(Error::ZeroPersistenceId));

    let mut committed_ahead = baseline.clone();
    committed_ahead.last_committed_height = committed_ahead.height;
    assert_eq!(
        committed_ahead.validate(),
        Err(Error::CommittedHeightNotBehindActiveHeight)
    );

    let mut pending_apply = baseline;
    pending_apply.phase = SumeragiV2StatusPhase::PendingApply;
    pending_apply.body_state = SumeragiV2BodyState::PendingApply;
    assert_eq!(
        pending_apply.validate(),
        Err(Error::PendingApplyCommitMismatch)
    );
    pending_apply.last_committed_height = pending_apply.height;
    let committed = qc(
        &context,
        pending_apply.view,
        GlobalPhase::Commit,
        vec![0, 1, 2],
    );
    pending_apply.last_committed_subject = Some(committed.subject);
    pending_apply.last_commit_qc = Some(SumeragiV2CommitQcStatus {
        certificate: committed.as_ref(),
        validator_count: 4,
        signer_count: 3,
        min_signers: 3,
        signed_power: 3,
        total_power: 4,
    });
    assert_eq!(pending_apply.validate(), Ok(()));

    let mut invalid_commit_origin = pending_apply.clone();
    let invalid_certificate = &mut invalid_commit_origin
        .last_commit_qc
        .as_mut()
        .expect("commit summary")
        .certificate;
    invalid_certificate.proposal_round.view = invalid_certificate.round.view + 1;
    assert_eq!(
        invalid_commit_origin.validate(),
        Err(Error::CommitSummaryCertificateMismatch)
    );

    let mut invalid_context = status(&context);
    invalid_context.height_context.epoch_end_height = invalid_context.height - 1;
    assert_eq!(
        invalid_context.validate(),
        Err(Error::EpochEndsBeforeHeight)
    );

    let mut invalid_leader = status(&context);
    invalid_leader.leader = invalid_leader.height_context.validator_count;
    assert_eq!(invalid_leader.validate(), Err(Error::LeaderOutOfRange));

    let mut invalid_quorum = status(&context);
    invalid_quorum.height_context.quorum.min_signers -= 1;
    assert_eq!(
        invalid_quorum.validate(),
        Err(Error::InvalidHeightContextQuorum)
    );

    let mut invalid_commit_summary = pending_apply.clone();
    invalid_commit_summary
        .last_commit_qc
        .as_mut()
        .expect("commit summary")
        .signed_power = 2;
    assert_eq!(
        invalid_commit_summary.validate(),
        Err(Error::InvalidCommitSummaryQuorum)
    );

    let mut impossible_signer_power = pending_apply;
    let impossible_summary = impossible_signer_power
        .last_commit_qc
        .as_mut()
        .expect("commit summary");
    impossible_summary.signer_count = 4;
    impossible_summary.signed_power = 3;
    assert_eq!(
        impossible_signer_power.validate(),
        Err(Error::InvalidCommitSummaryQuorum),
        "each authenticated signer must contribute at least one unit of voting power"
    );

    let mut one_sided_commit = status(&context);
    one_sided_commit.height = 2;
    one_sided_commit.last_committed_height = 1;
    one_sided_commit.last_committed_subject = Some(subject(91));
    assert_eq!(
        one_sided_commit.validate(),
        Err(Error::CommitFrontierAuthenticationMismatch)
    );
}

#[test]
#[expect(
    clippy::too_many_lines,
    reason = "the complete liveness status matrix keeps round, quorum, timeout, and queue-ownership invariants together as one canonical protocol vector"
)]
fn status_validation_checks_liveness_rounds_quorums_and_queue_ownership() {
    use SumeragiV2StatusValidationError as Error;

    let context = context(&[1, 1, 1, 1]);
    let mut baseline = status(&context);
    let active_round = round(&context, 2);
    baseline.liveness = SumeragiV2LivenessStatus {
        generation: 4,
        prepare_quorums: vec![SumeragiV2VoteQuorumStatus {
            round: active_round,
            proposal_round: active_round,
            subject: subject(41),
            execution_commitment: execution_commitment(42),
            signer_count: 2,
            signed_power: 2,
            min_signers: 3,
            total_power: 4,
        }],
        outbound_intents: vec![SumeragiV2OutboundIntentStatus {
            kind: SumeragiV2OutboundIntentKind::Proposal,
            round: active_round,
            proposal_round: Some(active_round),
            subject: Some(subject(41)),
            execution_commitment: None,
            stage: SumeragiV2OutboundIntentStage::Sent,
        }],
        queues: vec![SumeragiV2QueueStatus {
            queue: SumeragiV2QueueKind::RuntimeProgress,
            depth: 1,
            capacity: 4,
            oldest_age_ms: Some(7),
            service_debt: 2,
        }],
        last_progress: Some(SumeragiV2ProgressTransitionStatus {
            generation: 4,
            round: active_round,
            transition: SumeragiV2ProgressTransition::PrepareVoteAdmitted,
            age_ms: 7,
        }),
        ..SumeragiV2LivenessStatus::default()
    };
    assert_eq!(baseline.validate(), Ok(()));

    let mut future_round = baseline.clone();
    future_round.liveness.prepare_quorums[0].round.view = future_round.view + 1;
    assert_eq!(
        future_round.validate(),
        Err(Error::LivenessRoundFromFutureView)
    );

    let mut cross_context_round = baseline.clone();
    cross_context_round.liveness.prepare_quorums[0]
        .round
        .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign liveness quorum context",
    )));
    assert_eq!(
        cross_context_round.validate(),
        Err(Error::LivenessRoundMismatch),
        "a liveness round must bind to the status height-context identity"
    );

    let mut cross_height_round = baseline.clone();
    cross_height_round.liveness.prepare_quorums[0].round.height =
        cross_height_round.liveness.prepare_quorums[0]
            .round
            .height
            .saturating_add(1);
    assert_eq!(
        cross_height_round.validate(),
        Err(Error::LivenessRoundMismatch),
        "a liveness round must bind independently to the status height"
    );

    let mut wrong_origin = baseline.clone();
    wrong_origin.liveness.prepare_quorums[0].proposal_round.view -= 1;
    assert_eq!(wrong_origin.validate(), Err(Error::InvalidProposalRound));

    let mut wrong_quorum = baseline.clone();
    wrong_quorum.liveness.prepare_quorums[0].total_power = 5;
    assert_eq!(wrong_quorum.validate(), Err(Error::InvalidLivenessQuorum));

    let mut invalid_queue = baseline.clone();
    invalid_queue.liveness.queues[0].depth = 0;
    assert_eq!(invalid_queue.validate(), Err(Error::InvalidLivenessQueue));

    let mut every_queue_kind = baseline.clone();
    every_queue_kind.liveness.queues = [
        SumeragiV2QueueKind::Ingress,
        SumeragiV2QueueKind::DeferredNormal,
        SumeragiV2QueueKind::DeferredProgress,
        SumeragiV2QueueKind::DeferredCompletion,
        SumeragiV2QueueKind::RuntimeNormal,
        SumeragiV2QueueKind::RuntimeProgress,
        SumeragiV2QueueKind::RuntimeCompletion,
        SumeragiV2QueueKind::EffectCompletion,
        SumeragiV2QueueKind::NetworkIngress,
        SumeragiV2QueueKind::EffectDispatch,
    ]
    .into_iter()
    .map(|queue| SumeragiV2QueueStatus {
        queue,
        depth: 0,
        capacity: 1,
        oldest_age_ms: None,
        service_debt: 0,
    })
    .collect();
    assert_eq!(every_queue_kind.validate(), Ok(()));

    let mut too_many_queues = every_queue_kind;
    too_many_queues.liveness.queues.push(SumeragiV2QueueStatus {
        queue: SumeragiV2QueueKind::NetworkIngress,
        depth: 0,
        capacity: 1,
        oldest_age_ms: None,
        service_debt: 0,
    });
    assert_eq!(
        too_many_queues.validate(),
        Err(Error::LivenessCollectionTooLarge)
    );

    let mut invalid_intent = baseline.clone();
    invalid_intent.liveness.outbound_intents[0].execution_commitment =
        Some(execution_commitment(42));
    assert_eq!(
        invalid_intent.validate(),
        Err(Error::InvalidOutboundIntentShape)
    );

    let mut missing_intent_origin = baseline.clone();
    missing_intent_origin.liveness.outbound_intents[0].proposal_round = None;
    assert_eq!(
        missing_intent_origin.validate(),
        Err(Error::InvalidOutboundIntentShape)
    );

    let mut mismatched_prepare_origin = baseline.clone();
    mismatched_prepare_origin.liveness.outbound_intents[0]
        .proposal_round
        .as_mut()
        .expect("proposal origin")
        .view -= 1;
    assert_eq!(
        mismatched_prepare_origin.validate(),
        Err(Error::InvalidProposalRound)
    );

    let mut cross_context_intent_origin = baseline.clone();
    cross_context_intent_origin.liveness.outbound_intents[0]
        .proposal_round
        .as_mut()
        .expect("proposal origin")
        .context_id = HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
        b"foreign outbound-intent origin",
    )));
    assert_eq!(
        cross_context_intent_origin.validate(),
        Err(Error::LivenessRoundMismatch)
    );

    let mut timeout_with_origin = baseline.clone();
    let intent = &mut timeout_with_origin.liveness.outbound_intents[0];
    intent.kind = SumeragiV2OutboundIntentKind::TimeoutVote;
    intent.subject = None;
    assert_eq!(
        timeout_with_origin.validate(),
        Err(Error::InvalidOutboundIntentShape)
    );

    let mut same_round_commit = baseline.clone();
    let intent = &mut same_round_commit.liveness.outbound_intents[0];
    intent.kind = SumeragiV2OutboundIntentKind::CommitVote;
    intent.execution_commitment = Some(execution_commitment(42));
    assert_eq!(same_round_commit.validate(), Ok(()));

    let mut stale_commit_round = same_round_commit.clone();
    let intent = &mut stale_commit_round.liveness.outbound_intents[0];
    intent.proposal_round.as_mut().expect("proposal round").view -= 1;
    assert_eq!(
        stale_commit_round.validate(),
        Err(Error::InvalidProposalRound)
    );

    let mut future_commit_origin = same_round_commit;
    future_commit_origin.liveness.outbound_intents[0]
        .proposal_round
        .as_mut()
        .expect("proposal origin")
        .view = active_round.view + 1;
    assert_eq!(
        future_commit_origin.validate(),
        Err(Error::InvalidProposalRound)
    );

    let mut future_generation = baseline;
    future_generation
        .liveness
        .last_progress
        .as_mut()
        .expect("progress record")
        .generation += 1;
    assert_eq!(
        future_generation.validate(),
        Err(Error::LivenessGenerationFromFuture)
    );
}
