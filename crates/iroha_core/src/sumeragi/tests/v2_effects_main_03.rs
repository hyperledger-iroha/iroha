#[test]
fn decided_apply_retries_after_exact_merge_sidecar_recovery() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (pending_validation, reference, entry_hash) = pending_merge_validation(&fixture);
    let mut certificate = fixture.qc(wire::GlobalPhase::Commit);
    certificate.round = pending_validation.task.round();
    certificate.proposal_round = pending_validation.task.round();
    certificate.subject = pending_validation.task.subject();
    let validated_receipt =
        ValidatedBodyReceipt::for_test(pending_validation.task.durable_receipt().clone());
    certificate.execution_commitment = validated_receipt.execution_commitment();
    let manifest = canonical_payload_manifest(
        &fixture.context,
        certificate.proposal_round,
        certificate.subject,
        &fixture.body,
    );
    executor.recovered_bodies.insert(
        (certificate.proposal_round, certificate.subject),
        (manifest, validated_receipt.durable().clone()),
    );
    executor.durable_bodies.insert(
        (certificate.proposal_round, certificate.subject),
        validated_receipt.durable().clone(),
    );
    executor.validated_bodies.insert(
        (certificate.proposal_round, certificate.subject),
        validated_receipt,
    );
    executor.runtime.round_tag = Some(tag(3));
    executor
        .begin_apply(
            tag(3),
            certificate.subject,
            certificate,
            RuntimeEffectOwnership::fresh_for_test(tag(3), 3),
            &mut services,
        )
        .expect("start Apply through the production admission path");
    let task = services.apply_tasks.pop().expect("production Apply task");
    let work_id = task.id();

    assert_eq!(
        executor
            .defer_application_for_merge_sidecar(work_id, &reference, &mut services)
            .expect("defer decided apply"),
        CompletionDisposition::Deferred
    );
    let status = executor.status();
    assert_eq!(status.deferred_merge_work, 1);
    assert_eq!(status.deferred_validation_merge_work, 0);
    assert_eq!(status.deferred_application_merge_work, 1);
    assert!(services.apply_tasks.is_empty());
    assert_eq!(
        executor
            .retry_deferred_merge_sidecar(entry_hash, &mut services)
            .expect("retry decided apply after sidecar persistence"),
        1
    );
    assert_eq!(
        services.apply_tasks.last().map(ApplyTask::id),
        Some(work_id)
    );
    assert_eq!(executor.status().deferred_merge_work, 0);
    assert!(executor.pending_applications.contains_key(&work_id));

    // Application deferral is also an ownership boundary. An internally
    // inconsistent decided task must fail before sidecar registration or
    // a recovery callback can treat it as legitimate pending work.
    executor
        .pending_applications
        .get_mut(&work_id)
        .expect("retained exact Apply owner")
        .task
        .certificate
        .subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"corrupt deferred apply")),
        ..task.subject
    };
    let deferred_callbacks = services.deferred_merge_sidecars.len();

    assert!(matches!(
        executor.defer_application_for_merge_sidecar(work_id, &reference, &mut services,),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("exact decided-body owner")
    ));
    assert!(executor.pending_applications.contains_key(&work_id));
    assert!(executor.deferred_merge_work.is_empty());
    assert_eq!(services.deferred_merge_sidecars.len(), deferred_callbacks);

    let pending = executor
        .pending_applications
        .get_mut(&work_id)
        .expect("retained Apply remains available for ordinal corruption");
    pending.task.certificate = task.certificate.clone();
    pending.task.lifecycle_ordinal = pending.task.lifecycle_ordinal.saturating_add(1);
    assert!(matches!(
        executor.defer_application_for_merge_sidecar(work_id, &reference, &mut services,),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("exact decided-body owner")
    ));
    assert!(executor.deferred_merge_work.is_empty());
    assert_eq!(services.deferred_merge_sidecars.len(), deferred_callbacks);
}

#[test]
fn deferred_merge_sidecar_accepts_earlier_carrier_and_rejects_future_or_foreign() {
    {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, mut reference, _) = pending_merge_validation(&fixture);
        let round = pending.task.round();
        let subject = pending.task.subject();
        let work_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            subject,
        )
        .id();
        reference.merge_qc.view = round.view.saturating_sub(1);

        assert_eq!(
            executor
                .complete_body_validation(
                    BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
                    &mut services,
                )
                .expect("retain a later-round validation for its immutable carrier"),
            CompletionDisposition::Deferred
        );
        assert_eq!(executor.status().deferred_merge_work, 1);
        assert!(!executor.status().fail_closed);
        assert_eq!(services.deferred_merge_sidecars.len(), 1);
    }

    for mismatch in 0..3 {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, mut reference, _) = pending_merge_validation(&fixture);
        let round = pending.task.round();
        let subject = pending.task.subject();
        let work_id = begin_reachable_merge_validation(
            &fixture,
            &mut executor,
            &mut services,
            round,
            subject,
        )
        .id();
        match mismatch {
            0 => {
                reference.merge_qc.carrier_height =
                    reference.merge_qc.carrier_height.saturating_add(1);
            }
            1 => {
                reference.merge_qc.carrier_parent_hash =
                    HashOf::from_untyped_unchecked(Hash::new(b"different merge carrier parent"));
            }
            2 => {
                reference.merge_qc.view = round.view.saturating_add(1);
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            executor.complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
                &mut services,
            ),
            Err(EffectExecutorError::BodyStore(_))
        ));
        assert!(executor.status().fail_closed);
        assert_eq!(executor.pending_validations.len(), 1);
        assert_eq!(executor.status().deferred_merge_work, 0);
        assert!(services.deferred_merge_sidecars.is_empty());
        assert!(executor.runtime.completions.is_empty());
    }
}

