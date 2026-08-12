#[test]
fn authority_transition_after_enter_view_rebinds_body_consumers_not_physical_tasks() {
    let fixture = Fixture::new();
    let original_tag = tag(0);
    let certified_tag = tag(1);
    let original_store = AdapterEffect::StoreBody {
        tag: original_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let certified_store = AdapterEffect::StoreBody {
        tag: certified_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let original_validate = AdapterEffect::ValidateBody {
        tag: original_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let certified_validate = AdapterEffect::ValidateBody {
        tag: certified_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let original_fetch = AdapterEffect::FetchBody {
        tag: original_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let certified_fetch = AdapterEffect::FetchBody {
        tag: certified_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(fixture.qc(wire::GlobalPhase::Commit)),
    };
    let stage_owners = |fetch: &AdapterEffect,
                        store: &AdapterEffect,
                        validate: &AdapterEffect,
                        root_tag,
                        ordinal| {
        let fetch = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(root_tag, ordinal)],
        )
        .expect("bind exact Fetch ownership")
        .pop()
        .expect("one Fetch owner");
        let store = fetch
            .rebind_as_inherited_adapter_effect(store)
            .expect("Fetch authorizes Store");
        let validate = store
            .rebind_as_inherited_adapter_effect(validate)
            .expect("Store authorizes Validate");
        (store, validate)
    };
    let (original_store_owner, original_validate_owner) = stage_owners(
        &original_fetch,
        &original_store,
        &original_validate,
        original_tag,
        7_101,
    );
    let (certified_store_owner, certified_validate_owner) = stage_owners(
        &certified_fetch,
        &certified_store,
        &certified_validate,
        certified_tag,
        7_102,
    );
    let body_key = (fixture.manifest.round, fixture.manifest.subject);

    let mut store_executor = fixture.executor(EffectQueueConfig::default());
    let mut store_services = fixture.services();
    store_executor
        .bind_body_pipeline_owner(original_tag, &fixture.manifest)
        .expect("bind original Store pipeline");
    store_executor
        .begin_store(
            original_tag,
            fixture.manifest.clone(),
            Arc::<[u8]>::from(fixture.body.clone()),
            StorePurpose::Reducer,
            original_store_owner,
            &mut store_services,
        )
        .expect("start original Store task");
    let store_id = store_services.store_tasks[0].id();
    let immutable_store = store_executor.pending_stores[&store_id].task.clone();
    let store_work_cursor = store_executor.next_work_id;
    store_executor.runtime.round_tag = Some(certified_tag);
    store_executor.reconciled_tag = Some(certified_tag);
    store_executor
        .retain_effect_batch(vec![certified_store], vec![certified_store_owner])
        .expect("retain Commit-authorized Store after EnterView");
    assert_eq!(
        store_executor
            .drain_retained_effect_batch(&mut store_services, false)
            .expect("redispatch the immutable Store task"),
        1
    );
    assert_eq!(store_executor.next_work_id, store_work_cursor);
    assert_eq!(
        store_executor.pending_stores[&store_id].task,
        immutable_store
    );
    assert_eq!(
        store_executor.pending_stores[&store_id]
            .consumer
            .as_ref()
            .expect("Store keeps one reducer consumer")
            .tag(),
        certified_tag
    );
    assert_eq!(
        store_executor.body_pipeline_owners[&body_key].tag,
        certified_tag
    );
    assert!(
        store_services
            .store_tasks
            .iter()
            .all(|task| task == &immutable_store),
        "the physical Store work ID, tag, bytes, and lifecycle owner remain immutable"
    );
    let stored = store_services.execute_store(store_id);
    let durable = stored.receipt().clone();
    assert_eq!(stored.tag(), original_tag);
    store_executor
        .complete_body_store(stored, &mut store_services)
        .expect("the old-tag physical Store completes into the rebound consumer");
    assert!(matches!(
        store_executor.runtime.completions.as_slice(),
        [RuntimeCompletion::BodyStored(tag, round, subject, _)]
            if *tag == certified_tag
                && *round == fixture.manifest.round
                && *subject == fixture.manifest.subject
    ));

    let mut validation_executor = fixture.executor(EffectQueueConfig::default());
    let mut validation_services = store_services;
    validation_executor
        .recovered_bodies
        .insert(body_key, (fixture.manifest.clone(), durable.clone()));
    validation_executor
        .durable_bodies
        .insert(body_key, durable.clone());
    validation_executor
        .bind_body_pipeline_owner(original_tag, &fixture.manifest)
        .expect("bind original validation pipeline");
    validation_executor
        .begin_validation(
            fixture.manifest.round,
            fixture.manifest.subject,
            durable,
            ValidationConsumer::Reducer {
                tag: original_tag,
                ownership: original_validate_owner,
            },
            &mut validation_services,
        )
        .expect("start original validation task");
    let validation_id = validation_services.validation_tasks[0].id();
    let immutable_validation = validation_executor.pending_validations[&validation_id]
        .task
        .clone();
    let validation_work_cursor = validation_executor.next_work_id;
    validation_executor.runtime.round_tag = Some(certified_tag);
    validation_executor.reconciled_tag = Some(certified_tag);
    validation_executor
        .retain_effect_batch(vec![certified_validate], vec![certified_validate_owner])
        .expect("retain Commit-authorized validation after EnterView");
    assert_eq!(
        validation_executor
            .drain_retained_effect_batch(&mut validation_services, false)
            .expect("redispatch the immutable validation task"),
        1
    );
    assert_eq!(validation_executor.next_work_id, validation_work_cursor);
    assert_eq!(
        validation_executor.pending_validations[&validation_id].task,
        immutable_validation
    );
    assert_eq!(
        validation_executor.pending_validations[&validation_id]
            .consumer
            .as_ref()
            .expect("validation keeps one reducer consumer")
            .tag(),
        certified_tag
    );
    assert_eq!(
        validation_executor.body_pipeline_owners[&body_key].tag,
        certified_tag
    );
    assert!(
        validation_services
            .validation_tasks
            .iter()
            .all(|task| task == &immutable_validation),
        "the physical validation work ID, durable receipt, and lifecycle owner remain immutable"
    );
    let validated = validation_services.execute_validation(validation_id);
    validation_executor
        .complete_body_validation(validated, &mut validation_services)
        .expect("the old physical validation completes into the rebound consumer");
    assert!(matches!(
        validation_executor.runtime.completions.as_slice(),
        [RuntimeCompletion::ValidationSucceeded(tag, round, subject, _)]
            if *tag == certified_tag
                && *round == fixture.manifest.round
                && *subject == fixture.manifest.subject
    ));
    assert!(!validation_executor.status().fail_closed);
}

#[test]
fn enter_view_and_fetch_authority_upgrade_retain_the_protected_fetch_owner() {
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
    executor
        .consume_effects(vec![ordinary], &mut services)
        .expect("start the ordinary protected-body acquisition");
    let original = services.fetch_tasks[0].clone();
    let work_id = original.id();

    // Model a later reducer macro-step whose stronger QC was admitted by
    // an independent runtime lifecycle. Retention must join that authority
    // to the already-live physical fetch before EnterView rebinds it.
    executor.runtime.effect_owners.clear();
    executor.runtime.round_tag = Some(tag(1));
    executor.runtime.locked_body = Some((fixture.manifest.round, fixture.manifest.subject));
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &prepare);
    let mut timeout = timeout_at_view(&fixture, 0);
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
                    certificate: Some(prepare.clone()),
                },
            ],
            &mut services,
        )
        .expect("EnterView and its authority upgrade retain one physical fetch owner");

    assert_eq!(executor.pending_fetches.len(), 1);
    let pending = &executor.pending_fetches[&work_id].task;
    assert_eq!(pending.tag, tag(1));
    assert_eq!(pending.ownership(), original.ownership());
    assert_eq!(
        pending
            .certified_request()
            .map(|request| &request.certificate),
        Some(&prepare)
    );
    assert!(
        services
            .fetch_tasks
            .iter()
            .all(|task| { task.id() == work_id && task.ownership() == original.ownership() })
    );
    assert!(!executor.status().fail_closed);
}

