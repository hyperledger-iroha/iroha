// Current storage identity, geometry and exact native journal controls.
fn published_fixture_geometry_maps(kura: &Kura) -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
    let entries = kura.lane_storage_entries.lock();
    (
        entries
            .values()
            .map(|entry| (entry.lane_id, entry.incarnation))
            .collect(),
        entries
            .values()
            .map(|entry| (entry.lane_id, entry.activation_height))
            .collect(),
    )
}

#[test]
fn lane_reference_publication_adds_exact_instances_and_refuses_physical_retirement() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let lane_count = NonZeroU32::new(4).unwrap();
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "beta".to_owned(),
        ..ModelLaneConfig::default()
    };
    let initial_catalog = LaneCatalog::new(lane_count, vec![lane0.clone(), lane1.clone()]).unwrap();
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let config = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &initial);
    let old = kura
        .lane_storage_entry(lane1.id)
        .expect("actual published lane1 identity");
    let old_blocks = old.blocks_dir(&store_root);
    assert!(old_blocks.is_dir());
    let lane2 = ModelLaneConfig {
        id: LaneId::new(2),
        alias: "gamma".to_owned(),
        ..ModelLaneConfig::default()
    };
    let extended = RuntimeLaneConfig::from_catalog(
        &LaneCatalog::new(
            lane_count,
            vec![lane0.clone(), lane1.clone(), lane2.clone()],
        )
        .unwrap(),
    );
    let (initial_incarnations, initial_activations) = published_fixture_geometry_maps(&kura);
    let mut extended_incarnations = initial_incarnations.clone();
    extended_incarnations.insert(lane2.id, Hash::new(b"explicit lane2 incarnation"));
    let mut extended_activations = initial_activations.clone();
    extended_activations.insert(lane2.id, 0);
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("journal exact lane2 creation");
    kura.mark_lane_geometry_catalog_published(
        &extended,
        &extended_incarnations,
        &extended_activations,
        None,
    )
    .expect("publish exact lane2 reference");
    let added = kura.lane_storage_entry(lane2.id).unwrap();
    let added_blocks = added.blocks_dir(&store_root);
    for name in [INDEX_FILE_NAME, DATA_FILE_NAME, HASHES_FILE_NAME] {
        assert!(
            added_blocks.join(name).is_file(),
            "new lane structure missing {name}"
        );
    }
    assert!(added_blocks.join(COUNT_FILE_NAME).is_file());
    let retired =
        RuntimeLaneConfig::from_catalog(&LaneCatalog::new(lane_count, vec![lane0, lane2]).unwrap());
    let mut retired_incarnations = extended_incarnations.clone();
    retired_incarnations.remove(&lane1.id);
    let mut retired_activations = extended_activations.clone();
    retired_activations.remove(&lane1.id);
    let original_journal = fs::read(store_root.join("lane_geometry_journal.norito")).unwrap();
    let original_marker = fs::read(old_blocks.join(".lane-incarnation.norito")).unwrap();
    kura.apply_lane_geometry_transition(
        &extended,
        &retired,
        &extended_incarnations,
        &retired_incarnations,
        &extended_activations,
        &retired_activations,
        &BTreeSet::new(),
    )
    .expect_err("removal cannot substitute for original native release authority");
    assert_eq!(
        kura.lane_storage_entry(lane1.id).unwrap().identity,
        old.identity
    );
    assert_eq!(
        fs::read(old_blocks.join(".lane-incarnation.norito")).unwrap(),
        original_marker
    );
    assert_eq!(
        fs::read(store_root.join("lane_geometry_journal.norito")).unwrap(),
        original_journal
    );
    kura.restore_published_lane_geometry_for_test(&extended)
        .expect("unchanged reference replay");
    assert_eq!(
        kura.lane_storage_entry(LaneId::new(2)).unwrap().identity,
        added.identity
    );
}

