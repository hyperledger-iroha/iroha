#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests of the Iroha Client CLI

use std::{
    num::NonZeroU32,
    path::{Path, PathBuf},
    process::Command as ProcessCommand,
    sync::Once,
    time::Duration,
};

use integration_tests::sandbox;
use iroha::{
    client::Client,
    config::{DEFAULT_TRANSACTION_STATUS_TIMEOUT, DEFAULT_TRANSACTION_TIME_TO_LIVE},
    crypto::{ExposedPrivateKey, Hash, KeyPair},
    data_model::{
        Encode,
        soracloud::{
            AgentApartmentManifestV1, SoraContainerManifestV1, SoraServiceManifestV1,
            SoraStateMutabilityV1,
        },
    },
};
use iroha_config_base::toml::WriteExt;
use iroha_data_model::prelude::DomainId;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::sample_ivm_path;
use norito::json::{self, Value};
use reqwest::Url;

fn program() -> PathBuf {
    enable_reentrant_builds_for_tests();
    configure_cli_program_override_from_existing_binary();
    iroha_test_network::Program::Iroha.resolve().unwrap()
}

fn enable_reentrant_builds_for_tests() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Cargo sets `CARGO` for test binaries, which disables reentrant builds by default.
        // Allow nested builds so the CLI binary can be compiled on-demand in fresh workspaces.
        set_env_var("IROHA_TEST_ALLOW_REENTRANT_BUILD", "1");
        if std::env::var_os("IROHA_TEST_BUILD_PROFILE").is_none() {
            set_env_var("IROHA_TEST_BUILD_PROFILE", "debug");
        }
    });
}

fn configure_cli_program_override_from_existing_binary() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        const TEST_NETWORK_BIN_IROHA: &str = "TEST_NETWORK_BIN_IROHA";
        if std::env::var_os(TEST_NETWORK_BIN_IROHA).is_some() {
            return;
        }
        if let Some(path) = find_existing_cli_binary_path() {
            let value = path.to_string_lossy().into_owned();
            set_env_var(TEST_NETWORK_BIN_IROHA, &value);
        }
    });
}

fn find_existing_cli_binary_path() -> Option<PathBuf> {
    const IROHA_TEST_TARGET_SUBDIR: &str = "iroha-test-network";
    let mut target_roots = Vec::new();
    if let Some(target_dir) = std::env::var_os("CARGO_TARGET_DIR") {
        let target_dir = PathBuf::from(target_dir);
        target_roots.push(target_dir.join(IROHA_TEST_TARGET_SUBDIR));
        target_roots.push(target_dir);
    }
    let workspace_target = workspace_root().join("target");
    target_roots.push(workspace_target.join(IROHA_TEST_TARGET_SUBDIR));
    target_roots.push(workspace_target);

    let mut profiles = Vec::new();
    if let Ok(profile) = std::env::var("PROFILE")
        && !profile.trim().is_empty()
    {
        profiles.push(profile);
    }
    if !profiles.iter().any(|value| value == "debug") {
        profiles.push("debug".to_owned());
    }
    if !profiles.iter().any(|value| value == "release") {
        profiles.push("release".to_owned());
    }

    find_existing_cli_binary_path_from_roots(&target_roots, &profiles)
        .filter(|path| binary_supports_training_job_commands(path.as_path()))
}

