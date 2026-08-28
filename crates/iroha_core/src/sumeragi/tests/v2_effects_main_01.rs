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
    responder_index: wire::ValidatorIndex,
) -> wire::CertifiedBodyResponse {
    let request = task
        .certified_request()
        .expect("certified fetch task owns its request");
    let responder_index = usize::try_from(responder_index).expect("responder index");
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(request),
        manifest,
        body,
        responder: fixture.context.roster[responder_index].validator.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.validator_keys[responder_index].private_key(),
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
fn bound_leader_wire_ingress_ownership(
    ingress: &crate::sumeragi::FairV2Ingress,
    message: wire::ConsensusMessageV2,
    sender: PeerId,
) -> FairV2IngressOwnershipEvidence {
    let expected = BlockMessage::V2(message.clone());
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            expected.clone(),
            sender,
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut delivered = ingress
        .try_recv()
        .expect("bound leader-wire ingress returns its admitted owner");
    assert_eq!(delivered.message().encode(), expected.encode());
    let ownership = delivered
        .take_ingress_ownership()
        .expect("bound leader-wire ingress attaches exact ownership");
    assert!(
        ownership.leader_wire_token().is_some(),
        "productive wire must carry its full-roster lifecycle token"
    );
    assert!(
        ownership.leader_wire_runtime_receipt().is_some(),
        "checked dequeue must durably transfer the leader-wire token to runtime"
    );
    ownership
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
    let proposal_a = AdapterEffect::FetchBody {
        tag: tag(0),
        round: fixture.manifest.round,
        subject: fixture.manifest.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: Vec::new(),
        certificate: None,
    };
    let proposal_a_ownership = authenticated_proposal_fetch_ownership(&fixture, &proposal_a, 9_017);
    executor.runtime.exact_effect_ownership = Some((proposal_a.clone(), proposal_a_ownership));
    executor
        .consume_effects(vec![proposal_a], &mut services)
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
#[allow(clippy::too_many_lines)]
fn production_capacity_saturation_admits_response_and_reconstructible_fetch() {
    let mut fixture = ProductionTransportFixture::new_with_runtime_queue_config(
        RuntimeQueueConfig::new(12, 4, 4),
    );
    fixture.executor.config = EffectQueueConfig::new(2, 4, 1 << 20, 1);
    fixture.executor.outstanding_requests =
        OutstandingCertifiedBodyRequests::new(1).expect("one certified-request slot");
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    let prepare_a =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let effects = vec![AdapterEffect::FetchBody {
        tag: fixture.executor.current_tag(),
        round: fixture.round,
        subject: fixture.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: fixture.certified_sources(&prepare_a),
        certificate: Some(prepare_a),
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
    let task_a = services.fetch_tasks[0].clone();
    let request_hash_a = HashOf::new(
        task_a
            .certified_request()
            .expect("A owns its exact signed request"),
    );

    let round_b = round(&fixture.context, 1);
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
    let prepare_b = fixture.quorum_certificate_for(
        round_b,
        subject_b,
        wire::GlobalPhase::Prepare,
        commitment_b,
    );
    let fetch_b_effects = vec![AdapterEffect::FetchBody {
        tag: EventTag::new(
            fixture.context.height,
            round_b.view,
            fixture.executor.current_tag().generation(),
        ),
        round: round_b,
        subject: subject_b,
        manifest: Some(manifest_b),
        certified_sources: fixture.certified_sources(&prepare_b),
        certificate: Some(prepare_b),
    }];
    let initial_queues = fixture.executor.status().runtime_queues;
    let (_saturation_ingress_directory, saturation_ingress, _saturation_ingress_gate) =
        fixture.bound_certified_response_ingress();
    for ordinal in 0..initial_queues.normal.capacity {
        let message = fixture
            .signed_normal_proposal(u64::try_from(ordinal).expect("normal saturation ordinal"));
        let sender = fixture.context.roster[ordinal].validator.clone();
        let ingress_ownership =
            bound_leader_wire_ingress_ownership(&saturation_ingress, message.clone(), sender);
        assert!(
            fixture
                .executor
                .can_admit_network_message_with_ingress_ownership(&message, &ingress_ownership)
        );
        fixture
            .executor
            .enqueue_network_with_ingress_ownership(message, ingress_ownership)
            .expect("admit production Normal ingress");
    }
    let progress_reserve = initial_queues
        .progress
        .capacity
        .checked_sub(initial_queues.normal.capacity)
        .expect("production Progress reserve");
    for offset in 0..progress_reserve {
        let view = 10_000_u64
            .checked_add(u64::try_from(offset).expect("progress saturation offset"))
            .expect("progress saturation view");
        let signer = wire::ValidatorIndex::try_from(offset)
            .expect("four-validator Progress saturation signer");
        let message = fixture.signed_timeout_vote_from(view, signer);
        let ingress_ownership = bound_leader_wire_ingress_ownership(
            &saturation_ingress,
            message.clone(),
            fixture.context.roster[offset].validator.clone(),
        );
        assert!(
            fixture
                .executor
                .can_admit_network_message_with_ingress_ownership(&message, &ingress_ownership)
        );
        fixture
            .executor
            .enqueue_network_with_ingress_ownership(message, ingress_ownership)
            .expect("admit production Progress ingress");
    }
    let next_work_id_before_b = fixture.executor.next_work_id;
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
        0,
    );
    assert_eq!(fixture.executor.pending_fetches.len(), 1);
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);
    assert!(
        fixture
            .executor
            .certified_work
            .contains_key(&request_hash_a)
    );
    assert!(fixture.executor.retained_effect_batch.is_some());
    assert!(
        !fixture
            .executor
            .body_pipeline_owners
            .contains_key(&(round_b, subject_b)),
        "B cannot gain partial pipeline ownership while its request is parked",
    );
    let saturated_status = fixture.executor.status();
    assert_eq!(
        saturated_status.runtime_queues.normal.depth,
        initial_queues.normal.capacity,
    );
    assert_eq!(
        saturated_status.runtime_queues.progress.depth,
        progress_reserve,
    );
    assert_eq!(saturated_status.runtime_queues.completion.depth, 0);
    let saturated_runtime_queues = saturated_status.runtime_queues;
    let saturated_queued_commands = saturated_status.queued_runtime_completions;
    let saturated_completion_capacity = fixture.executor.remaining_completion_capacity();

    let mut response_a = wire::CertifiedBodyResponse {
        request_hash: request_hash_a,
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: fixture.context.roster[0].validator.clone(),
        signature: Vec::new(),
    };
    response_a.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response_a.signature_preimage(),
    )
    .payload()
    .to_vec();
    let (_ingress_directory, ingress, ingress_gate) = fixture.bound_certified_response_ingress();
    let response_message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response_a),
    ));
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            response_message,
            fixture.context.roster[0].validator.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let response_ordinal = ingress.state.lock().last_admission_ordinal;
    let response_leader_wire_token =
        queued_leader_wire_ingress_token(&ingress, &ingress_gate, response_ordinal);
    let prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, response_ordinal)
        .expect("the exact A response crosses authenticated selector capture");
    let effect_a = task_a.adapter_effect();
    let pending_a = task_a
        .ownership()
        .exact_pending_adapter_effect_binding(&effect_a)
        .expect("A retains its exact ordinal-free Fetch binding");
    let proofs = fixture
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified = VerifiedHeightContext::genesis(fixture.context.clone(), proofs)
        .expect("verified lifecycle owner context");
    let owner_directory = TempDir::new().expect("temporary lifecycle owner storage");
    let (mut owner, lifecycle_ordinal, lifecycle_source) =
        crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1::waiting_fetch_for_ingress_test(
            verified,
            &prepared,
            effect_a,
            pending_a,
            &fixture.validator_keys[0],
            owner_directory.path(),
        );
    assert!(response_leader_wire_token.scheduler_ordinal() > lifecycle_ordinal);
    let (mut production_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    production_services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    let mut planner_io = owner.bind_body_store_to_planner_io_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
        1,
    );
    planner_io.install_output_guard_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
    );
    let locked_candidate_tag = task_a.tag;
    let locked_candidate_round = task_a.round;
    let locked_candidate_subject = task_a.subject;
    production_services
        .request_locked_candidate(
            locked_candidate_tag,
            locked_candidate_round,
            locked_candidate_subject,
        )
        .expect("queue the exact locked-body lookup before certified recovery");
    assert_eq!(
        planner_io.execute_one_locked_candidate_load(&mut production_services),
        None,
        "the absent locked body must wait for certified persistence"
    );
    V2EffectServices::enqueue_body_fetch(&mut production_services, task_a.clone())
        .expect("install A's exact certified-Fetch service owner");
    let planned = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    let queued = match planned {
        Ok(ProductionIngressTurnPreparation::Queued(queued)) => queued,
        Ok(ProductionIngressTurnPreparation::CapacityWait(_)) => {
            panic!("completion capacity must not depend on saturated Normal/Progress lanes")
        }
        Err(error) => panic!(
            "the reservation-bound exact A response must publish one Fetch persistence command, not fail carrier attestation: {}",
            error.reason(),
        ),
    };
    assert_eq!(queued.ordinal(), lifecycle_ordinal);
    planner_io.execute_one_certified_fetch(Arc::clone(&fixture.executor.output_guard));
    let completion = match production_services
        .take_next_lifecycle_completion()
        .expect("the persisted A response retains its physical completion owner")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::CertifiedFetch(completion) => {
            completion
        }
        _ => panic!("the persisted A response must classify as CertifiedFetch"),
    };
    owner
        .complete_certified_fetch_for_test(
            &mut fixture.executor,
            &mut production_services,
            &ingress,
            completion,
        )
        .unwrap_or_else(|error| match error {
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::Retry(
                error,
            ) => panic!(
                "A must publish Ready and retire its physical response: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(
                error,
            ) => panic!(
                "A lost its productive ingress before persistence: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
                error,
            ) => panic!(
                "A reached a restart-only persistence failure: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(
                error,
            ) => panic!("A lost its exact Runtime handoff after dequeue: {error}"),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
                error,
            ) => panic!("A failed after the persistence commit: {error}"),
        });
    assert_eq!(
        planner_io.execute_one_locked_candidate_load(&mut production_services),
        Some(locked_candidate_tag),
        "certified persistence must wake the matching waiting acquisition"
    );
    let recovered_locked_candidate = production_services
        .take_loaded_candidate()
        .expect("deliver the certified body to the waiting locked proposal");
    assert_eq!(recovered_locked_candidate.tag(), locked_candidate_tag);
    assert_eq!(recovered_locked_candidate.round(), locked_candidate_round);
    assert_eq!(
        recovered_locked_candidate.subject(),
        locked_candidate_subject
    );
    assert_eq!(
        recovered_locked_candidate.into_canonical_wire(),
        fixture.body
    );
    assert!(matches!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        (Some(LifecycleState::Ready), Some(2), None, false)
    ));
    assert!(fixture.executor.pending_fetches.is_empty());
    assert!(fixture.executor.certified_work.is_empty());
    assert!(fixture.executor.outstanding_requests.is_empty());
    assert_eq!(ingress.len(), 0);
    assert_leader_wire_body_terminal(&ingress_gate, &response_leader_wire_token);
    let post_completion_status = fixture.executor.status();
    for (before, after) in [
        (
            saturated_runtime_queues.normal,
            post_completion_status.runtime_queues.normal,
        ),
        (
            saturated_runtime_queues.progress,
            post_completion_status.runtime_queues.progress,
        ),
        (
            saturated_runtime_queues.completion,
            post_completion_status.runtime_queues.completion,
        ),
    ] {
        assert_eq!(
            (after.depth, after.capacity, after.max_service_debt),
            (before.depth, before.capacity, before.max_service_debt),
            "lifecycle persistence cannot change runtime queue ownership or service debt",
        );
        assert_eq!(after.oldest_age.is_some(), before.oldest_age.is_some());
        assert!(
            after.oldest_age >= before.oldest_age,
            "the same queued owners can only age while Phase B runs",
        );
    }
    assert_eq!(
        post_completion_status.queued_runtime_completions,
        saturated_queued_commands,
    );
    assert_eq!(
        fixture.executor.remaining_completion_capacity(),
        saturated_completion_capacity,
        "lifecycle persistence must not mint a runtime BodyAvailable reservation",
    );

    assert_eq!(
        fixture
            .executor
            .step(Instant::now(), &mut services)
            .expect("released request capacity drains the retained Fetch B"),
        EffectExecutorStep::Advanced { effects: 1 }
    );
    let task_b = services
        .fetch_tasks
        .last()
        .expect("B acquires a Fetch owner");
    assert_eq!(task_b.round, round_b);
    assert_eq!(task_b.subject, subject_b);
    assert_eq!(
        fixture.executor.next_work_id,
        next_work_id_before_b
            .checked_add(1)
            .expect("B advances the work ID exactly once"),
    );
    assert_eq!(fixture.executor.pending_fetches.len(), 1);
    assert_eq!(fixture.executor.outstanding_requests.len(), 1);
    assert_eq!(fixture.executor.certified_work.len(), 1);
    assert!(fixture.executor.retained_effect_batch.is_none());
    assert!(!fixture.executor.output_guard.restart_required());
    assert!(!fixture.executor.status().fail_closed);
    planner_io.detach(&mut production_services);
}
#[test]
#[allow(clippy::too_many_lines)]
fn ungated_certified_fetch_phase_b_restarts_before_ledger_without_mutation() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    let prepare =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let effects = vec![AdapterEffect::FetchBody {
        tag: fixture.executor.current_tag(),
        round: fixture.round,
        subject: fixture.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: fixture.certified_sources(&prepare),
        certificate: Some(prepare),
    }];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&effects)
        .expect("bind one production certified-Fetch owner");
    assert_eq!(
        fixture
            .executor
            .consume_effects(effects, &mut services)
            .expect("admit one production certified-Fetch owner"),
        1,
    );
    let task = services.fetch_tasks[0].clone();
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(
            task.certified_request()
                .expect("the Fetch owns its exact signed request"),
        ),
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: fixture.context.roster[0].validator.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.validator_keys[0].private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let ingress =
        crate::sumeragi::FairV2Ingress::new(32, 5 * 512 * 1024, 512 * 1024, 0, 512 * 1024);
    ingress
        .configure_roster(
            fixture
                .context
                .roster
                .iter()
                .map(|power| power.validator.clone()),
        )
        .expect("fixture roster fits the deliberately ungated ingress");
    ingress.state.lock().leader_wire_context = Some((fixture.context.id(), fixture.context.height));
    ingress.open().expect("open deliberately ungated ingress");
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
            )),
            fixture.context.roster[0].validator.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let response_ordinal = ingress.state.lock().last_admission_ordinal;
    let prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, response_ordinal)
        .expect("authenticate the exact ungated response family");
    assert_eq!(
        prepared.certified_fetch_preledger_productive_ingress_token_for_test(),
        Err(CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken),
        "Phase A may observe a test-only ungated owner but Phase B must reject it",
    );
    assert!(prepared.selected_certified_fetch_is_ungated_for_test());
    assert!(ingress.exact_queued_ungated_occurrence_for_test(response_ordinal));
    let effect = task.adapter_effect();
    let pending = task
        .ownership()
        .exact_pending_adapter_effect_binding(&effect)
        .expect("retain the exact Fetch registry carrier");
    let proofs = fixture
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified = VerifiedHeightContext::genesis(fixture.context.clone(), proofs)
        .expect("verified lifecycle owner context");
    let owner_directory = TempDir::new().expect("temporary ungated lifecycle owner storage");
    let (mut owner, lifecycle_ordinal, lifecycle_source) =
        crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1::waiting_fetch_for_ingress_test(
            verified,
            &prepared,
            effect,
            pending,
            &fixture.validator_keys[0],
            owner_directory.path(),
        );
    let (mut production_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    production_services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    let mut planner_io = owner.bind_body_store_to_planner_io_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
        1,
    );
    planner_io.install_output_guard_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
    );
    V2EffectServices::enqueue_body_fetch(&mut production_services, task.clone())
        .expect("install the exact certified-Fetch service owner");
    let planned = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    let queued = match planned {
        Ok(ProductionIngressTurnPreparation::Queued(queued)) => queued,
        Ok(ProductionIngressTurnPreparation::CapacityWait(_)) => {
            panic!("available exact capacity cannot delay the ungated Phase-A fixture")
        }
        Err(error) => panic!(
            "real Phase A must persist the reservation-bound selected ungated response: {}",
            error.reason(),
        ),
    };
    assert_eq!(queued.ordinal(), lifecycle_ordinal);
    planner_io.execute_one_certified_fetch(Arc::clone(&fixture.executor.output_guard));
    let completion = match production_services
        .take_next_lifecycle_completion()
        .expect("take the real persisted Fetch completion before fail-stop")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::CertifiedFetch(completion) => {
            completion
        }
        _ => panic!("the persisted response must classify as certified Fetch"),
    };
    let work_id = completion.work_id();
    assert!(planner_io.certified_fetch_completion_is_pending(work_id));
    assert!(matches!(
        production_services
            .take_next_lifecycle_completion()
            .expect("the completion FIFO remains open before the fail-stop boundary"),
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::None
    ));
    let wait_before = owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source);
    let registry_before = owner.fetch_registry_snapshot_for_test();
    let pending_before = fixture.executor.pending_fetches.clone();
    let certified_before = fixture.executor.certified_work.clone();
    let outstanding_before = fixture.executor.outstanding_requests.hashes();
    let claims_before = fixture.executor.outstanding_requests.response_claim_count();
    let next_work_id_before = fixture.executor.next_work_id;
    let ingress_depth_before = ingress.len();
    let ingress_cut_before = ingress.next_physical_admission_ordinal();
    let files_before = regular_file_bytes_below_for_test(owner_directory.path());
    drop(
        production_services
            .prepare_certified_body_fetch_owner_removal(&task)
            .expect("snapshot the exact retained service owner"),
    );
    assert!(!fixture.executor.output_guard.restart_required());
    let failure = match owner.complete_certified_fetch_for_test(
        &mut fixture.executor,
        &mut production_services,
        &ingress,
        completion,
    ) {
        Err(
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(
                error,
            ),
        ) => error,
        Ok(()) => panic!("ungated productive ingress cannot cross Phase B"),
        Err(
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::Retry(
                error,
            ),
        ) => panic!(
            "structurally invalid productive ingress cannot spin: {}: {}",
            error.reason(),
            error.detail(),
        ),
        Err(
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
                error,
            ),
        ) => panic!("ungated rejection must precede Ledger: {}", error.detail()),
        Err(
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(
                error,
            ),
        ) => panic!("ungated rejection must precede dequeue: {error}"),
        Err(
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
                error,
            ),
        ) => panic!("ungated rejection must precede the volatile commit tail: {error}"),
    };
    assert_eq!(failure.work_id(), work_id);
    assert_eq!(
        failure.failure(),
        CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken,
    );
    assert!(fixture.executor.output_guard.restart_required());
    assert_eq!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        wait_before,
    );
    assert_eq!(owner.fetch_registry_snapshot_for_test(), registry_before);
    assert_eq!(fixture.executor.pending_fetches, pending_before);
    assert_eq!(fixture.executor.certified_work, certified_before);
    assert_eq!(
        fixture.executor.outstanding_requests.hashes(),
        outstanding_before,
    );
    assert_eq!(
        fixture.executor.outstanding_requests.response_claim_count(),
        claims_before,
    );
    assert_eq!(fixture.executor.next_work_id, next_work_id_before);
    assert_eq!(ingress.len(), ingress_depth_before);
    assert_eq!(
        ingress.next_physical_admission_ordinal(),
        ingress_cut_before
    );
    assert_eq!(
        regular_file_bytes_below_for_test(owner_directory.path()),
        files_before,
        "pre-ledger fail-stop cannot publish lifecycle or body-store bytes",
    );
    assert!(
        ingress.exact_queued_ungated_occurrence_for_test(response_ordinal),
        "the exact response remains queue-owned without a fabricated Runtime receipt",
    );
    drop(
        production_services
            .prepare_certified_body_fetch_owner_removal(&task)
            .expect("pre-ledger fail-stop retains the exact service owner"),
    );
    assert!(planner_io.certified_fetch_completion_is_pending(work_id));
    assert!(
        !production_services.has_reparked_certified_fetch_completion_for_test(),
        "the move-only restart error, not a second FIFO, retains the completion",
    );
    drop(failure);
    planner_io.detach(&mut production_services);
}
#[test]
#[allow(clippy::too_many_lines)]
fn request_bound_rotated_archive_completes_fetch_without_leader_wire_slot() {
    let mut fixture = ProductionTransportFixture::new();
    fixture.executor.recovered_bodies.clear();
    let mut services = FakeServices {
        requester_key: Some(fixture.requester_key.clone()),
        ..FakeServices::default()
    };
    let prepare =
        fixture.quorum_certificate(wire::GlobalPhase::Prepare, fixture.canonical_commitment);
    let effects = vec![AdapterEffect::FetchBody {
        tag: fixture.executor.current_tag(),
        round: fixture.round,
        subject: fixture.subject,
        manifest: Some(fixture.manifest.clone()),
        certified_sources: fixture.certified_sources(&prepare),
        certificate: Some(prepare),
    }];
    fixture
        .executor
        .runtime
        .retain_retransmit_effect_ownership_for_test(&effects)
        .expect("bind one production certified-Fetch owner");
    assert_eq!(
        fixture
            .executor
            .consume_effects(effects, &mut services)
            .expect("admit one production certified-Fetch owner"),
        1,
    );
    let task = services.fetch_tasks[0].clone();
    let rotated_responder = PeerId::new(fixture.responder_key.public_key().clone());
    assert!(
        fixture
            .context
            .roster
            .iter()
            .all(|power| power.validator != rotated_responder),
        "the current archive responder must be outside the frozen roster",
    );
    let mut response = wire::CertifiedBodyResponse {
        request_hash: HashOf::new(
            task.certified_request()
                .expect("the Fetch owns its exact signed request"),
        ),
        manifest: fixture.manifest.clone(),
        body: fixture.body.clone(),
        responder: rotated_responder.clone(),
        signature: Vec::new(),
    };
    response.signature = Signature::new(
        fixture.responder_key.private_key(),
        &response.signature_preimage(),
    )
    .payload()
    .to_vec();
    let (_ingress_directory, ingress, _ingress_gate) = fixture.bound_certified_response_ingress();
    {
        let state = ingress.state.lock();
        assert!(state.requires_leader_wire_lifecycle_gate);
        assert!(state.leader_wire_lifecycle_gate.is_some());
        assert!(!state.roster.contains(&rotated_responder));
    }
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response),
            )),
            rotated_responder.clone(),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let response_ordinal = ingress.state.lock().last_admission_ordinal;
    assert!(ingress.exact_queued_ungated_occurrence_for_test(response_ordinal));
    {
        let state = ingress.state.lock();
        let response_entry = state
            .lanes
            .values()
            .flat_map(|lane| lane.entries.iter())
            .find(|entry| entry.admission_ordinal == response_ordinal)
            .expect("the rotated response remains physically queued");
        let ownership = response_entry
            .inbound
            .ingress_ownership()
            .expect("the rotated response retains exact ingress ownership");
        assert!(ownership.request_bound_non_roster_completion());
        assert!(state.leader_wire_lifecycles.is_empty());
    }
    let outsider_chunk = wire::PayloadChunk {
        manifest_hash: HashOf::new(&fixture.manifest),
        index: 0,
        bytes: vec![0xA5],
        sender: 0,
        signature: vec![0x5A],
    };
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::from_authenticated_peer(
            BlockMessage::V2(wire::ConsensusMessageV2::new(
                wire::ConsensusMessageV2Payload::PayloadChunk(outsider_chunk),
            )),
            rotated_responder,
        )),
        Err(crate::sumeragi::FairV2IngressPushError::Rejected(_))
    ));
    let prepared = fixture
        .executor
        .prepare_lifecycle_ingress_selector(&ingress, response_ordinal)
        .expect("authenticate the exact request-bound archive response family");
    assert!(prepared.selected_certified_fetch_is_request_bound_archive_for_test());
    assert_eq!(
        prepared.certified_fetch_preledger_productive_ingress_token_for_test(),
        Err(CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken),
        "the archive response must not consume a roster-sized generic lifecycle slot",
    );
    let effect = task.adapter_effect();
    let pending = task
        .ownership()
        .exact_pending_adapter_effect_binding(&effect)
        .expect("retain the exact Fetch registry carrier");
    let proofs = fixture
        .validator_keys
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("validator proof of possession")
        })
        .collect::<Vec<_>>();
    let verified = VerifiedHeightContext::genesis(fixture.context.clone(), proofs)
        .expect("verified lifecycle owner context");
    let owner_directory = TempDir::new().expect("temporary archive lifecycle owner storage");
    let (mut owner, lifecycle_ordinal, lifecycle_source) =
        crate::sumeragi::v2_lifecycle_coordinator::ProductionLifecycleOwnerV1::waiting_fetch_for_ingress_test(
            verified,
            &prepared,
            effect,
            pending,
            &fixture.validator_keys[0],
            owner_directory.path(),
        );
    let (mut production_services, _) = crate::sumeragi::v2_worker::tests::fixture();
    production_services.set_exact_output_admission_hook(|_post, _ticket| Ok(()));
    let mut planner_io = owner.bind_body_store_to_planner_io_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
        1,
    );
    planner_io.install_output_guard_for_test(
        &mut production_services,
        Arc::clone(&fixture.executor.output_guard),
    );
    V2EffectServices::enqueue_body_fetch(&mut production_services, task.clone())
        .expect("install the exact certified-Fetch service owner");
    let planned = owner.plan_ingress_turn_for_test(
        &production_services,
        &fixture.executor,
        fixture.executor.lifecycle_mode_rank_snapshot(),
        prepared,
        crate::sumeragi::v2_runner::lifecycle_ingress_rank_snapshot_for_test(&fixture.context),
    );
    let queued = match planned {
        Ok(ProductionIngressTurnPreparation::Queued(queued)) => queued,
        Ok(ProductionIngressTurnPreparation::CapacityWait(_)) => {
            panic!("available exact capacity cannot delay the archive response")
        }
        Err(_) => panic!("real Phase A must persist the request-bound archive response"),
    };
    assert_eq!(queued.ordinal(), lifecycle_ordinal);
    planner_io.execute_one_certified_fetch(Arc::clone(&fixture.executor.output_guard));
    let completion = match production_services
        .take_next_lifecycle_completion()
        .expect("take the persisted archive Fetch completion")
    {
        crate::sumeragi::v2_worker::LifecycleCompletionTakeV1::CertifiedFetch(completion) => {
            completion
        }
        _ => panic!("the persisted archive response must classify as certified Fetch"),
    };
    owner
        .complete_certified_fetch_for_test(
            &mut fixture.executor,
            &mut production_services,
            &ingress,
            completion,
        )
        .unwrap_or_else(|error| match error {
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::Retry(
                error,
            ) => panic!(
                "archive completion must publish Ready and retire its response: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(
                error,
            ) => panic!(
                "archive completion lost its sealed exact-request ingress mode: {}: {}",
                error.reason(),
                error.detail(),
            ),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequired(
                error,
            ) => panic!("archive completion failed after Ledger: {}", error.detail()),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(
                error,
            ) => panic!("archive completion lost its post-dequeue ownership: {error}"),
            crate::sumeragi::v2_lifecycle_coordinator::CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(
                error,
            ) => panic!("archive completion failed after its commit tail: {error}"),
        });
    assert!(matches!(
        owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source),
        (Some(LifecycleState::Ready), Some(2), None, false)
    ));
    assert!(fixture.executor.pending_fetches.is_empty());
    assert!(fixture.executor.certified_work.is_empty());
    assert!(fixture.executor.outstanding_requests.is_empty());
    assert_eq!(
        fixture.executor.outstanding_requests.response_claim_count(),
        0
    );
    assert_eq!(ingress.len(), 0);
    assert!(ingress.state.lock().leader_wire_lifecycles.is_empty());
    assert!(!fixture.executor.output_guard.restart_required());
    planner_io.detach(&mut production_services);
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
