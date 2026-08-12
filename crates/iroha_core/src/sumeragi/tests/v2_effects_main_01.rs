#[test]
fn queue_configuration_rejects_zero_and_pending_capacity_retains_causal_tail() {
    let fixture = Fixture::new();
    assert!(matches!(
        V2EffectExecutor::with_runtime(
            FakeRuntime::default(),
            BTreeMap::new(),
            fixture.context.clone(),
            PeerId::new(fixture.requester_key.public_key().clone()),
            Some(0),
            EffectQueueConfig::new(0, 1, 1, 1),
        ),
        Err(EffectExecutorError::InvalidQueueConfig)
    ));

    let mut executor = fixture.executor(EffectQueueConfig::new(1, 1, 1_048_576, 1));
    let mut services = fixture.services();
    persist_fsynced_validation_marker(
        &mut executor,
        &mut services,
        &fixture,
        fixture.manifest.clone(),
    );
    assert!(executor.can_admit_local_proposal());
    let effects = (0_u64..3)
        .map(|view| timeout_sign(&fixture, view))
        .collect::<Vec<_>>();
    assert_eq!(
        executor
            .consume_effects(effects, &mut services)
            .expect("retain the capacity-blocked causal suffix"),
        1
    );
    assert_eq!(executor.status().pending_signatures, 1);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 2);
    assert_eq!(
        executor.status().effect_dispatch_queue.capacity,
        2 * MAX_EFFECTS_PER_STEP
    );
    assert_eq!(executor.status().effect_dispatch_queue.max_service_debt, 0);
    assert!(!executor.can_admit_local_proposal());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());

    let first = services.sign_tasks[0].clone();
    let signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &first.request.signature_preimage(),
    )
    .payload()
    .to_vec();
    executor
        .complete_consensus_signature(first.id(), signature, &mut services)
        .expect("release the first signing slot");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("drain retained signing debt before another runtime step"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.sign_tasks.len(), 2);
    assert_eq!(executor.status().pending_signatures, 1);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert_eq!(
        executor.status().effect_dispatch_queue.max_service_debt,
        0,
        "capacity retry is not scheduler debt and cannot transfer between FIFO heads"
    );

    let second = services.sign_tasks[1].clone();
    let signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &second.request.signature_preimage(),
    )
    .payload()
    .to_vec();
    executor
        .complete_consensus_signature(second.id(), signature, &mut services)
        .expect("release the second signing slot");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("drain the final retained signing effect"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.sign_tasks.len(), 3);
    assert_eq!(executor.status().pending_signatures, 1);
    assert_eq!(executor.status().effect_dispatch_queue.depth, 0);
    assert!(!executor.status().fail_closed);
}

#[test]
fn executor_threads_its_independent_pending_bound_into_runtime_ownership() {
    let fixture = Fixture::new();
    let config = EffectQueueConfig::default();
    let executor = fixture.executor(config);
    assert_eq!(
        executor.runtime.external_lifecycle_owner_capacity,
        Some(config.max_pending_work + 2 * MAX_EFFECTS_PER_STEP)
    );
}

#[test]
fn proposal_a_distinct_prepare_qc_b_and_timeout_sign_progress_at_capacity_two() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(2, 4, 1 << 20, 4));
    let mut services = fixture.services();

    // reducer.rs::on_proposal: an authenticated Proposal A with a missing
    // body emits the ordinary reconstruction request.
    fixture
        .manifest
        .validate(&fixture.context)
        .expect("Proposal A manifest is structurally valid");
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
        .expect("Proposal A starts ordinary reconstruction");
    let proposal_a_work = services.fetch_tasks[0].id();

    // reducer.rs::on_prepare_qc: a valid same-view PrepareQC for distinct
    // subject B is independently progress-relevant and starts a certified
    // reconstruction owner.
    let (subject_b, _) = distinct_body(&fixture);
    assert_ne!(subject_b, fixture.manifest.subject);
    let prepare_b = prepare_qc_for_subject(fixture.manifest.round, subject_b);
    prepare_b
        .validate(&fixture.context)
        .expect("distinct PrepareQC B is structurally valid");
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: prepare_b.round,
                subject: prepare_b.subject,
                manifest: None,
                certified_sources: certified_sources(&fixture, &prepare_b),
                certificate: Some(prepare_b.clone()),
            }],
            &mut services,
        )
        .expect("PrepareQC B starts certified reconstruction");
    assert_eq!(executor.pending_work(), 2);

    // reducer.rs::on_timeout: durable TimeoutVote signing must not fail
    // closed behind either body source. It deterministically retires the
    // lower-evidence Proposal A fetch and owns the released slot.
    assert_eq!(
        executor
            .consume_effects(vec![timeout_sign(&fixture, 0)], &mut services)
            .expect("timeout signing preempts reconstructible work"),
        1
    );
    assert_eq!(services.cancelled_fetches, vec![proposal_a_work]);
    assert_eq!(executor.pending_work(), 2);
    assert_eq!(executor.pending_signatures.len(), 1);
    assert_eq!(executor.pending_fetches.len(), 1);
    assert!(executor.pending_fetches.values().all(|pending| {
        pending.task.round == prepare_b.round && pending.task.subject == prepare_b.subject
    }));
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
    assert!(services.closed.is_empty());
}

#[test]
fn passive_fetch_does_not_block_prepare_qc_or_timeout_in_serialized_runtime() {
    let ProductionTransportFixture {
        context,
        validator_keys,
        requester_key,
        ..
    } = ProductionTransportFixture::new();
    let proofs = validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified =
        VerifiedHeightContext::genesis(context.clone(), proofs).expect("verified context");
    let directory = TempDir::new().expect("serialized capacity-trace directory");
    let (adapter, startup_effects) = SumeragiV2Adapter::open(
        directory.path().join("capacity-trace-safety.wal"),
        verified,
        Some(0),
        Generation::new(1),
        [0x74; 32],
        AdapterFingerprints {
            node: Hash::new(b"capacity trace node"),
            build: Hash::new(b"capacity trace build"),
            config: Hash::new(b"capacity trace config"),
        },
        DeferredAdmissionOrdinalSource::new(0),
    )
    .expect("open source-faithful adapter");
    assert!(startup_effects.is_empty());
    let started = Instant::now();
    let (runtime, startup_effects) = SerializedV2Runtime::new(
        adapter,
        startup_effects,
        started,
        Duration::from_secs(10),
        RuntimeQueueConfig::new(8, 2, 2),
    )
    .expect("serialized runtime");
    assert!(startup_effects.is_empty());
    let mut executor = V2EffectExecutor::with_runtime(
        runtime,
        BTreeMap::new(),
        context.clone(),
        PeerId::new(requester_key.public_key().clone()),
        Some(0),
        EffectQueueConfig::new(2, 4, 1 << 20, 4),
    )
    .expect("capacity-two executor");
    executor
        .arm_live_clocks(started)
        .expect("arm source-faithful timeout");
    assert!(
        executor
            .can_schedule_local_proposal()
            .expect("armed active-view producer is available"),
        "the production preflight must expose the reserved one-shot local producer",
    );
    let mut services = FakeServices {
        requester_key: Some(requester_key),
        ..FakeServices::default()
    };

    let round = round(&context, 0);
    let body_a = b"authenticated Proposal A body".to_vec();
    let subject_a = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"Proposal A block")),
        payload_hash: Hash::new(&body_a),
    };
    let manifest_a = canonical_payload_manifest(&context, round, subject_a, &body_a);
    let proposer = context.leader(0);
    let mut proposal_a = wire::Proposal {
        round,
        proposer,
        subject: subject_a,
        manifest: manifest_a,
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    proposal_a.signature = Signature::new(
        validator_keys[usize::try_from(proposer).expect("leader index")].private_key(),
        &proposal_a.signature_preimage(),
    )
    .payload()
    .to_vec();
    executor
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::Proposal(proposal_a),
        ))
        .expect("authenticate Proposal A through production ingress");
    for _ in 0..8 {
        let _ = executor
            .step(started, &mut services)
            .expect("drive Proposal A reducer transition");
        if executor.pending_fetches.len() == 1 {
            break;
        }
    }
    assert_eq!(executor.pending_fetches.len(), 1);
    executor
        .publish_external_lifecycle_owners()
        .expect("publish runnable asynchronous ownership");
    assert_eq!(
        executor.runtime.external_lifecycle_owner_count(),
        0,
        "a passive network fetch must not become the actor-global scheduler minimum"
    );
    let proposal_a_work = services.fetch_tasks[0].id();

    let body_b = b"distinct certified subject B".to_vec();
    let subject_b = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"PrepareQC B block")),
        payload_hash: Hash::new(&body_b),
    };
    let commitment_b = fixture_execution_commitment();
    let signers = vec![0, 1, 2];
    let vote_preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject_b,
        execution_commitment: commitment_b,
        signer: signers[0],
        signature: Vec::new(),
    }
    .signature_preimage();
    let shares = signers
        .iter()
        .map(|signer| {
            Signature::new(
                validator_keys[usize::try_from(*signer).expect("signer index")].private_key(),
                &vote_preimage,
            )
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let share_refs = shares.iter().map(Vec::as_slice).collect::<Vec<_>>();
    let prepare_b = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject_b,
        execution_commitment: commitment_b,
        signers,
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&share_refs)
            .expect("aggregate PrepareQC B"),
    };
    executor
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(prepare_b.clone()),
        ))
        .expect("authenticate distinct PrepareQC B through production ingress");
    for _ in 0..8 {
        let _ = executor
            .step(started, &mut services)
            .expect("drive PrepareQC B reducer transition");
        if executor.pending_fetches.len() == 2 {
            break;
        }
    }
    assert_eq!(executor.pending_fetches.len(), 2);

    let timeout_now = started + Duration::from_secs(30);
    for _ in 0..8 {
        let _ = executor
            .step(timeout_now, &mut services)
            .expect("drive durable timeout transition");
        if !executor.pending_signatures.is_empty() {
            break;
        }
    }
    assert_eq!(services.cancelled_fetches, vec![proposal_a_work]);
    assert_eq!(executor.pending_signatures.len(), 1);
    assert_eq!(executor.pending_fetches.len(), 1);
    assert!(executor.pending_fetches.values().all(|pending| {
        pending.task.round == prepare_b.round && pending.task.subject == prepare_b.subject
    }));
    assert!(matches!(
        services.sign_tasks.last().map(|task| &task.request),
        Some(SignRequest::TimeoutVote(_))
    ));
    assert!(!executor.status().fail_closed);
}