#[test]
fn fetch_owner_replacement_is_rejected_before_upgrade_refinement_or_request_work() {
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
    executor
        .consume_effects(vec![ordinary], &mut services)
        .expect("admit the incumbent ordinary Fetch owner");

    let certificate = fixture.qc(wire::GlobalPhase::Prepare);
    let sources = certified_sources(&fixture, &certificate);
    let upgrade = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: sources.clone(),
        certificate: Some(certificate.clone()),
    };
    let foreign = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&upgrade),
        vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 9_001)],
    )
    .expect("bind the foreign authority-upgrade owner")
    .pop()
    .expect("one Fetch upgrade has one ownership carrier");
    let before = executor.body_ownership_projection();
    let fetch_tasks_before = services.fetch_tasks.clone();
    let operation_calls_before = services.operation_calls.clone();

    assert!(matches!(
        executor.begin_fetch(
            tag(0),
            fixture.manifest.round,
            fixture.manifest.subject,
            Some(fixture.manifest.clone()),
            sources,
            Some(certificate),
            foreign,
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("changed its exact lifecycle owner")
    ));
    assert_eq!(executor.body_ownership_projection(), before);
    assert_eq!(services.fetch_tasks, fetch_tasks_before);
    assert_eq!(services.operation_calls, operation_calls_before);
}

