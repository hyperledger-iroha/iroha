fn terminal_capacity_with_allowed_view_temp(kura: &Kura, temp_path: &Path) -> u64 {
    let _prune_guard = kura.prune_lock.lock();
    kura.ensure_prune_recovery_not_required()
        .expect("view capacity fixture has no prune recovery");
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    let (missing, incomplete) = kura
        .autonomous_global_terminal_reservation_counts_with_allowed_view_temp_locked(Some(
            temp_path,
        ))
        .expect("measure terminal reservations with one authenticated view temp");
    let terminal_max =
        u64::try_from(AUTONOMOUS_LIFECYCLE_TERMINAL_OUTCOME_MAX_BYTES).expect("terminal max fits");
    u64::try_from(missing)
        .expect("missing terminal count fits")
        .checked_mul(terminal_max)
        .and_then(|stable| stable.checked_add(if incomplete == 0 { 0 } else { terminal_max }))
        .expect("terminal reservation capacity fits")
}

fn pending_canonical_capacity_bytes(kura: &Kura) -> u64 {
    let _prune_guard = kura.prune_lock.lock();
    kura.ensure_prune_recovery_not_required()
        .expect("capacity fixture has no prune recovery");
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    kura.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
        .expect("measure pending canonical capacity")
}

#[test]
fn autonomous_execution_input_preflights_complete_progress_peak_before_mutation() {
    let temp_dir = TempDir::new().expect("execution-input capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (chain_id_hash, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("execution-input capacity Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
        .expect("persist capacity payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&payload.origin_proposal, chain_id_hash, epoch)
        .expect("recover exact autonomous execution input");
    let artifact = LaneBlockExecutionInputArtifact::new(recovered.clone());
    let payload_bytes = artifact.encode_framed().expect("encode execution input");
    let descriptor = &payload.origin_proposal.descriptor;
    let (data_path, index_path) =
        Kura::lane_block_execution_input_paths_for_entry(lane, temp_dir.path());
    let additional_peak = {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("capacity fixture has no prune recovery");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("measure pending canonical execution-input bytes");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.preflight_lane_block_execution_input_publication_locked(
            pending_canonical_bytes,
            &data_path,
            &index_path,
            descriptor.lane_block_height,
            u64::try_from(payload_bytes.len()).expect("execution input length fits"),
        )
        .expect("derive complete execution-input peak")
        .additional_physical_peak_bytes
    };
    kura.refresh_disk_usage_bytes()
        .expect("refresh execution-input baseline accounting");
    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure execution-input physical baseline");
    let terminal = kura
        .autonomous_global_terminal_outcome_reserved_bytes()
        .expect("measure execution-input terminal reservations");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure execution-input carrier reservations");
    let certified_bundles = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure execution-input certified-bundle reservations");
    let pending_canonical = pending_canonical_capacity_bytes(&kura);
    let prune_headroom = Kura::canonical_prune_intent_maintenance_headroom_bytes();
    let exact_limit = used
        .checked_add(pending_canonical)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified_bundles))
        .and_then(|bytes| bytes.checked_add(prune_headroom))
        .and_then(|bytes| bytes.checked_add(additional_peak))
        .expect("execution-input exact peak fits");
    let directory_before = snapshot_regular_files_recursively(temp_dir.path());
    let accounting_before = kura.disk_usage.load(Ordering::Relaxed);
    let reservations_before = kura.post_wsv_lane_artifact_budget_reservations.lock().clone();
    Arc::get_mut(&mut kura)
        .expect("execution-input capacity Kura is exclusive")
        .max_disk_usage_bytes = exact_limit - 1;

    assert!(
        kura.persist_lane_block_execution_input(&recovered).is_err(),
        "one byte below the complete execution-input peak must reject",
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        directory_before,
        "capacity rejection must leave the complete Kura directory byte-for-byte unchanged",
    );
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        accounting_before,
        "capacity rejection must not change tracked disk accounting",
    );
    assert_eq!(
        *kura.post_wsv_lane_artifact_budget_reservations.lock(),
        reservations_before,
        "capacity rejection must not consume another carrier reservation",
    );

    Arc::get_mut(&mut kura)
        .expect("execution-input capacity Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit;
    kura.persist_lane_block_execution_input(&recovered)
        .expect("the exact complete execution-input peak succeeds");
    assert_eq!(
        kura.read_lane_block_execution_input(lane.lane_id, descriptor.lane_block_height),
        Some(artifact),
    );
    assert!(!Kura::bound_progress_append_build_path(&index_path).exists());
    assert!(!Kura::bound_progress_append_intent_path(&index_path).exists());
    assert!(!index_path.with_extension("index.prepend.tmp").exists());
    {
        let _prune_guard = kura.prune_lock.lock();
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("measure pending canonical bytes for guarded execution-input retry");
        kura.persist_lane_block_execution_input_under_prune_and_canonical_guards(
            &recovered,
            pending_canonical_bytes,
        )
        .expect("guarded execution-input seam is idempotent and non-recursive");
    }
}