#[test]
fn late_passive_fetch_completion_issues_one_serve_predecessor_episode_and_steps() {
    let mut fixture = ProductionTransportFixture::new();
    let fetch_ordinal = fixture
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve the passive Fetch lifecycle before Serve");
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        4_000,
        0,
    );
    let signature =
        SignatureOf::try_from_hash(fixture.validator_keys[0].private_key(), header.hash())
            .expect("late Fetch block signature");
    let block = SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
    let body = block
        .encode_wire()
        .expect("late Fetch canonical block wire");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&body),
    };
    let manifest = canonical_payload_manifest(&fixture.context, fixture.round, subject, &body);
    let fetch = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.round,
        subject,
        manifest: Some(manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let ownership = bind_adapter_effect_batch_ownership(
        std::slice::from_ref(&fetch),
        vec![RuntimeEffectOwnership::fresh_for_test(
            tag(0),
            fetch_ordinal,
        )],
    )
    .expect("bind the passive Fetch to the shared actor ordinal");
    fixture
        .executor
        .retain_effect_batch(vec![fetch], ownership)
        .expect("retain the production-shaped Fetch effect");
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    assert_eq!(
        fixture
            .executor
            .drain_retained_effect_batch(&mut services, true)
            .expect("dispatch the passive Fetch"),
        1
    );
    let task = services
        .fetch_tasks
        .first()
        .expect("Fetch service owns the passive request")
        .clone();
    assert_eq!(task.lifecycle_ordinal(), fetch_ordinal);
    let serve_ordinal = fixture
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve the selected Serve target after Fetch");
    assert!(
        fixture
            .executor
            .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
            .expect("observe the selected Serve before Fetch completion")
            .is_none(),
        "passive Fetch transport work alone cannot block Serve"
    );

    fixture
        .executor
        .complete_body_reconstruction(&task, manifest, body, &mut services)
        .expect("late reconstruction materializes BodyAvailable under the Fetch owner");
    let witness = fixture
        .executor
        .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
        .expect("observe late BodyAvailable behind the selected Serve")
        .expect("late runnable predecessor reopens the completed Serve episode");
    assert_eq!(witness.serve_lifecycle_ordinal(), serve_ordinal);
    assert_eq!(witness.predecessor_lifecycle_ordinal(), fetch_ordinal);
    assert_eq!(witness.episode(), 1);
    let retained_response_ordinal = fixture
        .lifecycle_ordinals
        .reserve_one()
        .expect("reserve an isolated retained-response target after Serve");
    assert!(
        fixture
            .executor
            .older_runtime_lifecycle_predates_retained_response(
                Instant::now(),
                retained_response_ordinal,
            )
            .expect("exercise the published retained-response predecessor probe")
    );
    assert_eq!(
        fixture
            .executor
            .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
            .expect("retained-response probing cannot reset the selected-Serve witness"),
        Some(witness),
        "one continuous predecessor prefix retains one witness across target probes"
    );
    assert_eq!(
        fixture.executor.status().queued_runtime_completions,
        1,
        "the late Fetch successor is runnable inside serialized runtime"
    );

    assert!(matches!(
        fixture
            .executor
            .step(Instant::now(), &mut services)
            .expect("the reopened predecessor owns the next serialized step"),
        EffectExecutorStep::Advanced { .. }
    ));
    assert_eq!(fixture.executor.status().queued_runtime_completions, 0);
    assert_eq!(
        services.store_tasks.len(),
        1,
        "the reopened BodyAvailable transition must produce one Store successor"
    );
    assert_eq!(
        services.store_tasks[0].lifecycle_ordinal(),
        fetch_ordinal,
        "the Store successor must keep the reopened Fetch owner"
    );
    assert!(fixture.executor.pending_fetches.is_empty());
    assert!(
        fixture
            .executor
            .exact_serve_predecessor_episode_witness(Instant::now(), serve_ordinal, None)
            .expect("an incomplete Store remains passive")
            .is_none(),
        "pending Store work alone cannot reopen the Serve episode"
    );
    let stored_completion_evidence =
        ExactServePredecessorCompletionEvidence::try_new(fetch_ordinal)
            .expect("tracked Store completion retains the exact Fetch ordinal");
    let replenished = fixture
        .executor
        .exact_serve_predecessor_episode_witness(
            Instant::now(),
            serve_ordinal,
            Some(stored_completion_evidence),
        )
        .expect("a completed Store is runnable")
        .expect("a completed Store reopens one later Serve episode");
    assert_eq!(replenished.predecessor_lifecycle_ordinal(), fetch_ordinal);
    assert_eq!(replenished.episode(), 2);
    assert!(!fixture.executor.status().fail_closed);
}

