fn initial_primary_dataspace_for_pair_fixture() -> DataSpaceId {
    ModelLaneConfig::default().dataspace_id
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
fn inverse_pair_move_recovers_a_seal_persisted_before_the_first_rename() {
    fn exercise(
        kura: &Kura,
        root: &Path,
        label: &str,
        shared_blocks: bool,
        shared_merge: bool,
        inverse_target_kind: GeometryPairTargetKind,
    ) {
        let case_root = root.join(label);
        let binding = LaneGeometryBinding::from_identity(LaneStorageIdentity {
            network_id: geometry_fixture_network_id(),
            lane_id: LaneId::new(20),
            dataspace_id: initial_primary_dataspace_for_pair_fixture(),
            incarnation: Hash::new(label.as_bytes()),
            activation_height: 1,
        });
        let original_blocks = kura.binding_blocks_path(&binding);
        let original_merge = kura.binding_merge_path(&binding);
        let forward_blocks = if shared_blocks {
            original_blocks.clone()
        } else {
            case_root.join("forward-blocks")
        };
        let forward_merge = if shared_merge {
            original_merge.clone()
        } else {
            case_root.join("forward-merge.log")
        };
        kura.provision_geometry_binding(&binding)
            .expect("provision original pair");
        fs::write(&original_merge, format!("{label}-merge-evidence")).expect("seed merge evidence");
        kura.seal_geometry_pair_move(
            &binding,
            &original_blocks,
            &original_merge,
            &forward_blocks,
            &forward_merge,
        )
        .expect("persist forward seal before first rename");
        kura.move_geometry_binding_pair(
            &binding,
            &forward_blocks,
            &forward_merge,
            &original_blocks,
            &original_merge,
            inverse_target_kind,
        )
        .expect("inverse move recognizes the exact opposite-path seal");
        assert!(original_blocks.is_dir());
        assert!(original_merge.is_file());
        if forward_blocks != original_blocks {
            assert!(!forward_blocks.exists());
        }
        if forward_merge != original_merge {
            assert!(!forward_merge.exists());
        }
        match inverse_target_kind {
            GeometryPairTargetKind::MutableLive => {
                let marker = kura
                    .read_lane_marker(&original_blocks.join(MARKER_FILE_NAME))
                    .expect("read normalized mutable marker");
                assert!(marker.move_target_blocks.is_none());
                assert!(marker.move_target_merge.is_none());
            }
            GeometryPairTargetKind::ImmutableRetained => kura
                .require_sealed_geometry_pair_at(
                    &binding,
                    &original_blocks,
                    &original_merge,
                    &original_blocks,
                    &original_merge,
                )
                .expect("immutable inverse target retains its normalized seal"),
        }
    }
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    exercise(
        &kura,
        &root,
        "full-mutable",
        false,
        false,
        GeometryPairTargetKind::MutableLive,
    );
    exercise(
        &kura,
        &root,
        "full-immutable",
        false,
        false,
        GeometryPairTargetKind::ImmutableRetained,
    );
    exercise(
        &kura,
        &root,
        "stationary-blocks",
        true,
        false,
        GeometryPairTargetKind::MutableLive,
    );
    exercise(
        &kura,
        &root,
        "stationary-merge",
        false,
        true,
        GeometryPairTargetKind::MutableLive,
    );
}
#[test]
fn inverse_pair_move_recovers_clear_temp_after_both_renames() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let moved_blocks = root.join("clear-temp/moved-blocks");
    let moved_merge = root.join("clear-temp/moved-merge.log");
    let binding = LaneGeometryBinding::from_identity(LaneStorageIdentity {
        network_id: geometry_fixture_network_id(),
        lane_id: LaneId::new(21),
        dataspace_id: initial_primary_dataspace_for_pair_fixture(),
        incarnation: Hash::new(b"clear-temp-direction-reversal"),
        activation_height: 1,
    });
    let original_blocks = kura.binding_blocks_path(&binding);
    let original_merge = kura.binding_merge_path(&binding);
    kura.provision_geometry_binding(&binding)
        .expect("provision movable pair");
    fs::write(&original_merge, b"clear-temp-merge-evidence").expect("seed merge evidence");
    fs::write(original_blocks.join("payload"), b"block-image-evidence")
        .expect("seed block evidence");
    kura.seal_geometry_pair_move(
        &binding,
        &original_blocks,
        &original_merge,
        &moved_blocks,
        &moved_merge,
    )
    .expect("seal forward pair move");
    kura.move_geometry_path(&original_blocks, &moved_blocks, true)
        .expect("move block half");
    kura.move_geometry_path(&original_merge, &moved_merge, false)
        .expect("move merge half");
    let stale_clear = LaneIncarnationMarker {
        version: MARKER_VERSION,
        network_id: binding.network_id,
        dataspace_id: binding.dataspace_id,
        lane_id: binding.lane_id,
        incarnation: binding.incarnation,
        activation_height: binding.activation_height,
        move_target_blocks: None,
        move_target_merge: None,
        block_store_digest: kura
            .geometry_block_store_digest(&moved_blocks)
            .expect("moved block digest"),
        merge_log_digest: kura
            .geometry_merge_log_digest(&moved_merge)
            .expect("moved merge digest"),
    };
    let stale_temp = moved_blocks.join(MARKER_TEMP_FILE_NAME);
    fs::write(&stale_temp, stale_clear.encode())
        .expect("simulate crash before seal-clear marker rename");
    kura.move_geometry_binding_pair(
        &binding,
        &moved_blocks,
        &moved_merge,
        &original_blocks,
        &original_merge,
        GeometryPairTargetKind::MutableLive,
    )
    .expect("inverse direction discards the authenticated uncommitted clear temp");
    assert!(!stale_temp.exists());
    assert_eq!(
        fs::read(original_blocks.join("payload")).expect("block bytes restored"),
        b"block-image-evidence"
    );
    assert_eq!(
        fs::read(&original_merge).expect("merge bytes restored"),
        b"clear-temp-merge-evidence"
    );
    let marker = kura
        .read_lane_marker(&original_blocks.join(MARKER_FILE_NAME))
        .expect("read restored live marker");
    assert!(marker.move_target_blocks.is_none());
    assert!(marker.move_target_merge.is_none());
    kura.move_geometry_binding_pair(
        &binding,
        &moved_blocks,
        &moved_merge,
        &original_blocks,
        &original_merge,
        GeometryPairTargetKind::MutableLive,
    )
    .expect("completed inverse remains idempotent");
    let foreign_temp = original_blocks.join(MARKER_TEMP_FILE_NAME);
    fs::write(
        &foreign_temp,
        LaneIncarnationMarker {
            version: MARKER_VERSION,
            network_id: binding.network_id,
            dataspace_id: binding.dataspace_id,
            lane_id: binding.lane_id,
            incarnation: Hash::new(b"foreign-marker-temp"),
            activation_height: binding.activation_height,
            move_target_blocks: None,
            move_target_merge: None,
            block_store_digest: kura
                .geometry_block_store_digest(&original_blocks)
                .expect("current block digest"),
            merge_log_digest: kura
                .geometry_merge_log_digest(&original_merge)
                .expect("current merge digest"),
        }
        .encode(),
    )
    .expect("inject foreign marker temp");
    let error = kura
        .move_geometry_binding_pair(
            &binding,
            &original_blocks,
            &original_merge,
            &moved_blocks,
            &moved_merge,
            GeometryPairTargetKind::MutableLive,
        )
        .expect_err("foreign marker temp must not be removed or adopted");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "lane storage incarnation marker does not match authoritative binding",
    );
    assert!(foreign_temp.is_file());
    assert!(original_blocks.is_dir());
    assert!(original_merge.is_file());
}
#[test]
fn immutable_pair_move_rejects_a_post_crash_foreign_merge_swap() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let target_blocks = root.join("sealed-pair/archive-blocks");
    let target_merge = root.join("sealed-pair/archive-merge.log");
    let binding = LaneGeometryBinding::from_identity(LaneStorageIdentity {
        network_id: geometry_fixture_network_id(),
        lane_id: LaneId::new(8),
        dataspace_id: initial_primary_dataspace_for_pair_fixture(),
        incarnation: Hash::new(b"immutable-retained-pair"),
        activation_height: 1,
    });
    let source_blocks = kura.binding_blocks_path(&binding);
    let source_merge = kura.binding_merge_path(&binding);
    kura.provision_geometry_binding(&binding)
        .expect("provision retained geometry pair");
    fs::write(&source_merge, b"authoritative-merge-history")
        .expect("seed authoritative merge bytes");
    kura.move_geometry_binding_pair(
        &binding,
        &source_blocks,
        &source_merge,
        &target_blocks,
        &target_merge,
        GeometryPairTargetKind::ImmutableRetained,
    )
    .expect("archive authenticated pair");
    fs::write(&target_merge, b"foreign-valid-looking-merge-history")
        .expect("swap retained merge bytes");
    let error = kura
        .move_geometry_binding_pair(
            &binding,
            &source_blocks,
            &source_merge,
            &target_blocks,
            &target_merge,
            GeometryPairTargetKind::ImmutableRetained,
        )
        .expect_err("retained pair digest must reject a foreign merge swap");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "lane geometry pair does not match its durable block/merge evidence",
    );
    assert!(target_blocks.is_dir());
    assert_eq!(
        fs::read(&target_merge).expect("foreign bytes retained for operator inspection"),
        b"foreign-valid-looking-merge-history"
    );
}
#[test]
fn immutable_pair_move_rejects_a_post_crash_block_image_swap() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let target_blocks = root.join("sealed-block-pair/archive-blocks");
    let target_merge = root.join("sealed-block-pair/archive-merge.log");
    let binding = LaneGeometryBinding::from_identity(LaneStorageIdentity {
        network_id: geometry_fixture_network_id(),
        lane_id: LaneId::new(9),
        dataspace_id: initial_primary_dataspace_for_pair_fixture(),
        incarnation: Hash::new(b"immutable-retained-block-image"),
        activation_height: 1,
    });
    let source_blocks = kura.binding_blocks_path(&binding);
    let source_merge = kura.binding_merge_path(&binding);
    kura.provision_geometry_binding(&binding)
        .expect("provision retained geometry pair");
    let payload = source_blocks.join("retained-payload");
    fs::write(&payload, b"authoritative-block-image").expect("seed block image bytes");
    kura.move_geometry_binding_pair(
        &binding,
        &source_blocks,
        &source_merge,
        &target_blocks,
        &target_merge,
        GeometryPairTargetKind::ImmutableRetained,
    )
    .expect("archive authenticated pair");
    let retained_payload = target_blocks.join("retained-payload");
    fs::write(&retained_payload, b"foreign-valid-block-image").expect("swap retained block bytes");
    let error = kura
        .move_geometry_binding_pair(
            &binding,
            &source_blocks,
            &source_merge,
            &target_blocks,
            &target_merge,
            GeometryPairTargetKind::ImmutableRetained,
        )
        .expect_err("retained pair digest must reject a foreign block image");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "lane geometry pair does not match its durable block/merge evidence",
    );
    assert_eq!(
        fs::read(&retained_payload).expect("foreign bytes retained for inspection"),
        b"foreign-valid-block-image"
    );
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
        .resolve_relative_path(&operation.unpublished_blocks_path)
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
    let merge = geometry_fixture_merge(&kura, lane, &extended_incarnations, &extended_activations);
    assert_eq!(staged_blocks, blocks);
    assert!(blocks.join(MARKER_FILE_NAME).is_file());
    assert!(merge.is_file());
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[0].phase,
        LaneGeometryPhase::CatalogPublished
    );
}
#[test]
fn replacement_rollback_preserves_both_exact_instances_and_rejects_foreign_marker() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = nonzero!(2_u32);
    let primary = ModelLaneConfig::default();
    let active_lane = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "replace-before".to_owned(),
        ..ModelLaneConfig::default()
    };
    let replacement_lane = ModelLaneConfig {
        alias: "replace-after".to_owned(),
        visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
        ..active_lane.clone()
    };
    let base_catalog = LaneCatalog::new(lane_count, vec![primary.clone()]).expect("base catalog");
    let active_catalog =
        LaneCatalog::new(lane_count, vec![primary.clone(), active_lane]).expect("active catalog");
    let replacement_catalog =
        LaneCatalog::new(lane_count, vec![primary, replacement_lane]).expect("replacement catalog");
    let base = RuntimeLaneConfig::from_catalog(&base_catalog);
    let active = RuntimeLaneConfig::from_catalog(&active_catalog);
    let replacement = RuntimeLaneConfig::from_catalog(&replacement_catalog);
    let base_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x31; Hash::LENGTH]))]);
    let active_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x32; Hash::LENGTH])),
    ]);
    let replacement_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x33; Hash::LENGTH])),
    ]);
    let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let active_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 4)]);
    let replacement_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 5)]);
    let kura = open_kura(&root, &base);
    authenticate_transition_fixture_primary(&kura, &base, &base_incarnations);
    kura.apply_lane_geometry_transition(
        &base,
        &active,
        &base_incarnations,
        &active_incarnations,
        &base_activations,
        &active_activations,
        &BTreeSet::new(),
    )
    .expect("create replaceable lane");
    kura.mark_lane_geometry_catalog_published(
        &active,
        &active_incarnations,
        &active_activations,
        None,
    )
    .expect("publish replaceable lane");
    kura.apply_lane_geometry_transition(
        &active,
        &replacement,
        &active_incarnations,
        &replacement_incarnations,
        &active_activations,
        &replacement_activations,
        &BTreeSet::from([LaneId::new(1)]),
    )
    .expect("apply replacement before simulated rollback crash");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("replacement journal");
    let operation = journal.records[1].operations[0].clone();
    assert_eq!(operation.kind, LaneGeometryOperationKind::Replace);
    let updated = operation.updated.as_ref().expect("updated binding");
    let previous = operation.previous.as_ref().expect("previous binding");
    let updated_blocks = kura.binding_blocks_path(updated);
    let updated_merge = kura.binding_merge_path(updated);
    let previous_blocks = kura.binding_blocks_path(previous);
    let previous_merge = kura.binding_merge_path(previous);
    assert_ne!(updated_blocks, previous_blocks);
    assert_ne!(updated_merge, previous_merge);
    let original_previous_marker = fs::read(previous_blocks.join(MARKER_FILE_NAME)).unwrap();
    let original_updated_marker = fs::read(updated_blocks.join(MARKER_FILE_NAME)).unwrap();
    fs::write(
        &previous_merge,
        b"previous merge remains at its immutable address",
    )
    .unwrap();
    fs::write(
        updated_blocks.join("replacement-payload"),
        b"retained replacement image",
    )
    .unwrap();
    for (catalog, incarnations, activations, phase) in [
        (
            &active,
            &active_incarnations,
            &active_activations,
            LaneGeometryPhase::RolledBack,
        ),
        (
            &replacement,
            &replacement_incarnations,
            &replacement_activations,
            LaneGeometryPhase::CatalogPublished,
        ),
        (
            &active,
            &active_incarnations,
            &active_activations,
            LaneGeometryPhase::RolledBack,
        ),
    ] {
        kura.recover_lane_geometry_journal(catalog, incarnations, activations)
            .expect("recovery changes only the exact reference frontier");
        kura.require_complete_geometry_binding_at(previous, &previous_blocks, &previous_merge)
            .expect("previous instance remains complete");
        kura.require_complete_geometry_binding_at(updated, &updated_blocks, &updated_merge)
            .expect("replacement instance remains complete");
        assert_eq!(
            fs::read(previous_blocks.join(MARKER_FILE_NAME)).unwrap(),
            original_previous_marker
        );
        assert_eq!(
            fs::read(updated_blocks.join(MARKER_FILE_NAME)).unwrap(),
            original_updated_marker
        );
        assert_eq!(
            fs::read(&previous_merge).unwrap(),
            b"previous merge remains at its immutable address"
        );
        assert_eq!(
            fs::read(updated_blocks.join("replacement-payload")).unwrap(),
            b"retained replacement image"
        );
        assert_eq!(
            kura.read_lane_geometry_journal().unwrap().records[1].phase,
            phase
        );
    }
    assert!(
        !root.join("retired/lane_geometry").exists(),
        "rollback has no archive move"
    );
    let journal_before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    let marker_path = updated_blocks.join(MARKER_FILE_NAME);
    let mut foreign = decode_exact::<LaneIncarnationMarker>(&original_updated_marker).unwrap();
    foreign.incarnation = Hash::new(b"foreign replacement instance");
    let foreign_bytes = foreign.encode();
    fs::write(&marker_path, &foreign_bytes).unwrap();
    kura.recover_lane_geometry_journal(
        &replacement,
        &replacement_incarnations,
        &replacement_activations,
    )
    .expect_err("retained reference cannot adopt a foreign instance at the correct path");
    assert_eq!(fs::read(marker_path).unwrap(), foreign_bytes);
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).unwrap(),
        journal_before
    );
    assert_eq!(
        fs::read(&previous_merge).unwrap(),
        b"previous merge remains at its immutable address"
    );
}

