#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests of the Iroha Client CLI

use std::{path::PathBuf, sync::Once, time::Duration};

use integration_tests::sandbox;
use iroha::{
    client::Client,
    config::{DEFAULT_TRANSACTION_STATUS_TIMEOUT, DEFAULT_TRANSACTION_TIME_TO_LIVE},
    crypto::{ExposedPrivateKey, Hash, KeyPair},
    data_model::{
        Encode,
        soracloud::{SoraContainerManifestV1, SoraServiceManifestV1},
    },
};
use iroha_config_base::toml::WriteExt;
use iroha_data_model::prelude::AccountId;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::sample_ivm_path;
use norito::json::{self, Value};
use reqwest::Url;

fn program() -> PathBuf {
    enable_reentrant_builds_for_tests();
    iroha_test_network::Program::Iroha.resolve().unwrap()
}

fn enable_reentrant_builds_for_tests() {
    static INIT: Once = Once::new();
    INIT.call_once(|| {
        // Cargo sets `CARGO` for test binaries, which disables reentrant builds by default.
        // Allow nested builds so the CLI binary can be compiled on-demand in fresh workspaces.
        set_env_var("IROHA_TEST_ALLOW_REENTRANT_BUILD", "1");
    });
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

struct ProgramConfig {
    torii_url: Url,
    account: AccountId,
    key: KeyPair,
    status_timeout: Duration,
    ttl: Duration,
}

impl From<&Client> for ProgramConfig {
    fn from(value: &Client) -> Self {
        let torii_url = value.torii_url.clone();
        let account = value.account.clone();
        let key = value.key_pair.clone();
        let status_timeout = value.transaction_status_timeout;
        let ttl = value
            .transaction_ttl
            .unwrap_or(DEFAULT_TRANSACTION_TIME_TO_LIVE);
        Self {
            torii_url,
            account,
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
            ("ACCOUNT_DOMAIN", self.account.domain().to_string()),
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
            .write(["account", "domain"], self.account.domain().to_string())
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
