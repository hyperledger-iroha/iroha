fn bind_productive_orphan_test_ingress(
    service: &mut ProductionV2Services,
    directory: &TempDir,
) -> Arc<FairV2Ingress> {
    let roster = service
        .context
        .roster
        .iter()
        .map(|entry| entry.validator.clone())
        .collect::<Vec<_>>();
    let ingress = Arc::new(
        FairV2Ingress::new_with_source_geometry_and_transport_frame_caps(
            64,
            512 * 1024 * 1024,
            64 * 1024 * 1024,
            super::super::CERTIFIED_FENCE_ESCAPE_RESERVE_BYTES,
            8 * 1024 * 1024,
            8 * 1024 * 1024,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            usize::MAX,
            None,
        ),
    );
    ingress
        .configure_roster_for_context(
            roster.clone(),
            &service.context.network_id,
            service.context.da_layout,
        )
        .expect("configure productive-orphan ingress");
    ingress.require_leader_wire_lifecycle_gate();
    let capacity =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::derived_capacity(
            roster.len(),
            service.context.da_layout.max_chunk_count,
        )
        .expect("derive productive-orphan lifecycle capacity");
    let owner = [0xE2; 32];
    let recovery_authority =
        super::super::serviced_candidate_store::LeaderWireRecoveryAuthority::from_replayed_adapter(
            service.context.id(),
            service.context.height,
            owner,
            0,
            false,
        );
    let (gate, restore) =
        super::super::serviced_candidate_store::LeaderWireLifecycleStoreGate::open(
            &directory.path().join("productive-orphan-tail.wal"),
            service.context.id(),
            service.context.height,
            owner,
            roster.iter().cloned().collect(),
            capacity,
            service.context.da_layout.max_chunk_count,
            recovery_authority,
            &[],
            &[],
        )
        .expect("open productive-orphan lifecycle gate");
    ingress
        .bind_leader_wire_lifecycle_gate(
            gate,
            restore,
            RuntimeLifecycleOrdinalSource::after_high_watermark(64),
            service.context.id(),
            service.context.height,
        )
        .expect("bind productive-orphan lifecycle gate");
    ingress.open().expect("open productive-orphan ingress");
    service.leader_wire_recovery_authority = recovery_authority;
    service.leader_wire_ingress = Arc::clone(&ingress);
    ingress
}

fn admit_productive_orphan_runtime(
    ingress: &FairV2Ingress,
    message: wire::ConsensusMessageV2,
    sender: PeerId,
) -> FairV2IngressOwnershipEvidence {
    assert!(matches!(
        ingress.try_push(InboundBlockMessage::new(
            BlockMessage::V2(message),
            Some(sender),
        )),
        Ok(FairV2IngressPushDisposition::Enqueued)
    ));
    let mut admitted = ingress.try_recv().expect("drain productive-orphan ingress");
    let mut ownership = admitted
        .take_ingress_ownership()
        .expect("productive orphan retains fair-ingress ownership");
    ingress
        .bind_leader_wire_runtime_ownership(&mut ownership)
        .expect("bind productive-orphan runtime receipt");
    ownership
}

fn buffer_productive_orphan_for_replay(
    service: &mut ProductionV2Services,
    ingress: &FairV2Ingress,
    sender: PeerId,
    chunk: wire::PayloadChunk,
) -> super::super::FairV2IngressLeaderWireToken {
    let ownership = admit_productive_orphan_runtime(
        ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())),
        sender.clone(),
    );
    let token = ownership
        .leader_wire_token()
        .expect("productive orphan has a leader-wire token")
        .clone();
    assert_eq!(
        service.buffer_orphan_payload_chunk_owned(sender, chunk, ownership),
        PayloadChunkDisposition::Buffered
    );
    token
}