#[test]
fn replacement_intent_rollback_completes_owned_block_only_instance() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = nonzero!(2_u32);
    let primary = ModelLaneConfig::default();
    let active_lane = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "intent-replace-before".to_owned(),
        ..ModelLaneConfig::default()
    };
    let replacement_lane = ModelLaneConfig {
        alias: "intent-replace-after".to_owned(),
        visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
        ..active_lane.clone()
    };
    let base_catalog = LaneCatalog::new(lane_count, vec![primary.clone()]).expect("base catalog");
    let active_catalog =
        LaneCatalog::new(lane_count, vec![primary.clone(), active_lane]).expect("active catalog");
    let replacement_catalog =
        LaneCatalog::new(lane_count, vec![primary, replacement_lane]).expect("replacement catalog");
    let base = RuntimeLaneConfig::from_catalog(&base_catalog);
    let active = RuntimeLaneConfig::from_catalog(&active_catalog);
    let replacement = RuntimeLaneConfig::from_catalog(&replacement_catalog);
    let base_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x51; Hash::LENGTH]))]);
    let active_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x52; Hash::LENGTH])),
    ]);
    let replacement_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x53; Hash::LENGTH])),
    ]);
    let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let active_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 4)]);
    let replacement_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 5)]);
    let kura = open_kura(&root, &base);
    authenticate_transition_fixture_primary(&kura, &base, &base_incarnations);
    kura.apply_lane_geometry_transition(
        &base,
        &active,
        &base_incarnations,
        &active_incarnations,
        &base_activations,
        &active_activations,
        &BTreeSet::new(),
    )
    .expect("create replaceable lane");
    kura.mark_lane_geometry_catalog_published(
        &active,
        &active_incarnations,
        &active_activations,
        None,
    )
    .expect("publish replaceable lane");
    kura.apply_lane_geometry_transition(
        &active,
        &replacement,
        &active_incarnations,
        &replacement_incarnations,
        &active_activations,
        &replacement_activations,
        &BTreeSet::from([LaneId::new(1)]),
    )
    .expect("apply replacement before simulated Intent crash");
    let mut journal = kura
        .read_lane_geometry_journal()
        .expect("replacement journal");
    journal.records[1].phase = LaneGeometryPhase::Intent;
    let operation = journal.records[1].operations[0].clone();
    kura.write_lane_geometry_journal(&journal)
        .expect("restore the pre-files-applied Intent frontier");
    let updated = operation.updated.as_ref().expect("updated binding");
    let previous = operation.previous.as_ref().expect("previous binding");
    let updated_blocks = kura.binding_blocks_path(updated);
    let updated_merge = kura.binding_merge_path(updated);
    let previous_blocks = kura.binding_blocks_path(previous);
    let previous_merge = kura.binding_merge_path(previous);
    let previous_marker = fs::read(previous_blocks.join(MARKER_FILE_NAME)).unwrap();
    assert_eq!(fs::metadata(&updated_merge).unwrap().len(), 0);
    fs::remove_file(&updated_merge)
        .expect("crash cut after owned block/marker creation before merge creation");
    assert!(updated_blocks.is_dir());
    assert!(!updated_merge.exists());
    kura.recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
        .expect("Intent rollback completes only its exact empty owned pair");
    kura.require_complete_geometry_binding_at(updated, &updated_blocks, &updated_merge)
        .expect("interrupted instance remains complete at its original address");
    kura.require_complete_geometry_binding_at(previous, &previous_blocks, &previous_merge)
        .expect("previous instance remains complete without a move");
    assert_eq!(
        fs::read(previous_blocks.join(MARKER_FILE_NAME)).unwrap(),
        previous_marker
    );
    assert_eq!(
        kura.read_lane_geometry_journal().unwrap().records[1].phase,
        LaneGeometryPhase::RolledBack
    );
    assert!(!root.join("retired/lane_geometry").exists());
    kura.recover_lane_geometry_journal(
        &replacement,
        &replacement_incarnations,
        &replacement_activations,
    )
    .expect("exact retained replacement can be referenced again");
    assert_eq!(
        kura.read_lane_geometry_journal().unwrap().records[1].phase,
        LaneGeometryPhase::CatalogPublished
    );
}

