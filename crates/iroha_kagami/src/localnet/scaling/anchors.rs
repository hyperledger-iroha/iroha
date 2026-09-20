//! Original genesis anchors derived by the fixed generator under retained file custody.
use super::*;
use iroha_core::sumeragi::GenesisMergeAuthority;
use iroha_data_model::block::consensus_v2::HeightContextId;

pub(in crate::localnet) const RECEIPT_FILE: &str = "genesis-anchors.json";
const MAX_RECEIPT_BYTES: usize = 32 * 1024;
const MAX_MANIFEST_BYTES: u64 = 16 * 1024 * 1024;

/// Final generated originals, including the non-serializable Core authority.
pub(in crate::localnet) struct Inputs {
    files: crate::secure_fs::RetainedPrivateFiles,
    root: PathBuf,
    manifest: usize,
    signed: usize,
    expected_hash: HashOf<BlockHeader>,
    authority: Option<GenesisMergeAuthority>,
    checked_peers: usize,
    primary_paths: Vec<(String, String)>,
    genesis_parameters: Option<(iroha_crypto::PublicKey, u16)>,
}

#[derive(norito::derive::JsonSerialize)]
struct Artifact {
    path: String,
    sha256: String,
    bytes: u64,
}
#[derive(norito::derive::JsonSerialize)]
struct OriginalPeer {
    role: String,
    node_public_key: iroha_crypto::PublicKey,
    torii_url: String,
    config: String,
    client_config: String,
    primary_block_store: String,
    primary_merge_log: String,
}
#[derive(norito::derive::JsonSerialize)]
struct OriginalAccount {
    index: usize,
    account_id: AccountId,
    config: String,
}
#[derive(norito::derive::JsonSerialize)]
struct Receipt {
    schema: &'static str,
    version: u8,
    consensus_mode: &'static str,
    chain_id: String,
    genesis_public_key: iroha_crypto::PublicKey,
    chain_discriminant: u16,
    lane_count: u16,
    genesis_hash: HashOf<BlockHeader>,
    context_id: HeightContextId,
    network_id: NetworkId,
    peers: Vec<OriginalPeer>,
    accounts: Vec<OriginalAccount>,
    artifacts: Vec<Artifact>,
}

impl Inputs {
    /// Capture the final generated genesis before admitting any final peer config.
    pub(in crate::localnet) fn new(
        root: &Path,
        expected_hash: HashOf<BlockHeader>,
    ) -> Result<Self> {
        let mut files = crate::secure_fs::RetainedPrivateFiles::new(root)?;
        files.capture_directory("storage")?;
        for index in 0..4 {
            files.capture_directory(&format!("storage/peer{index}"))?;
            files.capture_directory(&format!("storage/peer{index}/kura"))?;
            files.capture_directory(&format!("storage/peer{index}/state"))?;
        }
        let manifest = files.capture("genesis.json", MAX_MANIFEST_BYTES, None)?;
        let signed = files.capture(
            "genesis.signed.nrt",
            SIGNED_GENESIS_MAX_BYTES_V1 as u64,
            None,
        )?;
        let network_record = format!("{}\n", NetworkId::from_genesis_hash(expected_hash));
        files.capture(
            GENESIS_EXPECTED_HASH_FILE,
            1024,
            Some(network_record.as_bytes()),
        )?;
        Ok(Self {
            files,
            root: root.to_owned(),
            manifest,
            signed,
            expected_hash,
            authority: None,
            checked_peers: 0,
            primary_paths: Vec::new(),
            genesis_parameters: None,
        })
    }

    /// Bind a just-written artifact to its exact producer-owned bytes.
    pub(in crate::localnet) fn retain(&mut self, name: &str, bytes: &[u8]) -> Result<()> {
        self.files.capture(name, 1024 * 1024, Some(bytes))?;
        Ok(())
    }

