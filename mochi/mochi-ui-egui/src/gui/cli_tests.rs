use std::{
    env,
    ffi::OsString,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use super::*;

fn cli_env_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

struct CliEnvGuard {
    key: &'static str,
    prev: Option<String>,
}

impl CliEnvGuard {
    fn set(key: &'static str, value: &str) -> Self {
        let prev = env::var(key).ok();
        // SAFETY: Tests serialise environment mutations via `cli_env_lock`.
        unsafe { env::set_var(key, value) };
        Self { key, prev }
    }
}

impl Drop for CliEnvGuard {
    fn drop(&mut self) {
        if let Some(prev) = self.prev.as_ref() {
            // SAFETY: Tests serialise environment mutations via `cli_env_lock`.
            unsafe { env::set_var(self.key, prev) };
        } else {
            // SAFETY: Tests serialise environment mutations via `cli_env_lock`.
            unsafe { env::remove_var(self.key) };
        }
    }
}

#[test]
fn parse_cli_kagami_override_sets_path() {
    let args = vec![OsString::from("--kagami"), OsString::from("/tmp/kagami")];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert!(!parsed.help, "help should not be triggered");
    assert_eq!(
        parsed.overrides.binaries.kagami.as_deref(),
        Some(Path::new("/tmp/kagami"))
    );
}

#[test]
fn parse_cli_config_override_sets_path() {
    let args = vec![
        OsString::from("--config"),
        OsString::from("/tmp/mochi.toml"),
    ];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert_eq!(
        parsed.overrides.config_path.as_deref(),
        Some(Path::new("/tmp/mochi.toml"))
    );
}

#[test]
fn parse_cli_help_flag_short_circuits() {
    let parsed = parse_cli_overrides_from(vec![OsString::from("--help")]).expect("parse help flag");
    assert!(parsed.help, "help flag should be detected");
}

#[test]
fn parse_cli_sandbox_serve_command_and_workspace_root() {
    let parsed = parse_cli_overrides_from(vec![
        OsString::from("sandbox"),
        OsString::from("serve"),
        OsString::from("--workspace-root"),
        OsString::from("/tmp/workspace"),
    ])
    .expect("parse CLI");
    assert_eq!(parsed.command, CliCommand::SandboxServe);
    assert_eq!(
        parsed.overrides.workspace_root.as_deref(),
        Some(Path::new("/tmp/workspace"))
    );
}

#[test]
fn parse_cli_chain_id_override_sets_value() {
    let args = vec![OsString::from("--chain-id"), OsString::from("demo-chain")];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert_eq!(parsed.overrides.chain_id.as_deref(), Some("demo-chain"));
}

#[test]
fn parse_cli_genesis_profile_sets_value() {
    let args = vec![
        OsString::from("--genesis-profile"),
        OsString::from("iroha3-dev"),
    ];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert_eq!(
        parsed.overrides.genesis_profile,
        Some(GenesisProfile::Iroha3Dev)
    );
}

#[test]
fn parse_cli_profile_inline_table_sets_custom_profile() {
    let args = vec![
        OsString::from("--profile"),
        OsString::from("{ peer_count = 3, consensus_mode = \"permissioned\" }"),
    ];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    let profile = parsed.overrides.profile.expect("profile override");
    assert_eq!(profile.preset, None);
    assert_eq!(profile.topology.peer_count, 3);
    assert_eq!(profile.consensus_mode, SumeragiConsensusMode::Permissioned);
}

#[test]
fn parse_cli_profile_inline_table_sets_genesis_profile() {
    let args = vec![
        OsString::from("--profile"),
        OsString::from(
            "{ peer_count = 4, consensus_mode = \"npos\", genesis_profile = \"iroha3-dev\" }",
        ),
    ];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert_eq!(
        parsed.overrides.genesis_profile,
        Some(GenesisProfile::Iroha3Dev)
    );
}

#[test]
fn parse_cli_profile_genesis_conflict_errors() {
    let args = vec![
        OsString::from("--profile"),
        OsString::from(
            "{ peer_count = 4, consensus_mode = \"npos\", genesis_profile = \"iroha3-dev\" }",
        ),
        OsString::from("--genesis-profile"),
        OsString::from("iroha3-testus"),
    ];
    let err = parse_cli_overrides_from(args).expect_err("conflict should error");
    assert!(
        err.to_string().contains("conflicts"),
        "unexpected error message: {err}"
    );
}

#[test]
fn parse_cli_vrf_seed_sets_value() {
    let args = vec![OsString::from("--vrf-seed-hex"), OsString::from("abcd")];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert_eq!(parsed.overrides.vrf_seed_hex.as_deref(), Some("abcd"));
}

#[test]
fn parse_cli_nexus_config_sets_table() {
    let temp = tempfile::tempdir().expect("temp dir");
    let config_path = temp.path().join("nexus.toml");
    fs::write(
        &config_path,
        r#"
enabled = true
lane_count = 2
"#,
    )
    .expect("write nexus config");
    let args = vec![
        OsString::from("--nexus-config"),
        OsString::from(config_path.as_os_str()),
    ];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    let nexus = parsed.overrides.nexus_config.expect("nexus config");
    assert!(matches!(
        nexus.get("enabled"),
        Some(toml::Value::Boolean(true))
    ));
    assert_eq!(
        nexus.get("lane_count").and_then(toml::Value::as_integer),
        Some(2)
    );
}

#[test]
fn parse_cli_nexus_flags_set_overrides() {
    let args = vec![
        OsString::from("--enable-nexus"),
        OsString::from("--nexus-lane-count"),
        OsString::from("3"),
    ];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert_eq!(parsed.overrides.nexus_enabled, Some(true));
    assert_eq!(parsed.overrides.nexus_lane_count, Some(3));
}

#[test]
fn parse_cli_rejects_retired_da_flags() {
    let error = parse_cli_overrides_from(vec![OsString::from("--disable-da")])
        .expect_err("retired DA toggle must be rejected");
    assert!(error.to_string().contains("unknown option"));
}

#[test]
fn parse_cli_restart_mode_never_sets_policy() {
    let args = vec![OsString::from("--restart-mode"), OsString::from("never")];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    assert!(matches!(
        parsed.overrides.restart_policy,
        Some(RestartPolicy::Never)
    ));
}

#[test]
fn parse_cli_restart_on_failure_overrides_attempts() {
    let args = vec![
        OsString::from("--restart-max"),
        OsString::from("5"),
        OsString::from("--restart-backoff-ms"),
        OsString::from("2500"),
    ];
    let parsed = parse_cli_overrides_from(args).expect("parse CLI");
    match parsed.overrides.restart_policy.expect("restart policy") {
        RestartPolicy::OnFailure {
            max_restarts,
            backoff,
        } => {
            assert_eq!(max_restarts, 5);
            assert_eq!(backoff, Duration::from_millis(2500));
        }
        RestartPolicy::Never => panic!("expected on-failure policy"),
    }
}

#[test]
fn parse_cli_build_binaries_flag_enables_auto_build() {
    let parsed =
        parse_cli_overrides_from(vec![OsString::from("--build-binaries")]).expect("parse CLI");
    assert_eq!(parsed.overrides.build_binaries, Some(true));
}

#[test]
fn parse_cli_no_build_binaries_flag_disables_auto_build() {
    let parsed =
        parse_cli_overrides_from(vec![OsString::from("--no-build-binaries")]).expect("parse CLI");
    assert_eq!(parsed.overrides.build_binaries, Some(false));
}

#[test]
fn parse_cli_disable_smoke_flag_disables_readiness_smoke() {
    let parsed =
        parse_cli_overrides_from(vec![OsString::from("--disable-smoke")]).expect("parse CLI");
    assert_eq!(parsed.overrides.readiness_smoke, Some(false));
}

#[test]
fn parse_cli_enable_smoke_flag_enables_readiness_smoke() {
    let parsed =
        parse_cli_overrides_from(vec![OsString::from("--enable-smoke")]).expect("parse CLI");
    assert_eq!(parsed.overrides.readiness_smoke, Some(true));
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
fn parse_cli_readiness_timeout_rejects_zero() {
    let error = parse_cli_overrides_from(vec![
        OsString::from("--readiness-timeout-ms"),
        OsString::from("0"),
    ])
    .expect_err("zero readiness timeout must be rejected");
    assert!(error.to_string().contains("greater than zero"));
}

#[test]
fn parse_cli_unknown_flag_errors() {
    let err = parse_cli_overrides_from(vec![OsString::from("--unknown")])
        .expect_err("unknown flag should error");
    assert!(
        err.to_string().contains("unknown flag"),
        "unexpected error message: {err}"
    );
}

#[test]
fn env_profile_override_applies() {
    let _guard = cli_env_lock().lock().expect("env lock");
    let _profile = CliEnvGuard::set("MOCHI_PROFILE", "four-peer-bft");
    let overrides = parse_env_overrides().expect("parse env overrides");
    assert_eq!(
        overrides.profile,
        Some(NetworkProfile::from_preset(ProfilePreset::FourPeerBft))
    );
}

#[test]
fn env_workspace_root_override_applies() {
    let _guard = cli_env_lock().lock().expect("env lock");
    let _workspace = CliEnvGuard::set("MOCHI_WORKSPACE_ROOT", "/tmp/workspace");
    let overrides = parse_env_overrides().expect("parse env overrides");
    assert_eq!(
        overrides.workspace_root.as_deref(),
        Some(Path::new("/tmp/workspace"))
    );
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

#[test]
fn env_build_binaries_override_applies() {
    let _guard = cli_env_lock().lock().expect("env lock");
    let _build = CliEnvGuard::set("MOCHI_BUILD_BINARIES", "true");
    let overrides = parse_env_overrides().expect("parse env overrides");
    assert_eq!(overrides.build_binaries, Some(true));
}

#[test]
fn env_readiness_smoke_override_applies() {
    let _guard = cli_env_lock().lock().expect("env lock");
    let _smoke = CliEnvGuard::set("MOCHI_READINESS_SMOKE", "false");
    let overrides = parse_env_overrides().expect("parse env overrides");
    assert_eq!(overrides.readiness_smoke, Some(false));
}

#[test]
fn env_readiness_timeout_override_applies() {
    let _guard = cli_env_lock().lock().expect("env lock");
    let _timeout = CliEnvGuard::set("MOCHI_READINESS_TIMEOUT_MS", "45000");
    let overrides = parse_env_overrides().expect("parse env overrides");
    assert_eq!(overrides.readiness_timeout, Some(Duration::from_secs(45)));
}

#[test]
fn cli_readiness_timeout_overrides_environment() {
    let env_overrides = CliOverrides {
        readiness_timeout: Some(Duration::from_secs(45)),
        ..Default::default()
    };
    let cli_overrides = CliOverrides {
        readiness_timeout: Some(Duration::from_secs(300)),
        ..Default::default()
    };
    let merged = merge_overrides(env_overrides, cli_overrides);
    assert_eq!(merged.readiness_timeout, Some(Duration::from_secs(300)));
}

#[test]
fn cli_flags_override_env_values() {
    let _guard = cli_env_lock().lock().expect("env lock");
    let _profile = CliEnvGuard::set("MOCHI_PROFILE", "four-peer-bft");
    let env_overrides = parse_env_overrides().expect("parse env overrides");

    let cli = parse_cli_overrides_from(vec![
        OsString::from("--profile"),
        OsString::from("single-peer"),
    ])
    .expect("parse CLI");

    let merged = merge_overrides(env_overrides, cli.overrides);
    assert_eq!(
        merged.profile,
        Some(NetworkProfile::from_preset(ProfilePreset::SinglePeer))
    );
}

#[cfg(unix)]
fn write_kagami_override_stub(root: &Path) -> (PathBuf, PathBuf) {
    use std::os::unix::fs::PermissionsExt;

    let script_path = root.join("kagami_cli_override.sh");
    let log_path = root.join("kagami_cli_override.log");
    let script = r#"#!/bin/sh
set -e
SCRIPT_DIR="$(CDPATH= cd -- "$(dirname "$0")" && pwd)"
printf '%s\n' "$@" >> "$SCRIPT_DIR/kagami_cli_override.log"
if [ "$1" = "--version" ]; then
  echo "kagami-stub iroha3"
  exit 0
fi
cat <<'JSON'
{"chain":"00000000-0000-0000-0000-000000000000","ivm_dir":".","consensus_mode":"Permissioned","transactions":[{"instructions":[]}]}
JSON
"#;
    std::fs::write(&script_path, script).expect("write kagami CLI override stub");
    let mut perms = std::fs::metadata(&script_path)
        .expect("override stub metadata")
        .permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script_path, perms).expect("set override stub permissions");
    (script_path, log_path)
}

#[cfg(unix)]
#[test]
fn cli_overrides_apply_kagami_path_to_supervisor_builder() {
    if !super::socket_bind_available() {
        eprintln!("Skipping CLI override supervisor test due to socket restrictions");
        return;
    }
    let temp = tempfile::tempdir().expect("temp dir");
    let (script_path, log_path) = write_kagami_override_stub(temp.path());

    let mut overrides = CliOverrides::default();
    overrides.binaries.kagami = Some(script_path.clone());

    let builder = SupervisorBuilder::new(ProfilePreset::SinglePeer).data_root(temp.path());
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