#[test]
fn same_alias_replacement_rollback_preserves_old_pair_during_new_instance_creation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = nonzero!(2_u32);
    let primary = ModelLaneConfig::default();
    let active_lane = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "same-path-replacement".to_owned(),
        ..ModelLaneConfig::default()
    };
    let replacement_lane = ModelLaneConfig {
        visibility: iroha_data_model::nexus::LaneVisibility::Restricted,
        ..active_lane.clone()
    };
    let base_catalog = LaneCatalog::new(lane_count, vec![primary.clone()]).expect("base catalog");
    let active_catalog =
        LaneCatalog::new(lane_count, vec![primary.clone(), active_lane]).expect("active catalog");
    let replacement_catalog =
        LaneCatalog::new(lane_count, vec![primary, replacement_lane]).expect("replacement catalog");
    let base = RuntimeLaneConfig::from_catalog(&base_catalog);
    let active = RuntimeLaneConfig::from_catalog(&active_catalog);
    let replacement = RuntimeLaneConfig::from_catalog(&replacement_catalog);
    let base_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x41; Hash::LENGTH]))]);
    let active_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x42; Hash::LENGTH])),
    ]);
    let replacement_incarnations = BTreeMap::from([
        (LaneId::SINGLE, base_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x43; Hash::LENGTH])),
    ]);
    let base_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let active_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 4)]);
    let replacement_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 5)]);
    let kura = open_kura(&root, &base);
    authenticate_transition_fixture_primary(&kura, &base, &base_incarnations);
    kura.apply_lane_geometry_transition_at_height(
        &base,
        &active,
        &base_incarnations,
        &active_incarnations,
        &base_activations,
        &active_activations,
        &BTreeSet::new(),
        4,
    )
    .expect("create replaceable lane");
    kura.mark_lane_geometry_catalog_published(
        &active,
        &active_incarnations,
        &active_activations,
        None,
    )
    .expect("publish replaceable lane");
    let previous_bindings = kura
        .geometry_bindings(&active, &active_incarnations, &active_activations)
        .expect("active bindings");
    let updated_bindings = kura
        .geometry_bindings(
            &replacement,
            &replacement_incarnations,
            &replacement_activations,
        )
        .expect("replacement bindings");
    let previous_catalog = geometry_catalog_fingerprint(&previous_bindings);
    let updated_catalog = geometry_catalog_fingerprint(&updated_bindings);
    let previous_lineage_root = unscoped_lineage_root(&previous_bindings);
    let updated_lineage_root = unscoped_lineage_root(&updated_bindings);
    let mut journal = kura.read_lane_geometry_journal().expect("active journal");
    let transition_sequence = journal.records[0]
        .transition_sequence
        .checked_add(1)
        .expect("transition sequence");
    let transition_height = 5;
    let transition_id = geometry_transition_id(
        transition_sequence,
        transition_height,
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
            &BTreeSet::from([LaneId::new(1)]),
        )
        .expect("same-path replacement operation");
    let operation = operations[0].clone();
    let previous = operation.previous.as_ref().expect("previous binding");
    let updated = operation.updated.as_ref().expect("updated binding");
    assert_ne!(previous.blocks_path, updated.blocks_path);
    assert_ne!(previous.merge_path, updated.merge_path);
    journal.records.push(LaneGeometryIntent {
        transition_id,
        transition_sequence,
        transition_height,
        previous_catalog,
        previous_lineage_root,
        updated_catalog,
        updated_lineage_root,
        previous_bindings,
        updated_bindings,
        phase: LaneGeometryPhase::Intent,
        operations,
    });
    kura.write_lane_geometry_journal(&journal)
        .expect("persist replacement intent");
    let previous_blocks = kura.binding_blocks_path(previous);
    let previous_merge = kura.binding_merge_path(previous);
    let updated_blocks = kura.binding_blocks_path(updated);
    let updated_merge = kura.binding_merge_path(updated);
    let previous_marker = fs::read(previous_blocks.join(MARKER_FILE_NAME)).unwrap();
    let sentinel = b"old-merge-half-must-remain-live";
    fs::write(&previous_merge, sentinel).expect("write old merge sentinel");
    fs::create_dir_all(&updated_blocks)
        .expect("crash cut after exact owned instance directory creation");
    assert!(previous_blocks.is_dir());
    assert!(previous_merge.is_file());
    assert!(!updated_merge.exists());
    kura.recover_lane_geometry_journal_at_height(
        &active,
        &active_incarnations,
        &active_activations,
        4,
    )
    .expect("rollback must preserve the prior immutable pair while finishing owned creation");
    assert_eq!(fs::read(&previous_merge).unwrap(), sentinel);
    assert_eq!(
        fs::read(previous_blocks.join(MARKER_FILE_NAME)).unwrap(),
        previous_marker
    );
    kura.require_complete_geometry_binding_at(updated, &updated_blocks, &updated_merge)
        .expect("rollback retains the complete authenticated empty replacement image");
    assert_eq!(
        kura.read_lane_geometry_journal().unwrap().records[1].phase,
        LaneGeometryPhase::RolledBack
    );
    kura.recover_lane_geometry_journal_at_height(
        &replacement,
        &replacement_incarnations,
        &replacement_activations,
        5,
    )
    .expect("replay uses the distinct exact replacement instance");
    assert_eq!(fs::read(&previous_merge).unwrap(), sentinel);
    assert_eq!(
        fs::read(previous_blocks.join(MARKER_FILE_NAME)).unwrap(),
        previous_marker
    );
    assert!(updated_blocks.is_dir() && updated_merge.is_file());
    assert!(!root.join("retired/lane_geometry").exists());
}

