#[test]
fn certified_response_priority_probe_reads_exact_or_conflicting_family_claim() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: certified_sources(&fixture, &prepare),
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("hybrid fetch");
    let task = services.fetch_tasks[0].clone();
    let claimed = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let claimed_responder = fixture.context.roster[0].validator.clone();
    let authenticated = executor
        .outstanding_requests
        .authenticate_response(&fixture.context, claimed.clone(), &claimed_responder)
        .expect("authenticate setup claim");
    assert_eq!(
        executor
            .outstanding_requests
            .prepare_authenticated_response_claim(&authenticated)
            .expect("prepare setup claim")
            .commit(),
        super::super::v2_transport::CertifiedBodyResponseClaimDisposition::Acquired
    );
    let ownership_before = executor.body_ownership_projection();
    let claims_before = executor.outstanding_requests.response_claim_count();
    let exact = executor
        .probe_certified_response_priority(&claimed, &claimed_responder)
        .expect("exact retransmission remains a preflight candidate");
    let CertifiedResponsePriorityProbe::PreflightRequired(exact) = exact else {
        panic!("the exact claimed response must remain retryable")
    };
    assert_eq!(
        exact.claim_preflight(),
        &CertifiedBodyResponseClaimPreflight::ExactRetransmission
    );
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before
    );
    assert_eq!(
        executor
            .outstanding_requests
            .preflight_authenticated_response_claim(&authenticated),
        Ok(CertifiedBodyResponseClaimPreflight::ExactRetransmission)
    );
    let competing = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        1,
    );
    let competing_responder = fixture.context.roster[1].validator.clone();
    assert!(matches!(
        executor
            .probe_certified_response_priority(&competing, &competing_responder)
            .expect("a family conflict is a typed non-priority result"),
        CertifiedResponsePriorityProbe::DefinitelyNonPriority(
            CertifiedResponsePriorityNonPriority::ConflictingFamilyClaim {
                request_hash,
                claimed_response_hash,
                incoming_response_hash,
            }
        ) if request_hash == claimed.request_hash
            && claimed_response_hash == HashOf::new(&claimed)
            && incoming_response_hash == HashOf::new(&competing)
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before
    );
    assert_eq!(
        executor
            .outstanding_requests
            .preflight_authenticated_response_claim(&authenticated),
        Ok(CertifiedBodyResponseClaimPreflight::ExactRetransmission)
    );
    assert!(executor.runtime.completions.is_empty());
    assert!(!executor.status().fail_closed);
    assert!(
        executor
            .certified_work
            .remove(&claimed.request_hash)
            .is_some()
    );
    assert!(executor.outstanding_requests.cancel(claimed.request_hash));
    assert!(matches!(
        executor.validated_certified_request_presence(),
        Err(EffectTransportError::Authentication(
            V2TransportError::InconsistentRequestIndex(request_hash)
        )) if request_hash == claimed.request_hash
    ));
    assert!(matches!(
        executor.probe_certified_response_priority(&claimed, &claimed_responder),
        Err(EffectTransportError::Authentication(
            V2TransportError::InconsistentRequestIndex(request_hash)
        )) if request_hash == claimed.request_hash
    ));
}
#[test]
fn different_subject_decision_supersedes_protected_lock_and_frees_losing_capacity() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 2));
    let mut services = fixture.services();
    let (losing_subject, losing_body) = distinct_body(&fixture);
    let losing_manifest = canonical_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        losing_subject,
        &losing_body,
    );
    let losing_lock = (losing_manifest.round, losing_manifest.subject);
    let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    losing_prepare.subject = losing_manifest.subject;
    let losing_certified_sources = certified_sources(&fixture, &losing_prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: losing_manifest.round,
                subject: losing_manifest.subject,
                manifest: Some(losing_manifest),
                certified_sources: losing_certified_sources,
                certificate: Some(losing_prepare),
            }],
            &mut services,
        )
        .expect("fill the only pending-work slot with a losing fetch");
    let losing_id = services.fetch_tasks[0].id();
    executor.protected_lock = Some(losing_lock);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let commit_certified_sources = certified_sources(&fixture, &commit);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: commit.round,
                subject: commit.subject,
                manifest: None,
                certified_sources: commit_certified_sources,
                certificate: Some(commit.clone()),
            }],
            &mut services,
        )
        .expect("Decision cleanup frees capacity before decided-body recovery");
    assert_eq!(
        executor.protected_decision,
        Some((
            commit.round,
            commit.proposal_round,
            commit.subject,
            commit.execution_commitment,
        ))
    );
    assert_eq!(
        executor.protected_lock,
        Some((commit.proposal_round, commit.subject))
    );
    assert_eq!(executor.pending_fetches.len(), 1);
    assert!(executor.pending_fetches.values().all(|pending| {
        pending.task.round == commit.round && pending.task.subject == commit.subject
    }));
    assert_eq!(services.cancelled_fetches, vec![losing_id]);
    assert_eq!(services.retired_all_outbound, 1);
    assert_eq!(services.retired_candidate_work, 1);
    assert_eq!(services.durable_runtime_decision, Some(commit.subject));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}
#[test]
fn decision_installed_by_same_runtime_step_retires_stale_terminal_effects() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decision_on_next_step = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![
            AdapterEffect::Broadcast(proposal(&fixture)),
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
        ])));
    services.fail_on = Some("broadcast");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("durable Decision retires stale in-flight effects"),
        EffectExecutorStep::Advanced { effects: 0 }
    );
    assert_eq!(services.fail_on, Some("broadcast"));
    assert!(services.broadcasts.is_empty());
    assert!(services.sign_tasks.is_empty());
    assert_eq!(services.retired_all_outbound, 1);
    assert_eq!(services.retired_candidate_work, 1);
    assert_eq!(services.durable_runtime_decision, Some(commit.subject));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}
#[test]
fn decision_installed_by_same_runtime_step_keeps_exact_commit_and_body_work() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let exact_commit_message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(commit.clone()),
    );
    let (losing_subject, _) = distinct_body(&fixture);
    let mut losing_commit = commit.clone();
    losing_commit.subject = losing_subject;
    let losing_commit_message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(losing_commit),
    );
    let certified_sources = certified_sources(&fixture, &commit);
    executor.runtime.decision_on_next_step = Some(decision);
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![
            AdapterEffect::Broadcast(proposal(&fixture)),
            AdapterEffect::Broadcast(losing_commit_message),
            AdapterEffect::Broadcast(exact_commit_message.clone()),
            AdapterEffect::FetchBody {
                tag: tag(0),
                round: commit.round,
                subject: losing_subject,
                manifest: None,
                certified_sources: Vec::new(),
                certificate: None,
            },
            AdapterEffect::FetchBody {
                tag: tag(0),
                round: commit.round,
                subject: commit.subject,
                manifest: None,
                certified_sources,
                certificate: Some(commit.clone()),
            },
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
        ])));
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("dispatch only exact post-Decision effects"),
        EffectExecutorStep::Advanced { effects: 2 }
    );
    assert!(services.broadcasts.is_empty());
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    assert!(services.sign_tasks.is_empty());
    assert_eq!(services.fetch_tasks.len(), 1);
    assert_eq!(services.fetch_tasks[0].round, commit.round);
    assert_eq!(services.fetch_tasks[0].subject, commit.subject);
    assert_eq!(services.retired_all_outbound, 1);
    assert_eq!(services.retired_candidate_work, 1);
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}
#[test]
fn decision_commit_broadcast_yields_exact_apply_until_lifecycle_output_settles() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let exact_commit_message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(commit.clone()),
    );
    let apply = AdapterEffect::Apply {
        tag: tag(0),
        subject: fixture.manifest.subject,
        certificate: commit,
    };
    executor.runtime.decision_on_next_step = Some(decision);
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![
            AdapterEffect::Broadcast(exact_commit_message),
            apply.clone(),
        ])));

    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("park the CommitQC output before terminal Decision cleanup"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(executor.protected_decision, Some(decision));
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    assert!(services.apply_tasks.is_empty());
    let retained = executor
        .retained_effect_batch
        .as_ref()
        .expect("retain the exact Apply suffix behind lifecycle settlement");
    assert_eq!(retained.effects.len(), 1);
    let retained_apply = retained.effects.front().expect("one retained Apply owner");
    assert_eq!(retained_apply.effect, apply);
    assert!(
        retained_apply
            .ownership
            .exactly_binds_adapter_effect(&retained_apply.effect)
    );
    assert!(services.broadcasts.is_empty());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}
#[test]
fn newly_installed_decision_holds_apply_until_runner_cleanup_acknowledges_exact_subject() {
    let mut fixture = Fixture::new();
    fixture.manifest = canonical_payload_manifest(
        &fixture.context,
        round(&fixture.context, 3),
        fixture.manifest.subject,
        &fixture.body,
    );
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let apply = AdapterEffect::Apply {
        tag: tag(0),
        subject: commit.subject,
        certificate: commit,
    };
    executor.runtime.decision_on_next_step = Some(decision);
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![apply.clone()])));

    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("retain Apply behind the runner Decision-cleanup fence"),
        EffectExecutorStep::Advanced { effects: 0 }
    );
    assert_eq!(executor.protected_decision, Some(decision));
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .map(|pending| pending.decision),
        Some(decision)
    );
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .expect("new Decision cleanup owner")
            .owner_tag,
        tag(0)
    );
    assert!(services.apply_tasks.is_empty());
    assert_ne!(tag(0).view(), decision.0.view);
    assert_eq!(
        executor
            .retained_effect_batch
            .as_ref()
            .and_then(|batch| batch.effects.front())
            .map(|owned| &owned.effect),
        Some(&apply)
    );
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("an unacknowledged runner cleanup remains inert"),
        EffectExecutorStep::Idle
    );
    assert!(services.apply_tasks.is_empty());

    let owner_tag = tag(0);
    let wrong_generation = EventTag::new(
        owner_tag.height(),
        owner_tag.view(),
        Generation::new(owner_tag.generation().get().saturating_add(1)),
    );
    assert!(matches!(
        executor.acknowledge_runner_decision_cleanup(wrong_generation, Some(decision.2),),
        Err(EffectExecutorError::Contract(_))
    ));
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .map(|pending| pending.decision),
        Some(decision)
    );

    executor
        .acknowledge_runner_decision_cleanup(owner_tag, Some(decision.2))
        .expect("acknowledge current-owner cleanup for a future-view CommitQC");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("dispatch Apply only after exact runner cleanup"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(services.apply_tasks[0].subject(), decision.2);
    assert!(executor.pending_runner_decision_cleanup.is_none());
}
#[test]
fn live_decision_installed_before_apply_step_still_waits_for_runner_cleanup() {
    let mut fixture = Fixture::new();
    fixture.manifest = canonical_payload_manifest(
        &fixture.context,
        round(&fixture.context, 3),
        fixture.manifest.subject,
        &fixture.body,
    );
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let apply = AdapterEffect::Apply {
        tag: tag(0),
        subject: commit.subject,
        certificate: commit,
    };
    executor.runtime.decided_body = Some(decision);
    assert!(
        executor
            .plan_runner_decision_cleanup(Some(decision), Some(decision))
            .expect("inspect the cold recovered Decision")
            .is_none(),
        "an unarmed cold recovery has no process-local proposal owner to clean up"
    );
    executor.runtime.live_clocks_armed = true;
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![apply.clone()])));

    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("retain Apply for a Decision installed before this live step"),
        EffectExecutorStep::Advanced { effects: 0 }
    );
    assert_eq!(executor.protected_decision, Some(decision));
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .map(|pending| pending.decision),
        Some(decision)
    );
    assert!(services.apply_tasks.is_empty());
    assert_eq!(
        executor
            .retained_effect_batch
            .as_ref()
            .and_then(|batch| batch.effects.front())
            .map(|owned| &owned.effect),
        Some(&apply)
    );

    executor
        .acknowledge_runner_decision_cleanup(tag(0), Some(decision.2))
        .expect("acknowledge the exact live Decision after runner owner cleanup");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("dispatch retained Apply after runner cleanup"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(services.apply_tasks[0].subject(), decision.2);
    assert!(executor.pending_runner_decision_cleanup.is_none());
}

#[test]
fn lifecycle_apply_dispatch_waits_for_exact_runner_decision_cleanup() {
    let mut fixture = Fixture::new();
    fixture.manifest = canonical_payload_manifest(
        &fixture.context,
        round(&fixture.context, 3),
        fixture.manifest.subject,
        &fixture.body,
    );
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    executor.runtime.decided_body = Some(decision);
    executor.runtime.live_clocks_armed = true;
    executor.runtime.steps.push_back(Ok(RuntimeStep::Idle));

    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("observe the live Decision before any Apply effect exists"),
        EffectExecutorStep::Idle
    );
    assert_eq!(
        executor
            .reconcile_runtime_decision(&mut services)
            .expect("protect the exact runtime Decision before runner cleanup"),
        Some(decision)
    );
    assert_eq!(executor.protected_decision, Some(decision));
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .map(|pending| pending.decision),
        Some(decision)
    );
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.parked_effect_batch.is_none());
    assert_eq!(executor.pending_work(), 0);
    assert!(
        !executor.lifecycle_decision_apply_executor_owners_are_empty(),
        "the runner's process-local Decision handoff must be an independent Apply fence"
    );

    executor
        .acknowledge_runner_decision_cleanup(tag(0), Some(decision.2))
        .expect("retire the exact current runner Decision owner");
    assert!(
        executor.lifecycle_decision_apply_executor_owners_are_empty(),
        "retiring only the exact runner handoff must reopen the otherwise-empty Apply cut"
    );
}

#[cfg(feature = "bls")]
#[test]
fn apply_barrier_handoff_retires_exact_live_proposal_and_lane_losers() {
    let mut fixture = ProductionTransportFixture::new();
    let started = Instant::now();
    fixture
        .executor
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            started,
        )
        .expect("arm the production serialized runtime");
    assert!(!fixture.executor.has_pending_runner_decision_cleanup_for_test());

    let pre_decision_directive = fixture
        .executor
        .local_proposal_directive()
        .expect("read the live pre-Decision proposal owner");
    assert!(pre_decision_directive.decided_subject().is_none());
    let mut local_proposal =
        super::super::v2_runner::ProductionLifecycleLocalProposalStateV1::with_attempted_for_test(
            pre_decision_directive,
        );
    assert!(local_proposal.already_attempted(pre_decision_directive));
    let mut lane_work =
        super::super::v2_lane_work::tests::runner_handoff_losing_merge_fixture_for_test();
    assert_eq!(
        super::super::v2_lane_work::tests::runner_handoff_losing_merge_counts_for_test(
            &lane_work,
        ),
        (1, 1)
    );

    let commit =
        fixture.quorum_certificate(wire::GlobalPhase::Commit, fixture.canonical_commitment);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let message =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::QuorumCertificate(
            commit,
        ));
    let sender = PeerId::new(fixture.validator_keys[0].public_key().clone());
    let ownership = fair_transport_ingress_ownership(message.clone(), sender);
    let _admission = fixture
        .executor
        .enqueue_discovered_commit_certificate(message.clone(), ownership)
        .expect("enqueue the authenticated live CommitQC");
    let output_guard = Arc::clone(&fixture.executor.output_guard);
    let mut services = FakeServices::default();
    for _ in 0..8 {
        fixture
            .executor
            .step(Instant::now(), &mut services)
            .expect("advance the authenticated live CommitQC");
        if fixture.executor.has_pending_runner_decision_cleanup_for_test() {
            break;
        }
    }
    assert_eq!(fixture.executor.protected_decision, Some(decision));
    assert_eq!(
        fixture
            .executor
            .pending_runner_decision_cleanup
            .map(|pending| pending.decision),
        Some(decision)
    );
    assert!(fixture.executor.retained_effect_batch.is_none());
    assert!(fixture.executor.parked_effect_batch.is_none());
    assert!(services.apply_tasks.is_empty());
    assert!(local_proposal.already_attempted(pre_decision_directive));

    let permit = super::super::v2_runner::LifecycleProducerClaimDispositionV1::AwaitingApplyCompletion
        .decided_lane_recovery_permit()
        .expect("the typed Apply barrier mints only decided-lane authority");
    super::super::v2_runner::lifecycle_run_inner::settle_apply_barrier_runner_decision_handoff(
        &mut fixture.executor,
        &mut services,
        &mut local_proposal,
        &mut lane_work,
        output_guard.as_ref(),
        &permit,
    )
    .expect("retire the exact live Decision handoff behind Apply");
    assert!(!fixture.executor.has_pending_runner_decision_cleanup_for_test());
    assert!(local_proposal.is_pristine_for_test());
    assert!(!local_proposal.already_attempted(pre_decision_directive));
    assert_eq!(
        super::super::v2_lane_work::tests::runner_handoff_losing_merge_counts_for_test(
            &lane_work,
        ),
        (0, 0)
    );
    assert!(!output_guard.restart_required());
}

#[test]
fn decision_cleanup_batch_accepts_zero_or_one_exact_apply_only() {
    let fixture = Fixture::new();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let apply = AdapterEffect::Apply {
        tag: tag(0),
        subject: commit.subject,
        certificate: commit,
    };
    assert!(
        V2EffectExecutor::<FakeRuntime>::new_decision_batch_has_only_exact_apply(
            &[],
            decision,
            Some(tag(0)),
        )
    );
    assert!(
        V2EffectExecutor::<FakeRuntime>::new_decision_batch_has_only_exact_apply(
            std::slice::from_ref(&apply),
            decision,
            Some(tag(0)),
        )
    );
    assert!(
        !V2EffectExecutor::<FakeRuntime>::new_decision_batch_has_only_exact_apply(
            &[apply.clone(), apply.clone()],
            decision,
            Some(tag(0)),
        )
    );
    let owner_tag = tag(0);
    let wrong_generation = EventTag::new(
        owner_tag.height(),
        owner_tag.view(),
        Generation::new(owner_tag.generation().get().saturating_add(1)),
    );
    assert!(
        !V2EffectExecutor::<FakeRuntime>::new_decision_batch_has_only_exact_apply(
            &[apply],
            decision,
            Some(wrong_generation),
        )
    );
}
#[test]
fn split_decision_fetch_and_apply_stops_runtime_until_runner_cleanup_acknowledges() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let apply = AdapterEffect::Apply {
        tag: tag(0),
        subject: commit.subject,
        certificate: commit.clone(),
    };
    let fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: commit.round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit),
    };
    executor.runtime.decision_on_next_step = Some(decision);
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![fetch])));
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![apply])));

    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("dispatch Decision Fetch while arming runner cleanup"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .map(|pending| pending.decision),
        Some(decision)
    );
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .expect("split Decision cleanup owner")
            .owner_tag,
        tag(0)
    );
    assert_eq!(services.fetch_tasks.len(), 1);
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("do not let delayed Apply overtake runner cleanup"),
        EffectExecutorStep::Idle
    );
    assert_eq!(executor.runtime.steps.len(), 1);
    assert!(services.apply_tasks.is_empty());

    let owner_tag = tag(0);
    let advanced_generation = EventTag::new(
        owner_tag.height(),
        owner_tag.view(),
        Generation::new(owner_tag.generation().get().saturating_add(1)),
    );
    executor.runtime.round_tag = Some(advanced_generation);
    assert!(matches!(
        executor.acknowledge_runner_decision_cleanup(advanced_generation, Some(decision.2),),
        Err(EffectExecutorError::Contract(_))
    ));
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .expect("Decision-install cleanup owner remains retained")
            .owner_tag,
        owner_tag
    );
    executor.runtime.round_tag = Some(owner_tag);
    executor
        .acknowledge_runner_decision_cleanup(owner_tag, Some(decision.2))
        .expect("a split Decision can be acknowledged before Apply is emitted");
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("dispatch delayed Apply after runner cleanup"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(services.apply_tasks[0].subject(), decision.2);
}
#[test]
fn pacemaker_decision_holds_apply_until_runner_cleanup_acknowledges() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let apply = AdapterEffect::Apply {
        tag: tag(0),
        subject: commit.subject,
        certificate: commit,
    };
    executor.runtime.decision_on_next_step = Some(decision);
    executor
        .runtime
        .pacemaker_steps
        .push_back(Ok(Some(RuntimeStep::Advanced(vec![apply]))));

    assert_eq!(
        executor
            .step_pacemaker_once(Instant::now(), &mut services)
            .expect("retain pacemaker Apply behind runner cleanup"),
        EffectExecutorStep::Advanced { effects: 0 }
    );
    assert_eq!(
        executor
            .pending_runner_decision_cleanup
            .map(|pending| pending.decision),
        Some(decision)
    );
    assert_eq!(
        executor
            .step_pacemaker_once(Instant::now(), &mut services)
            .expect("pending cleanup blocks another pacemaker turn"),
        EffectExecutorStep::Idle
    );
    assert!(services.apply_tasks.is_empty());

    executor
        .acknowledge_runner_decision_cleanup(tag(0), Some(decision.2))
        .expect("acknowledge pacemaker Decision cleanup");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("dispatch pacemaker Apply after runner cleanup"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(services.apply_tasks[0].subject(), decision.2);
}