fn find_existing_cli_binary_path_from_roots(
    target_roots: &[PathBuf],
    profiles: &[String],
) -> Option<PathBuf> {
    let binary_name = if cfg!(windows) { "iroha.exe" } else { "iroha" };
    for target_root in target_roots {
        for profile in profiles {
            let candidate = target_root.join(profile).join(binary_name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

fn binary_supports_training_job_commands(path: &std::path::Path) -> bool {
    let output = ProcessCommand::new(path)
        .arg("app")
        .arg("soracloud")
        .arg("--help")
        .output();
    let Ok(output) = output else {
        return false;
    };
    if !output.status.success() {
        return false;
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout.contains("training-job-start")
}

#[allow(unsafe_code)]
fn set_env_var(key: &str, value: &str) {
    unsafe {
        std::env::set_var(key, value);
    }
}

fn ivm_build_profile_exists() -> bool {
    // Mirror the check used in other integration tests
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../crates/ivm/target/prebuilt/build_config.toml")
        .exists()
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..")
}

fn soracloud_fixture(path: &str) -> PathBuf {
    workspace_root().join(path)
}

#[test]
fn find_existing_cli_binary_path_from_roots_returns_first_match() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root_a = temp.path().join("target-a");
    let root_b = temp.path().join("target-b");
    let binary_name = if cfg!(windows) { "iroha.exe" } else { "iroha" };
    let expected = root_b.join("debug").join(binary_name);
    std::fs::create_dir_all(expected.parent().expect("parent dir")).expect("create dirs");
    std::fs::write(&expected, b"binary").expect("create fake binary");

    let profiles = vec!["debug".to_owned(), "release".to_owned()];
    let found = find_existing_cli_binary_path_from_roots(&[root_a, root_b], &profiles);
    assert_eq!(found, Some(expected));
}

#[test]
fn find_existing_cli_binary_path_from_roots_returns_none_when_missing() {
    let temp = tempfile::tempdir().expect("tempdir");
    let root = temp.path().join("target");
    std::fs::create_dir_all(&root).expect("create dirs");

    let profiles = vec!["debug".to_owned(), "release".to_owned()];
    let found = find_existing_cli_binary_path_from_roots(&[root], &profiles);
    assert!(found.is_none());
}

#[test]
fn binary_supports_training_job_commands_rejects_missing_binary() {
    let missing = PathBuf::from("/definitely/missing/iroha");
    assert!(!binary_supports_training_job_commands(&missing));
}

#[cfg(unix)]
#[test]
fn binary_supports_training_job_commands_rejects_help_without_subcommand() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let script = temp.path().join("fake_iroha.sh");
    std::fs::write(&script, "#!/bin/sh\necho 'app soracloud help output'\n").expect("write script");
    let mut perms = std::fs::metadata(&script).expect("metadata").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("set permissions");
    assert!(!binary_supports_training_job_commands(&script));
}

#[cfg(unix)]
#[test]
fn binary_supports_training_job_commands_accepts_help_with_subcommand() {
    use std::os::unix::fs::PermissionsExt;

    let temp = tempfile::tempdir().expect("tempdir");
    let script = temp.path().join("fake_iroha.sh");
    std::fs::write(
        &script,
        "#!/bin/sh\necho 'Commands:\\n  training-job-start\\n  model-weight-register'\n",
    )
    .expect("write script");
    let mut perms = std::fs::metadata(&script).expect("metadata").permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(&script, perms).expect("set permissions");
    assert!(binary_supports_training_job_commands(&script));
}

fn local_program_config() -> ProgramConfig {
    let key = KeyPair::random();
    let domain_id: DomainId = "wonderland".parse().expect("literal domain should parse");
    ProgramConfig {
        torii_url: Url::parse("http://127.0.0.1:8080").expect("literal URL should parse"),
        account_domain: domain_id,
        key,
        status_timeout: DEFAULT_TRANSACTION_STATUS_TIMEOUT,
        ttl: DEFAULT_TRANSACTION_TIME_TO_LIVE,
    }
}

struct ProgramConfig {
    torii_url: Url,
    account_domain: DomainId,
    key: KeyPair,
    status_timeout: Duration,
    ttl: Duration,
}

impl From<&Client> for ProgramConfig {
    fn from(value: &Client) -> Self {
        let torii_url = value.torii_url.clone();
        let account_domain: DomainId = "wonderland"
            .parse()
            .expect("wonderland domain should parse");
        let key = value.key_pair.clone();
        let status_timeout = value.transaction_status_timeout;
        let ttl = value
            .transaction_ttl
            .unwrap_or(DEFAULT_TRANSACTION_TIME_TO_LIVE);
        Self {
            torii_url,
            account_domain,
            key,
            status_timeout,
            ttl,
        }
    }
}

impl ProgramConfig {
    fn envs(&self) -> impl IntoIterator<Item = (&str, String)> {
        [
            ("CHAIN", iroha_test_network::chain_id().to_string()),
            ("TORII_URL", self.torii_url.to_string()),
            ("ACCOUNT_DOMAIN", self.account_domain.to_string()),
            ("ACCOUNT_PUBLIC_KEY", self.key.public_key().to_string()),
            (
                "ACCOUNT_PRIVATE_KEY",
                ExposedPrivateKey(self.key.private_key().clone()).to_string(),
            ),
            (
                "TRANSACTION_STATUS_TIMEOUT_MS",
                self.status_timeout.as_millis().to_string(),
            ),
            (
                "TRANSACTION_TIME_TO_LIVE_MS",
                self.ttl.as_millis().to_string(),
            ),
        ]
    }

    fn toml(&self) -> toml::Table {
        toml::Table::new()
            .write("chain", iroha_test_network::chain_id().to_string())
            .write("torii_url", self.torii_url.to_string())
            .write(["account", "domain"], self.account_domain.to_string())
            .write(
                ["account", "private_key"],
                ExposedPrivateKey(self.key.private_key().clone()).to_string(),
            )
            .write(["account", "public_key"], self.key.public_key().to_string())
            .write(
                ["transaction", "status_timeout_ms"],
                i64::try_from(self.status_timeout.as_millis()).unwrap_or_else(|_| {
                    i64::try_from(DEFAULT_TRANSACTION_STATUS_TIMEOUT.as_millis())
                        .unwrap_or(i64::MAX)
                }),
            )
            .write(
                ["transaction", "time_to_live_ms"],
                i64::try_from(self.ttl.as_millis()).unwrap_or_else(|_| {
                    i64::try_from(DEFAULT_TRANSACTION_TIME_TO_LIVE.as_millis()).unwrap_or(i64::MAX)
                }),
            )
    }
}

async fn run_soracloud_command(
    cwd: &Path,
    config: &ProgramConfig,
    args: &[&str],
) -> eyre::Result<std::process::Output> {
    Ok(tokio::process::Command::new(program())
        .current_dir(cwd)
        .arg("app")
        .arg("soracloud")
        .args(args)
        .envs(config.envs())
        .output()
        .await?)
}

fn assert_requires_torii_url(output: &std::process::Output) {
    assert!(
        !output.status.success(),
        "command unexpectedly succeeded with stdout: {}",
        String::from_utf8_lossy(&output.stdout)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("--torii-url is required for Soracloud live control-plane access"),
        "unexpected stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn can_upgrade_executor() -> eyre::Result<()> {
    if !ivm_build_profile_exists() {
        eprintln!("Skipping test: missing IVM build profile");
        return Ok(());
    }
    // Guard against placeholder prebuilt samples (tiny `.to` blobs). Real
    // executor bytecode should be larger than a minimal stub. If we detect a
    // stub, skip this test to avoid a false failure.
    let sample_path = sample_ivm_path("executor_with_admin");
    if let Ok(meta) = tokio::fs::metadata(&sample_path).await
        && meta.len() < 64
    {
        eprintln!(
            "Skipping test: prebuilt IVM sample appears to be a placeholder: {} ({} bytes)",
            sample_path.display(),
            meta.len()
        );
        return Ok(());
    }
    // Assuming Alice already has the CanUpgradeExecutor permission
    let builder = NetworkBuilder::new()
        .with_ivm_fuel(iroha_test_network::IvmFuelConfig::Auto)
        .with_min_peers(4);
    let Some(network) =
        sandbox::start_network_async_or_skip(builder, stringify!(can_upgrade_executor)).await?
    else {
        return Ok(());
    };

    // Use an explicit client.toml to avoid any env resolution pitfalls
    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut child = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("ops")
        .arg("executor")
        .arg("upgrade")
        .arg("--path")
        .arg(sample_path)
        // Also pass envs to exercise the CLI's env handling and avoid dead-code.
        .envs(config.envs())
        .spawn()?;
    let exit_status = child.wait().await?;

    assert!(exit_status.success());

    Ok(())
}

#[tokio::test]
async fn reads_client_toml_by_default() -> eyre::Result<()> {
    let Some(network) = sandbox::start_network_async_or_skip(
        NetworkBuilder::new(),
        stringify!(reads_client_toml_by_default),
    )
    .await?
    else {
        return Ok(());
    };
    let config = ProgramConfig::from(&network.client());

    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut child = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("ledger")
        .arg("domain")
        .arg("list")
        .arg("all")
        .spawn()?;
    let exit_status = child.wait().await?;

    assert!(exit_status.success());

    Ok(())
}

// Add more CLI tests here!

#[tokio::test]
async fn soracloud_status_uses_live_torii_control_plane() -> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(soracloud_status_uses_live_torii_control_plane),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let output = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("status")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        output.status.success(),
        "CLI exited with status {} and stderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );

    let payload: Value =
        json::from_slice(&output.stdout).expect("CLI should emit JSON soracloud status payload");
    assert_eq!(
        payload.get("source").and_then(Value::as_str),
        Some("torii_control_plane")
    );
    let network_status = payload
        .get("network_status")
        .and_then(Value::as_object)
        .expect("network_status object present");
    assert_eq!(
        network_status.get("schema_version").and_then(Value::as_u64),
        Some(1)
    );
    assert!(network_status.contains_key("service_health"));
    assert!(network_status.contains_key("routing"));
    assert!(network_status.contains_key("resource_pressure"));
    assert!(network_status.contains_key("failed_admissions"));

    Ok(())
}

#[tokio::test]
async fn soracloud_mutations_use_live_torii_control_plane() -> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(soracloud_mutations_use_live_torii_control_plane),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let container_fixture = soracloud_fixture("fixtures/soracloud/sora_container_manifest_v1.json");
    let service_fixture = soracloud_fixture("fixtures/soracloud/sora_service_manifest_v1.json");
    let container: SoraContainerManifestV1 =
        norito::json::from_slice(&std::fs::read(&container_fixture)?)?;
    let mut service_v1: SoraServiceManifestV1 =
        norito::json::from_slice(&std::fs::read(&service_fixture)?)?;
    service_v1.service_version = "1.0.0".to_string();
    service_v1.container.manifest_hash = Hash::new(Encode::encode(&container));
    let mut service_v2 = service_v1.clone();
    service_v2.service_version = "1.1.0".to_string();

    let container_path = dir.path().join("container_manifest.json");
    let service_v1_path = dir.path().join("service_v1.json");
    let service_v2_path = dir.path().join("service_v2.json");
    tokio::fs::write(
        &container_path,
        norito::json::to_vec_pretty(&container).expect("encode container"),
    )
    .await?;
    tokio::fs::write(
        &service_v1_path,
        norito::json::to_vec_pretty(&service_v1).expect("encode service v1"),
    )
    .await?;
    tokio::fs::write(
        &service_v2_path,
        norito::json::to_vec_pretty(&service_v2).expect("encode service v2"),
    )
    .await?;

    let deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("deploy")
        .arg("--container")
        .arg(container_path.to_string_lossy().into_owned())
        .arg("--service")
        .arg(service_v1_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        deploy.status.success(),
        "deploy failed with status {} and stderr: {}",
        deploy.status,
        String::from_utf8_lossy(&deploy.stderr)
    );

    let deploy_payload: Value =
        json::from_slice(&deploy.stdout).expect("CLI should emit deploy JSON payload");
    assert_eq!(
        deploy_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );

    let upgrade = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("upgrade")
        .arg("--container")
        .arg(container_path.to_string_lossy().into_owned())
        .arg("--service")
        .arg(service_v2_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        upgrade.status.success(),
        "upgrade failed with status {} and stderr: {}",
        upgrade.status,
        String::from_utf8_lossy(&upgrade.stderr)
    );

    let upgrade_payload: Value =
        json::from_slice(&upgrade.stdout).expect("CLI should emit upgrade JSON payload");
    assert_eq!(
        upgrade_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.1.0")
    );
    let rollout_handle = upgrade_payload
        .get("rollout_handle")
        .and_then(Value::as_str)
        .expect("upgrade should return rollout_handle");
    assert_eq!(
        upgrade_payload
            .get("rollout_stage")
            .and_then(Value::as_object)
            .and_then(|stage| stage.get("stage"))
            .and_then(Value::as_str),
        Some("Canary")
    );

    let rollout = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("rollout")
        .arg("--service-name")
        .arg(service_v1.service_name.to_string())
        .arg("--rollout-handle")
        .arg(rollout_handle)
        .arg("--promote-to-percent")
        .arg("100")
        .arg("--governance-tx-hash")
        .arg(Hash::new(b"cli-live-rollout-promote").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        rollout.status.success(),
        "rollout failed with status {} and stderr: {}",
        rollout.status,
        String::from_utf8_lossy(&rollout.stderr)
    );

    let rollout_payload: Value =
        json::from_slice(&rollout.stdout).expect("CLI should emit rollout JSON payload");
    assert_eq!(
        rollout_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.1.0")
    );
    assert_eq!(
        rollout_payload
            .get("stage")
            .and_then(Value::as_object)
            .and_then(|stage| stage.get("stage"))
            .and_then(Value::as_str),
        Some("Promoted")
    );
    assert_eq!(
        rollout_payload
            .get("traffic_percent")
            .and_then(Value::as_u64),
        Some(100)
    );

    let rollback = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("rollback")
        .arg("--service-name")
        .arg(service_v1.service_name.to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        rollback.status.success(),
        "rollback failed with status {} and stderr: {}",
        rollback.status,
        String::from_utf8_lossy(&rollback.stderr)
    );

    let rollback_payload: Value =
        json::from_slice(&rollback.stdout).expect("CLI should emit rollback JSON payload");
    assert_eq!(
        rollback_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );

    let status = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("status")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        status.status.success(),
        "status failed with status {} and stderr: {}",
        status.status,
        String::from_utf8_lossy(&status.stderr)
    );

    let status_payload: Value =
        json::from_slice(&status.stdout).expect("CLI should emit soracloud status payload");
    let network_status = status_payload
        .get("network_status")
        .and_then(Value::as_object)
        .expect("network_status object");
    let control_plane = network_status
        .get("control_plane")
        .and_then(Value::as_object)
        .expect("control_plane object");
    assert_eq!(
        control_plane.get("service_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        control_plane
            .get("audit_event_count")
            .and_then(Value::as_u64),
        Some(4)
    );
    let services = control_plane
        .get("services")
        .and_then(Value::as_array)
        .expect("services array");
    assert_eq!(services.len(), 1);
    assert_eq!(
        services[0].get("current_version").and_then(Value::as_str),
        Some("1.0.0")
    );

    Ok(())
}

#[tokio::test]
async fn soracloud_scr_host_admission_rejects_invalid_manifests_live_torii_control_plane()
-> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(soracloud_scr_host_admission_rejects_invalid_manifests_live_torii_control_plane),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let container_fixture = soracloud_fixture("fixtures/soracloud/sora_container_manifest_v1.json");
    let service_fixture = soracloud_fixture("fixtures/soracloud/sora_service_manifest_v1.json");
    let container: SoraContainerManifestV1 =
        norito::json::from_slice(&std::fs::read(&container_fixture)?)?;
    let mut service: SoraServiceManifestV1 =
        norito::json::from_slice(&std::fs::read(&service_fixture)?)?;
    service.service_version = "1.0.0".to_string();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));

    let mut over_cap_container = container.clone();
    over_cap_container.resources.cpu_millis = NonZeroU32::new(64_001).expect("non-zero cpu");
    let mut over_cap_service = service.clone();
    over_cap_service.container.manifest_hash = Hash::new(Encode::encode(&over_cap_container));
    let over_cap_container_path = dir.path().join("container_over_cap.json");
    let over_cap_service_path = dir.path().join("service_over_cap.json");
    tokio::fs::write(
        &over_cap_container_path,
        norito::json::to_vec_pretty(&over_cap_container).expect("encode over-cap container"),
    )
    .await?;
    tokio::fs::write(
        &over_cap_service_path,
        norito::json::to_vec_pretty(&over_cap_service).expect("encode over-cap service"),
    )
    .await?;

    let over_cap_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("deploy")
        .arg("--container")
        .arg(over_cap_container_path.to_string_lossy().into_owned())
        .arg("--service")
        .arg(over_cap_service_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        !over_cap_deploy.status.success(),
        "deploy with over-cap cpu should fail, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&over_cap_deploy.stdout),
        String::from_utf8_lossy(&over_cap_deploy.stderr)
    );
    assert!(
        String::from_utf8_lossy(&over_cap_deploy.stderr).contains("returned 400"),
        "{}",
        String::from_utf8_lossy(&over_cap_deploy.stderr)
    );
    assert!(
        String::from_utf8_lossy(&over_cap_deploy.stderr)
            .contains("resources.cpu_millis exceeds SCR cap"),
        "{}",
        String::from_utf8_lossy(&over_cap_deploy.stderr)
    );

    let mut no_write_container = container.clone();
    no_write_container.capabilities.allow_state_writes = false;
    assert!(
        service
            .state_bindings
            .iter()
            .any(|binding| binding.mutability != SoraStateMutabilityV1::ReadOnly),
        "fixture should include at least one non-readonly state binding"
    );
    let no_write_container_hash = Hash::new(Encode::encode(&no_write_container));
    let no_write_container_path = dir.path().join("container_no_write.json");
    let no_write_service_path = dir.path().join("service_no_write.json");
    tokio::fs::write(
        &no_write_container_path,
        norito::json::to_vec_pretty(&no_write_container).expect("encode no-write container"),
    )
    .await?;
    let mut no_write_service = service.clone();
    no_write_service.container.manifest_hash = no_write_container_hash;
    tokio::fs::write(
        &no_write_service_path,
        norito::json::to_vec_pretty(&no_write_service).expect("encode no-write service"),
    )
    .await?;

    let no_write_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("deploy")
        .arg("--container")
        .arg(no_write_container_path.to_string_lossy().into_owned())
        .arg("--service")
        .arg(no_write_service_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        !no_write_deploy.status.success(),
        "deploy with allow_state_writes=false should fail, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&no_write_deploy.stdout),
        String::from_utf8_lossy(&no_write_deploy.stderr)
    );
    let no_write_stderr = String::from_utf8_lossy(&no_write_deploy.stderr);
    assert!(
        no_write_stderr.contains("returned 400")
            || no_write_stderr.contains("Failed to run the command"),
        "{}",
        no_write_stderr
    );
    assert!(
        no_write_stderr.contains("bundle field")
            && no_write_stderr.contains("allow_state_writes")
            && no_write_stderr.contains("is invalid"),
        "{}",
        no_write_stderr
    );
    assert!(
        no_write_stderr.contains("binding `session_store` requires mutable writes (`ReadWrite`)"),
        "{}",
        no_write_stderr
    );

    Ok(())
}

