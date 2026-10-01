//! Fresh operator publication of independently selected parent bootstrap authority.
//!
//! Public policy and canonical native parent material are validated before opening private
//! signing custody. The output is one atomically published owner-private directory; installation
//! and HTTPS distribution remain explicit operator actions.

use std::{
    io::{BufWriter, Write},
    path::PathBuf,
    str::FromStr,
};

use clap::Args as ClapArgs;
use color_eyre::eyre::{WrapErr as _, bail, ensure};
use iroha_crypto::{ExposedPrivateKey, Hash, HashOf, KeyPair, PublicKey};
use iroha_data_model::{
    NetworkId,
    account::AccountId,
    asset::AssetDefinitionId,
    block::{BlockHeader, consensus::SumeragiRootScope},
    sumeragi_finality::{
        MAX_FINALITY_BLOCK_BYTES, MAX_FINALITY_CHECKPOINT_BYTES, SumeragiFinalityCheckpoint,
    },
};
use iroha_deploy::bootstrap::{
    InstalledNetworkProfile, InstalledNetworkProfiles, NETWORK_PROFILES_FILENAME, NetworkRelease,
    ReleaseBuildRegistry, ReleaseFaucet, ReleasePeer, SignedNetworkCheckpoint,
};
use iroha_fs::OwnerDirectory;
use iroha_model_base::peer::PeerId;
use iroha_primitives::numeric::Quantity;
use norito::derive::{JsonDeserialize, JsonSerialize};

use crate::{Outcome, RunArgs};

const MAX_DEFINITION_BYTES: usize = 64 * 1024;
const MAX_RELEASE_KEY_BYTES: usize = 4096;

/// Explicit operator inputs; no endpoint response can select any trust field.
#[derive(Debug, ClapArgs)]
pub(crate) struct Args {
    /// Closed public policy JSON, including independent native checkpoint hash and release key
    #[arg(long)]
    definition: PathBuf,
    /// Independently authenticated canonical native finality checkpoint
    #[arg(long)]
    checkpoint: PathBuf,
    /// Original canonical signed parent genesis
    #[arg(long)]
    genesis: PathBuf,
    /// Owner-private Ed25519 release key in canonical Kagami multihash form
    #[arg(long)]
    release_key: PathBuf,
    /// Fresh publication directory; an existing destination is refused
    #[arg(long)]
    out_dir: PathBuf,
}

#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct Definition {
    schema_version: u32,
    network_name: String,
    serial: u64,
    generation: u64,
    network_id: NetworkId,
    chain_id: String,
    account_chain_discriminant: u16,
    native_world_schema: Hash,
    issued_at_ms: u64,
    expires_at_ms: u64,
    torii_roots: Vec<String>,
    peers: Vec<PeerDefinition>,
    faucet: Option<FaucetDefinition>,
    build_registry: Option<RegistryDefinition>,
    checkpoint_hash: Hash,
    checkpoint_height: u64,
    checkpoint_block_hash: Hash,
    genesis_public_key: PublicKey,
    release_public_key: PublicKey,
    minimum_serial: u64,
    checkpoint_url: String,
}

#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct PeerDefinition {
    node_id: PeerId,
    torii_root: String,
}

#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct FaucetDefinition {
    torii_root: String,
    issuer: AccountId,
    asset_definition_id: AssetDefinitionId,
    amount: Quantity,
    max_operation_fee: Quantity,
    max_namespace_rent: Quantity,
}

#[derive(Clone, Debug, JsonDeserialize, JsonSerialize)]
#[norito(deny_unknown_fields)]
struct RegistryDefinition {
    torii_roots: Vec<String>,
}

impl Definition {
    fn release(&self) -> NetworkRelease {
        NetworkRelease {
            network_name: self.network_name.clone(),
            serial: self.serial,
            generation: self.generation,
            network_id: self.network_id,
            chain_id: self.chain_id.clone(),
            account_chain_discriminant: self.account_chain_discriminant,
            native_world_schema: self.native_world_schema,
            issued_at_ms: self.issued_at_ms,
            expires_at_ms: self.expires_at_ms,
            torii_roots: self.torii_roots.clone(),
            peers: self
                .peers
                .iter()
                .map(|peer| ReleasePeer {
                    node_id: peer.node_id.clone(),
                    torii_root: peer.torii_root.clone(),
                })
                .collect(),
            faucet: self.faucet.as_ref().map(|policy| ReleaseFaucet {
                torii_root: policy.torii_root.clone(),
                issuer: policy.issuer.clone(),
                asset_definition_id: policy.asset_definition_id.clone(),
                amount: policy.amount.clone(),
                max_operation_fee: policy.max_operation_fee.clone(),
                max_namespace_rent: policy.max_namespace_rent.clone(),
            }),
            build_registry: self
                .build_registry
                .as_ref()
                .map(|policy| ReleaseBuildRegistry {
                    torii_roots: policy.torii_roots.clone(),
                }),
            checkpoint_hash: self.checkpoint_hash,
            checkpoint_height: self.checkpoint_height,
            checkpoint_block_hash: self.checkpoint_block_hash,
        }
    }

