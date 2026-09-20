//! Tempfile-only custody regressions; no live signer or network is created.
use super::*;
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use tempfile::TempDir;

fn network() -> WalletNetwork {
    WalletNetwork::new(
        "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0"
            .parse()
            .unwrap(),
        ChainId::from(TAIRA_CHAIN_ID),
        "http://127.0.0.1:8080/".parse().unwrap(),
        369,
    )
    .unwrap()
}

fn key(seed: u8) -> KeyPair {
    KeyPair::try_from_seed(vec![seed; 32], iroha_crypto::Algorithm::Ed25519).unwrap()
}

fn fixture() -> (TempDir, WalletStore) {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir(&project).unwrap();
    let store = WalletStore::open(&temporary.path().join("wallets"), Some(&project)).unwrap();
    (temporary, store)
}

fn write_private(path: &Path, bytes: &[u8]) {
    fs::write(path, bytes).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
}

#[test]
fn generated_wallet_is_private_native_and_public_output_has_no_key() {
    let (_temporary, store) = fixture();
    let info = store.create("developer", &network()).unwrap();
    assert_eq!(store.root().metadata().unwrap().mode() & 0o777, 0o700);
    let root = info.config_path.parent().unwrap();
    assert_eq!(root.metadata().unwrap().mode() & 0o777, 0o700);
    for name in ["wallet.json", "client.toml", "private.key"] {
        let metadata = root.join(name).metadata().unwrap();
        assert_eq!(metadata.mode() & 0o777, 0o600);
        assert_eq!(metadata.nlink(), 1);
    }
    let loaded = store.load_config("developer").unwrap();
    assert_eq!(loaded.network_id, network().network_id);
    assert_eq!(loaded.account_chain_discriminant, 369);
    assert_eq!(loaded.key_pair.public_key().to_string(), info.public_key);
    let _profile = ChainDiscriminantGuard::enter(369);
    assert_eq!(loaded.account.to_string(), info.account_id);
    let native = Config::load_file(&info.config_path).unwrap();
    assert_eq!(native.key_pair.public_key(), loaded.key_pair.public_key());
    let secret = ExposedPrivateKey(loaded.key_pair.private_key().clone()).to_string();
    let public = json::to_json(&info).unwrap();
    assert!(!public.contains(&secret));
    assert!(!format!("{info:?}").contains(&secret));
    let config = fs::read_to_string(&info.config_path).unwrap();
    assert!(config.contains("private_key_file = \"private.key\""));
    assert!(!config.contains(&secret));
    assert_eq!(store.config_path("developer").unwrap(), info.config_path);
}

#[test]
fn wallets_are_distinct_sorted_and_duplicate_names_never_replace() {
    let (_temporary, store) = fixture();
    let z = store.create("zebra", &network()).unwrap();
    let a = store.create("alice", &network()).unwrap();
    assert_ne!(z.public_key, a.public_key);
    let before = fs::read(a.config_path.parent().unwrap().join("private.key")).unwrap();
    assert!(store.create("alice", &network()).is_err());
    assert_eq!(
        before,
        fs::read(a.config_path.parent().unwrap().join("private.key")).unwrap()
    );
    assert_eq!(
        store
            .list()
            .unwrap()
            .iter()
            .map(|info| info.name.as_str())
            .collect::<Vec<_>>(),
        ["alice", "zebra"]
    );
    assert!(fs::read_dir(store.root()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".pending-")
    }));
}

#[test]
fn pending_entries_cannot_bypass_the_collection_scan_bound() {
    let (_temporary, store) = fixture();
    for index in 0..MAX_WALLETS {
        fs::create_dir(store.root().join(format!(".pending-{index}"))).unwrap();
    }
    assert!(store.list().unwrap().is_empty());
    fs::create_dir(store.root().join(".pending-over-bound")).unwrap();
    assert!(
        store
            .list()
            .unwrap_err()
            .to_string()
            .contains("fixed entry bound")
    );
}

#[test]
fn import_key_preserves_signer_and_rejects_noncanonical_or_unsafe_sources() {
    let (temporary, store) = fixture();
    let source = temporary.path().join("key.input");
    let encoded = format!("{}\n", ExposedPrivateKey(key(3).private_key().clone()));
    write_private(&source, encoded.as_bytes());
    let imported = store
        .import_key_file("imported", &network(), &source)
        .unwrap();
    assert_eq!(imported.public_key, key(3).public_key().to_string());
    assert_eq!(fs::read_to_string(&source).unwrap(), encoded);
    assert_eq!(store.load_config("imported").unwrap().key_pair, key(3));
    fs::set_permissions(&source, fs::Permissions::from_mode(0o400)).unwrap();
    assert_eq!(
        store
            .import_key_file("readonly", &network(), &source)
            .unwrap()
            .public_key,
        imported.public_key
    );
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
    for bad in [
        "",
        "secret-do-not-echo",
        "first\nsecond\n",
        " bad-key ",
        "key\r\n",
    ] {
        write_private(&source, bad.as_bytes());
        let error = store
            .import_key_file("rejected", &network(), &source)
            .unwrap_err();
        assert!(!format!("{error:#}").contains("secret-do-not-echo"));
        assert!(!store.root().join("rejected").exists());
    }
    write_private(&source, encoded.as_bytes());
    fs::set_permissions(&source, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.import_key_file("wide", &network(), &source).is_err());
    fs::set_permissions(&source, fs::Permissions::from_mode(0o600)).unwrap();
    let alias = temporary.path().join("linked-key");
    symlink(&source, &alias).unwrap();
    assert!(
        store
            .import_key_file("symbolic", &network(), &alias)
            .is_err()
    );
    fs::remove_file(&alias).unwrap();
    fs::hard_link(&source, &alias).unwrap();
    assert!(
        store
            .import_key_file("hardlinked", &network(), &source)
            .is_err()
    );
}