#[test]
fn full_capacity_certified_fetch_retains_its_exact_owner_until_capacity_releases() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 4, 1 << 20, 4));
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
        .expect("fill the only slot with Proposal A reconstruction");
    let proposal_a_task = services.fetch_tasks[0].clone();

    let (subject_b, _) = distinct_body(&fixture);
    let prepare_b = prepare_qc_for_subject(fixture.manifest.round, subject_b);
    let fetch_b = AdapterEffect::FetchBody {
        tag: tag(0),
        round: prepare_b.round,
        subject: prepare_b.subject,
        manifest: None,
        certified_sources: certified_sources(&fixture, &prepare_b),
        certificate: Some(prepare_b.clone()),
    };
    assert_eq!(
        executor
            .consume_effects(vec![fetch_b], &mut services)
            .expect("full-capacity Fetch retains its exact reducer effect owner"),
        0
    );
    assert_eq!(services.fetch_tasks.len(), 1);
    assert!(
        !executor
            .body_pipeline_owners
            .contains_key(&(prepare_b.round, prepare_b.subject))
    );
    assert!(executor.retained_effect_batch.is_some());
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    let retained_ownership = executor
        .retained_effect_batch
        .as_ref()
        .and_then(|batch| batch.effects.front())
        .expect("capacity-blocked Fetch B retains its exact FIFO owner")
        .ownership
        .clone();
    assert!(!executor.status().fail_closed);

    executor
        .complete_body_reconstruction(
            &proposal_a_task,
            fixture.manifest.clone(),
            fixture.body.clone(),
            &mut services,
        )
        .expect("Proposal A reconstruction terminates and releases capacity");
    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("released capacity drains the already-admitted Fetch B"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(services.fetch_tasks.len(), 2);
    let admitted_b = executor
        .pending_fetches
        .values()
        .find(|pending| {
            pending.task.round == prepare_b.round && pending.task.subject == prepare_b.subject
        })
        .expect("Fetch B acquires one pending service owner");
    assert_eq!(admitted_b.task.ownership(), &retained_ownership);
    assert!(executor.retained_effect_batch.is_none());
    assert!(!executor.status().fail_closed);
}