#[tokio::test]
async fn soracloud_training_and_model_weight_lifecycle_use_live_torii_control_plane()
-> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(soracloud_training_and_model_weight_lifecycle_use_live_torii_control_plane),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let container_fixture = soracloud_fixture("fixtures/soracloud/sora_container_manifest_v1.json");
    let service_fixture = soracloud_fixture("fixtures/soracloud/sora_service_manifest_v1.json");
    let mut container: SoraContainerManifestV1 =
        norito::json::from_slice(&std::fs::read(&container_fixture)?)?;
    let mut service: SoraServiceManifestV1 =
        norito::json::from_slice(&std::fs::read(&service_fixture)?)?;
    container.capabilities.allow_model_training = true;
    service.service_version = "2.0.0".to_string();
    service.container.manifest_hash = Hash::new(Encode::encode(&container));

    let container_path = dir.path().join("container_training.json");
    let service_path = dir.path().join("service_training.json");
    tokio::fs::write(
        &container_path,
        norito::json::to_vec_pretty(&container).expect("encode container"),
    )
    .await?;
    tokio::fs::write(
        &service_path,
        norito::json::to_vec_pretty(&service).expect("encode service"),
    )
    .await?;

    let deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("deploy")
        .arg("--container")
        .arg(container_path.to_string_lossy().into_owned())
        .arg("--service")
        .arg(service_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        deploy.status.success(),
        "deploy for training lifecycle failed with status {} and stderr: {}",
        deploy.status,
        String::from_utf8_lossy(&deploy.stderr)
    );

    let service_name = service.service_name.to_string();
    let model_name = "ops_model";
    let dataset_ref = "dataset://ops/synthetic-v1";

    let training_start_1 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("training-job-start")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--job-id")
        .arg("job-001")
        .arg("--worker-group-size")
        .arg("2")
        .arg("--target-steps")
        .arg("4")
        .arg("--checkpoint-interval-steps")
        .arg("2")
        .arg("--max-retries")
        .arg("2")
        .arg("--step-compute-units")
        .arg("25")
        .arg("--compute-budget-units")
        .arg("200")
        .arg("--storage-budget-bytes")
        .arg("8192")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        training_start_1.status.success(),
        "training-job-start #1 failed with status {} and stderr: {}",
        training_start_1.status,
        String::from_utf8_lossy(&training_start_1.stderr)
    );

    let checkpoint_1a = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("training-job-checkpoint")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--job-id")
        .arg("job-001")
        .arg("--completed-step")
        .arg("2")
        .arg("--checkpoint-size-bytes")
        .arg("1024")
        .arg("--metrics-hash")
        .arg(Hash::new(b"metrics-job-001-step-2").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        checkpoint_1a.status.success(),
        "training-job-checkpoint #1a failed with status {} and stderr: {}",
        checkpoint_1a.status,
        String::from_utf8_lossy(&checkpoint_1a.stderr)
    );

    let checkpoint_1b = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("training-job-checkpoint")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--job-id")
        .arg("job-001")
        .arg("--completed-step")
        .arg("4")
        .arg("--checkpoint-size-bytes")
        .arg("1536")
        .arg("--metrics-hash")
        .arg(Hash::new(b"metrics-job-001-step-4").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        checkpoint_1b.status.success(),
        "training-job-checkpoint #1b failed with status {} and stderr: {}",
        checkpoint_1b.status,
        String::from_utf8_lossy(&checkpoint_1b.stderr)
    );

    let training_status_1 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("training-job-status")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--job-id")
        .arg("job-001")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        training_status_1.status.success(),
        "training-job-status #1 failed with status {} and stderr: {}",
        training_status_1.status,
        String::from_utf8_lossy(&training_status_1.stderr)
    );
    let training_status_payload_1: Value =
        json::from_slice(&training_status_1.stdout).expect("training-job-status #1 json payload");
    let job_1 = training_status_payload_1
        .get("job")
        .and_then(Value::as_object)
        .expect("training status job object");
    assert_eq!(job_1.get("job_id").and_then(Value::as_str), Some("job-001"));
    assert_eq!(
        job_1.get("completed_steps").and_then(Value::as_u64),
        Some(4)
    );

    let artifact_register_1 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-artifact-register")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--training-job-id")
        .arg("job-001")
        .arg("--weight-artifact-hash")
        .arg(Hash::new(b"weight-artifact-v1").to_string())
        .arg("--dataset-ref")
        .arg(dataset_ref)
        .arg("--training-config-hash")
        .arg(Hash::new(b"training-config-v1").to_string())
        .arg("--reproducibility-hash")
        .arg(Hash::new(b"repro-v1").to_string())
        .arg("--provenance-attestation-hash")
        .arg(Hash::new(b"attestation-v1").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        artifact_register_1.status.success(),
        "model-artifact-register #1 failed with status {} and stderr: {}",
        artifact_register_1.status,
        String::from_utf8_lossy(&artifact_register_1.stderr)
    );

    let weight_register_1 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-weight-register")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--weight-version")
        .arg("1.0.0")
        .arg("--training-job-id")
        .arg("job-001")
        .arg("--weight-artifact-hash")
        .arg(Hash::new(b"weight-artifact-v1").to_string())
        .arg("--dataset-ref")
        .arg(dataset_ref)
        .arg("--training-config-hash")
        .arg(Hash::new(b"training-config-v1").to_string())
        .arg("--reproducibility-hash")
        .arg(Hash::new(b"repro-v1").to_string())
        .arg("--provenance-attestation-hash")
        .arg(Hash::new(b"attestation-v1").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        weight_register_1.status.success(),
        "model-weight-register #1 failed with status {} and stderr: {}",
        weight_register_1.status,
        String::from_utf8_lossy(&weight_register_1.stderr)
    );

    let training_start_2 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("training-job-start")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--job-id")
        .arg("job-002")
        .arg("--worker-group-size")
        .arg("2")
        .arg("--target-steps")
        .arg("4")
        .arg("--checkpoint-interval-steps")
        .arg("2")
        .arg("--max-retries")
        .arg("2")
        .arg("--step-compute-units")
        .arg("25")
        .arg("--compute-budget-units")
        .arg("220")
        .arg("--storage-budget-bytes")
        .arg("8192")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        training_start_2.status.success(),
        "training-job-start #2 failed with status {} and stderr: {}",
        training_start_2.status,
        String::from_utf8_lossy(&training_start_2.stderr)
    );

    let checkpoint_2a = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("training-job-checkpoint")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--job-id")
        .arg("job-002")
        .arg("--completed-step")
        .arg("2")
        .arg("--checkpoint-size-bytes")
        .arg("1024")
        .arg("--metrics-hash")
        .arg(Hash::new(b"metrics-job-002-step-2").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        checkpoint_2a.status.success(),
        "training-job-checkpoint #2a failed with status {} and stderr: {}",
        checkpoint_2a.status,
        String::from_utf8_lossy(&checkpoint_2a.stderr)
    );

    let checkpoint_2b = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("training-job-checkpoint")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--job-id")
        .arg("job-002")
        .arg("--completed-step")
        .arg("4")
        .arg("--checkpoint-size-bytes")
        .arg("1536")
        .arg("--metrics-hash")
        .arg(Hash::new(b"metrics-job-002-step-4").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        checkpoint_2b.status.success(),
        "training-job-checkpoint #2b failed with status {} and stderr: {}",
        checkpoint_2b.status,
        String::from_utf8_lossy(&checkpoint_2b.stderr)
    );

    let artifact_register_2 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-artifact-register")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--training-job-id")
        .arg("job-002")
        .arg("--weight-artifact-hash")
        .arg(Hash::new(b"weight-artifact-v2").to_string())
        .arg("--dataset-ref")
        .arg(dataset_ref)
        .arg("--training-config-hash")
        .arg(Hash::new(b"training-config-v2").to_string())
        .arg("--reproducibility-hash")
        .arg(Hash::new(b"repro-v2").to_string())
        .arg("--provenance-attestation-hash")
        .arg(Hash::new(b"attestation-v2").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        artifact_register_2.status.success(),
        "model-artifact-register #2 failed with status {} and stderr: {}",
        artifact_register_2.status,
        String::from_utf8_lossy(&artifact_register_2.stderr)
    );

    let weight_register_2 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-weight-register")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--weight-version")
        .arg("1.1.0")
        .arg("--training-job-id")
        .arg("job-002")
        .arg("--parent-version")
        .arg("1.0.0")
        .arg("--weight-artifact-hash")
        .arg(Hash::new(b"weight-artifact-v2").to_string())
        .arg("--dataset-ref")
        .arg(dataset_ref)
        .arg("--training-config-hash")
        .arg(Hash::new(b"training-config-v2").to_string())
        .arg("--reproducibility-hash")
        .arg(Hash::new(b"repro-v2").to_string())
        .arg("--provenance-attestation-hash")
        .arg(Hash::new(b"attestation-v2").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        weight_register_2.status.success(),
        "model-weight-register #2 failed with status {} and stderr: {}",
        weight_register_2.status,
        String::from_utf8_lossy(&weight_register_2.stderr)
    );

    let promote_v2 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-weight-promote")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--weight-version")
        .arg("1.1.0")
        .arg("--gate-approved")
        .arg("--gate-report-hash")
        .arg(Hash::new(b"gate-report-v2").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        promote_v2.status.success(),
        "model-weight-promote failed with status {} and stderr: {}",
        promote_v2.status,
        String::from_utf8_lossy(&promote_v2.stderr)
    );

    let status_after_promote = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-weight-status")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        status_after_promote.status.success(),
        "model-weight-status after promote failed with status {} and stderr: {}",
        status_after_promote.status,
        String::from_utf8_lossy(&status_after_promote.stderr)
    );
    let status_after_promote_payload: Value =
        json::from_slice(&status_after_promote.stdout).expect("model-weight-status promote json");
    let model_after_promote = status_after_promote_payload
        .get("model")
        .and_then(Value::as_object)
        .expect("model object after promote");
    assert_eq!(
        model_after_promote
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.1.0")
    );
    assert_eq!(
        model_after_promote
            .get("version_count")
            .and_then(Value::as_u64),
        Some(2)
    );

    let rollback = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-weight-rollback")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--target-version")
        .arg("1.0.0")
        .arg("--reason")
        .arg("roll back to baseline")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        rollback.status.success(),
        "model-weight-rollback failed with status {} and stderr: {}",
        rollback.status,
        String::from_utf8_lossy(&rollback.stderr)
    );

    let status_after_rollback = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-weight-status")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--model-name")
        .arg(model_name)
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        status_after_rollback.status.success(),
        "model-weight-status after rollback failed with status {} and stderr: {}",
        status_after_rollback.status,
        String::from_utf8_lossy(&status_after_rollback.stderr)
    );
    let status_after_rollback_payload: Value =
        json::from_slice(&status_after_rollback.stdout).expect("model-weight-status rollback json");
    let model_after_rollback = status_after_rollback_payload
        .get("model")
        .and_then(Value::as_object)
        .expect("model object after rollback");
    assert_eq!(
        model_after_rollback
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );

    let artifact_status_2 = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("model-artifact-status")
        .arg("--service-name")
        .arg(&service_name)
        .arg("--training-job-id")
        .arg("job-002")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        artifact_status_2.status.success(),
        "model-artifact-status #2 failed with status {} and stderr: {}",
        artifact_status_2.status,
        String::from_utf8_lossy(&artifact_status_2.stderr)
    );
    let artifact_status_payload_2: Value =
        json::from_slice(&artifact_status_2.stdout).expect("model-artifact-status #2 json");
    let artifact_2 = artifact_status_payload_2
        .get("artifact")
        .and_then(Value::as_object)
        .expect("artifact object #2");
    assert_eq!(
        artifact_2.get("training_job_id").and_then(Value::as_str),
        Some("job-002")
    );
    assert_eq!(
        artifact_2
            .get("consumed_by_version")
            .and_then(Value::as_str),
        Some("1.1.0")
    );

    Ok(())
}

