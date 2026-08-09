// Node reconciliation, appeal-finance, storage, and repair regressions.

fn reconciliation_handle_with_governance(root: &Path) -> NodeHandle {
    let signer = Arc::new(TestGovernanceDagSigner::new());
    let cfg = StorageConfig::builder()
        .enabled(true)
        .data_dir(root.join("storage"))
        .governance_dir(Some(root.join("governance")))
        .governance_dag_publisher_peer_id(Some(
            String::from_utf8(signer.publisher_peer_id().to_vec()).expect("test peer id is UTF-8"),
        ))
        .governance_dag_signer_handle(Some(signer.handle().to_owned()))
        .governance_dag_signer_qualification(
            Some(TestGovernanceDagSigner::expected_qualification()),
        )
        .governance_dag_checkpoint_store_handle(Some(
            TestGovernanceDagCheckpointStore::HANDLE.to_owned(),
        ))
        .governance_dag_checkpoint_store_qualification(Some(
            TestGovernanceDagCheckpointStore::expected_qualification(),
        ))
        .governance_dag_publisher_public_key_hex(Some(hex::encode(signer.public_key())))
        .build();
    let handle = NodeHandle::try_new_with_policies_and_runtime_deps(
        cfg,
        enabled_repair_config(1),
        GcConfig::default(),
        NodeRuntimeDeps::default()
            .with_governance_dag_signer(signer)
            .with_governance_dag_checkpoint_store(Arc::new(
                TestGovernanceDagCheckpointStore::default(),
            )),
    )
    .expect("runtime-signed governance publisher");
    ensure_test_capacity_provider(&handle);
    assert!(handle.has_governance_publisher());
    handle
}

#[test]
fn governance_dag_file_reads_are_descriptor_rooted_and_bounded() {
    let temp = tempfile::tempdir().expect("create governance readback root");
    let handle = reconciliation_handle_with_governance(temp.path());
    let governance = temp.path().join("governance");
    fs::create_dir(governance.join("snapshots")).expect("create snapshot directory");
    fs::write(governance.join("snapshots/state.json"), b"state-v1")
        .expect("write governance snapshot");

    assert_eq!(
        handle
            .read_governance_dag_file(Path::new("snapshots/state.json"), 8)
            .expect("read exact bounded snapshot"),
        b"state-v1"
    );
    assert_eq!(
        handle
            .read_governance_dag_file(Path::new("snapshots/state.json"), 7)
            .expect_err("oversized snapshot must fail")
            .kind(),
        io::ErrorKind::InvalidData
    );
    assert_eq!(
        handle
            .read_governance_dag_file(Path::new("../outside"), 8)
            .expect_err("parent traversal must fail")
            .kind(),
        io::ErrorKind::InvalidInput
    );
    assert_eq!(
        handle
            .read_governance_dag_file(&governance.join("snapshots/state.json"), 8)
            .expect_err("absolute path must fail")
            .kind(),
        io::ErrorKind::InvalidInput
    );

    #[cfg(unix)]
    {
        let outside = temp.path().join("outside");
        fs::create_dir(&outside).expect("create outside directory");
        fs::write(outside.join("state.json"), b"outside!").expect("write outside snapshot");
        std::os::unix::fs::symlink(&outside, governance.join("linked"))
            .expect("create substituted directory link");
        assert!(
            handle
                .read_governance_dag_file(Path::new("linked/state.json"), 8)
                .is_err(),
            "descriptor-rooted traversal must reject a linked path component"
        );
    }
}

fn weekly_rollup_publish_index_entry(root: &Path) -> JsonValue {
    let publication_state_path = root
        .join("governance")
        .join("governance-publication-state-v1.json");
    let publication_state = norito::json::from_slice::<JsonValue>(
        &fs::read(&publication_state_path)
            .expect("read authoritative governance publication state"),
    )
    .expect("decode authoritative governance publication state");
    publication_state
        .get("publish_index")
        .expect("governance publication state publish index")
        .get("entries")
        .and_then(JsonValue::as_array)
        .expect("governance publish-index entries")
        .iter()
        .find(|entry| {
            entry.get("payload_kind").and_then(JsonValue::as_str)
                == Some("appeal_finance_weekly_rollup")
        })
        .cloned()
        .expect("weekly rollup publish-index entry")
}

fn indexed_governance_artifact(root: &Path, entry: &JsonValue, field: &str) -> PathBuf {
    let relative = entry
        .get(field)
        .and_then(JsonValue::as_str)
        .expect("governance artifact path");
    root.join("governance").join(relative)
}

fn rewrite_test_digest_sidecar(path: &Path) {
    let bytes = fs::read(path).expect("read governance artifact");
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .map_or_else(
            || "blake3".to_string(),
            |extension| format!("{extension}.blake3"),
        );
    fs::write(
        path.with_extension(extension),
        format!("{}\n", blake3::hash(&bytes).to_hex()),
    )
    .expect("rewrite governance artifact digest");
}

#[test]
fn node_handle_reconciliation_includes_appeal_finance_rollups() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let handle = reconciliation_handle_with_governance(&root);

    let rollup = appeal_finance_weekly_rollup_fixture();
    handle
        .publish_authenticated_appeal_finance_weekly_rollup(
            rollup.clone(),
            governance_submission_account(0xB8),
        )
        .expect("publish appeal finance weekly rollup");

    let reconciliation = handle
        .run_reconciliation_once(1_700_000_300, &empty_finalized_repair_projection())
        .expect("reconciliation report");
    let appeal_finance = reconciliation
        .appeal_finance
        .as_ref()
        .expect("appeal finance reconciliation summary");
    assert_eq!(appeal_finance.rollup_count, 1);
    assert_ne!(appeal_finance.rollup_snapshot_hash, [0u8; 32]);
    assert_eq!(appeal_finance.source_report_count, rollup.report_count);
    assert_eq!(appeal_finance.case_count, rollup.case_count);
    assert_eq!(appeal_finance.total_treasury_xor, rollup.total_treasury_xor);
    assert_eq!(
        appeal_finance.total_rewards_forfeited_treasury_xor,
        rollup.total_rewards_forfeited_treasury_xor
    );
}