#[test]
fn later_decision_apply_uses_its_runtime_owner_after_terminal_validate() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = executor.durable_bodies[&key].clone();

    // Retain the inert retry marker left by an old-view lifecycle Validate
    // which durably terminalized without a successor.
    let old_tag = tag(0);
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let fetch = AdapterEffect::FetchBody {
        tag: old_tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let store = AdapterEffect::StoreBody {
        tag: old_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag: old_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let terminal_validate_ordinal = 9_028;
    let store_ownership = bound_test_effect_ownership(&fetch, old_tag, terminal_validate_ordinal)
        .rebind_as_inherited_adapter_effect(&store)
        .expect("project the terminal Validate Store predecessor");
    let store_pending = store_ownership
        .exact_pending_adapter_effect_binding(&store)
        .expect("seal the terminal Validate Store predecessor");
    let prepared_store = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the terminal Validate Store marker")
        .bind_store_successor(&store, &store_pending)
        .expect("bind the terminal Validate Store marker");
    executor.commit_published_lifecycle_store_retry_marker(prepared_store);
    let validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("project the terminal Validate owner");
    let validate_pending = validate_ownership
        .exact_pending_adapter_effect_binding(&validate)
        .expect("seal the terminal Validate owner");
    let prepared_validate = executor
        .prepare_published_lifecycle_validate_retry_marker(&durable)
        .expect("preflight the terminal Validate marker")
        .bind_validate_successor(&validate, &validate_pending)
        .expect("bind the terminal Validate marker");
    executor.commit_published_lifecycle_validate_retry_marker(
        prepared_validate,
        terminal_validate_ordinal,
    );
    assert!(
        executor
            .release_validate_retry_lifecycle_ordinal(key, terminal_validate_ordinal)
            .expect("release the durably terminal Validate ordinal")
    );

    // EnterView is outside this focused fixture. Synchronize both sides of
    // its already-settled reconciliation frontier before the later Decision
    // emits its ordinary Apply-only macro-step.
    let current_tag = tag(1);
    executor.runtime.round_tag = Some(current_tag);
    executor.reconciled_tag = Some(current_tag);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let apply = AdapterEffect::Apply {
        tag: current_tag,
        subject: commit.subject,
        certificate: commit.clone(),
    };
    let decision_apply_ordinal = 9_029;
    let decision_apply_ownership = bound_test_apply_ownership(
        current_tag,
        commit.subject,
        &commit,
        current_tag,
        decision_apply_ordinal,
    );
    assert_ne!(decision_apply_ordinal, terminal_validate_ordinal);
    executor.runtime.decision_on_next_step = Some(decision);
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![apply.clone()])));
    executor.runtime.exact_effect_ownership =
        Some((apply.clone(), decision_apply_ownership.clone()));

    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("retain the later Decision Apply behind runner cleanup"),
        EffectExecutorStep::Advanced { effects: 0 }
    );
    assert!(executor.pending_applications.is_empty());
    assert!(!executor.published_lifecycle_validate_retry_markers[&key].owns_live_lifecycle_row());
    executor
        .acknowledge_runner_decision_cleanup(current_tag, Some(decision.2))
        .expect("acknowledge the later Decision cleanup");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("dispatch the ordinary Decision Apply"),
        EffectExecutorStep::Advanced { effects: 1 }
    );

    let pending = executor
        .pending_applications
        .values()
        .next()
        .expect("the ordinary Decision Apply remains pending");
    assert_eq!(pending.task.lifecycle_ordinal(), decision_apply_ordinal);
    assert_eq!(pending.task.authorized_owner_tag(), current_tag);
    assert_eq!(pending.ownership.owner(), decision_apply_ownership.owner());
    assert!(pending.ownership.exactly_binds_adapter_effect(&apply));
    assert!(
        !executor
            .published_lifecycle_validate_retry_markers
            .contains_key(&key),
        "Decision cleanup must retire the inert terminal Validate marker"
    );
    assert!(executor.live_lifecycle_decision_apply.is_none());
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(
        services.apply_tasks[0].lifecycle_ordinal(),
        decision_apply_ordinal
    );
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn commit_fetch_adopts_and_replaces_matching_parked_physical_lineage() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let ordinary = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let ordinary_ownership = executor
        .runtime
        .take_effect_ownership(std::slice::from_ref(&ordinary))
        .expect("bind ordinary parked Fetch");
    let parked_owner = ordinary_ownership[0].clone();
    executor
        .retain_effect_batch(vec![ordinary], ordinary_ownership)
        .expect("retain ordinary Fetch suffix");
    executor
        .park_retained_effect_batch()
        .expect("park ordinary Fetch behind certified progress");
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let commit_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit.clone()),
    };
    assert_eq!(
        executor
            .consume_effects(vec![commit_fetch], &mut services)
            .expect("Commit-certified Fetch replaces its parked physical lineage"),
        1
    );
    assert_eq!(services.fetch_tasks.len(), 1);
    assert_eq!(services.fetch_tasks[0].ownership(), &parked_owner);
    assert_eq!(executor.pending_fetches.len(), 1);
    assert!(executor.parked_effect_batch.is_none());
    assert!(executor.retained_effect_batch.is_none());
    assert_eq!(executor.pending_work(), 1);
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}
#[test]
fn failed_decision_cleanup_keeps_losing_owner_and_requires_restart() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (losing_subject, losing_body) = distinct_body(&fixture);
    let losing_manifest = canonical_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        losing_subject,
        &losing_body,
    );
    let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    losing_prepare.subject = losing_manifest.subject;
    let certified_sources = certified_sources(&fixture, &losing_prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: losing_manifest.round,
                subject: losing_manifest.subject,
                manifest: Some(losing_manifest),
                certified_sources,
                certificate: Some(losing_prepare),
            }],
            &mut services,
        )
        .expect("admit losing body recovery");
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let durable_decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    executor.runtime.decided_body = Some(durable_decision);
    let before = executor.body_ownership_projection();
    services.fail_on = Some("cancel-fetch");
    assert!(matches!(
        executor.consume_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(executor.runtime.decided_body, Some(durable_decision));
    assert_eq!(executor.protected_decision, None);
    assert!(executor.output_guard.restart_required());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
    assert!(matches!(
        executor.consume_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::FailClosed(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn decision_cleanup_fetch_failure_preserves_exact_local_pipeline_consumer() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start exact decided local store");
    let (losing_subject, losing_body) = distinct_body(&fixture);
    let losing_manifest = canonical_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        losing_subject,
        &losing_body,
    );
    let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    losing_prepare.subject = losing_manifest.subject;
    let certified_sources = certified_sources(&fixture, &losing_prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: losing_manifest.round,
                subject: losing_manifest.subject,
                manifest: Some(losing_manifest),
                certified_sources,
                certificate: Some(losing_prepare),
            }],
            &mut services,
        )
        .expect("admit losing certified recovery beside decided local work");
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let before = executor.body_ownership_projection();
    services.fail_on = Some("cancel-fetch");
    assert!(matches!(
        executor.consume_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(executor.pending_stores.len(), 1);
    assert!(matches!(
        executor
            .pending_stores
            .values()
            .next()
            .and_then(|pending| pending.consumer.as_ref()),
        Some(StoreConsumer::LocalProposal { .. })
    ));
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn decision_cleanup_rejects_inconsistent_certified_request_before_mutation() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (losing_subject, losing_body) = distinct_body(&fixture);
    let losing_manifest = canonical_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        losing_subject,
        &losing_body,
    );
    let mut losing_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    losing_prepare.subject = losing_manifest.subject;
    let certified_sources = certified_sources(&fixture, &losing_prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: losing_manifest.round,
                subject: losing_manifest.subject,
                manifest: Some(losing_manifest),
                certified_sources,
                certificate: Some(losing_prepare),
            }],
            &mut services,
        )
        .expect("admit losing certified recovery");
    let request_hash = *executor
        .certified_work
        .keys()
        .next()
        .expect("certified request index");
    assert!(executor.outstanding_requests.cancel(request_hash));
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let durable_decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    executor.runtime.decided_body = Some(durable_decision);
    let before = executor.body_ownership_projection();
    assert!(matches!(
        executor.consume_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Contract(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(executor.runtime.decided_body, Some(durable_decision));
    assert_eq!(executor.protected_decision, None);
    assert!(services.cancelled_fetches.is_empty());
    assert_eq!(services.retired_all_outbound, 0);
    assert_eq!(services.retired_candidate_work, 0);
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn decision_preserves_current_tag_local_proposal_for_direct_apply() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start exact local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::LocalProposal(_, manifest, ..)]
            if manifest == &fixture.manifest
    ));
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let certified_sources = certified_sources(&fixture, &commit);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: commit.round,
                subject: commit.subject,
                manifest: None,
                certified_sources,
                certificate: Some(commit),
            }],
            &mut services,
        )
        .expect("preserve the exact local completion across Decision cleanup");
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::LocalProposal(completion_tag, manifest, ..)]
            if *completion_tag == tag(0) && manifest == &fixture.manifest
    ));
    assert_eq!(executor.body_pipeline_owners.len(), 1);
    assert!(services.fetch_tasks.is_empty());
    assert_eq!(services.retired_all_outbound, 1);
    assert_eq!(services.retired_candidate_work, 1);
    executor
        .consume_effects(Vec::new(), &mut services)
        .expect("Decision reconciliation is idempotent");
    assert_eq!(services.retired_all_outbound, 1);
    assert_eq!(services.retired_candidate_work, 1);
    assert!(!executor.status().fail_closed);
}
#[test]
fn decision_commitment_mismatch_fails_closed_before_apply() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start exact local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    let conflicting_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"Decision conflict parent state"),
        Hash::new(b"Decision conflict post state"),
        Hash::new(b"Decision conflict ordinary writes"),
        1,
        Hash::new(b"Decision conflict executed block"),
    );
    assert_ne!(conflicting_commitment, fixture_execution_commitment());
    executor.runtime.decided_body = Some((
        fixture.manifest.round,
        fixture.manifest.round,
        fixture.manifest.subject,
        conflicting_commitment,
    ));
    assert!(matches!(
        executor.consume_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Runtime(reason))
            if reason.contains("conflicts with the durable Decision")
    ));
    assert!(executor.status().fail_closed);
    assert!(services.apply_tasks.is_empty());
    assert!(services.fetch_tasks.is_empty());
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::LocalProposal(completion_tag, manifest, ..)]
            if *completion_tag == tag(0) && manifest == &fixture.manifest
    ));
}
#[test]
fn reconciled_decision_rejects_same_round_subject_commitment_drift() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let first = (
        fixture.manifest.round,
        fixture.manifest.round,
        fixture.manifest.subject,
        fixture_execution_commitment(),
    );
    executor.runtime.decided_body = Some(first);
    executor
        .consume_effects(Vec::new(), &mut services)
        .expect("install the first full durable Decision identity");
    assert_eq!(executor.protected_decision, Some(first));
    let retired_outbound = services.retired_all_outbound;
    let retired_candidate = services.retired_candidate_work;
    let drifted_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"drifted Decision parent state"),
        Hash::new(b"drifted Decision post state"),
        Hash::new(b"drifted Decision ordinary writes"),
        1,
        Hash::new(b"drifted Decision executed block"),
    );
    assert_ne!(drifted_commitment, first.3);
    executor.runtime.decided_body = Some((first.0, first.1, first.2, drifted_commitment));
    assert!(matches!(
        executor.consume_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("two different durable Decision identities")
    ));
    assert_eq!(executor.protected_decision, Some(first));
    assert_eq!(services.retired_all_outbound, retired_outbound);
    assert_eq!(services.retired_candidate_work, retired_candidate);
    assert!(executor.output_guard.restart_required());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn stale_generation_local_completion_uses_durable_recovery() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start old-generation local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    let current_tag = EventTag::new(1, 1, Generation::new(8));
    executor.runtime.round_tag = Some(current_tag);
    executor.reconciled_tag = Some(current_tag);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let certified_sources = certified_sources(&fixture, &commit);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: current_tag,
                round: commit.round,
                subject: commit.subject,
                manifest: None,
                certified_sources,
                certificate: Some(commit),
            }],
            &mut services,
        )
        .expect("stale completion falls back to durable body reconstruction");
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
            if *completion_tag == current_tag && manifest == &fixture.manifest
    ));
    assert!(services.apply_tasks.is_empty());
    assert!(services.fetch_tasks.is_empty());
    assert_eq!(executor.body_pipeline_owners.len(), 1);
    assert!(!executor.status().fail_closed);
}
#[test]
fn decision_body_stage_adoption_promotes_matching_prepare_authority() {
    let fixture = Fixture::new();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    assert_eq!(prepare.execution_commitment, commit.execution_commitment);
    let prepare_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let validate = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let incumbent = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&prepare_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 9_010)],
    )
    .expect("bind Prepare-authorized FetchBody")
    .pop()
    .expect("one Prepare FetchBody owner")
    .rebind_as_inherited_adapter_effect(&validate)
    .expect("carry Prepare authority into ValidateBody");
    let commit_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit.clone()),
    };
    let incoming = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&commit_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 9_011)],
    )
    .expect("bind Commit-authorized FetchBody")
    .pop()
    .expect("one Commit FetchBody owner")
    .rebind_as_inherited_adapter_effect(&validate)
    .expect("carry Commit authority into ValidateBody");
    assert_ne!(incumbent, incoming);
    let adopted = incumbent
        .adopt_incumbent_body_stage_for_durable_decision(
            &incoming,
            &validate,
            commit.round,
            commit.proposal_round,
            commit.subject,
            commit.execution_commitment,
        )
        .expect("matching Commit authority adopts the incumbent validation root");
    assert_eq!(adopted, incumbent);
}

fn prepared_remote_proposal_fetch_replay(
    fixture: &Fixture,
    replay_tag: EventTag,
    ordinal: u128,
) -> (
    AdapterEffect,
    RuntimeEffectOwnership,
    PreparedRemoteProposalFetchReplayPreAdmission,
) {
    let fetch_effect = AdapterEffect::FetchBody {
        tag: replay_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let mut fetch_ownership = bound_test_effect_ownership(&fetch_effect, replay_tag, ordinal);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal(fixture).payload else {
        unreachable!("Proposal fixture has one Proposal payload")
    };
    assert!(
        fetch_ownership
            .bind_authenticated_remote_proposal_replay_for_test(proposal, &fetch_effect,)
    );
    let fetch_replay = PreparedRemoteProposalFetchReplayPreAdmission::seal_exact_fetch(
        fetch_effect.clone(),
        fetch_ownership.clone(),
    )
    .unwrap_or_else(|_| panic!("seal exact authenticated Proposal Fetch"));
    (fetch_effect, fetch_ownership, fetch_replay)
}

fn prepared_remote_proposal_store_replay(
    fixture: &Fixture,
    replay_tag: EventTag,
    ordinal: u128,
) -> (
    AdapterEffect,
    RuntimeEffectOwnership,
    AdapterEffect,
    RuntimeEffectOwnership,
    PreparedRemoteProposalStoreReplayPreAdmission,
) {
    let (fetch_effect, fetch_ownership, fetch_replay) =
        prepared_remote_proposal_fetch_replay(fixture, replay_tag, ordinal);
    let store_effect = AdapterEffect::StoreBody {
        tag: replay_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let store_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("project exact Proposal Store owner");
    let store_replay = fetch_replay
        .project_store(store_effect.clone(), store_ownership.clone())
        .unwrap_or_else(|_| panic!("project exact authenticated Proposal Store"));
    (
        fetch_effect,
        fetch_ownership,
        store_effect,
        store_ownership,
        store_replay,
    )
}

fn install_stored_remote_proposal_replay(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    fixture: &Fixture,
    replay_tag: EventTag,
    ordinal: u128,
) -> RuntimeEffectOwnership {
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let (_fetch_effect, _fetch_ownership, _store_effect, store_ownership, store_replay) =
        prepared_remote_proposal_store_replay(fixture, replay_tag, ordinal);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    let stored_replay = store_replay
        .bind_durable_body(durable.clone())
        .unwrap_or_else(|_| panic!("bind exact durable Proposal body"));
    assert!(executor.durable_bodies.insert(key, durable).is_none());
    assert!(
        executor
            .remote_proposal_replay
            .insert(
                key,
                RemoteProposalReplayStageV1::Stored {
                    replay: stored_replay,
                    ownership: store_ownership.clone(),
                },
            )
            .is_none()
    );
    store_ownership
}

#[test]
fn stored_proposal_replay_projects_store_owner_before_terminal_coalescing() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let incumbent = install_stored_remote_proposal_replay(&mut executor, &fixture, tag(0), 9_022);
    let later_store = AdapterEffect::StoreBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let foreign_ordinary = bound_test_effect_ownership(&later_store, tag(1), 9_023);
    assert_ne!(foreign_ordinary.owner(), incumbent.owner());
    let projected = executor
        .stored_replay_incumbent_store_ownership(key, &later_store, &foreign_ordinary)
        .expect("inspect exact durable Proposal replay")
        .expect("the Stored Proposal projects its physical Store owner");
    assert_eq!(projected.owner(), incumbent.owner());
    assert!(projected.exactly_binds_adapter_effect(&later_store));

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let certified_fetch = AdapterEffect::FetchBody {
        tag: tag(1),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let certified_store = bound_test_effect_ownership(&certified_fetch, tag(1), 9_024)
        .rebind_as_inherited_adapter_effect(&later_store)
        .expect("carry Prepare authority into the later Store");
    let strengthened = executor
        .stored_replay_incumbent_store_ownership(key, &later_store, &certified_store)
        .expect("inspect exact durable Proposal replay")
        .expect("the Stored Proposal adopts comparable Prepare authority");
    assert_eq!(strengthened.owner(), incumbent.owner());
    assert_ne!(
        strengthened.candidate_semantic_identity(),
        incumbent.candidate_semantic_identity(),
        "authority strengthening remains visible without replacing the physical owner",
    );

    let mismatched_store = AdapterEffect::StoreBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: wire::BlockSubject {
            payload_hash: Hash::new(b"foreign durable Store payload"),
            ..fixture.manifest.subject
        },
    };
    let mismatched_owner = bound_test_effect_ownership(&mismatched_store, tag(1), 9_025);
    assert!(matches!(
        executor.stored_replay_incumbent_store_ownership(
            key,
            &mismatched_store,
            &mismatched_owner,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("incumbent Store owner")
    ));
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));
    assert!(!executor.status().fail_closed);
}

#[test]
fn stored_proposal_validate_owner_reaches_terminal_query_before_late_carrier() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let store_incumbent =
        install_stored_remote_proposal_replay(&mut executor, &fixture, tag(0), 9_030);
    let later_validate = AdapterEffect::ValidateBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let foreign_later = bound_test_effect_ownership(&later_validate, tag(1), 9_031);
    assert_ne!(foreign_later.owner(), store_incumbent.owner());

    executor
        .retain_effect_batch(vec![later_validate.clone()], vec![foreign_later])
        .expect("Stored replay adopts Validate before the runtime terminal query");
    let [queried] = executor.runtime.terminal_body_candidate_queries.as_slice() else {
        panic!("one Validate carrier must reach one runtime terminal query")
    };
    assert_eq!(queried.owner(), store_incumbent.owner());
    assert!(queried.exactly_binds_adapter_effect(&later_validate));
    assert!(!executor.status().fail_closed);
}

