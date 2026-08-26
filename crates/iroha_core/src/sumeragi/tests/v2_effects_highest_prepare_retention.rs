fn highest_prepare_test_proposal(
    fixture: &Fixture,
    manifest: &wire::PayloadManifest,
) -> wire::Proposal {
    wire::Proposal {
        round: manifest.round,
        proposer: fixture.context.leader(manifest.round.view),
        subject: manifest.subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(
            wire::ParentCommitJustification { certificate: None },
        ),
        signature: vec![0xA7],
    }
}

fn prepared_highest_prepare_fetch_replay(
    fixture: &Fixture,
    manifest: &wire::PayloadManifest,
    replay_tag: EventTag,
    ordinal: u128,
) -> (
    AdapterEffect,
    RuntimeEffectOwnership,
    PreparedRemoteProposalFetchReplayPreAdmission,
) {
    let fetch = AdapterEffect::FetchBody {
        tag: replay_tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let mut ownership = bound_test_effect_ownership(&fetch, replay_tag, ordinal);
    assert!(ownership.bind_authenticated_remote_proposal_replay_for_test(
        highest_prepare_test_proposal(fixture, manifest),
        &fetch,
    ));
    let replay = PreparedRemoteProposalFetchReplayPreAdmission::seal_exact_fetch(
        fetch.clone(),
        ownership.clone(),
    )
    .unwrap_or_else(|_| panic!("seal exact highest-Prepare Proposal Fetch replay"));
    (fetch, ownership, replay)
}

fn install_highest_prepare_stored_replay(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    fixture: &Fixture,
    manifest: &wire::PayloadManifest,
    replay_tag: EventTag,
    ordinal: u128,
) {
    let key = (manifest.round, manifest.subject);
    let (_fetch, fetch_ownership, fetch_replay) =
        prepared_highest_prepare_fetch_replay(fixture, manifest, replay_tag, ordinal);
    let store = AdapterEffect::StoreBody {
        tag: replay_tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let store_ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&store)
        .expect("project exact highest-Prepare Proposal Store owner");
    let store_replay = fetch_replay
        .project_store(store, store_ownership.clone())
        .unwrap_or_else(|_| panic!("project exact highest-Prepare Proposal Store replay"));
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(manifest),
    );
    let stored_replay = store_replay
        .bind_durable_body(durable.clone())
        .unwrap_or_else(|_| panic!("bind exact highest-Prepare durable Proposal body"));
    if let Some(existing) = executor.durable_bodies.insert(key, durable.clone()) {
        assert_eq!(existing, durable, "one body key has one durable receipt");
    }
    assert!(
        executor
            .remote_proposal_replay
            .insert(
                key,
                RemoteProposalReplayStageV1::Stored {
                    replay: stored_replay,
                    ownership: store_ownership,
                },
            )
            .is_none(),
        "the replay fixture must not replace a live lineage",
    );
}

fn install_highest_prepare_inflight_store(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    services: &mut FakeServices,
    fixture: &Fixture,
    manifest: &wire::PayloadManifest,
    body: Vec<u8>,
    replay_tag: EventTag,
    ordinal: u128,
) -> EffectWorkId {
    let key = (manifest.round, manifest.subject);
    let ready = ReadyBody::derive(
        &fixture.context,
        manifest.round,
        manifest.subject,
        body,
    )
    .expect("derive the highest-Prepare Proposal body");
    executor.ready_body_bytes = u64::try_from(ready.bytes.len()).expect("fixture body length");
    assert!(executor.ready_bodies.insert(key, ready).is_none());
    assert!(
        executor
            .body_pipeline_owners
            .insert(
                key,
                BodyPipelineOwner {
                    tag: replay_tag,
                    manifest_hash: Some(HashOf::new(manifest)),
                },
            )
            .is_none()
    );
    let (_fetch, fetch_ownership, fetch_replay) =
        prepared_highest_prepare_fetch_replay(fixture, manifest, replay_tag, ordinal);
    assert!(
        executor
            .remote_proposal_replay
            .insert(
                key,
                RemoteProposalReplayStageV1::BodyAvailable(fetch_replay),
            )
            .is_none()
    );
    let store = AdapterEffect::StoreBody {
        tag: replay_tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let ownership = fetch_ownership
        .rebind_as_inherited_adapter_effect(&store)
        .expect("project exact highest-Prepare Proposal Store owner");
    executor
        .retain_effect_batch(vec![store], vec![ownership])
        .expect("retain the highest-Prepare Proposal Store");
    assert_eq!(
        executor
            .drain_retained_effect_batch(services, true)
            .expect("start the highest-Prepare durable Store"),
        1
    );
    services
        .store_tasks
        .last()
        .expect("one highest-Prepare Store task")
        .id()
}

fn consume_highest_prepare_enter_view(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    services: &mut FakeServices,
    next_tag: EventTag,
    certificate: wire::TimeoutCertificate,
    protected_lock: Option<wire::QuorumCertificate>,
    highest_prepare: wire::QuorumCertificateRef,
) {
    executor.runtime.round_tag = Some(next_tag);
    executor.runtime.locked_body = protected_lock
        .as_ref()
        .map(|certificate| (certificate.proposal_round, certificate.subject));
    executor.runtime.highest_prepare = Some(highest_prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::EnterView {
                tag: next_tag,
                certificate,
                protected_lock,
            }],
            services,
        )
        .expect("consume EnterView with its atomic durable highest-Prepare frontier");
}

