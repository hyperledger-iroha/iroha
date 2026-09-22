// Strict receipt reads with real signed finality, failed durability barriers,
// original namespace custody, and exact resource bounds.

struct ResetReceiptNamespaceSyncFaults;

fn receipt_namespace_barrier_failure_modes() -> [(&'static str, ProgressSidecarBarrierFailure); 7] {
    [
        ("data", ProgressSidecarBarrierFailure::Data),
        ("index", ProgressSidecarBarrierFailure::Index),
        (
            "lane-artifacts-directory",
            ProgressSidecarBarrierFailure::ImmediateDirectory,
        ),
        (
            "instance-directory",
            ProgressSidecarBarrierFailure::AncestorDirectory(0),
        ),
        (
            "instances-directory",
            ProgressSidecarBarrierFailure::AncestorDirectory(1),
        ),
        (
            "blocks-directory",
            ProgressSidecarBarrierFailure::AncestorDirectory(2),
        ),
        (
            "store-root-directory",
            ProgressSidecarBarrierFailure::AncestorDirectory(3),
        ),
    ]
}

impl Drop for ResetReceiptNamespaceSyncFaults {
    fn drop(&mut self) {
        Kura::receipt_namespace_directory_failure_for_tests(false);
        FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_INDEXED_SIDECAR_INDEX_SYNC.with(|flag| flag.set(false));
        FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(false));
        FAIL_PROGRESS_SIDECAR_ANCESTOR_SYNC_AT.with(|fault| fault.set(None));
    }
}

#[test]
fn receipt_namespace_custody_hydration_observers_preserve_current_epoch() {
    with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
        let expected = kura
            .read_lane_completion_receipt(proposal)
            .unwrap()
            .unwrap();
        let artifact = kura
            .read_lane_block_artifact_read_only(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .unwrap()
            .unwrap();
        let epoch = kura
            .lock_consensus_sidecar_read()
            .unwrap()
            .mutation_epoch()
            .unwrap();
        for observer in 0..5 {
            match observer {
                0 => assert!(
                    kura.historical_autonomous_lane_recovery_records_bounded(1)
                        .unwrap()
                        .is_empty()
                ),
                1 => assert!(
                    kura.latest_autonomous_lane_block_artifacts_snapshot(
                        test_network_id(b"empty-hydration-observer"),
                        1,
                        |_| Ok(0),
                    )
                    .unwrap()
                    .is_empty()
                ),
                2 => assert_eq!(
                    kura.read_native_amx_participant_application_history(
                        proposal.descriptor.lane_id,
                    )
                    .unwrap()
                    .entries()
                    .count(),
                    0
                ),
                3 => kura
                    .recover_exact_canonical_lane_artifact(&artifact)
                    .unwrap(),
                4 => assert!(
                    kura.read_current_autonomous_lane_block_artifact(
                        proposal.descriptor.lane_id,
                        proposal.descriptor.lane_block_height,
                        test_network_id(b"empty-current-autonomous-observer"),
                        0,
                    )
                    .unwrap()
                    .is_none()
                ),
                _ => unreachable!(),
            }
            assert_eq!(
                kura.lock_consensus_sidecar_read().unwrap().mutation_epoch(),
                Some(epoch)
            );
            assert!(!Kura::receipt_namespace_directory_failure_for_tests(true));
            assert_eq!(
                kura.read_lane_completion_receipt(proposal).unwrap(),
                Some(expected.clone())
            );
            assert!(
                Kura::receipt_namespace_directory_failure_for_tests(false),
                "observer {observer} must leave the receipt directory barrier retained"
            );
        }
    });
}

