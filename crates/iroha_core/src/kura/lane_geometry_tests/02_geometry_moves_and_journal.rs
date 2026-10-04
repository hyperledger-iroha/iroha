struct GeometryReferenceResumeGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for GeometryReferenceResumeGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}

fn authenticate_transition_fixture_primary(
    kura: &Kura,
    config: &RuntimeLaneConfig,
    incarnations: &BTreeMap<LaneId, Hash>,
) {
    kura.establish_or_verify_configured_primary_geometry_anchor(
        config.primary(),
        incarnations[&LaneId::SINGLE],
        kura.configured_lane_catalog_baseline()
            .expect("fixture baseline read")
            .expect("fixture configured catalog is authenticated"),
    )
    .expect("authenticate exact fixture H0 instance before reference transitions");
}

#[test]
fn recovery_completes_journal_owned_staging_created_before_marker() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    let previous_bindings = kura
        .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
        .expect("initial bindings");
    let updated_bindings = kura
        .geometry_bindings(&extended, &extended_incarnations, &extended_activations)
        .expect("extended bindings");
    let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
    let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
    let previous_lineage_root = unscoped_lineage_root(&previous_bindings);
    let updated_lineage_root = unscoped_lineage_root(&updated_bindings);
    let transition_id = geometry_transition_id(
        0,
        0,
        previous_catalog,
        previous_lineage_root,
        updated_catalog,
        updated_lineage_root,
    );
    let operations = kura
        .build_geometry_operations(
            transition_id,
            &previous_bindings,
            &updated_bindings,
            &BTreeSet::new(),
        )
        .expect("create operation");
    let intent = LaneGeometryIntent {
        transition_id,
        transition_sequence: 0,
        transition_height: 0,
        previous_catalog,
        previous_lineage_root,
        updated_catalog,
        updated_lineage_root,
        previous_bindings,
        updated_bindings,
        phase: LaneGeometryPhase::Intent,
        operations,
    };
    let mut journal = kura
        .read_lane_geometry_journal()
        .expect("authenticated baseline");
    journal.records.push(intent);
    kura.write_lane_geometry_journal(&journal)
        .expect("persist create intent before provisioning");
    let operation = &journal.records[0].operations[0];
    let staged_blocks = kura
        .resolve_relative_path(&operation.created.blocks_path)
        .expect("staged blocks path");
    fs::create_dir_all(&staged_blocks)
        .expect("simulate crash after creating the journal-owned staging directory");
    assert!(!staged_blocks.join(MARKER_FILE_NAME).exists());
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect(
            "recovery must finish the journal-owned immutable pair before publishing its reference",
        );
    let lane = extended.entry(LaneId::new(1)).expect("created lane");
    let blocks =
        geometry_fixture_blocks(&kura, lane, &extended_incarnations, &extended_activations);
    assert_eq!(staged_blocks, blocks);
    assert!(blocks.join(MARKER_FILE_NAME).is_file());
    assert!(blocks.join(MARKER_FILE_NAME).is_file());
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[0].phase,
        LaneGeometryPhase::CatalogPublished
    );
}

#[test]
fn recovery_distinguishes_repeated_catalogs_by_retained_lineage_root() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (incarnations, activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    let first = Hash::new(b"first original lineage");
    let second = Hash::new(b"second original lineage");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &initial,
        &extended,
        &initial_incarnations,
        &incarnations,
        &initial_activations,
        &activations,
        first,
        second,
        &BTreeSet::new(),
        9,
    )
    .unwrap();
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    assert!(
        kura.mark_lane_geometry_catalog_published_with_lineage_root(
            &extended,
            &incarnations,
            &activations,
            Hash::new(b"foreign lineage"),
            None
        )
        .is_err()
    );
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &extended,
        &incarnations,
        &activations,
        second,
        None,
    )
    .unwrap();
    let published = fs::read(kura.lane_geometry_journal_path()).unwrap();
    assert!(
        kura.recover_lane_geometry_journal_at_height_with_lineage_root(
            &extended,
            &incarnations,
            &activations,
            9,
            first
        )
        .is_err()
    );
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).unwrap(),
        published
    );
    kura.recover_lane_geometry_journal_at_height_with_lineage_root(
        &extended,
        &incarnations,
        &activations,
        9,
        second,
    )
    .unwrap();
    let binding = kura
        .geometry_binding(
            extended.entry(LaneId::new(1)).unwrap(),
            &incarnations,
            &activations,
        )
        .unwrap();
    assert_eq!(
        kura.lane_storage_entry(LaneId::new(1)).unwrap().identity,
        binding.identity()
    );
}