fn productive_chunk_at_view(
    service: &ProductionV2Services,
    keys: &[KeyPair],
    view: u64,
) -> (
    Vec<u8>,
    wire::PayloadManifest,
    wire::Proposal,
    wire::PayloadChunk,
    PeerId,
) {
    let (canonical_wire, payload) = proposal_body_and_payload_at_view(&service.context, keys, view);
    let (manifest, chunks) = payload.into_parts();
    assert!(
        !chunks.is_empty(),
        "fixture body must have an exact data chunk"
    );
    let proposer = service.context.leader(view);
    let proposer_index = usize::try_from(proposer).expect("small proposer index");
    let sender = service.context.roster[proposer_index].validator.clone();
    let proposal = wire::Proposal {
        round: manifest.round,
        proposer,
        subject: manifest.subject,
        manifest: manifest.clone(),
        justification: wire::ProposalJustification::ParentCommit(wire::ParentCommitJustification {
            certificate: None,
        }),
        signature: Vec::new(),
    };
    let mut chunk = wire::PayloadChunk {
        manifest_hash: HashOf::new(&manifest),
        index: 0,
        bytes: chunks.into_iter().next().expect("fixture data chunk"),
        sender: proposer,
        signature: Vec::new(),
    };
    chunk.signature = Signature::new(
        keys[proposer_index].private_key(),
        &chunk
            .signature_preimage(&service.context, &manifest)
            .expect("chunk signature preimage"),
    )
    .payload()
    .to_vec();
    (canonical_wire, manifest, proposal, chunk, sender)
}

fn admit_and_terminalize_productive_proposal(
    ingress: &FairV2Ingress,
    proposal: wire::Proposal,
    sender: PeerId,
) {
    let ownership = admit_productive_orphan_runtime(
        ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal)),
        sender,
    );
    ingress
        .mark_leader_wire_volatile_terminal(
            ownership
                .leader_wire_runtime_receipt()
                .expect("proposal has productive runtime ownership"),
        )
        .expect("terminalize proposal after binding its manifest coordinates");
}

fn chunk_effect_executor(
    service: &ProductionV2Services,
    recovered: BTreeMap<
        (wire::ConsensusRound, wire::BlockSubject),
        (wire::PayloadManifest, DurableBodyReceipt),
    >,
) -> V2EffectExecutor<SaturatedCompletionRuntime> {
    V2EffectExecutor::with_runtime(
        SaturatedCompletionRuntime::new(0, 8),
        recovered,
        service.context.clone(),
        service.local_peer.clone(),
        service.local_validator,
        EffectQueueConfig::default(),
    )
    .expect("construct productive-chunk effect executor")
}