#[test]
fn blank_kura_reference_publication_uses_only_isolated_storage() {
    static CWD_LOCK: std::sync::LazyLock<std::sync::Mutex<()>> =
        std::sync::LazyLock::new(|| std::sync::Mutex::new(()));
    struct WorkingDirGuard(std::path::PathBuf);
    impl Drop for WorkingDirGuard {
        fn drop(&mut self) {
            let _ = std::env::set_current_dir(&self.0);
        }
    }
    let _guard = CWD_LOCK.lock().expect("lock cwd");
    let temp_dir = TempDir::new().expect("create temp dir");
    let original_dir = std::env::current_dir().expect("current dir");
    std::env::set_current_dir(temp_dir.path()).expect("set current dir");
    let _restore_dir = WorkingDirGuard(original_dir);
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::from(1),
        alias: "beta".to_string(),
        ..ModelLaneConfig::default()
    };
    let catalog = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("catalog");
    let lane_config = RuntimeLaneConfig::from_catalog(&catalog);
    let config = kura_config_for_path(Path::new("ignored-relative-fixture-root"), BLOCKS_IN_MEMORY);
    let kura = Kura::new_temporary_with_configured_lane_catalog(&config, &lane_config, &catalog)
        .expect("admit the exact catalog before creating the isolated canonical store");
    kura.bind_lane_storage_network(native_storage_network_id())
        .unwrap();
    let initial = RuntimeLaneConfig::default();
    let initial_incarnations = BTreeMap::from([(LaneId::SINGLE, Hash::new(b"blank-primary"))]);
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        LaneLifecycleParameterV1::catalog_hash(&catalog),
    )
    .expect("publish the initial exact reference before extending it");
    let extended_incarnations = BTreeMap::from([
        (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::new(b"blank-secondary")),
    ]);
    kura.apply_lane_geometry_transition(
        &initial,
        &lane_config,
        &initial_incarnations,
        &extended_incarnations,
        &BTreeMap::from([(LaneId::SINGLE, 0)]),
        &BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]),
        &BTreeSet::new(),
    )
    .expect("publish exact reference under the isolated Kura root");
    assert_eq!(
        kura.lane_storage_entry(LaneId::new(1)).unwrap().incarnation,
        extended_incarnations[&LaneId::new(1)]
    );
    let published = kura.lane_storage_entry(LaneId::new(1)).unwrap();
    assert!(published.blocks_dir(&kura.store_root()).is_dir());
    assert!(
        published
            .blocks_dir(&kura.store_root())
            .join(COUNT_FILE_NAME)
            .is_file()
    );
    assert!(
        !temp_dir.path().join("blocks").exists(),
        "blank Kura must not create lane block directories in the working directory"
    );
    assert!(
        !temp_dir.path().join("merge_ledger").exists(),
        "blank Kura must not create merge-ledger log directories in the working directory"
    );
}