#[test]
fn certified_request_pressure_cannot_suppress_timeout_signing_or_lose_fetch_owner() {
    let mut fixture = ProductionTransportFixture::new_validator();
    fixture.executor.config = EffectQueueConfig::new(2, 4, 1 << 20, 1);
    fixture.executor.outstanding_requests =
        OutstandingCertifiedBodyRequests::new(1).expect("one certified-request slot");
    fixture.executor.recovered_bodies.clear();
    let started = Instant::now();
    fixture
        .executor
        .arm_live_clocks(started)
        .expect("arm source-faithful timeout/retransmission clocks");
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };

    let certificate_a =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let tag_a = fixture.executor.current_tag();
    let sources_a = fixture.certified_sources(&certificate_a);
    fixture
        .executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag_a,
                round: fixture.round,
                subject: fixture.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources_a,
                certificate: Some(certificate_a),
            }],
            &mut services,
        )
        .expect("A occupies the sole certified-request slot");
    let task_a = services.fetch_tasks[0].clone();
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);

    let body_b = b"source-faithful certified-request debt B".to_vec();
    let subject_b = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"source-faithful certified-request block B",
        )),
        payload_hash: Hash::new(&body_b),
    };
    let certificate_b = fixture.quorum_certificate_for(
        fixture.round,
        subject_b,
        wire::GlobalPhase::Prepare,
        fixture.conflicting_commitment,
    );
    fixture
        .executor
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate_b.clone()),
        ))
        .expect("authenticate PrepareQC B through production ingress");
    assert!(matches!(
        fixture
            .executor
            .step(started, &mut services)
            .expect("PrepareQC B emits the source-refined FetchBody"),
        EffectExecutorStep::Advanced { .. }
    ));

    assert_eq!(services.fetch_tasks.len(), 1);
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);
    assert!(fixture.executor.pending_fetches.values().all(|pending| {
        pending.task.round != certificate_b.round || pending.task.subject != certificate_b.subject
    }));
    assert!(
        !fixture
            .executor
            .body_pipeline_owners
            .contains_key(&(certificate_b.round, certificate_b.subject)),
        "request-saturated Fetch B must not gain partial pipeline ownership"
    );
    assert!(fixture.executor.retained_effect_batch.is_some());
    assert_eq!(fixture.executor.status().effect_dispatch_queue.depth, 1);
    let retained_b_ownership = fixture
        .executor
        .retained_effect_batch
        .as_ref()
        .and_then(|batch| batch.effects.front())
        .expect("request-saturated Fetch B retains its exact runtime owner")
        .ownership
        .clone();

    // Fetch B remains exact ordinary debt, but transport capacity is not
    // pacemaker authority. The absolute timeout gets one typed turn,
    // preempts the reconstructible Fetch A slot, and leaves B parked with
    // its original lifecycle owner.
    let timeout_now = started + Duration::from_secs(30);
    assert_eq!(
        fixture
            .executor
            .step(timeout_now, &mut services)
            .expect("request pressure cannot suppress timeout signing"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    assert_eq!(fixture.executor.pending_signatures.len(), 1);
    assert_eq!(services.sign_tasks.len(), 1);
    assert_eq!(services.cancelled_fetches, vec![task_a.id()]);
    assert_eq!(fixture.executor.outstanding_requests.len(), 0);
    assert!(fixture.executor.retained_effect_batch.is_none());
    assert!(fixture.executor.parked_effect_batch.is_some());
    assert_eq!(fixture.executor.status().effect_dispatch_queue.depth, 1);
    assert_eq!(
        fixture
            .executor
            .status()
            .effect_dispatch_queue
            .max_service_debt,
        1
    );

    assert_eq!(
        fixture
            .executor
            .step(timeout_now, &mut services)
            .expect("the exact parked Fetch B resumes after the control turn"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    let admitted_b = fixture
        .executor
        .pending_fetches
        .values()
        .find(|pending| {
            pending.task.round == certificate_b.round
                && pending.task.subject == certificate_b.subject
        })
        .expect("Fetch B acquires one exact pending owner");
    assert_eq!(admitted_b.task.ownership(), &retained_b_ownership);
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);
    assert_eq!(services.fetch_tasks.len(), 2);
    assert_eq!(services.sign_tasks.len(), 1);
    assert!(fixture.executor.retained_effect_batch.is_none());
    assert!(fixture.executor.parked_effect_batch.is_none());
    assert!(!fixture.executor.status().fail_closed);
}

#[test]
fn serialized_runtime_retained_retry_atomically_upgrades_existing_fetch_after_request_pressure() {
    let mut fixture = ProductionTransportFixture::new_validator();
    fixture.executor.config = EffectQueueConfig::new(3, 4, 1 << 20, 1);
    fixture.executor.outstanding_requests =
        OutstandingCertifiedBodyRequests::new(1).expect("one certified-request slot");
    fixture.executor.recovered_bodies.clear();
    let started = Instant::now();
    fixture
        .executor
        .arm_live_clocks(started)
        .expect("arm production retransmission clocks");
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };

    let certificate_a =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let tag_a = fixture.executor.current_tag();
    let sources_a = fixture.certified_sources(&certificate_a);
    fixture
        .executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag_a,
                round: fixture.round,
                subject: fixture.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: sources_a,
                certificate: Some(certificate_a),
            }],
            &mut services,
        )
        .expect("A occupies the sole certified-request slot");
    let task_a = services.fetch_tasks[0].clone();
    let request_hash_a = HashOf::new(
        task_a
            .certified_request()
            .expect("A owns the sole certified request"),
    );
    assert_eq!(
        fixture.executor.outstanding_requests.hashes(),
        BTreeSet::from([request_hash_a])
    );
    assert_eq!(
        fixture.executor.certified_work,
        BTreeMap::from([(request_hash_a, task_a.id())])
    );
    assert!(
        fixture
            .executor
            .outstanding_requests
            .plan_retirement(request_hash_a)
            .is_ok(),
        "A is present in both exact outstanding-request indexes"
    );

    let proposal_b_message = fixture.signed_normal_proposal(0xb);
    let wire::ConsensusMessageV2Payload::Proposal(proposal_b) = &proposal_b_message.payload else {
        panic!("normal production fixture emits a Proposal")
    };
    let round_b = proposal_b.round;
    let subject_b = proposal_b.subject;
    fixture
        .executor
        .enqueue_network(proposal_b_message)
        .expect("authenticate Proposal B through production ingress");
    for _ in 0..8 {
        let _ = fixture
            .executor
            .step(started, &mut services)
            .expect("drive Proposal B reducer transition");
        if services
            .fetch_tasks
            .iter()
            .any(|task| task.round == round_b && task.subject == subject_b)
        {
            break;
        }
    }
    let ordinary_b = services
        .fetch_tasks
        .iter()
        .find(|task| task.round == round_b && task.subject == subject_b)
        .expect("Proposal B creates an ordinary body-fetch service task")
        .clone();
    let work_id_b = ordinary_b.id();
    assert!(ordinary_b.certified_request().is_none());
    assert!(ordinary_b.sources.is_empty());
    let pending_a_before_upgrade = fixture.executor.pending_fetches[&task_a.id()].clone();
    let pending_b_before_upgrade = fixture.executor.pending_fetches[&work_id_b].clone();
    let pipeline_owners_before_upgrade = fixture.executor.body_pipeline_owners.clone();
    let certified_work_before_upgrade = fixture.executor.certified_work.clone();
    let outstanding_before_upgrade = fixture.executor.outstanding_requests.hashes();
    let next_work_id_before_upgrade = fixture.executor.next_work_id;
    let service_tasks_before_upgrade = services.fetch_tasks.len();
    assert_eq!(service_tasks_before_upgrade, 2);
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(1));

    let certificate_b = fixture.quorum_certificate_for(
        round_b,
        subject_b,
        wire::GlobalPhase::Prepare,
        fixture.conflicting_commitment,
    );
    fixture
        .executor
        .enqueue_network(wire::ConsensusMessageV2::new(
            wire::ConsensusMessageV2Payload::QuorumCertificate(certificate_b.clone()),
        ))
        .expect("authenticate PrepareQC B through production ingress");
    assert!(matches!(
        fixture
            .executor
            .step(started, &mut services)
            .expect("drive pressured PrepareQC B transition"),
        EffectExecutorStep::Advanced { .. }
    ));

    assert_eq!(fixture.executor.next_work_id, next_work_id_before_upgrade);
    assert_eq!(services.fetch_tasks.len(), service_tasks_before_upgrade);
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(1));
    assert_eq!(
        fixture.executor.pending_fetches.get(&task_a.id()),
        Some(&pending_a_before_upgrade)
    );
    assert_eq!(
        fixture.executor.pending_fetches.get(&work_id_b),
        Some(&pending_b_before_upgrade),
        "request pressure must leave B's exact ordinary owner unchanged"
    );
    assert_eq!(
        fixture.executor.body_pipeline_owners,
        pipeline_owners_before_upgrade
    );
    assert_eq!(
        fixture.executor.certified_work,
        certified_work_before_upgrade
    );
    assert_eq!(
        fixture.executor.outstanding_requests.hashes(),
        outstanding_before_upgrade
    );
    assert!(
        fixture
            .executor
            .outstanding_requests
            .plan_retirement(request_hash_a)
            .is_ok(),
        "the pressured upgrade preserves A in both request indexes"
    );
    assert!(
        fixture.executor.pending_fetches[&work_id_b]
            .task
            .certified_request()
            .is_none()
    );
    assert!(fixture.executor.retained_effect_batch.is_some());
    assert_eq!(fixture.executor.status().effect_dispatch_queue.depth, 1);

    let mut response_a = wire::CertifiedBodyResponse {
        request_hash: request_hash_a,
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: 0,
        signature: Vec::new(),
    };
    response_a.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response_a.signature_preimage(),
    )
    .payload()
    .to_vec();
    assert_eq!(
        fixture
            .executor
            .accept_certified_body_response(
                response_a,
                &fixture.context.roster[0].validator,
                &mut services,
            )
            .expect("authenticated A response releases request capacity"),
        CompletionDisposition::Accepted
    );
    assert!(fixture.executor.outstanding_requests.is_empty());
    assert!(fixture.executor.certified_work.is_empty());
    assert_eq!(
        fixture.executor.pending_fetches[&work_id_b], pending_b_before_upgrade,
        "releasing A does not replace B's ordinary service owner"
    );

    assert_eq!(
        fixture
            .executor
            .step(started, &mut services)
            .expect("retry the original admitted upgrade after capacity release"),
        EffectExecutorStep::Advanced { effects: 1 }
    );

    let upgraded_b = services
        .fetch_tasks
        .last()
        .expect("retained retry reaches the body-fetch service");
    let request_b = upgraded_b
        .certified_request()
        .expect("retained retry certifies B's existing fetch");
    let request_hash_b = HashOf::new(request_b);
    assert_eq!(upgraded_b.id(), work_id_b);
    assert_eq!(upgraded_b.round, round_b);
    assert_eq!(upgraded_b.subject, subject_b);
    assert_eq!(request_b.certificate, certificate_b);
    assert_eq!(fixture.executor.next_work_id, next_work_id_before_upgrade);
    assert_eq!(services.fetch_tasks.len(), service_tasks_before_upgrade + 1);
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(2));
    assert_eq!(
        upgraded_b.ownership, ordinary_b.ownership,
        "certifying an existing acquisition preserves its immutable owner"
    );
    assert_eq!(
        upgraded_b.lifecycle_ordinal(),
        ordinary_b.lifecycle_ordinal(),
        "authority refinement cannot allocate a second Fetch lifecycle"
    );
    assert_eq!(
        fixture.executor.pending_fetches[&work_id_b].task,
        *upgraded_b
    );
    assert_eq!(
        fixture.executor.pending_fetches[&work_id_b].request_hash,
        Some(request_hash_b)
    );
    assert_eq!(
        fixture.executor.outstanding_requests.hashes(),
        BTreeSet::from([request_hash_b])
    );
    assert_eq!(
        fixture.executor.certified_work,
        BTreeMap::from([(request_hash_b, work_id_b)])
    );
    assert!(
        fixture
            .executor
            .outstanding_requests
            .plan_retirement(request_hash_b)
            .is_ok(),
        "B is atomically installed in both outstanding-request indexes"
    );
    assert_eq!(
        fixture
            .executor
            .body_pipeline_owners
            .get(&(round_b, subject_b)),
        pipeline_owners_before_upgrade.get(&(round_b, subject_b))
    );
    assert!(fixture.executor.retained_effect_batch.is_none());
    assert_eq!(fixture.executor.status().effect_dispatch_queue.depth, 0);
    assert!(!fixture.executor.status().fail_closed);

    assert_eq!(
        fixture
            .executor
            .step(started, &mut services)
            .expect("idle after the admitted upgrade drains"),
        EffectExecutorStep::Idle
    );
    assert_eq!(
        services.fetch_tasks.len(),
        service_tasks_before_upgrade + 1,
        "draining the retry must not create an unowned service poll"
    );
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(2));
    assert!(fixture.executor.retained_effect_batch.is_none());
}