#[test]
#[allow(clippy::too_many_lines)]
fn durable_reconstructed_body_terminalizes_late_chunk_across_arrival_order() {
    for durable_before_late_chunk in [false, true] {
        let (mut service, keys) = fixture_with_block_payload();
        service.max_orphan_chunks = 16;
        service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
        let gate_directory = TempDir::new().expect("temporary durable-chunk gate");
        let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
        let (_, manifest, proposal, chunk, sender) = productive_chunk_at_view(&service, &keys, 0);
        admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());
        let ownership = admit_productive_orphan_runtime(
            &ingress,
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(
                chunk.clone(),
            )),
            sender.clone(),
        );
        let token = ownership
            .leader_wire_token()
            .expect("late chunk has a productive token")
            .clone();
        let durable = DurableBodyReceipt::for_test(
            service.context.id(),
            manifest.round,
            manifest.subject,
            HashOf::new(&manifest),
        );
        let recovered = BTreeMap::from([(
            (manifest.round, manifest.subject),
            (manifest.clone(), durable),
        )]);
        let mut executor = chunk_effect_executor(
            &service,
            if durable_before_late_chunk {
                recovered.clone()
            } else {
                BTreeMap::new()
            },
        );
        let disposition = service
            .route_payload_chunk(&mut executor, sender.clone(), chunk, ownership)
            .expect("route late chunk around durable recovery");
        if durable_before_late_chunk {
            assert_eq!(
                disposition,
                PayloadChunkDisposition::Duplicate,
                "pre-existing durable recovery must terminalize the late chunk immediately"
            );
        } else {
            assert_eq!(
                disposition,
                PayloadChunkDisposition::Buffered,
                "the late chunk must remain owned until durable recovery arrives"
            );
            assert_eq!(service.orphan_chunk_count, 1);
            assert_ne!(service.orphan_chunk_bytes, 0);
            executor = chunk_effect_executor(&service, recovered);
            assert_eq!(
                service
                    .replay_buffered_chunks(&mut executor)
                    .expect("durable recovery sweeps the buffered runtime owner"),
                0
            );
        }
        assert!(
            service.orphan_chunks.is_empty(),
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert_eq!(
            service.orphan_chunk_count, 0,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert_eq!(
            service.orphan_chunk_bytes, 0,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert_eq!(
            ingress.state.lock().leader_wire_lifecycles[&token.slot].status,
            super::super::FairV2IngressLeaderWireStatus::Terminal,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );

        let next_view = (1..=1_024)
            .find(|view| service.context.leader(*view) == service.context.leader(0))
            .expect("bounded view search returns to the same leader");
        let (_, _, next_proposal, next_chunk, next_sender) =
            productive_chunk_at_view(&service, &keys, next_view);
        assert_eq!(
            next_sender, sender,
            "view rotation returns to the same origin"
        );
        admit_and_terminalize_productive_proposal(&ingress, next_proposal, next_sender.clone());
        let next = admit_productive_orphan_runtime(
            &ingress,
            wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(
                next_chunk,
            )),
            next_sender,
        );
        let next_token = next.leader_wire_token().expect("next-view token");
        assert_eq!(
            next_token.view(),
            next_view,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
        assert!(
            next.leader_wire_runtime_receipt().is_some(),
            "higher-view chunk must reach Runtime admission"
        );
        assert_eq!(
            ingress.state.lock().leader_wire_lifecycles[&next_token.slot].status,
            super::super::FairV2IngressLeaderWireStatus::Runtime,
            "durable_before_late_chunk={durable_before_late_chunk}"
        );
    }
}

#[test]
fn productive_orphan_lifecycle_sweep_bounds_turns_services_completion_and_wraps() {
    let (mut service, keys) = fixture_with_block_payload();
    let capacity = usize::try_from(service.context.da_layout.max_chunk_count)
        .expect("fixture orphan capacity fits usize");
    service.max_orphan_chunks = capacity;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let gate_directory = TempDir::new().expect("temporary bounded orphan-sweep gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let mut complete_recovered = BTreeMap::new();
    let mut recovered_keys = Vec::with_capacity(capacity);
    let mut tokens = Vec::with_capacity(capacity);

    for view in 0..u64::try_from(capacity).expect("fixture capacity fits u64") {
        let (_, manifest, proposal, chunk, sender) =
            productive_chunk_at_view(&service, &keys, view);
        admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());
        let manifest_hash = HashOf::new(&manifest);
        let token = buffer_productive_orphan_for_replay(&mut service, &ingress, sender, chunk);
        tokens.push((manifest_hash, token));
        let durable = DurableBodyReceipt::for_test(
            service.context.id(),
            manifest.round,
            manifest.subject,
            manifest_hash,
        );
        let key = (manifest.round, manifest.subject);
        recovered_keys.push((manifest_hash, key));
        complete_recovered.insert(key, (manifest, durable));
    }
    assert_eq!(service.orphan_chunk_count, capacity);
    assert_eq!(
        service
            .orphan_chunks
            .values()
            .map(VecDeque::len)
            .sum::<usize>(),
        capacity
    );

    // Keep the last deterministic sweep position live while every other
    // exact owner is already durable. This forces a full cursor cycle and
    // a wrap before the final owner can retire.
    let retained_manifest_hash = *service
        .orphan_chunks
        .keys()
        .next_back()
        .expect("capacity fixture has a final manifest");
    let retained_key = recovered_keys
        .iter()
        .find_map(|(manifest_hash, key)| (*manifest_hash == retained_manifest_hash).then_some(*key))
        .expect("retained manifest has exact recovered coordinates");
    let mut partial_recovered = complete_recovered.clone();
    assert!(partial_recovered.remove(&retained_key).is_some());
    let mut executor = chunk_effect_executor(&service, partial_recovered);

    let (command_tx, _command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    completion_tx
        .try_send(V2IoCompletion::AuxiliaryNoop)
        .expect("queue completion behind the first bounded sweep");

    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect("first bounded lifecycle sweep remains valid"),
        0
    );
    assert_eq!(service.orphan_chunk_count, capacity.saturating_sub(1));
    assert_eq!(
        service
            .drain_completions(&mut executor)
            .expect("bounded sweep returns a completion service opportunity"),
        1,
        "a ready service completion must run before the next lifecycle sweep"
    );
    // No worker owns this synthetic channel; remove it before service Drop
    // attempts the production shutdown handshake.
    drop(service.io.take());

    for _ in 1..capacity {
        let before = service.orphan_chunk_count;
        assert_eq!(
            service
                .replay_buffered_chunks(&mut executor)
                .expect("bounded lifecycle sweep remains valid"),
            0,
            "terminal lifecycle sweeping must not report chunk delivery"
        );
        assert!(
            before.saturating_sub(service.orphan_chunk_count) <= 1,
            "one service turn may deeply classify at most one orphan"
        );
    }
    assert_eq!(
        service.orphan_chunk_count, 1,
        "one Retain owner must not starve the durable tail during a complete cursor cycle"
    );
    let retained_token = tokens
        .iter()
        .find_map(|(manifest_hash, token)| {
            (*manifest_hash == retained_manifest_hash).then_some(token)
        })
        .expect("retained manifest has a lifecycle token");
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles[&retained_token.slot].status,
        super::super::FairV2IngressLeaderWireStatus::Runtime
    );

    let mut complete_executor = chunk_effect_executor(&service, complete_recovered);
    assert_eq!(
        service
            .replay_buffered_chunks(&mut complete_executor)
            .expect("cursor wrap reaches the newly durable retained owner"),
        0
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(tokens.iter().all(|(_, token)| {
        ingress.state.lock().leader_wire_lifecycles[&token.slot].status
            == super::super::FairV2IngressLeaderWireStatus::Terminal
    }));
    assert_eq!(
        service
            .replay_buffered_chunks(&mut complete_executor)
            .expect("an empty lifecycle sweep is idle"),
        0
    );
    assert!(service.orphan_lifecycle_sweep_cursor.is_none());
}

#[test]
fn productive_retry_after_proofless_reconstruction_does_not_become_orphan() {
    let (mut service, keys) = fixture_with_block_payload();
    service.max_orphan_chunks = 16;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let _chunk_root = install_temporary_chunk_root(&mut service);
    let gate_directory = TempDir::new().expect("temporary reconstructed-chunk gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let (_, manifest, proposal, chunk, sender) = productive_chunk_at_view(&service, &keys, 0);

    let proofless = admit_productive_orphan_runtime(
        &ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())),
        sender.clone(),
    );
    assert!(proofless.leader_wire_runtime_receipt().is_none());
    let mut executor = chunk_effect_executor(&service, BTreeMap::new());
    assert_eq!(
        service
            .route_payload_chunk(&mut executor, sender.clone(), chunk.clone(), proofless)
            .expect("buffer proofless chunk"),
        PayloadChunkDisposition::Buffered
    );

    admit_and_terminalize_productive_proposal(&ingress, proposal, sender.clone());
    let tag = service.active_tag;
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut service,
        )
        .expect("open proofless reconstruction fetch");
    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect("reconstruct proofless body"),
        1
    );
    assert_eq!(service.local_completions.len(), 1);

    let productive = admit_productive_orphan_runtime(
        &ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::PayloadChunk(chunk.clone())),
        sender.clone(),
    );
    let token = productive
        .leader_wire_token()
        .expect("retransmit binds productive token")
        .clone();
    assert_eq!(
        service
            .route_payload_chunk(&mut executor, sender, chunk, productive)
            .expect("queued reconstruction owns the exact bytes"),
        PayloadChunkDisposition::Duplicate
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(
        ingress.state.lock().leader_wire_lifecycles[&token.slot].status,
        super::super::FairV2IngressLeaderWireStatus::VolatileTerminal
    );
}