#[test]
fn snapshot_lane_restore_uses_exact_height_and_authenticated_lineage() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let lane0 = ModelLaneConfig::default();
    let stale_config_lane = ModelLaneConfig {
        id: LaneId::new(2),
        alias: "stale-config-lane".to_owned(),
        ..ModelLaneConfig::default()
    };
    let configured_catalog = LaneCatalog::new(
        nonzero!(3_u32),
        vec![lane0.clone(), stale_config_lane.clone()],
    )
    .expect("configured catalog");
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&kura_cfg, &configured)
        .expect("init Kura");
    let restored_lane = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "restored-elastic-lane".to_owned(),
        ..ModelLaneConfig::default()
    };
    let restored_catalog = LaneCatalog::new(
        nonzero!(3_u32),
        vec![lane0, stale_config_lane.clone(), restored_lane.clone()],
    )
    .expect("restored catalog");
    let restored = RuntimeLaneConfig::from_catalog(&restored_catalog);
    let primary_incarnation = Hash::new(b"snapshot restore primary incarnation");
    let stale_incarnation = Hash::new(b"snapshot restore stale incarnation");
    let restored_incarnation = Hash::new(b"snapshot restore active incarnation");
    let configured_incarnations = BTreeMap::from([
        (LaneId::SINGLE, primary_incarnation),
        (stale_config_lane.id, stale_incarnation),
    ]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (stale_config_lane.id, 0)]);
    let configured_lineage_root = Hash::new(b"snapshot restore configured lineage");
    let network_id = native_storage_network_id();
    kura.bind_lane_storage_network(network_id)
        .expect("bind explicit snapshot network");
    let stale_identity = LaneStorageIdentity {
        network_id,
        lane_id: stale_config_lane.id,
        dataspace_id: stale_config_lane.dataspace_id,
        incarnation: stale_incarnation,
        activation_height: 0,
    };
    let stale_dir = stale_identity.blocks_dir(&store_root);
    let baseline = kura
        .lane_geometry_journal_state_for_test()
        .expect("read configured catalog baseline")
        .0
        .expect("configured catalog baseline");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        configured.primary(),
        primary_incarnation,
        baseline,
    )
    .expect("authenticate the configured primary before secondary publication");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &RuntimeLaneConfig::default(),
        &configured,
        &BTreeMap::from([(LaneId::SINGLE, primary_incarnation)]),
        &configured_incarnations,
        &BTreeMap::from([(LaneId::SINGLE, 0)]),
        &configured_activations,
        Hash::new(b"snapshot restore primary lineage"),
        configured_lineage_root,
        &BTreeSet::new(),
        0,
    )
    .expect("publish the initial configured secondary lane");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &configured,
        &configured_incarnations,
        &configured_activations,
        configured_lineage_root,
        Some(baseline),
    )
    .expect("retain the initial configured geometry authority");
    let restored_incarnations = BTreeMap::from([
        (LaneId::SINGLE, primary_incarnation),
        (restored_lane.id, restored_incarnation),
        (stale_config_lane.id, stale_incarnation),
    ]);
    let restored_activations = BTreeMap::from([
        (LaneId::SINGLE, 0),
        (restored_lane.id, 1),
        (stale_config_lane.id, 0),
    ]);
    let restored_lineage_root = Hash::new(b"snapshot restore active lineage");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &configured,
        &restored,
        &configured_incarnations,
        &restored_incarnations,
        &configured_activations,
        &restored_activations,
        configured_lineage_root,
        restored_lineage_root,
        &BTreeSet::new(),
        1,
    )
    .expect("apply authenticated post-snapshot geometry transition");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &restored,
        &restored_incarnations,
        &restored_activations,
        restored_lineage_root,
        None,
    )
    .expect("publish authenticated post-snapshot geometry transition");
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &configured,
        &configured_incarnations,
        &configured_activations,
        0,
        configured_lineage_root,
    )
    .expect("restore exact pre-transition snapshot geometry");
    assert!(
        kura.lane_storage_entry(restored_lane.id).is_err(),
        "a lane introduced after the snapshot must not remain active"
    );
    assert!(
        stale_dir.exists(),
        "snapshot-authoritative lane must be restored"
    );
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &restored,
        &restored_incarnations,
        &restored_activations,
        1,
        restored_lineage_root,
    )
    .expect("restore exact post-transition snapshot geometry");
    let restored_entry = kura
        .lane_storage_entry(restored_lane.id)
        .expect("restored lane must be addressable");
    assert_eq!(restored_entry.lane_id, restored_lane.id);
    assert_eq!(restored_entry.incarnation, restored_incarnation);
    assert_eq!(restored_entry.activation_height, 1);
    assert!(restored_entry.blocks_dir(&store_root).exists());
    assert!(
        kura.lane_storage_entry(stale_config_lane.id).is_ok(),
        "exact additions retain the earlier authenticated lane reference"
    );
    assert!(
        stale_dir.is_dir(),
        "replaying the cursor retains the earlier exact instance"
    );
}

#[test]
fn authenticated_snapshot_lane_restore_rejects_primary_identity_drift_atomically() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let configured_catalog = LaneCatalog::new(nonzero!(1_u32), vec![ModelLaneConfig::default()])
        .expect("configured catalog");
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let kura_cfg = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&kura_cfg, &configured)
        .expect("init Kura");
    let configured_incarnation = Hash::new(b"configured primary restore incarnation");
    let configured_incarnations = BTreeMap::from([(LaneId::SINGLE, configured_incarnation)]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let configured_lineage_root = Hash::new(b"configured primary restore lineage");
    kura.bind_lane_storage_network(native_storage_network_id())
        .unwrap();
    kura.establish_or_verify_configured_primary_geometry_anchor(
        configured.primary(),
        configured_incarnation,
        LaneLifecycleParameterV1::catalog_hash(&configured_catalog),
    )
    .expect("establish exact configured H0 anchor");
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &configured,
        &configured_incarnations,
        &configured_activations,
        0,
        configured_lineage_root,
    )
    .expect("authenticate configured primary geometry");
    let drifted_catalog = LaneCatalog::new(
        nonzero!(1_u32),
        vec![ModelLaneConfig {
            alias: "drifted-primary".to_owned(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("drifted catalog");
    let drifted = RuntimeLaneConfig::from_catalog(&drifted_catalog);
    let drifted_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::new(b"drifted primary incarnation"))]);
    let drifted_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &drifted,
        &drifted_incarnations,
        &drifted_activations,
        0,
        Hash::new(b"drifted primary lineage"),
    )
    .expect_err("primary identity drift must fail closed");
    assert_eq!(
        kura.lane_storage_entry(LaneId::SINGLE)
            .expect("configured primary remains installed")
            .incarnation,
        configured_incarnation
    );
}