#[test]
fn post_validate_proposal_seal_projects_late_store_before_terminal_coalescing() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let store_incumbent =
        install_stored_remote_proposal_replay(&mut executor, &fixture, tag(0), 9_026);
    let validate_effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let validate_ownership = store_incumbent
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("derive the exact Proposal Validate owner");
    assert!(
        executor
            .validate_body(
                tag(0),
                fixture.manifest.round,
                fixture.manifest.subject,
                validate_ownership.clone(),
                &mut services,
            )
            .expect("consume Stored replay into durable Validate admission")
            .is_none()
    );
    assert!(executor.remote_proposal_replay.is_empty());
    let DurableValidateRetrySealV1::Live {
        ownership,
        store_terminal: Some(_),
        ..
    } = &executor.durable_validate_retry_seals[&key]
    else {
        panic!("live Validate admission must retain its inert Store terminal owner")
    };
    assert_eq!(ownership.owner(), validate_ownership.owner());

    let later_store = AdapterEffect::StoreBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ordinary_retry = bound_test_effect_ownership(&later_store, tag(1), 9_027);
    assert_ne!(ordinary_retry.owner(), store_incumbent.owner());
    let sealed_stale = executor
        .stored_replay_incumbent_store_ownership(key, &later_store, &ordinary_retry)
        .expect("inspect the post-Validate Store terminal seal")
        .expect("the seal projects its immutable physical Store owner");
    assert_eq!(sealed_stale.owner(), store_incumbent.owner());
    assert!(sealed_stale.exactly_binds_adapter_effect(&later_store));

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let prepare_fetch = AdapterEffect::FetchBody {
        tag: tag(1),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let stronger_foreign = bound_test_effect_ownership(&prepare_fetch, tag(1), 9_029)
        .rebind_as_inherited_adapter_effect(&later_store)
        .expect("carry Prepare authority into the late Store");
    let strengthened = executor
        .stored_replay_incumbent_store_ownership(key, &later_store, &stronger_foreign)
        .expect("inspect the post-Validate Store terminal seal")
        .expect("the seal strengthens its immutable physical Store owner");
    assert_eq!(strengthened.owner(), store_incumbent.owner());
    assert_ne!(
        strengthened.candidate_semantic_identity(),
        sealed_stale.candidate_semantic_identity(),
        "Prepare authority must remain distinct from the sealed ordinary retry"
    );
    assert_eq!(strengthened.owner(), sealed_stale.owner());
    assert!(!executor.status().fail_closed);

    let foreign_effect = AdapterEffect::StoreBody {
        tag: tag(2),
        round: fixture.manifest.round,
        subject: wire::BlockSubject {
            payload_hash: Hash::new(b"foreign post-Validate Store payload"),
            ..fixture.manifest.subject
        },
    };
    let foreign_owner = bound_test_effect_ownership(&foreign_effect, tag(2), 9_028);
    assert!(matches!(
        executor.stored_replay_incumbent_store_ownership(
            key,
            &foreign_effect,
            &foreign_owner,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("post-Validate Store terminal changed")
    ));
    assert!(!executor.status().fail_closed);
}

fn install_inflight_remote_proposal_store(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    services: &mut FakeServices,
    fixture: &Fixture,
    replay_tag: EventTag,
    ordinal: u128,
) -> EffectWorkId {
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let ready = ReadyBody::derive(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        fixture.body.clone(),
    )
    .expect("derive the authenticated Proposal body");
    executor.ready_body_bytes =
        u64::try_from(ready.bytes.len()).expect("fixture body length is representable");
    assert!(executor.ready_bodies.insert(key, ready).is_none());
    assert!(
        executor
            .body_pipeline_owners
            .insert(
                key,
                BodyPipelineOwner {
                    tag: replay_tag,
                    manifest_hash: Some(HashOf::new(&fixture.manifest)),
                },
            )
            .is_none()
    );
    let (_fetch_effect, fetch_ownership, fetch_replay) =
        prepared_remote_proposal_fetch_replay(fixture, replay_tag, ordinal);
    assert!(
        executor
            .remote_proposal_replay
            .insert(
                key,
                RemoteProposalReplayStageV1::BodyAvailable(fetch_replay),
            )
            .is_none()
    );
    let store_effect = AdapterEffect::StoreBody {
        tag: replay_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let store_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("project the exact authenticated Proposal Store owner");
    executor
        .retain_effect_batch(vec![store_effect], vec![store_ownership])
        .expect("retain the authenticated Proposal Store");
    assert_eq!(
        executor
            .drain_retained_effect_batch(services, true)
            .expect("start the protected durable Store"),
        1
    );
    let store_id = services.store_tasks[0].id();
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Store { work_id, .. })
            if *work_id == store_id
    ));
    store_id
}

fn install_recovered_validate_retry_seal(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    fixture: &Fixture,
    replay_tag: EventTag,
    ordinal: u128,
) -> (
    (wire::ConsensusRound, wire::BlockSubject),
    AdapterEffect,
    DurableBodyReceipt,
) {
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let store_ownership =
        install_stored_remote_proposal_replay(executor, fixture, replay_tag, ordinal);
    let store_effect = AdapterEffect::StoreBody {
        tag: replay_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let effect = AdapterEffect::ValidateBody {
        tag: replay_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let pending = store_ownership
        .exact_pending_adapter_effect_binding(&store_effect)
        .expect("project recovered Validate parent binding")
        .project_store_validate_successor(&store_effect, &effect)
        .expect("project recovered Validate pending binding");
    let durable = executor.durable_bodies[&key].clone();
    let owner = RecoveredDurableValidateRetryOwnerV1::for_test(
        effect.clone(),
        durable.clone(),
        &pending,
        ordinal,
        None,
    )
    .expect("seal recovered Validate retry owner");
    let mut installation = executor
        .prepare_recovered_durable_validate_retry_install()
        .expect("prepare recovered Validate retry installation");
    installation
        .absorb(owner)
        .expect("preflight recovered Validate retry owner");
    installation
        .commit()
        .expect("install recovered Validate retry owner");
    executor.remote_proposal_replay.clear();
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    (key, effect, durable)
}

fn recovered_validate_retry_ownership(
    fixture: &Fixture,
    effect: &AdapterEffect,
    certificate: Option<wire::QuorumCertificate>,
    ordinal: u128,
) -> RuntimeEffectOwnership {
    let AdapterEffect::ValidateBody { tag, .. } = effect else {
        unreachable!("retry fixture retains one Validate effect")
    };
    let fetch_ownership = match certificate {
        None => prepared_remote_proposal_fetch_replay(fixture, *tag, ordinal).1,
        Some(certificate) => {
            let fetch = AdapterEffect::FetchBody {
                tag: *tag,
                round: certificate.proposal_round,
                subject: certificate.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: certified_sources(fixture, &certificate),
                certificate: Some(certificate),
            };
            bound_test_effect_ownership(&fetch, *tag, ordinal)
        }
    };
    fetch_ownership
        .rebind_as_inherited_adapter_effect(effect)
        .expect("carry exact retry authority into Validate")
}

fn assert_recovered_validate_retry_stutter_is_inert(executor: &V2EffectExecutor<FakeRuntime>) {
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.parked_effect_batch.is_none());
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert_eq!(executor.pending_work(), 0);
}

fn install_exact_recovered_body_without_lifecycle_replay(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    fixture: &Fixture,
) -> (
    (wire::ConsensusRound, wire::BlockSubject),
    DurableBodyReceipt,
) {
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(executor.authenticated_genesis_replay.is_empty());
    (key, durable)
}

fn protected_prepare_validate_fixture(
    fixture: &Fixture,
    ordinal: u128,
) -> (
    wire::QuorumCertificate,
    AdapterEffect,
    RuntimeEffectOwnership,
) {
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ownership =
        recovered_validate_retry_ownership(fixture, &effect, Some(prepare.clone()), ordinal);
    (prepare, effect, ownership)
}

fn assert_missing_replay_validate_fails_closed_without_body_mutation(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    services: &mut FakeServices,
    effect: AdapterEffect,
    ownership: RuntimeEffectOwnership,
    expected_reason: &str,
) {
    executor.runtime.exact_effect_ownership = Some((effect.clone(), ownership));
    let before = executor.body_ownership_projection();
    assert!(matches!(
        executor.consume_effects(vec![effect], services),
        Err(EffectExecutorError::Contract(reason)) if reason.contains(expected_reason)
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert!(executor.durable_validate_retry_seals.is_empty());
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(executor.authenticated_genesis_replay.is_empty());
    assert!(services.fetch_tasks.is_empty());
    assert!(services.store_tasks.is_empty());
    assert!(services.apply_tasks.is_empty());
    assert!(executor.status().fail_closed);
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}

#[test]
fn protected_prepare_validate_reseeds_missing_replay_from_exact_recovered_body() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (key, durable) =
        install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
    let (prepare, effect, ownership) = protected_prepare_validate_fixture(&fixture, 9_100);
    executor.protected_lock = Some(key);
    executor.runtime.locked_body = Some(key);
    executor.runtime.durable_body_authority_certificate = Some(prepare);
    executor.runtime.exact_effect_ownership = Some((effect.clone(), ownership.clone()));

    assert_eq!(
        executor
            .consume_effects(vec![effect.clone()], &mut services)
            .expect("the exact protected PrepareQC reseeds one durable Validate owner"),
        1
    );
    let pending = executor
        .pending_durable_validate_admissions
        .get(&key)
        .expect("missing Proposal replay is replaced by one normal pending admission");
    assert!(pending.exactly_retains_for_test(&effect, false));
    assert!(pending.exactly_matches_retry(&effect, &ownership));
    assert!(
        !pending.projects_local_proposal_handoff_for_test(),
        "historical protected-lock replay cannot become a local proposal producer",
    );
    assert_eq!(
        executor
            .durable_validate_retry_seals
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![key]
    );
    let DurableValidateRetrySealV1::Live {
        ownership: validate_incumbent,
        store_terminal: Some(_),
        ..
    } = &executor.durable_validate_retry_seals[&key]
    else {
        panic!("protected-lock Validate must retain its inert Store predecessor")
    };
    assert_eq!(validate_incumbent.owner(), ownership.owner());
    let validate_incumbent_owner = validate_incumbent.owner().clone();

    let later_store = AdapterEffect::StoreBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let later_ordinary = bound_test_effect_ownership(&later_store, tag(1), 9_107);
    assert_ne!(later_ordinary.owner(), &validate_incumbent_owner);
    let adopted = executor
        .stored_replay_incumbent_store_ownership(key, &later_store, &later_ordinary)
        .expect("inspect the protected-lock Store predecessor seal")
        .expect("the replay-retired Validate adopts a later stale Store carrier");
    assert_eq!(adopted.owner(), &validate_incumbent_owner);
    assert!(adopted.exactly_binds_adapter_effect(&later_store));
    executor.runtime.terminal_body_candidate_queries.clear();
    let terminal_identity = adopted
        .candidate_semantic_identity()
        .expect("the protected-lock Store predecessor has one candidate identity");
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(terminal_identity, adopted.clone())
            .is_none()
    );
    executor
        .retain_effect_batch(vec![later_store], vec![later_ordinary])
        .expect("the protected-lock predecessor adopts Store before terminal comparison");
    assert!(executor.retained_effect_batch.is_none());
    let [queried] = executor.runtime.terminal_body_candidate_queries.as_slice() else {
        panic!("the later Store must reach one terminal query under the predecessor owner")
    };
    assert_eq!(queried.owner(), &validate_incumbent_owner);
    assert_eq!(executor.runtime.terminal_body_candidate_commits, 1);
    assert_eq!(
        executor.durable_bodies.get(&key),
        Some(&durable),
        "Store retry projection cannot replace the durable receipt",
    );
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(executor.authenticated_genesis_replay.is_empty());
    assert!(services.fetch_tasks.is_empty());
    assert!(services.store_tasks.is_empty());
    assert!(services.apply_tasks.is_empty());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn protected_prepare_readmission_replaces_terminal_live_validate_tombstone() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (key, durable) =
        install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
    let terminal_ordinal = 9_106;
    install_bound_validate_retry_authority_for_cleanup(
        &mut executor,
        &fixture,
        key,
        BoundValidateRetryAuthorityKind::Live,
        terminal_ordinal,
    );
    assert!(
        executor
            .release_validate_retry_lifecycle_ordinal(key, terminal_ordinal)
            .expect("release the old view's terminal Validate row")
    );
    assert_eq!(
        executor.durable_validate_retry_seals[&key].lifecycle_ordinal(),
        None,
        "the completed old row remains only as an inert retry tombstone"
    );

    let current_tag = tag(1);
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let validated =
        ValidatedBodyReceipt::for_test_with_commitment(durable, prepare.execution_commitment);
    assert!(
        executor
            .validated_bodies
            .insert(key, validated.clone())
            .is_none()
    );
    let validate = AdapterEffect::ValidateBody {
        tag: current_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ownership =
        recovered_validate_retry_ownership(&fixture, &validate, Some(prepare.clone()), 9_110);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor.runtime.round_tag = Some(current_tag);
    executor.runtime.locked_body = Some(key);
    executor
        .install_view(
            current_tag,
            timeout,
            Some(prepare.clone()),
            None,
            &mut services,
        )
        .expect("install the exact protected view before its Validate retry");
    executor.runtime.durable_body_authority_certificate = Some(prepare);
    executor.runtime.exact_effect_ownership = Some((validate.clone(), ownership.clone()));

    assert_eq!(
        executor
            .consume_effects(vec![validate.clone()], &mut services)
            .expect("the protected Prepare reopens one normal Validate admission"),
        1,
        "an ordinal-free old-view tombstone cannot supply the current view's completion",
    );
    let pending = executor
        .pending_durable_validate_admissions
        .get(&key)
        .expect("the protected Prepare owns one fresh pending Validate admission");
    assert!(pending.exactly_matches_retry(&validate, &ownership));
    let DurableValidateRetrySealV1::Live {
        effect: retained_effect,
        ownership: retained_ownership,
        lifecycle_ordinal,
        ..
    } = &executor.durable_validate_retry_seals[&key]
    else {
        panic!("the fresh protected Validate must retain live lineage")
    };
    assert_eq!(retained_effect, &validate);
    assert_eq!(retained_ownership, &ownership);
    assert_eq!(*lifecycle_ordinal, None);
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));
    assert!(executor.pending_applications.is_empty());
    assert!(executor.live_lifecycle_decision_apply.is_none());
    assert!(services.apply_tasks.is_empty());
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}

#[test]
fn protected_prepare_readmission_rolls_back_with_a_malformed_later_effect() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (key, _) = install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
    let terminal_ordinal = 9_120;
    install_bound_validate_retry_authority_for_cleanup(
        &mut executor,
        &fixture,
        key,
        BoundValidateRetryAuthorityKind::Live,
        terminal_ordinal,
    );
    assert!(
        executor
            .release_validate_retry_lifecycle_ordinal(key, terminal_ordinal)
            .expect("release the old terminal Validate before rollback preflight")
    );
    let tombstone_before = executor.durable_validate_retry_seals[&key].clone();

    let current_tag = tag(1);
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor.runtime.round_tag = Some(current_tag);
    executor.runtime.locked_body = Some(key);
    executor
        .install_view(
            current_tag,
            timeout,
            Some(prepare.clone()),
            None,
            &mut services,
        )
        .expect("install the exact protected view for rollback preflight");

    let prepare_fetch = AdapterEffect::FetchBody {
        tag: current_tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let validate = AdapterEffect::ValidateBody {
        tag: current_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let store = AdapterEffect::StoreBody {
        tag: current_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let source_effects = vec![prepare_fetch, store];
    let mut owners = bind_adapter_effect_batch_ownership(
        &source_effects,
        vec![
            RuntimeEffectOwnership::fresh_for_test(current_tag, 9_121),
            RuntimeEffectOwnership::fresh_for_test(current_tag, 9_122),
        ],
    )
    .expect("bind the two-position rollback source batch");
    let prepare_validate_owner = owners
        .remove(0)
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("carry Prepare authority into the first Validate position");
    let malformed_later_owner = owners.remove(0);
    let terminal_commits_before = executor.runtime.terminal_body_candidate_commits;

    assert!(
        executor
            .retain_effect_batch(
                vec![validate.clone(), validate],
                vec![prepare_validate_owner, malformed_later_owner],
            )
            .is_err(),
        "the Store-bound second owner must fail the later Validate position"
    );
    assert_eq!(
        executor.durable_validate_retry_seals[&key], tombstone_before,
        "the transactional clone must restore the first position's tombstone"
    );
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before
    );
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
}

#[test]
fn missing_replay_validate_rejects_absent_protected_lock_or_durable_prepare_qc() {
    let fixture = Fixture::new();

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let _recovered =
            install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
        let (prepare, effect, ownership) = protected_prepare_validate_fixture(&fixture, 9_101);
        executor.runtime.durable_body_authority_certificate = Some(prepare);
        assert_missing_replay_validate_fails_closed_without_body_mutation(
            &mut executor,
            &mut services,
            effect,
            ownership,
            "is not the protected durable body",
        );
    }

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (key, _) =
            install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
        let (_prepare, effect, ownership) = protected_prepare_validate_fixture(&fixture, 9_102);
        executor.protected_lock = Some(key);
        executor.runtime.locked_body = Some(key);
        assert_missing_replay_validate_fails_closed_without_body_mutation(
            &mut executor,
            &mut services,
            effect,
            ownership,
            "omitted its durable QC",
        );
    }
}

#[test]
fn missing_replay_validate_rejects_mismatched_durable_prepare_commitment() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (key, _) = install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
    let (prepare, effect, ownership) = protected_prepare_validate_fixture(&fixture, 9_103);
    let mut mismatched = prepare;
    mismatched.execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"foreign protected-lock parent state"),
        Hash::new(b"foreign protected-lock post state"),
        Hash::new(b"foreign protected-lock ordinary writes"),
        1,
        Hash::new(b"foreign protected-lock executed block"),
    );
    assert!(mismatched.validate(&fixture.context).is_ok());
    executor.protected_lock = Some(key);
    executor.runtime.locked_body = Some(key);
    executor.runtime.durable_body_authority_certificate = Some(mismatched);
    assert_missing_replay_validate_fails_closed_without_body_mutation(
        &mut executor,
        &mut services,
        effect,
        ownership,
        "changed its durable QC coordinates",
    );
}

#[test]
fn missing_replay_validate_rejects_wrong_recovered_manifest_or_receipt() {
    let fixture = Fixture::new();

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (key, durable) =
            install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
        let foreign_manifest = deliberately_conflicting_payload_manifest(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            b"foreign recovered protected-lock body",
        );
        assert_ne!(HashOf::new(&foreign_manifest), durable.manifest_hash());
        assert!(
            executor
                .recovered_bodies
                .insert(key, (foreign_manifest, durable))
                .is_some()
        );
        let (prepare, effect, ownership) = protected_prepare_validate_fixture(&fixture, 9_104);
        executor.protected_lock = Some(key);
        executor.runtime.locked_body = Some(key);
        executor.runtime.durable_body_authority_certificate = Some(prepare);
        assert_missing_replay_validate_fails_closed_without_body_mutation(
            &mut executor,
            &mut services,
            effect,
            ownership,
            "could not reseal exact lifecycle replay",
        );
    }

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (key, durable) =
            install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
        let foreign_manifest = deliberately_conflicting_payload_manifest(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            b"foreign recovered protected-lock receipt",
        );
        let foreign_receipt = DurableBodyReceipt::for_test(
            fixture.context.id(),
            fixture.manifest.round,
            fixture.manifest.subject,
            HashOf::new(&foreign_manifest),
        );
        assert_ne!(foreign_receipt, durable);
        assert!(
            executor
                .recovered_bodies
                .insert(key, (fixture.manifest.clone(), foreign_receipt))
                .is_some()
        );
        let (prepare, effect, ownership) = protected_prepare_validate_fixture(&fixture, 9_105);
        executor.protected_lock = Some(key);
        executor.runtime.locked_body = Some(key);
        executor.runtime.durable_body_authority_certificate = Some(prepare);
        assert_missing_replay_validate_fails_closed_without_body_mutation(
            &mut executor,
            &mut services,
            effect,
            ownership,
            "changed its durable body receipt",
        );
    }
}

#[derive(Clone, Copy, Debug)]
enum BoundValidateRetryAuthorityKind {
    Live,
    Recovered,
    Published,
}

fn install_bound_validate_retry_authority_for_cleanup(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    fixture: &Fixture,
    key: (wire::ConsensusRound, wire::BlockSubject),
    kind: BoundValidateRetryAuthorityKind,
    lifecycle_ordinal: u128,
) {
    let effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: key.0,
        subject: key.1,
    };
    let fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: key.0,
        subject: key.1,
        manifest: None,
        certified_sources: Vec::new(),
        certificate: None,
    };
    let ownership = bound_test_effect_ownership(&fetch, tag(0), lifecycle_ordinal + 1)
        .rebind_as_inherited_adapter_effect(&effect)
        .expect("project exact cleanup Validate owner");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&effect)
        .expect("seal exact cleanup Validate binding");
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        key.0,
        key.1,
        HashOf::new(&fixture.manifest),
    );
    match kind {
        BoundValidateRetryAuthorityKind::Live => {
            assert!(
                executor
                    .durable_validate_retry_seals
                    .insert(
                        key,
                        DurableValidateRetrySealV1::Live {
                            effect,
                            ownership,
                            store_terminal: None,
                            lifecycle_ordinal: Some(lifecycle_ordinal),
                        },
                    )
                    .is_none()
            );
        }
        BoundValidateRetryAuthorityKind::Recovered => {
            let owner = RecoveredDurableValidateRetryOwnerV1::for_test(
                effect,
                durable,
                &pending,
                lifecycle_ordinal,
                None,
            )
            .expect("seal recovered cleanup Validate owner");
            let frontier = owner
                .initial_retry_frontier()
                .expect("recovered cleanup owner retains its initial frontier");
            assert!(
                executor
                    .durable_validate_retry_seals
                    .insert(
                        key,
                        DurableValidateRetrySealV1::Recovered {
                            owner: Arc::new(owner),
                            frontier,
                            lifecycle_ordinal: Some(lifecycle_ordinal),
                        },
                    )
                    .is_none()
            );
        }
        BoundValidateRetryAuthorityKind::Published => {
            let mut marker =
                PublishedLifecycleValidateRetryMarkerV1::prepare(&effect, &durable, &pending)
                    .expect("seal published cleanup Validate marker");
            marker
                .bind_lifecycle_ordinal(lifecycle_ordinal)
                .expect("bind published cleanup Validate ordinal");
            assert!(
                executor
                    .published_lifecycle_validate_retry_markers
                    .insert(key, marker)
                    .is_none()
            );
        }
    }
}