#[test]
fn session_changed_terminal_failure_still_retires_productive_orphan_tail() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    service.max_orphan_chunks = 4;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let _chunk_root = install_temporary_chunk_root(&mut service);
    let gate_directory = TempDir::new().expect("temporary productive-orphan gate");
    let ingress = bind_productive_orphan_test_ingress(&mut service, &gate_directory);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let proposer = service.context.roster
        [usize::try_from(proposal.proposer).expect("small proposer index")]
    .validator
    .clone();
    let _proposal_ownership = admit_productive_orphan_runtime(
        &ingress,
        wire::ConsensusMessageV2::new(wire::ConsensusMessageV2Payload::Proposal(proposal.clone())),
        proposer,
    );
    let (manifest, chunks) = payload.into_parts();
    assert_eq!(chunks.len(), 1, "fixture body must have one exact chunk");
    let mut completing_chunk = wire::PayloadChunk {
        manifest_hash: HashOf::new(&manifest),
        index: 0,
        bytes: chunks.into_iter().next().expect("one fixture chunk"),
        sender: 0,
        signature: Vec::new(),
    };
    completing_chunk.signature = Signature::new(
        keys[0].private_key(),
        &completing_chunk
            .signature_preimage(&service.context, &manifest)
            .expect("canonical chunk signature preimage"),
    )
    .payload()
    .to_vec();
    let sender = service.context.roster[0].validator.clone();
    let current_failure_chunk = chunk(HashOf::new(&manifest), 1, b"current terminal failure", 0);
    let tail_failure_chunk = chunk(HashOf::new(&manifest), 2, b"tail terminal failure", 0);
    let tail_success_chunk = chunk(HashOf::new(&manifest), 3, b"tail terminal success", 0);
    let expected_bytes = [
        &completing_chunk,
        &current_failure_chunk,
        &tail_failure_chunk,
        &tail_success_chunk,
    ]
    .into_iter()
    .map(|chunk| u64::try_from(chunk.bytes.len()).expect("small orphan chunk"))
    .sum::<u64>();

    let _completing_token = buffer_productive_orphan_for_replay(
        &mut service,
        &ingress,
        sender.clone(),
        completing_chunk,
    );
    let current_failure_token = buffer_productive_orphan_for_replay(
        &mut service,
        &ingress,
        sender.clone(),
        current_failure_chunk,
    );
    let tail_failure_token = buffer_productive_orphan_for_replay(
        &mut service,
        &ingress,
        sender.clone(),
        tail_failure_chunk,
    );
    let tail_success_token =
        buffer_productive_orphan_for_replay(&mut service, &ingress, sender, tail_success_chunk);
    assert_eq!(service.orphan_chunk_count, 4);
    assert_eq!(service.orphan_chunk_bytes, expected_bytes);

    {
        let mut state = ingress.state.lock();
        state
            .leader_wire_lifecycles
            .get_mut(&current_failure_token.slot)
            .expect("current faulted productive orphan remains indexed")
            .status = super::super::FairV2IngressLeaderWireStatus::Terminal;
        assert!(
            state
                .leader_wire_lifecycles
                .remove(&tail_failure_token.slot)
                .is_some(),
            "tail fault injection removes only its in-memory terminal target"
        );
    }

    let mut executor = V2EffectExecutor::with_runtime(
        SaturatedCompletionRuntime::new(0, 8),
        BTreeMap::new(),
        service.context.clone(),
        service.local_peer.clone(),
        service.local_validator,
        EffectQueueConfig::default(),
    )
    .expect("construct productive-orphan effect executor");
    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    executor
        .consume_effects(
            vec![AdapterEffect::FetchBody {
                tag,
                round: manifest.round,
                subject: manifest.subject,
                manifest: Some(manifest.clone()),
                certified_sources: Vec::new(),
                certificate: None,
            }],
            &mut service,
        )
        .expect("open productive-orphan fetch session");

    let current_error = "leader-wire volatile terminal changed runtime ownership";
    let tail_error = "leader-wire volatile terminal has no runtime record";
    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect_err("current session-changed terminal transfer must fail"),
        format!(
            "{current_error}; additionally failed to retire buffered payload tail: {tail_error}"
        )
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(matches!(
        service.local_completions.front(),
        Some(LocalCompletion::Reconstructed { body, .. })
            if body.as_ref() == canonical_wire.as_slice()
    ));
    let state = ingress.state.lock();
    assert_eq!(
        state
            .leader_wire_lifecycles
            .get(&current_failure_token.slot)
            .expect("current faulted owner remains indexed")
            .status,
        super::super::FairV2IngressLeaderWireStatus::Terminal
    );
    assert!(
        !state
            .leader_wire_lifecycles
            .contains_key(&tail_failure_token.slot),
        "the combined error must come from attempting the missing tail target"
    );
    assert_eq!(
        state
            .leader_wire_lifecycles
            .get(&tail_success_token.slot)
            .expect("last tail owner remains indexed")
            .status,
        super::super::FairV2IngressLeaderWireStatus::VolatileTerminal,
        "tail retirement must continue after retaining its first error"
    );
}

