test_items! {
fn direct_mode_enable_renders_snippet() {
    let plan = direct_mode_enable_test_plan(direct_mode_enable_capabilities());
    let plan_file = write_direct_mode_plan(&plan);
    let args = GatewayDirectModeEnableArgs {
        plan: plan_file.path().to_path_buf(),
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("enable command runs");
    assert_eq!(ctx.outputs().len(), 1);
    let snippet = &ctx.outputs()[0];
    assert!(snippet.contains("require_manifest_envelope = true"));
    assert!(snippet.contains("enforce_admission = true"));
    assert!(snippet.contains("enforce_capabilities = true"));
    assert!(!snippet.contains(" = false"));
    assert!(snippet.contains("direct_car_canonical"));
    assert!(snippet.contains(&plan.provider_id_hex));
    assert!(snippet.contains("[sorafs.gateway.direct_mode]"));
    assert!(!snippet.contains("[torii.sorafs_gateway]"));
    assert_sorafs_config_snippet_is_schema_valid(snippet);
}
fn direct_mode_enable_rejects_missing_direct_car_capability() {
    let plan = direct_mode_enable_test_plan(ManifestCapabilitySummary::default());
    let plan_file = write_direct_mode_plan(&plan);
    let args = GatewayDirectModeEnableArgs {
        plan: plan_file.path().to_path_buf(),
    };
    let mut ctx = TestContext::new();
    let err = args
        .run(&mut ctx)
        .expect_err("missing direct-CAR capability must fail");
    assert!(format!("{err:#}").contains("capabilities.direct_car_supported=true"));
    assert!(ctx.outputs().is_empty());
}
fn direct_mode_enable_rejects_manifest_envelope_disabled() {
    let capabilities = ManifestCapabilitySummary {
        requires_manifest_envelope: false,
        direct_car_supported: true,
        ..ManifestCapabilitySummary::default()
    };
    let plan = direct_mode_enable_test_plan(capabilities);
    let plan_file = write_direct_mode_plan(&plan);
    let args = GatewayDirectModeEnableArgs {
        plan: plan_file.path().to_path_buf(),
    };
    let mut ctx = TestContext::new();
    let err = args
        .run(&mut ctx)
        .expect_err("disabled envelope enforcement must fail");
    assert!(format!("{err:#}").contains("requires_manifest_envelope=true"));
    assert!(ctx.outputs().is_empty());
}
fn direct_mode_enable_rejects_tampered_direct_car_locator() {
    let mut plan = direct_mode_enable_test_plan(direct_mode_enable_capabilities());
    plan.direct_car.canonical_url = format!(
        "https://evil.example/direct/v1/car/{}",
        plan.manifest_digest_hex
    );
    let plan_file = write_direct_mode_plan(&plan);
    let args = GatewayDirectModeEnableArgs {
        plan: plan_file.path().to_path_buf(),
    };
    let mut ctx = TestContext::new();
    let err = args
        .run(&mut ctx)
        .expect_err("tampered direct-CAR locator must fail");
    assert!(format!("{err:#}").contains("direct_car.canonical_url mismatch"));
    assert!(ctx.outputs().is_empty());
}
fn direct_mode_toml_string_escape_blocks_config_injection() {
    assert_eq_compact! { escape_toml_basic_string("nexus\"\nenforce_admission = false\\") => "nexus\\\"\\nenforce_admission = false\\\\" };
}
fn direct_mode_rollback_snippet_matches_defaults() {
    let args = GatewayDirectModeRollbackArgs;
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("rollback command runs");
    assert_eq_compact! { ctx.outputs() => &[render_direct_mode_rollback_snippet().to_owned()] };
    let snippet = &ctx.outputs()[0];
    assert!(snippet.contains("[sorafs.gateway]"));
    assert!(!snippet.contains("[torii.sorafs_gateway]"));
    assert_sorafs_config_snippet_is_schema_valid(snippet);
}
fn gateway_route_plan_writes_plan_and_headers() {
    use base64::engine::general_purpose::STANDARD as BASE64;
    use tempfile::TempDir;
    let tmp = TempDir::new().expect("temp dir");
    let manifest_path = tmp.path().join("manifest.json");
    fs::write(&manifest_path, r#"{"root_cid":[1,2,3]}"#).expect("write manifest");
    let output_path = tmp.path().join("route_plan.json");
    let headers_path = tmp.path().join("gateway.route.headers.txt");
    let args = GatewayRoutePlanArgs {
        manifest_json: manifest_path.clone(),
        hostname: "docs.sora.link".to_owned(),
        alias: Some("sora:docs".to_owned()),
        route_label: Some("docs@2026-03-21".to_owned()),
        proof_status: None,
        release_tag: Some("v2026.03.21".to_owned()),
        cutover_window: Some("2026-03-21T15:00Z/2026-03-21T15:30Z".to_owned()),
        output_path: output_path.clone(),
        headers_out: Some(headers_path.clone()),
        rollback_manifest_json: None,
        rollback_headers_out: None,
        rollback_route_label: None,
        rollback_release_tag: None,
        no_csp: false,
        no_permissions_policy: false,
        no_hsts: false,
        now_override: Some("2026-03-21T10:00:00Z".to_owned()),
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx)
        .expect("route plan command should succeed");
    let plan_bytes = fs::read(&output_path).expect("route plan json");
    let plan: Value =
        norito::json::from_slice(&plan_bytes).expect("route plan JSON should parse");
    assert_eq_compact! { plan["manifest_json"].as_str().expect("manifest string") => manifest_path.display().to_string() };
    assert_eq_compact! { plan["hostname"].as_str().expect("hostname string") => "docs.sora.link" };
    let headers = plan["headers"].as_object().expect("headers object missing");
    assert_eq_compact! { headers["Sora-Name"].as_str().expect("Sora-Name must exist") => "sora:docs" };
    let proof_json = BASE64
        .decode(headers["Sora-Proof"].as_str().expect("Sora-Proof base64"))
        .expect("decode proof payload");
    let proof_value: Value =
        norito::json::from_slice(&proof_json).expect("decode proof payload JSON");
    assert_eq_compact! { proof_value["alias"].as_str().expect("alias string") => "sora:docs" };
    assert_compact! { plan["headers_template"].as_str().expect("headers template string").contains("Sora-Route-Binding"); "expected rendered header template" };
    let header_file = fs::read_to_string(&headers_path).expect("header template");
    assert!(header_file.contains("Sora-Content-CID"));
    assert_compact! { ctx.outputs().iter().any(|line| line.contains(output_path.to_string_lossy().as_ref())) };
}
fn gateway_route_plan_supports_rollback_and_toggles() {
    use tempfile::TempDir;
    let tmp = TempDir::new().expect("temp dir");
    let manifest_path = tmp.path().join("manifest.json");
    let rollback_path = tmp.path().join("rollback.json");
    fs::write(&manifest_path, r#"{"root_cid_hex":"0102"}"#).expect("write manifest");
    fs::write(&rollback_path, r#"{"root_cid":[240,5]}"#).expect("write rollback manifest");
    let output_path = tmp.path().join("route_plan.json");
    let headers_path = tmp.path().join("gateway.route.headers.txt");
    let rollback_headers_path = tmp.path().join("gateway.route.rollback.headers.txt");
    let args = GatewayRoutePlanArgs {
        manifest_json: manifest_path.clone(),
        hostname: "nexus.sora.link".to_owned(),
        alias: None,
        route_label: None,
        proof_status: None,
        release_tag: None,
        cutover_window: None,
        output_path: output_path.clone(),
        headers_out: Some(headers_path.clone()),
        rollback_manifest_json: Some(rollback_path.clone()),
        rollback_headers_out: Some(rollback_headers_path.clone()),
        rollback_route_label: Some("docs@previous".to_owned()),
        rollback_release_tag: Some("previous".to_owned()),
        no_csp: true,
        no_permissions_policy: true,
        no_hsts: true,
        now_override: Some("2026-03-21T10:00:00Z".to_owned()),
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx)
        .expect("route plan command should succeed");
    let plan_bytes = fs::read(&output_path).expect("route plan json");
    let plan: Value =
        norito::json::from_slice(&plan_bytes).expect("route plan JSON should parse");
    assert!(plan["headers"].get("Content-Security-Policy").is_none());
    assert!(plan["headers"].get("Permissions-Policy").is_none());
    assert!(plan["headers"].get("Strict-Transport-Security").is_none());
    let rollback = plan["rollback"]
        .as_object()
        .expect("rollback object missing");
    assert_eq_compact! { rollback["manifest_json"].as_str().expect("rollback manifest") => rollback_path.display().to_string() };
    assert_eq_compact! { rollback["release_tag"].as_str().expect("release tag") => "previous" };
    assert_compact! { rollback.get("headers_path").and_then(Value::as_str).is_some_and(|value| value.contains("gateway.route.rollback.headers.txt")) };
    let header_file = fs::read_to_string(&headers_path).expect("header template");
    assert_compact! { !header_file.contains("Content-Security-Policy"); "CSP header should be omitted when --no-csp is set" };
    let rollback_headers =
        fs::read_to_string(&rollback_headers_path).expect("rollback header template");
    assert_compact! { rollback_headers.contains("Sora-Route-Binding"); "rollback template should include Sora-Route-Binding" };
    assert_compact! { ctx.outputs().iter().any(|line| line.contains("rollback headers written")); "expected rollback output message" };
}
fn gateway_cache_invalidate_prints_payload_and_curl() {
    let args = GatewayCacheInvalidateArgs {
        endpoint: "https://cache.example.com/purge".to_owned(),
        aliases: vec!["docs:portal".to_owned()],
        manifest_digest_hex: "AA".repeat(32),
        car_digest_hex: None,
        release_tag: Some("portal-2026.04.01".to_owned()),
        auth_env: "CACHE_TOKEN".to_owned(),
        output: None,
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("cache invalidate command runs");
    assert_eq!(ctx.outputs().len(), 2);
    let payload: Value = norito::json::from_str(&ctx.outputs()[0]).expect("json payload");
    assert_eq_compact! { payload["aliases"] => Value::Array(vec![Value::from("docs:portal")]) };
    assert_eq!(payload["manifest_digest_hex"], Value::from("aa".repeat(32)));
    assert_eq!(payload["release_tag"], Value::from("portal-2026.04.01"));
    assert_eq!(payload["car_digest_hex"], Value::Null);
    let curl = &ctx.outputs()[1];
    assert_compact! { curl.contains("https://cache.example.com/purge"); "curl snippet should reference endpoint" };
    assert_compact! { curl.contains("Authorization: Bearer $CACHE_TOKEN"); "curl snippet should reference the auth env var" };
}
fn gateway_cache_invalidate_writes_payload_file() {
    let temp_payload = NamedTempFile::new().expect("temp payload file");
    let path = temp_payload.into_temp_path();
    let args = GatewayCacheInvalidateArgs {
        endpoint: "https://cache.example.com/purge".to_owned(),
        aliases: vec!["docs:portal".to_owned(), "sns:preview".to_owned()],
        manifest_digest_hex: "bb".repeat(32),
        car_digest_hex: Some("cc".repeat(32)),
        release_tag: None,
        auth_env: String::new(),
        output: Some(path.to_path_buf()),
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("cache invalidate command runs");
    assert_eq!(ctx.outputs().len(), 2);
    assert_eq_compact! { ctx.outputs()[0] => format!("wrote cache invalidation payload to {}", path.display()) };
    let payload_str = std::fs::read_to_string(&path).expect("read payload");
    let payload: Value = norito::json::from_str(&payload_str).expect("json payload");
    assert_eq!(payload["release_tag"], Value::Null);
    assert_eq!(payload["car_digest_hex"], Value::from("cc".repeat(32)));
    let curl = &ctx.outputs()[1];
    assert_compact! { curl.contains("--data '{"); "curl snippet should embed the JSON payload" };
}
fn gateway_cache_invalidate_rejects_invalid_alias() {
    let args = GatewayCacheInvalidateArgs {
        endpoint: "https://cache.example.com/purge".to_owned(),
        aliases: vec!["invalid-alias".to_owned()],
        manifest_digest_hex: "aa".repeat(32),
        car_digest_hex: None,
        release_tag: None,
        auth_env: "CACHE_TOKEN".to_owned(),
        output: None,
    };
    let mut ctx = TestContext::new();
    let result = args.run(&mut ctx);
    assert!(result.is_err(), "invalid alias should fail");
}
fn incentives_compute_generates_instruction() {
    let mut config_file = NamedTempFile::new().expect("config file");
    config_file
        .write_all(
            &norito::json::to_vec(&sample_reward_config_json()).expect("serialize config"),
        )
        .expect("write config");
    let metrics = sample_metrics();
    let mut metrics_file = NamedTempFile::new().expect("metrics file");
    metrics_file
        .write_all(&to_bytes(&metrics).expect("encode metrics"))
        .expect("write metrics");
    let bond = sample_bond_entry(2_000);
    let mut bond_file = NamedTempFile::new().expect("bond file");
    bond_file
        .write_all(&to_bytes(&bond).expect("encode bond"))
        .expect("write bond");
    let instruction_file = NamedTempFile::new().expect("instruction file");
    let instruction_path = instruction_file.path().to_path_buf();
    let args = IncentivesComputeArgs {
        config: config_file.path().to_path_buf(),
        metrics: metrics_file.path().to_path_buf(),
        bond: bond_file.path().to_path_buf(),
        beneficiary: sample_account_literal("beneficiary"),
        norito_out: Some(instruction_path.clone()),
        pretty: true,
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("compute command runs");
    assert_eq!(ctx.outputs().len(), 1, "expected JSON output");
    let value: norito::json::Value =
        norito::json::from_str(&ctx.outputs()[0]).expect("parse instruction JSON");
    assert_compact! { value.get("relay_id").is_some(); "relay id missing in output" };
    let bytes = std::fs::read(&instruction_path).expect("read instruction");
    let decoded: RelayRewardInstructionV1 =
        decode_from_bytes(&bytes).expect("decode instruction");
    assert_eq!(decoded.beneficiary, sample_account_id("beneficiary"));
    assert!(decoded.payout_amount > Quantity::zero());
}
fn incentives_open_dispute_produces_payload() {
    let instruction = sample_reward_instruction();
    let mut instruction_file = NamedTempFile::new().expect("instruction file");
    let instruction_bytes = to_bytes(&instruction).expect("encode instruction");
    instruction_file
        .write_all(&instruction_bytes)
        .expect("write instruction");
    let dispute_file = NamedTempFile::new().expect("dispute file");
    let dispute_path = dispute_file.path().to_path_buf();
    let args = IncentivesOpenDisputeArgs {
        instruction: instruction_file.path().to_path_buf(),
        treasury_account: sample_account_literal("treasury"),
        submitted_by: sample_account_literal("operator"),
        requested_amount: "25".into(),
        reason: "calibration".into(),
        submitted_at: Some(1_234),
        norito_out: Some(dispute_path.clone()),
        pretty: false,
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("open dispute runs");
    assert_eq!(ctx.outputs().len(), 1, "expected JSON output");
    let value: norito::json::Value =
        norito::json::from_str(&ctx.outputs()[0]).expect("parse dispute JSON");
    assert_eq!(value["reason"].as_str(), Some("calibration"));
    let bytes = std::fs::read(&dispute_path).expect("read dispute");
    let dispute: RelayRewardDisputeV1 = decode_from_bytes(&bytes).expect("decode dispute");
    assert_eq!(dispute.submitted_at_unix, 1_234);
    assert_eq!(dispute.submitted_by, sample_account_id("operator"));
}
fn incentives_dashboard_summarises_rewards() {
    let mut inst1 = sample_reward_instruction();
    inst1.payout_amount = Quantity::from(40_u32);
    let mut inst1_file = NamedTempFile::new().expect("inst1");
    let inst1_bytes = to_bytes(&inst1).expect("encode inst1");
    inst1_file.write_all(&inst1_bytes).expect("write inst1");
    let mut inst2 = sample_reward_instruction();
    inst2.epoch = inst1.epoch + 1;
    inst2.payout_amount = Quantity::from(10_u32);
    let mut inst2_file = NamedTempFile::new().expect("inst2");
    let inst2_bytes = to_bytes(&inst2).expect("encode inst2");
    inst2_file.write_all(&inst2_bytes).expect("write inst2");
    let args = IncentivesDashboardArgs {
        instructions: vec![
            inst1_file.path().to_path_buf(),
            inst2_file.path().to_path_buf(),
        ],
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("dashboard runs");
    assert_eq!(ctx.outputs().len(), 1, "expected JSON output");
    let summary: norito::json::Value =
        norito::json::from_str(&ctx.outputs()[0]).expect("parse summary");
    assert_eq!(summary["total_relays"].as_u64(), Some(1));
    assert_eq!(summary["total_payout"].as_str(), Some("50"));
    let rows = summary["rows"].as_array().expect("rows present");
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0]["payout_count"].as_u64(), Some(2));
    assert_eq!(rows[0]["payout_amount"].as_str(), Some("50"));
}
}
#[test]
#[allow(clippy::too_many_lines)]
fn incentives_service_shadow_run_generates_summary() {
    fn metrics_for(
        relay_id: RelayId,
        epoch: u32,
        uptime: u32,
        scheduled: u32,
        bandwidth: u128,
        compliance: RelayComplianceStatusV1,
    ) -> RelayEpochMetricsV1 {
        RelayEpochMetricsV1 {
            relay_id,
            epoch,
            uptime_seconds: u64::from(uptime),
            scheduled_uptime_seconds: u64::from(scheduled),
            verified_bandwidth_bytes: bandwidth,
            compliance,
            reward_score: 0,
            confidence_floor_per_mille: 1_000,
            measurement_ids: Vec::new(),
            metadata: Metadata::default(),
        }
    }
    let config_file = write_sample_reward_config_file();
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
    let metrics_dir = tmp_dir.path().join("metrics");
    fs::create_dir_all(&metrics_dir).expect("create metrics dir");
    let relay_a = [0x21_u8; 32];
    let relay_b = [0x43_u8; 32];
    let relay_primary_bond = RelayBondLedgerEntryV1 {
        relay_id: relay_a,
        bonded_amount: Quantity::from(5_000_u32),
        bond_asset_id: xor_asset_id(),
        bonded_since_unix: 1,
        exit_capable: true,
    };
    let relay_secondary_bond = RelayBondLedgerEntryV1 {
        relay_id: relay_b,
        bonded_amount: Quantity::from(7_500_u32),
        bond_asset_id: xor_asset_id(),
        bonded_since_unix: 1,
        exit_capable: true,
    };
    let relay_primary_bond_file = write_bond_file(&relay_primary_bond);
    let relay_secondary_bond_file = write_bond_file(&relay_secondary_bond);
    let write_metrics_file = |relay: RelayId, epoch: u32, metrics: RelayEpochMetricsV1| {
        let relay_hex = relay_id_to_hex(relay);
        let file_path = metrics_dir.join(format!("relay-{relay_hex}-epoch-{epoch}.to"));
        fs::write(
            &file_path,
            to_bytes(&metrics).expect("encode metrics snapshot"),
        )
        .expect("write metrics snapshot");
    };
    write_metrics_file(
        relay_a,
        1,
        metrics_for(
            relay_a,
            1,
            3_600,
            3_600,
            1_000_000,
            RelayComplianceStatusV1::Clean,
        ),
    );
    write_metrics_file(
        relay_a,
        2,
        metrics_for(
            relay_a,
            2,
            3_500,
            3_600,
            950_000,
            RelayComplianceStatusV1::Warning,
        ),
    );
    write_metrics_file(
        relay_b,
        1,
        metrics_for(
            relay_b,
            1,
            3_400,
            3_600,
            1_200_000,
            RelayComplianceStatusV1::Clean,
        ),
    );
    let mut relay_entries = Vec::new();
    let mut primary_relay_entry = Map::new();
    primary_relay_entry.insert(
        "relay_id".to_string(),
        Value::String(relay_id_to_hex(relay_a)),
    );
    primary_relay_entry.insert(
        "beneficiary".to_string(),
        Value::String(sample_account_literal("relay-a")),
    );
    primary_relay_entry.insert(
        "bond_path".to_string(),
        Value::String(relay_primary_bond_file.path().display().to_string()),
    );
    relay_entries.push(Value::Object(primary_relay_entry));
    let mut secondary_relay_entry = Map::new();
    secondary_relay_entry.insert(
        "relay_id".to_string(),
        Value::String(relay_id_to_hex(relay_b)),
    );
    secondary_relay_entry.insert(
        "beneficiary".to_string(),
        Value::String(sample_account_literal("relay-b")),
    );
    secondary_relay_entry.insert(
        "bond_path".to_string(),
        Value::String(relay_secondary_bond_file.path().display().to_string()),
    );
    relay_entries.push(Value::Object(secondary_relay_entry));
    let mut root = Map::new();
    root.insert("relays".to_string(), Value::Array(relay_entries));
    let config_json = Value::Object(root);
    let config_path = tmp_dir.path().join("shadow_config.json");
    fs::write(
        &config_path,
        norito::json::to_vec_pretty(&config_json).expect("encode config"),
    )
    .expect("write config");
    let args = IncentivesServiceShadowRunArgs {
        state: state_path.clone(),
        config: config_path,
        metrics_dir: metrics_dir.clone(),
        report_out: None,
        pretty: true,
    };
    let mut ctx = TestContext::new();
    args.run(&mut ctx).expect("shadow run executes");
    assert_eq!(ctx.outputs().len(), 1, "expected JSON summary output");
    let summary: norito::json::Value =
        norito::json::from_str(&ctx.outputs()[0]).expect("parse summary json");
    assert_eq!(summary["processed_payouts"].as_u64(), Some(3));
    assert_eq!(summary["total_relays"].as_u64(), Some(2));
    let expected_budget_hex = sample_budget_id_hex();
    assert_eq_compact! { summary["expected_budget_approval"].as_str() => Some(expected_budget_hex.as_str()) };
    assert_eq!(summary["missing_budget_approval"].as_u64(), Some(0));
    assert_eq!(summary["mismatched_budget_approval"].as_u64(), Some(0));
    let relays = summary["relays"]
        .as_array()
        .expect("relay summaries present");
    assert_eq!(relays.len(), 2);
    assert_compact! { relays.iter().any(|relay| relay["warning_epochs"].as_u64() == Some(1)) };
}
test_items! {
fn incentives_service_shadow_run_rejects_state_without_budget_id() {
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    write_state_without_budget(&state_path);
    let metrics_dir = tmp_dir.path().join("metrics");
    fs::create_dir_all(&metrics_dir).expect("create metrics dir");
    let config_path = tmp_dir.path().join("shadow_config.json");
    fs::write(&config_path, r#"{"relays": []}"#).expect("write config");
    let args = IncentivesServiceShadowRunArgs {
        state: state_path,
        config: config_path,
        metrics_dir,
        report_out: None,
        pretty: true,
    };
    let mut ctx = TestContext::new();
    let err = args
        .run(&mut ctx)
        .expect_err("shadow run must require budget approval id");
    assert_compact! { err.to_string().contains("budget_approval_id"); "unexpected error: {err}" };
    assert!(ctx.outputs().is_empty());
}
fn incentives_shadow_run_summary_reports_unconvertible_payout_amount() {
    let relay_id_hex = relay_id_to_hex([0x5A; 32]);
    let summary = DaemonIterationSummary {
        processed: vec![DaemonProcessedPayoutSummary {
            relay_id_hex: relay_id_hex.clone(),
            epoch: 7,
            payout_amount: "340282366920938463463374607431768211456"
                .parse::<Quantity>()
                .expect("2^128 quantity"),
            budget_approval_id: Some(sample_budget_id_hex()),
            metrics: PayoutMetricsSnapshot {
                availability_per_mille: 1_000,
                bandwidth_per_mille: 1_000,
                compliance_per_mille: 1_000,
                compliance_status: "clean".to_string(),
                score_per_mille: 900,
                exit_bonus_applied: false,
            },
            instruction_path: None,
            transfer_path: None,
            metrics_archived_to: None,
        }],
        ..DaemonIterationSummary::default()
    };
    let shadow = build_shadow_run_summary(&summary);
    assert_eq!(shadow.processed_payouts, 1);
    assert_eq!(shadow.total_payout_nanos, 0);
    assert_eq!(shadow.payout_amount_conversion_errors.len(), 1);
    let error = &shadow.payout_amount_conversion_errors[0];
    assert_eq!(error.relay_id_hex, relay_id_hex);
    assert_eq!(error.epoch, 7);
    assert_eq!(error.amount, "340282366920938463463374607431768211456");
    assert_eq!(error.reason, "too_wide_mantissa");
    assert_eq!(shadow.relays.len(), 1);
    assert_eq!(shadow.relays[0].amount_conversion_errors, 1);
    assert_eq!(shadow.relays[0].payout_nanos, 0);
}
fn incentives_state_roundtrip_serializes() {
    let policy = RelayBondPolicyV1 {
        minimum_exit_bond: Quantity::from(1_000_u32),
        bond_asset_id: xor_asset_id(),
        uptime_floor_per_mille: 900,
        slash_penalty_basis_points: 250,
        activation_grace_epochs: 0,
    };
    let reward_config = RewardConfig {
        policy: policy.clone(),
        base_reward: Quantity::from(75_u32),
        uptime_weight_per_mille: 600,
        bandwidth_weight_per_mille: 400,
        compliance_penalty_basis_points: 0,
        bandwidth_target_bytes: 10_000,
        budget_approval_id: Some(sample_budget_id()),
        metrics_log_path: None,
    };
    let treasury_account = sample_account_id("treasury");
    let mut state = IncentivesState::new(&reward_config, treasury_account.clone());
    state.payouts.push(sample_reward_instruction());
    let directory = tempfile::tempdir().expect("state directory");
    let path = directory.path().join("payout_state.json");
    save_incentives_state(&path, &state).expect("save incentives state JSON");
    let bytes = fs::read(&path).expect("read saved state JSON");
    let decoded = load_incentives_state(&path).expect("load incentives state JSON");
    assert_eq!(norito::json::to_vec_pretty(&decoded).expect("reencode state JSON"), bytes);
    decoded.ensure_current().expect("state version matches");
    assert_eq!(decoded.treasury_account, treasury_account);
    assert_eq!(decoded.payouts.len(), state.payouts.len());
    assert_eq_compact! { decoded.reward_config.base_reward => state.reward_config.base_reward };
    assert!(parse_incentives_state_snapshot(b"{").is_err());
    let mut trailing = bytes;
    trailing.extend_from_slice(b" []");
    assert!(parse_incentives_state_snapshot(&trailing).is_err());
    state.version = IncentivesState::VERSION + 1;
    save_incentives_state(&path, &state).expect("save unsupported state version");
    let error = load_incentives_state(&path).expect_err("unsupported state version");
    assert!(error.to_string().contains("unsupported incentives state version"));
}
fn incentives_service_init_rejects_missing_budget_id() {
    let config_file = write_reward_config_with_budget(None);
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let args = IncentivesServiceInitArgs {
        state: state_path.clone(),
        config: config_file.path().to_path_buf(),
        treasury_account: sample_account_literal("treasury"),
        force: false,
    };
    let mut ctx = TestContext::new();
    let err = args
        .run(&mut ctx)
        .expect_err("init must require budget approval id");
    assert_compact! { err.to_string().contains("budget_approval_id"); "unexpected error: {err}" };
    assert_compact! { !state_path.exists(); "init must not write state without budget approval" };
}
fn incentives_service_process_rejects_state_without_budget_id() {
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    write_state_without_budget(&state_path);
    let metrics_file = write_metrics_file(&sample_metrics());
    let bond_file = write_bond_file(&sample_bond_entry(2_000));
    let args = IncentivesServiceProcessArgs {
        state: state_path,
        metrics: vec![metrics_file.path().to_path_buf()],
        bond: vec![bond_file.path().to_path_buf()],
        beneficiary: vec![sample_account_literal("beneficiary")],
        instruction_out: None,
        transfer_out: None,
        submit_transfer: false,
        pretty: false,
    };
    let mut ctx = TestContext::new();
    let err = args
        .run(&mut ctx)
        .expect_err("budget id should be required");
    assert_compact! { err.to_string().contains("budget_approval_id"); "unexpected error: {err}" };
}
fn incentives_service_audit_flags_underbonded_relay() {
    let config_file = write_sample_reward_config_file();
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
    let underbonded = sample_bond_entry(500);
    let bond_file = write_bond_file(&underbonded);
    let mut relay_entry = Map::new();
    relay_entry.insert(
        "relay_id".to_string(),
        Value::String(relay_id_to_hex(underbonded.relay_id)),
    );
    relay_entry.insert(
        "beneficiary".to_string(),
        Value::String(sample_account_literal("relay-audited")),
    );
    relay_entry.insert(
        "bond_path".to_string(),
        Value::String(bond_file.path().display().to_string()),
    );
    let mut root = Map::new();
    root.insert(
        "relays".to_string(),
        Value::Array(vec![Value::Object(relay_entry)]),
    );
    let daemon_config = tmp_dir.path().join("daemon_config.json");
    fs::write(
        &daemon_config,
        norito::json::to_vec_pretty(&root).expect("encode daemon config"),
    )
    .expect("write daemon config");
    let args = IncentivesServiceAuditArgs {
        state: state_path,
        config: daemon_config,
        scopes: vec![IncentiveAuditScope::Bond],
        pretty: true,
    };
    let mut ctx = TestContext::new();
    let err = args
        .run(&mut ctx)
        .expect_err("audit should fail when bond minimum is not met");
    assert!(err.to_string().contains("issue"), "unexpected error: {err}");
    assert_eq!(ctx.outputs().len(), 1, "expected JSON summary output");
    let summary: Value = norito::json::from_str(&ctx.outputs()[0]).expect("parse summary");
    assert_eq_compact! { summary["bond"]["insufficient_bond"].as_u64() => Some(1); "underbonded relay should be reported" };
}
fn incentives_service_audit_flags_budget_mismatch_and_missing() {
    let config_file = write_sample_reward_config_file();
    let reward_config = read_reward_config(config_file.path()).expect("reward config");
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let mut state =
        IncentivesState::new(&reward_config, sample_account_id("treasury-budget-audit"));
    let mut mismatched = sample_reward_instruction();
    mismatched.relay_id = [0xEE; 32];
    mismatched.budget_approval_id = Some([0xFF; 32]);
    let mut missing = sample_reward_instruction();
    missing.relay_id = [0xDD; 32];
    missing.budget_approval_id = None;
    state.payouts = vec![mismatched, missing];
    save_incentives_state(&state_path, &state).expect("write incentives state");
    let daemon_config = tmp_dir.path().join("daemon_config.json");
    fs::write(&daemon_config, r#"{"relays": []}"#).expect("write daemon config");
    let args = IncentivesServiceAuditArgs {
        state: state_path,
        config: daemon_config,
        scopes: vec![IncentiveAuditScope::Budget],
        pretty: true,
    };
    let mut ctx = TestContext::new();
    let err = args.run(&mut ctx).expect_err("budget audit should fail");
    assert!(err.to_string().contains("issue"), "unexpected error: {err}");
    assert_eq!(ctx.outputs().len(), 1, "expected JSON summary output");
    let summary: Value = norito::json::from_str(&ctx.outputs()[0]).expect("parse summary");
    let budget = summary["budget"]
        .as_object()
        .expect("budget summary present");
    let expected_budget = sample_budget_id_hex();
    assert_eq_compact! { budget.get("configured_budget_approval_id").and_then(Value::as_str) => Some(expected_budget.as_str()) };
    assert_eq_compact! { budget.get("mismatched_budget_approval").and_then(Value::as_u64) => Some(1) };
    assert_eq_compact! { budget.get("payouts_without_budget").and_then(Value::as_u64) => Some(1) };
}
fn incentives_service_init_writes_state() {
    let config_file = write_sample_reward_config_file();
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let _ctx = initialize_incentives_state(config_file.path(), &state_path);
    assert!(state_path.exists());
    let state = read_state(&state_path);
    assert_eq!(state.version, IncentivesState::VERSION);
    assert_eq!(state.treasury_account, sample_account_id("treasury"));
    assert!(state.payouts.is_empty());
    assert!(state.disputes.is_empty());
    assert_eq_compact! { state.reward_config.policy.bond_asset_id => xor_asset_id().to_string() };
}
fn incentives_service_process_records_reward() {
    let config_file = write_sample_reward_config_file();
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
    let metrics = sample_metrics();
    let metrics_file = write_metrics_file(&metrics);
    let bond_file = write_bond_file(&sample_bond_entry(2_000));
    let instruction_out = NamedTempFile::new().expect("instruction file");
    let args = IncentivesServiceProcessArgs {
        state: state_path.clone(),
        metrics: vec![metrics_file.path().to_path_buf()],
        bond: vec![bond_file.path().to_path_buf()],
        beneficiary: vec![sample_account_literal("beneficiary")],
        instruction_out: Some(instruction_out.path().to_path_buf()),
        transfer_out: None,
        submit_transfer: false,
        pretty: true,
    };
    let mut process_ctx = TestContext::new();
    args.run(&mut process_ctx).expect("process command runs");
    assert_eq!(process_ctx.outputs().len(), 1);
    let summary: norito::json::Value =
        norito::json::from_str(&process_ctx.outputs()[0]).expect("parse summary");
    assert_eq!(summary["epoch"].as_u64(), Some(u64::from(metrics.epoch)));
    assert_eq!(summary["ledger"]["total_paid"].as_str(), Some("100"));
    let state = read_state(&state_path);
    assert_eq!(state.payouts.len(), 1);
    assert_eq!(state.payouts[0].epoch, metrics.epoch);
    assert_eq_compact! { state.payouts[0].beneficiary => sample_account_id("beneficiary") };
    let instruction_bytes = fs::read(instruction_out.path()).expect("read instruction");
    let instruction: RelayRewardInstructionV1 =
        decode_from_bytes(&instruction_bytes).expect("decode instruction");
    assert_eq!(instruction.epoch, metrics.epoch);
}
fn incentives_daemon_rejects_state_without_budget_id() {
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    write_state_without_budget(&state_path);
    let metrics_dir = tmp_dir.path().join("metrics");
    fs::create_dir_all(&metrics_dir).expect("create metrics dir");
    let bond_entry = sample_bond_entry(2_000);
    let relay_hex = relay_id_to_hex(bond_entry.relay_id);
    let bond_file = write_bond_file(&bond_entry);
    let mut relay_entry = Map::new();
    relay_entry.insert("relay_id".to_string(), Value::String(relay_hex));
    relay_entry.insert(
        "beneficiary".to_string(),
        Value::String(sample_account_literal("relay-a")),
    );
    relay_entry.insert(
        "bond_path".to_string(),
        Value::String(bond_file.path().display().to_string()),
    );
    let mut root = Map::new();
    root.insert(
        "relays".to_string(),
        Value::Array(vec![Value::Object(relay_entry)]),
    );
    let config_path = tmp_dir.path().join("daemon_config.json");
    fs::write(
        &config_path,
        norito::json::to_vec_pretty(&root).expect("encode config"),
    )
    .expect("write config");
    let daemon_args = IncentivesServiceDaemonArgs {
        state: state_path,
        config: config_path,
        metrics_dir,
        instruction_out_dir: None,
        transfer_out_dir: None,
        archive_dir: None,
        poll_interval: 1,
        once: true,
        pretty: true,
    };
    let mut ctx = TestContext::new();
    let result = daemon_args.run(&mut ctx);
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert_compact! { err.contains("budget_approval_id"); "unexpected error: {err}" };
}
fn incentives_daemon_reports_budget_hash() {
    let config_file = write_sample_reward_config_file();
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
    let metrics_dir = tmp_dir.path().join("metrics");
    fs::create_dir_all(&metrics_dir).expect("create metrics dir");
    let metrics = sample_metrics();
    let relay_hex = relay_id_to_hex(metrics.relay_id);
    let metrics_path =
        metrics_dir.join(format!("relay-{relay_hex}-epoch-{}.to", metrics.epoch));
    fs::write(
        &metrics_path,
        to_bytes(&metrics).expect("encode metrics snapshot"),
    )
    .expect("write metrics");
    let bond_entry = sample_bond_entry(2_000);
    let bond_file = write_bond_file(&bond_entry);
    let mut relay_entry = Map::new();
    relay_entry.insert("relay_id".to_string(), Value::String(relay_hex));
    relay_entry.insert(
        "beneficiary".to_string(),
        Value::String(sample_account_literal("relay-a")),
    );
    relay_entry.insert(
        "bond_path".to_string(),
        Value::String(bond_file.path().display().to_string()),
    );
    let mut root = Map::new();
    root.insert(
        "relays".to_string(),
        Value::Array(vec![Value::Object(relay_entry)]),
    );
    let config_path = tmp_dir.path().join("daemon_config.json");
    fs::write(
        &config_path,
        norito::json::to_vec_pretty(&root).expect("encode config"),
    )
    .expect("write config");
    let daemon_args = IncentivesServiceDaemonArgs {
        state: state_path,
        config: config_path,
        metrics_dir,
        instruction_out_dir: None,
        transfer_out_dir: None,
        archive_dir: None,
        poll_interval: 1,
        once: true,
        pretty: true,
    };
    let mut ctx = TestContext::new();
    daemon_args.run(&mut ctx).expect("daemon run succeeds");
    assert_eq!(ctx.outputs().len(), 1);
    let summary: norito::json::Value =
        norito::json::from_str(&ctx.outputs()[0]).expect("parse daemon summary");
    assert_eq!(summary["processed"].as_array().map(Vec::len), Some(1));
    assert_eq!(summary["missing_budget_approval"].as_u64(), Some(0));
    assert_eq!(summary["mismatched_budget_approval"].as_u64(), Some(0));
    let expected_budget_hex = sample_budget_id_hex();
    assert_eq_compact! { summary["expected_budget_approval"].as_str() => Some(expected_budget_hex.as_str()) };
}
}
#[test]
#[allow(clippy::too_many_lines)]
fn incentives_service_dispute_flow_updates_state() {
    let config_file = write_sample_reward_config_file();
    let tmp_dir = tempfile::tempdir().expect("temp dir");
    let state_path = tmp_dir.path().join("payout_state.json");
    let _init_ctx = initialize_incentives_state(config_file.path(), &state_path);
    let metrics_file = write_metrics_file(&sample_metrics());
    let bond_file = write_bond_file(&sample_bond_entry(2_000));
    let process_args = IncentivesServiceProcessArgs {
        state: state_path.clone(),
        metrics: vec![metrics_file.path().to_path_buf()],
        bond: vec![bond_file.path().to_path_buf()],
        beneficiary: vec![sample_account_literal("beneficiary")],
        instruction_out: None,
        transfer_out: None,
        submit_transfer: false,
        pretty: false,
    };
    let mut process_ctx = TestContext::new();
    process_args
        .run(&mut process_ctx)
        .expect("process command runs");
    let state = read_state(&state_path);
    let instruction = state.payouts[0].clone();
    let file_args = IncentivesServiceDisputeFileArgs {
        state: state_path.clone(),
        relay_id: hex::encode(instruction.relay_id),
        epoch: instruction.epoch,
        submitted_by: sample_account_literal("operator"),
        requested_amount: "120".into(),
        reason: "missing bandwidth".into(),
        filed_at: Some(9_999),
        adjust_credit: Some("25".into()),
        adjust_debit: None,
        norito_out: None,
        pretty: true,
    };
    let mut dispute_ctx = TestContext::new();
    file_args.run(&mut dispute_ctx).expect("file dispute runs");
    assert_eq!(dispute_ctx.outputs().len(), 1);
    let state = read_state(&state_path);
    assert_eq!(state.disputes.len(), 1);
    let stored = &state.disputes[0];
    assert_eq_compact! { stored.requested_amount => Quantity::from_str("120").expect("quantity literal") };
    assert_eq_compact! { stored.requested_adjustment.as_ref().expect("adjustment present").amount => Quantity::from_str("25").expect("quantity literal") };
    let transfer_file = NamedTempFile::new().expect("transfer file");
    let resolve_args = IncentivesServiceDisputeResolveArgs {
        state: state_path.clone(),
        dispute_id: stored.id,
        resolution: IncentivesDisputeResolutionKind::Credit,
        amount: Some("25".into()),
        notes: "approved".into(),
        resolved_at: Some(10_500),
        transfer_out: Some(transfer_file.path().to_path_buf()),
        pretty: true,
    };
    let mut resolve_ctx = TestContext::new();
    resolve_args
        .run(&mut resolve_ctx)
        .expect("resolve dispute runs");
    assert_eq!(resolve_ctx.outputs().len(), 1);
    let state = read_state(&state_path);
    assert_eq!(state.disputes.len(), 1);
    match &state.disputes[0].status {
        StoredDisputeStatus::Resolved { kind, amount, .. } => {
            assert!(matches!(kind, StoredResolutionKind::Credit));
            assert_eq_compact! { amount.clone() => Some(Quantity::from_str("25").expect("quantity literal")) };
        }
        other => panic!("unexpected dispute status: {other:?}"),
    }
    let transfer_bytes = fs::read(transfer_file.path()).expect("read transfer");
    let transfer: InstructionBox = decode_from_bytes(&transfer_bytes).expect("decode transfer");
    let transfer_box = transfer
        .as_any()
        .downcast_ref::<TransferBox>()
        .expect("transfer instruction");
    let TransferBox::Asset(transfer) = transfer_box else {
        panic!("expected asset transfer, found {transfer_box:?}");
    };
    assert_eq!(transfer.object, Quantity::from(25_u32));
    assert_eq!(transfer.destination, sample_account_id("beneficiary"));
    assert_eq!(transfer.source.account, sample_account_id("treasury"));
}