#[test]
fn certified_view_prunes_unprotected_merge_sidecar_work_but_keeps_high_qc_subject() {
    for protected in [false, true] {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
        let work_id = pending.task.id();
        let subject = pending.task.subject();
        let round = pending.task.round();
        let durable = pending.task.durable_receipt().clone();
        let manifest = canonical_payload_manifest(&fixture.context, round, subject, &fixture.body);
        executor
            .recovered_bodies
            .insert((round, subject), (manifest, durable.clone()));
        executor.durable_bodies.insert((round, subject), durable);
        executor.body_pipeline_owners.insert(
            (round, subject),
            BodyPipelineOwner {
                tag: tag(round.view),
                manifest_hash: Some(pending.task.durable_receipt().manifest_hash()),
            },
        );
        executor.pending_validations.insert(work_id, pending);
        executor
            .complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
                &mut services,
            )
            .expect("defer exact prior-view work");

        let mut timeout = timeout_at_view(&fixture, round.view);
        let protected_lock = protected.then(|| {
            let mut highest = fixture.qc(wire::GlobalPhase::Prepare);
            highest.round = round;
            highest.proposal_round = round;
            highest.subject = subject;
            highest
        });
        timeout.groups[0].highest_prepare_qc = protected_lock.clone();
        executor
            .install_view(tag(round.view + 1), timeout, protected_lock, &mut services)
            .expect("install certified next view");

        assert_eq!(
            executor.retains_deferred_merge_sidecar(work_id, round, subject, entry_hash),
            protected
        );
        assert_eq!(
            executor.pending_validations.contains_key(&work_id),
            protected
        );
        assert_eq!(
            executor.status().deferred_merge_work,
            if protected { 1 } else { 0 }
        );
        assert!(
            !executor
                .body_pipeline_owners
                .contains_key(&(round, subject))
        );
        if protected {
            assert!(executor.pending_validations[&work_id].consumer.is_none());
        }
        assert!(
            services.cancelled_validations.is_empty(),
            "a completed sidecar-deferred validation has no live I/O owner to cancel"
        );
    }
}

#[test]
fn certified_view_protects_only_the_exact_high_qc_round_for_one_subject() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (first, reference, entry_hash) = pending_merge_validation(&fixture);
    let subject = first.task.subject();
    let first_round = first.task.round();
    let second_round = round(&fixture.context, first_round.view + 1);
    let first_id = begin_reachable_merge_validation(
        &fixture,
        &mut executor,
        &mut services,
        first_round,
        subject,
    )
    .id();
    let second_id = begin_reachable_merge_validation(
        &fixture,
        &mut executor,
        &mut services,
        second_round,
        subject,
    )
    .id();

    let mut second_reference = reference.clone();
    second_reference.merge_qc.view = second_round.view;
    for (work_id, reference) in [(first_id, reference), (second_id, second_reference)] {
        executor
            .complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
                &mut services,
            )
            .expect("defer same-subject validation round");
    }

    let mut timeout = timeout_at_view(&fixture, second_round.view);
    let mut highest = fixture.qc(wire::GlobalPhase::Prepare);
    highest.round = second_round;
    highest.proposal_round = second_round;
    highest.subject = subject;
    timeout.groups[0].highest_prepare_qc = Some(highest.clone());
    executor
        .install_view(
            tag(second_round.view + 1),
            timeout,
            Some(highest),
            &mut services,
        )
        .expect("install certified view with exact high PrepareQC");

    assert!(!executor.retains_deferred_merge_sidecar(first_id, first_round, subject, entry_hash));
    assert!(executor.retains_deferred_merge_sidecar(second_id, second_round, subject, entry_hash));
    assert_eq!(executor.status().deferred_merge_work, 1);
    assert!(executor.pending_validations[&second_id].consumer.is_none());
}

