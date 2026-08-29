use super::test_support::{
    TestEnvGuard, env_lock, genesis_invocation_count, install_kagami_stub, install_noop_stub,
    kagami_sign_invocation_count,
};
use super::{
    ActiveView, CliOverrides, InstructionPermission, MaintenanceCommand, MaintenanceState,
    MaintenanceTask, MochiApp, ProfilePreset, SignerEntryForm, SignerEntryState, StatePageCache,
    StateQueryKind, SupervisorBuilder, SupervisorError, compose_app_env_recipe,
    compose_launch_recipe, ensure_http_base, filter_state_entries, reset_cli_overrides_for_tests,
    shell_quote,
};
use egui::{CentralPanel, Color32, Context, FontFamily, TextStyle};
use iroha_data_model::{
    account::{AccountAdmissionMode, admission::ImplicitAccountFeeDestination},
    asset::id::AssetId,
    block::{
        BlockHeader,
        consensus::{
            SumeragiDataspaceCommitment, SumeragiDiagnosticsStatus, SumeragiLaneCommitment,
            SumeragiLaneGovernance, SumeragiRuntimeUpgradeHook,
        },
        consensus_v2::{
            ConsensusMode, DualQuorum, HeightContextId, PROTOCOL_VERSION, SumeragiV2BodyState,
            SumeragiV2HeightContextStatus, SumeragiV2Status, SumeragiV2StatusPhase,
        },
    },
    da::commitment::DaProofScheme,
    events::{
        EventBox,
        time::{TimeEvent, TimeInterval},
    },
    nexus::{DataSpaceId, LaneId, LaneRelayEnvelope, LaneStorageProfile, LaneVisibility},
    prelude::{Hash, HashOf},
    role::RoleId,
};
use iroha_test_samples::{ALICE_ID, ALICE_KEYPAIR};
use mochi_core::{
    ExposedPrivateKey, TelemetryStatus, ToriiError, TxGossipSnapshot,
    torii::{GovernanceStatus, StatusMetrics, Uptime},
};
use norito::json::{self, Value};
use std::{
    collections::VecDeque,
    num::NonZeroU64,
    path::Path,
    time::{Duration, Instant},
};
#[test]
fn snapshot_label_preview_matches_expectations() {
    assert_eq!(
        MochiApp::preview_snapshot_label("My Snapshot!!!"),
        Some("my-snapshot".to_owned())
    );
    assert_eq!(
        MochiApp::preview_snapshot_label("   "),
        None,
        "blank labels should produce no preview"
    );
}
#[test]
fn theme_palette_applied_to_visuals() {
    let mut app = MochiApp::default();
    let ctx = Context::default();
    app.ensure_theme(&ctx);
    let visuals = &ctx.style().visuals;
    let palette = MochiApp::palette();
    assert_eq!(visuals.panel_fill, palette.panel);
    assert_eq!(visuals.window_fill, palette.panel);
    assert_eq!(visuals.hyperlink_color, palette.accent);
    assert_eq!(visuals.selection.bg_fill, palette.accent);
    assert_eq!(visuals.widgets.inactive.bg_fill, palette.surface);
    assert_eq!(visuals.weak_text_color, Some(palette.text_muted));
    let style = ctx.style();
    let heading = style
        .text_styles
        .get(&TextStyle::Heading)
        .expect("heading style");
    assert_eq!(heading.family, FontFamily::Proportional);
    assert!((heading.size - 20.0).abs() < f32::EPSILON);
}
#[test]
fn shell_quote_handles_spaces_and_single_quotes() {
    assert_eq!(shell_quote("mochi-local"), "mochi-local");
    assert_eq!(shell_quote("/tmp/mochi data"), "'/tmp/mochi data'");
    assert_eq!(shell_quote("alice's sandbox"), "'alice'\"'\"'s sandbox'");
}
#[test]
fn ensure_http_base_adds_scheme_once() {
    assert_eq!(ensure_http_base("127.0.0.1:8080"), "http://127.0.0.1:8080");
    assert_eq!(
        ensure_http_base("http://127.0.0.1:8080/"),
        "http://127.0.0.1:8080"
    );
}
#[test]
fn compose_launch_recipe_includes_current_flags() {
    let recipe = compose_launch_recipe(
        "four-peer-bft",
        "/tmp/mochi data",
        "mochi-local",
        Some(8080),
        Some(1337),
        true,
        false,
    );
    assert!(
        recipe.starts_with("cargo run -p mochi-ui --features gui --bin mochi -- sandbox serve")
    );
    assert!(recipe.contains("--profile four-peer-bft"));
    assert!(recipe.contains("--workspace-root '/tmp/mochi data'"));
    assert!(recipe.contains("--chain-id mochi-local"));
    assert!(recipe.contains("--torii-start 8080"));
    assert!(recipe.contains("--p2p-start 1337"));
    assert!(recipe.contains("--build-binaries"));
    assert!(recipe.contains("--disable-smoke"));
}
#[test]
fn compose_app_env_recipe_emits_local_bootstrap_exports() {
    let recipe = compose_app_env_recipe(
        "127.0.0.1:8080",
        "127.0.0.1:8080",
        Some("http://127.0.0.1:8080/v1/mcp"),
        "mochi-local",
        Some("alice@wonderland"),
        Some("deadbeef"),
    );
    assert!(recipe.contains("export IROHA_API_BASE=http://127.0.0.1:8080"));
    assert!(recipe.contains("export IROHA_TORII_URL=http://127.0.0.1:8080"));
    assert!(recipe.contains("export IROHA_MCP_URL=http://127.0.0.1:8080/v1/mcp"));
    assert!(recipe.contains("export IROHA_CHAIN_ID=mochi-local"));
    assert!(recipe.contains("export IROHA_ACCOUNT_ID=alice@wonderland"));
    assert!(recipe.contains("export IROHA_PRIVATE_KEY=deadbeef"));
}
#[test]
fn render_view_tabs_keeps_active_view() {
    let mut app = MochiApp::default();
    app.active_view = ActiveView::Activity;
    let ctx = Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        CentralPanel::default().show(ctx, |ui| {
            app.render_view_tabs(ui);
        });
    });
    assert_eq!(app.active_view, ActiveView::Activity);
}
#[test]
fn render_overview_bar_smoke() {
    if !super::socket_bind_available() {
        eprintln!("Skipping overview bar test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_ui_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("ui-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    let mut supervisor = app.supervisor.take().expect("supervisor ready");
    let peer_rows = app.build_peer_rows(&supervisor);
    let metrics = app.collect_dashboard_metrics(&peer_rows);
    let ctx = Context::default();
    let _ = ctx.run(Default::default(), |ctx| {
        CentralPanel::default().show(ctx, |ui| {
            app.render_overview_bar(ui, &mut supervisor, &peer_rows, &metrics);
        });
    });
    assert!(!app.settings_dialog);
    app.supervisor = Some(supervisor);
}
#[test]
fn cli_profile_override_reconfigures_builder() {
    let overrides = CliOverrides {
        profile: Some(NetworkProfile::from_preset(ProfilePreset::FourPeerBft)),
        ..Default::default()
    };
    let builder = overrides.apply_to(SupervisorBuilder::new(ProfilePreset::FourPeerBft));
    assert_eq!(builder.profile().preset, Some(ProfilePreset::FourPeerBft));
    assert_eq!(builder.profile().topology.peer_count, 4);
}
#[test]
fn maintenance_state_running_shows_spinner() {
    let banner = MaintenanceState::Running(MaintenanceTask::Snapshot)
        .banner()
        .expect("running state should surface banner");
    assert!(banner.show_spinner, "running banner should show spinner");
    assert!(
        !banner.dismissable,
        "running banner should not be dismissable"
    );
}
#[test]
fn maintenance_state_completed_is_dismissable() {
    let banner = MaintenanceState::Completed {
        message: "Network reset complete".to_owned(),
    }
    .banner()
    .expect("completed state should surface banner");
    assert!(
        !banner.show_spinner,
        "completed banner should not show spinner"
    );
    assert!(banner.dismissable, "completed banner should be dismissable");
    assert!(
        banner.text.contains("Network reset"),
        "completed banner should retain completion message"
    );
}
#[test]
fn entry_form_to_state_accepts_valid_inputs() {
    let form = SignerEntryForm {
        label: "Test signer".to_owned(),
        account: account_literal(&ALICE_ID),
        private_key: ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone())
            .to_string()
            .into(),
        permissions: [InstructionPermission::MintAsset].into_iter().collect(),
        roles: String::new(),
    };
    let state =
        MochiApp::entry_form_to_state(&form).expect("valid signer form should produce entry state");
    assert_eq!(state.label, "Test signer");
    assert_eq!(state.account, account_literal(&ALICE_ID));
    assert!(
        state
            .permissions
            .contains(&InstructionPermission::MintAsset)
    );
}
#[test]
fn entry_form_to_state_rejects_missing_fields() {
    let form = SignerEntryForm {
        label: "Missing account".to_owned(),
        private_key: "deadbeef".to_owned().into(),
        ..Default::default()
    };
    let err = match MochiApp::entry_form_to_state(&form) {
        Ok(_) => panic!("missing account should produce validation error"),
        Err(err) => err,
    };
    assert!(
        err.contains("Account identifier is required"),
        "unexpected error: {err}"
    );
}
#[test]
fn signer_entries_to_signers_converts_entries() {
    let private_key = ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone()).to_string();
    let entry = SignerEntryState {
        label: "Alice real".to_owned(),
        account: account_literal(&ALICE_ID),
        private_key: private_key.into(),
        permissions: InstructionPermission::all().into_iter().collect(),
        roles: String::new(),
    };
    let signers =
        MochiApp::signer_entries_to_signers(&[entry]).expect("expected successful conversion");
    assert_eq!(signers.len(), 1);
    let signer = &signers[0];
    assert_eq!(signer.label(), "Alice real");
    assert_eq!(signer.account_id(), &*ALICE_ID);
}
#[test]
fn signer_entries_to_signers_rejects_empty_permissions() {
    let entry = SignerEntryState {
        label: "No perms".to_owned(),
        account: account_literal(&ALICE_ID),
        private_key: ExposedPrivateKey(ALICE_KEYPAIR.private_key().clone())
            .to_string()
            .into(),
        permissions: Default::default(),
        roles: String::new(),
    };
    let err = MochiApp::signer_entries_to_signers(&[entry])
        .expect_err("empty permission list should be rejected");
    assert!(
        err.contains("must permit at least one instruction"),
        "unexpected error message: {err}"
    );
}
#[test]
fn parse_role_list_accepts_comma_separated_roles() {
    let roles = MochiApp::parse_role_list("auditor, basic_user").expect("role list should parse");
    assert_eq!(roles.len(), 2);
}
#[test]
fn parse_role_list_rejects_invalid_roles() {
    let err =
        MochiApp::parse_role_list("not a role").expect_err("invalid roles should be rejected");
    assert!(
        err.contains("Invalid role"),
        "unexpected error message: {err}"
    );
}
#[test]
fn parse_optional_u32_accepts_empty_and_numbers() {
    assert_eq!(
        MochiApp::parse_optional_u32("", "Max per tx").expect("empty ok"),
        None
    );
    assert_eq!(
        MochiApp::parse_optional_u32(" 7 ", "Max per tx").expect("parse ok"),
        Some(7)
    );
}
#[test]
fn parse_lane_count_input_rejects_zero() {
    let err =
        MochiApp::parse_lane_count_input("0").expect_err("zero lane count should be rejected");
    assert!(err.contains("greater than zero"), "unexpected error: {err}");
}
#[test]
fn parse_lane_count_input_accepts_numbers() {
    assert_eq!(
        MochiApp::parse_lane_count_input(" 3 ").expect("parse ok"),
        Some(3)
    );
    assert_eq!(
        MochiApp::parse_lane_count_input("").expect("empty ok"),
        None
    );
}
#[test]
fn toml_helpers_require_exact_toml_types() {
    let value = TomlValue::String("alpha".to_owned());
    assert_eq!(toml_string(&value).as_deref(), Some("alpha"));
    assert_eq!(toml_u32(&TomlValue::Integer(7)), Some(7));
    assert_eq!(toml_u32(&TomlValue::String("12".to_owned())), None);
}
include!("gui/tests/lane_and_admission.rs");
#[test]
fn maintenance_export_snapshot_creates_snapshot_directory() {
    if !super::socket_bind_available() {
        eprintln!("Skipping snapshot maintenance test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_snapshot_stub.sh");
    let log_path = temp.path().join("kagami_snapshot.log");
    let _log_guard = TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &log_path);
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("snapshot-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    let mut app = MochiApp::default();
    let mut supervisor_slot = app.supervisor.take();
    let initial_invocations = genesis_invocation_count(&log_path);
    assert!(
        app.begin_maintenance(MaintenanceTask::Snapshot),
        "snapshot maintenance should start when idle"
    );
    let label = "Smoke Snapshot 42".to_owned();
    app.maintenance_command = Some(MaintenanceCommand::ExportSnapshot {
        label: Some(label.clone()),
    });
    app.schedule_pending_maintenance(&mut supervisor_slot);
    for _ in 0..100 {
        app.poll_maintenance_updates(&mut supervisor_slot);
        if !matches!(app.maintenance_state, MaintenanceState::Running(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !matches!(app.maintenance_state, MaintenanceState::Running(_)),
        "snapshot maintenance did not finish in time"
    );
    assert!(
        supervisor_slot.is_some(),
        "supervisor should be restored after maintenance"
    );
    app.supervisor = supervisor_slot;
    let supervisor = app.supervisor.as_ref().expect("supervisor restored");
    match &app.maintenance_state {
        MaintenanceState::Completed { message } => {
            assert!(
                message.contains("Snapshot exported"),
                "expected completion message, got `{message}`"
            );
        }
        other => panic!("snapshot maintenance did not complete: {other:?}"),
    }
    let snapshots_dir = supervisor.paths().snapshots_dir();
    let snapshot_slug = "smoke-snapshot-42";
    let snapshot_root = snapshots_dir.join(snapshot_slug);
    assert!(
        snapshot_root.exists(),
        "expected snapshot directory {}",
        snapshot_root.display()
    );
    let metadata_bytes =
        fs::read(snapshot_root.join("metadata.json")).expect("read snapshot metadata");
    let metadata: Value = json::from_slice(&metadata_bytes).expect("parse snapshot metadata");
    assert_eq!(
        metadata.get("snapshot").and_then(Value::as_str),
        Some(snapshot_slug),
        "metadata should record sanitized snapshot slug"
    );
    assert_eq!(
        metadata.get("peer_count").and_then(Value::as_u64),
        Some(supervisor.peers().len() as u64),
        "metadata peer_count should match supervisor peer count"
    );
    let final_invocations = genesis_invocation_count(&log_path);
    assert_eq!(
        final_invocations, initial_invocations,
        "exporting snapshots must not trigger additional kagami invocations"
    );
}
#[test]
fn maintenance_reset_invokes_kagami_and_cleans_storage() {
    if !super::socket_bind_available() {
        eprintln!("Skipping reset maintenance test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_reset_stub.sh");
    let log_path = temp.path().join("kagami_reset.log");
    let _log_guard = TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &log_path);
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("reset-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    let mut app = MochiApp::default();
    let mut supervisor_slot = app.supervisor.take();
    {
        let supervisor = supervisor_slot.as_ref().expect("supervisor ready");
        for peer in supervisor.peers() {
            let storage_dir = peer.storage_dir();
            fs::write(storage_dir.join("junk.bin"), b"junk").expect("write junk file");
        }
    }
    let baseline_invocations = genesis_invocation_count(&log_path);
    assert!(
        app.begin_maintenance(MaintenanceTask::Reset),
        "reset maintenance should start when idle"
    );
    app.maintenance_command = Some(MaintenanceCommand::Reset);
    app.schedule_pending_maintenance(&mut supervisor_slot);
    for _ in 0..120 {
        app.poll_maintenance_updates(&mut supervisor_slot);
        if !matches!(app.maintenance_state, MaintenanceState::Running(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !matches!(app.maintenance_state, MaintenanceState::Running(_)),
        "reset maintenance did not finish in time"
    );
    assert!(
        supervisor_slot.is_some(),
        "supervisor should be restored after maintenance"
    );
    app.supervisor = supervisor_slot;
    let supervisor = app.supervisor.as_ref().expect("supervisor restored");
    match &app.maintenance_state {
        MaintenanceState::Completed { message } => {
            assert!(
                message.contains("reset"),
                "expected reset completion message, got `{message}`"
            );
        }
        other => panic!("reset maintenance did not complete: {other:?}"),
    }
    for peer in supervisor.peers() {
        let storage_dir = peer.storage_dir();
        assert!(
            !storage_dir.join("junk.bin").exists(),
            "storage should remove junk for {}",
            peer.alias()
        );
        let snapshot_dir = peer.snapshot_dir();
        assert!(
            snapshot_dir.exists(),
            "snapshot directory should exist for {}",
            peer.alias()
        );
        let entries = fs::read_dir(&snapshot_dir)
            .expect("snapshot dir entries")
            .map(|entry| entry.expect("snapshot entry").file_name())
            .collect::<Vec<_>>();
        assert_eq!(entries, vec!["generations"]);
        assert!(
            fs::read_dir(snapshot_dir.join("generations"))
                .expect("snapshot generations")
                .next()
                .is_none(),
            "snapshot generations should remain empty for {}",
            peer.alias()
        );
    }
    let final_invocations = genesis_invocation_count(&log_path);
    assert!(
        final_invocations > baseline_invocations,
        "wipe & re-genesis should invoke kagami again"
    );
}
#[test]
fn maintenance_restore_snapshot_rehydrates_storage() {
    if !super::socket_bind_available() {
        eprintln!("Skipping restore maintenance test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_restore_stub.sh");
    let log_path = temp.path().join("kagami_restore.log");
    let _log_guard = TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &log_path);
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp.path().join("restore-data");
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    let mut app = MochiApp::default();
    let mut supervisor_slot = app.supervisor.take();
    let supervisor = supervisor_slot.as_mut().expect("supervisor ready");
    let peer = supervisor.peers().first().expect("at least one peer");
    let storage_dir = peer.storage_dir();
    let marker_path = storage_dir.join("marker.txt");
    fs::write(&marker_path, b"snapshot-data").expect("write snapshot data");
    let snapshot_root = supervisor
        .export_snapshot(Some("Restore Snapshot 7"))
        .expect("export snapshot");
    let slug = snapshot_root
        .file_name()
        .unwrap()
        .to_string_lossy()
        .to_string();
    fs::write(&marker_path, b"mutated-data").expect("mutate storage marker");
    assert!(
        app.begin_maintenance(MaintenanceTask::Restore),
        "restore maintenance should start when idle"
    );
    let target = slug.clone();
    app.maintenance_command = Some(MaintenanceCommand::Restore { target });
    app.schedule_pending_maintenance(&mut supervisor_slot);
    for _ in 0..120 {
        app.poll_maintenance_updates(&mut supervisor_slot);
        if !matches!(app.maintenance_state, MaintenanceState::Running(_)) {
            break;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    assert!(
        !matches!(app.maintenance_state, MaintenanceState::Running(_)),
        "restore maintenance did not finish in time"
    );
    assert!(
        supervisor_slot.is_some(),
        "supervisor should be restored after maintenance"
    );
    app.supervisor = supervisor_slot;
    let supervisor = app.supervisor.as_ref().expect("supervisor restored");
    match &app.maintenance_state {
        MaintenanceState::Completed { message } => {
            assert!(
                message.contains("restored"),
                "expected restore completion message, got `{message}`"
            );
        }
        other => panic!("restore maintenance did not complete: {other:?}"),
    }
    let restored_marker =
        fs::read(marker_path).expect("read storage marker after restore completed");
    assert_eq!(
        restored_marker, b"snapshot-data",
        "restore should rehydrate storage contents"
    );
    let snapshots_dir = supervisor.paths().snapshots_dir();
    assert!(
        snapshots_dir.join(&slug).exists(),
        "snapshot should remain available for future restores"
    );
}
fn sample_sumeragi_status_wire() -> SumeragiV2Status {
    SumeragiV2Status {
        protocol_version: PROTOCOL_VERSION,
        node_fingerprint: Hash::new(b"mochi-ui-node"),
        build_fingerprint: Hash::new(b"mochi-ui-build"),
        config_fingerprint: Hash::new(b"mochi-ui-config"),
        restart_required: false,
        height_context_id: HeightContextId(HashOf::from_untyped_unchecked(Hash::new(
            b"mochi-ui-context",
        ))),
        height: 10,
        view: 4,
        phase: SumeragiV2StatusPhase::Prepare,
        leader: 1,
        locked_prepare_qc: None,
        highest_prepare_qc: None,
        last_timeout_certificate: None,
        body_state: SumeragiV2BodyState::Validated,
        pending_persistence_id: None,
        last_committed_height: 9,
        last_committed_subject: None,
        height_context: SumeragiV2HeightContextStatus {
            epoch: 1,
            epoch_end_height: 100,
            mode: ConsensusMode::Permissioned,
            epoch_seed: [0xA5; 32],
            validator_count: 4,
            quorum: DualQuorum {
                min_signers: 3,
                total_power: 4,
            },
        },
        last_commit_qc: None,
        liveness: Default::default(),
    }
}
fn sample_sumeragi_diagnostics() -> SumeragiDiagnosticsStatus {
    SumeragiDiagnosticsStatus {
        pipeline_execution: Default::default(),
        tx_queue_depth: 4,
        tx_queue_capacity: 128,
        tx_queue_retained_bytes: 0,
        tx_queue_max_retained_bytes: 1,
        tx_queue_saturated: false,
        tx_queue_saturated_by_count: false,
        tx_queue_saturated_by_bytes: false,
        tx_queue_saturated_by_age: false,
        tx_queue_oldest_queued_age_ms: 0,
        npos: None,
        lane_commitments: vec![SumeragiLaneCommitment {
            block_height: 10,
            lane_id: LaneId::new(0),
            tx_count: 3,
            total_chunks: 4,
            rbc_bytes_total: 384,
            teu_total: 96,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0x90; Hash::LENGTH],
            )),
        }],
        dataspace_commitments: vec![SumeragiDataspaceCommitment {
            block_height: 10,
            lane_id: LaneId::new(0),
            dataspace_id: DataSpaceId::new(2),
            tx_count: 1,
            total_chunks: 2,
            rbc_bytes_total: 128,
            teu_total: 32,
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed(
                [0x91; Hash::LENGTH],
            )),
        }],
        lane_settlement_commitments: Vec::new(),
        lane_relay_envelopes: Vec::new(),
        lane_payload_ownerships: Vec::new(),
        committed_lane_blocks: Vec::new(),
        lane_block_sessions: Vec::new(),
        lane_governance_sealed_total: 0,
        lane_governance_sealed_aliases: Vec::new(),
        lane_governance: vec![SumeragiLaneGovernance {
            lane_id: LaneId::new(0),
            alias: "alpha".to_owned(),
            governance: Some("parliament".to_owned()),
            manifest_required: true,
            manifest_ready: true,
            manifest_path: Some("/etc/iroha/lanes/alpha.json".to_owned()),
            validator_ids: vec![
                "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".to_owned(),
                "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB".to_owned(),
            ],
            quorum: Some(2),
            protected_namespaces: vec!["finance".to_owned()],
            runtime_upgrade: Some(SumeragiRuntimeUpgradeHook {
                allow: true,
                require_metadata: true,
                metadata_key: Some("upgrade_id".to_owned()),
                allowed_ids: vec!["alpha-upgrade".to_owned()],
            }),
        }],
        native_amx_participant_applications: Vec::new(),
        autonomous_lane_executions: Vec::new(),
    }
}
#[test]
fn ensure_selection_picks_first_available() {
    let mut selection = Some("missing".to_owned());
    let aliases = vec!["alpha".to_owned(), "beta".to_owned()];
    MochiApp::ensure_selection(&mut selection, &aliases);
    assert_eq!(selection, Some("alpha".to_owned()));
}
#[test]
fn collect_event_text_includes_summary_and_detail() {
    let rendered = vec![
        RenderedEventLine::new(
            "alpha",
            "[alpha] Transaction Rejected — ABC".to_owned(),
            Some("hash=ABCDEF • raw=128B".to_owned()),
            Color32::from_rgb(225, 90, 90),
            RenderedEventKind::Category(EventCategory::Pipeline),
        )
        .with_badges(vec![EventBadge::new(
            "reason",
            "invalid_signature".to_owned(),
            None,
            Color32::from_rgb(255, 140, 140),
        )]),
        RenderedEventLine::new(
            "alpha",
            "[alpha] Block Committed — height 1".to_owned(),
            None,
            Color32::from_rgb(110, 200, 220),
            RenderedEventKind::Category(EventCategory::Pipeline),
        ),
    ];
    let export = collect_event_text(&[0, 1], &rendered);
    let lines: Vec<&str> = export.lines().collect();
    assert_eq!(lines.len(), 2, "expected two exported lines");
    assert_eq!(
        lines[0], "[alpha] Block Committed — height 1",
        "newest event should appear first"
    );
    assert_eq!(
        lines[1],
        "[alpha] Transaction Rejected — ABC — hash=ABCDEF • raw=128B — reason=invalid_signature"
    );
}
#[test]
fn collect_event_json_serializes_structured_events() {
    let time_interval = TimeInterval::new(Duration::from_millis(10), Duration::from_millis(5));
    let time_event = EventBox::Time(TimeEvent::new(time_interval));
    let summary = EventSummary {
        category: EventCategory::Time,
        label: "Interval".to_owned(),
        detail: Some("since=10ms length=5ms".to_owned()),
    };
    let structured_event = EventDisplay {
        alias: Some("alpha".to_owned()),
        event: EventStreamEvent::Event {
            summary,
            event: Arc::new(time_event),
            raw_len: 42,
        },
    };
    let as_json = collect_event_json(&[0], std::slice::from_ref(&structured_event))
        .expect("structured event should export to JSON");
    let parsed: Value =
        json::from_str(&as_json).expect("exported JSON should be parseable via Norito");
    let array = parsed.as_array().expect("export must be a JSON array");
    assert_eq!(array.len(), 1, "expected a single exported event payload");
    let text_event = EventDisplay {
        alias: Some("alpha".to_owned()),
        event: EventStreamEvent::Text {
            text: "note".to_owned(),
        },
    };
    assert!(
        collect_event_json(&[1], &[structured_event, text_event]).is_err(),
        "JSON export must fail when no structured events match"
    );
}
fn sample_state_entry(title: &str, bytes: Vec<u8>) -> super::StateEntry {
    let json_payload = format!("{{\"title\":\"{title}\"}}");
    super::StateEntry {
        title: title.to_owned(),
        subtitle: "subtitle".to_owned(),
        detail: "detail".to_owned(),
        raw: "{}".to_owned(),
        domain: None,
        domain_lower: None,
        owner: None,
        owner_lower: None,
        asset_definition: None,
        asset_definition_lower: None,
        json: Some(json_payload),
        norito_bytes: Some(bytes),
        search_blob: title.to_ascii_lowercase(),
    }
}
fn sample_state_entry_with_domain(title: &str, domain: &str, bytes: Vec<u8>) -> super::StateEntry {
    let mut entry = sample_state_entry(title, bytes);
    let lower = domain.to_ascii_lowercase();
    entry.domain = Some(domain.to_owned());
    entry.domain_lower = Some(lower.clone());
    entry.search_blob.push(' ');
    entry.search_blob.push_str(&lower);
    entry
}
#[test]
fn collect_state_json_exports_array() {
    let entries = [
        sample_state_entry(
            "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            vec![0xAA, 0x01],
        ),
        sample_state_entry(
            "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB",
            vec![0xBB, 0x02],
        ),
    ];
    let refs: Vec<&super::StateEntry> = entries.iter().collect();
    let json_text = super::collect_state_json(&refs).expect("export filtered state to json");
    let parsed: Value = json::from_str(&json_text).expect("parse exported Norito JSON");
    let array = parsed
        .as_array()
        .expect("exported state should be a JSON array");
    assert_eq!(array.len(), 2, "expected two exported state entries");
    assert_eq!(
        array[0].get("title").and_then(Value::as_str),
        Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D")
    );
    assert_eq!(
        array[1].get("title").and_then(Value::as_str),
        Some("sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB")
    );
}
#[test]
fn collect_state_norito_exports_hex_dump() {
    let entries = [sample_state_entry(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        vec![0xAB, 0xCD],
    )];
    let refs: Vec<&super::StateEntry> = entries.iter().collect();
    let dump = super::collect_state_norito(&refs).expect("export filtered state to norito");
    assert!(
        dump.contains("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"),
        "export should include the entry title"
    );
    let mut parts = dump.split(':');
    let _ = parts.next().expect("title prefix should be present");
    let hex_section = parts
        .next()
        .expect("hex suffix should be present")
        .trim()
        .to_owned();
    assert!(
        !hex_section.is_empty(),
        "hex section should not be empty for Norito export"
    );
    assert!(
        hex_section.chars().all(|c| c.is_ascii_hexdigit()),
        "hex section should contain only hexadecimal digits"
    );
}
#[test]
fn save_state_json_to_file_writes_filtered_entries() {
    let entries = [sample_state_entry(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        vec![0x01, 0x02],
    )];
    let refs: Vec<&super::StateEntry> = entries.iter().collect();
    let dir = tempfile::tempdir().expect("tempdir");
    let path = super::save_state_json_to_file(&refs, Some(dir.path())).expect("export state json");
    assert!(
        path.starts_with(dir.path()),
        "export path should reside within provided directory"
    );
    let written = std::fs::read_to_string(&path).expect("read exported state json");
    assert!(
        written.contains("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"),
        "exported JSON should include entry identifier"
    );
}
#[test]
fn save_state_json_to_file_rejects_empty_entries() {
    let entries: Vec<&super::StateEntry> = Vec::new();
    assert!(
        super::save_state_json_to_file(&entries, None).is_err(),
        "export should fail when no state entries are selected"
    );
}
#[test]
fn save_state_norito_to_file_writes_filtered_entries() {
    let entries = [sample_state_entry(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        vec![0x0A, 0x0B],
    )];
    let refs: Vec<&super::StateEntry> = entries.iter().collect();
    let dir = tempfile::tempdir().expect("tempdir");
    let path =
        super::save_state_norito_to_file(&refs, Some(dir.path())).expect("export state norito");
    assert!(
        path.starts_with(dir.path()),
        "export path should reside within provided directory"
    );
    let written = std::fs::read_to_string(&path).expect("read exported state norito");
    assert!(
        written.contains("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"),
        "exported Norito dump should include entry identifier"
    );
}
#[test]
fn save_state_norito_to_file_rejects_empty_entries() {
    let entries: Vec<&super::StateEntry> = Vec::new();
    assert!(
        super::save_state_norito_to_file(&entries, None).is_err(),
        "export should fail when no state entries are selected"
    );
}
#[test]
fn state_tab_select_page_updates_entries_and_remaining() {
    let mut tab = super::StateTabState::new(StateQueryKind::Accounts);
    let first = sample_state_entry(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        vec![0xAA],
    );
    let second = sample_state_entry(
        "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB",
        vec![0xBB],
    );
    tab.pages = vec![
        StatePageCache {
            entries: vec![first.clone()],
            remaining: 2,
        },
        StatePageCache {
            entries: vec![second.clone()],
            remaining: 0,
        },
    ];
    tab.select_page(0);
    assert_eq!(tab.entries.len(), 1, "expected a single entry on page 0");
    assert_eq!(
        tab.entries.first().map(|entry| entry.title.as_str()),
        Some("sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D"),
        "selecting first page should surface corresponding entries"
    );
    assert_eq!(
        tab.remaining,
        Some(2),
        "remaining counter should be preserved when greater than zero"
    );
    tab.select_page(1);
    assert_eq!(tab.entries.len(), 1, "expected a single entry on page 1");
    assert_eq!(
        tab.entries.first().map(|entry| entry.title.as_str()),
        Some("sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB"),
        "switching pages should update visible entries"
    );
    assert_eq!(
        tab.remaining, None,
        "remaining counter should drop to None when reported as zero"
    );
}
#[test]
fn state_tabs_reset_results_preserves_filters() {
    let mut tabs = super::StateTabs::default();
    let tab = tabs.get_mut(StateQueryKind::Accounts);
    tab.filter.search = "alice".to_owned();
    tab.entries.push(sample_state_entry(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        vec![0x01],
    ));
    tab.pages.push(StatePageCache {
        entries: vec![sample_state_entry(
            "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
            vec![0x02],
        )],
        remaining: 1,
    });
    let peer_tab = tabs.get_mut(StateQueryKind::Peers);
    peer_tab
        .entries
        .push(sample_state_entry("peer#0", vec![0x03]));
    peer_tab.pages.push(StatePageCache {
        entries: vec![sample_state_entry("peer#1", vec![0x04])],
        remaining: 0,
    });
    tabs.reset_results_for_all();
    let tab = tabs.get(StateQueryKind::Accounts);
    assert!(
        tab.entries.is_empty(),
        "reset should drop cached entries for each tab"
    );
    assert!(
        tab.pages.is_empty(),
        "reset should clear page caches for each tab"
    );
    assert_eq!(
        tab.filter.search, "alice",
        "reset should not discard the active search query"
    );
    let peer_tab = tabs.get(StateQueryKind::Peers);
    assert!(
        peer_tab.entries.is_empty(),
        "reset should drop cached entries for the peers tab"
    );
    assert!(
        peer_tab.pages.is_empty(),
        "reset should clear cached pages for the peers tab"
    );
}
#[test]
fn state_filter_adapts_peer_fields() {
    let mut filter = super::StateFilter {
        search: "peer".to_owned(),
        domain: "wonderland".to_owned(),
        owner: "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D".to_owned(),
        asset_definition: sample_rose_definition_literal(),
    };
    filter.adapt_to_kind(StateQueryKind::Peers);
    assert_eq!(
        filter.search, "peer",
        "adaptation should not clear the free-form search query"
    );
    assert!(
        filter.domain.is_empty(),
        "peer filter should not retain domain constraints"
    );
    assert!(
        filter.owner.is_empty(),
        "peer filter should not retain owner constraints"
    );
    assert!(
        filter.asset_definition.is_empty(),
        "peer filter should not retain asset definition constraints"
    );
}
#[test]
fn filter_state_entries_collects_cached_matches() {
    let entry_page0 = sample_state_entry(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        vec![0xAA],
    );
    let entry_page1 = sample_state_entry(
        "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB",
        vec![0xBB],
    );
    let pages = vec![
        StatePageCache {
            entries: vec![entry_page0.clone()],
            remaining: 1,
        },
        StatePageCache {
            entries: vec![entry_page1.clone()],
            remaining: 0,
        },
    ];
    let current_entries = vec![entry_page0];
    let (page_indices, cached_matches) = filter_state_entries(
        &pages,
        &current_entries,
        StateQueryKind::Accounts,
        Some("rrjxvb"),
        None,
        None,
        None,
    );
    assert!(
        page_indices.is_empty(),
        "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB is not present on the selected page"
    );
    assert_eq!(
        cached_matches.len(),
        1,
        "expected a cached match sourced from another page"
    );
    assert_eq!(
        cached_matches[0].title, "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB",
        "cached match should reference the sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB entry"
    );
}
#[test]
fn filter_state_entries_falls_back_to_current_page() {
    let entry_page0 = sample_state_entry(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        vec![0xAC],
    );
    let current_entries = vec![entry_page0.clone()];
    let pages: Vec<StatePageCache> = Vec::new();
    let (page_indices, cached_matches) = filter_state_entries(
        &pages,
        &current_entries,
        StateQueryKind::Accounts,
        Some("ggm2d"),
        None,
        None,
        None,
    );
    assert_eq!(
        page_indices,
        vec![0],
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D should be matched on the current page"
    );
    assert_eq!(
        cached_matches.len(),
        1,
        "fallback to current page should return a single match"
    );
    assert_eq!(
        cached_matches[0].title, "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        "cached results should include the local page entry"
    );
}
#[test]
fn filter_state_entries_respects_domain_filter() {
    let entry_page0 = sample_state_entry_with_domain(
        "sorauﾛ1PaQｽGh1ｴ6pAﾜnqｸfJuｿMﾑVqﾏvQﾐﾚｼｾﾋaﾈｳﾊc1ｺﾊ1GGM2D",
        "wonderland",
        vec![0xDE, 0x01],
    );
    let entry_page1 = sample_state_entry_with_domain(
        "sorauﾛ1PｸCｶrﾑhyﾜｴﾄhｳﾔSqP2GFGﾗヱﾐｹﾇﾏzﾍｵﾐMﾇﾖﾄksJヱRRJXVB",
        "narnia",
        vec![0xDE, 0x02],
    );
    let pages = vec![
        StatePageCache {
            entries: vec![entry_page0.clone()],
            remaining: 1,
        },
        StatePageCache {
            entries: vec![entry_page1.clone()],
            remaining: 0,
        },
    ];
    let current_entries = vec![entry_page0];
    let (page_indices, cached_matches) = filter_state_entries(
        &pages,
        &current_entries,
        StateQueryKind::Accounts,
        None,
        Some("narnia"),
        None,
        None,
    );
    assert!(
        page_indices.is_empty(),
        "domain filter should skip non-matching entries on the current page"
    );
    assert_eq!(
        cached_matches.len(),
        1,
        "domain filter should surface matches from cached pages"
    );
    assert_eq!(
        cached_matches[0].domain.as_deref(),
        Some("narnia"),
        "matched entry should report the requested domain"
    );
}
#[test]
fn collect_log_text_joins_lines() {
    let entries = vec![
        (0usize, "[alpha] started".to_owned()),
        (1, "[alpha] running".to_owned()),
    ];
    let text = super::collect_log_text(&entries).expect("export log lines");
    assert_eq!(text, "[alpha] started\n[alpha] running");
}
include!("gui/collect_log_text_empty_test.rs");
#[test]
fn save_logs_to_file_writes_filtered_entries() {
    let entries = vec![
        (0usize, "[alpha] started".to_owned()),
        (1, "[alpha] running".to_owned()),
    ];
    let dir = tempfile::tempdir().expect("tempdir");
    let path = super::save_logs_to_file(&entries, Some(dir.path())).expect("save filtered logs");
    assert!(
        path.starts_with(dir.path()),
        "export should write inside the provided directory"
    );
    let written = std::fs::read_to_string(&path).expect("read exported logs");
    assert!(written.contains("[alpha] started"));
    assert!(written.contains("[alpha] running"));
}
#[test]
fn save_logs_to_file_rejects_empty_entries() {
    let entries: Vec<(usize, String)> = Vec::new();
    assert!(
        super::save_logs_to_file(&entries, None).is_err(),
        "export should fail with no matching logs"
    );
}
#[test]
fn log_kind_filter_respects_settings() {
    let mut app = MochiApp::default();
    app.settings_log_stdout = false;
    let stdout_event = PeerLogEvent::Line {
        alias: Arc::from("alpha"),
        kind: LogStreamKind::Stdout,
        timestamp_ms: 0,
        message: "stdout".to_owned(),
    };
    assert!(
        !app.is_log_kind_enabled(MochiApp::log_event_kind(&stdout_event)),
        "stdout events should be hidden when the toggle is disabled"
    );
    let system_event = PeerLogEvent::Lifecycle {
        alias: Arc::from("alpha"),
        event: LifecycleEvent::Started { attempt: 0 },
        timestamp_ms: 0,
    };
    assert!(
        app.is_log_kind_enabled(MochiApp::log_event_kind(&system_event)),
        "system events remain visible by default"
    );
}
#[test]
fn event_filter_honours_alias_filters() {
    let mut filter = EventFilterState::default();
    let line = RenderedEventLine::new(
        "alpha",
        "[alpha] Text frame".to_owned(),
        None,
        Color32::from_gray(190),
        RenderedEventKind::Text,
    );
    assert!(
        filter.matches(&line, None),
        "default filter should allow events"
    );
    filter.toggle_alias("alpha", false);
    assert!(
        !filter.matches(&line, None),
        "disabled alias should be filtered out"
    );
    filter.toggle_alias("alpha", true);
    assert!(
        filter.matches(&line, None),
        "re-enabled alias should match again"
    );
}
#[test]
fn event_filter_supports_multiple_peer_toggles() {
    let mut filter = EventFilterState::default();
    let alpha = RenderedEventLine::new(
        "alpha",
        "[alpha] text frame".to_owned(),
        None,
        Color32::from_gray(190),
        RenderedEventKind::Text,
    );
    let beta = RenderedEventLine::new(
        "beta",
        "[beta] text frame".to_owned(),
        None,
        Color32::from_gray(190),
        RenderedEventKind::Text,
    );
    assert!(filter.matches(&alpha, None));
    assert!(filter.matches(&beta, None));
    filter.toggle_alias("beta", false);
    assert!(
        filter.matches(&alpha, None),
        "unfiltered alias should remain visible"
    );
    assert!(
        !filter.matches(&beta, None),
        "disabled alias should be hidden"
    );
}
#[test]
fn event_filter_alias_toggle_is_case_insensitive() {
    let mut filter = EventFilterState::default();
    filter.toggle_alias("BETA", false);
    let beta = RenderedEventLine::new(
        "beta",
        "[beta] text frame".to_owned(),
        None,
        Color32::from_gray(190),
        RenderedEventKind::Text,
    );
    assert!(
        !filter.matches(&beta, None),
        "alias filters should treat names case-insensitively"
    );
}
#[test]
fn event_filter_serializes_and_restores_state() {
    let mut filter = EventFilterState {
        search: "Hash:ABC".to_owned(),
        show_decode_errors: false,
        ..EventFilterState::default()
    };
    filter.toggle_alias("alpha", false);
    let serialized = serialize_event_filter(&filter).expect("filter should serialize");
    let parsed: Value =
        json::from_str(&serialized).expect("serialized filter should be valid JSON");
    let restored =
        EventFilterState::from_json_value(&parsed).expect("filter should restore from JSON");
    assert_eq!(restored.search, filter.search);
    assert_eq!(restored.show_decode_errors, filter.show_decode_errors);
    assert!(
        !restored.alias_selected("alpha"),
        "alias selection should persist with lowercasing"
    );

    for mutation in ["missing", "unknown", "wrong_type", "noncanonical_alias"] {
        let mut invalid = parsed.clone();
        let map = invalid.as_object_mut().expect("filter object");
        match mutation {
            "missing" => {
                map.remove("show_data");
            }
            "unknown" => {
                map.insert("legacy".to_owned(), Value::Bool(true));
            }
            "wrong_type" => {
                map.insert("show_data".to_owned(), Value::String("true".to_owned()));
            }
            "noncanonical_alias" => {
                map.insert(
                    "alias_filters".to_owned(),
                    Value::Array(vec![Value::String("Alpha".to_owned())]),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            EventFilterState::from_json_value(&invalid).is_none(),
            "mutation {mutation} must be rejected"
        );
    }
}
#[test]
fn persisted_scalar_values_require_exact_first_release_spelling() {
    assert_eq!(
        ActiveView::from_storage_value(ActiveView::Activity.storage_value()),
        Some(ActiveView::Activity)
    );
    assert!(ActiveView::from_storage_value(" activity ").is_none());
    assert!(parse_first_run_completed("true"));
    for invalid in [" true ", "TRUE", "1", "false"] {
        assert!(!parse_first_run_completed(invalid));
    }
}
#[test]
fn collect_dashboard_metrics_counts_resources() {
    let mut app = MochiApp::default();
    let peer_rows = vec![
        PeerRow {
            alias: "alpha".to_owned(),
            state: PeerState::Running,
            torii: "http://alpha".to_owned(),
            api_base: Some("http://alpha".to_owned()),
            api_error: None,
            config: "config-alpha".to_owned(),
            logs: "logs-alpha".to_owned(),
        },
        PeerRow {
            alias: "beta".to_owned(),
            state: PeerState::Stopped,
            torii: "http://beta".to_owned(),
            api_base: Some("http://beta".to_owned()),
            api_error: None,
            config: "config-beta".to_owned(),
            logs: "logs-beta".to_owned(),
        },
    ];
    let summary = BlockSummary {
        height: 5,
        hash_hex: "hash".to_owned(),
        transaction_count: 3,
        rejected_transaction_count: 1,
        time_trigger_count: 0,
        signature_count: 2,
        view_change_index: 0,
        creation_time_ms: 42,
        is_genesis: false,
    };
    let block_snapshot = BlockStreamSnapshot {
        connected: true,
        last_summary: Some(summary),
        ..Default::default()
    };
    app.block_snapshots
        .insert("alpha".to_owned(), block_snapshot);
    app.block_events.push(DisplayEvent {
        alias: Some("alpha".to_owned()),
        event: BlockStreamEvent::Text {
            text: "started".to_owned(),
        },
    });
    let event_snapshot = EventSnapshot {
        connected: true,
        ..Default::default()
    };
    app.event_snapshots
        .insert("alpha".to_owned(), event_snapshot);
    app.event_events.push(EventDisplay {
        alias: Some("alpha".to_owned()),
        event: EventStreamEvent::Text {
            text: "ping".to_owned(),
        },
    });
    app.log_events
        .push(MochiApp::system_log_event("alpha", "log".to_owned()));
    let metrics = app.collect_dashboard_metrics(&peer_rows);
    assert_eq!(metrics.total_peers, 2);
    assert_eq!(metrics.running_peers, 1);
    assert_eq!(metrics.connected_block_streams, 1);
    assert_eq!(metrics.connected_event_streams, 1);
    assert_eq!(metrics.latest_height, Some(5));
    assert_eq!(metrics.total_tx, 3);
    assert_eq!(metrics.total_rejected_tx, 1);
    assert_eq!(metrics.pending_block_events, 1);
    assert_eq!(metrics.pending_event_frames, 1);
    assert_eq!(metrics.stored_logs, 1);
    assert!(metrics.avg_queue.is_none());
    assert!(metrics.avg_commit_latency_ms.is_none());
}
#[test]
fn peer_status_view_captures_metrics_and_errors() {
    let mut view = PeerStatusView::default();
    let now = Instant::now();
    let initial = TelemetryStatus {
        build: Default::default(),
        peers: 2,
        blocks: 10,
        blocks_non_empty: 8,
        commit_time_ms: 45,
        txs_approved: 5,
        txs_rejected: 1,
        last_rejection_at_ms: None,
        txs_rejected_recent_5m: 0,
        uptime: Uptime(Duration::from_secs(5)),
        view_changes: 0,
        queue_size: 4,
        crypto: Default::default(),
        stack: Default::default(),
        sumeragi: None,
        governance: GovernanceStatus::default(),
        teu_lane_commit: Vec::new(),
        teu_dataspace_backlog: Vec::new(),
        tx_gossip: TxGossipSnapshot::default(),
        taikai_ingest: Vec::new(),
        taikai_alias_rotations: Vec::new(),
        da_receipt_cursors: Vec::new(),
        ..TelemetryStatus::default()
    };
    let mut sumeragi_initial = sample_sumeragi_status_wire();
    sumeragi_initial.height = 21;
    let initial_snapshot = ToriiStatusSnapshot {
        timestamp: now,
        status: initial.clone(),
        metrics: StatusMetrics::from_samples(None, &initial),
    };
    view.record_snapshot(
        initial_snapshot,
        Some(sumeragi_initial),
        Some(sample_sumeragi_diagnostics()),
        None,
        None,
        now,
    );
    assert!(view.delta_summary().is_none());
    let (label, color) = view.status_label();
    assert!(label.contains("peers=2"));
    assert!(label.contains("queue=4"));
    assert!(label.contains("commit=45ms"));
    assert_eq!(color, Color32::from_rgb(80, 160, 80));
    let membership_summary = view.membership_summary().expect("membership summary");
    assert!(membership_summary.contains("h21"));
    assert!(membership_summary.contains("leader 1"));
    let updated = TelemetryStatus {
        build: Default::default(),
        peers: 3,
        blocks: 11,
        blocks_non_empty: 9,
        commit_time_ms: 120,
        txs_approved: 9,
        txs_rejected: 3,
        last_rejection_at_ms: Some(6_000),
        txs_rejected_recent_5m: 3,
        uptime: Uptime(Duration::from_secs(6)),
        view_changes: 1,
        queue_size: 9,
        crypto: Default::default(),
        stack: Default::default(),
        sumeragi: None,
        governance: GovernanceStatus::default(),
        teu_lane_commit: Vec::new(),
        teu_dataspace_backlog: Vec::new(),
        tx_gossip: TxGossipSnapshot::default(),
        taikai_ingest: Vec::new(),
        taikai_alias_rotations: Vec::new(),
        da_receipt_cursors: Vec::new(),
        ..TelemetryStatus::default()
    };
    let mut sumeragi_updated = sample_sumeragi_status_wire();
    sumeragi_updated.height = 30;
    let updated_snapshot = ToriiStatusSnapshot {
        timestamp: now + Duration::from_secs(2),
        status: updated.clone(),
        metrics: StatusMetrics::from_samples(Some(&initial), &updated),
    };
    view.record_snapshot(
        updated_snapshot,
        Some(sumeragi_updated),
        Some(sample_sumeragi_diagnostics()),
        None,
        None,
        now + Duration::from_secs(2),
    );
    let delta = view.delta_summary().expect("delta summary");
    assert!(delta.contains("tx +4 / -2"));
    assert!(delta.contains("queue +5"));
    assert!(delta.contains("view +1"));
    let (label, color) = view.status_label();
    assert!(label.contains("peers=3"));
    assert!(label.contains("commit=120ms"));
    assert_eq!(color, Color32::from_rgb(200, 160, 64));
    let membership_summary = view.membership_summary().expect("membership summary");
    assert!(membership_summary.contains("h30"));
    assert!(membership_summary.contains("committed 9"));
    let err_info = ToriiError::Decode("bad payload".to_owned()).summarize();
    view.record_error(err_info, now + Duration::from_secs(3));
    let (label, color) = view.status_label();
    assert!(label.to_ascii_lowercase().contains("decode"));
    assert_eq!(color, Color32::from_rgb(200, 160, 64));
    assert!(view.membership_summary().is_some());
}
#[test]
fn peer_status_view_surfaces_sealed_lanes() {
    let mut view = PeerStatusView::default();
    let now = Instant::now();
    let status = TelemetryStatus {
        build: Default::default(),
        peers: 2,
        blocks: 10,
        blocks_non_empty: 8,
        commit_time_ms: 42,
        txs_approved: 4,
        txs_rejected: 0,
        last_rejection_at_ms: None,
        txs_rejected_recent_5m: 0,
        uptime: Uptime(Duration::from_secs(5)),
        view_changes: 0,
        queue_size: 3,
        crypto: Default::default(),
        stack: Default::default(),
        sumeragi: None,
        governance: GovernanceStatus::default(),
        teu_lane_commit: Vec::new(),
        teu_dataspace_backlog: Vec::new(),
        tx_gossip: TxGossipSnapshot::default(),
        taikai_ingest: Vec::new(),
        taikai_alias_rotations: Vec::new(),
        da_receipt_cursors: Vec::new(),
        ..TelemetryStatus::default()
    };
    let snapshot = ToriiStatusSnapshot {
        timestamp: now,
        status: status.clone(),
        metrics: StatusMetrics::from_samples(None, &status),
    };
    let sumeragi = sample_sumeragi_status_wire();
    let mut diagnostics = sample_sumeragi_diagnostics();
    diagnostics.lane_governance_sealed_total = 2;
    diagnostics.lane_governance_sealed_aliases = vec![
        "archive".to_owned(),
        "payments".to_owned(),
        "vip".to_owned(),
        "ops".to_owned(),
        "extra".to_owned(),
    ];
    view.record_snapshot(snapshot, Some(sumeragi), Some(diagnostics), None, None, now);
    let (label, color) = view.status_label();
    assert!(
        label.contains("sealed=2"),
        "label should surface sealed lane count: {label}"
    );
    assert_eq!(
        color,
        Color32::from_rgb(220, 140, 80),
        "status color should downgrade to amber when lanes remain sealed"
    );
    let summary = view.sealed_summary().expect("sealed summary");
    assert!(
        summary.contains("Sealed lanes: 2"),
        "summary should include sealed count: {summary}"
    );
    assert!(
        summary.contains("… +2"),
        "summary should collapse additional aliases: {summary}"
    );
}
#[test]
fn lane_status_rows_surface_relay_lag_and_cursor() {
    let mut view = PeerStatusView::default();
    let now = Instant::now();
    let status = TelemetryStatus {
        da_receipt_cursors: vec![iroha_telemetry::metrics::DaReceiptCursorStatus {
            lane_id: 0,
            epoch: 2,
            highest_sequence: 7,
        }],
        ..TelemetryStatus::default()
    };
    let snapshot = ToriiStatusSnapshot {
        timestamp: now,
        status: status.clone(),
        metrics: StatusMetrics::from_samples(None, &status),
    };
    let sumeragi = sample_sumeragi_status_wire();
    let mut diagnostics = sample_sumeragi_diagnostics();
    let header = BlockHeader::new(NonZeroU64::new(9).expect("height"), None, None, None, 0, 0);
    let settlement = iroha_data_model::block::consensus::LaneBlockCommitment {
        block_height: 9,
        lane_id: LaneId::new(0),
        lane_incarnation: Hash::new(b"lane-block-commitment-incarnation"),
        dataspace_id: DataSpaceId::new(0),
        tx_count: 1,
        total_local_amount: "0".parse().expect("valid settlement quantity"),
        total_xor_due: "0".parse().expect("valid settlement quantity"),
        total_xor_after_haircut: "0".parse().expect("valid settlement quantity"),
        total_xor_variance: "0".parse().expect("valid settlement quantity"),
        swap_metadata: None,
        receipts: Vec::new(),
        nexus_fee_receipts: Vec::new(),
        native_amx_receipts: Vec::new(),
    };
    let envelope = LaneRelayEnvelope::new(header, None, settlement, 256).expect("envelope");
    diagnostics.lane_relay_envelopes = vec![envelope];
    view.record_snapshot(snapshot, Some(sumeragi), Some(diagnostics), None, None, now);
    let rows = view.lane_status_rows(&lane_catalog_snapshot(None));
    assert_eq!(rows.len(), 1);
    let row = &rows[0];
    assert_eq!(row.lane_id, 0);
    assert_eq!(row.alias, "alpha");
    assert_eq!(row.relay_lag, Some(1));
    assert_eq!(row.rbc_bytes, Some(384));
    assert_eq!(row.da_cursor_label(), "e2 s7");
    assert!(matches!(row.relay_state, RelayIngestState::MissingFinality));
}
#[test]
fn composer_update_success_records_message() {
    let mut app = MochiApp::default();
    app.composer_submitting = true;
    app.handle_composer_update(ComposerSubmitUpdate {
        peer: "alpha".to_owned(),
        result: Ok("hash123".to_owned()),
    });
    assert!(!app.composer_submitting);
    assert_eq!(
        app.composer_submit_success.as_deref(),
        Some("Submitted transaction hash123 to alpha.")
    );
    assert!(app.composer_submit_error.is_none());
    assert_eq!(
        app.last_info.as_deref(),
        Some("Submitted transaction hash123 to alpha.")
    );
    assert!(app.last_error.is_none());
    assert_eq!(app.active_view, ActiveView::Activity);
    assert_eq!(app.activity_view, ActivityView::Events);
    assert_eq!(app.event_selected_peer.as_deref(), Some("alpha"));
    assert_eq!(app.event_filter.search, "hash123");
    assert!(app.auto_event_stream);
}
#[test]
fn composer_update_failure_records_error() {
    let mut app = MochiApp::default();
    app.composer_submitting = true;
    let info = ToriiErrorInfo::new(ToriiErrorKind::HttpTransport, "network error");
    app.handle_composer_update(ComposerSubmitUpdate {
        peer: "beta".to_owned(),
        result: Err(info),
    });
    assert!(!app.composer_submitting);
    assert!(app.composer_submit_success.is_none());
    assert_eq!(
        app.composer_submit_error
            .as_ref()
            .map(|error| error.message.as_str()),
        Some("network error")
    );
    assert!(app.last_info.is_none());
    assert_eq!(
        app.last_error.as_deref(),
        Some("Failed to submit transaction to beta: network error")
    );
    assert_eq!(app.active_view, ActiveView::Activity);
    assert_eq!(app.activity_view, ActivityView::Logs);
    assert_eq!(app.log_selected_peer.as_deref(), Some("beta"));
    assert!(app.auto_log_stream);
}
#[test]
fn add_instruction_to_batch_appends_draft() {
    let mut app = MochiApp::default();
    app.composer_selected_signer = Some(0);
    let asset = AssetId::new(sample_rose_definition_id(), ALICE_ID.clone());
    app.composer_asset_id = asset_literal(&asset);
    app.composer_quantity = "5".to_owned();
    app.add_instruction_to_batch(None);
    assert_eq!(app.composer_drafts.len(), 1);
    assert!(app.composer_error.is_none());
}
#[test]
fn transfer_without_destination_records_error() {
    let mut app = MochiApp::default();
    app.composer_instruction_kind = ComposerInstructionKind::TransferAsset;
    app.composer_selected_signer = Some(0);
    let asset = AssetId::new(sample_rose_definition_id(), ALICE_ID.clone());
    app.composer_asset_id = asset_literal(&asset);
    app.composer_quantity = "1".to_owned();
    app.add_instruction_to_batch(None);
    assert!(app.composer_drafts.is_empty(), "draft should not be added");
    assert!(
        app.composer_error
            .as_deref()
            .unwrap_or_default()
            .contains("Destination account"),
        "expected destination error message"
    );
}
#[test]
fn add_instruction_respects_signer_permissions() {
    let mut app = MochiApp::default();
    app.composer_instruction_kind = ComposerInstructionKind::RegisterAccount;
    app.composer_account_id = sample_account_id(SAMPLE_OTHER_PUBLIC_KEY);
    // Bob is the second development signer and lacks register-account permission.
    app.composer_selected_signer = Some(1);
    app.add_instruction_to_batch(None);
    assert!(
        app.composer_drafts.is_empty(),
        "unauthorised draft should not be added"
    );
    let message = app.composer_error.as_deref().unwrap_or_default().to_owned();
    assert!(
        message.contains("cannot register accounts"),
        "expected permission error, got `{message}`"
    );
}
#[test]
fn composer_template_prefills_mint_inputs() {
    if !super::socket_bind_available() {
        eprintln!("Skipping mint template test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
    let config_dir = temp.path().join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("local.toml");
    fs::write(&config_path, "[supervisor]\n").expect("write config stub");
    let data_root = temp
        .path()
        .join(format!("mochi-data-{}", std::process::id()));
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    app.composer_selected_signer = Some(0);
    let signers = development_signing_authorities();
    app.apply_composer_template(ComposerTemplate::MintRoseToSigner, signers);
    assert_eq!(
        app.composer_instruction_kind,
        ComposerInstructionKind::MintAsset
    );
    assert!(
        app.composer_asset_id
            .contains(&sample_rose_definition_literal()),
        "expected rose asset id, got {}",
        app.composer_asset_id
    );
    assert_eq!(app.composer_quantity, "10");
    assert!(
        app.composer_destination_account.is_empty(),
        "mint template should not set destination"
    );
    assert!(
        app.last_info
            .as_deref()
            .unwrap_or_default()
            .contains("rose mint"),
        "should surface mint template info banner"
    );
}
#[test]
fn composer_template_prefills_burn_inputs() {
    if !super::socket_bind_available() {
        eprintln!("Skipping burn template test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
    let config_dir = temp.path().join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("local.toml");
    fs::write(&config_path, "[supervisor]\n").expect("write config stub");
    let data_root = temp
        .path()
        .join(format!("mochi-data-{}", std::process::id()));
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    app.composer_selected_signer = Some(0);
    let signers = development_signing_authorities();
    app.apply_composer_template(ComposerTemplate::BurnRoseFromSigner, signers);
    assert_eq!(
        app.composer_instruction_kind,
        ComposerInstructionKind::BurnAsset
    );
    assert!(
        app.composer_asset_id
            .contains(&sample_rose_definition_literal()),
        "expected rose asset id, got {}",
        app.composer_asset_id
    );
    assert_eq!(app.composer_quantity, "1");
    assert!(
        app.composer_destination_account.is_empty(),
        "burn template should not set destination"
    );
    assert!(
        app.last_info
            .as_deref()
            .unwrap_or_default()
            .contains("burn template"),
        "should surface burn template info banner"
    );
}
#[test]
fn composer_template_prefills_transfer_inputs() {
    if !super::socket_bind_available() {
        eprintln!("Skipping transfer template test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
    let config_dir = temp.path().join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("local.toml");
    fs::write(&config_path, "[supervisor]\n").expect("write config stub");
    let data_root = temp
        .path()
        .join(format!("mochi-data-{}", std::process::id()));
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    app.composer_selected_signer = Some(0);
    let signers = development_signing_authorities();
    app.apply_composer_template(ComposerTemplate::TransferRoseToTeammate, signers);
    assert_eq!(
        app.composer_instruction_kind,
        ComposerInstructionKind::TransferAsset
    );
    assert!(
        app.composer_asset_id
            .contains(&sample_rose_definition_literal()),
        "expected rose asset id, got {}",
        app.composer_asset_id
    );
    assert_eq!(app.composer_quantity, "2");
    assert!(
        !app.composer_destination_account.is_empty(),
        "transfer template must set a destination account"
    );
    assert_ne!(
        app.composer_destination_account,
        account_literal(signers[0].account_id()),
        "destination should differ from the source signer"
    );
}
#[test]
fn queue_plot_points_returns_points() {
    let mut app = MochiApp::default();
    let base = Instant::now();
    let mut history = VecDeque::new();
    let status_a = TelemetryStatus {
        queue_size: 1,
        txs_approved: 2,
        ..Default::default()
    };
    let snapshot_a = ToriiStatusSnapshot {
        timestamp: base,
        status: status_a.clone(),
        metrics: StatusMetrics::from_samples(None, &status_a),
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base,
        snapshot: snapshot_a,
        metrics: None,
    });
    let mut status_b = status_a.clone();
    status_b.queue_size = 4;
    status_b.txs_approved = 5;
    let snapshot_b = ToriiStatusSnapshot {
        timestamp: base + Duration::from_secs(1),
        status: status_b.clone(),
        metrics: StatusMetrics::from_samples(Some(&status_a), &status_b),
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base + Duration::from_secs(1),
        snapshot: snapshot_b,
        metrics: None,
    });
    app.status_history.insert("alpha".to_owned(), history);
    assert!(app.queue_plot_points("alpha").is_some());
}
#[test]
fn commit_latency_plot_points_require_multiple_samples() {
    let mut app = MochiApp::default();
    assert!(
        app.commit_latency_plot_points("beta").is_none(),
        "no history should produce no plot"
    );
    let base = Instant::now();
    let mut history = VecDeque::new();
    let status_a = TelemetryStatus {
        commit_time_ms: 75,
        queue_size: 3,
        ..Default::default()
    };
    let snapshot_a = ToriiStatusSnapshot {
        timestamp: base,
        status: status_a.clone(),
        metrics: StatusMetrics::from_samples(None, &status_a),
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base,
        snapshot: snapshot_a,
        metrics: None,
    });
    app.status_history.insert("beta".to_owned(), history);
    assert!(
        app.commit_latency_plot_points("beta").is_none(),
        "a single sample should not emit a plot"
    );
    let mut status_b = status_a.clone();
    status_b.commit_time_ms = 140;
    status_b.queue_size = 5;
    let snapshot_b = ToriiStatusSnapshot {
        timestamp: base + Duration::from_secs(1),
        status: status_b.clone(),
        metrics: StatusMetrics::from_samples(Some(&status_a), &status_b),
    };
    app.status_history
        .get_mut("beta")
        .expect("history must exist")
        .push_back(StatusHistoryEntry {
            timestamp: base + Duration::from_secs(1),
            snapshot: snapshot_b,
            metrics: None,
        });
    assert!(
        app.commit_latency_plot_points("beta").is_some(),
        "two samples should produce a commit latency plot"
    );
}
#[test]
fn throughput_plot_points_returns_points() {
    let mut app = MochiApp::default();
    let base = Instant::now();
    let mut history = VecDeque::new();
    let status_a = TelemetryStatus {
        txs_approved: 8,
        ..Default::default()
    };
    let snapshot_a = ToriiStatusSnapshot {
        timestamp: base,
        status: status_a.clone(),
        metrics: StatusMetrics::from_samples(None, &status_a),
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base,
        snapshot: snapshot_a,
        metrics: None,
    });
    let mut status_b = status_a.clone();
    status_b.txs_approved = 12;
    let snapshot_b = ToriiStatusSnapshot {
        timestamp: base + Duration::from_secs(2),
        status: status_b.clone(),
        metrics: StatusMetrics::from_samples(Some(&status_a), &status_b),
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base + Duration::from_secs(2),
        snapshot: snapshot_b,
        metrics: None,
    });
    app.status_history.insert("alpha".to_owned(), history);
    assert!(
        app.throughput_plot_points("alpha").is_some(),
        "two samples should produce throughput points"
    );
}
#[test]
fn consensus_queue_plot_points_require_metrics() {
    let mut app = MochiApp::default();
    let base = Instant::now();
    let mut history = VecDeque::new();
    let status = TelemetryStatus::default();
    let snapshot_a = ToriiStatusSnapshot {
        timestamp: base,
        status: status.clone(),
        metrics: StatusMetrics::from_samples(None, &status),
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base,
        snapshot: snapshot_a,
        metrics: Some(sample_metrics_snapshot(base, 2.0, 8.0)),
    });
    let snapshot_b = ToriiStatusSnapshot {
        timestamp: base + Duration::from_secs(1),
        status,
        metrics: StatusMetrics::default(),
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base + Duration::from_secs(1),
        snapshot: snapshot_b,
        metrics: Some(sample_metrics_snapshot(
            base + Duration::from_secs(1),
            5.0,
            8.0,
        )),
    });
    app.status_history.insert("alpha".to_owned(), history);
    assert!(
        app.consensus_queue_plot_points("alpha").is_some(),
        "two metrics samples should produce consensus queue points"
    );
}
#[test]
fn view_change_plot_points_record_deltas() {
    let mut app = MochiApp::default();
    let base = Instant::now();
    let mut history = VecDeque::new();
    let status_a = TelemetryStatus {
        view_changes: 3,
        ..Default::default()
    };
    let mut metrics_a = StatusMetrics::from_samples(None, &status_a);
    metrics_a.sample_interval_ms = 1_000;
    let snapshot_a = ToriiStatusSnapshot {
        timestamp: base,
        status: status_a.clone(),
        metrics: metrics_a,
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base,
        snapshot: snapshot_a,
        metrics: None,
    });
    let mut status_b = status_a.clone();
    status_b.view_changes = 6;
    let mut metrics_b = StatusMetrics::from_samples(Some(&status_a), &status_b);
    metrics_b.sample_interval_ms = 1_000;
    let snapshot_b = ToriiStatusSnapshot {
        timestamp: base + Duration::from_secs(1),
        status: status_b,
        metrics: metrics_b,
    };
    history.push_back(StatusHistoryEntry {
        timestamp: base + Duration::from_secs(1),
        snapshot: snapshot_b,
        metrics: None,
    });
    app.status_history.insert("alpha".to_owned(), history);
    assert!(
        app.view_change_plot_points("alpha").is_some(),
        "non-zero view change deltas should produce points"
    );
}
#[test]
fn peer_status_view_summarises_metrics() {
    let mut view = PeerStatusView::default();
    let snapshot = ToriiStatusSnapshot {
        timestamp: Instant::now(),
        status: TelemetryStatus::default(),
        metrics: StatusMetrics::default(),
    };
    view.record_snapshot(
        snapshot.clone(),
        None,
        None,
        Some(sample_metrics_snapshot(Instant::now(), 3.0, 10.0)),
        None,
        Instant::now(),
    );
    let summary = view
        .consensus_queue_summary()
        .expect("consensus summary expected");
    assert!(
        summary.contains("3"),
        "summary should include queue depth: {summary}"
    );
    let storage = view.storage_summary().expect("storage summary expected");
    assert!(
        storage.contains("Tiered state"),
        "storage string should include Tiered state label"
    );
    view.record_snapshot(
        snapshot,
        None,
        None,
        None,
        Some(ToriiErrorInfo::new(
            ToriiErrorKind::HttpTransport,
            "Metrics unavailable",
        )),
        Instant::now(),
    );
    let (message, _) = view.metrics_error_label().expect("metrics error label");
    assert!(
        message.contains("Metrics"),
        "metrics error label should include prefix"
    );
}
fn sample_metrics_snapshot(timestamp: Instant, depth: f64, capacity: f64) -> ToriiMetricsSnapshot {
    ToriiMetricsSnapshot {
        timestamp,
        queue_size: None,
        view_changes: None,
        sumeragi_tx_queue_depth: Some(depth),
        sumeragi_tx_queue_capacity: Some(capacity),
        sumeragi_tx_queue_saturated: None,
        state_tiered_hot_entries: Some(4.0),
        state_tiered_cold_entries: Some(2.0),
        state_tiered_cold_bytes: Some(1024.0),
        uptime_since_genesis_ms: None,
    }
}
#[test]
fn peer_state_color_matches_palette() {
    let palette = MochiApp::palette();
    assert_eq!(
        MochiApp::peer_state_color(PeerState::Running),
        palette.success
    );
    assert_eq!(
        MochiApp::peer_state_color(PeerState::Stopped),
        palette.danger
    );
    assert_eq!(
        MochiApp::peer_state_color(PeerState::Restarting),
        palette.warning
    );
}
include!("gui/settings_tail_tests.rs");