#[test]
fn receipt_namespace_custody_existing_recovery_reauthenticates_carrier() {
    with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
        let receipt = kura
            .read_lane_completion_receipt(proposal)
            .unwrap()
            .unwrap();
        let artifact = kura
            .read_lane_block_artifact_read_only(
                proposal.descriptor.lane_id,
                proposal.descriptor.lane_block_height,
            )
            .unwrap()
            .unwrap();
        let entry = kura
            .lane_storage_entry(proposal.descriptor.lane_id)
            .unwrap();
        let (data, index) = Kura::lane_artifact_paths_for_entry(&entry, &kura.store_root());
        let raw = (fs::read(&data).unwrap(), fs::read(&index).unwrap());
        let finality_path = kura.v2_finality_artifact_path(receipt.application_block_height);
        let original = fs::read(&finality_path).unwrap();
        let mut input = original.as_slice();
        let mut record = KuraV2FinalityRecord::decode_all(&mut input).unwrap();
        record.artifact.height = record.artifact.height.checked_add(1).unwrap();
        fs::write(&finality_path, record.encode()).unwrap();
        assert!(
            kura.recover_exact_canonical_lane_artifact(&artifact)
                .is_err(),
            "the exact raw slot cannot replace fresh signed-carrier authentication"
        );
        assert_eq!((fs::read(&data).unwrap(), fs::read(&index).unwrap()), raw);
        fs::write(&finality_path, original).unwrap();
        kura.recover_exact_canonical_lane_artifact(&artifact)
            .unwrap();
        let mut different = artifact.clone();
        different.ownership.lane_block_height = different
            .ownership
            .lane_block_height
            .checked_add(1)
            .unwrap();
        assert!(
            kura.recover_exact_canonical_lane_artifact(&different)
                .is_err(),
            "signed ownership must match before any existing-slot fast return"
        );
        assert_eq!((fs::read(&data).unwrap(), fs::read(&index).unwrap()), raw);
    });
}

#[test]
fn receipt_namespace_custody_missing_recovery_keeps_mutation_and_first_barriers() {
    for fault in std::iter::once(None).chain(
        receipt_namespace_barrier_failure_modes()
            .into_iter()
            .map(|(_, fault)| Some(fault)),
    ) {
        with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
            let artifact = kura
                .read_lane_block_artifact_read_only(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .unwrap()
                .unwrap();
            let entry = kura
                .lane_storage_entry(proposal.descriptor.lane_id)
                .unwrap();
            let (data, index) = Kura::lane_artifact_paths_for_entry(&entry, &kura.store_root());
            {
                let _mutation = kura.sidecar_lock.lock();
                fs::remove_file(&data).unwrap();
                fs::remove_file(&index).unwrap();
            }
            assert!(
                kura.read_lane_block_artifact_read_only(
                    proposal.descriptor.lane_id,
                    proposal.descriptor.lane_block_height,
                )
                .unwrap()
                .is_none()
            );
            let before = kura
                .lock_consensus_sidecar_read()
                .unwrap()
                .mutation_epoch()
                .unwrap();
            if let Some(fault) = fault {
                fault.inject();
            }
            let recovered = kura.recover_exact_canonical_lane_artifact(&artifact);
            let after = kura
                .lock_consensus_sidecar_read()
                .unwrap()
                .mutation_epoch()
                .unwrap();
            assert!(
                after > before,
                "missing-slot repair must acquire mutation custody even on failure"
            );
            if fault.is_some() {
                assert!(
                    recovered.is_err(),
                    "missing-slot repair must perform its first-write barrier: {fault:?}"
                );
                if let Some(
                    fault @ (ProgressSidecarBarrierFailure::ImmediateDirectory
                    | ProgressSidecarBarrierFailure::AncestorDirectory(_)),
                ) = fault
                {
                    assert!(
                        !data.exists() && !index.exists(),
                        "failed namespace attestation must precede raw payload publication"
                    );
                    fault.inject();
                    assert!(
                        kura.recover_exact_canonical_lane_artifact(&artifact)
                            .is_err(),
                        "retry must reattest the failed original namespace, never take an existing-slot shortcut"
                    );
                    assert!(!data.exists() && !index.exists());
                    kura.recover_exact_canonical_lane_artifact(&artifact)
                        .unwrap();
                    assert_eq!(
                        kura.read_lane_block_artifact_read_only(
                            proposal.descriptor.lane_id,
                            proposal.descriptor.lane_block_height,
                        )
                        .unwrap(),
                        Some(artifact)
                    );
                }
            } else {
                recovered.unwrap();
                assert_eq!(
                    kura.read_lane_block_artifact_read_only(
                        proposal.descriptor.lane_id,
                        proposal.descriptor.lane_block_height,
                    )
                    .unwrap(),
                    Some(artifact)
                );
            }
        });
    }
}