#[test]
fn files_applied_phase_rolls_forward_when_catalog_is_already_authoritative() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("prepare transition");
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[0].phase,
        LaneGeometryPhase::FilesApplied
    );
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect("recover post-catalog crash");
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[0].phase,
        LaneGeometryPhase::CatalogPublished
    );
}

#[test]
fn primary_alias_update_and_restart_preserve_exact_instance_and_chain() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let initial_catalog = configured_primary_catalog("primary-alpha");
    let updated_catalog = configured_primary_catalog("primary-beta");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let updated = RuntimeLaneConfig::from_catalog(&updated_catalog);
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    // Bind the actual configured primary before retaining geometry or bodies for restart.
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        incarnations[&LaneId::SINGLE],
        kura.configured_lane_catalog_baseline()
            .expect("read authenticated configured catalog")
            .expect("configured catalog was admitted at open"),
    )
    .expect("anchor configured primary before restart fixture writes");
    let _ = store_structural_geometry_chain(&kura, 3);
    let expected_hashes = (1..=3)
        .map(|height| {
            kura.get_durable_block_hash(NonZeroUsize::new(height).expect("non-zero"))
                .expect("durable block hash")
        })
        .collect::<Vec<_>>();
    let blocks = geometry_fixture_blocks(&kura, initial.primary(), &incarnations, &activations);
    assert_eq!(
        blocks,
        geometry_fixture_blocks(&kura, updated.primary(), &incarnations, &activations)
    );
    let marker_before = fs::read(blocks.join(MARKER_FILE_NAME)).unwrap();
    let journal_before = kura.read_lane_geometry_journal().unwrap();
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &incarnations,
        &activations,
        &activations,
        &BTreeSet::new(),
    )
    .expect("alias metadata cannot move an instance");
    assert_eq!(kura.read_lane_geometry_journal().unwrap(), journal_before);
    assert_eq!(
        fs::read(blocks.join(MARKER_FILE_NAME)).unwrap(),
        marker_before
    );
    assert!(blocks.is_dir() && blocks.join(MARKER_FILE_NAME).is_file());
    drop(kura);
    let reopened = open_kura(&root, &initial);
    assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 3);
    assert_eq!(
        *reopened.active_blocks_dir.lock(),
        Kura::canonical_storage_path(&root)
    );
    for (height, expected) in (1..=3).zip(expected_hashes) {
        assert_eq!(
            reopened.get_durable_block_hash(NonZeroUsize::new(height).unwrap()),
            Some(expected)
        );
    }
    for catalog in [&updated, &initial, &initial] {
        reopened
            .recover_lane_geometry_journal(catalog, &incarnations, &activations)
            .expect("both alias projections resolve the same exact instance");
        assert!(blocks.is_dir() && blocks.join(MARKER_FILE_NAME).is_file());
        assert_eq!(
            fs::read(blocks.join(MARKER_FILE_NAME)).unwrap(),
            marker_before
        );
        assert_eq!(
            *reopened.active_blocks_dir.lock(),
            Kura::canonical_storage_path(&root)
        );
        assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 3);
    }
    assert_eq!(
        reopened.read_lane_geometry_journal().unwrap(),
        journal_before
    );
}

