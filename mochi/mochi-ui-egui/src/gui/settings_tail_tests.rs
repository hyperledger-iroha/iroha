use std::{fs, path::PathBuf};
use super::*;
#[test]
fn applying_settings_persists_config_and_rebuilds_supervisor() {
    if !super::socket_bind_available() {
        eprintln!("Skipping settings persistence test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let config_dir = temp.path().join("config");
    fs::create_dir_all(&config_dir).expect("config dir");
    let config_path = config_dir.join("local.toml");
    fs::write(&config_path, "[supervisor]\n").expect("write starter config");
    let kagami_log = temp.path().join("kagami_settings.log");
    let _log_guard = TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &kagami_log);
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let _config_guard = TestEnvGuard::set("MOCHI_CONFIG", &config_path);
    let initial_root = temp
        .path()
        .join(format!("mochi-data-{}", std::process::id()));
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &initial_root);
    reset_cli_overrides_for_tests();
    let mut app = MochiApp::default();
    let resolved_path = app
        .bundle_config
        .as_ref()
        .map(|cfg| cfg.path.clone())
        .unwrap_or_else(|| config_path.clone());
    let new_root = temp.path().join("custom-root");
    app.settings_data_root_input = new_root.display().to_string();
    app.settings_torii_port_input = "15000".to_owned();
    app.settings_p2p_port_input = "16000".to_owned();
    app.settings_chain_id_input = "custom-chain".to_owned();
    app.settings_profile_input = "{ peer_count = 7, consensus_mode = \"npos\" }".to_owned();
    app.settings_nexus_enabled = true;
    app.settings_nexus_lane_count_input = "2".to_owned();
    app.settings_nexus_lane_catalog_input =
        "[[lane_catalog]]\nindex = 0\nalias = \"core\"\ndataspace = \"universal\"\nmetadata = {}"
            .to_owned();
    app.settings_nexus_dataspace_catalog_input =
        "[[dataspace_catalog]]\nalias = \"universal\"\nid = 0".to_owned();
    let export_dir = temp.path().join("log-export");
    app.settings_log_export_dir_input = export_dir.display().to_string();
    let state_export_dir = temp.path().join("state-export");
    app.settings_state_export_dir_input = state_export_dir.display().to_string();
    app.apply_settings_changes_with_restart(false)
        .expect("settings persistence should succeed");
    assert!(genesis_invocation_count(&kagami_log) >= 2);
    assert!(kagami_sign_invocation_count(&kagami_log) >= 2);
    let bundle = app
        .bundle_config
        .as_ref()
        .expect("bundle config should be tracked after apply");
    if bundle.path != resolved_path {
        let expected_suffix = Path::new("config").join("local.toml");
        assert!(
            bundle.path.ends_with(&expected_suffix),
            "bundle config path should match override or default; got {}, expected {}",
            bundle.path.display(),
            resolved_path.display()
        );
    }
    assert_eq!(
        bundle.config.workspace_root.as_deref(),
        Some(new_root.as_path())
    );
    assert!(bundle.config.data_root.is_none());
    assert_eq!(bundle.config.torii_start, Some(15000));
    assert_eq!(bundle.config.p2p_start, Some(16000));
    assert_eq!(bundle.config.chain_id.as_deref(), Some("custom-chain"));
    let profile = bundle.config.profile.as_ref().expect("profile config");
    assert_eq!(profile.preset, None);
    assert_eq!(profile.topology.peer_count, 7);
    assert_eq!(profile.consensus_mode, SumeragiConsensusMode::Npos);
    let nexus = bundle.config.nexus.as_ref().expect("nexus config");
    assert_eq!(
        nexus.get("enabled").and_then(TomlValue::as_bool),
        Some(true)
    );
    assert_eq!(
        nexus.get("lane_count").and_then(TomlValue::as_integer),
        Some(2)
    );
    let lane_catalog = nexus
        .get("lane_catalog")
        .and_then(TomlValue::as_array)
        .expect("lane catalog array");
    assert_eq!(lane_catalog.len(), 1);
    let lane0 = lane_catalog[0].as_table().expect("lane table");
    assert_eq!(lane0.get("alias").and_then(TomlValue::as_str), Some("core"));
    let dataspace_catalog = nexus
        .get("dataspace_catalog")
        .and_then(TomlValue::as_array)
        .expect("dataspace catalog array");
    assert_eq!(dataspace_catalog.len(), 1);
    let dataspace = dataspace_catalog[0].as_table().expect("dataspace table");
    assert_eq!(
        dataspace.get("alias").and_then(TomlValue::as_str),
        Some("universal")
    );
    assert!(bundle.config.sumeragi.is_none());
    assert!(bundle.config.torii.is_none());
    assert_eq!(app.log_export_dir.as_deref(), Some(export_dir.as_path()));
    assert_eq!(
        app.state_export_dir.as_deref(),
        Some(state_export_dir.as_path())
    );
    let round_trip = super::config::load_bundle_config()
        .expect("reload persisted config")
        .expect("config should exist on disk");
    assert_eq!(round_trip.path, bundle.path);
    assert_eq!(
        round_trip.config.workspace_root.as_deref(),
        Some(new_root.as_path())
    );
    assert!(round_trip.config.data_root.is_none());
    assert_eq!(round_trip.config.torii_start, Some(15000));
    assert_eq!(round_trip.config.p2p_start, Some(16000));
    assert_eq!(round_trip.config.chain_id.as_deref(), Some("custom-chain"));
    let round_trip_profile = round_trip.config.profile.expect("profile config");
    assert_eq!(round_trip_profile.preset, None);
    assert_eq!(round_trip_profile.topology.peer_count, 7);
    assert_eq!(
        round_trip_profile.consensus_mode,
        SumeragiConsensusMode::Npos
    );
    let round_trip_nexus = round_trip.config.nexus.expect("nexus config");
    assert_eq!(
        round_trip_nexus.get("enabled").and_then(TomlValue::as_bool),
        Some(true)
    );
    assert!(round_trip.config.sumeragi.is_none());
    assert!(round_trip.config.torii.is_none());
    let _ = fs::remove_file(&bundle.path);
    assert!(
        !app.settings_dialog,
        "settings dialog should close after a successful apply"
    );
    let supervisor = app
        .supervisor
        .as_ref()
        .expect("rebuild should leave a supervisor instance");
    assert_eq!(
        MochiApp::supervisor_base_data_root(supervisor),
        sandbox_root_for_workspace(&new_root),
        "rebuilt supervisor should derive sandbox state under the workspace"
    );
    assert_eq!(supervisor.chain_id(), "custom-chain");
    assert_eq!(
        app.settings_data_root_input,
        new_root.display().to_string(),
        "settings inputs should reflect rebuilt supervisor state"
    );
    assert_eq!(
        app.settings_log_export_dir_input,
        export_dir.display().to_string(),
        "log export directory input should reflect applied setting"
    );
    assert_eq!(
        app.settings_state_export_dir_input,
        state_export_dir.display().to_string(),
        "state export directory input should reflect applied setting"
    );
    assert_eq!(
        app.settings_chain_id_input, "custom-chain",
        "chain id input should reflect applied value"
    );
}
#[test]
fn default_app_uses_four_peer_profile() {
    if !super::socket_bind_available() {
        eprintln!("Skipping default app supervisor test due to socket restrictions");
        return;
    }
    let _lock = env_lock().lock().expect("env lock");
    let temp = tempfile::tempdir().expect("tempdir");
    let (kagami_stub, _signature_guard) = install_kagami_stub(temp.path());
    let irohad_stub = install_noop_stub(temp.path(), "irohad_stub.sh");
    let _kagami_guard = TestEnvGuard::set("MOCHI_KAGAMI", &kagami_stub);
    let _irohad_guard = TestEnvGuard::set("MOCHI_IROHAD", &irohad_stub);
    let data_root = temp
        .path()
        .join(format!("mochi-data-{}", std::process::id()));
    let _data_guard = TestEnvGuard::set("MOCHI_DATA_ROOT", &data_root);
    reset_cli_overrides_for_tests();
    let app = MochiApp::default();
    if let Some(err) = app.supervisor_error.as_ref() {
        panic!("default supervisor preparation should succeed: {err}");
    }
    let supervisor = app
        .supervisor
        .as_ref()
        .expect("default supervisor preparation should succeed");
    assert_eq!(
        supervisor.profile().topology.peer_count,
        4,
        "default topology must match the four-peer BFT preset"
    );
    assert_eq!(supervisor.chain_id(), "mochi-local");
    assert!(app.last_error.is_none());
    assert!(!app.theme_applied);
    assert!(matches!(app.active_view, ActiveView::Dashboard));
    assert!(matches!(app.activity_view, ActivityView::Logs));
    assert!(app.auto_block_stream);
    assert!(app.auto_event_stream);
    assert!(app.auto_log_stream);
    assert!(app.block_stream.is_none());
    assert!(app.block_receiver.is_none());
    assert!(app.block_events.is_empty());
    assert!(app.block_stream_peer.is_none());
    assert!(app.block_snapshots.is_empty());
    assert!(app.event_stream.is_none());
    assert!(app.event_receiver.is_none());
    assert!(app.event_events.is_empty());
    assert!(app.event_stream_peer.is_none());
    assert!(app.event_selected_peer.is_none());
    assert!(app.event_snapshots.is_empty());
    assert!(app.log_receiver.is_none());
    assert!(app.log_events.is_empty());
    assert!(app.log_stream_peer.is_none());
    assert!(app.log_snapshots.is_empty());
    assert!(app.log_filter.is_empty());
    assert!(app.status_snapshots.is_empty());
    assert!(app.status_streams.is_empty());
    assert!(
        matches!(app.maintenance_state, MaintenanceState::Idle),
        "maintenance state should start idle"
    );
    assert!(!app.settings_dialog);
    assert!(app.settings_log_stdout);
    assert!(app.settings_log_stderr);
    assert!(app.settings_log_system);
    assert!(app.log_export_dir.is_none());
    assert!(app.settings_log_export_dir_input.is_empty());
    assert!(app.state_export_dir.is_none());
    assert!(app.settings_state_export_dir_input.is_empty());
    assert_eq!(PathBuf::from(&app.settings_data_root_input), data_root);
    assert_eq!(app.settings_torii_port_input, "8080");
    assert_eq!(app.settings_p2p_port_input, "1337");
}
