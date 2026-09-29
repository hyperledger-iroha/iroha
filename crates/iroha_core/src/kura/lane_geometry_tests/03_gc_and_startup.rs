#[cfg(unix)]
#[test]
fn geometry_sidecar_temp_symlink_and_regular_collision_fail_without_clobbering() {
    use std::os::unix::fs::symlink;
    for collision_kind in ["symlink", "regular"] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join(format!("kura-{collision_kind}"));
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let collision = root.join(JOURNAL_TEMP_FILE_NAME);
        let outside = temp.path().join("operator-data");
        fs::write(&outside, b"operator-owned").expect("outside sentinel");
        if collision_kind == "symlink" {
            symlink(&outside, &collision).expect("journal temp symlink");
        } else {
            fs::write(&collision, b"operator-owned").expect("journal temp collision");
        }
        kura.apply_lane_geometry_transition(
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
            &BTreeSet::new(),
        )
        .expect_err("unsafe or unrelated temp collision must fail closed");
        assert_eq!(
            fs::read(&outside).expect("outside retained"),
            b"operator-owned"
        );
        if collision_kind == "regular" {
            assert_eq!(
                fs::read(&collision).expect("regular collision retained"),
                b"operator-owned"
            );
        }
    }
}

#[test]
fn geometry_inode_identity_detects_path_replacement() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let path = root.join("inode-guard.norito");
    fs::write(&path, b"first").expect("first inode");
    let identity = kura
        .geometry_path_identity(&path, false)
        .expect("capture first inode");
    fs::rename(&path, root.join("inode-guard.old")).expect("move first inode");
    fs::write(&path, b"second").expect("replacement inode");
    kura.require_geometry_path_identity(&path, false, identity)
        .expect_err("replacement inode must not pass identity revalidation");
}

#[test]
fn recovery_rejects_pre_release_journal_layout() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::kura::lane_geometry::tests::recovery_rejects_pre_release_journal_layout::PreReleaseLaneGeometryJournal"
    )]
    #[derive(Encode)]
    struct PreReleaseLaneGeometryJournal {
        version: u8,
        records: Vec<LaneGeometryIntent>,
    }
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let kura = open_kura(&root, &initial);
    let pre_release = PreReleaseLaneGeometryJournal {
        version: 1,
        records: Vec::new(),
    };
    fs::write(kura.lane_geometry_journal_path(), pre_release.encode())
        .expect("write pre-release journal");
    kura.read_lane_geometry_journal()
        .expect_err("pre-release journal layout must fail closed");
}

#[test]
fn recovery_rejects_corrupt_and_forged_journals() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    let pristine = fs::read(kura.lane_geometry_journal_path())
        .expect("retain the authenticated fixture baseline");
    fs::write(kura.lane_geometry_journal_path(), b"not norito").expect("write corrupt journal");
    kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
        .expect_err("corrupt journal must fail closed");
    fs::write(kura.lane_geometry_journal_path(), pristine)
        .expect("restore the exact authenticated baseline after the corruption control");
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("prepare valid journal");
    let valid = kura.read_lane_geometry_journal().expect("valid journal");
    let mut forged_root = valid.clone();
    forged_root.records[0].updated_lineage_root = Hash::new(b"forged-lineage-root");
    fs::write(kura.lane_geometry_journal_path(), forged_root.encode())
        .expect("write forged lineage root");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("lineage-root tampering must invalidate the transition id");
    let mut forged_sequence = valid.clone();
    forged_sequence.records[0].transition_sequence = forged_sequence.records[0]
        .transition_sequence
        .checked_add(1)
        .expect("test transition sequence");
    fs::write(kura.lane_geometry_journal_path(), forged_sequence.encode())
        .expect("write forged transition sequence");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("transition-sequence tampering must invalidate the transition id");
    let mut forged_height = valid.clone();
    forged_height.records[0].transition_height = forged_height.records[0]
        .transition_height
        .checked_add(1)
        .expect("test transition height");
    fs::write(kura.lane_geometry_journal_path(), forged_height.encode())
        .expect("write forged transition height");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("transition-height tampering must invalidate the transition id");
    fs::write(kura.lane_geometry_journal_path(), valid.encode()).expect("restore valid journal");
    let mut forged = valid;
    forged.records[0].operations[0].created.blocks_path = "../escape".to_owned();
    fs::write(kura.lane_geometry_journal_path(), forged.encode())
        .expect("write forged journal bytes");
    kura.recover_lane_geometry_journal(&extended, &extended_incarnations, &extended_activations)
        .expect_err("forged archive path must fail closed");
}