fn with_receipt_namespace_custody_fixture(
    check: impl FnOnce(&Kura, &LaneBlockProposalV1, &Path, &Path),
) {
    let (
        (_temp_dir, _config, _lane_config),
        (_lane_id, lane_entry, _lane_block_height),
        (block, _ownership, proposal),
        kura,
    ) = MarkedLaneBlockFixture::committed().into_parts();
    store_finalized_fixture_block(&kura, Arc::new(block));
    kura.persist_lane_block_application_receipt(&proposal)
        .expect("publish canonical fixture receipt");
    // Separate the measured first read from any setup/writer observations.
    *kura.lane_receipt_namespace_durability.lock() = LaneReceiptNamespaceDurability::default();
    let (data, index) =
        Kura::lane_block_application_receipt_paths_for_entry(&lane_entry, &kura.store_root());
    let _fault_reset = ResetReceiptNamespaceSyncFaults;
    check(&kura, &proposal, &data, &index);
}

#[test]
fn receipt_namespace_custody_reuses_directory_barriers_but_syncs_both_files() {
    // An injected failure remains pending after a warm read. Clearing only the
    // retained directory owner must expose that exact previously injected fault.
    for (label, fault) in receipt_namespace_barrier_failure_modes()
        .into_iter()
        .skip(2)
    {
        with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
            let expected = kura
                .read_lane_completion_receipt(proposal)
                .unwrap()
                .unwrap();
            assert!(
                !kura
                    .lane_receipt_namespace_durability
                    .lock()
                    .slots
                    .iter()
                    .all(Option::is_none)
            );
            fault.inject();
            assert_eq!(
                kura.read_lane_completion_receipt(proposal).unwrap(),
                Some(expected.clone()),
                "unchanged {label} must reuse its completed directory barrier"
            );
            *kura.lane_receipt_namespace_durability.lock() =
                LaneReceiptNamespaceDurability::default();
            assert!(
                kura.read_lane_completion_receipt(proposal).is_err(),
                "warm read must have left the {label} fault unconsumed"
            );
            assert_eq!(
                kura.read_lane_completion_receipt(proposal).unwrap(),
                Some(expected)
            );
        });
    }
    for (label, fault) in receipt_namespace_barrier_failure_modes()
        .into_iter()
        .take(2)
    {
        with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
            let expected = kura
                .read_lane_completion_receipt(proposal)
                .unwrap()
                .unwrap();
            fault.inject();
            assert!(
                kura.read_lane_completion_receipt(proposal).is_err(),
                "warm directory custody must never skip the fresh {label} barrier"
            );
            assert_eq!(
                kura.read_lane_completion_receipt(proposal).unwrap(),
                Some(expected)
            );
        });
    }
}

#[test]
fn receipt_namespace_custody_failed_first_barrier_never_authorizes_retry() {
    for (label, fault) in receipt_namespace_barrier_failure_modes() {
        with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
            for _ in 0..2 {
                fault.inject();
                assert!(
                    kura.read_lane_completion_receipt(proposal).is_err(),
                    "failed first {label} must not mint durability custody"
                );
                assert!(
                    kura.lane_receipt_namespace_durability
                        .lock()
                        .slots
                        .iter()
                        .all(Option::is_none)
                );
            }
            assert!(
                kura.read_lane_completion_receipt(proposal)
                    .unwrap()
                    .is_some()
            );
            assert!(
                !kura
                    .lane_receipt_namespace_durability
                    .lock()
                    .slots
                    .iter()
                    .all(Option::is_none)
            );
        });
    }
}

