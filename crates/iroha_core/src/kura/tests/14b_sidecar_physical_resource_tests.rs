// Actual sidecar owners and exact checked physical scope; no aggregate is inferred from counts.

fn sidecar_physical_test_paths(kura: &Kura) -> (PathBuf, PathBuf) {
    let directory = kura.store_dir().unwrap().join(PIPELINE_DIR_NAME);
    fs::create_dir_all(&directory).unwrap();
    (
        directory.join(PIPELINE_SIDECARS_DATA_FILE),
        directory.join(PIPELINE_SIDECARS_INDEX_FILE),
    )
}

fn sidecar_physical_assert_actual(kura: &Kura) -> IndexResourceCounts {
    let actual = kura
        .physical_resource_scope()
        .unwrap()
        .observe(kura.evidence_resource_limits())
        .unwrap();
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert_eq!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .unwrap(),
            actual[family as usize],
            "{family:?}"
        );
    }
    actual
}

fn sidecar_physical_assert_unavailable(kura: &Kura) {
    for family in PHYSICAL_RESOURCE_FAMILIES {
        assert!(
            kura.resource_inventory
                .component_usage_for_tests(family)
                .is_err(),
            "{family:?} must remain unavailable after an incomplete writer"
        );
    }
}

fn sidecar_physical_stage_rewrite(data: &Path, index: &Path, payload: &[u8], height: u64) {
    fs::write(data.with_extension("norito.tmp"), payload).unwrap();
    let mut bytes = SidecarIndexLayout::base_header(height).to_vec();
    if !payload.is_empty() {
        bytes.extend_from_slice(
            &SidecarIndexEntry {
                offset: 0,
                len: u64::try_from(payload.len()).unwrap(),
            }
            .to_bytes(),
        );
    }
    fs::write(index.with_extension("index.tmp"), bytes).unwrap();
}

#[test]
fn sidecar_physical_footprint_counts_data_and_all_five_index_or_evidence_files() {
    let (_directory, _config, kura) = unwrapped_kura_fixture();
    let (data, index) = sidecar_physical_test_paths(&kura);
    let paths = Kura::sidecar_physical_resource_paths(&data, &index);
    assert_eq!(
        paths,
        vec![
            data.clone(),
            index.clone(),
            data.with_extension("norito.tmp"),
            index.with_extension("index.tmp"),
            index.with_extension("index.prepend.tmp"),
            Kura::bound_progress_append_build_path(&index),
            Kura::bound_progress_append_intent_path(&index),
        ]
    );
    assert_eq!(paths.iter().collect::<BTreeSet<_>>().len(), 7);
    let mut entry = SidecarIndexLayout::base_header(1).to_vec();
    entry.extend_from_slice(&SidecarIndexEntry { offset: 0, len: 3 }.to_bytes());
    let payloads = [
        vec![1; 3],
        entry.clone(),
        vec![2; 5],
        entry.clone(),
        entry.clone(),
        vec![3; 7],
        vec![4; 11],
    ];
    for (path, bytes) in paths.iter().zip(&payloads) {
        fs::write(path, bytes).unwrap();
    }
    let usage = physical_resource_paths_usage(&paths, kura.evidence_resource_limits()).unwrap();
    let indexed = usage[ResourceFamily::PipelineIndex as usize];
    assert_eq!(indexed.persisted_entries, 3);
    assert_eq!(indexed.index_bytes, u64::try_from(entry.len()).unwrap());
    assert_eq!(
        indexed.temporary_index_bytes,
        2 * u64::try_from(entry.len()).unwrap()
    );
    assert_eq!(
        usage[ResourceFamily::EvidenceKeyRecords as usize].persisted_entries,
        2
    );
    assert_eq!(
        usage[ResourceFamily::EvidenceKeyRecords as usize].temporary_index_bytes,
        18
    );
    assert_eq!(
        usage[ResourceFamily::StorageBytes as usize].storage_bytes,
        payloads
            .iter()
            .map(|bytes| u64::try_from(bytes.len()).unwrap())
            .sum::<u64>()
    );
    kura.reconcile_physical_resource_inventory().unwrap();
    let before = sidecar_physical_assert_actual(&kura);
    let _owner = kura.sidecar_lock.lock();
    let mutation = kura
        .begin_total_disk_usage_mutation()
        .with_resource_paths(paths.clone());
    for path in paths {
        fs::remove_file(path).unwrap();
    }
    mutation.finish_resources_before_disk_rescan();
    let after = sidecar_physical_assert_actual(&kura);
    assert_eq!(
        before[ResourceFamily::StorageBytes as usize].storage_bytes
            - after[ResourceFamily::StorageBytes as usize].storage_bytes,
        usage[ResourceFamily::StorageBytes as usize].storage_bytes
    );
}