#[test]
fn client_import_resolves_exact_relative_key_and_network_identity_files() {
    let (temporary, store) = fixture();
    let source_dir = temporary.path().join("input");
    fs::create_dir(&source_dir).unwrap();
    let record = WalletRecord {
        schema: WALLET_SCHEMA.to_owned(),
        name: "input".to_owned(),
        public_key: key(5).public_key().to_string(),
        network: network(),
    };
    write_private(
        &source_dir.join("private.key"),
        format!("{}\n", ExposedPrivateKey(key(5).private_key().clone())).as_bytes(),
    );
    fs::write(
        source_dir.join("network.id"),
        format!("{}\n", network().network_id),
    )
    .unwrap();
    let config = render_config(&record, None).unwrap().replace(
        &format!(
            "network_id = {}",
            toml::Value::String(network().network_id.to_string())
        ),
        "network_id_file = \"network.id\"",
    );
    write_private(&source_dir.join("client.toml"), config.as_bytes());
    let info = store
        .import_client_file("connected", &source_dir.join("client.toml"))
        .unwrap();
    assert_eq!(info.network, network());
    assert_eq!(store.load_config("connected").unwrap().key_pair, key(5));
    fs::remove_file(source_dir.join("private.key")).unwrap();
    assert_eq!(
        store.load_config("connected").unwrap().key_pair,
        key(5),
        "imported custody is independent from its source"
    );
    let private_inline = render_config(
        &record,
        Some(&ExposedPrivateKey(key(5).private_key().clone()).to_string()),
    )
    .unwrap();
    write_private(&source_dir.join("client.toml"), private_inline.as_bytes());
    assert!(
        store
            .import_client_file("inline", &source_dir.join("client.toml"))
            .is_ok()
    );
    write_private(
        &source_dir.join("client.toml"),
        format!(
            "{private_inline}\n[basic_auth]\nweb_login=\"login\"\npassword=\"never-echo-this\"\n"
        )
        .as_bytes(),
    );
    let error = store
        .import_client_file("auth", &source_dir.join("client.toml"))
        .unwrap_err();
    assert!(!format!("{error:#}").contains("never-echo-this"));
}

