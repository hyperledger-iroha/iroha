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
        let original_blocks = case_root.join("original-blocks");
        let original_merge = case_root.join("original-merge.log");
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
        let binding = LaneGeometryBinding {
            lane_id: LaneId::new(20),
            incarnation: Hash::new(label.as_bytes()),
            activation_height: 1,
            blocks_path: kura
                .relative_geometry_path(&original_blocks)
                .expect("relative original blocks"),
            merge_path: kura
                .relative_geometry_path(&original_merge)
                .expect("relative original merge"),
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
    let original_blocks = root.join("clear-temp/original-blocks");
    let original_merge = root.join("clear-temp/original-merge.log");
    let moved_blocks = root.join("clear-temp/moved-blocks");
    let moved_merge = root.join("clear-temp/moved-merge.log");
    let binding = LaneGeometryBinding {
        lane_id: LaneId::new(21),
        incarnation: Hash::new(b"clear-temp-direction-reversal"),
        activation_height: 1,
        blocks_path: kura
            .relative_geometry_path(&original_blocks)
            .expect("relative original blocks"),
        merge_path: kura
            .relative_geometry_path(&original_merge)
            .expect("relative original merge"),
    };
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
    let source_blocks = root.join("sealed-pair/live-blocks");
    let source_merge = root.join("sealed-pair/live-merge.log");
    let target_blocks = root.join("sealed-pair/archive-blocks");
    let target_merge = root.join("sealed-pair/archive-merge.log");
    let binding = LaneGeometryBinding {
        lane_id: LaneId::new(8),
        incarnation: Hash::new(b"immutable-retained-pair"),
        activation_height: 1,
        blocks_path: kura
            .relative_geometry_path(&source_blocks)
            .expect("relative source block path"),
        merge_path: kura
            .relative_geometry_path(&source_merge)
            .expect("relative source merge path"),
    };
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
    let source_blocks = root.join("sealed-block-pair/live-blocks");
    let source_merge = root.join("sealed-block-pair/live-merge.log");
    let target_blocks = root.join("sealed-block-pair/archive-blocks");
    let target_merge = root.join("sealed-block-pair/archive-merge.log");
    let binding = LaneGeometryBinding {
        lane_id: LaneId::new(9),
        incarnation: Hash::new(b"immutable-retained-block-image"),
        activation_height: 1,
        blocks_path: kura
            .relative_geometry_path(&source_blocks)
            .expect("relative source block path"),
        merge_path: kura
            .relative_geometry_path(&source_merge)
            .expect("relative source merge path"),
    };
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
    let mut journal = LaneGeometryJournal::default();
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
        .expect("recovery must finish marker-first staging and publish it atomically");
    let lane = extended.entry(LaneId::new(1)).expect("created lane");
    assert!(lane.blocks_dir(&root).join(MARKER_FILE_NAME).is_file());
    assert!(lane.merge_log_path(&root).is_file());
    assert!(!staged_blocks.exists());
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[0].phase,
        LaneGeometryPhase::CatalogPublished
    );
}
#[test]
fn replacement_rollback_finishes_merge_half_after_block_archive_crash() {
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
    let updated_blocks = kura.binding_blocks_path(updated);
    let updated_merge = kura.binding_merge_path(updated);
    let unpublished_blocks = kura
        .resolve_relative_path(&operation.unpublished_blocks_path)
        .expect("unpublished blocks");
    let unpublished_merge = kura
        .resolve_relative_path(&operation.unpublished_merge_path)
        .expect("unpublished merge");
    kura.seal_geometry_pair_move(
        updated,
        &updated_blocks,
        &updated_merge,
        &unpublished_blocks,
        &unpublished_merge,
    )
    .expect("seal replacement rollback move before its block half");
    kura.move_geometry_path(&updated_blocks, &unpublished_blocks, true)
        .expect("simulate crash after archiving replacement blocks only");
    assert!(!updated_blocks.exists());
    assert!(updated_merge.is_file());
    assert!(!unpublished_merge.exists());
    kura.recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
        .expect("rollback must finish the replacement merge half before restoring the prior lane");
    assert!(!updated_merge.exists());
    assert!(unpublished_blocks.is_dir());
    assert!(unpublished_merge.is_file());
    let active_lane = active.entry(LaneId::new(1)).expect("active lane");
    assert!(active_lane.blocks_dir(&root).is_dir());
    assert!(active_lane.merge_log_path(&root).is_file());
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[1].phase,
        LaneGeometryPhase::RolledBack
    );
    // Replacement replay has the same pre-first-rename frontier as Create: the retained
    // updated incarnation can already carry its exact live-target seal while the journal is
    // still terminally `RolledBack`. Retrying the replacement authority must consume it.
    kura.seal_geometry_pair_move(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &updated_blocks,
        &updated_merge,
    )
    .expect("inject replacement replay crash before first rename");
    kura.recover_lane_geometry_journal(
        &replacement,
        &replacement_incarnations,
        &replacement_activations,
    )
    .expect("same-authority replacement replay resumes its pre-rename seal");
    kura.require_complete_geometry_binding_at(updated, &updated_blocks, &updated_merge)
        .expect("replacement incarnation is live after replay");
    assert!(!unpublished_blocks.exists());
    assert!(!unpublished_merge.exists());
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[1].phase,
        LaneGeometryPhase::CatalogPublished
    );
    kura.recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
        .expect("return replayed replacement to its retained rollback image");
    kura.require_sealed_geometry_pair_at(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &unpublished_blocks,
        &unpublished_merge,
    )
    .expect("replacement lifecycle restores an immutable rollback image");
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[1].phase,
        LaneGeometryPhase::RolledBack
    );
    let previous = operation.previous.as_ref().expect("previous binding");
    assert_ne!(updated_blocks, kura.binding_blocks_path(previous));
    assert_ne!(updated_merge, kura.binding_merge_path(previous));
    fs::create_dir_all(&updated_blocks).expect("create duplicate updated block path");
    fs::copy(
        unpublished_blocks.join(MARKER_FILE_NAME),
        updated_blocks.join(MARKER_FILE_NAME),
    )
    .expect("copy duplicate updated marker");
    create_dir_all_with_context(updated_merge.parent().expect("updated merge parent"))
        .expect("create duplicate updated merge parent");
    fs::copy(&unpublished_merge, &updated_merge).expect("copy duplicate updated merge log");
    let error = kura
        .recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
        .expect_err("rolled-back replacement must reject duplicate updated live storage");
    assert_geometry_io_error(
        &error,
        ErrorKind::AlreadyExists,
        "replacement rollback left the updated incarnation live",
    );
    assert!(updated_blocks.is_dir());
    assert!(updated_merge.is_file());
    assert!(unpublished_blocks.is_dir());
    assert!(unpublished_merge.is_file());
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[1].phase,
        LaneGeometryPhase::RolledBack
    );
}
#[test]
fn replacement_intent_rollback_resumes_block_only_inverse_half() {
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
    let updated_blocks = kura.binding_blocks_path(updated);
    let updated_merge = kura.binding_merge_path(updated);
    let unpublished_blocks = kura
        .resolve_relative_path(&operation.unpublished_blocks_path)
        .expect("unpublished blocks");
    let unpublished_merge = kura
        .resolve_relative_path(&operation.unpublished_merge_path)
        .expect("unpublished merge");
    kura.seal_geometry_pair_move(
        updated,
        &updated_blocks,
        &updated_merge,
        &unpublished_blocks,
        &unpublished_merge,
    )
    .expect("seal Intent rollback before its block half");
    kura.move_geometry_path(&updated_blocks, &unpublished_blocks, true)
        .expect("simulate Intent rollback crash after moving only blocks");
    kura.recover_lane_geometry_journal(&active, &active_incarnations, &active_activations)
        .expect("Intent retry must resume its own inverse merge half");
    assert!(!updated_blocks.exists());
    assert!(!updated_merge.exists());
    kura.require_sealed_geometry_pair_at(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &unpublished_blocks,
        &unpublished_merge,
    )
    .expect("Intent rollback retains the exact updated image");
    let previous = operation.previous.as_ref().expect("previous binding");
    kura.require_complete_geometry_binding_at(
        previous,
        &kura.binding_blocks_path(previous),
        &kura.binding_merge_path(previous),
    )
    .expect("previous replacement image restored");
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[1].phase,
        LaneGeometryPhase::RolledBack
    );
}
#[test]
fn same_path_replacement_rollback_preserves_old_merge_after_forward_half_archive() {
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
    assert_eq!(previous.blocks_path, updated.blocks_path);
    assert_eq!(previous.merge_path, updated.merge_path);
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
    let archived_blocks = kura
        .resolve_relative_path(&operation.archived_blocks_path)
        .expect("archived blocks");
    let archived_merge = kura
        .resolve_relative_path(&operation.archived_merge_path)
        .expect("archived merge");
    let unpublished_merge = kura
        .resolve_relative_path(&operation.unpublished_merge_path)
        .expect("unpublished merge");
    let unpublished_blocks = kura
        .resolve_relative_path(&operation.unpublished_blocks_path)
        .expect("unpublished blocks");
    let sentinel = b"old-merge-half-must-remain-live";
    fs::write(&previous_merge, sentinel).expect("write old merge sentinel");
    kura.seal_geometry_pair_move(
        previous,
        &previous_blocks,
        &previous_merge,
        &archived_blocks,
        &archived_merge,
    )
    .expect("seal previous archive move before its block half");
    kura.move_geometry_path(&previous_blocks, &archived_blocks, true)
        .expect("simulate crash after archiving only old blocks");
    assert!(!previous_blocks.exists());
    assert!(previous_merge.is_file());
    assert!(!archived_merge.exists());
    kura.recover_lane_geometry_journal_at_height(
        &active,
        &active_incarnations,
        &active_activations,
        4,
    )
    .expect("rollback must recognize the shared live merge as the old half");
    assert!(previous_blocks.is_dir());
    assert_eq!(
        fs::read(&previous_merge).expect("old merge restored"),
        sentinel
    );
    kura.require_sealed_geometry_pair_at(
        updated,
        &unpublished_blocks,
        &unpublished_merge,
        &unpublished_blocks,
        &unpublished_merge,
    )
    .expect("rollback retains an authenticated empty replacement image");
    assert_eq!(
        kura.read_lane_geometry_journal().expect("journal").records[1].phase,
        LaneGeometryPhase::RolledBack
    );
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
    assert!(!lane1.blocks_dir(&root).exists());
    kura.recover_lane_geometry_journal_at_height_with_lineage_root(
        &extended,
        &second_incarnations,
        &second_activations,
        10,
        lineage_second_active,
    )
    .expect("restore second active lineage after exact rooted rollback");
    assert!(lane1.blocks_dir(&root).exists());
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
fn primary_relabel_files_applied_restart_recovers_exact_chain() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let initial_catalog = configured_primary_catalog("primary-alpha");
    let updated_catalog = configured_primary_catalog("primary-beta");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let updated = RuntimeLaneConfig::from_catalog(&updated_catalog);
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    let _ = durable_geometry_snapshot_identity(&kura, 3);
    let expected_hashes = (1..=3)
        .map(|height| {
            kura.get_durable_block_hash(NonZeroUsize::new(height).expect("non-zero"))
                .expect("durable block hash")
        })
        .collect::<Vec<_>>();
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &incarnations,
        &activations,
        &activations,
        &BTreeSet::new(),
    )
    .expect("durably move primary files before catalog publication");
    assert_eq!(
        kura.read_lane_geometry_journal().unwrap().records[0].phase,
        LaneGeometryPhase::FilesApplied
    );
    let old_blocks = initial.primary().blocks_dir(&root);
    let new_blocks = updated.primary().blocks_dir(&root);
    assert!(!old_blocks.exists());
    assert!(new_blocks.exists());
    drop(kura);
    let reopened = open_kura(&root, &initial);
    assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 3);
    assert_eq!(*reopened.active_blocks_dir.lock(), new_blocks);
    assert!(
        !old_blocks.exists(),
        "startup must not provision an empty old path"
    );
    for (height, expected) in (1..=3).zip(expected_hashes) {
        assert_eq!(
            reopened.get_durable_block_hash(NonZeroUsize::new(height).unwrap()),
            Some(expected)
        );
    }
    reopened
        .recover_lane_geometry_journal(&initial, &incarnations, &activations)
        .expect("authoritative old catalog rolls the durable intent back");
    assert_eq!(*reopened.active_blocks_dir.lock(), old_blocks);
    assert!(old_blocks.exists());
    assert!(!new_blocks.exists());
    assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 3);
}
#[test]
fn two_lane_relabel_files_applied_restart_recovers_exact_chain() {
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
    let initial_primary = initial.primary();
    let initial_secondary = initial
        .entry(LaneId::new(1))
        .expect("initial secondary lane");
    let updated_primary = updated.primary();
    let updated_secondary = updated
        .entry(LaneId::new(1))
        .expect("updated secondary lane");
    let old_primary_blocks = initial_primary.blocks_dir(&root);
    let old_primary_merge = initial_primary.merge_log_path(&root);
    let old_secondary_blocks = initial_secondary.blocks_dir(&root);
    let old_secondary_merge = initial_secondary.merge_log_path(&root);
    let new_primary_blocks = updated_primary.blocks_dir(&root);
    let new_primary_merge = updated_primary.merge_log_path(&root);
    let new_secondary_blocks = updated_secondary.blocks_dir(&root);
    let new_secondary_merge = updated_secondary.merge_log_path(&root);
    let secondary_sentinel = "secondary-lane-sentinel";
    fs::write(
        old_secondary_blocks.join(secondary_sentinel),
        b"secondary-block-state",
    )
    .expect("seed secondary block state");
    fs::write(&old_secondary_merge, b"secondary-merge-state").expect("seed secondary merge state");
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &incarnations,
        &activations,
        &activations,
        &BTreeSet::new(),
    )
    .expect("durably relabel both lanes before catalog publication");
    let journal = kura.read_lane_geometry_journal().expect("geometry journal");
    assert_eq!(journal.records[0].phase, LaneGeometryPhase::FilesApplied);
    assert_eq!(journal.records[0].operations.len(), 2);
    assert!(
        journal.records[0]
            .operations
            .iter()
            .all(|operation| operation.kind == LaneGeometryOperationKind::Relabel),
        "both lane operations must be relabels"
    );
    for path in [
        &old_primary_blocks,
        &old_primary_merge,
        &old_secondary_blocks,
        &old_secondary_merge,
    ] {
        assert!(!path.exists(), "old lane path must be consumed: {path:?}");
    }
    for path in [
        &new_primary_blocks,
        &new_primary_merge,
        &new_secondary_blocks,
        &new_secondary_merge,
    ] {
        assert!(path.exists(), "updated lane path must exist: {path:?}");
    }
    assert_eq!(exact_chain(&kura), expected_chain);
    drop(kura);
    let reopened = open_kura(&root, &initial);
    assert_eq!(
        fs::canonicalize(reopened.active_blocks_dir.lock().as_path())
            .expect("canonical active primary blocks path"),
        fs::canonicalize(&new_primary_blocks).expect("canonical updated primary blocks path")
    );
    assert_eq!(
        fs::canonicalize(reopened.active_merge_path.lock().as_path())
            .expect("canonical active primary merge path"),
        fs::canonicalize(&new_primary_merge).expect("canonical updated primary merge path")
    );
    for path in [
        &old_primary_blocks,
        &old_primary_merge,
        &old_secondary_blocks,
        &old_secondary_merge,
    ] {
        assert!(
            !path.exists(),
            "startup must not prematurely recreate a journal-owned old path: {path:?}"
        );
    }
    assert_eq!(
        fs::read(new_secondary_blocks.join(secondary_sentinel))
            .expect("read relabelled secondary block state"),
        b"secondary-block-state"
    );
    assert_eq!(
        fs::read(&new_secondary_merge).expect("read relabelled secondary merge state"),
        b"secondary-merge-state"
    );
    assert_eq!(exact_chain(&reopened), expected_chain);
    reopened
        .recover_lane_geometry_journal(&initial, &incarnations, &activations)
        .expect("authoritative old catalog rolls both relabels back");
    reopened
        .recover_lane_geometry_journal(&initial, &incarnations, &activations)
        .expect("two-lane rollback recovery is idempotent");
    assert_eq!(
        fs::canonicalize(reopened.active_blocks_dir.lock().as_path())
            .expect("canonical recovered primary blocks path"),
        fs::canonicalize(&old_primary_blocks).expect("canonical old primary blocks path")
    );
    assert_eq!(
        fs::canonicalize(reopened.active_merge_path.lock().as_path())
            .expect("canonical recovered primary merge path"),
        fs::canonicalize(&old_primary_merge).expect("canonical old primary merge path")
    );
    for path in [
        &old_primary_blocks,
        &old_primary_merge,
        &old_secondary_blocks,
        &old_secondary_merge,
    ] {
        assert!(path.exists(), "rolled-back lane path must exist: {path:?}");
    }
    for path in [
        &new_primary_blocks,
        &new_primary_merge,
        &new_secondary_blocks,
        &new_secondary_merge,
    ] {
        assert!(
            !path.exists(),
            "updated lane path must be consumed: {path:?}"
        );
    }
    assert_eq!(
        fs::read(old_secondary_blocks.join(secondary_sentinel))
            .expect("read recovered secondary block state"),
        b"secondary-block-state"
    );
    assert_eq!(
        fs::read(&old_secondary_merge).expect("read recovered secondary merge state"),
        b"secondary-merge-state"
    );
    assert_eq!(
        reopened
            .read_lane_geometry_journal()
            .expect("recovered geometry journal")
            .records[0]
            .phase,
        LaneGeometryPhase::RolledBack
    );
    assert_eq!(
        reopened.exact_durable_blocks_count().unwrap(),
        expected_chain.len()
    );
    assert_eq!(exact_chain(&reopened), expected_chain);
}
struct PrimaryRelabelResumeGuard<'a>(&'a std::sync::atomic::AtomicBool);
impl Drop for PrimaryRelabelResumeGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, std::sync::atomic::Ordering::Release);
    }
}
#[test]
fn primary_relabel_reader_blocks_until_retarget() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let initial = RuntimeLaneConfig::from_catalog(&configured_primary_catalog("reader-alpha"));
    let updated = RuntimeLaneConfig::from_catalog(&configured_primary_catalog("reader-beta"));
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    let _ = durable_geometry_snapshot_identity(&kura, 1);
    let expected = kura
        .get_durable_block_hash(nonzero!(1_usize))
        .expect("durable block hash");
    kura.block_data.lock()[0].1 = None;
    kura.pause_primary_relabel_before_retarget
        .store(true, std::sync::atomic::Ordering::Release);
    thread::scope(|scope| {
        let transition = scope.spawn(|| {
            kura.apply_lane_geometry_transition(
                &initial,
                &updated,
                &incarnations,
                &incarnations,
                &activations,
                &activations,
                &BTreeSet::new(),
            )
        });
        let resume_guard = PrimaryRelabelResumeGuard(&kura.primary_relabel_paused);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !kura
            .primary_relabel_paused
            .load(std::sync::atomic::Ordering::Acquire)
        {
            assert!(Instant::now() < deadline, "primary relabel did not pause");
            thread::yield_now();
        }
        assert!(
            kura.block_store.try_lock().is_none(),
            "the canonical BlockStore guard must span rename through retarget"
        );
        let (reader_tx, reader_rx) = mpsc::channel();
        let reader_kura = Arc::clone(&kura);
        let reader = scope.spawn(move || {
            let block = reader_kura.get_block(nonzero!(1_usize));
            reader_tx.send(block).expect("send reader result");
        });
        assert!(
            reader_rx.recv_timeout(Duration::from_millis(50)).is_err(),
            "reader must not reopen the old path while relabel is between rename and retarget"
        );
        drop(resume_guard);
        transition
            .join()
            .expect("transition thread")
            .expect("journaled primary relabel");
        let block = reader_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("reader completes after retarget")
            .expect("canonical block remains readable");
        assert_eq!(block.hash(), expected);
        reader.join().expect("reader thread");
    });
}
#[test]
fn lane_geometry_recovery_holds_sidecar_lock() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let initial = RuntimeLaneConfig::from_catalog(&configured_primary_catalog("lock-alpha"));
    let updated = RuntimeLaneConfig::from_catalog(&configured_primary_catalog("lock-beta"));
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    kura.apply_lane_geometry_transition(
        &initial,
        &updated,
        &incarnations,
        &incarnations,
        &activations,
        &activations,
        &BTreeSet::new(),
    )
    .expect("apply primary relabel");
    kura.pause_primary_relabel_before_retarget
        .store(true, std::sync::atomic::Ordering::Release);
    thread::scope(|scope| {
        let recovery = scope
            .spawn(|| kura.recover_lane_geometry_journal(&initial, &incarnations, &activations));
        let resume_guard = PrimaryRelabelResumeGuard(&kura.primary_relabel_paused);
        let deadline = Instant::now() + Duration::from_secs(5);
        while !kura
            .primary_relabel_paused
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
            .expect("recover primary relabel");
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
        two.entry(LaneId::new(2))
            .expect("lane two")
            .blocks_dir(&root)
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
    let mut stale_incarnations = extended_incarnations.clone();
    stale_incarnations.insert(LaneId::new(1), Hash::prehashed([0x77; Hash::LENGTH]));
    let stale = kura
        .geometry_binding(
            extended.entry(LaneId::new(1)).expect("lane one"),
            &stale_incarnations,
            &extended_activations,
        )
        .expect("stale binding");
    kura.write_lane_marker(&stale).expect("write stale marker");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("stale incarnation marker must fail closed");
}
#[test]
fn transition_rejects_reserved_archive_collision_before_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    let previous_bindings = kura
        .geometry_bindings(&initial, &initial_incarnations, &initial_activations)
        .expect("initial bindings");
    let updated_bindings = kura
        .geometry_bindings(&extended, &extended_incarnations, &extended_activations)
        .expect("updated bindings");
    let transition = geometry_transition_id(
        0,
        0,
        geometry_catalog_fingerprint(&previous_bindings),
        unscoped_lineage_root(&previous_bindings),
        geometry_catalog_fingerprint(&updated_bindings),
        unscoped_lineage_root(&updated_bindings),
    );
    let collision = root
        .join("retired/lane_geometry")
        .join(hex::encode(transition.as_ref()))
        .join("lane_0000000001/previous_blocks");
    fs::create_dir_all(&collision).expect("seed archive collision");
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect_err("archive collision must fail before applying files");
    assert!(
        !extended
            .entry(LaneId::new(1))
            .expect("lane one")
            .blocks_dir(&root)
            .exists()
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
    let target = extended
        .entry(LaneId::new(1))
        .expect("lane one")
        .blocks_dir(&root);
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
    let kura = open_kura(&root, &initial_and_extended_configs().0);
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
    assert_eq!(summary.removed_archive_roots, 2);
    assert!(
        summary.reclaimed_bytes
            >= u64::try_from(GC_PAYLOAD_LEN).expect("GC payload length fits u64")
    );
    assert!(!fixture.archive_root.exists());
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
    let summary = kura
        .checkpoint_lane_geometry_with_proven_snapshot(
            extended_bindings,
            extended_lineage_root,
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
    assert!(fixture.archive_root.exists());
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
    assert!(fixture.archive_root.exists());
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
    assert_eq!(summary.removed_archive_roots, 2);
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
    assert!(fixture.archive_root.exists());
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
    assert_eq!(resumed.removed_archive_roots, 2);
    assert!(!fixture.archive_root.exists());
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
        assert!(fixture.archive_root.exists());
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
    kura.checkpoint_lane_geometry_with_proven_snapshot(
        stale_bindings,
        stale_lineage_root,
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
    let summary = kura
        .checkpoint_lane_geometry_with_proven_snapshot(
            recreated_bindings,
            recreated_lineage_root,
            30,
            Some(block_hash),
            state_hash,
            Vec::new(),
        )
        .expect("fresh incarnation checkpoint");
    assert_eq!(summary.compacted_transitions, 1);
}