#[tokio::test]
async fn soracloud_templates_deploy_site_and_webapp_with_rollout_and_rollback() -> eyre::Result<()>
{
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(soracloud_templates_deploy_site_and_webapp_with_rollout_and_rollback),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let site_dir = dir.path().join("soracloud_site");
    let webapp_dir = dir.path().join("soracloud_webapp");

    let site_init = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("init")
        .arg("--template")
        .arg("site")
        .arg("--service-name")
        .arg("docs_portal")
        .arg("--service-version")
        .arg("1.0.0")
        .arg("--output-dir")
        .arg(site_dir.to_string_lossy().into_owned())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        site_init.status.success(),
        "site init failed with status {} and stderr: {}",
        site_init.status,
        String::from_utf8_lossy(&site_init.stderr)
    );

    let webapp_init = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("init")
        .arg("--template")
        .arg("webapp")
        .arg("--service-name")
        .arg("agent_console")
        .arg("--service-version")
        .arg("1.0.0")
        .arg("--output-dir")
        .arg(webapp_dir.to_string_lossy().into_owned())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        webapp_init.status.success(),
        "webapp init failed with status {} and stderr: {}",
        webapp_init.status,
        String::from_utf8_lossy(&webapp_init.stderr)
    );

    let site_service_path = site_dir.join("service_manifest.json");
    let webapp_service_path = webapp_dir.join("service_manifest.json");
    let site_service: SoraServiceManifestV1 =
        norito::json::from_slice(&std::fs::read(&site_service_path)?)?;
    let webapp_service: SoraServiceManifestV1 =
        norito::json::from_slice(&std::fs::read(&webapp_service_path)?)?;
    let site_package_json = std::fs::read_to_string(site_dir.join("site/package.json"))?;
    let webapp_frontend_package_json =
        std::fs::read_to_string(webapp_dir.join("webapp/frontend/package.json"))?;
    assert!(
        site_package_json.contains("\"build\": \"vite build\""),
        "site template must include a vite build script"
    );
    assert!(
        webapp_frontend_package_json.contains("\"build\": \"vite build\""),
        "webapp frontend template must include a vite build script"
    );
    assert_eq!(
        site_service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/")
    );
    assert_eq!(
        webapp_service
            .route
            .as_ref()
            .map(|route| route.path_prefix.as_str()),
        Some("/api")
    );

    let site_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("deploy")
        .arg("--container")
        .arg(
            site_dir
                .join("container_manifest.json")
                .to_string_lossy()
                .into_owned(),
        )
        .arg("--service")
        .arg(site_service_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        site_deploy.status.success(),
        "site deploy failed with status {} and stderr: {}",
        site_deploy.status,
        String::from_utf8_lossy(&site_deploy.stderr)
    );
    let site_deploy_payload: Value =
        json::from_slice(&site_deploy.stdout).expect("site deploy JSON payload");
    assert_eq!(
        site_deploy_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );

    let webapp_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("deploy")
        .arg("--container")
        .arg(
            webapp_dir
                .join("container_manifest.json")
                .to_string_lossy()
                .into_owned(),
        )
        .arg("--service")
        .arg(webapp_service_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        webapp_deploy.status.success(),
        "webapp deploy failed with status {} and stderr: {}",
        webapp_deploy.status,
        String::from_utf8_lossy(&webapp_deploy.stderr)
    );
    let webapp_deploy_payload: Value =
        json::from_slice(&webapp_deploy.stdout).expect("webapp deploy JSON payload");
    assert_eq!(
        webapp_deploy_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );

    let mut site_service_v2 = site_service.clone();
    site_service_v2.service_version = "1.1.0".to_string();
    let site_service_v2_path = site_dir.join("service_manifest_v2.json");
    tokio::fs::write(
        &site_service_v2_path,
        norito::json::to_vec_pretty(&site_service_v2).expect("encode site service v2"),
    )
    .await?;

    let site_upgrade = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("upgrade")
        .arg("--container")
        .arg(
            site_dir
                .join("container_manifest.json")
                .to_string_lossy()
                .into_owned(),
        )
        .arg("--service")
        .arg(site_service_v2_path.to_string_lossy().into_owned())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        site_upgrade.status.success(),
        "site upgrade failed with status {} and stderr: {}",
        site_upgrade.status,
        String::from_utf8_lossy(&site_upgrade.stderr)
    );
    let site_upgrade_payload: Value =
        json::from_slice(&site_upgrade.stdout).expect("site upgrade JSON payload");
    assert_eq!(
        site_upgrade_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.1.0")
    );
    let rollout_handle = site_upgrade_payload
        .get("rollout_handle")
        .and_then(Value::as_str)
        .expect("site upgrade should emit rollout handle");

    let site_rollout = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("rollout")
        .arg("--service-name")
        .arg("docs_portal")
        .arg("--rollout-handle")
        .arg(rollout_handle)
        .arg("--promote-to-percent")
        .arg("100")
        .arg("--governance-tx-hash")
        .arg(Hash::new(b"site-template-rollout").to_string())
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        site_rollout.status.success(),
        "site rollout failed with status {} and stderr: {}",
        site_rollout.status,
        String::from_utf8_lossy(&site_rollout.stderr)
    );
    let site_rollout_payload: Value =
        json::from_slice(&site_rollout.stdout).expect("site rollout JSON payload");
    assert_eq!(
        site_rollout_payload
            .get("stage")
            .and_then(Value::as_object)
            .and_then(|stage| stage.get("stage"))
            .and_then(Value::as_str),
        Some("Promoted")
    );

    let site_rollback = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("rollback")
        .arg("--service-name")
        .arg("docs_portal")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        site_rollback.status.success(),
        "site rollback failed with status {} and stderr: {}",
        site_rollback.status,
        String::from_utf8_lossy(&site_rollback.stderr)
    );
    let site_rollback_payload: Value =
        json::from_slice(&site_rollback.stdout).expect("site rollback JSON payload");
    assert_eq!(
        site_rollback_payload
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );

    let status = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("status")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        status.status.success(),
        "status failed with status {} and stderr: {}",
        status.status,
        String::from_utf8_lossy(&status.stderr)
    );

    let status_payload: Value =
        json::from_slice(&status.stdout).expect("CLI should emit soracloud status payload");
    let network_status = status_payload
        .get("network_status")
        .and_then(Value::as_object)
        .expect("network_status object");
    let control_plane = network_status
        .get("control_plane")
        .and_then(Value::as_object)
        .expect("control_plane object");
    assert_eq!(
        control_plane.get("service_count").and_then(Value::as_u64),
        Some(2)
    );

    let services = control_plane
        .get("services")
        .and_then(Value::as_array)
        .expect("services array");
    let site_snapshot = services
        .iter()
        .find(|service| {
            service
                .get("service_name")
                .and_then(Value::as_str)
                .is_some_and(|name| name == "docs_portal")
        })
        .expect("docs_portal service snapshot");
    let webapp_snapshot = services
        .iter()
        .find(|service| {
            service
                .get("service_name")
                .and_then(Value::as_str)
                .is_some_and(|name| name == "agent_console")
        })
        .expect("agent_console service snapshot");
    assert_eq!(
        site_snapshot.get("current_version").and_then(Value::as_str),
        Some("1.0.0")
    );
    assert_eq!(
        webapp_snapshot
            .get("current_version")
            .and_then(Value::as_str),
        Some("1.0.0")
    );
    assert_eq!(
        site_snapshot
            .get("latest_revision")
            .and_then(Value::as_object)
            .and_then(|revision| revision.get("route_path_prefix"))
            .and_then(Value::as_str),
        Some("/")
    );
    assert_eq!(
        webapp_snapshot
            .get("latest_revision")
            .and_then(Value::as_object)
            .and_then(|revision| revision.get("route_path_prefix"))
            .and_then(Value::as_str),
        Some("/api")
    );

    Ok(())
}