#[test]
#[allow(clippy::too_many_lines)]
fn lockless_enter_view_retains_only_highest_prepare_store_lineage() {
    let fixture = Fixture::new();
    let high_manifest = manifest_at_view(&fixture, 1);
    let high_key = (high_manifest.round, high_manifest.subject);
    let high_prepare = prepare_qc_for_subject(high_manifest.round, high_manifest.subject);
    let (unrelated_subject, unrelated_body) = distinct_body(&fixture);
    let unrelated_manifest = canonical_payload_manifest(
        &fixture.context,
        high_manifest.round,
        unrelated_subject,
        &unrelated_body,
    );
    let unrelated_key = (unrelated_manifest.round, unrelated_manifest.subject);

    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    install_highest_prepare_stored_replay(
        &mut executor,
        &fixture,
        &high_manifest,
        tag(1),
        97_001,
    );
    install_highest_prepare_stored_replay(
        &mut executor,
        &fixture,
        &unrelated_manifest,
        tag(1),
        97_002,
    );
    consume_highest_prepare_enter_view(
        &mut executor,
        &mut services,
        tag(2),
        timeout_at_view(&fixture, 1),
        None,
        high_prepare.as_ref(),
    );
    assert_eq!(executor.protected_lock, None);
    assert_eq!(executor.remote_proposal_replay.len(), 1);
    assert!(matches!(
        executor.remote_proposal_replay.get(&high_key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));
    assert!(!executor.remote_proposal_replay.contains_key(&unrelated_key));
    assert!(!executor.status().fail_closed);

    // The durable high is cleanup authority only: it cannot preserve a Fetch
    // or a ready body as though it were the reducer's voting lock.
    let mut fetch_executor = fixture.executor(EffectQueueConfig::default());
    let mut fetch_services = fixture.services();
    let (_fetch, fetch_ownership, fetch_replay) =
        prepared_highest_prepare_fetch_replay(&fixture, &high_manifest, tag(1), 97_003);
    let fetch_id = EffectWorkId::for_test(97_003);
    let fetch_task = BodyFetchTask {
        id: fetch_id,
        tag: tag(1),
        round: high_manifest.round,
        subject: high_manifest.subject,
        manifest: Some(high_manifest.clone()),
        sources: Vec::new(),
        certified_request: None,
        ownership: fetch_ownership,
    };
    assert!(
        fetch_executor
            .pending_fetches
            .insert(
                fetch_id,
                PendingFetch {
                    task: fetch_task,
                    request_hash: None,
                },
            )
            .is_none()
    );
    assert!(
        fetch_executor
            .body_pipeline_owners
            .insert(
                high_key,
                BodyPipelineOwner {
                    tag: tag(1),
                    manifest_hash: Some(HashOf::new(&high_manifest)),
                },
            )
            .is_none()
    );
    assert!(
        fetch_executor
            .remote_proposal_replay
            .insert(
                high_key,
                RemoteProposalReplayStageV1::Fetch {
                    work_id: fetch_id,
                    replay: fetch_replay,
                },
            )
            .is_none()
    );
    consume_highest_prepare_enter_view(
        &mut fetch_executor,
        &mut fetch_services,
        tag(2),
        timeout_at_view(&fixture, 1),
        None,
        high_prepare.as_ref(),
    );
    assert!(fetch_executor.pending_fetches.is_empty());
    assert!(fetch_executor.remote_proposal_replay.is_empty());
    assert!(fetch_executor.body_pipeline_owners.is_empty());
    assert_eq!(fetch_services.cancelled_fetches, vec![fetch_id]);

    let mut ready_executor = fixture.executor(EffectQueueConfig::default());
    let mut ready_services = fixture.services();
    let (_fetch, _ownership, ready_replay) =
        prepared_highest_prepare_fetch_replay(&fixture, &high_manifest, tag(1), 97_004);
    let ready = ReadyBody::derive(
        &fixture.context,
        high_manifest.round,
        high_manifest.subject,
        fixture.body.clone(),
    )
    .expect("derive the cleanup-only high ready body");
    ready_executor.ready_body_bytes =
        u64::try_from(ready.bytes.len()).expect("fixture body length");
    assert!(ready_executor.ready_bodies.insert(high_key, ready).is_none());
    assert!(
        ready_executor
            .body_pipeline_owners
            .insert(
                high_key,
                BodyPipelineOwner {
                    tag: tag(1),
                    manifest_hash: Some(HashOf::new(&high_manifest)),
                },
            )
            .is_none()
    );
    assert!(
        ready_executor
            .remote_proposal_replay
            .insert(
                high_key,
                RemoteProposalReplayStageV1::BodyAvailable(ready_replay),
            )
            .is_none()
    );
    ready_executor
        .runtime
        .completions
        .push(RuntimeCompletion::BodyAvailable(tag(1), high_manifest.clone()));
    consume_highest_prepare_enter_view(
        &mut ready_executor,
        &mut ready_services,
        tag(2),
        timeout_at_view(&fixture, 1),
        None,
        high_prepare.as_ref(),
    );
    assert!(ready_executor.ready_bodies.is_empty());
    assert_eq!(ready_executor.ready_body_bytes, 0);
    assert!(ready_executor.remote_proposal_replay.is_empty());
    assert!(ready_executor.body_pipeline_owners.is_empty());
    assert!(ready_executor.runtime.completions.is_empty());
    assert!(!ready_executor.status().fail_closed);
}

#[test]
fn older_tc_lock_and_newer_highest_prepare_store_form_a_bounded_frontier() {
    let fixture = Fixture::new();
    let old_manifest = fixture.manifest.clone();
    let old_key = (old_manifest.round, old_manifest.subject);
    let old_prepare = prepare_qc_for_subject(old_manifest.round, old_manifest.subject);
    let high_manifest = manifest_at_view(&fixture, 1);
    let high_key = (high_manifest.round, high_manifest.subject);
    let high_prepare = prepare_qc_for_subject(high_manifest.round, high_manifest.subject);
    let (unrelated_subject, unrelated_body) = distinct_body(&fixture);
    let unrelated_manifest = canonical_payload_manifest(
        &fixture.context,
        high_manifest.round,
        unrelated_subject,
        &unrelated_body,
    );
    let unrelated_key = (unrelated_manifest.round, unrelated_manifest.subject);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();

    install_highest_prepare_stored_replay(
        &mut executor,
        &fixture,
        &high_manifest,
        tag(1),
        97_010,
    );
    consume_highest_prepare_enter_view(
        &mut executor,
        &mut services,
        tag(2),
        timeout_at_view(&fixture, 1),
        None,
        high_prepare.as_ref(),
    );
    assert!(executor.remote_proposal_replay.contains_key(&high_key));

    // Model an older body reaching fsync while view 2 remains active. A later
    // valid TC can still promote that historical PrepareQC as the first lock.
    install_highest_prepare_stored_replay(
        &mut executor,
        &fixture,
        &old_manifest,
        tag(2),
        97_011,
    );
    install_highest_prepare_stored_replay(
        &mut executor,
        &fixture,
        &unrelated_manifest,
        tag(2),
        97_012,
    );
    let mut timeout = timeout_at_view(&fixture, 2);
    timeout.groups[0].highest_prepare_qc = Some(old_prepare.clone());
    consume_highest_prepare_enter_view(
        &mut executor,
        &mut services,
        tag(3),
        timeout,
        Some(old_prepare),
        high_prepare.as_ref(),
    );

    assert_eq!(executor.protected_lock, Some(old_key));
    assert!(executor.remote_proposal_replay.len() <= 2);
    assert_eq!(
        executor
            .remote_proposal_replay
            .keys()
            .copied()
            .collect::<std::collections::BTreeSet<_>>(),
        std::collections::BTreeSet::from([old_key, high_key])
    );
    assert!(!executor.remote_proposal_replay.contains_key(&unrelated_key));
    assert!(executor.remote_proposal_replay.values().all(|stage| matches!(
        stage,
        RemoteProposalReplayStageV1::Stored { .. }
    )));
    assert!(!executor.status().fail_closed);
}

#[test]
fn lockless_enter_view_detaches_highest_prepare_store_and_keeps_its_owner() {
    let fixture = Fixture::new();
    let high_manifest = manifest_at_view(&fixture, 1);
    let high_key = (high_manifest.round, high_manifest.subject);
    let high_prepare = prepare_qc_for_subject(high_manifest.round, high_manifest.subject);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.round_tag = Some(tag(1));
    executor.reconciled_tag = Some(tag(1));
    let store_id = install_highest_prepare_inflight_store(
        &mut executor,
        &mut services,
        &fixture,
        &high_manifest,
        fixture.body.clone(),
        tag(1),
        97_020,
    );
    assert!(executor.pending_stores[&store_id].consumer.is_some());

    consume_highest_prepare_enter_view(
        &mut executor,
        &mut services,
        tag(2),
        timeout_at_view(&fixture, 1),
        None,
        high_prepare.as_ref(),
    );

    assert_eq!(executor.protected_lock, None);
    assert_eq!(executor.pending_stores.len(), 1);
    assert!(executor.pending_stores[&store_id].consumer.is_none());
    assert!(matches!(
        executor.remote_proposal_replay.get(&high_key),
        Some(RemoteProposalReplayStageV1::Store { work_id, .. })
            if *work_id == store_id
    ));
    assert_eq!(
        executor
            .body_pipeline_owners
            .get(&high_key)
            .map(|owner| owner.tag),
        Some(tag(1)),
        "the detached Store keeps its exact physical pipeline owner",
    );
    assert!(services.cancelled_stores.is_empty());
    assert_eq!(executor.remote_proposal_replay.len(), 1);
    assert!(!executor.status().fail_closed);
}

#[test]
fn cleanup_only_high_retires_queued_terminals_but_keeps_stored_replay() {
    let fixture = Fixture::new();
    let high_manifest = manifest_at_view(&fixture, 1);
    let high_key = (high_manifest.round, high_manifest.subject);
    let high_prepare = prepare_qc_for_subject(high_manifest.round, high_manifest.subject);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();

    install_highest_prepare_stored_replay(
        &mut executor,
        &fixture,
        &high_manifest,
        tag(1),
        97_030,
    );
    assert!(
        executor
            .body_pipeline_owners
            .insert(
                high_key,
                BodyPipelineOwner {
                    tag: tag(1),
                    manifest_hash: Some(HashOf::new(&high_manifest)),
                },
            )
            .is_none()
    );
    let durable = executor.durable_bodies[&high_key].clone();
    executor.runtime.completions.extend([
        RuntimeCompletion::BodyStored(
            tag(1),
            high_manifest.round,
            high_manifest.subject,
            durable.clone(),
        ),
        RuntimeCompletion::LocalProposal(
            tag(1),
            high_manifest.clone(),
            durable.clone(),
            ValidatedBodyReceipt::for_test(durable),
        ),
    ]);

    consume_highest_prepare_enter_view(
        &mut executor,
        &mut services,
        tag(2),
        timeout_at_view(&fixture, 1),
        None,
        high_prepare.as_ref(),
    );

    assert!(
        executor.runtime.completions.is_empty(),
        "cleanup-only authority cannot retain BodyStored or LocalProposalReady terminals",
    );
    assert!(matches!(
        executor.remote_proposal_replay.get(&high_key),
        Some(RemoteProposalReplayStageV1::Stored { .. })
    ));
    assert!(!executor.status().fail_closed);
}

#[test]
fn enter_view_retires_unprotected_terminals_before_dropping_pipeline_owner() {
    let fixture = Fixture::new();
    let stale_manifest = fixture.manifest.clone();
    let stale_key = (stale_manifest.round, stale_manifest.subject);
    let high_manifest = manifest_at_view(&fixture, 1);
    let high_prepare = prepare_qc_for_subject(high_manifest.round, high_manifest.subject);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        stale_manifest.round,
        stale_manifest.subject,
        HashOf::new(&stale_manifest),
    );

    assert!(
        executor
            .body_pipeline_owners
            .insert(
                stale_key,
                BodyPipelineOwner {
                    tag: tag(1),
                    manifest_hash: Some(HashOf::new(&stale_manifest)),
                },
            )
            .is_none()
    );
    executor.runtime.completions.extend([
        RuntimeCompletion::BodyStored(
            tag(1),
            stale_manifest.round,
            stale_manifest.subject,
            durable.clone(),
        ),
        RuntimeCompletion::LocalProposal(
            tag(1),
            stale_manifest,
            durable.clone(),
            ValidatedBodyReceipt::for_test(durable),
        ),
    ]);

    consume_highest_prepare_enter_view(
        &mut executor,
        &mut services,
        tag(2),
        timeout_at_view(&fixture, 1),
        None,
        high_prepare.as_ref(),
    );

    assert!(executor.body_pipeline_owners.is_empty());
    assert!(
        executor.runtime.completions.is_empty(),
        "EnterView cannot orphan a queued body terminal after releasing its exact owner",
    );
    assert!(!executor.status().fail_closed);
}