#[test]
fn protected_deferred_validation_retags_consumer_across_view_churn_before_retry() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let (pending, reference, entry_hash) = pending_merge_validation(&fixture);
    let work_id = pending.task.id();
    let round = pending.task.round();
    let subject = pending.task.subject();
    let durable = pending.task.durable_receipt().clone();
    let manifest = canonical_payload_manifest(&fixture.context, round, subject, &fixture.body);
    executor
        .recovered_bodies
        .insert((round, subject), (manifest.clone(), durable.clone()));
    executor
        .durable_bodies
        .insert((round, subject), durable.clone());
    executor.body_pipeline_owners.insert(
        (round, subject),
        BodyPipelineOwner {
            tag: tag(round.view),
            manifest_hash: Some(durable.manifest_hash()),
        },
    );
    executor.pending_validations.insert(work_id, pending);
    assert_eq!(
        executor
            .complete_body_validation(
                BodyValidationCompletion::DeferredMergeSidecar { work_id, reference },
                &mut services,
            )
            .expect("defer protected validation"),
        CompletionDisposition::Deferred
    );

    let mut high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    high_prepare.round = round;
    high_prepare.proposal_round = round;
    high_prepare.subject = subject;
    let sources = certified_sources(&fixture, &high_prepare);

    for entering_view in [round.view + 1, round.view + 2] {
        let mut timeout = timeout_at_view(&fixture, entering_view - 1);
        timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
        executor
            .consume_effects(
                vec![
                    AdapterEffect::EnterView {
                        tag: tag(entering_view),
                        certificate: timeout,
                        protected_lock: Some(high_prepare.clone()),
                    },
                    AdapterEffect::FetchBody {
                        tag: tag(entering_view),
                        round,
                        subject,
                        manifest: Some(manifest.clone()),
                        certified_sources: sources.clone(),
                        certificate: Some(high_prepare.clone()),
                    },
                ],
                &mut services,
            )
            .expect("replay protected body in current view");
        assert!(executor.pending_validations[&work_id].consumer.is_none());

        executor
            .consume_effects(
                vec![
                    AdapterEffect::StoreBody {
                        tag: tag(entering_view),
                        round,
                        subject,
                    },
                    AdapterEffect::ValidateBody {
                        tag: tag(entering_view),
                        round,
                        subject,
                    },
                ],
                &mut services,
            )
            .expect("adopt deferred validation in current view");
        assert!(matches!(
            &executor.pending_validations[&work_id].consumer,
            Some(ValidationConsumer::Reducer { tag: consumer, .. })
                if *consumer == tag(entering_view)
        ));
        assert_eq!(
            executor.deferred_merge_work.get(&work_id),
            Some(&entry_hash)
        );
    }

    assert_eq!(
        executor
            .retry_deferred_merge_sidecar(entry_hash, &mut services)
            .expect("retry protected validation after sidecar recovery"),
        1
    );
    assert_eq!(services.validation_tasks.len(), 1);
    assert_eq!(services.validation_tasks[0].id(), work_id);
    assert_eq!(
        executor
            .complete_body_validation(
                BodyValidationCompletion::Validated {
                    work_id,
                    receipt: ValidatedBodyReceipt::for_test(durable),
                },
                &mut services,
            )
            .expect("route retried validation to latest consumer"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::ValidationSucceeded(
            completion_tag,
            completion_round,
            completion_subject,
            _
        )) if *completion_tag == tag(round.view + 2)
            && *completion_round == round
            && *completion_subject == subject
    ));
    assert!(executor.pending_validations.is_empty());
    assert!(executor.deferred_merge_work.is_empty());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn certified_view_rebinds_inflight_high_qc_validation_through_current_fifo() {
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
        .expect("admit old-view body");
    let store_id = services.store_tasks[0].id();
    let stored = services.execute_store(store_id);
    executor
        .complete_body_store(stored, &mut services)
        .expect("start old-view validation");
    let work_id = services.validation_tasks[0].id();

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor
        .consume_effects(
            vec![
                AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout,
                    protected_lock: Some(prepare.clone()),
                },
                AdapterEffect::FetchBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                },
            ],
            &mut services,
        )
        .expect("install view and replay locked-body acquisition");

    assert_eq!(executor.pending_validations.len(), 1);
    assert!(executor.pending_validations[&work_id].consumer.is_none());
    assert_eq!(
        executor
            .body_pipeline_owners
            .get(&(fixture.manifest.round, fixture.manifest.subject))
            .map(|owner| owner.tag),
        Some(tag(1))
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
            if *completion_tag == tag(1) && manifest == &fixture.manifest
    ));

    executor
        .consume_effects(
            vec![
                AdapterEffect::StoreBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                },
                AdapterEffect::ValidateBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                },
            ],
            &mut services,
        )
        .expect("current view adopts retained immutable validation");
    assert!(matches!(
        &executor.pending_validations[&work_id].consumer,
        Some(ValidationConsumer::Reducer { tag: consumer, .. })
            if *consumer == tag(1)
    ));
    assert_eq!(services.validation_tasks.len(), 2);
    assert!(
        services
            .validation_tasks
            .iter()
            .all(|task| task.id() == work_id)
    );

    let completed = services.execute_validation(work_id);
    assert_eq!(
        executor
            .complete_body_validation(completed, &mut services)
            .expect("route retained completion to current consumer"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::ValidationSucceeded(
            completion_tag,
            completion_round,
            completion_subject,
            _
        )) if *completion_tag == tag(1)
            && *completion_round == fixture.manifest.round
            && *completion_subject == fixture.manifest.subject
    ));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn detached_validation_outcomes_replay_only_after_current_consumer_attaches() {
    for reject in [false, true] {
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
            .expect("admit old-view body");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("start old-view validation");
        let work_id = services.validation_tasks[0].id();

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let mut timeout = timeout_certificate(&fixture);
        timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout,
                    protected_lock: Some(prepare.clone()),
                }],
                &mut services,
            )
            .expect("detach protected validation");
        assert!(executor.pending_validations[&work_id].consumer.is_none());

        executor.runtime.completions.clear();
        if reject {
            services.validation_error = Some("detached rejection".to_owned());
        }
        let completion = services.execute_validation(work_id);
        assert_eq!(
            executor
                .complete_body_validation(completion, &mut services)
                .expect("cache detached terminal outcome"),
            CompletionDisposition::Accepted
        );
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.pending_validations.is_empty());

        let sources = certified_sources(&fixture, &prepare);
        executor
            .consume_effects(
                vec![
                    AdapterEffect::FetchBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                        manifest: Some(fixture.manifest.clone()),
                        certified_sources: sources,
                        certificate: Some(prepare),
                    },
                    AdapterEffect::StoreBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    },
                    AdapterEffect::ValidateBody {
                        tag: tag(1),
                        round: fixture.manifest.round,
                        subject: fixture.manifest.subject,
                    },
                ],
                &mut services,
            )
            .expect("replay cached outcome through current FIFO");
        if reject {
            assert!(matches!(
                executor.runtime.completions.last(),
                Some(RuntimeCompletion::ValidationFailed(
                    completion_tag,
                    completion_round,
                    completion_subject
                )) if *completion_tag == tag(1)
                    && *completion_round == fixture.manifest.round
                    && *completion_subject == fixture.manifest.subject
            ));
            assert!(
                executor
                    .rejected_bodies
                    .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
            );
        } else {
            assert!(matches!(
                executor.runtime.completions.last(),
                Some(RuntimeCompletion::ValidationSucceeded(
                    completion_tag,
                    completion_round,
                    completion_subject,
                    _
                )) if *completion_tag == tag(1)
                    && *completion_round == fixture.manifest.round
                    && *completion_subject == fixture.manifest.subject
            ));
        }
        assert_eq!(services.validation_tasks.len(), 1);
        assert!(!executor.status().fail_closed);
    }
}