#[test]
fn node_handle_reconciliation_ignores_tampered_rollup_json_mirror() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let handle = reconciliation_handle_with_governance(&root);
    let rollup = appeal_finance_weekly_rollup_fixture();
    handle
        .publish_authenticated_appeal_finance_weekly_rollup(
            rollup.clone(),
            governance_submission_account(0xB9),
        )
        .expect("publish appeal finance weekly rollup");

    let index_entry = weekly_rollup_publish_index_entry(&root);
    let json_path = indexed_governance_artifact(&root, &index_entry, "json_path");
    fs::write(
        &json_path,
        br#"{"metadata":{"cycle":"2099-W52","report_count":999999,"case_count":999999,"total_treasury_xor":"999999999","total_rewards_forfeited_treasury_xor":"999999999"}}"#,
    )
    .expect("replace display-only rollup JSON");
    rewrite_test_digest_sidecar(&json_path);

    let reconciliation = handle
        .run_reconciliation_once(1_700_000_301, &empty_finalized_repair_projection())
        .expect("reconciliation authenticates the signed canonical rollup");
    let summary = reconciliation
        .appeal_finance
        .expect("appeal finance reconciliation summary");
    assert_eq!(summary.rollup_count, 1);
    assert_eq!(summary.source_report_count, rollup.report_count);
    assert_eq!(summary.case_count, rollup.case_count);
    assert_eq!(summary.total_treasury_xor, rollup.total_treasury_xor);
    assert_eq!(
        summary.total_rewards_forfeited_treasury_xor,
        rollup.total_rewards_forfeited_treasury_xor
    );
}

#[test]
fn node_handle_reconciliation_rejects_rollup_source_substitution() {
    let temp_dir = tempfile::tempdir().expect("create temp dir");
    let root = temp_dir.path().canonicalize().expect("canonical temp dir");
    let handle = reconciliation_handle_with_governance(&root);
    handle
        .publish_authenticated_appeal_finance_weekly_rollup(
            appeal_finance_weekly_rollup_fixture(),
            governance_submission_account(0xBA),
        )
        .expect("publish appeal finance weekly rollup");

    let index_entry = weekly_rollup_publish_index_entry(&root);
    let encoded_path = indexed_governance_artifact(&root, &index_entry, "encoded_path");
    let mut encoded = fs::read(&encoded_path).expect("read canonical weekly rollup");
    let last = encoded.last_mut().expect("weekly rollup is not empty");
    *last ^= 1;
    fs::write(&encoded_path, encoded).expect("substitute canonical weekly rollup");
    rewrite_test_digest_sidecar(&encoded_path);

    let error = handle
        .run_reconciliation_once(1_700_000_302, &empty_finalized_repair_projection())
        .expect_err("signed source substitution must fail closed");
    let message = match error {
        ReconciliationError::AppealFinance(message) => message,
        other => panic!("unexpected reconciliation error: {other}"),
    };
    assert!(
        message.contains("source payload"),
        "unexpected source-substitution error: {message}"
    );
}

#[test]
fn reconciliation_without_provider_binding_fails_without_publication() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::default());
    let publisher = Arc::new(RecordingPublisher::default());
    handle.set_governance_publisher(publisher.clone());

    let error = handle
        .run_reconciliation_once(1_700_000_301, &empty_finalized_repair_projection())
        .expect_err("unbound reconciliation must fail closed");

    assert!(matches!(
        error,
        ReconciliationError::ProviderBindingUnavailable
    ));
    assert!(publisher.take().is_empty());
    assert_eq!(handle.pending_governance_publication_count(), 0);
}

#[test]
fn appeal_finance_exact_addition_normalizes_scale() {
    let sum = xor("420")
        .checked_add(&xor("80.2500"))
        .expect("exact XOR sum");

    assert_eq!(sum, xor("500.25"));
}

#[test]
fn node_handle_gc_skips_manifest_with_active_repair_task() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let gc_actual = iroha_config::parameters::actual::SorafsGc {
        enabled: true,
        retention_grace_secs: 0,
        max_deletions_per_run: 10,
        ..Default::default()
    };
    let handle =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));
    let provider_id = ensure_test_capacity_provider(&handle);

    let payload = b"gc-repair-blocked";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let retention_epoch = 1_700_000_000;
    let now_unix = retention_epoch + 10;
    let mut policy = PinPolicy::default();
    policy.retention_epoch = retention_epoch;
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(policy)
        .build()
        .expect("manifest");

    let mut reader = payload.as_slice();
    let manifest_id = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest manifest");
    let manifest_digest: [u8; 32] = manifest.digest().expect("digest").into();

    let repair_projection = finalized_repair_projection(vec![active_native_repair_task(
        manifest_digest,
        provider_id,
    )]);
    let report = run_test_gc(&handle, now_unix, &repair_projection);
    assert!(report.evictions.is_empty());
    assert!(
        report
            .skipped
            .iter()
            .any(|skip| skip.reason == "repair_active")
    );
    assert!(handle.manifest_metadata(&manifest_id).is_ok());
}