    /// Authenticate the same final genesis under every original peer's effective config.
    pub(in crate::localnet) fn stage_peer(
        &mut self,
        config: &actual::Root,
        peers: &[Peer],
        index: usize,
    ) -> Result<Vec<u8>> {
        self.files.check()?;
        ensure!(
            peers.len() == 4 && index == self.checked_peers && index < 4,
            "fixed anchor peer roles are incomplete or reordered"
        );
        ensure!(
            config.genesis.expected_hash == self.expected_hash
                && config.common.key_pair.public_key() == &peers[index].public_key,
            "fixed anchor config does not bind its original peer and genesis"
        );
        let manifest = RawGenesisTransaction::from_json_slice_at_path(
            self.files.bytes(self.manifest)?,
            self.root.join("genesis.json"),
        )
        .map_err(|_| eyre!("retained final genesis manifest is invalid"))?;
        let authority = crate::genesis::staged_signed_genesis_merge_authority(
            &manifest,
            self.files.bytes(self.signed)?,
            config,
        )
        .map_err(|_| eyre!("retained final genesis authority is invalid"))?;
        let expected = peers
            .iter()
            .map(|peer| PeerId::new(peer.public_key.clone()))
            .collect::<BTreeSet<_>>();
        let actual = authority
            .context()
            .roster
            .iter()
            .map(|entry| entry.validator.clone())
            .collect::<BTreeSet<_>>();
        ensure!(
            expected.len() == 4
                && actual == expected
                && peers.iter().all(|peer| matches!(
                    peer.public_key.try_algorithm(),
                    Ok(iroha_crypto::Algorithm::BlsNormal)
                )),
            "fixed anchor committee differs from the original four BLS peers"
        );
        ensure!(
            authority.context().network_id == NetworkId::from_genesis_hash(self.expected_hash),
            "fixed anchor context has a foreign genesis network"
        );
        if let Some(original) = &self.authority {
            ensure!(
                authority.context() == original.context()
                    && authority.proofs_of_possession() == original.proofs_of_possession()
                    && authority.catalog_hash() == original.catalog_hash()
                    && authority.active_lanes() == original.active_lanes()
                    && authority.lane_authority_catalog() == original.lane_authority_catalog(),
                "original peer configs disagree on final genesis authority"
            );
        }
        let genesis_parameters = (
            config.genesis.public_key.clone(),
            *config.common.chain_discriminant.value(),
        );
        if let Some(original) = &self.genesis_parameters {
            ensure!(
                original == &genesis_parameters,
                "fixed peers disagree on effective genesis parameters"
            );
        }
        let primary_paths = canonical_reader_paths(config)?;
        let bytes = super::encode_genesis_context(&authority)?;
        if self.authority.is_none() {
            self.authority = Some(authority);
        }
        if self.genesis_parameters.is_none() {
            self.genesis_parameters = Some(genesis_parameters);
        }
        self.primary_paths.push(primary_paths);
        self.checked_peers += 1;
        self.files.check()?;
        Ok(bytes)
    }

    /// Reopen only exact producer-pinned node bytes and the already published context.
    pub(in crate::localnet) fn authenticate(
        &mut self,
        peers: &[Peer],
        config_digests: &[[u8; 32]],
        context: &[u8],
    ) -> Result<()> {
        ensure!(
            config_digests.len() == 4,
            "fixed anchor config identities are incomplete"
        );
        for (index, digest) in config_digests.iter().enumerate() {
            let name = format!("peer{index}.toml");
            let captured = self.files.capture(&name, 1024 * 1024, None)?;
            let bytes = self.files.bytes(captured)?;
            ensure!(
                iroha_crypto::sha256(bytes) == *digest,
                "fixed peer config changed after generation"
            );
            let text = std::str::from_utf8(bytes)
                .map_err(|_| eyre!("generated peer config is not UTF-8"))?;
            let config = parse_localnet_peer_config(text, Some(&self.root.join(&name)))?;
            super::runtime_paths::validate(
                &config,
                &LocalnetPeerStoragePaths::scaling(&self.root, index),
            )?;
            ensure!(
                self.stage_peer(&config, peers, index)? == context,
                "fixed published context differs from final peer authority"
            );
        }
        self.retain(GENESIS_CONTEXT_FILE, context)
    }

