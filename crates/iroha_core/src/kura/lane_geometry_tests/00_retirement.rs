#[test]
fn retirement_work_bound_scales_with_routes_and_configured_retention() {
    for (routes, retention) in [(4_usize, 4_096_usize), (1_024, 512)] {
        let diagnostic_suffix = routes
            * retention
            * (LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE
                + LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE);
        assert!(
            diagnostic_suffix > 65_536,
            "fixture must exceed the retired fixed aggregate cap"
        );
        let limit = lane_retirement_aggregate_work_item_limit(
            routes,
            retention,
            retention,
            V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY.get(),
        )
        .expect("valid route/configuration bound");
        let expected = routes
            * (LANE_RETIREMENT_REGULAR_SIDECARS_PER_ROUTE
                * (retention + V2_PENDING_CERTIFIED_MERGE_ENTRY_CAPACITY.get())
                + LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE * retention)
            + HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS;
        assert_eq!(limit, expected);
        assert!(
            limit >= diagnostic_suffix,
            "correctly compacted diagnostic suffix must fit the aggregate scan bound"
        );
    }
    assert!(
        lane_retirement_aggregate_work_item_limit(usize::MAX, usize::MAX, usize::MAX, usize::MAX,)
            .is_none(),
        "hostile configuration arithmetic must fail closed on overflow"
    );
    assert_eq!(
        MAX_LANE_RETIREMENT_WORK_ITEMS_PER_SIDECAR, 65_536,
        "aggregate scaling must not weaken the per-sidecar corruption cap"
    );
}
#[test]
fn retirement_artifact_file_bound_counts_every_fixed_frontier() {
    assert_eq!(
        LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE, 17,
        "seven data/index pairs plus three independent frontier/index files are fixed per route"
    );
    for native_retention in [0_usize, 1, 4_096] {
        let expected = MAX_AUTONOMOUS_LANE_ATTEMPT_NAMESPACE_FILES
            + LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE
            + LANE_RETIREMENT_HISTORICAL_RECOVERY_NAMESPACES_PER_ROUTE
            + native_retention * LANE_RETIREMENT_NATIVE_SIDECARS_PER_ROUTE;
        assert_eq!(
            lane_retirement_per_route_artifact_file_limit(native_retention),
            Some(expected),
        );
    }
    assert!(lane_retirement_per_route_artifact_file_limit(usize::MAX).is_none());
}
#[test]
fn retirement_historical_recovery_record_bound_is_global_and_exact() {
    assert_eq!(
        accumulate_lane_retirement_historical_recovery_records(
            0,
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
        ),
        Some(HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS),
    );
    assert_eq!(
        accumulate_lane_retirement_historical_recovery_records(
            HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
            1,
        ),
        None,
        "the 4,097th record must fail even when it belongs to another route",
    );
}
#[test]
fn retirement_two_lane_scan_passes_remaining_global_budget_before_decode() {
    let first_lane = TempDir::new().expect("first historical recovery lane");
    let second_lane = TempDir::new().expect("second historical recovery lane");
    let name = format!("{:0width$x}.norito", 0, width = Hash::LENGTH * 2);
    fs::write(first_lane.path().join(&name), [0_u8; 3])
        .expect("write first-lane historical recovery record");
    fs::write(second_lane.path().join(&name), [0_u8; 3])
        .expect("write second-lane historical recovery record");
    let (first_records, first_bytes) = bounded_historical_autonomous_recovery_entries(
        first_lane.path(),
        HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS,
        5,
        |path| {
            let metadata =
                fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
            Ok(((), metadata))
        },
    )
    .expect("first lane fits the global retirement budget");
    let (remaining_records, remaining_bytes) =
        remaining_lane_retirement_historical_recovery_budget(first_records.len(), first_bytes, 5)
            .expect("first lane leaves a representable global budget");
    assert_eq!(
        remaining_records,
        HISTORICAL_AUTONOMOUS_RECOVERY_MAX_RECORDS - 1
    );
    assert_eq!(remaining_bytes, 2);
    bounded_historical_autonomous_recovery_entries(
        second_lane.path(),
        remaining_records,
        remaining_bytes,
        |path| {
            let metadata =
                fs::symlink_metadata(path).map_err(|error| Error::IO(error, path.to_path_buf()))?;
            Ok(((), metadata))
        },
    )
    .expect_err(
        "the second lane must fail during bounded enumeration before its record can be decoded",
    );
}
#[test]
fn retirement_artifact_snapshot_accepts_the_exact_fixed_namespace_boundary() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("retirement-fixed-file-boundary");
    let (_, configured) = retirement_test_configs();
    let kura = open_kura(&root, &configured);
    let (incarnations, activations) = retirement_test_geometry();
    let artifact_dir = Kura::lane_artifact_dir(&geometry_fixture_blocks(
        &kura,
        configured.primary(),
        &incarnations,
        &activations,
    ));
    fs::create_dir_all(&artifact_dir).expect("create lane artifact directory");
    let fixed_files = [
        LANE_ARTIFACTS_DATA_FILE,
        LANE_ARTIFACTS_INDEX_FILE,
        CERTIFIED_LANE_BLOCKS_DATA_FILE,
        CERTIFIED_LANE_BLOCKS_INDEX_FILE,
        LATEST_CERTIFIED_LANE_BLOCK_FRONTIER_FILE,
        LANE_BLOCK_EXECUTION_INPUTS_DATA_FILE,
        LANE_BLOCK_EXECUTION_INPUTS_INDEX_FILE,
        LANE_BLOCK_EXECUTION_PREFLIGHTS_DATA_FILE,
        LANE_BLOCK_EXECUTION_PREFLIGHTS_INDEX_FILE,
        AUTONOMOUS_LANE_MERGE_BUNDLES_DATA_FILE,
        AUTONOMOUS_LANE_MERGE_BUNDLES_INDEX_FILE,
        CANONICAL_AUTONOMOUS_LANE_REPLICAS_DATA_FILE,
        CANONICAL_AUTONOMOUS_LANE_REPLICAS_INDEX_FILE,
        LANE_BLOCK_APPLICATION_RECEIPTS_DATA_FILE,
        LANE_BLOCK_APPLICATION_RECEIPTS_INDEX_FILE,
        NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE,
        LANE_MERGE_APPLICATION_FRONTIER_FILE,
    ];
    assert_eq!(
        fixed_files.len(),
        LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE,
        "the scanner fixture must enumerate every allowed fixed artifact"
    );
    for name in fixed_files {
        fs::write(artifact_dir.join(name), b"fixed retirement artifact")
            .expect("write fixed retirement artifact");
    }
    fs::create_dir(artifact_dir.join(HISTORICAL_AUTONOMOUS_RECOVERY_DIRECTORY_V1))
        .expect("create historical autonomous recovery namespace");
    let exact_fixed_namespace_limit = LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE
        + LANE_RETIREMENT_HISTORICAL_RECOVERY_NAMESPACES_PER_ROUTE;
    let directory = Kura::open_bound_progress_directory(&kura.store_root(), &artifact_dir)
        .expect("bind exact fixed retirement namespace");
    let snapshot = kura
        .geometry_bound_progress_directory_snapshot(
            &directory,
            exact_fixed_namespace_limit,
            "retirement fixed artifact scan",
        )
        .expect("the exact fixed-file boundary must fit");
    assert_eq!(snapshot.len(), exact_fixed_namespace_limit);
    drop(directory);
    fs::write(artifact_dir.join("one-file-over-bound"), b"overflow")
        .expect("write one excess artifact");
    let directory = Kura::open_bound_progress_directory(&kura.store_root(), &artifact_dir)
        .expect("rebind oversized retirement namespace");
    let error = kura
        .geometry_bound_progress_directory_snapshot(
            &directory,
            exact_fixed_namespace_limit,
            "retirement fixed artifact scan",
        )
        .expect_err("one file beyond the exact scanner boundary must fail");
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "retirement fixed artifact scan exceeds its bounded directory-entry count",
    );
}
/// Temporary directory whose exposed path uses the same canonical spelling as Kura.
///
/// macOS exposes its temporary hierarchy through `/var` while canonical paths use
/// `/private/var`.  Geometry tests pass paths back into a Kura instance after startup, so the
/// harness must retain the canonical spelling selected by `Kura::new_inner`; otherwise exact
/// containment and test-hook identity comparisons fail before exercising the intended gate.
struct TempDir {
    _inner: RawTempDir,
    canonical_path: PathBuf,
}
impl TempDir {
    fn new() -> std::io::Result<Self> {
        let inner = RawTempDir::new()?;
        let canonical_path = fs::canonicalize(inner.path())?;
        Ok(Self {
            _inner: inner,
            canonical_path,
        })
    }
    fn path(&self) -> &Path {
        &self.canonical_path
    }
}
fn open_kura(root: &Path, lane_config: &RuntimeLaneConfig) -> Arc<Kura> {
    let config = kura_config(root);
    let kura = Kura::open_test_kura_with_configured_lane_config(&config, lane_config)
        .expect("open canonical-only test Kura")
        .0;
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind the fixture's explicit network");
    kura
}
/// Resolve one exact fixture instance without consulting the current LaneId map.
fn geometry_fixture_blocks(
    kura: &Kura,
    entry: &LaneConfigEntry,
    incarnations: &BTreeMap<LaneId, Hash>,
    activations: &BTreeMap<LaneId, u64>,
) -> PathBuf {
    kura.binding_blocks_path(
        &kura
            .geometry_binding(entry, incarnations, activations)
            .expect("complete fixture instance identity"),
    )
}
fn geometry_fixture_merge(
    kura: &Kura,
    entry: &LaneConfigEntry,
    incarnations: &BTreeMap<LaneId, Hash>,
    activations: &BTreeMap<LaneId, u64>,
) -> PathBuf {
    kura.binding_merge_path(
        &kura
            .geometry_binding(entry, incarnations, activations)
            .expect("complete fixture instance identity"),
    )
}
/// Actual journal predecessor for structural storage-only snapshot fixtures.
fn geometry_fixture_recovery(
    kura: &Kura,
    height: u64,
    current: &[LaneGeometryBinding],
    root: Hash,
) -> (Vec<LaneGeometryBinding>, Hash) {
    kura.geometry_recovery_references_at_snapshot(
        &kura
            .read_lane_geometry_journal()
            .expect("read actual fixture history"),
        height,
        current,
        root,
    )
    .expect("resolve exact historical predecessor")
}