#[test]
fn node_handle_gc_blocks_shared_chunks_and_records_metrics() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let gc_actual = iroha_config::parameters::actual::SorafsGc {
        enabled: true,
        retention_grace_secs: 0,
        max_deletions_per_run: 10,
        ..Default::default()
    };
    let handle =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));

    let payload = b"shared-chunk-payload";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let retention_epoch = 1_700_000_000;
    let now_unix = retention_epoch + 10;
    let mut policy = PinPolicy::default();
    policy.retention_epoch = retention_epoch;

    let manifest_a = manifest_builder_for_plan(payload, &plan)
        .add_metadata("test.fixture_id", "gc-shared-a")
        .pin_policy(policy.clone())
        .build()
        .expect("manifest a");
    let manifest_b = manifest_builder_for_plan(payload, &plan)
        .add_metadata("test.fixture_id", "gc-shared-b")
        .pin_policy(policy)
        .build()
        .expect("manifest b");

    let mut reader = payload.as_slice();
    handle
        .ingest_manifest(&manifest_a, &plan, &mut reader)
        .expect("ingest manifest a");
    let mut reader = payload.as_slice();
    handle
        .ingest_manifest(&manifest_b, &plan, &mut reader)
        .expect("ingest manifest b");

    let metrics = global_or_default();
    let before = metrics
        .torii_sorafs_gc_blocked_total
        .with_label_values(&["shared_chunks"])
        .get();

    let report = run_test_gc(&handle, now_unix, &empty_finalized_repair_projection());
    assert!(report.evictions.is_empty());
    assert!(
        report
            .skipped
            .iter()
            .any(|skip| skip.reason == "shared_chunks")
    );

    let after = metrics
        .torii_sorafs_gc_blocked_total
        .with_label_values(&["shared_chunks"])
        .get();
    assert!(after >= before.saturating_add(1));
}

#[test]
fn node_handle_reflects_config() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg.clone());

    assert!(handle.is_enabled());
    let observed = handle.config();
    assert_eq!(observed.enabled(), cfg.enabled());
    assert_eq!(observed.data_dir(), cfg.data_dir());
    assert_eq!(observed.max_capacity_bytes().0, cfg.max_capacity_bytes().0);
    assert_eq!(observed.max_parallel_fetches(), cfg.max_parallel_fetches());
    assert_eq!(observed.max_pins(), cfg.max_pins());
    assert_eq!(
        observed.por_sample_interval_secs(),
        cfg.por_sample_interval_secs()
    );
    assert_eq!(observed.alias(), cfg.alias());
    assert_eq!(observed.adverts().topics(), cfg.adverts().topics());
    assert!(handle.storage().is_some());
    assert!(handle.pdp_provider_protocol().is_some());
}

#[test]
fn node_handle_is_disabled_when_backend_is_unavailable() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let mut handle = NodeHandle::new(cfg);

    handle.storage = None;

    assert!(!handle.is_enabled());
}

#[test]
fn node_handle_threads_repair_and_gc_config() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let actual_repair = iroha_config::parameters::actual::SorafsRepair {
        enabled: true,
        claim_ttl_secs: 900,
        heartbeat_interval_secs: 45,
        max_attempts: 6,
        worker_concurrency: 9,
    };

    let actual_gc = iroha_config::parameters::actual::SorafsGc {
        enabled: true,
        interval_secs: 300,
        max_deletions_per_run: 2_000,
        retention_grace_secs: 86_400,
        ..Default::default()
    };

    let repair_cfg = RepairConfig::from(&actual_repair);
    let gc_cfg = GcConfig::from(&actual_gc);
    let handle = NodeHandle::new_with_policies(cfg, repair_cfg.clone(), gc_cfg.clone());

    assert!(handle.repair_config().enabled());
    assert_eq!(handle.repair_config().claim_ttl_secs(), 900);
    assert_eq!(handle.repair_config().heartbeat_interval_secs(), 45);
    assert_eq!(handle.repair_config().max_attempts(), 6);
    assert_eq!(handle.repair_config().worker_concurrency(), 9);

    assert!(handle.gc_config().enabled());
    assert_eq!(handle.gc_config().interval_secs(), 300);
    assert_eq!(handle.gc_config().max_deletions_per_run(), 2_000);
    assert_eq!(handle.gc_config().retention_grace_secs(), 86_400);
}