#[test]
fn owned_orphan_chunk_replay_preserves_alternate_source_routes_and_cursors() {
    let (mut service, keys) = fixture();
    allow_fixture_block_payload(&mut service.context);
    service.max_orphan_chunks = 4;
    service.max_orphan_chunk_bytes = service.context.da_layout.max_payload_size_bytes;
    let _chunk_root = install_temporary_chunk_root(&mut service);
    let (canonical_wire, payload, proposal) = proposal_body_and_payload(&service.context, &keys);
    let (manifest, chunks) = payload.into_parts();
    assert_eq!(chunks.len(), 1, "fixture body must have one exact chunk");
    let mut payload_chunk = wire::PayloadChunk {
        manifest_hash: HashOf::new(&manifest),
        index: 0,
        bytes: chunks.into_iter().next().expect("one fixture chunk"),
        sender: 0,
        signature: Vec::new(),
    };
    payload_chunk.signature = Signature::new(
        keys[0].private_key(),
        &payload_chunk
            .signature_preimage(&service.context, &manifest)
            .expect("canonical chunk signature preimage"),
    )
    .payload()
    .to_vec();

    let sender = service.context.roster[0].validator.clone();
    let hub_a = PeerId::new(KeyPair::random().public_key().clone());
    let hub_b = PeerId::new(KeyPair::random().public_key().clone());
    let mut route_fixture = NetworkReplyRouteTestFixture::with_source_capacity(hub_a.clone(), 2);
    let route_a = route_fixture.mint_via(sender.clone(), hub_a.clone());
    let route_b = route_fixture.mint_via(sender.clone(), hub_b.clone());
    let message = BlockMessage::V2(wire::ConsensusMessageV2::new(
        wire::ConsensusMessageV2Payload::PayloadChunk(payload_chunk.clone()),
    ));
    let (_, mut ownership_a) =
        fair_ingress_route_owner(message.clone(), sender.clone(), hub_a, route_a.clone());
    let (_, ownership_b) =
        fair_ingress_route_owner(message, sender.clone(), hub_b, route_b.clone());
    assert!(ownership_a.advance_reply_cursors(&route_a, 3, 5));

    let mut executor = V2EffectExecutor::with_runtime(
        SaturatedCompletionRuntime::new(0, 8),
        BTreeMap::new(),
        service.context.clone(),
        service.local_peer.clone(),
        service.local_validator,
        EffectQueueConfig::default(),
    )
    .expect("construct exact-body effect executor");
    assert_eq!(
        service
            .route_payload_chunk(
                &mut executor,
                sender.clone(),
                payload_chunk.clone(),
                ownership_a,
            )
            .expect("buffer owned orphan chunk"),
        PayloadChunkDisposition::Buffered
    );
    assert_eq!(
        service
            .route_payload_chunk(&mut executor, sender, payload_chunk.clone(), ownership_b,)
            .expect("coalesce alternate owned orphan route"),
        PayloadChunkDisposition::Duplicate
    );

    let expected_ownership_projection = {
        let ownership = service
            .orphan_chunks
            .get_mut(&payload_chunk.manifest_hash)
            .and_then(|buffered| buffered.front_mut())
            .and_then(|buffered| buffered.ingress_ownership.as_mut())
            .expect("coalesced orphan retains fair-ingress ownership");
        assert!(ownership.advance_reply_cursors(&route_b, 7, 11));
        assert_eq!(ownership.admission_count, 2);
        let routes = ownership
            .current_reply_routes()
            .expect("both authenticated source routes remain owned");
        assert_eq!(routes.len(), 2);
        assert!(routes.iter().any(|route| route.same_delivery(&route_a)));
        assert!(routes.iter().any(|route| route.same_delivery(&route_b)));
        let source_a = ownership
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&route_a))
            .expect("source A cursor ownership");
        let source_b = ownership
            .attempts
            .iter()
            .find(|attempt| attempt.route.same_source(&route_b))
            .expect("source B cursor ownership");
        assert_eq!((source_a.message_cursor, source_a.chunk_cursor), (3, 5));
        assert_eq!((source_b.message_cursor, source_b.chunk_cursor), (7, 11));
        ownership.process_local_projection_hash()
    };

    let tag = EventTag::new(
        service.context.height,
        proposal.round.view,
        Generation::new(service.context.height),
    );
    assert_eq!(
        executor
            .consume_effects(
                vec![AdapterEffect::FetchBody {
                    tag,
                    round: manifest.round,
                    subject: manifest.subject,
                    manifest: Some(manifest.clone()),
                    certified_sources: Vec::new(),
                    certificate: None,
                }],
                &mut service,
            )
            .expect("open matching live fetch session"),
        1
    );
    assert!(
        service
            .fetch_work_for_manifest(payload_chunk.manifest_hash)
            .is_some()
    );
    let retained = service
        .orphan_chunks
        .get(&payload_chunk.manifest_hash)
        .and_then(|buffered| buffered.front())
        .and_then(|buffered| buffered.ingress_ownership.as_ref())
        .expect("opening the session must not alter orphan ownership");
    assert_eq!(
        retained.process_local_projection_hash(),
        expected_ownership_projection
    );

    assert_eq!(
        service
            .replay_buffered_chunks(&mut executor)
            .expect("replay exact owned orphan chunk"),
        1
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(matches!(
        service.local_completions.front(),
        Some(LocalCompletion::Reconstructed {
            manifest: completed_manifest,
            body,
            ..
        }) if completed_manifest == &manifest && body.as_ref() == canonical_wire.as_slice()
    ));
    assert!(!service.output_guard.restart_required());
}