#[test]
fn recovery_rejects_noncontiguous_phase_frontiers() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let kura = open_kura(&root, &initial_and_extended_configs().0);
    let _fixture = prepare_retired_geometry_archive(&kura, &root);
    let valid = kura
        .read_lane_geometry_journal()
        .expect("two-transition published journal");
    assert_eq!(valid.records.len(), 2);
    for (_label, phases, expected_message) in [
        (
            "published-after-rollback",
            [
                LaneGeometryPhase::RolledBack,
                LaneGeometryPhase::CatalogPublished,
            ],
            "lane geometry journal phases do not form a durable applied frontier",
        ),
        (
            "multiple-uncertain-boundaries",
            [LaneGeometryPhase::Intent, LaneGeometryPhase::FilesApplied],
            "lane geometry journal has more than one uncertain transition boundary",
        ),
    ] {
        let mut forged = valid.clone();
        for (record, phase) in forged.records.iter_mut().zip(phases) {
            record.phase = phase;
        }
        fs::write(kura.lane_geometry_journal_path(), forged.encode())
            .expect("write phase-frontier forgery");
        let error = kura
            .read_lane_geometry_journal()
            .expect_err("impossible phase topology must fail closed");
        assert_geometry_io_error(&error, ErrorKind::InvalidData, expected_message);
    }
}

#[test]
fn recovery_rejects_both_branch_v5_journal_layouts_without_migration() {
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::kura::lane_geometry::tests::recovery_rejects_both_branch_v5_journal_layouts_without_migration::HeightCursorJournalV5"
    )]
    #[derive(Encode)]
    struct HeightCursorJournalV5 {
        version: u8,
        configured_catalog_hash: Option<Hash>,
        configured_primary_binding: Option<LaneGeometryBinding>,
        checkpoint: Option<Vec<u8>>,
        pending_archive_gc: Vec<Vec<u8>>,
        records: Vec<LaneGeometryIntent>,
    }
    #[derive(norito::NoritoSchema)]
    #[norito_schema(
        name = "iroha_core::kura::lane_geometry::tests::recovery_rejects_both_branch_v5_journal_layouts_without_migration::LineageJournalV5"
    )]
    #[derive(Encode)]
    struct LineageJournalV5 {
        version: u8,
        configured_catalog_hash: Option<Hash>,
        // These containers are empty below, so their bytes exactly match the lineage
        // branch's checkpoint and transition container encodings.
        checkpoint: Option<Vec<u8>>,
        pending_archive_gc: Vec<Vec<u8>>,
        records: Vec<LaneGeometryIntent>,
    }
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    let obsolete_layouts = [
        (
            "height-cursor v5",
            HeightCursorJournalV5 {
                version: 5,
                configured_catalog_hash: None,
                configured_primary_binding: None,
                checkpoint: None,
                pending_archive_gc: Vec::new(),
                records: Vec::new(),
            }
            .encode(),
        ),
        (
            "lineage v5",
            LineageJournalV5 {
                version: 5,
                configured_catalog_hash: Some(Hash::new(b"lineage-v5")),
                checkpoint: None,
                pending_archive_gc: Vec::new(),
                records: Vec::new(),
            }
            .encode(),
        ),
    ];
    for (name, bytes) in obsolete_layouts {
        let journal_path = kura.lane_geometry_journal_path();
        fs::write(&journal_path, &bytes).expect("write obsolete v5 journal");
        let error = match kura.recover_lane_geometry_journal(
            &initial,
            &initial_incarnations,
            &initial_activations,
        ) {
            Ok(()) => panic!("{name} must not be migrated to the current journal"),
            Err(error) => error,
        };
        assert_eq!(
            fs::read(&journal_path).expect("read rejected v5 journal"),
            bytes,
            "recovery must leave the rejected {name} bytes untouched"
        );
        if name == "height-cursor v5" {
            assert_kura_io_error(
                &error,
                std::io::ErrorKind::InvalidData,
                &format!("unsupported lane geometry journal version 5; expected {JOURNAL_VERSION}"),
            );
        }
    }
}

#[test]
fn recovery_rejects_retired_checkpoint_journal_without_repair() {
    #[derive(Encode)]
    struct RetiredCheckpointJournal {
        version: u8,
        configured_catalog_hash: Option<Hash>,
        configured_primary_binding: Option<LaneGeometryBinding>,
        checkpoint: Option<Vec<u8>>,
        records: Vec<LaneGeometryIntent>,
    }
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &incarnations);
    let journal = kura.read_lane_geometry_journal().unwrap();
    for checkpoint in [None, Some(vec![1, 2, 3])] {
        let old = RetiredCheckpointJournal {
            version: JOURNAL_VERSION,
            configured_catalog_hash: journal.configured_catalog_hash,
            configured_primary_binding: journal.configured_primary_binding.clone(),
            checkpoint,
            records: Vec::new(),
        };
        let bytes = old.encode();
        fs::write(kura.lane_geometry_journal_path(), &bytes).unwrap();
        assert!(
            kura.recover_lane_geometry_journal(&initial, &incarnations, &activations)
                .is_err()
        );
        assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), bytes);
    }
}