#[test]
fn receipt_namespace_custody_rechecks_every_held_directory_generation() {
    // Current layout: lane_artifacts, instance ID, instances, blocks, root.
    for ordinal in 0..LANE_RECEIPT_NAMESPACE_DEPTH {
        with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
            let expected = kura
                .read_lane_completion_receipt(proposal)
                .unwrap()
                .unwrap();
            let (path, modified) = {
                let custody = kura.lane_receipt_namespace_durability.lock();
                let namespace = &custody
                    .slots
                    .iter()
                    .flatten()
                    .next()
                    .expect("original directory custody")
                    .namespace;
                assert_eq!(
                    namespace.directories.len(),
                    LANE_RECEIPT_NAMESPACE_DEPTH,
                    "all real fixture ancestors covered"
                );
                let directory = &namespace.directories[ordinal];
                (
                    directory.expected_path.clone(),
                    directory.metadata.modified().unwrap(),
                )
            };
            fs::write(
                path.join(format!("receipt-custody-sibling-{ordinal}")),
                b"new sibling",
            )
            .expect("mutate this actual namespace");
            // Guarantee a distinct sampled generation without sleeps or relying
            // on the filesystem's timestamp tick for two rapid operations.
            fs::File::open(&path)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(modified + Duration::from_secs(60)))
                .expect("set an observably distinct directory generation");
            {
                let custody = kura.lane_receipt_namespace_durability.lock();
                assert!(!Kura::receipt_namespace_generations_unchanged(
                    &custody.slots.iter().flatten().next().unwrap().namespace
                ));
            }
            let fault = if ordinal == 0 {
                ProgressSidecarBarrierFailure::ImmediateDirectory
            } else {
                ProgressSidecarBarrierFailure::AncestorDirectory(ordinal - 1)
            };
            fault.inject();
            assert!(
                kura.read_lane_completion_receipt(proposal).is_err(),
                "changed directory {ordinal} must force a new barrier"
            );
            assert_eq!(
                kura.read_lane_completion_receipt(proposal).unwrap(),
                Some(expected)
            );
        });
    }
}

#[test]
fn receipt_namespace_custody_cannot_hide_occupied_corruption() {
    for mutation in ["data", "index", "missing_index", "unresolved_rewrite"] {
        with_receipt_namespace_custody_fixture(|kura, proposal, data, index| {
            kura.read_lane_completion_receipt(proposal)
                .unwrap()
                .unwrap();
            let changed_path = match mutation {
                "data" => data.to_path_buf(),
                "index" => index.to_path_buf(),
                "missing_index" => {
                    fs::remove_file(index).unwrap();
                    index.to_path_buf()
                }
                "unresolved_rewrite" => index.with_extension("index.tmp"),
                _ => unreachable!(),
            };
            match mutation {
                "data" | "index" => {
                    let mut bytes = fs::read(&changed_path).unwrap();
                    assert!(!bytes.is_empty());
                    bytes[0] ^= 0xff;
                    fs::write(&changed_path, &bytes).unwrap();
                }
                "unresolved_rewrite" => fs::write(&changed_path, b"unresolved rewrite").unwrap(),
                "missing_index" => {}
                _ => unreachable!(),
            }
            let before = fs::read(&changed_path).ok();
            assert!(
                kura.read_lane_completion_receipt(proposal).is_err(),
                "occupied {mutation} is a typed error, never absence or cached success"
            );
            assert_eq!(
                fs::read(&changed_path).ok(),
                before,
                "strict read must preserve occupied evidence"
            );
        });
    }
}