    fn profile(&self) -> color_eyre::Result<InstalledNetworkProfile> {
        ensure!(
            self.schema_version == 1,
            "network bootstrap definition requires schema_version 1"
        );
        ensure!(
            self.minimum_serial <= self.serial,
            "installation rollback floor exceeds publication serial"
        );
        Ok(InstalledNetworkProfile::new(
            self.network_name.clone(),
            self.release_public_key.clone(),
            self.minimum_serial,
            self.checkpoint_url.clone(),
        )?)
    }

    fn validate_public(
        &self,
        checkpoint_bytes: &[u8],
        genesis_bytes: &[u8],
    ) -> color_eyre::Result<(
        NetworkRelease,
        SumeragiFinalityCheckpoint,
        InstalledNetworkProfile,
    )> {
        let profile = self.profile()?;
        ensure!(
            Hash::new(checkpoint_bytes) == self.checkpoint_hash,
            "native checkpoint differs from independently selected hash"
        );
        let checkpoint = SumeragiFinalityCheckpoint::decode_canonical(checkpoint_bytes)?;
        let (genesis_hash, metadata) = iroha_core::release_identity::genesis_identity(
            genesis_bytes,
            &self.genesis_public_key,
        )?;
        ensure!(
            metadata.sumeragi_context.root_scope == SumeragiRootScope::Global,
            "parent bootstrap requires signed Global root genesis"
        );
        ensure!(
            NetworkId::from_genesis_hash(HashOf::<BlockHeader>::from_untyped_unchecked(
                genesis_hash
            )) == self.network_id,
            "selected signed genesis belongs to another network"
        );
        let release = self.release();
        release.validate_checkpoint(&checkpoint)?;
        Ok((release, checkpoint, profile))
    }
}

impl<T: Write> RunArgs<T> for Args {
    fn run(self, writer: &mut BufWriter<T>) -> Outcome {
        let definition_bytes = iroha_fs::read_regular(&self.definition, MAX_DEFINITION_BYTES)?;
        let definition: Definition = norito::json::from_slice(&definition_bytes)
            .wrap_err("invalid closed network bootstrap definition")?;
        let checkpoint_bytes =
            iroha_fs::read_regular(&self.checkpoint, MAX_FINALITY_CHECKPOINT_BYTES)?;
        let genesis_bytes = iroha_fs::read_regular(&self.genesis, MAX_FINALITY_BLOCK_BYTES)?;
        let (release, checkpoint, profile) =
            definition.validate_public(&checkpoint_bytes, &genesis_bytes)?;
        let now_ms = u64::try_from(
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)?
                .as_millis(),
        )?;
        ensure!(
            definition.issued_at_ms <= now_ms && now_ms < definition.expires_at_ms,
            "bootstrap publication is outside its explicit validity window"
        );
        // Invalid public policy never opens secret custody. The key is runtime-only and is
        // checked against the separately selected public release authority before signing.
        let key_bytes = iroha_fs::read_private(&self.release_key, MAX_RELEASE_KEY_BYTES)?;
        let key_text = std::str::from_utf8(&key_bytes).wrap_err("release key is not UTF-8")?;
        let key_text = key_text.strip_suffix('\n').unwrap_or(key_text);
        let private =
            ExposedPrivateKey::from_str(key_text).wrap_err("invalid release key multihash")?;
        let pair = KeyPair::from_private_key(private.0).wrap_err("invalid release key")?;
        ensure!(
            pair.public_key() == &definition.release_public_key,
            "release key differs from independently selected authority"
        );
        let artifact = SignedNetworkCheckpoint::sign(release, &checkpoint, pair.private_key())?
            .encode_canonical()?;
        let profiles = InstalledNetworkProfiles::new(vec![profile])?.encode_installation()?;
        let canonical_definition = norito::json::to_vec(&definition)?;
        // Revalidate immutable public input custody after signing and before publication.
        for (path, original, bound) in [
            (
                &self.definition,
                definition_bytes.as_slice(),
                MAX_DEFINITION_BYTES,
            ),
            (
                &self.checkpoint,
                checkpoint_bytes.as_slice(),
                MAX_FINALITY_CHECKPOINT_BYTES,
            ),
            (
                &self.genesis,
                genesis_bytes.as_slice(),
                MAX_FINALITY_BLOCK_BYTES,
            ),
        ] {
            ensure!(
                iroha_fs::read_regular(path, bound)?.as_slice() == original,
                "bootstrap public input changed before publication"
            );
        }
        let parent = self
            .out_dir
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
            .unwrap_or_else(|| std::path::Path::new("."));
        let Some(name) = self.out_dir.file_name() else {
            bail!("bootstrap output requires a fresh directory name");
        };
        let custody = OwnerDirectory::open_or_create(parent)?;
        let published = custody.publish_private_child(
            name,
            &[
                ("network-checkpoint.nrt", &artifact),
                (NETWORK_PROFILES_FILENAME, &profiles),
                ("definition.json", &canonical_definition),
                ("genesis.signed.nrt", &genesis_bytes),
                ("native-checkpoint.nrt", &checkpoint_bytes),
            ],
        )?;
        writeln!(
            writer,
            "Published network bootstrap: {}",
            published.path().display()
        )?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