#[test]
fn orphan_chunk_bounds_preserve_exact_duplicate_semantics_at_capacity() {
    let (mut service, _) = fixture();
    let hash = manifest_hash(b"manifest-a");
    let sender = service.context.roster[0].validator.clone();
    let first = chunk(hash, 0, b"a", 0);

    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), first.clone()),
        PayloadChunkDisposition::Buffered
    );
    assert_eq!(service.orphan_chunk_count, 1);
    assert_eq!(service.orphan_chunk_bytes, 1);
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), first),
        PayloadChunkDisposition::Duplicate,
        "an exact retransmission remains idempotent even when the buffer is full"
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), chunk(hash, 0, b"b", 0)),
        PayloadChunkDisposition::Rejected,
        "a conflicting claim cannot replace retained bytes"
    );
    assert_eq!(
        service
            .buffer_orphan_payload_chunk(sender, chunk(manifest_hash(b"manifest-b"), 0, b"c", 0)),
        PayloadChunkDisposition::Rejected,
        "one unknown manifest cannot force storage beyond the global bound"
    );
    assert_eq!(service.orphan_chunk_count, 1);
    assert_eq!(service.orphan_chunk_bytes, 1);
}

#[test]
fn proofless_orphan_eviction_releases_exact_count_and_byte_capacity() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 2;
    service.max_orphan_chunk_bytes = 2;
    let sender = service.context.roster[0].validator.clone();
    let first_hash = manifest_hash(b"proofless-eviction-a");
    let second_hash = manifest_hash(b"proofless-eviction-b");
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender.clone(), chunk(first_hash, 0, b"a", 0),),
        PayloadChunkDisposition::Buffered
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(sender, chunk(second_hash, 0, b"b", 0)),
        PayloadChunkDisposition::Buffered
    );

    assert!(service.evict_one_proofless_orphan_chunk());
    assert_eq!(service.orphan_chunk_count, 1);
    assert_eq!(service.orphan_chunk_bytes, 1);
    assert_eq!(
        service
            .orphan_chunks
            .values()
            .map(VecDeque::len)
            .sum::<usize>(),
        1
    );
    assert!(service.evict_one_proofless_orphan_chunk());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
    assert!(service.orphan_chunks.is_empty());
    assert!(!service.evict_one_proofless_orphan_chunk());
}