#[tokio::test]
async fn soracloud_agent_autonomy_runtime_uses_live_torii_control_plane() -> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(soracloud_agent_autonomy_runtime_uses_live_torii_control_plane),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut manifest: AgentApartmentManifestV1 = norito::json::from_slice(&std::fs::read(
        soracloud_fixture("fixtures/soracloud/agent_apartment_manifest_v1.json"),
    )?)?;
    manifest
        .policy_capabilities
        .push("agent.autonomy.run".parse().expect("valid capability"));
    manifest.validate().expect("manifest should remain valid");

    let manifest_path = dir.path().join("agent_apartment_manifest.json");
    tokio::fs::write(
        &manifest_path,
        norito::json::to_vec_pretty(&manifest).expect("encode apartment manifest"),
    )
    .await?;

    let deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-deploy")
        .arg("--manifest")
        .arg(manifest_path.to_string_lossy().into_owned())
        .arg("--lease-ticks")
        .arg("30")
        .arg("--autonomy-budget-units")
        .arg("500")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        deploy.status.success(),
        "agent deploy failed with status {} and stderr: {}",
        deploy.status,
        String::from_utf8_lossy(&deploy.stderr)
    );
    let deploy_payload: Value =
        json::from_slice(&deploy.stdout).expect("agent deploy JSON payload");
    assert_eq!(
        deploy_payload
            .get("action")
            .and_then(Value::as_object)
            .and_then(|action| action.get("action"))
            .and_then(Value::as_str),
        Some("Deploy")
    );
    assert_eq!(
        deploy_payload
            .get("budget_remaining_units")
            .and_then(Value::as_u64),
        Some(500)
    );

    let allow = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-artifact-allow")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--artifact-hash")
        .arg("hash:ABCD0123#01")
        .arg("--provenance-hash")
        .arg("hash:PROV0001#01")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        allow.status.success(),
        "artifact allow failed with status {} and stderr: {}",
        allow.status,
        String::from_utf8_lossy(&allow.stderr)
    );
    let allow_payload: Value = json::from_slice(&allow.stdout).expect("agent allow JSON payload");
    assert_eq!(
        allow_payload.get("allowlist_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        allow_payload
            .get("budget_remaining_units")
            .and_then(Value::as_u64),
        Some(500)
    );

    let run = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-autonomy-run")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--artifact-hash")
        .arg("hash:ABCD0123#01")
        .arg("--provenance-hash")
        .arg("hash:PROV0001#01")
        .arg("--budget-units")
        .arg("120")
        .arg("--run-label")
        .arg("nightly-train-step-1")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        run.status.success(),
        "autonomy run failed with status {} and stderr: {}",
        run.status,
        String::from_utf8_lossy(&run.stderr)
    );
    let run_payload: Value = json::from_slice(&run.stdout).expect("agent run JSON payload");
    assert_eq!(
        run_payload.get("run_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        run_payload
            .get("budget_remaining_units")
            .and_then(Value::as_u64),
        Some(380)
    );
    assert!(
        run_payload
            .get("run_id")
            .and_then(Value::as_str)
            .is_some_and(|run_id| !run_id.is_empty())
    );

    let status = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-autonomy-status")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        status.status.success(),
        "autonomy status failed with status {} and stderr: {}",
        status.status,
        String::from_utf8_lossy(&status.stderr)
    );
    let status_payload: Value =
        json::from_slice(&status.stdout).expect("autonomy status JSON payload");
    assert_eq!(
        status_payload
            .get("allowlist_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status_payload.get("run_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        status_payload
            .get("budget_remaining_units")
            .and_then(Value::as_u64),
        Some(380)
    );

    let revoke = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-policy-revoke")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--capability")
        .arg("agent.autonomy.run")
        .arg("--reason")
        .arg("manual-review")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        revoke.status.success(),
        "policy revoke failed with status {} and stderr: {}",
        revoke.status,
        String::from_utf8_lossy(&revoke.stderr)
    );
    let revoke_payload: Value =
        json::from_slice(&revoke.stdout).expect("policy revoke JSON payload");
    assert_eq!(
        revoke_payload
            .get("revoked_policy_capability_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    let revoked_run = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-autonomy-run")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--artifact-hash")
        .arg("hash:ABCD0123#01")
        .arg("--provenance-hash")
        .arg("hash:PROV0001#01")
        .arg("--budget-units")
        .arg("1")
        .arg("--run-label")
        .arg("revoked")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        !revoked_run.status.success(),
        "autonomy run with revoked capability should fail, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&revoked_run.stdout),
        String::from_utf8_lossy(&revoked_run.stderr)
    );
    assert!(
        String::from_utf8_lossy(&revoked_run.stderr).contains("agent.autonomy.run"),
        "unexpected revoked-capability error: {}",
        String::from_utf8_lossy(&revoked_run.stderr)
    );

    Ok(())
}