#[test]
fn lane_instance_creation_conflict_preserves_storage_and_reference_authority() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let initial_catalog =
        LaneCatalog::new(nonzero!(2_u32), vec![ModelLaneConfig::default()]).unwrap();
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let config = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &initial);
    let (initial_incarnations, initial_activations) = published_fixture_geometry_maps(&kura);
    let added = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "conflict".to_owned(),
        ..ModelLaneConfig::default()
    };
    let extended = RuntimeLaneConfig::from_catalog(
        &LaneCatalog::new(
            nonzero!(2_u32),
            vec![ModelLaneConfig::default(), added.clone()],
        )
        .unwrap(),
    );
    let mut incarnations = initial_incarnations.clone();
    let incarnation = Hash::new(b"occupied exact lane instance");
    incarnations.insert(added.id, incarnation);
    let mut activations = initial_activations.clone();
    activations.insert(added.id, 0);
    let identity = LaneStorageIdentity {
        network_id: kura.lane_storage_entry(LaneId::SINGLE).unwrap().network_id,
        lane_id: added.id,
        dataspace_id: added.dataspace_id,
        incarnation,
        activation_height: 0,
    };
    let conflict = identity.blocks_dir(&store_root);
    fs::create_dir_all(conflict.parent().unwrap()).unwrap();
    fs::write(&conflict, b"foreign-instance-target").unwrap();
    let journal_before = fs::read(store_root.join("lane_geometry_journal.norito"))
        .expect("read authenticated geometry journal");
    let error = kura
        .apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &incarnations,
            &initial_activations,
            &activations,
            &BTreeSet::new(),
        )
        .expect_err("occupied target must fail before a creation intent is published");
    assert!(
        matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidData),
        "wrong target kind must remain an explicit storage error: {error:?}"
    );
    assert_eq!(fs::read(&conflict).unwrap(), b"foreign-instance-target");
    assert!(!store_root.join("merge_ledger").exists());
    assert!(kura.lane_storage_entry(added.id).is_err());
    assert_eq!(
        fs::read(store_root.join("lane_geometry_journal.norito"))
            .expect("read authenticated geometry journal"),
        journal_before
    );
}