#[test]
fn authenticated_orphan_flood_stays_inside_frozen_count_and_byte_geometry() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 4;
    service.max_orphan_chunk_bytes = 4;

    for sender_index in 0..4_u32 {
        let sender_position = usize::try_from(sender_index).expect("test sender index fits usize");
        let sender = service.context.roster[sender_position].validator.clone();
        assert_eq!(
            service.buffer_orphan_payload_chunk(
                sender,
                chunk(
                    manifest_hash(&[0xA0, u8::try_from(sender_index).expect("small index")]),
                    0,
                    &[u8::try_from(sender_index).expect("small index")],
                    sender_index,
                ),
            ),
            PayloadChunkDisposition::Buffered,
            "each authenticated roster source can consume only the shared finite orphan budget"
        );
    }
    assert_eq!(service.orphan_chunk_count, 4);
    assert_eq!(service.orphan_chunk_bytes, 4);

    let attacker = service.context.roster[0].validator.clone();
    let retained = chunk(manifest_hash(&[0xA0, 0]), 0, &[0], 0);
    assert_eq!(
        service.buffer_orphan_payload_chunk(attacker.clone(), retained),
        PayloadChunkDisposition::Duplicate,
        "the exact retained identity still coalesces at the capacity boundary"
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(
            attacker,
            chunk(manifest_hash(b"fifth authenticated orphan"), 1, &[0xFF], 0),
        ),
        PayloadChunkDisposition::Rejected,
        "authenticated junk cannot replenish beyond the frozen global owner universe"
    );
    assert_eq!(service.orphan_chunk_count, 4);
    assert_eq!(service.orphan_chunk_bytes, 4);
}