#[test]
fn native_repair_config_fails_startup_outside_consensus_and_resource_bounds() {
    let baseline = iroha_config::parameters::actual::SorafsRepair {
        enabled: true,
        claim_ttl_secs: 2,
        heartbeat_interval_secs: 1,
        max_attempts: 1,
        worker_concurrency: 1,
    };
    let mut invalid = Vec::new();

    let mut lease_too_small = baseline;
    lease_too_small.claim_ttl_secs = 0;
    invalid.push(("claim_ttl_secs", lease_too_small));
    let mut lease_overflow = baseline;
    lease_overflow.claim_ttl_secs = u64::MAX;
    invalid.push(("overflows", lease_overflow));
    let mut renewal_zero = baseline;
    renewal_zero.heartbeat_interval_secs = 0;
    invalid.push(("heartbeat_interval_secs", renewal_zero));
    let mut renewal_not_below = baseline;
    renewal_not_below.heartbeat_interval_secs = renewal_not_below.claim_ttl_secs;
    invalid.push(("strictly below", renewal_not_below));
    let mut attempts_zero = baseline;
    attempts_zero.max_attempts = 0;
    invalid.push(("max_attempts", attempts_zero));
    let mut attempts_large = baseline;
    attempts_large.max_attempts =
        iroha_config::parameters::defaults::sorafs::repair::MAX_ATTEMPTS_LIMIT + 1;
    invalid.push(("max_attempts", attempts_large));
    let mut concurrency_zero = baseline;
    concurrency_zero.worker_concurrency = 0;
    invalid.push(("worker_concurrency", concurrency_zero));
    let mut concurrency_large = baseline;
    concurrency_large.worker_concurrency =
        iroha_config::parameters::defaults::sorafs::repair::WORKER_CONCURRENCY_LIMIT + 1;
    invalid.push(("worker_concurrency", concurrency_large));
    let mut disabled_but_consumed = baseline;
    disabled_but_consumed.enabled = false;
    disabled_but_consumed.max_attempts = 0;
    invalid.push(("max_attempts", disabled_but_consumed));

    let temp = tempfile::tempdir().expect("temp dir");
    for (expected, repair) in invalid {
        let config = StorageConfig::builder()
            .enabled(false)
            .data_dir(temp.path().join(expected))
            .build();
        let error = NodeHandle::try_new_with_policies(
            config,
            RepairConfig::from(repair),
            GcConfig::default(),
        )
        .expect_err("invalid enabled native repair config must fail startup");
        assert!(matches!(error, NodeInitError::NativeRepairConfig { .. }));
        assert!(error.to_string().contains(expected), "{error}");
    }

    let maximum = iroha_config::parameters::actual::SorafsRepair {
        claim_ttl_secs: REPAIR_LEDGER_MAX_LEASE_MS_V1 / 1_000,
        heartbeat_interval_secs: REPAIR_LEDGER_MAX_LEASE_MS_V1 / 1_000 - 1,
        ..baseline
    };
    validate_native_repair_config(&RepairConfig::from(maximum))
        .expect("maximum bounded native lease config is accepted");
}

#[test]
fn node_handle_records_capacity_declaration() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg);

    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: [0x11; 32],
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xAA; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 100,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 100,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 100,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 2,
        metadata: vec![],
    };
    let payload = to_bytes(&declaration).expect("encode declaration");
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        payload,
        declaration.committed_capacity_gib,
        1,
        1,
        2,
        Metadata::default(),
    );

    handle
        .record_capacity_declaration(&record)
        .expect("record declaration");

    let usage = handle.capacity_usage();
    assert_eq!(usage.provider_id, Some([0x11; 32]));
    assert_eq!(usage.committed_total_gib, 100);
    assert_eq!(usage.available_total_gib, 100);

    let telemetry = handle
        .build_capacity_telemetry()
        .expect("telemetry accumulator present")
        .expect("telemetry payload");
    assert_eq!(telemetry.declared_capacity_gib, 100);
    assert_eq!(telemetry.utilised_capacity_gib, 0);
    assert_eq!(telemetry.successful_replications, 0);
}

#[test]
fn node_handle_completes_replication_order() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg);

    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: [0x22; 32],
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xAA; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 200,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 200,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 200,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 100,
        metadata: vec![],
    };
    let payload = norito::to_bytes(&declaration).expect("encode declaration");
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        payload,
        declaration.committed_capacity_gib,
        1,
        1,
        100,
        Metadata::default(),
    );

    handle
        .record_capacity_declaration(&record)
        .expect("record declaration");

    let order = ReplicationOrderV1 {
        version: sorafs_manifest::capacity::REPLICATION_ORDER_VERSION_V1,
        order_id: [0x99; 32],
        manifest_cid: vec![0x55; 32],
        manifest_digest: [0x77; 32],
        chunking_profile: "sorafs.sf1@1.0.0".into(),
        target_replicas: 1,
        assignments: vec![sorafs_manifest::capacity::ReplicationAssignmentV1 {
            provider_id: [0x22; 32],
            slice_gib: 50,
            lane: Some("default".into()),
        }],
        issued_at: 10,
        deadline_at: 20,
        sla: sorafs_manifest::capacity::ReplicationOrderSlaV1 {
            ingest_deadline_secs: 600,
            min_availability_percent_milli: 99_000,
            min_por_success_percent_milli: 99_000,
        },
        metadata: Vec::new(),
    };

    let plan = handle
        .schedule_replication_order(&order)
        .expect("schedule order")
        .expect("plan produced");
    assert_eq!(plan.assigned_slice_gib, 50);

    let release = handle
        .complete_replication_order(order.order_id)
        .expect("complete order");
    assert_eq!(release.released_gib, 50);
    assert_eq!(release.remaining_total_gib, 200);
}