fn wait_for_total_usage_scan_pause(kura: &Kura) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !kura.total_disk_usage_scan_paused_for_tests() {
        if Instant::now() >= deadline {
            kura.resume_total_disk_usage_scan_for_tests();
            panic!("disk-usage scan did not reach its deterministic publication barrier");
        }
        thread::yield_now();
    }
}
fn kura_config(root: &Path) -> KuraConfig {
    KuraConfig {
        init_mode: iroha_config::kura::InitMode::Strict,
        store_dir: WithOrigin::inline(root.to_path_buf()),
        max_disk_usage_bytes: MAX_DISK_USAGE_BYTES,
        blocks_in_memory: BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Always,
        fsync_interval: FSYNC_INTERVAL,
        lane_history_retention: LANE_HISTORY_RETENTION,
        block_hash_history_bytes: iroha_config::parameters::defaults::kura::BLOCK_HASH_HISTORY_BYTES,
        transaction_history_bytes: iroha_config::parameters::defaults::kura::TRANSACTION_HISTORY_BYTES,
        fastpq_artifacts: iroha_config::parameters::defaults::kura::FASTPQ_ARTIFACT_POLICY,
        replica_advert: iroha_config::parameters::defaults::kura::REPLICA_ADVERT_POLICY,
    }
}
fn configured_primary_catalog(alias: &str) -> LaneCatalog {
    LaneCatalog::new(
        nonzero!(1_u32),
        vec![ModelLaneConfig {
            alias: alias.to_owned(),
            ..ModelLaneConfig::default()
        }],
    )
    .expect("configured primary-lane catalog")
}
fn assert_lane_paths_absent(root: &Path, _lane_config: &RuntimeLaneConfig) {
    assert!(
        !root.join("blocks/instances").exists(),
        "rejected pre-State startup must not create any instance block path"
    );
    assert!(
        !root.join("merge_ledger/instances").exists(),
        "rejected pre-State startup must not create any instance merge path"
    );
}
fn assert_kura_io_error(error: &Error, kind: std::io::ErrorKind, message: &str) {
    let Error::IO(source, _) = error else {
        panic!("expected Kura IO error containing {message:?}, got {error:?}");
    };
    assert_eq!(source.kind(), kind, "unexpected Kura IO error: {error:?}");
    assert!(
        source.to_string().contains(message),
        "Kura IO source did not contain {message:?}: {error:?}"
    );
}
fn initial_and_extended_configs() -> (RuntimeLaneConfig, RuntimeLaneConfig) {
    let lane0 = ModelLaneConfig::default();
    let lane1 = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "elastic-one".to_owned(),
        ..ModelLaneConfig::default()
    };
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let initial = LaneCatalog::new(lane_count, vec![lane0.clone()]).expect("initial catalog");
    let extended = LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("extended catalog");
    (
        RuntimeLaneConfig::from_catalog(&initial),
        RuntimeLaneConfig::from_catalog(&extended),
    )
}
fn initial_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
    (
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x11; Hash::LENGTH]))]),
        BTreeMap::from([(LaneId::SINGLE, 0)]),
    )
}
fn extended_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
    (
        BTreeMap::from([
            (LaneId::SINGLE, Hash::prehashed([0x11; Hash::LENGTH])),
            (LaneId::new(1), Hash::prehashed([0x22; Hash::LENGTH])),
        ]),
        BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 9)]),
    )
}
fn persist_create_intent(
    kura: &Kura,
    previous: &RuntimeLaneConfig,
    updated: &RuntimeLaneConfig,
    previous_incarnations: &BTreeMap<LaneId, Hash>,
    updated_incarnations: &BTreeMap<LaneId, Hash>,
    previous_activations: &BTreeMap<LaneId, u64>,
    updated_activations: &BTreeMap<LaneId, u64>,
) -> LaneGeometryOperation {
    authenticate_transition_fixture_primary(kura, previous, previous_incarnations);
    let previous_bindings = kura
        .geometry_bindings(previous, previous_incarnations, previous_activations)
        .expect("previous geometry bindings");
    let updated_bindings = kura
        .geometry_bindings(updated, updated_incarnations, updated_activations)
        .expect("updated geometry bindings");
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
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].kind, LaneGeometryOperationKind::Create);
    let operation = operations[0].clone();
    let mut journal = kura
        .read_lane_geometry_journal()
        .expect("retain admitted H0 journal");
    assert!(
        journal.records.is_empty(),
        "fixture starts before its first intent"
    );
    journal.records.push(LaneGeometryIntent {
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
    });
    kura.write_lane_geometry_journal(&journal)
        .expect("persist create intent");
    operation
}
#[test]
fn before_first_height_cursor_replays_same_height_transitions_in_sequence() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let lane_count = nonzero!(2_u32);
    let primary = ModelLaneConfig::default();
    let second = ModelLaneConfig {
        id: LaneId::new(1),
        alias: "same-height-a".to_owned(),
        ..ModelLaneConfig::default()
    };
    let relabelled = ModelLaneConfig {
        alias: "same-height-b".to_owned(),
        ..second.clone()
    };
    let initial_catalog =
        LaneCatalog::new(lane_count, vec![primary.clone()]).expect("initial catalog");
    let added_catalog =
        LaneCatalog::new(lane_count, vec![primary.clone(), second]).expect("added catalog");
    let relabelled_catalog =
        LaneCatalog::new(lane_count, vec![primary, relabelled]).expect("relabelled catalog");
    let initial = RuntimeLaneConfig::from_catalog(&initial_catalog);
    let added = RuntimeLaneConfig::from_catalog(&added_catalog);
    let relabelled = RuntimeLaneConfig::from_catalog(&relabelled_catalog);
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, Hash::prehashed([0x51; Hash::LENGTH]))]);
    let added_incarnations = BTreeMap::from([
        (LaneId::SINGLE, initial_incarnations[&LaneId::SINGLE]),
        (LaneId::new(1), Hash::prehashed([0x52; Hash::LENGTH])),
    ]);
    let mut relabelled_incarnations = added_incarnations.clone();
    relabelled_incarnations.insert(LaneId::new(1), Hash::prehashed([0x53; Hash::LENGTH]));
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, 0)]);
    let added_activations = BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 7)]);
    let kura = open_kura(&root, &initial);
    kura.apply_lane_geometry_transition_at_height(
        &initial,
        &added,
        &initial_incarnations,
        &added_incarnations,
        &initial_activations,
        &added_activations,
        &BTreeSet::new(),
        7,
    )
    .expect("apply first height-seven transition");
    kura.mark_lane_geometry_catalog_published(
        &added,
        &added_incarnations,
        &added_activations,
        None,
    )
    .expect("publish first height-seven transition");
    kura.apply_lane_geometry_transition_at_height(
        &added,
        &relabelled,
        &added_incarnations,
        &relabelled_incarnations,
        &added_activations,
        &added_activations,
        &BTreeSet::new(),
        7,
    )
    .expect("apply second height-seven transition");
    kura.mark_lane_geometry_catalog_published(
        &relabelled,
        &relabelled_incarnations,
        &added_activations,
        None,
    )
    .expect("publish second height-seven transition");
    let original = kura
        .read_lane_geometry_journal()
        .expect("published journal");
    let cursors = original
        .records
        .iter()
        .map(|record| (record.transition_id, record.transition_sequence))
        .collect::<Vec<_>>();
    assert_eq!(original.records.len(), 2);
    kura.recover_lane_geometry_journal_before_first_transition_at_height(
        &initial,
        &initial_incarnations,
        &initial_activations,
        7,
    )
    .expect("restore cursor before every transition at height seven");
    assert!(
        kura.read_lane_geometry_journal()
            .expect("rolled-back journal")
            .records
            .iter()
            .all(|record| record.phase == LaneGeometryPhase::RolledBack)
    );
    kura.apply_lane_geometry_transition_at_height(
        &initial,
        &added,
        &initial_incarnations,
        &added_incarnations,
        &initial_activations,
        &added_activations,
        &BTreeSet::new(),
        7,
    )
    .expect("retry first transition in sequence");
    kura.mark_lane_geometry_catalog_published(
        &added,
        &added_incarnations,
        &added_activations,
        None,
    )
    .expect("republish first transition");
    kura.apply_lane_geometry_transition_at_height(
        &added,
        &relabelled,
        &added_incarnations,
        &relabelled_incarnations,
        &added_activations,
        &added_activations,
        &BTreeSet::new(),
        7,
    )
    .expect("retry second transition in sequence");
    kura.mark_lane_geometry_catalog_published(
        &relabelled,
        &relabelled_incarnations,
        &added_activations,
        None,
    )
    .expect("republish second transition");
    let replayed = kura.read_lane_geometry_journal().expect("replayed journal");
    assert_eq!(
        replayed
            .records
            .iter()
            .map(|record| (record.transition_id, record.transition_sequence))
            .collect::<Vec<_>>(),
        cursors
    );
    assert!(
        replayed
            .records
            .iter()
            .all(|record| record.phase == LaneGeometryPhase::CatalogPublished)
    );
}
fn open_configured_anchor_for_publication_test(
    root: &Path,
    catalog: &LaneCatalog,
    primary_incarnation: Hash,
) -> Arc<Kura> {
    let baseline = LaneLifecycleParameterV1::catalog_hash(catalog);
    let lane_config = RuntimeLaneConfig::from_catalog(catalog);
    let (kura, _) =
        Kura::new_with_configured_lane_catalog(&kura_config(root), &lane_config, catalog)
            .expect("open the exact authenticated configured catalog");
    kura.bind_lane_storage_network(geometry_fixture_network_id())
        .expect("bind the exact configured fixture network");
    kura.establish_or_verify_configured_primary_geometry_anchor(
        lane_config.primary(),
        primary_incarnation,
        baseline,
    )
    .expect("anchor configured primary before catalog publication");
    kura
}
#[test]
fn post_write_publication_failure_restores_anchored_description_only_journal() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let mut lanes = LaneCatalog::default().lanes().to_vec();
    lanes[0].description = Some("operator-only catalog description".to_owned());
    let catalog = LaneCatalog::new(nonzero!(1_u32), lanes).expect("description-only lane catalog");
    let config = RuntimeLaneConfig::from_catalog(&catalog);
    let baseline = iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(&catalog);
    let (incarnations, activation_heights) = initial_geometry();
    let kura =
        open_configured_anchor_for_publication_test(&root, &catalog, incarnations[&LaneId::SINGLE]);
    let journal_path = kura.lane_geometry_journal_path();
    let prior_bytes = fs::read(&journal_path).expect("anchored journal");
    kura.apply_lane_geometry_transition(
        &config,
        &config,
        &incarnations,
        &incarnations,
        &activation_heights,
        &activation_heights,
        &BTreeSet::new(),
    )
    .expect("description-only catalog has no physical geometry transition");
    assert_eq!(
        fs::read(&journal_path).expect("unchanged journal"),
        prior_bytes
    );
    kura.fail_next_lane_geometry_publication_after_write_for_test();
    let error = kura
        .mark_lane_geometry_catalog_published(
            &config,
            &incarnations,
            &activation_heights,
            Some(baseline),
        )
        .expect_err("failure after target replacement must restore prior absence");
    assert!(
        !matches!(&error, Error::LaneGeometryPublicationRestoreFailed { .. }),
        "exact restoration should preserve the original injected publication error: {error}"
    );
    assert_eq!(
        fs::read(&journal_path).expect("restored anchored journal"),
        prior_bytes
    );
    let (restored_baseline, phases, has_temp) = kura
        .lane_geometry_journal_state_for_test()
        .expect("read restored absent journal state");
    assert_eq!(restored_baseline, Some(baseline));
    assert!(phases.is_empty());
    assert!(!has_temp, "rollback must not leave owned temp files");
    kura.mark_lane_geometry_catalog_published(
        &config,
        &incarnations,
        &activation_heights,
        Some(baseline),
    )
    .expect("one-shot failure permits an exact corrected retry");
    let (retried_baseline, phases, has_temp) = kura
        .lane_geometry_journal_state_for_test()
        .expect("read corrected publication");
    assert_eq!(retried_baseline, Some(baseline));
    assert!(phases.is_empty());
    assert!(!has_temp);
}
#[test]
fn publication_temp_recovery_consumes_only_an_exact_preexisting_value() {
    let catalog = LaneCatalog::default();
    let config = RuntimeLaneConfig::from_catalog(&catalog);
    let baseline = iroha_data_model::nexus::LaneLifecycleParameterV1::catalog_hash(&catalog);
    let (incarnations, activation_heights) = initial_geometry();
    let unrelated_temp = TempDir::new().expect("temporary directory");
    let unrelated_root = unrelated_temp.path().join("kura");
    let unrelated_kura = open_configured_anchor_for_publication_test(
        &unrelated_root,
        &catalog,
        incarnations[&LaneId::SINGLE],
    );
    let publication_temp = unrelated_root.join(JOURNAL_TEMP_FILE_NAME);
    fs::write(&publication_temp, b"operator-owned-temp").expect("seed unrelated temp");
    let error = unrelated_kura
        .mark_lane_geometry_catalog_published(
            &config,
            &incarnations,
            &activation_heights,
            Some(baseline),
        )
        .expect_err("an unrelated preexisting temp must fail closed");
    assert!(
        !matches!(&error, Error::LaneGeometryPublicationRestoreFailed { .. }),
        "an untouched preexisting temp does not make prior-target restoration ambiguous: {error}"
    );
    assert_eq!(
        fs::read(&publication_temp).expect("unrelated temp retained"),
        b"operator-owned-temp"
    );
    assert!(
        unrelated_kura.lane_geometry_journal_path().is_file(),
        "a temp collision must retain the authenticated target"
    );
    let resumable_temp = TempDir::new().expect("temporary directory");
    let resumable_root = resumable_temp.path().join("kura");
    let resumable_kura = open_configured_anchor_for_publication_test(
        &resumable_root,
        &catalog,
        incarnations[&LaneId::SINGLE],
    );
    let expected_journal = resumable_kura
        .read_lane_geometry_journal()
        .expect("anchored resumable journal");
    let publication_temp = resumable_root.join(JOURNAL_TEMP_FILE_NAME);
    fs::write(&publication_temp, expected_journal.encode()).expect("seed exact resume temp");
    resumable_kura.fail_next_lane_geometry_publication_after_write_for_test();
    let error = resumable_kura
        .mark_lane_geometry_catalog_published(
            &config,
            &incarnations,
            &activation_heights,
            Some(baseline),
        )
        .expect_err("inject failure after consuming exact resume temp");
    assert!(!matches!(
        &error,
        Error::LaneGeometryPublicationRestoreFailed { .. }
    ));
    assert!(
        !publication_temp.exists(),
        "an exact resumable temp is consumed by target replacement"
    );
    assert!(
        fs::read(resumable_kura.lane_geometry_journal_path())
            .expect("post-write rollback restores the authenticated target")
            == expected_journal.encode(),
        "post-write rollback must restore the exact authenticated target"
    );
}
#[test]
fn post_write_publication_failure_restores_exact_files_applied_journal() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let catalog = LaneCatalog::default();
    let baseline = LaneLifecycleParameterV1::catalog_hash(&catalog);
    let kura = open_configured_anchor_for_publication_test(
        &root,
        &catalog,
        initial_incarnations[&LaneId::SINGLE],
    );
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("prepare files-applied geometry intent");
    let journal_path = kura.lane_geometry_journal_path();
    let prior_bytes = fs::read(&journal_path).expect("capture exact files-applied journal");
    let prior_journal =
        decode_exact::<LaneGeometryJournal>(&prior_bytes).expect("decode files-applied journal");
    assert_eq!(prior_journal.configured_catalog_hash, Some(baseline));
    assert_eq!(
        prior_journal.records.last().map(|record| record.phase),
        Some(LaneGeometryPhase::FilesApplied)
    );
    kura.fail_next_lane_geometry_publication_after_write_for_test();
    let error = kura
        .mark_lane_geometry_catalog_published(
            &extended,
            &extended_incarnations,
            &extended_activations,
            Some(baseline),
        )
        .expect_err("inject failure after replacing an existing journal");
    assert!(!matches!(
        &error,
        Error::LaneGeometryPublicationRestoreFailed { .. }
    ));
    assert_eq!(
        fs::read(&journal_path).expect("read restored journal"),
        prior_bytes,
        "rollback must restore the exact prior encoding, including FilesApplied phase"
    );
    let (restored_baseline, phases, has_temp) = kura
        .lane_geometry_journal_state_for_test()
        .expect("read exact restored journal state");
    assert_eq!(restored_baseline, Some(baseline));
    assert_eq!(phases, vec!["files_applied"]);
    assert!(!has_temp);
    kura.recover_lane_geometry_journal(&initial, &initial_incarnations, &initial_activations)
        .expect("restored FilesApplied intent remains available for State geometry rollback");
    assert_eq!(
        kura.read_lane_geometry_journal()
            .expect("journal after State-equivalent rollback")
            .records
            .last()
            .map(|record| record.phase),
        Some(LaneGeometryPhase::RolledBack)
    );
}
#[test]
fn publication_restore_failure_is_distinct_and_leaves_published_journal_fail_closed() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let catalog = LaneCatalog::default();
    let baseline = LaneLifecycleParameterV1::catalog_hash(&catalog);
    let kura = open_configured_anchor_for_publication_test(
        &root,
        &catalog,
        initial_incarnations[&LaneId::SINGLE],
    );
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("prepare files-applied geometry intent");
    let prior_bytes =
        fs::read(kura.lane_geometry_journal_path()).expect("capture exact files-applied journal");
    let restore_temp = root.join(JOURNAL_RESTORE_TEMP_FILE_NAME);
    fs::write(&restore_temp, b"operator-owned-restore-temp").expect("seed restore-temp collision");
    kura.fail_next_lane_geometry_publication_after_write_for_test();
    let error = kura
        .mark_lane_geometry_catalog_published(
            &extended,
            &extended_incarnations,
            &extended_activations,
            Some(baseline),
        )
        .expect_err("restore-temp collision must prevent claiming exact restoration");
    assert!(matches!(
        &error,
        Error::LaneGeometryPublicationRestoreFailed { .. }
    ));
    assert_eq!(
        fs::read(&restore_temp).expect("restore collision retained"),
        b"operator-owned-restore-temp"
    );
    assert_ne!(
        fs::read(kura.lane_geometry_journal_path()).expect("published journal remains"),
        prior_bytes,
        "restore failure must not be reported as if the prior journal were restored"
    );
    let journal = kura
        .read_lane_geometry_journal()
        .expect("published journal remains internally valid");
    assert_eq!(journal.configured_catalog_hash, Some(baseline));
    assert_eq!(
        journal.records.last().map(|record| record.phase),
        Some(LaneGeometryPhase::CatalogPublished),
        "State must stop instead of rolling geometry back under a published journal"
    );
}
fn retirement_test_configs() -> (RuntimeLaneConfig, RuntimeLaneConfig) {
    let lane0 = ModelLaneConfig {
        dataspace_id: DataSpaceId::new(7),
        ..ModelLaneConfig::default()
    };
    let lane1 = ModelLaneConfig {
        id: LaneId::new(1),
        dataspace_id: DataSpaceId::new(8),
        alias: "retirement-participant".to_owned(),
        ..ModelLaneConfig::default()
    };
    let lane_count = NonZeroU32::new(2).expect("non-zero lane count");
    let initial =
        LaneCatalog::new(lane_count, vec![lane0.clone()]).expect("retirement initial catalog");
    let extended =
        LaneCatalog::new(lane_count, vec![lane0, lane1]).expect("retirement extended catalog");
    (
        RuntimeLaneConfig::from_catalog(&initial),
        RuntimeLaneConfig::from_catalog(&extended),
    )
}
fn retirement_test_geometry() -> (BTreeMap<LaneId, Hash>, BTreeMap<LaneId, u64>) {
    (
        BTreeMap::from([
            (LaneId::SINGLE, Hash::prehashed([0x61; Hash::LENGTH])),
            (LaneId::new(1), Hash::prehashed([0x62; Hash::LENGTH])),
        ]),
        BTreeMap::from([(LaneId::SINGLE, 0), (LaneId::new(1), 1)]),
    )
}
fn install_retirement_test_lane_markers(
    kura: &Kura,
    config: &RuntimeLaneConfig,
    incarnations: &BTreeMap<LaneId, Hash>,
    activation_heights: &BTreeMap<LaneId, u64>,
) {
    authenticate_transition_fixture_primary(kura, config, incarnations);
    kura.replace_lane_storage_entries_for_test(config, incarnations, activation_heights)
        .expect("install exact structural retirement fixture identities");
    for binding in kura
        .geometry_bindings(config, incarnations, activation_heights)
        .expect("retirement test geometry bindings")
    {
        kura.write_lane_marker(&binding)
            .expect("install authoritative retirement-test lane marker");
        kura.provision_geometry_binding(&binding)
            .expect("provision paired retirement-test lane storage");
    }
}
#[allow(clippy::too_many_arguments)]
fn open_published_retirement_kura(
    root: &Path,
    initial: &RuntimeLaneConfig,
    extended: &RuntimeLaneConfig,
    initial_incarnations: &BTreeMap<LaneId, Hash>,
    extended_incarnations: &BTreeMap<LaneId, Hash>,
    initial_activations: &BTreeMap<LaneId, u64>,
    extended_activations: &BTreeMap<LaneId, u64>,
) -> (Arc<Kura>, Vec<u8>, usize) {
    let kura = open_kura(root, initial);
    authenticate_transition_fixture_primary(&kura, initial, initial_incarnations);
    kura.apply_lane_geometry_transition(
        initial,
        extended,
        initial_incarnations,
        extended_incarnations,
        initial_activations,
        extended_activations,
        &BTreeSet::new(),
    )
    .expect("journal dynamic retirement-test lane creation");
    kura.mark_lane_geometry_catalog_published(
        extended,
        extended_incarnations,
        extended_activations,
        None,
    )
    .expect("publish dynamic retirement-test lane catalog");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("read published retirement-test journal");
    let journal_bytes = fs::read(kura.lane_geometry_journal_path())
        .expect("read exact published retirement-test journal bytes");
    (kura, journal_bytes, journal.records.len())
}
fn assert_geometry_io_error(error: &Error, expected_kind: ErrorKind, expected_message: &str) {
    let Error::IO(source, _) = error else {
        panic!("unexpected lane geometry error: {error:?}");
    };
    assert_eq!(source.kind(), expected_kind, "lane geometry error: {error}");
    assert_eq!(source.to_string(), expected_message);
}
struct RetiredGeometryFixture {
    initial: RuntimeLaneConfig,
    extended: RuntimeLaneConfig,
    initial_incarnations: BTreeMap<LaneId, Hash>,
    initial_activations: BTreeMap<LaneId, u64>,
    extended_incarnations: BTreeMap<LaneId, Hash>,
    extended_activations: BTreeMap<LaneId, u64>,
    retained_blocks: PathBuf,
    retained_bytes: u64,
}
struct TombstonedAutonomousArchiveFixture {
    geometry: RetiredGeometryFixture,
    archived_blocks: PathBuf,
    binding: LaneGeometryBinding,
    autonomous_attempt: PathBuf,
    view_state: PathBuf,
    height_pointer: PathBuf,
    route_pointer: PathBuf,
}
struct NativeAmxArchiveFixture {
    geometry: RetiredGeometryFixture,
    archived_blocks: PathBuf,
    binding: LaneGeometryBinding,
    manifest: PathBuf,
    receipt: PathBuf,
    latest_index: PathBuf,
}
fn prepare_retired_geometry_archive(kura: &Kura, root: &Path) -> RetiredGeometryFixture {
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    authenticate_transition_fixture_primary(kura, &initial, &initial_incarnations);
    let (extended_incarnations, extended_activations) = extended_geometry();
    kura.apply_lane_geometry_transition(
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
        &BTreeSet::new(),
    )
    .expect("create elastic lane");
    kura.mark_lane_geometry_catalog_published(
        &extended,
        &extended_incarnations,
        &extended_activations,
        None,
    )
    .expect("publish elastic lane catalog");
    let lane_one_blocks = geometry_fixture_blocks(
        kura,
        extended.entry(LaneId::new(1)).expect("elastic lane"),
        &extended_incarnations,
        &extended_activations,
    );
    let lane_one_merge = geometry_fixture_merge(
        kura,
        extended.entry(LaneId::new(1)).expect("elastic lane"),
        &extended_incarnations,
        &extended_activations,
    );
    let retained_bytes = Kura::regular_geometry_archive_tree_bytes(&lane_one_blocks)
        .expect("measure the legitimate instance namespace")
        + fs::metadata(lane_one_merge)
            .expect("exact instance merge metadata")
            .len();
    assert!(retained_bytes > 0);
    kura.apply_lane_geometry_transition(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .expect("retire elastic lane");
    kura.mark_lane_geometry_catalog_published(
        &initial,
        &initial_incarnations,
        &initial_activations,
        None,
    )
    .expect("publish retired catalog");
    assert!(
        lane_one_blocks.exists(),
        "retirement retains the immutable instance"
    );
    assert!(
        !root.join("retired/lane_geometry").exists(),
        "Apply publishes references without physical retirement"
    );
    RetiredGeometryFixture {
        initial,
        extended,
        initial_incarnations,
        initial_activations,
        extended_incarnations,
        extended_activations,
        retained_blocks: lane_one_blocks,
        retained_bytes,
    }
}
fn checkpoint_retired_geometry(
    kura: &Kura,
    fixture: &RetiredGeometryFixture,
    height: u64,
) -> Result<LaneGeometryGcSummary> {
    let (block_hash, state_hash) = durable_geometry_snapshot_identity(kura, height);
    let bindings = kura.geometry_bindings(
        &fixture.initial,
        &fixture.initial_incarnations,
        &fixture.initial_activations,
    )?;
    let lineage_root = unscoped_lineage_root(&bindings);
    let (recovery_bindings, recovery_root) = kura.geometry_recovery_references_at_snapshot(
        &kura.read_lane_geometry_journal()?,
        height,
        &bindings,
        lineage_root,
    )?;
    kura.checkpoint_lane_geometry_with_proven_snapshot(
        bindings,
        lineage_root,
        recovery_bindings,
        recovery_root,
        height,
        Some(block_hash),
        state_hash,
        Vec::new(),
    )
}
fn lifecycle_bound_autonomous_retirement_payload(
    template: &crate::lane_consensus::LaneExecutablePayloadV1,
    height_context_id: HeightContextId,
    signer: &KeyPair,
) -> crate::lane_consensus::LaneExecutablePayloadV1 {
    let local_peer = PeerId::new(signer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        crate::sumeragi::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            template.network_id,
            height_context_id,
            template.epoch,
            &template.origin_proposal,
            &local_peer,
        )
        .expect("derive geometry-retirement lifecycle reservation identities");
    let mut reservation_keys = template.reservation_keys.clone();
    for reservation in &mut reservation_keys {
        reservation.reservation_owner_hash = reservation_owner_hash;
        reservation.proposal_identity_hash = proposal_identity_hash;
    }
    crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        template.network_id,
        template.epoch,
        template.origin_proposal.clone(),
        template.entrypoints.clone(),
        reservation_keys,
        template.routing_plans.clone(),
        template.native_amx_receipts.clone(),
        local_peer,
        signer.private_key(),
    )
    .expect("construct lifecycle-bound geometry-retirement payload")
}
fn sign_geometry_retirement_lifecycle_cursor(
    sequence: u64,
    previous_cursor_hash: Option<Hash>,
    binding: &AutonomousLifecycleAttemptBindingV1,
    phase: AutonomousLifecycleCursorPhaseV1,
    signer: &KeyPair,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
) -> AutonomousLifecycleCursorV1 {
    let unsigned = AutonomousLifecycleCursorUnsignedV1::new(
        sequence,
        previous_cursor_hash,
        binding.clone(),
        phase,
        PeerId::new(signer.public_key().clone()),
    )
    .expect("construct geometry-retirement lifecycle cursor");
    let signature = Signature::try_new(
        signer.private_key(),
        &unsigned
            .signing_preimage()
            .expect("encode geometry-retirement lifecycle cursor preimage"),
    )
    .expect("sign geometry-retirement lifecycle cursor");
    unsigned
        .finalize(
            <[u8; 96]>::try_from(signature.payload())
                .expect("BLS-normal geometry-retirement signature is exactly 96 bytes"),
            &payload.origin_proposal.descriptor.validator_set,
        )
        .expect("finalize geometry-retirement lifecycle cursor")
}
fn install_initial_geometry_retirement_lifecycle_cursor(
    kura: &Kura,
    generation: &AutonomousLifecycleProcessGenerationClaim,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    binding: &AutonomousLifecycleAttemptBindingV1,
    projection: crate::sumeragi::v2_core::ProductionInFlightFirstReleaseStateProjection,
    signer: &KeyPair,
) -> AutonomousLifecycleCursorV1 {
    let cursor = sign_geometry_retirement_lifecycle_cursor(
        1,
        None,
        binding,
        AutonomousLifecycleCursorPhaseV1::live(generation.generation(), projection)
            .expect("construct initial geometry-retirement Live phase"),
        signer,
        payload,
    );
    let read = kura
        .read_autonomous_lifecycle_cursor(payload, binding, generation)
        .expect("read absent geometry-retirement lifecycle cursor");
    assert!(read.cursor().is_none());
    let (_, lease) = read.into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(lease, cursor.clone())
            .expect("persist initial geometry-retirement lifecycle cursor")
            .cursor(),
        Some(&cursor),
    );
    cursor
}
fn append_geometry_retirement_lifecycle_phase(
    kura: &Kura,
    generation: &AutonomousLifecycleProcessGenerationClaim,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    binding: &AutonomousLifecycleAttemptBindingV1,
    current: &AutonomousLifecycleCursorV1,
    phase: AutonomousLifecycleCursorPhaseV1,
    signer: &KeyPair,
) -> AutonomousLifecycleCursorV1 {
    let cursor = sign_geometry_retirement_lifecycle_cursor(
        current
            .sequence()
            .checked_add(1)
            .expect("geometry-retirement lifecycle sequence remains in range"),
        Some(current.cursor_hash()),
        binding,
        phase,
        signer,
        payload,
    );
    let read = kura
        .read_autonomous_lifecycle_cursor(payload, binding, generation)
        .expect("read current geometry-retirement lifecycle cursor");
    assert_eq!(read.cursor(), Some(current));
    let (_, lease) = read.into_parts();
    assert_eq!(
        kura.compare_and_swap_autonomous_lifecycle_cursor(lease, cursor.clone())
            .expect("append geometry-retirement lifecycle cursor")
            .cursor(),
        Some(&cursor),
    );
    cursor
}
fn append_geometry_retirement_lifecycle_transition(
    kura: &Kura,
    generation: &AutonomousLifecycleProcessGenerationClaim,
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    binding: &AutonomousLifecycleAttemptBindingV1,
    current: &AutonomousLifecycleCursorV1,
    transition: ProductionInFlightFirstReleaseTransitionProjection,
    signer: &KeyPair,
) -> AutonomousLifecycleCursorV1 {
    let prepared = append_geometry_retirement_lifecycle_phase(
        kura,
        generation,
        payload,
        binding,
        current,
        AutonomousLifecycleCursorPhaseV1::prepared(generation.generation(), transition)
            .expect("construct geometry-retirement Prepared phase"),
        signer,
    );
    append_geometry_retirement_lifecycle_phase(
        kura,
        generation,
        payload,
        binding,
        &prepared,
        AutonomousLifecycleCursorPhaseV1::live(generation.generation(), transition.after)
            .expect("construct geometry-retirement successor Live phase"),
        signer,
    )
}
fn geometry_canonical_merge_terminal_projection(
    binding: &AutonomousLifecycleAttemptBindingV1,
    ready_qc: &iroha_data_model::block::consensus::LaneBlockQcV1,
) -> ProductionInFlightFirstReleaseStateProjection {
    let (_, validator_set_hash, validator_count) = binding.validator_set_identity();
    assert_eq!(validator_count, 4);
    assert_eq!(ready_qc.validator_set_hash, validator_set_hash);
    assert_eq!(ready_qc.signers_bitmap.len(), 1);
    let ready_signers = u128::from(ready_qc.signers_bitmap[0]);
    assert_eq!(ready_signers.count_ones(), 3);
    let validator_mask = (1_u128 << validator_count) - 1;
    assert_eq!(ready_signers & !validator_mask, 0);
    let producer = binding.producer_actor_projection();
    let local_actor = binding.local_validator_identity().1;
    assert_eq!(local_actor, producer);
    let durable_owners = ready_signers | producer;
    let reservation_group = binding.reservation_group_binding();
    let binding_a = canonical_lane_queue_reservation_group_identity_projection(reservation_group);
    let projection = ProductionInFlightFirstReleaseStateProjection {
        validator_count: u8::try_from(validator_count).expect("retirement committee count"),
        producer,
        producer_selected_owner: producer,
        replicated_carrier_owners: validator_mask & !producer,
        payload_binding_a: durable_owners,
        binding_a,
        queue: ProductionInFlightFirstReleaseQueueProjection {
            plan_state: IN_FLIGHT_FIRST_RELEASE_QUEUE_PLAN_TOMBSTONED,
            selected_count: reservation_group.reservation_count,
            reservation_state: IN_FLIGHT_FIRST_RELEASE_RESERVATION_COMMIT_FORGOTTEN,
        },
        carrier: ProductionInFlightFirstReleaseCarrierProjection {
            kura_active: durable_owners,
            execution_input_durable: durable_owners,
            ready_qc_durable: true,
        },
        session: ProductionInFlightFirstReleaseSessionProjection {
            bodies: durable_owners,
            ready_authorized: ready_signers,
            producer_alive: true,
            ..ProductionInFlightFirstReleaseSessionProjection::default()
        },
        history: ProductionInFlightFirstReleaseHistoryProjection {
            ever_queue_plan_v1: true,
            ever_reservation_v1: true,
            ever_execution_input_durable: durable_owners,
            ever_ready_authorized: ready_signers,
            ready_signed: ready_signers,
            ever_ready_qc_durable: true,
            reservation_committed_prefix: reservation_group.reservation_count,
            queue_plan_tombstoned_prefix: reservation_group.reservation_count,
            reservation_commit_forgotten_prefix: reservation_group.reservation_count,
            ..ProductionInFlightFirstReleaseHistoryProjection::default()
        },
        decision: ProductionInFlightFirstReleaseDecisionProjection {
            lane_commit_scope: binding_a,
            lane_commit_owner: local_actor,
            wsv_committed: true,
            application_count: 1,
            applied_by: local_actor,
            ..ProductionInFlightFirstReleaseDecisionProjection::default()
        },
        release: ProductionInFlightFirstReleaseReleaseProjection::default(),
    };
    assert!(
        production_in_flight_first_release_state_kernel(projection),
        "geometry canonical merge terminal projection must satisfy the production kernel"
    );
    projection
}
fn prepare_tombstoned_autonomous_archive(
    root: &Path,
) -> (Arc<Kura>, TombstonedAutonomousArchiveFixture) {
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, extended_activations) = retirement_test_geometry();
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = kura
        .lane_storage_entry(retiring_lane)
        .expect("retiring lane");
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload_template) = autonomous_retirement_payload_for_routes(
        retiring_lane,
        retiring_entry.dataspace_id,
        retiring_incarnation,
        LaneId::new(9),
        DataSpaceId::new(19),
        Hash::new(b"tombstoned-autonomous-unrelated-participant"),
        &producer,
    );
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"geometry-retirement-lifecycle-height-context"),
    ));
    let payload = lifecycle_bound_autonomous_retirement_payload(
        &payload_template,
        height_context_id,
        &producer,
    );
    let local_peer = PeerId::new(producer.public_key().clone());
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind geometry-retirement lifecycle signer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim geometry-retirement lifecycle process generation");
    let lane_block_height = payload.origin_proposal.descriptor.lane_block_height;
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload before terminal retirement");
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind geometry-retirement reservation group");
    let binding = AutonomousLifecycleAttemptBindingV1::from_payload(
        height_context_id,
        lane_block_height,
        &payload,
        reservation_group,
        &local_peer,
    )
    .expect("bind geometry-retirement lifecycle attempt");
    let retirement = crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
    let barrier = retirement
        .queue_release_barrier()
        .expect("derive geometry-retirement Queue barrier");
    let view_state_path = Kura::autonomous_lane_block_attempt_view_state_path_for_entry(
        &retiring_entry,
        root,
        lane_block_height,
        payload.origin_proposal.descriptor.proposal_height,
    );
    let retirement_projection = kura
        .authorize_autonomous_lane_slot_retirement_persistence(
            &payload,
            &retirement,
            &view_state_path,
        )
        .expect("authorize geometry-retirement persistence")
        .consume_for_persistence(&payload, &retirement, &view_state_path)
        .expect("consume exact geometry-retirement persistence authority");
    let release_context =
        AutonomousLaneReleaseProjectionContext::from_payload(&kura, &payload, &retirement)
            .expect("construct geometry-retirement release context");
    let entrypoint_hash = *payload
        .entrypoint_hashes
        .first()
        .expect("geometry-retirement payload has one entrypoint");
    assert_eq!(payload.entrypoint_hashes.len(), 1);
    let claim_path =
        Kura::autonomous_lane_entrypoint_claim_path(root, &payload.network_id, &entrypoint_hash);
    let retirement_hash = retirement
        .digest()
        .expect("hash geometry-retirement retirement");
    let pending_claim = AutonomousLaneEntrypointClaimV1::release_pending_for_payload(
        &payload,
        entrypoint_hash,
        retirement_hash,
    );
    let pending_projection = release_context
        .claim_transition_authorization(
            &claim_path,
            &pending_claim,
            false,
            AutonomousLaneClaimReleaseAuthorizationMode::QueuePrepared,
            0,
            AutonomousLaneReleasedClaimDisposition::QueueReleasePrepared,
        )
        .expect("authorize geometry-retirement ReleasePending claim")
        .consume_for_persistence(&claim_path, &pending_claim)
        .expect("consume geometry-retirement ReleasePending authority");
    let (queue_preparation_projection, claims_fully_released) = release_context
        .queue_preparation_authorization(&retirement, &barrier, false)
        .expect("authorize geometry-retirement Queue preparation")
        .consume_for_queue(&barrier)
        .expect("consume geometry-retirement Queue preparation authority");
    assert!(!claims_fully_released);
    let released_claim = AutonomousLaneEntrypointClaimV1::released_for_payload(
        &payload,
        entrypoint_hash,
        retirement_hash,
    );
    let released_projection = release_context
        .claim_transition_authorization(
            &claim_path,
            &released_claim,
            true,
            AutonomousLaneClaimReleaseAuthorizationMode::QueuePrepared,
            0,
            AutonomousLaneReleasedClaimDisposition::QueueReleasePrepared,
        )
        .expect("authorize geometry-retirement Released claim")
        .consume_for_persistence(&claim_path, &released_claim)
        .expect("consume geometry-retirement Released authority");
    let queue_finalization_projections = release_context
        .queue_finalization_authorization(&retirement, &barrier)
        .expect("authorize geometry-retirement Queue finalization")
        .consume_for_queue(&barrier)
        .expect("consume geometry-retirement Queue finalization authority");
    let mut lifecycle_cursor = install_initial_geometry_retirement_lifecycle_cursor(
        &kura,
        &generation,
        &payload,
        &binding,
        retirement_projection.before,
        &producer,
    );
    let pending_error = kura
        .first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            retiring_entry.dataspace_id,
            retiring_incarnation,
        )
        .expect_err("non-terminal autonomous work must block production retirement");
    assert_geometry_io_error(
        &pending_error,
        ErrorKind::WouldBlock,
        "lane attempt cannot archive before its slot retirement is durable",
    );
    lifecycle_cursor = append_geometry_retirement_lifecycle_phase(
        &kura,
        &generation,
        &payload,
        &binding,
        &lifecycle_cursor,
        AutonomousLifecycleCursorPhaseV1::prepared(generation.generation(), retirement_projection)
            .expect("prepare geometry-retirement persistence"),
        &producer,
    );
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("persist exact autonomous slot retirement");
    lifecycle_cursor = append_geometry_retirement_lifecycle_phase(
        &kura,
        &generation,
        &payload,
        &binding,
        &lifecycle_cursor,
        AutonomousLifecycleCursorPhaseV1::live(
            generation.generation(),
            retirement_projection.after,
        )
        .expect("complete geometry-retirement persistence"),
        &producer,
    );
    for transition in [pending_projection, queue_preparation_projection] {
        lifecycle_cursor = append_geometry_retirement_lifecycle_transition(
            &kura,
            &generation,
            &payload,
            &binding,
            &lifecycle_cursor,
            transition,
            &producer,
        );
    }
    kura.finalize_autonomous_lane_slot_release(&retirement, &barrier, network_id, epoch)
        .expect("finalize geometry-retirement Released claim");
    lifecycle_cursor = append_geometry_retirement_lifecycle_transition(
        &kura,
        &generation,
        &payload,
        &binding,
        &lifecycle_cursor,
        released_projection,
        &producer,
    );
    for transition in queue_finalization_projections {
        lifecycle_cursor = append_geometry_retirement_lifecycle_transition(
            &kura,
            &generation,
            &payload,
            &binding,
            &lifecycle_cursor,
            transition,
            &producer,
        );
    }
    let terminal_projection = queue_finalization_projections[2].after;
    let _terminal_cursor = append_geometry_retirement_lifecycle_phase(
        &kura,
        &generation,
        &payload,
        &binding,
        &lifecycle_cursor,
        AutonomousLifecycleCursorPhaseV1::Terminal {
            owner_generation: generation.generation(),
            projection: AutonomousLifecycleStableStateV1::from_production(terminal_projection),
        },
        &producer,
    );
    kura.first_release_lane_retirement_admissible_for_test(
        retiring_lane,
        retiring_entry.dataspace_id,
        retiring_incarnation,
    )
    .expect("production retirement policy accepts exact tombstoned autonomous evidence");
    kura.apply_lane_geometry_transition(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .expect("archive tombstoned autonomous lane");
    kura.mark_lane_geometry_catalog_published(
        &initial,
        &initial_incarnations,
        &initial_activations,
        None,
    )
    .expect("publish tombstoned autonomous retirement");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("tombstoned autonomous geometry journal");
    let retirement_transition = journal.records.last().expect("retirement transition");
    let binding = retirement_transition
        .operations
        .iter()
        .find_map(|operation| {
            (operation.lane_id == retiring_lane)
                .then_some(operation.previous.as_ref())
                .flatten()
        })
        .expect("retired autonomous lane binding")
        .clone();
    let archived_blocks = retiring_entry.blocks_dir(root);
    let lane_artifacts = archived_blocks.join(LANE_ARTIFACTS_DIR_NAME);
    let fixture = TombstonedAutonomousArchiveFixture {
        geometry: RetiredGeometryFixture {
            initial,
            extended,
            initial_incarnations,
            initial_activations,
            extended_incarnations,
            extended_activations,
            retained_bytes: Kura::regular_geometry_archive_tree_bytes(&archived_blocks)
                .expect("measure retained evidence tree")
                .checked_add(
                    Kura::file_len_or_zero(&retiring_entry.merge_log_path(root))
                        .expect("measure retained merge scaffold"),
                )
                .expect("retained fixture bytes fit"),
            retained_blocks: archived_blocks.clone(),
        },
        archived_blocks,
        binding,
        autonomous_attempt: lane_artifacts.join(format!(
            "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_PREFIX}_{lane_block_height:020}_{:020}.norito",
            payload.origin_proposal.descriptor.proposal_height,
        )),
        view_state: lane_artifacts.join(format!(
            "{AUTONOMOUS_LANE_BLOCK_ATTEMPT_VIEW_PREFIX}_{lane_block_height:020}_{:020}.norito",
            payload.origin_proposal.descriptor.proposal_height,
        )),
        height_pointer: lane_artifacts.join(format!(
            "{AUTONOMOUS_LANE_BLOCK_LATEST_ATTEMPT_PREFIX}_{lane_block_height:020}.norito"
        )),
        route_pointer: lane_artifacts.join(AUTONOMOUS_LANE_ROUTE_LATEST_ATTEMPT_FILE),
    };
    (kura, fixture)
}
fn native_amx_archive_finality(
    block: &SignedBlock,
    execution_commitment: ExecutionCommitment,
) -> V2FinalityArtifact {
    let mut keypairs = (0_u8..4)
        .map(|index| {
            KeyPair::try_from_seed(
                vec![0xD0_u8.saturating_add(index); 32],
                Algorithm::BlsNormal,
            )
            .expect("derive deterministic Native archive finality key")
        })
        .collect::<Vec<_>>();
    keypairs.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let roster = keypairs
        .iter()
        .map(|keypair| ValidatorPower {
            validator: PeerId::new(keypair.public_key().clone()),
            power: 1,
        })
        .collect::<Vec<_>>();
    let height = block.header().height().get();
    assert_eq!(height, 1, "Native archive fixture uses one global block");
    let network_id = crate::sumeragi::synthetic_network_id("native-amx-lane-archive-test");
    let (kagemusha_mint_finality_epoch_id, kagemusha_mint_finality_epoch_roster) =
        crate::kagemusha_v1_test_fixtures::mint_finality_roster_and_id(network_id, 0, &roster);
    let context = HeightContext {
        network_id,
        protocol_version: PROTOCOL_VERSION,
        height,
        epoch: 0,
        epoch_end_height: 100,
        next_epoch_snapshot: None,
        mode: ConsensusMode::Permissioned,
        parent_commit_qc: None,
        snapshot_bootstrap: None,
        quorum: DualQuorum::from_roster(&roster).expect("valid Native archive quorum"),
        roster,
        kagemusha_mint_finality_epoch_id,
        kagemusha_mint_finality_epoch_roster,
        nexus_amx_context_hash: Hash::new(b"Native archive AMX context"),
        execution_policy_hash: iroha_crypto::Hash::new(b"test execution policy"),
        da_layout: DataAvailabilityLayout {
            encoding: PayloadEncoding::ReedSolomon16,
            chunk_size_bytes: 1_024,
            data_shards: 1,
            parity_shards: 1,
            max_payload_size_bytes: 4_096,
            max_chunk_count: 8,
        },
        leader_seed: [0x6D; 32],
    };
    let subject = BlockSubject {
        parent_block_hash: block.header().prev_block_hash(),
        block_hash: block.hash(),
        payload_hash: block
            .canonical_proposal_wire_hash()
            .expect("hash Native archive proposal wire"),
    };
    let round = ConsensusRound {
        context_id: context.id(),
        height,
        view: block.header().view_change_index(),
    };
    let mut commit_qc = QuorumCertificate {
        round,
        proposal_round: round,
        phase: GlobalPhase::Commit,
        subject,
        execution_commitment,
        signers: vec![0, 1, 2],
        aggregate_signature: vec![1],
    };
    let preimage = commit_qc
        .signer_preimage(&context, 0)
        .expect("construct Native archive finality signer preimage");
    let signatures = commit_qc
        .signers
        .iter()
        .map(|index| {
            Signature::try_new(
                keypairs[usize::try_from(*index).expect("fixture signer index")].private_key(),
                &preimage,
            )
            .expect("sign Native archive finality vote")
            .payload()
            .to_vec()
        })
        .collect::<Vec<_>>();
    let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
    commit_qc.aggregate_signature = iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
        .expect("aggregate Native archive finality votes");
    let validator_set_pops = keypairs
        .iter()
        .map(|keypair| {
            bls_normal_pop_prove(keypair.private_key()).expect("derive Native archive finality PoP")
        })
        .collect();
    let artifact = V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops);
    artifact
        .verify()
        .expect("Native archive finality fixture is valid");
    artifact
}
fn prepare_native_amx_archive(root: &Path) -> (Arc<Kura>, NativeAmxArchiveFixture) {
    let (initial, extended) = retirement_test_configs();
    let (extended_incarnations, mut extended_activations) = retirement_test_geometry();
    extended_activations.insert(LaneId::new(1), 0);
    let initial_incarnations =
        BTreeMap::from([(LaneId::SINGLE, extended_incarnations[&LaneId::SINGLE])]);
    let initial_activations =
        BTreeMap::from([(LaneId::SINGLE, extended_activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        root,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let retiring_lane = LaneId::new(1);
    let retiring_entry = kura
        .lane_storage_entry(retiring_lane)
        .expect("Native archive participant lane");
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let mut proposal = certified_geometry_lane_block(
        retiring_lane,
        retiring_entry.dataspace_id,
        retiring_incarnation,
        1,
    )
    .proposal;
    proposal.descriptor.proposal_height = 1;
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    crate::lane_consensus::validate_lane_block_proposal(&proposal)
        .expect("valid Native archive participant proposal");
    let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    crate::kura::tests::install_network_index_test_outputs(&mut block, Vec::new());
    let block = Arc::new(block);
    kura.store_block(Arc::clone(&block))
        .expect("persist Native archive application block");
    let source_id = [0xA7; Hash::LENGTH];
    let result = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
    let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
        proposal.descriptor.accepted_transaction_hashes[0],
    );
    let settlement = iroha_data_model::block::consensus::NativeAmxParticipantSettlement::try_new(
        proposal.descriptor.lane_id,
        proposal.descriptor.dataspace_id,
        proposal.descriptor.lane_incarnation,
        proposal.descriptor.lane_block_height,
        1,
        None,
        vec![source_id],
    )
    .expect("valid Native participant control");
    let settlement_hash = settlement
        .computed_hash()
        .expect("hash Native archive settlement");
    let executed_block_wire = block
        .encode_wire()
        .expect("encode Native archive executed block wire");
    let executed_block_wire_len = u64::try_from(executed_block_wire.len())
        .expect("Native archive executed block wire length fits u64");
    let executed_block_wire_hash = Hash::new(&executed_block_wire);
    let leaf = NativeAmxApplicationManifestLeafV1 {
        version: iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        participant_height: proposal.descriptor.lane_block_height,
        participant_view: proposal.descriptor.lane_block_view,
        predecessor_height: proposal.descriptor.previous_lane_block_height,
        predecessor_descriptor_hash: proposal.descriptor.previous_lane_block_descriptor_hash,
        descriptor_hash: proposal.descriptor.descriptor_hash,
        proposal_hash: proposal.proposal_hash,
        settlement_hash,
        previous_native_settlement_hash: None,
        members: vec![NativeAmxApplicationManifestMemberV1 {
            entrypoint_index: proposal.descriptor.accepted_candidate_indices[0],
            source_id,
            entrypoint_hash,
            result_hash: result.hash(),
        }],
        application_block_height: 1,
        application_block_hash: block.hash(),
        executed_block_wire_hash,
    };
    leaf.validate()
        .expect("valid Native archive application manifest leaf");
    let tree = [HashOf::new(&leaf)].into_iter().collect::<MerkleTree<_>>();
    let manifest_root = tree
        .root()
        .map(Hash::from)
        .expect("one-leaf Native archive manifest root");
    let execution_commitment =
        ExecutionCommitment::new_with_native_amx_application_manifest_without_merge_carrier(
            Hash::new(b"Native archive parent state"),
            Hash::new(b"Native archive post state"),
            Hash::new(b"Native archive ordinary writes"),
            None,
            0,
            iroha_data_model::block::consensus_v2::NATIVE_AMX_APPLICATION_MANIFEST_VERSION,
            manifest_root,
            1,
            executed_block_wire_len,
            executed_block_wire_hash,
        )
        .expect("valid Native archive execution commitment");
    let finality = native_amx_archive_finality(block.as_ref(), execution_commitment);
    let _ = kura
        .store_v2_finality_artifact(&finality)
        .expect("persist Native archive finality");
    let checkpoint_hash = Hash::new(b"Native archive WSV checkpoint");
    kura.store_wsv_checkpoint(1, block.hash(), checkpoint_hash)
        .expect("persist Native archive WSV checkpoint");
    kura.store_commit_manifest(
        CommitManifest::new(1, block.hash(), None, None, checkpoint_hash, None)
            .with_authenticated_v2_commit_authority(&finality),
    )
    .expect("persist Native archive commit manifest");
    let finality_artifact_hash = HashOf::new(&finality);
    let manifest = NativeAmxParticipantApplicationManifestArtifactV1 {
        version: NativeAmxParticipantApplicationManifestArtifactV1::VERSION,
        leaf,
        leaf_index: 0,
        proof: tree.get_proof(0).expect("one-leaf Native archive proof"),
        manifest_root,
        manifest_leaf_count: 1,
        finality_artifact_hash,
    };
    Kura::validate_native_amx_participant_application_manifest_artifact(&manifest)
        .expect("validate Native archive manifest artifact");
    let receipt = NativeAmxParticipantApplicationReceiptArtifact {
        version: NativeAmxParticipantApplicationReceiptArtifact::VERSION,
        participant_proposal: proposal,
        participant_settlement: settlement,
        participant_settlement_hash: settlement_hash,
        application_block_height: 1,
        application_block_hash: block.hash(),
        executed_block_wire_hash,
        finality_artifact_hash,
        manifest_artifact_hash: HashOf::new(&manifest),
        source_ids: vec![source_id],
        entrypoint_indices: vec![0],
        entrypoint_hashes: vec![entrypoint_hash],
        result_hashes: vec![result.hash()],
        results: vec![result],
    };
    Kura::validate_native_amx_participant_application_receipt_artifact(&receipt)
        .expect("validate Native archive receipt artifact");
    let manifest_path =
        Kura::native_amx_application_manifest_path_for_entry(&retiring_entry, root, 1);
    fs::create_dir_all(
        manifest_path
            .parent()
            .expect("Native manifest path has an artifact directory"),
    )
    .expect("create Native archive artifact directory");
    assert!(
        kura.write_atomic_synced_noclobber(
            &manifest_path,
            &manifest
                .encode_framed()
                .expect("encode Native archive manifest"),
        )
        .expect("persist Native archive manifest")
    );
    let receipt_path =
        Kura::native_amx_participant_receipt_path_for_entry(&retiring_entry, root, 1);
    assert!(
        kura.write_atomic_synced_noclobber(
            &receipt_path,
            &receipt
                .encode_framed()
                .expect("encode Native archive receipt"),
        )
        .expect("persist Native archive receipt")
    );
    assert_eq!(
        kura.rebuild_native_amx_participant_receipt_latest_indexes_on_startup()
            .expect("publish Native archive latest index"),
        1
    );
    assert!(
        kura.read_native_amx_participant_application_history(
            receipt.participant_proposal.descriptor.lane_id
        )
        .expect("authenticate Native archive history")
        .drain_evidence(receipt.participant_proposal.descriptor.lane_block_height)
        .is_some(),
        "complete Native archive fixture must revalidate as drain evidence"
    );
    kura.apply_lane_geometry_transition(
        &extended,
        &initial,
        &extended_incarnations,
        &initial_incarnations,
        &extended_activations,
        &initial_activations,
        &BTreeSet::new(),
    )
    .expect("archive durably applied Native participant evidence");
    kura.mark_lane_geometry_catalog_published(
        &initial,
        &initial_incarnations,
        &initial_activations,
        None,
    )
    .expect("publish Native participant retirement");
    let journal = kura
        .read_lane_geometry_journal()
        .expect("Native archive geometry journal");
    let retirement_transition = journal.records.last().expect("retirement transition");
    let binding = retirement_transition
        .operations
        .iter()
        .find_map(|operation| {
            (operation.lane_id == retiring_lane)
                .then_some(operation.previous.as_ref())
                .flatten()
        })
        .expect("retired Native participant binding")
        .clone();
    let archived_blocks = retiring_entry.blocks_dir(root);
    let lane_artifacts = archived_blocks.join(LANE_ARTIFACTS_DIR_NAME);
    (
        kura,
        NativeAmxArchiveFixture {
            geometry: RetiredGeometryFixture {
                initial,
                extended,
                initial_incarnations,
                initial_activations,
                extended_incarnations,
                extended_activations,
                retained_bytes: Kura::regular_geometry_archive_tree_bytes(&archived_blocks)
                    .expect("measure retained evidence tree")
                    .checked_add(
                        Kura::file_len_or_zero(&retiring_entry.merge_log_path(root))
                            .expect("measure retained merge scaffold"),
                    )
                    .expect("retained fixture bytes fit"),
                retained_blocks: archived_blocks.clone(),
            },
            archived_blocks,
            binding,
            manifest: lane_artifacts.join(
                manifest_path
                    .file_name()
                    .expect("Native manifest path has a filename"),
            ),
            receipt: lane_artifacts.join(
                receipt_path
                    .file_name()
                    .expect("Native receipt path has a filename"),
            ),
            latest_index: lane_artifacts.join(NATIVE_AMX_PARTICIPANT_RECEIPTS_LATEST_INDEX_FILE),
        },
    )
}
fn durable_geometry_snapshot_identity(kura: &Kura, height: u64) -> (HashOf<BlockHeader>, Hash) {
    assert!(height > 0, "geometry GC test proof must be non-genesis");
    let mut previous = NonZeroUsize::new(kura.exact_durable_blocks_count().unwrap())
        .and_then(|height| kura.get_block(height));
    while u64::try_from(kura.exact_durable_blocks_count().unwrap()).expect("block count fits u64")
        < height
    {
        let mut block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, previous.as_deref())
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
        crate::kura::tests::install_network_index_test_outputs(&mut block, Vec::new());
        let block = Arc::new(block);
        kura.store_block(Arc::clone(&block))
            .expect("store durable geometry proof block");
        previous = Some(block);
    }
    let height_usize = NonZeroUsize::new(usize::try_from(height).expect("height fits usize"))
        .expect("non-zero height");
    let block_hash = kura
        .get_durable_block_hash(height_usize)
        .expect("durable geometry proof block hash");
    let state_hash = Hash::new([0xC0, u8::try_from(height).unwrap_or(u8::MAX)]);
    kura.store_wsv_checkpoint(height, block_hash, state_hash)
        .expect("store durable geometry proof WSV checkpoint");
    (block_hash, state_hash)
}
fn certified_geometry_lane_block(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    incarnation: Hash,
    lane_block_height: u64,
) -> CertifiedLaneBlockArtifact {
    let keypair = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let validator_set = vec![PeerId::new(keypair.public_key().clone())];
    let mut descriptor = LaneBlockDescriptorV1 {
        lane_id,
        dataspace_id,
        lane_incarnation: incarnation,
        proposal_height: lane_block_height.saturating_add(1).max(2),
        previous_lane_block_height: lane_block_height.saturating_sub(1),
        previous_lane_block_descriptor_hash: lane_block_height
            .checked_sub(1)
            .filter(|height| *height > 0)
            .map(|height| Hash::new(height.to_le_bytes())),
        lane_block_height,
        lane_block_view: 1,
        subject_hash: Hash::new(b"geometry-gc-certified-subject"),
        payload_ownership_hash: Hash::new(b"geometry-gc-certified-ownership"),
        rbc_instance_hash: Hash::new(b"geometry-gc-certified-rbc"),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![Hash::new(b"geometry-gc-certified-entrypoint")],
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set: validator_set.clone(),
        validator_count: 1,
        min_quorum: 1,
        qc_mode_tag: "permissioned:geometry-gc".to_owned(),
        descriptor_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    descriptor.descriptor_hash = descriptor.computed_descriptor_hash();
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    certified_geometry_lane_block_for_proposal(proposal, &keypair)
}
fn certified_geometry_lane_block_for_proposal(
    proposal: LaneBlockProposalV1,
    keypair: &iroha_crypto::KeyPair,
) -> CertifiedLaneBlockArtifact {
    let signer_pop = bls_normal_pop_prove(keypair.private_key()).expect("geometry GC signer PoP");
    let validator_set = proposal.descriptor.validator_set.clone();
    assert_eq!(
        validator_set,
        vec![PeerId::new(keypair.public_key().clone())],
        "geometry certified fixture uses its signing peer as the only validator"
    );
    let vote = |phase| {
        let body = proposal.vote_body(phase);
        LaneBlockVoteV1 {
            bls_signature: Signature::try_new(keypair.private_key(), &body.signature_preimage())
                .expect("geometry GC lane vote signature")
                .payload()
                .to_vec(),
            body,
            signer: PeerId::new(keypair.public_key().clone()),
            payload_availability_vote: None,
        }
    };
    let prepare_vote = vote(CertPhase::Prepare);
    let prepare_qc = aggregate_lane_block_votes_to_qc(
        prepare_vote.body.clone(),
        validator_set.clone(),
        std::slice::from_ref(&prepare_vote),
    )
    .expect("geometry GC prepare QC");
    let commit_vote = vote(CertPhase::Commit);
    let commit_qc = aggregate_lane_block_votes_to_qc(
        commit_vote.body.clone(),
        validator_set,
        std::slice::from_ref(&commit_vote),
    )
    .expect("geometry GC commit QC");
    CertifiedLaneBlockArtifact::new(
        CommittedLaneBlockSession {
            proposal,
            prepare_qc,
            commit_qc,
        },
        BTreeMap::from([(keypair.public_key().clone(), signer_pop)]),
    )
}
fn certified_geometry_autonomous_lane_block(
    payload: &crate::lane_consensus::LaneExecutablePayloadV1,
    validators: &[KeyPair],
) -> (CertifiedLaneBlockArtifact, AutonomousLaneMergeBundleV1) {
    let proposal = payload.origin_proposal.clone();
    let validator_set = validators
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    assert_eq!(validator_set, proposal.descriptor.validator_set);
    assert_eq!(proposal.descriptor.validator_count, 4);
    assert_eq!(proposal.descriptor.min_quorum, 3);
    let validator_set_pops = validators
        .iter()
        .map(|keypair| {
            bls_normal_pop_prove(keypair.private_key())
                .expect("geometry autonomous merge validator PoP")
        })
        .collect::<Vec<_>>();
    let availability_body = crate::lane_consensus::lane_payload_availability_body(
        payload,
        &proposal,
        payload.network_id,
        payload.epoch,
    )
    .expect("geometry autonomous merge availability body");
    let votes = |phase| {
        validators[..3]
            .iter()
            .map(|keypair| {
                let signer = PeerId::new(keypair.public_key().clone());
                let body = proposal.vote_body(phase);
                LaneBlockVoteV1 {
                    bls_signature: Signature::try_new(
                        keypair.private_key(),
                        &body.signature_preimage(),
                    )
                    .expect("geometry autonomous merge lane vote signature")
                    .payload()
                    .to_vec(),
                    body,
                    payload_availability_vote: (phase == CertPhase::Prepare).then(|| {
                        crate::lane_consensus::LanePayloadAvailabilityVoteV1::new_signed(
                            availability_body.clone(),
                            signer.clone(),
                            validator_set_pops.clone(),
                            keypair.private_key(),
                        )
                        .expect("geometry autonomous merge availability vote")
                    }),
                    signer,
                }
            })
            .collect::<Vec<_>>()
    };
    let prepare_qc = aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Prepare),
        validator_set.clone(),
        &votes(CertPhase::Prepare),
    )
    .expect("geometry autonomous merge prepare QC");
    let commit_qc = aggregate_lane_block_votes_to_qc(
        proposal.vote_body(CertPhase::Commit),
        validator_set,
        &votes(CertPhase::Commit),
    )
    .expect("geometry autonomous merge commit QC");
    let signer_pops = validators[..3]
        .iter()
        .zip(&validator_set_pops[..3])
        .map(|(keypair, pop)| (keypair.public_key().clone(), pop.clone()))
        .collect();
    let certified = CertifiedLaneBlockArtifact::new(
        CommittedLaneBlockSession {
            proposal,
            prepare_qc: prepare_qc.clone(),
            commit_qc,
        },
        signer_pops,
    );
    let bundle = AutonomousLaneMergeBundleV1 {
        version: AutonomousLaneMergeBundleV1::VERSION,
        autonomous: AutonomousLaneBlockArtifact {
            format: crate::kura::AutonomousLaneBlockArtifactFormat::Current,
            executable_payload: payload.clone(),
            availability_certificate: Some(
                crate::lane_consensus::DurableLanePayloadAvailabilityCertificateV1 {
                    certificate: prepare_qc,
                },
            ),
            view_checkpoint: None,
            new_view_certificates: Vec::new(),
        },
        certified: certified.clone(),
    };
    Kura::validate_autonomous_lane_merge_bundle(&bundle, payload.network_id, payload.epoch)
        .expect("validate geometry autonomous merge bundle");
    (certified, bundle)
}
struct MergeAppliedRetirementWork {
    certified: CertifiedLaneBlockArtifact,
    ownership: SumeragiLanePayloadOwnership,
    entry: MergeLedgerEntry,
    carrier: Arc<SignedBlock>,
    release: LaneGeometryMergeRelease,
}
fn install_merge_applied_retirement_work(
    kura: &Kura,
    lane_incarnation: Hash,
) -> MergeAppliedRetirementWork {
    let lane_id = LaneId::new(1);
    let dataspace_id = DataSpaceId::new(8);
    let mut validators = (0..4)
        .map(|_| crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal))
        .collect::<Vec<_>>();
    validators.sort_by(|left, right| {
        PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
    });
    let validator_set = validators
        .iter()
        .map(|keypair| PeerId::new(keypair.public_key().clone()))
        .collect::<Vec<_>>();
    let network_id = geometry_fixture_network_id();
    let epoch = 1;
    let transaction = TransactionBuilder::new(
        network_id,
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "geometry durability merge execution".to_owned(),
    )])
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    )
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let entrypoint = TransactionEntrypoint::External(transaction);
    let entrypoint_hash = entrypoint.hash();
    let (proposal, ownership) = geometry_lane_proposal_and_ownership(
        lane_id,
        dataspace_id,
        lane_incarnation,
        2,
        0,
        1,
        0,
        Hash::from(entrypoint_hash),
        validator_set,
    );
    let producer_id = crate::lane_consensus::deterministic_lane_author(
        &proposal.descriptor.validator_set,
        proposal.descriptor.lane_block_height,
    )
    .expect("four-validator retirement committee has a deterministic producer");
    let producer = validators
        .iter()
        .find(|keypair| keypair.public_key() == producer_id.public_key())
        .expect("retirement producer belongs to the exact committee");
    let height_context_id = HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
        Hash::new(b"geometry-durability-merge-height-context"),
    ));
    let local_peer = PeerId::new(producer.public_key().clone());
    let (reservation_owner_hash, proposal_identity_hash) =
        crate::sumeragi::lane_planner::autonomous_lane_reservation_identity_hashes_for_proposal(
            network_id,
            height_context_id,
            epoch,
            &proposal,
            &local_peer,
        )
        .expect("derive geometry merge lifecycle reservation identities");
    let routing_plan = crate::queue::RoutingPlan::single(crate::queue::RoutingDecision::new(
        lane_id,
        dataspace_id,
    ));
    let reservation = crate::queue::LaneQueueReservationKeyV1 {
        version: crate::queue::LaneQueueReservationKeyV1::VERSION,
        entrypoint_hash,
        queue_plan_admission_binding_hash: Hash::new(
            b"geometry-durability-merge-queue-plan-binding",
        ),
        routing_plan_digest: routing_plan.digest(),
        coordinator_leg: routing_plan.coordinator_leg(),
        lane_id,
        dataspace_id,
        lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash,
        proposal_identity_hash,
    };
    let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal.clone(),
        vec![entrypoint],
        vec![reservation],
        vec![routing_plan],
        vec![None],
        local_peer.clone(),
        producer.private_key(),
    )
    .expect("construct geometry autonomous merge payload");
    kura.bind_local_peer_id(local_peer.clone())
        .expect("bind geometry merge lifecycle signer");
    let generation = kura
        .claim_autonomous_lifecycle_process_generation(network_id, &local_peer)
        .expect("claim geometry merge lifecycle process generation");
    let (certified, bundle) = certified_geometry_autonomous_lane_block(&payload, &validators);
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist merge-applied retirement executable payload");
    let recovered = kura
        .recover_autonomous_lane_block_payload(&proposal, network_id, epoch)
        .expect("recover merge-applied retirement execution input");
    kura.persist_lane_block_execution_input(&recovered)
        .expect("persist merge-applied retirement execution input");
    let availability = bundle
        .autonomous
        .availability_certificate
        .clone()
        .expect("geometry autonomous merge bundle has READY evidence");
    kura.persist_lane_payload_availability_certificate(
        lane_id,
        proposal.descriptor.lane_block_height,
        availability,
        network_id,
        epoch,
    )
    .expect("persist merge-applied retirement READY certificate");
    let session = CommittedLaneBlockSession {
        proposal: certified.proposal.clone(),
        prepare_qc: certified.prepare_qc.clone(),
        commit_qc: certified.commit_qc.clone(),
    };
    kura.persist_committed_lane_block_session(&session, &certified.signer_pops)
        .expect("persist merge-applied retirement certified source");
    let durable_source = kura
        .durable_autonomous_lane_merge_source(
            lane_id,
            proposal.descriptor.lane_block_height,
            network_id,
            epoch,
        )
        .expect("read merge-applied retirement durable source");
    assert_eq!(
        durable_source.bundle, bundle,
        "durable merge source must preserve the exact authenticated fixture bundle"
    );
    let mut genesis: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    crate::kura::tests::install_network_index_test_outputs(&mut genesis, Vec::new());
    let genesis = Arc::new(genesis);
    kura.store_block(Arc::clone(&genesis))
        .expect("store merge-applied retirement genesis");
    let result = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::default()));
    let settlement = LaneBlockCommitment {
        block_height: 1,
        lane_id,
        lane_incarnation,
        dataspace_id,
        tx_count: 1,
        total_local_amount: "0".parse().expect("zero local amount"),
        total_xor_due: "0".parse().expect("zero XOR due"),
        total_xor_after_haircut: "0".parse().expect("zero XOR after haircut"),
        total_xor_variance: "0".parse().expect("zero XOR variance"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
        .expect("merge-applied retirement settlement hash");
    let source_bundle = durable_source.source_bundle;
    let source_bundle_hash = durable_source.bundle_hash;
    let execution = MergeLaneExecution {
        source_bundle_hash,
        source_bundle,
        proposal: proposal.clone(),
        origin_proposal: proposal.clone(),
        prepare_qc: certified.prepare_qc.clone(),
        commit_qc: certified.commit_qc.clone(),
        signer_proofs: certified
            .signer_pops
            .iter()
            .map(|(public_key, proof_of_possession)| MergeLaneSignerProof {
                public_key: public_key.clone(),
                proof_of_possession: proof_of_possession.clone(),
            })
            .collect(),
        autonomous_network_id: network_id,
        autonomous_epoch: epoch,
        autonomous_payload_hash: payload.payload_hash,
        entrypoint_hashes: payload.entrypoint_hashes.clone(),
        authenticated_signed_replay_aliases: vec![None; payload.entrypoints.len()],
        entrypoints: payload.entrypoints.clone(),
        reservation_keys: payload
            .reservation_keys
            .iter()
            .map(|reservation| {
                norito::encode_canonical(reservation)
                    .expect("encode merge-applied retirement reservation key")
            })
            .collect(),
        routing_plans: payload
            .routing_plans
            .iter()
            .map(|routing_plan| {
                norito::encode_canonical(routing_plan)
                    .expect("encode merge-applied retirement routing plan")
            })
            .collect(),
        native_amx_receipts: payload.native_amx_receipts.clone(),
        result_hashes: vec![Hash::from(result.hash())],
        results: vec![result],
        settlement_commitment: settlement,
        settlement_hash,
        fastpq_transcripts: Vec::new().into(),
    };
    let lanes = vec![execution];
    let base_state_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"geometry-durability-merge-base-state"));
    let write_set_root = Hash::new(b"geometry-durability-merge-write-set");
    let mut batch = MergeExecutionBatch {
        version: 1,
        base_state_height: 1,
        base_state_hash,
        application_block_header: BlockHeader::new(
            nonzero!(2_u64),
            Some(genesis.hash()),
            None,
            1,
            0,
        ),
        entrypoint_count: 1,
        entrypoint_merkle_root: crate::merge::merge_execution_entrypoint_merkle_root(&lanes)
            .expect("merge-applied retirement entrypoint root"),
        result_merkle_root: crate::merge::merge_execution_result_merkle_root(&lanes)
            .expect("merge-applied retirement result root"),
        execution_root: crate::merge::merge_execution_root(&lanes),
        lanes,
        application_write_set_root: Hash::new(b"geometry-durability-merge-application-write-set"),
        write_set_root,
        expected_post_state_hash: crate::merge::merge_expected_post_state_hash(
            1,
            base_state_hash,
            write_set_root,
        ),
        batch_hash: Hash::prehashed([0; Hash::LENGTH]),
    };
    batch.batch_hash = crate::merge::merge_execution_batch_hash(&batch);
    let validator_set = Vec::<PeerId>::new();
    let active_lanes = vec![MergeLaneBinding {
        lane_id,
        dataspace_id,
        lane_config_hash: Hash::new(b"geometry-durability-merge-lane-config"),
        incarnation: lane_incarnation,
        activation_height: 1,
    }];
    let mut entry = MergeLedgerEntry {
        version: MergeLedgerEntry::VERSION,
        epoch_id: 1,
        lane_catalog_hash: Hash::new(b"geometry-durability-merge-catalog"),
        active_lanes,
        lane_authority_catalog:
            iroha_data_model::merge::MergeLaneAuthorityCatalogV1::from_lane_committees(
                std::slice::from_ref(&certified.proposal.descriptor.validator_set),
            )
            .expect("catalog matches the certified retirement fixture lane authority"),
        incarnation_root: Hash::new(b"geometry-durability-merge-incarnations"),
        activation_root: Hash::new(b"geometry-durability-merge-activations"),
        lane_snapshots: Vec::new(),
        global_state_root: Hash::new(b"geometry-durability-merge-global-state"),
        merge_qc: MergeQuorumCertificate::new(
            0,
            1,
            2,
            genesis.hash(),
            iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
                Hash::new(b"geometry-durability-merge-chain"),
            )),
            VALIDATOR_SET_HASH_VERSION_V1,
            HashOf::new(&validator_set),
            validator_set,
            Vec::new(),
            Vec::new(),
            Vec::new(),
            Hash::new(b"geometry-durability-merge-qc"),
        ),
        execution_batch: Some(batch),
        lane_drain_certificates: Vec::new(),
    };
    let mut carrier: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, Some(&genesis))
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    let execution_batch = entry
        .execution_batch
        .as_mut()
        .expect("merge-applied retirement batch");
    execution_batch.application_block_header =
        crate::merge::merge_application_header_from_carrier(&carrier.header());
    execution_batch.batch_hash = crate::merge::merge_execution_batch_hash(execution_batch);
    carrier.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(&entry)),
    ));
    crate::kura::tests::install_network_index_test_outputs(&mut carrier, Vec::new());
    let carrier = Arc::new(carrier);
    assert_eq!(
        entry.merge_qc.view,
        carrier.header().view_change_index(),
        "merge fixture QC and carrier use the same view"
    );
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store merge-applied retirement carrier");
    let finality_chain =
        crate::kura::tests::persist_v2_finality_chain_through(kura, nonzero!(2_usize));
    assert_eq!(
        finality_chain
            .last()
            .expect("merge carrier finality")
            .commit_qc
            .execution_commitment
            .merge_carrier,
        Some(
            iroha_data_model::block::consensus_v2::MergeCarrierCommitmentV1::new(
                entry.canonical_hash(),
            ),
        ),
        "merge carrier finality must authenticate the exact durable entry"
    );
    kura.persist_merge_lane_block_application_receipts(
        &entry,
        carrier.header().height().get(),
        carrier.hash(),
    )
    .expect("persist merge-applied retirement receipt");
    let marker_set = crate::state::State::expected_merge_execution_marker_payloads(
        &entry,
        entry
            .execution_batch
            .as_ref()
            .expect("merge-applied retirement batch"),
    )
    .expect("derive merge-applied retirement marker set");
    let release = kura
        .geometry_merge_release(
            &entry,
            entry
                .execution_batch
                .as_ref()
                .expect("merge-applied retirement batch"),
            entry
                .execution_batch
                .as_ref()
                .and_then(|batch| batch.lanes.first())
                .expect("merge-applied retirement lane execution"),
            LaneGeometryMergeCarrier {
                block_height: carrier.header().height().get(),
                block_hash: carrier.hash(),
                entry_hash: entry.canonical_hash(),
            },
            geometry_merge_marker_set_root(&marker_set),
        )
        .expect("derive merge-applied retirement release");
    let reservation_group =
        lane_queue_reservation_group_binding_from_ordered_keys(payload.reservation_keys.iter())
            .expect("bind geometry merge lifecycle reservation group");
    let binding = AutonomousLifecycleAttemptBindingV1::from_payload(
        height_context_id,
        proposal.descriptor.lane_block_height,
        &payload,
        reservation_group,
        &local_peer,
    )
    .expect("bind geometry merge lifecycle attempt");
    let terminal_projection =
        geometry_canonical_merge_terminal_projection(&binding, &certified.prepare_qc);
    let live_cursor = install_initial_geometry_retirement_lifecycle_cursor(
        kura,
        &generation,
        &payload,
        &binding,
        terminal_projection,
        &producer,
    );
    let _terminal_cursor = append_geometry_retirement_lifecycle_phase(
        kura,
        &generation,
        &payload,
        &binding,
        &live_cursor,
        AutonomousLifecycleCursorPhaseV1::Terminal {
            owner_generation: generation.generation(),
            projection: AutonomousLifecycleStableStateV1::from_production(terminal_projection),
        },
        &producer,
    );
    let source_publication = kura
        .persist_autonomous_lifecycle_canonical_terminal_outcomes_pending(&entry)
        .expect("persist merge-applied canonical terminal source outcome")
        .expect("merge-applied execution batch publishes a canonical terminal source outcome");
    let mut source_authorizations = source_publication
        .consume_for_v2_apply(&entry)
        .expect("consume exact merge-applied canonical source publication");
    assert_eq!(
        source_authorizations.len(),
        1,
        "single-lane merge fixture must publish one terminal source outcome"
    );
    let (published_group, source_authorization) = source_authorizations
        .pop()
        .expect("single-lane canonical terminal source authorization");
    let (authorized_group, ordered_keys, pending_outcome_hash) = source_authorization
        .consume_for_queue()
        .expect("consume exact canonical Queue source authorization");
    assert_eq!(published_group, reservation_group);
    assert_eq!(authorized_group, reservation_group);
    assert_eq!(ordered_keys, payload.reservation_keys);
    kura.complete_autonomous_lifecycle_terminal_outcome(
        reservation_group,
        terminal_projection,
        true,
        pending_outcome_hash,
    )
    .expect("complete merge-applied canonical terminal source outcome");
    MergeAppliedRetirementWork {
        certified,
        ownership,
        entry,
        carrier,
        release,
    }
}
#[allow(clippy::too_many_arguments)]
fn geometry_lane_proposal_and_ownership(
    lane_id: LaneId,
    dataspace_id: DataSpaceId,
    lane_incarnation: Hash,
    proposal_height: u64,
    proposal_view: u64,
    lane_block_height: u64,
    lane_block_view: u64,
    entrypoint_hash: Hash,
    validator_set: Vec<PeerId>,
) -> (LaneBlockProposalV1, SumeragiLanePayloadOwnership) {
    let validator_count = u32::try_from(validator_set.len()).expect("geometry committee count");
    let min_quorum = u32::try_from(crate::sumeragi::network_topology::commit_quorum_from_len(
        validator_set.len(),
    ))
    .expect("geometry committee quorum");
    let mut ownership = SumeragiLanePayloadOwnership {
        proposal_height,
        proposal_view,
        lane_id,
        dataspace_id,
        lane_incarnation,
        lane_block_height,
        lane_block_view,
        subject_hash: Hash::new(b"geometry-retirement-subject-placeholder"),
        qc_mode_tag: "permissioned:geometry-retirement".to_owned(),
        accepted_candidate_indices: vec![0],
        accepted_transaction_hashes: vec![entrypoint_hash],
        previous_lane_block_height: lane_block_height.saturating_sub(1),
        previous_lane_block_descriptor_hash: lane_block_height
            .checked_sub(1)
            .filter(|height| *height > 0)
            .map(|height| Hash::new(height.to_le_bytes())),
        lane_block_descriptor_hash: Some(Hash::new(b"geometry-retirement-descriptor-placeholder")),
        lane_block_descriptor_validator_set: validator_set.clone(),
        lane_block_descriptor_validator_count: validator_count,
        lane_block_descriptor_min_quorum: min_quorum,
        payload_ownership_hash: Hash::new(b"geometry-retirement-payload-placeholder"),
        rbc_instance_hash: Hash::new(b"geometry-retirement-rbc-placeholder"),
    };
    let replay = ownership
        .compute_replay_hashes()
        .expect("geometry retirement replay hashes");
    ownership.subject_hash = replay.subject_hash;
    ownership.payload_ownership_hash = replay.payload_ownership_hash;
    ownership.rbc_instance_hash = replay.rbc_instance_hash;
    ownership.lane_block_descriptor_hash = Some(replay.lane_block_descriptor_hash);
    let descriptor = LaneBlockDescriptorV1 {
        lane_id,
        dataspace_id,
        lane_incarnation,
        proposal_height,
        previous_lane_block_height: ownership.previous_lane_block_height,
        previous_lane_block_descriptor_hash: ownership.previous_lane_block_descriptor_hash,
        lane_block_height,
        lane_block_view,
        subject_hash: ownership.subject_hash,
        payload_ownership_hash: ownership.payload_ownership_hash,
        rbc_instance_hash: ownership.rbc_instance_hash,
        accepted_candidate_indices: ownership.accepted_candidate_indices.clone(),
        accepted_transaction_hashes: ownership.accepted_transaction_hashes.clone(),
        validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
        validator_set_hash: HashOf::new(&validator_set),
        validator_set,
        validator_count,
        min_quorum,
        qc_mode_tag: ownership.qc_mode_tag.clone(),
        descriptor_hash: replay.lane_block_descriptor_hash,
    };
    let mut proposal = LaneBlockProposalV1 {
        descriptor,
        proposal_hash: Hash::prehashed([0; Hash::LENGTH]),
        payload_block_hint: None,
    };
    proposal.proposal_hash = proposal.computed_proposal_hash();
    (proposal, ownership)
}
fn geometry_native_amx_receipt(
    network_id: iroha_data_model::NetworkId,
    source_id: [u8; Hash::LENGTH],
    entrypoint_hash: HashOf<TransactionEntrypoint>,
    plan: &crate::queue::RoutingPlan,
    coordinator_proposal: &LaneBlockProposalV1,
    participant_lane_incarnation: Hash,
    epoch: u64,
    participant_keypair: &KeyPair,
) -> NativeAmxReceipt {
    let crate::queue::RoutingPlan::NativeAmx(native_plan) = plan else {
        panic!("geometry retirement fixture requires a native AMX plan");
    };
    let participant = native_plan
        .participants
        .first()
        .expect("geometry retirement fixture participant");
    let participant_validator_set = vec![PeerId::new(participant_keypair.public_key().clone())];
    let descriptor = &coordinator_proposal.descriptor;
    let (participant_proposal, _) = geometry_lane_proposal_and_ownership(
        participant.route.lane_id,
        participant.route.dataspace_id,
        participant_lane_incarnation,
        descriptor.proposal_height,
        descriptor.lane_block_view,
        1,
        0,
        Hash::from(entrypoint_hash),
        participant_validator_set.clone(),
    );
    let participant_descriptor = &participant_proposal.descriptor;
    let mut prepare_body = NativeAmxAttestationBodyV2 {
        round: ConsensusRound {
            context_id: HeightContextId(HashOf::<HeightContext>::from_untyped_unchecked(
                Hash::new(b"geometry-native-amx-v2-context"),
            )),
            height: descriptor.proposal_height,
            view: descriptor.lane_block_view,
        },
        epoch,
        network_id,
        source_id,
        tx_entrypoint_hash: entrypoint_hash,
        plan_digest: plan.digest(),
        phase: NativeAmxPhase::Prepare,
        coordinator_lane_id: descriptor.lane_id,
        coordinator_dataspace_id: descriptor.dataspace_id,
        coordinator_lane_incarnation: descriptor.lane_incarnation,
        participant_lane_id: participant.route.lane_id,
        participant_dataspace_id: participant.route.dataspace_id,
        participant_lane_incarnation,
        participant_previous_block_height: participant_descriptor.previous_lane_block_height,
        participant_previous_block_descriptor_hash: participant_descriptor
            .previous_lane_block_descriptor_hash,
        participant_lane_block_height: participant_descriptor.lane_block_height,
        participant_lane_block_view: participant_descriptor.lane_block_view,
        participant_proposal_hash: participant_proposal.proposal_hash,
        participant_settlement_commitment: Hash::prehashed([0; Hash::LENGTH]),
        participant_validator_set_hash: HashOf::new(&participant_validator_set),
        participant_validator_count: 1,
        participant_min_quorum: 1,
        authority_context_height: descriptor.proposal_height,
        planned_coordinator_block_height: descriptor.lane_block_height,
        coordinator_lane_block_view: descriptor.lane_block_view,
        coordinator_proposal_hash: coordinator_proposal.proposal_hash,
    };
    prepare_body.participant_settlement_commitment = prepare_body
        .computed_grouped_participant_settlement_commitment(None, &[prepare_body.source_id])
        .expect("single-source test fixture settlement is valid");
    let participant_settlement = prepare_body
        .computed_grouped_participant_settlement(None, &[prepare_body.source_id])
        .expect("single-source test fixture settlement is valid");
    let participant_settlement_hash = participant_settlement
        .computed_hash()
        .expect("geometry participant settlement hashes");
    let participant_pop = bls_normal_pop_prove(participant_keypair.private_key())
        .expect("geometry retirement participant PoP");
    let qc = |body| {
        NativeAmxAttestationQcV2::try_new(
            body,
            VALIDATOR_SET_HASH_VERSION_V1,
            HashOf::new(&participant_validator_set),
            participant_validator_set.clone(),
            vec![participant_pop.clone()],
            vec![1],
            vec![0_u8; crate::native_amx::NATIVE_AMX_BLS_PROOF_BYTES],
        )
        .expect("geometry fixture validator set and proofs must align")
    };
    let prepare_qc = qc(prepare_body);
    let mut commit_body = prepare_body;
    commit_body.phase = NativeAmxPhase::Commit;
    let commit_qc = qc(commit_body);
    NativeAmxReceipt {
        version: 2,
        source_id,
        network_id,
        plan_digest: plan.digest(),
        lane_id: descriptor.lane_id,
        dataspace_id: descriptor.dataspace_id,
        lane_incarnation: descriptor.lane_incarnation,
        authority_context_height: descriptor.proposal_height,
        lane_block_height: descriptor.lane_block_height,
        lane_block_view: descriptor.lane_block_view,
        coordinator_proposal_hash: coordinator_proposal.proposal_hash,
        legs: vec![NativeAmxLegRecordV2 {
            lane_id: participant.route.lane_id,
            dataspace_id: participant.route.dataspace_id,
            participant_proposal,
            participant_settlement,
            participant_settlement_hash,
            prepare_qc,
            commit_qc,
        }],
    }
}
fn autonomous_retirement_payload(
    coordinator_incarnation: Hash,
    participant_lane_id: LaneId,
    participant_dataspace_id: DataSpaceId,
    participant_incarnation: Hash,
    producer: &KeyPair,
) -> (
    iroha_data_model::NetworkId,
    u64,
    crate::lane_consensus::LaneExecutablePayloadV1,
) {
    autonomous_retirement_payload_for_routes(
        LaneId::SINGLE,
        DataSpaceId::new(7),
        coordinator_incarnation,
        participant_lane_id,
        participant_dataspace_id,
        participant_incarnation,
        producer,
    )
}
#[allow(clippy::too_many_arguments)]
fn autonomous_retirement_payload_for_routes(
    coordinator_lane_id: LaneId,
    coordinator_dataspace_id: DataSpaceId,
    coordinator_incarnation: Hash,
    participant_lane_id: LaneId,
    participant_dataspace_id: DataSpaceId,
    participant_incarnation: Hash,
    producer: &KeyPair,
) -> (
    iroha_data_model::NetworkId,
    u64,
    crate::lane_consensus::LaneExecutablePayloadV1,
) {
    let network_id = geometry_fixture_network_id();
    let transaction = TransactionBuilder::new(
        network_id,
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "geometry retirement payload".to_owned(),
    )])
    .with_admission_intent(
        iroha_data_model::transaction::TransactionAdmissionIntent::QueuePlanSynced,
    )
    .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key());
    let entrypoint = TransactionEntrypoint::External(transaction);
    let entrypoint_hash = entrypoint.hash();
    let mut source_id = [0_u8; Hash::LENGTH];
    source_id.copy_from_slice(entrypoint_hash.as_ref());
    let coordinator =
        crate::queue::RoutingDecision::new(coordinator_lane_id, coordinator_dataspace_id);
    let participant = crate::queue::RouteLeg::new(
        crate::queue::RoutingDecision::new(participant_lane_id, participant_dataspace_id),
        crate::queue::RouteLegRole::Participant,
    );
    let plan = crate::queue::RoutingPlan::native_amx(coordinator, vec![participant]);
    let (proposal, _) = geometry_lane_proposal_and_ownership(
        coordinator_lane_id,
        coordinator_dataspace_id,
        coordinator_incarnation,
        42,
        0,
        1,
        0,
        Hash::from(entrypoint_hash),
        vec![PeerId::new(producer.public_key().clone())],
    );
    let epoch = 9;
    let receipt = geometry_native_amx_receipt(
        network_id,
        source_id,
        entrypoint_hash,
        &plan,
        &proposal,
        participant_incarnation,
        epoch,
        producer,
    );
    let reservation = crate::queue::LaneQueueReservationKeyV1 {
        version: crate::queue::LaneQueueReservationKeyV1::VERSION,
        entrypoint_hash,
        queue_plan_admission_binding_hash: Hash::new(
            b"geometry-retirement-queue-plan-admission-binding",
        ),
        routing_plan_digest: plan.digest(),
        coordinator_leg: plan.coordinator_leg(),
        lane_id: proposal.descriptor.lane_id,
        dataspace_id: proposal.descriptor.dataspace_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        proposal_height: proposal.descriptor.proposal_height,
        lane_block_height: proposal.descriptor.lane_block_height,
        lane_block_view: proposal.descriptor.lane_block_view,
        reservation_owner_hash: Hash::new(b"geometry-retirement-reservation-owner"),
        proposal_identity_hash: Hash::new(b"geometry-retirement-proposal-identity"),
    };
    let payload = crate::lane_consensus::LaneExecutablePayloadV1::new_signed_with_reservations(
        network_id,
        epoch,
        proposal,
        vec![entrypoint],
        vec![reservation],
        vec![plan],
        vec![Some(receipt)],
        PeerId::new(producer.public_key().clone()),
        producer.private_key(),
    )
    .expect("geometry autonomous retirement payload");
    (network_id, epoch, payload)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[test]