#[test]
fn configured_catalog_preflight_persists_baseline_before_any_lane_path() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured_a = configured_primary_catalog("crash-a");
    let configured_b = configured_primary_catalog("crash-b");
    let lane_config_a = RuntimeLaneConfig::from_catalog(&configured_a);
    let lane_config_b = RuntimeLaneConfig::from_catalog(&configured_b);
    let config = kura_config(&root);
    Kura::fail_after_configured_catalog_preflight_for_test(&root);
    let error = Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
        .expect_err("injected crash must stop immediately after baseline establishment");
    assert!(matches!(
        error,
        Error::IO(ref source, _) if source.kind() == ErrorKind::Interrupted
    ));
    assert_lane_paths_absent(&root, &lane_config_a);
    let journal = decode_exact::<LaneGeometryJournal>(
        &fs::read(root.join(JOURNAL_FILE_NAME)).expect("durable baseline journal"),
    )
    .expect("decode durable baseline journal");
    assert_eq!(
        journal.configured_catalog_hash,
        Some(LaneLifecycleParameterV1::catalog_hash(&configured_a))
    );
    Kura::new_with_configured_lane_catalog(&config, &lane_config_b, &configured_b)
        .expect_err("a reconstructed process must reject a different configured catalog");
    assert_lane_paths_absent(&root, &lane_config_b);
    Kura::new_with_configured_lane_catalog(&config, &lane_config_a, &configured_a)
        .expect("the exact configured catalog must resume after the crash boundary");
}

#[cfg(unix)]
#[test]
fn authenticated_primary_admission_rejects_block_path_symlink_before_external_write() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-blocks");
    fs::create_dir_all(&outside).expect("outside directory");
    let configured = configured_primary_catalog("primary-block-symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(
        &root,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("establish configured-catalog baseline");
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open canonical storage before authenticated lane admission");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network");
    let incarnation = Hash::prehashed([0xA6; Hash::LENGTH]);
    let incarnations = BTreeMap::from([(LaneId::SINGLE, incarnation)]);
    let activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let blocks = geometry_fixture_blocks(&kura, lane_config.primary(), &incarnations, &activations);
    fs::create_dir_all(blocks.parent().expect("block parent")).expect("block parent");
    symlink(&outside, &blocks).expect("configured primary block symlink");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnation,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect_err("exact primary admission must reject the substituted path before opening it");
    assert!(blocks.is_symlink());
    assert_eq!(
        fs::read_dir(&outside).expect("outside directory").count(),
        0,
        "preflight rejection must not create block-store files outside the Kura root"
    );
    assert!(!root.join("merge_ledger").exists());
}

#[cfg(unix)]
#[test]
fn authenticated_primary_admission_rejects_base_file_symlink_before_external_write() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-data");
    fs::write(&outside, b"operator-owned").expect("outside data sentinel");
    let configured = configured_primary_catalog("primary-data-symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(
        &root,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("establish configured-catalog baseline");
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open canonical storage before authenticated lane admission");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network");
    let incarnation = Hash::prehashed([0xA6; Hash::LENGTH]);
    let incarnations = BTreeMap::from([(LaneId::SINGLE, incarnation)]);
    let activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let blocks = geometry_fixture_blocks(&kura, lane_config.primary(), &incarnations, &activations);
    fs::create_dir_all(&blocks).expect("block instance directory");
    let data = blocks.join(DATA_FILE_NAME);
    symlink(&outside, &data).expect("configured primary data symlink");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnation,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect_err("exact primary admission must reject the substituted path before opening it");
    assert!(data.is_symlink());
    assert_eq!(
        fs::read(&outside).expect("outside sentinel"),
        b"operator-owned"
    );
    assert!(!blocks.join(MARKER_FILE_NAME).exists());
}

#[cfg(unix)]
#[test]
fn configured_primary_preflight_rejects_core_block_file_symlinks_before_external_write() {
    use std::os::unix::fs::symlink;
    for file_name in [
        INDEX_FILE_NAME,
        DATA_FILE_NAME,
        HASHES_FILE_NAME,
        COUNT_FILE_NAME,
    ] {
        let temp = TempDir::new().expect("temporary directory");
        let root = temp.path().join("kura");
        let outside = temp.path().join(format!("outside-{file_name}"));
        fs::write(&outside, b"operator-owned-block-file").expect("outside sentinel");
        let configured = configured_primary_catalog("child-link");
        let lane_config = RuntimeLaneConfig::from_catalog(&configured);
        let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
        let incarnation = Hash::prehashed([0xA7; Hash::LENGTH]);
        let (kura, _) =
            Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
                .expect("open authenticated configured Kura");
        kura.bind_lane_storage_network(geometry_fixture_network_id())
            .expect("bind explicit fixture network before geometry authority");
        kura.establish_or_verify_configured_primary_geometry_anchor(
            lane_config.primary(),
            incarnation,
            baseline,
        )
        .expect("bind configured primary");
        let child = geometry_fixture_blocks(
            &kura,
            lane_config.primary(),
            &BTreeMap::from([(LaneId::SINGLE, incarnation)]),
            &BTreeMap::from([(LaneId::SINGLE, 0)]),
        )
        .join(file_name);
        drop(kura);
        fs::remove_file(&child).expect("remove core block file before symlink injection");
        symlink(&outside, &child).expect("inject core block-file symlink");
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("configured primary descendants must be rejected before BlockStore opens");
        assert!(child.is_symlink());
        assert_eq!(
            fs::read(&outside).expect("outside sentinel retained"),
            b"operator-owned-block-file",
            "outside target changed for {file_name}"
        );
    }
}