fn bound_validate_retry_ordinal_for_cleanup(
    executor: &V2EffectExecutor<FakeRuntime>,
    key: (wire::ConsensusRound, wire::BlockSubject),
    kind: BoundValidateRetryAuthorityKind,
) -> Option<u128> {
    match kind {
        BoundValidateRetryAuthorityKind::Live | BoundValidateRetryAuthorityKind::Recovered => {
            executor
                .durable_validate_retry_seals
                .get(&key)
                .and_then(DurableValidateRetrySealV1::lifecycle_ordinal)
        }
        BoundValidateRetryAuthorityKind::Published => executor
            .published_lifecycle_validate_retry_markers
            .get(&key)
            .and_then(|marker| marker.lifecycle_ordinal),
    }
}

#[test]
fn recovered_apply_releases_only_its_authenticated_validate_retry_predecessor() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let key = (commit.proposal_round, commit.subject);
    let validate_predecessor_ordinal = 40_030;
    let apply_ordinal = 40_042;
    assert!(
        validate_predecessor_ordinal + 1 < apply_ordinal,
        "the regression must retain a non-adjacent recovered predecessor"
    );
    install_bound_validate_retry_authority_for_cleanup(
        &mut executor,
        &fixture,
        key,
        BoundValidateRetryAuthorityKind::Recovered,
        validate_predecessor_ordinal,
    );
    executor.protected_decision = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let dispatch_key = LifecycleDecisionApplyDispatchKeyV1::for_height_context_test(
        &fixture.context,
        apply_ordinal,
        0xD4,
    );

    assert!(matches!(
        executor.preflight_recovered_apply_validate_retry_predecessor(
            dispatch_key,
            key,
            validate_predecessor_ordinal + 1,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("exact durable Validate predecessor ordinal")
    ));
    assert_eq!(
        bound_validate_retry_ordinal_for_cleanup(
            &executor,
            key,
            BoundValidateRetryAuthorityKind::Recovered,
        ),
        Some(validate_predecessor_ordinal),
        "a foreign predecessor ordinal must not mutate the recovered retry owner"
    );
    assert_eq!(
        executor
            .preflight_recovered_apply_validate_retry_predecessor(
                dispatch_key,
                key,
                validate_predecessor_ordinal,
            )
            .expect("preflight the authenticated non-adjacent Validate predecessor"),
        Some(validate_predecessor_ordinal)
    );
    assert!(
        executor
            .release_recovered_apply_validate_retry_predecessor(
                dispatch_key,
                key,
                validate_predecessor_ordinal,
            )
            .expect("release the authenticated recovered Validate predecessor")
    );
    assert_eq!(
        bound_validate_retry_ordinal_for_cleanup(
            &executor,
            key,
            BoundValidateRetryAuthorityKind::Recovered,
        ),
        None
    );
    assert!(
        !executor
            .release_recovered_apply_validate_retry_predecessor(
                dispatch_key,
                key,
                validate_predecessor_ordinal,
            )
            .expect("an already-inert recovered predecessor is idempotent")
    );
}

#[test]
fn missing_replay_validate_rejects_ordinary_phase_none_binding() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (key, _) = install_exact_recovered_body_without_lifecycle_replay(&mut executor, &fixture);
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ownership = recovered_validate_retry_ownership(&fixture, &effect, None, 9_106);
    executor.protected_lock = Some(key);
    executor.runtime.locked_body = Some(key);
    executor.runtime.durable_body_authority_certificate = Some(prepare);
    assert_missing_replay_validate_fails_closed_without_body_mutation(
        &mut executor,
        &mut services,
        effect,
        ownership,
        "omitted its mandatory lifecycle replay owner",
    );
}

#[test]
#[allow(clippy::too_many_lines)]
fn decision_cleanup_defers_live_validate_authority_retirement_until_exact_resolution() {
    let fixture = Fixture::new();
    let (foreign_subject, _) = distinct_body(&fixture);
    let selected_key = (fixture.manifest.round, fixture.manifest.subject);
    let foreign_key = (fixture.manifest.round, foreign_subject);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let kinds = [
        BoundValidateRetryAuthorityKind::Live,
        BoundValidateRetryAuthorityKind::Recovered,
        BoundValidateRetryAuthorityKind::Published,
    ];

    for (kind_index, kind) in kinds.into_iter().enumerate() {
        for authority_is_selected in [false, true] {
            for drain_decision_body in [false, true] {
                let mut executor = fixture.executor(EffectQueueConfig::default());
                let mut services = fixture.services();
                let key = if authority_is_selected {
                    selected_key
                } else {
                    foreign_key
                };
                let lifecycle_ordinal = 40_000_u128
                    + u128::try_from(kind_index).expect("kind index fits u128") * 100
                    + if authority_is_selected { 10 } else { 0 }
                    + if drain_decision_body { 1 } else { 0 };
                install_bound_validate_retry_authority_for_cleanup(
                    &mut executor,
                    &fixture,
                    key,
                    kind,
                    lifecycle_ordinal,
                );

                executor
                    .reconcile_decision_work(decision, drain_decision_body, &mut services)
                    .unwrap_or_else(|error| {
                        panic!(
                            "{kind:?} Decision cleanup failed for selected={authority_is_selected}, drain={drain_decision_body}: {error:?}"
                        )
                    });
                assert_eq!(
                    bound_validate_retry_ordinal_for_cleanup(&executor, key, kind),
                    Some(lifecycle_ordinal),
                    "{kind:?} cleanup retired a live row for selected={authority_is_selected}, drain={drain_decision_body}",
                );
                assert!(!executor.durable_validate_retry_seals_are_finalization_inert());

                let wrong_ordinal = lifecycle_ordinal + 1;
                assert!(matches!(
                    executor.release_validate_retry_lifecycle_ordinal(key, wrong_ordinal),
                    Err(EffectExecutorError::Contract(reason)) if reason.contains("ordinal")
                ));
                assert_eq!(
                    bound_validate_retry_ordinal_for_cleanup(&executor, key, kind),
                    Some(lifecycle_ordinal),
                    "{kind:?} wrong-ordinal release mutated the live authority",
                );

                assert_eq!(
                    executor
                        .release_validate_retry_lifecycle_ordinal(key, lifecycle_ordinal)
                        .expect("release exact cleanup Validate authority"),
                    true,
                );
                let retains_selected_tombstone = authority_is_selected && !drain_decision_body;
                assert_eq!(
                    bound_validate_retry_ordinal_for_cleanup(&executor, key, kind),
                    None,
                    "{kind:?} exact release retained the live ordinal",
                );
                let authority_still_present = match kind {
                    BoundValidateRetryAuthorityKind::Live
                    | BoundValidateRetryAuthorityKind::Recovered => {
                        executor.durable_validate_retry_seals.contains_key(&key)
                    }
                    BoundValidateRetryAuthorityKind::Published => executor
                        .published_lifecycle_validate_retry_markers
                        .contains_key(&key),
                };
                assert_eq!(authority_still_present, retains_selected_tombstone);
                assert!(executor.durable_validate_retry_seals_are_finalization_inert());
            }
        }
    }
}

#[test]
fn live_validate_successor_refines_only_the_same_attested_row() {
    let fixture = Fixture::new();
    let round = fixture.manifest.round;
    let subject = fixture.manifest.subject;
    let ordinal = 41_u128;
    let key = |causal_root, first_ordinal, ordinal, slot_index, digest| {
        LifecycleValidateDispatchKeyV1::for_test(
            &fixture.context,
            LifecycleDigest::new([causal_root; 32]),
            first_ordinal,
            ordinal,
            slot_index,
            LifecycleDigest::new([digest; 32]),
        )
        .expect("construct an exact Validate dispatch key")
    };
    let wake_key = key(0x41, ordinal, ordinal, 0, 0x51);
    let validated_key = key(0x41, ordinal, ordinal, 0, 0x52);
    let rejected_key = key(0x41, ordinal, ordinal, 0, 0x53);
    let true_owner = LiveLifecycleValidateSuccessorOwnerV1 {
        dispatch_key: wake_key,
        round,
        subject,
        apply_is_authorized: true,
    };
    let validated = LiveLifecycleValidateSuccessorOwnerV1 {
        dispatch_key: validated_key,
        round,
        subject,
        apply_is_authorized: true,
    };
    let rejected = LiveLifecycleValidateSuccessorOwnerV1 {
        dispatch_key: rejected_key,
        round,
        subject,
        apply_is_authorized: false,
    };
    assert!(!true_owner.can_refine_to(&true_owner));
    assert!(true_owner.can_refine_to(&validated));
    assert!(true_owner.can_refine_to(&rejected));
    assert!(!rejected.can_refine_to(&validated));
    assert!(!rejected.can_refine_to(&LiveLifecycleValidateSuccessorOwnerV1 {
        dispatch_key: key(0x41, ordinal, ordinal, 0, 0x54),
        round,
        subject,
        apply_is_authorized: false,
    }));

    let mut foreign_round = round;
    foreign_round.view = foreign_round.view.saturating_add(1);
    let mut foreign_subject = subject;
    foreign_subject.payload_hash = Hash::new(b"foreign Validate successor subject");
    let logical_substitutions = [
        LiveLifecycleValidateSuccessorOwnerV1 {
            dispatch_key: key(0x42, ordinal, ordinal, 0, 0x52),
            round,
            subject,
            apply_is_authorized: true,
        },
        LiveLifecycleValidateSuccessorOwnerV1 {
            dispatch_key: key(0x41, ordinal, ordinal + 1, 0, 0x52),
            round,
            subject,
            apply_is_authorized: true,
        },
        LiveLifecycleValidateSuccessorOwnerV1 {
            dispatch_key: key(0x41, ordinal, ordinal, 1, 0x52),
            round,
            subject,
            apply_is_authorized: true,
        },
        LiveLifecycleValidateSuccessorOwnerV1 {
            dispatch_key: validated_key,
            round: foreign_round,
            subject,
            apply_is_authorized: true,
        },
        LiveLifecycleValidateSuccessorOwnerV1 {
            dispatch_key: validated_key,
            round,
            subject: foreign_subject,
            apply_is_authorized: true,
        },
    ];
    assert!(
        logical_substitutions
            .iter()
            .all(|candidate| !true_owner.can_refine_to(candidate))
    );
}

#[test]
fn unwoken_validate_sidecar_cancellation_retires_only_its_exact_retry_authority() {
    let fixture = Fixture::new();
    let ordinal = 42_u128;
    let cancellation_key = || {
        LifecycleValidateDispatchKeyV1::for_test(
            &fixture.context,
            LifecycleDigest::new([0x42; 32]),
            ordinal,
            ordinal,
            0,
            LifecycleDigest::new([0x52; 32]),
        )
        .expect("construct the cancelled Validate dispatch key")
    };

    let mut executor = fixture.executor(EffectQueueConfig::default());
    let services = fixture.services();
    let (key, _, _) =
        install_recovered_validate_retry_seal(&mut executor, &fixture, tag(0), ordinal);
    assert_eq!(
        executor.validate_retry_lifecycle_ordinal_for_test(key),
        Some(Some(ordinal))
    );
    assert_eq!(
        executor.pending_kura_apply_owner_flags_for_test(),
        (false, false, false, false, false),
        "an unwoken sidecar wait must not own a preliminary successor"
    );
    let cancellation =
        CancelledLifecycleValidateSidecarV1::for_test(cancellation_key(), key.0, key.1)
            .expect("seal the exact unwoken sidecar cancellation");
    executor
        .cancel_unwoken_lifecycle_validate_retry(cancellation)
        .expect("retire the exact cancelled sidecar retry authority");
    assert_eq!(
        executor.validate_retry_lifecycle_ordinal_for_test(key),
        None
    );
    assert_eq!(
        executor.pending_kura_apply_owner_flags_for_test(),
        (false, false, false, false, false)
    );
    assert!(!executor.output_guard.restart_required());
    assert!(services.apply_tasks.is_empty());

    let mut mismatched = fixture.executor(EffectQueueConfig::default());
    let mismatch_services = fixture.services();
    let (mismatch_key, _, _) =
        install_recovered_validate_retry_seal(&mut mismatched, &fixture, tag(0), ordinal);
    let mut foreign_subject = mismatch_key.1;
    foreign_subject.payload_hash = Hash::new(b"foreign unwoken sidecar cancellation");
    let foreign = CancelledLifecycleValidateSidecarV1::for_test(
        cancellation_key(),
        mismatch_key.0,
        foreign_subject,
    )
    .expect("seal an exact-shape foreign cancellation");
    let before = mismatched.body_ownership_projection();
    assert!(matches!(
        mismatched.cancel_unwoken_lifecycle_validate_retry(foreign),
        Err(EffectExecutorError::Contract(reason))
            if reason == "cancelled unwoken Validate changed its exact retry authority"
    ));
    assert_eq!(mismatched.body_ownership_projection(), before);
    assert_eq!(
        mismatched.validate_retry_lifecycle_ordinal_for_test(mismatch_key),
        Some(Some(ordinal)),
        "a substituted subject must leave the retry authority live"
    );
    assert!(!mismatched.output_guard.restart_required());
    assert!(mismatch_services.apply_tasks.is_empty());

    mismatched.live_lifecycle_validate_successor = Some(LiveLifecycleValidateSuccessorOwnerV1 {
        dispatch_key: cancellation_key(),
        round: mismatch_key.0,
        subject: mismatch_key.1,
        apply_is_authorized: true,
    });
    let phase_mismatch = CancelledLifecycleValidateSidecarV1::for_test(
        cancellation_key(),
        mismatch_key.0,
        mismatch_key.1,
    )
    .expect("seal the phase-mismatched cancellation");
    assert!(matches!(
        mismatched.cancel_unwoken_lifecycle_validate_retry(phase_mismatch),
        Err(EffectExecutorError::Contract(reason))
            if reason == "cancelled unwoken Validate changed its exact retry authority"
    ));
    assert_eq!(
        mismatched.validate_retry_lifecycle_ordinal_for_test(mismatch_key),
        Some(Some(ordinal)),
        "an already-published successor must not enter the pre-wake cleanup path"
    );
    assert_eq!(
        mismatched.pending_kura_apply_owner_flags_for_test(),
        (false, true, false, false, false)
    );
    assert!(mismatch_services.apply_tasks.is_empty());
}

#[test]
#[allow(clippy::too_many_lines)]
fn recovered_validate_retry_frontier_is_monotonic_and_keeps_its_physical_owner() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let key;
    let initial_effect;
    (key, initial_effect, _) =
        install_recovered_validate_retry_seal(&mut executor, &fixture, tag(0), 9_030);
    let DurableValidateRetrySealV1::Recovered {
        owner: initial_owner,
        frontier: initial_frontier,
        ..
    } = &executor.durable_validate_retry_seals[&key]
    else {
        panic!("cold installation must retain Recovered lineage")
    };
    let initial_owner = Arc::clone(initial_owner);
    assert_eq!(initial_frontier.phase_for_test(), None);

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let prepare_commitment = prepare.execution_commitment;
    let prepare_effect = AdapterEffect::ValidateBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let prepare_ownership =
        recovered_validate_retry_ownership(&fixture, &prepare_effect, Some(prepare.clone()), 9_031);
    let prepare_origin = prepare_ownership.owner().causal_origin().clone();
    executor
        .retain_effect_batch(vec![prepare_effect.clone()], vec![prepare_ownership])
        .expect("None-to-Prepare retry advances only the inert frontier");
    let DurableValidateRetrySealV1::Recovered {
        owner, frontier, ..
    } = &executor.durable_validate_retry_seals[&key]
    else {
        panic!("authority refinement changed Recovered lineage")
    };
    assert!(Arc::ptr_eq(owner, &initial_owner));
    assert_eq!(frontier.phase_for_test(), Some(wire::GlobalPhase::Prepare));
    assert_eq!(
        frontier.commitment_ceiling_for_test(),
        Some(prepare_commitment)
    );
    assert_recovered_validate_retry_stutter_is_inert(&executor);

    let same_ownership =
        recovered_validate_retry_ownership(&fixture, &prepare_effect, Some(prepare.clone()), 9_036);
    assert_ne!(
        same_ownership.owner().causal_origin(),
        &prepare_origin,
        "the Same retry must exercise a separately authenticated causal root"
    );
    executor
        .retain_effect_batch(vec![prepare_effect], vec![same_ownership])
        .expect("same authority from a distinct causal root remains an inert stutter");
    let DurableValidateRetrySealV1::Recovered {
        owner, frontier, ..
    } = &executor.durable_validate_retry_seals[&key]
    else {
        panic!("same retry changed Recovered lineage")
    };
    assert!(Arc::ptr_eq(owner, &initial_owner));
    assert_eq!(frontier.phase_for_test(), Some(wire::GlobalPhase::Prepare));
    assert_recovered_validate_retry_stutter_is_inert(&executor);

    let stale_effect = AdapterEffect::ValidateBody {
        tag: tag(2),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let stale_ownership = recovered_validate_retry_ownership(&fixture, &stale_effect, None, 9_032);
    executor
        .retain_effect_batch(vec![stale_effect], vec![stale_ownership])
        .expect("stale weaker retry stutters without downgrading authority");
    let DurableValidateRetrySealV1::Recovered {
        owner, frontier, ..
    } = &executor.durable_validate_retry_seals[&key]
    else {
        panic!("stale retry changed Recovered lineage")
    };
    assert!(Arc::ptr_eq(owner, &initial_owner));
    assert_eq!(frontier.phase_for_test(), Some(wire::GlobalPhase::Prepare));
    assert_recovered_validate_retry_stutter_is_inert(&executor);

    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let commit_effect = AdapterEffect::ValidateBody {
        tag: tag(3),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let commit_ownership =
        recovered_validate_retry_ownership(&fixture, &commit_effect, Some(commit), 9_033);
    executor
        .retain_effect_batch(vec![commit_effect], vec![commit_ownership])
        .expect("Prepare-to-Commit retry advances the same inert frontier");
    let accepted = executor.durable_validate_retry_seals[&key].clone();
    let DurableValidateRetrySealV1::Recovered {
        owner, frontier, ..
    } = &accepted
    else {
        panic!("Commit refinement changed Recovered lineage")
    };
    assert!(Arc::ptr_eq(owner, &initial_owner));
    assert_eq!(frontier.phase_for_test(), Some(wire::GlobalPhase::Commit));
    assert_recovered_validate_retry_stutter_is_inert(&executor);

    let rollback_ownership =
        recovered_validate_retry_ownership(&fixture, &initial_effect, None, 9_034);
    assert!(matches!(
        executor.retain_effect_batch(vec![initial_effect], vec![rollback_ownership]),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("body, tag, or exact binding")
    ));
    assert_eq!(executor.durable_validate_retry_seals[&key], accepted);

    let mut conflicting = prepare;
    conflicting.execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"foreign recovered retry parent state"),
        Hash::new(b"foreign recovered retry post state"),
        Hash::new(b"foreign recovered retry writes"),
        1,
        Hash::new(b"foreign recovered retry block"),
    );
    let conflicting_effect = AdapterEffect::ValidateBody {
        tag: tag(4),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let conflicting_ownership =
        recovered_validate_retry_ownership(&fixture, &conflicting_effect, Some(conflicting), 9_035);
    assert!(
        executor
            .retain_effect_batch(vec![conflicting_effect], vec![conflicting_ownership])
            .is_err()
    );
    assert_eq!(executor.durable_validate_retry_seals[&key], accepted);
    assert_recovered_validate_retry_stutter_is_inert(&executor);
}

