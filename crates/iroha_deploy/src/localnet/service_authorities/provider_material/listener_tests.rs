//! Exact retained listener rendering; genuine profile generation starts no worker or TLS server.

use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};

fn fixture() -> (tempfile::TempDir, PreparedLocalnet) {
    let temporary = tempfile::tempdir().unwrap();
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = prepare_localnet_at(
        "provider-listener",
        &temporary.path().join("generation"),
        &ports,
        LocalnetServiceProfile::StreamTokenAuthorities,
        None,
    )
    .unwrap();
    (temporary, prepared)
}

#[test]
fn peer_https_table_uses_receiver_accepted_canonical_address() {
    let address = SocketAddr::from(([127, 0, 0, 1], 3030));
    let selected = PeerHttps {
        address: address.clone(),
        certificate: PathBuf::from("identity/leaf.der"),
        private_key: PathBuf::from("identity/key.der"),
        timeout_ms: 10_000,
    };
    let table = selected.table().unwrap();
    let literal = table["address"].as_str().unwrap();
    let decoded: SocketAddr =
        norito::json::from_value(norito::json::Value::String(literal.into())).unwrap();
    assert_eq!(decoded, address);
    assert!(
        norito::json::from_value::<SocketAddr>(norito::json::Value::String(
            "127.0.0.1:3030".into(),
        ))
        .is_err()
    );
    assert_eq!(
        table["certificate_chain"][0].as_str(),
        Some("identity/leaf.der")
    );
    assert_eq!(table["private_key"].as_str(), Some("identity/key.der"));
    assert_eq!(table["handshake_timeout_ms"].as_integer(), Some(10_000));
}

#[test]
fn three_original_provider_peers_render_distinct_exact_https_identities() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let plans = prepared.provider_service_plans().unwrap().unwrap();
    let manifest = prepared.stream_token_authorities().unwrap().unwrap();
    let root = prepared.context.client_config.parent().unwrap();
    let mut ports = BTreeSet::new();
    for (index, peer) in prepared.peers.iter().enumerate() {
        let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024).unwrap();
        let config = parse_localnet_peer_config(
            std::str::from_utf8(&bytes).unwrap(),
            Some(&peer.config_path),
        )
        .unwrap();
        validate_peer_https(
            &manifest,
            root,
            index,
            config.torii.transport.https.as_ref(),
        )
        .unwrap();
        if index < PROVIDER_COUNT {
            let plan = &plans[index];
            assert_eq!(plan.peer_index(), index);
            let selected = policy(&plan.admission_material().proposal.capabilities).unwrap();
            assert!(ports.insert(selected.https_port));
            let https = config.torii.transport.https.as_ref().unwrap();
            assert_eq!(
                https.address.value().to_string(),
                format!("127.0.0.1:{}", selected.https_port)
            );
            assert_eq!(
                https.certificate_chain,
                [root
                    .join(LOCALNET_RUNTIME_DIRECTORY)
                    .join(DIRECTORY)
                    .join(PROVIDERS_DIRECTORY)
                    .join(index.to_string())
                    .join(tls_identity::LEAF_CERT)]
            );
            assert_eq!(
                https.private_key,
                root.join(LOCALNET_RUNTIME_DIRECTORY)
                    .join(DIRECTORY)
                    .join(PROVIDERS_DIRECTORY)
                    .join(index.to_string())
                    .join(tls_identity::LEAF_KEY)
            );
        } else {
            assert!(config.torii.transport.https.is_none());
        }
        assert!(!config.torii.sorafs_storage.stream_tokens.enabled);
        assert!(config.torii.sorafs_storage.stream_tokens.signer.is_none());
        assert!(
            config
                .torii
                .sorafs_storage
                .stream_tokens
                .admission_native
                .is_none()
        );
    }
    assert!(
        prepared
            .context
            .load_client_config()
            .unwrap()
            .torii_api_url
            .as_str()
            .starts_with("http://127.0.0.1:")
    );
    // Preparation released its port reservation but never opened a provider listener.
    let _bounds = ports
        .into_iter()
        .map(|port| TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, port)).unwrap())
        .collect::<Vec<_>>();
}