#[cfg(unix)]
#[test]
fn configured_primary_preflight_rejects_retired_root_artifact_symlink() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-retired-artifact");
    fs::write(&outside, b"operator-owned-retired-artifact").expect("outside sentinel");
    let configured = configured_primary_catalog("root-sidecar-link");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open authenticated configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        Hash::prehashed([0xA8; Hash::LENGTH]),
        baseline,
    )
    .expect("bind configured primary");
    drop(kura);
    let sidecar_temp = root.join("commit-rosters.norito.tmp");
    symlink(&outside, &sidecar_temp).expect("inject retired-artifact symlink");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("retired root artifact must fail before configured-lane reconciliation");
    assert!(sidecar_temp.is_symlink());
    assert_eq!(
        fs::read(&outside).expect("outside sentinel retained"),
        b"operator-owned-retired-artifact"
    );
}

#[test]
fn configured_primary_preflight_rejects_foreign_marker_before_kura_reconciliation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("primary-marker");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("open configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    let incarnation = Hash::prehashed([0xA1; Hash::LENGTH]);
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        incarnation,
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("bind configured primary");
    let marker_path = geometry_fixture_blocks(
        &kura,
        lane_config.primary(),
        &BTreeMap::from([(LaneId::SINGLE, incarnation)]),
        &BTreeMap::from([(LaneId::SINGLE, 0)]),
    )
    .join(MARKER_FILE_NAME);
    fs::write(
        &marker_path,
        LaneIncarnationMarker {
            version: MARKER_VERSION,
            network_id: geometry_fixture_network_id(),
            dataspace_id: lane_config.primary().dataspace_id,
            lane_id: LaneId::SINGLE,
            incarnation: Hash::prehashed([0xA2; Hash::LENGTH]),
            activation_height: 0,
        }
        .encode(),
    )
    .expect("write foreign marker");
    drop(kura);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("foreign configured-primary marker must fail before Kura reconciliation");
    let marker = decode_exact::<LaneIncarnationMarker>(
        &fs::read(&marker_path).expect("foreign marker retained"),
    )
    .expect("decode retained marker");
    assert_eq!(marker.incarnation, Hash::prehashed([0xA2; Hash::LENGTH]));
}

#[test]
fn configured_catalog_preflight_rejects_nonzero_physical_primary_without_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let nonzero_root = temp.path().join("nonzero-primary");
    let nonzero_catalog = LaneCatalog::new(
        nonzero!(2_u32),
        vec![ModelLaneConfig {
            id: LaneId::new(1),
            alias: "not-physical-primary".to_owned(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("sparse nonzero-only catalog");
    let nonzero_config = RuntimeLaneConfig::from_catalog(&nonzero_catalog);
    let error = Kura::new_with_configured_lane_catalog(
        &kura_config(&nonzero_root),
        &nonzero_config,
        &nonzero_catalog,
    )
    .expect_err("authenticated Kura must require physical lane zero");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidInput,
        "authenticated configured catalog must contain physical primary lane zero",
    );
    assert!(!nonzero_root.exists());
}

#[test]
fn configured_catalog_preflight_refuses_to_bind_a_nonpristine_root() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    fs::create_dir_all(&root).expect("seed Kura root");
    let sentinel = root.join("operator-ledger-data");
    fs::write(&sentinel, b"must-not-adopt-or-delete").expect("seed foreign ledger data");
    let configured = configured_primary_catalog("pristine-root-required");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let error =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect_err("a missing baseline must never bind an existing ledger root");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "cannot establish a configured-catalog baseline on a non-pristine Kura root",
    );
    assert_eq!(
        fs::read(&sentinel).expect("foreign data retained"),
        b"must-not-adopt-or-delete"
    );
    assert!(!root.join(JOURNAL_FILE_NAME).exists());
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    assert_lane_paths_absent(&root, &lane_config);
}

