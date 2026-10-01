//! Portable wallet custody tests shared by native Windows, macOS and Linux runners.

use super::*;

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

#[test]
fn native_wallet_publication_and_reopen_preserve_one_owner() {
    let temporary = tempfile::tempdir().unwrap();
    let store = WalletStore::open(&temporary.path().join("wallets"), None).unwrap();
    let info = store.create("owner", &network()).unwrap();
    let config = store.load_config("owner").unwrap();
    let secret = ExposedPrivateKey(config.key_pair.private_key().clone()).to_string();
    assert_eq!(info.public_key, config.key_pair.public_key().to_string());
    assert!(!json::to_json(&info).unwrap().contains(&secret));
    assert!(store.create("owner", &network()).is_err());
    assert_eq!(
        store.load_config("owner").unwrap().key_pair.public_key(),
        config.key_pair.public_key()
    );
    assert!(
        store
            .directory
            .entries(16)
            .unwrap()
            .iter()
            .all(|name| !name.to_string_lossy().starts_with(".pending-"))
    );
    let path = store.operation_path("owner", "admission").unwrap();
    let journal = crate::operation_journal::Journal::create(&path).unwrap();
    let operation = norito::json!({"wire": "original"});
    journal.write_operation(&operation).unwrap();
    assert!(journal.record_submission(&operation).unwrap());
    drop(journal);
    let journal = crate::operation_journal::Journal::open(&path).unwrap();
    assert!(!journal.record_submission(&operation).unwrap());
    assert_eq!(store.show("owner").unwrap().public_key, info.public_key);
}

#[test]
fn pending_cleanup_refuses_unknown_files_and_published_wallets() {
    let temporary = tempfile::tempdir().unwrap();
    let store = WalletStore::open(&temporary.path().join("wallets"), None).unwrap();
    let pending = store.directory.create_child(".pending-test").unwrap();
    pending
        .write_atomic("unknown", b"preserve", PublishMode::CreateNew)
        .unwrap();
    assert!(remove_pending(&store.directory, pending).is_err());
    let pending = store.directory.open_child(".pending-test").unwrap();
    assert_eq!(&*pending.read("unknown", 64).unwrap(), b"preserve");
    let published = store.directory.create_child("published").unwrap();
    assert!(remove_pending(&store.directory, published).is_err());
    assert!(store.directory.open_child("published").is_ok());
}

#[test]
fn typed_owner_import_binds_the_parent_without_forwarding_child_credentials() {
    let temporary = tempfile::tempdir().unwrap();
    let store = WalletStore::open(&temporary.path().join("wallets"), None).unwrap();
    let mut child_network = network();
    child_network.chain_id = "private-root".into();
    child_network.chain_discriminant = 42;
    child_network.torii_url = "http://127.0.0.1:18081/".into();
    child_network.network_id =
        NetworkId::from_genesis_hash(iroha_crypto::HashOf::from_untyped_unchecked(
            iroha_crypto::Hash::new(b"private wallet root"),
        ));
    store.create("child", &child_network).unwrap();
    let mut child = store.load_config("child").unwrap();
    child.api_token = Some(iroha::secrecy::SecretString::new(
        "child-listener-secret".into(),
    ));
    let parent = network();
    let imported = store
        .import_key_pair("parent", &parent, &child.key_pair)
        .unwrap();
    let retained = store.load_config("parent").unwrap();
    assert_eq!(retained.account, child.account);
    assert_eq!(retained.key_pair, child.key_pair);
    assert_eq!(retained.network_id, parent.network_id);
    assert_eq!(retained.torii_api_url.as_str(), parent.torii_url);
    assert_ne!(retained.network_id, child.network_id);
    assert!(retained.api_token.is_none());
    assert!(retained.basic_auth.is_none());
    let public = json::to_json(&imported).unwrap();
    assert!(!public.contains("child-listener-secret"));
    assert!(!public.contains(&ExposedPrivateKey(child.key_pair.private_key().clone()).to_string()));
    let other_key = KeyPair::try_random().unwrap();
    assert!(
        store
            .import_key_pair("parent", &parent, &other_key)
            .is_err()
    );
    assert_eq!(
        store.load_config("parent").unwrap().key_pair,
        child.key_pair
    );
}

#[cfg(windows)]
#[test]
fn windows_wallet_default_uses_the_native_application_data_convention() {
    let base = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
    match base.filter(|path| path.is_absolute()) {
        Some(base) => assert_eq!(
            default_wallet_dir().unwrap(),
            base.join("Iroha").join("wallets")
        ),
        None => assert!(default_wallet_dir().is_err()),
    }
}