#[test]
fn capacity_declaration_reservations_and_meter_survive_restart() {
    let (base, _dir) = storage_config_with_temp_dir();
    let cfg = StorageConfig::builder()
        .enabled(true)
        .data_dir(base.data_dir().clone())
        .runtime_retention(RuntimeRetentionPolicy::new(4, 8, 2 * 1024 * 1024))
        .build();
    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: [0x23; 32],
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xAA; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 200,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 200,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 200,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 100,
        metadata: vec![],
    };
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        norito::to_bytes(&declaration).expect("encode declaration"),
        declaration.committed_capacity_gib,
        1,
        1,
        100,
        Metadata::default(),
    );
    let order = ReplicationOrderV1 {
        version: sorafs_manifest::capacity::REPLICATION_ORDER_VERSION_V1,
        order_id: [0x9A; 32],
        manifest_cid: vec![0x55; 32],
        manifest_digest: [0x78; 32],
        chunking_profile: "sorafs.sf1@1.0.0".into(),
        target_replicas: 1,
        assignments: vec![sorafs_manifest::capacity::ReplicationAssignmentV1 {
            provider_id: declaration.provider_id,
            slice_gib: 50,
            lane: Some("default".into()),
        }],
        issued_at: 10,
        deadline_at: 20,
        sla: sorafs_manifest::capacity::ReplicationOrderSlaV1 {
            ingest_deadline_secs: 600,
            min_availability_percent_milli: 99_000,
            min_por_success_percent_milli: 99_000,
        },
        metadata: Vec::new(),
    };
    let source = NodeHandle::new(cfg.clone());
    source
        .record_capacity_declaration(&record)
        .expect("persist declaration");
    source
        .schedule_replication_order(&order)
        .expect("persist order")
        .expect("targeted plan");
    drop(source);

    let restored = NodeHandle::new(cfg);
    let usage = restored.capacity_usage();
    assert_eq!(usage.provider_id, Some(declaration.provider_id));
    assert_eq!(usage.allocated_total_gib, 50);
    assert_eq!(usage.outstanding_orders.len(), 1);
    assert_eq!(usage.outstanding_orders[0].issued_at, 10);
    let meter = restored.metering_snapshot();
    assert_eq!(meter.declared_gib, 200);
    assert_eq!(meter.orders_issued, 1);
    assert_eq!(meter.outstanding_orders, 1);
    assert_eq!(meter.outstanding_total_gib, 50);
    assert!(restored.build_capacity_telemetry().is_some());
    let release = restored
        .complete_replication_order(order.order_id)
        .expect("complete restored order");
    assert_eq!(release.released_gib, 50);
    assert_eq!(restored.capacity_usage().allocated_total_gib, 0);
}

#[test]
fn node_handle_meter_tracks_replication_flow() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg);

    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: [0x55; 32],
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xAA; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 256,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 256,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 256,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 10,
        metadata: vec![],
    };
    let payload = norito::to_bytes(&declaration).expect("encode declaration");
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        payload,
        declaration.committed_capacity_gib,
        0,
        1,
        10,
        Metadata::default(),
    );
    handle
        .record_capacity_declaration(&record)
        .expect("record declaration");

    let meter = handle.capacity_meter();
    let snapshot = meter.snapshot();
    assert_eq!(snapshot.declared_gib, 256);
    assert_eq!(snapshot.orders_issued, 0);
    assert_eq!(snapshot.outstanding_orders, 0);

    let order = ReplicationOrderV1 {
        version: REPLICATION_ORDER_VERSION_V1,
        order_id: [0x44; 32],
        manifest_cid: vec![0xDE, 0xAD],
        manifest_digest: [0xCD; 32],
        chunking_profile: "sorafs.sf1@1.0.0".into(),
        target_replicas: 1,
        assignments: vec![ReplicationAssignmentV1 {
            provider_id: declaration.provider_id,
            slice_gib: 64,
            lane: Some("default".into()),
        }],
        issued_at: 100,
        deadline_at: 400,
        sla: ReplicationOrderSlaV1 {
            ingest_deadline_secs: 600,
            min_availability_percent_milli: 99_000,
            min_por_success_percent_milli: 98_000,
        },
        metadata: vec![CapacityMetadataEntry {
            key: "priority".into(),
            value: "standard".into(),
        }],
    };

    let plan = handle
        .schedule_replication_order(&order)
        .expect("schedule ok")
        .expect("plan expected");
    assert_eq!(plan.assigned_slice_gib, 64);

    let snapshot_after_schedule = meter.snapshot();
    assert_eq!(snapshot_after_schedule.orders_issued, 1);
    assert_eq!(snapshot_after_schedule.outstanding_orders, 1);
    assert_eq!(snapshot_after_schedule.outstanding_total_gib, 64);

    handle
        .complete_replication_order(order.order_id)
        .expect("complete order");

    let snapshot_after_complete = meter.snapshot();
    assert_eq!(snapshot_after_complete.orders_completed, 1);
    assert_eq!(snapshot_after_complete.utilised_gib, 64);
    assert_eq!(snapshot_after_complete.outstanding_orders, 0);

    handle.update_telemetry(|acc| {
        acc.record_uptime_sample(540, 600).expect("uptime sample");
        acc.record_por_sample(true);
        acc.record_por_sample(false);
    });

    let telemetry = handle
        .build_capacity_telemetry()
        .expect("telemetry accumulator present")
        .expect("payload");
    assert_eq!(telemetry.successful_replications, 1);
    assert_eq!(telemetry.failed_replications, 0);
    assert_eq!(telemetry.uptime_percent_milli, 90_000);
    assert_eq!(telemetry.por_success_percent_milli, 50_000);
}

#[test]
fn node_handle_storage_ingest_and_fetch_range() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg);

    let payload = b"node handle storage fetch test";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");

    let mut reader = &payload[..];
    let manifest_id = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest");

    let bytes = handle
        .read_payload_range(&manifest_id, 5, 6)
        .expect("read range");
    assert_eq!(bytes, b"handle"[..]);
}

#[test]
fn node_handle_storage_sample_por() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg);

    let payload = b"SoraFS node handle PoR sampling payload";
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let manifest = manifest_builder_for_plan(payload, &plan)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");

    let mut reader = &payload[..];
    let manifest_id = handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest");

    let storage = handle.storage().expect("storage backend");
    let stored = storage.manifest(&manifest_id).expect("stored manifest");
    let expected = stored.por_tree().leaf_count().min(3);

    let samples = handle.sample_por(&manifest_id, 3, 99).expect("sample por");
    assert_eq!(samples.len(), expected);
    let root = *stored.por_tree().root();

    for (_idx, proof) in samples {
        assert!(proof.verify(&root));
    }
}

