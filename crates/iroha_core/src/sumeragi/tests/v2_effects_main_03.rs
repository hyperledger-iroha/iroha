#[test]
fn reordered_enter_view_fails_before_fresh_sign_dispatch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let next_tag = tag(1);
    executor.runtime.round_tag = Some(next_tag);
    let mut fresh_vote = vote(&fixture);
    fresh_vote.round = round(&fixture.context, 1);
    fresh_vote.proposal_round = fresh_vote.round;
    assert!(matches!(
        executor.consume_effects(
            vec![
                AdapterEffect::Sign {
                    tag: next_tag,
                    request: SignRequest::Vote(fresh_vote),
                },
                AdapterEffect::EnterView {
                    tag: next_tag,
                    certificate: timeout_at_view(&fixture, 0),
                    protected_lock: None,
                },
            ],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("must be the first effect")
    ));
    assert_eq!(executor.runtime.effect_ownership_calls, 0);
    assert!(executor.runtime.effect_owners.is_empty());
    assert!(services.sign_tasks.is_empty());
    assert!(services.entered_views.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn same_view_higher_generation_tc_requires_and_installs_leading_enter_view() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    for view in [1, 2] {
        let next_tag = tag(view);
        executor.runtime.round_tag = Some(next_tag);
        executor
            .consume_effects(
                vec![AdapterEffect::EnterView {
                    tag: next_tag,
                    certificate: timeout_at_view(&fixture, view - 1),
                    protected_lock: None,
                }],
                &mut services,
            )
            .expect("install the ordinary certified view transition");
    }
    let current = tag(2);
    let upgraded = EventTag::new(
        current.height(),
        current.view(),
        Generation::new(current.generation().get() + 1),
    );
    executor.runtime.round_tag = Some(upgraded);
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: upgraded,
                certificate: timeout_at_view(&fixture, 1),
                protected_lock: None,
            }],
            &mut services,
        )
        .expect("install the alternate same-view TC generation");
    assert_eq!(executor.reconciled_tag, Some(upgraded));
    assert_eq!(services.entered_views.last(), Some(&upgraded));
    assert!(!executor.status().fail_closed);
}
#[test]
fn first_lock_retires_unlocked_fetch_store_and_validation_owners() {
    let fixture = Fixture::new();
    let consumer = EventTag::new(1, 3, Generation::new(84));
    let staged = manifest_at_view(&fixture, consumer.view());
    let (replacement_subject, _) = distinct_body(&fixture);
    let first_lock = (
        round(&fixture.context, consumer.view()),
        replacement_subject,
    );
    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                    manifest: Some(staged.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("start unlocked candidate fetch");
        let fetch_id = services.fetch_tasks[0].id();
        executor
            .reconcile_locked_body_for_recovery(consumer, first_lock, &mut services)
            .expect("first different lock retires the unlocked fetch");
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert_eq!(services.cancelled_fetches, vec![fetch_id]);
        assert_eq!(services.retired_outbound_subjects, vec![staged.subject]);
    }
    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                consumer,
                staged.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("start unlocked local-proposal store");
        let store_id = services.store_tasks[0].id();
        executor
            .reconcile_locked_body_for_recovery(consumer, first_lock, &mut services)
            .expect("first different lock retires the unlocked store");
        assert!(executor.pending_stores.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert_eq!(executor.pending_store_bytes, 0);
        assert_eq!(services.cancelled_stores, vec![store_id]);
    }
    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                consumer,
                staged.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("start unlocked local-proposal pipeline");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("advance unlocked candidate to validation");
        let validation_id = services.validation_tasks[0].id();
        executor
            .reconcile_locked_body_for_recovery(consumer, first_lock, &mut services)
            .expect("first different lock retires the unlocked validation");
        assert!(executor.pending_validations.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert_eq!(services.cancelled_validations, vec![validation_id]);
    }
}
#[test]
fn higher_lock_retires_queued_store_validation_and_local_proposal_completions() {
    let fixture = Fixture::new();
    let consumer = EventTag::new(1, 3, Generation::new(85));
    let staged = manifest_at_view(&fixture, consumer.view() - 1);
    let (replacement_subject, _) = distinct_body(&fixture);
    let first_lock = (
        round(&fixture.context, consumer.view()),
        replacement_subject,
    );
    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .reconcile_locked_body_for_recovery(
                consumer,
                (staged.round, staged.subject),
                &mut services,
            )
            .expect("publish the earlier exact lock");
        executor
            .retain_locked_body_for_recovery(
                consumer,
                staged.round,
                staged.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage earlier exact-origin reducer bytes");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                    manifest: Some(staged.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("bind unlocked BodyAvailable completion");
        executor.runtime.completions.clear();
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                }],
                &mut services,
            )
            .expect("start unlocked reducer store");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("queue BodyStored before lock installation");
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::BodyStored(tag, round, subject, _)]
                if *tag == consumer && *round == staged.round && *subject == staged.subject
        ));
        executor
            .reconcile_locked_body_for_recovery(consumer, first_lock, &mut services)
            .expect("first lock retires queued BodyStored");
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
    }
    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .admit_local_proposal(
                consumer,
                staged.clone(),
                fixture.body.clone(),
                &mut services,
            )
            .expect("start unlocked local proposal");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("advance local proposal to validation");
        let validation_id = services.validation_tasks[0].id();
        let validated = services.execute_validation(validation_id);
        executor
            .complete_body_validation(validated, &mut services)
            .expect("queue LocalProposalReady before lock installation");
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::LocalProposal(tag, manifest, ..)]
                if *tag == consumer && manifest == &staged
        ));
        executor
            .reconcile_locked_body_for_recovery(consumer, first_lock, &mut services)
            .expect("first lock retires queued LocalProposalReady");
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
    }
    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .reconcile_locked_body_for_recovery(
                consumer,
                (staged.round, staged.subject),
                &mut services,
            )
            .expect("publish the earlier exact lock");
        executor
            .retain_locked_body_for_recovery(
                consumer,
                staged.round,
                staged.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage earlier exact-origin bytes for validation");
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                    manifest: Some(staged.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut services,
            )
            .expect("bind unlocked completion");
        executor.runtime.completions.clear();
        executor
            .consume_effects(
                vec![AdapterEffect::StoreBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                }],
                &mut services,
            )
            .expect("start reducer store");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("record durable reducer body");
        executor.runtime.completions.clear();
        executor
            .consume_effects(
                vec![AdapterEffect::ValidateBody {
                    tag: consumer,
                    round: staged.round,
                    subject: staged.subject,
                }],
                &mut services,
            )
            .expect("start reducer validation");
        let validation_id = services.validation_tasks[0].id();
        let validated = services.execute_validation(validation_id);
        executor
            .complete_body_validation(validated, &mut services)
            .expect("queue ValidationSucceeded before lock installation");
        assert!(matches!(
            executor.runtime.completions.as_slice(),
            [RuntimeCompletion::ValidationSucceeded(tag, round, subject, _)]
                if *tag == consumer && *round == staged.round && *subject == staged.subject
        ));
        executor
            .reconcile_locked_body_for_recovery(consumer, first_lock, &mut services)
            .expect("first lock retires queued ValidationSucceeded");
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
    }
}
#[test]
fn lock_reconciliation_rejects_same_round_conflict_and_late_lower_lock() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let consumer = EventTag::new(1, 3, Generation::new(81));
    let first = (round(&fixture.context, 0), fixture.manifest.subject);
    let (replacement_subject, _) = distinct_body(&fixture);
    executor
        .reconcile_locked_body_for_recovery(consumer, first, &mut services)
        .expect("publish initial lock");
    assert!(matches!(
        executor.reconcile_locked_body_for_recovery(
            consumer,
            (first.0, replacement_subject),
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("strictly increase PrepareQC round")
    ));
    assert_eq!(executor.protected_lock, Some(first));
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
    let higher = (round(&fixture.context, 1), replacement_subject);
    executor
        .reconcile_locked_body_for_recovery(consumer, higher, &mut services)
        .expect("publish strictly higher lock");
    assert!(matches!(
        executor.reconcile_locked_body_for_recovery(consumer, first, &mut services),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("strictly increase PrepareQC round")
    ));
    assert_eq!(executor.protected_lock, Some(higher));
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}
#[test]
fn higher_round_same_subject_retires_old_origin_pipeline_with_same_tag() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let consumer = EventTag::new(1, 3, Generation::new(82));
    let body_len = u64::try_from(fixture.body.len()).expect("body length");
    let first = (round(&fixture.context, 0), fixture.manifest.subject);
    executor
        .reconcile_locked_body_for_recovery(consumer, first, &mut services)
        .expect("publish initial same-subject lock");
    executor
        .retain_locked_body_for_recovery(
            consumer,
            first.0,
            fixture.manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("retain and stage initial bytes");
    let staged = manifest_at_view(&fixture, first.0.view);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: consumer,
                round: staged.round,
                subject: staged.subject,
                manifest: Some(staged.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("queue the old round-bound completion");
    assert_eq!(executor.ready_body_bytes, body_len * 2);
    let higher = (round(&fixture.context, 1), fixture.manifest.subject);
    executor
        .reconcile_locked_body_for_recovery(consumer, higher, &mut services)
        .expect("higher same-subject lock retires old proposal-origin ownership");
    assert_eq!(executor.protected_lock, Some(higher));
    assert!(executor.ready_bodies.is_empty());
    assert!(executor.body_pipeline_owners.is_empty());
    assert!(executor.runtime.completions.is_empty());
    assert!(executor.retained_locked_body.is_some());
    assert_eq!(executor.ready_body_bytes, body_len);
    executor
        .retain_locked_body_for_recovery(
            consumer,
            higher.0,
            fixture.manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("stage the higher certified proposal origin");
    executor
        .reconcile_locked_body_for_recovery(consumer, higher, &mut services)
        .expect("exact lock reconciliation is idempotent");
    executor
        .retain_locked_body_for_recovery(
            consumer,
            higher.0,
            fixture.manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("exact cache restaging is idempotent");
    assert_eq!(executor.ready_bodies.len(), 1);
    assert_eq!(executor.ready_body_bytes, body_len * 2);
    assert!(!executor.status().fail_closed);
    // Exact lock repetition used to return before global byte accounting
    // was checked. Exercise the direct lock-recovery entrypoint with both
    // low and inflated counters: neither corruption may hide behind the
    // idempotent lock fast path or mutate an exact owner.
    for corruption in ["low", "high"] {
        let fixture = Fixture::new();
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let consumer = EventTag::new(1, 3, Generation::new(82));
        let exact_lock = (round(&fixture.context, 0), fixture.manifest.subject);
        executor
            .reconcile_locked_body_for_recovery(consumer, exact_lock, &mut services)
            .expect("publish the exact lock before staging bytes");
        executor
            .retain_locked_body_for_recovery(
                consumer,
                exact_lock.0,
                fixture.manifest.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage one exact retained owner");
        executor.ready_body_bytes = match corruption {
            "low" => 0,
            "high" => executor
                .ready_body_bytes
                .checked_add(1)
                .expect("small test counter"),
            _ => unreachable!("the test enumerates low and high corruption"),
        };
        let before = executor.body_ownership_projection();
        assert!(matches!(
            executor.reconcile_locked_body_for_recovery(
                consumer,
                exact_lock,
                &mut services,
            ),
            Err(EffectExecutorError::Contract(reason))
                if reason.contains("body byte accounting")
        ));
        assert_eq!(executor.body_ownership_projection(), before);
        assert_eq!(executor.protected_lock, Some(exact_lock));
        assert!(services.cancelled_fetches.is_empty());
        assert!(services.cancelled_stores.is_empty());
        assert!(services.cancelled_validations.is_empty());
    }
}
#[test]
fn retained_locked_body_survives_same_lock_view_churn_before_fetch_adopts_it() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let protected = (fixture.manifest.round, fixture.manifest.subject);
    let protected_lock = fixture.qc(wire::GlobalPhase::Prepare);
    let body_len = u64::try_from(fixture.body.len()).expect("body length");
    let initial_tag = EventTag::new(1, 0, Generation::new(40));
    executor
        .reconcile_locked_body_for_recovery(initial_tag, protected, &mut services)
        .expect("publish the exact protected origin");
    executor
        .retain_locked_body_for_recovery(
            initial_tag,
            protected.0,
            fixture.manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("stage one view-independent locked-body cache");
    assert_eq!(executor.ready_body_bytes, body_len * 2);
    assert!(executor.body_pipeline_owners.is_empty());
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: EventTag::new(1, 1, Generation::new(41)),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: Some(protected_lock.clone()),
            }],
            &mut services,
        )
        .expect("an omitted TC high preserves the effective local lock cache");
    assert_eq!(executor.ready_body_bytes, body_len * 2);
    assert!(executor.retained_locked_body.is_some());
    assert_eq!(executor.ready_bodies.len(), 1);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: EventTag::new(1, 1, Generation::new(41)),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("the new view adopts staged bytes without starting network work");
    assert!(services.fetch_tasks.is_empty());
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
            if *completion_tag == EventTag::new(1, 1, Generation::new(41))
                && manifest == &fixture.manifest
    ));
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: EventTag::new(1, 2, Generation::new(42)),
                certificate: timeout_at_view(&fixture, 1),
                protected_lock: Some(protected_lock),
            }],
            &mut services,
        )
        .expect("the queued completion rebinds on repeated same-lock churn");
    assert!(matches!(
        executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyAvailable(completion_tag, manifest)]
            if *completion_tag == EventTag::new(1, 2, Generation::new(42))
                && manifest == &fixture.manifest
    ));
    assert_eq!(executor.ready_body_bytes, body_len * 2);
    assert!(executor.retained_locked_body.is_some());
}
#[test]
fn higher_different_lock_releases_retained_cache_before_replacement_staging() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let original = (fixture.manifest.round, fixture.manifest.subject);
    let original_tag = EventTag::new(1, 0, Generation::new(50));
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
        .expect("retain the original lock cache");
    let replacement_header = BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        2_000,
        0,
    );
    let replacement_signature = SignatureOf::try_from_hash(
        fixture.validator_keys[0].private_key(),
        replacement_header.hash(),
    )
    .expect("replacement block signature");
    let replacement_block = SignedBlock::presigned(
        BlockSignature::new(0, replacement_signature),
        replacement_header,
        Vec::new(),
    );
    let replacement_body = replacement_block
        .encode_wire()
        .expect("replacement canonical body");
    let replacement_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: replacement_block.hash(),
        payload_hash: Hash::new(&replacement_body),
    };
    let replacement_round = round(&fixture.context, 1);
    let mut replacement = fixture.qc(wire::GlobalPhase::Prepare);
    replacement.round = replacement_round;
    replacement.proposal_round = replacement_round;
    replacement.subject = replacement_subject;
    let mut timeout = timeout_at_view(&fixture, 1);
    timeout.groups[0].highest_prepare_qc = Some(replacement.clone());
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: EventTag::new(1, 2, Generation::new(52)),
                certificate: timeout,
                protected_lock: Some(replacement),
            }],
            &mut services,
        )
        .expect("the higher different lock retires the old subject cache");
    assert!(executor.retained_locked_body.is_none());
    assert!(executor.ready_bodies.is_empty());
    assert_eq!(executor.ready_body_bytes, 0);
    executor
        .retain_locked_body_for_recovery(
            EventTag::new(1, 2, Generation::new(52)),
            replacement_round,
            replacement_subject,
            replacement_body.clone(),
            &mut services,
        )
        .expect("replacement lock can claim all released cache capacity");
    assert_eq!(
        executor
            .retained_locked_body
            .as_ref()
            .map(|(subject, _)| *subject),
        Some(replacement_subject)
    );
    assert_eq!(
        executor.ready_body_bytes,
        u64::try_from(replacement_body.len()).expect("replacement body length") * 2
    );
}
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
fn rejected_local_proposal_is_terminal_and_fails_closed_without_new_work() {
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
        .expect("admit the initial local proposal");
    let store_id = services.store_tasks[0].id();
    let stored = services.execute_store(store_id);
    executor
        .complete_body_store(stored, &mut services)
        .expect("persist the local proposal before validation");
    let first_validation_id = services.validation_tasks[0].id();
    services.validation_error = Some("transient local prerequisite".to_owned());
    let rejected = services.execute_validation(first_validation_id);
    assert_eq!(
        executor
            .complete_body_validation(rejected, &mut services)
            .expect("record the local validation rejection"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_validations.is_empty());
    assert!(executor.local_validate_replay.is_empty());
    assert!(executor.rejected_bodies.contains_key(&key));
    assert!(executor.durable_bodies.contains_key(&key));
    assert!(executor.recovered_bodies.contains_key(&key));
    assert!(executor.body_pipeline_owners.contains_key(&key));
    assert_eq!(
        services.rejected_validations,
        vec!["transient local prerequisite"]
    );

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
    assert_eq!(
        services.store_tasks.len(),
        1,
        "a sealed rejection must not repeat the durable Store operation"
    );
    assert_eq!(services.validation_tasks.len(), 1);
    assert_eq!(executor.body_ownership_projection(), before);
    assert!(executor.rejected_bodies.contains_key(&key));
    assert!(executor.durable_bodies.contains_key(&key));
    assert!(executor.recovered_bodies.contains_key(&key));
    assert!(executor.pending_validations.is_empty());
    assert!(executor.local_validate_replay.is_empty());
    assert!(executor.validated_bodies.is_empty());
    assert!(executor.local_proposal_ready_replay.is_empty());
    assert!(executor.status().fail_closed);
    assert_eq!(services.closed.len(), 1);
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
            assert!(executor.local_validate_replay.is_empty());
            assert!(executor.local_proposal_ready_replay.is_empty());
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