#[test]
fn lane_alias_changes_preserve_instance_and_canonical_storage() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let store_root = temp_dir.path().join("kura");
    let catalog = |alias: &str| {
        LaneCatalog::new(
            nonzero!(1_u32),
            vec![ModelLaneConfig {
                alias: alias.to_owned(),
                ..ModelLaneConfig::default()
            }],
        )
        .unwrap()
    };
    let initial = RuntimeLaneConfig::from_catalog(&catalog("Alpha Lane"));
    let updated = RuntimeLaneConfig::from_catalog(&catalog("Payments Lane"));
    let config = kura_config_for_path(&store_root, BLOCKS_IN_MEMORY);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &initial);
    let original = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
    let (incarnations, activations) = published_fixture_geometry_maps(&kura);
    let blocks = original.blocks_dir(&store_root);
    let canonical_blocks = Kura::canonical_storage_path(&kura.store_root);
    assert!(blocks.is_dir());
    let marker_bytes = fs::read(blocks.join(".lane-incarnation.norito")).unwrap();
    let reference_before = fs::read(store_root.join("lane_geometry_journal.norito"))
        .expect("read exact reference journal");
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &incarnations,
        &activations,
        &activations,
        &BTreeSet::new(),
    )
    .expect("alias-only geometry has no storage work");
    kura.mark_lane_geometry_catalog_published(&updated, &incarnations, &activations, None)
        .expect("alias update preserves the same storage reference");
    let current = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
    assert_eq!(current.identity, original.identity);
    assert_eq!(current.blocks_dir(&store_root), blocks);
    assert_eq!(
        fs::read(blocks.join(".lane-incarnation.norito")).unwrap(),
        marker_bytes
    );
    assert_eq!(
        fs::read(store_root.join("lane_geometry_journal.norito"))
            .expect("read exact reference journal"),
        reference_before
    );
    assert_eq!(*kura.active_blocks_dir.lock(), canonical_blocks);
    assert_eq!(kura.block_store.lock().path_to_blockchain, canonical_blocks);

    // A real identity replacement still requires unavailable native release authority.
    let replacement = RuntimeLaneConfig::from_catalog(&catalog("Treasury Lane"));
    let replacement_identity = LaneStorageIdentity {
        incarnation: Hash::new(b"rejected replacement identity"),
        activation_height: 1,
        ..original.identity
    };
    let replacement_blocks = replacement_identity.blocks_dir(&store_root);
    fs::create_dir(&replacement_blocks).unwrap();
    fs::write(replacement_blocks.join("sentinel"), b"foreign replacement").unwrap();
    let replacement_incarnations =
        BTreeMap::from([(LaneId::SINGLE, replacement_identity.incarnation)]);
    let replacement_activations = BTreeMap::from([(LaneId::SINGLE, 1)]);
    let error = kura
        .apply_lane_geometry_transition_at_height(
            &updated,
            &replacement,
            &incarnations,
            &replacement_incarnations,
            &activations,
            &replacement_activations,
            &BTreeSet::from([LaneId::SINGLE]),
            1,
        )
        .expect_err("occupied replacement cannot be adopted or overwrite the old instance");
    assert!(
        matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidInput
        && source.to_string().contains("native geometry currently permits exact additions only")),
        "replacement must be refused before physical effects: {error:?}"
    );
    assert_eq!(
        kura.lane_storage_entry(LaneId::SINGLE).unwrap().identity,
        original.identity
    );
    assert_eq!(
        fs::read(replacement_blocks.join("sentinel")).unwrap(),
        b"foreign replacement"
    );
    assert!(!store_root.join("merge_ledger").exists());
    assert_eq!(
        fs::read(blocks.join(".lane-incarnation.norito")).unwrap(),
        marker_bytes
    );
    assert_eq!(
        fs::read(store_root.join("lane_geometry_journal.norito"))
            .expect("read exact reference journal"),
        reference_before
    );
    assert_eq!(*kura.active_blocks_dir.lock(), canonical_blocks);
    assert_eq!(kura.block_store.lock().path_to_blockchain, canonical_blocks);
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
}

#[test]
fn block_bytes_returns_memory_mapped_slice() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut store = new_block_store(&temp_dir);
    store
        .create_files_if_they_do_not_exist()
        .expect("initialise store files");
    let payload = b"test block payload";
    store
        .write_block_data(0, payload.as_ref())
        .expect("write payload");
    let (slice_ptr, slice_len) = {
        let slice = store
            .block_bytes(0, payload.len() as u64)
            .expect("read payload");
        assert_eq!(slice, payload);
        (slice.as_ptr(), slice.len())
    };
    let mirror = store
        .data_mmap
        .as_ref()
        .expect("mirror should be initialised after block_bytes()");
    assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
    assert_eq!(mirror.len(), payload.len());
    let mirror_slice = mirror.slice(0, mirror.len());
    assert_eq!(mirror_slice, payload);
    assert_eq!(slice_len, payload.len());
    assert_eq!(slice_ptr, mirror_slice.as_ptr());
    assert_eq!(store.data_mmap_len, payload.len() as u64);
}