#[test]
fn published_store_survives_enter_view_and_stutters_stale_foreign_owner() {
    let fixture = Fixture::new();
    let store_tag = tag(10);
    let retry_tag = tag(11);
    let manifest = manifest_at_view(&fixture, 8);
    let key = (manifest.round, manifest.subject);
    let prepare = prepare_qc_for_subject(manifest.round, manifest.subject);
    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        manifest.round,
        manifest.subject,
        HashOf::new(&manifest),
    );
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.round_tag = Some(store_tag);
    executor.reconciled_tag = Some(store_tag);
    assert!(
        executor
            .recovered_bodies
            .insert(key, (manifest.clone(), durable.clone()))
            .is_none()
    );
    assert!(executor.durable_bodies.insert(key, durable.clone()).is_none());

    let certified_fetch = AdapterEffect::FetchBody {
        tag: store_tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest.clone()),
        certified_sources: certified_sources(&fixture, &prepare),
        certificate: Some(prepare.clone()),
    };
    let published_store = AdapterEffect::StoreBody {
        tag: store_tag,
        round: manifest.round,
        subject: manifest.subject,
    };
    let published_ownership = bound_test_effect_ownership(&certified_fetch, store_tag, 97_040)
        .rebind_as_inherited_adapter_effect(&published_store)
        .expect("project the certified lifecycle Store owner");
    let published_pending = published_ownership
        .exact_pending_adapter_effect_binding(&published_store)
        .expect("seal the certified lifecycle Store binding");
    let marker = executor
        .prepare_published_lifecycle_store_retry_marker(&durable)
        .expect("preflight the published Store marker")
        .bind_store_successor(&published_store, &published_pending)
        .expect("bind the exact published Store successor");
    executor.commit_published_lifecycle_store_retry_marker(marker);
    let accepted_marker = executor.published_lifecycle_store_retry_markers[&key].clone();

    let mut timeout = timeout_at_view(&fixture, 10);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    consume_highest_prepare_enter_view(
        &mut executor,
        &mut services,
        retry_tag,
        timeout,
        Some(prepare.clone()),
        prepare.as_ref(),
    );
    assert_eq!(
        executor.published_lifecycle_store_retry_markers[&key],
        accepted_marker,
        "the exact protected Store row must survive its view transition",
    );

    let ordinary_fetch = AdapterEffect::FetchBody {
        tag: retry_tag,
        round: manifest.round,
        subject: manifest.subject,
        manifest: Some(manifest),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let retry_store = AdapterEffect::StoreBody {
        tag: retry_tag,
        round: key.0,
        subject: key.1,
    };
    let retry_ownership = bound_test_effect_ownership(&ordinary_fetch, retry_tag, 97_041)
        .rebind_as_inherited_adapter_effect(&retry_store)
        .expect("project the fresh ordinary Store retry owner");
    let retry_pending = retry_ownership
        .exact_pending_adapter_effect_binding(&retry_store)
        .expect("seal the fresh ordinary Store retry binding");
    assert_ne!(published_ownership.owner(), retry_ownership.owner());
    assert_eq!(
        published_pending
            .candidate_statement()
            .zip(retry_pending.candidate_statement())
            .and_then(|(published, retry)| published.body_stage_authority_relation_to(retry)),
        Some(RuntimeFetchAuthorityRelation::Stale),
        "the regression needs a weaker retry under a foreign physical owner",
    );
    let query_identity = retry_ownership
        .candidate_semantic_identity()
        .expect("the Store retry has one candidate identity");
    assert!(
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(query_identity, published_ownership)
            .is_none()
    );
    let queries_before = executor.runtime.terminal_body_candidate_queries.clone();
    let commits_before = executor.runtime.terminal_body_candidate_commits;

    executor
        .retain_effect_batch(vec![retry_store], vec![retry_ownership])
        .expect("the active Store marker stutters the stale foreign-owner retry");

    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.pending_stores.is_empty());
    assert!(services.store_tasks.is_empty());
    assert_eq!(
        executor.runtime.terminal_body_candidate_queries, queries_before,
        "the Store marker must stutter before runtime terminal-owner comparison",
    );
    assert_eq!(executor.runtime.terminal_body_candidate_commits, commits_before);
    assert_eq!(
        executor.published_lifecycle_store_retry_markers[&key],
        accepted_marker,
    );
    assert!(!executor.status().fail_closed);
    assert!(!executor.output_guard.restart_required());
    assert!(services.closed.is_empty());
}
