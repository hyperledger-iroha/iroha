// Actual retained geometry operation ownership, including real journal I/O failures.

fn with_raw_geometry_fixture(test: impl FnOnce(&Kura, &ReplayGeometryBindingRequest<'_>)) {
    with_raw_geometry_fixture_mode(false, MAX_DISK_USAGE_BYTES, test);
}

fn with_raw_geometry_fixture_mode(
    in_memory: bool,
    max_disk_usage_bytes: iroha_config_base::util::Bytes,
    test: impl FnOnce(&Kura, &ReplayGeometryBindingRequest<'_>),
) {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let mut kura = open_kura(&root, &initial);
    Arc::get_mut(&mut kura)
        .expect("fixture owns its sole Kura")
        .max_disk_usage_bytes = max_disk_usage_bytes.get();
    let (previous_incarnations, previous_activation_heights) = initial_geometry();
    let (updated_incarnations, updated_activation_heights) = extended_geometry();
    authenticate_transition_fixture_primary(&kura, &initial, &previous_incarnations);
    let previous_lineage_root = unscoped_lineage_root(
        &kura
            .geometry_bindings(
                &initial,
                &previous_incarnations,
                &previous_activation_heights,
            )
            .unwrap(),
    );
    let updated_lineage_root = unscoped_lineage_root(
        &kura
            .geometry_bindings(
                &extended,
                &updated_incarnations,
                &updated_activation_heights,
            )
            .unwrap(),
    );
    let request = ReplayGeometryBindingRequest {
        previous: &initial,
        updated: &extended,
        previous_incarnations: &previous_incarnations,
        updated_incarnations: &updated_incarnations,
        previous_activation_heights: &previous_activation_heights,
        updated_activation_heights: &updated_activation_heights,
        previous_lineage_root,
        updated_lineage_root,
        transition_height: 9,
    };
    if in_memory {
        Arc::get_mut(&mut kura)
            .expect("fixture owns its sole Kura")
            .store_root
            .clear();
    }
    test(&kura, &request);
}

#[test]
fn raw_geometry_uses_lease_pending_capacity_without_a_held_lock_rescan() {
    let capacity = iroha_config_base::util::Bytes(u64::MAX / 4);
    with_raw_geometry_fixture_mode(false, capacity, |kura, request| {
        let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, None)
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
        block.set_execution_context(Some(BlockExecutionContextBundle::new(Vec::new())));
        kura.append_pending_block_for_bench(Arc::new(block));
        kura.pending_budget_raw_scans.store(0, Ordering::Relaxed);
        let lease = kura.try_publication_lease().unwrap();
        assert!(lease.pending_canonical_bytes() > 0);
        assert_eq!(kura.pending_budget_raw_scans.load(Ordering::Relaxed), 1);
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        // An invalidated global cache must not cause a metadata/sidecar scan
        // after this original lease has acquired the inner publication locks.
        kura.invalidate_pending_budget_cache();
        original.resume_under(&lease).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::FilesApplied);
        assert_eq!(kura.pending_budget_raw_scans.load(Ordering::Relaxed), 1);
        original.publish_catalog_under(&lease, None).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::CatalogPublished);
        assert_eq!(kura.pending_budget_raw_scans.load(Ordering::Relaxed), 1);
    });
}

#[test]
fn raw_geometry_capture_is_pure_and_excludes_competing_owners() {
    with_raw_geometry_fixture(|kura, request| {
        let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
        let lease = kura.try_publication_lease().unwrap();
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::Captured);
        let busy =
            match lease.begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new()) {
                Err(error @ Error::LaneGeometryAttemptBusy { .. }) => error,
                _ => panic!("the original claim must exclude competing owners"),
            };
        assert!(busy.to_string().contains("release pending"));
        assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
        original.rollback_under(&lease).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::RolledBack);
        assert!(busy.to_string().contains("released; retry acquisition"));
        let replacement = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        assert!(replacement.matches_request(request, &BTreeSet::new(), &BTreeMap::new()));
        drop(replacement);
        assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
    });
}

#[test]
fn raw_geometry_files_applied_keeps_claim_and_original_catalog_retry() {
    with_raw_geometry_fixture(|kura, request| {
        let lease = kura.try_publication_lease().unwrap();
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        original.resume_under(&lease).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::FilesApplied);
        drop(lease);
        let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
        assert!(matches!(
            kura.mark_lane_geometry_catalog_published_with_lineage_root(
                request.updated,
                request.updated_incarnations,
                request.updated_activation_heights,
                request.updated_lineage_root,
                None
            ),
            Err(Error::LaneGeometryAttemptBusy { .. })
        ));
        assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
        let lease = kura.try_publication_lease().unwrap();
        crate::kura::fail_bound_progress_intent_directory_sync_for_tests(0, 0);
        assert!(original.publish_catalog_under(&lease, None).is_err());
        assert_eq!(original.phase(), RawGeometryPhase::PublishingCatalog);
        assert!(original.has_pending_journal_write());
        let promoted = secure_file_metadata::from_path(&kura.lane_geometry_journal_path()).unwrap();
        assert!(original.rollback_under(&lease).is_err());
        drop(lease);
        let lease = kura.try_publication_lease().unwrap();
        original.publish_catalog_under(&lease, None).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::CatalogPublished);
        assert!(Kura::sidecar_metadata_same_object(
            &promoted,
            &secure_file_metadata::from_path(&kura.lane_geometry_journal_path()).unwrap()
        ));
        assert!(!original.has_pending_journal_write());
        assert!(
            lease
                .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
                .is_ok()
        );
    });
}