#[test]
fn adapter_effect_retry_policy_is_closed_over_all_eleven_effect_classes() {
    let fixture = Fixture::new();
    let wire::ConsensusMessageV2Payload::Proposal(mut unsigned_proposal) =
        proposal(&fixture).payload
    else {
        unreachable!("proposal fixture carries a Proposal")
    };
    unsigned_proposal.signature.clear();
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let cases = vec![
        (
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Proposal(unsigned_proposal),
            },
            Some(false),
        ),
        (
            AdapterEffect::Sign {
                tag: tag(0),
                request: SignRequest::Vote(vote(&fixture)),
            },
            Some(false),
        ),
        (timeout_sign(&fixture, 0), Some(false)),
        (
            AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            },
            Some(true),
        ),
        (
            AdapterEffect::StoreBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            },
            Some(true),
        ),
        (
            AdapterEffect::ValidateBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            },
            Some(true),
        ),
        (
            AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: commit,
            },
            Some(true),
        ),
        (AdapterEffect::Broadcast(proposal(&fixture)), None),
        (
            AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_certificate(&fixture),
                protected_lock: None,
            },
            None,
        ),
        (
            AdapterEffect::ReportEquivocation {
                evidence: vote_equivocation_evidence(&fixture, 0),
            },
            None,
        ),
        (
            AdapterEffect::ReportInvalidCertifiedBody {
                subject: fixture.manifest.subject,
                certificate: prepare,
            },
            None,
        ),
    ];
    assert_eq!(cases.len(), 11);
    for (effect, expected) in cases {
        assert_eq!(
            V2EffectExecutor::<FakeRuntime>::candidate_retry_is_redispatched(&effect),
            expected,
            "closed retry classification drifted for {effect:?}"
        );
    }
}