#[tokio::test]
async fn soracloud_agent_wallet_mailbox_and_lease_recovery_use_live_torii_control_plane()
-> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(soracloud_agent_wallet_mailbox_and_lease_recovery_use_live_torii_control_plane),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut sender: AgentApartmentManifestV1 = norito::json::from_slice(&std::fs::read(
        soracloud_fixture("fixtures/soracloud/agent_apartment_manifest_v1.json"),
    )?)?;
    sender
        .policy_capabilities
        .push("agent.mailbox.send".parse().expect("valid capability"));
    sender
        .validate()
        .expect("sender manifest should remain valid");

    let mut recipient = sender.clone();
    recipient.apartment_name = "worker_agent".parse().expect("valid apartment name");
    recipient
        .policy_capabilities
        .retain(|capability| capability.as_ref() != "agent.mailbox.send");
    recipient
        .policy_capabilities
        .push("agent.mailbox.receive".parse().expect("valid capability"));
    recipient
        .validate()
        .expect("recipient manifest should remain valid");

    let sender_manifest_path = dir.path().join("sender_agent_manifest.json");
    let recipient_manifest_path = dir.path().join("recipient_agent_manifest.json");
    tokio::fs::write(
        &sender_manifest_path,
        norito::json::to_vec_pretty(&sender).expect("encode sender manifest"),
    )
    .await?;
    tokio::fs::write(
        &recipient_manifest_path,
        norito::json::to_vec_pretty(&recipient).expect("encode recipient manifest"),
    )
    .await?;

    let sender_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-deploy")
        .arg("--manifest")
        .arg(sender_manifest_path.to_string_lossy().into_owned())
        .arg("--lease-ticks")
        .arg("1")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        sender_deploy.status.success(),
        "sender deploy failed with status {} and stderr: {}",
        sender_deploy.status,
        String::from_utf8_lossy(&sender_deploy.stderr)
    );

    let recipient_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-deploy")
        .arg("--manifest")
        .arg(recipient_manifest_path.to_string_lossy().into_owned())
        .arg("--lease-ticks")
        .arg("30")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        recipient_deploy.status.success(),
        "recipient deploy failed with status {} and stderr: {}",
        recipient_deploy.status,
        String::from_utf8_lossy(&recipient_deploy.stderr)
    );

    let expired_wallet = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-wallet-spend")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--asset-definition")
        .arg("xor#sora")
        .arg("--amount-nanos")
        .arg("1000")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        !expired_wallet.status.success(),
        "wallet spend with expired lease should fail, stdout: {}, stderr: {}",
        String::from_utf8_lossy(&expired_wallet.stdout),
        String::from_utf8_lossy(&expired_wallet.stderr)
    );
    assert!(
        String::from_utf8_lossy(&expired_wallet.stderr).contains("lease expired"),
        "unexpected lease-expiry rejection error: {}",
        String::from_utf8_lossy(&expired_wallet.stderr)
    );

    let renew = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-lease-renew")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--lease-ticks")
        .arg("20")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        renew.status.success(),
        "lease renew failed with status {} and stderr: {}",
        renew.status,
        String::from_utf8_lossy(&renew.stderr)
    );

    let restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-restart")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--reason")
        .arg("manual-restart")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        restart.status.success(),
        "restart failed with status {} and stderr: {}",
        restart.status,
        String::from_utf8_lossy(&restart.stderr)
    );
    let restart_payload: Value = json::from_slice(&restart.stdout).expect("restart JSON payload");
    assert_eq!(
        restart_payload.get("restart_count").and_then(Value::as_u64),
        Some(1)
    );

    let status = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-status")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        status.status.success(),
        "agent status failed with status {} and stderr: {}",
        status.status,
        String::from_utf8_lossy(&status.stderr)
    );
    let status_payload: Value =
        json::from_slice(&status.stdout).expect("agent status JSON payload");
    let apartments = status_payload
        .get("apartments")
        .and_then(Value::as_array)
        .expect("apartments array");
    assert_eq!(apartments.len(), 1);
    let apartment = &apartments[0];
    assert_eq!(
        apartment
            .get("status")
            .and_then(Value::as_object)
            .and_then(|status| status.get("status"))
            .and_then(Value::as_str),
        Some("Running")
    );
    assert_eq!(
        apartment.get("restart_count").and_then(Value::as_u64),
        Some(1)
    );

    let wallet_request = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-wallet-spend")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--asset-definition")
        .arg("xor#sora")
        .arg("--amount-nanos")
        .arg("1000000")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        wallet_request.status.success(),
        "wallet spend failed with status {} and stderr: {}",
        wallet_request.status,
        String::from_utf8_lossy(&wallet_request.stderr)
    );
    let wallet_request_payload: Value =
        json::from_slice(&wallet_request.stdout).expect("wallet request JSON payload");
    let request_id = wallet_request_payload
        .get("request_id")
        .and_then(Value::as_str)
        .expect("wallet request id present")
        .to_owned();
    assert_eq!(
        wallet_request_payload
            .get("pending_request_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    let wallet_approve = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-wallet-approve")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--request-id")
        .arg(request_id)
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        wallet_approve.status.success(),
        "wallet approve failed with status {} and stderr: {}",
        wallet_approve.status,
        String::from_utf8_lossy(&wallet_approve.stderr)
    );
    let wallet_approve_payload: Value =
        json::from_slice(&wallet_approve.stdout).expect("wallet approve JSON payload");
    assert_eq!(
        wallet_approve_payload
            .get("pending_request_count")
            .and_then(Value::as_u64),
        Some(0)
    );

    let message_send = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-message-send")
        .arg("--from-apartment")
        .arg("ops_agent")
        .arg("--to-apartment")
        .arg("worker_agent")
        .arg("--channel")
        .arg("ops.sync")
        .arg("--payload")
        .arg("rotate-key-42")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        message_send.status.success(),
        "message send failed with status {} and stderr: {}",
        message_send.status,
        String::from_utf8_lossy(&message_send.stderr)
    );
    let message_send_payload: Value =
        json::from_slice(&message_send.stdout).expect("message send JSON payload");
    let message_id = message_send_payload
        .get("message_id")
        .and_then(Value::as_str)
        .expect("message id present")
        .to_owned();
    assert_eq!(
        message_send_payload
            .get("pending_message_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    let mailbox_status_queued = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-mailbox-status")
        .arg("--apartment-name")
        .arg("worker_agent")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        mailbox_status_queued.status.success(),
        "mailbox status (queued) failed with status {} and stderr: {}",
        mailbox_status_queued.status,
        String::from_utf8_lossy(&mailbox_status_queued.stderr)
    );
    let mailbox_status_queued_payload: Value =
        json::from_slice(&mailbox_status_queued.stdout).expect("mailbox status JSON payload");
    assert_eq!(
        mailbox_status_queued_payload
            .get("pending_message_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let queued_messages = mailbox_status_queued_payload
        .get("messages")
        .and_then(Value::as_array)
        .expect("mailbox messages array");
    assert_eq!(queued_messages.len(), 1);
    assert_eq!(
        queued_messages[0].get("message_id").and_then(Value::as_str),
        Some(message_id.as_str())
    );

    let message_ack = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-message-ack")
        .arg("--apartment-name")
        .arg("worker_agent")
        .arg("--message-id")
        .arg(message_id)
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        message_ack.status.success(),
        "message ack failed with status {} and stderr: {}",
        message_ack.status,
        String::from_utf8_lossy(&message_ack.stderr)
    );
    let message_ack_payload: Value =
        json::from_slice(&message_ack.stdout).expect("message ack JSON payload");
    assert_eq!(
        message_ack_payload
            .get("pending_message_count")
            .and_then(Value::as_u64),
        Some(0)
    );

    let mailbox_status_empty = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-mailbox-status")
        .arg("--apartment-name")
        .arg("worker_agent")
        .arg("--torii-url")
        .arg(network.client().torii_url.to_string())
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        mailbox_status_empty.status.success(),
        "mailbox status (empty) failed with status {} and stderr: {}",
        mailbox_status_empty.status,
        String::from_utf8_lossy(&mailbox_status_empty.stderr)
    );
    let mailbox_status_empty_payload: Value =
        json::from_slice(&mailbox_status_empty.stdout).expect("mailbox status JSON payload");
    assert_eq!(
        mailbox_status_empty_payload
            .get("pending_message_count")
            .and_then(Value::as_u64),
        Some(0)
    );
    let empty_messages = mailbox_status_empty_payload
        .get("messages")
        .and_then(Value::as_array)
        .expect("empty mailbox messages array");
    assert!(empty_messages.is_empty());

    Ok(())
}