#[test]
fn raw_geometry_pending_intent_retries_original_then_owned_rollback() {
    with_raw_geometry_fixture(|kura, request| {
        let lease = kura.try_publication_lease().unwrap();
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        crate::kura::fail_next_bound_progress_intent_file_sync_for_tests();
        assert!(original.resume_under(&lease).is_err());
        assert!(original.has_pending_journal_write());
        assert!(original.rollback_under(&lease).is_err());
        let temporary = kura.store_root.join(JOURNAL_TEMP_FILE_NAME);
        let original_temp = secure_file_metadata::from_path(&temporary).unwrap();
        drop(lease);
        let lease = kura.try_publication_lease().unwrap();
        // Original temporary custody remains live across the physical lease release.
        assert!(Kura::sidecar_metadata_same_object(
            &original_temp,
            &secure_file_metadata::from_path(&temporary).unwrap()
        ));
        original.resume_under(&lease).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::FilesApplied);
        crate::kura::fail_bound_progress_intent_directory_sync_for_tests(0, 0);
        assert!(original.rollback_under(&lease).is_err());
        assert_eq!(original.phase(), RawGeometryPhase::RollingBack);
        assert!(original.has_pending_journal_write());
        let promoted = secure_file_metadata::from_path(&kura.lane_geometry_journal_path()).unwrap();
        original.rollback_under(&lease).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::RolledBack);
        assert!(Kura::sidecar_metadata_same_object(
            &promoted,
            &secure_file_metadata::from_path(&kura.lane_geometry_journal_path()).unwrap()
        ));
        let bindings = kura
            .geometry_bindings(
                request.previous,
                request.previous_incarnations,
                request.previous_activation_heights,
            )
            .unwrap();
        assert_eq!(kura.lane_storage_entries.lock().len(), bindings.len());
    });
}

#[test]
fn raw_geometry_abandoned_partial_operation_refuses_replacement() {
    with_raw_geometry_fixture(|kura, request| {
        let lease = kura.try_publication_lease().unwrap();
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        crate::kura::fail_next_bound_progress_intent_file_sync_for_tests();
        assert!(original.resume_under(&lease).is_err());
        let before = fs::read(kura.store_root.join(JOURNAL_TEMP_FILE_NAME)).unwrap();
        drop(original);
        assert!(matches!(
            lease.begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new()),
            Err(Error::LaneGeometryAttemptAbandoned)
        ));
        assert_eq!(
            fs::read(kura.store_root.join(JOURNAL_TEMP_FILE_NAME)).unwrap(),
            before
        );
    });
}

#[test]
fn raw_geometry_namespace_retry_retains_original_descriptor_and_refuses_uncaptured_creation() {
    with_raw_geometry_fixture(|kura, request| {
        let binding = kura
            .geometry_bindings(
                request.previous,
                request.previous_incarnations,
                request.previous_activation_heights,
            )
            .unwrap()
            .remove(0);
        let blocks = kura.binding_blocks_path(&binding);
        let path = Kura::lane_artifact_dir(&blocks);
        fs::remove_dir(&path).unwrap();
        let mut receipts = Vec::new();
        kura.ensure_authoritative_lane_artifact_namespace(&binding, &blocks, Some(&mut receipts))
            .unwrap();
        assert_eq!(receipts.len(), 1);
        let original =
            secure_file_metadata::from_file(&receipts[0].held.as_ref().unwrap().file).unwrap();
        receipts[0].inventory = None;
        kura.ensure_authoritative_lane_artifact_namespace(&binding, &blocks, Some(&mut receipts))
            .unwrap();
        assert!(Kura::sidecar_metadata_same_object(
            &original,
            &secure_file_metadata::from_file(&receipts[0].held.as_ref().unwrap().file).unwrap()
        ));
        assert!(receipts[0].inventory.is_some());
        // Model the retained native mkdir result whose immediate descriptor open failed.
        // The existing pathname must never be promoted to creation authority on retry.
        receipts[0].held = None;
        let before = secure_file_metadata::from_path(&path).unwrap();
        assert!(
            kura.ensure_authoritative_lane_artifact_namespace(
                &binding,
                &blocks,
                Some(&mut receipts)
            )
            .is_err()
        );
        assert!(Kura::sidecar_metadata_same_object(
            &before,
            &secure_file_metadata::from_path(&path).unwrap()
        ));
    });
}

