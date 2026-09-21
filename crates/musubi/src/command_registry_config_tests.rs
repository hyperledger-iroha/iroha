//! Registry wallet selectors and workspace binding tests use only private temporary fixtures.

use super::*;
use iroha::crypto::{Hash, HashOf};
use iroha_wallet::{WalletNetwork, WalletStore};
use tempfile::TempDir;

fn fixture() -> (TempDir, PathBuf, WalletStore) {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().join("project");
    let result = invoke([
        OsString::from("musubi"),
        "new".into(),
        root.as_os_str().into(),
        "--namespace".into(),
        "demo".into(),
    ])
    .output
    .render(OutputFormat::Human)
    .unwrap();
    assert_eq!(result.exit_code(), 0, "{}", result.stderr());
    let store = WalletStore::open(&temporary.path().join("wallets"), Some(&root)).unwrap();
    (temporary, root, store)
}

fn network(seed: u8) -> WalletNetwork {
    WalletNetwork::new(
        iroha_data_model::NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(Hash::new(
            [seed],
        ))),
        "fc56984b-2be7-431d-840e-21514d1883f0".into(),
        "https://taira.sora.org/".parse().unwrap(),
        369,
    )
    .unwrap()
}

fn bind(root: &Path, wallet: &iroha_wallet::WalletInfo) {
    fs::write(root.join("Musubi.networks.toml"), format!(
        "version = 1\ndefault = \"taira\"\n[networks.taira]\nconfig = {}\nnetwork-id = {}\nchain-discriminant = 369\n[networks.taira.fee]\npayer = \"authority\"\n",
        toml::Value::String(wallet.config_path.display().to_string()),
        toml::Value::String(wallet.network.network_id.to_string()),
    )).unwrap();
}

#[test]
fn registry_commands_accept_wallet_selection_and_reject_conflicting_native_config() {
    for prefix in [
        vec!["publish"],
        vec!["search", "coffee"],
        vec!["info", "demo/coffee"],
        vec!["versions", "demo/coffee"],
        vec!["owner", "list", "demo/coffee"],
    ] {
        let mut args = vec!["musubi"];
        args.extend(prefix);
        args.extend(["--wallet", "alice", "--wallet-dir", "/runtime/wallets"]);
        assert!(Cli::try_parse_from(&args).is_ok(), "{args:?}");
        args.extend(["--config", "/runtime/client.toml"]);
        assert!(Cli::try_parse_from(&args).is_err(), "{args:?}");
    }
}

#[test]
fn explicit_wallet_selection_and_workspace_defaults_need_only_public_metadata() {
    let (_temporary, root, store) = fixture();
    let info = store.create("alice", &network(1)).unwrap();
    bind(&root, &info);
    fs::remove_file(info.config_path.parent().unwrap().join("private.key")).unwrap();
    let selected = NetworkArgs {
        wallet: Some("alice".into()),
        wallet_dir: Some(store.root().to_owned()),
        ..NetworkArgs::default()
    };
    assert_eq!(selected.config_path().unwrap(), info.config_path);
    let direct = selected.workspace_image(None).unwrap();
    assert_eq!(
        direct.registry_binding().unwrap(),
        (info.network.network_id, 369)
    );
    let inherited = NetworkArgs::default().workspace_image(Some(&root)).unwrap();
    assert_eq!(inherited.path(), info.config_path);
    let resumed = NetworkArgs::default()
        .publication_image(Some(&root.join("Musubi.toml")))
        .unwrap();
    assert_eq!(resumed.path(), info.config_path);
    assert_eq!(resumed.bytes(), inherited.bytes());
    assert!(
        NetworkArgs::default()
            .publication_image(Some(&root.join("missing.toml")))
            .is_err()
    );
}

#[test]
fn explicit_registry_wallet_cannot_change_a_pinned_workspace_network() {
    let (_temporary, root, store) = fixture();
    let selected = store.create("alice", &network(2)).unwrap();
    bind(&root, &selected);
    let other = store.create("bob", &network(3)).unwrap();
    let args = NetworkArgs {
        wallet: Some("bob".into()),
        wallet_dir: Some(store.root().to_owned()),
        ..NetworkArgs::default()
    };
    assert!(args.workspace_image(Some(&root)).is_err());
    let explicit = NetworkArgs {
        config: Some(other.config_path),
        ..NetworkArgs::default()
    };
    assert!(explicit.workspace_image(Some(&root)).is_err());
    assert_eq!(
        NetworkArgs::default()
            .workspace_image(Some(&root))
            .unwrap()
            .registry_binding()
            .unwrap()
            .0,
        selected.network.network_id
    );
}