#[test]
fn retained_effect_tail_is_fifo_and_refilters_after_durable_decision() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
    let mut services = fixture.services();
    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("fill signing capacity");
    services.effect_service_order.clear();
    let message = proposal(&fixture);
    executor
        .consume_effects(
            vec![
                AdapterEffect::Broadcast(message.clone()),
                timeout_sign(&fixture, 1),
                AdapterEffect::ReportEquivocation {
                    evidence: vote_equivocation_evidence(&fixture, 1),
                },
            ],
            &mut services,
        )
        .expect("dispatch prefix and retain exact causal suffix");
    assert_eq!(services.effect_service_order, vec!["broadcast"]);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 2);
    let first = services.sign_tasks[0].clone();
    let signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &first.request.signature_preimage(),
    )
    .payload()
    .to_vec();
    executor
        .complete_consensus_signature(first.id(), signature, &mut services)
        .expect("release retained FIFO head");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("drain exact retained suffix"),
        EffectExecutorStep::Advanced { effects: 2 }
    );
    assert_eq!(
        services.effect_service_order,
        vec!["broadcast", "sign", "equivocation"]
    );

    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
    let mut services = fixture.services();
    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("fill signing capacity before Decision");
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let exact_commit = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(commit.clone()),
    );
    executor
        .consume_effects(
            vec![
                timeout_sign(&fixture, 1),
                AdapterEffect::Broadcast(proposal(&fixture)),
                AdapterEffect::Broadcast(exact_commit.clone()),
            ],
            &mut services,
        )
        .expect("retain pre-Decision suffix");
    assert_eq!(executor.status().effect_dispatch_queue.depth, 3);
    executor.runtime.decided_body = Some((
        commit.round,
        commit.proposal_round,
        commit.subject,
        commit.execution_commitment,
    ));
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("Decision refilters retained suffix before retry"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.broadcasts, vec![exact_commit]);
    assert_eq!(services.sign_tasks.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
}

#[test]
fn retained_effect_tail_stops_at_next_blocked_producer_without_reordering() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
    let mut services = fixture.services();
    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("fill signing capacity");
    services.effect_service_order.clear();

    executor
        .consume_effects(
            vec![
                timeout_sign(&fixture, 1),
                AdapterEffect::ReportEquivocation {
                    evidence: vote_equivocation_evidence(&fixture, 1),
                },
                timeout_sign(&fixture, 2),
            ],
            &mut services,
        )
        .expect("retain two producer occurrences and their ordered diagnostic");
    assert_eq!(executor.status().effect_dispatch_queue.depth, 3);
    assert!(services.effect_service_order.is_empty());

    let first = services.sign_tasks[0].clone();
    let signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &first.request.signature_preimage(),
    )
    .payload()
    .to_vec();
    executor
        .complete_consensus_signature(first.id(), signature, &mut services)
        .expect("release the first retained producer");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("drain through the synchronous prefix only"),
        EffectExecutorStep::Advanced { effects: 2 }
    );
    assert_eq!(services.effect_service_order, vec!["sign", "equivocation"]);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert_eq!(executor.pending_signatures.len(), 1);

    let second = services.sign_tasks[1].clone();
    let signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &second.request.signature_preimage(),
    )
    .payload()
    .to_vec();
    executor
        .complete_consensus_signature(second.id(), signature, &mut services)
        .expect("release the next retained producer");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("drain the final producer without overtaking"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(
        services.effect_service_order,
        vec!["sign", "equivocation", "sign"]
    );
    assert_eq!(executor.pending_signatures.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
}

#[test]
fn pending_work_producer_inventory_is_exhaustive_and_source_linked() {
    let fixture = Fixture::new();
    let certificate = fixture.qc(wire::GlobalPhase::Commit);
    let cases = [
        (
            timeout_sign(&fixture, 0),
            Some(PendingWorkProducer::Sign),
            RestartEffectSource::DurableConsensusEvidence,
        ),
        (
            AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            },
            Some(PendingWorkProducer::Fetch),
            RestartEffectSource::BodyReconstruction,
        ),
        (
            AdapterEffect::StoreBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            },
            Some(PendingWorkProducer::Store),
            RestartEffectSource::BodyReconstruction,
        ),
        (
            AdapterEffect::ValidateBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
            },
            Some(PendingWorkProducer::Validate),
            RestartEffectSource::DurableBody,
        ),
        (
            AdapterEffect::Apply {
                tag: tag(0),
                subject: fixture.manifest.subject,
                certificate: certificate.clone(),
            },
            Some(PendingWorkProducer::Apply),
            RestartEffectSource::DurableDecision,
        ),
        (
            AdapterEffect::Broadcast(proposal(&fixture)),
            None,
            RestartEffectSource::DurableConsensusEvidence,
        ),
        (
            AdapterEffect::EnterView {
                tag: tag(1),
                certificate: timeout_at_view(&fixture, 0),
                protected_lock: None,
            },
            None,
            RestartEffectSource::RecoveredView,
        ),
        (
            AdapterEffect::ReportEquivocation {
                evidence: vote_equivocation_evidence(&fixture, 1),
            },
            None,
            RestartEffectSource::DurableAccountabilityEvidence,
        ),
        (
            AdapterEffect::ReportInvalidCertifiedBody {
                subject: fixture.manifest.subject,
                certificate,
            },
            None,
            RestartEffectSource::DiagnosticOnly,
        ),
    ];
    for (effect, expected_producer, expected_restart_source) in cases {
        assert_eq!(
            V2EffectExecutor::<FakeRuntime>::pending_work_producer(&effect),
            expected_producer
        );
        assert_eq!(
            V2EffectExecutor::<FakeRuntime>::restart_effect_source(&effect),
            expected_restart_source
        );
    }
}

