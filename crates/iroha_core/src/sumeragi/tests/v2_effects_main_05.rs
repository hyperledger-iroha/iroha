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
        certificate: Some(commit),
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
    let conflicting_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
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
    let drifted_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
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
        pending,
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
    let DurableValidateRetrySealV1::Recovered { owner, frontier } =
        &executor.durable_validate_retry_seals[&key]
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
    let DurableValidateRetrySealV1::Recovered { owner, frontier } =
        &executor.durable_validate_retry_seals[&key]
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
    let DurableValidateRetrySealV1::Recovered { owner, frontier } =
        &executor.durable_validate_retry_seals[&key]
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
    let DurableValidateRetrySealV1::Recovered { owner, frontier } = &accepted else {
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
    conflicting.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
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
    let conflicting_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
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
        certificate: Some(commit),
    };
    let commit_validate_ownership = bound_test_effect_ownership(&commit_fetch, tag, 9_016)
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry durable Commit authority into Validate");
    assert!(
        commit_validate_ownership
            .binds_durable_decision_authority(decision.0, decision.1, decision.2, decision.3,)
    );
    executor.runtime.decided_body = Some(decision);
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
        .install_view(next_tag, timeout, Some(prepare.clone()), &mut services)
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
        certificate: Some(prepare),
    };
    let prepare_validate_ownership = bound_test_effect_ownership(&prepare_fetch, next_tag, 9_018)
        .rebind_as_inherited_adapter_effect(&validate_effect)
        .expect("carry Prepare authority into the protected Validate");
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
    conflicting.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
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
        .install_view(next_tag, timeout_certificate(&fixture), None, &mut services)
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
    conflicting.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
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
#[test]
fn recovery_runtime_step_rejects_invalid_scheduler_ownership_before_callbacks() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.reject_scheduler_ownership = true;
    executor.runtime.steps.push_back(Ok(RuntimeStep::Idle));
    assert!(matches!(
        executor.step_pending_tip_recovery(Instant::now(), &mut services),
        Err(EffectExecutorError::Runtime(reason))
            if reason.contains("scheduler owner was invalid")
    ));
    assert!(services.statuses.is_empty());
    assert!(services.durable_runtime_decision.is_none());
    assert!(executor.output_guard.restart_required());
}
include!("v2_effects_02_admission_handoffs.rs");
