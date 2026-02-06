#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
//! Integration tests of the Iroha Client CLI

use std::{path::PathBuf, sync::Once, time::Duration};

use integration_tests::sandbox;
use iroha::{
    client::Client,
    config::{DEFAULT_TRANSACTION_STATUS_TIMEOUT, DEFAULT_TRANSACTION_TIME_TO_LIVE},
    crypto::{ExposedPrivateKey, KeyPair},
};
use iroha_config_base::toml::WriteExt;
use iroha_data_model::prelude::AccountId;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::sample_ivm_path;
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