#[test]
fn retained_locked_body_reenters_its_exact_origin_store_and_validation_pipeline() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let manifest = manifest_at_view(&fixture, 3);
    let current_tag = tag(3);
    let key = (manifest.round, manifest.subject);

    executor
        .reconcile_locked_body_for_recovery(
            current_tag,
            (manifest.round, manifest.subject),
            &mut services,
        )
        .expect("publish the exact protected proposal origin");
    executor
        .retain_locked_body_for_recovery(
            current_tag,
            manifest.round,
            manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("stage exact locked bytes under their proposal origin");
    assert_eq!(executor.ready_bodies[&key].manifest, manifest);
    assert!(services.fetch_tasks.is_empty());

    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: current_tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("authenticated proposal adopts the retained exact body");
    assert!(services.fetch_tasks.is_empty());
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(tag, completion_manifest))
            if *tag == current_tag && completion_manifest == &manifest
    ));

    executor
        .consume_effects(
            vec![AdapterEffect::StoreBody {
                tag: current_tag,
                round: manifest.round,
                subject: manifest.subject,
            }],
            &mut services,
        )
        .expect("current round requests durable storage");
    let store_id = services.store_tasks[0].id();
    let stored = services.execute_store(store_id);
    executor
        .complete_body_store(stored, &mut services)
        .expect("current-round body store completes");
    executor
        .consume_effects(
            vec![AdapterEffect::ValidateBody {
                tag: current_tag,
                round: manifest.round,
                subject: manifest.subject,
            }],
            &mut services,
        )
        .expect("current round starts deterministic validation");
    let validation_id = services.validation_tasks[0].id();
    let validated = services.execute_validation(validation_id);
    executor
        .complete_body_validation(validated, &mut services)
        .expect("current-round validation completion is rebound to the follower");

    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::ValidationSucceeded(
            tag,
            completion_round,
            completion_subject,
            receipt,
        )) if *tag == current_tag
            && *completion_round == manifest.round
            && *completion_subject == manifest.subject
            && receipt.durable().round() == manifest.round
    ));
    assert!(!executor.status().fail_closed);
}

#[test]
fn retained_locked_body_finishes_an_already_started_exact_origin_fetch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let manifest = manifest_at_view(&fixture, 4);
    let current_tag = tag(4);

    executor
        .reconcile_locked_body_for_recovery(
            current_tag,
            (manifest.round, manifest.subject),
            &mut services,
        )
        .expect("publish the exact protected proposal origin");
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: current_tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("start exact-origin reconstruction");
    let fetch_id = services.fetch_tasks[0].id();
    assert_eq!(executor.pending_fetches.len(), 1);

    executor
        .retain_locked_body_for_recovery(
            current_tag,
            manifest.round,
            manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("trusted locked bytes win the exact-origin acquisition race");

    assert!(services.cancelled_fetches.is_empty());
    assert_eq!(services.completed_reconstruction_fetches, vec![fetch_id]);
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.ready_bodies.len(), 1);
    assert!(matches!(
        executor.runtime.completions.last(),
        Some(RuntimeCompletion::BodyAvailable(tag, completion_manifest))
            if *tag == current_tag && completion_manifest == &manifest
    ));
    assert!(!executor.status().fail_closed);
}