#[test]
fn memory_mirror_updates_after_appending_data() {
    let temp_dir = TempDir::new().expect("create temp dir");
    let mut store = new_block_store(&temp_dir);
    store
        .create_files_if_they_do_not_exist()
        .expect("initialise store files");
    let initial = b"initial payload";
    store
        .write_block_data(0, initial.as_ref())
        .expect("write initial payload");
    {
        let slice = store
            .block_bytes(0, initial.len() as u64)
            .expect("prime mirror with initial payload");
        assert_eq!(slice, initial);
    }
    let expected_initial_len = initial.len();
    let mirror = store
        .data_mmap
        .as_ref()
        .expect("mirror initialised after first read");
    assert_eq!(mirror.len(), expected_initial_len);
    assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
    let appended = b" appended payload";
    store
        .write_block_data(initial.len() as u64, appended.as_ref())
        .expect("append payload");
    let total_len = (initial.len() + appended.len()) as u64;
    let combined = {
        let slice = store
            .block_bytes(0, total_len)
            .expect("read combined payload");
        assert_eq!(slice.len(), initial.len() + appended.len());
        slice.to_vec()
    };
    let mirror = store
        .data_mmap
        .as_ref()
        .expect("mirror should be remapped after append");
    assert_eq!(mirror.kind(), MemoryMirrorKind::MemoryMapped);
    assert_eq!(mirror.len(), initial.len() + appended.len());
    let mut expected = Vec::with_capacity(initial.len() + appended.len());
    expected.extend_from_slice(initial);
    expected.extend_from_slice(appended);
    assert_eq!(mirror.slice(0, mirror.len()), expected.as_slice());
    assert_eq!(combined, expected);
    assert_eq!(store.data_mmap_len, total_len);
}

fn indices<const N: usize>(value: [(u64, u64); N]) -> [BlockIndex; N] {
    let mut ret = [BlockIndex {
        start: 0,
        length: 0,
    }; N];
    for idx in 0..value.len() {
        ret[idx] = value[idx].into();
    }
    ret
}

