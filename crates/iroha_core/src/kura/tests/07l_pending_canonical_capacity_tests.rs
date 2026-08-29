fn pending_canonical_capacity_fixture() -> (TempDir, Arc<Kura>) {
    let temp_dir = TempDir::new().expect("pending canonical capacity temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = two_lane_runtime_config();
    let (mut kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("pending capacity Kura");
    Arc::get_mut(&mut kura)
        .expect("pending capacity Kura is exclusive")
        .max_disk_usage_bytes = u64::MAX / 4;
    kura.append_pending_block_for_bench(DummyBlocks::new().next());
    (temp_dir, kura)
}
fn pending_canonical_capacity_snapshot(kura: &Kura) -> u64 {
    let _prune_guard = kura.prune_lock.lock();
    kura.ensure_prune_recovery_not_required()
        .expect("pending capacity fixture has no prune recovery");
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    kura.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
        .expect("measure pending canonical bytes")
}
fn pending_canonical_capacity_stable_required(kura: &Kura, pending: u64) -> u64 {
    kura.refresh_disk_usage_bytes()
        .expect("refresh pending capacity physical accounting");
    kura.kura_disk_usage_bytes()
        .expect("measure pending capacity physical bytes")
        .checked_add(pending)
        .and_then(|bytes| {
            bytes.checked_add(
                kura.autonomous_global_terminal_outcome_reserved_bytes()
                    .expect("measure pending capacity terminal reservations"),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                kura.post_wsv_lane_artifact_budget_reserved_bytes()
                    .expect("measure pending capacity post-WSV reservations"),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(
                kura.certified_bundle_capacity_reserved_bytes()
                    .expect("measure pending capacity certified-bundle reservations"),
            )
        })
        .and_then(|bytes| {
            bytes.checked_add(Kura::canonical_prune_intent_maintenance_headroom_bytes())
        })
        .expect("pending stable capacity fits")
}
#[test]
fn shared_autonomous_mutation_gate_counts_pending_canonical_bytes_exactly() {
    const ADDITIONAL_PEAK: u64 = 37;
    let (temp_dir, mut kura) = pending_canonical_capacity_fixture();
    let pending = pending_canonical_capacity_snapshot(&kura);
    assert!(
        pending > 0,
        "fixture must contain an in-memory canonical block"
    );
    let exact_limit = pending_canonical_capacity_stable_required(&kura, pending)
        .checked_add(ADDITIONAL_PEAK)
        .expect("pending mutation exact limit fits");
    let directory_before = snapshot_regular_files_recursively(temp_dir.path());
    let disk_usage_before = kura.disk_usage.load(Ordering::Relaxed);
    let total_disk_usage_before = kura.disk_usage_total.load(Ordering::Relaxed);
    let reservations_before = kura
        .post_wsv_lane_artifact_budget_reservations
        .lock()
        .clone();
    Arc::get_mut(&mut kura)
        .expect("pending capacity Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit - 1;
    let rejected = {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("pending mutation fixture has no prune recovery");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("remeasure pending canonical bytes before rejection");
        assert_eq!(pending_canonical_bytes, pending);
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            ADDITIONAL_PEAK,
            false,
            false,
            temp_dir.path(),
        )
    };
    assert!(
        rejected.is_err(),
        "one byte under the exact peak must reject"
    );
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        directory_before,
        "capacity rejection must not mutate durable bytes",
    );
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), disk_usage_before);
    assert_eq!(
        kura.disk_usage_total.load(Ordering::Relaxed),
        total_disk_usage_before,
    );
    assert_eq!(
        *kura.post_wsv_lane_artifact_budget_reservations.lock(),
        reservations_before,
    );
    Arc::get_mut(&mut kura)
        .expect("pending capacity Kura remains exclusive at exact limit")
        .max_disk_usage_bytes = exact_limit;
    let accepted = {
        let _prune_guard = kura.prune_lock.lock();
        kura.ensure_prune_recovery_not_required()
            .expect("pending mutation fixture remains recoverable");
        let _canonical_chain_guard = kura.canonical_chain_lock.lock();
        let pending_canonical_bytes = kura
            .pending_canonical_capacity_bytes_under_prune_and_canonical_guards()
            .expect("remeasure pending canonical bytes at exact limit");
        let _geometry_guard = kura.lane_geometry_lock.lock();
        let _sidecar_guard = kura.sidecar_lock.lock();
        kura.validate_configured_autonomous_mutation_disk_peak_locked(
            pending_canonical_bytes,
            ADDITIONAL_PEAK,
            false,
            false,
            temp_dir.path(),
        )
    };
    accepted.expect("the exact peak must be accepted");
}
#[test]
fn startup_capacity_counts_pending_before_geometry_and_rejects_without_mutation() {
    let (temp_dir, mut kura) = pending_canonical_capacity_fixture();
    let pending = pending_canonical_capacity_snapshot(&kura);
    assert!(
        pending > 0,
        "fixture must contain an in-memory canonical block"
    );
    let exact_limit = pending_canonical_capacity_stable_required(&kura, pending);
    let directory_before = snapshot_regular_files_recursively(temp_dir.path());
    let disk_usage_before = kura.disk_usage.load(Ordering::Relaxed);
    let total_disk_usage_before = kura.disk_usage_total.load(Ordering::Relaxed);
    let reservations_before = kura
        .post_wsv_lane_artifact_budget_reservations
        .lock()
        .clone();
    Arc::get_mut(&mut kura)
        .expect("startup capacity Kura remains exclusive")
        .max_disk_usage_bytes = exact_limit - 1;
    assert!(matches!(
        kura.validate_and_publish_configured_kura_capacity_after_startup_recovery(true),
        Err(Error::StorageBudgetExceeded { required, .. }) if required == exact_limit
    ));
    assert_eq!(
        snapshot_regular_files_recursively(temp_dir.path()),
        directory_before,
        "startup rejection must not mutate durable bytes",
    );
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), disk_usage_before);
    assert_eq!(
        kura.disk_usage_total.load(Ordering::Relaxed),
        total_disk_usage_before,
    );
    assert!(!kura.disk_usage_initialized.load(Ordering::Relaxed));
    assert!(!kura.disk_usage_total_initialized.load(Ordering::Relaxed));
    assert!(kura.durable_budget_snapshot().is_none());
    assert_eq!(
        *kura.post_wsv_lane_artifact_budget_reservations.lock(),
        reservations_before,
    );
    Arc::get_mut(&mut kura)
        .expect("startup capacity Kura remains exclusive at exact limit")
        .max_disk_usage_bytes = exact_limit;
    kura.invalidate_pending_budget_cache();
    kura.pending_budget_raw_scans.store(0, Ordering::Relaxed);
    let geometry_guard = kura.lane_geometry_lock.lock();
    let worker_kura = Arc::clone(&kura);
    let (done_tx, done_rx) = std::sync::mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        done_tx
            .send(
                worker_kura
                    .validate_and_publish_configured_kura_capacity_after_startup_recovery(true),
            )
            .expect("report startup capacity result");
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while kura.pending_budget_raw_scans.load(Ordering::Relaxed) == 0 && Instant::now() < deadline {
        thread::yield_now();
    }
    assert_eq!(
        kura.pending_budget_raw_scans.load(Ordering::Relaxed),
        1,
        "pending canonical bytes must be scanned before geometry acquisition",
    );
    assert!(
        done_rx.try_recv().is_err(),
        "startup validation must still be waiting at the geometry lock",
    );
    drop(geometry_guard);
    done_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("startup validation completes after geometry release")
        .expect("the exact startup capacity succeeds");
    worker.join().expect("join startup capacity validator");
    let (exact_enforced, exact_total) = kura
        .kura_disk_usage_bytes_with_total()
        .expect("rescan exact startup usage");
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), exact_enforced);
    assert_eq!(kura.disk_usage_total.load(Ordering::Relaxed), exact_total);
    assert!(kura.disk_usage_initialized.load(Ordering::Relaxed));
    assert!(kura.disk_usage_total_initialized.load(Ordering::Relaxed));
}
#[test]
fn startup_combined_scan_error_is_propagated_without_partial_cache_publication() {
    let (_temp_dir, kura) = pending_canonical_capacity_fixture();
    kura.validate_and_publish_configured_kura_capacity_after_startup_recovery(true)
        .expect("establish exact startup accounting");
    let enforced_before = kura.disk_usage.load(Ordering::Relaxed);
    let total_before = kura.disk_usage_total.load(Ordering::Relaxed);
    let blocks_dir = kura.active_blocks_dir.lock().clone();
    let invalid_total_only_directory = Kura::retained_block_rewrite_staging_dir_for(&blocks_dir);
    std::fs::write(&invalid_total_only_directory, b"not a directory")
        .expect("plant invalid total-only directory path");
    kura.validate_and_publish_configured_kura_capacity_after_startup_recovery(true)
        .expect_err("the authenticated combined scan must fail closed");
    assert!(!kura.disk_usage_initialized.load(Ordering::Relaxed));
    assert!(!kura.disk_usage_total_initialized.load(Ordering::Relaxed));
    assert!(kura.durable_budget_snapshot().is_none());
    assert_eq!(kura.disk_usage.load(Ordering::Relaxed), enforced_before);
    assert_eq!(kura.disk_usage_total.load(Ordering::Relaxed), total_before);
}