#[test]
fn retained_profile_refuses_missing_moved_or_retargeted_https_without_reissuing_material() {
    let _resources = crate::managed::native_test_guard();
    let (_temporary, prepared) = fixture();
    let root = prepared.context.client_config.parent().unwrap();
    let directory = PrivateDirectory::open_exact(root).unwrap();
    let manifest = prepared.stream_token_authorities().unwrap().unwrap();
    let original = directory.read("peer0.toml", 1024 * 1024).unwrap();
    let base = crate::secret_toml::Table::new(
        crate::secret_toml::parse_table(
            std::str::from_utf8(&original).unwrap(),
            "original listener fixture",
        )
        .unwrap(),
    );
    for which in 0..5 {
        let mut table = crate::secret_toml::Table::new((*base).clone());
        let transport = table["torii"]["transport"].as_table_mut().unwrap();
        if which == 0 {
            transport.remove("https");
        } else {
            let https = transport["https"].as_table_mut().unwrap();
            match which {
                1 => {
                    https.insert(
                        "address".into(),
                        toml::Value::String(SocketAddr::from(([127, 0, 0, 1], 1)).to_literal()),
                    );
                }
                2 => {
                    https.insert(
                        "private_key".into(),
                        toml::Value::String(
                            root.join(LOCALNET_RUNTIME_DIRECTORY)
                                .join(DIRECTORY)
                                .join(tls_identity::CA_KEY)
                                .to_str()
                                .unwrap()
                                .into(),
                        ),
                    );
                }
                3 => {
                    https.insert(
                        "certificate_chain".into(),
                        toml::Value::Array(vec![toml::Value::String(
                            root.join(LOCALNET_RUNTIME_DIRECTORY)
                                .join(DIRECTORY)
                                .join(tls_identity::CA_CERT)
                                .to_str()
                                .unwrap()
                                .into(),
                        )]),
                    );
                }
                _ => {
                    https.insert("handshake_timeout_ms".into(), toml::Value::Integer(1));
                }
            }
        }
        let changed = Zeroizing::new(toml::to_string(&*table).unwrap());
        parse_localnet_peer_config(&changed, Some(&prepared.peers[0].config_path)).unwrap();
        directory
            .write_atomic("peer0.toml", changed.as_bytes(), PublishMode::Replace)
            .unwrap();
        assert!(
            prepared
                .provider_service_plans()
                .map(|plans| plans.map(|[first, _, _]| first))
                .is_err()
        );
    }
    directory
        .write_atomic("peer0.toml", &original, PublishMode::Replace)
        .unwrap();
    let peer1 = directory.read("peer1.toml", 1024 * 1024).unwrap();
    let mut changed = crate::secret_toml::Table::new(
        crate::secret_toml::parse_table(
            std::str::from_utf8(&peer1).unwrap(),
            "second peer fixture",
        )
        .unwrap(),
    );
    changed["torii"]["transport"]
        .as_table_mut()
        .unwrap()
        .insert("https".into(), base["torii"]["transport"]["https"].clone());
    let rendered = Zeroizing::new(toml::to_string(&*changed).unwrap());
    directory
        .write_atomic("peer1.toml", rendered.as_bytes(), PublishMode::Replace)
        .unwrap();
    assert!(
        prepared
            .provider_service_plans()
            .map(|plans| plans.map(|[first, _, _]| first))
            .is_err()
    );
    directory
        .write_atomic("peer1.toml", &peer1, PublishMode::Replace)
        .unwrap();
    assert_eq!(
        prepared.stream_token_authorities().unwrap().unwrap(),
        manifest
    );
    assert!(
        prepared
            .provider_service_plans()
            .map(|plans| plans.map(|[first, _, _]| first))
            .unwrap()
            .is_some()
    );
}