#[test]
fn receipt_namespace_custody_reauthenticates_current_canonical_finality() {
    with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
        let expected = kura
            .read_lane_completion_receipt(proposal)
            .unwrap()
            .unwrap();
        let path = kura.v2_finality_artifact_path(expected.application_block_height);
        let original = fs::read(&path).unwrap();
        let mut input = original.as_slice();
        let mut record = KuraV2FinalityRecord::decode_all(&mut input).unwrap();
        record.artifact.height = record.artifact.height.checked_add(1).unwrap();
        fs::write(&path, record.encode()).unwrap();
        {
            let custody = kura.lane_receipt_namespace_durability.lock();
            assert!(
                Kura::receipt_namespace_generations_unchanged(
                    &custody.slots.iter().flatten().next().unwrap().namespace
                ),
                "in-place finality mutation must isolate canonical authentication from namespace invalidation"
            );
        }
        assert!(
            kura.read_lane_completion_receipt(proposal).is_err(),
            "unchanged receipt directory custody cannot authorize changed canonical evidence"
        );
        // Do not clear Kura's finality cache: its real reader must notice the
        // changed record by itself, both on refusal and after valid restoration.
        fs::write(&path, original).unwrap();
        assert_eq!(
            kura.read_lane_completion_receipt(proposal).unwrap(),
            Some(expected)
        );
    });
}

#[test]
fn receipt_namespace_custody_replacement_requires_a_new_barrier() {
    with_receipt_namespace_custody_fixture(|kura, proposal, data, _| {
        let expected = kura
            .read_lane_completion_receipt(proposal)
            .unwrap()
            .unwrap();
        let original = fs::read(data).unwrap();
        fs::rename(data, data.with_extension("displaced")).unwrap();
        fs::write(data, original).unwrap();
        fail_next_indexed_sidecar_dir_sync_for_tests();
        assert!(
            kura.read_lane_completion_receipt(proposal).is_err(),
            "same bytes in a new named object require fresh directory durability"
        );
        assert_eq!(
            kura.read_lane_completion_receipt(proposal).unwrap(),
            Some(expected)
        );
    });
}

#[test]
fn receipt_namespace_custody_mutation_epoch_requires_a_new_directory_barrier() {
    with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
        let expected = kura
            .read_lane_completion_receipt(proposal)
            .unwrap()
            .unwrap();
        let original_epoch = {
            let custody = kura.lane_receipt_namespace_durability.lock();
            custody
                .slots
                .iter()
                .flatten()
                .next()
                .unwrap()
                .mutation_epoch
        };
        // No filesystem operation occurs: unchanged timestamps alone cannot
        // authorize reuse after the actual mutation-capable gate was acquired.
        let writer_epoch = {
            let writer = kura.sidecar_lock.lock();
            writer
                .mutation_epoch()
                .expect("tracked original sidecar mutex")
        };
        assert!(writer_epoch > original_epoch);
        {
            let custody = kura.lane_receipt_namespace_durability.lock();
            let retained = custody.slots.iter().flatten().next().unwrap();
            assert!(Kura::receipt_namespace_generations_unchanged(
                &retained.namespace
            ));
            assert_ne!(retained.mutation_epoch, writer_epoch);
        }
        fail_next_indexed_sidecar_dir_sync_for_tests();
        assert!(
            kura.read_lane_completion_receipt(proposal).is_err(),
            "a no-op writer acquisition still invalidates the old directory barrier"
        );
        assert_eq!(
            kura.read_lane_completion_receipt(proposal).unwrap(),
            Some(expected)
        );
        let custody = kura.lane_receipt_namespace_durability.lock();
        assert!(
            custody
                .slots
                .iter()
                .flatten()
                .any(|entry| entry.mutation_epoch == writer_epoch),
            "successful retry must record the actual current sidecar guard epoch"
        );
    });
}