#[cfg(unix)]
#[test]
fn configured_catalog_admits_only_the_bound_public_reset_storage_marker() {
    use std::os::unix::fs::PermissionsExt as _;

    let temp = TempDir::new().expect("temporary directory");
    let state = temp.path().join("taira-validator-1");
    let root = state.join("storage");
    fs::create_dir_all(&root).expect("create fresh validator storage");
    for path in [&state, &root] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))
            .expect("retain reset directory custody");
    }
    let marker = PublicResetGeneratedMarkerV1 {
        schema: "iroha.taira.public-reset.generated-path.v1".into(),
        kind: "fresh_state".into(),
        host_slug: "taira-validator-1".into(),
        inventory_sha256: "a".repeat(64),
        authorization_nonce: "b".repeat(32),
        revision: "c".repeat(40),
        created_at_unix_ms: 1,
    };
    let marker_path = |directory: &Path| directory.join(".public-reset-generated-v1.json");
    let write_marker = |directory: &Path, marker: &PublicResetGeneratedMarkerV1| {
        let path = marker_path(directory);
        fs::write(
            &path,
            norito::json::to_json(marker).expect("canonical reset marker"),
        )
        .expect("write reset marker");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .expect("retain reset marker custody");
    };
    write_marker(&state, &marker);
    let mut child = marker.clone();
    child.kind = "fresh_state_entry".into();
    write_marker(&root, &child);
    assert!(exact_public_reset_storage_marker(&root));

    let configured = configured_primary_catalog("public-reset-storage-marker");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
            .expect("fresh reset storage marker is part of the exact Kura root closure");
    drop(kura);
    assert!(root.join(JOURNAL_FILE_NAME).is_file());
    assert!(marker_path(&root).is_file());

    child.authorization_nonce = "d".repeat(32);
    write_marker(&root, &child);
    assert!(!exact_public_reset_storage_marker(&root));
    child.authorization_nonce = marker.authorization_nonce.clone();
    write_marker(&root, &child);
    fs::set_permissions(marker_path(&root), fs::Permissions::from_mode(0o644))
        .expect("weaken marker mode");
    assert!(!exact_public_reset_storage_marker(&root));
}

#[test]
fn authenticated_primary_restore_heals_missing_lane_artifact_namespace() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured_catalog = configured_primary_catalog("authenticated-primary");
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let (incarnations, activation_heights) = initial_geometry();
    let configured_catalog_hash = LaneLifecycleParameterV1::catalog_hash(&configured_catalog);
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("open authenticated configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        configured.primary(),
        incarnations[&LaneId::SINGLE],
        configured_catalog_hash,
    )
    .expect("authenticate configured primary geometry");
    let bindings = kura
        .geometry_bindings(&configured, &incarnations, &activation_heights)
        .expect("derive authenticated primary binding");
    let lineage_root = unscoped_lineage_root(&bindings);
    let primary_blocks = kura.binding_blocks_path(&bindings[0]);
    let lane_artifacts = Kura::lane_artifact_dir(&primary_blocks);
    if lane_artifacts.exists() {
        fs::remove_dir(&lane_artifacts).expect("remove empty primary artifact namespace");
    }
    assert!(
        !lane_artifacts.exists(),
        "fixture must restore an authenticated primary without its empty artifact namespace"
    );
    kura.restore_lane_segments_with_geometry_at_height_and_lineage_root(
        &configured,
        &incarnations,
        &activation_heights,
        0,
        lineage_root,
    )
    .expect("restore must durably heal the authenticated primary namespace");
    let namespace = Kura::open_bound_progress_directory(&root, &lane_artifacts)
        .expect("healed primary artifact namespace is descriptor-bound");
    assert!(
        kura.geometry_bound_progress_directory_unchanged(&namespace),
        "healed primary artifact namespace must retain its durable identity"
    );
    drop(namespace);
    drop(kura);
    let (reopened, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("reopen authenticated configured Kura");
    reopened
        .bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    reopened
        .restore_lane_segments_with_geometry_at_height_and_lineage_root(
            &configured,
            &incarnations,
            &activation_heights,
            0,
            lineage_root,
        )
        .expect("authenticated namespace healing must be restart-idempotent");
    let namespace = Kura::open_bound_progress_directory(&root, &lane_artifacts)
        .expect("reopened primary artifact namespace is descriptor-bound");
    assert!(
        reopened.geometry_bound_progress_directory_unchanged(&namespace),
        "reopened primary artifact namespace must retain its durable identity"
    );
}