#[test]
fn two_lane_alias_update_and_restart_preserve_exact_instances_and_chain() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = nonzero!(2_u32);
    let initial_primary = ModelLaneConfig {
        alias: "primary-alpha".to_owned(),
        ..ModelLaneConfig::default()
    };
    let initial_secondary = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "secondary-alpha".to_owned(),
        ..ModelLaneConfig::default()
    };
    let updated_primary = ModelLaneConfig {
        alias: "primary-beta".to_owned(),
        ..initial_primary.clone()
    };
    let updated_secondary = ModelLaneConfig {
        alias: "secondary-beta".to_owned(),
        ..initial_secondary.clone()
    };
    let base_catalog = LaneCatalog::new(lane_count, vec![initial_primary.clone()]).unwrap();
    let base = RuntimeLaneConfig::from_catalog(&base_catalog);
    let initial_catalog = LaneCatalog::new(lane_count, vec![initial_primary, initial_secondary])
        .expect("initial two-lane catalog");
    let updated_catalog = LaneCatalog::new(lane_count, vec![updated_primary, updated_secondary])
        .expect("relabelled two-lane catalog");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let updated = RuntimeLaneConfig::from_catalog(&updated_catalog);
    let incarnations = BTreeMap::from([
        (LaneId::SINGLE, Hash::prehashed([0x31; Hash::LENGTH])),
        (LaneId::new(1), Hash::prehashed([0x32; Hash::LENGTH])),
    ]);
    let activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
    let kura = open_kura(&root, &initial);
    // Bind the actual configured primary before retaining geometry or bodies for restart.
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        incarnations[&LaneId::SINGLE],
        kura.configured_lane_catalog_baseline()
            .expect("read authenticated configured catalog")
            .expect("configured catalog was admitted at open"),
    )
    .expect("anchor configured primary before restart fixture writes");
    kura.apply_lane_geometry_transition_at_height(
        &base,
        &initial,
        &BTreeMap::from([(LaneId::SINGLE, incarnations[&LaneId::SINGLE])]),
        &incarnations,
        &BTreeMap::from([(LaneId::SINGLE, 0)]),
        &activations,
        &BTreeSet::new(),
        0,
    )
    .expect("journal the configured secondary instance");
    kura.mark_lane_geometry_catalog_published(&initial, &incarnations, &activations, None)
        .expect("publish the exact configured instance references");
    let _ = store_structural_geometry_chain(&kura, 3);
    let history_budget = iroha_allocation::AllocationBudget::new(64 * 1024 * 1024);
    let exact_chain = |kura: &Kura| {
        (1..=kura.exact_durable_blocks_count().unwrap())
            .map(|height| {
                kura.get_block(
                    NonZeroUsize::new(height).expect("non-zero block height"),
                    &history_budget,
                )
                .expect("funded offline fixture read")
                .expect("durable block")
                .encode_wire()
                .expect("encode canonical block wire")
            })
            .collect::<Vec<_>>()
    };
    let expected_chain = exact_chain(&kura);
    let initial_bindings = kura
        .geometry_bindings(&initial, &incarnations, &activations)
        .unwrap();
    let updated_bindings = kura
        .geometry_bindings(&updated, &incarnations, &activations)
        .unwrap();
    assert_eq!(initial_bindings, updated_bindings);
    let paths = initial_bindings
        .iter()
        .map(|binding| kura.binding_blocks_path(binding))
        .collect::<Vec<_>>();
    let secondary = initial_bindings
        .iter()
        .find(|binding| binding.lane_id == LaneId::new(1))
        .unwrap();
    let secondary_blocks = kura.binding_blocks_path(secondary);
    let sentinel = secondary_blocks.join(MARKER_FILE_NAME);
    let expected_instance_marker = fs::read(&sentinel).unwrap();
    let journal_before = kura.read_lane_geometry_journal().unwrap();
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &incarnations,
        &activations,
        &activations,
        &BTreeSet::new(),
    )
    .expect("two alias updates share the exact existing objects");
    assert_eq!(kura.read_lane_geometry_journal().unwrap(), journal_before);
    assert_eq!(exact_chain(&kura), expected_chain);
    assert!(paths.iter().all(|path| path.exists()));
    drop(kura);
    let reopened = open_kura(&root, &initial);
    for catalog in [&updated, &initial, &initial] {
        reopened
            .recover_lane_geometry_journal(catalog, &incarnations, &activations)
            .expect("alias-independent reference restoration is idempotent");
        assert_eq!(
            *reopened.active_blocks_dir.lock(),
            Kura::canonical_storage_path(&root)
        );
        assert!(paths.iter().all(|path| path.exists()));
        assert_eq!(fs::read(&sentinel).unwrap(), expected_instance_marker);
        assert_eq!(
            reopened.exact_durable_blocks_count().unwrap(),
            expected_chain.len()
        );
        assert_eq!(exact_chain(&reopened), expected_chain);
        assert_eq!(
            reopened.read_lane_geometry_journal().unwrap(),
            journal_before
        );
    }
}