#[test]
fn recovery_distinguishes_repeated_catalogs_by_retained_lineage_root() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (first_incarnations, first_activations) = extended_geometry();
    let mut second_incarnations = first_incarnations.clone();
    second_incarnations.insert(LaneId::new(1), Hash::prehashed([0x44; Hash::LENGTH]));
    let mut second_activations = first_activations.clone();
    second_activations.insert(LaneId::new(1), 10);
    let lineage_initial = Hash::new(b"lineage:initial:never-seen");
    let lineage_first_active = Hash::new(b"lineage:first:active");
    let lineage_first_retired = Hash::new(b"lineage:first:retired");
    let lineage_second_active = Hash::new(b"lineage:second:active");
    let lineage_second_retired = Hash::new(b"lineage:second:retired");
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    // Bind the actual configured primary before retaining geometry or bodies for restart.
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        kura.configured_lane_catalog_baseline()
            .expect("read authenticated configured catalog")
            .expect("configured catalog was admitted at open"),
    )
    .expect("anchor configured primary before restart fixture writes");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &initial,
        &extended,
        &initial_incarnations,
        &first_incarnations,
        &initial_activations,
        &first_activations,
        lineage_initial,
        lineage_first_active,
        &BTreeSet::new(),
        9,
    )
    .expect("create first lane incarnation");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &extended,
        &first_incarnations,
        &first_activations,
        Hash::new(b"lineage:first:wrong"),
        None,
    )
    .expect_err("publication must reject a mismatched retained-lineage root");
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("unpublished rooted journal")
            .records[0]
            .phase,
        LaneGeometryPhase::FilesApplied
    );
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &extended,
        &first_incarnations,
        &first_activations,
        lineage_first_active,
        None,
    )
    .expect("publish first active lineage");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &extended,
        &initial,
        &first_incarnations,
        &initial_incarnations,
        &first_activations,
        &initial_activations,
        lineage_first_active,
        lineage_first_retired,
        &BTreeSet::new(),
        10,
    )
    .expect("retire first lane incarnation");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &initial,
        &initial_incarnations,
        &initial_activations,
        lineage_first_retired,
        None,
    )
    .expect("publish first retired lineage");
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &initial,
        &extended,
        &initial_incarnations,
        &second_incarnations,
        &initial_activations,
        &second_activations,
        lineage_first_retired,
        lineage_second_active,
        &BTreeSet::new(),
        10,
    )
    .expect("create second lane incarnation");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &extended,
        &second_incarnations,
        &second_activations,
        lineage_second_active,
        None,
    )
    .expect("publish second active lineage");
    let lane1 = extended.entry(LaneId::new(1)).expect("lane one");
    kura.recover_lane_geometry_journal_before_transition_with_lineage_root(
        &initial,
        &initial_incarnations,
        &initial_activations,
        lineage_first_retired,
        10,
    )
    .expect("recover first retired lineage while second incarnation is live");
    assert!(
        geometry_fixture_blocks(&kura, lane1, &first_incarnations, &first_activations).is_dir()
    );
    assert!(
        geometry_fixture_blocks(&kura, lane1, &second_incarnations, &second_activations).is_dir()
    );
    assert!(kura.lane_storage_entry(LaneId::new(1)).is_err());
    kura.recover_lane_geometry_journal_at_height_with_lineage_root(
        &extended,
        &second_incarnations,
        &second_activations,
        10,
        lineage_second_active,
    )
    .expect("restore second active lineage after exact rooted rollback");
    let second_blocks =
        geometry_fixture_blocks(&kura, lane1, &second_incarnations, &second_activations);
    assert!(second_blocks.is_dir());
    assert_eq!(
        kura.lane_storage_entry(LaneId::new(1))
            .unwrap()
            .blocks_dir(&root),
        second_blocks
    );
    kura.apply_lane_geometry_transition_at_height_with_lineage_roots(
        &extended,
        &initial,
        &second_incarnations,
        &initial_incarnations,
        &second_activations,
        &initial_activations,
        lineage_second_active,
        lineage_second_retired,
        &BTreeSet::new(),
        11,
    )
    .expect("retire second lane incarnation");
    kura.mark_lane_geometry_catalog_published_with_lineage_root(
        &initial,
        &initial_incarnations,
        &initial_activations,
        lineage_second_retired,
        None,
    )
    .expect("publish second retired lineage");
    let phases = kura
        .read_lane_geometry_journal()
        .expect("four-transition journal")
        .records
        .into_iter()
        .map(|record| record.phase)
        .collect::<Vec<_>>();
    assert_eq!(phases, vec![LaneGeometryPhase::CatalogPublished; 4]);
    kura.recover_lane_geometry_journal_before_transition_with_lineage_root(
        &initial,
        &initial_incarnations,
        &initial_activations,
        lineage_first_retired,
        10,
    )
    .expect("recover the first repeated retired catalog exactly");
    let phases = kura
        .read_lane_geometry_journal()
        .expect("rolled-back future lineage journal")
        .records
        .into_iter()
        .map(|record| record.phase)
        .collect::<Vec<_>>();
    assert_eq!(
        phases,
        vec![
            LaneGeometryPhase::CatalogPublished,
            LaneGeometryPhase::CatalogPublished,
            LaneGeometryPhase::RolledBack,
            LaneGeometryPhase::RolledBack,
        ]
    );
    let before_unknown = kura
        .read_lane_geometry_journal()
        .expect("journal before unknown root");
    kura.recover_lane_geometry_journal_before_transition_with_lineage_root(
        &initial,
        &initial_incarnations,
        &initial_activations,
        Hash::new(b"lineage:unknown"),
        10,
    )
    .expect_err("an unretained lineage root must fail closed");
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("journal after unknown root"),
        before_unknown,
        "failed recovery must not rewrite transition phases"
    );
    drop(kura);
    let restarted = open_kura(&root, &initial);
    restarted
        .recover_lane_geometry_journal_at_height_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            11,
            lineage_second_retired,
        )
        .expect("restart recovers the latest repeated retired catalog");
    assert!(
        restarted
            .read_lane_geometry_journal()
            .expect("restarted journal")
            .records
            .iter()
            .all(|record| record.phase == LaneGeometryPhase::CatalogPublished)
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
    let _ = durable_geometry_snapshot_identity(&kura, 3);
    let expected_hashes = (1..=3)
        .map(|height| {
            kura.get_durable_block_hash(NonZeroUsize::new(height).expect("non-zero"))
                .expect("durable block hash")
        })
        .collect::<Vec<_>>();
    let blocks = geometry_fixture_blocks(&kura, initial.primary(), &incarnations, &activations);
    let merge = geometry_fixture_merge(&kura, initial.primary(), &incarnations, &activations);
    assert_eq!(
        blocks,
        geometry_fixture_blocks(&kura, updated.primary(), &incarnations, &activations)
    );
    assert_eq!(
        merge,
        geometry_fixture_merge(&kura, updated.primary(), &incarnations, &activations)
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
    assert!(blocks.is_dir() && merge.is_file());
    drop(kura);
    let reopened = open_kura(&root, &initial);
    assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 3);
    assert_eq!(
        *reopened.active_blocks_dir.lock(),
        Kura::canonical_storage_paths(&root).0
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
        assert!(blocks.is_dir() && merge.is_file());
        assert_eq!(
            fs::read(blocks.join(MARKER_FILE_NAME)).unwrap(),
            marker_before
        );
        assert_eq!(
            *reopened.active_blocks_dir.lock(),
            Kura::canonical_storage_paths(&root).0
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
    let _ = durable_geometry_snapshot_identity(&kura, 3);
    let exact_chain = |kura: &Kura| {
        (1..=kura.exact_durable_blocks_count().unwrap())
            .map(|height| {
                kura.get_block(NonZeroUsize::new(height).expect("non-zero block height"))
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
        .flat_map(|binding| {
            [
                kura.binding_blocks_path(binding),
                kura.binding_merge_path(binding),
            ]
        })
        .collect::<Vec<_>>();
    let secondary = initial_bindings
        .iter()
        .find(|binding| binding.lane_id == LaneId::new(1))
        .unwrap();
    let secondary_blocks = kura.binding_blocks_path(secondary);
    let secondary_merge = kura.binding_merge_path(secondary);
    let sentinel = secondary_blocks.join(MARKER_FILE_NAME);
    let expected_instance_marker = fs::read(&sentinel).unwrap();
    let expected_instance_merge = fs::read(&secondary_merge).unwrap();
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
            Kura::canonical_storage_paths(&root).0
        );
        assert_eq!(
            *reopened.active_merge_path.lock(),
            Kura::canonical_storage_paths(&root).1
        );
        assert!(paths.iter().all(|path| path.exists()));
        assert_eq!(fs::read(&sentinel).unwrap(), expected_instance_marker);
        assert_eq!(fs::read(&secondary_merge).unwrap(), expected_instance_merge);
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
struct GeometryReferenceResumeGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for GeometryReferenceResumeGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
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
    let _ = durable_geometry_snapshot_identity(&kura, 1);
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
            Kura::canonical_storage_paths(&kura.store_root).0
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
            .get_block(nonzero!(1_usize))
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
    let merge = geometry_fixture_merge(&kura, lane, &extended_incarnations, &extended_activations);
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
    assert!(!merge.exists());
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
fn snapshot_checkpoint_compacts_only_proven_history_and_preserves_latest_recovery() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let (initial_incarnations, _) = initial_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
    // Bind the actual configured primary before retaining geometry or bodies for restart.
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        kura.configured_lane_catalog_baseline()
            .expect("read authenticated configured catalog")
            .expect("configured catalog was admitted at open"),
    )
    .expect("anchor configured primary before restart fixture writes");
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    // Before checkpoint publication, both the old and current authoritative catalogs remain
    // recoverable from the retained transition chain.
    kura.recover_lane_geometry_journal_at_height(
        &fixture.extended,
        &fixture.extended_incarnations,
        &fixture.extended_activations,
        0,
    )
    .expect("old snapshot geometry remains recoverable before GC");
    kura.recover_lane_geometry_journal_at_height(
        &fixture.initial,
        &fixture.initial_incarnations,
        &fixture.initial_activations,
        1,
    )
    .expect("restore current snapshot geometry");
    durable_geometry_snapshot_identity(&kura, 20);
    let cached_before = kura.refresh_disk_usage_bytes().expect("usage before GC");
    let summary = checkpoint_retired_geometry(&kura, &fixture, 20)
        .expect("checkpoint current snapshot geometry");
    assert_eq!(summary.compacted_transitions, 2);
    assert_eq!(summary.removed_archive_roots, 1);
    assert!(summary.reclaimed_bytes >= fixture.retained_bytes);
    assert!(!fixture.retained_blocks.exists());
    let journal = kura
        .read_lane_geometry_journal()
        .expect("compacted journal");
    assert!(journal.records.is_empty());
    assert!(journal.pending_archive_gc.is_empty());
    assert_eq!(
        journal
            .checkpoint
            .as_ref()
            .map(|checkpoint| checkpoint.catalog),
        Some(geometry_catalog_fingerprint(
            &kura
                .geometry_bindings(
                    &fixture.initial,
                    &fixture.initial_incarnations,
                    &fixture.initial_activations,
                )
                .expect("initial bindings")
        ))
    );
    let cached_after = kura.disk_usage.load(std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        cached_after,
        kura.kura_disk_usage_bytes().expect("exact usage scan")
    );
    assert!(cached_after < cached_before);
    assert_eq!(
        checkpoint_retired_geometry(&kura, &fixture, 20).expect("checkpoint replay is idempotent"),
        LaneGeometryGcSummary::default()
    );
    kura.recover_lane_geometry_journal(
        &fixture.initial,
        &fixture.initial_incarnations,
        &fixture.initial_activations,
    )
    .expect("new snapshot remains recoverable");
    kura.recover_lane_geometry_journal(
        &fixture.extended,
        &fixture.extended_incarnations,
        &fixture.extended_activations,
    )
    .expect_err("checkpointed-away old snapshot must not synthesize empty lane storage");
    drop(kura);
    let restarted = open_kura(&root, &fixture.initial);
    restarted
        .recover_lane_geometry_journal(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
        )
        .expect("restart recovers checkpoint-authoritative geometry");
}
#[test]
fn configured_primary_replay_preflight_is_read_only_when_floor_is_retained() {
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
    .expect("retain primary-to-extended transition");
    kura.mark_lane_geometry_catalog_published(
        &extended,
        &extended_incarnations,
        &extended_activations,
        None,
    )
    .expect("publish extended geometry");
    let journal_path = kura.lane_geometry_journal_path();
    let journal_before = fs::read(&journal_path).expect("retained geometry journal");
    let initial_bindings = kura
        .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
        .expect("configured-primary bindings");
    kura.preflight_lane_geometry_recovery_floor_with_lineage_root(
        &initial,
        &initial_incarnations,
        &initial_activations,
        unscoped_lineage_root(&initial_bindings),
    )
    .expect("retained transition must preserve the configured-primary replay floor");
    assert_eq!(
        fs::read(&journal_path).expect("journal after replay preflight"),
        journal_before,
        "replay preflight must not rewrite retained geometry"
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
    assert!(journal.checkpoint.is_none());
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
fn configured_primary_replay_preflight_requires_snapshot_after_compaction() {
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
    .expect("create geometry transition to compact");
    kura.mark_lane_geometry_catalog_published(
        &extended,
        &extended_incarnations,
        &extended_activations,
        None,
    )
    .expect("publish extended geometry");
    let (block_hash, state_hash) = durable_geometry_snapshot_identity(&kura, 20);
    let extended_bindings = kura
        .geometry_bindings(&extended, &extended_incarnations, &extended_activations)
        .expect("extended checkpoint bindings");
    let extended_lineage_root = unscoped_lineage_root(&extended_bindings);
    let (recovery_bindings, recovery_root) =
        geometry_fixture_recovery(&kura, 20, &extended_bindings, extended_lineage_root);
    let summary = kura
        .checkpoint_lane_geometry_with_proven_snapshot(
            extended_bindings,
            extended_lineage_root,
            recovery_bindings,
            recovery_root,
            20,
            Some(block_hash),
            state_hash,
            Vec::new(),
        )
        .expect("compact transition behind the extended snapshot");
    assert_eq!(summary.compacted_transitions, 1);
    let journal_path = kura.lane_geometry_journal_path();
    let journal_before = fs::read(&journal_path).expect("compacted geometry journal");
    let initial_bindings = kura
        .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
        .expect("configured-primary bindings");
    let error = kura
        .preflight_lane_geometry_recovery_floor_with_lineage_root(
            &initial,
            &initial_incarnations,
            &initial_activations,
            unscoped_lineage_root(&initial_bindings),
        )
        .expect_err("empty-state replay must not cross compacted geometry");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "state snapshot at height 20 is required because the configured-primary lane-geometry recovery floor was compacted",
    );
    assert_eq!(
        fs::read(&journal_path).expect("journal after rejected replay preflight"),
        journal_before,
        "rejected replay preflight must leave compacted geometry untouched"
    );
    kura.preflight_lane_geometry_recovery_floor_with_lineage_root(
        &extended,
        &extended_incarnations,
        &extended_activations,
        extended_lineage_root,
    )
    .expect("checkpoint-authoritative geometry remains a valid recovery floor");
    assert_eq!(
        fs::read(&journal_path).expect("journal after checkpoint preflight"),
        journal_before,
        "successful checkpoint preflight must also be read-only"
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
fn public_checkpoint_requires_exact_durable_block_and_wsv_identity() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    let (block_hash, state_hash) = durable_geometry_snapshot_identity(&kura, 20);
    kura.checkpoint_lane_geometry_after_durable_snapshot(
        &fixture.initial,
        &fixture.initial_incarnations,
        &fixture.initial_activations,
        20,
        Some(HashOf::from_untyped_unchecked(Hash::new(b"wrong-block"))),
        state_hash,
        &BTreeMap::new(),
    )
    .expect_err("mismatched canonical block hash must retain rollback evidence");
    assert!(fixture.retained_blocks.exists());
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("retained journal")
            .records
            .len(),
        2
    );
    kura.checkpoint_lane_geometry_after_durable_snapshot(
        &fixture.initial,
        &fixture.initial_incarnations,
        &fixture.initial_activations,
        20,
        Some(block_hash),
        Hash::new(b"wrong-state"),
        &BTreeMap::new(),
    )
    .expect_err("mismatched canonical state hash must retain rollback evidence");
    assert!(fixture.retained_blocks.exists());
    let summary = kura
        .checkpoint_lane_geometry_after_durable_snapshot(
            &fixture.initial,
            &fixture.initial_incarnations,
            &fixture.initial_activations,
            20,
            Some(block_hash),
            state_hash,
            &BTreeMap::new(),
        )
        .expect("exact durable snapshot identity permits GC");
    assert_eq!(summary.compacted_transitions, 2);
    assert_eq!(summary.removed_archive_roots, 1);
}
#[test]
fn pending_gc_rejoins_checkpoint_to_current_canonical_wsv_before_deletion() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
    checkpoint_retired_geometry(&kura, &fixture, 20)
        .expect_err("leave a durable pending deletion intent");
    let original_state_hash = kura
        .wsv_checkpoint(20)
        .expect("read WSV checkpoint")
        .expect("WSV checkpoint exists")
        .state_hash();
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(
        20,
        Hash::new(b"forked-state"),
        None,
    )
    .expect("replace WSV checkpoint for adversarial test");
    kura.resume_proven_lane_geometry_archive_gc()
        .expect_err("changed WSV identity must block replayed deletion");
    assert!(fixture.retained_blocks.exists());
    assert!(
        !kura
            .read_lane_geometry_journal()
            .expect("pending journal")
            .pending_archive_gc
            .is_empty()
    );
    kura.overwrite_wsv_checkpoint_without_validation_for_tests(20, original_state_hash, None)
        .expect("restore authoritative WSV checkpoint");
    let resumed = kura
        .resume_proven_lane_geometry_archive_gc()
        .expect("matching canonical WSV resumes deletion");
    assert_eq!(resumed.removed_archive_roots, 1);
    assert!(!fixture.retained_blocks.exists());
}
#[test]
fn pending_gc_rejects_ahead_missing_and_unbound_checkpoint_metadata() {
    for case in ["ahead", "missing", "unbound"] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(format!("kura-{case}"));
        let kura = open_kura(&root, &initial_and_extended_configs().0);
        let fixture = prepare_retired_geometry_archive(&kura, &root);
        kura.fail_next_lane_geometry_gc_at_stage_for_test(GC_FAIL_AFTER_COMPACTION_INTENT);
        checkpoint_retired_geometry(&kura, &fixture, 20)
            .expect_err("leave a durable pending deletion intent");
        let mut journal = kura.read_lane_geometry_journal().expect("pending journal");
        match case {
            "ahead" => {
                let checkpoint = journal.checkpoint.as_mut().expect("checkpoint");
                checkpoint.snapshot_height = 21;
                checkpoint.snapshot_block_hash =
                    Some(HashOf::from_untyped_unchecked(Hash::new(b"ahead-block")));
                checkpoint.snapshot_state_hash = Hash::new(b"ahead-state");
                checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
            }
            "missing" => journal.checkpoint = None,
            "unbound" => {
                let checkpoint = journal.checkpoint.as_mut().expect("checkpoint");
                checkpoint.pending_archive_gc_root = Some(Hash::new(b"wrong-gc-root"));
                checkpoint.commitment = geometry_checkpoint_commitment(checkpoint);
            }
            _ => unreachable!(),
        }
        fs::write(kura.lane_geometry_journal_path(), journal.encode())
            .expect("persist adversarial journal");
        kura.resume_proven_lane_geometry_archive_gc()
            .expect_err("invalid pending checkpoint metadata must fail closed");
        assert!(fixture.retained_blocks.exists());
    }
}
#[test]
fn checkpoint_rejects_stale_height_and_lane_incarnation_aba() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let fixture = prepare_retired_geometry_archive(&kura, &root);
    checkpoint_retired_geometry(&kura, &fixture, 20).expect("initial checkpoint");
    checkpoint_retired_geometry(&kura, &fixture, 19)
        .expect_err("older snapshot checkpoint must fail closed");
    let mut recreated_incarnations = fixture.extended_incarnations.clone();
    recreated_incarnations.insert(LaneId::new(1), Hash::prehashed([0x33; Hash::LENGTH]));
    let mut recreated_activations = fixture.extended_activations.clone();
    recreated_activations.insert(LaneId::new(1), 21);
    kura.apply_lane_geometry_transition_at_height(
        &fixture.initial,
        &fixture.extended,
        &fixture.initial_incarnations,
        &recreated_incarnations,
        &fixture.initial_activations,
        &recreated_activations,
        &BTreeSet::new(),
        21,
    )
    .expect("recreate lane id with fresh incarnation");
    kura.mark_lane_geometry_catalog_published(
        &fixture.extended,
        &recreated_incarnations,
        &recreated_activations,
        None,
    )
    .expect("publish recreated lane");
    let stale_bindings = kura
        .geometry_bindings(
            &fixture.extended,
            &fixture.extended_incarnations,
            &fixture.extended_activations,
        )
        .expect("stale bindings");
    let (block_hash, state_hash) = durable_geometry_snapshot_identity(&kura, 30);
    let stale_lineage_root = unscoped_lineage_root(&stale_bindings);
    let actual_bindings = kura
        .geometry_bindings(
            &fixture.extended,
            &recreated_incarnations,
            &recreated_activations,
        )
        .expect("current actual instance references");
    let actual_root = unscoped_lineage_root(&actual_bindings);
    let (recovery_bindings, recovery_root) =
        geometry_fixture_recovery(&kura, 30, &actual_bindings, actual_root);
    kura.checkpoint_lane_geometry_with_proven_snapshot(
        stale_bindings,
        stale_lineage_root,
        recovery_bindings,
        recovery_root,
        30,
        Some(block_hash),
        state_hash,
        Vec::new(),
    )
    .expect_err("same lane id with an old incarnation is not a reachable checkpoint");
    let recreated_bindings = kura
        .geometry_bindings(
            &fixture.extended,
            &recreated_incarnations,
            &recreated_activations,
        )
        .expect("recreated bindings");
    let recreated_lineage_root = unscoped_lineage_root(&recreated_bindings);
    let (recovery_bindings, recovery_root) =
        geometry_fixture_recovery(&kura, 30, &recreated_bindings, recreated_lineage_root);
    let summary = kura
        .checkpoint_lane_geometry_with_proven_snapshot(
            recreated_bindings,
            recreated_lineage_root,
            recovery_bindings,
            recovery_root,
            30,
            Some(block_hash),
            state_hash,
            Vec::new(),
        )
        .expect("fresh incarnation checkpoint");
    assert_eq!(summary.compacted_transitions, 1);
}

#[test]
fn fixture_lane_marker_provisions_paired_storage_and_preserves_existing_binding() {
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
    assert!(kura.binding_merge_path(&binding).is_file());
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
    kura.require_complete_geometry_binding_at(
        &fresh,
        &kura.binding_blocks_path(&fresh),
        &kura.binding_merge_path(&fresh),
    )
    .expect("fresh identity has its own complete pair");
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
    let _ = durable_geometry_snapshot_identity(&kura, 1);
    let (blocks, _) = Kura::canonical_storage_paths(&kura.store_root);
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
    let (blocks, merge) = Kura::canonical_storage_paths(&kura.store_root);
    Kura::reverify_canonical_blocks_open(&mut preflight, &blocks, false).unwrap();
    Kura::reverify_canonical_merge_open(&mut preflight, &merge, false).unwrap();
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