#[test]
fn configured_multilane_startup_defers_secondary_provisioning_to_geometry_journal() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let primary = ModelLaneConfig::default();
    let secondary = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "configured-secondary".to_owned(),
        ..ModelLaneConfig::default()
    };
    let initial_catalog = LaneCatalog::new(lane_count, vec![primary.clone()])
        .expect("configured startup base catalog");
    let configured_catalog = LaneCatalog::new(lane_count, vec![primary, secondary])
        .expect("configured startup two-lane catalog");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x81; Hash::LENGTH]))]);
    let configured_incarnations = BTreeMap::from([
        (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x82; Hash::LENGTH])),
    ]);
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
    let secondary_entry = configured.entry(LaneId::new(1)).expect("secondary lane");
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("open authenticated configured Kura");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    let secondary_blocks = geometry_fixture_blocks(
        &kura,
        secondary_entry,
        &configured_incarnations,
        &configured_activations,
    );
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        LaneLifecycleParameterV1::catalog_hash(&configured_catalog),
    )
    .expect("bind configured primary before publishing the full catalog");
    assert!(
        !secondary_blocks.exists(),
        "authenticated Kura open must not precreate secondary storage without incarnation evidence"
    );
    assert!(
        kura.lane_storage_entry(LaneId::new(1)).is_err(),
        "authenticated Kura must not advertise an unowned secondary segment"
    );
    kura.apply_lane_geometry_transition(
        &initial,
        &configured,
        &initial_incarnations,
        &configured_incarnations,
        &initial_activations,
        &configured_activations,
        &BTreeSet::new(),
    )
    .expect("journal configured secondary-lane creation");
    kura.mark_lane_geometry_catalog_published(
        &configured,
        &configured_incarnations,
        &configured_activations,
        Some(LaneLifecycleParameterV1::catalog_hash(&configured_catalog)),
    )
    .expect("publish configured secondary-lane geometry");
    let secondary_binding = kura
        .geometry_bindings(
            &configured,
            &configured_incarnations,
            &configured_activations,
        )
        .expect("configured geometry bindings")
        .into_iter()
        .find(|binding| binding.lane_id == LaneId::new(1))
        .expect("secondary geometry binding");
    kura.require_lane_marker(&secondary_binding)
        .expect("secondary storage has the exact authoritative marker");
    assert!(secondary_blocks.join(MARKER_FILE_NAME).is_file());
    assert!(kura.lane_storage_entry(LaneId::new(1)).is_ok());
    drop(kura);
    let (reopened, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("reopen exact configured Kura");
    reopened
        .bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    reopened
        .recover_lane_geometry_journal(
            &configured,
            &configured_incarnations,
            &configured_activations,
        )
        .expect("reopen authenticates published configured geometry");
    reopened
        .require_lane_marker(&secondary_binding)
        .expect("reopened secondary marker remains exact");
    fs::remove_dir_all(&secondary_blocks).expect("simulate loss of published secondary blocks");
    let error = reopened
        .recover_lane_geometry_journal(
            &configured,
            &configured_incarnations,
            &configured_activations,
        )
        .expect_err("published configured secondary must never be silently recreated empty");
    assert_geometry_io_error(
        &error,
        ErrorKind::NotFound,
        "durable lane instance evidence is missing; refusing empty provisioning",
    );
    assert!(!secondary_blocks.exists());
}

#[test]
fn configured_multilane_startup_rejects_unjournaled_secondary_storage() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let primary = ModelLaneConfig::default();
    let secondary = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "unjournaled-secondary".to_owned(),
        ..ModelLaneConfig::default()
    };
    let initial_catalog = LaneCatalog::new(lane_count, vec![primary.clone()])
        .expect("configured startup base catalog");
    let configured_catalog = LaneCatalog::new(lane_count, vec![primary, secondary])
        .expect("configured startup two-lane catalog");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let configured = RuntimeLaneConfig::from_catalog(&configured_catalog);
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x91; Hash::LENGTH]))]);
    let configured_incarnations = BTreeMap::from([
        (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x92; Hash::LENGTH])),
    ]);
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let configured_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 0)]);
    let (kura, _) = Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &configured,
        &configured_catalog,
    )
    .expect("canonical open must not provision an unowned secondary instance");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        LaneLifecycleParameterV1::catalog_hash(&configured_catalog),
    )
    .expect("authenticate the existing primary before secondary admission");
    let secondary_blocks = geometry_fixture_blocks(
        &kura,
        configured.entry(LaneId::new(1)).expect("secondary lane"),
        &configured_incarnations,
        &configured_activations,
    );
    assert!(!secondary_blocks.exists());
    fs::create_dir_all(&secondary_blocks).expect("seed unjournaled exact secondary namespace");
    let sentinel = secondary_blocks.join("operator-sentinel");
    fs::write(&sentinel, b"must-not-adopt-or-delete").expect("seed unjournaled sentinel");
    let error = kura
        .apply_lane_geometry_transition(
            &initial,
            &configured,
            &initial_incarnations,
            &configured_incarnations,
            &initial_activations,
            &configured_activations,
            &BTreeSet::new(),
        )
        .expect_err("unjournaled secondary storage must not be adopted");
    assert_geometry_io_error(
        &error,
        ErrorKind::AlreadyExists,
        "new lane instance target already contains storage",
    );
    assert_eq!(
        fs::read(&sentinel).expect("unjournaled sentinel retained"),
        b"must-not-adopt-or-delete"
    );
    assert!(
        kura.read_lane_geometry_journal()
            .expect("configured baseline journal")
            .records
            .is_empty(),
        "rejection must precede geometry intent publication"
    );
}