// Test-only matched workloads. Both use the same current binary, real fixture,
// first warm read, exact canonical authentication, and 32 subsequent reads.
// Resetting the private test owner forces directory reattestation without a
// production behavior flag or an alternative reader implementation.
fn repeat_canonical_receipt_reads_for_namespace_measurement(reset_before_read: bool) {
    with_receipt_namespace_custody_fixture(|kura, proposal, _, _| {
        let expected = kura
            .read_lane_completion_receipt(proposal)
            .unwrap()
            .unwrap();
        for _ in 0..32 {
            if reset_before_read {
                *kura.lane_receipt_namespace_durability.lock() =
                    LaneReceiptNamespaceDurability::default();
            }
            assert_eq!(
                kura.read_lane_completion_receipt(proposal).unwrap(),
                Some(expected.clone())
            );
        }
    });
}

#[test]
fn receipt_namespace_custody_repeated_canonical_reads_control() {
    repeat_canonical_receipt_reads_for_namespace_measurement(true);
}

#[test]
fn receipt_namespace_custody_repeated_canonical_reads_retained() {
    repeat_canonical_receipt_reads_for_namespace_measurement(false);
}

#[test]
fn receipt_namespace_custody_admission_bounds_descriptors_and_actual_heap() {
    with_receipt_namespace_custody_fixture(|kura, _, data, index| {
        let mut custody = LaneReceiptNamespaceDurability::default();
        for ordinal in 0..(LANE_RECEIPT_NAMESPACE_SLOTS * 2) {
            let sibling_data = data.with_file_name(format!("receipt-admission-{ordinal}.norito"));
            let sibling_index = index.with_file_name(format!("receipt-admission-{ordinal}.index"));
            let namespace = kura
                .open_bound_progress_namespace(&sibling_data, &sibling_index)
                .unwrap();
            assert_eq!(namespace.directories.len(), LANE_RECEIPT_NAMESPACE_DEPTH);
            custody.retain(namespace, 7);
            assert!(custody.directory_count() <= LANE_RECEIPT_DURABILITY_DIRECTORY_CAPACITY);
            assert!(custody.heap_bytes() <= LANE_RECEIPT_DURABILITY_HEAP_BYTES);
            assert!(resident_inventory::ResidentOwner::resident_complete(
                &custody
            ));
        }
        assert_eq!(
            custody.slots.iter().flatten().count(),
            LANE_RECEIPT_NAMESPACE_SLOTS
        );
        assert_eq!(
            custody.directory_count(),
            LANE_RECEIPT_NAMESPACE_SLOTS * LANE_RECEIPT_NAMESPACE_DEPTH
        );
        assert_eq!(
            resident_inventory::ResidentOwner::resident_associations(&custody).unwrap(),
            (LANE_RECEIPT_NAMESPACE_SLOTS * (LANE_RECEIPT_NAMESPACE_DEPTH + 1)) as u64
        );
        let before = custody.heap_bytes();
        let mut oversized = kura.open_bound_progress_namespace(data, index).unwrap();
        oversized
            .data_path
            .reserve(LANE_RECEIPT_DURABILITY_HEAP_BYTES + 1);
        custody.retain(oversized, 7);
        assert_eq!(
            custody.heap_bytes(),
            before,
            "oversized path capacity is never retained"
        );
        let mut deep = kura.open_bound_progress_namespace(data, index).unwrap();
        deep.directories
            .reserve(LANE_RECEIPT_DURABILITY_DIRECTORY_CAPACITY + 1);
        custody.retain(deep, 7);
        assert_eq!(
            custody.heap_bytes(),
            before,
            "excess unused Vec capacity is never retained"
        );
        custody.retain(kura.open_bound_progress_namespace(data, index).unwrap(), 8);
        assert_eq!(
            custody.slots.iter().flatten().count(),
            1,
            "new mutation epoch drops old custody"
        );
    });
}
