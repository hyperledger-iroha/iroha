//! Roster validation, output custody and policy completeness of [`generate_network`].
use super::*;
use iroha_genesis::read_signed_genesis;

fn bls_identity() -> (KeyPair, String) {
    let key_pair =
        KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::BlsNormal).expect("BLS key");
    let pop = iroha_crypto::bls_normal_pop_prove(key_pair.private_key()).expect("BLS PoP");
    (key_pair, hex::encode(pop))
}

fn validator_at(address: &str) -> NetworkValidator {
    let (key_pair, pop) = bls_identity();
    format!("{}@{address}={pop}", key_pair.public_key())
        .parse()
        .expect("valid roster entry")
}

fn roster(count: u16) -> Vec<NetworkValidator> {
    (0..count)
        .map(|index| validator_at(&format!("127.0.0.1:{}", 1337 + index)))
        .collect()
}

/// A fresh output path inside an owner-private parent, as operators use one.
fn output_path(parent: &tempfile::TempDir) -> PathBuf {
    iroha_fs::PrivateDirectory::open_or_create(parent.path().join("private"))
        .expect("private parent")
        .path()
        .join("network")
}

fn create(
    consensus_mode: SumeragiConsensusMode,
    validators: Vec<NetworkValidator>,
) -> (tempfile::TempDir, PathBuf, String) {
    let parent = tempfile::tempdir().expect("temporary parent");
    let out_dir = output_path(&parent);
    let mut writer = BufWriter::new(Vec::new());
    generate_network(
        &NetworkOptions {
            validators,
            chain_id: Some("network-test".to_owned()),
            chain_discriminant: None,
            consensus_mode,
            torii_url: None,
            out_dir: out_dir.clone(),
        },
        &mut writer,
    )
    .expect("create network");
    let stdout = String::from_utf8(writer.into_inner().expect("flush")).expect("UTF-8 summary");
    (parent, out_dir, stdout)
}

fn read_table(path: &Path) -> toml::Table {
    fs::read_to_string(path)
        .expect("read generated TOML")
        .parse()
        .expect("parse generated TOML")
}

/// A validator configuration built from the published shared file, as nodes read it.
fn published_node_config(out_dir: &Path) -> actual::Root {
    let path = out_dir.join(NETWORK_CONFIG_FILE);
    let shared = read_table(&path);
    let rendered = ReferenceNode::generate()
        .expect("reference node")
        .config(&shared, NetworkGenesisIdentity::Published)
        .expect("render reference node");
    parse_localnet_peer_config(&rendered, Some(&path)).expect("parse published node config")
}

fn restage(out_dir: &Path, config: &actual::Root) -> Result<()> {
    let bound = RawGenesisTransaction::from_path(out_dir.join(GENESIS_MANIFEST_FILE))?;
    let signed = read_signed_genesis(&out_dir.join(SIGNED_GENESIS_FILE))?;
    let (nexus, execution) =
        crate::genesis::staging::staged_signed_sumeragi_context_hashes(&bound, &signed, config)?;
    let signed_context = bound.sumeragi_context_parameters();
    ensure!(nexus == Hash::prehashed(signed_context.nexus_amx_context_hash));
    ensure!(execution == Hash::prehashed(signed_context.execution_policy_hash));
    Ok(())
}