#[test]
fn node_handle_plan_por_challenges_handles_vrf_and_forced() {
    use std::collections::HashMap;
    let (cfg, _dir) = storage_config_with_temp_dir();
    let handle = NodeHandle::new(cfg);

    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: [0x11; 32],
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xAA; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 128,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 128,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 128,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 2,
        metadata: vec![],
    };
    let payload = to_bytes(&declaration).expect("encode declaration");
    let provider_metadata = {
        let mut metadata = Metadata::default();
        metadata.insert(
            Name::from_str("profile.sample_multiplier").expect("valid metadata key"),
            2u64,
        );
        metadata
    };
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        payload,
        declaration.committed_capacity_gib,
        1,
        1,
        2,
        provider_metadata,
    );
    handle
        .record_capacity_declaration(&record)
        .expect("record declaration");

    let payload = vec![0xEE; 128 * 1024];
    let plan = CarBuildPlan::single_file(&payload).expect("plan");
    let manifest = manifest_builder_for_plan(&payload, &plan)
        .pin_policy(PinPolicy::default())
        .build()
        .expect("manifest");

    let mut reader = &payload[..];
    handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest");

    let randomness = PorRandomness {
        epoch_id: 42,
        issued_at_unix: 1_700_000_000,
        response_window_secs: 900,
        drand_round: 12345,
        drand_randomness: [0x33; 32],
        drand_signature: [0x44; 48],
    };

    let plans = handle
        .plan_por_challenges(randomness.clone(), &HashMap::new())
        .expect("forced challenge");
    assert_eq!(plans.len(), 1);
    let forced = &plans[0].challenge;
    assert!(forced.forced);
    assert!(forced.vrf_output.is_none());
    assert!(forced.sample_count > 0);
    assert_eq!(forced.sample_count, 128);

    let mut inert_randomness = randomness.clone();
    inert_randomness.drand_signature = [0; 48];
    assert!(matches!(
        handle.plan_por_challenges(inert_randomness, &HashMap::new()),
        Err(PorChallengePlannerError::InvalidDrandSignature)
    ));

    let mut vrf_records = HashMap::new();
    vrf_records.insert(
        ManifestVrfKey {
            provider_id: forced.provider_id,
            manifest_digest: forced.manifest_digest,
        },
        ManifestVrfBundle {
            provider_id: forced.provider_id,
            manifest_digest: forced.manifest_digest,
            epoch_id: randomness.epoch_id,
            drand_round: randomness.drand_round,
            output: [0x55; 32],
            proof: iroha_crypto::vrf::VrfProof::SigInG1([0x66; 48]),
        },
    );

    let plans_with_vrf = handle
        .plan_por_challenges(randomness.clone(), &vrf_records)
        .expect("vrf-backed challenge");
    let satisfied = &plans_with_vrf[0].challenge;
    assert!(!satisfied.forced);
    assert_eq!(satisfied.vrf_output, Some([0x55; 32]));
    assert_eq!(satisfied.sample_count, 128);
    assert!(matches!(
        satisfied.vrf_proof,
        Some(iroha_crypto::vrf::VrfProof::SigInG1(_))
    ));

    assert!(matches!(
        handle.plan_por_challenges_with_forced_policy(randomness.clone(), &HashMap::new(), false,),
        Err(PorChallengePlannerError::MissingVrfBeforeDeadline { .. })
    ));
}

#[test]
fn node_handle_plan_por_challenges_skips_expired_manifest() {
    let (cfg, _dir) = storage_config_with_temp_dir();
    let gc_actual = iroha_config::parameters::actual::SorafsGc {
        retention_grace_secs: 0,
        ..Default::default()
    };
    let handle =
        NodeHandle::new_with_policies(cfg, RepairConfig::default(), GcConfig::from(&gc_actual));

    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id: [0x22; 32],
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xAA; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 128,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 128,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 128,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 2,
        metadata: vec![],
    };
    let payload = to_bytes(&declaration).expect("encode declaration");
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(declaration.provider_id),
        payload,
        declaration.committed_capacity_gib,
        1,
        1,
        2,
        Metadata::default(),
    );
    handle
        .record_capacity_declaration(&record)
        .expect("record declaration");

    let now_unix = 1_700_000_000;
    let expired_manifest = build_manifest_with_retention(
        vec![0x01; 8],
        now_unix - 10,
        b"expired-por-manifest",
        &handle,
    );
    let active_manifest = build_manifest_with_retention(
        vec![0x02; 8],
        now_unix + 86_400,
        b"active-por-manifest",
        &handle,
    );

    let randomness = PorRandomness {
        epoch_id: 7,
        issued_at_unix: now_unix,
        response_window_secs: 900,
        drand_round: 777,
        drand_randomness: [0x55; 32],
        drand_signature: [0x66; 48],
    };

    let plans = handle
        .plan_por_challenges(randomness, &HashMap::new())
        .expect("plan por");
    assert_eq!(plans.len(), 1);
    assert_eq!(plans[0].challenge.manifest_digest, active_manifest);
    assert_ne!(plans[0].challenge.manifest_digest, expired_manifest);
}

fn build_manifest_with_retention(
    fixture_id: Vec<u8>,
    retention_epoch: u64,
    payload: &[u8],
    handle: &NodeHandle,
) -> [u8; 32] {
    let plan = CarBuildPlan::single_file(payload).expect("plan");
    let mut policy = PinPolicy::default();
    policy.retention_epoch = retention_epoch;
    let manifest = manifest_builder_for_plan(payload, &plan)
        .add_metadata("test.fixture_id", hex::encode(fixture_id))
        .pin_policy(policy)
        .build()
        .expect("manifest");
    let mut reader = payload;
    handle
        .ingest_manifest(&manifest, &plan, &mut reader)
        .expect("ingest");
    manifest.digest().expect("digest").into()
}