#[test]
fn raw_geometry_in_memory_map_change_keeps_abandonment_fence() {
    with_raw_geometry_fixture_mode(true, MAX_DISK_USAGE_BYTES, |kura, request| {
        let lease = kura.try_publication_lease().unwrap();
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        original.resume_under(&lease).unwrap();
        assert_eq!(original.phase(), RawGeometryPhase::FilesApplied);
        assert_eq!(
            kura.lane_storage_entries.lock().len(),
            request.updated.entries().len()
        );
        drop(original);
        assert!(matches!(
            lease.begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new()),
            Err(Error::LaneGeometryAttemptAbandoned)
        ));
    });
}

#[test]
fn raw_geometry_in_memory_catalog_and_owned_rollback_keep_exact_maps() {
    with_raw_geometry_fixture_mode(true, MAX_DISK_USAGE_BYTES, |kura, request| {
        let lease = kura.try_publication_lease().unwrap();
        let mut rollback = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        rollback.resume_under(&lease).unwrap();
        rollback.rollback_under(&lease).unwrap();
        assert_eq!(rollback.phase(), RawGeometryPhase::RolledBack);
        assert_eq!(
            *kura.lane_storage_entries.lock(),
            kura.lane_storage_entries_from_geometry(
                request.previous,
                request.previous_incarnations,
                request.previous_activation_heights,
            )
            .unwrap()
        );
        let mut published = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        published.resume_under(&lease).unwrap();
        published.publish_catalog_under(&lease, None).unwrap();
        assert_eq!(published.phase(), RawGeometryPhase::CatalogPublished);
        assert_eq!(
            *kura.lane_storage_entries.lock(),
            kura.lane_storage_entries_from_geometry(
                request.updated,
                request.updated_incarnations,
                request.updated_activation_heights,
            )
            .unwrap()
        );
        kura.raw_geometry_claim.ensure_unclaimed().unwrap();
    });
}

#[test]
fn raw_geometry_resumes_snapshot_proven_gc_under_original_joint_lease() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
    checkpoint_retired_geometry(&kura, &fixture, 20).expect_err("retain actual pending collection");
    assert!(fixture.retained_blocks.exists());
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    let bindings = kura
        .geometry_bindings(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )
        .unwrap();
    let lineage_root = unscoped_lineage_root(&bindings);
    let request = ReplayGeometryBindingRequest {
        previous: &fixture.initial,
        updated: &fixture.initial,
        previous_incarnations: &fixture.initial_incarnations,
        updated_incarnations: &fixture.initial_incarnations,
        previous_activation_heights: &fixture.initial_activations,
        updated_activation_heights: &fixture.initial_activations,
        previous_lineage_root: lineage_root,
        updated_lineage_root: lineage_root,
        transition_height: 21,
    };
    let lease = kura.try_publication_lease().unwrap();
    let mut original = lease
        .begin_raw_geometry_attempt(&request, &BTreeSet::new(), &BTreeMap::new())
        .unwrap();
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
    assert!(
        fixture.retained_blocks.exists(),
        "capture cannot collect the original instance"
    );
    original.resume_under(&lease).unwrap();
    assert!(!fixture.retained_blocks.exists());
    assert!(
        kura.read_lane_geometry_journal()
            .unwrap()
            .pending_archive_gc
            .is_empty()
    );
    original.publish_catalog_under(&lease, None).unwrap();
    assert_eq!(original.phase(), RawGeometryPhase::CatalogPublished);
}

#[test]
fn raw_geometry_refuses_canonical_recovery_debt_without_consuming_it() {
    with_raw_geometry_fixture(|kura, request| {
        let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
        kura.block_store.lock().deferred_da_recovery_fault =
            Some("original deferred rewrite fault".into());
        let lease = kura.try_publication_lease().unwrap();
        assert!(matches!(
            lease.begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new()),
            Err(Error::LaneGeometryCanonicalRecoveryRequired)
        ));
        assert_eq!(
            kura.block_store
                .lock()
                .deferred_da_recovery_fault
                .as_deref(),
            Some("original deferred rewrite fault")
        );
        assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
    });
}

