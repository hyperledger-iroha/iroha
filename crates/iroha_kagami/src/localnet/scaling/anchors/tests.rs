//! Final generator receipt bindings, role projection and publication failures.
use super::*;
use clap::Parser as _;
use iroha_data_model::block::consensus_v2::HeightContext;
#[cfg(unix)]
use std::os::fd::{AsRawFd as _, IntoRawFd as _};

// Canonical descriptor payload derived from the original deterministic fixture label.
const SEED: &str = "bff633fc85f3ba8c77bdd830cd9cbf008f790180a575ff52279c7e226483a08c";
struct Fixture {
    _temp: tempfile::TempDir,
    root: PathBuf,
    expected: HashOf<BlockHeader>,
    peers: Vec<Peer>,
    accounts: Vec<LocalnetClientIdentity>,
    configs: Vec<[u8; 32]>,
    client: [u8; 32],
    account_configs: Vec<[u8; 32]>,
    context: Vec<u8>,
    receipt: norito::json::Value,
}
impl Fixture {
    fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let root = fs::canonicalize(temp.path()).unwrap().join("fixed");
        #[cfg(unix)]
        let seed_input = super::super::tests::seed_pipe(SEED.as_bytes());
        #[cfg(unix)]
        let seed_fd = seed_input.as_raw_fd().to_string();
        #[cfg(not(unix))]
        let seed_fd = "3".to_owned();
        let cli = crate::Cli::try_parse_from([
            "kagami",
            "localnet",
            "--out-dir",
            root.to_str().unwrap(),
            "--seed-fd",
            &seed_fd,
            "--scaling-lanes",
            "4",
        ])
        .unwrap();
        let mut output = BufWriter::new(Vec::new());
        #[cfg(unix)]
        let _transferred_seed = seed_input.into_raw_fd();
        cli.command.run(&mut output).unwrap();
        let output = output.into_inner().unwrap();
        assert_eq!(output, fs::read(root.join(RECEIPT_FILE)).unwrap());
        let receipt: norito::json::Value = norito::json::from_slice(&output).unwrap();
        let signed =
            iroha_genesis::read_signed_genesis_bytes(&root.join("genesis.signed.nrt")).unwrap();
        let expected = iroha_genesis::decode_signed_genesis(&signed)
            .unwrap()
            .hash();
        let peers = build_peers(4, Some(SEED.as_bytes()), 8080, 1337).unwrap();
        let accounts = Self::layout().identities(Some(SEED.as_bytes())).unwrap();
        let configs = (0..4)
            .map(|index| {
                iroha_crypto::sha256(&fs::read(root.join(format!("peer{index}.toml"))).unwrap())
            })
            .collect();
        let client = iroha_crypto::sha256(&fs::read(root.join("client.toml")).unwrap());
        let account_configs = (0..4)
            .map(|index| {
                iroha_crypto::sha256(&fs::read(root.join(account_config_name(index))).unwrap())
            })
            .collect();
        let context = fs::read(root.join(GENESIS_CONTEXT_FILE)).unwrap();
        Self {
            _temp: temp,
            root,
            expected,
            peers,
            accounts,
            configs,
            client,
            account_configs,
            context,
            receipt,
        }
    }
    fn layout() -> ScalingLayout {
        ScalingLayout::from_args(Some(ScalingLanes::Four), Some(4))
            .unwrap()
            .unwrap()
    }
    fn host() -> CanonicalHost {
        CanonicalHost::parse(DEFAULT_PUBLIC_HOST, "test").unwrap()
    }
    fn reopen(&self) -> Inputs {
        // Remove only the prior successful test publications; originals stay unchanged.
        for name in std::iter::once(RECEIPT_FILE.to_owned())
            .chain((0..4).map(|index| format!("peer{index}-client.toml")))
        {
            fs::remove_file(self.root.join(name)).unwrap();
        }
        let mut inputs = Inputs::new(&self.root, self.expected).unwrap();
        inputs
            .authenticate(&self.peers, &self.configs, &self.context)
            .unwrap();
        inputs
    }
    fn publish<W: Write>(&self, inputs: Inputs, writer: &mut BufWriter<W>) -> Result<()> {
        inputs.publish(
            writer,
            Self::layout(),
            &self.peers,
            &self.accounts,
            &Self::host(),
            DEFAULT_CHAIN_ID,
            self.client,
            &self.account_configs,
        )
    }
}
#[test]
fn receipt_public_typed_anchors_and_census_match_every_original() {
    let f = Fixture::new();
    let receipt = &f.receipt;
    assert_eq!(
        receipt
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([
            "schema",
            "version",
            "consensus_mode",
            "chain_id",
            "lane_count",
            "genesis_hash",
            "context_id",
            "network_id",
            "peers",
            "accounts",
            "artifacts",
            "genesis_public_key",
            "chain_discriminant"
        ])
    );
    assert_eq!(
        receipt["schema"].as_str(),
        Some("iroha.sumeragi_v2.scaling.genesis_anchors.v1")
    );
    assert_eq!(receipt["version"].as_u64(), Some(1));
    assert_eq!(receipt["lane_count"].as_u64(), Some(4));
    assert_eq!(receipt["consensus_mode"].as_str(), Some("npos"));
    assert_eq!(receipt["chain_id"].as_str(), Some(DEFAULT_CHAIN_ID));
    let context: HeightContext = norito::decode_from_bytes(&f.context).unwrap();
    assert_eq!(
        receipt["genesis_hash"],
        norito::json::to_value(&f.expected).unwrap()
    );
    assert_eq!(
        receipt["context_id"],
        norito::json::to_value(&context.id()).unwrap()
    );
    assert_eq!(
        receipt["network_id"],
        norito::json::to_value(&context.network_id).unwrap()
    );
    assert_eq!(receipt["peers"].as_array().unwrap().len(), 4);
    assert_eq!(receipt["accounts"].as_array().unwrap().len(), 4);
    let mut census = BTreeSet::new();
    for entry in receipt["artifacts"].as_array().unwrap() {
        let path = entry["path"].as_str().unwrap();
        assert!(census.insert(path));
        assert_ne!(path, RECEIPT_FILE);
        let bytes = fs::read(f.root.join(path)).unwrap();
        assert_eq!(entry["bytes"].as_u64(), Some(bytes.len() as u64));
        assert_eq!(
            entry["sha256"].as_str().unwrap(),
            hex::encode(iroha_crypto::sha256(&bytes))
        );
    }
    assert_eq!(census.len(), 17);
    for (index, peer) in f.peers.iter().enumerate() {
        assert_eq!(
            receipt["peers"][index]["role"].as_str(),
            Some(format!("peer{index}").as_str())
        );
        assert_eq!(
            receipt["peers"][index]["node_public_key"],
            norito::json::to_value(&peer.public_key).unwrap()
        );
        let peer_fields = receipt["peers"][index]
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            peer_fields,
            BTreeSet::from([
                "role",
                "node_public_key",
                "torii_url",
                "config",
                "client_config",
                "primary_block_store",
                "primary_merge_log"
            ])
        );
        let node_path = f.root.join(format!("peer{index}.toml"));
        let node = Zeroizing::new(fs::read_to_string(&node_path).unwrap());
        let config = parse_localnet_peer_config(&node, Some(&node_path)).unwrap();
        assert_eq!(
            receipt["genesis_public_key"],
            norito::json::to_value(&config.genesis.public_key).unwrap()
        );
        assert_eq!(
            receipt["chain_discriminant"].as_u64(),
            Some(u64::from(*config.common.chain_discriminant.value()))
        );
        let primary = config.nexus.lane_config.primary();
        let blocks = primary.blocks_dir(config.kura.store_dir.value());
        let merge = primary.merge_log_path(config.kura.store_dir.value());
        assert_eq!(
            receipt["peers"][index]["primary_block_store"].as_str(),
            blocks.to_str()
        );
        assert_eq!(
            receipt["peers"][index]["primary_merge_log"].as_str(),
            merge.to_str()
        );
        assert!(!blocks.exists());
        assert!(!merge.exists());
        let name = format!("peer{index}-client.toml");
        let text = Zeroizing::new(fs::read_to_string(f.root.join(&name)).unwrap());
        let table =
            crate::secret_toml::Table::new(crate::secret_toml::parse_table(&text, "test").unwrap());
        assert!(!table.contains_key("extends"));
        assert!(!table.contains_key("network_id_file"));
        assert_eq!(table["network_id"].as_str(), receipt["network_id"].as_str());
        assert_eq!(
            table["torii_url"].as_str(),
            Some(Fixture::host().torii_url(peer.api_port).as_str())
        );
        assert_eq!(
            receipt["peers"][index]["client_config"].as_str(),
            Some(name.as_str())
        );
        assert!(census.contains(name.as_str()));
    }
    let public = norito::json::to_json(receipt).unwrap();
    assert!(!public.contains(SEED));
    assert!(!public.contains("private_key"));
    assert!(!public.contains("password"));
}
#[test]
fn foreign_network_record_before_capture_is_rejected() {
    let f = Fixture::new();
    let foreign = NetworkId::from_genesis_hash(HashOf::from_untyped_unchecked(
        iroha_crypto::Hash::new(b"foreign"),
    ));
    fs::write(
        f.root.join(GENESIS_EXPECTED_HASH_FILE),
        format!("{foreign}\n").as_bytes(),
    )
    .unwrap();
    assert!(Inputs::new(&f.root, f.expected).is_err());
}
#[test]
fn original_client_mutations_before_capture_are_rejected() {
    for workload in [false, true] {
        for field in ["torii_url", "chain", "private_key", "network_id"] {
            let f = Fixture::new();
            let inputs = f.reopen();
            let name = if workload {
                account_config_name(0)
            } else {
                "client.toml".to_owned()
            };
            let text = Zeroizing::new(fs::read_to_string(f.root.join(&name)).unwrap());
            let mut table = crate::secret_toml::Table::new(
                crate::secret_toml::parse_table(&text, "test").unwrap(),
            );
            if field == "private_key" {
                table
                    .get_mut("account")
                    .unwrap()
                    .as_table_mut()
                    .unwrap()
                    .insert(
                        field.to_owned(),
                        Value::String("foreign-private-material".to_owned()),
                    );
            } else {
                crate::secret_toml::insert(
                    &mut table,
                    field.to_owned(),
                    Value::String("foreign".to_owned()),
                );
            }
            let bytes = Zeroizing::new(toml::to_string(&*table).unwrap());
            fs::write(f.root.join(&name), bytes.as_bytes()).unwrap();
            assert!(
                f.publish(inputs, &mut BufWriter::new(Vec::new())).is_err(),
                "{workload}/{field}"
            );
            assert!(!f.root.join(RECEIPT_FILE).exists());
        }
    }
}
#[test]
fn original_peer_and_context_mismatches_are_rejected() {
    let f = Fixture::new();
    let mut input = Inputs::new(&f.root, f.expected).unwrap();
    let mut digests = f.configs.clone();
    digests[0][0] ^= 1;
    assert!(input.authenticate(&f.peers, &digests, &f.context).is_err());
    let mut input = Inputs::new(&f.root, f.expected).unwrap();
    let mut context = f.context.clone();
    context[0] ^= 1;
    assert!(input.authenticate(&f.peers, &f.configs, &context).is_err());
    let config = parse_localnet_peer_config(
        &fs::read_to_string(f.root.join("peer0.toml")).unwrap(),
        Some(&f.root.join("peer0.toml")),
    )
    .unwrap();
    let mut input = Inputs::new(&f.root, f.expected).unwrap();
    assert!(input.stage_peer(&config, &f.peers, 1).is_err());
    let mut foreign = build_peers(4, Some(b"foreign"), 8080, 1337).unwrap();
    foreign[0].public_key = f.peers[0].public_key.clone();
    assert!(input.stage_peer(&config, &foreign, 0).is_err());
}
struct OutputFailure {
    mutation: Option<PathBuf>,
    fail_write: bool,
    fail_flush: bool,
}
impl Write for OutputFailure {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.fail_write {
            return Err(std::io::Error::other("fixture output write"));
        }
        if let Some(path) = self.mutation.take() {
            fs::write(path, b"changed original").unwrap();
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        if self.fail_flush {
            Err(std::io::Error::other("fixture output flush"))
        } else {
            Ok(())
        }
    }
}
#[test]
fn publication_keeps_originals_through_output_write_flush_and_final_check() {
    for (write, flush, mutate) in [
        (true, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let f = Fixture::new();
        let input = f.reopen();
        let output = OutputFailure {
            mutation: mutate.then(|| f.root.join("peer0.toml")),
            fail_write: write,
            fail_flush: flush,
        };
        assert!(f.publish(input, &mut BufWriter::new(output)).is_err());
        assert!(f.root.join(RECEIPT_FILE).exists());
    }
}
#[test]
fn retained_anchor_republication_requires_complete_original_pins() {
    let f = Fixture::new();
    let input = f.reopen();
    let mut output = BufWriter::new(Vec::new());
    f.publish(input, &mut output).unwrap();
    let bytes = output.into_inner().unwrap();
    assert_eq!(bytes, fs::read(f.root.join(RECEIPT_FILE)).unwrap());
    assert_eq!(
        norito::json::from_slice::<norito::json::Value>(&bytes).unwrap(),
        f.receipt
    );
}

#[test]
fn incomplete_authority_and_oversized_public_chain_cannot_publish() {
    let f = Fixture::new();
    let incomplete = Inputs::new(&f.root, f.expected).unwrap();
    assert!(
        f.publish(incomplete, &mut BufWriter::new(Vec::new()))
            .is_err()
    );
    let input = f.reopen();
    assert!(
        input
            .publish(
                &mut BufWriter::new(Vec::new()),
                Fixture::layout(),
                &f.peers,
                &f.accounts,
                &Fixture::host(),
                &"x".repeat(1025),
                f.client,
                &f.account_configs
            )
            .is_err()
    );
    assert!(!f.root.join(RECEIPT_FILE).exists());
    assert!(!f.root.join("peer0-client.toml").exists());
}

#[test]
fn receipt_rejects_unlisted_scaffold_and_nonempty_initial_runtime() {
    for nested in [false, true] {
        let f = Fixture::new();
        let input = f.reopen();
        let path = f.root.join(if nested {
            "storage/peer0/state/foreign"
        } else {
            "unlisted"
        });
        fs::write(path, b"foreign").unwrap();
        assert!(f.publish(input, &mut BufWriter::new(Vec::new())).is_err());
    }
}

#[test]
fn primary_reader_paths_follow_final_native_lane_geometry_and_bounds() {
    let f = Fixture::new();
    for index in 0..4 {
        let path = f.root.join(format!("peer{index}.toml"));
        let text = Zeroizing::new(fs::read_to_string(&path).unwrap());
        let config = parse_localnet_peer_config(&text, Some(&path)).unwrap();
        let (blocks, merge) = primary_reader_paths(&config).unwrap();
        let root = config.kura.store_dir.value();
        assert!(Path::new(&blocks).starts_with(root));
        assert!(Path::new(&merge).starts_with(root));
        assert_ne!(Path::new(&blocks), root);
        assert_ne!(blocks, merge);
        let mut altered = config.clone();
        altered.kura.store_dir = iroha_config::base::WithOrigin::inline(PathBuf::from("relative"));
        assert!(primary_reader_paths(&altered).is_err());
        altered.kura.store_dir =
            iroha_config::base::WithOrigin::inline(PathBuf::from(format!("/{}", "x".repeat(4096))));
        assert!(primary_reader_paths(&altered).is_err());
        assert_eq!((blocks, merge), primary_reader_paths(&config).unwrap());
    }
}