#[test]
fn autonomous_view_recovery_preflights_named_and_atomic_temp_peak() {
    let temp_dir = TempDir::new().expect("view recovery capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (chain_id_hash, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("view recovery capacity Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
        .expect("persist view recovery payload");
    let descriptor = &payload.origin_proposal.descriptor;
    let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        lane,
        temp_dir.path(),
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let temp_path = Kura::autonomous_lane_block_view_state_temp_path(&view_path);
    let valid_bytes = fs::read(&view_path).expect("read valid view state");
    fs::write(&temp_path, &valid_bytes).expect("stage valid named view temp");
    fs::write(&view_path, &valid_bytes[..valid_bytes.len() / 2])
        .expect("stage malformed main view state");
    kura.refresh_disk_usage_bytes()
        .expect("refresh named-temp disk accounting");

    let read_only_before = snapshot_regular_files_recursively(temp_dir.path());
    assert!(
        kura.read_autonomous_lane_block_artifact_with_recovery_policy(
            lane.lane_id,
            descriptor.lane_block_height,
            chain_id_hash,
            epoch,
            false,
        )
        .is_none(),
        "read-only view validation must fail on the malformed main",
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        read_only_before,
        "recover=false must not promote or delete a named view temp",
    );

    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure named-temp physical baseline");
    let terminal = terminal_capacity_with_allowed_view_temp(&kura, &temp_path);
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure named-temp carrier reservations");
    let certified_bundles = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure named-temp certified-bundle reservations");
    let pending_canonical = pending_canonical_capacity_bytes(&kura);
    let prune_headroom = Kura::canonical_prune_intent_maintenance_headroom_bytes();
    let replacement_len = u64::try_from(valid_bytes.len()).expect("view state length fits");
    let exact_limit = used
        .checked_add(pending_canonical)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified_bundles))
        .and_then(|bytes| bytes.checked_add(prune_headroom))
        .and_then(|bytes| bytes.checked_add(replacement_len))
        .expect("named plus atomic temp peak fits");
    let directory_before = snapshot_regular_files_recursively(temp_dir.path());
    let accounting_before = kura.disk_usage.load(Ordering::Relaxed);
    Arc::get_mut(&mut kura)
        .expect("view recovery capacity Kura is exclusive")
        .max_disk_usage_bytes = exact_limit - 1;

    assert!(
        kura.read_autonomous_lane_block_artifact_with_recovery_policy(
            lane.lane_id,
            descriptor.lane_block_height,
            chain_id_hash,
            epoch,
            true,
        )
        .is_none(),
        "one byte below the named-temp plus atomic-temp peak must reject recovery",
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        directory_before,
        "recovery capacity rejection must be byte-for-byte non-mutating",
    );
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        accounting_before,
        "recovery capacity rejection must preserve accounting",
    );

    Arc::get_mut(&mut kura)
        .expect("view recovery capacity Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit;
    assert!(
        kura.read_autonomous_lane_block_artifact_with_recovery_policy(
            lane.lane_id,
            descriptor.lane_block_height,
            chain_id_hash,
            epoch,
            true,
        )
        .is_some(),
        "the exact named-temp plus atomic-temp peak succeeds",
    );
    assert_eq!(fs::read(&view_path).expect("read promoted view"), valid_bytes);
    assert!(!temp_path.exists(), "successful recovery removes the named temp");
}