#[test]
fn contradictory_terminal_validation_catalogues_fail_closed() {
    for conflicting_receipt in [false, true] {
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
            .expect("start exact-body pipeline");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("start deterministic validation");
        let work_id = services.validation_tasks[0].id();
        let durable = executor.pending_validations[&work_id]
            .task
            .durable_receipt()
            .clone();

        let error = if conflicting_receipt {
            let first = services.execute_validation(work_id);
            let first_receipt = first
                .validated_receipt()
                .expect("validated completion")
                .clone();
            executor
                .complete_body_validation(first, &mut services)
                .expect("record first validation receipt");
            let conflicting = ValidatedBodyReceipt::for_test(durable);
            assert_ne!(conflicting, first_receipt);
            executor
                .complete_body_validation(
                    BodyValidationCompletion::Validated {
                        work_id,
                        receipt: conflicting,
                    },
                    &mut services,
                )
                .expect_err("conflicting validation receipts must fail closed")
        } else {
            executor
                .complete_body_validation(
                    BodyValidationCompletion::Rejected {
                        work_id,
                        reason: "deterministic rejection".to_owned(),
                    },
                    &mut services,
                )
                .expect("record deterministic rejection");
            executor
                .complete_body_validation(
                    BodyValidationCompletion::Validated {
                        work_id,
                        receipt: ValidatedBodyReceipt::for_test(durable),
                    },
                    &mut services,
                )
                .expect_err("validated and rejected outcomes must fail closed")
        };

        assert!(matches!(
            error,
            EffectExecutorError::Contract(_) | EffectExecutorError::BodyStore(_)
        ));
        assert!(executor.status().fail_closed);
        assert_eq!(services.closed.len(), 1);
    }
}

#[test]
fn queued_protected_store_keeps_one_work_id_across_repeated_tcs() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            EventTag::new(1, 0, Generation::new(60)),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("queue the exact locked-body store");
    let original_task = services.store_tasks[0].clone();
    let protected = (fixture.manifest.round, fixture.manifest.subject);
    let mut high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    high_prepare.round = protected.0;
    high_prepare.subject = protected.1;

    for (view, generation) in [(1, 61), (2, 62)] {
        let mut timeout = timeout_at_view(&fixture, view - 1);
        timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, view, Generation::new(generation)),
                    certificate: timeout,
                    protected_lock: Some(high_prepare.clone()),
                }],
                &mut services,
            )
            .expect("preserve queued protected storage across the TC");
        assert_eq!(executor.pending_stores.len(), 1);
        assert_eq!(
            executor.pending_stores[&original_task.id()].task,
            original_task
        );
        assert!(
            executor.pending_stores[&original_task.id()]
                .consumer
                .is_none()
        );
        assert!(services.cancelled_stores.is_empty());
        assert_eq!(services.store_tasks.len(), 1);
    }

    let current_tag = EventTag::new(1, 2, Generation::new(62));
    let sources = certified_sources(&fixture, &high_prepare);
    executor
        .consume_effects(
            vec![
                AdapterEffect::FetchBody {
                    tag: current_tag,
                    round: protected.0,
                    subject: protected.1,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(high_prepare),
                },
                AdapterEffect::StoreBody {
                    tag: current_tag,
                    round: protected.0,
                    subject: protected.1,
                },
            ],
            &mut services,
        )
        .expect("the current reducer consumer adopts the immutable queued store");
    assert_eq!(services.store_tasks.len(), 1);
    assert!(matches!(
        &executor.pending_stores[&original_task.id()].consumer,
        Some(StoreConsumer::Reducer { tag: consumer, .. }) if *consumer == current_tag
    ));

    let completion = services.execute_store(original_task.id());
    assert_eq!(completion.tag(), original_task.tag());
    assert_eq!(
        executor
            .complete_body_store(completion, &mut services)
            .expect("the original immutable task routes to the latest consumer"),
        CompletionDisposition::Accepted
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyStored(completion_tag, _, _, _))
            if *completion_tag == current_tag
    ));
    assert_eq!(executor.pending_store_bytes, 0);
}

