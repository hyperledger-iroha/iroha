//! Developer localnet generation tests.

use super::*;
use iroha_data_model::isi::staking::RegisterPublicLaneValidator;
use iroha_genesis::{RawGenesisTransaction, read_signed_genesis};
use std::fs;

fn private_parent() -> (tempfile::TempDir, PathBuf) {
    let temporary = tempfile::tempdir().expect("create temporary directory");
    let private = iroha_fs::PrivateDirectory::open_or_create(temporary.path().join("private"))
        .expect("create private parent");
    let path = private.path().to_path_buf();
    (temporary, path)
}

fn options(out_dir: PathBuf, consensus_mode: SumeragiConsensusMode) -> LocalnetOptions {
    LocalnetOptions {
        perf_profile: None,
        peers: NonZeroU16::new(4).expect("non-zero peer count"),
        seed: Some("developer-localnet-tests".to_owned()),
        bind_host: "127.0.0.1".to_owned(),
        public_host: "127.0.0.1".to_owned(),
        base_api_port: 18_080,
        base_p2p_port: 18_337,
        out_dir,
        extra_accounts: 0,
        assets: Vec::new(),
        block_cadence_ms: None,
        consensus_mode,
    }
}

fn generate_into(opts: &LocalnetOptions) -> Vec<u8> {
    let mut writer = BufWriter::new(Vec::new());
    generate_localnet(opts, &mut writer).expect("generate developer localnet");
    writer.into_inner().expect("flush summary")
}

fn peer_table(out_dir: &Path, index: usize) -> toml::Table {
    fs::read_to_string(out_dir.join(format!("peer{index}.toml")))
        .expect("read generated peer config")
        .parse()
        .expect("parse generated peer config")
}

fn assert_genesis_applies(out_dir: &Path) {
    let signed = read_signed_genesis(&out_dir.join("genesis.signed.nrt"))
        .expect("decode generated signed genesis");
    signed
        .validate_output_merkle_cache()
        .expect("signed genesis has authenticated execution outputs");
    for index in 0..signed.network_entrypoint_count() {
        let (_, output) = signed
            .network_output_at(u32::try_from(index).expect("genesis index fits u32"))
            .expect("every genesis input has its Network output");
        assert!(output.result.is_ok(), "genesis input {index} must apply");
    }
    let expected = fs::read_to_string(out_dir.join(GENESIS_EXPECTED_HASH_FILE))
        .expect("read genesis identity")
        .trim()
        .parse::<NetworkId>()
        .expect("parse genesis identity");
    assert_eq!(signed.hash(), expected.into_genesis_hash());
}

#[test]
fn localnet_refuses_public_chain_identities_before_creating_output() {
    let (_temporary, parent) = private_parent();
    for (index, chain) in [PUBLIC_TAIRA_CHAIN_ID, PUBLIC_NEXUS_CHAIN_ID]
        .into_iter()
        .enumerate()
    {
        let out_dir = parent.join(format!("public{index}"));
        let error = generate_localnet_with_chain(
            &options(out_dir.clone(), SumeragiConsensusMode::Npos),
            &mut BufWriter::new(Vec::new()),
            Some(chain),
            None,
        )
        .expect_err("a developer localnet cannot use a public chain identity");
        assert!(
            error.to_string().contains("kagami sora-network generate"),
            "the refusal names the Sora network command: {error}"
        );
        assert!(
            !out_dir.exists(),
            "a refused request must not create output"
        );
    }
}

#[test]
fn localnet_chain_resolution_keeps_generator_validation() {
    assert_eq!(
        resolve_localnet_chain_id(None).expect("default developer chain"),
        DEFAULT_CHAIN_ID
    );
    let custom = "6b6d7e8a-7e43-4b8e-9d1d-2d5c1e0f4a11";
    assert_eq!(
        resolve_localnet_chain_id(Some(custom)).expect("custom developer chain"),
        custom
    );
    for rejected in [
        "",
        "   ",
        " 6b6d7e8a-7e43-4b8e-9d1d-2d5c1e0f4a11",
        "iroha3-taira",
        "iroha3-nexus",
        "cbdc16",
    ] {
        assert!(
            resolve_localnet_chain_id(Some(rejected)).is_err(),
            "developer chain identity {rejected:?} must be rejected"
        );
    }
}