#[test]
#[allow(clippy::too_many_lines)]
fn recovered_validate_retry_later_marker_and_decision_joins_are_atomic() {
    let fixture = Fixture::new();
    let commitment = fixture.qc(wire::GlobalPhase::Commit).execution_commitment;
    let conflicting_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"late cold fact parent state"),
        Hash::new(b"late cold fact post state"),
        Hash::new(b"late cold fact writes"),
        1,
        Hash::new(b"late cold fact block"),
    );

    let mut marker_executor = fixture.executor(EffectQueueConfig::default());
    let (key, _, durable) =
        install_recovered_validate_retry_seal(&mut marker_executor, &fixture, tag(0), 9_040);
    let conflicting_prepare = {
        let mut certificate = fixture.qc(wire::GlobalPhase::Prepare);
        certificate.execution_commitment = conflicting_commitment;
        certificate
    };
    let effect = AdapterEffect::ValidateBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ownership =
        recovered_validate_retry_ownership(&fixture, &effect, Some(conflicting_prepare), 9_041);
    marker_executor
        .retain_effect_batch(vec![effect], vec![ownership])
        .expect("first authenticated commitment latches the recovered frontier");
    let before_marker = marker_executor.durable_validate_retry_seals[&key].clone();
    let marker = ValidatedBodyReceipt::for_test_with_commitment(durable.clone(), commitment);
    assert!(matches!(
        marker_executor.record_lifecycle_validated_body(
            ReadyValidatedExecutorCatalogAuthorityV1::for_test(marker.clone())
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("durable commitment")
    ));
    assert!(!marker_executor.validated_bodies.contains_key(&key));
    assert_eq!(
        marker_executor.durable_validate_retry_seals[&key],
        before_marker
    );

    let mut decision_executor = fixture.executor(EffectQueueConfig::default());
    let (decision_key, _, _) =
        install_recovered_validate_retry_seal(&mut decision_executor, &fixture, tag(0), 9_042);
    let conflicting_effect = AdapterEffect::ValidateBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let conflicting_ownership = recovered_validate_retry_ownership(
        &fixture,
        &conflicting_effect,
        Some({
            let mut certificate = fixture.qc(wire::GlobalPhase::Prepare);
            certificate.execution_commitment = conflicting_commitment;
            certificate
        }),
        9_043,
    );
    decision_executor
        .retain_effect_batch(vec![conflicting_effect], vec![conflicting_ownership])
        .expect("latch conflicting frontier before Decision join");
    let before_decision = decision_executor.body_ownership_projection();
    let before_decision_seal =
        decision_executor.durable_validate_retry_seals[&decision_key].clone();
    let mut services = fixture.services();
    assert!(matches!(
        decision_executor.reconcile_decision_work(
            (
                fixture.manifest.round,
                fixture.manifest.round,
                fixture.manifest.subject,
                commitment,
            ),
            false,
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("durable commitment")
    ));
    assert_eq!(
        decision_executor.body_ownership_projection(),
        before_decision
    );
    assert_eq!(
        decision_executor.durable_validate_retry_seals[&decision_key],
        before_decision_seal
    );
    assert!(decision_executor.protected_decision.is_none());
    assert!(services.operation_calls.is_empty());
    assert_eq!(services.retired_all_outbound, 0);
    assert_eq!(services.retired_candidate_work, 0);
    assert!(services.sign_tasks.is_empty());
    assert!(services.fetch_tasks.is_empty());
    assert!(services.store_tasks.is_empty());
    assert!(services.apply_tasks.is_empty());

    let mut accepted_executor = fixture.executor(EffectQueueConfig::default());
    let (accepted_key, _, accepted_durable) =
        install_recovered_validate_retry_seal(&mut accepted_executor, &fixture, tag(0), 9_044);
    let exact_marker = ValidatedBodyReceipt::for_test_with_commitment(accepted_durable, commitment);
    accepted_executor
        .record_lifecycle_validated_body(ReadyValidatedExecutorCatalogAuthorityV1::for_test(
            exact_marker,
        ))
        .expect("exact later marker latches the empty recovered ceiling");
    let exact_commit = fixture.qc(wire::GlobalPhase::Commit);
    let exact_effect = AdapterEffect::ValidateBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let exact_ownership =
        recovered_validate_retry_ownership(&fixture, &exact_effect, Some(exact_commit), 9_045);
    accepted_executor
        .retain_effect_batch(vec![exact_effect], vec![exact_ownership])
        .expect("Commit matching the later marker remains an inert stutter");
    let mut accepted_services = fixture.services();
    accepted_executor
        .reconcile_decision_work(
            (
                fixture.manifest.round,
                fixture.manifest.round,
                fixture.manifest.subject,
                commitment,
            ),
            false,
            &mut accepted_services,
        )
        .expect("Decision matching the marker-bound ceiling commits atomically");
    assert_eq!(accepted_services.retired_all_outbound, 1);
    assert_eq!(accepted_services.retired_candidate_work, 1);
    assert_eq!(
        accepted_services.operation_calls.get("retire-all-outbound"),
        Some(&1)
    );
    assert_eq!(
        accepted_services
            .operation_calls
            .get("retire-candidate-work"),
        Some(&1)
    );
    let DurableValidateRetrySealV1::Recovered { frontier, .. } =
        &accepted_executor.durable_validate_retry_seals[&accepted_key]
    else {
        panic!("exact later facts changed Recovered lineage")
    };
    assert_eq!(frontier.commitment_ceiling_for_test(), Some(commitment));
    assert_eq!(frontier.phase_for_test(), Some(wire::GlobalPhase::Commit));
    assert!(accepted_executor.protected_decision.is_some());
    assert_recovered_validate_retry_stutter_is_inert(&accepted_executor);

    let mut decision_then_marker_executor = fixture.executor(EffectQueueConfig::default());
    let (decision_then_marker_key, _, decision_then_marker_durable) =
        install_recovered_validate_retry_seal(
            &mut decision_then_marker_executor,
            &fixture,
            tag(0),
            9_046,
        );
    let mut decision_then_marker_services = fixture.services();
    decision_then_marker_executor
        .reconcile_decision_work(
            (
                fixture.manifest.round,
                fixture.manifest.round,
                fixture.manifest.subject,
                commitment,
            ),
            false,
            &mut decision_then_marker_services,
        )
        .expect("exact Decision latches the empty recovered ceiling");
    assert_eq!(decision_then_marker_services.retired_all_outbound, 1);
    assert_eq!(decision_then_marker_services.retired_candidate_work, 1);
    assert_eq!(
        decision_then_marker_services
            .operation_calls
            .get("retire-all-outbound"),
        Some(&1)
    );
    assert_eq!(
        decision_then_marker_services
            .operation_calls
            .get("retire-candidate-work"),
        Some(&1)
    );
    decision_then_marker_executor
        .record_lifecycle_validated_body(ReadyValidatedExecutorCatalogAuthorityV1::for_test(
            ValidatedBodyReceipt::for_test_with_commitment(
                decision_then_marker_durable,
                commitment,
            ),
        ))
        .expect("marker matching the Decision-bound ceiling commits atomically");
    let DurableValidateRetrySealV1::Recovered { frontier, .. } =
        &decision_then_marker_executor.durable_validate_retry_seals[&decision_then_marker_key]
    else {
        panic!("Decision-then-marker join changed Recovered lineage")
    };
    assert_eq!(frontier.commitment_ceiling_for_test(), Some(commitment));
    assert!(
        decision_then_marker_executor
            .validated_bodies
            .contains_key(&decision_then_marker_key)
    );
    assert_recovered_validate_retry_stutter_is_inert(&decision_then_marker_executor);
}

#[test]
#[allow(clippy::too_many_lines)]
fn periodic_proposal_fetch_stutters_only_against_its_exact_advanced_replay_family() {
    let fixture = Fixture::new();
    let replay_tag = tag(0);
    let key = (fixture.manifest.round, fixture.manifest.subject);

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (fetch, ownership, replay) =
            prepared_remote_proposal_fetch_replay(&fixture, replay_tag, 9_014);
        let work_id = EffectWorkId::for_test(9_014);
        let task = BodyFetchTask {
            id: work_id,
            tag: replay_tag,
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
            manifest: Some(fixture.manifest.clone()),
            sources: Vec::new(),
            certified_request: None,
            ownership: ownership.clone(),
        };
        assert!(
            !executor
                .bind_body_pipeline_owner_hash(
                    replay_tag,
                    key,
                    Some(HashOf::new(&fixture.manifest)),
                )
                .expect("install the exact in-flight Proposal body owner")
        );
        assert!(
            executor
                .pending_fetches
                .insert(
                    work_id,
                    PendingFetch {
                        task: task.clone(),
                        request_hash: None,
                    },
                )
                .is_none()
        );
        assert!(
            executor
                .remote_proposal_replay
                .insert(key, RemoteProposalReplayStageV1::Fetch { work_id, replay },)
                .is_none()
        );
        executor
            .retain_effect_batch(vec![fetch], vec![ownership])
            .expect("retain the exact in-flight Proposal Fetch rediscovery");
        assert_eq!(
            executor
                .drain_retained_effect_batch(&mut services, true)
                .expect("the exact in-flight Proposal Fetch is redispatched"),
            1
        );
        assert_eq!(services.fetch_tasks, vec![task]);
        assert!(matches!(
            executor.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::Fetch {
                work_id: retained,
                ..
            }) if *retained == work_id
        ));
    }

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (fetch, ownership, replay) =
            prepared_remote_proposal_fetch_replay(&fixture, replay_tag, 9_015);
        assert!(
            executor
                .remote_proposal_replay
                .insert(key, RemoteProposalReplayStageV1::BodyAvailable(replay))
                .is_none()
        );
        executor
            .retain_effect_batch(vec![fetch], vec![ownership])
            .expect("retain the Proposal Fetch rediscovered after BodyAvailable");
        assert_eq!(
            executor
                .drain_retained_effect_batch(&mut services, true)
                .expect("BodyAvailable stutters the exact Fetch rediscovery"),
            1
        );
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::BodyAvailable(_))
        ));
    }

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (fetch, ownership, _store, _store_ownership, replay) =
            prepared_remote_proposal_store_replay(&fixture, replay_tag, 9_016);
        assert!(
            executor
                .remote_proposal_replay
                .insert(key, RemoteProposalReplayStageV1::StoreAdmission(replay))
                .is_none()
        );
        executor
            .retain_effect_batch(vec![fetch], vec![ownership])
            .expect("retain the Proposal Fetch rediscovered during Store admission");
        assert!(matches!(
            executor.drain_retained_effect_batch(&mut services, true),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("transient Store admission")
        ));
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::StoreAdmission(_))
        ));
    }

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (fetch, ownership, _store, _store_ownership, replay) =
            prepared_remote_proposal_store_replay(&fixture, replay_tag, 9_017);
        let work_id = EffectWorkId::for_test(9_017);
        assert!(
            executor
                .remote_proposal_replay
                .insert(key, RemoteProposalReplayStageV1::Store { work_id, replay },)
                .is_none()
        );
        executor
            .retain_effect_batch(vec![fetch], vec![ownership])
            .expect("retain the Proposal Fetch rediscovered during Store I/O");
        assert_eq!(
            executor
                .drain_retained_effect_batch(&mut services, true)
                .expect("Store I/O stutters the exact Fetch rediscovery"),
            1
        );
        assert!(services.fetch_tasks.is_empty());
        assert!(matches!(
            executor.remote_proposal_replay.get(&key),
            Some(RemoteProposalReplayStageV1::Store {
                work_id: retained,
                ..
            }) if *retained == work_id
        ));
    }

    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_stored_remote_proposal_replay(&mut executor, &fixture, replay_tag, 9_018);
    let (fetch, rediscovery, _replay) =
        prepared_remote_proposal_fetch_replay(&fixture, replay_tag, 9_019);
    executor
        .retain_effect_batch(vec![fetch], vec![rediscovery])
        .expect("retain one periodic Proposal Fetch rediscovery");
    assert_eq!(
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("the exact Stored Proposal family stutters the stale Fetch"),
        1
    );
    assert!(services.fetch_tasks.is_empty());
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));

    let foreign_manifest = deliberately_conflicting_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        b"foreign periodic Proposal body",
    );
    let foreign_fetch = AdapterEffect::FetchBody {
        tag: replay_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(foreign_manifest),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let foreign_owner = bound_test_effect_ownership(&foreign_fetch, replay_tag, 9_020);
    executor
        .retain_effect_batch(vec![foreign_fetch], vec![foreign_owner])
        .expect("retain the conflicting periodic Fetch for exact replay rejection");
    assert!(matches!(
        executor.drain_retained_effect_batch(&mut services, true),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("changed its retained authenticated replay origin")
    ));
    assert!(services.fetch_tasks.is_empty());
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));
}

#[test]
fn authenticated_ordinary_proposal_stutters_behind_certified_fetch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let certified = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare.clone()),
    };
    executor
        .consume_effects(vec![certified], &mut services)
        .expect("admit the stronger certified Fetch first");
    let incumbent = services
        .fetch_tasks
        .first()
        .expect("certified Fetch owns one service task")
        .clone();
    let request = incumbent
        .certified_request()
        .expect("incumbent retains certified request")
        .clone();

    let ordinary = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let mut ordinary_ownership = bound_test_effect_ownership(&ordinary, tag(0), 9_017);
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal(&fixture).payload else {
        unreachable!("Proposal fixture has one Proposal payload")
    };
    assert!(
        ordinary_ownership.bind_authenticated_remote_proposal_replay_for_test(proposal, &ordinary)
    );
    assert!(
        ordinary_ownership
            .exact_remote_proposal_fetch_replay(&ordinary)
            .is_some()
    );
    executor.runtime.exact_effect_ownership = Some((ordinary.clone(), ordinary_ownership));

    assert_eq!(
        executor
            .consume_effects(vec![ordinary], &mut services)
            .expect("the stale authenticated Proposal terminates behind certified acquisition"),
        0
    );
    assert_eq!(services.fetch_tasks, vec![incumbent.clone()]);
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.pending_fetches[&incumbent.id()].task, incumbent);
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert_eq!(
        executor.pending_fetches[&incumbent.id()]
            .task
            .certified_request(),
        Some(&request)
    );
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
}

#[test]
#[allow(clippy::too_many_lines)]
fn durable_decision_preserves_stored_proposal_replay_for_commit_refined_validate() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let tag = tag(0);
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let store_ownership =
        install_stored_remote_proposal_replay(&mut executor, &fixture, tag, 9_015);

    let validate_effect = AdapterEffect::ValidateBody {
        tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ordinary_validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("project ordinary Proposal Validate owner");
    executor
        .retain_effect_batch(
            vec![validate_effect.clone()],
            vec![ordinary_validate_ownership],
        )
        .expect("retain ordinary Proposal Validate before Decision");
    executor
        .park_retained_effect_batch()
        .expect("park ordinary Proposal Validate behind the next runtime turn");

    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let commit_fetch = AdapterEffect::FetchBody {
        tag,
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit.clone()),
    };
    let commit_validate_ownership = bound_test_effect_ownership(&commit_fetch, tag, 9_016)
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry durable Commit authority into Validate");
    assert!(
        commit_validate_ownership
            .binds_durable_decision_authority(decision.0, decision.1, decision.2, decision.3,)
    );
    executor.runtime.decided_body = Some(decision);
    executor.runtime.durable_body_authority_certificate = Some(commit);
    executor.runtime.exact_effect_ownership =
        Some((validate_effect.clone(), commit_validate_ownership));

    assert_eq!(
        executor
            .consume_effects(vec![validate_effect], &mut services)
            .expect("Decision preserves and consumes the exact stored Proposal replay"),
        1
    );
    assert_eq!(executor.protected_decision, Some(decision));
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(
        executor
            .pending_durable_validate_admissions
            .contains_key(&key)
    );
    assert_eq!(
        executor
            .durable_validate_retry_seals
            .keys()
            .copied()
            .collect::<Vec<_>>(),
        vec![key]
    );
    assert!(executor.durable_validate_retry_seals_are_finalization_inert());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn enter_view_preserves_stored_proposal_replay_for_prepare_refined_validate() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let next_tag = tag(1);
    install_stored_remote_proposal_replay(&mut executor, &fixture, tag(0), 9_017);

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor.runtime.round_tag = Some(next_tag);
    executor.runtime.locked_body = Some(key);
    executor
        .install_view(
            next_tag,
            timeout,
            Some(prepare.clone()),
            None,
            &mut services,
        )
        .expect("EnterView preserves the protected fsynced Proposal Store lineage");
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));

    let validate_effect = AdapterEffect::ValidateBody {
        tag: next_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let prepare_fetch = AdapterEffect::FetchBody {
        tag: next_tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare.clone()),
    };
    let prepare_validate_ownership = bound_test_effect_ownership(&prepare_fetch, next_tag, 9_018)
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry Prepare authority into the protected Validate");
    executor.runtime.durable_body_authority_certificate = Some(prepare);
    executor
        .retain_effect_batch(vec![validate_effect], vec![prepare_validate_ownership])
        .expect("adopt the Stored Proposal root under Prepare authority");
    assert_eq!(
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("consume the replay-authorized protected Validate"),
        1
    );
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(
        executor
            .pending_durable_validate_admissions
            .contains_key(&key)
    );
    assert!(executor.durable_validate_retry_seals.contains_key(&key));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn enter_view_preserves_inflight_proposal_store_replay_through_late_completion() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let original_tag = tag(0);
    let next_tag = tag(1);
    let store_id = install_inflight_remote_proposal_store(
        &mut executor,
        &mut services,
        &fixture,
        original_tag,
        9_019,
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor.runtime.round_tag = Some(next_tag);
    executor.runtime.locked_body = Some(key);
    executor
        .install_view(
            next_tag,
            timeout,
            Some(prepare.clone()),
            None,
            &mut services,
        )
        .expect("EnterView detaches but preserves the protected in-flight Store");
    assert!(executor.pending_stores[&store_id].consumer.is_none());
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Store { work_id, .. })
            if *work_id == store_id
    ));

    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: next_tag,
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("current StoreBody adopts the detached protected Store");
    assert_eq!(
        executor
            .body_pipeline_owners
            .get(&key)
            .map(|owner| owner.tag),
        Some(next_tag)
    );
    assert_eq!(
        services.store_tasks.len(),
        1,
        "protected Store handoff must not duplicate physical I/O",
    );

    let completion = services.execute_store(store_id);
    assert_eq!(
        executor
            .complete_body_store(completion, &mut services)
            .expect("late Store completion advances the retained replay owner"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));
    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: next_tag,
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("current StoreBody adopts the durable old-view replay owner");
    assert_eq!(
        services.store_tasks.len(),
        1,
        "the durable retry must not duplicate Store I/O"
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyStored(completion_tag, round, subject, _))
            if *completion_tag == next_tag
                && *round == fixture.manifest.round
                && *subject == fixture.manifest.subject
    ));
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));

    let validate_effect = AdapterEffect::ValidateBody {
        tag: next_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let prepare_fetch = AdapterEffect::FetchBody {
        tag: next_tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare.clone()),
    };
    let prepare_validate_ownership = bound_test_effect_ownership(&prepare_fetch, next_tag, 9_020)
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry Prepare authority into the protected Validate");
    executor.runtime.durable_body_authority_certificate = Some(prepare);
    executor
        .retain_effect_batch(vec![validate_effect], vec![prepare_validate_ownership])
        .expect("adopt the late Stored Proposal root under Prepare authority");
    assert_eq!(
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("consume the late replay-authorized protected Validate"),
        1
    );
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(
        executor
            .pending_durable_validate_admissions
            .contains_key(&key)
    );
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn admitted_validate_retry_seal_coalesces_exact_authority_upgrade_without_replay_reuse() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let validate_effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let store_ownership =
        install_stored_remote_proposal_replay(&mut executor, &fixture, tag(0), 9_019);
    let ordinary_validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("project the ordinary Proposal Validate owner");
    executor
        .retain_effect_batch(
            vec![validate_effect.clone()],
            vec![ordinary_validate_ownership],
        )
        .expect("retain the first replay-authorized Validate");
    assert_eq!(
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("install the first durable Validate admission"),
        1
    );
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(
        executor
            .pending_durable_validate_admissions
            .remove(&key)
            .is_some(),
        "model the move-only owner transferring into the lifecycle registry"
    );
    executor
        .bind_validate_retry_lifecycle_ordinal(key, 9_021)
        .expect("bind the transferred Validate to its active registry row");

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let prepare_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare.clone()),
    };
    let prepare_validate_ownership = bound_test_effect_ownership(&prepare_fetch, tag(0), 9_020)
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry Prepare authority into the duplicate Validate");
    executor
        .retain_effect_batch(
            vec![validate_effect.clone()],
            vec![prepare_validate_ownership],
        )
        .expect("the exact authority upgrade stutters at its retained seal");
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.remote_proposal_replay.is_empty());
    let accepted_seal = executor.durable_validate_retry_seals[&key].clone();

    let mut conflicting = prepare;
    conflicting.execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"conflicting sealed Validate parent state"),
        Hash::new(b"conflicting sealed Validate post state"),
        Hash::new(b"conflicting sealed Validate ordinary writes"),
        1,
        Hash::new(b"conflicting sealed Validate executed block"),
    );
    let conflicting_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: conflicting.proposal_round,
        subject: conflicting.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &conflicting),
        certificate: Some(conflicting),
    };
    let conflicting_validate = bound_test_effect_ownership(&conflicting_fetch, tag(0), 9_021)
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry the conflicting Prepare authority into Validate");
    assert!(matches!(
        executor.retain_effect_batch(vec![validate_effect], vec![conflicting_validate]),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("body key or authority commitment")
    ));
    assert_eq!(executor.durable_validate_retry_seals[&key], accepted_seal);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn published_lifecycle_validate_marker_coalesces_timer_authority_upgrade() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()),)
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let initial_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare.clone()),
    };
    let initial_store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let initial_validate = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let initial_store_ownership = bound_test_effect_ownership(&initial_fetch, tag(0), 9_022)
        .rebind_as_inherited_adapter_effect(&initial_store)
        .expect("project the lifecycle-published Prepare Store owner");
    let initial_store_pending = initial_store_ownership
        .exact_pending_adapter_effect_binding(&initial_store)
        .expect("seal the lifecycle-published Store binding");
    let prepared_store = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the direct lifecycle Store marker catalog")
        .bind_store_successor(&initial_store, &initial_store_pending)
        .expect("bind the exact lifecycle-published Store successor");
    executor.commit_published_lifecycle_store_retry_marker(prepared_store);
    let initial_validate_ownership = initial_store_ownership
        .rebind_as_inherited_adapter_effect(&initial_validate)
        .expect("project the lifecycle-published Prepare Validate owner");
    let initial_pending = initial_validate_ownership
        .exact_pending_adapter_effect_binding(&initial_validate)
        .expect("seal the lifecycle-published Validate binding");
    let prepared = executor
        .prepare_published_lifecycle_validate_retry_marker(&durable)
        .expect("preflight the direct lifecycle marker catalog")
        .bind_validate_successor(&initial_validate, &initial_pending)
        .expect("bind the exact lifecycle-published Validate successor");
    executor.commit_published_lifecycle_validate_retry_marker(prepared, 9_022);

    let next_tag = tag(1);
    let retry_fetch = AdapterEffect::FetchBody {
        tag: next_tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let retry_store = AdapterEffect::StoreBody {
        tag: next_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let retry_store_ownership = bound_test_effect_ownership(&retry_fetch, next_tag, 9_023)
        .rebind_as_inherited_adapter_effect(&retry_store)
        .expect("carry Prepare authority into the later Store retry");
    assert_ne!(
        retry_store_ownership.owner(),
        initial_store_ownership.owner(),
        "the regression needs a fresh lifecycle owner"
    );
    let terminal_identity = initial_store_ownership
        .candidate_semantic_identity()
        .expect("the original Store has one candidate identity");
    assert_eq!(
        retry_store_ownership.candidate_semantic_identity(),
        Some(terminal_identity),
        "the later Store must collide with the queued terminal's exact candidate"
    );
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(terminal_identity, initial_store_ownership.clone())
            .is_none()
    );
    let queued_terminal = RuntimeCompletion::BodyStored(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        durable.clone(),
    );
    executor.runtime.completions.push(queued_terminal.clone());
    let accepted_marker = executor.published_lifecycle_validate_retry_markers[&key].clone();
    let terminal_owners_before = executor.runtime.terminal_body_candidate_owners.clone();
    let terminal_queries_before = executor.runtime.terminal_body_candidate_queries.clone();
    let terminal_commits_before = executor.runtime.terminal_body_candidate_commits;

    let conflicting_manifest = deliberately_conflicting_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        b"foreign direct-lifecycle Store receipt",
    );
    let conflicting_receipt = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&conflicting_manifest),
    );
    assert_eq!(
        executor
            .durable_bodies
            .insert(key, conflicting_receipt.clone()),
        Some(durable.clone())
    );
    assert!(matches!(
        executor.retain_effect_batch(
            vec![retry_store.clone()],
            vec![retry_store_ownership.clone()],
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("changed its durable body or regressed its tag")
    ));
    assert_eq!(
        executor.durable_bodies.insert(key, durable.clone()),
        Some(conflicting_receipt)
    );
    assert_eq!(
        executor.published_lifecycle_validate_retry_markers[&key],
        accepted_marker
    );
    assert!(executor.pending_stores.is_empty());
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners,
        terminal_owners_before
    );
    assert_eq!(executor.runtime.completions, vec![queued_terminal.clone()]);
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries,
        terminal_queries_before
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before
    );
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());

    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let stronger_fetch = AdapterEffect::FetchBody {
        tag: next_tag,
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit.clone()),
    };
    let stronger_store_ownership = bound_test_effect_ownership(&stronger_fetch, next_tag, 9_024)
        .rebind_as_inherited_adapter_effect(&retry_store)
        .expect("carry Commit authority into the stronger Store retry");
    assert!(matches!(
        executor.retain_effect_batch(
            vec![retry_store.clone()],
            vec![stronger_store_ownership],
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("outran its published Validate authority")
    ));
    assert_eq!(
        executor.published_lifecycle_validate_retry_markers[&key],
        accepted_marker
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners,
        terminal_owners_before
    );
    assert_eq!(executor.runtime.completions, vec![queued_terminal.clone()]);
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries,
        terminal_queries_before
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before
    );

    executor
        .retain_effect_batch(vec![retry_store], vec![retry_store_ownership])
        .expect("the published Validate marker stutters the exact later Store retry");
    assert!(executor.retained_effect_batch.is_none());
    assert_eq!(
        executor.published_lifecycle_validate_retry_markers[&key],
        accepted_marker
    );
    assert!(executor.pending_stores.is_empty());
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners,
        terminal_owners_before
    );
    assert_eq!(executor.runtime.completions, vec![queued_terminal]);
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries, terminal_queries_before,
        "the marker must stutter before runtime terminal-owner comparison"
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before
    );
    let mut services = fixture.services();
    assert_eq!(
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("no later Store dispatch remains"),
        0
    );
    assert!(services.store_tasks.is_empty());

    let commit_fetch = AdapterEffect::FetchBody {
        tag: next_tag,
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit),
    };
    let timer_validate = AdapterEffect::ValidateBody {
        tag: next_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let timer_ownership = bound_test_effect_ownership(&commit_fetch, next_tag, 9_025)
        .rebind_as_inherited_adapter_effect(&timer_validate)
        .expect("carry Commit authority into the periodic Validate retry");
    executor
        .retain_effect_batch(vec![timer_validate.clone()], vec![timer_ownership])
        .expect("the periodic Validate stutters at its direct lifecycle marker");

    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert!(executor.durable_validate_retry_seals.is_empty());
    let marker = &executor.published_lifecycle_validate_retry_markers[&key];
    assert_eq!(marker.latest_effect, timer_validate);
    assert_eq!(
        marker.latest_statement.phase(),
        Some(wire::GlobalPhase::Commit)
    );
    assert_eq!(executor.pending_work(), 0);
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
}

