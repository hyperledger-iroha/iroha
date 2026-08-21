#[test]
fn durable_sign_preemption_orders_speculative_certified_and_locked_fetches() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(4, 8, 1 << 20, 8));
    let mut services = fixture.services();
    let speculative_old = manifest_for_payload(&fixture, b"oldest speculative fetch");
    let speculative_new = manifest_for_payload(&fixture, b"newer speculative fetch");
    let certified = manifest_for_payload(&fixture, b"certified non-lock fetch");
    let locked = manifest_for_payload(&fixture, b"durable locked fetch");
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: speculative_old.round,
                subject: speculative_old.subject,
                manifest: Some(speculative_old),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("start oldest speculative fetch");
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: speculative_new.round,
                subject: speculative_new.subject,
                manifest: Some(speculative_new),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("start newer speculative fetch");
    let certified_qc = prepare_qc_for_subject(certified.round, certified.subject);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: certified.round,
                subject: certified.subject,
                manifest: None,
                certified_sources: certified_sources(&fixture, &certified_qc),
                certificate: Some(certified_qc),
            }],
            &mut services,
        )
        .expect("start certified non-lock fetch");
    let locked_qc = prepare_qc_for_subject(locked.round, locked.subject);
    executor.protected_lock = Some((locked.round, locked.subject));
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: locked.round,
                subject: locked.subject,
                manifest: None,
                certified_sources: certified_sources(&fixture, &locked_qc),
                certificate: Some(locked_qc),
            }],
            &mut services,
        )
        .expect("start protected locked fetch");
    let fetch_ids = services
        .fetch_tasks
        .iter()
        .map(BodyFetchTask::id)
        .collect::<Vec<_>>();
    assert_eq!(executor.pending_work(), 4);
    for view in 0_u64..4 {
        executor
            .consume_effects(vec![timeout_sign(&fixture, view)], &mut services)
            .expect("each durable Sign owns one deterministically preempted slot");
    }
    assert_eq!(services.cancelled_fetches, fetch_ids);
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.pending_signatures.len(), 4);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1 << 20, 2));
    let mut services = fixture.services();
    let decided_qc = fixture.qc(wire::GlobalPhase::Prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: decided_qc.round,
                subject: decided_qc.subject,
                manifest: None,
                certified_sources: certified_sources(&fixture, &decided_qc),
                certificate: Some(decided_qc.clone()),
            }],
            &mut services,
        )
        .expect("start the exact decided-body fetch fixture");
    executor.protected_decision = Some((
        decided_qc.round,
        decided_qc.proposal_round,
        decided_qc.subject,
        decided_qc.execution_commitment,
    ));
    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("decided fetch is protected and Sign remains bounded debt");
    assert!(services.cancelled_fetches.is_empty());
    assert_eq!(executor.pending_fetches.len(), 1);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert!(!executor.status().fail_closed);
    let decided_task = services.fetch_tasks[0].clone();
    executor
        .complete_body_reconstruction(
            &decided_task,
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("exact decided Fetch terminates normally");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("retained Sign drains before another runtime transition"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert!(services.cancelled_fetches.is_empty());
    assert_eq!(executor.pending_fetches.len(), 0);
    assert_eq!(executor.pending_signatures.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
}
#[test]
fn durable_sign_preemption_retires_a_retryable_body_token() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1 << 20, 2));
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
        .expect("fill pending-work capacity with one certified fetch");
    let task = services.fetch_tasks[0].clone();
    let retryable = executor
        .runtime
        .reserve_body_available_with_owner(task.tag, fixture.manifest.clone(), task.ownership())
        .expect("reserve the unpublished BodyAvailable completion");
    assert!(retryable.owns_new_slot());
    assert_eq!(
        executor
            .body_ownership_projection()
            .runtime_body_reservation,
        Some(retryable),
        "the retryable completion must retain its exact Fetch owner",
    );
    assert!(executor.runtime.completions.is_empty());

    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("durable signing preempts the retryable fetch and its token");
    assert_eq!(services.cancelled_fetches, vec![task.id()]);
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.certified_work.is_empty());
    assert!(executor.outstanding_requests.is_empty());
    assert!(executor.runtime.reserved_body_available.is_none());
    assert!(executor.runtime.completions.is_empty());
    assert_eq!(executor.pending_signatures.len(), 1);
    assert!(!executor.output_guard.restart_required());
    assert!(!executor.status().fail_closed);
}
#[test]
fn retained_producer_suffix_allows_exact_payload_chunk_to_release_fetch_capacity() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1 << 20, 2));
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
        .expect("start exact ordinary body acquisition");
    let task = services.fetch_tasks[0].clone();
    executor.protected_decision = Some((
        fixture.manifest.round,
        fixture.manifest.round,
        fixture.manifest.subject,
        fixture_execution_commitment(),
    ));
    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("retain the producer behind decided-body fetch capacity");
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    let chunk = signed_payload_chunk(&fixture);
    assert!(executor.retained_dispatch_allows_network_ingress(
        &wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())
    ));
    assert!(
        !executor.retained_dispatch_allows_network_ingress(&proposal(&fixture).payload),
        "control ingress must not overtake the retained reducer suffix"
    );
    executor
        .accept_payload_chunk(
            task.id(),
            chunk,
            &fixture.context.roster[0].validator,
            &mut services,
        )
        .expect("transport-only chunk reaches the exact live fetch");
    executor
        .complete_body_reconstruction(
            &task,
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("exact chunk reconstruction releases pending-work capacity");
    assert!(executor.pending_fetches.is_empty());
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("released capacity drains the retained producer"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(executor.pending_signatures.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
}
#[test]
fn retained_effect_batch_rejects_overtaking_and_oversize_before_partial_dispatch() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1 << 20, 2));
    let mut services = fixture.services();
    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("fill signing capacity");
    executor
        .consume_effects(vec![timeout_sign(&fixture, 1)], &mut services)
        .expect("retain a second durable Sign");
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert!(matches!(
        executor.consume_effects(
            vec![AdapterEffect::Broadcast(proposal(&fixture))],
            &mut services,
        ),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("overtook retained causal dispatch debt")
    ));
    assert!(services.broadcasts.is_empty());
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let oversized = (0..=MAX_EFFECTS_PER_STEP)
        .map(|_| AdapterEffect::Broadcast(proposal(&fixture)))
        .collect::<Vec<_>>();
    assert!(matches!(
        executor.consume_effects(oversized, &mut services),
        Err(EffectExecutorError::Contract(reason)) if reason.contains("adapter bound")
    ));
    assert!(services.broadcasts.is_empty());
    assert!(executor.status().fail_closed);
}
#[test]
fn exact_candidate_retry_coalesces_under_the_incumbent_owner() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let effect = timeout_sign(&fixture, 0);
    executor
        .consume_effects(vec![effect.clone()], &mut services)
        .expect("dispatch the first exact candidate owner");
    assert_eq!(executor.pending_signatures.len(), 1);
    assert_eq!(services.sign_tasks.len(), 1);
    executor
        .consume_effects(vec![effect.clone()], &mut services)
        .expect("equal-owner retransmission coalesces into the live task");
    assert_eq!(executor.pending_signatures.len(), 1);
    assert_eq!(services.sign_tasks.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    let incumbent_sign_owner = executor
        .pending_signatures
        .values()
        .next()
        .expect("one exact Sign owner remains pending")
        .ownership
        .clone();
    let conflicting = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&effect),
        vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 999)],
    )
    .expect("construct an independently produced semantic retry");
    executor
        .retain_effect_batch(vec![effect.clone()], conflicting)
        .expect("independent causal producer coalesces under the incumbent Sign owner");
    assert_eq!(executor.pending_signatures.len(), 1);
    assert_eq!(services.sign_tasks.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    assert_eq!(
        executor
            .pending_signatures
            .values()
            .next()
            .expect("foreign retry cannot retire the incumbent Sign owner")
            .ownership,
        incumbent_sign_owner
    );
    let reincarnated = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&effect),
        vec![RuntimeEffectOwnership::fresh_for_test(
            EventTag::new(1, 0, Generation::new(9)),
            1_000,
        )],
    )
    .expect("construct a later-incarnation semantic retry");
    executor
        .retain_effect_batch(vec![effect], reincarnated)
        .expect("later causal producer cannot replace the incumbent Sign owner");
    assert_eq!(executor.pending_signatures.len(), 1);
    assert_eq!(services.sign_tasks.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    assert_eq!(
        executor
            .pending_signatures
            .values()
            .next()
            .expect("reincarnation cannot replace the incumbent Sign owner")
            .ownership,
        incumbent_sign_owner
    );
    let mut fetch_executor = fixture.executor(EffectQueueConfig::default());
    let mut fetch_services = fixture.services();
    let fetch_effect = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    fetch_executor
        .consume_effects(vec![fetch_effect.clone()], &mut fetch_services)
        .expect("admit one exact Fetch stage owner");
    let incumbent = fetch_executor
        .pending_fetches
        .values()
        .next()
        .expect("ordinary acquisition retained Fetch work")
        .task
        .ownership()
        .clone();
    let retry_effects = vec![AdapterEffect::Broadcast(proposal(&fixture)), fetch_effect];
    let foreign = bind_adapter_effect_batch_ownership(
        &retry_effects,
        vec![
            RuntimeEffectOwnership::fresh_for_test(tag(0), 1_001),
            RuntimeEffectOwnership::fresh_for_test(tag(0), 1_002),
        ],
    )
    .expect("construct a two-effect retry with a foreign Fetch owner");
    assert_eq!(
        foreign[1].candidate_semantic_identity(),
        incumbent.candidate_semantic_identity(),
        "the stage discriminator and six-coordinate statement are identical"
    );
    let foreign_owner = foreign[1].owner().clone();
    assert_ne!(foreign[1].owner(), incumbent.owner());
    fetch_executor
        .retain_effect_batch(retry_effects, foreign)
        .expect("exact duplicate Fetch adopts the incumbent task owner");
    assert_eq!(
        fetch_executor
            .drain_retained_effect_batch(&mut fetch_services, false)
            .expect("redispatch the exact incumbent Fetch task"),
        2,
        "the unrelated broadcast and coalesced Fetch each dispatch once"
    );
    assert_eq!(fetch_executor.pending_fetches.len(), 1);
    assert_eq!(fetch_services.fetch_tasks.len(), 2);
    let retained_fetch = fetch_executor
        .pending_fetches
        .values()
        .next()
        .expect("the incumbent Fetch task remains live");
    assert_eq!(retained_fetch.task.ownership(), &incumbent);
    assert!(
        fetch_services
            .fetch_tasks
            .iter()
            .all(|task| task.ownership() == &incumbent),
        "every physical Fetch call keeps the original task owner"
    );
    assert!(fetch_executor.retained_effect_batch.is_none());
    let external = fetch_executor
        .external_lifecycle_owners()
        .expect("inspect external lifecycle ownership after coalescing the retry");
    assert!(
        external.iter().all(|owner| owner != incumbent.owner()),
        "passive network Fetch ownership is deliberately not runnable clock work"
    );
    assert!(external.iter().all(|owner| owner != &foreign_owner));
    let mut services = fixture.services();
    let four_candidates = (0..4)
        .map(|signer| timeout_sign(&fixture, signer))
        .collect::<Vec<_>>();
    let assignments = (0..4)
        .map(|ordinal| RuntimeEffectOwnership::fresh_for_test(tag(0), 2_000 + ordinal))
        .collect::<Vec<_>>();
    assert!(matches!(
        bind_adapter_effect_batch_ownership(&four_candidates, assignments),
        Err(reason) if reason.contains("causal-successor bound")
    ));
    assert!(services.sign_tasks.is_empty());
    assert!(executor.retained_effect_batch.is_none());
    let mut executor = fixture.executor(EffectQueueConfig::default());
    assert!(matches!(
        executor.consume_effects(four_candidates, &mut services),
        Err(EffectExecutorError::Runtime(reason))
            if reason.contains("effect batch exceeded the causal-successor bound")
    ));
    assert!(services.sign_tasks.is_empty());
    assert!(executor.retained_effect_batch.is_none());
    assert!(executor.output_guard.restart_required());
    assert!(executor.status().fail_closed);
}
#[test]
fn runtime_terminal_body_candidate_stutters_before_cached_redispatch() {
    let fixture = Fixture::new();
    let cases = [
        AdapterEffect::StoreBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
        },
        AdapterEffect::ValidateBody {
            tag: tag(0),
            round: fixture.manifest.round,
            subject: fixture.manifest.subject,
        },
    ];
    for effect in cases {
        let mut executor = fixture.executor(EffectQueueConfig::default());
        let mut services = fixture.services();
        let root = executor.runtime.test_effect_ownership(&effect);
        let bound = bind_adapter_effect_batch_ownership(std::slice::from_ref(&effect), vec![root])
            .expect("bind the exact cached body candidate")
            .pop()
            .expect("one body effect has one candidate owner");
        let identity = bound
            .candidate_semantic_identity()
            .expect("Store/Validate has one route-neutral identity");
        executor
            .runtime
            .terminal_body_candidate_owners
            .insert(identity, bound.clone());
        executor
            .consume_effects(vec![effect.clone()], &mut services)
            .expect("the runtime terminal turns the retry into a 1-to-1 stutter");
        assert!(executor.pending_stores.is_empty());
        assert!(executor.pending_durable_validate_admissions.is_empty());
        assert!(services.store_tasks.is_empty());
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.retained_effect_batch.is_none());
        assert!(!executor.status().fail_closed);
        let foreign = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(&effect),
            vec![RuntimeEffectOwnership::fresh_for_test(tag(0), 90_001)],
        )
        .expect("bind a foreign terminal retry")
        .pop()
        .expect("one foreign body-stage owner");
        assert_ne!(foreign, bound);
        executor
            .retain_effect_batch(vec![effect], vec![foreign])
            .expect("an exact foreign carrier stutters under the terminal owner");
        assert_eq!(executor.runtime.terminal_body_candidate_commits, 2);
        assert_eq!(
            executor.runtime.terminal_body_candidate_owners[&identity], bound,
            "candidate-specific terminal ownership remains unchanged"
        );
        assert!(executor.pending_stores.is_empty());
        assert!(executor.pending_durable_validate_admissions.is_empty());
        assert!(services.store_tasks.is_empty());
        assert!(executor.runtime.completions.is_empty());
        assert!(executor.retained_effect_batch.is_none());
        assert!(!executor.status().fail_closed);
    }
}
#[test]
fn runtime_terminal_authority_commits_only_after_full_positional_gate() {
    let fixture = Fixture::new();
    let effect = AdapterEffect::StoreBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let sibling = timeout_sign(&fixture, 0);
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let roots = vec![
        executor.runtime.test_effect_ownership(&effect),
        executor.runtime.test_effect_ownership(&sibling),
    ];
    let malformed_batch =
        bind_adapter_effect_batch_ownership(&[effect.clone(), sibling.clone()], roots)
            .expect("bind a two-effect terminal carrier");
    let incumbent = malformed_batch[0].clone();
    let identity = incumbent
        .candidate_semantic_identity()
        .expect("StoreBody has one route-neutral identity");
    executor
        .runtime
        .terminal_body_candidate_owners
        .insert(identity, incumbent.clone());
    assert!(matches!(
        executor.retain_effect_batch(vec![effect.clone()], vec![incumbent.clone()]),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("exact candidate-ownership refinement")
    ));
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits, 0,
        "a malformed positional binding cannot refine the runtime terminal"
    );
    assert!(executor.retained_effect_batch.is_none());
    let roots = vec![
        executor.runtime.test_effect_ownership(&effect),
        executor.runtime.test_effect_ownership(&sibling),
    ];
    let mut later_malformed =
        bind_adapter_effect_batch_ownership(&[effect.clone(), sibling.clone()], roots)
            .expect("bind a two-effect terminal carrier");
    later_malformed[1] = later_malformed[1]
        .rebind_same_adapter_effect(&sibling)
        .expect("make only the later positional binding malformed");
    let incumbent = later_malformed[0].clone();
    let identity = incumbent
        .candidate_semantic_identity()
        .expect("StoreBody retains its route-neutral identity");
    executor
        .runtime
        .terminal_body_candidate_owners
        .insert(identity, incumbent.clone());
    assert!(matches!(
        executor.retain_effect_batch(vec![effect.clone(), sibling], later_malformed),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("exact candidate-ownership refinement")
    ));
    assert_eq!(
        executor.runtime.terminal_body_candidate_commits, 0,
        "a valid first terminal cannot commit before a later batch position fails"
    );
    assert!(executor.retained_effect_batch.is_none());
    let exact = incumbent
        .rebind_same_adapter_effect(&effect)
        .expect("rebind the same terminal owner to a one-effect batch");
    executor
        .retain_effect_batch(vec![effect], vec![exact])
        .expect("the exact 1-to-1 terminal retry stutters");
    assert_eq!(executor.runtime.terminal_body_candidate_commits, 1);
    assert!(executor.retained_effect_batch.is_none());
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
    let ordinary_ownership = authenticated_proposal_fetch_ownership(&fixture, &ordinary, 9_023);
    executor.runtime.exact_effect_ownership = Some((ordinary.clone(), ordinary_ownership));
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
fn enter_view_and_ordinary_fetch_retry_preserve_authenticated_replay_owner() {
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
    let ordinary_ownership = authenticated_proposal_fetch_ownership(&fixture, &ordinary, 9_024);
    executor.runtime.exact_effect_ownership = Some((ordinary.clone(), ordinary_ownership));
    executor
        .consume_effects(vec![ordinary.clone()], &mut services)
        .expect("start the authenticated ordinary protected-body acquisition");
    let original = services.fetch_tasks[0].clone();
    assert!(
        original
            .ownership()
            .exact_remote_proposal_fetch_replay(&ordinary)
            .is_some()
    );

    executor.runtime.effect_owners.clear();
    executor.runtime.round_tag = Some(tag(1));
    executor.runtime.locked_body = Some((fixture.manifest.round, fixture.manifest.subject));
    let prepare = fixture.qc(wire::GlobalPhase::Prepare);
    let mut timeout = timeout_at_view(&fixture, 0);
    timeout.groups[0].highest_prepare_qc = Some(prepare.clone());
    let mut retry = ordinary;
    let AdapterEffect::FetchBody { tag: retry_tag, .. } = &mut retry else {
        unreachable!("ordinary fixture remains FetchBody")
    };
    *retry_tag = tag(1);
    executor
        .consume_effects(
            vec![
                AdapterEffect::EnterView {
                    tag: tag(1),
                    certificate: timeout,
                    protected_lock: Some(prepare),
                },
                retry.clone(),
            ],
            &mut services,
        )
        .expect("post-EnterView ordinary retry retains the authenticated Fetch owner");

    assert_eq!(executor.pending_fetches.len(), 1);
    let pending = executor
        .pending_fetches
        .values()
        .next()
        .expect("one protected Fetch remains live");
    assert_eq!(pending.task.tag, tag(1));
    assert_eq!(pending.task.ownership(), original.ownership());
    assert!(
        pending
            .task
            .ownership()
            .exact_remote_proposal_fetch_replay(&retry)
            .is_some(),
        "the post-prefix Same carrier must keep the exact authenticated Proposal envelope"
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
            None,
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
    let mut executor = fixture.executor(EffectQueueConfig::new(2, 4, 1 << 20, 4));
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
    assert!(services.effect_service_order.is_empty());
    assert_eq!(executor.status().effect_dispatch_queue.depth, 2);
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    // The production runner settles this exact Broadcast owner before it
    // re-enters the retained suffix. Keep the move-only owner alive locally
    // while this executor-only fixture exercises the subsequent FIFO drain.
    let output_key = *executor
        .pending_lifecycle_output_admissions
        .keys()
        .next()
        .expect("one parked Broadcast owner");
    let _settled_broadcast_owner = executor
        .pending_lifecycle_output_admissions
        .remove(&output_key)
        .expect("transfer the exact Broadcast owner to lifecycle settlement");
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
    assert_eq!(services.effect_service_order, vec!["sign"]);
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
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
    assert!(services.broadcasts.is_empty());
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
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
            .expect("drain only the first producer before output capacity"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.effect_service_order, vec!["sign"]);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 2);
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
            .expect("transfer the ordered diagnostic before the final producer"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.effect_service_order, vec!["sign"]);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    assert!(executor.pending_signatures.is_empty());
    // Production settles the move-only output owner before re-entering this
    // retained suffix. Model that transfer without executing diagnostic I/O.
    let output_key = *executor
        .pending_lifecycle_output_admissions
        .keys()
        .next()
        .expect("one parked equivocation owner");
    let _settled_equivocation_owner = executor
        .pending_lifecycle_output_admissions
        .remove(&output_key)
        .expect("transfer the exact equivocation owner to lifecycle settlement");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("drain the final producer without overtaking"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.effect_service_order, vec!["sign", "sign"]);
    assert_eq!(executor.pending_signatures.len(), 1);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
}
#[test]
fn lifecycle_output_waits_at_saturated_pending_capacity_without_losing_fifo_owner() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
    let mut services = fixture.services();
    executor
        .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
        .expect("fill the sole pending-work position");
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let output = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(commit),
    ));
    assert_eq!(
        executor
            .consume_effects(vec![output.clone()], &mut services)
            .expect("retain lifecycle output behind saturated pending work"),
        0
    );
    assert!(executor.pending_lifecycle_output_admissions.is_empty());
    let retained = executor
        .retained_effect_batch
        .as_ref()
        .expect("retain the exact output occurrence");
    let retained_output = retained.effects.front().expect("one retained output");
    assert_eq!(retained_output.effect, output);
    assert!(
        retained_output
            .ownership
            .exactly_binds_adapter_effect(&retained_output.effect)
    );
    let sign = services.sign_tasks[0].clone();
    let signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &sign.request.signature_preimage(),
    )
    .payload()
    .to_vec();
    executor
        .complete_consensus_signature(sign.id(), signature, &mut services)
        .expect("release pending-work capacity");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("transfer the retained output into lifecycle admission"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    assert_eq!(executor.status().pending_outputs, 1);
    assert!(executor.retained_effect_batch.is_none());
    assert!(services.broadcasts.is_empty());
    assert!(!executor.status().fail_closed);
}
#[test]
fn lifecycle_output_key_collision_preserves_incumbent_move_only_owner() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::default());
    let commit = fixture.qc(wire::GlobalPhase::Commit);
    let output = AdapterEffect::Broadcast(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::QuorumCertificate(commit),
    ));
    let first_assignment = executor.runtime.test_effect_ownership(&output);
    let first =
        bind_adapter_effect_batch_ownership(std::slice::from_ref(&output), vec![first_assignment])
            .expect("bind the one-effect incumbent output")
            .pop()
            .expect("one incumbent output binding");
    executor
        .park_lifecycle_output_admission(output.clone(), first.clone())
        .expect("park the incumbent output owner");

    let companion = timeout_sign(&fixture, 1);
    let foreign_effects = [output.clone(), companion.clone()];
    let output_assignment = executor.runtime.test_effect_ownership(&output);
    let companion_assignment = executor.runtime.test_effect_ownership(&companion);
    let foreign = bind_adapter_effect_batch_ownership(
        &foreign_effects,
        vec![output_assignment, companion_assignment],
    )
    .expect("bind a foreign positional occurrence of the same output")
    .remove(0);
    assert!(!first.exactly_matches_bound_effect_occurrence(&foreign, &output));
    assert!(matches!(
        executor.park_lifecycle_output_admission(output.clone(), foreign.clone()),
        Err(EffectExecutorError::Contract(message))
            if message == "lifecycle output admission key collided with a foreign owner"
    ));
    assert_eq!(executor.pending_lifecycle_output_admissions.len(), 1);
    let incumbent = executor
        .pending_lifecycle_output_admissions
        .values()
        .next()
        .expect("incumbent output owner survived the collision");
    assert!(incumbent.exactly_matches_retry(&output, &first));
    assert!(!incumbent.exactly_matches_retry(&output, &foreign));
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
            Some(PendingWorkProducer::Output),
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
            Some(PendingWorkProducer::Output),
            RestartEffectSource::DurableAccountabilityEvidence,
        ),
        (
            AdapterEffect::ReportInvalidCertifiedBody {
                subject: fixture.manifest.subject,
                certificate,
            },
            Some(PendingWorkProducer::Output),
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
