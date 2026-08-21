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