#[test]
fn terminal_published_validate_retry_requires_live_wal_apply_admission() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = services
        .body_store
        .as_mut()
        .expect("body store service")
        .store(fixture.manifest.clone(), fixture.body.clone())
        .expect("persist the exact pre-view-change body");
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()),)
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let old_tag = tag(0);
    let initial_fetch = AdapterEffect::FetchBody {
        tag: old_tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let initial_store = AdapterEffect::StoreBody {
        tag: old_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let initial_validate = AdapterEffect::ValidateBody {
        tag: old_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let initial_store_ownership = bound_test_effect_ownership(&initial_fetch, old_tag, 9_026)
        .rebind_as_inherited_adapter_effect(&initial_store)
        .expect("project the lifecycle-published Store owner");
    let initial_store_pending = initial_store_ownership
        .exact_pending_adapter_effect_binding(&initial_store)
        .expect("seal the lifecycle-published Store binding");
    let prepared_store = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the direct lifecycle Store marker")
        .bind_store_successor(&initial_store, &initial_store_pending)
        .expect("bind the exact lifecycle-published Store successor");
    executor.commit_published_lifecycle_store_retry_marker(prepared_store);
    let initial_validate_ownership = initial_store_ownership
        .rebind_as_inherited_adapter_effect(&initial_validate)
        .expect("project the lifecycle-published Validate owner");
    let initial_validate_pending = initial_validate_ownership
        .exact_pending_adapter_effect_binding(&initial_validate)
        .expect("seal the lifecycle-published Validate binding");
    let prepared_validate = executor
        .prepare_published_lifecycle_validate_retry_marker(&durable)
        .expect("preflight the direct lifecycle Validate marker")
        .bind_validate_successor(&initial_validate, &initial_validate_pending)
        .expect("bind the exact lifecycle-published Validate successor");
    executor.commit_published_lifecycle_validate_retry_marker(prepared_validate, 9_026);

    let validated =
        validate_durable_body_fixture(&mut services, &fixture.manifest, durable.clone());
    executor
        .record_lifecycle_validated_body(ReadyValidatedExecutorCatalogAuthorityV1::for_test(
            validated.clone(),
        ))
        .expect("cache the physically completed validation receipt");
    assert!(
        executor
            .release_validate_retry_lifecycle_ordinal(key, 9_026)
            .expect("release the terminal Validate row's exact retry ordinal"),
        "the terminal Validate row must release its inert marker"
    );

    // Model the executor projection after EnterView won the publication race:
    // the old Ready row sealed ValidateNoSuccessor, leaving only its inert
    // marker and independently fsynced validated receipt. There is no live
    // validation admission or service work left to satisfy a later retry.
    let current_tag = tag(1);
    executor.runtime.round_tag = Some(current_tag);
    executor.reconciled_tag = Some(current_tag);
    assert_eq!(
        executor.published_lifecycle_validate_retry_markers[&key].published_effect,
        initial_validate
    );
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert_eq!(executor.pending_work(), 0);
    let terminal_marker = executor.published_lifecycle_validate_retry_markers[&key].clone();

    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let current_fetch = AdapterEffect::FetchBody {
        tag: current_tag,
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit.clone()),
    };
    let current_validate = AdapterEffect::ValidateBody {
        tag: current_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let current_validate_ownership =
        bound_test_effect_ownership(&current_fetch, current_tag, 9_027)
            .rebind_as_inherited_adapter_effect(&current_validate)
            .expect("carry current-view Commit authority into Validate");
    assert!(
        current_validate_ownership
            .binds_durable_decision_authority(decision.0, decision.1, decision.2, decision.3,)
    );
    executor.runtime.decided_body = Some(decision);
    executor.runtime.durable_body_authority_certificate = Some(commit);
    executor.runtime.live_clocks_armed = true;
    executor.runtime.exact_effect_ownership =
        Some((current_validate.clone(), current_validate_ownership.clone()));

    assert_eq!(
        executor
            .consume_effects(vec![current_validate.clone()], &mut services)
            .expect("consume the released marker through exact live Decision authority"),
        1,
        "the Commit-owned Validate must consume its retained FIFO occurrence",
    );
    assert_eq!(executor.protected_decision, Some(decision));
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.parked_effect_batch.is_none());
    assert!(executor
        .published_lifecycle_validate_retry_markers
        .contains_key(&key));
    assert!(executor.pending_released_lifecycle_validate_apply.is_some());
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert!(executor.durable_validate_retry_seals.is_empty());
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));
    assert!(executor.pending_applications.is_empty());
    assert!(executor.live_lifecycle_decision_apply.is_none());
    assert_eq!(executor.pending_work(), 1);
    assert!(services.apply_tasks.is_empty());
    assert_eq!(executor.status().pending_applications, 0);
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());

    // Restore the exact pre-refinement terminal cut and add the affine source
    // which only the real acknowledged Decision WAL can mint. The same Commit
    // retry may now re-enter normal Validate admission, but it still cannot
    // manufacture or enqueue Apply at the executor boundary.
    assert!(
        executor
            .published_lifecycle_validate_retry_markers
            .insert(key, terminal_marker.clone())
            .is_some()
    );
    executor.runtime.pending_live_decision_apply = Some((current_tag, decision));
    let malformed_store = AdapterEffect::StoreBody {
        tag: current_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let malformed_later_owner = bound_test_effect_ownership(&malformed_store, current_tag, 9_028);
    assert!(
        executor
            .retain_effect_batch(
                vec![current_validate.clone(), current_validate.clone()],
                vec![current_validate_ownership.clone(), malformed_later_owner],
            )
            .is_err(),
        "a malformed later position must roll back the projected marker readmission"
    );
    assert_eq!(
        executor.published_lifecycle_validate_retry_markers[&key], terminal_marker,
        "batch preflight failure must retain the exact terminal marker"
    );
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert!(executor.durable_validate_retry_seals.is_empty());

    executor.runtime.exact_effect_ownership = Some((
        current_validate.clone(),
        current_validate_ownership.clone(),
    ));
    assert_eq!(
        executor
            .consume_effects(vec![current_validate.clone()], &mut services)
            .expect("readmit the exact terminal Validate beneath its live Decision WAL source"),
        1,
    );
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.parked_effect_batch.is_none());
    assert!(!executor
        .published_lifecycle_validate_retry_markers
        .contains_key(&key));
    assert!(executor.pending_durable_validate_admissions[&key]
        .exactly_matches_retry(&current_validate, &current_validate_ownership));
    assert!(matches!(
        executor.durable_validate_retry_seals.get(&key),
        Some(DurableValidateRetrySealV1::Live {
            lifecycle_ordinal: None,
            ..
        })
    ));
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));
    assert!(executor.pending_applications.is_empty());
    assert!(executor.live_lifecycle_decision_apply.is_none());
    assert_eq!(executor.pending_work(), 1);
    assert!(services.apply_tasks.is_empty());
    assert_eq!(executor.status().pending_validations, 1);
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}

#[test]
fn published_store_marker_carries_stronger_authority_through_validate_handoff() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let initial_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare.clone()),
    };
    let initial_store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let initial_ownership = bound_test_effect_ownership(&initial_fetch, tag(0), 9_028)
        .rebind_as_inherited_adapter_effect(&initial_store)
        .expect("project the published Prepare Store owner");
    let initial_pending = initial_ownership
        .exact_pending_adapter_effect_binding(&initial_store)
        .expect("seal the published Prepare Store binding");
    let marker = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the published Store marker")
        .bind_store_successor(&initial_store, &initial_pending)
        .expect("bind the published Prepare Store successor");
    executor.commit_published_lifecycle_store_retry_marker(marker);
    let immutable_publication =
        PublishedLifecycleStoreRetryCensusEntryV1::from_exact_published_store(
            &initial_store,
            &initial_pending,
            &durable,
        )
        .expect("reconstruct the registry-side immutable Store publication");
    let immutable_census = BTreeMap::from([(key, immutable_publication.clone())]);
    assert_eq!(
        executor
            .published_lifecycle_store_retry_census()
            .expect("project the initial executor Store census"),
        immutable_census,
    );
    assert_eq!(
        executor.published_lifecycle_store_retry_markers[&key]
            .statement
            .phase(),
        Some(wire::GlobalPhase::Prepare),
    );

    let initial_terminal_identity = initial_ownership
        .candidate_semantic_identity()
        .expect("the published Store has one candidate identity");
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(initial_terminal_identity, initial_ownership.clone())
            .is_none()
    );

    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let stronger_fetch = AdapterEffect::FetchBody {
        tag: tag(1),
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit.clone()),
    };
    let stronger_store = AdapterEffect::StoreBody {
        tag: tag(1),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let stronger_ownership = bound_test_effect_ownership(&stronger_fetch, tag(1), 9_029)
        .rebind_as_inherited_adapter_effect(&stronger_store)
        .expect("project the stronger Commit Store retry owner");
    let stronger_pending = stronger_ownership
        .exact_pending_adapter_effect_binding(&stronger_store)
        .expect("seal the stronger Commit Store retry binding");
    assert_ne!(initial_ownership.owner(), stronger_ownership.owner());
    assert_eq!(
        initial_pending
            .candidate_statement()
            .zip(stronger_pending.candidate_statement())
            .and_then(|(initial, stronger)| initial.body_stage_authority_relation_to(stronger)),
        Some(RuntimeFetchAuthorityRelation::Upgrade),
        "the first retry must strengthen Prepare authority to Commit",
    );
    let stronger_terminal_ownership = bound_test_effect_ownership(&stronger_fetch, tag(1), 9_031)
        .rebind_as_inherited_adapter_effect(&stronger_store)
        .expect("project the foreign Commit Store terminal owner");
    assert_ne!(
        stronger_ownership.owner(),
        stronger_terminal_ownership.owner(),
    );
    let stronger_terminal_identity = stronger_ownership
        .candidate_semantic_identity()
        .expect("the stronger Store retry has one candidate identity");
    assert_ne!(
        stronger_terminal_identity, initial_terminal_identity,
        "the exact candidate identity retains the inherited authority statement",
    );
    assert_eq!(
        stronger_terminal_ownership.candidate_semantic_identity(),
        Some(stronger_terminal_identity),
    );
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(stronger_terminal_identity, stronger_terminal_ownership)
            .is_none()
    );
    let terminal_owners_before = executor.runtime.terminal_body_candidate_owners.clone();
    let terminal_queries_before = executor.runtime.terminal_body_candidate_queries.clone();
    let terminal_commits_before = executor.runtime.terminal_body_candidate_commits;

    executor
        .retain_effect_batch(
            vec![stronger_store.clone()],
            vec![stronger_ownership.clone()],
        )
        .expect("the active Store marker stutters and records the Commit upgrade");

    assert!(executor.retained_effect_batch.is_none());
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries, terminal_queries_before,
        "the stronger Store retry must stutter before runtime terminal-owner comparison",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before,
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners,
        terminal_owners_before,
    );
    let strongest_store_marker = executor.published_lifecycle_store_retry_markers[&key].clone();
    assert_eq!(
        strongest_store_marker.statement.phase(),
        Some(wire::GlobalPhase::Commit),
        "the active Store marker must retain its strongest accepted authority",
    );
    let upgraded_census = executor
        .published_lifecycle_store_retry_census()
        .expect("project the authority-upgraded executor Store census");
    assert_eq!(
        upgraded_census, immutable_census,
        "the mutable strongest-authority overlay must not rewrite registry publication identity",
    );
    let wrong_immutable_publication =
        PublishedLifecycleStoreRetryCensusEntryV1::from_exact_published_store(
            &stronger_store,
            &stronger_pending,
            &durable,
        )
        .expect("reconstruct a distinct same-body Store publication");
    assert_eq!(wrong_immutable_publication.key(), key);
    assert_ne!(
        wrong_immutable_publication, immutable_publication,
        "same body key must not hide a changed immutable Store publication",
    );
    let wrong_immutable_census = BTreeMap::from([(key, wrong_immutable_publication)]);
    assert_ne!(
        upgraded_census, wrong_immutable_census,
        "finalization must reject a same-key immutable publication mismatch",
    );
    assert_ne!(
        upgraded_census,
        BTreeMap::new(),
        "finalization must reject an executor-only Store publication",
    );
    assert_ne!(
        BTreeMap::new(),
        immutable_census,
        "finalization must reject a registry-only Store publication",
    );

    let validate = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let validate_ownership = initial_ownership
        .rebind_as_inherited_adapter_effect(&validate)
        .expect("project Validate from the immutable published Store row");
    let validate_pending = validate_ownership
        .exact_pending_adapter_effect_binding(&validate)
        .expect("seal the immutable published Validate binding");
    let validate_marker = executor
        .prepare_published_lifecycle_validate_retry_marker(&durable)
        .expect("preflight the live Store-to-Validate marker handoff")
        .bind_validate_successor(&validate, &validate_pending)
        .expect("bind Validate to the strongest published Store marker");
    executor.commit_published_lifecycle_validate_retry_marker(validate_marker, 9_034);

    assert!(
        !executor
            .published_lifecycle_store_retry_markers
            .contains_key(&key)
    );
    let accepted_validate_marker =
        executor.published_lifecycle_validate_retry_markers[&key].clone();
    assert_eq!(
        accepted_validate_marker.latest_statement.phase(),
        Some(wire::GlobalPhase::Commit),
    );
    assert_eq!(
        accepted_validate_marker
            .store_terminal
            .pending
            .candidate_statement()
            .and_then(RuntimeCandidateSemanticStatement::phase),
        Some(wire::GlobalPhase::Prepare),
        "the reverse Store fingerprint remains the immutable Prepare publication",
    );
    assert_eq!(
        accepted_validate_marker.store_terminal, strongest_store_marker,
        "Store-to-Validate publication must transfer the strongest Store authority unchanged",
    );

    let weaker_fetch = AdapterEffect::FetchBody {
        tag: tag(2),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let weaker_store = AdapterEffect::StoreBody {
        tag: tag(2),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let weaker_ownership = bound_test_effect_ownership(&weaker_fetch, tag(2), 9_030)
        .rebind_as_inherited_adapter_effect(&weaker_store)
        .expect("project the later weaker Prepare Store retry owner");
    let weaker_pending = weaker_ownership
        .exact_pending_adapter_effect_binding(&weaker_store)
        .expect("seal the later weaker Prepare Store retry binding");
    assert_ne!(initial_ownership.owner(), weaker_ownership.owner());
    assert_eq!(
        accepted_validate_marker
            .store_terminal
            .statement
            .body_stage_authority_relation_to(
                weaker_pending
                    .candidate_statement()
                    .expect("the weaker retry retains one body statement"),
            ),
        Some(RuntimeFetchAuthorityRelation::Stale),
        "the post-Validate retry must be weaker than the retained Commit Store authority",
    );
    assert_eq!(
        weaker_ownership.candidate_semantic_identity(),
        Some(initial_terminal_identity),
    );

    executor
        .retain_effect_batch(vec![weaker_store], vec![weaker_ownership])
        .expect("the Validate marker stutters the later weaker Store retry");

    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_stores.is_empty());
    assert_eq!(
        executor.published_lifecycle_validate_retry_markers[&key], accepted_validate_marker,
        "the weaker retry must not downgrade transferred Store authority",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries, terminal_queries_before,
        "the weaker post-Validate retry must also stutter before terminal-owner comparison",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before,
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners,
        terminal_owners_before,
    );
    assert_eq!(executor.pending_work(), 0);
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
}