#[test]
fn validator_entries_parse_node_init_output_and_reject_unusable_identities() {
    let (key_pair, pop) = bls_identity();
    let key = key_pair.public_key();
    let entry = format!("{key}@203.0.113.7:1337={pop}");
    let parsed = entry.parse::<NetworkValidator>().expect("node init entry");
    assert_eq!(parsed.public_key(), key);
    assert_eq!(parsed.address().to_string(), "203.0.113.7:1337");
    assert_eq!(parsed.to_string(), entry);
    let prefixed = format!("{key}@203.0.113.7:1337=0x{pop}");
    assert_eq!(
        prefixed.parse::<NetworkValidator>().unwrap().to_string(),
        entry
    );
    for address in ["node-a.example:1337", "[2001:db8::7]:1337"] {
        assert!(
            format!("{key}@{address}={pop}")
                .parse::<NetworkValidator>()
                .is_ok(),
            "{address}"
        );
    }
    assert_eq!(
        format!("{key}@Node-A.Example:1337={pop}")
            .parse::<NetworkValidator>()
            .unwrap()
            .address()
            .to_string(),
        "node-a.example:1337"
    );
    let (_, other_pop) = bls_identity();
    let ed25519 = KeyPair::try_random_with_algorithm(iroha_crypto::Algorithm::Ed25519).unwrap();
    for (rejected, reason) in [
        (
            format!("{key}@203.0.113.7:1337"),
            "PUBLIC_KEY@HOST:PORT=POP_HEX",
        ),
        (format!("{key}={pop}"), "PUBLIC_KEY@HOST:PORT"),
        (
            format!("{}@203.0.113.7:1337={pop}", ed25519.public_key()),
            "bls_normal",
        ),
        (
            format!("{key}@203.0.113.7:1337={other_pop}"),
            "does not verify",
        ),
        (format!("{key}@203.0.113.7:1337=zz"), "not hex"),
        (format!("{key}@203.0.113.7:0={pop}"), "nonzero"),
        (format!("{key}@0.0.0.0:1337={pop}"), "wildcard"),
        (format!("{key}@[::]:1337={pop}"), "wildcard"),
        (format!("{key}@no-port={pop}"), "address"),
        (format!("{key}@:1337={pop}"), "address"),
        (format!("{key}@bad host:1337={pop}"), "address"),
        (format!("{key}@bad\\\"host:1337={pop}"), "address"),
    ] {
        let error = rejected
            .parse::<NetworkValidator>()
            .expect_err("unusable roster entry");
        assert!(error.to_string().contains(reason), "{rejected}: {error}");
    }
}

#[test]
fn network_roster_requires_an_exact_committee_of_distinct_validators() {
    for count in [0, 1, 3, 5, 6, 8] {
        assert!(
            validate_network_roster(&roster(count))
                .expect_err("not a 3f+1 committee")
                .to_string()
                .contains("3f+1"),
            "{count} validators"
        );
    }
    for count in [4, 7] {
        assert_eq!(
            validate_network_roster(&roster(count)).unwrap().get(),
            count
        );
    }
    let mut repeated = roster(4);
    repeated[3] = repeated[0].clone();
    assert!(
        validate_network_roster(&repeated)
            .unwrap_err()
            .to_string()
            .contains("more than once")
    );
    let mut shared_address = roster(4);
    shared_address[3] = validator_at(&shared_address[0].address().to_string());
    assert!(
        validate_network_roster(&shared_address)
            .unwrap_err()
            .to_string()
            .contains("share the P2P address")
    );
}

#[test]
fn network_chain_ids_are_fresh_by_default_and_never_public() {
    let first = resolve_network_chain_id(None).unwrap();
    let second = resolve_network_chain_id(None).unwrap();
    assert_ne!(first, second, "every default network gets its own chain id");
    assert!(first.parse::<ChainId>().is_ok());
    assert_eq!(first.len(), 36);
    assert_eq!(&first[14..15], "4", "UUID version 4: {first}");
    assert_eq!(
        resolve_network_chain_id(Some("acme-net.v1")).unwrap(),
        "acme-net.v1"
    );
    for rejected in [
        PUBLIC_TAIRA_CHAIN_ID,
        PUBLIC_NEXUS_CHAIN_ID,
        "",
        " acme",
        "acme net",
    ] {
        assert!(
            resolve_network_chain_id(Some(rejected)).is_err(),
            "{rejected:?}"
        );
    }
}

#[test]
fn network_client_torii_url_defaults_to_the_node_init_api_address() {
    assert_eq!(
        resolve_network_torii_url(None).unwrap().as_str(),
        DEFAULT_NETWORK_TORII_URL
    );
    assert_eq!(
        resolve_network_torii_url(Some("https://203.0.113.7:8080"))
            .unwrap()
            .as_str(),
        "https://203.0.113.7:8080/"
    );
    for rejected in ["203.0.113.7:8080", "ftp://203.0.113.7/", "not a url"] {
        assert!(
            resolve_network_torii_url(Some(rejected)).is_err(),
            "{rejected}"
        );
    }
}

