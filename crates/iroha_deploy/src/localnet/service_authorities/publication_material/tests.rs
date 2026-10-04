//! Genuine generated-profile intent and explicit lower-owner provisioning controls.
//! No test here claims native pin finality, provider completion or an installed listener.
use super::*;
use iroha_fs::{PrivateDirectory, PublishMode};

fn prepared(root: &Path, profile: LocalnetServiceProfile) -> PreparedLocalnet {
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    prepare_localnet_at("publication-profile", root, &ports, profile, None).unwrap()
}

#[test]
fn generated_publication_retains_exact_owner_pin_session_tls_port_and_disabled_originals() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let prepared = prepared(
        &temporary.path().join("generation"),
        LocalnetServiceProfile::StreamTokenAuthorities,
    );
    let manifest = prepared.stream_token_authorities().unwrap().unwrap();
    let original = prepared.publication_service_plan().unwrap().unwrap();
    let provider = &manifest.providers[usize::from(SELECTED_SLOT)];
    assert_eq!(prepared.peers.len(), 4);
    assert_eq!(manifest.providers.len(), 3);
    assert_eq!(manifest.network.authorities.len(), 3);
    assert_eq!(original.network_id(), manifest.network_id);
    assert_eq!(original.peer_index(), 0);
    assert_eq!(original.seed_provider(), provider.provider_id);
    assert_eq!(
        original.ingress_broker(),
        &provider
            .authority(StreamTokenAuthorityRole::IssuerOperator)
            .unwrap()
            .account
    );
    assert_eq!(
        original.pin_authority(),
        &manifest
            .network
            .authority(NetworkServiceAuthorityRole::MusubiPin)
            .unwrap()
            .account
    );
    assert_ne!(original.pin_authority(), original.ingress_broker());
    assert_ne!(original.pin_authority(), &manifest.manager);
    assert_ne!(original.session_id(), [0; 32]);
    let config = original.installation_config();
    let selected = config.installation.as_ref().unwrap();
    assert_eq!(selected.pin_session, original.session_id());
    assert_eq!(selected.seed_provider, provider.provider_id);
    assert_eq!(selected.ingress_broker, *original.ingress_broker());
    assert_eq!(
        config
            .paid_pin_policy(original.ingress_broker())
            .transaction_authority,
        *original.pin_authority()
    );
    assert_eq!(selected.readback_request_timeout_ms, 30_000);
    assert_eq!(
        original.configuration_table().unwrap()["installation"]["readback_request_timeout_ms"]
            .as_integer(),
        Some(30_000),
    );
    assert_eq!(selected.pin_authorization_window_ms, 600_000);
    assert_eq!(selected.pin_max_check_rounds, 16);
    assert_eq!(selected.pin_per_transaction_fee_limit, Quantity::from(1u64));
    assert_eq!(selected.pin_total_fee_limit, Quantity::from(64u64));
    assert_eq!(config.private_tls_bind.ip(), Ipv4Addr::LOCALHOST);
    assert_eq!(config.private_tls_bind.port(), original.https_port());
    assert_ne!(original.https_port(), 0);
    for plan in prepared.provider_service_plans().unwrap().unwrap() {
        let url = url::Url::parse(plan.https_origin()).unwrap();
        assert_ne!(url.port_or_known_default(), Some(original.https_port()));
    }
    for peer in &prepared.peers {
        let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024).unwrap();
        let parsed = parse_localnet_peer_config(
            std::str::from_utf8(&bytes).unwrap(),
            Some(&peer.config_path),
        )
        .unwrap();
        assert!(parsed.musubi_publication.installation.is_none());
    }
    let again = prepared.publication_service_plan().unwrap().unwrap();
    assert_eq!(again.installation_config(), config);
    assert_eq!(again.https_origin(), original.https_origin());
    let transport = original.publication_transport().unwrap();
    assert_eq!(transport.network_id(), original.network_id());
    assert_eq!(transport.chain_id(), original.chain_id());
    assert_eq!(transport.provider_id(), original.seed_provider());
    assert_eq!(
        transport.base_url().as_str(),
        format!("{}/", original.https_origin())
    );
    assert!(
        !original
            .configuration_table()
            .unwrap()
            .contains_key("private_mount_prefix")
    );
    let peer = &prepared.peers[0].config_path;
    let original_bytes = iroha_fs::read_private(peer, 1024 * 1024).unwrap();
    let mut table = crate::secret_toml::parse_table(
        std::str::from_utf8(&original_bytes).unwrap(),
        "publication intent roundtrip",
    )
    .unwrap();
    {
        let _foreign_address_profile = ChainDiscriminantGuard::enter(369);
        table.insert(
            "musubi_publication".into(),
            toml::Value::Table(original.configuration_table().unwrap()),
        );
        assert_eq!(
            iroha_data_model::account::address::chain_discriminant(),
            369
        );
    }
    let parsed = parse_localnet_peer_config(&toml::to_string(&table).unwrap(), Some(peer)).unwrap();
    assert_eq!(parsed.musubi_publication, config);
    let publication = table
        .get_mut("musubi_publication")
        .unwrap()
        .as_table_mut()
        .unwrap();
    publication.insert(
        "private_tls_bind".into(),
        toml::Value::String("127.0.0.1:0".into()),
    );
    assert!(parse_localnet_peer_config(&toml::to_string(&table).unwrap(), Some(peer)).is_err());
    assert_eq!(
        iroha_fs::read_private(peer, 1024 * 1024)
            .unwrap()
            .as_slice(),
        original_bytes.as_slice()
    );
    assert!(
        selected
            .broker_key_file
            .ends_with("providers/0/issuer-operator.key")
    );
    assert!(selected.pin_key_file.ends_with("network/musubi-pin.key"));
    assert!(
        selected
            .tls_private_key_file
            .ends_with("providers/0/provider-tls.key.der")
    );
}