#[tokio::test]
async fn soracloud_agent_runtime_state_recovers_after_peer_restart_live_torii_control_plane()
-> eyre::Result<()> {
    let builder = NetworkBuilder::new()
        .with_min_peers(4)
        .with_config_layer(|layer| {
            layer
                .write("telemetry_enabled", true)
                .write("telemetry_profile", "full");
        });
    let Some(network) = sandbox::start_network_async_or_skip(
        builder,
        stringify!(
            soracloud_agent_runtime_state_recovers_after_peer_restart_live_torii_control_plane
        ),
    )
    .await?
    else {
        return Ok(());
    };

    let config = ProgramConfig::from(&network.client());
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut sender: AgentApartmentManifestV1 = norito::json::from_slice(&std::fs::read(
        soracloud_fixture("fixtures/soracloud/agent_apartment_manifest_v1.json"),
    )?)?;
    for capability in ["agent.mailbox.send", "agent.autonomy.run"] {
        let parsed = capability.parse().expect("valid capability");
        if !sender.policy_capabilities.contains(&parsed) {
            sender.policy_capabilities.push(parsed);
        }
    }
    sender
        .validate()
        .expect("sender manifest should remain valid");

    let mut recipient = sender.clone();
    recipient.apartment_name = "worker_agent".parse().expect("valid apartment name");
    recipient
        .policy_capabilities
        .retain(|capability| capability.as_ref() != "agent.mailbox.send");
    recipient
        .policy_capabilities
        .push("agent.mailbox.receive".parse().expect("valid capability"));
    recipient
        .validate()
        .expect("recipient manifest should remain valid");

    let sender_manifest_path = dir.path().join("sender_agent_manifest.json");
    let recipient_manifest_path = dir.path().join("recipient_agent_manifest.json");
    tokio::fs::write(
        &sender_manifest_path,
        norito::json::to_vec_pretty(&sender).expect("encode sender manifest"),
    )
    .await?;
    tokio::fs::write(
        &recipient_manifest_path,
        norito::json::to_vec_pretty(&recipient).expect("encode recipient manifest"),
    )
    .await?;

    let restart_peer = network.peers().first().expect("network peer").clone();
    let restart_torii_url = restart_peer.torii_url();

    let sender_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-deploy")
        .arg("--manifest")
        .arg(sender_manifest_path.to_string_lossy().into_owned())
        .arg("--lease-ticks")
        .arg("80")
        .arg("--autonomy-budget-units")
        .arg("500")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        sender_deploy.status.success(),
        "sender deploy failed with status {} and stderr: {}",
        sender_deploy.status,
        String::from_utf8_lossy(&sender_deploy.stderr)
    );

    let recipient_deploy = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-deploy")
        .arg("--manifest")
        .arg(recipient_manifest_path.to_string_lossy().into_owned())
        .arg("--lease-ticks")
        .arg("80")
        .arg("--autonomy-budget-units")
        .arg("250")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        recipient_deploy.status.success(),
        "recipient deploy failed with status {} and stderr: {}",
        recipient_deploy.status,
        String::from_utf8_lossy(&recipient_deploy.stderr)
    );

    let allow = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-artifact-allow")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--artifact-hash")
        .arg("hash:ABCD0123#01")
        .arg("--provenance-hash")
        .arg("hash:PROV0001#01")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        allow.status.success(),
        "artifact allow failed with status {} and stderr: {}",
        allow.status,
        String::from_utf8_lossy(&allow.stderr)
    );

    let run_before_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-autonomy-run")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--artifact-hash")
        .arg("hash:ABCD0123#01")
        .arg("--provenance-hash")
        .arg("hash:PROV0001#01")
        .arg("--budget-units")
        .arg("120")
        .arg("--run-label")
        .arg("before-restart")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        run_before_restart.status.success(),
        "autonomy run before restart failed with status {} and stderr: {}",
        run_before_restart.status,
        String::from_utf8_lossy(&run_before_restart.stderr)
    );
    let run_before_restart_payload: Value =
        json::from_slice(&run_before_restart.stdout).expect("autonomy run JSON payload");
    assert_eq!(
        run_before_restart_payload
            .get("budget_remaining_units")
            .and_then(Value::as_u64),
        Some(380)
    );
    assert_eq!(
        run_before_restart_payload
            .get("checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        run_before_restart_payload
            .get("persistent_state_key_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let persistent_bytes_after_first_run = run_before_restart_payload
        .get("persistent_state_total_bytes")
        .and_then(Value::as_u64)
        .expect("persistent_state_total_bytes after first run");
    assert!(
        persistent_bytes_after_first_run > 0,
        "first autonomy run should create a persisted checkpoint"
    );

    let wallet_request = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-wallet-spend")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--asset-definition")
        .arg("xor#sora")
        .arg("--amount-nanos")
        .arg("1000000")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        wallet_request.status.success(),
        "wallet request failed with status {} and stderr: {}",
        wallet_request.status,
        String::from_utf8_lossy(&wallet_request.stderr)
    );
    let wallet_request_payload: Value =
        json::from_slice(&wallet_request.stdout).expect("wallet request JSON payload");
    let request_id = wallet_request_payload
        .get("request_id")
        .and_then(Value::as_str)
        .expect("wallet request id present")
        .to_owned();
    assert_eq!(
        wallet_request_payload
            .get("pending_request_count")
            .and_then(Value::as_u64),
        Some(1)
    );

    let message_send = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-message-send")
        .arg("--from-apartment")
        .arg("ops_agent")
        .arg("--to-apartment")
        .arg("worker_agent")
        .arg("--channel")
        .arg("ops.sync")
        .arg("--payload")
        .arg("rotate-key-42")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        message_send.status.success(),
        "message send failed with status {} and stderr: {}",
        message_send.status,
        String::from_utf8_lossy(&message_send.stderr)
    );
    let message_send_payload: Value =
        json::from_slice(&message_send.stdout).expect("message send JSON payload");
    let message_id = message_send_payload
        .get("message_id")
        .and_then(Value::as_str)
        .expect("message id present")
        .to_owned();

    let config_layers: Vec<_> = network.config_layers().collect();
    restart_peer.shutdown().await;
    let restart_timeout = network.peer_startup_timeout();
    tokio::time::timeout(
        restart_timeout,
        restart_peer.start_checked(config_layers.iter().cloned(), None),
    )
    .await
    .map_err(|_| {
        eyre::eyre!(
            "restarted peer did not become healthy within {:?}",
            restart_timeout
        )
    })??;

    let status_after_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-status")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        status_after_restart.status.success(),
        "agent status after restart failed with status {} and stderr: {}",
        status_after_restart.status,
        String::from_utf8_lossy(&status_after_restart.stderr)
    );
    let status_after_restart_payload: Value =
        json::from_slice(&status_after_restart.stdout).expect("agent status JSON payload");
    let apartments = status_after_restart_payload
        .get("apartments")
        .and_then(Value::as_array)
        .expect("apartments array");
    assert_eq!(apartments.len(), 1);
    let apartment = &apartments[0];
    assert_eq!(
        apartment
            .get("status")
            .and_then(Value::as_object)
            .and_then(|status| status.get("status"))
            .and_then(Value::as_str),
        Some("Running")
    );
    assert_eq!(
        apartment
            .get("pending_wallet_request_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        apartment.get("autonomy_run_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        apartment.get("process_generation").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        apartment.get("checkpoint_count").and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        apartment
            .get("persistent_state_key_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        apartment
            .get("persistent_state_total_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes >= persistent_bytes_after_first_run)
    );
    assert!(
        apartment
            .get("lease_remaining_ticks")
            .and_then(Value::as_u64)
            .is_some_and(|ticks| ticks > 0)
    );
    let process_generation_before_manual_restart = apartment
        .get("process_generation")
        .and_then(Value::as_u64)
        .expect("process_generation in status after peer restart");

    let manual_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-restart")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--reason")
        .arg("resume-after-peer-restart")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        manual_restart.status.success(),
        "manual agent restart after peer restart failed with status {} and stderr: {}",
        manual_restart.status,
        String::from_utf8_lossy(&manual_restart.stderr)
    );
    let manual_restart_payload: Value =
        json::from_slice(&manual_restart.stdout).expect("manual restart payload");
    let process_generation_after_manual_restart = manual_restart_payload
        .get("process_generation")
        .and_then(Value::as_u64)
        .expect("process_generation after manual restart");
    assert_eq!(
        process_generation_after_manual_restart,
        process_generation_before_manual_restart.saturating_add(1)
    );

    let autonomy_status_after_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-autonomy-status")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        autonomy_status_after_restart.status.success(),
        "autonomy status after restart failed with status {} and stderr: {}",
        autonomy_status_after_restart.status,
        String::from_utf8_lossy(&autonomy_status_after_restart.stderr)
    );
    let autonomy_status_after_restart_payload: Value =
        json::from_slice(&autonomy_status_after_restart.stdout).expect("autonomy status payload");
    assert_eq!(
        autonomy_status_after_restart_payload
            .get("run_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        autonomy_status_after_restart_payload
            .get("budget_remaining_units")
            .and_then(Value::as_u64),
        Some(380)
    );
    assert_eq!(
        autonomy_status_after_restart_payload
            .get("process_generation")
            .and_then(Value::as_u64),
        Some(process_generation_after_manual_restart)
    );
    assert_eq!(
        autonomy_status_after_restart_payload
            .get("checkpoint_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert_eq!(
        autonomy_status_after_restart_payload
            .get("persistent_state_key_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    assert!(
        autonomy_status_after_restart_payload
            .get("persistent_state_total_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes >= persistent_bytes_after_first_run)
    );

    let mailbox_status_after_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-mailbox-status")
        .arg("--apartment-name")
        .arg("worker_agent")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        mailbox_status_after_restart.status.success(),
        "mailbox status after restart failed with status {} and stderr: {}",
        mailbox_status_after_restart.status,
        String::from_utf8_lossy(&mailbox_status_after_restart.stderr)
    );
    let mailbox_status_after_restart_payload: Value =
        json::from_slice(&mailbox_status_after_restart.stdout).expect("mailbox status payload");
    assert_eq!(
        mailbox_status_after_restart_payload
            .get("pending_message_count")
            .and_then(Value::as_u64),
        Some(1)
    );
    let queued_messages = mailbox_status_after_restart_payload
        .get("messages")
        .and_then(Value::as_array)
        .expect("mailbox messages array");
    assert_eq!(queued_messages.len(), 1);
    assert_eq!(
        queued_messages[0].get("message_id").and_then(Value::as_str),
        Some(message_id.as_str())
    );

    let wallet_approve_after_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-wallet-approve")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--request-id")
        .arg(request_id)
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        wallet_approve_after_restart.status.success(),
        "wallet approve after restart failed with status {} and stderr: {}",
        wallet_approve_after_restart.status,
        String::from_utf8_lossy(&wallet_approve_after_restart.stderr)
    );
    let wallet_approve_after_restart_payload: Value =
        json::from_slice(&wallet_approve_after_restart.stdout).expect("wallet approve payload");
    assert_eq!(
        wallet_approve_after_restart_payload
            .get("pending_request_count")
            .and_then(Value::as_u64),
        Some(0)
    );

    let message_ack_after_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-message-ack")
        .arg("--apartment-name")
        .arg("worker_agent")
        .arg("--message-id")
        .arg(message_id)
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        message_ack_after_restart.status.success(),
        "message ack after restart failed with status {} and stderr: {}",
        message_ack_after_restart.status,
        String::from_utf8_lossy(&message_ack_after_restart.stderr)
    );
    let message_ack_after_restart_payload: Value =
        json::from_slice(&message_ack_after_restart.stdout).expect("message ack payload");
    assert_eq!(
        message_ack_after_restart_payload
            .get("pending_message_count")
            .and_then(Value::as_u64),
        Some(0)
    );

    let run_after_restart = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-autonomy-run")
        .arg("--apartment-name")
        .arg("ops_agent")
        .arg("--artifact-hash")
        .arg("hash:ABCD0123#01")
        .arg("--provenance-hash")
        .arg("hash:PROV0001#01")
        .arg("--budget-units")
        .arg("50")
        .arg("--run-label")
        .arg("after-restart")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        run_after_restart.status.success(),
        "autonomy run after restart failed with status {} and stderr: {}",
        run_after_restart.status,
        String::from_utf8_lossy(&run_after_restart.stderr)
    );
    let run_after_restart_payload: Value =
        json::from_slice(&run_after_restart.stdout).expect("autonomy run after restart payload");
    assert_eq!(
        run_after_restart_payload
            .get("run_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        run_after_restart_payload
            .get("budget_remaining_units")
            .and_then(Value::as_u64),
        Some(330)
    );
    assert_eq!(
        run_after_restart_payload
            .get("checkpoint_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        run_after_restart_payload
            .get("persistent_state_key_count")
            .and_then(Value::as_u64),
        Some(2)
    );
    assert_eq!(
        run_after_restart_payload
            .get("process_generation")
            .and_then(Value::as_u64),
        Some(process_generation_after_manual_restart)
    );
    assert!(
        run_after_restart_payload
            .get("persistent_state_total_bytes")
            .and_then(Value::as_u64)
            .is_some_and(|bytes| bytes > persistent_bytes_after_first_run)
    );

    let mailbox_status_empty = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("app")
        .arg("soracloud")
        .arg("agent-mailbox-status")
        .arg("--apartment-name")
        .arg("worker_agent")
        .arg("--torii-url")
        .arg(&restart_torii_url)
        .envs(config.envs())
        .output()
        .await?;
    assert!(
        mailbox_status_empty.status.success(),
        "mailbox status final check failed with status {} and stderr: {}",
        mailbox_status_empty.status,
        String::from_utf8_lossy(&mailbox_status_empty.stderr)
    );
    let mailbox_status_empty_payload: Value =
        json::from_slice(&mailbox_status_empty.stdout).expect("mailbox status payload");
    assert_eq!(
        mailbox_status_empty_payload
            .get("pending_message_count")
            .and_then(Value::as_u64),
        Some(0)
    );

    Ok(())
}

#[tokio::test]
async fn soracloud_agent_autonomy_commands_require_torii_url() -> eyre::Result<()> {
    let config = local_program_config();
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut manifest: AgentApartmentManifestV1 = norito::json::from_slice(&std::fs::read(
        soracloud_fixture("fixtures/soracloud/agent_apartment_manifest_v1.json"),
    )?)?;
    manifest
        .policy_capabilities
        .push("agent.autonomy.run".parse().expect("valid capability"));
    manifest.validate().expect("manifest should remain valid");

    let manifest_path = dir.path().join("agent_apartment_manifest.json");
    tokio::fs::write(
        &manifest_path,
        norito::json::to_vec_pretty(&manifest).expect("encode apartment manifest"),
    )
    .await?;

    let deploy = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-deploy",
            "--manifest",
            &manifest_path.to_string_lossy(),
            "--autonomy-budget-units",
            "500",
        ],
    )
    .await?;
    assert_requires_torii_url(&deploy);

    let allow = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-artifact-allow",
            "--apartment-name",
            "ops_agent",
            "--artifact-hash",
            "hash:ABCD0123#01",
            "--provenance-hash",
            "hash:PROV0001#01",
        ],
    )
    .await?;
    assert_requires_torii_url(&allow);

    let run = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-autonomy-run",
            "--apartment-name",
            "ops_agent",
            "--artifact-hash",
            "hash:ABCD0123#01",
            "--provenance-hash",
            "hash:PROV0001#01",
            "--budget-units",
            "120",
            "--run-label",
            "nightly-train-step-1",
        ],
    )
    .await?;
    assert_requires_torii_url(&run);

    let status = run_soracloud_command(
        dir.path(),
        &config,
        &["agent-autonomy-status", "--apartment-name", "ops_agent"],
    )
    .await?;
    assert_requires_torii_url(&status);

    let revoke = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-policy-revoke",
            "--apartment-name",
            "ops_agent",
            "--capability",
            "agent.autonomy.run",
            "--reason",
            "manual-review",
        ],
    )
    .await?;
    assert_requires_torii_url(&revoke);

    Ok(())
}