#[test]
fn retained_locked_body_cannot_rebind_to_a_later_proposal_origin() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let original_manifest = manifest_at_view(&fixture, 3);
    let later_manifest = manifest_at_view(&fixture, 4);
    let later_tag = tag(4);

    executor
        .reconcile_locked_body_for_recovery(
            tag(3),
            (original_manifest.round, original_manifest.subject),
            &mut services,
        )
        .expect("publish the original protected proposal origin");
    executor
        .retain_locked_body_for_recovery(
            tag(3),
            original_manifest.round,
            original_manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("retain exact locked bytes at their immutable origin");
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: later_tag,
                round: later_manifest.round,
                subject: later_manifest.subject,
                manifest: Some(later_manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("later proposal cannot adopt protected origin bytes");

    assert_eq!(services.fetch_tasks.len(), 1);
    assert!(
        !executor
            .ready_bodies
            .contains_key(&(later_manifest.round, later_manifest.subject))
    );
    assert!(executor.runtime.completions.is_empty());
    assert!(!executor.status().fail_closed);
}

#[test]
fn same_tag_higher_lock_retires_exact_origin_ownership_before_staging() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let consumer = EventTag::new(1, 3, Generation::new(80));
    let first_lock = (round(&fixture.context, 0), fixture.manifest.subject);
    executor
        .reconcile_locked_body_for_recovery(consumer, first_lock, &mut services)
        .expect("publish the initial exact lock rank");
    executor
        .retain_locked_body_for_recovery(
            consumer,
            first_lock.0,
            fixture.manifest.subject,
            fixture.body.clone(),
            &mut services,
        )
        .expect("stage the first lock under its exact proposal origin");
    let staged = manifest_at_view(&fixture, first_lock.0.view);
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
        .expect("bind the staged cache to a queued reducer completion");
    assert_eq!(first_lock.0, staged.round);
    assert_eq!(executor.ready_bodies.len(), 1);
    assert_eq!(executor.body_pipeline_owners.len(), 1);
    assert_eq!(executor.runtime.completions.len(), 1);

    let (replacement_subject, replacement_body) = distinct_body(&fixture);
    let replacement = (round(&fixture.context, 1), replacement_subject);
    executor
        .reconcile_locked_body_for_recovery(consumer, replacement, &mut services)
        .expect("same-tag higher lock retires every active owner of the old subject");
    assert_eq!(executor.protected_lock, Some(replacement));
    assert!(executor.retained_locked_body.is_none());
    assert!(executor.ready_bodies.is_empty());
    assert!(executor.body_pipeline_owners.is_empty());
    assert!(executor.runtime.completions.is_empty());
    assert_eq!(executor.ready_body_bytes, 0);

    executor
        .retain_locked_body_for_recovery(
            consumer,
            replacement.0,
            replacement_subject,
            replacement_body,
            &mut services,
        )
        .expect("the replacement lock claims the released bounded cache");
    assert_eq!(executor.ready_bodies.len(), 1);
    assert_eq!(
        executor
            .retained_locked_body
            .as_ref()
            .map(|(subject, _)| *subject),
        Some(replacement_subject)
    );
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn same_tag_higher_lock_retires_fetch_store_and_validation_owners() {
    let fixture = Fixture::new();
    let consumer = EventTag::new(1, 3, Generation::new(83));
    let first = (round(&fixture.context, 0), fixture.manifest.subject);
    let (replacement_subject, _) = distinct_body(&fixture);
    let higher = (round(&fixture.context, 1), replacement_subject);
    let staged = manifest_at_view(&fixture, first.0.view);

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .reconcile_locked_body_for_recovery(consumer, first, &mut services)
            .expect("publish fetch-stage lock");
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
            .expect("start exact locked-origin fetch");
        let fetch_id = services.fetch_tasks[0].id();
        executor
            .reconcile_locked_body_for_recovery(consumer, higher, &mut services)
            .expect("higher lock retires superseded fetch ownership");
        assert!(executor.pending_fetches.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert_eq!(services.cancelled_fetches, vec![fetch_id]);
    }

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .reconcile_locked_body_for_recovery(consumer, first, &mut services)
            .expect("publish store-stage lock");
        executor
            .retain_locked_body_for_recovery(
                consumer,
                first.0,
                staged.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage exact bytes");
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
            .expect("bind ready completion");
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
            .expect("start exact locked-origin store");
        let store_id = services.store_tasks[0].id();
        assert_ne!(executor.pending_store_bytes, 0);
        executor
            .reconcile_locked_body_for_recovery(consumer, higher, &mut services)
            .expect("higher lock retires superseded store ownership");
        assert!(executor.pending_stores.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert_eq!(executor.pending_store_bytes, 0);
        assert_eq!(services.cancelled_stores, vec![store_id]);
    }

    {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        executor
            .reconcile_locked_body_for_recovery(consumer, first, &mut services)
            .expect("publish validation-stage lock");
        executor
            .retain_locked_body_for_recovery(
                consumer,
                first.0,
                staged.subject,
                fixture.body.clone(),
                &mut services,
            )
            .expect("stage exact bytes");
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
            .expect("bind ready completion");
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
            .expect("start exact store");
        let store_id = services.store_tasks[0].id();
        let stored = services.execute_store(store_id);
        executor
            .complete_body_store(stored, &mut services)
            .expect("complete exact store");
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
            .expect("start exact locked-origin validation");
        let validation_id = services.validation_tasks[0].id();
        executor
            .reconcile_locked_body_for_recovery(consumer, higher, &mut services)
            .expect("higher lock retires superseded validation ownership");
        assert!(executor.pending_validations.is_empty());
        assert!(executor.body_pipeline_owners.is_empty());
        assert_eq!(services.cancelled_validations, vec![validation_id]);
    }
}

#[test]
fn same_tag_higher_lock_retires_parked_old_body_before_empty_progress_batch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let consumer = EventTag::new(1, 3, Generation::new(91));
    let first = (round(&fixture.context, 0), fixture.manifest.subject);
    let (replacement_subject, _) = distinct_body(&fixture);
    let higher = (round(&fixture.context, 1), replacement_subject);
    executor.runtime.round_tag = Some(consumer);
    executor.reconciled_tag = Some(consumer);
    executor
        .reconcile_locked_body_for_recovery(consumer, first, &mut services)
        .expect("publish the initial lock");

    let parked = AdapterEffect::StoreBody {
        tag: consumer,
        round: first.0,
        subject: first.1,
    };
    let ownership = executor
        .runtime
        .take_effect_ownership(std::slice::from_ref(&parked))
        .expect("bind parked old-body ownership");
    executor
        .retain_effect_batch(vec![parked], ownership)
        .expect("retain the ordinary old-body suffix");
    executor
        .park_retained_effect_batch()
        .expect("park ordinary suffix behind certified progress");

    executor.runtime.locked_body = Some(higher);
    assert_eq!(
        executor
            .consume_effects(Vec::new(), &mut services)
            .expect("empty progress batch still commits its lock frontier"),
        0
    );
    assert_eq!(executor.protected_lock, Some(higher));
    assert!(executor.parked_effect_batch.is_none());
    assert!(executor.retained_effect_batch.is_none());
    assert!(services.store_tasks.is_empty());
    assert!(!executor.status().fail_closed);
}

#[test]
fn next_view_higher_lock_cancels_old_signer_before_fresh_dispatch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let old_tag = tag(1);
    let next_tag = tag(2);
    let first = (round(&fixture.context, 0), fixture.manifest.subject);
    executor.runtime.round_tag = Some(old_tag);
    executor.reconciled_tag = Some(old_tag);
    executor
        .reconcile_locked_body_for_recovery(old_tag, first, &mut services)
        .expect("publish the old lock");

    let mut old_vote = vote(&fixture);
    old_vote.round = first.0;
    old_vote.proposal_round = first.0;
    executor
        .consume_effects(
            vec![AdapterEffect::Sign {
                tag: old_tag,
                request: SignRequest::Vote(old_vote),
            }],
            &mut services,
        )
        .expect("start the old-lock signature");
    let old_sign_id = services.sign_tasks[0].id();

    let parked = AdapterEffect::StoreBody {
        tag: old_tag,
        round: first.0,
        subject: first.1,
    };
    let ownership = executor
        .runtime
        .take_effect_ownership(std::slice::from_ref(&parked))
        .expect("bind parked old-view suffix");
    executor
        .retain_effect_batch(vec![parked], ownership)
        .expect("retain old-view suffix");
    executor
        .park_retained_effect_batch()
        .expect("park old-view suffix");

    let (higher_subject, _) = distinct_body(&fixture);
    let higher_round = round(&fixture.context, 1);
    let higher = (higher_round, higher_subject);
    let mut high_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    high_prepare.round = higher_round;
    high_prepare.proposal_round = higher_round;
    high_prepare.subject = higher_subject;
    let mut timeout = timeout_at_view(&fixture, 1);
    timeout.groups[0].highest_prepare_qc = Some(high_prepare.clone());
    let mut fresh_vote = vote(&fixture);
    fresh_vote.round = higher_round;
    fresh_vote.proposal_round = higher_round;
    fresh_vote.phase = wire::GlobalPhase::Commit;
    fresh_vote.subject = higher_subject;
    executor.runtime.round_tag = Some(next_tag);
    executor.runtime.locked_body = Some(higher);

    executor
        .consume_effects(
            vec![
                AdapterEffect::EnterView {
                    tag: next_tag,
                    certificate: timeout,
                    protected_lock: Some(high_prepare),
                },
                AdapterEffect::Sign {
                    tag: next_tag,
                    request: SignRequest::Vote(fresh_vote),
                },
            ],
            &mut services,
        )
        .expect("certified view transition retires old work before fresh Sign");

    assert!(services.cancelled_signatures.contains(&old_sign_id));
    assert_eq!(executor.pending_signatures.len(), 1);
    assert_eq!(services.sign_tasks.len(), 2);
    assert_eq!(
        services.sign_tasks.last().expect("fresh Sign").tag(),
        next_tag
    );
    assert_eq!(executor.protected_lock, Some(higher));
    assert!(executor.parked_effect_batch.is_none());
    assert!(services.store_tasks.is_empty());
    assert!(!executor.status().fail_closed);
}

