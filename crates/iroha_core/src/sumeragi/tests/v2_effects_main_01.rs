fn distinct_body(fixture: &Fixture) -> (wire::BlockSubject, Vec<u8>) {
    let header = BlockHeader::new(
        NonZeroU64::new(1).expect("height"),
        None,
        None,
        None,
        2_000,
        0,
    );
    let signature =
        SignatureOf::try_from_hash(fixture.validator_keys[0].private_key(), header.hash())
            .expect("distinct block signature");
    let block = SignedBlock::presigned(BlockSignature::new(0, signature), header, Vec::new());
    let body = block.encode_wire().expect("distinct canonical body");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&body),
    };
    (subject, body)
}
fn timeout_at_view(fixture: &Fixture, view: u64) -> wire::TimeoutCertificate {
    wire::TimeoutCertificate {
        round: round(&fixture.context, view),
        groups: vec![wire::TimeoutVoteGroup {
            highest_prepare_qc: None,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![1],
        }],
    }
}
fn timeout_sign(fixture: &Fixture, view: u64) -> AdapterEffect {
    AdapterEffect::Sign {
        tag: tag(view),
        request: SignRequest::TimeoutVote(wire::TimeoutVote {
            round: round(&fixture.context, view),
            highest_prepare_qc: None,
            signer: 0,
            signature: Vec::new(),
        }),
    }
}
fn prepare_qc_for_subject(
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> wire::QuorumCertificate {
    wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject,
        execution_commitment: fixture_execution_commitment(),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    }
}
fn certified_sources(fixture: &Fixture, _certificate: &wire::QuorumCertificate) -> Vec<PeerId> {
    fixture
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect()
}
fn signed_payload_chunk(fixture: &Fixture) -> wire::PayloadChunk {
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
    chunk
}
fn signed_certified_response(
    fixture: &Fixture,
    task: &BodyFetchTask,
    manifest: wire::PayloadManifest,
    body: Vec<u8>,
    responder: wire::ValidatorIndex,
) -> wire::CertifiedBodyResponse {
    let request = task
        .certified_request()
        .expect("certified fetch task owns its request");
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(request),
        manifest,
        body,
        responder,
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.validator_keys[usize::try_from(responder).expect("responder index")].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    response
}
fn certified_response_ingress_ownership(
    response: &wire::CertifiedBodyResponse,
    responder: PeerId,
) -> FairV2IngressOwnershipEvidence {
    fair_transport_ingress_ownership(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::CertifiedBodyResponse(
            response.clone(),
        )),
        responder,
    )
}
fn payload_chunk_ingress_ownership(
    chunk: &wire::PayloadChunk,
    sender: PeerId,
) -> FairV2IngressOwnershipEvidence {
    fair_transport_ingress_ownership(
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())),
        sender,
    )
}
fn fair_transport_ingress_ownership(
    message: wire::ConsensusMessageV2,
    sender: PeerId,
) -> FairV2IngressOwnershipEvidence {
    let mut inbound = crate::sumeragi::fair_v2_ingress_admit_for_test(
        InboundBlockMessage::from_authenticated_peer(BlockMessage::V2(message), sender),
    );
    inbound
        .take_ingress_ownership()
        .expect("real fair ingress attaches certified-response ownership")
}
fn manifest_for_payload(fixture: &Fixture, label: &'static [u8]) -> wire::PayloadManifest {
    let body = label.to_vec();
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(label)),
        payload_hash: Hash::new(&body),
    };
    canonical_payload_manifest(&fixture.context, fixture.manifest.round, subject, &body)
}
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
    install_fsynced_validation_fixture(
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
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            started,
        )
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
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            started,
        )
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
