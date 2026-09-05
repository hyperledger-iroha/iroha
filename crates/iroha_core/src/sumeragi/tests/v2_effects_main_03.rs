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
    let next_tag = EventTag::new(1, 1, Generation::new(41));
    executor.runtime.round_tag = Some(next_tag);
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: next_tag,
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: Some(protected_lock.clone()),
            }],
            &mut services,
        )
        .expect("an omitted TC high preserves the effective local lock cache");
    assert_eq!(services.entered_view_locks.last(), Some(&Some(protected)));
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
                certified_sources: certified_sources(&fixture, &protected_lock),
                certificate: Some(protected_lock.clone()),
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
    let following_tag = EventTag::new(1, 2, Generation::new(42));
    executor.runtime.round_tag = Some(following_tag);
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: following_tag,
                certificate: timeout_at_view(&fixture, 1),
                protected_lock: Some(protected_lock),
            }],
            &mut services,
        )
        .expect("the queued completion rebinds on repeated same-lock churn");
    assert_eq!(services.entered_view_locks.last(), Some(&Some(protected)));
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
    let carrier_parent_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"deferred Apply merge carrier parent"));
    let merge_subject = wire::BlockSubject {
        parent_block_hash: Some(carrier_parent_hash),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"deferred Apply merge carrier")),
        payload_hash: Hash::new(&fixture.body),
    };
    let manifest = canonical_payload_manifest(
        &fixture.context,
        fixture.manifest.round,
        merge_subject,
        &fixture.body,
    );
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let validated_receipt = ValidatedBodyReceipt::for_test(durable.clone());
    let validator_set = fixture
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let mut signers_bitmap = vec![0_u8; validator_set.len().div_ceil(8)];
    for index in 0..validator_set.len() {
        signers_bitmap[index / 8] |= 1 << (index % 8);
    }
    let entry = MergeLedgerEntry {
        version: MergeLedgerEntry::VERSION,
        epoch_id: fixture.context.epoch,
        lane_catalog_hash: Hash::new(b"deferred Apply merge lane catalog"),
        active_lanes: Vec::new(),
        lane_authority_catalog: iroha_data_model::merge::MergeLaneAuthorityCatalogV1::default(),
        incarnation_root: Hash::new(b"deferred Apply merge incarnations"),
        activation_root: Hash::new(b"deferred Apply merge activations"),
        lane_snapshots: Vec::new(),
        global_state_root: Hash::new(b"deferred Apply merge global state"),
        merge_qc: MergeQuorumCertificate::new(
            manifest.round.view,
            fixture.context.epoch,
            fixture.context.height,
            carrier_parent_hash,
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"deferred Apply merge network"),
            )),
            iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            HashOf::new(&validator_set),
            validator_set,
            signers_bitmap,
            Vec::new(),
            vec![0x5A; 96],
            Hash::new(b"deferred Apply merge certificate"),
        ),
        execution_batch: None,
        lane_drain_certificates: Vec::new(),
    };
    let reference = CertifiedMergeLedgerReference::new(&entry);
    let entry_hash = reference.entry_hash;
    let mut certificate = fixture.qc(wire::GlobalPhase::Commit);
    certificate.round = manifest.round;
    certificate.proposal_round = manifest.round;
    certificate.subject = manifest.subject;
    certificate.execution_commitment = validated_receipt.execution_commitment();
    executor.recovered_bodies.insert(
        (certificate.proposal_round, certificate.subject),
        (manifest, durable.clone()),
    );
    executor
        .durable_bodies
        .insert((certificate.proposal_round, certificate.subject), durable);
    executor.validated_bodies.insert(
        (certificate.proposal_round, certificate.subject),
        validated_receipt,
    );
    executor.runtime.round_tag = Some(tag(3));
    let apply_ownership =
        bound_test_apply_ownership(tag(3), certificate.subject, &certificate, tag(3), 3);
    executor
        .begin_apply(
            tag(3),
            certificate.subject,
            certificate,
            apply_ownership,
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
    assert_eq!(executor.status().deferred_application_merge_work, 0);
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