#[test]
fn developer_bundle_is_minimal_and_loads_like_the_node() {
    let (_temporary, parent) = private_parent();
    let out_dir = parent.join("network");
    let summary = String::from_utf8(generate_into(&options(
        out_dir.clone(),
        SumeragiConsensusMode::Permissioned,
    )))
    .expect("UTF-8 summary");
    assert!(summary.contains("consensus_mode: permissioned"));
    assert!(summary.contains("next_start: cd "));
    for present in [
        ".gitignore",
        "README.md",
        "client.toml",
        "genesis.json",
        "genesis.signed.nrt",
        "nexus-amx-context.v1.bin",
        GENESIS_EXPECTED_HASH_FILE,
        GENESIS_PUBLIC_KEY_FILE,
        GENESIS_PRIVATE_KEY_FILE,
        "start.sh",
        "stop.sh",
        "runtime/operator-signer.key",
        config::RANS_TABLE_RELATIVE_PATH,
    ] {
        assert!(
            out_dir.join(present).is_file(),
            "{present} must be generated"
        );
    }
    for production_only in [
        "alias-setup.intent.json",
        "runtime/onboarding.token",
        "runtime/onboarding-signer.key",
        "runtime/ledger-signer.key",
        "nodes",
    ] {
        assert!(
            !out_dir.join(production_only).exists(),
            "{production_only} belongs to Sora network bundles"
        );
    }
    assert_genesis_applies(&out_dir);
    let signed_hash = read_signed_genesis(&out_dir.join("genesis.signed.nrt"))
        .expect("decode signed genesis")
        .hash();
    for index in 0..4 {
        let path = out_dir.join(format!("peer{index}.toml"));
        let rendered = fs::read_to_string(&path).expect("read peer config");
        let parsed = config::parse_peer_config(&rendered, &path).expect("node accepts peer config");
        assert_eq!(parsed.genesis.expected_hash, signed_hash);
        let table = peer_table(&out_dir, index);
        let torii = table["torii"].as_table().expect("torii section");
        for absent in ["faucet", "account_onboarding", "peer_telemetry_urls", "mcp"] {
            assert!(
                !torii.contains_key(absent),
                "torii.{absent} is not generated"
            );
        }
        assert!(
            !table["network"]
                .as_table()
                .expect("network section")
                .contains_key("soranet_vpn")
        );
        assert!(
            !table.contains_key("data_dir"),
            "bundles use explicit paths"
        );
        assert!(!table.contains_key("confidential"));
        assert_eq!(
            torii["operator_signatures"]["enabled"].as_bool(),
            Some(true),
            "operator diagnostics stay available"
        );
    }
    let start = fs::read_to_string(out_dir.join("start.sh")).expect("read start.sh");
    assert!(
        !start.contains("python"),
        "the launcher needs only a POSIX shell"
    );
    assert!(start.contains("--sumeragi-assert-fresh-key"));
    assert!(
        start.contains(r#"(cd "$DIR/state/peer${i}" && exec nohup "$@""#),
        "each validator runs from its own state directory"
    );
    let client = fs::read_to_string(out_dir.join("client.toml"))
        .expect("read client config")
        .parse::<toml::Table>()
        .expect("parse client config");
    assert_eq!(
        client["http_transport_policy"].as_str(),
        Some(HttpTransportPolicy::Ordinary.as_str())
    );
}

#[test]
fn npos_localnet_registers_one_staked_validator_per_peer() {
    let (_temporary, parent) = private_parent();
    let out_dir = parent.join("npos");
    generate_into(&options(out_dir.clone(), SumeragiConsensusMode::Npos));
    assert_genesis_applies(&out_dir);
    let manifest = RawGenesisTransaction::from_path(out_dir.join("genesis.json"))
        .expect("parse generated genesis");
    assert_eq!(manifest.consensus_mode(), SumeragiConsensusMode::Npos);
    let registrations = manifest
        .instructions()
        .filter(|instruction| {
            instruction
                .as_any()
                .downcast_ref::<RegisterPublicLaneValidator>()
                .is_some()
        })
        .count();
    assert_eq!(registrations, 4);
}

#[test]
fn custom_chain_discriminant_renders_prefixed_account_literals() {
    let (_temporary, parent) = private_parent();
    let out_dir = parent.join("prefixed");
    generate_localnet_with_chain(
        &options(out_dir.clone(), SumeragiConsensusMode::Permissioned),
        &mut BufWriter::new(Vec::new()),
        None,
        Some(369),
    )
    .expect("generate localnet with a custom address prefix");
    assert_genesis_applies(&out_dir);
    let manifest = RawGenesisTransaction::from_path(out_dir.join("genesis.json"))
        .expect("parse generated genesis");
    assert_eq!(manifest.chain_discriminant(), 369);
    let client = fs::read_to_string(out_dir.join("client.toml"))
        .expect("read client config")
        .parse::<toml::Table>()
        .expect("parse client config");
    assert_eq!(
        client["account"]["chain_discriminant"].as_integer(),
        Some(369)
    );
    assert!(
        peer_table(&out_dir, 0)["network"]["soranet_vpn"]
            .get("operator_account_id")
            .is_some()
    );
}

#[test]
fn requested_assets_and_extra_accounts_belong_to_the_generated_client() {
    let (_temporary, parent) = private_parent();
    let out_dir = parent.join("assets");
    let mut opts = options(out_dir.clone(), SumeragiConsensusMode::Permissioned);
    opts.extra_accounts = 2;
    opts.assets = vec![
        AssetSpec {
            id: localnet_sample_asset_literal(),
            name: LOCALNET_SAMPLE_ASSET_NAME.to_owned(),
            alias: None,
            owned_by: localnet_client_account_id(),
            mint_to: localnet_client_account_id(),
            quantity: 100,
        },
        requested_localnet_asset_spec("61CtjvNd9T3THAR65GsMVHr82Bjc").expect("requested asset"),
    ];
    generate_into(&opts);
    assert_genesis_applies(&out_dir);
    let manifest = fs::read_to_string(out_dir.join("genesis.json")).expect("read genesis");
    assert!(manifest.contains(&localnet_sample_asset_literal()));
    assert!(manifest.contains("61CtjvNd9T3THAR65GsMVHr82Bjc"));
    assert!(
        !manifest.contains(PLACEHOLDER_CLIENT_PUBLIC_KEY),
        "the placeholder client is replaced by the generated client"
    );
}

#[test]
fn perf_profile_requires_its_consensus_mode_before_creating_output() {
    let (_temporary, parent) = private_parent();
    let out_dir = parent.join("perf");
    let mut opts = options(out_dir.clone(), SumeragiConsensusMode::Permissioned);
    opts.perf_profile = Some(LocalnetPerfProfile::Throughput10kNpos);
    assert!(generate_localnet(&opts, &mut BufWriter::new(Vec::new())).is_err());
    assert!(!out_dir.exists());
}

#[test]
fn managed_localnet_prepares_a_standard_generation_the_engine_accepts() {
    let (_temporary, parent) = private_parent();
    let directory = parent.join("generation");
    let ports = crate::managed::LocalnetPorts::reserve().expect("reserve loopback ports");
    let prepared = prepare_localnet_at(
        "local",
        &directory,
        &ports,
        LocalnetServiceProfile::Standard,
        None,
    )
    .expect("prepare managed developer network");
    assert_eq!(
        prepared.service_profile,
        crate::localnet::LocalnetServiceProfile::Standard
    );
    assert_eq!(prepared.peers.len(), 4);
    assert_eq!(prepared.context.dataspace_alias, "universal");
    assert!(
        crate::localnet::service_authorities::validate_retained(&prepared)
            .expect("the managed engine accepts the generation")
            .is_none()
    );
    prepared
        .load_operator_key_pair()
        .expect("Mochi's operator signer binds every validator");
    assert_genesis_applies(&directory);
    for index in 0..4 {
        let table = peer_table(&directory, index);
        assert!(table.contains_key("data_dir"));
        assert_eq!(table["torii"]["mcp"]["enabled"].as_bool(), Some(true));
    }
    assert!(!directory.join("start.sh").exists());
    let client = fs::read_to_string(directory.join("client.toml"))
        .expect("read client config")
        .parse::<toml::Table>()
        .expect("parse client config");
    assert_eq!(
        client["http_transport_policy"].as_str(),
        Some(HttpTransportPolicy::DirectLoopback.as_str())
    );
}

#[test]
fn storage_preparation_binds_four_native_configs_and_a_private_client() {
    let _resources = crate::managed::native_test_guard();
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path().canonicalize().unwrap().join("bundle");
    let ports = crate::managed::LocalnetPorts::reserve().unwrap();
    let prepared = prepare_localnet("local", &root, &ports).unwrap();
    assert_eq!(prepared.peers.len(), 4);
    assert_eq!(prepared.context.name, "local");
    assert_eq!(
        prepared.build_cache_root(),
        root.join("runtime/build-cache")
    );
    assert!(!prepared.build_cache_root().exists());
    let mut another_generation = prepared.clone();
    another_generation.context.client_config = root.join("next/client.toml");
    assert_ne!(
        prepared.build_cache_root(),
        another_generation.build_cache_root()
    );
    assert!(!another_generation.build_cache_root().exists());
    assert!(
        !root.join("next").exists(),
        "path intent must not create a generation"
    );
    assert_eq!(prepared.context.dataspace_alias, "universal");
    assert_eq!(prepared.context.dataspace_id, 0);
    let config = prepared.context.load_client_config().unwrap();
    assert_eq!(
        prepared.service_profile,
        LocalnetServiceProfile::StreamTokenAuthorities
    );
    let authorities = prepared.stream_token_authorities().unwrap().unwrap();
    assert_eq!(authorities.network.authorities.len(), 3);
    assert_eq!(authorities.providers.len(), 3);
    for provider in &authorities.providers {
        assert_eq!(
            provider.authorities.len(),
            10,
            "every original provider holds its ten fixed service roles"
        );
    }
    assert_eq!(authorities.network_id, config.network_id);
    assert_eq!(authorities.manager, config.account);
    let operator = prepared.load_operator_key_pair().unwrap();
    assert_ne!(operator.public_key(), config.key_pair.public_key());
    for (index, peer) in prepared.peers.iter().enumerate() {
        let bytes = iroha_fs::read_private(&peer.config_path, 1024 * 1024).unwrap();
        let table: toml::Table = std::str::from_utf8(&bytes).unwrap().parse().unwrap();
        let config =
            config::parse_peer_config(std::str::from_utf8(&bytes).unwrap(), &peer.config_path)
                .unwrap();
        // Proof validity is committed chain policy; generated node settings must
        // neither select a backend nor preserve a retired compatibility setting.
        let local_zk = table.get("zk").and_then(toml::Value::as_table);
        assert!(local_zk.is_none_or(|zk| !zk.contains_key("pipa_r")));
        assert!(local_zk.is_none_or(|zk| !zk.contains_key("halo2")));
        assert!(config.zk.pipa_r.enabled);
        for enabled in [true, false] {
            let node_validity = Zeroizing::new(format!(
                "{}\n[zk.pipa_r]\nenabled = {enabled}\n",
                std::str::from_utf8(&bytes).unwrap(),
            ));
            assert!(config::parse_peer_config(&node_validity, &peer.config_path).is_err());
        }
        let retired = Zeroizing::new(format!(
            "{}\n[zk.halo2]\nenabled = true\n",
            std::str::from_utf8(&bytes).unwrap()
        ));
        assert!(config::parse_peer_config(&retired, &peer.config_path).is_err());
        assert_eq!(
            config::parse_peer_config(std::str::from_utf8(&bytes).unwrap(), &peer.config_path)
                .map(|retry| retry.genesis.expected_hash)
                .unwrap(),
            config.genesis.expected_hash,
            "the same original generated config remains admissible after retired-field refusal"
        );
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
        assert_eq!(
            config.network.connect_startup_delay,
            std::time::Duration::ZERO
        );
        assert_eq!(
            (config.network.dial_timeout, config.network.preauth_timeout),
            (
                iroha_config::parameters::defaults::network::DIAL_TIMEOUT,
                iroha_config::parameters::defaults::network::PREAUTH_TIMEOUT,
            )
        );
        // All-loopback validators use the cheap admission puzzle.
        let pow = &config.network.soranet_handshake.pow;
        assert_eq!(pow.difficulty, 6);
        assert_eq!(
            (
                pow.puzzle.memory_kib.get(),
                pow.puzzle.time_cost.get(),
                pow.puzzle.lanes.get()
            ),
            (4_096, 1, 1)
        );
        let node = root.join("nodes").join(format!("peer{index}"));
        assert_eq!(table["data_dir"].as_str(), node.to_str());
        assert!(table["sumeragi"].get("mint_finality_seed_fd").is_none());
        assert_eq!(
            table["nexus"]["storage"]["local_budget_bytes"].as_integer(),
            Some(config::NEXUS_STORAGE_BUDGET_BYTES),
            "every managed validator needs its finite developer storage cap"
        );
        let node = iroha_fs::PrivateDirectory::open(node).unwrap();
        assert!(!node.path().join("secrets/mint_finality.seed").exists());
    }
    assert!(!root.join("start.sh").exists());
    assert!(!root.join("stop.sh").exists());
    assert!(prepare_localnet("local", &root, &ports).is_err());
    assert_eq!(ports.reserved_count(), 8);
    let runtime = iroha_fs::PrivateDirectory::open(root.join(RUNTIME_DIRECTORY)).unwrap();
    runtime
        .write_atomic(
            OPERATOR_SIGNER_KEY_FILE,
            b"malformed-secret\n",
            iroha_fs::PublishMode::Replace,
        )
        .unwrap();
    let error = prepared.load_operator_key_pair().unwrap_err().to_string();
    assert!(!error.contains("malformed-secret"));
}

#[test]
fn custom_chain_prefix_configs_leave_default_governance_accounts_implicit() {
    let (_temporary, parent) = private_parent();
    let out_dir = parent.join("custom-prefix");
    let opts = options(out_dir.clone(), SumeragiConsensusMode::Permissioned);
    generate_localnet_with_chain(&opts, &mut BufWriter::new(Vec::new()), None, Some(777))
        .expect("generate a localnet with a custom account prefix");
    let path = out_dir.join("peer0.toml");
    let source = fs::read_to_string(&path).expect("read generated peer config");
    assert!(
        source.starts_with(config::PEER_CONFIG_HEADER),
        "operator-visible configs explain which values a node needs"
    );
    let table: toml::Table = toml::from_str(&source).expect("parse generated peer config");
    assert_eq!(table["chain_discriminant"].as_integer(), Some(777));
    assert!(
        !table.contains_key("gov"),
        "default governance accounts stay valid for every chain prefix"
    );
    let _chain_discriminant =
        iroha_data_model::account::address::ChainDiscriminantGuard::enter(777);
    let parsed = config::parse_peer_config(&source, &path).expect("generated config");
    let defaults = iroha_config::parameters::defaults::governance::bond_escrow_account_id();
    assert_eq!(parsed.gov.bond_escrow_account, defaults);
    assert_eq!(parsed.gov.citizenship_escrow_account, defaults);
    assert_eq!(
        parsed.gov.sorafs_pin_fee_treasury_account,
        iroha_config::parameters::defaults::governance::sorafs_pin_fee::treasury_account_id()
    );
}