fn retirement_maintenance_preserves_recovery_order_before_invalid_frontier() {
    let temp = TempDir::new().expect("temporary directory");
    let root = temp.path().join("retirement-maintenance-order");
    let (initial, extended) = retirement_test_configs();
    let (incarnations, activations) = retirement_test_geometry();
    let initial_incarnations = BTreeMap::from([(LaneId::SINGLE, incarnations[&LaneId::SINGLE])]);
    let initial_activations = BTreeMap::from([(LaneId::SINGLE, activations[&LaneId::SINGLE])]);
    let (kura, _, _) = open_published_retirement_kura(
        &root,
        &initial,
        &extended,
        &initial_incarnations,
        &incarnations,
        &initial_activations,
        &activations,
    );
    let retiring_lane = LaneId::new(1);
    let entry = kura
        .lane_storage_entry(retiring_lane)
        .expect("retiring lane entry");
    let incarnation = incarnations[&retiring_lane];
    let _work = install_merge_applied_retirement_work(&kura, incarnation);
    let journal_before = fs::read(kura.lane_geometry_journal_path()).expect("geometry journal");
    let (certified_data, certified_index) =
        Kura::certified_lane_block_paths_for_entry(&entry, &root);
    let data_before = fs::read(&certified_data).expect("certified data");
    let index_before = fs::read(&certified_index).expect("certified index");
    fs::remove_file(&certified_data).expect("interrupt certified pair after frontier publication");
    fs::remove_file(&certified_index).expect("remove interrupted certified index");

    let (receipt_data, receipt_index) =
        Kura::lane_block_application_receipt_paths_for_entry(&entry, &root);
    let receipt_temp = receipt_data.with_extension("norito.tmp");
    let receipt_index_temp = receipt_index.with_extension("index.tmp");
    // Stage the exact authenticated receipt pair, as at a real compaction
    // interruption. Arbitrary bytes would be corruption, not recoverable work.
    let unpublished = fs::read(&receipt_data).expect("retained authenticated receipt data");
    let unpublished_index = fs::read(&receipt_index).expect("retained receipt index");
    fs::write(&receipt_temp, &unpublished).expect("stage later fixed-pair data maintenance");
    fs::write(&receipt_index_temp, &unpublished_index)
        .expect("stage later fixed-pair index maintenance");
    let frontier_path = Kura::lane_merge_application_frontier_path_for_entry(&entry, &root);
    let frontier_before = fs::read(&frontier_path).expect("authenticated merge frontier");
    let mut substituted: LaneMergeApplicationFrontierV1 =
        norito::decode_canonical(&frontier_before).expect("decode original frontier");
    substituted.application_block_hash =
        HashOf::from_untyped_unchecked(Hash::new(b"foreign maintenance carrier"));
    let substituted_bytes = norito::encode_canonical(&substituted).expect("encode substitution");
    fs::write(&frontier_path, &substituted_bytes).expect("substitute carrier identity");

    let error = kura
        .first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            entry.dataspace_id,
            incarnation,
        )
        .expect_err(
            "maintenance must authenticate the frontier before compaction or pair recovery",
        );
    assert_geometry_io_error(
        &error,
        ErrorKind::InvalidData,
        "lane retirement merge application frontier has no authenticated carrier",
    );
    assert_eq!(
        fs::read(&certified_data).expect("earlier certified recovery completed"),
        data_before,
    );
    assert_eq!(
        fs::read(&certified_index).expect("earlier certified index recovery completed"),
        index_before,
    );
    assert_eq!(
        fs::read(&receipt_temp).expect("later pair recovery has not started"),
        unpublished,
    );
    assert_eq!(
        fs::read(&receipt_index_temp).expect("later index recovery has not started"),
        unpublished_index,
    );
    assert_eq!(
        fs::read(&frontier_path).expect("rejected frontier remains unchanged"),
        substituted_bytes,
    );
    assert_eq!(
        fs::read(kura.lane_geometry_journal_path()).expect("unchanged geometry journal"),
        journal_before,
        "maintenance failure must not publish a geometry transition",
    );

    fs::write(&frontier_path, frontier_before).expect("restore the actual authenticated frontier");
    for _ in 0..2 {
        kura.first_release_lane_retirement_admissible_for_test(
            retiring_lane,
            entry.dataspace_id,
            incarnation,
        )
        .expect("valid maintenance retry and its fixed point must admit the same terminal work");
        assert!(!receipt_temp.exists());
        assert!(!receipt_index_temp.exists());
        assert_eq!(
            fs::read(kura.lane_geometry_journal_path()).expect("maintenance-only geometry journal"),
            journal_before,
        );
    }
}