#[test]
fn reference_publication_does_not_lock_canonical_block_store() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, updated) = initial_and_extended_configs();
    let (incarnations, activations) = initial_geometry();
    let (updated_incarnations, updated_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &incarnations);
    let _ = store_structural_geometry_chain(&kura, 1);
    let expected = kura
        .get_durable_block_hash(nonzero!(1_usize))
        .expect("durable block hash");
    kura.block_data.lock()[0].1 = None;
    kura.pause_geometry_reference_publication
        .store(true, std::sync::atomic::Ordering::Release);
    thread::scope(|scope| {
        let transition = scope.spawn(|| {
            kura.apply_lane_geometry_transition(
                &initial,
                &updated,
                &incarnations,
                &updated_incarnations,
                &activations,
                &updated_activations,
                &BTreeSet::new(),
            )
        });
        let resume_guard =
            GeometryReferenceResumeGuard(&kura.geometry_reference_publication_paused);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !kura
            .geometry_reference_publication_paused
            .load(std::sync::atomic::Ordering::Acquire)
        {
            assert!(
                Instant::now() < deadline,
                "reference publication did not pause"
            );
            thread::yield_now();
        }
        assert!(
            kura.block_store.try_lock().is_some(),
            "reference publication must not retain the independent canonical BlockStore guard"
        );
        let canonical_path = kura.block_store.lock().path_to_blockchain.clone();
        assert_eq!(
            canonical_path,
            Kura::canonical_storage_path(&kura.store_root)
        );
        let mut reader =
            BlockStore::open_read_only(&canonical_path).expect("independent canonical reader");
        let index = reader
            .read_block_index(0)
            .expect("canonical index during reference publication");
        assert!(index.length > 0);
        drop(resume_guard);
        transition
            .join()
            .expect("transition thread")
            .expect("journaled reference publication");
        let block = kura
            .get_block(
                nonzero!(1_usize),
                &iroha_allocation::AllocationBudget::new(64 * 1024 * 1024),
            )
            .expect("funded offline fixture read")
            .expect("canonical block remains readable");
        assert_eq!(block.hash(), expected);
    });
}

#[test]
fn lane_geometry_recovery_holds_sidecar_lock() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, updated) = initial_and_extended_configs();
    let (incarnations, activations) = initial_geometry();
    let (updated_incarnations, updated_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &incarnations);
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &updated_incarnations,
        &activations,
        &updated_activations,
        &BTreeSet::new(),
    )
    .expect("apply reference publication");
    kura.pause_geometry_reference_publication
        .store(true, std::sync::atomic::Ordering::Release);
    thread::scope(|scope| {
        let recovery = scope
            .spawn(|| kura.recover_lane_geometry_journal(&initial, &incarnations, &activations));
        let resume_guard =
            GeometryReferenceResumeGuard(&kura.geometry_reference_publication_paused);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !kura
            .geometry_reference_publication_paused
            .load(std::sync::atomic::Ordering::Acquire)
        {
            assert!(Instant::now() < deadline, "lane recovery did not pause");
            thread::yield_now();
        }
        assert!(
            kura.sidecar_lock.try_lock().is_none(),
            "runtime geometry recovery must exclude lane sidecar I/O"
        );
        drop(resume_guard);
        recovery
            .join()
            .expect("recovery thread")
            .expect("recover reference publication");
    });
}

