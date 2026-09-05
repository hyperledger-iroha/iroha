#[cfg(feature = "bls")]
fn write_and_reopen_authenticated_wal_startup(
    directory: &TempDir,
    context: &wire::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    local_validator: wire::ValidatorIndex,
    consensus_key_hash: [u8; 32],
    records: Vec<WalRecordV2>,
) -> RecoveredAdapterStartup {
    write_and_reopen_authenticated_wal_startup_at_path(
        directory.path().join("authenticated-fifo-safety.wal"),
        context,
        proofs_of_possession,
        local_validator,
        consensus_key_hash,
        records,
    )
}
#[cfg(feature = "bls")]
fn write_and_reopen_authenticated_wal_startup_at_path(
    wal_path: PathBuf,
    context: &wire::HeightContext,
    proofs_of_possession: &[Vec<u8>],
    local_validator: wire::ValidatorIndex,
    consensus_key_hash: [u8; 32],
    records: Vec<WalRecordV2>,
) -> RecoveredAdapterStartup {
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs_of_possession.to_vec())
        .expect("verify authenticated FIFO context");
    let (mut adapter, startup) = SumeragiV2Adapter::open_with_aggregator_and_publication(
        wal_path.clone(),
        verified,
        Some(local_validator),
        reducer::Generation::new(50),
        consensus_key_hash,
        fingerprints(),
        Box::new(TestAggregator),
        false,
        deferred_admission_ordinals(),
    )
    .expect("open authenticated FIFO WAL writer");
    assert!(startup.is_empty());
    for (index, record) in records.into_iter().enumerate() {
        let persistence_id = u64::try_from(index)
            .expect("small FIFO index")
            .checked_add(1)
            .expect("FIFO persistence id");
        let payload = WalEnvelopeV2 {
            protocol_version: wire::PROTOCOL_VERSION,
            persistence_id,
            record,
        }
        .encode();
        let receipt = adapter.wal.append(&payload).expect("append FIFO WAL frame");
        assert_eq!(receipt.sequence().checked_add(1), Some(persistence_id));
    }
    drop(adapter);
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs_of_possession.to_vec())
        .expect("reverify authenticated FIFO context");
    SumeragiV2Adapter::open_recovered_startup_with_aggregator(
        wal_path,
        verified,
        Some(local_validator),
        reducer::Generation::new(50),
        consensus_key_hash,
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen authenticated FIFO WAL")
}
fn take_current_sign(effects: &mut Vec<AdapterEffect>) -> AdapterEffect {
    let signs = effects
        .iter()
        .enumerate()
        .filter_map(|(index, effect)| matches!(effect, AdapterEffect::Sign { .. }).then_some(index))
        .collect::<Vec<_>>();
    let [index] = signs.as_slice() else {
        panic!("expected one current Sign beside inert completion effects: {effects:?}")
    };
    effects.remove(*index)
}
fn persist_proposal_intent_for_control_recovery(directory: &TempDir, marker: u8) {
    let (mut adapter, startup) =
        open_test_as_leader(directory).expect("open local ProposalIntent fixture");
    assert!(startup.is_empty());
    let proposal = proposal(
        &adapter.wire_context,
        adapter.wire_context.leader(0),
        subject(marker),
    );
    let wire::ConsensusMessageV2Payload::Proposal(proposal) = proposal.payload else {
        unreachable!("proposal fixture")
    };
    let (durable, validated) =
        validated_receipts_for_manifest(&adapter.wire_context, &proposal.manifest);
    let sign = adapter
        .local_proposal_ready(
            adapter.current_tag(),
            proposal.manifest,
            &durable,
            &validated,
        )
        .expect("persist exact ProposalIntent");
    assert!(matches!(
        sign.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::Proposal(_),
            ..
        }]
    ));
}
fn persist_timeout_intent_for_control_recovery(directory: &TempDir) {
    let (mut adapter, startup) = open_test(directory).expect("open TimeoutIntent fixture");
    assert!(startup.is_empty());
    let sign = adapter
        .timeout_elapsed(adapter.current_tag())
        .expect("persist exact TimeoutIntent");
    assert!(matches!(
        sign.effects(),
        [AdapterEffect::Sign {
            request: SignRequest::TimeoutVote(_),
            ..
        }]
    ));
}
fn open_control_owner_for_test(
    safety: &TempDir,
    storage: &TempDir,
    proposal: bool,
) -> ProductionLifecycleOwnerV1 {
    let startup = if proposal {
        open_recovered_leader_startup_test(safety)
    } else {
        open_recovered_startup_test(safety)
    }
    .expect("open exact recovered control startup");
    let authenticated = startup
        .authenticate_final_wal_startup_authority()
        .unwrap_or_else(|(error, _startup)| {
            panic!("authenticate exact recovered control startup: {error}")
        });
    assert!(authenticated.has_recovered_control_sign_for_test());
    assert!(authenticated.effects.is_empty());
    let local_signer = KeyPair::try_from_seed(vec![1; 32], Algorithm::BlsNormal)
        .expect("deterministic BLS control-startup signer");
    authenticated
        .open_production_lifecycle_owner_v1_from_roots_for_test(
            &lifecycle_owner_config(),
            4,
            &storage.path().join("ledger"),
            &storage.path().join("serve"),
            &storage.path().join("body"),
            super::super::v2_body_store::BlockSignaturePolicy::RotatingLeader,
            &local_signer,
        )
        .unwrap_or_else(|error| panic!("open exact recovered control owner: {error}"))
}
#[cfg(feature = "bls")]
fn write_authenticated_decision_startup(
    safety: &TempDir,
    marker: u8,
) -> (RecoveredAdapterStartup, wire::HeightContext, Vec<Vec<u8>>) {
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let mut decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: subject(marker),
        execution_commitment: execution_commitment(marker),
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut decision, &keys);
    let startup = write_and_reopen_authenticated_wal_startup(
        safety,
        &context,
        &proofs,
        0,
        [marker; 32],
        vec![WalRecordV2::Decision(decision)],
    );
    (startup, context, proofs)
}
#[cfg(feature = "bls")]
fn reopen_authenticated_decision_startup(
    safety: &TempDir,
    context: &wire::HeightContext,
    proofs: Vec<Vec<u8>>,
    marker: u8,
) -> RecoveredAdapterStartup {
    let verified = VerifiedHeightContext::genesis(context.clone(), proofs)
        .expect("reverify Decision Fetch context");
    SumeragiV2Adapter::open_recovered_startup_with_aggregator(
        safety.path().join("authenticated-fifo-safety.wal"),
        verified,
        Some(0),
        reducer::Generation::new(50),
        [marker; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen authenticated Decision WAL")
}
#[cfg(feature = "bls")]
#[derive(Clone, Copy)]
enum DecisionBodyMarkerFixture {
    Validated,
    Rejected,
    DurableOnly,
}
#[cfg(feature = "bls")]
#[allow(clippy::too_many_lines)]
fn write_decision_startup_with_body_marker(
    safety: &TempDir,
    body_root: &std::path::Path,
    marker: u8,
    outcome: DecisionBodyMarkerFixture,
) -> (
    RecoveredAdapterStartup,
    super::super::v2_body_store::V2BodyStore,
) {
    let (context, keys, proofs) = authenticated_context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let leader = context.leader(round.view);
    let leader_index = usize::try_from(leader).expect("fixture leader index fits usize");
    let header = BlockHeader::new(
        NonZeroU64::new(round.height).expect("fixture height is non-zero"),
        None,
        None,
        None,
        8_000 + u64::from(marker),
        round.view,
    );
    let signature = SignatureOf::try_from_hash(keys[leader_index].private_key(), header.hash())
        .expect("sign Decision body-marker fixture");
    let block = SignedBlock::presigned(
        BlockSignature::new(u64::from(leader), signature),
        header,
        Vec::new(),
    );
    let canonical_wire = block
        .encode_wire()
        .expect("encode Decision body-marker SignedBlockWire");
    let subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: block.hash(),
        payload_hash: Hash::new(&canonical_wire),
    };
    let chunks = wire::encode_payload_chunks(context.da_layout, &canonical_wire)
        .expect("encode Decision body-marker chunks");
    let manifest = wire::PayloadManifest::derive(
        &context,
        round,
        subject,
        u64::try_from(canonical_wire.len()).expect("fixture body length fits u64"),
        &chunks,
    )
    .expect("derive Decision body-marker manifest");
    let mut body_store = super::super::v2_body_store::V2BodyStore::open(body_root, context.clone())
        .expect("open Decision body-marker store");
    let durable = body_store
        .store(manifest, canonical_wire)
        .expect("fsync Decision body-marker body");
    let commitment = execution_commitment(marker);
    let validation = match outcome {
        DecisionBodyMarkerFixture::Validated => Some(
            body_store
                .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
                    Ok::<_, String>(commitment)
                })
                .expect("fsync Decision body validation marker"),
        ),
        DecisionBodyMarkerFixture::Rejected => Some(
            body_store
                .execute_durable_validation(durable.clone(), durable.manifest_hash(), |_| {
                    Err::<wire::ExecutionCommitment, _>(
                        "deterministic Decision body rejection".to_owned(),
                    )
                })
                .expect("fsync Decision body rejection marker"),
        ),
        DecisionBodyMarkerFixture::DurableOnly => None,
    };
    match outcome {
        DecisionBodyMarkerFixture::Validated => {
            assert!(
                validation
                    .as_ref()
                    .and_then(|outcome| outcome.validated_receipt())
                    .is_some()
            )
        }
        DecisionBodyMarkerFixture::Rejected => {
            assert!(
                validation
                    .as_ref()
                    .and_then(|outcome| outcome.rejection_reason())
                    .is_some()
            )
        }
        DecisionBodyMarkerFixture::DurableOnly => assert!(validation.is_none()),
    }
    let mut decision = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment: commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: Vec::new(),
    };
    authenticate_qc(&mut decision, &keys);
    let startup = write_and_reopen_authenticated_wal_startup(
        safety,
        &context,
        &proofs,
        0,
        [marker; 32],
        vec![WalRecordV2::Decision(decision)],
    );
    (startup, body_store)
}
fn lifecycle_owner_config() -> SumeragiV2Config {
    SumeragiV2Config {
        format_version: SUMERAGI_V2_CONFIG_FORMAT_VERSION,
        protocol_version: wire::PROTOCOL_VERSION,
        mode: wire::ConsensusMode::Permissioned,
        block_cadence_ms: 1_000,
        limits: SumeragiV2Limits {
            max_transactions: 512,
            max_payload_bytes: 16 * 1024 * 1024,
            max_queue_scan: 2_048,
            control_queue_capacity: 128,
            runtime_command_capacity: 8,
            runtime_progress_reserve: 2,
            runtime_completion_reserve: 2,
            body_queue_capacity: 16,
            authenticated_non_validator_source_capacity: 2,
            body_bytes: 160 * 1024 * 1024,
            body_source_bytes: 32 * 1024 * 1024,
            chunk_queue_capacity: 64,
            effect_work_capacity: 2,
            ready_body_capacity: 8,
            ready_body_bytes: 32 * 1024 * 1024,
            certified_request_capacity: 8,
            authenticated_merge_qc_capacity: 64,
            merge_leader_body_frame_headroom_bytes: 1024 * 1024,
            autonomous_carrier_headroom_bytes: 1024 * 1024,
            autonomous_producer_recheck_ms: 100,
            historical_recovery_stuck_attempts: 32,
            historical_recovery_retry_tier_attempts: 4,
            historical_recovery_max_retry_tier: 6,
            sidecar_service_burst: 8,
            merge_sidecar_inbound_session_capacity: 32,
            merge_sidecar_inbound_sessions_per_peer: 4,
            merge_sidecar_inbound_assembly_bytes: 64 * 1024 * 1024,
            merge_sidecar_inbound_assembly_bytes_per_peer: 32 * 1024 * 1024,
            merge_sidecar_deferred_block_capacity: 128,
            merge_sidecar_future_block_distance: 64,
            merge_sidecar_request_timeout_ms: 10_000,
            merge_sidecar_outbound_sessions_per_source: 2,
            merge_sidecar_outbound_bytes_per_source: 16 * 1024 * 1024,
            merge_sidecar_server_request_gates_per_source: 4,
            pending_certified_merge_entry_capacity: 1_024,
            pending_queue_plan_admission_capacity: 1_024,
            pending_control_sidecar_bytes: 256 * 1024 * 1024,
            merge_signing_guard_record_capacity: 1_024,
            merge_signing_guard_record_bytes: 16 * 1024 * 1024 + 64 * 1024,
            merge_signing_guard_total_bytes: 256 * 1024 * 1024,
            native_amx_signing_guard_record_capacity: 524_288,
            native_amx_signing_guard_record_bytes: 16 * 1024,
            native_amx_signing_guard_anchor_bytes: 4 * 1024,
        },
        key_policy: SumeragiV2KeyPolicy {
            activation_lead_blocks: 1,
            overlap_grace_blocks: 1,
            expiry_grace_blocks: 1,
            allowed_algorithms: vec![Algorithm::BlsNormal],
        },
    }
}
fn lifecycle_factory_state_for_test(
    kura: Arc<Kura>,
    network_id: iroha_data_model::NetworkId,
) -> Arc<crate::state::State> {
    Arc::new(
        crate::state::State::new_with_chain_and_network_id_for_testing(
            crate::state::World::default(),
            kura,
            crate::query::store::LiveQueryStore::start_test(),
            "sumeragi-v2-lifecycle-test"
                .parse()
                .expect("lifecycle fixture chain id"),
            network_id,
        ),
    )
}
fn quarantined_lifecycle_body_store_for_test(
    body_store: super::super::v2_body_store::V2BodyStore,
) -> super::super::v2_body_store::QuarantinedV2BodyStore {
    body_store
        .into_quarantined_recovered_startup()
        .expect("freshly opened lifecycle body store enters quarantine")
}
fn lifecycle_factory_inputs_for_test(
    startup: &AuthenticatedRecoveredAdapterStartup,
    storage: RecoveredLifecycleStorageAuthorityV1,
    kura: Arc<Kura>,
    local_signer: &KeyPair,
) -> RecoveredLifecycleOwnerFactoryInputsV1 {
    let state = lifecycle_factory_state_for_test(
        Arc::clone(&kura),
        startup.adapter.wire_context.network_id,
    );
    try_lifecycle_factory_inputs_for_test(startup, storage, state, kura, local_signer)
        .unwrap_or_else(|error| panic!("bind exact lifecycle factory inputs: {error}"))
}
#[allow(clippy::result_large_err)]
fn try_lifecycle_factory_inputs_for_test(
    startup: &AuthenticatedRecoveredAdapterStartup,
    storage: RecoveredLifecycleStorageAuthorityV1,
    state: Arc<crate::state::State>,
    kura: Arc<Kura>,
    local_signer: &KeyPair,
) -> Result<RecoveredLifecycleOwnerFactoryInputsV1, ProductionLifecycleOwnerStartupErrorV1> {
    let (events_sender, _events_receiver) = tokio::sync::broadcast::channel(8);
    let queue = Arc::new(crate::queue::Queue::from_config(
        iroha_config::parameters::actual::Queue::default(),
        events_sender.clone(),
    ));
    let block_cadence = state.sumeragi_block_cadence();
    startup.bind_production_lifecycle_owner_factory_inputs_v1(
        super::super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1::for_test(
            local_signer.clone(),
            block_cadence,
        ),
        storage,
        state,
        queue,
        kura,
        None,
        None,
        events_sender,
    )
}
fn open_test_with_capacity_geometry(
    directory: &TempDir,
    capacity_geometry: ServicedCandidateCapacityGeometry,
) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
    SumeragiV2Adapter::open_with_aggregator_and_publication_with_capacity(
        SafetyWalOpenTarget::FixturePath(directory.path().join("capacity-safety.wal")),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(1),
        [0x12; 32],
        fingerprints(),
        Box::new(TestAggregator),
        true,
        capacity_geometry,
        deferred_admission_ordinals(),
    )
}
fn assert_registry_eq(actual: &WireRegistry, expected: &WireRegistry) {
    assert_eq!(actual.wire_context, expected.wire_context);
    assert_eq!(actual.context_id, expected.context_id);
    assert_eq!(actual.peers, expected.peers);
    assert_eq!(actual.validators, expected.validators);
    assert_eq!(actual.subjects, expected.subjects);
    assert_eq!(actual.manifests, expected.manifests);
    assert_eq!(actual.execution_commitments, expected.execution_commitments);
    assert_eq!(actual.certificates, expected.certificates);
    assert_eq!(actual.proposals, expected.proposals);
}
fn open_test_as_leader(
    directory: &TempDir,
) -> Result<(SumeragiV2Adapter, Vec<AdapterEffect>), AdapterError> {
    let context = context();
    let leader = context.leader(0);
    SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("leader-safety.wal"),
        verified_genesis(context),
        Some(leader),
        reducer::Generation::new(1),
        [0x22; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
}
fn unowned_body_event(adapter: &SumeragiV2Adapter, marker: u8) -> reducer::Event {
    reducer::Event::BodyAvailable {
        tag: adapter.current_tag(),
        round: reducer::Round::new(adapter.wire_context.height, adapter.current_tag().view()),
        subject: reducer::Subject::repeat(marker),
    }
}
fn durably_retire_unowned_body_event(adapter: &mut SumeragiV2Adapter, marker: u8) {
    let event = unowned_body_event(adapter, marker);
    assert!(
        adapter
            .enqueue_deferred(event, false, DeferredPriority::Completion, None, None, None,)
            .expect("retain the terminal candidate under exact deferred ownership")
            .is_some()
    );
    assert!(
        adapter
            .drain_deferred()
            .expect("durably retire the terminal candidate")
            .is_empty()
    );
}
#[test]
fn direct_internal_discard_tombstones_a_b_a_and_survives_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let a_marker = 0x31;
    let b_marker = 0x32;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let initial = adapter.serviced_candidate_count_for_test();
        let a = unowned_body_event(&adapter, a_marker);
        let b = unowned_body_event(&adapter, b_marker);
        assert_ne!(a, b);
        assert_ne!(
            adapter
                .step(a.clone())
                .expect("service candidate A")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_ne!(
            adapter
                .step(b)
                .expect("service equal-rank replacement B")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Busy)
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 2);
        assert_eq!(adapter.durable_serviced_candidates.len(), initial + 2);
        assert_eq!(
            adapter
                .step(a)
                .expect("coalesce resurrected candidate A")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 2);
    }
    let context = context();
    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context.clone()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen with exact direct-discard terminal records");
    assert!(startup.is_empty());
    let retained = restarted.serviced_candidate_count_for_test();
    assert_eq!(
        retained, 2,
        "direct and deferred internal NoMatchingWork discards are restart-stable"
    );
    let restarted_a = unowned_body_event(&restarted, a_marker);
    assert_eq!(
        restarted
            .step(restarted_a)
            .expect("coalesce A after process generation changes")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(restarted.serviced_candidate_count_for_test(), retained);
}
#[test]
fn nonquorum_vote_retransmission_rebuilds_volatile_pool_after_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let context = context();
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view: 0,
    };
    let vote = wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Prepare,
        subject: subject(0x35),
        execution_commitment: execution_commitment(0x35),
        signer: 1,
        signature: vec![0x35],
    }));
    let replacement =
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Vote(wire::Vote {
            round,
            proposal_round: round,
            phase: wire::GlobalPhase::Prepare,
            subject: subject(0x35),
            execution_commitment: execution_commitment(0x35),
            signer: 2,
            signature: vec![0x36],
        }));
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote.clone()))
                .expect("admit one nonquorum Prepare vote")
                .disposition(),
            reducer::StepDisposition::Applied
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), 1);
        assert!(
            adapter.durable_serviced_candidates.is_empty(),
            "a volatile quorum contribution is process-local, never a restart tombstone"
        );
        let first_key = IngressSemanticKey::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            signer: 1,
        };
        adapter.ingress_deliveries.remove(&first_key);
        adapter.ingress_equivocations.remove(&first_key);
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(replacement,))
                .expect("service equal-rank candidate B")
                .disposition(),
            reducer::StepDisposition::Applied
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
        let replacement_key = IngressSemanticKey::Vote {
            round,
            phase: wire::GlobalPhase::Prepare,
            signer: 2,
        };
        adapter.ingress_deliveries.remove(&replacement_key);
        adapter.ingress_equivocations.remove(&replacement_key);
        assert_eq!(
            adapter
                .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote.clone()))
                .expect("coalesce candidate A after equal-rank replacement B")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate),
            "same-generation A -> B -> A service must not resurrect A"
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), 2);
        assert!(
            adapter
                .status()
                .expect("one-vote status")
                .liveness
                .prepare_quorums
                .iter()
                .any(|quorum| quorum.round == round && quorum.signer_count == 2)
        );
    }
    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restart after losing the volatile vote pool");
    assert!(startup.is_empty());
    assert_eq!(restarted.serviced_candidate_count_for_test(), 0);
    assert!(
        restarted
            .status()
            .expect("empty post-restart pool")
            .liveness
            .prepare_quorums
            .is_empty()
    );
    assert_eq!(
        restarted
            .receive_authenticated(AuthenticatedConsensusMessage::for_test(vote))
            .expect("retransmission reconstructs the lost vote owner")
            .disposition(),
        reducer::StepDisposition::Applied
    );
    assert!(
        restarted
            .status()
            .expect("rebuilt vote pool")
            .liveness
            .prepare_quorums
            .iter()
            .any(|quorum| quorum.round == round && quorum.signer_count == 1)
    );
}
#[test]
fn deferred_discard_tombstones_before_owner_release_and_restart() {
    let directory = TempDir::new().expect("temporary directory");
    let marker = 0x33;
    {
        let (mut adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let initial = adapter.serviced_candidate_count_for_test();
        let discarded = unowned_body_event(&adapter, marker);
        assert!(
            adapter
                .enqueue_deferred(
                    discarded.clone(),
                    false,
                    DeferredPriority::Completion,
                    None,
                    None,
                    None,
                )
                .expect("retain the candidate under deferred ownership")
                .is_some()
        );
        assert_eq!(adapter.deferred_completions.len(), 1);
        let effects = adapter
            .drain_deferred()
            .expect("service the nondispatchable candidate exactly once");
        assert!(effects.is_empty());
        assert!(adapter.deferred_completions.is_empty());
        assert_eq!(
            adapter.serviced_candidate_count_for_test(),
            initial + 1,
            "the terminal discard must be durable before the deferred owner is released"
        );
        assert_eq!(
            adapter
                .step(discarded)
                .expect("coalesce retransmission after deferred drain")
                .disposition(),
            reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
        );
        assert_eq!(adapter.serviced_candidate_count_for_test(), initial + 1);
    }
    let context = context();
    let (mut restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restore the terminal candidate tombstone");
    assert!(startup.is_empty());
    let retained = restarted.serviced_candidate_count_for_test();
    let retransmitted = unowned_body_event(&restarted, marker);
    assert_eq!(
        restarted
            .step(retransmitted)
            .expect("coalesce retransmission after same-height restart")
            .disposition(),
        reducer::StepDisposition::Ignored(reducer::IgnoreReason::Duplicate)
    );
    assert_eq!(restarted.serviced_candidate_count_for_test(), retained);
}
#[test]
fn serviced_candidate_write_failure_is_fail_closed_and_retains_deferred_owner() {
    let directory = TempDir::new().expect("temporary directory");
    let (mut adapter, startup) = open_test(&directory).expect("open adapter");
    assert!(startup.is_empty());
    durably_retire_unowned_body_event(&mut adapter, 0x40);
    let event = unowned_body_event(&adapter, 0x41);
    assert!(
        adapter
            .enqueue_deferred(event, false, DeferredPriority::Completion, None, None, None,)
            .expect("retain candidate in deferred ownership")
            .is_some()
    );
    let path = adapter
        .serviced_candidate_store_path_for_test()
        .to_path_buf();
    std::fs::remove_file(&path).expect("remove published snapshot");
    std::fs::create_dir(&path).expect("replace snapshot target with a directory");
    let retained = adapter.deferred_completions.len();
    assert!(matches!(
        adapter.drain_deferred(),
        Err(AdapterError::ServicedCandidateStore(_))
    ));
    assert!(adapter.fail_closed);
    assert_eq!(
        adapter.deferred_completions.len(),
        retained,
        "failed publication retains the selected owner before fail-stop"
    );
}
#[test]
fn restored_producer_reuses_runtime_key_and_ordinal_and_does_not_resurrect() {
    let directory = TempDir::new().expect("temporary directory");
    let causal_key;
    {
        let (adapter, startup) = open_test(&directory).expect("open adapter");
        assert!(startup.is_empty());
        let started_at = Instant::now();
        let lifecycle_ordinals =
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let (mut runtime, startup) =
            super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                adapter,
                startup,
                started_at,
                Duration::from_secs(4),
                super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
                lifecycle_ordinals,
            )
            .expect("construct the original serialized runtime");
        assert!(startup.is_empty());
        runtime
            .arm_live_clocks(started_at)
            .expect("arm the original runtime");
        let owner = runtime
            .frozen_timeout_owner_for_test(started_at + Duration::from_secs(4))
            .expect("freeze the deterministic original timeout owner");
        causal_key = owner.causal_origin().lifecycle_key;
        assert_eq!(owner.lifecycle_ordinal(), 1);
        let mut adapter = runtime.into_driver();
        let event = reducer::Event::TimeoutElapsed {
            tag: adapter.current_tag(),
        };
        let candidate = adapter
            .serviced_candidate(&event, DeferredPriority::Completion, None, None)
            .expect("timeout has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(causal_key, owner.lifecycle_ordinal())
            .expect("bind selected source");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("reserve before source retirement")
            .expect("tracked candidate reserves an address");
        let address = reservation.address;
        assert_eq!(
            adapter.producer_continuations[&address].status(),
            ProducerContinuationStatus::Reserved
        );
        assert_eq!(
            adapter.durable_producer_continuations.get(&address),
            adapter.producer_continuations.get(&address),
            "reservation is synchronized before its source can retire"
        );
    }
    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restart with exact active admission metadata");
    assert!(startup.is_empty());
    let restored = restarted
        .producer_continuations
        .values()
        .next()
        .expect("active producer metadata reopens");
    assert_eq!(restored.status(), ProducerContinuationStatus::Reserved);
    assert_eq!(restored.identity().admission_ordinal(), 1);
    let restored_address = restored.identity().address();
    assert_eq!(restored_address.lifecycle_slot(), 1);
    assert_eq!(
        restarted.restored_producer_continuation_ordinal_high_watermark(),
        Some(1)
    );
    assert!(
        restarted
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );
    assert!(
        restarted
            .dormant_local_fifo_reservations()
            .expect("validate restored timeout metadata")
            .is_empty(),
        "a restart-dormant timeout remains a non-FIFO clock root"
    );
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(1);
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            startup,
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals,
        )
        .expect("construct the restarted serialized runtime");
    assert!(startup.is_empty());
    let completion_capacity_before_clock_arm = runtime.remaining_completion_capacity();
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime");
    assert_eq!(
        runtime.remaining_completion_capacity(),
        completion_capacity_before_clock_arm,
        "arming non-FIFO clock roots cannot consume completion capacity"
    );
    let retransmit_due = started_at + runtime.retransmit_interval();
    assert!(
        runtime
            .try_step_pacemaker_escape(retransmit_due)
            .expect("freeze the newer post-restart retransmit")
            .is_none(),
        "a periodic timer alone is not a pacemaker escape"
    );
    let timeout_due = started_at + runtime.round_timeout();
    let step = runtime
        .try_step_pacemaker_escape(timeout_due)
        .expect("replayed timeout supersedes the newer frozen retransmit")
        .expect("the due absolute timeout owns the pacemaker step");
    let super::super::v2_runtime::RuntimeStep::Advanced(effects) = step else {
        panic!("the exact replayed timeout must advance");
    };
    assert!(!effects.is_empty(), "timeout retains a concrete successor");
    let scheduler = runtime
        .take_last_scheduler_ownership()
        .expect("timeout publishes exact scheduler ownership");
    assert_eq!(
        scheduler.selected,
        super::super::v2_runtime::RuntimeSelectedOwnerKind::Timeout
    );
    let effect_ownership = runtime
        .take_effect_ownership(effects.len())
        .expect("take the concrete successor ownership");
    assert!(
        effect_ownership
            .iter()
            .all(|ownership| ownership.owner().lifecycle_ordinal() == 1),
        "every concrete successor retains the original owner 1"
    );
    let retained = runtime
        .driver()
        .producer_continuations
        .get(&restored_address)
        .expect("runtime acknowledgement retains its process-local terminal");
    assert_eq!(
        retained.identity().admission_ordinal(),
        1,
        "restart cannot replace the immutable first-admission ordinal"
    );
    assert_eq!(
        retained.identity().causal_lifecycle_key(),
        effect_ownership[0].owner().causal_origin().lifecycle_key
    );
    assert_eq!(
        retained.identity().causal_lifecycle_key(),
        causal_key,
        "the exact retry retains its persisted causal identity"
    );
    assert_eq!(retained.status(), ProducerContinuationStatus::Terminal);
    assert!(
        !runtime
            .driver()
            .durable_producer_continuations
            .contains_key(&restored_address),
        "a concrete volatile successor removes the dormant restart record"
    );
    assert!(
        !runtime
            .driver()
            .restored_dormant_producer_continuations
            .contains(&restored_address)
    );
    drop(runtime.into_driver());
    let (restarted_again, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("restart after the runtime handoff");
    assert!(
        matches!(
            startup.as_slice(),
            [AdapterEffect::Sign {
                request: SignRequest::TimeoutVote(_),
                ..
            }]
        ),
        "restart reconstructs the durable exact successor instead of the drained timeout stage"
    );
    assert!(
        restarted_again.producer_continuations.is_empty()
            && restarted_again.durable_producer_continuations.is_empty()
            && restarted_again
                .restored_dormant_producer_continuations
                .is_empty(),
        "the drained logical request cannot be recreated at its old stage"
    );
}
struct StageSevenCrashCut {
    wire_context: wire::HeightContext,
    round: wire::ConsensusRound,
    body_subject: wire::BlockSubject,
    manifest: wire::PayloadManifest,
    logical_key: Hash,
    logical_ordinal: u128,
    restored_address: ProducerContinuationAddress,
}
fn persist_stage_seven_crash_cut(directory: &TempDir, marker: u8) -> StageSevenCrashCut {
    let wire_context = context();
    let round = wire::ConsensusRound {
        context_id: wire_context.id(),
        height: wire_context.height,
        view: 0,
    };
    let body_subject = subject(marker);
    let payload = vec![marker; 32];
    let chunks = wire::encode_payload_chunks(wire_context.da_layout, &payload)
        .expect("encode canonical body chunks");
    let manifest = wire::PayloadManifest::derive(
        &wire_context,
        round,
        body_subject,
        u64::try_from(payload.len()).expect("fixture payload length fits u64"),
        &chunks,
    )
    .expect("derive canonical body manifest");
    let logical_key;
    let logical_ordinal;
    let restored_address;
    {
        let (adapter, startup) = open_test(directory).expect("open original adapter");
        assert!(startup.is_empty());
        let tag = adapter.current_tag();
        let fetch = AdapterEffect::FetchBody {
            tag,
            round,
            subject: body_subject,
            manifest: Some(manifest.clone()),
            certified_sources: Vec::new(),
            certificate: None,
        };
        let started_at = Instant::now();
        let lifecycle_ordinals =
            super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(0);
        let (mut runtime, startup) =
            super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
                adapter,
                vec![fetch.clone()],
                started_at,
                Duration::from_secs(4),
                super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
                lifecycle_ordinals,
            )
            .expect("construct runtime with the original body fetch");
        assert_eq!(startup, vec![fetch]);
        let mut ownership = runtime
            .take_effect_ownership(1)
            .expect("take the original body-fetch ownership");
        let fetch_ownership = ownership.pop().expect("one body-fetch owner");
        assert!(ownership.is_empty());
        logical_key = fetch_ownership.owner().causal_origin().lifecycle_key;
        logical_ordinal = fetch_ownership.owner().lifecycle_ordinal();
        assert_eq!(logical_ordinal, 1);
        let mut adapter = runtime.into_driver();
        let event = reducer::Event::BodyAvailable {
            tag,
            round: reducer::Round::new(round.height, round.view),
            subject: reducer::Subject::new(Hash::new(body_subject.encode()).into()),
        };
        let completion_evidence = BodyPipelineCompletionEvidence::BodyAvailable {
            manifest: manifest.clone(),
        };
        let candidate = adapter
            .serviced_candidate(
                &event,
                DeferredPriority::Completion,
                Some(&completion_evidence),
                None,
            )
            .expect("BodyAvailable has a producer stage");
        adapter
            .bind_selected_producer_lifecycle(logical_key, logical_ordinal)
            .expect("bind the body-fetch lifecycle");
        let reservation = adapter
            .reserve_selected_producer_continuation(Some(candidate))
            .expect("persist before the BodyAvailable reducer step")
            .expect("BodyAvailable reserves a producer continuation");
        restored_address = reservation.address;
        let record = &adapter.producer_continuations[&restored_address];
        assert_eq!(record.status(), ProducerContinuationStatus::Reserved);
        assert_eq!(
            record.identity().stage(),
            ServicedCandidateStage::BodyAvailable as u8
        );
        assert_eq!(
            record.source_class(),
            ProducerContinuationSourceClass::VolatileBody
        );
        assert_eq!(
            adapter
                .durable_producer_continuations
                .get(&restored_address),
            Some(record),
            "the stage-7 crash cut must be durable before reducer service"
        );
    }
    StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    }
}
#[test]
fn body_rebind_coalescence_preserves_the_only_persistent_producer() {
    let directory = TempDir::new().expect("temporary durable coalescence directory");
    let StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    } = persist_stage_seven_crash_cut(&directory, 0xBC);
    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen the stage-7 coalescence crash cut");
    assert!(startup.is_empty());
    let previous = restarted.current_tag();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: body_subject,
        execution_commitment: wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"coalescence parent state"),
            Hash::new(b"coalescence post state"),
            Hash::new(b"coalescence writes"),
            1,
            Hash::new(b"coalescence executed block"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xBC; 96],
    };
    certificate
        .validate(&wire_context)
        .expect("coalescence reconstruction certificate is structurally valid");
    let protected_lock = wire::QuorumCertificate {
        phase: wire::GlobalPhase::Prepare,
        ..certificate.clone()
    };
    let fetch = AdapterEffect::FetchBody {
        tag: previous,
        round,
        subject: body_subject,
        manifest: Some(manifest.clone()),
        certified_sources: wire_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            logical_ordinal,
        );
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            vec![fetch.clone()],
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals,
        )
        .expect("construct runtime for durable coalescence");
    assert_eq!(startup, vec![fetch]);
    let mut ownership = runtime
        .take_effect_ownership(1)
        .expect("take the reconstructed body-fetch owner");
    let fetch_ownership = ownership.pop().expect("one reconstructed fetch owner");
    let reservation = runtime
        .reserve_body_available_with_owner(previous, manifest.clone(), &fetch_ownership)
        .expect("reserve the restart-restored body completion");
    runtime
        .commit_body_available(reservation)
        .expect("materialize the restart-restored source owner");
    let rebound = reducer::EventTag::new(
        previous.height(),
        previous.view() + 1,
        reducer::Generation::new(previous.generation().get() + 1),
    );
    runtime
        .observe_effects_with_test_ownership(
            started_at,
            &[AdapterEffect::EnterView {
                tag: rebound,
                certificate: wire::TimeoutCertificate {
                    round,
                    groups: vec![wire::TimeoutVoteGroup {
                        highest_prepare_qc: None,
                        signers: vec![0, 1, 2],
                        aggregate_signature: vec![0xCD; 96],
                    }],
                },
                protected_lock: Some(protected_lock),
            }],
        )
        .expect("install the certified destination incarnation");
    runtime
        .enqueue_volatile_body_available_for_test(rebound, manifest.clone())
        .expect("stage an independently volatile destination owner");
    assert_eq!(runtime.queued_commands(), 2);
    assert!(
        runtime
            .rebind_body_available(previous, rebound, &manifest)
            .expect("coalesce while retaining the persistent source")
    );
    assert_eq!(runtime.queued_commands(), 1);
    assert!(
        !runtime
            .rebind_body_available(previous, rebound, &manifest)
            .expect("the old source tag is now vacant")
    );
    let retained = runtime
        .driver()
        .producer_continuations
        .get(&restored_address)
        .expect("coalescence retains the process producer record");
    assert_eq!(retained.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(retained.identity().admission_ordinal(), logical_ordinal);
    assert_eq!(
        runtime
            .driver()
            .durable_producer_continuations
            .get(&restored_address),
        Some(retained),
    );
    assert!(
        runtime
            .driver()
            .restored_dormant_producer_continuations
            .contains(&restored_address),
        "the rebound volatile carrier aliases the same restart-dormant producer",
    );
    drop(runtime.into_driver());
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after durable-owner coalescence");
    let reopened = restarted_again
        .producer_continuations
        .get(&restored_address)
        .expect("the surviving producer remains restart-recoverable");
    assert_eq!(reopened.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(reopened.identity().admission_ordinal(), logical_ordinal);
    assert!(
        restarted_again
            .restored_dormant_producer_continuations
            .contains(&restored_address),
    );
}
#[test]
fn restored_body_available_reuses_logical_lifecycle_spends_one_fresh_slot_and_does_not_resurrect() {
    let directory = TempDir::new().expect("temporary directory");
    let StageSevenCrashCut {
        wire_context,
        round,
        body_subject,
        manifest,
        logical_key,
        logical_ordinal,
        restored_address,
    } = persist_stage_seven_crash_cut(&directory, 0xB7);
    let (restarted, startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(2),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen the stage-7 crash cut");
    assert!(startup.is_empty());
    let restored = restarted
        .producer_continuations
        .get(&restored_address)
        .expect("stage-7 logical lifecycle reopens");
    assert_eq!(restored.status(), ProducerContinuationStatus::Reserved);
    assert_eq!(restored.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(restored.identity().admission_ordinal(), logical_ordinal);
    assert_eq!(
        restored.source_class(),
        ProducerContinuationSourceClass::VolatileBody
    );
    assert!(
        restarted
            .dormant_local_fifo_reservations()
            .expect("validate restored BodyAvailable metadata")
            .is_empty(),
        "stage 7 preserves logical identity without a latent FIFO slot"
    );
    let restarted_tag = restarted.current_tag();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject: body_subject,
        execution_commitment: wire::ExecutionCommitment::without_kagemusha_top_ups_or_merge_carrier(
            Hash::new(b"stage-seven parent state"),
            Hash::new(b"stage-seven post state"),
            Hash::new(b"stage-seven writes"),
            1,
            Hash::new(b"stage-seven executed block"),
        ),
        signers: vec![0, 1, 2],
        aggregate_signature: vec![0xB7; 96],
    };
    certificate
        .validate(&wire_context)
        .expect("certified reconstruction is structurally valid");
    let reconstructed_fetch = AdapterEffect::FetchBody {
        tag: restarted_tag,
        round,
        subject: body_subject,
        manifest: Some(manifest.clone()),
        certified_sources: wire_context
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect(),
        certificate: Some(certificate),
    };
    let lifecycle_ordinals =
        super::super::v2_runtime::RuntimeLifecycleOrdinalSource::after_high_watermark(
            logical_ordinal,
        );
    let started_at = Instant::now();
    let (mut runtime, startup) =
        super::super::v2_runtime::SerializedV2Runtime::new_with_lifecycle_ordinals(
            restarted,
            vec![reconstructed_fetch.clone()],
            started_at,
            Duration::from_secs(4),
            super::super::v2_runtime::RuntimeQueueConfig::new(6, 2, 1),
            lifecycle_ordinals.clone(),
        )
        .expect("construct runtime with the reconstructed body fetch");
    assert_eq!(startup, vec![reconstructed_fetch.clone()]);
    runtime
        .arm_live_clocks(started_at)
        .expect("arm the restarted runtime");
    let mut ownership = runtime
        .take_effect_ownership(1)
        .expect("take reconstructed body-fetch ownership");
    let fetch_ownership = ownership.pop().expect("one reconstructed fetch owner");
    assert!(ownership.is_empty());
    assert_ne!(
        fetch_ownership.owner().causal_origin().lifecycle_key,
        logical_key,
        "certified reconstruction owns a different physical Fetch lifecycle"
    );
    assert_eq!(
        fetch_ownership.owner().lifecycle_ordinal(),
        logical_ordinal + 1
    );
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect the shared source before completion admission"),
        Some(logical_ordinal + 2),
        "the certified Fetch owns one new external lifecycle before completion admission"
    );
    let capacity_before = runtime.remaining_completion_capacity();
    let reservation = runtime
        .reserve_body_available_with_owner(restarted_tag, manifest.clone(), &fetch_ownership)
        .expect("reserve the reconstructed stage-7 completion");
    assert!(reservation.owns_new_slot());
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before - 1);
    let source_after_reserve = lifecycle_ordinals
        .next_ordinal_for_test()
        .expect("inspect the shared source after completion admission");
    assert_eq!(source_after_reserve, Some(logical_ordinal + 3));
    let retry = runtime
        .reserve_body_available_with_owner(restarted_tag, manifest.clone(), &fetch_ownership)
        .expect("exact reconstruction retry coalesces with its token");
    assert_eq!(retry, reservation);
    assert_eq!(runtime.remaining_completion_capacity(), capacity_before - 1);
    assert_eq!(
        lifecycle_ordinals
            .next_ordinal_for_test()
            .expect("inspect the shared source after exact retry"),
        source_after_reserve,
        "an exact retry cannot spend a second physical admission position"
    );
    runtime
        .commit_body_available(retry)
        .expect("materialize the reconstructed completion");
    assert_eq!(runtime.queued_commands(), 1);
    let step = runtime
        .step(started_at)
        .expect("service the restored BodyAvailable handoff");
    let super::super::v2_runtime::RuntimeStep::Advanced(effects) = step else {
        panic!("the restored BodyAvailable completion must dispatch");
    };
    runtime
        .take_last_scheduler_ownership()
        .expect("BodyAvailable dispatch publishes scheduler ownership");
    if !effects.is_empty() {
        runtime
            .take_effect_ownership(effects.len())
            .expect("take BodyAvailable successor ownership");
    }
    let terminal = runtime
        .driver()
        .producer_continuations
        .get(&restored_address)
        .expect("service acknowledgement retains a process-local terminal");
    assert_eq!(terminal.status(), ProducerContinuationStatus::Terminal);
    assert_eq!(terminal.identity().causal_lifecycle_key(), logical_key);
    assert_eq!(terminal.identity().admission_ordinal(), logical_ordinal);
    assert!(
        !runtime
            .driver()
            .durable_producer_continuations
            .contains_key(&restored_address),
        "the service handoff removes the restart-stable stage-7 record"
    );
    drop(runtime.into_driver());
    let (restarted_again, _startup) = SumeragiV2Adapter::open_with_aggregator(
        directory.path().join("safety.wal"),
        verified_genesis(context()),
        Some(0),
        reducer::Generation::new(3),
        [0x11; 32],
        fingerprints(),
        Box::new(TestAggregator),
        deferred_admission_ordinals(),
    )
    .expect("reopen after the stage-7 service handoff");
    assert!(
        restarted_again.producer_continuations.is_empty()
            && restarted_again.durable_producer_continuations.is_empty()
            && restarted_again
                .restored_dormant_producer_continuations
                .is_empty(),
        "the serviced old stage cannot resurrect on a second restart"
    );
}