#[test]
fn certified_request_pressure_retains_higher_authority_upgrade_under_one_owner() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(2, 4, 1 << 20, 1));
    let mut services = fixture.services();
    let first_prepare = fixture.qc(wire::GlobalPhase::Prepare);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(0),
                round: fixture.manifest.round,
                subject: fixture.manifest.subject,
                manifest: Some(fixture.manifest.clone()),
                certified_sources: certified_sources(&fixture, &first_prepare),
                certificate: Some(first_prepare),
            }],
            &mut services,
        )
        .expect("admit the sole certified-request owner");
    let first_task = services.fetch_tasks[0].clone();

    let (second_subject, second_body) = distinct_body(&fixture);
    let second_round = round(&fixture.context, 1);
    let second_manifest =
        canonical_payload_manifest(&fixture.context, second_round, second_subject, &second_body);
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag: tag(1),
                round: second_round,
                subject: second_subject,
                manifest: Some(second_manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut services,
        )
        .expect("admit ordinary acquisition at the higher view");
    let second_work_id = services.fetch_tasks[1].id();
    let higher_prepare = prepare_qc_for_subject(second_round, second_subject);
    let certified_upgrade = AdapterEffect::FetchBody {
        tag: tag(1),
        round: second_round,
        subject: second_subject,
        manifest: Some(second_manifest),
        certified_sources: certified_sources(&fixture, &higher_prepare),
        certificate: Some(higher_prepare),
    };

    assert_eq!(
        executor
            .consume_effects(vec![certified_upgrade], &mut services)
            .expect("request pressure retains the exact authority upgrade"),
        0
    );
    assert_eq!(executor.pending_work(), 2);
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert_eq!(services.fetch_tasks.len(), 2);
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(1));
    assert!(
        executor
            .pending_fetches
            .get(&second_work_id)
            .is_some_and(|pending| pending.task.certified_request().is_none()),
        "the ordinary acquisition remains live without a partial authority upgrade"
    );
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert!(executor.retained_effect_batch.is_some());
    assert!(!executor.status().fail_closed);

    let response = signed_certified_response(
        &fixture,
        &first_task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    assert_eq!(
        executor
            .accept_certified_body_response(
                response,
                &fixture.context.roster[0].validator,
                &mut services,
            )
            .expect("exact first response releases request capacity"),
        CompletionDisposition::Accepted
    );
    assert!(executor.outstanding_requests.is_empty());

    assert_eq!(
        executor
            .step(Instant::now(), &mut services)
            .expect("capacity release retries the already-admitted authority upgrade"),
        EffectExecutorStep::Idle
    );
    assert_eq!(executor.pending_work(), 1);
    assert_eq!(executor.outstanding_requests.len(), 1);
    assert_eq!(services.fetch_tasks.len(), 3);
    assert_eq!(services.fetch_tasks[2].id(), second_work_id);
    assert!(services.fetch_tasks[2].certified_request().is_some());
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(2));
    assert!(executor.retained_effect_batch.is_some());
    assert_eq!(executor.status().effect_dispatch_queue.depth, 1);
    assert!(!executor.status().fail_closed);
}

#[test]
fn reconstructible_new_certified_fetch_acquires_ownership_from_retained_admission() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.config = EffectQueueConfig::new(2, 4, 1 << 20, 1);
    fixture.executor.outstanding_requests =
        OutstandingCertifiedBodyRequests::new(1).expect("one certified-request slot");
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };

    let certificate_a =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let sources_a = fixture.certified_sources(&certificate_a);
    let fetch_a_effects = vec![AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.round,
        subject: fixture.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: sources_a,
        certificate: Some(certificate_a),
    }];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&fetch_a_effects)
        .expect("bind production retransmit ownership for Fetch A");
    fixture
        .executor
        .consume_effects(fetch_a_effects, &mut services)
        .expect("A acquires the sole certified-request slot");
    let task_a = services.fetch_tasks[0].clone();
    let request_hash_a = HashOf::new(
        task_a
            .certified_request()
            .expect("A owns a certified request"),
    );
    assert_eq!(fixture.executor.pending_work(), 1);
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);
    assert_eq!(
        fixture.executor.certified_work.get(&request_hash_a),
        Some(&task_a.id())
    );

    let round_b = round(&fixture.context, 1);
    let body_b = b"independent production certified body B".to_vec();
    let subject_b = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"independent production certified block B",
        )),
        payload_hash: Hash::new(&body_b),
    };
    let manifest_b = canonical_payload_manifest(&fixture.context, round_b, subject_b, &body_b);
    let durable_b = DurableBodyReceipt::for_test(
        fixture.context.id(),
        round_b,
        subject_b,
        HashOf::new(&manifest_b),
    );
    let commitment_b = ValidatedBodyReceipt::for_test(durable_b).execution_commitment();
    let certificate_b = fixture.quorum_certificate_for(
        round_b,
        subject_b,
        wire::GlobalPhase::Prepare,
        commitment_b,
    );
    let sources_b = fixture.certified_sources(&certificate_b);
    let fetch_b = AdapterEffect::FetchBody {
        tag: tag(1),
        round: round_b,
        subject: subject_b,
        manifest: Some(manifest_b.clone()),
        certified_sources: sources_b,
        certificate: Some(certificate_b),
    };
    let next_work_id_before_b = fixture.executor.next_work_id;
    let fetch_b_effects = vec![fetch_b];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&fetch_b_effects)
        .expect("bind production retransmit ownership for Fetch B");
    assert_eq!(
        fixture
            .executor
            .consume_effects(fetch_b_effects, &mut services)
            .expect("certified-request pressure retains independent Fetch B"),
        0
    );

    assert_eq!(fixture.executor.pending_work(), 1);
    assert_eq!(fixture.executor.next_work_id, next_work_id_before_b);
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);
    assert_eq!(fixture.executor.certified_work.len(), 1);
    assert_eq!(
        fixture.executor.certified_work.get(&request_hash_a),
        Some(&task_a.id())
    );
    assert_eq!(services.fetch_tasks.len(), 1);
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(1));
    assert!(
        fixture
            .executor
            .pending_fetches
            .values()
            .all(|pending| { pending.task.round != round_b || pending.task.subject != subject_b }),
        "B must not gain partial pending-work ownership"
    );
    assert!(
        !fixture
            .executor
            .body_pipeline_owners
            .contains_key(&(round_b, subject_b)),
        "B must not gain partial pipeline ownership"
    );
    assert_eq!(fixture.executor.status().effect_dispatch_queue.depth, 1);
    assert!(fixture.executor.retained_effect_batch.is_some());
    assert!(!fixture.executor.status().fail_closed);

    let mut response_a = wire::CertifiedBodyResponse {
        request_hash: request_hash_a,
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: 0,
        signature: Vec::new(),
    };
    response_a.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response_a.signature_preimage(),
    )
    .payload()
    .to_vec();
    let response_envelope = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response_a.clone()),
    );
    let responder = fixture.context.roster[0].validator.clone();
    let ingress_ownership = certified_response_ingress_ownership(&response_a, responder.clone());
    assert!(
        fixture
            .executor
            .can_admit_network_message_with_ingress_ownership(
                &response_envelope,
                &ingress_ownership,
            ),
        "the exact transport completion remains admissible under request pressure"
    );
    assert_eq!(
        fixture
            .executor
            .accept_certified_body_response_with_ingress_ownership(
                response_a,
                &responder,
                &ingress_ownership,
                &mut services,
            )
            .expect("exact A response releases certified-request capacity"),
        CompletionDisposition::Accepted
    );
    assert!(fixture.executor.outstanding_requests.is_empty());
    assert!(fixture.executor.certified_work.is_empty());

    assert_eq!(
        fixture
            .executor
            .step(Instant::now(), &mut services)
            .expect("released capacity drains the admitted independent Fetch B"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    let task_b = services
        .fetch_tasks
        .last()
        .expect("B is admitted after A completes");
    let request_hash_b = HashOf::new(
        task_b
            .certified_request()
            .expect("B atomically owns a certified request"),
    );
    assert_eq!(services.fetch_tasks.len(), 2);
    assert_eq!(task_b.round, round_b);
    assert_eq!(task_b.subject, subject_b);
    assert_eq!(fixture.executor.pending_work(), 1);
    assert_eq!(
        fixture.executor.next_work_id,
        next_work_id_before_b
            .checked_add(1)
            .expect("work ID advances once")
    );
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);
    assert_eq!(
        fixture.executor.certified_work.get(&request_hash_b),
        Some(&task_b.id())
    );
    assert!(fixture.executor.pending_fetches.contains_key(&task_b.id()));
    assert!(
        fixture
            .executor
            .body_pipeline_owners
            .contains_key(&(round_b, subject_b))
    );
    assert_eq!(services.operation_calls.get("body-sign").copied(), Some(2));
    assert!(fixture.executor.retained_effect_batch.is_none());
    assert!(!fixture.executor.status().fail_closed);
}