#[test]
fn recovery_publishes_uncertain_boundary_before_rolling_tail_forward() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = nonzero!(3_u32);
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "frontier-one".to_owned(),
        ..ModelLaneConfig::default()
    };
    let lane2 = ModelLaneConfig {
        id: LaneId::new(2),
        alias: "frontier-two".to_owned(),
        ..ModelLaneConfig::default()
    };
    let base_catalog = LaneCatalog::new(lane_count, vec![lane0.clone()]).expect("base catalog");
    let one_catalog = LaneCatalog::new(lane_count, vec![lane0.clone(), lane1.clone()])
        .expect("one-lane extension");
    let two_catalog =
        LaneCatalog::new(lane_count, vec![lane0, lane1, lane2]).expect("two-lane extension");
    let base = RuntimeLaneConfig::from_catalog(&base_catalog);
    let one = RuntimeLaneConfig::from_catalog(&one_catalog);
    let two = RuntimeLaneConfig::from_catalog(&two_catalog);
    let base_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x41; Hash::LENGTH]))]);
    let one_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x42; Hash::LENGTH])),
    ]);
    let two_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), one_incarnations[&LaneId::new(1)]),
        (LaneId::new(2), Hash::prehashed([0x43; Hash::LENGTH])),
    ]);
    let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let one_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 6)]);
    let two_activations = BTreeMap::from([
        (LaneId::SINGLE, 0),
        (LaneId::new(1), 6),
        (LaneId::new(2), 7),
    ]);
    let kura = open_kura(&root, &base);
    authenticate_transition_fixture_primary(&kura, &base, &base_incarnations);
    kura.apply_lane_geometry_transition(
        &base,
        &one,
        &base_incarnations,
        &one_incarnations,
        &base_activations,
        &one_activations,
        &BTreeSet::new(),
    )
    .expect("apply first transition");
    kura.mark_lane_geometry_catalog_published(&one, &one_incarnations, &one_activations, None)
        .expect("publish first transition");
    kura.apply_lane_geometry_transition(
        &one,
        &two,
        &one_incarnations,
        &two_incarnations,
        &one_activations,
        &two_activations,
        &BTreeSet::new(),
    )
    .expect("apply second transition");
    kura.mark_lane_geometry_catalog_published(&two, &two_incarnations, &two_activations, None)
        .expect("publish second transition");
    let mut journal = kura
        .read_lane_geometry_journal()
        .expect("published journal");
    kura.apply_geometry_operations_rollback(
        &journal.records[1].operations,
        GeometryEvidencePolicy::RequireDurableEvidence,
    )
    .expect("place second transition behind the physical frontier");
    journal.records[0].phase = LaneGeometryPhase::FilesApplied;
    journal.records[1].phase = LaneGeometryPhase::RolledBack;
    kura.write_lane_geometry_journal(&journal)
        .expect("persist valid uncertain-plus-rolled-back frontier");
    kura.recover_lane_geometry_journal(&two, &two_incarnations, &two_activations)
        .expect("recovery must publish the uncertain boundary before the tail");
    let recovered = kura
        .read_lane_geometry_journal()
        .expect("recovered journal");
    assert_eq!(
        recovered
            .records
            .iter()
            .map(|record| record.phase)
            .collect::<Vec<_>>(),
        vec![
            LaneGeometryPhase::CatalogPublished,
            LaneGeometryPhase::CatalogPublished,
        ]
    );
    assert!(
        geometry_fixture_blocks(
            &kura,
            two.entry(LaneId::new(2)).expect("lane two"),
            &two_incarnations,
            &two_activations
        )
        .is_dir()
    );
}

#[test]
fn recovery_rejects_stale_incarnation_marker() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("prepare transition");
    let binding = kura
        .geometry_binding(
            extended.entry(LaneId::new(1)).unwrap(),
            &extended_incarnations,
            &extended_activations,
        )
        .unwrap();
    let marker_path = kura.binding_blocks_path(&binding).join(MARKER_FILE_NAME);
    let mut stale =
        decode_exact::<LaneIncarnationMarker>(&fs::read(&marker_path).unwrap()).unwrap();
    stale.incarnation = Hash::prehashed([0x77; Hash::LENGTH]);
    let stale_bytes = stale.encode();
    fs::write(&marker_path, &stale_bytes)
        .expect("inject foreign incarnation at the actual owned path");
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("stale incarnation marker must fail closed");
    assert_eq!(fs::read(marker_path).unwrap(), stale_bytes);
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
}

#[test]
fn transition_rejects_occupied_instance_before_intent_publication() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    let lane = extended.entry(LaneId::new(1)).unwrap();
    let collision =
        geometry_fixture_blocks(&kura, lane, &extended_incarnations, &extended_activations);
    fs::create_dir_all(&collision).expect("seed unowned exact-instance collision");
    let sentinel = collision.join("operator-owned");
    fs::write(&sentinel, b"must not adopt or overwrite").unwrap();
    let journal_before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect_err("unowned instance collision must fail before publishing an intent");
    assert!(!collision.join(MARKER_FILE_NAME).exists());
    assert_eq!(fs::read(sentinel).unwrap(), b"must not adopt or overwrite");
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).unwrap(),
        journal_before
    );
    assert!(
        kura.read_lane_geometry_journal()
            .expect("journal remains readable")
            .records
            .is_empty()
    );
}

