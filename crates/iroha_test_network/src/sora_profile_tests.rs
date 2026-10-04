// Sora profile and signed-consensus tests share valid, role-separated software custody.

fn sora_storage_profile_fixture() -> (tempfile::TempDir, Table) {
    let credentials = tempdir().expect("temporary SoraFS signer credentials");
    #[cfg(unix)]
    fs::set_permissions(credentials.path(), fs::Permissions::from_mode(0o700))
        .expect("owner-only signer directory");
    let root = fs::canonicalize(credentials.path()).expect("absolute credential directory");
    let mut layer = Table::new().write(["sorafs", "storage", "enabled"], true);
    // Storage also drains durable operations for every role, even when the
    // independent repair, reserve and orderbook producers are disabled.
    for (role, seed) in ["proof_outcome", "repair", "reserve", "orderbook"]
        .into_iter()
        .zip([0x41_u8, 0x42, 0x43, 0x44])
    {
        let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
            .expect("distinct deterministic software signer");
        let credential = root.join(role);
        let mut options = fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            options.mode(0o600);
        }
        let mut file = options
            .open(&credential)
            .expect("private signer credential");
        let encoded = zeroize::Zeroizing::new(format!(
            "{}\n",
            ExposedPrivateKey(key.private_key().clone())
                .try_to_multihash_string()
                .expect("canonical software private key")
        ));
        file.write_all(encoded.as_bytes())
            .expect("write signer credential");
        let (_, public_bytes) = key
            .public_key()
            .try_to_bytes()
            .expect("raw signer public key");
        let binding = Table::new()
            .write(
                "software_credential",
                credential.to_str().expect("credential path"),
            )
            .write(
                "handle",
                format!("software://sorafs/{}/primary", role.replace('_', "-")),
            )
            .write(
                "authority",
                AccountId::new(key.public_key().clone())
                    .to_i105_for_discriminant(defaults::common::chain_discriminant())
                    .expect("canonical signer authority"),
            )
            .write("algorithm", "ed25519")
            .write("public_key_hex", hex_lower(&public_bytes))
            .write("revision", 1_i64)
            .write("policy_digest_hex", hex_lower(&[seed; 32]));
        layer = layer.write(
            ["sorafs", "storage", "native_transaction_signers", role],
            binding,
        );
    }
    (credentials, layer)
}

#[test]
fn resolve_actual_config_applies_sora_profile_non_consensus_settings() {
    let (_credentials, storage_layer) = sora_storage_profile_fixture();
    let config_layers = vec![storage_layer];
    assert!(
        config_requires_sora_profile(&config_layers),
        "SoraFS-enabled configs should trigger --sora profile detection"
    );
    let mut merged = sora_profile_detection_defaults();
    for layer in &config_layers {
        merge_tables(&mut merged, layer);
    }
    apply_identity_defaults_for_detection(&mut merged);
    ensure_sora_profile_trusted_peer_pop(&mut merged);
    let actual = parse_actual_config_for_genesis(merged, &config_layers)
        .expect("should resolve runtime-equivalent config");
    assert!(actual.torii.sorafs_storage.enabled);
    assert!(
        actual.nexus.lane_config.entries().len() > 1,
        "Sora profile should expand lane catalog beyond single-lane defaults"
    );
}
#[test]
fn sora_profile_does_not_override_signed_genesis_mode() {
    init_instruction_registry();
    let (_credentials, storage_layer) = sora_storage_profile_fixture();
    let network = build_with_isolated_permit(
        NetworkBuilder::new()
            .with_peers(4)
            .with_permissioned_consensus()
            .with_config_table(storage_layer),
    );
    let layers = network
        .config_layers()
        .map(Cow::into_owned)
        .collect::<Vec<_>>();
    let actual = resolve_actual_config(&network.peers()[0], &layers)
        .expect("storage-enabled profile must remain a valid runtime configuration");
    assert!(actual.torii.sorafs_storage.enabled);
    assert!(actual.nexus.lane_config.entries().len() > 1);
    assert_eq!(
        network.consensus_bootstrap_profile().mode_tag,
        PERMISSIONED_TAG,
        "local Sora profile selection must not override the signed genesis mode",
    );
}

#[test]
fn genesis_projection_preserves_explicit_default_sora_topology_and_execution_policy() {
    let (_credentials, storage_layer) = sora_storage_profile_fixture();
    let topology_layer = toml::toml! {
        [nexus]
        lane_count = 1
        lane_catalog = []
        dataspace_catalog = []
        [nexus.routing_policy]
    };
    let config_layers = vec![storage_layer, topology_layer];
    assert!(config_requires_sora_profile(&config_layers));
    let mut merged = sora_profile_detection_defaults();
    for layer in &config_layers {
        merge_tables(&mut merged, layer);
    }
    apply_identity_defaults_for_detection(&mut merged);
    ensure_sora_profile_trusted_peer_pop(&mut merged);
    let before = ConfigReader::new()
        .with_env(MockEnv::default())
        .with_toml_source(TomlSource::inline(merged.clone()))
        .read_and_complete::<iroha_config::parameters::user::Root>()
        .expect("read explicit default geometry")
        .parse()
        .expect("parse explicit default geometry");
    assert!(!before.nexus.has_lane_overrides());
    let projected = parse_actual_config_for_genesis_result(merged, &config_layers)
        .expect("source-selected genesis projection");
    assert_eq!(projected.nexus.lane_catalog, before.nexus.lane_catalog);
    assert_eq!(
        projected.nexus.configured_lane_catalog,
        before.nexus.configured_lane_catalog
    );
    assert_eq!(projected.nexus.lane_config, before.nexus.lane_config);
    assert_eq!(
        projected.nexus.dataspace_catalog,
        before.nexus.dataspace_catalog
    );
    assert_eq!(
        projected.nexus.configured_dataspace_catalog,
        before.nexus.configured_dataspace_catalog
    );
    assert_eq!(projected.nexus.routing_policy, before.nexus.routing_policy);
    let execution_policy = |config: &iroha_config::parameters::actual::Root| {
        let manifests = iroha_core::governance::manifest::LaneManifestRegistry::from_config(
            &config.nexus.lane_catalog,
            &config.nexus.governance,
            &config.nexus.registry,
        );
        let nexus =
            iroha_config::parameters::actual::nexus_consensus_policy_digest_with_runtime_policies(
                &config.nexus,
                None,
                Some(manifests.baseline_consensus_policy_digest()),
            )
            .expect("fixture Nexus policy");
        iroha_config::parameters::actual::execution_policy_digest_v1(
            &config.pipeline,
            &config.oracle,
            &config.crypto,
            &config.fraud_monitoring,
            &config.gov,
            &config.content,
            &config.settlement,
            nexus,
            iroha_core::state::compute_zk_consensus_policy_hash(&config.zk),
        )
    };
    assert_eq!(execution_policy(&projected), execution_policy(&before));
    assert!(projected.torii.sorafs_storage.enabled);
}
