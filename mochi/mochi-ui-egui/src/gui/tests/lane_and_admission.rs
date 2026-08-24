#[test]
fn lane_slug_matches_supervisor_logic() {
    assert_eq!(lane_slug("Core Lane", 0), "core_lane");
    assert_eq!(lane_slug("Gov+Ops", 1), "gov_ops");
    assert_eq!(lane_slug("---", 3), "lane3");
}
#[test]
fn lane_catalog_snapshot_resolves_aliases_and_dataspaces() {
    let mut nexus = TomlTable::new();
    nexus.insert("lane_count".into(), TomlValue::Integer(2));
    let mut lane0 = TomlTable::new();
    lane0.insert("index".into(), TomlValue::Integer(0));
    lane0.insert("alias".into(), TomlValue::String("core".into()));
    lane0.insert("dataspace".into(), TomlValue::String("universal".into()));
    let mut lane1 = TomlTable::new();
    lane1.insert("index".into(), TomlValue::Integer(1));
    lane1.insert("alias".into(), TomlValue::String("ops".into()));
    lane1.insert("dataspace_id".into(), TomlValue::Integer(3));
    nexus.insert(
        "lane_catalog".into(),
        TomlValue::Array(vec![TomlValue::Table(lane0), TomlValue::Table(lane1)]),
    );
    let mut global = TomlTable::new();
    global.insert("alias".into(), TomlValue::String("universal".into()));
    global.insert("id".into(), TomlValue::Integer(0));
    let mut private = TomlTable::new();
    private.insert("alias".into(), TomlValue::String("private".into()));
    private.insert("id".into(), TomlValue::Integer(3));
    nexus.insert(
        "dataspace_catalog".into(),
        TomlValue::Array(vec![TomlValue::Table(global), TomlValue::Table(private)]),
    );
    let snapshot = lane_catalog_snapshot(Some(&nexus));
    assert_eq!(snapshot.lane_alias(0), "core");
    assert_eq!(snapshot.lane_alias(1), "ops");
    assert_eq!(
        snapshot.dataspace_label(snapshot.lane_dataspace_id(1)),
        "private"
    );
}
#[test]
fn lane_metadata_for_id_reads_lane_fields() {
    let mut nexus = TomlTable::new();
    let mut lane = TomlTable::new();
    lane.insert("index".into(), TomlValue::Integer(2));
    lane.insert("alias".into(), TomlValue::String("alpha".into()));
    lane.insert("dataspace".into(), TomlValue::String("universal".into()));
    lane.insert("visibility".into(), TomlValue::String("restricted".into()));
    lane.insert(
        "storage".into(),
        TomlValue::String("commitment_only".into()),
    );
    lane.insert(
        "proof_scheme".into(),
        TomlValue::String("merkle_sha256".into()),
    );
    lane.insert("governance".into(), TomlValue::String("parliament".into()));
    let mut metadata = TomlTable::new();
    metadata.insert("tier".into(), TomlValue::String("gold".into()));
    lane.insert("metadata".into(), TomlValue::Table(metadata));
    nexus.insert(
        "lane_catalog".into(),
        TomlValue::Array(vec![TomlValue::Table(lane)]),
    );
    let mut dataspace = TomlTable::new();
    dataspace.insert("alias".into(), TomlValue::String("universal".into()));
    dataspace.insert("id".into(), TomlValue::Integer(0));
    nexus.insert(
        "dataspace_catalog".into(),
        TomlValue::Array(vec![TomlValue::Table(dataspace)]),
    );
    let metadata = MochiApp::lane_metadata_for_id(Some(&nexus), 2);
    assert_eq!(metadata.id, LaneId::new(2));
    assert_eq!(metadata.alias, "alpha");
    assert_eq!(metadata.dataspace_id, DataSpaceId::new(0));
    assert_eq!(metadata.visibility, LaneVisibility::Restricted);
    assert_eq!(metadata.storage, LaneStorageProfile::CommitmentOnly);
    assert_eq!(metadata.proof_scheme, DaProofScheme::MerkleSha256);
    assert_eq!(metadata.governance.as_deref(), Some("parliament"));
    assert_eq!(
        metadata.metadata.get("tier").map(String::as_str),
        Some("gold")
    );
}
#[test]
fn lane_path_previews_include_slugged_paths() {
    if !super::socket_bind_available() {
        eprintln!("Skipping lane preview test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_preview_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("lane-preview-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let app = MochiApp::default();
    let selected_storage = app
        .supervisor
        .as_ref()
        .and_then(|supervisor| supervisor.peers().first())
        .map(|peer| peer.storage_dir().to_path_buf())
        .expect("selected peer storage");
    let mut lane = TomlTable::new();
    lane.insert("index".into(), TomlValue::Integer(0));
    lane.insert("alias".into(), TomlValue::String("Core Lane".into()));
    let lane_catalog = vec![TomlValue::Table(lane)];
    let previews = app
        .lane_path_previews(Some(1), Some(&lane_catalog))
        .expect("previews");
    assert_eq!(previews.len(), 1);
    let preview = &previews[0];
    assert!(
        preview
            .blocks_dir
            .starts_with(selected_storage.join("kura"))
    );
    assert!(preview.merge_log.starts_with(selected_storage.join("kura")));
    let blocks = preview.blocks_dir.to_string_lossy();
    let merge = preview.merge_log.to_string_lossy();
    assert!(blocks.contains("lane_000_core_lane"));
    assert!(merge.contains("lane_000_core_lane_merge.log"));
}
#[test]
fn lane_path_previews_without_supervisor_use_validated_selected_storage() {
    if !super::socket_bind_available() {
        eprintln!("Skipping detached lane preview test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_detached_preview_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("detached-lane-preview-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    let expected_storage = app.supervisor.as_ref().expect("supervisor ready").peers()[0]
        .storage_dir()
        .to_path_buf();
    app.supervisor = None;
    let previews = app
        .lane_path_previews(Some(1), None)
        .expect("validated detached previews");
    assert_eq!(previews.len(), 1);
    assert!(
        previews[0]
            .blocks_dir
            .starts_with(expected_storage.join("kura"))
    );
}
#[test]
fn detached_lane_path_previews_retain_selection_lease_until_drop() {
    if !super::socket_bind_available() {
        eprintln!("Skipping detached lane lease test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_detached_lease_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("detached-lane-lease-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    app.supervisor = None;
    let previews = app
        .lane_path_previews(Some(1), None)
        .expect("validated detached previews");
    let error = SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(&data_root)
        .build()
        .err()
        .expect("detached previews must retain the shared selection lease");
    assert!(matches!(error, SupervisorError::GenerationLocked { .. }));
    drop(previews);
    SupervisorBuilder::new(ProfilePreset::FourPeerBft)
        .data_root(&data_root)
        .build()
        .expect("dropping detached previews releases the writer lock");
}
#[test]
fn lane_path_previews_without_supervisor_reject_tampered_selection() {
    if !super::socket_bind_available() {
        eprintln!("Skipping tampered lane preview test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_tampered_preview_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("tampered-lane-preview-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    let supervisor = app.supervisor.as_ref().expect("supervisor ready");
    let inventory = supervisor
        .paths()
        .root()
        .join("generations")
        .join(supervisor.generation_id())
        .join("generation.json");
    fs::write(&inventory, b"tampered-inventory").expect("tamper selected inventory");
    app.supervisor = None;
    assert!(
        app.lane_path_previews(Some(1), None).is_none(),
        "detached preview must fail closed instead of guessing storage"
    );
}
#[test]
fn reset_lane_lifecycle_plan_builds_consensus_replacement() {
    if !super::socket_bind_available() {
        eprintln!("Skipping lane reset plan test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_lane_reset_inner_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("lane-reset-inner-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let app = MochiApp::default();
    let supervisor = app.supervisor.as_ref().expect("supervisor ready");
    let plan = MochiApp::lane_reset_lifecycle_plan(supervisor, 0);
    assert_eq!(plan.additions.len(), 1);
    assert_eq!(plan.additions[0].id, LaneId::new(0));
    assert_eq!(plan.retire, vec![LaneId::new(0)]);
}
#[test]
fn parse_min_initial_amounts_parses_lines() {
    let rose = sample_rose_definition_id();
    let cabbage = sample_cabbage_definition_id();
    let raw = format!("{rose} = 5\n{cabbage} = 1");
    let parsed = MochiApp::parse_min_initial_amounts(&raw).expect("amounts parse");
    assert_eq!(parsed.len(), 2);
    assert_eq!(parsed.get(&rose), Some(&"5".parse::<Quantity>().unwrap()));
    assert_eq!(
        parsed.get(&cabbage),
        Some(&"1".parse::<Quantity>().unwrap())
    );
}
#[test]
fn parse_min_initial_amounts_rejects_negative_values() {
    let rose = sample_rose_definition_id();
    let error = MochiApp::parse_min_initial_amounts(&format!("{rose} = -1"))
        .expect_err("negative admission minimum must be rejected");
    assert!(error.contains("Invalid amount"));
}
#[test]
fn parse_multisig_policy_parses_json() {
    let account = account_literal(&ALICE_ID);
    let json = format!(
        r#"{{
  "signatories": {{
    "{account}": 1
  }},
  "quorum": 1,
  "transaction_ttl_ms": 3600000
}}"#
    );
    let spec = MochiApp::parse_multisig_policy(&json).expect("policy should parse");
    assert!(spec.signatories.contains_key(&*ALICE_ID));
    assert_eq!(spec.quorum.get(), 1);
    assert_eq!(spec.transaction_ttl_ms.get(), 3_600_000);
}
#[test]
fn admission_mode_label_matches_variants() {
    assert_eq!(
        MochiApp::admission_mode_label(AccountAdmissionMode::ImplicitReceive),
        "Implicit receive"
    );
    assert_eq!(
        MochiApp::admission_mode_label(AccountAdmissionMode::ExplicitOnly),
        "Explicit only"
    );
}
#[test]
fn account_admission_policy_requires_set_parameters_permission() {
    assert_eq!(
        ComposerInstructionKind::AccountAdmissionPolicy.permission(),
        InstructionPermission::SetParameters
    );
}
#[test]
fn parse_account_admission_policy_builds_policy() {
    let mut app = MochiApp::default();
    app.composer_admission_mode = AccountAdmissionMode::ImplicitReceive;
    app.composer_admission_max_per_tx = "2".to_owned();
    app.composer_admission_max_per_block = "5".to_owned();
    app.composer_admission_fee_enabled = true;
    app.composer_admission_fee_asset = sample_rose_definition_literal();
    app.composer_admission_fee_amount = "1".to_owned();
    app.composer_admission_fee_destination_burn = false;
    app.composer_admission_fee_destination_account = account_literal(&ALICE_ID);
    app.composer_admission_min_initial_amounts = format!("{} = 5", sample_rose_definition_id());
    app.composer_admission_default_role = "basic_user".to_owned();
    let policy = app
        .parse_account_admission_policy()
        .expect("policy should parse");
    assert_eq!(policy.mode, AccountAdmissionMode::ImplicitReceive);
    assert_eq!(policy.max_implicit_creations_per_tx, Some(2));
    assert_eq!(policy.max_implicit_creations_per_block, Some(5));
    let fee = policy.implicit_creation_fee.expect("fee configured");
    let asset = sample_rose_definition_id();
    assert_eq!(fee.asset_definition_id, asset);
    assert_eq!(fee.amount, "1".parse::<Quantity>().unwrap());
    let treasury = ALICE_ID.clone();
    match fee.destination {
        ImplicitAccountFeeDestination::Account(account) => assert_eq!(account, treasury),
        other => panic!("unexpected fee destination: {other:?}"),
    }
    let min_amounts = policy.min_initial_amounts;
    assert_eq!(min_amounts.len(), 1);
    let min_asset = sample_rose_definition_id();
    assert_eq!(
        min_amounts.get(&min_asset),
        Some(&"5".parse::<Quantity>().unwrap())
    );
    let expected_role: RoleId = "basic_user".parse().unwrap();
    assert_eq!(policy.default_role_on_create, Some(expected_role));
}
#[test]
fn reject_code_hint_covers_queue_and_axt() {
    assert_eq!(
        super::reject_code_hint("PRTRY:QUEUE_FULL"),
        Some("transaction queue full")
    );
    assert_eq!(
        super::reject_code_hint("PRTRY:AXT_HANDLE_ERA"),
        Some("AXT policy rejected")
    );
}
