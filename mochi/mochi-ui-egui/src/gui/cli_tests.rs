//! CLI integration with GUI startup, configuration and the supervisor builder.
use super::cli_options::parse_cli_overrides_from;
use super::*;
use std::{ffi::OsString, path::PathBuf};
#[test]
fn default_test_overrides_use_a_private_temporary_data_root() {
    let (overrides, guard) = isolate_default_test_data_root(CliOverrides::default());
    let guard = guard.expect("default test root must be isolated");
    let data_root = overrides.data_root.expect("isolated data root");
    assert!(data_root.starts_with(guard.path()));
    assert_eq!(overrides.build_binaries, Some(false));

    let explicit = CliOverrides {
        data_root: Some(PathBuf::from("/tmp/explicit-mochi-test-root")),
        ..CliOverrides::default()
    };
    let (explicit, guard) = isolate_default_test_data_root(explicit);
    assert!(guard.is_none());
    assert_eq!(
        explicit.data_root.as_deref(),
        Some(Path::new("/tmp/explicit-mochi-test-root"))
    );
}
#[test]
fn prepare_supervisor_rejects_a_missing_explicit_config_without_fallback() {
    let temp = tempfile::tempdir().expect("temp dir");
    let missing = temp.path().join("missing-mochi.toml");
    let overrides = CliOverrides {
        config_path: Some(missing.clone()),
        ..CliOverrides::default()
    };

    let (supervisor, error, config) = prepare_supervisor_with_overrides(&overrides);

    assert!(supervisor.is_none());
    assert!(config.is_none());
    let error = error.expect("missing explicit config must fail");
    assert!(
        error.to_string().contains("failed to load Mochi config")
            && error.to_string().contains(&missing.display().to_string()),
        "unexpected error: {error}"
    );
}
#[test]
fn parse_cli_readiness_timeout_applies_to_cold_start() {
    let parsed = parse_cli_overrides_from(vec![
        OsString::from("--readiness-timeout-ms"),
        OsString::from("300000"),
    ])
    .expect("parse CLI");
    assert_eq!(
        parsed.overrides.readiness_timeout,
        Some(Duration::from_secs(300))
    );
    let options = configured_readiness_options_for(&parsed.overrides);
    assert_eq!(options.timeout, Duration::from_secs(300));
    assert_eq!(options.poll_interval, READINESS_POLL_INTERVAL);
}
#[test]
fn sandbox_readiness_timeout_defaults_to_cold_start_budget() {
    let options = configured_readiness_options_for(&CliOverrides::default());
    assert_eq!(options.timeout, SANDBOX_READINESS_TIMEOUT);
    assert_eq!(options.poll_interval, READINESS_POLL_INTERVAL);
}
#[test]
fn should_default_workspace_root_when_no_paths_are_configured() {
    assert!(should_default_workspace_root(
        &CliOverrides::default(),
        None
    ));
}
#[test]
fn should_not_default_workspace_root_when_data_root_is_configured() {
    let config = ResolvedBundleConfig {
        config: BundleConfig {
            data_root: Some(PathBuf::from("/tmp/mochi")),
            ..Default::default()
        },
        path: PathBuf::from("/tmp/mochi.toml"),
    };
    assert!(!should_default_workspace_root(
        &CliOverrides::default(),
        Some(&config),
    ));
}
#[test]
fn resolved_build_binaries_defaults_to_true() {
    assert!(resolved_build_binaries(&CliOverrides::default(), None));
}
#[test]
fn resolved_build_binaries_honors_explicit_config_disable() {
    let config = ResolvedBundleConfig {
        config: BundleConfig {
            build_binaries: Some(false),
            ..Default::default()
        },
        path: PathBuf::from("/tmp/mochi.toml"),
    };
    assert!(!resolved_build_binaries(
        &CliOverrides::default(),
        Some(&config),
    ));
}
#[cfg(unix)]
#[test]
fn cli_overrides_apply_kagami_path_to_supervisor_builder() {
    if !super::socket_bind_available() {
        eprintln!("Skipping CLI override supervisor test due to socket restrictions");
        return;
    }
    let _lock = test_support::env_lock().lock().expect("test env lock");
    let temp = tempfile::tempdir().expect("temp dir");
    let log_path = temp.path().join("kagami_cli_override.log");
    let (script_path, _signature_guard) = test_support::install_kagami_stub(temp.path());
    let _log_guard = test_support::TestEnvGuard::set("MOCHI_TEST_KAGAMI_LOG", &log_path);
    let mut overrides = CliOverrides::default();
    overrides.binaries.kagami = Some(script_path.clone());
    let builder = SupervisorBuilder::new(ProfilePreset::FourPeerBft).data_root(temp.path());
    overrides
        .apply_to(builder)
        .build()
        .expect("build supervisor with CLI overrides");
    let log = std::fs::read_to_string(&log_path).expect("read CLI override kagami log");
    assert!(
        log.contains("--genesis-public-key"),
        "expected CLI override stub to capture genesis args, got `{log}`"
    );
}