#[test]
fn configured_catalog_preflight_recovers_exact_first_start_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    fs::create_dir_all(&root).expect("Kura root");
    let configured = configured_primary_catalog("temp-recovery");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let expected = LaneGeometryJournal {
        configured_catalog_hash: Some(LaneLifecycleParameterV1::catalog_hash(&configured)),
        ..LaneGeometryJournal::default()
    };
    fs::write(root.join(JOURNAL_TEMP_FILE_NAME), expected.encode())
        .expect("simulate synced first-start temp before hard-link promotion");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("reconstructed process must promote the exact baseline temp");
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    let recovered = decode_exact::<LaneGeometryJournal>(
        &fs::read(root.join(JOURNAL_FILE_NAME)).expect("promoted baseline journal"),
    )
    .expect("decode promoted baseline journal");
    assert_eq!(recovered, expected);
}

#[test]
fn configured_catalog_preflight_cleans_exact_startup_owned_hard_link_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("link-recovery");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline before simulated crash");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    fs::hard_link(&journal_path, root.join(JOURNAL_TEMP_FILE_NAME))
        .expect("simulate crash after durable hard-link promotion");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("exact startup-owned hard-link temp must be cleaned before lane storage opens");
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    assert!(journal_path.is_file());
}

#[test]
fn configured_catalog_preflight_rejects_unproven_restore_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("restore-temp");
    let attempted = configured_primary_catalog("restore-must-not-open");
    let attempted_lane_config = RuntimeLaneConfig::from_catalog(&attempted);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    fs::copy(&journal_path, root.join(JOURNAL_RESTORE_TEMP_FILE_NAME))
        .expect("seed byte-identical but unowned restore temp");
    Kura::new_with_configured_lane_catalog(
        &kura_config(&root),
        &attempted_lane_config,
        &configured,
    )
    .expect_err("byte equality does not prove restore-temp ownership");
    assert_lane_paths_absent(&root, &attempted_lane_config);
    assert!(root.join(JOURNAL_RESTORE_TEMP_FILE_NAME).is_file());
}

#[test]
fn configured_catalog_preflight_discards_uncommitted_restore_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("restore-temp");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let authoritative = fs::read(&journal_path).expect("authoritative journal bytes");
    let root_identity = configured_catalog_store_root_identity(&root).expect("root identity");
    write_initial_configured_catalog_temp(
        &root,
        root_identity,
        &root.join(JOURNAL_RESTORE_TEMP_FILE_NAME),
        b"synced-but-uncommitted-restore-bytes",
    )
    .expect("simulate crash before restore-temp rename");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("the final journal is the sole restore commit point");
    assert!(!root.join(JOURNAL_RESTORE_TEMP_FILE_NAME).exists());
    assert_eq!(
        fs::read(&journal_path).expect("journal retained"),
        authoritative
    );
}

#[test]
fn configured_catalog_preflight_discards_different_uncommitted_publication_temp() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("publication-temp");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let authoritative = fs::read(&journal_path).expect("authoritative journal bytes");
    let different = LaneGeometryJournal {
        configured_catalog_hash: Some(Hash::new(b"different-uncommitted-catalog")),
        ..LaneGeometryJournal::default()
    }
    .encode();
    let root_identity = configured_catalog_store_root_identity(&root).expect("root identity");
    write_initial_configured_catalog_temp(
        &root,
        root_identity,
        &root.join(JOURNAL_TEMP_FILE_NAME),
        &different,
    )
    .expect("simulate crash before publication-temp rename");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect("the final journal is the sole publication commit point");
    assert!(!root.join(JOURNAL_TEMP_FILE_NAME).exists());
    assert_eq!(
        fs::read(&journal_path).expect("journal retained"),
        authoritative
    );
}

#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_reserved_temp_symlink_without_touching_target() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let outside = temp.path().join("outside-temp-target");
    fs::write(&outside, b"operator-owned").expect("outside sentinel");
    let configured = configured_primary_catalog("reserved-temp-symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish authoritative baseline");
    let reserved = root.join(JOURNAL_TEMP_FILE_NAME);
    symlink(&outside, &reserved).expect("reserved temp symlink");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("reserved temp symlinks must never be deleted or followed");
    assert!(reserved.is_symlink());
    assert_eq!(
        fs::read(&outside).expect("outside sentinel"),
        b"operator-owned"
    );
}

#[test]
fn configured_catalog_preflight_rejects_tampered_v6_structure_before_lane_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("structural-baseline");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish current baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let mut journal = decode_exact::<LaneGeometryJournal>(
        &fs::read(&journal_path).expect("read valid baseline journal"),
    )
    .expect("decode valid baseline journal");
    let previous_catalog = Hash::new(b"forged previous catalog");
    let previous_lineage_root = Hash::new(b"forged previous lineage");
    let updated_catalog = Hash::new(b"forged updated catalog");
    let updated_lineage_root = Hash::new(b"forged updated lineage");
    journal.records.push(LaneGeometryIntent {
        transition_id: geometry_transition_id(
            0,
            0,
            previous_catalog,
            previous_lineage_root,
            updated_catalog,
            updated_lineage_root,
        ),
        transition_sequence: 0,
        transition_height: 0,
        previous_catalog,
        previous_lineage_root,
        updated_catalog,
        updated_lineage_root,
        previous_bindings: Vec::new(),
        updated_bindings: Vec::new(),
        phase: LaneGeometryPhase::Intent,
        operations: Vec::new(),
    });
    fs::write(&journal_path, journal.encode()).expect("write decodable structural forgery");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("correct baseline must not mask a structurally malformed journal");
    assert_lane_paths_absent(&root, &lane_config);
}