#[test]
fn production_capacity_saturation_admits_response_and_reconstructible_fetch() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };

    let initial_queues = fixture.executor.status().runtime_queues;
    assert_eq!(initial_queues.normal.capacity, 639);
    assert_eq!(initial_queues.progress.capacity, 767);
    assert_eq!(initial_queues.completion.capacity, 1_023);
    assert_eq!(
        fixture.executor.runtime.remaining_completion_capacity(),
        1_023
    );

    let request_capacity = fixture.executor.config.max_certified_requests;
    assert_eq!(request_capacity, 256);
    let generation = fixture.executor.current_tag().generation();
    for view in 0..request_capacity {
        let view = u64::try_from(view).expect("request view");
        let request_round = round(&fixture.context, view);
        let manifest = canonical_payload_manifest(
            &fixture.context,
            request_round,
            fixture.subject,
            &fixture.body,
        );
        let durable = DurableBodyReceipt::for_test(
            fixture.context.id(),
            request_round,
            fixture.subject,
            HashOf::new(&manifest),
        );
        let commitment = ValidatedBodyReceipt::for_test(durable).execution_commitment();
        if view == 0 {
            assert_eq!(manifest, fixture.manifest);
            assert_eq!(commitment, fixture.canonical_commitment);
        }
        let certificate = fixture.quorum_certificate_for(
            request_round,
            fixture.subject,
            wire::GlobalPhase::Prepare,
            commitment,
        );
        let certified_sources = fixture.certified_sources(&certificate);
        let effects = vec![AdapterEffect::FetchBody {
            tag: EventTag::new(fixture.context.height, view, generation),
            round: request_round,
            subject: fixture.subject,
            manifest: Some(manifest),
            certified_sources,
            certificate: Some(certificate),
        }];
        fixture
            .executor
            .runtime
            .retain_retransmit_effect_ownership_for_test(&effects)
            .expect("bind production retransmit lifecycle ownership");
        assert_eq!(
            fixture
                .executor
                .consume_effects(effects, &mut services)
                .expect("fill production certified-request ownership"),
            1
        );
    }
    let task_a = services
        .fetch_tasks
        .first()
        .expect("view-zero certified owner")
        .clone();
    let request_hash_a = HashOf::new(
        task_a
            .certified_request()
            .expect("view-zero fetch owns its request"),
    );
    assert_eq!(task_a.round, fixture.round);
    assert_eq!(task_a.subject, fixture.subject);
    assert_eq!(fixture.executor.pending_work(), request_capacity);
    assert_eq!(
        fixture.executor.outstanding_requests.len(),
        request_capacity
    );
    assert_eq!(fixture.executor.certified_work.len(), request_capacity);
    assert_eq!(services.fetch_tasks.len(), request_capacity);
    assert_eq!(
        services.operation_calls.get("body-sign").copied(),
        Some(request_capacity)
    );

    for ordinal in 0..initial_queues.normal.capacity {
        let message = fixture
            .signed_normal_proposal(u64::try_from(ordinal).expect("normal saturation ordinal"));
        assert!(fixture.executor.can_admit_network_message(&message));
        fixture
            .executor
            .enqueue_network(message)
            .expect("authenticate and admit production Normal ingress");
    }
    let blocked_normal = fixture.signed_normal_proposal(
        u64::try_from(initial_queues.normal.capacity).expect("blocked Normal ordinal"),
    );
    assert!(!fixture.executor.can_admit_network_message(&blocked_normal));

    let progress_reserve = initial_queues
        .progress
        .capacity
        .checked_sub(initial_queues.normal.capacity)
        .expect("production Progress reserve");
    for offset in 0..progress_reserve {
        let view = 10_000_u64
            .checked_add(u64::try_from(offset).expect("progress saturation offset"))
            .expect("progress saturation view");
        let message = fixture.signed_timeout_vote(view);
        assert!(fixture.executor.can_admit_network_message(&message));
        fixture
            .executor
            .enqueue_network(message)
            .expect("authenticate and admit production Progress ingress");
    }
    let blocked_progress = fixture.signed_timeout_vote(
        10_000_u64
            .checked_add(u64::try_from(progress_reserve).expect("blocked Progress offset"))
            .expect("blocked Progress view"),
    );
    assert!(
        !fixture
            .executor
            .can_admit_network_message(&blocked_progress)
    );

    let saturated_queues = fixture.executor.status().runtime_queues;
    assert_eq!(
        saturated_queues.normal.depth,
        initial_queues.normal.capacity
    );
    assert_eq!(saturated_queues.progress.depth, progress_reserve);
    assert_eq!(saturated_queues.completion.depth, 0);
    assert_eq!(
        fixture.executor.runtime.queued_commands(),
        initial_queues.progress.capacity
    );
    assert_eq!(
        fixture.executor.runtime.remaining_completion_capacity(),
        initial_queues
            .completion
            .capacity
            .checked_sub(initial_queues.progress.capacity)
            .expect("production Completion reserve")
    );

    let round_b = round(
        &fixture.context,
        u64::try_from(request_capacity).expect("deferred Fetch B view"),
    );
    let body_b = b"production-saturation deferred certified body B".to_vec();
    let subject_b = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"production-saturation deferred certified block B",
        )),
        payload_hash: Hash::new(&body_b),
    };
    let manifest_b = canonical_payload_manifest(&fixture.context, round_b, subject_b, &body_b);
    let durable_b = DurableBodyReceipt::for_test(
        fixture.context.id(),
        round_b,
        subject_b,
        HashOf::new(&manifest_b),
    );
    let commitment_b = ValidatedBodyReceipt::for_test(durable_b).execution_commitment();
    let certificate_b = fixture.quorum_certificate_for(
        round_b,
        subject_b,
        wire::GlobalPhase::Prepare,
        commitment_b,
    );
    let sources_b = fixture.certified_sources(&certificate_b);
    let fetch_b = AdapterEffect::FetchBody {
        tag: EventTag::new(fixture.context.height, round_b.view, generation),
        round: round_b,
        subject: subject_b,
        manifest: Some(manifest_b.clone()),
        certified_sources: sources_b,
        certificate: Some(certificate_b),
    };
    let next_work_id_before_b = fixture.executor.next_work_id;
    let pending_fetches_before_b = fixture.executor.pending_fetches.clone();
    let pipeline_owners_before_b = fixture.executor.body_pipeline_owners.clone();
    let certified_work_before_b = fixture.executor.certified_work.clone();
    let outstanding_requests_before_b = fixture.executor.outstanding_requests.hashes();
    let fetch_b_effects = vec![fetch_b];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&fetch_b_effects)
        .expect("bind deferred production retransmit lifecycle ownership");
    assert_eq!(
        fixture
            .executor
            .consume_effects(fetch_b_effects, &mut services)
            .expect("defer Fetch B at production certified-request capacity"),
        0
    );
    assert_eq!(fixture.executor.next_work_id, next_work_id_before_b);
    assert_eq!(fixture.executor.pending_fetches, pending_fetches_before_b);
    assert_eq!(
        fixture.executor.body_pipeline_owners,
        pipeline_owners_before_b
    );
    assert_eq!(fixture.executor.certified_work, certified_work_before_b);
    assert_eq!(
        fixture.executor.outstanding_requests.hashes(),
        outstanding_requests_before_b
    );
    assert_eq!(fixture.executor.pending_work(), request_capacity);
    assert_eq!(
        fixture.executor.outstanding_requests.len(),
        request_capacity
    );
    assert_eq!(fixture.executor.certified_work.len(), request_capacity);
    assert_eq!(services.fetch_tasks.len(), request_capacity);
    assert_eq!(
        services.operation_calls.get("body-sign").copied(),
        Some(request_capacity)
    );
    assert!(
        fixture
            .executor
            .pending_fetches
            .values()
            .all(|pending| { pending.task.round != round_b || pending.task.subject != subject_b })
    );
    assert!(
        !fixture
            .executor
            .body_pipeline_owners
            .contains_key(&(round_b, subject_b)),
        "deferred Fetch B must not gain partial pipeline ownership"
    );
    assert_eq!(fixture.executor.status().effect_dispatch_queue.depth, 1);
    assert!(fixture.executor.retained_effect_batch.is_some());
    assert!(!fixture.executor.status().fail_closed);

    let mut response_a = wire::CertifiedBodyResponse {
        request_hash: request_hash_a,
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: 0,
        signature: Vec::new(),
    };
    response_a.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response_a.signature_preimage(),
    )
    .payload()
    .to_vec();
    let response_envelope = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response_a.clone()),
    );
    let responder = fixture.context.roster[0].validator.clone();
    let ingress_ownership = certified_response_ingress_ownership(&response_a, responder.clone());
    assert!(
        fixture
            .executor
            .can_admit_network_message_with_ingress_ownership(
                &response_envelope,
                &ingress_ownership,
            ),
        "the exact response must cross the full reducer ingress prefix"
    );
    assert_eq!(
        fixture
            .executor
            .accept_certified_body_response_with_ingress_ownership(
                response_a,
                &responder,
                &ingress_ownership,
                &mut services,
            )
            .expect("reserve and enqueue exact BodyAvailable completion"),
        CompletionDisposition::Accepted
    );
    assert_eq!(services.completed_certified_fetches, vec![task_a.id()]);
    assert_eq!(
        fixture.executor.outstanding_requests.len(),
        request_capacity - 1
    );
    assert_eq!(fixture.executor.certified_work.len(), request_capacity - 1);
    assert!(
        !fixture
            .executor
            .certified_work
            .contains_key(&request_hash_a)
    );
    assert!(!fixture.executor.pending_fetches.contains_key(&task_a.id()));
    assert!(
        fixture
            .executor
            .ready_bodies
            .get(&(fixture.round, fixture.subject))
            .is_some_and(|ready| ready.manifest == fixture.manifest)
    );
    let completion_queues = fixture.executor.status().runtime_queues;
    assert_eq!(
        completion_queues.normal.depth,
        initial_queues.normal.capacity
    );
    assert_eq!(completion_queues.progress.depth, progress_reserve);
    assert_eq!(completion_queues.completion.depth, 1);
    assert_eq!(
        fixture.executor.runtime.queued_commands(),
        initial_queues.progress.capacity + 1
    );
    assert_eq!(
        fixture.executor.runtime.remaining_completion_capacity(),
        initial_queues.completion.capacity - initial_queues.progress.capacity - 1
    );
    assert_eq!(fixture.executor.status().effect_dispatch_queue.depth, 1);
    assert!(!fixture.executor.status().fail_closed);

    assert_eq!(
        fixture
            .executor
            .step(Instant::now(), &mut services)
            .expect("released capacity drains the admitted Fetch B"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    let task_b = services
        .fetch_tasks
        .last()
        .expect("retained Fetch B reaches the production service");
    let request_hash_b = HashOf::new(
        task_b
            .certified_request()
            .expect("Fetch B atomically owns a certified request"),
    );
    assert_eq!(services.fetch_tasks.len(), request_capacity + 1);
    assert_eq!(task_b.round, round_b);
    assert_eq!(task_b.subject, subject_b);
    assert_eq!(fixture.executor.pending_work(), request_capacity);
    assert_eq!(
        fixture.executor.outstanding_requests.len(),
        request_capacity
    );
    assert_eq!(fixture.executor.certified_work.len(), request_capacity);
    assert_eq!(
        fixture.executor.certified_work.get(&request_hash_b),
        Some(&task_b.id())
    );
    assert_eq!(
        fixture.executor.next_work_id,
        next_work_id_before_b
            .checked_add(1)
            .expect("Fetch B advances work ownership once")
    );
    assert!(fixture.executor.pending_fetches.contains_key(&task_b.id()));
    assert!(
        fixture
            .executor
            .body_pipeline_owners
            .contains_key(&(round_b, subject_b))
    );
    assert_eq!(
        services.operation_calls.get("body-sign").copied(),
        Some(request_capacity + 1)
    );
    assert!(fixture.executor.retained_effect_batch.is_none());
    let final_queues = fixture.executor.status().runtime_queues;
    assert_eq!(final_queues.normal.depth, initial_queues.normal.capacity);
    assert_eq!(final_queues.progress.depth, progress_reserve);
    assert_eq!(final_queues.completion.depth, 1);
    assert_eq!(
        fixture.executor.runtime.remaining_completion_capacity(),
        initial_queues.completion.capacity - initial_queues.progress.capacity - 1
    );
    assert!(!fixture.executor.status().fail_closed);
}

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
    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    services.retry_certified_fetch_once = true;
    assert_eq!(
        executor.accept_certified_body_response(
            response,
            &fixture.context.roster[0].validator,
            &mut services,
        ),
        Err(EffectTransportError::Backpressure),
    );
    assert!(
        executor
            .body_ownership_projection()
            .runtime_body_reservation
            .is_some(),
        "typed Retryable owns the unpublished Completion token",
    );

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
fn retained_producer_suffix_allows_exact_certified_response_to_release_fetch_capacity() {
    let fixture = Fixture::new();
    let mut executor = fixture.executor(EffectQueueConfig::new(1, 2, 1 << 20, 1));
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
        .expect("start exact certified body acquisition");
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

    let response = signed_certified_response(
        &fixture,
        &task,
        fixture.manifest.clone(),
        fixture.body.clone(),
        0,
    );
    assert!(executor.retained_dispatch_allows_network_ingress(
        &wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response.clone())
    ));
    let discovery_response = wire::CommitCertificateResponse {
        request_hash: HashOf::from_untyped_unchecked(Hash::new(
            b"retained suffix CommitQC request",
        )),
        certificate: fixture.qc(wire::GlobalPhase::Commit),
        responder: fixture.context.roster[0].validator.clone(),
        signature: Vec::new(),
    };
    assert!(
        executor.retained_dispatch_allows_network_ingress(
            &wire::ConsensusMessageV2Payload::CommitCertificateResponse(discovery_response)
        ),
        "an authenticated discovery CommitQC can retire a hung signing fence"
    );
    assert_eq!(
        executor
            .accept_certified_body_response(
                response,
                &fixture.context.roster[0].validator,
                &mut services,
            )
            .expect("transport-only response reaches the exact live fetch"),
        CompletionDisposition::Accepted
    );
    assert!(executor.pending_fetches.is_empty());
    assert!(executor.outstanding_requests.is_empty());
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
    .expect("construct an independently owned mutation candidate");
    assert!(matches!(
        executor.retain_effect_batch(vec![effect.clone()], conflicting),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("owner replacement")
    ));
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
    .expect("construct a local-incarnation owner replacement");
    assert!(matches!(
        executor.retain_effect_batch(vec![effect], reincarnated),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("owner replacement")
    ));
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
    assert!(matches!(
        fetch_executor.retain_effect_batch(retry_effects, foreign),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("owner replacement")
    ));
    assert_eq!(fetch_executor.pending_fetches.len(), 1);
    assert_eq!(fetch_services.fetch_tasks.len(), 1);
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
        .expect("inspect external lifecycle ownership after rejecting replacement");
    assert!(
        external.iter().all(|owner| owner != incumbent.owner()),
        "passive network Fetch ownership is deliberately not runnable clock work"
    );
    assert!(external.iter().all(|owner| owner != &foreign_owner));

    let mut executor = fixture.executor(EffectQueueConfig::default());
    let mut services = fixture.services();
    let four_candidates = (0..4)
        .map(|signer| timeout_sign(&fixture, signer))
        .collect::<Vec<_>>();
    let unbound_ownership = (0..4)
        .map(|ordinal| RuntimeEffectOwnership::fresh_for_test(tag(0), 2_000 + ordinal))
        .collect::<Vec<_>>();
    assert!(matches!(
        executor.retain_effect_batch(four_candidates.clone(), unbound_ownership),
        Err(EffectExecutorError::Contract(reason))
            if reason.contains("causal candidates above the abstract bound")
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
        assert!(executor.pending_validations.is_empty());
        assert!(services.store_tasks.is_empty());
        assert!(services.validation_tasks.is_empty());
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
        assert!(matches!(
            executor.retain_effect_batch(vec![effect], vec![foreign]),
            Err(EffectExecutorError::Runtime(reason))
                if reason.contains("owner replacement")
        ));
        assert!(executor.retained_effect_batch.is_none());
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
fn authority_transition_reuses_one_physical_store_and_validation_task() {
    let fixture = Fixture::new();
    let stage_tag = EventTag::new(fixture.context.height, 8, Generation::new(8));
    let store = AdapterEffect::StoreBody {
        tag: stage_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let validate = AdapterEffect::ValidateBody {
        tag: stage_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
    };
    let ordinary_fetch = AdapterEffect::FetchBody {
        tag: stage_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let decision = fixture.qc(wire::GlobalPhase::Commit);
    let decision_fetch = AdapterEffect::FetchBody {
        tag: stage_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(decision.clone()),
    };
    let stage_owners = |fetch: &AdapterEffect, ordinal| {
        let fetch = bind_adapter_effect_batch_ownership(
            std::slice::from_ref(fetch),
            vec![RuntimeEffectOwnership::fresh_for_test(stage_tag, ordinal)],
        )
        .expect("bind production-shaped Fetch ownership")
        .pop()
        .expect("one Fetch owner");
        let store = fetch
            .rebind_as_inherited_adapter_effect(&store)
            .expect("Fetch authorizes the exact Store stage");
        let validate = store
            .rebind_as_inherited_adapter_effect(&validate)
            .expect("Store authorizes the exact Validate stage");
        (store, validate)
    };
    let (ordinary_store, ordinary_validate) = stage_owners(&ordinary_fetch, 7_001);
    let (decision_store, decision_validate) = stage_owners(&decision_fetch, 7_002);
    assert_ne!(ordinary_store.owner(), decision_store.owner());
    assert_ne!(
        ordinary_store.candidate_semantic_identity(),
        decision_store.candidate_semantic_identity(),
        "Decision authority is deliberately distinct from the proposal lineage"
    );

    let mut store_executor = fixture.executor(EffectQueueConfig::default());
    store_executor.runtime.round_tag = Some(stage_tag);
    store_executor.reconciled_tag = Some(stage_tag);
    let mut store_services = fixture.services();
    store_executor
        .bind_body_pipeline_owner(stage_tag, &fixture.manifest)
        .expect("bind one exact body pipeline");
    store_executor
        .begin_store(
            stage_tag,
            fixture.manifest.clone(),
            Arc::<[u8]>::from(fixture.body.clone()),
            StorePurpose::Reducer,
            ordinary_store.clone(),
            &mut store_services,
        )
        .expect("start ordinary physical Store task");
    let store_id = store_services.store_tasks[0].id();
    let next_work_id = store_executor.next_work_id;
    store_executor
        .retain_effect_batch(vec![store.clone()], vec![decision_store])
        .expect("Decision Store adopts the ordinary task owner");
    assert_eq!(
        store_executor
            .drain_retained_effect_batch(&mut store_services, false)
            .expect("redispatch the one incumbent Store task"),
        1
    );
    assert_eq!(store_executor.next_work_id, next_work_id);
    assert_eq!(store_executor.pending_stores.len(), 1);
    assert!(
        store_services
            .store_tasks
            .iter()
            .all(|task| { task.id() == store_id && task.ownership() == &ordinary_store }),
        "authority upgrade must reuse the exact Store work ID and owner"
    );

    let durable = DurableBodyReceipt::for_test(
        fixture.context.id(),
        fixture.manifest.round,
        fixture.manifest.subject,
        HashOf::new(&fixture.manifest),
    );
    let body_key = (fixture.manifest.round, fixture.manifest.subject);
    let mut validation_executor = fixture.executor(EffectQueueConfig::default());
    validation_executor.runtime.round_tag = Some(stage_tag);
    validation_executor.reconciled_tag = Some(stage_tag);
    let mut validation_services = fixture.services();
    validation_executor
        .recovered_bodies
        .insert(body_key, (fixture.manifest.clone(), durable.clone()));
    validation_executor
        .durable_bodies
        .insert(body_key, durable.clone());
    validation_executor
        .bind_body_pipeline_owner(stage_tag, &fixture.manifest)
        .expect("bind validation to the exact durable body pipeline");
    validation_executor
        .begin_validation(
            fixture.manifest.round,
            fixture.manifest.subject,
            durable,
            ValidationConsumer::Reducer {
                tag: stage_tag,
                ownership: ordinary_validate.clone(),
            },
            &mut validation_services,
        )
        .expect("start ordinary physical validation task");
    let validation_id = validation_services.validation_tasks[0].id();
    let incumbent_validation = validation_executor.pending_validations[&validation_id]
        .task
        .ownership()
        .clone();
    let next_work_id = validation_executor.next_work_id;
    validation_executor
        .retain_effect_batch(vec![validate.clone()], vec![decision_validate.clone()])
        .expect("Decision Validate adopts the in-flight ordinary task owner");
    assert_eq!(
        validation_executor
            .drain_retained_effect_batch(&mut validation_services, false)
            .expect("redispatch the one incumbent validation task"),
        1
    );
    assert_eq!(validation_executor.next_work_id, next_work_id);
    assert_eq!(validation_executor.pending_validations.len(), 1);
    assert!(
        validation_services.validation_tasks.iter().all(|task| {
            task.id() == validation_id && task.ownership() == &incumbent_validation
        }),
        "authority upgrade must reuse the exact validation work ID and owner"
    );
    assert!(!validation_executor.status().fail_closed);

    let mut parked_executor = fixture.executor(EffectQueueConfig::default());
    parked_executor.runtime.round_tag = Some(stage_tag);
    parked_executor.reconciled_tag = Some(stage_tag);
    parked_executor.parked_effect_batch = Some(RetainedEffectBatch {
        effects: VecDeque::from([OwnedAdapterEffect {
            effect: validate.clone(),
            ownership: ordinary_validate.clone(),
        }]),
        oldest_at: Instant::now(),
    });
    parked_executor
        .retain_effect_batch(vec![validate.clone()], vec![decision_validate.clone()])
        .expect("Decision Validate adopts the parked ordinary stage owner");
    assert!(parked_executor.parked_effect_batch.is_none());
    let retained = parked_executor
        .retained_effect_batch
        .as_ref()
        .expect("one adopted Validate remains dispatchable");
    assert_eq!(retained.effects.len(), 1);
    assert_eq!(retained.effects[0].ownership, ordinary_validate);
    assert!(
        retained.effects[0]
            .ownership
            .binds_durable_decision_authority(
                decision.round,
                decision.proposal_round,
                decision.subject,
                decision.execution_commitment,
            )
    );

    let mut conflicting_decision = decision;
    conflicting_decision.execution_commitment =
        wire::ExecutionCommitment::without_topups_or_merge_carrier(
            Hash::new(b"conflicting stage parent state"),
            Hash::new(b"conflicting stage post state"),
            Hash::new(b"conflicting stage writes"),
            1,
            Hash::new(b"conflicting stage block"),
        );
    let conflicting_fetch = AdapterEffect::FetchBody {
        tag: stage_tag,
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: Some(conflicting_decision),
    };
    let (_, conflicting_validate) = stage_owners(&conflicting_fetch, 7_003);
    assert!(
        decision_validate
            .adopt_incumbent_body_stage_for_retry_or_authority(&conflicting_validate, &validate,)
            .is_err(),
        "one physical validation cannot coalesce conflicting execution commitments"
    );
    assert!(
        ordinary_store
            .adopt_incumbent_body_stage_for_retry_or_authority(&decision_validate, &validate,)
            .is_err(),
        "Store and Validate remain separate physical stages"
    );
}