#[test]
fn active_old_view_store_rebinds_current_consumer_before_late_completion() {
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
        .expect("start old-view body store");
    let store_id = services.store_tasks[0].id();
    services.inflight_stores.insert(store_id);

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout,
                protected_lock: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("detach active old-view store consumer");

    assert!(
        services.cancelled_stores.is_empty(),
        "the effective durable lock owns immutable store work across the TC"
    );
    assert_eq!(executor.pending_stores.len(), 1);
    assert!(executor.pending_stores[&store_id].consumer.is_none());
    assert_eq!(
        executor.pending_store_bytes,
        u64::try_from(fixture.body.len()).expect("body length")
    );
    assert!(executor.body_pipeline_owners.is_empty());

    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(1),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources,
                certificate: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("current view adopts retained store through FetchBody");
    assert!(services.fetch_tasks.is_empty());
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
            if *completion_tag == tag(1) && manifest == &fixture.manifest
    ));
    assert_eq!(
        executor
            .body_pipeline_owners
            .get(&(fixture.manifest.round, fixture.manifest.subject))
            .map(|owner| owner.tag),
        Some(tag(1))
    );

    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: tag(1),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("attach current reducer consumer without duplicate I/O");
    assert!(matches!(
        &executor.pending_stores[&store_id].consumer,
        Some(StoreConsumer::Reducer { tag: consumer, .. }) if *consumer == tag(1)
    ));
    assert_eq!(services.store_tasks.len(), 1);

    let late_completion = services.execute_store(store_id);
    assert_eq!(late_completion.tag(), tag(0));
    assert_eq!(
        executor
            .complete_body_store(late_completion, &mut services)
            .expect("route late immutable completion to current consumer"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_store_bytes, 0);
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyStored(
            completion_tag,
            completion_round,
            completion_subject,
            _
        )) if *completion_tag == tag(1)
            && *completion_round == fixture.manifest.round
            && *completion_subject == fixture.manifest.subject
    ));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn active_old_view_store_completes_between_current_fetch_and_store() {
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
        .expect("start old-view body store");
    let store_id = services.store_tasks[0].id();
    services.inflight_stores.insert(store_id);

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout,
                protected_lock: Some(prepare.clone()),
            }],
            &mut services,
        )
        .expect("detach active old-view store consumer");
    assert!(executor.pending_stores[&store_id].consumer.is_none());

    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(1),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("current FetchBody adopts detached store");
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
            if *completion_tag == tag(1) && manifest == &fixture.manifest
    ));
    assert!(executor.pending_stores[&store_id].consumer.is_none());
    assert_eq!(services.store_tasks.len(), 1);

    let late_completion = services.execute_store(store_id);
    assert_eq!(late_completion.tag(), tag(0));
    let expected_receipt = late_completion.receipt().clone();
    assert_eq!(
        executor
            .complete_body_store(late_completion, &mut services)
            .expect("catalog detached store before current StoreBody"),
        CompletionDisposition::Accepted
    );
    let key = (fixture.manifest.round, fixture.manifest.subject);
    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_store_bytes, 0);
    assert_eq!(executor.durable_bodies.get(&key), Some(&expected_receipt));
    assert_eq!(
        executor.recovered_bodies.get(&key),
        Some(&(fixture.manifest.clone(), expected_receipt.clone()))
    );
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
            if *completion_tag == tag(1) && manifest == &fixture.manifest
    ));

    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: tag(1),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("current StoreBody uses catalogued durable receipt");
    assert_eq!(services.store_tasks.len(), 1);
    assert!(executor.pending_stores.is_empty());
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyStored(
            completion_tag,
            completion_round,
            completion_subject,
            receipt
        )) if *completion_tag == tag(1)
            && *completion_round == fixture.manifest.round
            && *completion_subject == fixture.manifest.subject
            && receipt == &expected_receipt
    ));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn retired_store_and_current_fetch_completion_are_order_independent() {
    for store_finishes_first in [true, false] {
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
            .expect("start old-view body store");
        let store_id = services.store_tasks[0].id();
        services.inflight_stores.insert(store_id);
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout_certificate(&fixture),
                    protected_lock: None,
                }],
                &mut services,
            )
            .expect("retire unprotected active store consumer");
        assert!(executor.pending_stores.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());

        let prepare = fixture.qc(wire::GlobalPhase::Prepare);
        let sources = certified_sources(&fixture, &prepare);
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                    manifest: Some(fixture.manifest.clone()),
                    certified_sources: sources,
                    certificate: Some(prepare),
                }],
                &mut services,
            )
            .expect("start current fetch without a detached store");
        let fetch_task = services.fetch_tasks.last().expect("current fetch").clone();
        let request = fetch_task
            .certified_request()
            .expect("signed certified request")
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

        let late_completion = services.execute_store(store_id);
        let durable = late_completion.receipt().clone();
        if store_finishes_first {
            assert_eq!(
                executor
                    .complete_body_store(late_completion.clone(), &mut services)
                    .expect("catalog old store while current fetch is pending"),
                CompletionDisposition::Stale
            );
        }

        assert_eq!(
            executor
                .accept_certified_body_response(
                    response,
                    &fixture.context.roster[0].validator,
                    &mut services,
                )
                .expect("matching durable or empty state accepts current response"),
            CompletionDisposition::Accepted
        );
        let key = (fixture.manifest.round, fixture.manifest.subject);
        if store_finishes_first {
            assert!(executor.ready_bodies.is_empty());
            assert_eq!(executor.ready_body_bytes, 0);
        } else {
            assert_eq!(executor.ready_bodies.len(), 1);
            assert_eq!(
                executor
                    .complete_body_store(late_completion, &mut services)
                    .expect("catalog old store after current fetch completion"),
                CompletionDisposition::Stale
            );
            assert_eq!(executor.ready_bodies.len(), 1);
        }
        assert_eq!(executor.durable_bodies.get(&key), Some(&durable));
        assert_eq!(
            executor.recovered_bodies.get(&key),
            Some(&(fixture.manifest.clone(), durable.clone()))
        );
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
                if *completion_tag == tag(1) && manifest == &fixture.manifest
        ));

        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(1),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("current StoreBody reuses exact durable receipt");
        assert_eq!(services.store_tasks.len(), 1);
        assert!(executor.ready_bodies.is_empty());
        assert_eq!(executor.ready_body_bytes, 0);
        assert!(matches!(
            executor.runtime.completions.last(),
            Some(RuntimeCompletion::BodyStored(
                completion_tag,
                completion_round,
                completion_subject,
                receipt
            )) if *completion_tag == tag(1)
                && *completion_round == fixture.manifest.round
                && *completion_subject == fixture.manifest.subject
                && receipt == &durable
        ));
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.certified_work.is_empty());
        assert!(executor.outstanding_requests.is_empty());
        assert_eq!(services.completed_certified_fetches, vec![fetch_task.id()]);
        assert!(!executor.status().fail_closed);
        assert!(services.closed.is_empty());
    }
}

