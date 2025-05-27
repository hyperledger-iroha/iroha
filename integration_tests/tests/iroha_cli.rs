//! Integration tests of the Iroha Client CLI

use std::path::PathBuf;

use iroha::{
    client::Client,
    crypto::{ExposedPrivateKey, KeyPair},
};
use iroha_config_base::toml::WriteExt;
use iroha_data_model::prelude::AccountId;
use iroha_test_network::NetworkBuilder;
use iroha_test_samples::sample_wasm_path;
use reqwest::Url;

fn program() -> PathBuf {
    iroha_test_network::Program::Iroha.resolve().unwrap()
}

struct ProgramConfig {
    torii_url: Url,
    account: AccountId,
    key: KeyPair,
}

impl From<&Client> for ProgramConfig {
    fn from(value: &Client) -> Self {
        let torii_url = value.torii_url.clone();
        let account = value.account.clone();
        let key = value.key_pair.clone();
        Self {
            torii_url,
            account,
            key,
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
        ]
    }

    fn toml(&self) -> toml::Table {
        toml::Table::new()
            .write("chain", iroha_test_network::chain_id())
            .write("torii_url", &self.torii_url)
            .write(["account", "domain"], self.account.domain())
            .write(
                ["account", "private_key"],
                ExposedPrivateKey(self.key.private_key().clone()),
            )
            .write(["account", "public_key"], self.key.public_key())
    }
}

#[tokio::test]
async fn can_upgrade_executor() -> eyre::Result<()> {
    // Assuming Alice already has the CanUpgradeExecutor permission
    let network = NetworkBuilder::new()
        .with_wasm_fuel(iroha_test_network::WasmFuelConfig::Auto)
        .start()
        .await?;

    let config = ProgramConfig::from(&network.client());
    let mut child = tokio::process::Command::new(program())
        .current_dir(iroha_test_network::repo_root())
        .envs(config.envs())
        .arg("executor")
        .arg("upgrade")
        .arg("--path")
        .arg(sample_wasm_path("executor_with_admin"))
        .spawn()?;
    let exit_status = child.wait().await?;

    assert!(exit_status.success());

    Ok(())
}

#[tokio::test]
async fn reads_client_toml_by_default() -> eyre::Result<()> {
    let network = NetworkBuilder::new().start().await?;
    let config = ProgramConfig::from(&network.client());

    let dir = tempfile::tempdir()?;
    tokio::fs::write(
        dir.path().join("client.toml"),
        toml::to_string(&config.toml())?.as_bytes(),
    )
    .await?;

    let mut child = tokio::process::Command::new(program())
        .current_dir(dir.path())
        .arg("domain")
        .arg("list")
        .arg("all")
        .spawn()?;
    let exit_status = child.wait().await?;

    assert!(exit_status.success());

    Ok(())
}

// Add more CLI tests here!