#[test]
fn immutable_geometry_dataspace_change_is_an_exact_reference_replacement() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    let catalog_hash = kura
        .read_lane_geometry_journal()
        .unwrap()
        .configured_catalog_hash
        .unwrap();
    kura.establish_or_verify_configured_primary_geometry_anchor(
        initial.primary(),
        incarnations[&LaneId::SINGLE],
        catalog_hash,
    )
    .unwrap();
    let previous = kura
        .geometry_bindings(&initial, &incarnations, &activations)
        .unwrap();
    let mut identity = previous[0].identity();
    identity.dataspace_id = DataSpaceId::new(identity.dataspace_id.as_u64() + 1);
    let updated = vec![LaneGeometryBinding::from_identity(identity)];
    let operations = kura
        .build_geometry_operations(
            Hash::new(b"dataspace-reference-change"),
            &previous,
            &updated,
            &BTreeSet::new(),
        )
        .unwrap();
    assert_eq!(operations.len(), 1);
    assert_eq!(operations[0].kind, LaneGeometryOperationKind::Replace);
    assert_eq!(operations[0].previous.as_ref(), Some(&previous[0]));
    assert_eq!(operations[0].updated.as_ref(), Some(&updated[0]));
    assert_ne!(previous[0].blocks_path, updated[0].blocks_path);
    assert!(kura.binding_blocks_path(&previous[0]).exists());
    assert!(!kura.binding_blocks_path(&updated[0]).exists());
}