#[cfg(unix)]
#[test]
fn transition_rejects_symlink_lane_target() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside");
    fs::create_dir_all(&outside).expect("outside directory");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    let target = geometry_fixture_blocks(
        &kura,
        extended.entry(LaneId::new(1)).expect("lane one"),
        &extended_incarnations,
        &extended_activations,
    );
    fs::create_dir_all(target.parent().expect("target parent")).expect("target parent");
    symlink(&outside, &target).expect("seed symlink target");
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect_err("symlink target must fail closed");
    assert!(
        outside
            .read_dir()
            .expect("outside remains readable")
            .next()
            .is_none()
    );
}

#[test]
fn configured_primary_replay_preflight_checks_durable_binding_without_history() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("replay-binding");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open authenticated configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("explicit fixture network");
    let durable_incarnation = Hash::prehashed([0x61; Hash::LENGTH]);
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        durable_incarnation,
        baseline,
    )
    .expect("bind configured primary");
    let activation_heights = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let durable_incarnations = BTreeMap::from([(LaneId::SINGLE, durable_incarnation)]);
    let journal_path = kura.lane_geometry_journal_path();
    let journal = kura
        .read_lane_geometry_journal()
        .expect("read binding-only geometry journal");
    assert!(journal.records.is_empty());
    assert!(journal.configured_primary_binding.is_some());
    let journal_before = fs::read(&journal_path).expect("binding-only journal bytes");
    let mismatched_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x62; Hash::LENGTH]))]);
    let mismatched_bindings = kura
        .geometry_bindings(&lane_config, &mismatched_incarnations, &activation_heights)
        .expect("mismatched replay bindings");
    let error = kura
        .preflight_lane_geometry_recovery_floor_with_lineage_root(
            &lane_config,
            &mismatched_incarnations,
            &activation_heights,
            unscoped_lineage_root(&mismatched_bindings),
        )
        .expect_err("durable configured-primary binding must fail closed");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "configured-primary geometry binding differs from its durable anchor",
    );
    assert_eq!(
        fs::read(&journal_path).expect("journal after rejected binding preflight"),
        journal_before,
        "binding mismatch preflight must not rewrite the journal"
    );
    let durable_bindings = kura
        .geometry_bindings(&lane_config, &durable_incarnations, &activation_heights)
        .expect("durable replay bindings");
    kura.preflight_lane_geometry_recovery_floor_with_lineage_root(
        &lane_config,
        &durable_incarnations,
        &activation_heights,
        unscoped_lineage_root(&durable_bindings),
    )
    .expect("matching durable configured-primary binding remains replayable");
    assert_eq!(
        fs::read(&journal_path).expect("journal after matching binding preflight"),
        journal_before,
        "successful binding preflight must also remain read-only"
    );
}

#[test]
fn journal_publication_forces_a_paused_usage_scan_to_retry_exactly() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let (initial_incarnations, initial_activations) = initial_geometry();
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    let (extended_incarnations, extended_activations) = extended_geometry();
    kura.refresh_disk_usage_bytes()
        .expect("establish exact usage baseline");
    kura.pause_next_total_disk_usage_scan_after_scan_for_tests();
    let scan_kura = Arc::clone(&kura);
    let (scan_tx, scan_rx) = mpsc::channel();
    let scan = thread::spawn(move || {
        scan_tx
            .send(scan_kura.refresh_disk_usage_bytes())
            .expect("report usage scan result");
    });
    wait_for_total_usage_scan_pause(&kura);
    let publication = kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    );
    let remained_paused = matches!(
        scan_rx.recv_timeout(Duration::from_millis(50)),
        Err(mpsc::RecvTimeoutError::Timeout)
    );
    kura.resume_total_disk_usage_scan_for_tests();
    let refreshed = scan_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("paused usage scan must finish after release")
        .expect("retried usage scan succeeds");
    scan.join().expect("join paused usage scan");
    publication.expect("publish a real lane-geometry journal transition");
    assert!(
        remained_paused,
        "the deterministic scan barrier must remain active through publication"
    );
    assert!(
        !kura
            .read_lane_geometry_journal()
            .expect("published lane-geometry journal")
            .records
            .is_empty(),
        "the race must exercise a non-empty journal publication"
    );
    let exact_enforced = kura
        .kura_disk_usage_bytes()
        .expect("exact enforced usage after journal publication");
    let exact_total = kura
        .kura_total_disk_usage_bytes()
        .expect("exact total usage after journal publication");
    assert_eq!(refreshed, exact_enforced);
    assert_eq!(
        kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed),
        exact_enforced,
        "a scan spanning journal publication must retry before updating enforced usage"
    );
    assert_eq!(
        kura.disk_usage_total
            .load(std::sync::atomic::Ordering::Relaxed),
        exact_total,
        "a scan spanning journal publication must retry before updating total usage"
    );
}

