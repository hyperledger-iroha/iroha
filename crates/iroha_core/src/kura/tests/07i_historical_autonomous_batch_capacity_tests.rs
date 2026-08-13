fn historical_capacity_payload_for_kura(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_block_height: u64,
    tag: &str,
    signer: &KeyPair,
) -> (NetworkId, u64, LaneExecutablePayloadV1) {
    let (network_id, epoch, source) =
        autonomous_lane_payload_for_kura(lane_id, dataspace_id, lane_block_height, signer);
    let transaction = TransactionBuilder::new(
        test_network_id(b"kura-autonomous-view-checkpoint"),
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        format!("historical recovery capacity {tag}"),
    )])
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let entrypoint = TransactionEntrypoint::External(transaction);
    let entrypoint_hash = Hash::from(entrypoint.hash());
    let mut proposal = source.origin_proposal.clone();
    proposal.descriptor.accepted_candidate_indices = vec![0];
    proposal.descriptor.accepted_transaction_hashes = vec![entrypoint_hash];
    proposal.descriptor.subject_hash = Hash::new_from_chunks(&[
        b"iroha:kura:test:historical-capacity-subject:v1\0",
        tag.as_bytes(),
    ]);
    proposal.descriptor.payload_ownership_hash = Hash::new_from_chunks(&[
        b"iroha:kura:test:historical-capacity-ownership:v1\0",
        tag.as_bytes(),
    ]);
    proposal.descriptor.rbc_instance_hash = Hash::new_from_chunks(&[
        b"iroha:kura:test:historical-capacity-rbc:v1\0",
        tag.as_bytes(),
    ]);
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let routing_plan =
        RoutingPlan::single(crate::queue::RoutingDecision::new(lane_id, dataspace_id));
    let accepted = AcceptedTransaction::new_unchecked_entrypoint(Cow::Owned(entrypoint.clone()));
    let reservation = LaneQueueReservationKeyV2 {
        version: LaneQueueReservationKeyV2::VERSION,
        signed_transaction_hash: accepted.hash(),
        entrypoint_hash: entrypoint.hash(),
        queue_plan_admission_binding_hash: Hash::new_from_chunks(&[
            b"iroha:kura:test:historical-capacity-admission:v1\0",
            tag.as_bytes(),
        ]),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id,
        dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new_from_chunks(&[
            b"iroha:kura:test:historical-capacity-owner:v1\0",
            tag.as_bytes(),
        ]),
        proposal_identity_hash: proposal.proposal_hash,
    };
    let payload = LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal,
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        PeerId::new(signer.public_key().clone()),
        signer.private_key(),
    )
    .expect("historical capacity payload");
    (network_id, epoch, payload)
}
fn persist_historical_capacity_payload_fixture(kura: &Kura, payload: &LaneExecutablePayloadV1) {
    kura.persist_lane_executable_payload(payload, payload.network_id, payload.epoch)
        .expect("persist historical capacity payload dependency");
}
fn historical_capacity_required_limit(kura: &Kura, additional_peak: u64) -> u64 {
    kura.refresh_disk_usage_bytes()
        .expect("refresh historical capacity baseline accounting");
    let pending_canonical = {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("historical capacity fixture has no prune recovery");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        kura.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("measure historical pending canonical capacity")
    };
    kura.kura_disk_usage_bytes()
        .expect("measure historical capacity physical baseline")
        .checked_add(pending_canonical)
        .and_then(|bytes| {
            bytes.checked_add(
                kura.autonomous_global_terminal_outcome_reserved_bytes()
                    .expect("measure historical capacity terminal reservations"),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                kura.post_wsv_lane_artifact_budget_reserved_bytes()
                    .expect("measure historical capacity carrier reservations"),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                kura.certified_bundle_capacity_reserved_bytes()
                    .expect("measure historical capacity certified-bundle reservations"),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .and_then(|bytes| bytes.checked_add(additional_peak))
        .expect("historical capacity exact limit fits")
}
#[test]
#[allow(clippy::too_many_lines)]
fn historical_recovery_batch_capacity_is_exact_duplicate_aware_and_atomic_on_rejection() {
    let temp_dir = TempDir::new().expect("historical capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane_one = lane_config.entry(LaneId::new(1)).expect("lane one");
    let lane_zero = lane_config.entry(LaneId::SINGLE).expect("lane zero");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, lane_one_height_two) = historical_capacity_payload_for_kura(
        lane_one.lane_id,
        lane_one.dataspace_id,
        2,
        "same-route-height-two",
        &signer,
    );
    let (_, _, lane_one_height_one) = historical_capacity_payload_for_kura(
        lane_one.lane_id,
        lane_one.dataspace_id,
        1,
        "same-route-height-one",
        &signer,
    );
    let (_, _, lane_zero_height_one) = historical_capacity_payload_for_kura(
        lane_zero.lane_id,
        lane_zero.dataspace_id,
        1,
        "different-route-height-one",
        &signer,
    );
    let record_two = historical_autonomous_recovery_record_for_kura(
        &lane_one_height_two,
        &signer,
        b"capacity-same-route-height-two",
    );
    let record_one = historical_autonomous_recovery_record_for_kura(
        &lane_one_height_one,
        &signer,
        b"capacity-same-route-height-one",
    );
    let record_other = historical_autonomous_recovery_record_for_kura(
        &lane_zero_height_one,
        &signer,
        b"capacity-different-route-height-one",
    );
    let records = vec![
        record_two.clone(),
        record_two.clone(),
        record_one.clone(),
        record_other.clone(),
    ];
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("historical capacity Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &lane_one_height_one);
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &lane_zero_height_one);
    for payload in [
        &lane_one_height_one,
        &lane_one_height_two,
        &lane_zero_height_one,
    ] {
        persist_historical_capacity_payload_fixture(&kura, payload);
    }
    let additional_peak = kura
        .historical_autonomous_recovery_batch_additional_peak_for_test(&records)
        .expect("derive whole historical batch peak");
    let exact_limit = historical_capacity_required_limit(&kura, additional_peak);
    let bytes_before = snapshot_regular_files_recursively(temp_dir.path());
    let accounting_before = kura
        .disk_usage_accounting_snapshot_for_tests()
        .expect("snapshot historical capacity accounting");
    let revision_before = kura.committed_lane_status_revision();
    let post_wsv_before = kura
        .post_wsv_lane_artifact_budget_reservations
        .lock()
        .clone();
    let certified_before = kura.certified_bundle_capacity_reservations.lock().clone();
    Arc::get_mut(&mut kura)
        .expect("historical capacity Kura is exclusive")
        .max_disk_usage_bytes = exact_limit - 1;
    assert!(
        kura.persist_historical_autonomous_lane_recovery_records(&records)
            .is_err(),
        "one byte below the whole-batch peak must reject before its first mutation",
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        bytes_before
    );
    assert_eq!(
        kura.disk_usage_accounting_snapshot_for_tests()
            .expect("re-read rejected historical capacity accounting"),
        accounting_before,
    );
    assert_eq!(kura.committed_lane_status_revision(), revision_before);
    assert_eq!(
        *kura.post_wsv_lane_artifact_budget_reservations.lock(),
        post_wsv_before,
    );
    assert_eq!(
        *kura.certified_bundle_capacity_reservations.lock(),
        certified_before,
    );
    Arc::get_mut(&mut kura)
        .expect("historical capacity Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit;
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_records(&records)
            .expect("exact whole-batch peak succeeds"),
        vec![
            HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
            HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled,
            HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
            HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
        ],
    );
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_records(&records)
            .expect("complete duplicate batch is idempotent"),
        vec![HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled; records.len()],
    );
}
#[test]
fn historical_recovery_partial_batch_restart_completes_remaining_records() {
    let temp_dir = TempDir::new().expect("historical restart temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, first_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        2,
        "restart-first",
        &signer,
    );
    let (_, _, second_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "restart-second",
        &signer,
    );
    let first =
        historical_autonomous_recovery_record_for_kura(&first_payload, &signer, b"restart-first");
    let second =
        historical_autonomous_recovery_record_for_kura(&second_payload, &signer, b"restart-second");
    let (kura, _) = Kura::new(&config, &lane_config).expect("historical restart Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first_payload);
    persist_historical_capacity_payload_fixture(&kura, &second_payload);
    persist_historical_capacity_payload_fixture(&kura, &first_payload);
    let full_peak = kura
        .historical_autonomous_recovery_batch_additional_peak_for_test(&[
            first.clone(),
            second.clone(),
        ])
        .expect("derive complete pre-crash batch peak");
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&first))
            .expect("persist first crash-boundary record"),
        vec![HistoricalAutonomousLaneRecoveryPersistOutcome::Installed],
    );
    drop(kura);
    let (reopened, _) = Kura::new(&config, &lane_config).expect("reopen partial historical batch");
    let remaining_peak = reopened
        .historical_autonomous_recovery_batch_additional_peak_for_test(&[
            first.clone(),
            second.clone(),
        ])
        .expect("derive remaining restarted batch peak");
    let second_only_peak = reopened
        .historical_autonomous_recovery_batch_additional_peak_for_test(std::slice::from_ref(
            &second,
        ))
        .expect("derive second-record-only peak");
    assert_eq!(
        remaining_peak, second_only_peak,
        "restarted admission must reserve only dependencies and seal bytes still missing",
    );
    assert!(
        remaining_peak < full_peak,
        "durable first-record input and seal must reduce restarted admission",
    );
    assert_eq!(
        reopened
            .persist_historical_autonomous_lane_recovery_records(&[first.clone(), second.clone(),])
            .expect("complete partial historical batch after restart"),
        vec![
            HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled,
            HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
        ],
    );
    assert!(
        reopened
            .historical_autonomous_lane_recovery_record_matches(&second)
            .expect("revalidate restarted historical record"),
    );
}
#[test]
fn historical_recovery_append_crash_is_repaired_only_by_startup_before_replay() {
    let temp_dir = TempDir::new().expect("historical append-crash temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, first_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "append-crash-first",
        &signer,
    );
    let (_, _, second_payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        2,
        "append-crash-second",
        &signer,
    );
    let first = historical_autonomous_recovery_record_for_kura(
        &first_payload,
        &signer,
        b"append-crash-first",
    );
    let second = historical_autonomous_recovery_record_for_kura(
        &second_payload,
        &signer,
        b"append-crash-second",
    );
    let (kura, _) = Kura::new(&config, &lane_config).expect("historical append-crash Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &first_payload);
    persist_historical_capacity_payload_fixture(&kura, &first_payload);
    persist_historical_capacity_payload_fixture(&kura, &second_payload);
    assert_eq!(
        kura.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&first))
            .expect("persist append-crash prefix record"),
        vec![HistoricalAutonomousLaneRecoveryPersistOutcome::Installed],
    );
    fail_next_bound_progress_append_data_sync_for_tests();
    assert!(
        kura.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&second))
            .is_err(),
        "the injected append crash must stop before the second recovery seal",
    );
    let (_, input_index_path) =
        Kura::lane_block_execution_input_paths_for_entry(lane, temp_dir.path());
    let append_intent_path = Kura::bound_progress_append_intent_path(&input_index_path);
    assert!(
        append_intent_path.exists(),
        "the crash boundary must retain its durable append intent",
    );
    let crashed_bytes = snapshot_regular_files_recursively(temp_dir.path());
    assert!(
        kura.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&second))
            .is_err(),
        "live replay must not repair append residue ahead of whole-batch admission",
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        crashed_bytes,
        "the restart-required live retry must be byte-immutable",
    );
    drop(kura);
    let (reopened, _) = Kura::new(&config, &lane_config)
        .expect("startup repairs the historical execution-input append");
    assert!(
        !append_intent_path.exists(),
        "startup must retire the recovered execution-input append intent",
    );
    assert_eq!(
        reopened
            .persist_historical_autonomous_lane_recovery_records(&[first.clone(), second.clone(),])
            .expect("historical replay resumes after startup append recovery"),
        vec![
            HistoricalAutonomousLaneRecoveryPersistOutcome::AlreadyInstalled,
            HistoricalAutonomousLaneRecoveryPersistOutcome::Installed,
        ],
    );
}
#[test]
fn historical_recovery_seal_temp_uses_reserved_bytes_and_residue_fails_closed() {
    let temp_dir = TempDir::new().expect("historical seal-temp temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "seal-temp",
        &signer,
    );
    let record = historical_autonomous_recovery_record_for_kura(&payload, &signer, b"seal-temp");
    let seal_bytes = historical_autonomous_recovery_record_bytes(&record);
    let seal_len = u64::try_from(seal_bytes.len()).expect("historical seal length fits u64");
    let (kura, _) = Kura::new(&config, &lane_config).expect("historical seal-temp Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    persist_historical_capacity_payload_fixture(&kura, &payload);
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, network_id, epoch)
        .expect("recover seal-temp execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist seal-temp execution input dependency");
    assert_eq!(
        kura.historical_autonomous_recovery_batch_additional_peak_for_test(std::slice::from_ref(
            &record
        ),)
            .expect("derive seal-only physical peak"),
        seal_len,
        "the exact publication encoding is the one inode reserved for the no-clobber seal",
    );
    kura.fail_next_atomic_write_after_temporary_sync_for_test();
    assert!(
        kura.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&record))
            .is_err(),
        "the injected crash must retain the synced seal temp before rename",
    );
    let historical_directory =
        Kura::historical_autonomous_recovery_directory_for_entry(lane, temp_dir.path());
    let stable_seal = Kura::historical_autonomous_recovery_path_for_entry(
        lane,
        temp_dir.path(),
        record.recovery_id,
    );
    assert!(
        !stable_seal.exists(),
        "the injected crash must precede stable no-clobber publication",
    );
    let residue = std::fs::read_dir(&historical_directory)
        .expect("read historical seal-temp directory")
        .map(|entry| entry.expect("read historical seal-temp entry").path())
        .find(|path| {
            path.file_name()
                .and_then(std::ffi::OsStr::to_str)
                .is_some_and(|name| {
                    name.starts_with(HISTORICAL_AUTONOMOUS_RECOVERY_ATOMIC_TEMP_PREFIX)
                })
        })
        .expect("synced no-clobber residue remains visible");
    assert_eq!(
        std::fs::symlink_metadata(&residue)
            .expect("read historical seal-temp residue metadata")
            .len(),
        seal_len,
        "the temporary inode carries exactly the admitted stable seal bytes",
    );
    assert!(
        kura.kura_disk_usage_bytes().is_err(),
        "an orphan generic seal temp must make physical accounting fail closed",
    );
    let crashed_bytes = snapshot_regular_files_recursively(temp_dir.path());
    assert!(
        kura.persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&record))
            .is_err(),
        "historical replay must reject an unclassified generic seal temp",
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        crashed_bytes,
        "rejection of generic seal residue must not mutate durable bytes",
    );
}
#[test]
fn historical_recovery_acquires_prune_before_historical_mutation_lock() {
    let temp_dir = TempDir::new().expect("historical lock-order temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (_, _, payload) = historical_capacity_payload_for_kura(
        lane.lane_id,
        lane.dataspace_id,
        1,
        "lock-order",
        &signer,
    );
    let record = historical_autonomous_recovery_record_for_kura(&payload, &signer, b"lock-order");
    let (kura, _) = Kura::new(&config, &lane_config).expect("historical lock-order Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    persist_historical_capacity_payload_fixture(&kura, &payload);
    let historical_guard = kura.historical_autonomous_recovery_mutation_lock.lock();
    let worker_kura = Arc::clone(&kura);
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(0);
    let (finished_tx, finished_rx) = std::sync::mpsc::sync_channel(0);
    let worker = thread::spawn(move || {
        started_tx.send(()).expect("announce historical worker");
        let result = worker_kura
            .persist_historical_autonomous_lane_recovery_records(std::slice::from_ref(&record));
        finished_tx.send(result).expect("report historical worker");
    });
    started_rx.recv().expect("historical worker started");
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        let Some(prune_guard) = kura.prune_lock.try_lock() else {
            break;
        };
        drop(prune_guard);
        assert!(
            Instant::now() < deadline,
            "historical worker blocked on the inner lock without acquiring prune first",
        );
        thread::yield_now();
    }
    assert!(
        finished_rx.recv_timeout(Duration::from_millis(50)).is_err(),
        "historical worker must remain behind the held mutation lock",
    );
    drop(historical_guard);
    finished_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("historical worker finishes after lock release")
        .expect("historical worker persistence succeeds");
    worker.join().expect("historical worker joins");
}