#[test]
fn pipeline_physical_append_prepend_replacement_and_exact_retry_match_actual_files() {
    let (_directory, _config, kura) = unwrapped_kura_fixture();
    let hashes = store_dummy_blocks(&kura, 2);
    let (data, index) = sidecar_physical_test_paths(&kura);
    kura.reconcile_physical_resource_inventory().unwrap();
    let initial = sidecar_physical_assert_actual(&kura);
    // Height two first forces the later height-one publication through the prepend owner.
    for height in [2_u64, 1] {
        let sidecar = PipelineRecoverySidecar::new(
            height,
            hashes[usize::try_from(height - 1).expect("fixture value fits usize")],
            PipelineDagSnapshot {
                fingerprint: [u8::try_from(height).expect("fixture value fits u8"); 32],
                key_count: 1,
            },
            Vec::new(),
        );
        kura.write_pipeline_metadata(&sidecar);
        assert_eq!(
            kura.read_pipeline_metadata(height)
                .unwrap()
                .encode_framed()
                .unwrap(),
            sidecar.encode_framed().unwrap()
        );
        sidecar_physical_assert_actual(&kura);
    }
    let before_retry = sidecar_physical_assert_actual(&kura);
    let previous = kura.read_pipeline_metadata(1).unwrap();
    kura.write_pipeline_metadata(&previous);
    assert_eq!(sidecar_physical_assert_actual(&kura), before_retry);
    let mut replacement = previous;
    replacement.dag.key_count = 19;
    kura.write_pipeline_metadata(&replacement);
    assert_eq!(
        kura.read_pipeline_metadata(1)
            .unwrap()
            .encode_framed()
            .unwrap(),
        replacement.encode_framed().unwrap()
    );
    let after = sidecar_physical_assert_actual(&kura);
    assert_eq!(
        after[ResourceFamily::PipelineIndex as usize].persisted_entries,
        2
    );
    assert_eq!(
        after[ResourceFamily::StorageBytes as usize].storage_bytes
            - initial[ResourceFamily::StorageBytes as usize].storage_bytes,
        fs::metadata(&data).unwrap().len() + fs::metadata(&index).unwrap().len()
    );
    for path in Kura::sidecar_physical_resource_paths(&data, &index)
        .into_iter()
        .skip(2)
    {
        assert!(!path.exists(), "successful publication retires {path:?}");
    }
}