#[test]
fn fixture_lane_marker_provisions_complete_storage_and_preserves_existing_binding() {
    let kura = Kura::blank_kura_for_testing();
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("explicit fixture network");
    let (_, extended) = initial_and_extended_configs();
    let lane = extended
        .entries()
        .iter()
        .find(|entry| entry.lane_id == LaneId::new(1))
        .expect("secondary fixture lane");
    let incarnation = Hash::prehashed([0x42; Hash::LENGTH]);
    kura.install_lane_incarnation_marker_if_missing_for_test(lane, incarnation, 9)
        .expect("provision complete fixture storage");
    let incarnations = BTreeMap::from([(lane.lane_id, incarnation)]);
    let heights = BTreeMap::from([(lane.lane_id, 9)]);
    let binding = kura
        .geometry_binding(lane, &incarnations, &heights)
        .expect("exact fixture binding");
    assert!(kura.binding_blocks_path(&binding).is_dir());
    for name in [
        DATA_FILE_NAME,
        INDEX_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ] {
        assert!(kura.binding_blocks_path(&binding).join(name).is_file());
    }
    assert!(!kura.store_root.join("merge_ledger").exists());
    kura.require_lane_marker(&binding)
        .expect("exact fixture marker");
    let marker_path = kura.binding_blocks_path(&binding).join(MARKER_FILE_NAME);
    let original = fs::read(&marker_path).expect("read exact marker");
    kura.install_lane_incarnation_marker_if_missing_for_test(
        lane,
        Hash::prehashed([0x43; Hash::LENGTH]),
        10,
    )
    .expect("a fresh full identity leaves the previous instance intact");
    let fresh = kura
        .geometry_binding(
            lane,
            &BTreeMap::from([(lane.lane_id, Hash::prehashed([0x43; Hash::LENGTH]))]),
            &BTreeMap::from([(lane.lane_id, 10)]),
        )
        .unwrap();
    assert_ne!(
        kura.binding_blocks_path(&fresh),
        kura.binding_blocks_path(&binding)
    );
    kura.require_complete_geometry_binding_at(&fresh, &kura.binding_blocks_path(&fresh))
        .expect("fresh identity has its own complete storage");
    assert_eq!(
        fs::read(marker_path).expect("reread exact marker"),
        original
    );
}

#[test]
fn missing_canonical_store_cannot_reinitialize_an_anchored_chain() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let initial = RuntimeLaneConfig::from_catalog(&configured_primary_catalog("canonical-anchor"));
    let kura = open_kura(&root, &initial);
    let (incarnations, _) = initial_geometry();
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        incarnations[&LaneId::SINGLE],
        kura.configured_lane_catalog_baseline().unwrap().unwrap(),
    )
    .expect("anchor catalog");
    let _ = store_structural_geometry_chain(&kura, 1);
    let blocks = Kura::canonical_storage_path(&kura.store_root);
    drop(kura);
    let retained = root.join("removed-canonical-evidence");
    fs::rename(&blocks, &retained).expect("simulate loss of canonical namespace");
    let error = Kura::preflight_canonical_storage(&fs::canonicalize(&root).unwrap())
        .expect_err("an anchored chain cannot be recreated empty");
    assert!(matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::NotFound));
    assert!(!blocks.exists());
    assert!(retained.join(DATA_FILE_NAME).is_file());
}

#[test]
fn lane_geometry_rejects_canonical_namespace_ownership() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let mut binding = kura
        .geometry_binding(
            initial.primary(),
            &BTreeMap::from([(LaneId::SINGLE, Hash::new(b"reserved-path"))]),
            &BTreeMap::from([(LaneId::SINGLE, 0)]),
        )
        .expect("valid lane binding");
    for path in [
        "blocks/canonical",
        "blocks/canonical/child",
        "blocks",
        "merge_ledger/canonical.log",
        "merge_ledger",
    ] {
        binding.blocks_path = path.to_owned();
        assert!(
            validate_geometry_binding_structure(&kura.store_root, &binding).is_err(),
            "reserved path: {path}"
        );
    }
}