#[tokio::test]
async fn soracloud_agent_wallet_and_mailbox_commands_require_torii_url() -> eyre::Result<()> {
    let config = local_program_config();
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut sender: AgentApartmentManifestV1 = norito::json::from_slice(&std::fs::read(
        soracloud_fixture("fixtures/soracloud/agent_apartment_manifest_v1.json"),
    )?)?;
    sender
        .policy_capabilities
        .push("agent.mailbox.send".parse().expect("valid capability"));
    sender
        .validate()
        .expect("sender manifest should remain valid");

    let mut recipient = sender.clone();
    recipient.apartment_name = "worker_agent".parse().expect("valid apartment name");
    recipient
        .policy_capabilities
        .retain(|capability| capability.as_ref() != "agent.mailbox.send");
    recipient
        .policy_capabilities
        .push("agent.mailbox.receive".parse().expect("valid capability"));
    recipient
        .validate()
        .expect("recipient manifest should remain valid");

    let sender_manifest_path = dir.path().join("sender_agent_manifest.json");
    let recipient_manifest_path = dir.path().join("recipient_agent_manifest.json");
    tokio::fs::write(
        &sender_manifest_path,
        norito::json::to_vec_pretty(&sender).expect("encode sender manifest"),
    )
    .await?;
    tokio::fs::write(
        &recipient_manifest_path,
        norito::json::to_vec_pretty(&recipient).expect("encode recipient manifest"),
    )
    .await?;

    let sender_deploy = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-deploy",
            "--manifest",
            &sender_manifest_path.to_string_lossy(),
        ],
    )
    .await?;
    assert_requires_torii_url(&sender_deploy);

    let recipient_deploy = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-deploy",
            "--manifest",
            &recipient_manifest_path.to_string_lossy(),
        ],
    )
    .await?;
    assert_requires_torii_url(&recipient_deploy);

    let wallet_request = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-wallet-spend",
            "--apartment-name",
            "ops_agent",
            "--asset-definition",
            "xor#sora",
            "--amount-nanos",
            "1000000",
        ],
    )
    .await?;
    assert_requires_torii_url(&wallet_request);

    let wallet_approve = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-wallet-approve",
            "--apartment-name",
            "ops_agent",
            "--request-id",
            "req-1",
        ],
    )
    .await?;
    assert_requires_torii_url(&wallet_approve);

    let wallet_revoke = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-policy-revoke",
            "--apartment-name",
            "ops_agent",
            "--capability",
            "wallet.sign",
            "--reason",
            "rotated",
        ],
    )
    .await?;
    assert_requires_torii_url(&wallet_revoke);

    let message_send = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-message-send",
            "--from-apartment",
            "ops_agent",
            "--to-apartment",
            "worker_agent",
            "--channel",
            "ops.sync",
            "--payload",
            "rotate-key-42",
        ],
    )
    .await?;
    assert_requires_torii_url(&message_send);

    let message_ack = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-message-ack",
            "--apartment-name",
            "worker_agent",
            "--message-id",
            "msg-1",
        ],
    )
    .await?;
    assert_requires_torii_url(&message_ack);

    let mailbox_status = run_soracloud_command(
        dir.path(),
        &config,
        &["agent-mailbox-status", "--apartment-name", "worker_agent"],
    )
    .await?;
    assert_requires_torii_url(&mailbox_status);

    Ok(())
}

#[tokio::test]
async fn soracloud_agent_lease_commands_require_torii_url() -> eyre::Result<()> {
    let config = local_program_config();
    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let manifest: AgentApartmentManifestV1 = norito::json::from_slice(&std::fs::read(
        soracloud_fixture("fixtures/soracloud/agent_apartment_manifest_v1.json"),
    )?)?;
    let manifest_path = dir.path().join("agent_apartment_manifest.json");
    tokio::fs::write(
        &manifest_path,
        norito::json::to_vec_pretty(&manifest).expect("encode apartment manifest"),
    )
    .await?;

    let deploy = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-deploy",
            "--manifest",
            &manifest_path.to_string_lossy(),
            "--lease-ticks",
            "1",
        ],
    )
    .await?;
    assert_requires_torii_url(&deploy);

    let renew = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-lease-renew",
            "--apartment-name",
            "ops_agent",
            "--lease-ticks",
            "20",
        ],
    )
    .await?;
    assert_requires_torii_url(&renew);

    let wallet = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-wallet-spend",
            "--apartment-name",
            "ops_agent",
            "--asset-definition",
            "xor#sora",
            "--amount-nanos",
            "1000",
        ],
    )
    .await?;
    assert_requires_torii_url(&wallet);

    let restart = run_soracloud_command(
        dir.path(),
        &config,
        &[
            "agent-restart",
            "--apartment-name",
            "ops_agent",
            "--reason",
            "operator-check",
        ],
    )
    .await?;
    assert_requires_torii_url(&restart);

    let status = run_soracloud_command(
        dir.path(),
        &config,
        &["agent-status", "--apartment-name", "ops_agent"],
    )
    .await?;
    assert_requires_torii_url(&status);

    Ok(())
}