#[test]
fn startup_recovers_only_empty_instances_owned_by_exact_durable_intent() {
    for cut in 0..4 {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let operation = persist_create_intent(
            &kura,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let binding = operation.updated.as_ref().unwrap();
        let blocks = kura.binding_blocks_path(binding);
        let merge = kura.binding_merge_path(binding);
        if cut >= 1 {
            fs::create_dir_all(&blocks).unwrap();
        }
        if cut >= 2 {
            BlockStore::new(&blocks)
                .create_files_if_they_do_not_exist()
                .unwrap();
        }
        if cut >= 3 {
            kura.write_lane_marker(binding).unwrap();
        }
        let journal = fs::read(kura.lane_geometry_journal_path()).unwrap();
        drop(kura);
        let reopened =
            Kura::open_test_kura_with_configured_lane_config(&kura_config(&root), &initial)
                .expect("strict startup resumes admitted empty physical creation")
                .0;
        assert!(
            reopened.lane_storage_entries.lock().is_empty(),
            "physical repair grants no active producer"
        );
        reopened
            .require_exact_empty_journal_owned_pair_at(binding, &blocks, &merge)
            .unwrap();
        assert_eq!(
            fs::read(reopened.lane_geometry_journal_path()).unwrap(),
            journal,
            "physical recovery must not select or publish a catalog phase"
        );
    }
}

#[test]
fn startup_refuses_missing_completed_instance_even_without_auxiliary_namespace() {
    for phase in [
        LaneGeometryPhase::FilesApplied,
        LaneGeometryPhase::CatalogPublished,
        LaneGeometryPhase::RolledBack,
    ] {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        let operation = persist_create_intent(
            &kura,
            &initial,
            &extended,
            &initial_incarnations,
            &extended_incarnations,
            &initial_activations,
            &extended_activations,
        );
        let binding = operation.updated.as_ref().unwrap();
        kura.prepare_journal_owned_lane_instance(
            binding,
            GeometryEvidencePolicy::AllowJournalIntentProvisioning,
        )
        .unwrap();
        let mut journal = kura.read_lane_geometry_journal().unwrap();
        journal.records[0].phase = phase;
        kura.write_lane_geometry_journal(&journal).unwrap();
        let blocks = kura.binding_blocks_path(binding);
        let merge = kura.binding_merge_path(binding);
        fs::remove_dir_all(&blocks).unwrap();
        fs::remove_file(&merge).unwrap();
        let journal_bytes = fs::read(kura.lane_geometry_journal_path()).unwrap();
        drop(kura);
        Kura::open_test_kura_with_configured_lane_config(&kura_config(&root), &initial).expect_err(
            "a completed or rolled-back exact reference cannot manufacture missing storage",
        );
        assert!(!blocks.exists() && !merge.exists());
        assert_eq!(
            fs::read(root.join(JOURNAL_FILE_NAME)).unwrap(),
            journal_bytes
        );
    }
}

#[test]
fn journal_instance_recovery_refuses_occupied_and_unauthorized_targets_without_writes() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, extended) = initial_and_extended_configs();
    let (initial_incarnations, initial_activations) = initial_geometry();
    let (extended_incarnations, extended_activations) = extended_geometry();
    let kura = open_kura(&root, &initial);
    let operation = persist_create_intent(
        &kura,
        &initial,
        &extended,
        &initial_incarnations,
        &extended_incarnations,
        &initial_activations,
        &extended_activations,
    );
    let binding = operation.updated.as_ref().unwrap();
    let blocks = kura.binding_blocks_path(binding);
    let merge = kura.binding_merge_path(binding);
    let before = fs::read(kura.lane_geometry_journal_path()).unwrap();
    let previous = std::mem::replace(
        &mut *kura.provisional_snapshot_bootstrap.lock(),
        crate::kura::SnapshotBootstrapRuntimeState::Finalizing,
    );
    let refused = kura.recover_journal_owned_lane_instances_on_startup(
        crate::kura::StartupRecoveryMutationAuthority::Authenticated,
    );
    *kura.provisional_snapshot_bootstrap.lock() = previous;
    assert!(matches!(
        refused,
        Err(Error::SnapshotBootstrapAuthenticationPending)
    ));
    assert!(
        !blocks.exists() && !merge.exists(),
        "ordinary recovery cannot use provisional snapshot state"
    );
    fs::create_dir_all(&blocks).unwrap();
    let foreign = blocks.join("foreign-unowned-data");
    fs::write(&foreign, b"must remain unchanged").unwrap();
    kura.recover_journal_owned_lane_instances_on_startup(
        crate::kura::StartupRecoveryMutationAuthority::Authenticated,
    )
    .expect_err("intent cannot adopt occupied target");
    assert_eq!(fs::read(&foreign).unwrap(), b"must remain unchanged");
    assert!(!merge.exists());
    assert_eq!(fs::read(kura.lane_geometry_journal_path()).unwrap(), before);
}