#[cfg(unix)]
#[test]
fn canonical_preflight_does_not_traverse_unrelated_retired_evidence() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let outside = temp.path().join("outside-retired-evidence");
    fs::create_dir(&outside).unwrap();
    std::os::unix::fs::symlink(&outside, kura.store_root.join("unrelated-retired-evidence"))
        .unwrap();
    let mut preflight = Kura::preflight_canonical_storage(&kura.store_root)
        .expect("fixed canonical preflight does not inspect unrelated lane maintenance");
    let blocks = Kura::canonical_storage_path(&kura.store_root);
    Kura::reverify_canonical_blocks_open(&mut preflight, &blocks, false).unwrap();
    assert!(
        preflight_configured_store_tree(
            &kura.store_root,
            configured_catalog_store_root_identity(&kura.store_root).unwrap(),
        )
        .is_err(),
        "the explicit whole-tree policy still refuses the unrelated symlink"
    );
}

#[cfg(unix)]
#[test]
fn canonical_preflight_rejects_parent_replacement_even_with_original_leaf_identity() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let mut preflight = Kura::preflight_canonical_storage(&kura.store_root).unwrap();
    let parent = kura.store_root.join("blocks");
    let displaced = kura.store_root.join("displaced-blocks-parent");
    fs::rename(&parent, &displaced).unwrap();
    fs::create_dir(&parent).unwrap();
    fs::rename(displaced.join("canonical"), parent.join("canonical")).unwrap();
    let error =
        Kura::reverify_canonical_blocks_open(&mut preflight, &parent.join("canonical"), false)
            .expect_err("retaining the leaf inode does not authorize a replaced parent");
    assert!(
        matches!(error, Error::IO(ref source, _) if source.to_string().contains("parent identity changed"))
    );
}

#[test]
fn canonical_storage_rejects_retired_merge_namespaces_without_mutation() {
    for relative in ["merge_ledger", "retired/merge_ledger"] {
        for directory in [false, true] {
            let temp = TempDir::new().unwrap();
            let root = temp.path().join("kura");
            let (initial, _) = initial_and_extended_configs();
            let kura = open_kura(&root, &initial);
            let canonical = Kura::canonical_storage_path(&root);
            let original = [
                DATA_FILE_NAME,
                INDEX_FILE_NAME,
                HASHES_FILE_NAME,
                COUNT_FILE_NAME,
            ]
            .map(|name| fs::read(canonical.join(name)).unwrap());
            let retired = root.join(relative);
            fs::create_dir_all(retired.parent().unwrap()).unwrap();
            let sentinel = if directory {
                fs::create_dir(&retired).unwrap();
                retired.join("original.log")
            } else {
                retired.clone()
            };
            fs::write(&sentinel, b"obsolete storage is never rewritten").unwrap();
            assert!(Kura::reject_retired_merge_storage(&root).is_err());
            assert!(Kura::preflight_canonical_storage(&root).is_err());
            assert!(kura.physical_resource_scope().is_err());
            assert_eq!(
                fs::read(&sentinel).unwrap(),
                b"obsolete storage is never rewritten"
            );
            assert_eq!(
                [
                    DATA_FILE_NAME,
                    INDEX_FILE_NAME,
                    HASHES_FILE_NAME,
                    COUNT_FILE_NAME
                ]
                .map(|name| fs::read(canonical.join(name)).unwrap()),
                original
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn canonical_storage_rejects_retired_namespace_symlinks_without_following_them() {
    for relative in ["merge_ledger", "retired/merge_ledger"] {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).unwrap();
        fs::write(outside.join("sentinel"), b"external bytes").unwrap();
        let retired = root.join(relative);
        fs::create_dir_all(retired.parent().unwrap()).unwrap();
        std::os::unix::fs::symlink(&outside, &retired).unwrap();
        assert!(Kura::reject_retired_merge_storage(&root).is_err());
        assert!(kura.physical_resource_scope().is_err());
        assert!(retired.is_symlink());
        assert_eq!(fs::read_dir(&outside).unwrap().count(), 1);
        assert_eq!(
            fs::read(outside.join("sentinel")).unwrap(),
            b"external bytes"
        );
    }
}