fn wait_for_block_hash(kura: &Arc<Kura>, height: usize, expected: HashOf<BlockHeader>) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let target_index = height
        .checked_sub(1)
        .expect("block height should be non-zero");
    loop {
        {
            let mut store = kura.block_store.lock();
            if let Ok(count) = store.read_index_count() {
                if count > target_index as u64 {
                    if let Ok(hashes) = store.read_block_hashes(target_index as u64, 1) {
                        if hashes.first().copied() == Some(expected) {
                            return;
                        }
                    }
                }
            }
        }
        let now = Instant::now();
        assert!(
            now < deadline,
            "Timed out waiting for block {height} to persist"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

fn primary_blocks_dir(dir: &TempDir) -> PathBuf {
    let blocks_dir = Kura::canonical_storage_path(dir.path());
    std::fs::create_dir_all(&blocks_dir).unwrap();
    blocks_dir
}

fn new_block_store(dir: &TempDir) -> BlockStore {
    let blocks_dir = primary_blocks_dir(dir);
    BlockStore::new(&blocks_dir)
}

fn kura_config_for_path(path: &Path, blocks_in_memory: NonZeroUsize) -> KuraConfig {
    KuraConfig {
        init_mode: iroha_config::kura::InitMode::Strict,
        store_dir: WithOrigin::inline(path.to_path_buf()),
        max_disk_usage_bytes: iroha_config::parameters::defaults::kura::MAX_DISK_USAGE_BYTES,
        blocks_in_memory,
        debug_output_new_blocks: false,
        fsync_mode: FsyncMode::Batched,
        fsync_interval: FSYNC_INTERVAL,
        lane_history_retention: LANE_HISTORY_RETENTION,
        native_context_archive_max_bytes:
            iroha_config::parameters::defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES,
        block_hash_history_bytes:
            iroha_config::parameters::defaults::kura::BLOCK_HASH_HISTORY_BYTES,
        transaction_history_bytes:
            iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES,
        membership_storage: iroha_config::parameters::defaults::kura::MEMBERSHIP_STORAGE_POLICY,
        fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    }
}

fn kura_config_for_dir(dir: &TempDir, blocks_in_memory: NonZeroUsize) -> KuraConfig {
    kura_config_for_path(dir.path(), blocks_in_memory)
}

#[test]
fn native_context_archive_limit_is_retained_from_actual_kura_configuration() {
    let (_directory, mut config) =
        kura_storage_fixture("native context archive bound", BLOCKS_IN_MEMORY);
    config.native_context_archive_max_bytes = nonzero!(12_345_usize);
    let (kura, _) = test_kura_with_default_lane_markers(&config, &RuntimeLaneConfig::default());
    assert_eq!(
        kura.native_context_archive_max_bytes(),
        config.native_context_archive_max_bytes
    );
    assert_eq!(
        Kura::blank_kura_for_testing().native_context_archive_max_bytes(),
        iroha_config::parameters::defaults::kura::NATIVE_CONTEXT_ARCHIVE_MAX_BYTES
    );
}

impl PartialEq for BlockIndex {
    fn eq(&self, other: &Self) -> bool {
        self.start == other.start && self.length == other.length
    }
}
impl PartialEq<(u64, u64)> for BlockIndex {
    fn eq(&self, other: &(u64, u64)) -> bool {
        self.start == other.0 && self.length == other.1
    }
}
impl From<(u64, u64)> for BlockIndex {
    fn from(value: (u64, u64)) -> Self {
        Self {
            start: value.0,
            length: value.1,
        }
    }
}

#[test]
fn occupied_native_frame_rejects_certificate_substitution_without_journal_mutation() {
    let mut chain = CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000))
        .expect("execute native genesis");
    chain.commit(Vec::new());
    let kura = chain.kura();
    let original = Arc::clone(chain.committed(2).block());
    let original_wire = original.encode_wire().unwrap();
    let journal_image = || {
        let store = kura.block_store.lock();
        [
            DATA_FILE_NAME,
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ]
        .map(|name| fs::read(store.path_to_blockchain.join(name)).unwrap())
    };
    let before = journal_image();
    let certificate = original
        .commit_certificate()
        .expect("original native certificate");
    let mut substituted_qc = certificate.commit_qc().to_vec();
    assert!(!substituted_qc.is_empty());
    substituted_qc[0] ^= 1;
    let substituted = original.as_ref().clone().with_commit_certificate(Some(
        iroha_data_model::block::CommitCertificate::from_untrusted_parts(
            certificate.consensus_header().to_vec(),
            substituted_qc,
            certificate.result_preimage().to_vec(),
        ),
    ));
    assert_eq!(
        substituted.hash(),
        original.hash(),
        "the signed body is unchanged"
    );
    assert_ne!(substituted.encode_wire().unwrap(), original_wire);
    assert!(matches!(
        kura.store_block(substituted),
        Err(Error::CanonicalBlockWireMismatch { height: 2 })
    ));
    assert_eq!(journal_image(), before);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 2);
    assert_eq!(
        kura.canonical_block_wire_bytes_for_testing(nonzero!(2_usize))
            .unwrap(),
        original_wire
    );
    kura.store_block(Arc::clone(&original))
        .expect("exact original retry remains available");
    assert_eq!(journal_image(), before);
    assert!(!kura.canonical_storage_poisoned.load(Ordering::Acquire));
}

#[test]
fn native_height_gap_refusal_preserves_journals_and_contiguous_retry() {
    let frames = native_storage_frames(3);
    let kura = Kura::blank_kura_for_testing();
    kura.store_block(Arc::clone(&frames[0])).unwrap();
    let journal_image = || {
        let store = kura.block_store.lock();
        [
            DATA_FILE_NAME,
            INDEX_FILE_NAME,
            HASHES_FILE_NAME,
            COUNT_FILE_NAME,
        ]
        .map(|name| fs::read(store.path_to_blockchain.join(name)).unwrap())
    };
    let before = journal_image();
    assert!(matches!(
        kura.store_block(Arc::clone(&frames[2])),
        Err(Error::BlockHeightGap {
            expected_next_height: 2,
            actual_height: 3
        })
    ));
    assert_eq!(journal_image(), before);
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
    for frame in &frames[1..] {
        kura.store_block(Arc::clone(frame)).unwrap();
    }
    assert_eq!(kura.exact_durable_blocks_count().unwrap(), 3);
    for (offset, frame) in frames.iter().enumerate() {
        assert_eq!(
            kura.canonical_block_wire_bytes_for_testing(NonZeroUsize::new(offset + 1).unwrap())
                .unwrap(),
            frame.encode_wire().unwrap()
        );
    }
}
