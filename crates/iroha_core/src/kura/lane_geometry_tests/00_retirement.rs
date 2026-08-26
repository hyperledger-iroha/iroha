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
        LANE_RETIREMENT_FIXED_ARTIFACT_FILES_PER_ROUTE, 13,
        "five data/index pairs plus three independent frontier/index files are fixed per route"
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
    let artifact_dir = Kura::lane_artifact_dir(&configured.primary().blocks_dir(kura.store_root()));
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
    Kura::open_test_kura_with_configured_lane_config(&config, lane_config)
        .expect("open test Kura")
        .0
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
    KuraConfig { init_mode: iroha_config::kura::InitMode::Strict, store_dir: WithOrigin::inline(root.to_path_buf()),
        max_disk_usage_bytes: MAX_DISK_USAGE_BYTES,
        blocks_in_memory: BLOCKS_IN_MEMORY,
        debug_output_new_blocks: false,
        merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
        fsync_mode: FsyncMode::Always,
        fsync_interval: FSYNC_INTERVAL,
        lane_history_retention: LANE_HISTORY_RETENTION,
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
fn assert_lane_paths_absent(root: &Path, lane_config: &RuntimeLaneConfig) {
    let primary = lane_config.primary();
    assert!(
        !primary.blocks_dir(root).exists(),
        "rejected startup must not create its block-store path"
    );
    assert!(
        !primary.merge_log_path(root).exists(),
        "rejected startup must not create its merge-ledger path"
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
    let mut journal = LaneGeometryJournal::default();
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
        &added_incarnations,
        &added_activations,
        &added_activations,
        &BTreeSet::new(),
        7,
    )
    .expect("apply second height-seven transition");
    kura.mark_lane_geometry_catalog_published(
        &relabelled,
        &added_incarnations,
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
        &added_incarnations,
        &added_activations,
        &added_activations,
        &BTreeSet::new(),
        7,
    )
    .expect("retry second transition in sequence");
    kura.mark_lane_geometry_catalog_published(
        &relabelled,
        &added_incarnations,
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
    lane_config: &RuntimeLaneConfig,
    baseline: Hash,
    primary_incarnation: Hash,
) -> Arc<Kura> {
    Kura::establish_or_verify_configured_lane_catalog_baseline(root, baseline)
        .expect("establish configured baseline before opening lane storage");
    let kura = open_kura(root, lane_config);
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
    let kura = open_configured_anchor_for_publication_test(
        &root,
        &config,
        baseline,
        incarnations[&LaneId::SINGLE],
    );
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
        &config,
        baseline,
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
        &config,
        baseline,
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
    let baseline = Hash::new(b"configured-catalog-baseline");
    let kura = open_configured_anchor_for_publication_test(
        &root,
        &initial,
        baseline,
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
    let baseline = Hash::new(b"configured-catalog-baseline");
    let kura = open_configured_anchor_for_publication_test(
        &root,
        &initial,
        baseline,
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
    for binding in kura
        .geometry_bindings(config, incarnations, activation_heights)
        .expect("retirement test geometry bindings")
    {
        kura.write_lane_marker(&binding)
            .expect("install authoritative retirement-test lane marker");
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
    assert_eq!(source.kind(), expected_kind);
    assert_eq!(source.to_string(), expected_message);
}
struct RetiredGeometryFixture {
    initial: RuntimeLaneConfig,
    extended: RuntimeLaneConfig,
    initial_incarnations: BTreeMap<LaneId, Hash>,
    initial_activations: BTreeMap<LaneId, u64>,
    extended_incarnations: BTreeMap<LaneId, Hash>,
    extended_activations: BTreeMap<LaneId, u64>,
    archive_root: PathBuf,
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
    let lane_one_blocks = extended
        .entry(LaneId::new(1))
        .expect("elastic lane")
        .blocks_dir(root);
    fs::write(
        lane_one_blocks.join("gc-payload.norito"),
        [0xA5; GC_PAYLOAD_LEN],
    )
    .expect("seed archived payload bytes");
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
    let journal = kura.read_lane_geometry_journal().expect("geometry journal");
    let retired = journal.records.last().expect("retire transition");
    let archive_root = root
        .join("retired/lane_geometry")
        .join(hex::encode(retired.transition_id.as_ref()));
    assert!(archive_root.exists(), "retired lane archive exists");
    RetiredGeometryFixture {
        initial,
        extended,
        initial_incarnations,
        initial_activations,
        extended_incarnations,
        extended_activations,
        archive_root,
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
    kura.checkpoint_lane_geometry_with_proven_snapshot(
        bindings,
        lineage_root,
        height,
        Some(block_hash),
        state_hash,
        Vec::new(),
    )
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
    let retiring_entry = extended.entry(retiring_lane).expect("retiring lane");
    let retiring_incarnation = extended_incarnations[&retiring_lane];
    let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (network_id, epoch, payload) = autonomous_retirement_payload_for_routes(
        retiring_lane,
        retiring_entry.dataspace_id,
        retiring_incarnation,
        LaneId::new(9),
        DataSpaceId::new(19),
        Hash::new(b"tombstoned-autonomous-unrelated-participant"),
        &producer,
    );
    let lane_block_height = payload.origin_proposal.descriptor.lane_block_height;
    kura.persist_lane_executable_payload(&payload, network_id, epoch)
        .expect("persist autonomous payload before terminal retirement");
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
        "pending autonomous payload targets a retiring lane incarnation",
    );
    let retirement = crate::kura::AutonomousLaneSlotRetirementV1::from_payload(&payload);
    kura.persist_autonomous_lane_slot_retirement(&retirement, network_id, epoch)
        .expect("persist exact autonomous slot retirement");
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
    let archive_root = root
        .join("retired/lane_geometry")
        .join(hex::encode(retirement_transition.transition_id.as_ref()));
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
    let archived_blocks = archive_root.join("lane_0000000001/previous_blocks");
    let lane_artifacts = archived_blocks.join(LANE_ARTIFACTS_DIR_NAME);
    let fixture = TombstonedAutonomousArchiveFixture {
        geometry: RetiredGeometryFixture {
            initial,
            extended,
            initial_incarnations,
            initial_activations,
            extended_incarnations,
            extended_activations,
            archive_root,
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
    let context = HeightContext {
        network_id: crate::sumeragi::synthetic_network_id("native-amx-lane-archive-test"),
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
    let retiring_entry = extended
        .entry(retiring_lane)
        .expect("Native archive participant lane")
        .clone();
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
    let block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
    let block = Arc::new(block);
    kura.store_block(Arc::clone(&block))
        .expect("persist Native archive application block");
    let source_id = [0xA7; Hash::LENGTH];
    let result = TransactionResult::new(TransactionResultInner::Ok(DataTriggerSequence::new()));
    let entrypoint_hash = HashOf::<TransactionEntrypoint>::from_untyped_unchecked(
        proposal.descriptor.accepted_transaction_hashes[0],
    );
    let settlement = LaneBlockCommitment {
        block_height: proposal.descriptor.lane_block_height,
        lane_id: proposal.descriptor.lane_id,
        lane_incarnation: proposal.descriptor.lane_incarnation,
        dataspace_id: proposal.descriptor.dataspace_id,
        tx_count: 1,
        total_local_amount: "0".parse().expect("zero quantity"),
        total_xor_due: "0".parse().expect("zero quantity"),
        total_xor_after_haircut: "0".parse().expect("zero quantity"),
        total_xor_variance: "0".parse().expect("zero quantity"),
        swap_metadata: None,
        receipts: vec![iroha_data_model::block::consensus::LaneSettlementReceipt {
            source_id,
            local_amount: "0".parse().expect("zero quantity"),
            xor_due: "0".parse().expect("zero quantity"),
            xor_after_haircut: "0".parse().expect("zero quantity"),
            xor_variance: "0".parse().expect("zero quantity"),
            timestamp_ms: 1,
        }],
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let settlement_hash = iroha_data_model::nexus::compute_settlement_hash(&settlement)
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
        kura.native_amx_participant_application_drain_evidence(&receipt)
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
    let archive_root = root
        .join("retired/lane_geometry")
        .join(hex::encode(retirement_transition.transition_id.as_ref()));
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
    let archived_blocks = archive_root.join("lane_0000000001/previous_blocks");
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
                archive_root,
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
        let block: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
            .chain(0, previous.as_deref())
            .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
            .unpack(|_| {})
            .into();
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
    let producer = crate::kura::checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let transaction = TransactionBuilder::new(
        crate::sumeragi::synthetic_network_id("geometry-durability-merge"),
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "geometry durability merge execution".to_owned(),
    )])
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
        &producer,
    );
    let certified = certified_geometry_lane_block_for_proposal(proposal.clone(), &producer);
    kura.write_certified_lane_block_artifact(&certified)
        .expect("persist merge-applied retirement certificate");
    let genesis: SignedBlock = BlockBuilder::new(Vec::<AcceptedTransaction<'static>>::new())
        .chain(0, None)
        .sign(SAMPLE_GENESIS_ACCOUNT_KEYPAIR.private_key())
        .unpack(|_| {})
        .into();
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
    let source_bundle = certified
        .encode_framed()
        .expect("encode merge-applied retirement source bundle");
    let execution = MergeLaneExecution {
        source_bundle_hash: Hash::new(&source_bundle),
        source_bundle,
        proposal: proposal.clone(),
        origin_proposal: proposal,
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
        autonomous_network_id: crate::sumeragi::synthetic_network_id(
            "geometry-durability-merge-genesis",
        ),
        autonomous_epoch: 1,
        autonomous_payload_hash: Hash::new(b"geometry-durability-merge-payload"),
        entrypoint_hashes: vec![Hash::from(entrypoint_hash)],
        entrypoints: vec![entrypoint],
        reservation_keys: vec![vec![1]],
        routing_plans: vec![vec![2]],
        native_amx_receipts: vec![None],
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
    let entry = MergeLedgerEntry {
        version: MergeLedgerEntry::VERSION,
        epoch_id: 1,
        lane_catalog_hash: Hash::new(b"geometry-durability-merge-catalog"),
        active_lanes,
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
    carrier.set_execution_context(Some(
        BlockExecutionContextBundle::new(Vec::new())
            .with_merge_entry(CertifiedMergeLedgerReference::new(&entry)),
    ));
    carrier
        .set_transaction_results(Vec::new(), &[], Vec::new())
        .expect("attach empty deterministic merge carrier result");
    let carrier = Arc::new(carrier);
    assert_eq!(
        entry.merge_qc.view,
        carrier.header().view_change_index(),
        "merge fixture QC and carrier use the same view"
    );
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &entry)
        .expect("store merge-applied retirement carrier");
    let _ = crate::kura::tests::persist_v2_finality_chain_through(kura, nonzero!(2_usize));
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
    keypair: &KeyPair,
) -> (LaneBlockProposalV1, SumeragiLanePayloadOwnership) {
    let validator_set = vec![PeerId::new(keypair.public_key().clone())];
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
        lane_block_descriptor_validator_count: 1,
        lane_block_descriptor_min_quorum: 1,
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
        validator_count: 1,
        min_quorum: 1,
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
        participant_keypair,
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
        .computed_grouped_participant_settlement_commitment(&[prepare_body.source_id])
        .expect("single-source test fixture settlement is valid");
    let participant_settlement = prepare_body
        .computed_grouped_participant_settlement(&[prepare_body.source_id])
        .expect("single-source test fixture settlement is valid");
    let participant_settlement_hash =
        iroha_data_model::nexus::compute_settlement_hash(&participant_settlement)
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
    let network_id = crate::sumeragi::synthetic_network_id("geometry-retirement-autonomous");
    let transaction = TransactionBuilder::new(
        network_id,
        (*SAMPLE_GENESIS_ACCOUNT_ID).clone(),
        iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
    )
    .with_instructions([Log::new(
        Level::INFO,
        "geometry retirement payload".to_owned(),
    )])
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
        producer,
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