#[test]
fn active_published_store_marker_absorbs_stale_inflight_store_completion() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let store_id = install_inflight_remote_proposal_store(
        &mut executor,
        &mut services,
        &fixture,
        tag(0),
        9_033,
    );
    let completion = services.execute_store(store_id);
    let durable = completion.receipt().clone();
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let published_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let published_store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let published_ownership = bound_test_effect_ownership(&published_fetch, tag(0), 9_034)
        .rebind_as_inherited_adapter_effect(&published_store)
        .expect("project the lifecycle-published Prepare Store owner");
    let published_pending = published_ownership
        .exact_pending_adapter_effect_binding(&published_store)
        .expect("seal the lifecycle-published Store binding");
    let marker = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the overlapping published Store marker")
        .bind_store_successor(&published_store, &published_pending)
        .expect("bind the overlapping published Store marker");
    executor.commit_published_lifecycle_store_retry_marker(marker);
    let accepted_marker = executor.published_lifecycle_store_retry_markers[&key].clone();
    let terminal_identity = published_ownership
        .candidate_semantic_identity()
        .expect("the published Store has one terminal candidate identity");
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(terminal_identity, published_ownership.clone())
            .is_none()
    );
    let queued_terminal = RuntimeCompletion::BodyStored(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        durable.clone(),
    );
    executor.runtime.completions.push(queued_terminal.clone());
    let terminal_owners_before = executor.runtime.terminal_body_candidate_owners.clone();
    let terminal_queries_before = executor.runtime.terminal_body_candidate_queries.clone();
    let terminal_commits_before = executor.runtime.terminal_body_candidate_commits;

    assert_eq!(
        executor
            .complete_body_store(completion, &mut services)
            .expect("published Store absorbs the stale physical completion"),
        CompletionDisposition::Accepted,
    );

    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_store_bytes, 0);
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(!executor.body_pipeline_owners.contains_key(&key));
    assert_eq!(
        executor.recovered_bodies.get(&key),
        Some(&(fixture.manifest.clone(), durable.clone())),
    );
    assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
    assert_eq!(
        executor.published_lifecycle_store_retry_markers[&key], accepted_marker,
        "the stale completion cannot downgrade the active marker authority",
    );
    assert_eq!(executor.runtime.completions, vec![queued_terminal]);
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners, terminal_owners_before,
        "completion settlement must not rewrite the published terminal owner",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries, terminal_queries_before,
        "completion settlement must stutter before runtime terminal-owner comparison",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before,
    );
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}

#[test]
fn published_validate_marker_absorbs_stale_inflight_store_completion() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let store_id = install_inflight_remote_proposal_store(
        &mut executor,
        &mut services,
        &fixture,
        tag(0),
        9_035,
    );
    let completion = services.execute_store(store_id);
    let durable = completion.receipt().clone();
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let published_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let published_store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let published_store_ownership = bound_test_effect_ownership(&published_fetch, tag(0), 9_036)
        .rebind_as_inherited_adapter_effect(&published_store)
        .expect("project the lifecycle-published Prepare Store owner");
    let published_store_pending = published_store_ownership
        .exact_pending_adapter_effect_binding(&published_store)
        .expect("seal the lifecycle-published Store binding");
    let store_marker = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the overlapping published Store marker")
        .bind_store_successor(&published_store, &published_store_pending)
        .expect("bind the overlapping published Store marker");
    executor.commit_published_lifecycle_store_retry_marker(store_marker);

    let published_validate = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let published_validate_ownership = published_store_ownership
        .rebind_as_inherited_adapter_effect(&published_validate)
        .expect("project the lifecycle-published Prepare Validate owner");
    let published_validate_pending = published_validate_ownership
        .exact_pending_adapter_effect_binding(&published_validate)
        .expect("seal the lifecycle-published Validate binding");
    let validate_marker = executor
        .prepare_published_lifecycle_validate_retry_marker(&durable)
        .expect("preflight the overlapping Store-to-Validate marker handoff")
        .bind_validate_successor(&published_validate, &published_validate_pending)
        .expect("bind the overlapping published Validate marker");
    executor.commit_published_lifecycle_validate_retry_marker(validate_marker, 9_036);
    let accepted_marker = executor.published_lifecycle_validate_retry_markers[&key].clone();
    let terminal_identity = published_store_ownership
        .candidate_semantic_identity()
        .expect("the published Store has one terminal candidate identity");
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(terminal_identity, published_store_ownership.clone())
            .is_none()
    );
    let queued_terminal = RuntimeCompletion::BodyStored(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        durable.clone(),
    );
    executor.runtime.completions.push(queued_terminal.clone());
    let terminal_owners_before = executor.runtime.terminal_body_candidate_owners.clone();
    let terminal_queries_before = executor.runtime.terminal_body_candidate_queries.clone();
    let terminal_commits_before = executor.runtime.terminal_body_candidate_commits;

    assert_eq!(
        executor
            .complete_body_store(completion, &mut services)
            .expect("published Validate absorbs its stale Store predecessor completion"),
        CompletionDisposition::Accepted,
    );

    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_store_bytes, 0);
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(!executor.body_pipeline_owners.contains_key(&key));
    assert_eq!(
        executor.recovered_bodies.get(&key),
        Some(&(fixture.manifest.clone(), durable.clone())),
    );
    assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
    assert_eq!(
        executor.published_lifecycle_validate_retry_markers[&key],
        accepted_marker,
    );
    assert_eq!(executor.runtime.completions, vec![queued_terminal]);
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners, terminal_owners_before,
        "completion settlement must not rewrite the published terminal owner",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries, terminal_queries_before,
        "completion settlement must not query the foreign runtime terminal owner",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before,
    );
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}

fn assert_published_marker_absorbs_detached_store_completion(publish_validate: bool) {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let original_tag = tag(0);
    let published_tag = tag(1);
    let store_id = install_inflight_remote_proposal_store(
        &mut executor,
        &mut services,
        &fixture,
        original_tag,
        9_037,
    );
    let completion = services.execute_store(store_id);
    let durable = completion.receipt().clone();

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor.runtime.round_tag = Some(published_tag);
    executor.runtime.locked_body = Some(key);
    executor
        .install_view(
            published_tag,
            timeout,
            Some(prepare.clone()),
            None,
            &mut services,
        )
        .expect("EnterView detaches the exact protected Store task");
    assert!(executor.pending_stores[&store_id].consumer.is_none());
    assert!(matches!(
        executor.remote_proposal_replay.get(&key),
        Some(RemoteProposalReplayStageV1::Store { work_id, .. })
            if *work_id == store_id
    ));
    assert!(executor.body_pipeline_owners.contains_key(&key));

    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );
    let published_fetch = AdapterEffect::FetchBody {
        tag: published_tag,
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let published_store = AdapterEffect::StoreBody {
        tag: published_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let published_store_ownership =
        bound_test_effect_ownership(&published_fetch, published_tag, 9_038)
            .rebind_as_inherited_adapter_effect(&published_store)
            .expect("project the later published Store owner");
    let published_store_pending = published_store_ownership
        .exact_pending_adapter_effect_binding(&published_store)
        .expect("seal the later published Store binding");
    let store_marker = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the Store marker beside detached physical work")
        .bind_store_successor(&published_store, &published_store_pending)
        .expect("bind the later published Store marker");
    executor.commit_published_lifecycle_store_retry_marker(store_marker);

    let accepted_validate_marker = if publish_validate {
        let published_validate = AdapterEffect::ValidateBody {
            tag: published_tag,
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
        };
        let published_validate_ownership = published_store_ownership
            .rebind_as_inherited_adapter_effect(&published_validate)
            .expect("project the later published Validate owner");
        let published_validate_pending = published_validate_ownership
            .exact_pending_adapter_effect_binding(&published_validate)
            .expect("seal the later published Validate binding");
        let validate_marker = executor
            .prepare_published_lifecycle_validate_retry_marker(&durable)
            .expect("preflight the Store-to-Validate marker handoff")
            .bind_validate_successor(&published_validate, &published_validate_pending)
            .expect("bind the later published Validate marker");
        executor.commit_published_lifecycle_validate_retry_marker(validate_marker, 9_038);
        Some(executor.published_lifecycle_validate_retry_markers[&key].clone())
    } else {
        None
    };
    let accepted_store_marker = executor
        .published_lifecycle_store_retry_markers
        .get(&key)
        .cloned();

    let terminal_identity = published_store_ownership
        .candidate_semantic_identity()
        .expect("the published Store has one terminal candidate identity");
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(terminal_identity, published_store_ownership)
            .is_none()
    );
    executor.runtime.completions.clear();
    let queued_terminal = RuntimeCompletion::BodyStored(
        published_tag,
        fixture.manifest.round,
        fixture.manifest.subject,
        durable.clone(),
    );
    executor.runtime.completions.push(queued_terminal.clone());
    let terminal_owners_before = executor.runtime.terminal_body_candidate_owners.clone();
    let terminal_queries_before = executor.runtime.terminal_body_candidate_queries.clone();
    let terminal_commits_before = executor.runtime.terminal_body_candidate_commits;

    assert_eq!(
        executor
            .complete_body_store(completion, &mut services)
            .expect("the published marker absorbs the detached physical completion"),
        CompletionDisposition::Accepted,
    );

    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_store_bytes, 0);
    assert!(executor.remote_proposal_replay.is_empty());
    assert!(!executor.body_pipeline_owners.contains_key(&key));
    assert_eq!(
        executor.recovered_bodies.get(&key),
        Some(&(fixture.manifest.clone(), durable.clone())),
    );
    assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
    if let Some(accepted) = accepted_validate_marker {
        assert!(
            !executor
                .published_lifecycle_store_retry_markers
                .contains_key(&key)
        );
        assert_eq!(
            executor.published_lifecycle_validate_retry_markers[&key], accepted,
            "a historical Store completion cannot mutate its Validate successor",
        );
    } else {
        assert_eq!(
            executor.published_lifecycle_store_retry_markers.get(&key),
            accepted_store_marker.as_ref(),
            "weaker historical authority cannot downgrade the Store marker",
        );
    }
    assert_eq!(executor.runtime.completions, vec![queued_terminal]);
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners,
        terminal_owners_before,
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries,
        terminal_queries_before,
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before,
    );
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}

#[test]
fn active_published_store_marker_absorbs_detached_inflight_store_completion() {
    assert_published_marker_absorbs_detached_store_completion(false);
}

#[test]
fn published_validate_marker_absorbs_detached_inflight_store_completion() {
    assert_published_marker_absorbs_detached_store_completion(true);
}

#[test]
fn published_store_marker_atomically_absorbs_local_proposal_store_completion() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start the exact local-proposal Store");
    let store_id = services.store_tasks[0].id();
    assert!(matches!(
        &executor.pending_stores[&store_id].consumer,
        Some(StoreConsumer::LocalProposal { .. })
    ));
    assert!(executor.local_store_replay.contains_key(&store_id));
    let completion = services.execute_store(store_id);
    let durable = completion.receipt().clone();
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let published_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let published_store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let published_ownership = bound_test_effect_ownership(&published_fetch, tag(0), 9_039)
        .rebind_as_inherited_adapter_effect(&published_store)
        .expect("project the published Store owner");
    let published_pending = published_ownership
        .exact_pending_adapter_effect_binding(&published_store)
        .expect("seal the published Store binding");
    let marker = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the Store marker beside local physical work")
        .bind_store_successor(&published_store, &published_pending)
        .expect("bind the Store marker beside local physical work");
    executor.commit_published_lifecycle_store_retry_marker(marker);
    let accepted_marker = executor.published_lifecycle_store_retry_markers[&key].clone();
    let body_projection_before = executor.body_ownership_projection();

    assert_eq!(
        executor
            .complete_body_store(completion, &mut services)
            .expect("the published Store atomically absorbs local physical completion"),
        CompletionDisposition::Accepted,
    );

    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_store_bytes, 0);
    assert!(executor.local_store_replay.is_empty());
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert!(!executor.body_pipeline_owners.contains_key(&key));
    assert_eq!(
        executor.published_lifecycle_store_retry_markers[&key],
        accepted_marker,
    );
    assert_eq!(
        executor.recovered_bodies.get(&key),
        Some(&(fixture.manifest.clone(), durable.clone())),
    );
    assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
    assert!(executor.runtime.completions.is_empty());
    assert_ne!(
        executor.body_ownership_projection(),
        body_projection_before,
        "physical task and replay owners must retire after successful coalescence",
    );
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}

#[test]
fn cold_recovered_lifecycle_store_marker_stutters_stale_foreign_owner_before_terminal_query() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let recovered_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let recovered_store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let recovered_store_ownership = bound_test_effect_ownership(&recovered_fetch, tag(0), 9_026)
        .rebind_as_inherited_adapter_effect(&recovered_store)
        .expect("project the cold-recovered Prepare Store owner");
    let recovered_store_pending = recovered_store_ownership
        .exact_pending_adapter_effect_binding(&recovered_store)
        .expect("seal the cold-recovered Store binding");
    executor
        .install_recovered_published_lifecycle_store_retry_marker(
            &recovered_store,
            &recovered_store_pending,
            &durable,
        )
        .expect("restore the published Store marker before live clocks are armed");
    assert_eq!(executor.published_lifecycle_store_retry_markers.len(), 1);
    let installed_marker = executor.published_lifecycle_store_retry_markers[&key].clone();

    let retry_tag = tag(1);
    let ordinary_fetch = AdapterEffect::FetchBody {
        tag: retry_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let retry_store = AdapterEffect::StoreBody {
        tag: retry_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let retry_ownership = bound_test_effect_ownership(&ordinary_fetch, retry_tag, 9_027)
        .rebind_as_inherited_adapter_effect(&retry_store)
        .expect("project the fresh ordinary Store retry owner");
    let retry_pending = retry_ownership
        .exact_pending_adapter_effect_binding(&retry_store)
        .expect("seal the fresh ordinary Store retry binding");
    assert_ne!(recovered_store_ownership.owner(), retry_ownership.owner());
    assert_eq!(
        recovered_store_pending
            .candidate_statement()
            .zip(retry_pending.candidate_statement())
            .and_then(|(recovered, retry)| recovered.body_stage_authority_relation_to(retry)),
        Some(RuntimeFetchAuthorityRelation::Stale),
        "the regression needs a stale retry under a foreign physical owner",
    );
    let terminal_identity = recovered_store_ownership
        .candidate_semantic_identity()
        .expect("the recovered Store has one candidate identity");
    assert_ne!(
        retry_ownership.candidate_semantic_identity(),
        Some(terminal_identity),
        "ordinary and Prepare Store carriers intentionally have distinct candidate identities",
    );
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(terminal_identity, recovered_store_ownership)
            .is_none()
    );
    let queued_terminal = RuntimeCompletion::BodyStored(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        durable,
    );
    executor.runtime.completions.push(queued_terminal.clone());
    let terminal_owners_before = executor.runtime.terminal_body_candidate_owners.clone();
    let terminal_queries_before = executor.runtime.terminal_body_candidate_queries.clone();
    let terminal_commits_before = executor.runtime.terminal_body_candidate_commits;

    executor
        .retain_effect_batch(vec![retry_store], vec![retry_ownership])
        .expect("the cold Store marker stutters the stale foreign-owner retry");

    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_stores.is_empty());
    assert_eq!(
        executor.published_lifecycle_store_retry_markers[&key],
        installed_marker,
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_owners,
        terminal_owners_before,
    );
    assert_eq!(executor.runtime.completions, vec![queued_terminal]);
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries, terminal_queries_before,
        "the recovered marker must stutter before runtime terminal-owner comparison",
    );
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits,
        terminal_commits_before,
    );
    assert_eq!(executor.pending_work(), 0);
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
}

#[test]
fn cold_recovered_lifecycle_validate_marker_coalesces_timer_authority_upgrade() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()),)
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let initial_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let initial_validate = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let initial_validate_ownership = bound_test_effect_ownership(&initial_fetch, tag(0), 9_024)
        .rebind_as_inherited_adapter_effect(&initial_validate)
        .expect("project the cold-opened Prepare Validate owner");
    let initial_pending = initial_validate_ownership
        .exact_pending_adapter_effect_binding(&initial_validate)
        .expect("seal the cold-opened Validate binding");
    executor
        .install_recovered_published_lifecycle_validate_retry_marker(
            &initial_validate,
            &initial_pending,
            &durable,
            9_024,
        )
        .expect("restore the cold-opened Validate retry marker before clock activation");

    let next_tag = tag(1);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let commit_fetch = AdapterEffect::FetchBody {
        tag: next_tag,
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &commit),
        certificate: Some(commit),
    };
    let timer_validate = AdapterEffect::ValidateBody {
        tag: next_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let timer_ownership = bound_test_effect_ownership(&commit_fetch, next_tag, 9_025)
        .rebind_as_inherited_adapter_effect(&timer_validate)
        .expect("carry Commit authority into the recovered periodic Validate retry");
    executor
        .retain_effect_batch(vec![timer_validate.clone()], vec![timer_ownership])
        .expect("the periodic Validate stutters at its cold-recovered marker");

    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_durable_validate_admissions.is_empty());
    assert!(executor.durable_validate_retry_seals.is_empty());
    assert_eq!(executor.published_lifecycle_validate_retry_markers.len(), 1);
    let marker = &executor.published_lifecycle_validate_retry_markers[&key];
    assert_eq!(marker.latest_effect, timer_validate);
    assert_eq!(
        marker.latest_statement.phase(),
        Some(wire::GlobalPhase::Commit)
    );
    assert_eq!(executor.pending_work(), 0);
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
}

#[test]
fn unprotected_enter_view_preserves_active_published_store_marker() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    assert!(
        executor
            .recovered_bodies
            .insert(key, (fixture.manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(
        executor
            .durable_bodies
            .insert(key, durable.clone())
            .is_none()
    );

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare.proposal_round,
        subject: prepare.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare),
    };
    let store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ownership = bound_test_effect_ownership(&fetch, tag(0), 9_032)
        .rebind_as_inherited_adapter_effect(&store)
        .expect("project the registry-owned Store row");
    let pending = ownership
        .exact_pending_adapter_effect_binding(&store)
        .expect("seal the registry-owned Store row");
    let marker = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the active published Store marker")
        .bind_store_successor(&store, &pending)
        .expect("bind the active published Store marker");
    executor.commit_published_lifecycle_store_retry_marker(marker);
    let accepted_marker = executor.published_lifecycle_store_retry_markers[&key].clone();

    let next_tag = tag(1);
    executor.runtime.round_tag = Some(next_tag);
    executor.runtime.locked_body = None;
    executor
        .install_view(
            next_tag,
            timeout_certificate(&fixture),
            None,
            None,
            &mut services,
        )
        .expect("executor-only EnterView preserves an active registry Store row");

    assert_eq!(executor.protected_lock, None);
    assert_eq!(
        executor.published_lifecycle_store_retry_markers[&key], accepted_marker,
        "an unprotected non-highest active Store marker survives view cleanup",
    );
    assert_eq!(executor.pending_work(), 0);
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn unprotected_enter_view_retires_inert_validate_retry_seal_without_work_debt() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let validate_effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let store_ownership =
        install_stored_remote_proposal_replay(&mut executor, &fixture, tag(0), 9_022);
    let validate_ownership = store_ownership
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("project the ordinary Proposal Validate owner");
    executor
        .retain_effect_batch(vec![validate_effect], vec![validate_ownership])
        .expect("retain the replay-authorized Validate");
    assert_eq!(
        executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("install the durable Validate admission and retry tombstone"),
        1
    );
    assert!(
        executor
            .pending_durable_validate_admissions
            .remove(&key)
            .is_some(),
        "model the move-only owner transferring into the lifecycle registry"
    );
    assert!(executor.durable_validate_retry_seals.contains_key(&key));
    assert_eq!(executor.pending_work(), 0);
    assert_eq!(executor.status().pending_validations, 0);
    assert!(!executor.durable_validate_retry_seals_are_finalization_inert());

    let next_tag = tag(1);
    executor.runtime.round_tag = Some(next_tag);
    executor.runtime.locked_body = None;
    executor
        .install_view(
            next_tag,
            timeout_certificate(&fixture),
            None,
            None,
            &mut services,
        )
        .expect("unprotected EnterView retires the stale Validate tombstone");
    assert!(executor.durable_validate_retry_seals.is_empty());
    assert!(executor.durable_validate_retry_seals_are_finalization_inert());
    assert_eq!(executor.pending_work(), 0);
    assert_eq!(executor.status().pending_validations, 0);
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn decision_body_stage_adoption_rejects_commitment_drift() {
    let fixture = Fixture::new();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let incumbent = bound_test_effect_ownership(&store, tag(0), 9_020);
    let mut conflicting = commit.clone();
    conflicting.execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"conflicting Decision parent state"),
        Hash::new(b"conflicting Decision post state"),
        Hash::new(b"conflicting Decision ordinary writes"),
        1,
        Hash::new(b"conflicting Decision executed block"),
    );
    assert_ne!(
        conflicting.execution_commitment,
        commit.execution_commitment
    );
    let conflicting_fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: conflicting.proposal_round,
        subject: conflicting.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: certified_sources(&fixture, &conflicting),
        certificate: Some(conflicting),
    };
    let incoming = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&conflicting_fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 9_021)],
    )
    .expect("bind conflicting Commit-authorized FetchBody")
    .pop()
    .expect("one conflicting Commit FetchBody owner")
    .rebind_as_inherited_adapter_effect(&store)
    .expect("carry conflicting Commit authority into StoreBody");
    assert!(
        incumbent
            .adopt_incumbent_body_stage_for_durable_decision(
                &incoming,
                &store,
                commit.round,
                commit.proposal_round,
                commit.subject,
                commit.execution_commitment,
            )
            .expect_err("commitment drift must not adopt the incumbent task")
            .contains("proposal or quorum authority")
    );
}
#[test]
fn decision_rebinds_exact_local_store_under_incumbent_owner() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start exact local body store");
    let store_id = services.store_tasks[0].id();
    let incumbent_ownership = executor.pending_stores[&store_id].task.ownership().clone();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    let certified_sources = certified_sources(&fixture, &commit);
    let fetch_effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: commit.proposal_round,
        subject: commit.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources,
        certificate: Some(commit.clone()),
    };
    let decision_fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 9_002)],
    )
    .expect("bind the distinct Commit-authorized Decision root")
    .pop()
    .expect("one Decision FetchBody owner");
    let store_effect = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let decision_store_ownership = decision_fetch_ownership
        .rebind_as_inherited_adapter_effect(&store_effect)
        .expect("carry Commit authority into Decision StoreBody");
    assert_ne!(decision_store_ownership, incumbent_ownership);
    executor
        .consume_effects(vec![fetch_effect], &mut services)
        .expect("Decision recovery detaches the exact local store");
    assert!(executor.pending_stores[&store_id].consumer.is_none());
    assert!(
        executor.local_store_replay.is_empty(),
        "Decision detach consumes local replay authority"
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
            if *completion_tag == tag(0) && manifest == &fixture.manifest
    ));
    executor.runtime.completions.clear();
    executor
        .retain_effect_batch(vec![store_effect], vec![decision_store_ownership])
        .expect("retain the Commit-authorized StoreBody effect");
    executor
        .drain_retained_effect_batch(&mut services, true)
        .expect("Decision reducer adopts the immutable store task");
    assert_eq!(services.store_tasks.len(), 1, "store I/O is not duplicated");
    assert_eq!(
        executor.pending_stores[&store_id].task.ownership(),
        &incumbent_ownership
    );
    assert!(matches!(
        &executor.pending_stores[&store_id].consumer,
        Some(StoreConsumer::Reducer { ownership, .. })
            if ownership == &incumbent_ownership
                && ownership.binds_durable_decision_authority(
                    commit.round,
                    commit.proposal_round,
                    commit.subject,
                    commit.execution_commitment,
                )
    ));
    let completion = services.execute_store(store_id);
    assert_eq!(
        executor
            .complete_body_store(completion, &mut services)
            .expect("route the incumbent store completion to Decision recovery"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyStored(
            completion_tag,
            completion_round,
            completion_subject,
            _
        )) if *completion_tag == tag(0)
            && *completion_round == fixture.manifest.round
            && *completion_subject == fixture.manifest.subject
    ));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}