#[test]
fn orphan_chunk_cheap_checks_reject_spoofing_and_oversize_without_allocation() {
    let (mut service, _) = fixture();
    service.max_orphan_chunks = 8;
    let hash = manifest_hash(b"manifest-cheap-checks");
    let validator_zero = service.context.roster[0].validator.clone();
    let validator_one = service.context.roster[1].validator.clone();

    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_one, chunk(hash, 0, b"a", 0)),
        PayloadChunkDisposition::Rejected,
        "outer transport identity must match the claimed validator index"
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_zero.clone(), chunk(hash, 4, b"a", 0)),
        PayloadChunkDisposition::Rejected
    );
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_zero.clone(), chunk(hash, 0, &[], 0)),
        PayloadChunkDisposition::Rejected
    );
    assert_eq!(
        service
            .buffer_orphan_payload_chunk(validator_zero.clone(), chunk(hash, 0, b"123456789", 0)),
        PayloadChunkDisposition::Rejected
    );
    service.max_orphan_chunk_bytes = 1;
    assert_eq!(
        service.buffer_orphan_payload_chunk(validator_zero, chunk(hash, 0, b"ab", 0)),
        PayloadChunkDisposition::Rejected
    );
    assert!(service.orphan_chunks.is_empty());
    assert_eq!(service.orphan_chunk_count, 0);
    assert_eq!(service.orphan_chunk_bytes, 0);
}
