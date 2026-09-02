#[test]
fn abnormal_service_drop_shuts_worker_down_before_blocking_final_drain() {
    let (mut service, _) = fixture();
    service.clean_teardown = false;
    let output_guard = Arc::clone(&service.output_guard);
    let permit_guard = Arc::clone(&output_guard);
    let (permit_ready_tx, permit_ready_rx) = mpsc::sync_channel(1);
    let (release_permit_tx, release_permit_rx) = mpsc::sync_channel(1);
    let permit_holder = thread::spawn(move || {
        let admitted_output = permit_guard.acquire().expect("admit earlier output");
        permit_ready_tx.send(()).expect("publish admitted output");
        release_permit_rx
            .recv()
            .expect("release admitted output after worker shutdown");
        drop(admitted_output);
    });
    permit_ready_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("earlier output must be admitted before abnormal teardown");
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let (shutdown_seen_tx, shutdown_seen_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        assert!(matches!(command_rx.recv(), Ok(V2IoCommand::Shutdown)));
        shutdown_seen_tx.send(()).expect("publish worker shutdown");
        release_permit_tx
            .send(())
            .expect("release output after worker shutdown");
        drop(completion_tx);
    });
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: Some(worker),
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    drop(service);
    shutdown_seen_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("abnormal teardown must stop the worker before draining admitted output");
    permit_holder.join().expect("join admitted-output holder");
    assert_output_guard_closed(&output_guard);
}
#[test]
fn recovery_gate_is_cross_thread_and_precedes_fatal_completion() {
    let gate = ConsensusOutputGuard::isolated();
    let admitted_output = gate.acquire().expect("initial output permit");
    let worker_gate = Arc::clone(&gate);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    let later_candidate_published = Arc::new(AtomicBool::new(false));
    let worker_candidate_published = Arc::clone(&later_candidate_published);
    let worker = thread::spawn(move || {
        let fatal_operation = worker_gate
            .begin_fail_stop_operation()
            .expect("fatal worker output operation");
        drop(fatal_operation);
        let _ = completion_tx.try_send(V2IoCompletion::RecoveryRequired(
            "committed marker requires restart".to_owned(),
        ));
        assert!(worker_gate.restart_required());
        if worker_gate.acquire().is_some() {
            worker_candidate_published.store(true, Ordering::Release);
        }
    });
    let completion = completion_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("fatal completion must follow recovery admission closure");
    assert!(matches!(
        completion,
        V2IoCompletion::RecoveryRequired(reason)
            if reason == "committed marker requires restart"
    ));
    assert!(
        gate.restart_required(),
        "the guard must close before publishing the fatal completion"
    );
    assert!(
        gate.acquire().is_none(),
        "a second output must not enter while fatal recovery activation drains"
    );
    drop(admitted_output);
    worker.join().expect("join recovery worker");
    assert!(gate.restart_required());
    assert!(gate.acquire().is_none());
    assert!(
        !later_candidate_published.load(Ordering::Acquire),
        "no candidate may be published after the fatal durability transition"
    );
}
#[test]
fn io_command_panic_latches_restart_required_before_unwinding() {
    let output_guard = ConsensusOutputGuard::isolated();
    let unwind = std::panic::catch_unwind({
        let output_guard = Arc::clone(&output_guard);
        move || {
            let _ = execute_fail_stop_io_command(&output_guard, || {
                panic!("model I/O command panic");
            });
        }
    });
    assert!(unwind.is_err());
    assert_output_guard_closed(&output_guard);
}
#[test]
fn retire_panic_closes_gate_before_inflight_output_drains() {
    let output_guard = ConsensusOutputGuard::isolated();
    let admitted_output = output_guard.acquire().expect("admit earlier output");
    let worker_guard = Arc::clone(&output_guard);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let unwind = std::panic::catch_unwind(move || {
            let _ = execute_retire_io_command(&worker_guard, || {
                entered_tx.send(()).expect("publish Retire entry");
                panic!("model Retire panic");
            });
        });
        assert!(unwind.is_err());
    });
    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("Retire operation entered");
    let activation_deadline = Instant::now() + Duration::from_secs(1);
    while !output_guard.restart_required() && Instant::now() < activation_deadline {
        thread::yield_now();
    }
    assert!(
        output_guard.restart_required(),
        "Retire panic must close admission while earlier output still drains"
    );
    assert!(
        output_guard.acquire().is_none(),
        "no later output may cross the gate after the Retire panic"
    );
    drop(admitted_output);
    worker.join().expect("join panicking Retire model");
    assert!(output_guard.acquire().is_none());
}
#[test]
fn retire_failure_is_nonfatal_and_leaves_output_guard_open() {
    let output_guard = ConsensusOutputGuard::isolated();
    let mut worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), Arc::new(AtomicBool::new(false)));
    let completion = execute_retire_io_command(&output_guard, || {
        Err("injected post-finality retirement failure".to_owned())
    })
    .expect("open guard admits Retire");
    assert!(matches!(
        completion,
        V2IoCompletion::RetirementFailed(reason)
            if reason == "injected post-finality retirement failure"
    ));
    worker_failure_guard.disarm();
    drop(worker_failure_guard);
    assert_output_guard_open(&output_guard);
}
#[test]
fn io_worker_lifetime_guard_latches_panic_after_success_before_completion_delivery() {
    let output_guard = ConsensusOutputGuard::isolated();
    let unwind = std::panic::catch_unwind({
        let output_guard = Arc::clone(&output_guard);
        move || {
            let _worker_failure_guard = V2IoWorkerFailureGuard::new(
                Arc::clone(&output_guard),
                Arc::new(AtomicBool::new(false)),
            );
            let completion =
                execute_fail_stop_io_command(&output_guard, || Ok(V2IoCompletion::AuxiliaryNoop))
                    .expect("model successful I/O operation");
            assert!(matches!(completion, V2IoCompletion::AuxiliaryNoop));
            panic!("model panic before completion delivery");
        }
    });
    assert!(unwind.is_err());
    assert_output_guard_closed(&output_guard);
}
#[test]
fn io_worker_explicit_shutdown_leaves_output_guard_open() {
    let output_guard = ConsensusOutputGuard::isolated();
    let mut worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), Arc::new(AtomicBool::new(false)));
    worker_failure_guard.disarm();
    drop(worker_failure_guard);
    assert_output_guard_open(&output_guard);
}
#[test]
fn flagged_finalized_disconnect_leaves_output_guard_open() {
    let output_guard = ConsensusOutputGuard::isolated();
    let allow_finalized_disconnect = Arc::new(AtomicBool::new(false));
    allow_finalized_disconnect.store(true, AtomicOrdering::Release);
    let worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), allow_finalized_disconnect);
    drop(worker_failure_guard);
    assert_output_guard_open(&output_guard);
}
#[test]
fn flagged_worker_panic_closes_gate_before_inflight_output_drains() {
    let output_guard = ConsensusOutputGuard::isolated();
    let admitted_output = output_guard.acquire().expect("admit earlier output");
    let allow_finalized_disconnect = Arc::new(AtomicBool::new(true));
    let worker_output_guard = Arc::clone(&output_guard);
    let (entered_tx, entered_rx) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let _worker_failure_guard =
            V2IoWorkerFailureGuard::new(worker_output_guard, allow_finalized_disconnect);
        entered_tx.send(()).expect("publish worker entry");
        panic!("model flagged finalized-cleanup worker panic");
    });
    entered_rx
        .recv_timeout(Duration::from_secs(1))
        .expect("flagged worker entered");
    let activation_deadline = Instant::now() + Duration::from_secs(1);
    while !output_guard.restart_required() && Instant::now() < activation_deadline {
        thread::yield_now();
    }
    assert!(output_guard.restart_required());
    assert!(
        output_guard.acquire().is_none(),
        "the finalized-disconnect flag must never suppress panic closure"
    );
    drop(admitted_output);
    assert!(worker.join().is_err());
    assert!(output_guard.acquire().is_none());
}
#[test]
fn flagged_worker_fail_stop_error_still_latches_restart_required() {
    let output_guard = ConsensusOutputGuard::isolated();
    let allow_finalized_disconnect = Arc::new(AtomicBool::new(true));
    let worker_failure_guard =
        V2IoWorkerFailureGuard::new(Arc::clone(&output_guard), allow_finalized_disconnect);
    assert!(
        execute_fail_stop_io_command(&output_guard, || {
            Err("injected fail-stop I/O error".to_owned())
        })
        .is_err()
    );
    drop(worker_failure_guard);
    assert_output_guard_closed(&output_guard);
}
#[test]
fn recovery_gate_rejects_service_outputs_and_candidate_delivery() {
    let (mut service, _) = fixture();
    let (command_tx, command_rx, admission) = test_io_command_channel(1);
    let (completion_tx, completion_rx) = mpsc::sync_channel(1);
    service.io = Some(V2IoHandle {
        command_tx,
        completion_rx,
        join: None,
        allow_finalized_disconnect: Arc::new(AtomicBool::new(false)),
        admission,
    });
    let encoded = encode_payload(
        &service.context,
        wire::ConsensusRound {
            context_id: service.context.id(),
            height: service.context.height,
            view: 0,
        },
        wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked block")),
            payload_hash: Hash::new(b"blocked body"),
        },
        b"blocked body",
    )
    .expect("encode bounded payload");
    service
        .prepared_candidates
        .push_back(PreparedCandidateBody {
            tag: EventTag::new(1, 0, Generation::new(1)),
            subject: wire::BlockSubject {
                parent_block_hash: None,
                block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked candidate")),
                payload_hash: Hash::new(b"blocked payload"),
            },
        });
    service.output_guard.activate_restart_required();
    assert!(service.take_prepared_candidate().is_none());
    let blocked_subject = wire::BlockSubject {
        parent_block_hash: None,
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"blocked load block")),
        payload_hash: Hash::new(b"blocked load payload"),
    };
    assert!(
        service
            .request_locked_candidate(
                EventTag::new(1, 0, Generation::new(1)),
                locked_candidate_round(&service, 0),
                blocked_subject,
            )
            .is_err()
    );
    assert!(service.locked_candidate_acquisition.is_none());
    assert!(
        command_rx.try_recv().is_err(),
        "post-latch service work must not mutate the ordered I/O queue"
    );
    assert!(
        service
            .register_outbound_payload(service.active_tag, encoded)
            .is_err(),
        "recovery must reject new proposal material before publication"
    );
    assert!(service.output_permit().is_err());
    drop(completion_tx);
}