#[test]
fn advancing_frontier_without_enter_view_fails_before_dispatch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.round_tag = Some(tag(1));

    assert!(matches!(
        executor.consume_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("omitted its leading EnterView")
    ));
    assert_eq!(executor.runtime.effect_ownership_calls, 0);
    assert!(executor.runtime.effect_owners.is_empty());
    assert!(services.sign_tasks.is_empty());
    assert!(services.entered_views.is_empty());
    assert!(executor.status().fail_closed);
}

#[test]
fn advancing_pacemaker_frontier_without_enter_view_preserves_effect_sidecar() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.round_tag = Some(tag(1));

    assert!(matches!(
        executor.consume_pacemaker_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("omitted its leading EnterView")
    ));
    assert_eq!(executor.runtime.effect_ownership_calls, 0);
    assert!(executor.runtime.effect_owners.is_empty());
    assert!(services.entered_views.is_empty());
    assert!(executor.status().fail_closed);
}

#[test]
fn advancing_recovery_frontier_without_enter_view_preserves_effect_sidecar() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    executor.runtime.round_tag = Some(tag(1));

    assert!(matches!(
        executor.consume_pending_tip_recovery_effects(Vec::new(), &mut services),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("omitted its leading EnterView")
    ));
    assert_eq!(executor.runtime.effect_ownership_calls, 0);
    assert!(executor.runtime.effect_owners.is_empty());
    assert!(services.entered_views.is_empty());
    assert!(executor.status().fail_closed);
}

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
    assert_eq!(
        executor
            .complete_body_validation(duplicate_validation, &mut services)
            .expect("duplicate validation completion"),
        CompletionDisposition::Stale
    );
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
