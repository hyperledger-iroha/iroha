struct MergeReceiptCompactionFixture {
    temp_dir: TempDir,
    lane_entry: LaneConfigEntry,
    kura: Arc<Kura>,
    descriptor: LaneBlockDescriptorV1,
    frontier: LaneMergeApplicationFrontierV1,
}
fn merge_receipt_compaction_fixture() -> MergeReceiptCompactionFixture {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let lane_entry = lane_config.primary().clone();
    let (kura, _) = Kura::new(&config, &lane_config).expect("initialize Kura");
    let entrypoint = offline_top_up_entrypoint_for_index([0xC1; 32], [0xC2; 32]);
    let mut merge_entry = merge_entry_with_indexed_entrypoint(entrypoint);
    let execution = merge_entry
        .execution_batch
        .as_ref()
        .and_then(|batch| batch.lanes.first())
        .expect("merge execution fixture");
    let descriptor = execution.proposal.descriptor.clone();
    kura.install_lane_incarnation_marker_for_test(&lane_entry, descriptor.lane_incarnation, 0)
        .expect("install merge receipt lane marker");
    let mut blocks = DummyBlocks::new();
    let parent = blocks.next();
    let raw_carrier = blocks.next();
    let batch = merge_entry
        .execution_batch
        .as_mut()
        .expect("merge receipt fixture has an execution batch");
    batch.application_block_header =
        crate::merge::merge_application_header_from_carrier(&raw_carrier.header());
    batch.batch_hash = crate::merge::merge_execution_batch_hash(batch);
    let mut executed_carrier = raw_carrier.as_ref().clone();
    attach_ok_results_to_block(&mut executed_carrier);
    let carrier = bind_merge_entry_to_carrier(Arc::new(executed_carrier), &mut merge_entry);
    assert!(
        carrier.has_results(),
        "a canonical merge receipt carrier must contain execution results"
    );
    assert_eq!(
        carrier.results().count(),
        carrier.external_entrypoints_cloned().count(),
        "the merge receipt carrier must contain one result per ordinary entrypoint"
    );
    assert_eq!(
        merge_entry
            .execution_batch
            .as_ref()
            .expect("merge receipt fixture has an execution batch")
            .application_block_header,
        crate::merge::merge_application_header_from_carrier(&carrier.header()),
        "the merge receipt batch must bind the canonical stripped carrier header"
    );
    let carrier_height = carrier.header().height().get();
    let carrier_hash = carrier.hash();
    kura.store_block(parent)
        .expect("store merge carrier parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store committed merge carrier");
    let _ = persist_v2_finality_chain_through(
        &kura,
        NonZeroUsize::new(usize::try_from(carrier_height).expect("carrier height fits usize"))
            .expect("carrier height is non-zero"),
    );
    kura.persist_merge_lane_block_application_receipts(&merge_entry, carrier_height, carrier_hash)
        .expect("persist marker-bound merge receipt");
    assert_eq!(
        kura.read_lane_block_application_receipt(descriptor.lane_id, descriptor.lane_block_height,)
            .expect("read merge receipt")
            .format,
        LaneBlockApplicationReceiptArtifactFormat::MergeExecution,
    );
    let frontier_path =
        Kura::lane_merge_application_frontier_path_for_entry(&lane_entry, &kura.store_root());
    let frontier = kura
        .decode_lane_merge_application_frontier(&lane_entry, &frontier_path)
        .expect("decode merge application frontier")
        .expect("merge receipt must publish its terminal frontier");
    assert_eq!(frontier.lane_block_height, descriptor.lane_block_height);
    assert!(
        kura.lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
            &frontier,
        )
        .is_some(),
        "the compact cursor must revalidate against the exact merge entry and carrier"
    );
    MergeReceiptCompactionFixture {
        temp_dir,
        lane_entry,
        kura,
        descriptor,
        frontier,
    }
}
fn compact_fixture_lane_histories(
    kura: &Kura,
    lane_entry: &LaneConfigEntry,
    frontier: &LaneMergeApplicationFrontierV1,
) -> Result<LaneHistoryCompactionOutcome> {
    let _prune_guard = kura.prune_lock.lock();
    kura.ensure_prune_recovery_not_required()?;
    let _canonical_chain_guard = kura.canonical_chain_lock.lock();
    let pending_canonical_bytes =
        kura.pending_canonical_capacity_bytes_under_prune_and_canonical_guards()?;
    let _geometry_guard = kura.lane_geometry_lock.lock();
    let _sidecar_guard = kura.sidecar_lock.lock();
    kura.compact_lane_histories_through_merge_frontier_locked(
        pending_canonical_bytes,
        lane_entry,
        frontier,
    )
}
fn ensure_merge_receipt_lane_artifact_pair(
    fixture: &MergeReceiptCompactionFixture,
) -> (PathBuf, PathBuf) {
    let (data_path, index_path) =
        Kura::lane_artifact_paths_for_entry(&fixture.lane_entry, fixture.temp_dir.path());
    match (data_path.is_file(), index_path.is_file()) {
        (true, true) => {}
        (false, false) => {
            let receipt = fixture
                .kura
                .read_lane_block_application_receipt(
                    fixture.descriptor.lane_id,
                    fixture.descriptor.lane_block_height,
                )
                .expect("merge receipt fixture has its application receipt");
            let payload = receipt
                .artifact
                .encode_framed()
                .expect("encode merge receipt lane artifact");
            assert!(
                Kura::append_indexed_sidecar(
                    &data_path,
                    &index_path,
                    fixture.descriptor.lane_block_height,
                    &payload,
                    LaneBlockArtifact::FORMAT_LABEL,
                    FsyncMode::Always,
                    None,
                    SidecarIndexOrigin::FirstWrite,
                ),
                "install merge receipt lane artifact history",
            );
        }
        _ => panic!("merge receipt fixture lane artifact pair is only partially present"),
    }
    (data_path, index_path)
}
fn assert_terminal_frontier_recovery_error(error: Error, data_path: &Path) {
    match error {
        Error::IO(source, path) => {
            assert_eq!(source.kind(), ErrorKind::InvalidData);
            assert!(
                source
                    .to_string()
                    .contains("lane.block_artifact terminal-frontier recovery failed"),
                "unexpected recovery error: {source}",
            );
            assert_eq!(path, data_path);
        }
        other => panic!("unexpected malformed compaction error: {other:?}"),
    }
}
#[test]
fn lane_history_compaction_recovers_crash_temp_before_tight_capacity_refusal() {
    let mut fixture = merge_receipt_compaction_fixture();
    let (data_path, index_path) = ensure_merge_receipt_lane_artifact_pair(&fixture);
    let stable_data = std::fs::read(&data_path).expect("read stable lane artifact data");
    let stable_index = std::fs::read(&index_path).expect("read stable lane artifact index");
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    std::fs::write(&temp_data_path, &stable_data).expect("stage crash-temp lane artifact data");
    std::fs::write(&temp_index_path, &stable_index).expect("stage crash-temp lane artifact index");
    let staged_temp_bytes = u64::try_from(stable_data.len())
        .expect("lane artifact data length fits u64")
        .checked_add(
            u64::try_from(stable_index.len()).expect("lane artifact index length fits u64"),
        )
        .expect("staged lane artifact temp bytes fit u64");
    let enforced_with_temps = fixture
        .kura
        .refresh_disk_usage_bytes()
        .expect("refresh usage with staged compaction temps");
    let total_with_temps = fixture.kura.disk_usage_total.load(Ordering::Relaxed);
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive Kura before tight-cap compaction recovery")
        .max_disk_usage_bytes = 1;
    let outcome =
        compact_fixture_lane_histories(&fixture.kura, &fixture.lane_entry, &fixture.frontier)
            .expect("valid crash temp must recover before optional capacity refusal");
    assert_eq!(outcome, LaneHistoryCompactionOutcome::CapacityBlocked);
    assert!(!temp_data_path.exists(), "recovery must promote temp data");
    assert!(
        !temp_index_path.exists(),
        "recovery must promote temp index"
    );
    assert_eq!(
        std::fs::read(&data_path).expect("read recovered lane artifact data"),
        stable_data,
    );
    assert_eq!(
        std::fs::read(&index_path).expect("read recovered lane artifact index"),
        stable_index,
    );
    let enforced_after_recovery = enforced_with_temps
        .checked_sub(staged_temp_bytes)
        .expect("recovery removes exactly the staged enforced bytes");
    let total_after_recovery = total_with_temps
        .checked_sub(staged_temp_bytes)
        .expect("recovery removes exactly the staged total bytes");
    assert_eq!(
        fixture.kura.disk_usage.load(Ordering::Relaxed),
        enforced_after_recovery,
        "recovery must publish its enforced-usage delta before CapacityBlocked",
    );
    assert_eq!(
        fixture.kura.disk_usage_total.load(Ordering::Relaxed),
        total_after_recovery,
        "recovery must publish its total-usage delta before CapacityBlocked",
    );
    assert_eq!(
        fixture
            .kura
            .kura_disk_usage_bytes()
            .expect("scan enforced bytes after recovery"),
        enforced_after_recovery,
    );
    assert_eq!(
        fixture
            .kura
            .kura_total_disk_usage_bytes()
            .expect("scan total bytes after recovery"),
        total_after_recovery,
    );
    let recovered_history = snapshot_regular_files_recursively(fixture.temp_dir.path());
    fixture
        .kura
        .repair_lane_merge_application_frontiers_on_startup()
        .expect("tight-cap startup must retain recovered uncompacted history");
    fixture
        .kura
        .first_release_lane_retirement_admissible_for_test(
            fixture.descriptor.lane_id,
            fixture.descriptor.dataspace_id,
            fixture.descriptor.lane_incarnation,
        )
        .expect("tight-cap retirement must not be stranded by recovered temp files");
    assert_eq!(
        snapshot_regular_files_recursively(fixture.temp_dir.path()),
        recovered_history,
        "startup and retirement must retain recovered history when compaction is capacity-blocked",
    );
}
#[test]
fn lane_history_compaction_rejects_data_only_temp_before_capacity_refusal() {
    let mut fixture = merge_receipt_compaction_fixture();
    let (data_path, index_path) = ensure_merge_receipt_lane_artifact_pair(&fixture);
    let stable_data = std::fs::read(&data_path).expect("read stable lane artifact data");
    let stable_index = std::fs::read(&index_path).expect("read stable lane artifact index");
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    std::fs::write(&temp_data_path, &stable_data)
        .expect("stage malformed data-only compaction temp");
    assert!(!temp_index_path.exists());
    fixture
        .kura
        .refresh_disk_usage_bytes()
        .expect("refresh usage with malformed compaction temp");
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive Kura before malformed tight-cap recovery")
        .max_disk_usage_bytes = 1;
    let error =
        compact_fixture_lane_histories(&fixture.kura, &fixture.lane_entry, &fixture.frontier)
            .expect_err("data-only crash residue must fail before CapacityBlocked");
    assert_terminal_frontier_recovery_error(error, &data_path);
    assert!(temp_data_path.is_file(), "malformed evidence is retained");
    assert!(!temp_index_path.exists());
    assert_eq!(
        std::fs::read(&data_path).expect("read unchanged lane artifact data"),
        stable_data,
    );
    assert_eq!(
        std::fs::read(&index_path).expect("read unchanged lane artifact index"),
        stable_index,
    );
    assert!(
        fixture
            .kura
            .repair_lane_merge_application_frontiers_on_startup()
            .is_err(),
        "startup must not downgrade data-only rewrite residue to CapacityBlocked",
    );
}
#[test]
fn lane_history_compaction_rejects_corrupt_temp_index_before_capacity_refusal() {
    let mut fixture = merge_receipt_compaction_fixture();
    let (data_path, index_path) = ensure_merge_receipt_lane_artifact_pair(&fixture);
    let stable_data = std::fs::read(&data_path).expect("read stable lane artifact data");
    let stable_index = std::fs::read(&index_path).expect("read stable lane artifact index");
    let temp_data_path = data_path.with_extension("norito.tmp");
    let temp_index_path = index_path.with_extension("index.tmp");
    std::fs::write(&temp_data_path, &stable_data).expect("stage compaction temp data");
    std::fs::write(&temp_index_path, b"malformed temp index")
        .expect("stage corrupt compaction temp index");
    fixture
        .kura
        .refresh_disk_usage_bytes()
        .expect("refresh usage with corrupt compaction temp");
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive Kura before corrupt tight-cap recovery")
        .max_disk_usage_bytes = 1;
    let error =
        compact_fixture_lane_histories(&fixture.kura, &fixture.lane_entry, &fixture.frontier)
            .expect_err("corrupt temp index must fail before CapacityBlocked");
    assert_terminal_frontier_recovery_error(error, &data_path);
    assert!(temp_data_path.is_file(), "corrupt temp data is retained");
    assert!(temp_index_path.is_file(), "corrupt temp index is retained");
    assert_eq!(
        std::fs::read(&data_path).expect("read unchanged lane artifact data"),
        stable_data,
    );
    assert_eq!(
        std::fs::read(&index_path).expect("read unchanged lane artifact index"),
        stable_index,
    );
}