#[test]
fn configured_catalog_preflight_rejects_version_mismatch_before_lane_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("version-baseline");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish current baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let mut journal = decode_exact::<LaneGeometryJournal>(
        &fs::read(&journal_path).expect("read valid baseline journal"),
    )
    .expect("decode valid baseline journal");
    journal.version = JOURNAL_VERSION.saturating_add(1);
    fs::write(&journal_path, journal.encode()).expect("write unsupported journal version");
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("unsupported journal version must fail at the startup boundary");
    assert_lane_paths_absent(&root, &lane_config);
}

#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_journal_derived_symlink_before_lane_mutation() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = LaneCatalog::default();
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(&root), &initial, &configured)
            .expect("establish valid configured startup");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind explicit fixture network before geometry authority");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        initial_incarnations[&LaneId::SINGLE],
        LaneLifecycleParameterV1::catalog_hash(&configured),
    )
    .expect("authenticate primary before a transition can reference it");
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("persist a valid journal-derived archive path");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("transition journal");
    let binding = journal.records[0].operations[0]
        .updated
        .as_ref()
        .expect("created immutable instance binding");
    let link = kura.binding_blocks_path(binding);
    let displaced = link.with_extension("displaced");
    fs::rename(&link, &displaced).expect("retain the original journal-owned instance");
    let outside = temp.path().join("outside");
    fs::create_dir(&outside).expect("outside directory");
    fs::write(outside.join("operator-data"), b"retain").expect("outside sentinel");
    symlink(&outside, &link).expect("inject journal-derived immutable-path symlink");
    drop(kura);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &initial, &configured)
        .expect_err("journal-derived symlink must fail before opening attempted lane storage");
    assert!(link.is_symlink());
    assert!(displaced.is_dir());
    assert_eq!(fs::read(outside.join("operator-data")).unwrap(), b"retain");
}

#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_journal_symlink_before_lane_mutation() {
    use std::os::unix::fs::symlink;
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("journal-symlink-baseline");
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish valid configured baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    let outside_journal = temp.path().join("outside-journal.norito");
    fs::rename(&journal_path, &outside_journal).expect("move journal outside Kura root");
    symlink(&outside_journal, &journal_path).expect("replace journal with a symlink");
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("configured-catalog journal symlink must fail closed");
    assert_lane_paths_absent(&root, &lane_config);
    assert!(journal_path.is_symlink());
    assert!(outside_journal.is_file());
}

#[cfg(unix)]
#[test]
fn configured_catalog_preflight_rejects_journal_identity_swap_before_lane_mutation() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let configured = configured_primary_catalog("identity-baseline");
    let baseline = LaneLifecycleParameterV1::catalog_hash(&configured);
    Kura::establish_or_verify_configured_lane_catalog_baseline(&root, baseline)
        .expect("establish valid configured baseline");
    let journal_path = root.join(JOURNAL_FILE_NAME);
    fs::copy(&journal_path, root.join(JOURNAL_IDENTITY_SWAP_FILE_NAME))
        .expect("prepare same-content replacement inode");
    Kura::replace_configured_catalog_journal_after_open_for_test(&root);
    let lane_config = RuntimeLaneConfig::from_catalog(&configured);
    Kura::new_with_configured_lane_catalog(&kura_config(&root), &lane_config, &configured)
        .expect_err("journal identity replacement during read must fail closed");
    assert_lane_paths_absent(&root, &lane_config);
    assert!(root.join(JOURNAL_IDENTITY_DISPLACED_FILE_NAME).is_file());
}

#[test]
fn configured_catalog_preflight_rejects_current_journal_without_baseline_without_mutation() {
    let temp = tempfile::TempDir::new().unwrap();
    let catalog = configured_primary_catalog("missing-baseline");
    let path = temp.path().join(JOURNAL_FILE_NAME);
    let original = LaneGeometryJournal::default().encode();
    fs::write(&path, &original).unwrap();
    let error = Kura::new_with_configured_lane_catalog(
        &kura_config(temp.path()),
        &RuntimeLaneConfig::from_catalog(&catalog),
        &catalog,
    )
    .expect_err("a current journal cannot acquire a configured baseline on reopen");
    assert!(
        matches!(error, Error::IO(ref source, _) if source.kind() == ErrorKind::InvalidData
        && source.to_string().contains("has no configured lane catalog baseline"))
    );
    assert_eq!(fs::read(&path).unwrap(), original);
    assert!(!temp.path().join("blocks").exists());
    assert!(!temp.path().join("merge_ledger").exists());
}