#[test]
fn apply_requires_validated_body_and_typed_exact_kura_completion() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let initial_mode = executor.lifecycle_mode_rank_snapshot();
    assert_eq!(initial_mode.context_id(), fixture.context.id());
    assert_eq!(initial_mode.height(), fixture.context.height);
    assert_eq!(initial_mode.debt(), 1);
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::LocalProposal(completion_tag, manifest, durable, validated))
            if *completion_tag == tag(0)
                && manifest == &fixture.manifest
                && durable.subject() == fixture.manifest.subject
                && validated.durable() == durable
    ));
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    executor
        .consume_effects(
            vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: commit.clone(),
            }],
            &mut services,
        )
        .expect("begin application");
    let task = &services.apply_tasks[0];
    assert_eq!(task.tag(), tag(0));
    assert_eq!(task.subject(), fixture.manifest.subject);
    assert_eq!(task.certificate(), &commit);
    assert_eq!(
        task.validated_receipt().durable().subject(),
        fixture.manifest.subject
    );
    let work_id = task.id();
    let artifact = wire::finality::V2FinalityArtifact::new(
        fixture.context.clone(),
        fixture.manifest.subject,
        commit,
        vec![vec![0x5C]; fixture.context.roster.len()],
    );
    let receipt = KuraV2CommitReceipt::for_test(&artifact);
    assert_eq!(
        executor
            .complete_application(
                DurableApplyCompletion::new(work_id, receipt, artifact.clone()),
                &mut services,
            )
            .expect("durable application"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::Application(completion_tag, subject))
            if *completion_tag == tag(0) && *subject == fixture.manifest.subject
    ));
    assert_eq!(
        executor.durable_finality().expect("durable finality").1,
        &artifact
    );
    let applied_mode = executor.lifecycle_mode_rank_snapshot();
    assert_eq!(applied_mode.context_id(), fixture.context.id());
    assert_eq!(applied_mode.height(), fixture.context.height);
    assert_eq!(applied_mode.debt(), 0);
}
#[test]
fn apply_completion_rejects_detached_owner_fields_before_settlement() {
    for field in ["authorized owner tag", "lifecycle ordinal"] {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("local proposal");
        complete_local_proposal_fixture(&mut executor, &mut services);
        let commit = fixture.qc(wire::GlobalPhase::Commit);
        executor
            .consume_effects(
                vec![AdapterEffect::Apply {
                    tag: tag(0),
                    subject: fixture.manifest.subject,
                    certificate: commit.clone(),
                }],
                &mut services,
            )
            .expect("begin application");
        let task = services.apply_tasks[0].clone();
        let work_id = task.id();
        let pending = executor
            .pending_applications
            .get_mut(&work_id)
            .expect("ordinary Apply retains its exact runtime owner");
        match field {
            "authorized owner tag" => pending.task.authorized_owner_tag = tag(1),
            "lifecycle ordinal" => {
                pending.task.lifecycle_ordinal = pending.task.lifecycle_ordinal.saturating_add(1)
            }
            _ => unreachable!("the fixed owner-field matrix is exhaustive"),
        }
        let artifact = wire::finality::V2FinalityArtifact::new(
            fixture.context.clone(),
            fixture.manifest.subject,
            commit,
            vec![vec![0x5C]; fixture.context.roster.len()],
        );
        let receipt = KuraV2CommitReceipt::for_test(&artifact);
        assert!(matches!(
            executor.complete_application(
                DurableApplyCompletion::new(work_id, receipt, artifact),
                &mut services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("exact decided-body owner")
        ));
        assert!(executor.pending_applications.contains_key(&work_id));
        assert!(
            executor.status().fail_closed,
            "corrupt {field} must fail closed"
        );
        assert!(!services.closed.is_empty());
        assert!(
            !matches!(
                executor.runtime.completions.last(),
                Some(RuntimeCompletion::Application(_, _))
            ),
            "corrupt {field} cannot settle ApplicationCompleted"
        );
    }
}
crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    apply_worker_request_has_no_runtime_ownership_sidecar
);
#[test]
fn apply_accepts_decided_old_view_but_rejects_wrong_height_tag() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    assert!(matches!(
        executor.begin_apply(
            EventTag::new(2, 3, Generation::new(7)),
            fixture.manifest.subject,
            commit.clone(),
            bound_test_apply_ownership(
                EventTag::new(2, 3, Generation::new(7)),
                fixture.manifest.subject,
                &commit,
                tag(3),
                30,
            ),
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    assert!(executor.pending_applications.is_empty());
    assert!(services.apply_tasks.is_empty());
    executor.runtime.round_tag = Some(tag(3));
    assert!(matches!(
        executor.begin_apply(
            tag(2),
            fixture.manifest.subject,
            commit.clone(),
            bound_test_apply_ownership(tag(2), fixture.manifest.subject, &commit, tag(2), 31,),
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    executor
        .begin_apply(
            tag(3),
            fixture.manifest.subject,
            commit.clone(),
            bound_test_apply_ownership(tag(3), fixture.manifest.subject, &commit, tag(3), 32),
            &mut services,
        )
        .expect("a delayed decided CommitQC remains actionable");
    assert_eq!(executor.pending_applications.len(), 1);
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(services.apply_tasks[0].tag(), tag(3));
    assert_eq!(services.apply_tasks[0].authorized_owner_tag(), tag(3));
    assert_eq!(services.apply_tasks[0].certificate(), &commit);
}
#[test]
fn apply_rejects_matching_commit_qc_from_foreign_context_without_scheduling_work() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("local proposal");
    complete_local_proposal_fixture(&mut executor, &mut services);
    let mut foreign_context = fixture.context.clone();
    foreign_context.network_id =
        crate::sumeragi::synthetic_network_id("foreign-v2-effect-executor-test");
    let mut foreign_commit = fixture.qc(wire::GlobalPhase::Commit);
    foreign_commit.round.context_id = foreign_context.id();
    foreign_commit.proposal_round.context_id = foreign_context.id();
    assert_eq!(foreign_commit.round.height, fixture.manifest.round.height);
    assert_eq!(foreign_commit.round.view, fixture.manifest.round.view);
    assert_eq!(foreign_commit.subject, fixture.manifest.subject);
    assert!(
        foreign_commit.validate(&foreign_context).is_ok(),
        "the adversarial certificate must be internally valid for its foreign context"
    );
    let foreign_apply_ownership = bound_test_apply_ownership(
        tag(0),
        fixture.manifest.subject,
        &foreign_commit,
        tag(0),
        33,
    );
    assert!(matches!(
        executor.begin_apply(
            tag(0),
            fixture.manifest.subject,
            foreign_commit,
            foreign_apply_ownership,
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("frozen height's exact CommitQC")
    ));
    assert!(executor.pending_applications.is_empty());
    assert!(services.apply_tasks.is_empty());
}
fn recovered_preintent_executor(
    fixture: &Fixture,
    directory: TempDir,
    body_store: V2BodyStore,
    current_tag: EventTag,
) -> (V2EffectExecutor<FakeRuntime>, FakeServices) {
    let recovered_bodies = body_store.recovery_catalog().expect("recovery catalog");
    let recovered_validations = body_store.validated_recovery_catalog();
    let recovered_rejections = body_store.rejected_recovery_catalog();
    let retired_recovered_rejections = body_store.retired_rejected_recovery_catalog();
    let mut executor = V2EffectExecutor::with_runtime(
        FakeRuntime {
            round_tag: Some(current_tag),
            next_lifecycle_ordinal: 1,
            ..FakeRuntime::default()
        },
        recovered_bodies,
        fixture.context.clone(),
        PeerId::new(fixture.requester_key.public_key().clone()),
        Some(0),
        EffectQueueConfig::default(),
    )
    .expect("construct cold pre-intent executor");
    executor
        .install_recovered_validation_catalog(
            recovered_validations,
            recovered_rejections,
            retired_recovered_rejections,
        )
        .expect("install exact recovered body outcomes");
    let mut services = fixture.services();
    services.body_store = Some(body_store);
    services._body_directory = Some(directory);
    (executor, services)
}
fn reopen_rejection(fixture: &Fixture) -> (TempDir, V2BodyStore, DurableBodyReceipt) {
    let directory = TempDir::new().expect("rejected pre-intent body-store directory");
    let mut store = V2BodyStore::open_with_policy(
        directory.path(),
        fixture.context.clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
    )
    .expect("open rejected pre-intent body store");
    let durable = store
        .store(fixture.manifest.clone(), fixture.body.clone())
        .expect("persist exact rejected pre-intent body");
    let rejected = store
        .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
            Err::<wire::ExecutionCommitment, _>("deterministic recovered rejection".to_owned())
        })
        .expect("persist deterministic rejection marker");
    assert!(rejected.rejection_reason().is_some());
    drop(store);
    let reopened = V2BodyStore::open_with_policy(
        directory.path(),
        fixture.context.clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
    )
    .expect("reopen rejected pre-intent body store");
    (directory, reopened, durable)
}
#[test]
fn cold_active_rejection_denies_local_adoption_without_live_pipeline_owner() {
    let fixture = Fixture::new();
    let (directory, mut reopened, durable) = reopen_rejection(&fixture);
    reopened
        .revalidate_recovered_markers(|_| {
            Err::<wire::ExecutionCommitment, _>("deterministic recovered rejection".to_owned())
        })
        .expect("semantically replay the exact deterministic rejection");
    assert_eq!(reopened.rejected_recovery_catalog().len(), 1);
    assert!(reopened.retired_rejected_recovery_catalog().is_empty());
    let (mut executor, mut services) =
        recovered_preintent_executor(&fixture, directory, reopened, tag(0));
    let key = (fixture.manifest.round, fixture.manifest.subject);
    assert_eq!(executor.rejected_bodies.get(&key), Some(&durable));
    assert!(!executor.body_pipeline_owners.contains_key(&key));
    let before = executor.body_ownership_projection();

    assert!(matches!(
        executor.admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("durable deterministic rejection")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn cold_preintent_bytes_and_old_view_mismatches_fail_closed_without_work() {
    let fixture = Fixture::new();
    let open_store_only = || {
        let directory = TempDir::new().expect("mismatch pre-intent body-store directory");
        let mut store = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("open mismatch pre-intent body store");
        let _durable = store
            .store(fixture.manifest.clone(), fixture.body.clone())
            .expect("persist mismatch pre-intent body");
        drop(store);
        let reopened = V2BodyStore::open_with_policy(
            directory.path(),
            fixture.context.clone(),
            BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
        )
        .expect("reopen mismatch pre-intent body store");
        (directory, reopened)
    };
    let (directory, reopened) = open_store_only();
    let (mut bytes_executor, mut bytes_services) =
        recovered_preintent_executor(&fixture, directory, reopened, tag(0));
    let bytes_before = bytes_executor.body_ownership_projection();
    let mut wrong_bytes = fixture.body.clone();
    wrong_bytes[0] ^= 0x01;
    assert!(matches!(
        bytes_executor.admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            wrong_bytes,
            &mut bytes_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("bytes do not match")
    ));
    assert_eq!(bytes_executor.body_ownership_projection(), bytes_before);
    assert!(bytes_executor.output_guard.restart_required());
    assert!(bytes_services.store_tasks.is_empty());

    let (directory, reopened) = open_store_only();
    let successor_tag = EventTag::new(tag(0).height(), 1, tag(0).generation());
    let (mut view_executor, mut view_services) =
        recovered_preintent_executor(&fixture, directory, reopened, successor_tag);
    let view_before = view_executor.body_ownership_projection();
    assert!(matches!(
        view_executor.admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut view_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("exact authoritative round")
    ));
    assert_eq!(view_executor.body_ownership_projection(), view_before);
    assert!(view_executor.output_guard.restart_required());
    assert!(view_services.store_tasks.is_empty());
}
#[test]
fn recovered_validation_catalog_hydrates_direct_apply_durability() {
    let fixture = Fixture::new();
    let directory = TempDir::new().expect("body-store directory");
    let mut store = V2BodyStore::open_with_policy(
        directory.path(),
        fixture.context.clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
    )
    .expect("open body store");
    let durable = store
        .store(fixture.manifest.clone(), fixture.body.clone())
        .expect("persist exact body");
    let validated = store
        .validate(&durable, |_| {
            Ok::<_, &'static str>(fixture_execution_commitment())
        })
        .expect("persist exact validation marker");
    drop(store);
    let mut reopened = V2BodyStore::open_with_policy(
        directory.path(),
        fixture.context.clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
    )
    .expect("reopen exact body store");
    reopened
        .revalidate_recovered_markers(|_| Ok::<_, String>(fixture_execution_commitment()))
        .expect("semantically replay recovered validation marker");
    let recovered_bodies = reopened.recovery_catalog().expect("recovery catalog");
    let recovered_validations = reopened.validated_recovery_catalog();
    let recovered_rejections = reopened.rejected_recovery_catalog();
    let retired_recovered_rejections = reopened.retired_rejected_recovery_catalog();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let mut executor = V2EffectExecutor::with_runtime(
        FakeRuntime {
            round_tag: Some(tag(0)),
            ..FakeRuntime::default()
        },
        recovered_bodies,
        fixture.context.clone(),
        PeerId::new(fixture.requester_key.public_key().clone()),
        Some(0),
        EffectQueueConfig::default(),
    )
    .expect("reopened effect executor");
    executor
        .install_recovered_validation_catalog(
            recovered_validations,
            recovered_rejections,
            retired_recovered_rejections,
        )
        .expect("restore executor validation authority");
    assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let mut services = fixture.services();
    let apply_ownership =
        bound_test_apply_ownership(tag(0), fixture.manifest.subject, &commit, tag(0), 34);
    executor
        .begin_apply(
            tag(0),
            fixture.manifest.subject,
            commit.clone(),
            apply_ownership,
            &mut services,
        )
        .expect("replayed CommitQC applies without body-stage replay");
    assert_eq!(services.apply_tasks.len(), 1);
    assert_eq!(services.apply_tasks[0].certificate(), &commit);
    assert_eq!(services.apply_tasks[0].validated_receipt(), &validated);
    assert!(services.closed.is_empty());
    assert!(!executor.status().fail_closed);
}
#[test]
fn recovered_next_vote_body_catalog_join_is_exact_and_store_bound() {
    let fixture = Fixture::new();
    let directory = TempDir::new().expect("body-store directory");
    let mut store = V2BodyStore::open_with_policy(
        directory.path(),
        fixture.context.clone(),
        BlockSignaturePolicy::GenesisAuthority(fixture.validator_keys[0].public_key().clone()),
    )
    .expect("open body store");
    let durable = store
        .store(fixture.manifest.clone(), fixture.body.clone())
        .expect("persist exact body");
    let validated = store
        .validate(&durable, |_| {
            Ok::<_, &'static str>(fixture_execution_commitment())
        })
        .expect("persist exact validation marker");
    let body_store_identity = store.instance_identity();
    let recovered_bodies = store.recovery_catalog().expect("recovery catalog");
    let durable_bodies = BTreeMap::from([(
        (fixture.manifest.round, fixture.manifest.subject),
        durable.clone(),
    )]);
    let validated_bodies = BTreeMap::from([(
        (fixture.manifest.round, fixture.manifest.subject),
        validated.clone(),
    )]);
    let vote = wire::Vote {
        round: fixture.manifest.round,
        proposal_round: fixture.manifest.round,
        phase: wire::GlobalPhase::Prepare,
        subject: fixture.manifest.subject,
        execution_commitment: validated.execution_commitment(),
        signer: 0,
        signature: Vec::new(),
    };
    let lookup = || {
        RecoveredLifecycleNextVoteBodyLookupV1::for_test(
            &vote,
            Some(HashOf::new(&fixture.manifest)),
        )
        .expect("project exact next-Vote body lookup")
    };
    let authority = authenticate_recovered_lifecycle_next_vote_body_catalogs(
        lookup(),
        body_store_identity.clone(),
        &recovered_bodies,
        &durable_bodies,
        &validated_bodies,
    )
    .expect("exact catalogs mint the opaque body authority");
    assert!(authority.exactly_matches_for_test(&validated, &body_store_identity));
    assert!(
        authenticate_recovered_lifecycle_next_vote_body_catalogs(
            lookup(),
            body_store_identity,
            &recovered_bodies,
            &BTreeMap::new(),
            &validated_bodies,
        )
        .is_err(),
        "a missing durable owner must reject the otherwise exact body lookup"
    );
}
include!("v2_effects_kura_tip_replay.rs");
include!("v2_effects_01_view_churn_and_runtime_steps.rs");
include!("v2_effects_highest_prepare_retention.rs");
#[test]
fn runtime_step_reconciliation_rejects_durable_decision_loss() {
    let fixture = Fixture::new();
    let mut services = fixture.services();
    let subject = fixture.manifest.subject;
    services
        .finish_runtime_step_reconciliation(Some(subject))
        .expect("publish the durable Decision");
    let error = services
        .finish_runtime_step_reconciliation(None)
        .expect_err("a durable Decision cannot disappear on a later runtime step");
    assert!(error.contains("lost its durable Decision"));
    assert_eq!(services.durable_runtime_decision, Some(subject));
}
#[test]
fn live_runtime_step_rejects_missing_scheduler_ownership_before_callbacks() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.omit_scheduler_ownership = true;
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![AdapterEffect::Broadcast(
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(vote(&fixture))),
        )])));
    assert!(matches!(
        executor.step(Instant::now(), &mut services),
        Err(EffectExecutorError::Runtime(reason))
            if reason.contains("scheduler owner was missing")
    ));
    assert!(services.broadcasts.is_empty());
    assert!(services.statuses.is_empty());
    assert!(services.durable_runtime_decision.is_none());
    assert!(executor.output_guard.restart_required());
}
include!("v2_effects_02_admission_handoffs.rs");