#[test]
fn raw_geometry_claim_excludes_canonical_mutation_between_leases() {
    with_raw_geometry_fixture(|kura, request| {
        let lease = kura.try_publication_lease().unwrap();
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        original.resume_under(&lease).unwrap();
        drop(lease);
        {
            let _prune = kura.prune_lock.lock();
            let _canonical = kura.canonical_chain_lock.lock();
            assert!(matches!(
                kura.resolve_canonical_storage_before_mutation(),
                Err(Error::LaneGeometryAttemptBusy { .. })
            ));
        }
        let lease = kura.try_publication_lease().unwrap();
        original.publish_catalog_under(&lease, None).unwrap();
        drop(lease);
        let _prune = kura.prune_lock.lock();
        let _canonical = kura.canonical_chain_lock.lock();
        kura.resolve_canonical_storage_before_mutation().unwrap();
    });
}

#[test]
fn raw_geometry_live_namespace_inventory_retry_keeps_original_creation_owner() {
    for substitute in [false, true] {
        with_raw_geometry_fixture(|kura, request| {
            let bindings = kura
                .geometry_bindings(
                    request.previous,
                    request.previous_incarnations,
                    request.previous_activation_heights,
                )
                .unwrap();
            let binding = &bindings[0];
            let path = Kura::lane_artifact_dir(&kura.binding_blocks_path(binding));
            fs::remove_dir(&path).unwrap();
            let lease = kura.try_publication_lease().unwrap();
            let mut original = lease
                .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
                .unwrap();
            FAIL_NEXT_GEOMETRY_NAMESPACE_INVENTORY.with(|fault| fault.set(true));
            original
                .resume_under(&lease)
                .expect_err("fail after actual native creation descriptor retention");
            assert_eq!(original.phase(), RawGeometryPhase::Maintenance);
            assert!(!original.has_pending_journal_write());
            let created = secure_file_metadata::from_path(&path).unwrap();
            drop(lease);
            if substitute {
                fs::rename(&path, path.with_extension("original-created")).unwrap();
                fs::create_dir(&path).unwrap();
            }
            let lease = kura.try_publication_lease().unwrap();
            if substitute {
                original
                    .resume_under(&lease)
                    .expect_err("same path cannot replace original creation custody");
                assert_eq!(original.phase(), RawGeometryPhase::Maintenance);
            } else {
                original.resume_under(&lease).unwrap();
                assert!(Kura::sidecar_metadata_same_object(
                    &created,
                    &secure_file_metadata::from_path(&path).unwrap()
                ));
                original.publish_catalog_under(&lease, None).unwrap();
            }
        });
    }
}

#[test]
fn raw_geometry_partial_instance_provisioning_retains_original_recovery_cause() {
    with_raw_geometry_fixture(|kura, request| {
        let lease = kura.try_publication_lease().unwrap();
        let mut original = lease
            .begin_raw_geometry_attempt(request, &BTreeSet::new(), &BTreeMap::new())
            .unwrap();
        FAIL_NEXT_GEOMETRY_INSTANCE_AFTER_FILES.with(|fault| fault.set(true));
        let first = original
            .resume_under(&lease)
            .expect_err("native base files exist before failure");
        let Error::LaneGeometryInstanceRecoveryRequired {
            lane_id,
            operation,
            source,
        } = first
        else {
            panic!("partial provisioning must retain typed local recovery cause: {first:?}");
        };
        assert_eq!(lane_id, LaneId::new(1));
        assert_eq!(operation, 0);
        assert!(
            matches!(source.as_ref(), Error::IO(error, _) if error.to_string().contains("original instance provisioning"))
        );
        assert_eq!(original.phase(), RawGeometryPhase::RecoveryRequired);
        let journal = fs::read(kura.lane_geometry_journal_path()).unwrap();
        let binding = kura
            .geometry_bindings(
                request.updated,
                request.updated_incarnations,
                request.updated_activation_heights,
            )
            .unwrap()
            .into_iter()
            .find(|binding| binding.lane_id == lane_id)
            .unwrap();
        let blocks = kura.binding_blocks_path(&binding);
        assert!(blocks.join(DATA_FILE_NAME).is_file());
        assert!(!blocks.join(MARKER_FILE_NAME).exists());
        let created = secure_file_metadata::from_path(&blocks).unwrap();
        drop(lease);
        let lease = kura.try_publication_lease().unwrap();
        for refusal in [
            original.recovery_refusal().unwrap(),
            original.resume_under(&lease).unwrap_err(),
            original.rollback_under(&lease).unwrap_err(),
        ] {
            let Error::LaneGeometryInstanceRecoveryRequired { source: retry, .. } = refusal else {
                panic!("recovery refusal must preserve original cause: {refusal:?}");
            };
            assert!(Arc::ptr_eq(&source, &retry));
        }
        assert!(Kura::sidecar_metadata_same_object(
            &created,
            &secure_file_metadata::from_path(&blocks).unwrap()
        ));
        assert_eq!(
            fs::read(kura.lane_geometry_journal_path()).unwrap(),
            journal
        );
        assert!(
            !blocks.join(MARKER_FILE_NAME).exists(),
            "same-process refusal cannot manufacture missing authority"
        );
    });
}
