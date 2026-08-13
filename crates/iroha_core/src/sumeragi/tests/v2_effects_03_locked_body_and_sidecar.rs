#[test]
fn synthetic_higher_round_same_subject_retires_origin_bound_stages_before_raw_cache_reuse() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let body_len = u64::try_from(fixture.body.len()).expect("body length");
    let original_tag = EventTag::new(1, 0, Generation::new(70));
    let original = (fixture.manifest.round, fixture.manifest.subject);
    executor
        .reconcile_locked_body_for_recovery(original_tag, original, &mut services)
        .expect("publish the original exact lock");
    executor
        .retain_locked_body_for_recovery(
            original_tag,
            original.0,
            fixture.manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("retain and stage the exact-origin locked body");
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: original_tag,
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("queue the original-round BodyAvailable completion");
    assert_eq!(executor.ready_body_bytes, body_len * 2);
    assert_eq!(executor.runtime.completions.len(), 1);
    let replacement_manifest = manifest_at_view(&fixture, 1);
    let mut replacement = fixture.qc(wire::GlobalPhase::Prepare);
    replacement.round = replacement_manifest.round;
    replacement.proposal_round = replacement_manifest.round;
    replacement.subject = replacement_manifest.subject;
    let mut timeout = timeout_at_view(&fixture, 1);
    timeout.groups[0].highest_prepare_qc = Some(replacement.clone());
    let replacement_tag = EventTag::new(1, 2, Generation::new(72));
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: replacement_tag,
                certificate: timeout,
                protected_lock: Some(replacement.clone()),
            }],
            &mut services,
        )
        .expect("the higher round retires only the old round-bound stage");
    assert!(executor.ready_bodies.is_empty());
    assert!(executor.runtime.completions.is_empty());
    assert!(executor.body_pipeline_owners.is_empty());
    assert!(executor.retained_locked_body.is_some());
    assert_eq!(executor.ready_body_bytes, body_len);
    let sources = certified_sources(&fixture, &replacement);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: replacement_tag,
                round: replacement.round,
                subject: replacement.subject,
                manifest: Some(replacement_manifest.clone()),
                certified_sources: sources,
                certificate: Some(replacement),
            }],
            &mut services,
        )
        .expect("the new round remints its stage from the subject cache");
    assert_eq!(executor.ready_body_bytes, body_len * 2);
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
            if *completion_tag == replacement_tag && manifest == &replacement_manifest
    ));
    assert!(services.fetch_tasks.is_empty());
}
#[test]
#[allow(clippy::too_many_lines)]
fn local_proposal_async_chain_orders_and_reuses_bounded_work() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    for _ in 0..8 {
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("retry local store");
    }
    assert_eq!(executor.pending_stores.len(), 1);
    assert_eq!(executor.local_store_replay.len(), 1);
    assert!(executor.local_validate_replay.is_empty());
    assert!(executor.local_proposal_ready_replay.is_empty());
    assert!(executor.pending_validations.is_empty());
    assert_eq!(services.store_tasks.len(), 8);
    let store_id = services.store_tasks[0].id();
    assert!(
        services
            .store_tasks
            .iter()
            .all(|task| task.id() == store_id)
    );
    assert!(
        !executor
            .runtime
            .completions
            .iter()
            .any(|completion| matches!(completion, RuntimeCompletion::LocalProposal(..)))
    );
    let store_completion = services.execute_store(store_id);
    let duplicate_store = store_completion.clone();
    executor
        .complete_body_store(store_completion, &mut services)
        .expect("durable completion starts validation");
    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_validations.len(), 1);
    assert!(executor.local_store_replay.is_empty());
    assert_eq!(executor.local_validate_replay.len(), 1);
    assert!(executor.local_proposal_ready_replay.is_empty());
    assert_eq!(services.validation_tasks.len(), 1);
    assert_eq!(
        executor
            .complete_body_store(duplicate_store, &mut services)
            .expect("duplicate durable completion"),
        CompletionDisposition::Stale
    );
    for _ in 0..8 {
        executor
            .admit_local_proposal(
                tag(0),
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("retry local validation");
    }
    assert_eq!(executor.pending_validations.len(), 1);
    let validation_id = services.validation_tasks[0].id();
    assert!(
        services
            .validation_tasks
            .iter()
            .all(|task| task.id() == validation_id)
    );
    let validation_completion = services.execute_validation(validation_id);
    let duplicate_validation = validation_completion.clone();
    executor
        .complete_body_validation(validation_completion, &mut services)
        .expect("validated completion starts proposal");
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::LocalProposal(completion_tag, manifest, durable, validated))
            if *completion_tag == tag(0)
                && manifest == &fixture.manifest
                && validated.durable() == durable
    ));
    assert!(executor.local_validate_replay.is_empty());
    assert_eq!(executor.local_proposal_ready_replay.len(), 1);
    assert_eq!(
        executor
            .complete_body_validation(duplicate_validation, &mut services)
            .expect("duplicate validation completion"),
        CompletionDisposition::Stale
    );
    assert_eq!(executor.local_proposal_ready_replay.len(), 1);
    assert!(!executor.status().fail_closed);
    let wire::ConsensusMessageV2Payload::Proposal(mut unsigned_proposal) =
        proposal(&fixture).payload
    else {
        unreachable!("proposal fixture carries a Proposal")
    };
    unsigned_proposal.signature.clear();
    let proposal_intent = AdapterEffect::Sign {
        tag: tag(0),
        request: SignRequest::Proposal(unsigned_proposal),
    };
    executor
        .runtime
        .steps
        .push_back(Ok(RuntimeStep::Advanced(vec![proposal_intent.clone()])));
    assert!(matches!(
        executor
            .step(Instant::now(), &mut services)
            .expect("exact local command emits its ProposalIntent"),
        EffectExecutorStep::Advanced { effects: 1 }
    ));
    assert!(executor.local_proposal_ready_replay.is_empty());
    assert_eq!(executor.local_proposal_intent_replay.len(), 1);
    assert_eq!(executor.pending_signatures.len(), 1);
    let (command_identity, replay) = executor
        .local_proposal_intent_replay
        .first_key_value()
        .expect("ProposalIntent retains the body companion authority");
    let pending_signature = executor
        .pending_signatures
        .values()
        .next()
        .expect("ProposalIntent created one exact Sign task");
    assert!(replay.exactly_matches_proposal_intent(
        *command_identity,
        &proposal_intent,
        &pending_signature.ownership,
    ));
    let stores_before_retry = services.store_tasks.len();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("retry stutters beside the exact retained ProposalIntent composite");
    assert_eq!(services.store_tasks.len(), stores_before_retry);
    let signatures_before_drop = executor.pending_signatures.len();
    let (_, replay) = executor
        .local_proposal_intent_replay
        .pop_first()
        .expect("test owns the inert ProposalIntent composite");
    drop(replay);
    assert_eq!(executor.pending_signatures.len(), signatures_before_drop);
    assert!(!executor.status().fail_closed);
}
#[test]
fn failed_lock_cleanup_keeps_exact_owner_and_requires_restart() {
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
        .expect("admit superseded body recovery");
    let before = executor.body_ownership_projection();
    let (replacement_subject, _) = distinct_body(&fixture);
    services.fail_on = Some("cancel-fetch");
    assert!(matches!(
        executor.reconcile_locked_body_for_recovery(
            tag(1),
            (round(&fixture.context, 0), replacement_subject),
            &mut services,
        ),
        Err(EffectExecutorError::Service(reason)) if reason.contains("cancel-fetch failed")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(executor.protected_lock, None);
    assert!(executor.output_guard.restart_required());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
    assert!(matches!(
        executor.reconcile_locked_body_for_recovery(
            tag(1),
            (round(&fixture.context, 0), replacement_subject),
            &mut services,
        ),
        Err(EffectExecutorError::FailClosed(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn lock_cleanup_rejects_inconsistent_certified_request_before_mutation() {
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
        .expect("admit certified body recovery");
    let request_hash = *executor
        .certified_work
        .keys()
        .next()
        .expect("certified request index");
    assert!(executor.outstanding_requests.cancel(request_hash));
    let before = executor.body_ownership_projection();
    let (replacement_subject, _) = distinct_body(&fixture);
    assert!(matches!(
        executor.reconcile_locked_body_for_recovery(
            tag(1),
            (round(&fixture.context, 0), replacement_subject),
            &mut services,
        ),
        Err(EffectExecutorError::Contract(_))
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert!(services.cancelled_fetches.is_empty());
    assert_eq!(executor.protected_lock, None);
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn lock_cleanup_status_failure_preserves_committed_replacement() {
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
        .expect("admit superseded certified recovery");
    let old_work_id = services.fetch_tasks[0].id();
    let (replacement_subject, _) = distinct_body(&fixture);
    let replacement = (round(&fixture.context, 0), replacement_subject);
    services.fail_on = Some("status");
    assert!(matches!(
        executor.reconcile_locked_body_for_recovery(
            tag(1),
            replacement,
            &mut services,
        ),
        Err(EffectExecutorError::Service(reason)) if reason.contains("status failed")
    ));
    assert_eq!(executor.protected_lock, Some(replacement));
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
    assert_eq!(services.cancelled_fetches, vec![old_work_id]);
    assert!(executor.output_guard.restart_required());
    assert_eq!(services.closed.len(), 1);
    assert!(matches!(
        executor.reconcile_locked_body_for_recovery(tag(1), replacement, &mut services,),
        Err(EffectExecutorError::FailClosed(_))
    ));
    assert_eq!(services.closed.len(), 1);
}
#[test]
fn missing_merge_sidecar_retains_exact_validation_until_retry() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
    let round = pending.task.round();
    let subject = pending.task.subject();
    let task =
        begin_reachable_merge_validation(&fixture, &mut executor, &mut services, round, subject);
    let work_id = task.id();
    let durable = task.durable_receipt().clone();
    let completion = BodyValidationCompletion::DeferredMergeSidecar {
        work_id,
        reference: reference.clone(),
    };
    assert_eq!(
        executor
            .complete_body_validation(completion.clone(), &mut services)
            .expect("defer validation for exact merge sidecar"),
        CompletionDisposition::Deferred
    );
    assert_eq!(executor.pending_validations.len(), 1);
    let status = executor.status();
    assert_eq!(status.deferred_merge_work, 1);
    assert_eq!(status.deferred_validation_merge_work, 1);
    assert_eq!(status.deferred_application_merge_work, 0);
    assert_eq!(
        services.deferred_merge_sidecars,
        vec![(work_id, round, subject, reference.clone())]
    );
    assert!(executor.runtime.completions.is_empty());
    assert!(services.rejected_validations.is_empty());
    assert_eq!(
        executor
            .complete_body_validation(completion, &mut services)
            .expect("duplicate deferral is idempotent"),
        CompletionDisposition::Deferred
    );
    assert_eq!(services.deferred_merge_sidecars.len(), 1);
    let unrelated_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"unrelated certified merge entry"));
    assert_eq!(
        executor
            .retry_deferred_merge_sidecar(unrelated_hash, &mut services)
            .expect("unrelated sidecar completion is ignored"),
        0
    );
    let status = executor.status();
    assert_eq!(status.deferred_merge_work, 1);
    assert_eq!(status.deferred_validation_merge_work, 1);
    assert_eq!(status.deferred_application_merge_work, 0);
    assert_eq!(
        executor
            .retry_deferred_merge_sidecar(entry_hash, &mut services)
            .expect("retry exact deferred validation"),
        1
    );
    assert_eq!(executor.status().deferred_merge_work, 0);
    assert_eq!(services.validation_tasks.last(), Some(&task));
    assert_eq!(executor.pending_validations.len(), 1);
    assert_eq!(
        executor
            .complete_body_validation(
                BodyValidationCompletion::Validated {
                    work_id,
                    receipt: ValidatedBodyReceipt::for_test(durable),
                },
                &mut services,
            )
            .expect("complete exact retried validation"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_validations.is_empty());
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::ValidationSucceeded(
            completion_tag,
            completion_round,
            completion_subject,
            _
        )) if *completion_tag == tag(3)
            && *completion_round == round
            && *completion_subject == subject
    ));
}
#[test]
fn uniquely_invalid_merge_sidecar_terminally_rejects_exact_deferred_work() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
    let round = pending.task.round();
    let subject = pending.task.subject();
    let work_id =
        begin_reachable_merge_validation(&fixture, &mut executor, &mut services, round, subject)
            .id();
    executor
        .complete_body_validation(
            BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
            &mut services,
        )
        .expect("defer validation");
    let unrelated_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"unrelated certified merge entry"));
    assert_eq!(
        executor
            .reject_deferred_merge_sidecar(unrelated_hash, "invalid unrelated entry", &mut services)
            .expect("ignore unrelated rejection"),
        0
    );
    assert_eq!(executor.pending_validations.len(), 1);
    assert_eq!(
        executor
            .reject_deferred_merge_sidecar(entry_hash, "invalid certified entry", &mut services)
            .expect("reject exact deferred entry"),
        1
    );
    assert!(executor.pending_validations.is_empty());
    assert_eq!(executor.status().deferred_merge_work, 0);
    assert_eq!(
        services.rejected_validations,
        vec!["invalid certified entry".to_owned()]
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::ValidationFailed(
            completion_tag,
            completion_round,
            completion_subject
        )) if *completion_tag == tag(3)
            && *completion_round == round
            && *completion_subject == subject
    ));
}
#[test]
fn conflicting_reference_registration_rejects_only_its_exact_work_id() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (first, first_reference, entry_hash) = pending_merge_validation(&fixture);
    let first_round = first.task.round();
    let first_subject = first.task.subject();
    let second_subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"conflicting second carrier")),
        ..first_subject
    };
    let mut second_reference = first_reference.clone();
    second_reference.encoded_len += 1;
    let retry_reference = first_reference.clone();
    let first_id = begin_reachable_merge_validation(
        &fixture,
        &mut executor,
        &mut services,
        first_round,
        first_subject,
    )
    .id();
    let second_id = begin_reachable_merge_validation(
        &fixture,
        &mut executor,
        &mut services,
        first_round,
        second_subject,
    )
    .id();
    for (work_id, reference) in [(first_id, first_reference), (second_id, second_reference)] {
        executor
            .complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
                &mut services,
            )
            .expect("retain independently keyed deferral");
    }
    assert_eq!(executor.status().deferred_merge_work, 2);
    assert_eq!(
        executor
            .reject_deferred_merge_sidecar_work(
                second_id,
                "conflicting compact reference metadata",
                &mut services,
            )
            .expect("reject only conflicting registration"),
        CompletionDisposition::Accepted
    );
    assert!(!executor.pending_validations.contains_key(&second_id));
    assert!(executor.pending_validations.contains_key(&first_id));
    assert_eq!(
        executor.deferred_merge_work.get(&first_id),
        Some(&entry_hash)
    );
    let status = executor.status();
    assert_eq!(status.deferred_merge_work, 1);
    assert_eq!(status.deferred_validation_merge_work, 1);
    assert_eq!(status.deferred_application_merge_work, 0);
    // A multi-waiter retry is transactional with respect to executor
    // ownership. The first external enqueue may acknowledge before the
    // second fails, but no deferred entry or pending task is committed
    // away until every callback succeeds.
    let third_subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"retry third carrier")),
        ..first_subject
    };
    let third_id = begin_reachable_merge_validation(
        &fixture,
        &mut executor,
        &mut services,
        first_round,
        third_subject,
    )
    .id();
    executor
        .complete_body_validation(
            BodyValidationCompletion::DeferredMergeSidecar {
                work_id: third_id,
                reference: retry_reference,
            },
            &mut services,
        )
        .expect("retain a second reachable retry waiter");
    let before = executor.body_ownership_projection();
    let validation_tasks_before = services.validation_tasks.len();
    let validation_calls = services
        .operation_calls
        .get("validation")
        .copied()
        .expect("production validation admissions were counted");
    services.fail_on_call = Some(("validation", validation_calls + 2));
    assert!(matches!(
        executor.retry_deferred_merge_sidecar(entry_hash, &mut services),
        Err(EffectExecutorError::Service(reason))
            if reason.contains("validation call")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.validation_tasks.len(), validation_tasks_before + 1);
    assert!(executor.deferred_merge_work.contains_key(&first_id));
    assert!(executor.deferred_merge_work.contains_key(&third_id));
    assert!(executor.status().fail_closed);
    // Terminal rejection preflights the complete matching set before the
    // first waiter is completed. A corrupt later owner therefore cannot
    // allow an earlier validation rejection or ownership removal.
    {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
        let round = pending.task.round();
        let first_subject = pending.task.subject();
        let second_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"corrupt later waiter")),
            ..first_subject
        };
        let first_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            first_subject,
        )
        .id();
        let second_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            second_subject,
        )
        .id();
        for work_id in [first_id, second_id] {
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id,
                        reference: reference.clone(),
                    },
                    &mut services,
                )
                .expect("retain each reachable rejection waiter");
        }
        executor
            .body_pipeline_owners
            .get_mut(&(round, second_subject))
            .expect("second exact validation owner")
            .tag = EventTag::new(1, round.view, Generation::new(8));
        let before = executor.body_ownership_projection();
        assert!(matches!(
            executor.reject_deferred_merge_sidecar(
                entry_hash,
                "invalid shared merge entry",
                &mut services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("immutable pipeline owner")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(services.rejected_validations.is_empty());
        assert!(executor.runtime.completions.is_empty());
    }
    // Runtime failure is one atomic batch failure, so a later admission
    // cannot leave an earlier ValidationFailed completion visible while
    // every executor waiter is still retained.
    {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
        let round = pending.task.round();
        let first_subject = pending.task.subject();
        let second_subject = wire::BlockSubject {
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"runtime second waiter")),
            ..first_subject
        };
        let first_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            first_subject,
        )
        .id();
        let second_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            second_subject,
        )
        .id();
        for work_id in [first_id, second_id] {
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar {
                        work_id,
                        reference: reference.clone(),
                    },
                    &mut services,
                )
                .expect("retain each atomic rejection waiter");
        }
        let before = executor.body_ownership_projection();
        executor.runtime.fail_enqueue = true;
        assert!(
            executor
                .reject_deferred_merge_sidecar(
                    entry_hash,
                    "invalid shared merge entry",
                    &mut services,
                )
                .is_err()
        );
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.runtime.fail_enqueue_hits, 1);
        assert!(services.rejected_validations.is_empty());
    }
}