#[test]
fn public_listing_does_not_load_keys_but_signing_rejects_key_substitution() {
    let (_temporary, store) = fixture();
    let info = store.create("developer", &network()).unwrap();
    let key_path = info.config_path.parent().unwrap().join("private.key");
    write_private(&key_path, b"unreadable-as-key");
    assert_eq!(store.show("developer").unwrap(), info);
    assert_eq!(store.list().unwrap(), vec![info.clone()]);
    assert!(store.load_config("developer").is_err());
    write_private(
        &key_path,
        ExposedPrivateKey(key(9).private_key().clone())
            .to_string()
            .as_bytes(),
    );
    assert!(store.load_config("developer").is_err());
    fs::set_permissions(&key_path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(store.load_config("developer").is_err());
}

#[test]
fn wallet_storage_rejects_project_paths_names_and_nonprivate_roots() {
    let temporary = tempfile::tempdir().unwrap();
    let project = temporary.path().join("project");
    fs::create_dir(&project).unwrap();
    assert!(WalletStore::open(&project.join("wallet"), Some(&project)).is_err());
    assert!(WalletStore::open(temporary.path(), Some(&project)).is_err());
    for marker in [".git", "Musubi.toml"] {
        fs::write(project.join(marker), b"marker").unwrap();
        assert!(WalletStore::open(&project.join("wallet"), None).is_err());
        fs::remove_file(project.join(marker)).unwrap();
    }
    assert!(!project.join("wallet").exists());
    let store = WalletStore::open(&temporary.path().join("wallets"), None).unwrap();
    for name in ["", ".", "..", "../escape", "a/b", "A", "-a", "a-", "a_b"] {
        assert!(store.create(name, &network()).is_err());
        assert!(store.show(name).is_err());
    }
    assert!(store.list().unwrap().is_empty());
    let wide = temporary.path().join("wide");
    fs::create_dir(&wide).unwrap();
    fs::set_permissions(&wide, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(WalletStore::open(&wide, None).is_err());
    let link = temporary.path().join("link");
    symlink(store.root(), &link).unwrap();
    assert!(WalletStore::open(&link, None).is_err());
    assert!(WalletStore::open(&link.join("nested"), None).is_err());
}

#[test]
fn named_wallet_and_config_substitution_fail_closed() {
    let (temporary, store) = fixture();
    let info = store.create("developer", &network()).unwrap();
    let original = fs::read(&info.config_path).unwrap();
    write_private(&info.config_path, b"network_id = \"substituted\"\n");
    assert!(store.show("developer").is_err());
    assert!(store.load_config("developer").is_err());
    write_private(&info.config_path, &original);
    let alias = temporary.path().join("config-hardlink");
    fs::hard_link(&info.config_path, &alias).unwrap();
    assert!(store.show("developer").is_err());
    fs::remove_file(&alias).unwrap();
    let wallet_root = info.config_path.parent().unwrap();
    fs::rename(wallet_root, temporary.path().join("moved-wallet")).unwrap();
    symlink(temporary.path().join("moved-wallet"), wallet_root).unwrap();
    assert!(store.show("developer").is_err());
    assert!(store.load_config("developer").is_err());
}

#[test]
fn operation_paths_are_private_absent_unique_and_do_not_accept_symlinks() {
    let (temporary, store) = fixture();
    let info = store.create("developer", &network()).unwrap();
    let first = store.operation_path("developer", "transfer").unwrap();
    let second = store.operation_path("developer", "transfer").unwrap();
    assert_ne!(first, second);
    assert!(!first.exists() && !second.exists());
    assert_eq!(
        first.parent().unwrap().metadata().unwrap().mode() & 0o777,
        0o700
    );
    assert!(store.operation_path("developer", "../outside").is_err());
    let operations = info.config_path.parent().unwrap().join("operations");
    fs::remove_dir(&operations).unwrap();
    symlink(temporary.path(), &operations).unwrap();
    assert!(store.operation_path("developer", "transfer").is_err());
}

#[test]
fn public_network_context_rejects_credentials_and_mismatched_taira_binding() {
    for url in [
        "http://example.com/",
        "https://name:secret@example.com/",
        "https://example.com/?token=secret",
        "https://example.com/#secret",
        "file:///tmp/torii",
    ] {
        let mut value = network();
        value.torii_url = url.to_owned();
        assert!(value.validate().is_err());
    }
    for url in [
        "https://example.com/",
        "http://127.0.0.1:8080/",
        "http://[::1]:8080/",
    ] {
        let mut value = network();
        value.torii_url = url.to_owned();
        assert!(value.validate().is_ok());
    }
    let mut value = network();
    value.chain_discriminant = 753;
    assert!(value.validate().is_err());
    value = network();
    value.chain_id = "another-chain".to_owned();
    assert!(value.validate().is_err());
    value.chain_discriminant = 42;
    assert!(value.validate().is_ok());
    value.chain_discriminant = 0;
    assert!(value.validate().is_err());
    assert!(data_root(None, &["wallets"]).is_err());
    assert!(data_root(Some(PathBuf::from("relative")), &["wallets"]).is_err());
    assert_eq!(
        data_root(Some(PathBuf::from("/public-data")), &["iroha", "wallets"]).unwrap(),
        Path::new("/public-data/iroha/wallets")
    );
}

#[test]
fn bounded_reads_and_retained_directory_identity_reject_malformed_files() {
    let (temporary, store) = fixture();
    let key_path = temporary.path().join("large.key");
    write_private(&key_path, &vec![b'x'; MAX_KEY_BYTES + 1]);
    assert!(
        store
            .import_key_file("large", &network(), &key_path)
            .is_err()
    );
    fs::remove_file(&key_path).unwrap();
    let _socket = std::os::unix::net::UnixListener::bind(&key_path).unwrap();
    assert!(
        store
            .import_key_file("socket", &network(), &key_path)
            .is_err()
    );
    let pending = store.directory.child(".pending-test", true).unwrap();
    assert!(pending.write_new("../escape", b"no").is_err());
    pending.write_new("wallet.json", b"one").unwrap();
    assert!(pending.write_new("wallet.json", b"two").is_err());
    assert!(pending.read("wallet.json", 2).is_err());
    assert_eq!(pending.read("wallet.json", 3).unwrap().as_slice(), b"one");
    store.directory.remove_pending(&pending).unwrap();
    fs::rename(store.root(), temporary.path().join("moved-store")).unwrap();
    fs::create_dir(store.root()).unwrap();
    fs::set_permissions(store.root(), fs::Permissions::from_mode(0o700)).unwrap();
    assert!(store.list().is_err());
    assert!(store.create("replaced", &network()).is_err());
    assert!(!store.root().join("replaced").exists());
}