#[test]
fn publication_plan_mutation_cannot_replace_signed_original_or_select_another_provider() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let generation = temporary.path().join("generation");
    let prepared = prepared(&generation, LocalnetServiceProfile::StreamTokenAuthorities);
    let root = prepared.context.client_config.parent().unwrap();
    let directory =
        PrivateDirectory::open_exact(root.join(LOCALNET_RUNTIME_DIRECTORY).join(DIRECTORY))
            .unwrap();
    let original = directory.read(MANIFEST, MAX_MANIFEST).unwrap();
    let manifest: StreamTokenAuthorityManifest = norito::json::from_slice(&original).unwrap();
    let retained = prepared.publication_service_plan().unwrap().unwrap();
    for case in 0..9 {
        let mut changed = manifest.clone();
        let mut plan = decode(&changed.network.publication_plan).unwrap();
        match case {
            0 => plan.session_id[0] ^= 1,
            1 => plan.https_port = if plan.https_port == 8443 { 8444 } else { 8443 },
            2 => plan.slot = 1,
            3 => plan.seed_provider = manifest.providers[1].provider_id,
            4 => plan.ingress_broker = manifest.manager.clone(),
            5 => plan.pin_authority = plan.ingress_broker.clone(),
            6 => plan.pin_total_fee_limit = Quantity::from(65u64),
            7 => plan.pin_authorization_window_ms += 1,
            8 => plan.readback_request_timeout_ms += 1,
            _ => unreachable!(),
        }
        changed.network.publication_plan = encode(&plan).unwrap();
        directory
            .write_atomic(
                MANIFEST,
                &norito::json::to_vec(&changed).unwrap(),
                PublishMode::Replace,
            )
            .unwrap();
        assert!(prepared.publication_service_plan().is_err(), "case {case}");
        directory
            .write_atomic(MANIFEST, &original, PublishMode::Replace)
            .unwrap();
        let restored = prepared.publication_service_plan().unwrap().unwrap();
        assert_eq!(restored.session_id(), retained.session_id());
        assert_eq!(
            restored.installation_config(),
            retained.installation_config()
        );
    }
}