#[test]
fn startup_rejects_nonempty_instance_scaffolding_without_repair() {
    // These files are temporary empty scaffolding, not a second canonical store.
    // Their removal remains a separate connected schema change.
    for role in 0..5 {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("kura");
        let (initial, _) = initial_and_extended_configs();
        let kura = open_kura(&root, &initial);
        authenticate_transition_fixture_primary(&kura, &initial, &initial_geometry().0);
        let entry = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
        let blocks = entry.blocks_dir(&root);
        let path = match role {
            0 => blocks.join(DATA_FILE_NAME),
            1 => blocks.join(INDEX_FILE_NAME),
            2 => blocks.join(HASHES_FILE_NAME),
            3 => entry.merge_log_path(&root),
            _ => blocks.join(COUNT_FILE_NAME),
        };
        let journal_path = kura.lane_geometry_journal_path();
        let journal = fs::read(&journal_path).unwrap();
        drop(kura);
        let bytes = b"foreign nonempty instance base";
        fs::write(&path, bytes).unwrap();
        assert!(
            Kura::open_test_kura_with_configured_lane_config(&kura_config(&root), &initial)
                .is_err()
        );
        assert_eq!(fs::read(&path).unwrap(), bytes);
        assert_eq!(fs::read(&journal_path).unwrap(), journal);
    }
}