#[test]
fn fastpq_sidecar_physical_attachment_and_duplicate_preserve_exact_totals() {
    let (_directory, _config, kura, hash, sidecar) = default_pipeline_sidecar_fixture();
    kura.write_pipeline_metadata(&sidecar);
    kura.reconcile_physical_resource_inventory().unwrap();
    let before = sidecar_physical_assert_actual(&kura);
    let first = sample_fastpq_snapshot(1, hash, 8);
    let second = sample_fastpq_snapshot(1, hash, 9);
    assert!(matches!(
        kura.write_fastpq_proof_snapshots(
            &[&first, &second],
            &mut kura.sidecar_lock.defer_notifications()
        ),
        FastpqProofWriteResult::Written
    ));
    let persisted = kura.read_pipeline_metadata(1).unwrap();
    assert_eq!(persisted.fastpq_proofs.len(), 2);
    let after = sidecar_physical_assert_actual(&kura);
    assert_eq!(
        after[ResourceFamily::PipelineIndex as usize],
        before[ResourceFamily::PipelineIndex as usize]
    );
    assert_eq!(
        after[ResourceFamily::StorageBytes as usize].storage_bytes
            - before[ResourceFamily::StorageBytes as usize].storage_bytes,
        u64::try_from(persisted.encode_framed().unwrap().len()).unwrap()
    );
    assert!(matches!(
        kura.write_fastpq_proof_snapshots(
            &[&first, &second],
            &mut kura.sidecar_lock.defer_notifications()
        ),
        FastpqProofWriteResult::Written
    ));
    assert_eq!(sidecar_physical_assert_actual(&kura), after);
}

#[test]
fn pipeline_physical_failed_barriers_poison_storage_and_retry_does_not_repair_inventory() {
    for (label, inject) in strict_indexed_sidecar_failure_modes() {
        let (_directory, kura) = unwrapped_inline_kura_fixture_with_fsync(FsyncMode::Always);
        let hash = store_dummy_blocks(&kura, 1)[0];
        let sidecar = PipelineRecoverySidecar::new(
            1,
            hash,
            PipelineDagSnapshot {
                fingerprint: [3; 32],
                key_count: 1,
            },
            Vec::new(),
        );
        kura.reconcile_physical_resource_inventory().unwrap();
        inject();
        kura.write_pipeline_metadata(&sidecar);
        sidecar_physical_assert_unavailable(&kura);
        kura.write_pipeline_metadata(&sidecar);
        assert_eq!(
            kura.read_pipeline_metadata(1)
                .unwrap()
                .encode_framed()
                .unwrap(),
            sidecar.encode_framed().unwrap(),
            "{label}"
        );
        sidecar_physical_assert_unavailable(&kura);
        kura.reconcile_physical_resource_inventory().unwrap();
        sidecar_physical_assert_actual(&kura);
    }
}

#[test]
fn fastpq_sidecar_physical_failed_update_retains_unavailable_until_complete_reaudit() {
    let (_directory, kura) = unwrapped_inline_kura_fixture_with_fsync(FsyncMode::Always);
    let hash = store_dummy_blocks(&kura, 1)[0];
    let sidecar = PipelineRecoverySidecar::new(
        1,
        hash,
        PipelineDagSnapshot {
            fingerprint: [4; 32],
            key_count: 1,
        },
        Vec::new(),
    );
    kura.write_pipeline_metadata(&sidecar);
    kura.reconcile_physical_resource_inventory().unwrap();
    let snapshot = sample_fastpq_snapshot(1, hash, 8);
    fail_next_indexed_sidecar_data_sync_for_tests();
    assert!(matches!(
        kura.write_fastpq_proof_snapshots(
            &[&snapshot],
            &mut kura.sidecar_lock.defer_notifications()
        ),
        FastpqProofWriteResult::Retry
    ));
    sidecar_physical_assert_unavailable(&kura);
    assert!(matches!(
        kura.write_fastpq_proof_snapshots(
            &[&snapshot],
            &mut kura.sidecar_lock.defer_notifications()
        ),
        FastpqProofWriteResult::Written
    ));
    assert_eq!(
        kura.read_pipeline_metadata(1).unwrap().fastpq_proofs.len(),
        1
    );
    sidecar_physical_assert_unavailable(&kura);
    kura.reconcile_physical_resource_inventory().unwrap();
    sidecar_physical_assert_actual(&kura);
}