#[test]
fn current_fetch_fails_closed_on_conflicting_retired_store_receipt() {
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
    assert_ne!(alternate_manifest, fixture.manifest);
    executor
        .admit_local_proposal(
            tag(0),
            alternate_manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start old-view alternate store");
    let store_id = services.store_tasks[0].id();
    services.inflight_stores.insert(store_id);
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_certificate(&fixture),
                protected_lock: None,
            }],
            &mut services,
        )
        .expect("retire unprotected alternate store");

    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(1),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources,
                certificate: Some(prepare),
            }],
            &mut services,
        )
        .expect("start current canonical fetch");
    let request = services
        .fetch_tasks
        .last()
        .and_then(BodyFetchTask::certified_request)
        .expect("signed certified request")
        .clone();
    let late_completion = services.execute_store(store_id);
    let alternate_receipt = late_completion.receipt().clone();
    assert_eq!(
        executor
            .complete_body_store(late_completion, &mut services)
            .expect("catalog retired alternate store"),
        CompletionDisposition::Stale
    );

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
    assert!(matches!(
        executor.accept_certified_body_response(
            response,
            &fixture.context.roster[0].validator,
            &mut services,
        ),
        Err(EffectTransportError::FailClosed(reason))
            if reason.contains("retained durable body identity")
    ));
    let key = (fixture.manifest.round, fixture.manifest.subject);
    assert_eq!(
        executor.recovered_bodies.get(&key),
        Some(&(alternate_manifest, alternate_receipt.clone()))
    );
    assert_eq!(executor.durable_bodies.get(&key), Some(&alternate_receipt));
    assert!(executor.ready_bodies.is_empty());
    assert!(executor.runtime.completions.is_empty());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
}

#[test]
fn matching_ready_body_winner_makes_fetch_completion_idempotent() {
    let fixture = Fixture::new();
    let body_len = u64::try_from(fixture.body.len()).expect("body length");
    let mut executor = fixture.executor(EffectQueueConfig::new(8, 1, body_len, 4));
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
        .expect("start one exact fetch");
    let task = services.fetch_tasks[0].clone();
    let key = (fixture.manifest.round, fixture.manifest.subject);
    let ready = ReadyBody::derive(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        fixture.body.clone(),
    )
    .expect("derive exact ready body");
    assert_eq!(ready.manifest, fixture.manifest);
    executor.ready_bodies.insert(key, ready);
    executor.ready_body_bytes = body_len;

    assert_eq!(
        executor
            .complete_body_reconstruction(
                &task,
                fixture.manifest.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("matching ready winner is idempotent at full capacity"),
        CompletionDisposition::Accepted
    );
    assert_eq!(executor.ready_bodies.len(), 1);
    assert_eq!(executor.ready_body_bytes, body_len);
    assert!(executor.pending_fetches.is_empty());
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
            if *completion_tag == tag(0) && manifest == &fixture.manifest
    ));
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn late_retired_store_cannot_overwrite_current_pending_manifest() {
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
        .expect("start old-view body store");
    let retired_id = services.store_tasks[0].id();
    services.inflight_stores.insert(retired_id);
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_certificate(&fixture),
                protected_lock: None,
            }],
            &mut services,
        )
        .expect("retire unprotected active store consumer");
    assert!(executor.pending_stores.is_empty());

    let mut alternate_chunk = fixture.body.clone();
    alternate_chunk[0] ^= 1;
    let alternate_manifest = deliberately_conflicting_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        fixture.manifest.subject,
        &alternate_chunk,
    );
    assert_ne!(alternate_manifest, fixture.manifest);
    executor
        .admit_local_proposal(
            tag(1),
            alternate_manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("current view owns alternate exact manifest");
    let current_id = services.store_tasks.last().expect("current store").id();
    assert_ne!(current_id, retired_id);
    assert_eq!(executor.pending_stores.len(), 1);

    let late_completion = services.execute_store(retired_id);
    assert!(matches!(
        executor.complete_body_store(late_completion, &mut services),
        Err(EffectExecutorError::BodyStore(reason))
            if reason.contains("conflicts with retained exact-body ownership")
    ));
    assert_eq!(
        executor.pending_stores[&current_id].task.manifest(),
        &alternate_manifest
    );
    assert!(executor.recovered_bodies.is_empty());
    assert!(executor.durable_bodies.is_empty());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
}