#[test]
fn finalized_native_repair_rejects_stale_leases_and_deduplicates_after_restart() {
    use crate::{
        native_repair_worker::{NativeRepairExecutionErrorV1, NativeRepairTerminalKindV1},
        repair_transaction_forwarder::{
            RepairOperationV1, RepairTransactionContextV1, RepairTransactionEnqueueResultV1,
        },
    };
    use iroha_data_model::{
        ChainId,
        isi::sorafs::SorafsRepairTaskActionV1,
        sorafs::moderation_ledger::{
            REPAIR_LEDGER_TASK_VERSION_V1, RepairFinalizedCursorV1, RepairFinalizedTaskV1,
            RepairLedgerActionReceiptV1, RepairLedgerLeaseV1, RepairLedgerTaskV1,
            sorafs_repair_task_id_v1,
        },
    };

    let (cfg, _dir) = storage_config_with_temp_dir();
    let repair_actual = iroha_config::parameters::actual::SorafsRepair {
        enabled: true,
        ..Default::default()
    };
    let repair_config = RepairConfig::from(&repair_actual);
    let handle =
        NodeHandle::new_with_policies(cfg.clone(), repair_config.clone(), GcConfig::default());
    let provider_id = [0xC1; 32];
    let declaration = CapacityDeclarationV1 {
        version: CAPACITY_DECLARATION_VERSION_V1,
        provider_id,
        stake: sorafs_manifest::provider_advert::StakePointer {
            pool_id: [0xC2; 32],
            stake_amount: xor("1"),
        },
        committed_capacity_gib: 100,
        chunker_commitments: vec![ChunkerCommitmentV1 {
            profile_id: "sorafs.sf1@1.0.0".into(),
            profile_aliases: None,
            committed_gib: 100,
            capability_refs: Vec::new(),
        }],
        lane_commitments: vec![LaneCommitmentV1 {
            lane_id: "default".into(),
            max_gib: 100,
        }],
        pricing: None,
        valid_from: 1,
        valid_until: 10,
        metadata: Vec::new(),
    };
    let record = CapacityDeclarationRecord::new(
        ProviderId::new(provider_id),
        to_bytes(&declaration).expect("encode capacity declaration"),
        declaration.committed_capacity_gib,
        1,
        declaration.valid_from,
        declaration.valid_until,
        Metadata::default(),
    );
    handle
        .record_capacity_declaration(&record)
        .expect("bind local provider");

    let payload = b"finalized-native-repair-corrupt-chunk";
    let plan = CarBuildPlan::single_file(payload).expect("chunk plan");
    let build_manifest = |fixture_id: Vec<u8>| {
        manifest_builder_for_plan(payload, &plan)
            .add_metadata("test.fixture_id", hex::encode(fixture_id))
            .pin_policy(PinPolicy::default())
            .build()
            .expect("manifest")
    };
    let target_manifest = build_manifest(vec![0xC3; 16]);
    let source_manifest = build_manifest(vec![0xC4; 16]);
    let mut reader = payload.as_slice();
    handle
        .ingest_manifest(&target_manifest, &plan, &mut reader)
        .expect("ingest target manifest");
    let mut reader = payload.as_slice();
    handle
        .ingest_manifest(&source_manifest, &plan, &mut reader)
        .expect("ingest source manifest");
    let target_digest: [u8; 32] = target_manifest.digest().expect("target digest").into();
    let source_digest: [u8; 32] = source_manifest.digest().expect("source digest").into();
    let target = handle
        .manifest_metadata_by_digest(&target_digest)
        .expect("target metadata")
        .chunk(0)
        .expect("target chunk")
        .clone();
    let source = handle
        .manifest_metadata_by_digest(&source_digest)
        .expect("source metadata")
        .chunk(0)
        .expect("source chunk")
        .clone();
    assert_eq!(target.digest, source.digest);
    assert_ne!(target.path, source.path);
    let corrupt = vec![0xA5; target.length as usize];
    std::fs::write(&target.path, &corrupt).expect("corrupt target chunk");

    let authority_key =
        KeyPair::try_from_seed(vec![0xC5; 32], Algorithm::Ed25519).expect("authority key");
    let authority = AccountId::new(authority_key.public_key().clone());
    let other_key = KeyPair::try_from_seed(vec![0xC6; 32], Algorithm::Ed25519).expect("other key");
    let other = AccountId::new(other_key.public_key().clone());
    let ticket_id = RepairTicketId("REP-NATIVE-FINALIZED-1".to_owned());
    let report = RepairReportV1 {
        version: REPAIR_REPORT_VERSION_V1,
        ticket_id: ticket_id.clone(),
        auditor_account: authority.to_string(),
        submitted_at_unix: 1,
        evidence: RepairEvidenceV1 {
            version: REPAIR_EVIDENCE_VERSION_V1,
            manifest_digest: target_digest,
            provider_id,
            por_history_id: None,
            cause: RepairCauseV1::Manual(RepairManualCauseV1 {
                reason: "corrupt chunk".to_owned(),
            }),
            evidence_json: None,
            notes: None,
        },
        notes: None,
    };
    let source_identity = [0xC7; 32];
    let finalized_cursor = RepairFinalizedCursorV1 {
        height: 7,
        block_hash: [0xC8; 32],
    };
    let finalized_task = RepairFinalizedTaskV1 {
        finalized_cursor,
        task: RepairLedgerTaskV1 {
            version: REPAIR_LEDGER_TASK_VERSION_V1,
            task_id: sorafs_repair_task_id_v1(source_identity),
            source_identity,
            ticket_id: ticket_id.0.clone(),
            canonical_report: to_bytes(&report).expect("encode canonical report"),
            manifest_digest: target_digest,
            provider_id,
            submitted_by: authority.clone(),
            submitted_at_unix_ms: 1_000,
            revision: 2,
            lease: Some(RepairLedgerLeaseV1 {
                owner: authority.clone(),
                generation: 1,
                acquired_at_unix_ms: 1_000,
                renewed_at_unix_ms: 1_000,
                expires_at_unix_ms: 60_000,
            }),
            terminal_outcome: None,
            slash: None,
            appeal: None,
            action_receipts: vec![RepairLedgerActionReceiptV1 {
                idempotency_digest: [0xD1; 32],
                action_digest: [0xD2; 32],
                resulting_revision: 2,
            }],
            updated_at_unix_ms: 1_000,
        },
    };
    let context = RepairTransactionContextV1 {
        chain_id: ChainId::from("native-repair-test-chain"),
        finalized_cursor,
    };
    let stale_context = RepairTransactionContextV1 {
        chain_id: context.chain_id.clone(),
        finalized_cursor: RepairFinalizedCursorV1 {
            height: 8,
            block_hash: [0xC9; 32],
        },
    };
    assert!(matches!(
        handle.execute_finalized_native_repair(&finalized_task, &authority, &stale_context, 2_000,),
        Err(NativeRepairExecutionErrorV1::StaleFinalizedCursor)
    ));
    assert_eq!(
        std::fs::read(&target.path).expect("read corrupt target"),
        corrupt
    );
    assert!(
        handle
            .pending_repair_transactions_after(None, 8)
            .expect("empty forwarder")
            .is_empty()
    );
    assert!(matches!(
        handle.execute_finalized_native_repair(&finalized_task, &other, &context, 2_000,),
        Err(NativeRepairExecutionErrorV1::LeaseOwnerMismatch)
    ));
    assert_eq!(
        std::fs::read(&target.path).expect("read corrupt target"),
        corrupt
    );
    let mut malformed_task = finalized_task.clone();
    malformed_task.task.action_receipts[0].resulting_revision = 3;
    assert!(matches!(
        handle.execute_finalized_native_repair(&malformed_task, &authority, &context, 2_000,),
        Err(NativeRepairExecutionErrorV1::InvalidFinalizedTask)
    ));
    assert_eq!(
        std::fs::read(&target.path).expect("malformed task performs no storage I/O"),
        corrupt
    );

    std::fs::write(&source.path, &corrupt).expect("make every local replica invalid");
    let orchestrator_calls = Arc::new(AtomicUsize::new(0));
    handle.set_repair_orchestrator(Arc::new(FailingRepairOrchestrator {
        calls: Arc::clone(&orchestrator_calls),
    }));
    assert!(matches!(
        handle.execute_finalized_native_repair(&finalized_task, &authority, &context, 2_000,),
        Err(NativeRepairExecutionErrorV1::Orchestrator(_))
    ));
    assert_eq!(orchestrator_calls.load(Ordering::Relaxed), 1);
    assert!(
        handle
            .pending_repair_transactions_after(None, 8)
            .expect("transient orchestrator failure enqueues no terminal action")
            .is_empty()
    );
    assert_eq!(
        std::fs::read(&target.path).expect("orchestrator failure leaves target retryable"),
        corrupt
    );
    handle.clear_repair_orchestrator();
    std::fs::write(&source.path, payload).expect("restore a valid local source replica");

    let first = handle
        .execute_finalized_native_repair(&finalized_task, &authority, &context, 2_000)
        .expect("execute exact finalized native lease");
    assert!(matches!(
        first.enqueue_result,
        RepairTransactionEnqueueResultV1::Inserted { .. }
    ));
    assert!(matches!(
        first.terminal_kind,
        NativeRepairTerminalKindV1::Complete { .. }
    ));
    assert_eq!(first.invalid_chunks_before, 1);
    assert_eq!(first.invalid_chunks_after, 0);
    assert_eq!(
        blake3::hash(&std::fs::read(&target.path).expect("read restored target")).as_bytes(),
        &target.digest
    );

    let replay = handle
        .execute_finalized_native_repair(&finalized_task, &authority, &context, 2_001)
        .expect("deduplicate exact terminal operation");
    assert_eq!(replay.operation_id, first.operation_id);
    assert!(matches!(
        replay.enqueue_result,
        RepairTransactionEnqueueResultV1::Existing { .. }
    ));
    let request = handle
        .repair_transaction_operation_for_reconciliation(first.operation_id)
        .expect("read exact native terminal operation");
    assert_eq!(request.chain_id, context.chain_id);
    assert_eq!(request.authority, authority);
    assert!(matches!(
        request.operation,
        RepairOperationV1::Action(ref instruction)
            if matches!(
                instruction.action(),
                SorafsRepairTaskActionV1::Complete(action)
                    if action.lease_generation == 1
            )
    ));
    drop(handle);

    let restored = NodeHandle::new_with_policies(cfg, repair_config, GcConfig::default());
    let pending = restored
        .pending_repair_transactions_after(None, 8)
        .expect("restore durable native terminal operation");
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].operation_id, first.operation_id);
    assert_eq!(pending[0].chain_id, context.chain_id);
}