#[test]
fn autonomous_view_writer_preflights_even_with_named_temp() {
    let temp_dir = TempDir::new().expect("view writer capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (chain_id_hash, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let (mut kura, _) = Kura::new(&config, &lane_config).expect("view writer capacity Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
        .expect("persist view writer payload");
    let descriptor = &payload.origin_proposal.descriptor;
    let view_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        lane,
        temp_dir.path(),
        descriptor.lane_block_height,
        descriptor.proposal_height,
    );
    let temp_path = Kura::autonomous_lane_block_view_state_temp_path(&view_path);
    let stable_bytes = fs::read(&view_path).expect("read stable view state");
    fs::write(&temp_path, &stable_bytes).expect("stage redundant named temp");
    kura.refresh_disk_usage_bytes()
        .expect("refresh view writer named-temp accounting");
    let terminal = terminal_capacity_with_allowed_view_temp(&kura, &temp_path);
    let used = kura
        .kura_disk_usage_bytes()
        .expect("measure view writer physical baseline");
    let post_wsv = kura
        .post_wsv_lane_artifact_budget_reserved_bytes()
        .expect("measure view writer carrier reservations");
    let certified_bundles = kura
        .certified_bundle_capacity_reserved_bytes()
        .expect("measure view writer certified-bundle reservations");
    let pending_canonical = pending_canonical_capacity_bytes(&kura);
    let prune_headroom = Kura::canonical_prune_intent_maintenance_headroom_bytes();
    let exact_limit = used
        .checked_add(pending_canonical)
        .and_then(|bytes| bytes.checked_add(terminal))
        .and_then(|bytes| bytes.checked_add(post_wsv))
        .and_then(|bytes| bytes.checked_add(certified_bundles))
        .and_then(|bytes| bytes.checked_add(prune_headroom))
        .expect("equal-sized named-temp replacement peak fits");
    let state = Kura::decode_autonomous_lane_block_view_state(&view_path, &stable_bytes)
        .expect("decode stable view state");
    let directory_before = snapshot_regular_files_recursively(temp_dir.path());
    let accounting_before = kura.disk_usage.load(Ordering::Relaxed);
    Arc::get_mut(&mut kura)
        .expect("view writer capacity Kura is exclusive")
        .max_disk_usage_bytes = exact_limit - 1;

    let rejected = {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("view writer fixture has no prune recovery");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("measure pending canonical view-writer bytes");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.write_autonomous_lane_block_view_state_record_locked(
            pending_canonical_bytes,
            &payload,
            &state,
            &view_path,
            chain_id_hash,
            epoch,
        )
    };
    assert!(rejected.is_err());
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        directory_before,
        "writer capacity rejection must preserve main and named temp bytes",
    );
    assert_eq!(
        kura.disk_usage.load(Ordering::Relaxed),
        accounting_before,
        "writer capacity rejection must preserve accounting",
    );

    Arc::get_mut(&mut kura)
        .expect("view writer capacity Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit;
    {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("view writer fixture remains recoverable");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("remeasure pending canonical view-writer bytes");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.write_autonomous_lane_block_view_state_record_locked(
            pending_canonical_bytes,
            &payload,
            &state,
            &view_path,
            chain_id_hash,
            epoch,
        )
        .expect("exact equal-sized named-temp replacement peak succeeds");
    }
    assert_eq!(fs::read(&view_path).expect("read rewritten view"), stable_bytes);
    assert!(!temp_path.exists());
}

#[test]
fn autonomous_view_recovery_corridor_acquires_prune_before_geometry() {
    let temp_dir = TempDir::new().expect("view lock-order temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let lane = lane_config.entry(LaneId::new(1)).expect("lane one");
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (chain_id_hash, epoch, payload) =
        autonomous_lane_payload_for_kura(lane.lane_id, lane.dataspace_id, 1, &signer);
    let (kura, _) = Kura::new(&config, &lane_config).expect("view lock-order Kura");
    install_autonomous_lane_marker_for_kura(&kura, &lane_config, &payload);
    kura.persist_lane_executable_payload(&payload, chain_id_hash, epoch)
        .expect("persist lock-order payload");
    let descriptor = &payload.origin_proposal.descriptor;
    let lane_id = lane.lane_id;
    let lane_block_height = descriptor.lane_block_height;

    let prune_guard = kura.prune_lock.lock();
    assert!(
        kura.read_autonomous_lane_block_artifact_with_recovery_policy(
            lane_id,
            lane_block_height,
            chain_id_hash,
            epoch,
            false,
        )
        .is_some(),
        "recover=false must not recursively acquire prune_lock",
    );
    let worker_kura = Arc::clone(&kura);
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let result = worker_kura.read_autonomous_lane_block_artifact_with_recovery_policy(
            lane_id,
            lane_block_height,
            chain_id_hash,
            epoch,
            true,
        );
        done_tx.send(result.is_some()).expect("report recovery read");
    });
    assert!(
        done_rx.recv_timeout(Duration::from_millis(50)).is_err(),
        "recover=true must wait at the outer prune fence",
    );
    drop(prune_guard);
    assert!(
        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("recovery read completes after prune fence"),
    );
    worker.join().expect("join recovery reader");
}