    /// Publish the public anchor receipt after all generated clients are retained.
    #[allow(clippy::too_many_arguments)]
    pub(in crate::localnet) fn publish<T: Write>(
        mut self,
        writer: &mut BufWriter<T>,
        layout: ScalingLayout,
        peers: &[Peer],
        accounts: &[LocalnetClientIdentity],
        host: &CanonicalHost,
        chain_id: &str,
        client_digest: [u8; 32],
        account_digests: &[[u8; 32]],
    ) -> Result<()> {
        ensure!(
            self.checked_peers == 4 && self.primary_paths.len() == 4 && peers.len() == 4,
            "fixed anchors lack all original peers"
        );
        layout.validate_accounts(accounts)?;
        ensure!(
            !chain_id.is_empty() && chain_id.len() <= 1024,
            "fixed anchor chain label exceeds its bound"
        );
        ensure!(
            account_digests.len() == accounts.len(),
            "fixed original account digests are incomplete"
        );
        let client = self.files.capture("client.toml", 1024 * 1024, None)?;
        ensure!(
            iroha_crypto::sha256(self.files.bytes(client)?) == client_digest,
            "fixed client config changed after generation"
        );
        for (index, peer) in peers.iter().enumerate() {
            let text = std::str::from_utf8(self.files.bytes(client)?)
                .map_err(|_| eyre!("generated client is not UTF-8"))?;
            let mut table = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
                text,
                "generated anchor client",
            )?);
            ensure!(
                !table.contains_key("extends"),
                "generated anchor client cannot inherit configuration"
            );
            crate::secret_toml::remove(&mut table, "network_id_file");
            crate::secret_toml::insert(
                &mut table,
                "network_id".to_owned(),
                Value::String(NetworkId::from_genesis_hash(self.expected_hash).to_string()),
            );
            crate::secret_toml::insert(
                &mut table,
                "torii_url".to_owned(),
                Value::String(host.torii_url(peer.api_port)),
            );
            let rendered = Zeroizing::new(
                toml::to_string(&*table).map_err(|_| eyre!("render fixed peer client failed"))?,
            );
            let name = format!("peer{index}-client.toml");
            self.files
                .write_new(&name, rendered.as_bytes(), 1024 * 1024)?;
        }
        for (index, account) in accounts.iter().enumerate() {
            let name = account_config_name(index);
            let captured = self.files.capture(&name, 1024 * 1024, None)?;
            ensure!(
                iroha_crypto::sha256(self.files.bytes(captured)?) == account_digests[index],
                "fixed workload client changed after generation"
            );
            let text = std::str::from_utf8(self.files.bytes(captured)?)
                .map_err(|_| eyre!("generated workload client is not UTF-8"))?;
            let table = crate::secret_toml::Table::new(crate::secret_toml::parse_table(
                text,
                "generated workload client",
            )?);
            ensure!(
                table
                    .get("account")
                    .and_then(Value::as_table)
                    .and_then(|value| value.get("public_key"))
                    .and_then(Value::as_str)
                    == Some(account.public_key.to_string().as_str())
                    && !table.contains_key("extends")
                    && table.get("network_id_file").and_then(Value::as_str)
                        == Some(GENESIS_EXPECTED_HASH_FILE),
                "generated workload client differs from the original account or network source"
            );
        }
        let authority = self
            .authority
            .as_ref()
            .ok_or_else(|| eyre!("fixed anchors lack signed genesis authority"))?;
        let (genesis_public_key, chain_discriminant) = self
            .genesis_parameters
            .as_ref()
            .ok_or_else(|| eyre!("fixed anchors lack effective genesis parameters"))?;
        let receipt = Receipt {
            schema: "iroha.sumeragi_v2.scaling.genesis_anchors.v1",
            version: 1,
            consensus_mode: "npos",
            chain_id: chain_id.to_owned(),
            genesis_public_key: genesis_public_key.clone(),
            chain_discriminant: *chain_discriminant,
            lane_count: layout.lane_count(),
            genesis_hash: self.expected_hash,
            context_id: authority.context().id(),
            network_id: authority.context().network_id,
            peers: peers
                .iter()
                .enumerate()
                .map(|(index, peer)| OriginalPeer {
                    role: format!("peer{index}"),
                    node_public_key: peer.public_key.clone(),
                    torii_url: host.torii_url(peer.api_port),
                    config: format!("peer{index}.toml"),
                    client_config: format!("peer{index}-client.toml"),
                    primary_block_store: self.primary_paths[index].0.clone(),
                    primary_merge_log: self.primary_paths[index].1.clone(),
                })
                .collect(),
            accounts: accounts
                .iter()
                .enumerate()
                .map(|(index, account)| OriginalAccount {
                    index,
                    account_id: account.account_id.clone(),
                    config: account_config_name(index),
                })
                .collect(),
            artifacts: self
                .files
                .identities()
                .map(|(name, digest, count)| Artifact {
                    path: name.to_owned(),
                    sha256: hex::encode(digest),
                    bytes: count,
                })
                .collect(),
        };
        let mut bytes =
            norito::json::to_json_bounded(&receipt, MAX_RECEIPT_BYTES - 1)?.into_bytes();
        bytes.push(b'\n');
        ensure!(
            bytes.len() <= MAX_RECEIPT_BYTES,
            "fixed anchor receipt exceeds its 32-KiB bound"
        );
        self.files
            .write_new(RECEIPT_FILE, &bytes, MAX_RECEIPT_BYTES as u64)?;
        self.files.seal_namespace()?;
        self.files.check()?;
        writer.write_all(&bytes)?;
        writer.flush()?;
        self.files.check()
    }
}

/// Resolve the reader's chain-scoped canonical namespace from final effective config.
/// The generated Kura root starts empty; these descendants are created by the daemon.
fn canonical_reader_paths(config: &actual::Root) -> Result<(String, String)> {
    let root = config.kura.store_dir.value();
    let (blocks, merge) = iroha_core::kura::Kura::canonical_storage_paths(root);
    let encode = |path: &Path| -> Result<String> {
        ensure!(
            path.is_absolute() && path != root && path.starts_with(root),
            "fixed canonical reader path escapes its original Kura root"
        );
        let value = path
            .to_str()
            .ok_or_else(|| eyre!("fixed canonical reader path is not UTF-8"))?;
        ensure!(
            value.len() <= 4096,
            "fixed canonical reader path exceeds its bound"
        );
        Ok(value.to_owned())
    };
    ensure!(blocks != merge, "fixed canonical reader paths alias");
    Ok((encode(&blocks)?, encode(&merge)?))
}

#[cfg(test)]
mod tests;