#[test]
fn active_losing_store_releases_capacity_for_high_qc_fetch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("start losing old-view body store");
    let store_id = services.store_tasks[0].id();
    services.inflight_stores.insert(store_id);

    let high_subject = wire::BlockSubject {
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"high-QC block")),
        ..fixture.manifest.subject
    };
    let mut high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    high_prepare.subject = high_subject;
    let sources = certified_sources(&fixture, &high_prepare);
    let mut timeout = timeout_certificate(&fixture);
    timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout,
                protected_lock: Some(high_prepare.clone()),
            }],
            &mut services,
        )
        .expect("release active losing-store ownership");

    assert_eq!(services.cancelled_stores, vec![store_id]);
    assert!(executor.pending_stores.is_empty());
    assert_eq!(executor.pending_store_bytes, 0);
    assert!(executor.body_pipeline_owners.is_empty());

    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(1),
                round: high_prepare.round,
                subject: high_subject,
                manifest: None,
                certified_sources: sources,
                certificate: Some(high_prepare),
            }],
            &mut services,
        )
        .expect("high-QC fetch uses the released bounded slot");
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.pending_work(), 1);

    let late_completion = services.execute_store(store_id);
    assert_eq!(late_completion.tag(), tag(0));
    assert_eq!(
        executor
            .complete_body_store(late_completion, &mut services)
            .expect("catalogue late losing-store completion"),
        CompletionDisposition::Stale
    );
    assert_eq!(executor.pending_fetches.len(), 1);
    assert!(executor.runtime.completions.is_empty());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn view_change_cancels_non_durable_store_and_unprotected_validation() {
    for corrupt_class in ["store", "ready"] {
        for corruption in ["low", "high"] {
            let fixture = Fixture::new();
            let mut executor = fixture.executor(EffectQueueConfig::default());
            let mut services = fixture.services();
            match corrupt_class {
                "store" => {
                    executor
                        .admit_local_proposal(
                            tag(0),
                            fixture.manifest.clone(),
                            fixture.body.clone(),
                            &mut services,
                        )
                        .expect("queue stale store");
                    executor.pending_store_bytes = match corruption {
                        "low" => 0,
                        "high" => executor
                            .pending_store_bytes
                            .checked_add(1)
                            .expect("small test counter"),
                        _ => unreachable!("the test enumerates low and high corruption"),
                    };
                }
                "ready" => {
                    executor
                        .admit_ready_body_for_test(&fixture, &mut services)
                        .expect("queue stale BodyAvailable completion");
                    executor.ready_body_bytes = match corruption {
                        "low" => 0,
                        "high" => executor
                            .ready_body_bytes
                            .checked_add(1)
                            .expect("small test counter"),
                        _ => unreachable!("the test enumerates low and high corruption"),
                    };
                }
                _ => unreachable!("the test enumerates both byte-owner classes"),
            }
            let before = executor.body_ownership_projection();

            assert!(matches!(
                executor.consume_effects(
                    vec![AdapterEffect::EnterView {
                        tag: tag(1),
                        certificate: timeout_at_view(&fixture, 0),
                        protected_lock: None,
                    }],
                    &mut services,
                ),
                Err(EffectExecutorError::Contract(reason))
                    if reason.contains("body byte accounting")
            ));
            assert_eq!(
                executor.body_ownership_projection(),
                before,
                "{corrupt_class}/{corruption} accounting corruption must be rejected before ownership mutation"
            );
            assert!(services.cancelled_stores.is_empty());
            assert!(services.cancelled_fetches.is_empty());
            assert!(services.cancelled_validations.is_empty());
        }
    }

    // The counter covers the first ready body only. Without the global
    // preflight, lock reconciliation could retire that exact subset and
    // commit a zero residual before stale-view cleanup discovers the
    // second body's underflow.
    {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        for (view, generation) in [(0, 30), (2, 32)] {
            let manifest = manifest_at_view(&fixture, view);
            let key = (manifest.round, manifest.subject);
            let ready = ReadyBody::derive(
                &fixture.context,
                manifest.round,
                manifest.subject,
                fixture.body.clone(),
            )
            .expect("derive staged body at the selected view");
            let owner_tag = EventTag::new(1, view, Generation::new(generation));
            executor.body_pipeline_owners.insert(
                key,
                BodyPipelineOwner {
                    tag: owner_tag,
                    manifest_hash: Some(HashOf::new(&ready.manifest)),
                },
            );
            executor
                .runtime
                .completions
                .push(RuntimeCompletion::BodyAvailable(
                    owner_tag,
                    ready.manifest.clone(),
                ));
            executor.ready_bodies.insert(key, ready);
        }
        executor.ready_body_bytes = u64::try_from(fixture.body.len()).expect("one body length");
        let before = executor.body_ownership_projection();
        let mut replacement = fixture.qc(wire::GlobalPhase::Prepare);
        replacement.round = manifest_at_view(&fixture, 1).round;
        replacement.proposal_round = replacement.round;
        let mut timeout = timeout_at_view(&fixture, 2);
        timeout.groups[0].highest_prepare_qc = Some(replacement.clone());

        assert!(matches!(
            executor.consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: EventTag::new(1, 3, Generation::new(33)),
                    certificate: timeout,
                    protected_lock: Some(replacement.clone()),
                }],
                &mut services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("body byte accounting")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert!(executor.protected_lock.is_none());
        assert!(services.cancelled_stores.is_empty());
        assert!(services.cancelled_fetches.is_empty());
        assert!(services.cancelled_validations.is_empty());
    }

    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1_048_576, 1));
    let mut services = fixture.services();
    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("queue store");
    let store_id = services.store_tasks[0].id();
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        )
        .expect("install view");
    assert!(executor.pending_stores.is_empty());
    assert_eq!(services.cancelled_stores, vec![store_id]);

    let late_completion = services.execute_store(store_id);
    assert_eq!(
        executor
            .complete_body_store(late_completion, &mut services)
            .expect("late durable completion is retained"),
        CompletionDisposition::Stale
    );
    assert!(
        executor
            .durable_bodies
            .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
    );

    executor
        .admit_local_proposal(
            tag(0),
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("durable body starts validation");
    assert_eq!(executor.pending_validations.len(), 1);
    let validation_id = services.validation_tasks[0].id();
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            }],
            &mut services,
        )
        .expect("reinstall view for validation cancellation");
    assert!(
        executor.pending_validations.is_empty(),
        "a durable body remains reusable, but its stale validation survives only when the TC protects its exact high PrepareQC"
    );
    assert!(
        executor
            .durable_bodies
            .contains_key(&(fixture.manifest.round, fixture.manifest.subject))
    );
    assert_eq!(services.cancelled_stores, vec![store_id]);
    assert_eq!(services.cancelled_validations, vec![validation_id]);

    let late_validation = services.execute_validation(validation_id);
    let late_receipt = late_validation
        .validated_receipt()
        .expect("late validation succeeds deterministically")
        .clone();
    executor.runtime.completions.clear();
    assert_eq!(
        executor
            .complete_body_validation(late_validation, &mut services)
            .expect("late durable validation binds wire authority"),
        CompletionDisposition::Stale
    );
    assert!(
        executor.runtime.completions.is_empty(),
        "a retired reducer consumer must not be resurrected"
    );
    assert_eq!(
        executor.runtime.bound_validations,
        vec![(fixture.manifest.clone(), late_receipt.clone())],
        "the exact fsynced receipt must still release matching wire votes"
    );
    assert_eq!(
        executor
            .validated_bodies
            .get(&(fixture.manifest.round, fixture.manifest.subject)),
        Some(&late_receipt)
    );
    assert!(!executor.status().fail_closed);
}

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
    persist_fsynced_validation_marker(
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
    persist_fsynced_validation_marker(
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
    persist_fsynced_validation_marker(&mut executor, &mut services, &fixture, reproposal_manifest);
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
    persist_fsynced_validation_marker(
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
    persist_fsynced_validation_marker(
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
fn ordinary_fetch_authenticates_chunks_and_runs_store_validate_pipeline() {
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
    let fetch_task = services.fetch_tasks[0].clone();
    let work_id = fetch_task.id();
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
    let sender = fixture.context.roster[0].validator.clone();
    let ingress_ownership = payload_chunk_ingress_ownership(&chunk, sender.clone());
    executor
        .accept_payload_chunk_with_ingress_ownership(
            work_id,
            chunk,
            &sender,
            &ingress_ownership,
            &mut services,
        )
        .expect("authenticated chunk");
    assert_eq!(services.chunks, vec![work_id]);
    executor
        .complete_body_reconstruction(
            &fetch_task,
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("body reconstruction");
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(completion_tag, manifest))
            if *completion_tag == tag(0)
                && manifest == &fixture.manifest
    ));

    for _ in 0..8 {
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("retry store body");
    }
    assert_eq!(executor.pending_stores.len(), 1);
    assert!(
        services
            .store_tasks
            .iter()
            .all(|task| task.id() == services.store_tasks[0].id())
    );
    let store_id = services.store_tasks.last().expect("store task").id();
    let store_completion = services.execute_store(store_id);
    executor
        .complete_body_store(store_completion, &mut services)
        .expect("durable store completion");
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyStored(completion_tag, round, subject, receipt))
            if *completion_tag == tag(0)
                && *round == fixture.manifest.round
                && *subject == fixture.manifest.subject
                && receipt.subject() == fixture.manifest.subject
    ));
    for _ in 0..8 {
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: tag(0),
                    round: fixture.manifest.round,
                    subject: fixture.manifest.subject,
                }],
                &mut services,
            )
            .expect("retry validation");
    }
    assert_eq!(executor.pending_validations.len(), 1);
    assert!(
        services
            .validation_tasks
            .iter()
            .all(|task| task.id() == services.validation_tasks[0].id())
    );
    let validation_id = services
        .validation_tasks
        .last()
        .expect("validation task")
        .id();
    let validation_completion = services.execute_validation(validation_id);
    executor
        .complete_body_validation(validation_completion, &mut services)
        .expect("validation completion");
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::ValidationSucceeded(completion_tag, round, subject, receipt))
            if *completion_tag == tag(0)
                && *round == fixture.manifest.round
                && *subject == fixture.manifest.subject
                && receipt.durable().subject() == fixture.manifest.subject
    ));
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
fn validation_rejection_enqueues_failure_without_success_receipt() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor
        .admit_ready_body_for_test(&fixture, &mut services)
        .expect("ready body");
    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("store body");
    let store_id = services.store_tasks.last().expect("store task").id();
    let store_completion = services.execute_store(store_id);
    executor
        .complete_body_store(store_completion, &mut services)
        .expect("store completion");
    services.validation_error = Some("invalid transaction".to_owned());
    executor
        .consume_effects(
            vec![AdapterEffect::ValidateBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            }],
            &mut services,
        )
        .expect("queue validation");
    let validation_id = services
        .validation_tasks
        .last()
        .expect("validation task")
        .id();
    let validation_completion = services.execute_validation(validation_id);
    executor
        .complete_body_validation(validation_completion, &mut services)
        .expect("validation rejection is protocol input");
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::ValidationFailed(completion_tag, round, subject))
            if *completion_tag == tag(0)
                && *round == fixture.manifest.round
                && *subject == fixture.manifest.subject
    ));
    assert_eq!(services.rejected_validations, vec!["invalid transaction"]);
    assert!(!executor.status().fail_closed);
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
    let ingress_ownership = certified_response_ingress_ownership(&response, responder.clone());
    assert_eq!(
        executor
            .accept_certified_body_response_with_ingress_ownership(
                response.clone(),
                &responder,
                &ingress_ownership,
                &mut services,
            )
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
fn certified_body_response_carrier_swap_fails_closed_before_fetch_mutation() {
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
    let task = services.fetch_tasks[0].clone();
    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    let mut other = response.clone();
    other.body.push(0xFF);
    let responder = fixture.context.roster[0].validator.clone();
    let swapped_ownership = certified_response_ingress_ownership(&other, responder.clone());
    let pending_before = executor.pending_fetches.clone();
    let certified_before = executor.certified_work.clone();
    let outstanding_before = executor.outstanding_requests.hashes();

    assert!(matches!(
        executor.accept_certified_body_response_with_ingress_ownership(
            response,
            &responder,
            &swapped_ownership,
            &mut services,
        ),
        Err(EffectTransportError::FailClosed(reason))
            if reason.contains("fair-ingress ownership")
    ));
    assert_eq!(executor.pending_fetches, pending_before);
    assert_eq!(executor.certified_work, certified_before);
    assert_eq!(executor.outstanding_requests.hashes(), outstanding_before);
    assert!(services.completed_certified_fetches.is_empty());
    assert!(executor.status().fail_closed);
}
