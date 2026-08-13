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
            .claim_authenticated_response(&authenticated)
            .expect("install setup claim"),
        CertifiedBodyResponseClaimDisposition::Acquired
    );
    let ownership_before = executor.body_ownership_projection();
    let claims_before = executor.outstanding_requests.response_claim_count();
    let claim_hash_before = executor
        .outstanding_requests
        .response_claim_hash(claimed.request_hash);
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
            .response_claim_hash(claimed.request_hash),
        claim_hash_before
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
            .response_claim_hash(claimed.request_hash),
        claim_hash_before
    );
    assert!(services.completed_certified_fetches.is_empty());
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
fn certified_request_presence_rejects_retained_response_with_different_family_claim() {
    let fixture = Fixture::new();
    assert!(fixture.body.len() > 1);
    let mut executor = fixture.executor(EffectQueueConfig::new(8, 1, 1, 4));
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
    let retained = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let retained_responder = fixture.context.roster[0].validator.clone();
    let (_directory, _ingress, _gate, ingress_ownership) =
        certified_response_runtime_ingress_ownership(
            &fixture,
            &retained,
            retained_responder.clone(),
        );
    assert!(matches!(
        executor.accept_certified_body_response_with_ingress_ownership(
            retained.clone(),
            &retained_responder,
            &ingress_ownership,
            &mut services,
        ),
        Err(EffectTransportError::Backpressure)
    ));
    assert!(executor.has_retained_certified_body_response());
    assert_eq!(executor.outstanding_requests.response_claim_count(), 0);
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    let competing = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        1,
    );
    let competing_responder = fixture.context.roster[1].validator.clone();
    assert_ne!(HashOf::new(&retained), HashOf::new(&competing));
    let authenticated = executor
        .outstanding_requests
        .authenticate_response(&fixture.context, competing, &competing_responder)
        .expect("authenticate a deliberately different response occurrence");
    assert_eq!(
        executor
            .outstanding_requests
            .claim_authenticated_response(&authenticated)
            .expect("install the conflicting family claim"),
        CertifiedBodyResponseClaimDisposition::Acquired
    );
    assert!(matches!(
        executor.validated_certified_request_presence(),
        Err(EffectTransportError::Authentication(
            V2TransportError::InconsistentRequestIndex(request_hash)
        )) if request_hash == retained.request_hash
    ));
}
#[test]
fn retryable_certified_fetch_transfer_retains_claim_token_and_exact_service_owner() {
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
    let exact_response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let exact_responder = fixture.context.roster[0].validator.clone();
    let service_owners_before = services.fetch_tasks.clone();
    let ownership_before = executor.body_ownership_projection();
    services.retry_certified_fetch_once = true;
    assert_eq!(
        executor.accept_certified_body_response(
            exact_response.clone(),
            &exact_responder,
            &mut services,
        ),
        Err(EffectTransportError::Backpressure),
        "only the typed retryable service disposition reopens the handoff",
    );
    assert_eq!(executor.outstanding_requests.response_claim_count(), 1);
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(services.fetch_tasks, service_owners_before);
    assert!(services.completed_certified_fetches.is_empty());
    assert!(services.closed.is_empty());
    assert!(!executor.status().fail_closed);
    let ownership_after_retryable = executor.body_ownership_projection();
    let retained = ownership_after_retryable
        .runtime_body_reservation
        .as_ref()
        .expect("retryable service handoff retains the exact runtime token");
    assert_eq!(retained.tag(), tag(0));
    assert_eq!(retained.manifest(), &fixture.manifest);
    let mut without_token = ownership_after_retryable.clone();
    without_token.runtime_body_reservation = None;
    assert_eq!(
        without_token, ownership_before,
        "the typed retryable boundary changes only the explicit unpublished token",
    );
    let competing_response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        1,
    );
    let competing_responder = fixture.context.roster[1].validator.clone();
    assert!(matches!(
        executor.accept_certified_body_response(
            competing_response,
            &competing_responder,
            &mut services,
        ),
        Err(EffectTransportError::Authentication(
            V2TransportError::ConflictingCertifiedBodyResponseClaim { .. }
        ))
    ));
    assert_eq!(
        executor.body_ownership_projection(),
        ownership_after_retryable,
        "a losing authenticated occurrence cannot transfer any exact owner",
    );
    assert_eq!(executor.outstanding_requests.response_claim_count(), 1);
    assert_eq!(services.fetch_tasks, service_owners_before);
    assert!(services.completed_certified_fetches.is_empty());
    assert!(!executor.status().fail_closed);
    assert_eq!(
        executor
            .accept_certified_body_response(
                exact_response.clone(),
                &exact_responder,
                &mut services,
            )
            .expect("the identical claimed response resumes the same handoff"),
        CompletionDisposition::Accepted,
    );
    assert_eq!(services.completed_certified_fetches, vec![task.id()]);
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
    assert_eq!(executor.outstanding_requests.response_claim_count(), 0);
    assert!(
        executor
            .body_ownership_projection()
            .runtime_body_reservation
            .is_none()
    );
    let later_duplicate = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(exact_response.clone()),
    );
    assert!(
        executor.retained_dispatch_allows_network_ingress(&later_duplicate.payload),
        "a later physical duplicate remains ordinarily drainable after owner retirement",
    );
    assert!(matches!(
        executor
            .probe_certified_response_priority(&exact_response, &exact_responder)
            .expect("a retired response family has a closed non-priority classification"),
        CertifiedResponsePriorityProbe::DefinitelyNonPriority(
            CertifiedResponsePriorityNonPriority::Unsolicited { request_hash }
        ) if request_hash == exact_response.request_hash
    ));
    assert!(!executor.status().fail_closed);
}
#[test]
fn retained_response_certificate_escape_is_charged_only_once() {
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
    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let responder = fixture.context.roster[0].validator.clone();
    let (_directory, _ingress, _gate, ingress_ownership) =
        certified_response_runtime_ingress_ownership(&fixture, &response, responder.clone());
    services.retry_certified_fetch_once = true;
    assert_eq!(
        executor.accept_certified_body_response_with_ingress_ownership(
            response,
            &responder,
            &ingress_ownership,
            &mut services,
        ),
        Err(EffectTransportError::Backpressure)
    );
    assert!(executor.retained_response_may_admit_certified_fence_escape());
    executor.runtime.certified_fence_escape_credit = true;
    executor.reconcile_retained_response_certified_fence_escape_phase();
    assert_eq!(
        executor
            .retained_certified_body_response
            .as_ref()
            .map(|carrier| carrier.certified_fence_escape_phase),
        Some(RetainedCertifiedFenceEscapePhase::Charged)
    );
    assert!(!executor.retained_response_may_admit_certified_fence_escape());
    while executor.runtime.remaining_completion_capacity() > 0 {
        executor
            .runtime
            .completions
            .push(RuntimeCompletion::Signature(tag(0), vec![0xA5]));
    }
    executor.runtime.certified_fence_escape_credit = false;
    executor.reconcile_retained_response_certified_fence_escape_phase();
    assert_eq!(
        executor
            .retained_certified_body_response
            .as_ref()
            .map(|carrier| carrier.certified_fence_escape_phase),
        Some(RetainedCertifiedFenceEscapePhase::Spent)
    );
    executor.runtime.certified_fence_escape_credit = true;
    executor.reconcile_retained_response_certified_fence_escape_phase();
    assert!(
        !executor.retained_response_may_admit_certified_fence_escape(),
        "a later visible certificate cannot replenish this response's spent ingress escape"
    );
    assert_eq!(
        executor
            .retained_certified_body_response
            .as_ref()
            .map(|carrier| carrier.certified_fence_escape_phase),
        Some(RetainedCertifiedFenceEscapePhase::Spent)
    );
    assert!(!executor.status().fail_closed);
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
    assert_eq!(services.durable_serve_decision, Some(commit.subject));
    assert!(!services.decision_serve_reconciliation_pending);
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
    assert_eq!(services.durable_serve_decision, Some(commit.subject));
    assert!(!services.decision_serve_reconciliation_pending);
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
    assert_eq!(services.broadcasts, vec![exact_commit_message]);
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
    complete_local_proposal_chain(&mut executor, &mut services);
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
    complete_local_proposal_chain(&mut executor, &mut services);
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
    complete_local_proposal_chain(&mut executor, &mut services);
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
#[test]
fn decision_body_stage_adoption_rejects_commitment_drift() {
    let fixture = Fixture::new();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let store = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let incumbent = RuntimeEffectOwnership::fresh_for_test(tag(0), 9_020)
        .rebind_same_adapter_effect(&store)
        .expect("bind ordinary incumbent StoreBody");
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
fn decision_body_stage_retry_rejects_same_root_ordinary_binding_without_mutation() {
    let fixture = Fixture::new();
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let decision = (
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    );
    let mut store_executor = fixture.executor(EffectQueueConfig::default());
    let mut store_services = fixture.services();
    store_executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut store_services,
        )
        .expect("start ordinary local StoreBody");
    let store_id = store_services.store_tasks[0].id();
    store_executor
        .pending_stores
        .get_mut(&store_id)
        .expect("pending local store")
        .consumer = None;
    store_executor.protected_decision = Some(decision);
    let store_effect = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ordinary_store_retry = store_executor.pending_stores[&store_id]
        .task
        .ownership()
        .rebind_same_adapter_effect(&store_effect)
        .expect("rebind same-root ordinary StoreBody retry");
    assert_eq!(
        &ordinary_store_retry,
        store_executor.pending_stores[&store_id].task.ownership(),
        "ownership equality deliberately ignores the stale authority binding"
    );
    let store_before = store_executor.body_ownership_projection();
    let store_service_count = store_services.store_tasks.len();
    assert!(matches!(
        store_executor.begin_store_with_plans(
            tag(0),
            fixture.manifest.clone(),
            Arc::from(fixture.body.clone()),
            StorePurpose::Reducer,
            None,
            None,
            ordinary_store_retry,
            &mut store_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("proposal or quorum authority")
    ));
    assert_eq!(store_executor.body_ownership_projection(), store_before);
    assert_eq!(store_services.store_tasks.len(), store_service_count);
    let mut validation_executor = fixture.executor(EffectQueueConfig::default());
    let mut validation_services = fixture.services();
    validation_executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut validation_services,
        )
        .expect("start ordinary local proposal");
    let local_store_id = validation_services.store_tasks[0].id();
    let stored = validation_services.execute_store(local_store_id);
    validation_executor
        .complete_body_store(stored, &mut validation_services)
        .expect("advance ordinary body to ValidateBody");
    let validation_id = validation_services.validation_tasks[0].id();
    validation_executor
        .pending_validations
        .get_mut(&validation_id)
        .expect("pending local validation")
        .consumer = None;
    validation_executor.protected_decision = Some(decision);
    let validation_effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ordinary_validation_retry = validation_executor.pending_validations[&validation_id]
        .task
        .ownership()
        .rebind_same_adapter_effect(&validation_effect)
        .expect("rebind same-root ordinary ValidateBody retry");
    assert_eq!(
        &ordinary_validation_retry,
        validation_executor.pending_validations[&validation_id]
            .task
            .ownership(),
        "ownership equality deliberately ignores the stale authority binding"
    );
    let durable = validation_executor.pending_validations[&validation_id]
        .task
        .durable_receipt()
        .clone();
    let validation_before = validation_executor.body_ownership_projection();
    let validation_service_count = validation_services.validation_tasks.len();
    assert!(matches!(
        validation_executor.plan_begin_validation(
            fixture.manifest.round,
            fixture.manifest.subject,
            durable,
            ValidationConsumer::Reducer {
                tag: tag(0),
                ownership: ordinary_validation_retry,
            },
            None,
            None,
            None,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("proposal or quorum authority")
    ));
    assert_eq!(
        validation_executor.body_ownership_projection(),
        validation_before
    );
    assert_eq!(
        validation_services.validation_tasks.len(),
        validation_service_count
    );
}
#[test]
fn decision_rebinds_exact_local_validation_to_reducer_progress() {
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
    let store_id = services.store_tasks[0].id();
    let stored = services.execute_store(store_id);
    executor
        .complete_body_store(stored, &mut services)
        .expect("advance exact local proposal to validation");
    let validation_id = services.validation_tasks[0].id();
    let incumbent_ownership = executor.pending_validations[&validation_id]
        .task
        .ownership()
        .clone();
    assert!(matches!(
        &executor.pending_validations[&validation_id].consumer,
        Some(ValidationConsumer::LocalProposal { .. })
    ));
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
        manifest: None,
        certified_sources,
        certificate: Some(commit.clone()),
    };
    let decision_fetch_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch_effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 9_001)],
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
    let validation_effect = AdapterEffect::ValidateBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let decision_validation_ownership = decision_store_ownership
        .rebind_as_inherited_adapter_effect(&validation_effect)
        .expect("carry Commit authority into Decision ValidateBody");
    assert_ne!(decision_validation_ownership, incumbent_ownership);
    executor
        .consume_effects(vec![fetch_effect], &mut services)
        .expect("Decision detaches the exact local validation consumer");
    assert!(
        executor.pending_validations[&validation_id]
            .consumer
            .is_none()
    );
    assert!(
        executor.local_validate_replay.is_empty(),
        "Decision detach consumes local replay authority"
    );
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(_, manifest)] if manifest == &fixture.manifest
    ));
    executor.runtime.completions.clear();
    executor
        .retain_effect_batch(vec![store_effect], vec![decision_store_ownership])
        .expect("retain the Commit-authorized StoreBody effect");
    executor
        .drain_retained_effect_batch(&mut services, true)
        .expect("decided reducer adopts the exact durable body");
    executor.runtime.completions.clear();
    executor
        .retain_effect_batch(vec![validation_effect], vec![decision_validation_ownership])
        .expect("retain the Commit-authorized ValidateBody effect");
    executor
        .drain_retained_effect_batch(&mut services, true)
        .expect("decided reducer reattaches exact validation work");
    assert!(matches!(
        &executor.pending_validations[&validation_id].consumer,
        Some(ValidationConsumer::Reducer {
            tag: consumer,
            ownership,
        }) if *consumer == tag(0)
            && ownership == &incumbent_ownership
            && ownership.binds_durable_decision_authority(
                commit.round,
                commit.proposal_round,
                commit.subject,
                commit.execution_commitment,
            )
    ));
    assert_eq!(
        executor.pending_validations[&validation_id]
            .task
            .ownership(),
        &incumbent_ownership,
        "the physical validation keeps its original lifecycle root"
    );
    assert_eq!(
        services.validation_tasks.last().map(BodyValidationTask::id),
        Some(validation_id)
    );
    let completed = services.execute_validation(validation_id);
    assert_eq!(
        executor
            .complete_body_validation(completed, &mut services)
            .expect("route the incumbent validation completion under Commit authority"),
        CompletionDisposition::Accepted,
    );
    let completion_ownership = executor
        .runtime
        .validation_completion_ownerships
        .last()
        .expect("owned validation completion retains its reducer authority");
    assert_eq!(completion_ownership.owner(), incumbent_ownership.owner());
    assert!(completion_ownership.binds_durable_decision_authority(
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    assert!(!executor.status().fail_closed);
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
    complete_local_proposal_chain(&mut executor, &mut services);
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
        complete_local_proposal_chain(&mut executor, &mut services);
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
#[test]
fn reproposal_commit_qc_applies_the_exact_unchanged_body() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let mut commit = fixture.qc(wire::GlobalPhase::Commit);
    commit.round.view = fixture
        .manifest
        .round
        .view
        .checked_add(2)
        .expect("fixture reproposal view increment");
    commit.proposal_round = commit.round;
    let reproposal_manifest = canonical_payload_manifest(
        &fixture.context,
        commit.round,
        fixture.manifest.subject,
        &fixture.body,
    );
    assert!(executor.protected_lock.is_none());
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
                round: commit.proposal_round,
                subject: fixture.manifest.subject,
                manifest: None,
                certified_sources,
                certificate: Some(commit.clone()),
            }],
            &mut services,
        )
        .expect("lockless follower fetches the authenticated reproposal body");
    let fetch = services
        .fetch_tasks
        .last()
        .expect("certified fetch task")
        .clone();
    assert_eq!(fetch.round, commit.proposal_round);
    executor
        .complete_body_reconstruction(
            &fetch,
            reproposal_manifest,
            fixture.body.clone(),
            &mut services,
        )
        .expect("reproposal body arrives");
    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: tag(0),
                round: commit.proposal_round,
                subject: commit.subject,
            }],
            &mut services,
        )
        .expect("store the authenticated reproposal body");
    let store_id = services.store_tasks.last().expect("store task").id();
    let stored = services.execute_store(store_id);
    executor
        .complete_body_store(stored, &mut services)
        .expect("durable reproposal body");
    executor
        .consume_effects(
            vec![AdapterEffect::ValidateBody {
                tag: tag(0),
                round: commit.proposal_round,
                subject: commit.subject,
            }],
            &mut services,
        )
        .expect("validate the authenticated reproposal body");
    let validation_id = services
        .validation_tasks
        .last()
        .expect("validation task")
        .id();
    let validated = services.execute_validation(validation_id);
    executor
        .complete_body_validation(validated, &mut services)
        .expect("reproposal validation completes");
    executor
        .consume_effects(
            vec![AdapterEffect::Apply {
                tag: tag(0),
                subject: commit.subject,
                certificate: commit.clone(),
            }],
            &mut services,
        )
        .expect("reproposal CommitQC applies after its exact body arrives");
    let task = services.apply_tasks.last().expect("application task");
    assert_eq!(task.tag(), tag(0));
    assert_eq!(task.authorized_owner_tag(), tag(0));
    assert_eq!(task.certificate(), &commit);
    assert_eq!(
        task.validated_receipt().durable().round(),
        commit.proposal_round
    );
    assert_eq!(task.validated_receipt().durable().round(), commit.round);
    assert!(!executor.status().fail_closed);
}
#[test]
fn apply_worker_request_has_no_runtime_ownership_sidecar() {
    let source = include_str!("../v2_effects.rs");
    let task = source
        .split_once("pub(crate) struct ApplyTask {")
        .expect("ApplyTask has one declaration")
        .1
        .split_once("impl ApplyTask {")
        .expect("ApplyTask implementation follows its declaration")
        .0;
    for required in ["authorized_owner_tag: EventTag", "lifecycle_ordinal: u128"] {
        assert!(task.contains(required), "ApplyTask omitted {required}");
    }
    assert!(!task.contains("RuntimeEffectOwnership"));
    let pending = source
        .split_once("struct PendingApply {")
        .expect("ordinary Apply pending state has one declaration")
        .1
        .split_once("struct ReadyBody {")
        .expect("ReadyBody follows ordinary Apply pending state")
        .0;
    assert!(pending.contains("ownership: RuntimeEffectOwnership"));
    let preflight = source
        .split_once("fn preflight_pending_application_owner(")
        .expect("ordinary Apply owner has one exact preflight")
        .1
        .split_once("fn preflight_deferred_work_owner(")
        .expect("deferred work preflight follows Apply preflight")
        .0;
    assert!(
        preflight
            .contains("task.lifecycle_ordinal() != pending.ownership.owner().lifecycle_ordinal()")
    );
    let completion = source
        .split_once("pub(crate) fn complete_application")
        .expect("Apply completion has one production entrypoint")
        .1
        .split_once("/// Current bounded operational status.")
        .expect("status follows Apply completion")
        .0;
    let owner_preflight = completion
        .find("preflight_pending_application_owner(completion.work_id, pending)")
        .expect("Apply completion revalidates the retained runtime owner");
    let task_borrow = completion
        .find("let task = &pending.task;")
        .expect("Apply completion borrows the task after owner validation");
    assert!(owner_preflight < task_borrow);
}
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
    complete_local_proposal_chain(&mut executor, &mut services);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    assert!(matches!(
        executor.begin_apply(
            EventTag::new(2, 3, Generation::new(7)),
            fixture.manifest.subject,
            commit.clone(),
            RuntimeEffectOwnership::fresh_for_test(tag(3), 30),
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
            RuntimeEffectOwnership::fresh_for_test(tag(2), 31),
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    executor
        .begin_apply(
            tag(3),
            fixture.manifest.subject,
            commit.clone(),
            RuntimeEffectOwnership::fresh_for_test(tag(3), 32),
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
    complete_local_proposal_chain(&mut executor, &mut services);
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
    assert!(matches!(
        executor.begin_apply(
            tag(0),
            fixture.manifest.subject,
            foreign_commit,
            RuntimeEffectOwnership::fresh_for_test(tag(0), 33),
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("frozen height's exact CommitQC")
    ));
    assert!(executor.pending_applications.is_empty());
    assert!(services.apply_tasks.is_empty());
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
        .runtime
        .bind_validated_body(&fixture.manifest, &validated)
        .expect("restore runtime validation authority");
    executor
        .install_recovered_validation_catalog(recovered_validations)
        .expect("restore executor validation authority");
    assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
    assert_eq!(executor.validated_bodies.get(&key), Some(&validated));
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let mut services = fixture.services();
    executor
        .begin_apply(
            tag(0),
            fixture.manifest.subject,
            commit.clone(),
            RuntimeEffectOwnership::fresh_for_test(tag(0), 34),
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
fn decision_serve_fence_rejects_durable_decision_loss_without_reopening() {
    let fixture = Fixture::new();
    let mut services = fixture.services();
    let subject = fixture.manifest.subject;
    services
        .begin_decision_serve_reconciliation()
        .expect("raise the initial Decision/Serve fence");
    services
        .finish_decision_serve_reconciliation(Some(subject))
        .expect("publish the durable Serve Decision");
    services
        .begin_decision_serve_reconciliation()
        .expect("raise the next runtime-step fence");
    let error = services
        .finish_decision_serve_reconciliation(None)
        .expect_err("a durable Decision cannot disappear on a later runtime step");
    assert!(error.contains("lost its durable Decision"));
    assert_eq!(services.durable_serve_decision, Some(subject));
    assert!(
        services.decision_serve_reconciliation_pending,
        "failed reconciliation keeps exact Serve admission fenced"
    );
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
    assert!(services.decision_serve_reconciliation_pending);
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
    assert!(services.decision_serve_reconciliation_pending);
    assert!(executor.output_guard.restart_required());
}
include!("v2_effects_02_admission_handoffs.rs");
