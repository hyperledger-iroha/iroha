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
fn certified_response_runtime_ingress_ownership(
    fixture: &Fixture,
    response: &wire::CertifiedBodyResponse,
    responder: PeerId,
) -> (
    TempDir,
    crate::sumeragi::FairV2Ingress,
    Arc<super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate>,
    FairV2IngressOwnershipEvidence,
) {
    let roster = fixture
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let directory = TempDir::new().expect("temporary response leader-wire directory");
    let ingress = crate::sumeragi::FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
        64,
        128 * 1024 * 1024,
        16 * 1024 * 1024,
        super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
        4 * 1024 * 1024,
        4 * 1024 * 1024,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        usize::MAX,
        None,
    );
    ingress
        .configure_roster_for_context(
            roster.clone(),
            &fixture.context.network_id,
            fixture.context.da_layout,
        )
        .expect("configure response leader-wire ingress");
    ingress.require_leader_wire_lifecycle_gate();
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            fixture.context.da_layout.max_chunk_count,
        )
        .expect("derive response lifecycle capacity");
    let owner = [0xE5; 32];
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            fixture.context.id(),
            fixture.context.height,
            owner,
            response.manifest.round.view,
            false,
        );
    let (gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &directory.path().join("response-leader-wire.wal"),
            fixture.context.id(),
            fixture.context.height,
            owner,
            roster.iter().cloned().collect(),
            capacity,
            fixture.context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open response leader-wire gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            Arc::clone(&gate),
            restore,
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(64),
            fixture.context.id(),
            fixture.context.height,
        )
        .expect("bind response leader-wire gate");
    ingress.open().expect("open response leader-wire ingress");
    let message = wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response.clone()),
    );
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message),
            Some(responder),
        )),
        Ok(crate::sumeragi::FairV2IngressPushDisposition::Enqueued)
    ));
    let mut inbound = ingress
        .try_recv()
        .expect("drain exact response ingress carrier");
    let mut ownership = inbound
        .take_ingress_ownership()
        .expect("response retains fair-ingress ownership");
    ingress
        .bind_leader_wire_runtime_ownership(&mut ownership)
        .expect("bind response leader-wire runtime receipt");
    (directory, ingress, gate, ownership)
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
    let mut inbound = crate::sumeragi::fair_v2_ingress_admit_for_test(InboundBlockMessage::new(
        BlockMessage::V2(message),
        Some(sender),
    ));
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
fn pending_merge_validation(
    fixture: &Fixture,
) -> (
    PendingValidation,
    CertifiedMergeLedgerReference,
    HashOf<MergeLedgerEntry>,
) {
    let parent_hash = HashOf::from_untyped_unchecked(Hash::new(b"merge carrier parent"));
    let round = round(&fixture.context, 3);
    let subject = wire::BlockSubject {
        parent_block_hash: Some(parent_hash),
        ..fixture.manifest.subject
    };
    let manifest = canonical_payload_manifest(&fixture.context, round, subject, &fixture.body);
    let durable_receipt =
        DurableBodyReceipt::for_test(fixture.context.id(), round, subject, HashOf::new(&manifest));
    let task = BodyValidationTask::for_test(77, durable_receipt);
    let ownership = task.ownership().clone();
    let entry_hash = HashOf::from_untyped_unchecked(Hash::new(b"certified merge entry"));
    let reference = CertifiedMergeLedgerReference {
        version: 1,
        entry_hash,
        encoded_len: 512,
        epoch_id: 9,
        execution_batch_hash: None,
        entrypoint_count: None,
        entrypoint_merkle_root: None,
        result_merkle_root: None,
        base_state_height: None,
        base_state_hash: None,
        merge_qc: MergeQuorumCertificate::new(
            round.view,
            9,
            round.height,
            parent_hash,
            fixture.context.network_id,
            1,
            HashOf::new(&Vec::<PeerId>::new()),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Hash::new(b"merge certificate message"),
        ),
    };
    (
        PendingValidation {
            task,
            consumer: Some(ValidationConsumer::Reducer {
                tag: tag(3),
                ownership,
            }),
        },
        reference,
        entry_hash,
    )
}
fn begin_reachable_merge_validation(
    fixture: &Fixture,
    executor: &mut V2EffectExecutor<FakeRuntime>,
    services: &mut FakeServices,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
) -> BodyValidationTask {
    let manifest = canonical_payload_manifest(&fixture.context, round, subject, &fixture.body);
    let durable =
        DurableBodyReceipt::for_test(fixture.context.id(), round, subject, HashOf::new(&manifest));
    let key = (round, subject);
    executor
        .recovered_bodies
        .insert(key, (manifest.clone(), durable.clone()));
    executor.durable_bodies.insert(key, durable.clone());
    executor
        .bind_body_pipeline_owner(tag(round.view), &manifest)
        .expect("bind the exact production validation owner");
    executor
        .begin_validation(
            round,
            subject,
            durable,
            ValidationConsumer::Reducer {
                tag: tag(round.view),
                ownership: RuntimeEffectOwnership::fresh_for_test(
                    tag(round.view),
                    u128::from(round.view),
                ),
            },
            services,
        )
        .expect("start validation through the production admission path");
    services
        .validation_tasks
        .last()
        .expect("production validation task")
        .clone()
}
fn complete_local_proposal_chain(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    services: &mut FakeServices,
) {
    let store_id = services.store_tasks.last().expect("local store task").id();
    let store_completion = services.execute_store(store_id);
    executor
        .complete_body_store(store_completion, services)
        .expect("local durable store completion");
    let validation_id = services
        .validation_tasks
        .last()
        .expect("local validation task")
        .id();
    let validation_completion = services.execute_validation(validation_id);
    executor
        .complete_body_validation(validation_completion, services)
        .expect("local validation completion");
}
fn persist_fsynced_validation_marker(
    executor: &mut V2EffectExecutor<FakeRuntime>,
    services: &mut FakeServices,
    fixture: &Fixture,
    manifest: wire::PayloadManifest,
) {
    executor
        .admit_local_proposal(
            tag(manifest.round.view),
            manifest,
            fixture.body.clone(),
            services,
        )
        .expect("admit exact body before vote signing");
    complete_local_proposal_chain(executor, services);
    // The helper's purpose is only to cross the real body/marker fsync
    // boundary. Keep each caller's assertions focused on the subsequent
    // signature operation.
    executor.runtime.completions.clear();
    services.store_tasks.clear();
    services.validation_tasks.clear();
    services.statuses.clear();
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
        .arm_live_clocks(
            ProductionLifecycleLiveClockActivationPermitV1::for_test(),
            started,
        )
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