#[test]
fn authoritative_reference_restore_never_recreates_missing_initial_or_dynamic_marker() {
    for dynamic in [false, true] {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("kura");
        let (initial, extended) = initial_and_extended_configs();
        let (initial_incarnations, initial_activations) = initial_geometry();
        let (extended_incarnations, extended_activations) = extended_geometry();
        let kura = open_kura(&root, &initial);
        authenticate_transition_fixture_primary(&kura, &initial, &initial_incarnations);
        let (catalog, incarnations, activations, lane_id) = if dynamic {
            // Structural storage transition, not a finality or WSV authority fixture.
            kura.apply_lane_geometry_transition_at_height(
                &initial,
                &extended,
                &initial_incarnations,
                &extended_incarnations,
                &initial_activations,
                &extended_activations,
                &BTreeSet::new(),
                9,
            )
            .unwrap();
            kura.mark_lane_geometry_catalog_published(
                &extended,
                &extended_incarnations,
                &extended_activations,
                None,
            )
            .unwrap();
            (
                &extended,
                &extended_incarnations,
                &extended_activations,
                LaneId::new(1),
            )
        } else {
            (
                &initial,
                &initial_incarnations,
                &initial_activations,
                LaneId::SINGLE,
            )
        };
        let entry = kura.lane_storage_entry(lane_id).unwrap();
        assert_eq!(entry.activation_height > 0, dynamic);
        let blocks = entry.blocks_dir(&root);
        let marker = blocks.join(MARKER_FILE_NAME);
        let marker_bytes = fs::read(&marker).unwrap();
        let namespace = Kura::lane_artifact_dir(&blocks);
        if namespace.exists() {
            fs::remove_dir(&namespace).unwrap();
        }
        fs::remove_file(&marker).unwrap();
        let before = native_observation_tree(&root);
        let active = kura.lane_storage_entries.lock().clone();
        let mut receipts = Vec::new();
        {
            let _geometry = kura.lane_geometry_lock.lock();
            let _sidecar = kura.sidecar_lock.lock();
            let error = kura
                .ensure_authoritative_lane_markers_with_receipts(
                    catalog,
                    incarnations,
                    activations,
                    Some(&mut receipts),
                )
                .expect_err("a restored reference cannot recreate its missing marker");
            assert_geometry_io_error(
                &error,
                ErrorKind::InvalidData,
                "authoritative lane storage has no incarnation marker",
            );
        }
        assert!(receipts.is_empty());
        assert!(!marker.exists() && !namespace.exists());
        assert_eq!(native_observation_tree(&root), before);
        assert_eq!(*kura.lane_storage_entries.lock(), active);

        // Returning the original bytes repairs the injected fault; only then may
        // the existing owner provision the optional empty auxiliary namespace.
        fs::write(&marker, &marker_bytes).unwrap();
        {
            let _geometry = kura.lane_geometry_lock.lock();
            let _sidecar = kura.sidecar_lock.lock();
            kura.ensure_authoritative_lane_markers(catalog, incarnations, activations)
                .unwrap();
        }
        assert!(namespace.is_dir());
        assert_eq!(fs::read(&marker).unwrap(), marker_bytes);
    }
}

