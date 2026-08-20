#[test]
fn vote_signing_requires_the_exact_fsynced_execution_commitment() {
    let fixture = Fixture::new();
    let mut missing = fixture.executor(EffectQueueConfig::default());
    let mut missing_services = fixture.services();
    assert!(matches!(
        missing.consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            }],
            &mut missing_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("fsynced validation marker")
    ));
    assert!(missing.status().fail_closed);
    assert!(missing_services.sign_tasks.is_empty());
    let mut drift = fixture.executor(EffectQueueConfig::default());
    let mut drift_services = fixture.services();
    install_fsynced_validation_fixture(
        &mut drift,
        &mut drift_services,
        &fixture,
        fixture.manifest.clone(),
    );
    let mut drifted_vote = vote(&fixture);
    drifted_vote.execution_commitment = wire::ExecutionCommitment::without_topups_or_merge_carrier(
        Hash::new(b"drifted effects fixture parent state"),
        Hash::new(b"drifted effects fixture post state"),
        Hash::new(b"drifted effects fixture ordinary writes"),
        1,
        Hash::new(b"drifted effects fixture executed block wire"),
    );
    assert!(matches!(
        drift.consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(drifted_vote),
            }],
            &mut drift_services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("differs from the durable validation marker")
    ));
    assert!(drift.status().fail_closed);
    assert!(drift_services.sign_tasks.is_empty());
}
#[test]
fn split_round_commit_signing_is_rejected_before_service_dispatch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let mut commit = vote(&fixture);
    commit.round = round(&fixture.context, fixture.manifest.round.view + 2);
    commit.proposal_round = fixture.manifest.round;
    commit.phase = wire::GlobalPhase::Commit;
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(commit.round.view),
                request: SignRequest::Vote(commit),
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("same-round proposal authority")
    ));
    assert!(services.sign_tasks.is_empty());
}
#[test]
fn reproposal_commit_signing_uses_its_same_round_validation_marker() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let reproposal_round = round(&fixture.context, fixture.manifest.round.view + 2);
    let reproposal_manifest = canonical_payload_manifest(
        &fixture.context,
        reproposal_round,
        fixture.manifest.subject,
        &fixture.body,
    );
    install_fsynced_validation_fixture(&mut executor, &mut services, &fixture, reproposal_manifest);
    let mut commit = vote(&fixture);
    commit.round = reproposal_round;
    commit.proposal_round = reproposal_round;
    commit.phase = wire::GlobalPhase::Commit;
    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(reproposal_round.view),
                request: SignRequest::Vote(commit.clone()),
            }],
            &mut services,
        )
        .expect("same-round reproposal Commit owns its exact validation marker");
    assert!(matches!(
        services.sign_tasks.as_slice(),
        [task]
            if matches!(task.request(), SignRequest::Vote(vote) if vote == &commit)
    ));
    assert!(!executor.status().fail_closed);
}
#[test]
fn sign_effect_verifies_signature_and_preserves_original_tag() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    let request = SignRequest::Vote(vote(&fixture));
    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: request.clone(),
            }],
            &mut services,
        )
        .expect("consume sign");
    let task = services.sign_tasks[0].clone();
    let preimage = match task.request() {
        SignRequest::Vote(vote) => vote.signature_preimage(),
        _ => panic!("vote task expected"),
    };
    let signature = Signature::new(fixture.validator_keys[0].private_key(), &preimage)
        .payload()
        .to_vec();
    assert_eq!(
        executor
            .complete_consensus_signature(task.id(), signature.clone(), &mut services)
            .expect("complete signature"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        &executor.runtime.completions[0],
        RuntimeCompletion::Signature(completion_tag, completion)
            if *completion_tag == tag(0) && completion == &signature
    ));
    assert_eq!(
        executor
            .complete_consensus_signature(task.id(), signature, &mut services)
            .expect("stale completion"),
        CompletionDisposition::Stale
    );
}
#[test]
fn invalid_signer_completion_fails_closed_without_runtime_input() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_fsynced_validation_fixture(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            }],
            &mut services,
        )
        .expect("consume sign");
    let id = services.sign_tasks[0].id();
    let wrong = Signature::new(fixture.validator_keys[1].private_key(), b"wrong")
        .payload()
        .to_vec();
    assert!(matches!(
        executor.complete_consensus_signature(id, wrong, &mut services),
        Err(EffectExecutorError::InvalidConsensusSignature(_))
    ));
    assert!(executor.runtime.completions.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn broadcast_view_and_evidence_effects_reach_exact_hooks() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let message = wire::ConsensusMessageV2 {
        protocol_version: wire::PROTOCOL_VERSION,
        payload: wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            signature: vec![1],
            ..vote(&fixture)
        }),
    };
    executor
        .consume_effects(
            vec![
                AdapterEffect::Broadcast(message.clone()),
                AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_certificate(&fixture),
                    protected_lock: None,
                },
                AdapterEffect::ReportEquivocation {
                    evidence: vote_equivocation_evidence(&fixture, 1),
                },
                AdapterEffect::ReportInvalidCertifiedBody {
                    subject: fixture.manifest.subject,
                    certificate: fixture.qc(wire::GlobalPhase::Prepare),
                },
            ],
            &mut services,
        )
        .expect("consume immediate effects");
    assert_eq!(services.broadcasts, vec![message]);
    assert_eq!(services.entered_views, vec![tag(1)]);
    assert_eq!(services.equivocations.len(), 1);
    assert_eq!(services.invalid_bodies, vec![fixture.manifest.subject]);
}
#[test]
fn equivocation_reporting_rejects_a_mutated_non_conflicting_pair() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (first, mut second) = vote_equivocation_evidence(&fixture, 1)
        .into_vote_pair_for_test()
        .expect("vote evidence helper returns vote evidence");
    second.round = first.round;
    second.proposal_round = first.proposal_round;
    second.phase = first.phase;
    second.subject = first.subject;
    second.execution_commitment = first.execution_commitment;
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::ReportEquivocation {
                evidence: AdapterEquivocationEvidence::vote_for_test(first, second),
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("do not form one conflict")
    ));
    assert!(services.equivocations.is_empty());
}
#[test]
fn authenticated_chunk_reconstruction_rejection_retires_fetch_nonfatally() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("begin fetch");
    let work_id = services.fetch_tasks[0].id();
    services.reject_authenticated_chunks = true;
    let mut chunk = wire::PayloadChunk {
        manifest_hash: HashOf::new(&fixture.manifest),
        index: 0,
        bytes: fixture.encoded_chunks[0].clone(),
        sender: 0,
        signature: Vec::new(),
    };
    chunk.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &chunk
            .signature_preimage(&fixture.context, &fixture.manifest)
            .expect("chunk preimage"),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        executor.accept_payload_chunk(
            work_id,
            chunk,
            &fixture.context.roster[0].validator,
            &mut services,
        ),
        Err(EffectTransportError::BodyMismatch(
            "authenticated chunks reconstructed invalid or noncanonical body data"
        ))
    ));
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.body_pipeline_owners.is_empty());
    assert_eq!(services.chunks, vec![work_id]);
    assert!(services.closed.is_empty());
    assert!(!executor.status().fail_closed);
}
#[test]
fn failed_view_cleanup_keeps_stale_fetch_and_requires_restart() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let certified_sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("admit prior-view body recovery");
    let before = executor.body_ownership_projection();
    services.fail_on = Some("cancel-fetch");
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert!(services.entered_views.is_empty());
    assert!(executor.output_guard.restart_required());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::FailClosed(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn view_cleanup_rejects_inconsistent_protected_request_before_lock_mutation() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let certified_sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources,
                certificate: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("admit certified prior-view recovery");
    let request_hash = *executor
        .certified_work
        .keys()
        .next()
        .expect("certified request index");
    assert!(executor.outstanding_requests.cancel(request_hash));
    let before = executor.body_ownership_projection();
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: Some(prepare),
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(executor.protected_lock, None);
    assert!(services.cancelled_fetches.is_empty());
    assert!(services.entered_views.is_empty());
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn view_cleanup_second_cancellation_failure_commits_no_fetch_retirement() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let first_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let first_sources = certified_sources(&fixture, &first_prepare);
    let (second_subject, second_body) = distinct_body(&fixture);
    let second_manifest = canonical_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        second_subject,
        &second_body,
    );
    let mut second_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    second_prepare.subject = second_manifest.subject;
    let second_sources = certified_sources(&fixture, &second_prepare);
    executor
        .consume_effects(
            vec![
                AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: first_sources,
                    certificate: Some(first_prepare),
                },
                AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: second_manifest.round,
                    subject: second_manifest.subject,
                    manifest: Some(second_manifest),
                    certified_sources: second_sources,
                    certificate: Some(second_prepare),
                },
            ],
            &mut services,
        )
        .expect("admit two stale certified recoveries");
    assert_eq!(executor.pending_fetches.len(), 2);
    let first_work_id = services.fetch_tasks[0].id();
    let before = executor.body_ownership_projection();
    services.fail_on_call = Some(("cancel-fetch", 2));
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        ),
        Err(EffectExecutorError::Service(reason))
            if reason.contains("cancel-fetch call 2 failed")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.cancelled_fetches, vec![first_work_id]);
    assert!(services.entered_views.is_empty());
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn owned_payload_chunk_rejects_source_swap_before_service_and_keeps_unknown_work_nonfatal() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let chunk = signed_payload_chunk(&fixture);
    let sender = fixture.context.roster[0].validator.clone();
    let unknown = EffectWorkId::for_test(999);
    let exact_ownership = payload_chunk_ingress_ownership(&chunk, sender.clone());
    assert_eq!(
        executor.accept_payload_chunk_with_ingress_ownership(
            unknown,
            chunk.clone(),
            &sender,
            &exact_ownership,
            &mut services,
        ),
        Err(EffectTransportError::UnknownWork(unknown))
    );
    assert!(!executor.status().fail_closed);
    assert!(services.chunks.is_empty());
    let foreign_origin = fixture.context.roster[1].validator.clone();
    let swapped_ownership = payload_chunk_ingress_ownership(&chunk, foreign_origin);
    assert!(matches!(
        executor.accept_payload_chunk_with_ingress_ownership(
            unknown,
            chunk,
            &sender,
            &swapped_ownership,
            &mut services,
        ),
        Err(EffectTransportError::FailClosed(reason))
            if reason.contains("fair-ingress ownership")
    ));
    assert!(services.chunks.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn proposal_reconstruction_rejects_noncanonical_manifest_without_fail_close() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let mut alternate_chunk = fixture.body.clone();
    alternate_chunk[0] ^= 1;
    let alternate_manifest = deliberately_conflicting_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        &alternate_chunk,
    );
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(alternate_manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("proposal starts body acquisition");
    let fetch_task = services.fetch_tasks[0].clone();
    assert_eq!(
        executor
            .complete_body_reconstruction(
                &fetch_task,
                alternate_manifest,
                fixture.body.clone(),
                &mut services,
            )
            .expect("noncanonical proposal data is a recoverable remote rejection"),
        CompletionDisposition::Rejected
    );
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.ready_bodies.is_empty());
    assert!(executor.body_pipeline_owners.is_empty());
    assert!(executor.runtime.completions.is_empty());
    assert!(services.closed.is_empty());
    assert!(!executor.status().fail_closed);
}
#[test]
fn certified_response_is_bound_to_exact_request_and_consumed_once() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("certified fetch");
    let work_id = services.fetch_tasks[0].id();
    let request = services.fetch_tasks[0]
        .certified_request()
        .expect("signed request")
        .clone();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(&request),
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: 0,
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let responder = fixture.context.roster[0].validator.clone();
    assert_eq!(
        executor
            .accept_certified_body_response(response.clone(), &responder, &mut services)
            .expect("authenticated certified response"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
    assert_eq!(services.completed_certified_fetches, vec![work_id]);
    assert!(matches!(
        executor.accept_certified_body_response(response, &responder, &mut services,),
        Err(EffectTransportError::Authentication(
            V2TransportError::UnsolicitedResponse(_)
        ))
    ));
}
#[test]
fn certified_manifest_mismatch_is_recoverable_in_both_authority_orders() {
    for proposal_first in [true, false] {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        if proposal_first {
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: Vec::new(),
                        certificate: None,
                    }],
                    &mut services,
                )
                .expect("proposal starts body acquisition");
        }
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: proposal_first.then(|| fixture.manifest.clone()),
                    certified_sources: sources.clone(),
                    certificate: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("certificate adds body authority");
        if !proposal_first {
            executor
                .consume_effects(
                    vec![AdapterEffect::FetchBody {
                        tag: tag(0),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: sources,
                        certificate: Some(prepare),
                    }],
                    &mut services,
                )
                .expect("proposal adds manifest authority");
        }
        let work_id = services.fetch_tasks[0].id();
        let request = services
            .fetch_tasks
            .last()
            .and_then(BodyFetchTask::certified_request)
            .expect("signed certified request")
            .clone();
        let mut alternate_chunk = fixture.body.clone();
        alternate_chunk[0] ^= 1;
        let alternate_manifest = deliberately_conflicting_payload_manifest(
            &fixture.context,
            fixture.manifest.round,
            fixture.manifest.subject,
            &alternate_chunk,
        );
        assert_ne!(alternate_manifest, fixture.manifest);
        let signed_response = |manifest: wire::PayloadManifest, responder: wire::ValidatorIndex| {
            let mut response = wire::CertifiedBodyResponse {
                request_hash: HashOf::new(&request),
                manifest,
                body: fixture.body.clone(),
                responder,
                signature: Vec::new(),
            };
            response.signature = Signature::new(
                fixture.validator_keys[responder as usize].private_key(),
                &response.signature_preimage(),
            )
            .payload()
            .to_vec();
            response
        };
        let mismatched = signed_response(alternate_manifest, 0);
        mismatched
            .validate_against(
                &fixture.context,
                &request,
                &fixture.context.roster[0].validator,
            )
            .expect("alternate manifest passes request-level authentication");
        assert!(matches!(
            executor.accept_certified_body_response(
                mismatched,
                &fixture.context.roster[0].validator,
                &mut services,
            ),
            Err(EffectTransportError::BodyMismatch(
                "certified response manifest differs from proposal authority"
            ))
        ));
        assert_eq!(executor.pending_fetches.len(), 1);
        assert_eq!(executor.certified_work.len(), 1);
        assert_eq!(executor.outstanding_requests.len(), 1);
        assert!(
            executor.outstanding_requests.response_claim_count() == 0,
            "authenticated junk cannot acquire the physical response occurrence"
        );
        assert!(services.completed_certified_fetches.is_empty());
        assert!(services.closed.is_empty());
        assert!(!executor.status().fail_closed);
        assert!(executor.runtime.completions.is_empty());
        let correct = signed_response(fixture.manifest.clone(), 1);
        assert_eq!(
            executor
                .accept_certified_body_response(
                    correct,
                    &fixture.context.roster[1].validator,
                    &mut services,
                )
                .expect("another frozen-roster archive supplies the authoritative manifest"),
            CompletionDisposition::Accepted
        );
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.completed_certified_fetches, vec![work_id]);
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(0) && manifest == &fixture.manifest
        ));
    }
}
#[test]
fn certificate_first_response_rederives_manifest_before_proposal_authority() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: None,
                certified_sources: sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("certificate starts acquisition before proposal authority");
    let work_id = services.fetch_tasks[0].id();
    let request = services.fetch_tasks[0]
        .certified_request()
        .expect("signed certified request")
        .clone();
    let mut alternate_chunk = fixture.body.clone();
    alternate_chunk[0] ^= 1;
    let alternate_manifest = deliberately_conflicting_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        &alternate_chunk,
    );
    let signed_response = |manifest: wire::PayloadManifest, responder: wire::ValidatorIndex| {
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(&request),
            manifest,
            body: fixture.body.clone(),
            responder,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[responder as usize].private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        response
    };
    let mismatched = signed_response(alternate_manifest, 0);
    mismatched
        .validate_against(
            &fixture.context,
            &request,
            &fixture.context.roster[0].validator,
        )
        .expect("alternate manifest passes request-level authentication");
    assert!(matches!(
        executor.accept_certified_body_response(
            mismatched,
            &fixture.context.roster[0].validator,
            &mut services,
        ),
        Err(EffectTransportError::BodyMismatch(
            "certified response manifest is not canonical for its body"
        ))
    ));
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.certified_work.len(), 1);
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert!(
        executor.outstanding_requests.response_claim_count() == 0,
        "a noncanonical response cannot pin the physical response occurrence"
    );
    assert!(services.completed_certified_fetches.is_empty());
    assert!(services.closed.is_empty());
    assert!(!executor.status().fail_closed);
    assert_eq!(
        executor
            .accept_certified_body_response(
                signed_response(fixture.manifest.clone(), 1),
                &fixture.context.roster[1].validator,
                &mut services,
            )
            .expect("another frozen-roster archive supplies the canonical manifest"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
    assert_eq!(services.completed_certified_fetches, vec![work_id]);
}
#[test]
fn certified_response_retirement_failure_is_fail_closed_after_authentication() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("hybrid fetch");
    let request = services.fetch_tasks[0]
        .certified_request()
        .expect("signed request")
        .clone();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(&request),
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: 0,
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let before = executor.body_ownership_projection();
    services.fail_on = Some("complete-certified-fetch");
    assert!(matches!(
        executor.accept_certified_body_response(
            response,
            &fixture.context.roster[0].validator,
            &mut services,
        ),
        Err(EffectTransportError::FailClosed(_))
    ));
    assert!(executor.status().fail_closed);
    let mut after = executor.body_ownership_projection();
    let retained = after
        .runtime_body_reservation
        .take()
        .expect("failed service transfer retains the exact runtime token");
    assert_eq!(retained.tag(), tag(0));
    assert_eq!(retained.manifest(), &fixture.manifest);
    assert_eq!(
        after, before,
        "no ownership other than the explicit retry token changes",
    );
    assert!(
        services.fail_on.is_none(),
        "failure injection was not consumed"
    );
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn certified_response_priority_probe_is_read_only_and_detects_revalidation_drift() {
    let fixture = Fixture::new();
    assert!(fixture.body.len() > 1);
    let mut executor = fixture.executor(EffectQueueConfig::new(8, 1, 1, 4));
    assert_eq!(executor.validated_certified_request_presence(), Ok(false));
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
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    executor
        .validate_lifecycle_ingress_selector_authority()
        .expect("healthy executor can classify an exact queue cut");
    let task = services.fetch_tasks[0].clone();
    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let responder = fixture.context.roster[0].validator.clone();
    let ownership_before = executor.body_ownership_projection();
    let claims_before = executor.outstanding_requests.response_claim_count();
    let claim_hash_before = executor
        .outstanding_requests
        .response_claim_hash(response.request_hash);
    let candidate = match executor
        .probe_certified_response_priority(&response, &responder)
        .expect("exact response authentication succeeds")
    {
        CertifiedResponsePriorityProbe::PreflightRequired(candidate) => candidate,
        CertifiedResponsePriorityProbe::DefinitelyNonPriority(reason) => {
            panic!("exact outstanding response was classified non-priority: {reason:?}")
        }
        CertifiedResponsePriorityProbe::RecoveredPreflightRequired(_) => {
            panic!("ordinary outstanding response was classified as recovered")
        }
    };
    assert_eq!(candidate.context_id(), fixture.context.id());
    assert_eq!(candidate.height(), fixture.context.height);
    assert_eq!(candidate.work_id(), task.id());
    assert_eq!(candidate.fetch_tag(), tag(0));
    assert_eq!(candidate.request_hash(), response.request_hash);
    assert_eq!(candidate.response_hash(), HashOf::new(&response));
    assert_eq!(candidate.round(), fixture.manifest.round);
    assert_eq!(candidate.subject(), fixture.manifest.subject);
    assert_eq!(
        candidate.proposal_manifest_hash(),
        Some(HashOf::new(&fixture.manifest))
    );
    assert!(
        candidate
            .pending_effect_binding()
            .exactly_binds_adapter_effect(&task.adapter_effect())
    );
    assert_eq!(
        candidate.canonical_manifest_hash(),
        HashOf::new(&fixture.manifest)
    );
    assert_eq!(candidate.body_payload_hash(), Hash::new(&fixture.body));
    assert_eq!(
        candidate.claim_preflight(),
        &CertifiedBodyResponseClaimPreflight::Vacant
    );
    assert!(candidate.matches_authenticated_response(&response, &responder));
    assert_eq!(
        executor
            .revalidate_certified_response_priority_candidate(&candidate, &response, &responder,),
        Ok(true),
        "an unchanged executor must reproduce the exact opaque candidate"
    );
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before,
        "the read-only probe cannot acquire the response family"
    );
    assert_eq!(
        executor
            .outstanding_requests
            .response_claim_hash(response.request_hash),
        claim_hash_before
    );
    let mut unsolicited = response.clone();
    unsolicited.request_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"unsolicited certified response priority probe"));
    assert!(matches!(
        executor
            .probe_certified_response_priority(&unsolicited, &responder)
            .expect("unsolicited classification is not an authentication capability"),
        CertifiedResponsePriorityProbe::DefinitelyNonPriority(
            CertifiedResponsePriorityNonPriority::Unsolicited { request_hash }
        ) if request_hash == unsolicited.request_hash
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before
    );
    assert_eq!(
        executor
            .outstanding_requests
            .response_claim_hash(response.request_hash),
        claim_hash_before
    );
    let mut invalid = response.clone();
    invalid.signature[0] ^= 1;
    assert!(matches!(
        executor.probe_certified_response_priority(&invalid, &responder),
        Err(EffectTransportError::Authentication(
            V2TransportError::InvalidSignature { .. }
        ))
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(
        executor.outstanding_requests.response_claim_count(),
        claims_before
    );
    assert_eq!(
        executor
            .outstanding_requests
            .response_claim_hash(invalid.request_hash),
        claim_hash_before
    );
    assert!(services.completed_certified_fetches.is_empty());
    assert!(executor.runtime.completions.is_empty());
    assert!(!executor.status().fail_closed);
    let authenticated = executor
        .outstanding_requests
        .authenticate_response(&fixture.context, response.clone(), &responder)
        .expect("authenticate exact setup claim");
    assert_eq!(
        executor
            .outstanding_requests
            .claim_authenticated_response(&authenticated)
            .expect("install exact setup claim"),
        CertifiedBodyResponseClaimDisposition::Acquired
    );
    assert_eq!(
        executor
            .revalidate_certified_response_priority_candidate(&candidate, &response, &responder,),
        Ok(false),
        "Vacant-to-retransmission family drift must change the opaque candidate"
    );
    executor.output_guard.activate_restart_required();
    assert!(matches!(
        executor.validate_lifecycle_ingress_selector_authority(),
        Err(EffectTransportError::FailClosed(_))
    ));
}
#[test]
fn recovered_decision_fetch_fences_later_ordinary_body_coordinates() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let requester = PeerId::new(fixture.requester_key.public_key().clone());
    let certificate = fixture.qc(wire::GlobalPhase::Commit);
    let sources = certified_sources(&fixture, &certificate);
    let mut request = wire::CertifiedBodyRequest {
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        certificate: certificate.clone(),
        requester: requester.clone(),
        signature: Vec::new(),
    };
    request.signature = Signature::new(
        fixture.requester_key.private_key(),
        &request.signature_preimage(),
    )
    .payload()
    .to_vec();
    let authenticated = executor
        .authenticate_certified_body_request(request, &requester)
        .expect("authenticate the recovered request fixture");
    let key = crate::sumeragi::v2_lifecycle_coordinator::RecoveredDecisionFetchDispatchKeyV1::for_height_context_test(
        &fixture.context,
        41,
        0xD1,
    );
    let owner =
        RecoveredDecisionFetchRequestOwnerV1::for_test(key, tag(0), sources.clone(), authenticated);
    assert!(owner.validates_exact_executor_context(&fixture.context, &requester));
    assert_eq!(
        executor.recovered_decision_fetch_registration_available(&owner),
        Ok(true),
        "an exact vacant recovered owner reports physical executor capacity"
    );
    let request_hash = owner.request_hash();
    assert!(
        executor
            .recovered_decision_fetches
            .insert(key, owner)
            .is_none()
    );
    assert!(
        executor
            .recovered_decision_fetch_by_request
            .insert(request_hash, key)
            .is_none()
    );
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    assert_eq!(
        executor.recovered_decision_fetch_registration_available(
            executor
                .recovered_decision_fetches
                .get(&key)
                .expect("installed recovered owner remains indexed"),
        ),
        Err(RecoveredDecisionFetchRequestRegistrationErrorV1::Occupied),
        "an existing dedicated owner is corruption/ownership, not capacity backpressure"
    );

    let ingress =
        crate::sumeragi::FairV2Ingress::new(32, 5 * 512 * 1024, 512 * 1024, 0, 512 * 1024);
    ingress
        .configure_roster(
            fixture
                .context
                .roster
                .iter()
                .map(|power| power.validator.clone()),
        )
        .expect("fixture roster fits the recovered selector ingress");
    ingress.state.lock().leader_wire_context = Some((fixture.context.id(), fixture.context.height));
    ingress.open().expect("open recovered selector ingress");
    let recovered_response = |responder: wire::ValidatorIndex| {
        let mut response = wire::CertifiedBodyResponse {
            request_hash,
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[usize::try_from(responder).expect("small responder index")]
                .private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        response
    };
    let mut ordinary_response = recovered_response(2);
    ordinary_response.request_hash = HashOf::from_untyped_unchecked(Hash::new(
        b"ordinary response ahead of recovered Decision Fetch",
    ));
    ordinary_response.signature = Signature::new(
        fixture.validator_keys[2].private_key(),
        &ordinary_response.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(ordinary_response.clone()),
            )),
            Some(fixture.context.roster[2].validator.clone()),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let ordinary_ordinal = ingress.state.lock().last_admission_ordinal;
    let mut first_recovered_ordinal = None;
    for responder in [0, 1] {
        let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::CertifiedBodyResponse(recovered_response(responder)),
        ));
        assert!(matches!(
            ingress.try_push(InboundBlockMessage::new(
                message,
                Some(
                    fixture.context.roster
                        [usize::try_from(responder).expect("small responder index")]
                    .validator
                    .clone(),
                ),
            )),
            Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
        ));
        first_recovered_ordinal.get_or_insert_with(|| ingress.state.lock().last_admission_ordinal);
    }
    let queue_depth_before_selector = ingress.len();
    let physical_cut_before_selector = ingress.next_physical_admission_ordinal();
    assert!(
        executor
            .prepare_next_recovered_decision_fetch_ingress_selector(&ingress)
            .expect("ordinary fair head is a non-fatal lifecycle pass-through")
            .is_none(),
        "a later recovered response cannot leapfrog the ordinary fair winner",
    );
    assert_eq!(ingress.len(), queue_depth_before_selector);
    let drained_ordinary = ingress
        .try_recv_if_checked(|_| true)
        .expect("ordinary checked dequeue remains available")
        .expect("ordinary winner remains queued after lifecycle pass-through");
    assert_eq!(
        drained_ordinary
            .ingress_ownership()
            .expect("ordinary response retains physical ownership")
            .first
            .physical_admission_ordinal,
        ordinary_ordinal,
    );
    assert!(matches!(
        drained_ordinary.message(),
        BlockMessage::V2(message)
            if matches!(
                &message.payload,
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)
                    if response.request_hash == ordinary_response.request_hash
            )
    ));
    let mut selected = executor
        .prepare_next_recovered_decision_fetch_ingress_selector(&ingress)
        .expect("queue-owned recovered response selection remains exact")
        .expect("one recovered response family is selected");
    assert_eq!(
        selected.selected_cut_for_test().2,
        first_recovered_ordinal.expect("one recovered response was enqueued"),
        "the queue-owned selector chooses the next fair exact family occurrence",
    );
    let target = selected
        .take_lifecycle_io_target()
        .expect("the selected target remains a recovered Fetch persistence carrier");
    assert_eq!(
        target.kind(),
        crate::sumeragi::v2_lifecycle_coordinator::LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence
    );
    assert!(target.matches_recovered_decision_fetch_key(key));
    drop(selected);
    assert_eq!(ingress.len(), queue_depth_before_selector - 1);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        physical_cut_before_selector,
        "queue-owned selector discovery cannot dequeue or renumber ingress",
    );

    let uncertified_effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let uncertified_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&uncertified_effect),
        vec![executor.runtime.test_effect_ownership(&uncertified_effect)],
    )
    .expect("bind exact uncertified Fetch owner")
    .pop()
    .expect("one uncertified Fetch has one exact owner");
    let certified_effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: None,
        certified_sources: sources.clone(),
        certificate: Some(certificate.clone()),
    };
    let certified_ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&certified_effect),
        vec![executor.runtime.test_effect_ownership(&certified_effect)],
    )
    .expect("bind exact certified Fetch owner")
    .pop()
    .expect("one certified Fetch has one exact owner");
    let ownership_before = executor.body_ownership_projection();
    let service_fetches_before = services.fetch_tasks.clone();
    let uncertified = executor.begin_fetch(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        Some(fixture.manifest.clone()),
        Vec::new(),
        None,
        uncertified_ownership.clone(),
        None,
        &mut services,
    );
    assert!(matches!(
        uncertified,
        Err(EffectExecutorError::Contract(reason))
            if reason == "body-fetch coordinates already have a recovered Decision Fetch owner"
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(services.fetch_tasks, service_fetches_before);
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    let certified = executor.begin_fetch(
        tag(0),
        fixture.manifest.round,
        fixture.manifest.subject,
        None,
        sources,
        Some(certificate),
        certified_ownership,
        None,
        &mut services,
    );
    assert!(matches!(
        certified,
        Err(EffectExecutorError::Contract(reason))
            if reason == "body-fetch coordinates already have a recovered Decision Fetch owner"
    ));
    assert_eq!(executor.body_ownership_projection(), ownership_before);
    assert_eq!(services.fetch_tasks, service_fetches_before);
    assert_eq!(executor.validated_certified_request_presence(), Ok(true));
    let collision_id = EffectWorkId::for_test(73);
    executor.pending_fetches.insert(
        collision_id,
        PendingFetch {
            task: BodyFetchTask {
                id: collision_id,
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                sources: Vec::new(),
                certified_request: None,
                ownership: uncertified_ownership,
            },
            request_hash: None,
        },
    );
    assert!(matches!(
        executor.validated_certified_request_presence(),
        Err(EffectTransportError::Authentication(
            V2TransportError::InconsistentRequestIndex(hash)
        )) if hash == request_hash
    ));
}
#[test]
#[allow(clippy::too_many_lines)]
fn lifecycle_selector_capture_censuses_competing_response_family_exactly_once() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    let prepare =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let effects = vec![AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: fixture.certified_sources(&prepare),
        certificate: Some(prepare),
    }];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&effects)
        .expect("bind production Fetch ownership for selector capture");
    fixture
        .executor
        .consume_effects(effects, &mut services)
        .expect("hybrid fetch establishes one exact response family");
    let task = services.fetch_tasks[0].clone();
    let response = |responder: wire::ValidatorIndex| {
        let mut response = wire::CertifiedBodyResponse {
            request_hash: HashOf::new(
                task.certified_request()
                    .expect("selector Fetch owns its signed request"),
            ),
            manifest: fixture.manifest.clone(),
            body: fixture.body.clone(),
            responder,
            signature: Vec::new(),
        };
        response.signature = Signature::new(
            fixture.validator_keys[usize::try_from(responder).expect("small responder index")]
                .private_key(),
            &response.signature_preimage(),
        )
        .payload()
        .to_vec();
        response
    };
    let first_response = response(0);
    let second_response = response(1);
    assert_eq!(first_response.request_hash, second_response.request_hash);
    assert_ne!(HashOf::new(&first_response), HashOf::new(&second_response));
    let ingress =
        crate::sumeragi::FairV2Ingress::new(32, 5 * 512 * 1024, 512 * 1024, 0, 512 * 1024);
    ingress
        .configure_roster(
            fixture
                .context
                .roster
                .iter()
                .map(|power| power.validator.clone()),
        )
        .expect("fixture roster fits the selector ingress");
    ingress.state.lock().leader_wire_context = Some((fixture.context.id(), fixture.context.height));
    ingress.open().expect("open selector ingress");
    let first_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(first_response.clone()),
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(
            first_message,
            Some(fixture.context.roster[0].validator.clone()),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let first_ordinal = ingress.state.lock().last_admission_ordinal;
    let second_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(second_response),
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(
            second_message,
            Some(fixture.context.roster[1].validator.clone()),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let second_ordinal = ingress.state.lock().last_admission_ordinal;
    assert!(first_ordinal < second_ordinal);
    let prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, second_ordinal)
        .expect("complete selector preparation authenticates both occurrences");
    let (
        context,
        positions,
        selected_ordinal,
        physical_cut,
        selected_is_embedded,
        request_fence_active,
    ) = prepared.selected_cut_for_test();
    assert_eq!(context.height(), fixture.context.height);
    assert_eq!(selected_ordinal, second_ordinal);
    assert!(positions.into_iter().all(|position| position > 0));
    assert!(physical_cut > u128::from(second_ordinal));
    assert!(selected_is_embedded);
    assert!(request_fence_active);
    assert_eq!(prepared.verdict_count_for_test(), 2);
    assert_eq!(
        prepared.priority_owners_for_test(),
        &BTreeSet::from([first_ordinal, second_ordinal]),
        "both untrusted physical completions own the request fence",
    );
    assert_eq!(
        prepared.selector_debt_for_test(),
        2,
        "the claimed-family winner must not double-count its fence owner",
    );
    assert_eq!(
        prepared.claimed_family_winners_for_test(),
        BTreeMap::from([(first_response.request_hash, first_ordinal)]),
        "the lowest exact physical response wins the request family",
    );
    assert_eq!(
        prepared.certified_fetch_ready_authority_for_test(),
        Err(CertifiedFetchReadyPublicationError::SelectedOccurrenceNotClaimedResponse),
        "a selected request-fenced retransmission cannot borrow the other occurrence's family authority",
    );
    let winning_prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact family winner remains an authenticated prepared target");
    let (
        wake_context,
        wake_ordinal,
        wake_physical_digest,
        wake_request_hash,
        wake_key,
        _causal_root,
        wake_source,
    ) = winning_prepared
        .certified_fetch_ready_authority_for_test()
        .expect("the selected family winner derives one sealed Fetch wake authority");
    let signed_request = task
        .certified_request()
        .expect("the selected family retains its exact signed request");
    assert_eq!(wake_context, context);
    assert_eq!(wake_ordinal, first_ordinal);
    assert_ne!(wake_physical_digest, LifecycleDigest::new([0; 32]));
    assert_eq!(wake_request_hash, first_response.request_hash);
    assert_eq!(wake_key.phase(), LifecyclePhase::Fetch);
    assert_eq!(
        wake_key.round().height(),
        signed_request.certificate.round.height
    );
    assert_eq!(
        wake_key.round().view(),
        signed_request.certificate.round.view
    );
    assert_eq!(
        wake_key
            .proposal_round()
            .map(|round| (round.height(), round.view())),
        Some((signed_request.round.height, signed_request.round.view))
    );
    assert!(matches!(wake_source, WaitSource::External(_)));
    let incumbent_effect = task.adapter_effect();
    let incumbent_pending = task
        .ownership()
        .exact_pending_adapter_effect_binding(&incumbent_effect)
        .expect("the real Fetch task mints its exact ordinal-free registry binding");
    let queue_depth_before = ingress.len();
    let next_physical_ordinal_before = ingress.next_physical_admission_ordinal();
    let (incumbent_digest, replacement_digest) = winning_prepared
        .certified_fetch_registry_preflight_for_test(incumbent_effect, incumbent_pending)
        .expect("the real selector winner crosses the sealed registry preflight");
    assert_ne!(incumbent_digest, replacement_digest);
    assert_eq!(replacement_digest, wake_physical_digest);
    assert_eq!(ingress.len(), queue_depth_before);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        next_physical_ordinal_before,
        "registry preflight cannot dequeue, append, or renumber ingress",
    );
    let reprobed_winner = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact queued winner remains selectable after registry preflight");
    assert_eq!(
        reprobed_winner.selected_cut_for_test(),
        winning_prepared.selected_cut_for_test(),
        "registry preflight leaves the complete queue-minted cut unchanged",
    );
    assert_eq!(
        winning_prepared.selected_cut_for_test().2,
        first_ordinal,
        "deriving readiness borrows and preserves the complete prepared token",
    );
    let owner_effect = task.adapter_effect();
    let owner_pending = task
        .ownership()
        .exact_pending_adapter_effect_binding(&owner_effect)
        .expect("mint the exact Fetch registry carrier for owner admission");
    let proofs = fixture
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified = VerifiedHeightContext::genesis(fixture.context.clone(), proofs)
        .expect("verified owner context");
    let owner_directory = TempDir::new().expect("temporary lifecycle owner storage");
    let (mut owner, lifecycle_ordinal, lifecycle_source) =
        crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1::waiting_fetch_for_ingress_test(
            verified,
            &winning_prepared,
            owner_effect,
            owner_pending,
            &fixture.validator_keys[0],
            owner_directory.path(),
        );
    let (mut production_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    let before_foreign_sign_cursor =
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    assert!(matches!(
        owner.dispatch_recovered_lifecycle_sign(
            &production_services,
            &fixture.executor,
            crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context,),
        ),
        Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation)
    ));
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_foreign_sign_cursor,
        "a non-Completion runner cursor cannot claim or mutate a recovered Sign owner",
    );
    let before_unbound = owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let unbound_result = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        winning_prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    assert!(matches!(
        unbound_result,
        Err(ProductionIngressSchedulerInputsError::BodyStoreNotBound)
    ));
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_unbound,
        "an owner retaining its startup store cannot plan against an independent service",
    );
    let foreign_output_guard = ConsensusOutputGuard::isolated();
    let mut planner_io = owner.bind_body_store_to_planner_io_for_test(
        &mut production_services,
        Arc::clone(&foreign_output_guard),
        1,
    );
    let guard_mismatch_prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact winner remains selectable for the guard mismatch");
    let before_guard_mismatch =
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let guard_mismatch = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        guard_mismatch_prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    assert!(matches!(
        guard_mismatch,
        Err(ProductionIngressSchedulerInputsError::ForeignOutputGuard)
    ));
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_guard_mismatch,
        "a foreign service guard cannot advance the coordinator or claim a lease",
    );
    assert!(
        !fixture.executor.output_guard.restart_required(),
        "guard mismatch rejection leaves the executor's canonical output open",
    );
    assert!(
        !foreign_output_guard.restart_required(),
        "pre-capture mismatch rejection leaves the foreign service guard open",
    );
    planner_io.install_output_guard_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
    );
    V2EffectServices::enqueue_body_fetch(&mut production_services, task.clone())
        .expect("install the exact certified-Fetch service owner for Phase B");
    planner_io.saturate_consensus_prefix(&production_services);
    let waiting_prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact winner remains selectable for a capacity wait");
    let before_capacity_wait =
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let capacity_result = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        waiting_prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    let capacity_wait = match capacity_result {
        Ok(ProductionIngressTurnPreparation::CapacityWait(wait)) => wait,
        Ok(ProductionIngressTurnPreparation::Queued(_)) => {
            panic!("a saturated Consensus prefix cannot admit Fetch persistence")
        }
        Err(_) => panic!("saturation must return the opaque capacity wait"),
    };
    assert_eq!(
        capacity_wait.capacity_status(&production_services),
        ProductionIngressCapacityStatus::Pending
    );
    let capacity_wait = match capacity_wait.retry(&production_services, &fixture.executor) {
        ProductionIngressCapacityRetry::Pending(wait) => wait,
        ProductionIngressCapacityRetry::Released(_) => {
            panic!("the unchanged saturated generation cannot release capacity")
        }
        ProductionIngressCapacityRetry::RestartRequired => {
            panic!("the exact unchanged service/executor owners cannot require restart")
        }
    };
    planner_io.release_one_predecessor();
    assert_eq!(
        capacity_wait.capacity_status(&production_services),
        ProductionIngressCapacityStatus::Released
    );
    let released_selector = match capacity_wait.retry(&production_services, &fixture.executor) {
        ProductionIngressCapacityRetry::Released(selector) => selector,
        ProductionIngressCapacityRetry::Pending(_) => {
            panic!("an advanced service generation must release the retained selector")
        }
        ProductionIngressCapacityRetry::RestartRequired => {
            panic!("an exact generation release cannot require restart")
        }
    };
    drop(released_selector);
    planner_io.release_one_predecessor();
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_capacity_wait,
        "capacity waiting cannot advance the Fetch generation or claim a lease",
    );
    let winning_prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the exact winner remains selectable after capacity release");
    let mode = fixture.executor.lifecycle_mode_rank_snapshot();
    let runner =
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context);
    let planned = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        mode,
        winning_prepared,
        runner,
    );
    let queued = match planned {
        Ok(ProductionIngressTurnPreparation::Queued(queued)) => queued,
        Ok(ProductionIngressTurnPreparation::CapacityWait(_)) => {
            panic!("available exact capacity must not produce a capacity wait")
        }
        Err(_) => panic!("the exact locked Fetch transaction must publish its command"),
    };
    assert_eq!(queued.ordinal(), lifecycle_ordinal);
    assert!(matches!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        (
            Some(LifecycleState::Waiting(wait)),
            Some(1),
            None,
            false,
        ) if wait.source() == lifecycle_source && wait.observed_generation() == 1
    ));
    assert_eq!(planner_io.queued_certified_fetch_count(), 1);
    let repeated = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, first_ordinal)
        .expect("the queued physical winner remains selectable before Phase B");
    let before_repeat = owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let repeated_result = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        repeated,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    assert!(matches!(
        repeated_result,
        Err(ProductionIngressSchedulerInputsError::InFlightSelectedWork { .. })
    ));
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        before_repeat,
        "an in-flight exact command must reject before advancing Fetch generation",
    );
    assert_eq!(planner_io.queued_certified_fetch_count(), 1);
    assert!(!fixture.executor.output_guard.restart_required());
    let queue_depth_before_completion = ingress.len();
    planner_io.execute_one_certified_fetch(Arc::clone(&fixture.executor.output_guard));
    let completion = match production_services
        .take_next_lifecycle_completion()
        .expect("the persisted Fetch retains its exact physical completion owner")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::CertifiedFetch(completion) => {
            completion
        }
        _ => panic!("the persisted ordinary Fetch must classify as CertifiedFetch"),
    };
    if owner
        .complete_certified_fetch_for_test(
            &mut fixture.executor,
            &mut production_services,
            &ingress,
            completion,
        )
        .is_err()
    {
        panic!("Phase B must publish Ready and retire the exact physical response");
    }
    assert_eq!(
        ingress.len(),
        queue_depth_before_completion - 1,
        "Phase B dequeues only the selected response occurrence",
    );
    assert!(matches!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        (Some(LifecycleState::Ready), Some(1), None, false)
    ));
    assert!(matches!(
        production_services
            .take_next_lifecycle_completion()
            .expect("the completion FIFO remains valid after exact acknowledgement"),
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::None
    ));
    assert!(!fixture.executor.output_guard.restart_required());
    planner_io.detach(&mut production_services);
}