fn exact_output_backpressure(
    post: Post<NetworkMessage>,
    ticket: Option<NetworkActorAdmissionTicket>,
) -> Result<(), NetworkActorAdmissionError<Post<NetworkMessage>>> {
    Err(NetworkActorAdmissionError::Backpressured {
        message: post,
        ticket,
        rank: 1,
    })
}

fn install_exact_output_backpressure(service: &mut ProductionV2Services) {
    service.set_exact_output_admission_hook(exact_output_backpressure);
}

fn install_counting_exact_output_backpressure(
    service: &mut ProductionV2Services,
) -> Arc<AtomicUsize> {
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_hook = Arc::clone(&attempts);
    service.set_exact_output_admission_hook(move |post, ticket| {
        attempts_for_hook.fetch_add(1, Ordering::Relaxed);
        exact_output_backpressure(post, ticket)
    });
    attempts
}

fn assert_output_guard_closed(output_guard: &ConsensusOutputGuard) {
    assert!(output_guard.restart_required());
    assert!(output_guard.acquire().is_none());
}

fn assert_output_guard_open(output_guard: &ConsensusOutputGuard) {
    assert!(!output_guard.restart_required());
    assert!(output_guard.acquire().is_some());
}

fn assert_rejected_before_actor_admission<T: std::fmt::Debug>(
    service: &mut ProductionV2Services,
    operation: impl FnOnce(&ProductionV2Services) -> Result<T, String>,
    error_expectation: &str,
    error_fragment: &str,
    pending_inspection_expectation: &str,
) {
    let attempts = install_counting_exact_output_backpressure(service);
    let error = operation(service).expect_err(error_expectation);
    assert!(error.contains(error_fragment));
    assert_eq!(attempts.load(Ordering::Relaxed), 0);
    assert!(
        !service
            .has_pending_exact_output()
            .expect(pending_inspection_expectation)
    );
    assert!(service.output_guard.restart_required());
}