#[test]
fn active_artifact_and_marker_access_rejects_complete_and_partial_collection_seals() {
    let temp = TempDir::new().unwrap();
    let root = temp.path().join("kura");
    let (initial, _) = initial_and_extended_configs();
    let (incarnations, activations) = initial_geometry();
    let kura = open_kura(&root, &initial);
    authenticate_transition_fixture_primary(&kura, &initial, &incarnations);
    let entry = kura.lane_storage_entry(LaneId::SINGLE).unwrap();
    let blocks = entry.blocks_dir(&root);
    let marker_path = blocks.join(MARKER_FILE_NAME);
    let original = kura.read_lane_marker(&marker_path).unwrap();
    let original_bytes = fs::read(&marker_path).unwrap();
    let namespace = Kura::lane_artifact_dir(&blocks);
    if namespace.exists() {
        fs::remove_dir(&namespace).unwrap();
    }
    let mut validators = (0..4)
        .map(|_| {
            let key = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
            PeerId::new(key.public_key().clone())
        })
        .collect::<Vec<_>>();
    validators.sort();
    let (proposal, ownership) = geometry_lane_proposal_and_ownership(
        entry.lane_id,
        entry.dataspace_id,
        entry.incarnation,
        1,
        0,
        1,
        0,
        Hash::new(b"marker-seal-route-control"),
        validators,
    );
    {
        let _geometry = kura.lane_geometry_lock.lock();
        assert_eq!(
            kura.active_lane_incarnation_marker(&entry).unwrap(),
            (entry.incarnation, 0)
        );
        kura.require_active_lane_artifact(&entry, &proposal.descriptor)
            .unwrap();
        kura.require_active_lane_ownership_artifact(&entry, &ownership)
            .unwrap();
    }
    for (blocks_sealed, merge_sealed) in [(true, false), (false, true), (true, true)] {
        let mut marker = original.clone();
        marker.move_target_blocks = blocks_sealed.then(|| "retired/collection/blocks".to_owned());
        marker.move_target_merge = merge_sealed.then(|| "retired/collection/merge.log".to_owned());
        fs::write(&marker_path, marker.encode()).unwrap();
        let before = native_observation_tree(&root);
        let mut receipts = Vec::new();
        {
            let _geometry = kura.lane_geometry_lock.lock();
            let _sidecar = kura.sidecar_lock.lock();
            assert!(kura.active_lane_incarnation_marker(&entry).is_err());
            assert!(
                kura.require_active_lane_incarnation(&entry, entry.incarnation, 1)
                    .is_err()
            );
            assert!(
                kura.require_active_lane_artifact(&entry, &proposal.descriptor)
                    .is_err()
            );
            assert!(
                kura.require_active_lane_ownership_artifact(&entry, &ownership)
                    .is_err()
            );
            assert!(
                kura.ensure_authoritative_lane_markers_with_receipts(
                    &initial,
                    &incarnations,
                    &activations,
                    Some(&mut receipts),
                )
                .is_err()
            );
        }
        assert!(receipts.is_empty() && !namespace.exists());
        assert_eq!(
            native_observation_tree(&root),
            before,
            "a collection seal cannot be adopted, rewritten, or used to create a namespace"
        );
    }
    fs::write(&marker_path, &original_bytes).unwrap();
    let _geometry = kura.lane_geometry_lock.lock();
    kura.require_active_lane_artifact(&entry, &proposal.descriptor)
        .unwrap();
    assert_eq!(
        kura.active_lane_incarnation_marker(&entry).unwrap(),
        (entry.incarnation, 0)
    );
    assert_eq!(fs::read(&marker_path).unwrap(), original_bytes);
}