#[test]
fn sidecar_physical_recovery_accounts_pair_index_only_and_empty_rewrite() {
    for (index_only, empty) in [(false, false), (true, false), (false, true)] {
        let (_directory, _config, kura, _, sidecar) = default_pipeline_sidecar_fixture();
        kura.write_pipeline_metadata(&sidecar);
        let (data, index) = sidecar_physical_test_paths(&kura);
        let payload = if empty {
            Vec::new()
        } else {
            let mut replacement = sidecar.clone();
            replacement.dag.key_count = 23;
            replacement.encode_framed().unwrap()
        };
        sidecar_physical_stage_rewrite(&data, &index, &payload, if empty { 2 } else { 1 });
        if index_only {
            fs::rename(data.with_extension("norito.tmp"), &data).unwrap();
        }
        kura.reconcile_physical_resource_inventory().unwrap();
        let before = sidecar_physical_assert_actual(&kura);
        assert!(before[ResourceFamily::PipelineIndex as usize].temporary_index_bytes > 0);
        let _owner = kura.sidecar_lock.lock();
        assert!(kura.recover_indexed_sidecar_with_physical_resources(
            &data,
            &index,
            "physical recovery"
        ));
        assert_eq!(fs::read(&data).unwrap(), payload);
        let after = sidecar_physical_assert_actual(&kura);
        assert_eq!(
            after[ResourceFamily::PipelineIndex as usize].temporary_index_bytes,
            0
        );
        assert_eq!(
            after[ResourceFamily::PipelineIndex as usize].persisted_entries,
            u64::from(!empty)
        );
        assert!(!data.with_extension("norito.tmp").exists());
        assert!(!index.with_extension("index.tmp").exists());
        assert!(
            after[ResourceFamily::StorageBytes as usize].storage_bytes
                < before[ResourceFamily::StorageBytes as usize].storage_bytes
        );
    }
}

#[test]
fn sidecar_physical_failed_recovery_keeps_all_families_unavailable() {
    for fail_promotion in [false, true] {
        let (_directory, _config, kura, _, sidecar) = default_pipeline_sidecar_fixture();
        kura.write_pipeline_metadata(&sidecar);
        let (data, index) = sidecar_physical_test_paths(&kura);
        let old_index = fs::read(&index).unwrap();
        let mut replacement = sidecar;
        replacement.dag.key_count = 31;
        let payload = replacement.encode_framed().unwrap();
        if fail_promotion {
            sidecar_physical_stage_rewrite(&data, &index, &payload, 1);
        } else {
            fs::write(data.with_extension("norito.tmp"), &payload).unwrap();
        }
        kura.reconcile_physical_resource_inventory().unwrap();
        let _owner = kura.sidecar_lock.lock();
        if fail_promotion {
            fail_next_sidecar_promotion_dir_sync_for_tests();
        }
        assert!(!kura.recover_indexed_sidecar_with_physical_resources(
            &data,
            &index,
            "failed physical recovery"
        ));
        assert_eq!(fs::read(&index).unwrap(), old_index);
        sidecar_physical_assert_unavailable(&kura);
        if fail_promotion {
            assert!(kura.recover_indexed_sidecar_with_physical_resources(
                &data,
                &index,
                "retry physical recovery"
            ));
            assert_eq!(fs::read(&data).unwrap(), payload);
            sidecar_physical_assert_unavailable(&kura);
        }
    }
}

#[test]
fn sidecar_physical_absent_rewrite_markers_do_not_read_stable_index_or_repair_faults() {
    let (_directory, _config, kura) = unwrapped_kura_fixture();
    let (data, index) = sidecar_physical_test_paths(&kura);
    fs::write(&data, b"opaque fixture bytes").unwrap();
    fs::write(&index, b"not a V1 index").unwrap();
    kura.resource_inventory.invalidate(
        physical_resource_mask(),
        resource_inventory::Unavailable::Interrupted,
    );
    let generation = kura.resource_inventory.reconciliation_generation().unwrap();
    let _owner = kura.sidecar_lock.lock();
    assert!(kura.recover_indexed_sidecar_with_physical_resources(
        &data,
        &index,
        "read-only physical fast path"
    ));
    assert_eq!(
        kura.resource_inventory.reconciliation_generation().unwrap(),
        generation
    );
    assert_eq!(fs::read(index).unwrap(), b"not a V1 index");
    sidecar_physical_assert_unavailable(&kura);
}