#[test]
fn invalid_network_requests_create_no_output() {
    let parent = tempfile::tempdir().unwrap();
    let out_dir = output_path(&parent);
    for (validators, torii_url) in [(roster(3), None), (roster(4), Some("ftp://host/"))] {
        let outcome = generate_network(
            &NetworkOptions {
                validators,
                chain_id: None,
                chain_discriminant: None,
                consensus_mode: SumeragiConsensusMode::Permissioned,
                torii_url: torii_url.map(str::to_owned),
                out_dir: out_dir.clone(),
            },
            &mut BufWriter::new(Vec::new()),
        );
        assert!(outcome.is_err());
        assert!(!out_dir.exists(), "a refused request leaves no directory");
    }
    fs::create_dir(&out_dir).unwrap();
    fs::write(out_dir.join("keep"), b"operator data").unwrap();
    assert!(
        generate_network(
            &NetworkOptions {
                validators: roster(4),
                chain_id: None,
                chain_discriminant: None,
                consensus_mode: SumeragiConsensusMode::Permissioned,
                torii_url: None,
                out_dir: out_dir.clone(),
            },
            &mut BufWriter::new(Vec::new()),
        )
        .is_err()
    );
    assert_eq!(fs::read(out_dir.join("keep")).unwrap(), b"operator data");
}