fn post_history_response_for_rejection(
    service: &ProductionV2Services,
    peer: PeerId,
    message: wire::ConsensusMessageV2,
    operation_expectation: &str,
) -> Result<(), String> {
    let guard = Arc::clone(&service.output_guard);
    let operation = guard
        .begin_fail_stop_operation()
        .expect(operation_expectation);
    let result =
        service.post_durable_history_response_with_permit(peer, message, operation.permit());
    drop(operation);
    result
}

fn signed_worker_finality_artifact(
    context: &wire::HeightContext,
    validators: &[KeyPair],
    view: u64,
    subject: wire::BlockSubject,
    execution_commitment: wire::ExecutionCommitment,
    expectations: [&str; 3],
) -> wire::finality::V2FinalityArtifact {
    let round = wire::ConsensusRound {
        context_id: context.id(),
        height: context.height,
        view,
    };
    let preimage = wire::Vote {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signer: 0,
        signature: Vec::new(),
    }
    .signature_preimage();
    let signature_shares = validators[..3]
        .iter()
        .map(|key| {
            Signature::new(key.private_key(), &preimage)
                .payload()
                .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs: Vec<_> = signature_shares.iter().map(Vec::as_slice).collect();
    let certificate = wire::QuorumCertificate {
        round,
        proposal_round: round,
        phase: wire::GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
            .expect(expectations[0]),
    };
    let artifact = wire::finality::V2FinalityArtifact::new(
        context.clone(),
        subject,
        certificate,
        validators
            .iter()
            .map(|key| {
                iroha_crypto::bls_normal_pop_prove(key.private_key()).expect(expectations[1])
            })
            .collect(),
    );
    artifact.validate().expect(expectations[2]);
    artifact
}

fn manifest_hash(label: &[u8]) -> HashOf<wire::PayloadManifest> {
    HashOf::from_untyped_unchecked(Hash::new(label))
}
/// Build exact finality and Kura-receipt authority for sibling rollover tests.
pub(in crate::sumeragi) fn durable_finality_fixture(
    service: &ProductionV2Services,
    keys: &[KeyPair],
) -> (KuraV2CommitReceipt, wire::finality::V2FinalityArtifact) {
    let subject = wire::BlockSubject {
        parent_block_hash: service
            .context
            .parent_commit_qc
            .as_ref()
            .map(|parent| parent.subject.block_hash),
        block_hash: HashOf::from_untyped_unchecked(Hash::new(b"finalized worker block")),
        payload_hash: Hash::new(b"finalized worker payload"),
    };
    let execution_commitment = wire::ExecutionCommitment::without_offline_cash_top_ups_or_merge_carrier(
        Hash::new(b"worker parent state"),
        Hash::new(b"worker post state"),
        Hash::new(b"worker ordinary writes"),
        1,
        Hash::new(b"worker executed block wire"),
    );
    let artifact = signed_worker_finality_artifact(
        &service.context,
        keys,
        0,
        subject,
        execution_commitment,
        [
            "aggregate valid worker CommitQC",
            "worker fixture validator PoP",
            "valid worker finality artifact",
        ],
    );
    (KuraV2CommitReceipt::for_test(&artifact), artifact)
}
fn durable_receipt(service: &ProductionV2Services, keys: &[KeyPair]) -> KuraV2CommitReceipt {
    durable_finality_fixture(service, keys).0
}
fn seal_empty_exact_output_for_cleanup_test(service: &ProductionV2Services) {
    let pending = service
        .pending_exact_output
        .lock()
        .expect("cleanup fixture exact-output corridor");
    assert!(
        !pending.is_pending(),
        "cleanup fixture must not bypass pending exact output"
    );
    service
        .exact_output_handoff_owner
        .seal()
        .expect("seal the cleanup fixture's empty exact-output corridor");
}
/// Rebind closed-network production services to an exact durable context.
pub(in crate::sumeragi) fn service_for_history_context(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
) -> ProductionV2Services {
    service_for_history_context_with_local_validator(kura, context, validators, 0)
}
/// Rebind a closed-network service to an explicit paired handoff owner.
pub(in crate::sumeragi) fn service_for_history_context_with_handoff_owner(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
    exact_output_handoff_owner: DurableExactOutputServiceOwner,
) -> ProductionV2Services {
    let mut service = service_for_history_context(kura, context, validators);
    service.exact_output_handoff_owner = exact_output_handoff_owner;
    service
}
/// Rebind one explicit validator and pair the service with its exact handoff owner.
pub(in crate::sumeragi) fn service_for_history_context_with_local_validator_and_handoff_owner(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
    local_validator: wire::ValidatorIndex,
    exact_output_handoff_owner: DurableExactOutputServiceOwner,
) -> ProductionV2Services {
    let mut service = service_for_history_context_with_local_validator(
        kura,
        context,
        validators,
        local_validator,
    );
    service.exact_output_handoff_owner = exact_output_handoff_owner;
    service
}
/// Rebind closed-network production services to one validator in an exact durable context.
pub(in crate::sumeragi) fn service_for_history_context_with_local_validator(
    kura: Arc<Kura>,
    context: wire::HeightContext,
    validators: &[KeyPair],
    local_validator: wire::ValidatorIndex,
) -> ProductionV2Services {
    let (mut service, _) = fixture();
    context.validate().expect("valid history-fixture successor");
    let local_index = usize::try_from(local_validator)
        .expect("history-fixture validator index fits this platform");
    let local_key = validators
        .get(local_index)
        .expect("history-fixture validator index belongs to its key roster")
        .clone();
    let local_peer = PeerId::new(local_key.public_key().clone());
    assert_eq!(
        context
            .roster
            .get(local_index)
            .map(|entry| &entry.validator),
        Some(&local_peer),
        "history-fixture key roster must match its durable context"
    );
    service.validator_set_pops = validators
        .iter()
        .map(|key| {
            iroha_crypto::bls_normal_pop_prove(key.private_key())
                .expect("history-fixture validator PoP")
        })
        .collect();
    let business_chain_id = service.state.chain_id.clone();
    service.state = Arc::new(State::new_with_chain_and_network_id_for_testing(
        World::default(),
        Arc::clone(&kura),
        LiveQueryStore::start_test(),
        business_chain_id,
        context.network_id,
    ));
    service.context = context;
    service.local_peer = local_peer;
    service.local_validator = Some(local_validator);
    service.key_pair = local_key;
    service.kura = kura;
    service.active_tag = EventTag::new(
        service.context.height,
        0,
        Generation::new(service.context.height),
    );
    service
}
fn successor_service_for_history(
    kura: Arc<Kura>,
    parent: &wire::finality::V2FinalityArtifact,
    validators: &[KeyPair],
) -> ProductionV2Services {
    successor_service_for_history_as(kura, parent, validators, 0)
}
fn successor_service_for_history_as(
    kura: Arc<Kura>,
    parent: &wire::finality::V2FinalityArtifact,
    validators: &[KeyPair],
    local_validator: wire::ValidatorIndex,
) -> ProductionV2Services {
    let mut context = parent.height_context.clone();
    context.height = parent.height.saturating_add(1);
    context.parent_commit_qc = Some(parent.commit_qc.clone());
    service_for_history_context_with_local_validator(kura, context, validators, local_validator)
}