#[cfg(windows)]
mod native_pipeline_public_callers {
    //! Public pipeline and FASTPQ recovery retain originals and exact accounting.

    use super::*;

    struct ResetFaults;

    impl Drop for ResetFaults {
        fn drop(&mut self) {
            FAIL_NEXT_INDEXED_SIDECAR_INITIAL_DATA_SYNC.with(|flag| flag.set(false));
            FAIL_NEXT_INDEXED_SIDECAR_DATA_SYNC.with(|flag| flag.set(false));
            FAIL_NEXT_INDEXED_SIDECAR_DIR_SYNC.with(|flag| flag.set(false));
        }
    }

    fn fixture(count: usize) -> (TempDir, Arc<Kura>, Vec<HashOf<BlockHeader>>) {
        let temporary = TempDir::new().unwrap();
        let root = {
            let private =
                iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("store")).unwrap();
            private.path().to_path_buf()
        };
        let mut config = kura_config_for_path(&root, BLOCKS_IN_MEMORY);
        config.fsync_mode = FsyncMode::Always;
        let (kura, _) = Kura::open_test_kura_with_configured_lane_config(
            &config,
            &RuntimeLaneConfig::default(),
        )
        .unwrap();
        let hashes = store_dummy_blocks(&kura, count);
        kura.reconcile_resident_resource_inventory().unwrap();
        kura.reconcile_physical_resource_inventory().unwrap();
        (temporary, kura, hashes)
    }

    fn sidecar(height: u64, hash: HashOf<BlockHeader>) -> PipelineRecoverySidecar {
        PipelineRecoverySidecar::new(
            height,
            hash,
            PipelineDagSnapshot {
                fingerprint: [7; 32],
                key_count: 3,
            },
            Vec::new(),
        )
    }

    fn queue_records(kura: &Kura) -> u64 {
        kura.resource_inventory
            .component_usage_for_tests(ResourceFamily::ResidentQueue)
            .unwrap()
            .resident_associations
    }

    fn identity(path: &Path) -> iroha_fs::FileIdentity {
        iroha_fs::FileIdentity::of(&fs::File::open(path).unwrap()).unwrap()
    }

    #[test]
    fn native_pipeline_public_read_recovers_failed_payload_on_original_pair() {
        let _reset = ResetFaults;
        let (_temporary, kura, hashes) = fixture(1);
        let expected = sidecar(1, hashes[0]);
        let before = queue_records(&kura);
        fail_next_indexed_sidecar_initial_data_sync_for_tests();
        kura.write_pipeline_metadata(&expected);
        assert!(kura.native_pipeline_pending());
        assert_eq!(queue_records(&kura), before + 1);
        sidecar_physical_assert_unavailable(&kura);
        let (data, index) = sidecar_physical_test_paths(&kura);
        let originals = (identity(&data), identity(&index));
        for path in [&data, &index] {
            assert!(fs::OpenOptions::new().write(true).open(path).is_err());
        }
        assert_eq!(fs::metadata(&data).unwrap().len(), 0);
        assert_eq!(
            kura.read_pipeline_metadata(1)
                .unwrap()
                .encode_framed()
                .unwrap(),
            expected.encode_framed().unwrap()
        );
        assert!(!kura.native_pipeline_pending());
        assert_eq!(queue_records(&kura), before);
        assert_eq!((identity(&data), identity(&index)), originals);
        let published = (fs::read(&data).unwrap(), fs::read(&index).unwrap());
        kura.write_pipeline_metadata(&expected);
        assert_eq!(
            (fs::read(&data).unwrap(), fs::read(&index).unwrap()),
            published
        );
        sidecar_physical_assert_unavailable(&kura);
        kura.reconcile_physical_resource_inventory().unwrap();
        sidecar_physical_assert_actual(&kura);
    }

    #[test]
    fn native_pipeline_public_prepend_retry_keeps_published_index_and_resident_record() {
        let _reset = ResetFaults;
        let (_temporary, kura, hashes) = fixture(2);
        let later = sidecar(2, hashes[1]);
        let earlier = sidecar(1, hashes[0]);
        kura.write_pipeline_metadata(&later);
        let (data, index) = sidecar_physical_test_paths(&kura);
        let data_original = identity(&data);
        let replaced_index = identity(&index);
        let before = queue_records(&kura);
        fail_next_indexed_sidecar_dir_sync_for_tests();
        kura.write_pipeline_metadata(&earlier);
        assert!(kura.native_pipeline_pending());
        assert_eq!(queue_records(&kura), before + 1);
        let published_index = identity(&index);
        assert_ne!(published_index, replaced_index);
        let published_bytes = (fs::read(&data).unwrap(), fs::read(&index).unwrap());
        kura.write_pipeline_metadata(&earlier);
        assert!(!kura.native_pipeline_pending());
        assert_eq!(queue_records(&kura), before);
        assert_eq!(identity(&data), data_original);
        assert_eq!(identity(&index), published_index);
        assert_eq!(
            (fs::read(&data).unwrap(), fs::read(&index).unwrap()),
            published_bytes
        );
        for expected in [&earlier, &later] {
            assert_eq!(
                kura.read_pipeline_metadata(expected.height)
                    .unwrap()
                    .encode_framed()
                    .unwrap(),
                expected.encode_framed().unwrap()
            );
        }
        assert!(!index.with_extension("index.prepend.tmp").exists());
        sidecar_physical_assert_unavailable(&kura);
        kura.reconcile_physical_resource_inventory().unwrap();
        sidecar_physical_assert_actual(&kura);
    }

    #[test]
    fn native_fastpq_public_retry_recovers_one_proof_without_duplicate_payload() {
        let _reset = ResetFaults;
        let (_temporary, kura, hashes) = fixture(1);
        kura.write_pipeline_metadata(&sidecar(1, hashes[0]));
        let (data, index) = sidecar_physical_test_paths(&kura);
        let originals = (identity(&data), identity(&index));
        let snapshot = sample_fastpq_snapshot(1, hashes[0], 8);
        let before = queue_records(&kura);
        fail_next_indexed_sidecar_data_sync_for_tests();
        assert!(matches!(
            kura.write_fastpq_proof_snapshots(
                &[&snapshot],
                &mut kura.sidecar_lock.defer_notifications()
            ),
            FastpqProofWriteResult::Retry
        ));
        assert!(kura.native_pipeline_pending());
        assert_eq!(queue_records(&kura), before + 1);
        let published = (fs::read(&data).unwrap(), fs::read(&index).unwrap());
        assert!(matches!(
            kura.write_fastpq_proof_snapshots(
                &[&snapshot],
                &mut kura.sidecar_lock.defer_notifications()
            ),
            FastpqProofWriteResult::Written
        ));
        assert!(!kura.native_pipeline_pending());
        assert_eq!(queue_records(&kura), before);
        assert_eq!((identity(&data), identity(&index)), originals);
        assert_eq!(
            (fs::read(&data).unwrap(), fs::read(&index).unwrap()),
            published
        );
        assert_eq!(
            kura.read_pipeline_metadata(1).unwrap().fastpq_proofs.len(),
            1
        );
        sidecar_physical_assert_unavailable(&kura);
        kura.reconcile_physical_resource_inventory().unwrap();
        sidecar_physical_assert_actual(&kura);
    }
}