#[test]
fn network_outputs_keep_secrets_owner_only_and_publish_the_bound_roster() {
    let validators = roster(4);
    let (_parent, out_dir, stdout) =
        create(SumeragiConsensusMode::Permissioned, validators.clone());
    assert!(stdout.contains(
        "distribute: network.toml genesis.signed.nrt genesis.expected_hash nexus-amx-context.v1.bin"
    ));
    let listing = |path: &Path| {
        let mut names = fs::read_dir(path)
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect::<Vec<_>>();
        names.sort();
        names
    };
    assert_eq!(
        listing(&out_dir),
        [
            CLIENT_CONFIG_FILE,
            GENESIS_EXPECTED_HASH_FILE,
            GENESIS_MANIFEST_FILE,
            GENESIS_PUBLIC_KEY_FILE,
            SIGNED_GENESIS_FILE,
            NETWORK_CONFIG_FILE,
            NEXUS_CONTEXT_FILE,
            SECRETS_DIRECTORY,
        ]
    );
    let secrets = out_dir.join(SECRETS_DIRECTORY);
    assert_eq!(
        listing(&secrets),
        [GENESIS_PRIVATE_KEY_FILE, OPERATOR_KEY_FILE]
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt as _;
        let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&out_dir), 0o700);
        assert_eq!(mode(&secrets), 0o700);
        for name in [GENESIS_PRIVATE_KEY_FILE, OPERATOR_KEY_FILE] {
            assert_eq!(mode(&secrets.join(name)), 0o600, "{name}");
        }
    }

    // Secrets stay in secrets/: no public artifact carries a private key.
    let genesis_secret = fs::read_to_string(secrets.join(GENESIS_PRIVATE_KEY_FILE)).unwrap();
    let operator_secret = fs::read_to_string(secrets.join(OPERATOR_KEY_FILE)).unwrap();
    let genesis_key = KeyPair::from_private_key(
        genesis_secret
            .trim_end()
            .parse::<iroha_crypto::PrivateKey>()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        fs::read_to_string(out_dir.join(GENESIS_PUBLIC_KEY_FILE)).unwrap(),
        format!("{}\n", genesis_key.public_key())
    );
    for name in listing(&out_dir) {
        let path = out_dir.join(&name);
        if path.is_file() {
            let bytes = fs::read(&path).unwrap();
            for secret in [&genesis_secret, &operator_secret] {
                let secret = secret.trim_end().as_bytes();
                assert!(
                    !bytes.windows(secret.len()).any(|window| window == secret),
                    "{name} must not carry a private key"
                );
            }
        }
    }

    // The shared file names exactly the network-wide values and the bound [nexus] settings.
    let shared = read_table(&out_dir.join(NETWORK_CONFIG_FILE));
    let mut keys = shared.keys().map(String::as_str).collect::<Vec<_>>();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "chain",
            "genesis",
            "nexus",
            "trusted_peers",
            "trusted_peers_pop"
        ]
    );
    assert_eq!(shared["chain"].as_str(), Some("network-test"));
    let trusted = shared["trusted_peers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|peer| peer.as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        trusted,
        validators
            .iter()
            .map(NetworkValidator::trusted_peer)
            .collect::<Vec<_>>()
    );
    for (entry, validator) in shared["trusted_peers_pop"]
        .as_array()
        .unwrap()
        .iter()
        .zip(&validators)
    {
        assert_eq!(
            entry["public_key"].as_str(),
            Some(validator.public_key().to_string().as_str())
        );
        assert_eq!(
            entry["pop_hex"].as_str(),
            Some(validator.pop_hex().as_str())
        );
    }
    let genesis = shared["genesis"].as_table().unwrap();
    assert_eq!(
        genesis["public_key"].as_str(),
        Some(genesis_key.public_key().to_string().as_str())
    );
    assert_eq!(genesis["file"].as_str(), Some(SIGNED_GENESIS_FILE));
    assert_eq!(
        genesis["expected_hash_file"].as_str(),
        Some(GENESIS_EXPECTED_HASH_FILE)
    );
    assert_eq!(
        shared["nexus"]["fusion"]["exit_teu"].as_integer(),
        Some(i64::from(LOCALNET_LANE_TEU_CAPACITY))
    );
    assert!(shared["nexus"].get("dataspace_catalog").is_none());

    // Genesis carries exactly this roster with its PoPs, under the published identity.
    let signed = read_signed_genesis(&out_dir.join(SIGNED_GENESIS_FILE)).unwrap();
    let identity = fs::read_to_string(out_dir.join(GENESIS_EXPECTED_HASH_FILE)).unwrap();
    assert_eq!(
        identity.trim_end().parse::<NetworkId>().unwrap(),
        NetworkId::from_genesis_hash(signed.hash())
    );
    let bound = RawGenesisTransaction::from_path(out_dir.join(GENESIS_MANIFEST_FILE)).unwrap();
    let mut topology = bound
        .transactions()
        .iter()
        .flat_map(|transaction| transaction.topology().iter().cloned())
        .map(|entry| (entry.peer.public_key().clone(), entry.pop_hex))
        .collect::<Vec<_>>();
    topology.sort();
    let mut expected = validators
        .iter()
        .map(|validator| (validator.public_key().clone(), Some(validator.pop_hex())))
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(topology, expected);

    // A node that includes the shared file reproduces every signed commitment ...
    let config = published_node_config(&out_dir);
    assert_eq!(config.genesis.expected_hash, signed.hash());
    restage(&out_dir, &config).expect("shared settings reproduce the signed policy");
    // ... and one that drops the bound [nexus] settings is refused.
    let mut unbound = config;
    unbound.nexus.fusion.exit_teu = actual::Nexus::default().fusion.exit_teu;
    let error = restage(&out_dir, &unbound).expect_err("a node without [nexus] fails");
    assert!(
        matches!(
            error.downcast_ref::<iroha_core::block::BlockValidationError>(),
            Some(iroha_core::block::BlockValidationError::GenesisPolicyMismatch { .. })
        ),
        "{error:#}"
    );

    // The operator client loads as written and signs as the funded operator account.
    let client = iroha::config::Config::load(iroha::config::LoadPath::Explicit(
        out_dir.join(CLIENT_CONFIG_FILE),
    ))
    .expect("load operator client config");
    assert_eq!(client.chain.as_str(), "network-test");
    assert_eq!(client.torii_api_url.as_str(), DEFAULT_NETWORK_TORII_URL);
    let operator = KeyPair::from_private_key(
        operator_secret
            .trim_end()
            .parse::<iroha_crypto::PrivateKey>()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(client.key_pair.public_key(), operator.public_key());
}

#[test]
fn npos_network_binds_its_dataspace_catalog_into_the_shared_file() {
    let (_parent, out_dir, stdout) = create(SumeragiConsensusMode::Npos, roster(4));
    assert!(stdout.contains("consensus_mode: npos"));
    let shared = read_table(&out_dir.join(NETWORK_CONFIG_FILE));
    let catalog = shared["nexus"]["dataspace_catalog"].as_array().unwrap();
    assert_eq!(catalog.len(), 1);
    assert_eq!(catalog[0]["alias"].as_str(), Some("universal"));
    assert_eq!(catalog[0]["fault_tolerance"].as_integer(), Some(1));
    restage(&out_dir, &published_node_config(&out_dir))
        .expect("shared NPoS settings reproduce the signed policy");
}
