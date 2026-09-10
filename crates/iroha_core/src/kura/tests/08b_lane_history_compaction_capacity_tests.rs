struct MergeReceiptCompactionFixture {
    temp_dir: TempDir,
    lane_entry: LaneConfigEntry,
    kura: Arc<Kura>,
    descriptor: LaneBlockDescriptorV1,
    lane_artifact: LaneBlockArtifact,
    frontier: LaneMergeApplicationFrontierV1,
}
fn merge_receipt_compaction_lane_artifact(
    descriptor: &LaneBlockDescriptorV1,
    proposal_block_hash: HashOf<BlockHeader>,
    proposal_view: u64,
) -> LaneBlockArtifact {
    LaneBlockArtifact::new(
        proposal_block_hash,
        SumeragiLanePayloadOwnership {
            proposal_height: descriptor.proposal_height,
            proposal_view,
            lane_id: descriptor.lane_id,
            dataspace_id: descriptor.dataspace_id,
            lane_incarnation: descriptor.lane_incarnation,
            lane_block_height: descriptor.lane_block_height,
            lane_block_view: descriptor.lane_block_view,
            subject_hash: descriptor.subject_hash,
            qc_mode_tag: descriptor.qc_mode_tag.clone(),
            accepted_candidate_indices: descriptor.accepted_candidate_indices.clone(),
            accepted_transaction_hashes: descriptor.accepted_transaction_hashes.clone(),
            previous_lane_block_height: descriptor.previous_lane_block_height,
            previous_lane_block_descriptor_hash: descriptor.previous_lane_block_descriptor_hash,
            lane_block_descriptor_hash: Some(descriptor.descriptor_hash),
            lane_block_descriptor_validator_set: descriptor.validator_set.clone(),
            lane_block_descriptor_validator_count: descriptor.validator_count,
            lane_block_descriptor_min_quorum: descriptor.min_quorum,
            payload_ownership_hash: descriptor.payload_ownership_hash,
            rbc_instance_hash: descriptor.rbc_instance_hash,
        },
    )
}
fn merge_receipt_compaction_fixture() -> MergeReceiptCompactionFixture {
    let temp_dir = TempDir::new().expect("create temp dir");
    let config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    let lane_config = RuntimeLaneConfig::default();
    let lane_entry = lane_config.primary().clone();
    let (kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("initialize Kura");
    let entrypoint = indexed_log_entrypoint([0xC1; 32], [0xC2; 32]);
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
    let lane_artifact = merge_receipt_compaction_lane_artifact(
        &descriptor,
        carrier_hash,
        carrier.header().view_change_index(),
    );
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
        lane_artifact,
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
        Kura::lane_artifact_paths_for_entry(&fixture.lane_entry, &fixture.kura.store_root());
    match (data_path.is_file(), index_path.is_file()) {
        (true, true) => {}
        (false, false) => {
            let payload = fixture
                .lane_artifact
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

struct AutonomousHistoryCompactionFixture {
    temp_dir: TempDir,
    config: KuraConfig,
    lane_config: RuntimeLaneConfig,
    kura: Arc<Kura>,
    payloads: Vec<LaneExecutablePayloadV1>,
    sources: Vec<DurableAutonomousLaneMergeSource>,
    frontier: LaneMergeApplicationFrontierV1,
}

fn linked_compaction_payload(
    lane: &LaneConfigEntry,
    height: u64,
    predecessor: Option<Hash>,
    signer: &KeyPair,
) -> LaneExecutablePayloadV1 {
    let template =
        autonomous_capacity_payload_at(lane.lane_id, lane.dataspace_id, height, height, signer);
    let mut proposal = template.origin_proposal.clone();
    proposal.descriptor.previous_lane_block_descriptor_hash = predecessor;
    proposal.descriptor.descriptor_hash = proposal.descriptor.computed_descriptor_hash();
    proposal.proposal_hash = proposal.computed_proposal_hash();
    let mut keys = template.reservation_keys.clone();
    for key in &mut keys {
        key.proposal_identity_hash = proposal.proposal_hash;
        key.reservation_owner_hash = Hash::new_from_chunks(&[
            b"kura:compaction:reservation-owner",
            proposal.proposal_hash.as_ref(),
        ]);
    }
    LaneExecutablePayloadV1::new_signed_with_reservations(
        template.network_id,
        template.epoch,
        proposal,
        template.entrypoints.clone(),
        keys,
        template.routing_plans.clone(),
        vec![None; template.entrypoints.len()],
        PeerId::new(signer.public_key().clone()),
        signer.private_key(),
    )
    .expect("sign exact linked compaction payload")
}

fn autonomous_history_compaction_fixture(observer: bool) -> AutonomousHistoryCompactionFixture {
    autonomous_history_compaction_fixture_with_observer_append(observer, false)
}

fn autonomous_history_compaction_fixture_with_observer_append(
    observer: bool,
    append_pending: bool,
) -> AutonomousHistoryCompactionFixture {
    let temp_dir = TempDir::new().expect("autonomous history compaction root");
    let mut config = kura_config_for_dir(&temp_dir, BLOCKS_IN_MEMORY);
    config.lane_history_retention = NonZeroUsize::MIN;
    let lane_config = RuntimeLaneConfig::default();
    let lane = lane_config.primary();
    let signer = checked_keypair_with_algorithm(Algorithm::BlsNormal);
    let (mut kura, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("autonomous compaction producer");
    let mut payloads = Vec::new();
    let mut sources = Vec::new();
    let mut predecessor = None;
    for height in 1..=3 {
        let payload = linked_compaction_payload(lane, height, predecessor, &signer);
        predecessor = Some(payload.origin_proposal.descriptor.descriptor_hash);
        let prepared = prepare_autonomous_certification_for_capacity_payload(
            &kura,
            &lane_config,
            &payload,
            &signer,
        );
        if append_pending && !observer && height == 3 {
            fail_next_autonomous_merge_bundle_append_data_sync_for_tests();
            kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
                .expect_err("latest retained bundle append stops before index publication");
        } else {
            kura.persist_committed_lane_block_session(&prepared.session, &prepared.signer_pops)
                .expect("publish complete signed compaction source");
        }
        payloads.push(payload);
        sources.push(prepared.source);
    }
    let execution = canonical_terminal_merge_execution_from_durable_source_for_test(
        &payloads[1],
        sources[1].clone(),
    );
    let (parent, carrier, merge_entry) = canonical_terminal_merge_carrier_for_test(execution, 1);
    // An observer can apply a canonical carrier without having certified its
    // source locally. Keep the producer's authenticated source in the carrier,
    // while the observer has only the earlier local singleton certificate.
    let producer_dir = if observer {
        let observer_dir = TempDir::new().expect("compaction observer root");
        let mut observer_config = kura_config_for_dir(&observer_dir, BLOCKS_IN_MEMORY);
        observer_config.lane_history_retention = NonZeroUsize::MIN;
        let (observer_kura, _) =
            Kura::open_test_kura_with_configured_lane_config(&observer_config, &lane_config)
                .expect("open compaction observer");
        let local = prepare_autonomous_certification_for_capacity_payload(
            &observer_kura,
            &lane_config,
            &payloads[0],
            &signer,
        );
        if append_pending {
            fail_next_autonomous_merge_bundle_append_data_sync_for_tests();
            observer_kura
                .persist_committed_lane_block_session(&local.session, &local.signer_pops)
                .expect_err("observer crashes while appending its old local bundle");
            let (_, index) =
                Kura::autonomous_lane_merge_bundle_paths_for_entry(lane, &observer_kura.store_root);
            assert!(Kura::bound_progress_append_intent_path(&index).is_file());
        } else {
            observer_kura
                .persist_committed_lane_block_session(&local.session, &local.signer_pops)
                .expect("observer certifies only the predecessor");
        }
        kura = observer_kura;
        config = observer_config;
        Some(observer_dir)
    } else {
        None
    };
    kura.store_block(parent)
        .expect("store canonical compaction parent");
    kura.store_block_with_merge_entry(Arc::clone(&carrier), &merge_entry)
        .expect("store exact canonical compaction carrier");
    let _ = persist_v2_finality_chain_through(&kura, NonZeroUsize::new(2).expect("carrier height"));
    kura.persist_merge_lane_block_application_receipts(
        &merge_entry,
        carrier.header().height().get(),
        carrier.hash(),
    )
    .expect("publish authenticated application frontier after source slot two");
    let path = Kura::lane_merge_application_frontier_path_for_entry(lane, &kura.store_root);
    let frontier = kura
        .decode_lane_merge_application_frontier(lane, &path)
        .expect("decode compaction authority")
        .expect("applied frontier exists");
    assert_eq!(frontier.lane_block_height, 2);
    assert!(
        kura.lane_merge_application_frontier_expected_receipt_under_prune_and_canonical_guards(
            &frontier,
        )
        .is_some(),
        "retention authority must authenticate against the exact carrier and receipt"
    );
    AutonomousHistoryCompactionFixture {
        temp_dir: producer_dir.unwrap_or(temp_dir),
        config,
        lane_config,
        kura,
        payloads,
        sources,
        frontier,
    }
}

fn compaction_source_pairs(
    fixture: &AutonomousHistoryCompactionFixture,
) -> [((PathBuf, PathBuf), &'static str); 3] {
    let lane = fixture.lane_config.primary();
    [
        (
            Kura::lane_block_execution_input_paths_for_entry(lane, &fixture.kura.store_root),
            LaneBlockExecutionInputArtifact::FORMAT_LABEL,
        ),
        (
            Kura::certified_lane_block_paths_for_entry(lane, &fixture.kura.store_root),
            CertifiedLaneBlockArtifact::FORMAT_LABEL,
        ),
        (
            Kura::autonomous_lane_merge_bundle_paths_for_entry(lane, &fixture.kura.store_root),
            AutonomousLaneMergeBundleV1::FORMAT_LABEL,
        ),
    ]
}

fn assert_compaction_retained_sources(
    kura: &Kura,
    fixture_sources: &[DurableAutonomousLaneMergeSource],
) {
    for source in &fixture_sources[1..] {
        let payload = source.bundle.executable_payload();
        assert_eq!(
            kura.durable_autonomous_lane_merge_source(
                payload.origin_proposal.descriptor.lane_id,
                payload.origin_proposal.descriptor.lane_block_height,
                payload.network_id,
                payload.epoch,
            )
            .expect("every retained source remains exact after cold compaction"),
            *source
        );
    }
    assert_eq!(
        kura.certified_bundle_capacity_reserved_bytes()
            .expect("restored capacity"),
        0
    );
}

#[test]
fn lane_history_cold_restore_accepts_independent_authenticated_prefix_cuts() {
    for cut_pair in [0, 1] {
        let fixture = autonomous_history_compaction_fixture(false);
        let pairs = compaction_source_pairs(&fixture);
        let ((data, index), kind) = &pairs[cut_pair];
        assert!(Kura::prune_indexed_sidecars_through_terminal_frontier(
            data,
            index,
            fixture.frontier.lane_block_height,
            NonZeroUsize::MIN,
            kind,
        ));
        let AutonomousHistoryCompactionFixture {
            temp_dir,
            config,
            lane_config,
            kura,
            sources,
            ..
        } = fixture;
        drop(kura);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("cold recovery accepts one completed pair cut under authenticated retention");
        assert_compaction_retained_sources(&reopened, &sources);
        assert!(
            reopened
                .read_certified_lane_block_artifact(LaneId::SINGLE, 1)
                .is_none()
        );
        assert!(temp_dir.path().is_dir());
    }
}

#[test]
fn lane_history_cold_restore_recovers_certified_and_bundle_rewrite_cuts() {
    for observer in [false, true] {
        for pair_index in [1, 2] {
            for (data_promoted, index_pending) in [(false, true), (true, true), (false, false)] {
                let fixture = autonomous_history_compaction_fixture(observer);
                let (frontier_path, _) = Kura::latest_certified_lane_block_frontier_paths_for_entry(
                    fixture.lane_config.primary(),
                    &fixture.kura.store_root,
                );
                let singleton_before =
                    fs::read(&frontier_path).expect("retain exact singleton bytes");
                let pairs = compaction_source_pairs(&fixture);
                let ((data, index), kind) = &pairs[pair_index];
                let old_data = fs::read(data).expect("retain physical pre-compaction data");
                let old_index = fs::read(index).expect("retain physical pre-compaction index");
                assert!(Kura::prune_indexed_sidecars_through_terminal_frontier(
                    data,
                    index,
                    fixture.frontier.lane_block_height,
                    NonZeroUsize::MIN,
                    kind,
                ));
                let compacted_data = fs::read(data).expect("actual compacted data bytes");
                let compacted_index = fs::read(index).expect("actual compacted index bytes");
                assert!(compacted_data.len() < old_data.len());
                if observer {
                    assert!(compacted_data.is_empty());
                    assert_eq!(
                        compacted_index.len() as u64,
                        INDEXED_SIDECAR_BASE_HEADER_SIZE_U64,
                        "empty rewrite retains the canonical V1 header"
                    );
                }
                fs::write(index, old_index).expect("restore pre-promotion index");
                let temp_data = data.with_extension("norito.tmp");
                let temp_index = index.with_extension("index.tmp");
                if !data_promoted {
                    fs::write(data, old_data).expect("restore pre-promotion data");
                    fs::write(&temp_data, compacted_data)
                        .expect("stage exact durable rewrite data");
                }
                if index_pending {
                    fs::write(&temp_index, compacted_index)
                        .expect("stage exact durable rewrite index");
                } else {
                    assert!(
                        !temp_index.exists(),
                        "no index marker committed this data temporary"
                    );
                }
                let AutonomousHistoryCompactionFixture {
                    temp_dir,
                    config,
                    lane_config,
                    kura,
                    sources,
                    ..
                } = fixture;
                drop(kura);
                let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
                    .expect("cold recovery finishes committed rewrites or discards uncommitted data before inventory");
                assert!(!temp_data.exists() && !temp_index.exists());
                assert_eq!(
                    fs::read(&frontier_path).expect("retained singleton"),
                    singleton_before
                );
                if observer {
                    assert!(
                        reopened
                            .read_certified_lane_block_artifact_read_only(LaneId::SINGLE, 1)
                            .expect("read discarded observer slot without repair")
                            .is_none()
                    );
                    assert_eq!(
                        reopened
                            .certified_bundle_capacity_reserved_bytes()
                            .expect("observer reserve"),
                        0
                    );
                } else {
                    assert_compaction_retained_sources(&reopened, &sources);
                }
                assert!(temp_dir.path().is_dir());
            }
        }
    }
}

#[test]
fn lane_history_cold_restore_rejects_untrusted_frontier_and_retained_evidence_loss() {
    for damage in [0, 1, 2, 3, 4] {
        let fixture = autonomous_history_compaction_fixture(false);
        if damage == 0 {
            let mut frontier = fixture.frontier;
            frontier.application_block_hash =
                HashOf::from_untyped_unchecked(Hash::new(b"untrusted carrier"));
            let path = Kura::lane_merge_application_frontier_path_for_entry(
                fixture.lane_config.primary(),
                &fixture.kura.store_root,
            );
            fs::write(
                path,
                norito::encode_canonical(&frontier).expect("canonical untrusted frontier"),
            )
            .expect("replace only claimed carrier identity");
        } else {
            let pairs = compaction_source_pairs(&fixture);
            let pair_index = match damage {
                1 => 0,
                2 | 4 => 2,
                3 => 1,
                _ => unreachable!(),
            };
            let ((data, index), kind) = &pairs[pair_index];
            let original = (
                fs::read(data).expect("original data"),
                fs::read(index).expect("original index"),
            );
            // This genuine rewrite wrongly cuts retained slot2 as well as slot1.
            // An authenticated frontier2 only authorizes discarding slot1.
            assert!(Kura::prune_indexed_sidecars_through_terminal_frontier(
                data,
                index,
                3,
                NonZeroUsize::MIN,
                kind,
            ));
            if damage >= 3 {
                let candidate = (
                    fs::read(data).expect("invalid rewrite data"),
                    fs::read(index).expect("invalid rewrite index"),
                );
                fs::write(data, original.0).expect("restore valid original data");
                fs::write(index, original.1).expect("restore valid original index");
                fs::write(data.with_extension("norito.tmp"), candidate.0)
                    .expect("stage retained-slot-dropping data");
                fs::write(index.with_extension("index.tmp"), candidate.1)
                    .expect("stage retained-slot-dropping index");
            }
        }
        let AutonomousHistoryCompactionFixture {
            temp_dir,
            config,
            lane_config,
            kura,
            ..
        } = fixture;
        drop(kura);
        let before = snapshot_regular_test_tree(temp_dir.path());
        let error = match Kura::open_test_kura_with_configured_lane_config(&config, &lane_config) {
            Ok(_) => {
                panic!("untrusted frontier or retained evidence loss must reject cold startup")
            }
            Err(error) => error,
        };
        assert!(
            matches!(&error, Error::IO(_, _)),
            "wrong cold failure: {error}"
        );
        let expected = match damage {
            0 => "frontier",
            1 => "execution input",
            2 => "bundle",
            3 | 4 => "terminal rewrite omits a retained original slot",
            _ => unreachable!(),
        };
        assert!(
            error.to_string().contains(expected),
            "wrong evidence rejection: {error}"
        );
        assert_eq!(
            snapshot_regular_test_tree(temp_dir.path()),
            before,
            "invalid retention/evidence must not authorize repairs or deletions"
        );
    }
}

#[test]
fn lane_history_capacity_blocked_cold_restore_keeps_authenticated_prefix() {
    let mut fixture = autonomous_history_compaction_fixture(false);
    fixture
        .kura
        .refresh_disk_usage_bytes()
        .expect("refresh complete history bytes");
    let (persisted, unindexed) = fixture
        .kura
        .persisted_count_and_unindexed_bytes()
        .expect("canonical pending cursor");
    let baseline = fixture
        .kura
        .kura_disk_usage_bytes()
        .expect("physical history bytes")
        + fixture
            .kura
            .pending_block_bytes(persisted, unindexed)
            .expect("pending bytes")
        + Kura::canonical_prune_intent_maintenance_headroom_bytes();
    assert_eq!(
        fixture
            .kura
            .autonomous_global_terminal_outcome_reserved_bytes()
            .expect("no terminal lifecycle owner in this storage fixture"),
        0
    );
    assert_eq!(
        fixture
            .kura
            .certified_bundle_capacity_reserved_bytes()
            .expect("all local certification publications completed"),
        0
    );
    // This storage fixture creates no lifecycle cursor or terminal record, so
    // cold startup reconstructs no post-WSV owner. Its warm carrier publication
    // envelope must not inflate the cold limit and accidentally permit a rewrite.
    // The actual reopened owner inventories and constructor enforce this below.
    let pairs = compaction_source_pairs(&fixture);
    let ((data, index), _) = &pairs[1];
    let rewrite_bytes = Kura::sidecar_tracked_bytes(data, index).expect("certified rewrite peak");
    assert!(rewrite_bytes > 1);
    fixture.config.max_disk_usage_bytes = baseline + rewrite_bytes - 1;
    Arc::get_mut(&mut fixture.kura)
        .expect("exclusive compaction fixture")
        .max_disk_usage_bytes = fixture.config.max_disk_usage_bytes;
    let before = snapshot_regular_test_tree(fixture.temp_dir.path());
    assert_eq!(
        compact_fixture_lane_histories(
            &fixture.kura,
            fixture.lane_config.primary(),
            &fixture.frontier,
        )
        .expect("capacity refusal remains optional"),
        LaneHistoryCompactionOutcome::CapacityBlocked
    );
    assert_eq!(snapshot_regular_test_tree(fixture.temp_dir.path()), before);
    let AutonomousHistoryCompactionFixture {
        temp_dir,
        config,
        lane_config,
        kura,
        sources,
        ..
    } = fixture;
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("capacity-blocked compaction still permits valid startup");
    assert_eq!(
        reopened
            .post_wsv_lane_artifact_budget_reserved_bytes()
            .expect("actual cold post-WSV owner inventory"),
        0
    );
    assert_compaction_retained_sources(&reopened, &sources);
    assert_eq!(snapshot_regular_test_tree(temp_dir.path()), before);
    assert!(
        reopened
            .read_certified_lane_block_artifact(LaneId::SINGLE, 1)
            .is_some(),
        "capacity refusal must keep the old physical prefix"
    );
}

#[test]
fn lane_history_cold_restore_does_not_resurrect_terminal_local_frontier() {
    let fixture = autonomous_history_compaction_fixture(true);
    let original_frontier = fixture
        .kura
        .latest_certified_lane_block_frontier(LaneId::SINGLE)
        .expect("observer retains its earlier local certificate");
    assert_eq!(original_frontier.proposal.descriptor.lane_block_height, 1);
    let (singleton_path, _) = Kura::latest_certified_lane_block_frontier_paths_for_entry(
        fixture.lane_config.primary(),
        &fixture.kura.store_root,
    );
    let singleton_bytes = fs::read(&singleton_path).expect("retain exact original singleton");
    let pairs = compaction_source_pairs(&fixture);
    for ((data, index), kind) in &pairs {
        assert!(Kura::prune_indexed_sidecars_through_terminal_frontier(
            data,
            index,
            fixture.frontier.lane_block_height,
            NonZeroUsize::MIN,
            kind,
        ));
    }
    let AutonomousHistoryCompactionFixture {
        temp_dir,
        config,
        lane_config,
        kura,
        payloads,
        ..
    } = fixture;
    drop(kura);
    let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
        .expect("applied frontier ahead of local certification is valid after compaction");
    assert_eq!(
        reopened
            .certified_bundle_capacity_reserved_bytes()
            .expect("observer reserve"),
        0
    );
    assert!(
        reopened
            .read_certified_lane_block_artifact(LaneId::SINGLE, 1)
            .is_none(),
        "terminal local singleton must not repopulate compacted history"
    );
    let payload = &payloads[0];
    assert!(
        reopened
            .durable_autonomous_lane_merge_source(
                LaneId::SINGLE,
                1,
                payload.network_id,
                payload.epoch,
            )
            .is_err(),
        "discarded local payload is not re-created as a new publication"
    );
    let lane = lane_config.primary();
    let (incarnation, activation_height) = reopened
        .active_lane_incarnation_marker(lane)
        .expect("authenticate reopened primary incarnation");
    let descriptor = &original_frontier.proposal.descriptor;
    assert_eq!(descriptor.lane_id, lane.lane_id);
    assert_eq!(descriptor.dataspace_id, lane.dataspace_id);
    assert_eq!(descriptor.lane_incarnation, incarnation);
    assert_eq!(
        activation_height, 0,
        "fixture has no lane reset or reactivation"
    );
    // Use the existing storage-test authority seam with the actual reopened
    // geometry and signed singleton. No reset/replacement permission is granted;
    // production preflight still authenticates the carrier and occupied slots.
    let authority = crate::state::CertifiedLaneBlockPersistenceAuthority::for_test(
        lane.lane_id,
        lane.dataspace_id,
        incarnation,
        None,
    );
    assert!(authority.authorizes_proposal(&original_frontier.proposal));
    assert!(!authority.permits_slot_replacement(descriptor, descriptor));
    let before_preflight = snapshot_regular_test_tree(temp_dir.path());
    assert!(reopened.preflight_latest_certified_lane_block_frontier_with_authority(
        lane.lane_id, &authority,
    ).expect("State-facing preflight accepts proof-discardable singleton").is_none(),
        "obsolete local certification must not become State repair work");
    assert_eq!(
        snapshot_regular_test_tree(temp_dir.path()),
        before_preflight
    );
    assert_eq!(
        fs::read(&singleton_path).expect("preserved monotonic singleton"),
        singleton_bytes
    );
    assert!(temp_dir.path().is_dir());
}

#[test]
fn lane_history_cold_restore_admits_obsolete_append_at_exact_capacity() {
    // A newer local append is retained work, even when an earlier canonical
    // application frontier already permits compaction of a different prefix.
    {
        let fixture = autonomous_history_compaction_fixture_with_observer_append(false, true);
        let pairs = compaction_source_pairs(&fixture);
        let intent = Kura::bound_progress_append_intent_path(&pairs[2].0.1);
        assert!(intent.is_file());
        let AutonomousHistoryCompactionFixture {
            temp_dir,
            config,
            lane_config,
            kura,
            sources,
            ..
        } = fixture;
        drop(kura);
        let (reopened, _) = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config)
            .expect("repair retained current append before compacting the older applied prefix");
        assert!(!intent.exists());
        assert_compaction_retained_sources(&reopened, &sources);
        assert!(
            reopened
                .read_certified_lane_block_artifact_read_only(LaneId::SINGLE, 1)
                .expect("read compacted prefix without repair")
                .is_none()
        );
        assert!(temp_dir.path().is_dir());
    }
    for input_cut in [false, true] {
        for one_under in [true, false] {
            let mut fixture =
                autonomous_history_compaction_fixture_with_observer_append(true, true);
            let pairs = compaction_source_pairs(&fixture);
            if input_cut {
                let ((data, index), kind) = &pairs[0];
                assert!(Kura::prune_indexed_sidecars_through_terminal_frontier(
                    data,
                    index,
                    fixture.frontier.lane_block_height,
                    NonZeroUsize::MIN,
                    kind,
                ));
            }
            let input_before = (
                fs::read(&pairs[0].0.0).expect("retain exact input pair data"),
                fs::read(&pairs[0].0.1).expect("retain exact input pair index"),
            );
            let intent = Kura::bound_progress_append_intent_path(&pairs[2].0.1);
            assert!(intent.is_file());
            let before_plan = snapshot_regular_test_tree(fixture.temp_dir.path());
            let remaining_index_growth = {
                let _prune_guard = fixture.kura.prune_lock.lock();
                let lane = fixture.lane_config.primary();
                let proof = fixture
                    .kura
                    .authenticated_lane_history_retention_under_prune_guard(lane)
                    .expect("authenticate obsolete append retention")
                    .expect("receipt owns prefix");
                let frontier = &fixture.sources[0].bundle.certified;
                assert!(proof.permits_discard(&frontier.proposal.descriptor));
                let plans = fixture
                    .kura
                    .plan_obsolete_certified_bundle_appends_under_prune_guard(
                        lane, frontier, &proof,
                    )
                    .expect("read-only journal plan needs no discarded execution input");
                assert_eq!(plans.len(), 1);
                plans
                    .iter()
                    .map(ObsoleteCertifiedBundleAppendPlan::remaining_index_growth)
                    .sum::<u64>()
            };
            assert!(remaining_index_growth > 0);
            assert_eq!(
                snapshot_regular_test_tree(fixture.temp_dir.path()),
                before_plan
            );
            let (persisted, unindexed) = fixture
                .kura
                .persisted_count_and_unindexed_bytes()
                .expect("cold append pending cursor");
            let required = fixture
                .kura
                .kura_disk_usage_bytes()
                .expect("physical append preimage")
                + fixture
                    .kura
                    .pending_block_bytes(persisted, unindexed)
                    .expect("pending canonical bytes")
                + Kura::canonical_prune_intent_maintenance_headroom_bytes()
                + remaining_index_growth;
            fixture.config.max_disk_usage_bytes = required - u64::from(one_under);
            let AutonomousHistoryCompactionFixture {
                temp_dir,
                config,
                lane_config,
                kura,
                ..
            } = fixture;
            drop(kura);
            let before_open = snapshot_regular_test_tree(temp_dir.path());
            let reopened = Kura::open_test_kura_with_configured_lane_config(&config, &lane_config);
            if one_under {
                let error = match reopened {
                    Ok(_) => panic!("one byte below obsolete append completion must reject"),
                    Err(error) => error,
                };
                assert!(
                    matches!(error, Error::StorageBudgetExceeded { limit, required: actual, .. }
                    if limit == required - 1 && actual == required),
                    "wrong capacity rejection: {error}"
                );
                assert_eq!(
                    snapshot_regular_test_tree(temp_dir.path()),
                    before_open,
                    "aggregate capacity refusal must precede append recovery or compaction"
                );
                assert!(intent.is_file());
            } else {
                let (reopened, _) =
                    reopened.expect("exact obsolete append completion capacity succeeds");
                assert!(!intent.exists());
                assert_eq!(
                    reopened
                        .certified_bundle_capacity_reserved_bytes()
                        .expect("obsolete append leaves no live reservation"),
                    0
                );
                assert_eq!(
                    reopened
                        .post_wsv_lane_artifact_budget_reserved_bytes()
                        .expect("cold fixture has no lifecycle owner"),
                    0
                );
                if input_cut {
                    assert_eq!(
                        (
                            fs::read(&pairs[0].0.0).expect("post-recovery input data"),
                            fs::read(&pairs[0].0.1).expect("post-recovery input index"),
                        ),
                        input_before,
                        "append recovery must not reconstruct an already discarded input"
                    );
                }
                // Removing the completed intent may free enough space for
                // optional compaction; an intact old input may then be pruned.
                assert_eq!(
                    reopened
                        .read_lane_block_application_receipt(LaneId::SINGLE, 2)
                        .expect("canonical successor receipt survives old append recovery")
                        .proposal
                        .descriptor
                        .lane_block_height,
                    2
                );
            }
        }
    }
}