#[test]
fn fresh_generation_initializes_all_four_owners_and_missing_seed_marker_is_not_repaired() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let prepared = prepared(
        &temporary.path().join("generation"),
        LocalnetServiceProfile::StreamTokenAuthorities,
    );
    let retained = prepared.publication_service_plan().unwrap().unwrap();
    let config = retained.installation_config();
    let root = PrivateDirectory::open_exact(&config.custody_root).unwrap();
    assert_eq!(
        root.entries(4)
            .unwrap()
            .into_iter()
            .collect::<BTreeSet<_>>(),
        ["journal", "seed", "clock", "pin"]
            .map(std::ffi::OsString::from)
            .into_iter()
            .collect()
    );
    // Each lower owner is created exclusively through private directory custody;
    // reopening it must not recreate it or accept a replacement of the original.
    for name in ["journal", "seed", "clock", "pin"] {
        let child = root.open_child(name).unwrap();
        child.revalidate().unwrap();
        assert!(
            root.create_child(name).is_err(),
            "original {name} already exists"
        );
        child.revalidate().unwrap();
    }
    let service = MusubiPublicationServiceConfigurationV1 {
        network_id: retained.network_id(),
        ingress_broker: retained.ingress_broker().clone(),
        seed_provider: retained.seed_provider(),
        max_future_clock_skew_ms: config.max_future_clock_skew_ms,
        receipt_lifetime_ms: config.receipt_lifetime_ms,
    };
    let binding = MusubiPublicationServiceJournalBindingV1::from_configuration(&service);
    let journal = root.open_child("journal").unwrap();
    let before = journal.entries(8).unwrap();
    let reopened = DurableMusubiPublicationServiceJournalV1::open(
        journal.path(),
        binding.clone(),
        retained.plan.journal_limits,
    )
    .unwrap();
    assert_eq!(reopened.revision(), 1);
    drop(reopened);
    assert!(
        DurableMusubiPublicationServiceJournalV1::initialize(
            journal.path(),
            binding,
            retained.plan.journal_limits
        )
        .is_err()
    );
    assert_eq!(journal.entries(8).unwrap(), before);
    drop(
        DurableMusubiPublicationServiceClockV1::open_system(
            root.open_child("clock").unwrap().path(),
        )
        .unwrap(),
    );
    let seed = root.open_child("seed").unwrap();
    drop(
        MusubiSeedStagingBackendV1::open(
            seed.path(),
            retained.seed_provider(),
            config.max_seed_records,
            config.max_seed_bytes,
        )
        .unwrap(),
    );
    let pin = root.open_child("pin").unwrap();
    let pin_before = pin.entries(8).unwrap();
    assert!(
        NativeMusubiPinSessionV1::new(
            retained.network_id(),
            retained.pin_authority().clone(),
            retained.session_id(),
            retained.plan.pin_storage_class,
            retained.plan.pin_retention_horizon_secs
        )
        .unwrap()
        .initialize_private_journal(pin.path())
        .is_err()
    );
    assert_eq!(pin.entries(8).unwrap(), pin_before);
    std::fs::remove_file(seed.path().join("seed-owner.lock")).unwrap();
    let missing = seed.entries(4).unwrap();
    assert!(
        MusubiSeedStagingBackendV1::open(
            seed.path(),
            retained.seed_provider(),
            config.max_seed_records,
            config.max_seed_bytes
        )
        .is_err()
    );
    assert_eq!(seed.entries(4).unwrap(), missing);
    // Material selection does not silently take/recreate the live owners' history.
    assert_eq!(
        prepared
            .publication_service_plan()
            .unwrap()
            .unwrap()
            .session_id(),
        retained.session_id()
    );
    assert_eq!(seed.entries(4).unwrap(), missing);
}

#[test]
fn standard_profile_has_no_publication_role_plan_or_initialized_custody() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let generation = temporary.path().join("generation");
    let prepared = prepared(&generation, LocalnetServiceProfile::Standard);
    assert!(prepared.stream_token_authorities().unwrap().is_none());
    assert!(prepared.publication_service_plan().unwrap().is_none());
    assert!(!generation.join("state/peer0/musubi-publication").exists());
    for peer in &prepared.peers {
        let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024).unwrap();
        assert!(
            parse_localnet_peer_config(
                std::str::from_utf8(&bytes).unwrap(),
                Some(&peer.config_path)
            )
            .unwrap()
            .musubi_publication
            .installation
            .is_none()
        );
    }
}
