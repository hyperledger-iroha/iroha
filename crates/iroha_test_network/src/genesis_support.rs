//! Shared orchestration helpers for preparing and validating local genesis artifacts.
use color_eyre::eyre::{Result, WrapErr, eyre};
use iroha_config::{base::toml::TomlSource, parameters::actual};
use iroha_crypto::{Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    block::{BlockHeader, SignedBlock},
    da::commitment::DaProofPolicyBundle,
    parameter::system::SumeragiConsensusMode,
};
use iroha_genesis::{RawGenesisTransaction, ValidatedGenesisBundle};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};
/// Exact placeholder accepted while a genesis hash is not yet known.
///
/// The placeholder is replaced only in memory and only by
/// [`sign_prepared_genesis_from_config`]. Persisted node configurations still
/// have to contain the exact hash before normal startup validation.
pub const UNRESOLVED_GENESIS_EXPECTED_HASH: &str = "REPLACE_WITH_GENESIS_EXPECTED_HASH";
/// Filesystem paths controlled by a local node-generation orchestrator.
///
/// Every path is owned and detached from `iroha_config`'s origin wrappers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedNodePaths {
    /// Resolved Kura block-store directory.
    pub kura_store_dir: PathBuf,
    /// Resolved snapshot-store directory.
    pub snapshot_store_dir: PathBuf,
    /// Torii's persistent data directory.
    pub torii_data_dir: PathBuf,
    /// Torii's persistent DA replay-cache directory.
    pub torii_da_replay_cache_store_dir: PathBuf,
    /// Torii's persistent DA manifest-spool directory.
    pub torii_da_manifest_store_dir: PathBuf,
    /// Torii's SoraFS storage directory.
    pub torii_sorafs_storage_data_dir: PathBuf,
    /// Streaming session-store directory.
    pub streaming_session_store_dir: PathBuf,
    /// Streaming SoraNet provision-spool directory.
    pub streaming_soranet_provision_spool_dir: PathBuf,
    /// Streaming SoraVPN provision-spool directory.
    pub streaming_soravpn_provision_spool_dir: PathBuf,
    /// Streaming codec's signed rANS-table path.
    pub streaming_rans_tables_path: PathBuf,
    /// SoraNet proof-of-work ticket-revocation store path.
    pub soranet_pow_revocation_store_path: PathBuf,
}
/// Owned projection of the node configuration bindings needed by genesis orchestration.
///
/// This deliberately exposes domain types and owned values rather than any
/// `iroha_config` representation. Parsing therefore remains centralized and
/// canonical while callers cannot depend on configuration-layer internals.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ManagedNodeConfig {
    /// Configured chain identifier.
    pub chain_id: ChainId,
    /// Configured I105 chain discriminant.
    pub chain_discriminant: u16,
    /// Public key of the configured local node.
    pub local_public_key: PublicKey,
    /// Trusted validator proof-of-possession roster.
    pub trusted_peer_pops: BTreeMap<PublicKey, Vec<u8>>,
    /// Public key authorized to sign genesis.
    pub genesis_public_key: PublicKey,
    /// Exact configured genesis block-header hash.
    pub genesis_expected_hash: HashOf<BlockHeader>,
    /// Resolved path to the signed genesis block.
    pub genesis_block_path: PathBuf,
    /// Resolved path to the raw genesis manifest.
    pub genesis_manifest_path: PathBuf,
    /// Exact DA proof-policy bundle derived from the node configuration.
    pub da_proof_policies: DaProofPolicyBundle,
    /// Exact confidential-policy hash derived from the node configuration.
    pub genesis_confidential_policy_hash: [u8; 32],
    /// Filesystem paths managed by local node generation.
    pub managed_paths: ManagedNodePaths,
}
impl ManagedNodeConfig {
    /// Parse a node configuration through `iroha_config` and project the
    /// genesis bindings needed by local orchestration.
    ///
    /// Relative Kura, snapshot, genesis-block, and genesis-manifest paths are
    /// resolved against the configuration file which introduced them.
    ///
    /// # Errors
    ///
    /// Returns an error when the file cannot be read, canonical configuration
    /// parsing fails, or either required genesis artifact path is absent.
    pub fn from_path(path: &Path) -> Result<Self> {
        let (config, _) = load_node_config(path, false)?;
        Self::from_root(config)
    }
    fn from_root(config: actual::Root) -> Result<Self> {
        let genesis_block_path = config
            .genesis
            .file
            .as_ref()
            .ok_or_else(|| eyre!("node configuration omits required `genesis.file`"))?
            .resolve_relative_path();
        let genesis_manifest_path = config
            .genesis
            .manifest_json
            .as_ref()
            .ok_or_else(|| eyre!("node configuration omits required `genesis.manifest_json`"))?
            .resolve_relative_path();
        let managed_paths = ManagedNodePaths {
            kura_store_dir: config.kura.store_dir.resolve_relative_path(),
            snapshot_store_dir: config.snapshot.store_dir.resolve_relative_path(),
            torii_data_dir: config.torii.data_dir.clone(),
            torii_da_replay_cache_store_dir: config.torii.da_ingest.replay_cache_store_dir.clone(),
            torii_da_manifest_store_dir: config.torii.da_ingest.manifest_store_dir.clone(),
            torii_sorafs_storage_data_dir: config.torii.sorafs_storage.data_dir.clone(),
            streaming_session_store_dir: config.streaming.session_store_dir.clone(),
            streaming_soranet_provision_spool_dir: config
                .streaming
                .soranet
                .provision_spool_dir
                .clone(),
            streaming_soravpn_provision_spool_dir: config
                .streaming
                .soravpn
                .provision_spool_dir
                .clone(),
            streaming_rans_tables_path: config.streaming.codec.rans_tables_path.clone(),
            soranet_pow_revocation_store_path: PathBuf::from(
                config
                    .network
                    .soranet_handshake
                    .pow
                    .revocation_store_path
                    .as_ref(),
            ),
        };
        Ok(Self {
            chain_id: config.common.chain.clone(),
            chain_discriminant: *config.common.chain_discriminant.value(),
            local_public_key: config.common.key_pair.public_key().clone(),
            trusted_peer_pops: config.common.trusted_peers.value().pops.clone(),
            genesis_public_key: config.genesis.public_key.clone(),
            genesis_expected_hash: config.genesis.expected_hash,
            genesis_block_path,
            genesis_manifest_path,
            da_proof_policies: iroha_core::da::proof_policy_bundle(&config.nexus.lane_config),
            genesis_confidential_policy_hash:
                iroha_core::state::compute_genesis_confidential_policy_hash(&config.zk),
            managed_paths,
        })
    }
}
/// Build and sign a prepared genesis manifest using the policies selected by a
/// canonical node configuration.
///
/// The manifest chain, chain discriminant, optional expected consensus mode, signing key, and
/// canonical manifest path are bound to the parsed configuration before signing. The unresolved
/// expected-hash sentinel is accepted only for this preparation step and is replaced in memory
/// before canonical configuration parsing. If the configuration already selects an exact hash, the
/// newly produced block must match it.
///
/// # Errors
///
/// Returns an error when either input cannot be parsed, a binding differs, or
/// canonical genesis construction or signing fails.
pub fn sign_prepared_genesis_from_config(
    manifest_path: &Path,
    config_path: &Path,
    key_pair: &KeyPair,
    expected_consensus_mode: Option<SumeragiConsensusMode>,
) -> Result<SignedBlock> {
    iroha_genesis::init_instruction_registry();
    let (parsed_config, unresolved_hash_replaced) = load_node_config(config_path, true)?;
    let config = ManagedNodeConfig::from_root(parsed_config.clone())?;
    let selected_manifest = config
        .genesis_manifest_path
        .canonicalize()
        .wrap_err_with(|| {
            format!(
                "resolve configured genesis manifest `{}`",
                config.genesis_manifest_path.display()
            )
        })?;
    let supplied_manifest = manifest_path.canonicalize().wrap_err_with(|| {
        format!(
            "resolve supplied genesis manifest `{}`",
            manifest_path.display()
        )
    })?;
    if supplied_manifest != selected_manifest {
        return Err(eyre!(
            "supplied genesis manifest `{}` differs from configured manifest `{}`",
            supplied_manifest.display(),
            selected_manifest.display()
        ));
    }
    let manifest =
        RawGenesisTransaction::from_path(&config.genesis_manifest_path).wrap_err_with(|| {
            format!(
                "parse prepared genesis manifest `{}`",
                config.genesis_manifest_path.display()
            )
        })?;
    if manifest.chain_id() != &config.chain_id {
        return Err(eyre!(
            "genesis manifest chain `{}` differs from configured chain `{}`",
            manifest.chain_id(),
            config.chain_id
        ));
    }
    if manifest.chain_discriminant() != config.chain_discriminant {
        return Err(eyre!(
            "genesis manifest chain discriminant {} differs from configured discriminant {}",
            manifest.chain_discriminant(),
            config.chain_discriminant
        ));
    }
    if key_pair.public_key() != &config.genesis_public_key {
        return Err(eyre!(
            "genesis signing key `{}` differs from configured genesis key `{}`",
            key_pair.public_key(),
            config.genesis_public_key
        ));
    }
    if let Some(expected_mode) = expected_consensus_mode
        && manifest.consensus_mode() != expected_mode
    {
        return Err(eyre!(
            "genesis manifest consensus mode {:?} differs from expected mode {:?}",
            manifest.consensus_mode(),
            expected_mode
        ));
    }
    let proposal = manifest
        .build_and_sign_with_da_proof_policies_and_confidential_policy_hash(
            key_pair,
            Some(config.da_proof_policies),
            Some(config.genesis_confidential_policy_hash),
        )
        .wrap_err("build and sign canonical prepared genesis")?;
    let topology = iroha_core::sumeragi::signed_genesis_voting_peers(&proposal)
        .map_err(|error| eyre!("derive prepared genesis voting roster: {error}"))?;
    let genesis_account = AccountId::new(key_pair.public_key().clone());
    let (block, _) = crate::config::preexecute_genesis_with_runtime_config(
        &proposal,
        &genesis_account,
        &topology,
        key_pair,
        None,
        None,
        None,
        Some(&parsed_config),
    )
    .wrap_err("pre-execute canonical prepared genesis")?;
    if !unresolved_hash_replaced && block.hash() != config.genesis_expected_hash {
        return Err(eyre!(
            "prepared genesis hashes to {}, but configuration requires {}",
            block.hash(),
            config.genesis_expected_hash
        ));
    }
    Ok(block)
}
/// Validate a canonical prepared-genesis bundle for node startup.
///
/// This composes the independent manifest/wire validator with Core's complete
/// genesis-block invariant check. The expected chain is bound explicitly
/// before either validated result is returned.
///
/// # Errors
///
/// Returns an error when the manifest selects another chain, canonical bundle
/// validation fails, or Core rejects the genesis block.
pub fn validate_prepared_genesis_for_startup(
    signed_wire: &[u8],
    manifest: &RawGenesisTransaction,
    public_key: &PublicKey,
    expected_hash: HashOf<BlockHeader>,
    expected_chain_id: &ChainId,
) -> Result<ValidatedGenesisBundle> {
    if manifest.chain_id() != expected_chain_id {
        return Err(eyre!(
            "genesis manifest chain `{}` differs from expected chain `{}`",
            manifest.chain_id(),
            expected_chain_id
        ));
    }
    let validated = iroha_genesis::validate_prepared_genesis_bundle(
        signed_wire,
        manifest,
        public_key,
        expected_hash,
    )
    .wrap_err("validate canonical prepared genesis bundle")?;
    iroha_core::validate_genesis_block(validated.block(), &AccountId::new(public_key.clone()))
        .map_err(|error| eyre!("validate prepared genesis with Core: {error}"))?;
    Ok(validated)
}
fn load_node_config(path: &Path, allow_unresolved_hash: bool) -> Result<(actual::Root, bool)> {
    let mut source = TomlSource::from_file(path)
        .map_err(|error| eyre!("read node configuration `{}`: {error:?}", path.display()))?;
    let unresolved_hash_replaced = allow_unresolved_hash
        && source
            .table_mut()
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .and_then(|genesis| genesis.get_mut("expected_hash"))
            .is_some_and(|expected_hash| {
                expected_hash.as_str() == Some(UNRESOLVED_GENESIS_EXPECTED_HASH)
            });
    if unresolved_hash_replaced {
        let expected_hash = source
            .table_mut()
            .get_mut("genesis")
            .and_then(toml::Value::as_table_mut)
            .and_then(|genesis| genesis.get_mut("expected_hash"))
            .expect("sentinel location was just verified");
        let hash_body = Hash::new(b"unresolved genesis hash used only for policy derivation")
            .to_string()
            .to_ascii_uppercase();
        *expected_hash = toml::Value::String(norito::literal::format("hash", hash_body.as_str()));
    }
    let config = actual::Root::from_toml_source(source)
        .map_err(|error| eyre!("parse node configuration `{}`: {error:?}", path.display()))?;
    Ok((config, unresolved_hash_replaced))
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::{Algorithm, bls_normal_pop_prove};
    use iroha_data_model::peer::PeerId;
    use iroha_genesis::{GenesisBuilder, GenesisTopologyEntry};
    use std::fs;
    const CONFIGURED_HASH: &str =
        "hash:0000000000000000000000000000000000000000000000000000000000000001#C50E";
    const FIXTURE_GENESIS_PUBLIC_KEY: &str =
        "ed01204164BF554923ECE1FD412D241036D863A6AE430476C898248B8237D77534CFC4";
    fn prepared_manifest(chain_id: ChainId) -> (RawGenesisTransaction, KeyPair) {
        let topology = (0..4)
            .map(|_| {
                let validator = KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
                    .expect("generate validator key");
                let pop = bls_normal_pop_prove(validator.private_key())
                    .expect("generate validator proof of possession");
                GenesisTopologyEntry::new(PeerId::new(validator.public_key().clone()), pop)
            })
            .collect::<Vec<_>>();
        let manifest = GenesisBuilder::new_without_executor(chain_id, ".")
            .set_topology(topology)
            .build_raw()
            .with_consensus_meta();
        let genesis_key = KeyPair::try_random().expect("generate genesis key");
        (manifest, genesis_key)
    }
    fn write_node_config(
        directory: &Path,
        chain_id: &ChainId,
        chain_discriminant: u16,
        genesis_public_key: &PublicKey,
        expected_hash: &str,
    ) -> PathBuf {
        let managed_directory = directory.join("managed");
        fs::create_dir_all(&managed_directory).expect("create managed fixture directory");
        let rans_tables_path = managed_directory.join("rans_tables.toml");
        fs::write(
            &rans_tables_path,
            include_bytes!("../../../codec/rans/tables/rans_seed0.toml"),
        )
        .expect("write signed rANS tables fixture");
        let rans_tables_literal = rans_tables_path.to_string_lossy().replace('\\', "\\\\");
        let mut config = include_str!("../../iroha_config/iroha_test_config.toml").to_owned();
        config = config.replacen(
            "chain = \"00000000-0000-0000-0000-000000000000\"",
            &format!("chain = \"{chain_id}\"\nchain_discriminant = {chain_discriminant}"),
            1,
        );
        config = config.replacen(
            &format!("[genesis]\npublic_key = \"{FIXTURE_GENESIS_PUBLIC_KEY}\""),
            &format!("[genesis]\npublic_key = \"{genesis_public_key}\""),
            1,
        );
        config = config.replacen(
            "file = \"./genesis.signed.nrt\"",
            "file = \"managed/genesis.signed.nrt\"\nmanifest_json = \"genesis.json\"",
            1,
        );
        config = config.replacen(CONFIGURED_HASH, expected_hash, 1);
        config.push_str(
            r#"

[kura]
store_dir = "managed/kura"

[snapshot]
store_dir = "managed/snapshot"

[torii.da_ingest]
replay_cache_store_dir = "managed/torii/da-replay"
manifest_store_dir = "managed/torii/da-manifests"

[sorafs.storage]
data_dir = "managed/sorafs"

[streaming.codec]
rans_tables_path = "__RANS_TABLES_PATH__"

[streaming.soranet]
provision_spool_dir = "managed/streaming/soranet"

[streaming.soravpn]
provision_spool_dir = "managed/streaming/soravpn"

[network.soranet_handshake.pow]
revocation_store_path = "managed/soranet/revocations.norito"
"#,
        );
        config = config.replacen("__RANS_TABLES_PATH__", &rans_tables_literal, 1);
        config = config.replacen(
            "session_store_dir = \"./storage/streaming\"",
            "session_store_dir = \"managed/streaming\"",
            1,
        );
        config = config.replacen(
            "[torii]\naddress = \"addr:127.0.0.1:8080#8942\"",
            "[torii]\naddress = \"addr:127.0.0.1:8080#8942\"\ndata_dir = \"managed/torii\"",
            1,
        );
        let path = directory.join("config.toml");
        fs::write(&path, config).expect("write node config");
        path
    }
    #[test]
    fn managed_projection_owns_exact_genesis_bindings_and_paths() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let chain_id = ChainId::from("managed-projection-fixture");
        let genesis_key = KeyPair::try_random().expect("generate genesis key");
        let chain_discriminant = iroha_config::parameters::defaults::common::chain_discriminant();
        let config_path = write_node_config(
            directory.path(),
            &chain_id,
            chain_discriminant,
            genesis_key.public_key(),
            CONFIGURED_HASH,
        );
        let projected = ManagedNodeConfig::from_path(&config_path).expect("project config");
        assert_eq!(projected.chain_id, chain_id);
        assert_eq!(projected.chain_discriminant, chain_discriminant);
        assert_eq!(
            projected.genesis_public_key,
            genesis_key.public_key().clone()
        );
        assert_eq!(projected.trusted_peer_pops.len(), 4);
        assert_eq!(
            projected.genesis_block_path,
            directory.path().join("managed/genesis.signed.nrt")
        );
        assert_eq!(
            projected.genesis_manifest_path,
            directory.path().join("genesis.json")
        );
        assert_eq!(
            projected.managed_paths.kura_store_dir,
            directory.path().join("managed/kura")
        );
        assert_eq!(
            projected.managed_paths.snapshot_store_dir,
            directory.path().join("managed/snapshot")
        );
        assert_eq!(
            projected.managed_paths.torii_data_dir,
            PathBuf::from("managed/torii")
        );
        assert_eq!(
            projected.managed_paths.torii_da_replay_cache_store_dir,
            PathBuf::from("managed/torii/da-replay")
        );
        assert_eq!(
            projected.managed_paths.torii_da_manifest_store_dir,
            PathBuf::from("managed/torii/da-manifests")
        );
        assert_eq!(
            projected.managed_paths.torii_sorafs_storage_data_dir,
            PathBuf::from("managed/sorafs")
        );
        assert_eq!(
            projected.managed_paths.streaming_session_store_dir,
            PathBuf::from("managed/streaming")
        );
        assert_eq!(
            projected
                .managed_paths
                .streaming_soranet_provision_spool_dir,
            PathBuf::from("managed/streaming/soranet")
        );
        assert_eq!(
            projected
                .managed_paths
                .streaming_soravpn_provision_spool_dir,
            PathBuf::from("managed/streaming/soravpn")
        );
        assert_eq!(
            projected.managed_paths.streaming_rans_tables_path,
            directory.path().join("managed/rans_tables.toml")
        );
        assert_eq!(
            projected.managed_paths.soranet_pow_revocation_store_path,
            PathBuf::from("managed/soranet/revocations.norito")
        );
        let (parsed, replaced) = load_node_config(&config_path, false).expect("parse config");
        assert!(!replaced);
        assert_eq!(
            projected.da_proof_policies,
            iroha_core::da::proof_policy_bundle(&parsed.nexus.lane_config)
        );
        assert_eq!(
            projected.genesis_confidential_policy_hash,
            iroha_core::state::compute_genesis_confidential_policy_hash(&parsed.zk)
        );
    }
    #[test]
    fn signing_accepts_only_selected_manifest_and_exact_unresolved_sentinel() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let chain_id = ChainId::from("managed-signing-fixture");
        let (manifest, genesis_key) = prepared_manifest(chain_id.clone());
        let manifest_path = directory.path().join("genesis.json");
        fs::write(
            &manifest_path,
            norito::json::to_json_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        let config_path = write_node_config(
            directory.path(),
            &chain_id,
            manifest.chain_discriminant(),
            genesis_key.public_key(),
            UNRESOLVED_GENESIS_EXPECTED_HASH,
        );
        let (parsed, replaced) =
            load_node_config(&config_path, true).expect("parse sentinel config");
        assert!(replaced);
        let expected_da = iroha_core::da::proof_policy_bundle(&parsed.nexus.lane_config);
        let expected_confidential =
            iroha_core::state::compute_genesis_confidential_policy_hash(&parsed.zk);
        let error = sign_prepared_genesis_from_config(
            &manifest_path,
            &config_path,
            &genesis_key,
            Some(SumeragiConsensusMode::Npos),
        )
        .expect_err("unexpected consensus mode must fail closed");
        assert!(error.to_string().contains("differs from expected mode"));
        let block = sign_prepared_genesis_from_config(
            &manifest_path,
            &config_path,
            &genesis_key,
            Some(SumeragiConsensusMode::Permissioned),
        )
        .expect("sign selected manifest");
        assert!(
            fs::read_to_string(&config_path)
                .expect("read original config")
                .contains(UNRESOLVED_GENESIS_EXPECTED_HASH),
            "signing must not rewrite the unresolved sentinel on disk"
        );
        assert_eq!(block.da_proof_policies(), Some(&expected_da));
        assert_eq!(
            block
                .header()
                .confidential_features()
                .and_then(|features| features.zk_policy_hash),
            Some(expected_confidential)
        );
        let wire = block.encode_wire().expect("encode signed block");
        let validated = validate_prepared_genesis_for_startup(
            &wire,
            &manifest,
            genesis_key.public_key(),
            block.hash(),
            &chain_id,
        )
        .expect("run composed startup validation");
        assert_eq!(validated.block(), &block);
        let other_manifest = directory.path().join("other-genesis.json");
        fs::copy(&manifest_path, &other_manifest).expect("copy manifest");
        let error = sign_prepared_genesis_from_config(
            &other_manifest,
            &config_path,
            &genesis_key,
            Some(SumeragiConsensusMode::Permissioned),
        )
        .expect_err("unselected manifest path must fail closed");
        assert!(
            error
                .to_string()
                .contains("differs from configured manifest")
        );
    }
    #[test]
    fn resolved_hash_and_expected_chain_are_enforced() {
        let directory = tempfile::tempdir().expect("create temporary directory");
        let chain_id = ChainId::from("managed-binding-fixture");
        let (manifest, genesis_key) = prepared_manifest(chain_id.clone());
        let manifest_path = directory.path().join("genesis.json");
        fs::write(
            &manifest_path,
            norito::json::to_json_pretty(&manifest).expect("serialize manifest"),
        )
        .expect("write manifest");
        let config_path = write_node_config(
            directory.path(),
            &chain_id,
            manifest.chain_discriminant(),
            genesis_key.public_key(),
            CONFIGURED_HASH,
        );
        let error = sign_prepared_genesis_from_config(
            &manifest_path,
            &config_path,
            &genesis_key,
            Some(SumeragiConsensusMode::Permissioned),
        )
        .expect_err("resolved hash must bind the produced block");
        assert!(error.to_string().contains("configuration requires"));
        let near_miss = format!("{UNRESOLVED_GENESIS_EXPECTED_HASH} ");
        let config_path = write_node_config(
            directory.path(),
            &chain_id,
            manifest.chain_discriminant(),
            genesis_key.public_key(),
            &near_miss,
        );
        let error = sign_prepared_genesis_from_config(
            &manifest_path,
            &config_path,
            &genesis_key,
            Some(SumeragiConsensusMode::Permissioned),
        )
        .expect_err("a near-miss unresolved sentinel must not be substituted");
        assert!(error.to_string().contains("parse node configuration"));
        let error = validate_prepared_genesis_for_startup(
            &[],
            &manifest,
            genesis_key.public_key(),
            HashOf::from_untyped_unchecked(Hash::new(b"unused expected hash")),
            &ChainId::from("other-chain"),
        )
        .expect_err("wrong chain must fail before bundle decoding");
        assert!(error.to_string().contains("differs from expected chain"));
    }
}
