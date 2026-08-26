//! This module contains [`State`] snapshot actor service.
#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    kura::{BlockCount, CommitManifestBindingState, Error as KuraError, Kura},
    query::store::LiveQueryStoreHandle,
    secure_file_metadata::{self, SecureMetadata},
    state::{
        LaneIncarnationLineage, SnapshotNexusRuntime, SnapshotNoritoBlob,
        SnapshotPublicLaneRewardClaim, SnapshotSpaceDirectoryManifestSet, State, StateBlock,
        ValidatedSccpRegistryV1, WorldReadOnly, ZkConfigInstallError, deserialize::KuraSeed,
        lane_incarnation_lineage_root, public_lane_reward_record_matches_key,
        public_lane_stake_share_matches_key, public_lane_validator_record_matches_key,
    },
};
use blake2::{Blake2b, digest::consts::U32};
use hex;
use iroha_config::{
    parameters::actual::{Snapshot as Config, SnapshotBootstrapPolicy, SnapshotResourcePolicy},
    snapshot::Mode,
};
use iroha_crypto::{
    Algorithm, CompactMerkleProof, Hash, HashOf, KeyPair, MerkleTree, MerkleTreeCommitment,
    PublicKey, Signature,
};
use iroha_data_model::{
    ChainId, NetworkId,
    account::AccountId,
    asset::AssetId,
    block::{BlockHeader, consensus_v2::SnapshotV2BootstrapRecord},
    bridge::SccpRegistryV1,
    nexus::{LaneCatalog, LaneId},
    state_path::StatePath,
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_logger::prelude::*;
use mv::{
    cell::Cell,
    storage::{Storage, StorageReadOnly},
};
use norito::codec::{DecodeAll, Encode as NoritoEncode};
use norito::json::{self, JsonSerialize, JsonSerialize as JsonSerializeTrait};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Seek, Write},
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};
fn serialize_state_snapshot(state: &State, out: &mut String) {
    let view = state.view();
    let block_hashes: Vec<HashOf<BlockHeader>> = view.block_hashes.iter().copied().collect();
    let nexus_runtime = SnapshotNexusRuntime::from_nexus_with_autoscale_history(
        &view.nexus,
        &view.lane_incarnations,
        &view.lane_incarnation_activation_heights,
        &view.autoscale_sample_history,
        &view.lane_incarnation_lineage,
    );
    let public_lane_validators: Vec<_> = view
        .world
        .public_lane_validators
        .iter()
        .filter_map(|(key, value)| {
            public_lane_validator_record_matches_key(key, value).then(|| SnapshotNoritoBlob {
                encoded_hex: hex::encode(NoritoEncode::encode(value)),
            })
        })
        .collect();
    let public_lane_stake_shares: Vec<_> = view
        .world
        .public_lane_stake_shares
        .iter()
        .filter_map(|(key, value)| {
            public_lane_stake_share_matches_key(key, value).then(|| SnapshotNoritoBlob {
                encoded_hex: hex::encode(NoritoEncode::encode(value)),
            })
        })
        .collect();
    let public_lane_rewards: Vec<_> = view
        .world
        .public_lane_rewards
        .iter()
        .filter_map(|(key, value)| {
            public_lane_reward_record_matches_key(key, value).then(|| SnapshotNoritoBlob {
                encoded_hex: hex::encode(NoritoEncode::encode(value)),
            })
        })
        .collect();
    let public_lane_reward_claims: Vec<_> = view
        .world
        .public_lane_reward_claims
        .iter()
        .map(
            |(key, last_claimed_epoch): (&(LaneId, AccountId, AssetId), &u64)| {
                let (lane_id, account, asset) = key;
                SnapshotPublicLaneRewardClaim {
                    lane_id: *lane_id,
                    account: account.clone(),
                    asset: asset.clone(),
                    last_claimed_epoch: *last_claimed_epoch,
                }
            },
        )
        .collect();
    let space_directory_manifests: Vec<_> = view
        .world
        .space_directory_manifests
        .iter()
        .map(|(uaid, value)| SnapshotSpaceDirectoryManifestSet {
            uaid: *uaid,
            encoded_hex: hex::encode(NoritoEncode::encode(value)),
        })
        .collect();
    out.push('{');
    json::write_json_string("chain_id", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&state.chain_id, out);
    out.push(',');
    json::write_json_string("network_id", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&state.network_id, out);
    // Preserve the original authenticated bootstrap lineage in every later
    // snapshot.  The canonical WSV hash redacts this envelope, so carrying the
    // immutable trust root forward cannot make the anchor hash circular.
    if let Some(bootstrap) = state.authenticated_snapshot_v2_bootstrap() {
        out.push(',');
        json::write_json_string("sumeragi_v2_bootstrap", out);
        out.push(':');
        json::JsonSerialize::json_serialize(bootstrap, out);
    }
    out.push(',');
    json::write_json_string("world", out);
    out.push(':');
    state.world.json_serialize(out);
    out.push(',');
    json::write_json_string("nexus_runtime", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&nexus_runtime, out);
    out.push(',');
    json::write_json_string("block_hashes", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&block_hashes, out);
    out.push(',');
    json::write_json_string("transactions", out);
    out.push(':');
    state.transactions.json_serialize(out);
    out.push(',');
    json::write_json_string("public_lane_validators", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_validators, out);
    out.push(',');
    json::write_json_string("public_lane_stake_shares", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_stake_shares, out);
    out.push(',');
    json::write_json_string("public_lane_rewards", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_rewards, out);
    out.push(',');
    json::write_json_string("public_lane_reward_claims", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_reward_claims, out);
    out.push(',');
    json::write_json_string("space_directory_manifests", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&space_directory_manifests, out);
    out.push(',');
    json::write_json_string("commit_topology", out);
    out.push(':');
    state.commit_topology.json_serialize(out);
    out.push(',');
    json::write_json_string("prev_commit_topology", out);
    out.push(':');
    state.prev_commit_topology.json_serialize(out);
    out.push('}');
}
fn serialize_staged_state_snapshot(state: &StateBlock<'_>, out: &mut String) {
    let world = state.world();
    let block_hashes: Vec<HashOf<BlockHeader>> = state.block_hashes().iter().copied().collect();
    let nexus_runtime = SnapshotNexusRuntime::from_nexus_with_autoscale_history(
        &state.nexus,
        &state.lane_incarnations,
        &state.lane_incarnation_activation_heights,
        state.autoscale_sample_history_for_snapshot(),
        state.lane_incarnation_lineage_for_snapshot(),
    );
    let public_lane_validators: Vec<_> = world
        .public_lane_validators()
        .iter()
        .filter_map(|(key, value)| {
            public_lane_validator_record_matches_key(key, value).then(|| SnapshotNoritoBlob {
                encoded_hex: hex::encode(NoritoEncode::encode(value)),
            })
        })
        .collect();
    let public_lane_stake_shares: Vec<_> = world
        .public_lane_stake_shares()
        .iter()
        .filter_map(|(key, value)| {
            public_lane_stake_share_matches_key(key, value).then(|| SnapshotNoritoBlob {
                encoded_hex: hex::encode(NoritoEncode::encode(value)),
            })
        })
        .collect();
    let public_lane_rewards: Vec<_> = world
        .public_lane_rewards()
        .iter()
        .filter_map(|(key, value)| {
            public_lane_reward_record_matches_key(key, value).then(|| SnapshotNoritoBlob {
                encoded_hex: hex::encode(NoritoEncode::encode(value)),
            })
        })
        .collect();
    let public_lane_reward_claims: Vec<_> = world
        .public_lane_reward_claims()
        .iter()
        .map(
            |(key, last_claimed_epoch): (&(LaneId, AccountId, AssetId), &u64)| {
                let (lane_id, account, asset) = key;
                SnapshotPublicLaneRewardClaim {
                    lane_id: *lane_id,
                    account: account.clone(),
                    asset: asset.clone(),
                    last_claimed_epoch: *last_claimed_epoch,
                }
            },
        )
        .collect();
    let space_directory_manifests: Vec<_> = world
        .space_directory_manifests()
        .iter()
        .map(|(uaid, value)| SnapshotSpaceDirectoryManifestSet {
            uaid: *uaid,
            encoded_hex: hex::encode(NoritoEncode::encode(value)),
        })
        .collect();
    out.push('{');
    json::write_json_string("chain_id", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&state.chain_id, out);
    out.push(',');
    json::write_json_string("network_id", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&state.network_id, out);
    out.push(',');
    json::write_json_string("world", out);
    out.push(':');
    world.json_serialize(out);
    out.push(',');
    json::write_json_string("nexus_runtime", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&nexus_runtime, out);
    out.push(',');
    json::write_json_string("block_hashes", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&block_hashes, out);
    out.push(',');
    json::write_json_string("transactions", out);
    out.push(':');
    state.json_serialize_transactions_after_commit(out);
    out.push(',');
    json::write_json_string("public_lane_validators", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_validators, out);
    out.push(',');
    json::write_json_string("public_lane_stake_shares", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_stake_shares, out);
    out.push(',');
    json::write_json_string("public_lane_rewards", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_rewards, out);
    out.push(',');
    json::write_json_string("public_lane_reward_claims", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&public_lane_reward_claims, out);
    out.push(',');
    json::write_json_string("space_directory_manifests", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&space_directory_manifests, out);
    out.push(',');
    json::write_json_string("commit_topology", out);
    out.push(':');
    state.commit_topology.json_serialize(out);
    out.push(',');
    json::write_json_string("prev_commit_topology", out);
    out.push(':');
    state.prev_commit_topology.json_serialize(out);
    out.push('}');
}
// Serialize State as a minimal snapshot wrapper using Norito JSON writer.
impl JsonSerializeTrait for State {
    fn json_serialize(&self, out: &mut String) {
        serialize_state_snapshot(self, out);
    }
}
/// Name of the [`State`] snapshot file.
const SNAPSHOT_FILE_NAME: &str = "snapshot.data";
/// Name of the digest accompanying the snapshot file.
const SNAPSHOT_DIGEST_FILE_NAME: &str = "snapshot.sha256";
/// Name of the signature accompanying the digest.
const SNAPSHOT_SIGNATURE_FILE_NAME: &str = "snapshot.sig";
/// Name of the bounded recovery manifest authenticated with the payload digest.
const SNAPSHOT_FAST_MANIFEST_FILE_NAME: &str = "snapshot.fast.norito";
/// Name of the Merkle metadata accompanying the snapshot file.
const SNAPSHOT_MERKLE_FILE_NAME: &str = "snapshot.merkle.json";
/// Directory containing immutable, digest-named complete generations.
const SNAPSHOT_GENERATIONS_DIR_NAME: &str = "generations";
/// Atomically replaced canonical pointer to one immutable generation.
const SNAPSHOT_CURRENT_FILE_NAME: &str = "current";
const SNAPSHOT_DIGEST_MAX_BYTES: u64 = 65;
const SNAPSHOT_CURRENT_MAX_BYTES: u64 = SNAPSHOT_DIGEST_MAX_BYTES;
const SNAPSHOT_SIGNATURE_MAX_BYTES: u64 = 16 * 1024;
const SNAPSHOT_FAST_MANIFEST_MAX_BYTES: u64 = 512;
const SNAPSHOT_FAST_MANIFEST_VERSION: u8 = 1;
const SNAPSHOT_BUNDLE_SIGNATURE_DOMAIN: &[u8] = b"iroha:snapshot-bundle:v1\0";
const SNAPSHOT_MERKLE_FIXED_OVERHEAD_BYTES: u64 = 1024;
const SNAPSHOT_MERKLE_BYTES_PER_LEAF: u64 = 80;
const SNAPSHOT_GENERATION_GC_MAX_ENTRIES: usize = 4096;
static SNAPSHOT_PUBLICATION_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());

/// Constant-size authority needed by emergency Fast startup.
#[derive(Clone, Debug, PartialEq, Eq, norito::codec::Encode, norito::codec::Decode)]
struct EmergencyFastSnapshotManifestV1 {
    version: u8,
    payload_len: u64,
    chain_id: ChainId,
    network_id: NetworkId,
    committed_height: u64,
    tip_hash: Option<HashOf<BlockHeader>>,
    sccp_policy_hash: [u8; 32],
    has_snapshot_bootstrap_lineage: bool,
}

impl EmergencyFastSnapshotManifestV1 {
    fn validate(&self) -> Result<(), String> {
        if self.version != SNAPSHOT_FAST_MANIFEST_VERSION {
            return Err(format!(
                "unsupported emergency Fast manifest version {}; expected {}",
                self.version, SNAPSHOT_FAST_MANIFEST_VERSION
            ));
        }
        if (self.committed_height == 0) != self.tip_hash.is_none() {
            return Err(
                "emergency Fast manifest height and terminal hash presence disagree".to_owned(),
            );
        }
        usize::try_from(self.committed_height).map_err(|_| {
            "emergency Fast manifest height exceeds this host's index width".to_owned()
        })?;
        Ok(())
    }
}

fn snapshot_bundle_auth_digest(payload_sha256: &[u8; 32], manifest_bytes: &[u8]) -> [u8; 32] {
    let manifest_sha256: [u8; 32] = Sha256::digest(manifest_bytes).into();
    let mut hasher = Sha256::new();
    Digest::update(&mut hasher, SNAPSHOT_BUNDLE_SIGNATURE_DOMAIN);
    Digest::update(&mut hasher, payload_sha256);
    Digest::update(&mut hasher, manifest_sha256);
    hasher.finalize().into()
}
#[cfg(test)]
std::thread_local! {
    static SNAPSHOT_GC_FAILURE_STAGE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
    static SNAPSHOT_HASH_RECONCILIATION_PASSES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static SNAPSHOT_PAYLOAD_DIGEST_PASSES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static SNAPSHOT_DEEP_VALIDATION_PASSES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
    static SNAPSHOT_BLOCK_HASH_VECTOR_CLONES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
}
#[derive(thiserror::Error, Debug, displaydoc::Display)]
enum SnapshotMerkleError {
    /// Snapshot Merkle metadata missing
    #[cfg(test)]
    Missing,
    /// Snapshot Merkle metadata IO failure
    #[cfg(test)]
    Io(#[source] std::io::Error),
    /// Snapshot Merkle metadata parse error
    Parse(#[source] norito::json::Error),
    /// Snapshot Merkle chunk size mismatch (expected `{expected}`, got `{actual}`)
    ChunkSizeMismatch {
        /// Chunk size requested by the caller.
        expected: NonZeroUsize,
        /// Chunk size advertised by the metadata.
        actual: NonZeroUsize,
    },
    /// Snapshot Merkle chunk size is invalid (`{0}` bytes)
    ChunkSizeInvalid(u64),
    /// Snapshot Merkle root mismatch (expected `{expected}`, got `{actual}`)
    RootMismatch {
        /// Root derived from metadata.
        expected: String,
        /// Root derived from the snapshot payload.
        actual: String,
    },
    /// Snapshot length mismatch (expected `{expected}` bytes, got `{actual}` bytes)
    LengthMismatch {
        /// Length recorded in metadata.
        expected: u64,
        /// Actual snapshot payload length.
        actual: u64,
    },
    /// Snapshot Merkle leaf count mismatch (expected `{expected}`, got `{actual}`)
    LeafCountMismatch {
        /// Expected number of leaves for the snapshot length and chunk size.
        expected: u64,
        /// Actual number of leaves recorded in metadata.
        actual: u64,
    },
    /// Snapshot Merkle root could not be parsed from hex
    RootHexMalformed,
    /// Snapshot Merkle leaf could not be parsed from hex
    LeafHexMalformed,
    /// Snapshot Merkle proof missing for chunk `{chunk_index}`
    ProofUnavailable {
        /// Index of the chunk whose proof was requested.
        chunk_index: usize,
    },
    /// Snapshot Merkle proof invalid for chunk `{chunk_index}` (`{reason}`)
    ProofInvalid {
        /// Index of the chunk being verified.
        chunk_index: usize,
        /// Reason the proof failed verification.
        reason: String,
    },
}
#[derive(Debug, Clone, JsonSerialize)]
struct SnapshotMerkleMetadata {
    /// Chunk size in bytes used to compute leaf digests.
    chunk_size_bytes: u64,
    /// Length of the snapshot payload in bytes.
    total_len_bytes: u64,
    /// Hex-encoded Merkle root over the chunk digests.
    root_hex: String,
    /// Hex-encoded SHA-256 digests for each chunk.
    leaf_hashes_hex: Vec<String>,
}
impl SnapshotMerkleMetadata {
    fn parse_error(message: impl Into<String>) -> SnapshotMerkleError {
        SnapshotMerkleError::Parse(norito::json::Error::Message(message.into()))
    }
    fn expect_field<'a>(
        map: &'a norito::json::Map,
        field: &'static str,
    ) -> Result<&'a norito::json::Value, SnapshotMerkleError> {
        map.get(field)
            .ok_or_else(|| SnapshotMerkleError::Parse(norito::json::Error::missing_field(field)))
    }
    fn parse_u64_field(
        map: &norito::json::Map,
        field: &'static str,
    ) -> Result<u64, SnapshotMerkleError> {
        let value = Self::expect_field(map, field)?;
        if let Some(number) = value.as_u64() {
            return Ok(number);
        }
        Err(Self::parse_error(format!("`{field}` must be a u64")))
    }
    fn parse_string_field(
        map: &norito::json::Map,
        field: &'static str,
    ) -> Result<String, SnapshotMerkleError> {
        let value = Self::expect_field(map, field)?;
        value
            .as_str()
            .map(|raw| raw.to_owned())
            .ok_or_else(|| Self::parse_error(format!("`{field}` must be a string")))
    }
    fn parse_string_vec_field(
        map: &norito::json::Map,
        field: &'static str,
    ) -> Result<Vec<String>, SnapshotMerkleError> {
        let value = Self::expect_field(map, field)?;
        if let Some(array) = value.as_array() {
            return array
                .iter()
                .enumerate()
                .map(|(index, item)| {
                    item.as_str().map(|raw| raw.to_owned()).ok_or_else(|| {
                        Self::parse_error(format!(
                            "`{field}[{index}]` must be a string (hex digest)"
                        ))
                    })
                })
                .collect();
        }
        Err(Self::parse_error(format!(
            "`{field}` must be an array of hex strings"
        )))
    }
    fn from_json_value(value: norito::json::Value) -> Result<Self, SnapshotMerkleError> {
        let map = value
            .as_object()
            .ok_or_else(|| Self::parse_error("snapshot Merkle metadata must be a JSON object"))?;
        Ok(Self {
            chunk_size_bytes: Self::parse_u64_field(map, "chunk_size_bytes")?,
            total_len_bytes: Self::parse_u64_field(map, "total_len_bytes")?,
            root_hex: Self::parse_string_field(map, "root_hex")?,
            leaf_hashes_hex: Self::parse_string_vec_field(map, "leaf_hashes_hex")?,
        })
    }
    fn from_bytes(bytes: &[u8], chunk_size: NonZeroUsize) -> Self {
        let leaf_hashes = chunk_hashes(bytes, chunk_size);
        let tree = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaf_hashes.clone());
        let root = tree
            .root()
            .expect("Merkle tree with at least one leaf must have a root");
        SnapshotMerkleMetadata {
            chunk_size_bytes: u64::try_from(chunk_size.get())
                .expect("chunk size should fit in u64 for metadata"),
            total_len_bytes: bytes
                .len()
                .try_into()
                .expect("snapshot length should fit in u64 for metadata"),
            root_hex: hex::encode(root.as_ref()),
            leaf_hashes_hex: leaf_hashes.into_iter().map(hex::encode).collect(),
        }
    }
    fn chunk_size(&self) -> Result<NonZeroUsize, SnapshotMerkleError> {
        NonZeroUsize::new(usize::try_from(self.chunk_size_bytes).unwrap_or(0))
            .ok_or(SnapshotMerkleError::ChunkSizeInvalid(self.chunk_size_bytes))
    }
    fn parse_root(&self) -> Result<HashOf<MerkleTree<[u8; 32]>>, SnapshotMerkleError> {
        let bytes =
            hex::decode(&self.root_hex).map_err(|_| SnapshotMerkleError::RootHexMalformed)?;
        if bytes.len() != Hash::LENGTH {
            return Err(SnapshotMerkleError::RootHexMalformed);
        }
        let mut arr = [0u8; Hash::LENGTH];
        arr.copy_from_slice(&bytes);
        Ok(HashOf::from_untyped_unchecked(Hash::prehashed(arr)))
    }
    fn parse_leaves(&self) -> Result<Vec<[u8; 32]>, SnapshotMerkleError> {
        self.leaf_hashes_hex
            .iter()
            .map(|leaf| {
                let bytes = hex::decode(leaf).map_err(|_| SnapshotMerkleError::LeafHexMalformed)?;
                if bytes.len() != Hash::LENGTH {
                    return Err(SnapshotMerkleError::LeafHexMalformed);
                }
                let mut arr = [0u8; Hash::LENGTH];
                arr.copy_from_slice(&bytes);
                Ok(arr)
            })
            .collect()
    }
    fn expected_leaf_count(&self, chunk_size: NonZeroUsize) -> Result<u64, SnapshotMerkleError> {
        let chunk = u64::try_from(chunk_size.get())
            .map_err(|_| SnapshotMerkleError::ChunkSizeInvalid(self.chunk_size_bytes))?;
        if self.total_len_bytes == 0 {
            return Ok(1);
        }
        Ok((self.total_len_bytes - 1) / chunk + 1)
    }
    fn tree(&self) -> Result<MerkleTree<[u8; 32]>, SnapshotMerkleError> {
        let leaves = self.parse_leaves()?;
        Ok(MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves))
    }
    fn verify_self(&self) -> Result<(), SnapshotMerkleError> {
        let chunk_size = self.chunk_size()?;
        let expected_leaves = self.expected_leaf_count(chunk_size)?;
        let actual_leaves = self.leaf_hashes_hex.len() as u64;
        if expected_leaves != actual_leaves {
            return Err(SnapshotMerkleError::LeafCountMismatch {
                expected: expected_leaves,
                actual: actual_leaves,
            });
        }
        let tree = self.tree()?;
        let Some(root) = tree.root() else {
            return Err(SnapshotMerkleError::RootHexMalformed);
        };
        let expected_root = self.parse_root()?;
        if root != expected_root {
            return Err(SnapshotMerkleError::RootMismatch {
                expected: self.root_hex.clone(),
                actual: hex::encode(root.as_ref()),
            });
        }
        Ok(())
    }
    fn verify_against_bytes(
        &self,
        bytes: &[u8],
        expected_chunk_size: NonZeroUsize,
    ) -> Result<(), SnapshotMerkleError> {
        let metadata_chunk_size = self.chunk_size()?;
        if metadata_chunk_size != expected_chunk_size {
            return Err(SnapshotMerkleError::ChunkSizeMismatch {
                expected: expected_chunk_size,
                actual: metadata_chunk_size,
            });
        }
        let bytes_len: u64 = bytes
            .len()
            .try_into()
            .expect("snapshot length should fit in u64");
        if self.total_len_bytes != bytes_len {
            return Err(SnapshotMerkleError::LengthMismatch {
                expected: self.total_len_bytes,
                actual: bytes_len,
            });
        }
        let computed = SnapshotMerkleMetadata::from_bytes(bytes, metadata_chunk_size);
        if computed.root_hex != self.root_hex {
            return Err(SnapshotMerkleError::RootMismatch {
                expected: self.root_hex.clone(),
                actual: computed.root_hex,
            });
        }
        if bytes.is_empty() {
            self.verify_self()?;
        } else {
            let first_chunk_len = metadata_chunk_size.get().min(bytes.len());
            self.verify_chunk(0, &bytes[..first_chunk_len])?;
        }
        Ok(())
    }
    fn proof_for_chunk(
        &self,
        chunk_index: usize,
    ) -> Result<CompactMerkleProof<[u8; 32]>, SnapshotMerkleError> {
        let tree = self.tree()?;
        let index = u32::try_from(chunk_index)
            .map_err(|_| SnapshotMerkleError::ProofUnavailable { chunk_index })?;
        let Some(proof) = tree.get_proof(index) else {
            return Err(SnapshotMerkleError::ProofUnavailable { chunk_index });
        };
        CompactMerkleProof::try_from_full(proof).map_err(|error| {
            SnapshotMerkleError::ProofInvalid {
                chunk_index,
                reason: error.to_string(),
            }
        })
    }
    fn verify_chunk(
        &self,
        chunk_index: usize,
        chunk_bytes: &[u8],
    ) -> Result<(), SnapshotMerkleError> {
        self.verify_self()?;
        let proof = self.proof_for_chunk(chunk_index)?;
        let digest = Sha256::digest(chunk_bytes);
        let mut leaf = [0u8; Hash::LENGTH];
        leaf.copy_from_slice(&digest);
        let leaf = HashOf::from_untyped_unchecked(Hash::prehashed(leaf));
        let root = self.parse_root()?;
        let chunk_size = self.chunk_size()?;
        let leaf_count = NonZeroU64::new(self.expected_leaf_count(chunk_size)?).ok_or(
            SnapshotMerkleError::ProofInvalid {
                chunk_index,
                reason: "empty snapshot has no chunk membership proof".to_owned(),
            },
        )?;
        let commitment = MerkleTreeCommitment::new(root, leaf_count);
        if !proof.verify_sha256(&leaf, &commitment) {
            return Err(SnapshotMerkleError::ProofInvalid {
                chunk_index,
                reason: "failed to verify Merkle path".to_owned(),
            });
        }
        Ok(())
    }
    #[cfg(test)]
    fn from_path(path: &Path, max_bytes: u64) -> Result<Self, SnapshotMerkleError> {
        let bytes = match read_bounded_stable_regular_file(path, max_bytes) {
            Ok(Some(bytes)) => bytes,
            Ok(None) => return Err(SnapshotMerkleError::Missing),
            Err(err) => return Err(SnapshotMerkleError::Io(err)),
        };
        let value =
            json::from_slice::<norito::json::Value>(&bytes).map_err(SnapshotMerkleError::Parse)?;
        let metadata = Self::from_json_value(value)?;
        let canonical = json::to_json(&metadata).map_err(SnapshotMerkleError::Parse)?;
        if canonical.as_bytes() != bytes {
            return Err(Self::parse_error(
                "snapshot Merkle metadata is not canonically encoded",
            ));
        }
        Ok(metadata)
    }
}
fn chunk_hashes(bytes: &[u8], chunk_size: NonZeroUsize) -> Vec<[u8; 32]> {
    let chunk = chunk_size.get();
    if chunk == 0 {
        return Vec::new();
    }
    if bytes.is_empty() {
        let digest = Sha256::digest([]);
        let mut arr = [0u8; Hash::LENGTH];
        arr.copy_from_slice(&digest);
        return vec![arr];
    }
    bytes
        .chunks(chunk)
        .map(|chunk_bytes| {
            let digest = Sha256::digest(chunk_bytes);
            let mut arr = [0u8; Hash::LENGTH];
            arr.copy_from_slice(&digest);
            arr
        })
        .collect()
}
// /// Errors produced by [`SnapshotMaker`] actor.
// pub type Result<T, E = Error> = core::result::Result<T, E>;
/// Actor responsible for [`State`] snapshot reading and writing.
pub struct SnapshotMaker {
    state: Arc<State>,
    /// Frequency at which snapshot is made
    create_every: Duration,
    /// Path to the directory where snapshots are stored
    store_dir: PathBuf,
    /// Hash of the latest block stored in the state
    latest_block_hash: Option<HashOf<BlockHeader>>,
    /// Key used to sign snapshot digests.
    signing_key: KeyPair,
    /// Chunk size used to compute Merkle metadata.
    merkle_chunk_size: NonZeroUsize,
    /// Maximum canonical snapshot payload accepted by this node on restart.
    max_payload_bytes: NonZeroUsize,
    /// Typed decode and transient resource limits used for restart parity.
    resource_policy: SnapshotResourcePolicy,
}
impl SnapshotMaker {
    /// Start the actor.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> Child {
        Child::new(
            tokio::spawn(self.run(shutdown_signal)),
            OnShutdown::Wait(Duration::from_secs(30)),
        )
    }
    async fn run(mut self, shutdown_signal: ShutdownSignal) {
        let mut snapshot_create_every = tokio::time::interval(self.create_every);
        // Don't try to create snapshot more frequently if previous take longer time
        snapshot_create_every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tokio::select! {
                _ = snapshot_create_every.tick() => {
                    // Offload snapshot creation into blocking thread
                    self.create_snapshot();
                },
                () = shutdown_signal.receive() => {
                    info!("Saving latest snapshot and shutting down");
                    self.create_snapshot();
                    break;
                }
            }
            tokio::task::yield_now().await;
        }
    }
    /// Invoke snapshot creation task
    fn create_snapshot(&mut self) {
        let store_dir = self.store_dir.clone();
        let latest_block_hash = self.state.latest_block_hash_fast();
        let at_height = self.state.committed_height();
        if latest_block_hash != self.latest_block_hash {
            let state = self.state.clone();
            let store_dir = store_dir.clone();
            let signing_key = self.signing_key.clone();
            let merkle_chunk_size = self.merkle_chunk_size;
            let max_payload_bytes = self.max_payload_bytes;
            let resource_policy = self.resource_policy;
            let result = tokio::task::block_in_place(move || {
                try_write_snapshot_with_limit_and_policy(
                    &state,
                    store_dir,
                    &signing_key,
                    merkle_chunk_size,
                    max_payload_bytes,
                    resource_policy,
                )
            });
            match result {
                Ok(()) => {
                    iroha_logger::info!(at_height, "Successfully created a snapshot of state");
                    self.latest_block_hash = latest_block_hash;
                }
                Err(error @ TryWriteError::CommitEvidenceDeferred { .. }) => {
                    iroha_logger::debug!(%error, "Deferring snapshot until commit evidence is complete");
                }
                Err(error) => {
                    iroha_logger::error!(%error, "Failed to create a snapshot of state");
                }
            }
        }
    }
    /// Create from [`Config`].
    ///
    /// Might return [`None`] if the configuration is not suitable for _making_ snapshots.
    pub fn from_config(config: &Config, state: Arc<State>, signing_key: KeyPair) -> Option<Self> {
        if let Mode::ReadWrite = config.mode {
            let latest_block_hash = state.latest_block_hash_fast();
            Some(Self {
                state,
                create_every: config.create_every_ms.get(),
                store_dir: config.store_dir.resolve_relative_path(),
                latest_block_hash,
                signing_key,
                merkle_chunk_size: config.merkle_chunk_size_bytes,
                max_payload_bytes: config.max_payload_bytes,
                resource_policy: config.resources,
            })
        } else {
            None
        }
    }
}
#[cfg(unix)]
type StableSnapshotFileIdentity = (u64, u64);
#[cfg(windows)]
type StableSnapshotFileIdentity = (Option<u32>, Option<u64>);
#[cfg(not(any(unix, windows)))]
type StableSnapshotFileIdentity = ();
#[cfg(unix)]
fn stable_file_identity(metadata: &SecureMetadata) -> StableSnapshotFileIdentity {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}
#[cfg(windows)]
fn stable_file_identity(metadata: &SecureMetadata) -> StableSnapshotFileIdentity {
    (metadata.volume_serial_number(), metadata.file_index())
}
#[cfg(not(any(unix, windows)))]
fn stable_file_identity(_metadata: &SecureMetadata) -> StableSnapshotFileIdentity {}
#[cfg(unix)]
fn stable_file_identity_available(_identity: StableSnapshotFileIdentity) -> bool {
    true
}
#[cfg(windows)]
fn stable_file_identity_available(identity: StableSnapshotFileIdentity) -> bool {
    identity.0.is_some() && identity.1.is_some()
}
#[cfg(not(any(unix, windows)))]
fn stable_file_identity_available(_identity: StableSnapshotFileIdentity) -> bool {
    false
}
fn regular_file_has_single_link(metadata: &SecureMetadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}
#[cfg(unix)]
fn snapshot_unix_owner_and_mode_are_trusted(uid: u32, mode: u32, effective_uid: u32) -> bool {
    uid == effective_uid && mode & 0o022 == 0
}
fn snapshot_metadata_has_trusted_owner_and_mode(metadata: &SecureMetadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        snapshot_unix_owner_and_mode_are_trusted(
            metadata.uid(),
            metadata.mode(),
            rustix::process::geteuid().as_raw(),
        )
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        true
    }
}
fn bounded_snapshot_read_capacity(opened_len: u64, max_bytes: u64) -> std::io::Result<usize> {
    if opened_len > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("snapshot artifact is {opened_len} bytes; maximum is {max_bytes}"),
        ));
    }
    usize::try_from(opened_len).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact length does not fit memory",
        )
    })
}
fn stream_snapshot_file_digest(
    file: &std::fs::File,
    expected_len: u64,
    max_bytes: u64,
) -> std::io::Result<[u8; 32]> {
    if expected_len > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("snapshot artifact is {expected_len} bytes; maximum is {max_bytes}"),
        ));
    }
    let mut reader = file;
    reader.seek(std::io::SeekFrom::Start(0))?;
    let mut digest = Sha256::new();
    let mut total = 0_u64;
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        total = total
            .checked_add(u64::try_from(read).expect("snapshot read size fits u64"))
            .ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "snapshot artifact read length overflowed",
                )
            })?;
        if total > max_bytes || total > expected_len {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                "snapshot artifact exceeded its bound while streaming",
            ));
        }
        digest.update(&buffer[..read]);
    }
    if total != expected_len {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("snapshot artifact length changed from {expected_len} to {total}"),
        ));
    }
    reader.seek(std::io::SeekFrom::Start(0))?;
    Ok(digest.finalize().into())
}
fn bind_snapshot_file_handle(
    path: &Path,
    max_bytes: u64,
) -> std::io::Result<Option<BoundSnapshotFile>> {
    bind_snapshot_file_handle_with_digest(path, max_bytes, true)
}
fn bind_snapshot_file_handle_without_digest(
    path: &Path,
    max_bytes: u64,
) -> std::io::Result<Option<BoundSnapshotFile>> {
    bind_snapshot_file_handle_with_digest(path, max_bytes, false)
}
fn bind_snapshot_file_handle_with_digest(
    path: &Path,
    max_bytes: u64,
    hash_contents: bool,
) -> std::io::Result<Option<BoundSnapshotFile>> {
    let path_before = match secure_file_metadata::from_path(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if path_before.file_type().is_symlink()
        || !path_before.is_file()
        || !regular_file_has_single_link(&path_before)
        || !stable_file_identity_available(stable_file_identity(&path_before))
        || !snapshot_metadata_has_trusted_owner_and_mode(&path_before)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact must be a direct single-link regular file owned by the effective user and not writable by group or other users",
        ));
    }
    if path_before.len() > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "snapshot artifact is {} bytes; maximum is {max_bytes}",
                path_before.len()
            ),
        ));
    }
    let file = std::fs::File::open(path)?;
    let opened_before = secure_file_metadata::from_file(&file)?;
    if !opened_before.is_file()
        || !regular_file_has_single_link(&opened_before)
        || !stable_file_identity_available(stable_file_identity(&opened_before))
        || stable_file_identity(&opened_before) != stable_file_identity(&path_before)
        || !snapshot_metadata_has_trusted_owner_and_mode(&opened_before)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact identity changed while opening",
        ));
    }
    let bytes_sha256 = if hash_contents {
        #[cfg(test)]
        if path
            .file_name()
            .is_some_and(|name| name == SNAPSHOT_FILE_NAME)
        {
            SNAPSHOT_PAYLOAD_DIGEST_PASSES.with(|passes| passes.set(passes.get() + 1));
        }
        Some(stream_snapshot_file_digest(
            &file,
            opened_before.len(),
            max_bytes,
        )?)
    } else {
        None
    };
    let opened_after = secure_file_metadata::from_file(&file)?;
    let path_after = secure_file_metadata::from_path(path)?;
    if path_after.file_type().is_symlink()
        || !path_after.is_file()
        || !regular_file_has_single_link(&path_after)
        || !stable_file_identity_available(stable_file_identity(&path_after))
        || stable_file_identity(&opened_before) != stable_file_identity(&opened_after)
        || stable_file_identity(&opened_before) != stable_file_identity(&path_after)
        || opened_before.len() != opened_after.len()
        || !snapshot_metadata_has_trusted_owner_and_mode(&opened_after)
        || !snapshot_metadata_has_trusted_owner_and_mode(&path_after)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact changed while reading",
        ));
    }
    Ok(Some(BoundSnapshotFile {
        path: path.to_path_buf(),
        handle: Arc::new(file),
        identity: stable_file_identity(&opened_before),
        len: opened_before.len(),
        bytes_sha256,
        max_bytes,
    }))
}
fn read_bound_snapshot_payload(
    binding: &BoundSnapshotFile,
) -> Result<(Vec<u8>, [u8; 32]), TryReadError> {
    #[cfg(test)]
    SNAPSHOT_PAYLOAD_DIGEST_PASSES.with(|passes| passes.set(passes.get() + 1));
    let capacity = bounded_snapshot_read_capacity(binding.len, binding.max_bytes)
        .map_err(|error| TryReadError::IO(error, binding.path.clone()))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|error| {
        TryReadError::IO(
            std::io::Error::other(format!(
                "failed to reserve memory for authenticated snapshot payload: {error}"
            )),
            binding.path.clone(),
        )
    })?;
    let mut reader = binding.handle.as_ref();
    reader
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|error| TryReadError::IO(error, binding.path.clone()))?;
    let mut digest = Sha256::new();
    let mut remaining = capacity;
    let mut buffer = [0_u8; 64 * 1024];
    while remaining != 0 {
        let requested = remaining.min(buffer.len());
        let read = reader
            .read(&mut buffer[..requested])
            .map_err(|error| TryReadError::IO(error, binding.path.clone()))?;
        if read == 0 {
            return Err(TryReadError::SnapshotBindingChanged(binding.path.clone()));
        }
        bytes.extend_from_slice(&buffer[..read]);
        Digest::update(&mut digest, &buffer[..read]);
        remaining -= read;
    }
    reader
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|error| TryReadError::IO(error, binding.path.clone()))?;
    let actual_sha256: [u8; 32] = digest.finalize().into();
    if binding
        .bytes_sha256
        .is_some_and(|expected_sha256| actual_sha256 != expected_sha256)
    {
        return Err(TryReadError::SnapshotBindingChanged(binding.path.clone()));
    }
    verify_bound_snapshot_file_metadata_at(&binding.path, binding)?;
    Ok((bytes, actual_sha256))
}
#[derive(Clone, Debug)]
struct BoundSnapshotFile {
    path: PathBuf,
    handle: Arc<std::fs::File>,
    identity: StableSnapshotFileIdentity,
    len: u64,
    bytes_sha256: Option<[u8; 32]>,
    max_bytes: u64,
}
#[derive(Clone, Debug)]
enum BoundSnapshotDestination {
    Absent,
    Present {
        binding: BoundSnapshotFile,
        bytes: Vec<u8>,
    },
}
fn bind_snapshot_file(
    path: &Path,
    max_bytes: u64,
) -> Result<Option<(BoundSnapshotFile, Vec<u8>)>, TryReadError> {
    let Some(binding) = bind_snapshot_file_handle(path, max_bytes)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?
    else {
        return Ok(None);
    };
    let capacity = bounded_snapshot_read_capacity(binding.len, max_bytes)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|error| {
        TryReadError::IO(
            std::io::Error::other(format!(
                "failed to reserve memory for snapshot artifact: {error}"
            )),
            path.to_path_buf(),
        )
    })?;
    let mut reader = binding.handle.as_ref();
    reader
        .seek(std::io::SeekFrom::Start(0))
        .and_then(|_| {
            reader
                .take(max_bytes.saturating_add(1))
                .read_to_end(&mut bytes)
        })
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let actual_sha256: [u8; 32] = Sha256::digest(&bytes).into();
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) != binding.len
        || binding.bytes_sha256 != Some(actual_sha256)
    {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    Ok(Some((binding, bytes)))
}
#[cfg(test)]
fn read_bounded_stable_regular_file(
    path: &Path,
    max_bytes: u64,
) -> std::io::Result<Option<Vec<u8>>> {
    bind_snapshot_file(path, max_bytes)
        .map(|binding| binding.map(|(_, bytes)| bytes))
        .map_err(|error| match error {
            TryReadError::IO(error, _) => error,
            error => std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()),
        })
}
fn bind_snapshot_destination(
    path: &Path,
    max_bytes: u64,
) -> Result<BoundSnapshotDestination, TryReadError> {
    Ok(match bind_snapshot_file(path, max_bytes)? {
        Some((binding, bytes)) => BoundSnapshotDestination::Present { binding, bytes },
        None => BoundSnapshotDestination::Absent,
    })
}
fn verify_bound_snapshot_file_at(
    path: &Path,
    binding: &BoundSnapshotFile,
) -> Result<(), TryReadError> {
    verify_bound_snapshot_file_metadata_at(path, binding)?;
    let expected_sha256 = binding
        .bytes_sha256
        .ok_or_else(|| TryReadError::SnapshotBindingChanged(path.to_path_buf()))?;
    #[cfg(test)]
    if binding
        .path
        .file_name()
        .is_some_and(|name| name == SNAPSHOT_FILE_NAME)
    {
        SNAPSHOT_PAYLOAD_DIGEST_PASSES.with(|passes| passes.set(passes.get() + 1));
    }
    let digest = stream_snapshot_file_digest(&binding.handle, binding.len, binding.max_bytes)
        .map_err(|error| TryReadError::IO(error, binding.path.clone()))?;
    if digest != expected_sha256 {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    Ok(())
}
fn verify_bound_snapshot_file_metadata_at(
    path: &Path,
    binding: &BoundSnapshotFile,
) -> Result<(), TryReadError> {
    let metadata = secure_file_metadata::from_path(path)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let opened = secure_file_metadata::from_file(&binding.handle)
        .map_err(|error| TryReadError::IO(error, binding.path.clone()))?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || !opened.is_file()
        || !regular_file_has_single_link(&metadata)
        || !regular_file_has_single_link(&opened)
        || !stable_file_identity_available(stable_file_identity(&metadata))
        || !stable_file_identity_available(stable_file_identity(&opened))
        || stable_file_identity(&opened) != binding.identity
        || opened.len() != binding.len
        || stable_file_identity(&metadata) != binding.identity
        || metadata.len() != binding.len
        || !snapshot_metadata_has_trusted_owner_and_mode(&metadata)
        || !snapshot_metadata_has_trusted_owner_and_mode(&opened)
    {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    Ok(())
}
fn verify_bound_snapshot_file(binding: &BoundSnapshotFile) -> Result<(), TryReadError> {
    verify_bound_snapshot_file_at(&binding.path, binding)
}
fn verify_bound_snapshot_destination(
    path: &Path,
    binding: &BoundSnapshotDestination,
) -> Result<(), TryReadError> {
    match binding {
        BoundSnapshotDestination::Absent => match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Ok(_) | Err(_) => Err(TryReadError::SnapshotBindingChanged(path.to_path_buf())),
        },
        BoundSnapshotDestination::Present { binding, .. } => {
            verify_bound_snapshot_file_at(path, binding)
        }
    }
}
fn snapshot_merkle_max_bytes(payload_len: u64, merkle_chunk_size: NonZeroUsize) -> u64 {
    let chunk_size = u64::try_from(merkle_chunk_size.get()).unwrap_or(u64::MAX);
    let leaf_count = if payload_len == 0 {
        1
    } else {
        payload_len.saturating_sub(1) / chunk_size + 1
    };
    SNAPSHOT_MERKLE_FIXED_OVERHEAD_BYTES
        .saturating_add(leaf_count.saturating_mul(SNAPSHOT_MERKLE_BYTES_PER_LEAF))
}
fn verify_signature_hex(
    signature_hex: &str,
    digest: &[u8],
    verification_key: &PublicKey,
) -> Result<(), TryReadError> {
    let signature_bytes = hex::decode(signature_hex)
        .map_err(|_| TryReadError::SignatureMalformed(signature_hex.to_owned()))?;
    if hex::encode(&signature_bytes) != signature_hex {
        return Err(TryReadError::SignatureMalformed(signature_hex.to_owned()));
    }
    let algorithm = verification_key.try_algorithm().map_err(|err| {
        TryReadError::SignatureInvalid(format!("invalid verification key: {err}"))
    })?;
    let signature = match algorithm {
        Algorithm::Ed25519 => iroha_crypto::ed25519_parse_signature(&signature_bytes)
            .map_err(|_| TryReadError::SignatureMalformed(signature_hex.to_owned()))?,
        Algorithm::MlDsa => iroha_crypto::mldsa65_parse_signature(&signature_bytes)
            .map_err(|_| TryReadError::SignatureMalformed(signature_hex.to_owned()))?,
        _ => Signature::try_from_bytes(&signature_bytes)
            .map_err(|_| TryReadError::SignatureMalformed(signature_hex.to_owned()))?,
    };
    signature
        .verify(verification_key, digest)
        .map_err(|err| TryReadError::SignatureInvalid(err.to_string()))
}
fn sync_snapshot_directory(
    path: &Path,
    expected_identity: StableSnapshotFileIdentity,
) -> Result<(), TryReadError> {
    if direct_snapshot_directory_identity(path)? != expected_identity {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    let file =
        std::fs::File::open(path).map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let opened_before = secure_file_metadata::from_file(&file)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    if !opened_before.is_dir() || stable_file_identity(&opened_before) != expected_identity {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    file.sync_all()
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let opened_after = secure_file_metadata::from_file(&file)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    if !opened_after.is_dir()
        || stable_file_identity(&opened_after) != expected_identity
        || direct_snapshot_directory_identity(path)? != expected_identity
    {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    Ok(())
}
struct SnapshotReadOutcome {
    state: State,
}
fn direct_snapshot_directory_identity(
    path: &Path,
) -> Result<StableSnapshotFileIdentity, TryReadError> {
    let metadata = secure_file_metadata::from_path(path)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    if !snapshot_metadata_has_trusted_owner_and_mode(&metadata) {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason: "directory must be owned by the effective user and not writable by group or other users"
                .to_owned(),
        });
    }
    let identity = stable_file_identity(&metadata);
    if !stable_file_identity_available(identity) {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    Ok(identity)
}
struct SnapshotGenerationBytes {
    digest: Vec<u8>,
    signature: Vec<u8>,
    fast_manifest: Vec<u8>,
    merkle: Vec<u8>,
}
struct BoundSnapshotGeneration {
    store_dir: PathBuf,
    store_dir_identity: StableSnapshotFileIdentity,
    generations_dir: PathBuf,
    generations_dir_identity: StableSnapshotFileIdentity,
    pointer: BoundSnapshotFile,
    generation_dir: PathBuf,
    generation_dir_identity: StableSnapshotFileIdentity,
    payload: BoundSnapshotFile,
    artifacts: Vec<BoundSnapshotFile>,
    bytes: SnapshotGenerationBytes,
}
fn bind_required_snapshot_file(
    path: &Path,
    max_bytes: u64,
) -> Result<(BoundSnapshotFile, Vec<u8>), TryReadError> {
    bind_snapshot_file(path, max_bytes)?.ok_or_else(|| TryReadError::SnapshotGenerationInvalid {
        path: path.to_path_buf(),
        reason: "required generation artifact is missing".to_owned(),
    })
}
fn parse_snapshot_current_pointer(bytes: &[u8], path: &Path) -> Result<String, TryReadError> {
    let text = std::str::from_utf8(bytes).map_err(|_| TryReadError::SnapshotGenerationInvalid {
        path: path.to_path_buf(),
        reason: "current pointer is not UTF-8".to_owned(),
    })?;
    let Some(digest_hex) = text.strip_suffix('\n') else {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason: "current pointer lacks its canonical newline".to_owned(),
        });
    };
    let decoded = hex::decode(digest_hex).map_err(|_| TryReadError::SnapshotGenerationInvalid {
        path: path.to_path_buf(),
        reason: "current pointer is not a SHA-256 hex digest".to_owned(),
    })?;
    if decoded.len() != Hash::LENGTH || hex::encode(decoded) != digest_hex {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason: "current pointer is not canonical lowercase SHA-256 hex".to_owned(),
        });
    }
    Ok(digest_hex.to_owned())
}

fn parse_snapshot_digest_bytes(bytes: &[u8], path: &Path) -> Result<[u8; 32], TryReadError> {
    let digest_hex = parse_snapshot_current_pointer(bytes, path)?;
    let decoded = hex::decode(digest_hex).map_err(|_| TryReadError::SnapshotGenerationInvalid {
        path: path.to_path_buf(),
        reason: "snapshot digest is not canonical SHA-256".to_owned(),
    })?;
    decoded
        .try_into()
        .map_err(|_| TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason: "snapshot digest has the wrong length".to_owned(),
        })
}

fn decode_emergency_fast_manifest(
    bytes: &[u8],
    path: &Path,
) -> Result<EmergencyFastSnapshotManifestV1, TryReadError> {
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > SNAPSHOT_FAST_MANIFEST_MAX_BYTES {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason: "emergency Fast manifest exceeds its fixed size bound".to_owned(),
        });
    }
    let mut cursor: &[u8] = bytes;
    let manifest = EmergencyFastSnapshotManifestV1::decode_all(&mut cursor).map_err(|error| {
        TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason: format!("invalid emergency Fast manifest: {error}"),
        }
    })?;
    manifest
        .validate()
        .map_err(|reason| TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason,
        })?;
    if manifest.encode() != bytes {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: path.to_path_buf(),
            reason: "emergency Fast manifest is not canonical Norito".to_owned(),
        });
    }
    Ok(manifest)
}
fn bind_current_snapshot_generation(
    store_dir: &Path,
    payload_limit: u64,
    merkle_chunk_size: NonZeroUsize,
) -> Result<BoundSnapshotGeneration, TryReadError> {
    bind_current_snapshot_generation_with_mode(store_dir, payload_limit, merkle_chunk_size, false)
}
fn bind_current_snapshot_generation_emergency_fast(
    store_dir: &Path,
    payload_limit: u64,
    merkle_chunk_size: NonZeroUsize,
) -> Result<BoundSnapshotGeneration, TryReadError> {
    bind_current_snapshot_generation_with_mode(store_dir, payload_limit, merkle_chunk_size, true)
}
fn bind_current_snapshot_generation_with_mode(
    store_dir: &Path,
    payload_limit: u64,
    merkle_chunk_size: NonZeroUsize,
    emergency_fast: bool,
) -> Result<BoundSnapshotGeneration, TryReadError> {
    let store_dir_identity = direct_snapshot_directory_identity(store_dir)?;
    let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
    let generations_dir_identity = direct_snapshot_directory_identity(&generations_dir)?;
    let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
    let Some((pointer, pointer_bytes)) =
        bind_snapshot_file(&pointer_path, SNAPSHOT_CURRENT_MAX_BYTES)?
    else {
        return Err(TryReadError::NotFound);
    };
    let digest_hex = parse_snapshot_current_pointer(&pointer_bytes, &pointer_path)?;
    let generation_dir = generations_dir.join(&digest_hex);
    let generation_dir_identity = direct_snapshot_directory_identity(&generation_dir)?;
    if !snapshot_generation_has_exact_artifact_inventory(&generation_dir)? {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: generation_dir,
            reason: "generation does not contain exactly the five canonical artifacts".to_owned(),
        });
    }
    let payload_path = generation_dir.join(SNAPSHOT_FILE_NAME);
    let payload_binding = if emergency_fast {
        bind_snapshot_file_handle_without_digest(&payload_path, payload_limit)
    } else {
        bind_snapshot_file_handle(&payload_path, payload_limit)
    };
    let payload = payload_binding
        .map_err(|error| TryReadError::IO(error, payload_path.clone()))?
        .ok_or_else(|| TryReadError::SnapshotGenerationInvalid {
            path: payload_path,
            reason: "required generation artifact is missing".to_owned(),
        })?;
    let (digest, digest_bytes) = bind_required_snapshot_file(
        &generation_dir.join(SNAPSHOT_DIGEST_FILE_NAME),
        SNAPSHOT_DIGEST_MAX_BYTES,
    )?;
    if digest_bytes != pointer_bytes {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: digest.path.clone(),
            reason: "generation digest differs from the canonical current pointer".to_owned(),
        });
    }
    let (signature, signature_bytes) = bind_required_snapshot_file(
        &generation_dir.join(SNAPSHOT_SIGNATURE_FILE_NAME),
        SNAPSHOT_SIGNATURE_MAX_BYTES,
    )?;
    let (fast_manifest, fast_manifest_bytes) = bind_required_snapshot_file(
        &generation_dir.join(SNAPSHOT_FAST_MANIFEST_FILE_NAME),
        SNAPSHOT_FAST_MANIFEST_MAX_BYTES,
    )?;
    let mut artifacts = vec![payload.clone(), digest, signature, fast_manifest];
    let merkle_limit = snapshot_merkle_max_bytes(payload.len, merkle_chunk_size);
    let merkle_path = generation_dir.join(SNAPSHOT_MERKLE_FILE_NAME);
    let merkle_bytes = if emergency_fast {
        let merkle = bind_snapshot_file_handle_without_digest(&merkle_path, merkle_limit)
            .map_err(|error| TryReadError::IO(error, merkle_path.clone()))?
            .ok_or_else(|| TryReadError::SnapshotGenerationInvalid {
                path: merkle_path,
                reason: "required generation artifact is missing".to_owned(),
            })?;
        artifacts.push(merkle);
        Vec::new()
    } else {
        let (merkle, bytes) = bind_required_snapshot_file(&merkle_path, merkle_limit)?;
        artifacts.push(merkle);
        bytes
    };
    Ok(BoundSnapshotGeneration {
        store_dir: store_dir.to_path_buf(),
        store_dir_identity,
        generations_dir,
        generations_dir_identity,
        pointer,
        generation_dir,
        generation_dir_identity,
        payload: payload.clone(),
        artifacts,
        bytes: SnapshotGenerationBytes {
            digest: digest_bytes,
            signature: signature_bytes,
            fast_manifest: fast_manifest_bytes,
            merkle: merkle_bytes,
        },
    })
}
impl BoundSnapshotGeneration {
    fn verify_selection_unchanged(&self) -> Result<(), TryReadError> {
        verify_bound_snapshot_file(&self.pointer)?;
        self.verify_generation_unchanged()
    }
    fn verify_generation_unchanged(&self) -> Result<(), TryReadError> {
        if direct_snapshot_directory_identity(&self.store_dir)? != self.store_dir_identity
            || direct_snapshot_directory_identity(&self.generations_dir)?
                != self.generations_dir_identity
            || direct_snapshot_directory_identity(&self.generation_dir)?
                != self.generation_dir_identity
            || !snapshot_generation_has_exact_artifact_inventory(&self.generation_dir)?
        {
            return Err(TryReadError::SnapshotGenerationInvalid {
                path: self.store_dir.clone(),
                reason: "snapshot generation directory identity changed".to_owned(),
            });
        }
        for artifact in &self.artifacts {
            verify_bound_snapshot_file(artifact)?;
        }
        Ok(())
    }

    fn verify_emergency_fast_selection_unchanged(&self) -> Result<(), TryReadError> {
        verify_bound_snapshot_file(&self.pointer)?;
        if direct_snapshot_directory_identity(&self.store_dir)? != self.store_dir_identity
            || direct_snapshot_directory_identity(&self.generations_dir)?
                != self.generations_dir_identity
            || direct_snapshot_directory_identity(&self.generation_dir)?
                != self.generation_dir_identity
            || !snapshot_generation_has_exact_artifact_inventory(&self.generation_dir)?
        {
            return Err(TryReadError::SnapshotGenerationInvalid {
                path: self.store_dir.clone(),
                reason: "snapshot generation selection changed during emergency Fast restore"
                    .to_owned(),
            });
        }
        verify_bound_snapshot_file_metadata_at(&self.payload.path, &self.payload)?;
        for artifact in self.artifacts.iter().skip(1) {
            if artifact.bytes_sha256.is_some() {
                verify_bound_snapshot_file(artifact)?;
            } else {
                verify_bound_snapshot_file_metadata_at(&artifact.path, artifact)?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SnapshotPayloadAuthority {
    NormallySigned,
    ExactAuditedDigestBypass,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SnapshotBootstrapLineageAuthorityKind {
    ExactAuditedBoundary,
    NormallySignedCarriedLineage,
}
/// Non-forgeable proof that the snapshot reader authenticated the outer payload.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct SnapshotBootstrapLineageAuthority {
    kind: SnapshotBootstrapLineageAuthorityKind,
}
impl SnapshotBootstrapLineageAuthority {
    fn exact_audited_boundary() -> Self {
        Self {
            kind: SnapshotBootstrapLineageAuthorityKind::ExactAuditedBoundary,
        }
    }
    fn normally_signed_carried_lineage() -> Self {
        Self {
            kind: SnapshotBootstrapLineageAuthorityKind::NormallySignedCarriedLineage,
        }
    }
    pub(crate) fn permits_carried_lineage(self) -> bool {
        self.kind == SnapshotBootstrapLineageAuthorityKind::NormallySignedCarriedLineage
    }
}
/// Authenticated snapshot bootstrap record and the exact signed block-hash vector.
///
/// Fields and construction stay private to this module so raw decoded snapshot
/// bytes cannot authorize provisional Kura mutation.
#[derive(Clone, Debug)]
pub(crate) struct AuthenticatedSnapshotBootstrapPayload {
    record: SnapshotV2BootstrapRecord,
    block_hashes: Vec<HashOf<BlockHeader>>,
    authority: SnapshotBootstrapLineageAuthority,
}
impl AuthenticatedSnapshotBootstrapPayload {
    fn new(
        record: SnapshotV2BootstrapRecord,
        block_hashes: Vec<HashOf<BlockHeader>>,
        authority: SnapshotBootstrapLineageAuthority,
    ) -> Self {
        Self {
            record,
            block_hashes,
            authority,
        }
    }
    pub(crate) fn record(&self) -> &SnapshotV2BootstrapRecord {
        &self.record
    }
    pub(crate) fn block_hashes(&self) -> &[HashOf<BlockHeader>] {
        &self.block_hashes
    }
    pub(crate) fn is_exact_audited_boundary(&self) -> bool {
        self.authority.kind == SnapshotBootstrapLineageAuthorityKind::ExactAuditedBoundary
    }
    #[cfg(test)]
    pub(crate) fn for_testing(
        record: SnapshotV2BootstrapRecord,
        block_hashes: Vec<HashOf<BlockHeader>>,
    ) -> Self {
        Self::new(
            record,
            block_hashes,
            SnapshotBootstrapLineageAuthority::exact_audited_boundary(),
        )
    }
}
fn snapshot_payload_preview(bytes: &[u8]) -> String {
    let mut preview = String::new();
    let limit = bytes.len().min(96);
    for &byte in &bytes[..limit] {
        match byte {
            b'\n' => preview.push_str("\\n"),
            b'\r' => preview.push_str("\\r"),
            b'\t' => preview.push_str("\\t"),
            0x20..=0x7E => preview.push(char::from(byte)),
            _ => preview.push('.'),
        }
    }
    preview
}
#[derive(Clone, Copy, Debug, Default)]
struct SnapshotJsonSummary {
    has_space_directory_manifests: bool,
    block_hash_count: Option<usize>,
}
struct SnapshotJsonBudgetScanner<'a> {
    input: &'a str,
    cursor: usize,
    policy: SnapshotResourcePolicy,
    items: usize,
    transient_bytes: usize,
    largest_typed_field_bytes: usize,
    summary: SnapshotJsonSummary,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SnapshotJsonContext {
    Root,
    World,
    Other,
}
impl<'a> SnapshotJsonBudgetScanner<'a> {
    fn new(input: &'a str, policy: SnapshotResourcePolicy) -> Self {
        Self {
            input,
            cursor: 0,
            policy,
            items: 0,
            transient_bytes: input.len(),
            largest_typed_field_bytes: 0,
            summary: SnapshotJsonSummary::default(),
        }
    }
    fn scan(mut self) -> Result<SnapshotJsonSummary, TryReadError> {
        self.charge_transient(0)?;
        self.parse_value(1, false, SnapshotJsonContext::Root)?;
        if self.cursor != self.input.len() {
            return Err(TryReadError::NonCanonicalSnapshotPayload);
        }
        Ok(self.summary)
    }
    fn parse_value(
        &mut self,
        depth: usize,
        encoded_blob: bool,
        context: SnapshotJsonContext,
    ) -> Result<(), TryReadError> {
        if depth > self.policy.max_decode_depth.get() {
            return Err(TryReadError::SnapshotResourceLimit(format!(
                "JSON nesting depth {depth} exceeds configured limit {}",
                self.policy.max_decode_depth
            )));
        }
        match self.peek() {
            Some(b'{') => self.parse_object(depth, context),
            Some(b'[') => self.parse_array(depth),
            Some(b'"') => self.parse_string(encoded_blob).map(|_| ()),
            Some(b't') => self.consume_exact(b"true"),
            Some(b'f') => self.consume_exact(b"false"),
            Some(b'n') => self.consume_exact(b"null"),
            Some(b'-' | b'0'..=b'9') => self.parse_number(),
            _ => Err(TryReadError::NonCanonicalSnapshotPayload),
        }
    }
    fn parse_object(
        &mut self,
        depth: usize,
        context: SnapshotJsonContext,
    ) -> Result<(), TryReadError> {
        self.consume_byte(b'{')?;
        if self.peek() == Some(b'}') {
            self.cursor += 1;
            return Ok(());
        }
        loop {
            let key = self.parse_string(false)?;
            self.consume_byte(b':')?;
            self.charge_item()?;
            if depth == 1 && key.raw == "space_directory_manifests" {
                self.summary.has_space_directory_manifests = true;
            }
            let value_start = self.cursor;
            let child_context = if context == SnapshotJsonContext::Root && key.raw == "world" {
                SnapshotJsonContext::World
            } else {
                SnapshotJsonContext::Other
            };
            self.parse_value(
                depth.saturating_add(1),
                key.raw == "encoded_hex",
                child_context,
            )?;
            if context == SnapshotJsonContext::World
                || (context == SnapshotJsonContext::Root && key.raw != "world")
            {
                self.record_typed_field_bytes(self.cursor - value_start)?;
            }
            if depth == 1 && key.raw == "block_hashes" {
                self.summary.block_hash_count = Some(count_borrowed_json_array_items(
                    &self.input[value_start..self.cursor],
                )?);
            }
            match self.peek() {
                Some(b',') => self.cursor += 1,
                Some(b'}') => {
                    self.cursor += 1;
                    return Ok(());
                }
                _ => return Err(TryReadError::NonCanonicalSnapshotPayload),
            }
        }
    }
    fn parse_array(&mut self, depth: usize) -> Result<(), TryReadError> {
        self.consume_byte(b'[')?;
        if self.peek() == Some(b']') {
            self.cursor += 1;
            return Ok(());
        }
        let mut array_items = 0_usize;
        loop {
            array_items = array_items.checked_add(1).ok_or_else(|| {
                TryReadError::SnapshotResourceLimit("snapshot array length overflowed".to_owned())
            })?;
            if array_items > self.policy.max_blob_bytes.get() {
                return Err(TryReadError::SnapshotResourceLimit(format!(
                    "snapshot array contains more than {} elements; byte-vector blobs use one element per byte",
                    self.policy.max_blob_bytes
                )));
            }
            self.charge_item()?;
            self.parse_value(depth.saturating_add(1), false, SnapshotJsonContext::Other)?;
            match self.peek() {
                Some(b',') => self.cursor += 1,
                Some(b']') => {
                    self.cursor += 1;
                    return Ok(());
                }
                _ => return Err(TryReadError::NonCanonicalSnapshotPayload),
            }
        }
    }
    fn parse_string(&mut self, encoded_blob: bool) -> Result<SnapshotString<'a>, TryReadError> {
        self.consume_byte(b'"')?;
        let content_start = self.cursor;
        let mut decoded_len = 0_usize;
        let mut has_escape = false;
        loop {
            let byte = self
                .peek()
                .ok_or(TryReadError::NonCanonicalSnapshotPayload)?;
            match byte {
                b'"' => {
                    let content_end = self.cursor;
                    self.cursor += 1;
                    let raw = if has_escape {
                        ""
                    } else {
                        &self.input[content_start..content_end]
                    };
                    if encoded_blob {
                        if has_escape
                            || decoded_len % 2 != 0
                            || !raw.bytes().all(|byte| byte.is_ascii_hexdigit())
                            || raw.bytes().any(|byte| byte.is_ascii_uppercase())
                        {
                            return Err(TryReadError::NonCanonicalSnapshotPayload);
                        }
                        let blob_len = decoded_len / 2;
                        if blob_len > self.policy.max_blob_bytes.get() {
                            return Err(TryReadError::SnapshotResourceLimit(format!(
                                "decoded blob is {blob_len} bytes; maximum is {}",
                                self.policy.max_blob_bytes
                            )));
                        }
                    } else if decoded_len > self.policy.max_string_bytes.get() {
                        return Err(TryReadError::SnapshotResourceLimit(format!(
                            "decoded JSON string is {decoded_len} bytes; maximum is {}",
                            self.policy.max_string_bytes
                        )));
                    }
                    self.charge_transient(decoded_len)?;
                    return Ok(SnapshotString { raw });
                }
                b'\\' => {
                    has_escape = true;
                    self.cursor += 1;
                    let escaped = self
                        .peek()
                        .ok_or(TryReadError::NonCanonicalSnapshotPayload)?;
                    self.cursor += 1;
                    match escaped {
                        b'"' | b'\\' | b'b' | b'f' | b'n' | b'r' | b't' => {
                            decoded_len = decoded_len.checked_add(1).ok_or_else(|| {
                                TryReadError::SnapshotResourceLimit(
                                    "decoded string length overflowed".to_owned(),
                                )
                            })?;
                        }
                        b'u' => {
                            let digits = self
                                .input
                                .as_bytes()
                                .get(self.cursor..self.cursor.saturating_add(4))
                                .ok_or(TryReadError::NonCanonicalSnapshotPayload)?;
                            if digits[0] != b'0'
                                || digits[1] != b'0'
                                || !digits[2].is_ascii_hexdigit()
                                || !digits[3].is_ascii_hexdigit()
                                || digits.iter().any(u8::is_ascii_uppercase)
                            {
                                return Err(TryReadError::NonCanonicalSnapshotPayload);
                            }
                            let value = u8::from_str_radix(
                                std::str::from_utf8(digits)
                                    .map_err(|_| TryReadError::NonCanonicalSnapshotPayload)?,
                                16,
                            )
                            .map_err(|_| TryReadError::NonCanonicalSnapshotPayload)?;
                            if value >= 0x20 || matches!(value, 0x08 | 0x09 | 0x0A | 0x0C | 0x0D) {
                                return Err(TryReadError::NonCanonicalSnapshotPayload);
                            }
                            self.cursor += 4;
                            decoded_len = decoded_len.checked_add(1).ok_or_else(|| {
                                TryReadError::SnapshotResourceLimit(
                                    "decoded string length overflowed".to_owned(),
                                )
                            })?;
                        }
                        _ => return Err(TryReadError::NonCanonicalSnapshotPayload),
                    }
                }
                0x00..=0x1F => return Err(TryReadError::NonCanonicalSnapshotPayload),
                byte if byte.is_ascii() => {
                    self.cursor += 1;
                    decoded_len = decoded_len.checked_add(1).ok_or_else(|| {
                        TryReadError::SnapshotResourceLimit(
                            "decoded string length overflowed".to_owned(),
                        )
                    })?;
                }
                _ => {
                    let scalar = self.input[self.cursor..]
                        .chars()
                        .next()
                        .ok_or(TryReadError::NonCanonicalSnapshotPayload)?;
                    let width = scalar.len_utf8();
                    self.cursor += width;
                    decoded_len = decoded_len.checked_add(width).ok_or_else(|| {
                        TryReadError::SnapshotResourceLimit(
                            "decoded string length overflowed".to_owned(),
                        )
                    })?;
                }
            }
        }
    }
    fn parse_number(&mut self) -> Result<(), TryReadError> {
        let start = self.cursor;
        let mut parser = json::Parser::new_at(self.input, start);
        parser.skip_value().map_err(TryReadError::Serialization)?;
        self.cursor = parser.position();
        let raw = &self.input[start..self.cursor];
        let value: json::Value = json::from_str(raw).map_err(TryReadError::Serialization)?;
        let canonical = json::to_json(&value).map_err(TryReadError::Serialization)?;
        if canonical != raw {
            return Err(TryReadError::NonCanonicalSnapshotPayload);
        }
        Ok(())
    }
    fn charge_item(&mut self) -> Result<(), TryReadError> {
        self.items = self.items.checked_add(1).ok_or_else(|| {
            TryReadError::SnapshotResourceLimit("snapshot item count overflowed".to_owned())
        })?;
        if self.items > self.policy.max_decode_items.get() {
            return Err(TryReadError::SnapshotResourceLimit(format!(
                "snapshot contains more than {} aggregate items",
                self.policy.max_decode_items
            )));
        }
        // Account conservatively for typed collection nodes, allocator metadata, and indexes.
        // Encoded strings/blobs are charged separately at their decoded size.
        self.charge_transient(16 * std::mem::size_of::<usize>())
    }
    fn charge_transient(&mut self, bytes: usize) -> Result<(), TryReadError> {
        self.transient_bytes = self.transient_bytes.checked_add(bytes).ok_or_else(|| {
            TryReadError::SnapshotResourceLimit("snapshot transient budget overflowed".to_owned())
        })?;
        if self.transient_bytes > self.policy.max_transient_bytes.get() {
            return Err(TryReadError::SnapshotResourceLimit(format!(
                "snapshot transient estimate exceeds {} bytes",
                self.policy.max_transient_bytes
            )));
        }
        Ok(())
    }
    fn record_typed_field_bytes(&mut self, bytes: usize) -> Result<(), TryReadError> {
        if bytes <= self.largest_typed_field_bytes {
            return Ok(());
        }
        let additional = bytes - self.largest_typed_field_bytes;
        self.charge_transient(additional)?;
        self.largest_typed_field_bytes = bytes;
        Ok(())
    }
    fn consume_exact(&mut self, expected: &[u8]) -> Result<(), TryReadError> {
        let end = self.cursor.saturating_add(expected.len());
        if self.input.as_bytes().get(self.cursor..end) != Some(expected) {
            return Err(TryReadError::NonCanonicalSnapshotPayload);
        }
        self.cursor = end;
        Ok(())
    }
    fn consume_byte(&mut self, expected: u8) -> Result<(), TryReadError> {
        if self.peek() != Some(expected) {
            return Err(TryReadError::NonCanonicalSnapshotPayload);
        }
        self.cursor += 1;
        Ok(())
    }
    fn peek(&self) -> Option<u8> {
        self.input.as_bytes().get(self.cursor).copied()
    }
}
struct SnapshotString<'a> {
    raw: &'a str,
}
fn count_borrowed_json_array_items(input: &str) -> Result<usize, TryReadError> {
    let mut parser = json::Parser::new(input);
    parser.expect(b'[').map_err(TryReadError::Serialization)?;
    parser.skip_ws();
    let mut count = 0_usize;
    if parser.peek() == Some(b']') {
        parser.bump();
    } else {
        loop {
            parser.skip_value().map_err(TryReadError::Serialization)?;
            count = count.checked_add(1).ok_or_else(|| {
                TryReadError::SnapshotResourceLimit(
                    "snapshot block-hash count overflowed".to_owned(),
                )
            })?;
            parser.skip_ws();
            match parser.bump() {
                Some(b',') => {}
                Some(b']') => break,
                _ => return Err(TryReadError::NonCanonicalSnapshotPayload),
            }
        }
    }
    parser.skip_ws();
    if !parser.eof() {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    Ok(count)
}
fn validate_snapshot_json_resources(
    bytes: &[u8],
    policy: SnapshotResourcePolicy,
) -> Result<SnapshotJsonSummary, TryReadError> {
    let input = std::str::from_utf8(bytes)
        .map_err(|_| TryReadError::Serialization(json::Error::InvalidUtf8))?;
    SnapshotJsonBudgetScanner::new(input, policy).scan()
}
fn snapshot_object_field_raw<'a>(
    object: &'a str,
    wanted: &str,
) -> Result<Option<&'a str>, TryReadError> {
    let mut parser = json::Parser::new(object);
    parser.expect(b'{').map_err(TryReadError::Serialization)?;
    parser.skip_ws();
    if parser.peek() == Some(b'}') {
        parser.bump();
        return Ok(None);
    }
    loop {
        let key = parser.parse_string().map_err(TryReadError::Serialization)?;
        parser.expect(b':').map_err(TryReadError::Serialization)?;
        parser.skip_ws();
        let start = parser.position();
        parser.skip_value().map_err(TryReadError::Serialization)?;
        let end = parser.position();
        if key == wanted {
            return Ok(Some(&object[start..end]));
        }
        parser.skip_ws();
        match parser.bump() {
            Some(b',') => {}
            Some(b'}') => return Ok(None),
            _ => return Err(TryReadError::NonCanonicalSnapshotPayload),
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum CanonicalWsvPath {
    Root,
    World,
    Parameters,
    Sumeragi,
    Other,
}
#[derive(Clone, Copy, Default)]
struct CanonicalWsvOverrides<'a> {
    committed_external_event_buf: Option<&'a str>,
}
struct BorrowedJsonMember<'a> {
    key: String,
    encoded_key: &'a str,
    value: &'a str,
}
fn borrowed_json_object_members(input: &str) -> Result<Vec<BorrowedJsonMember<'_>>, TryReadError> {
    let mut parser = json::Parser::new(input);
    parser.expect(b'{').map_err(TryReadError::Serialization)?;
    parser.skip_ws();
    let mut members = Vec::new();
    if parser.peek() == Some(b'}') {
        parser.bump();
    } else {
        loop {
            let key_start = parser.position();
            let key = parser.parse_string().map_err(TryReadError::Serialization)?;
            let key_end = parser.position();
            parser.expect(b':').map_err(TryReadError::Serialization)?;
            parser.skip_ws();
            let value_start = parser.position();
            parser.skip_value().map_err(TryReadError::Serialization)?;
            let value_end = parser.position();
            members.push(BorrowedJsonMember {
                key,
                encoded_key: &input[key_start..key_end],
                value: &input[value_start..value_end],
            });
            parser.skip_ws();
            match parser.bump() {
                Some(b',') => {}
                Some(b'}') => break,
                _ => return Err(TryReadError::NonCanonicalSnapshotPayload),
            }
        }
    }
    parser.skip_ws();
    if !parser.eof() {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    Ok(members)
}
fn borrowed_json_array_items(input: &str) -> Result<Vec<&str>, TryReadError> {
    let mut parser = json::Parser::new(input);
    parser.expect(b'[').map_err(TryReadError::Serialization)?;
    parser.skip_ws();
    let mut items = Vec::new();
    if parser.peek() == Some(b']') {
        parser.bump();
    } else {
        loop {
            let start = parser.position();
            parser.skip_value().map_err(TryReadError::Serialization)?;
            items.push(&input[start..parser.position()]);
            parser.skip_ws();
            match parser.bump() {
                Some(b',') => {}
                Some(b']') => break,
                _ => return Err(TryReadError::NonCanonicalSnapshotPayload),
            }
        }
    }
    parser.skip_ws();
    if !parser.eof() {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    Ok(items)
}
fn canonical_wsv_member_is_redacted(path: CanonicalWsvPath, key: &str) -> bool {
    match path {
        CanonicalWsvPath::Root => matches!(
            key,
            "sumeragi_v2_bootstrap" | "commit_topology" | "prev_commit_topology"
        ),
        CanonicalWsvPath::World => matches!(key, "consensus_evidence" | "vrf_epochs"),
        CanonicalWsvPath::Parameters | CanonicalWsvPath::Sumeragi | CanonicalWsvPath::Other => {
            false
        }
    }
}
fn canonical_wsv_child_path(path: CanonicalWsvPath, key: &str) -> CanonicalWsvPath {
    match (path, key) {
        (CanonicalWsvPath::Root, "world") => CanonicalWsvPath::World,
        (CanonicalWsvPath::World, "parameters") => CanonicalWsvPath::Parameters,
        (CanonicalWsvPath::Parameters, "sumeragi") => CanonicalWsvPath::Sumeragi,
        _ => CanonicalWsvPath::Other,
    }
}
fn canonical_wsv_cell_value<'a>(
    path: CanonicalWsvPath,
    key: &str,
    input: &'a str,
) -> Result<&'a str, TryReadError> {
    const WORLD_CELL_FIELDS: &[&str] = &[
        "parameters",
        "peers",
        "viral_reward_budget",
        "viral_campaign_budget",
        "executor",
        "executor_data_model",
        "merge_hint_roots",
        "merge_global_state_root",
        "governance_last_unlock_sweep_height",
        "external_event_buf",
    ];
    if path != CanonicalWsvPath::World
        || !WORLD_CELL_FIELDS.contains(&key)
        || !input.starts_with('{')
    {
        return Ok(input);
    }
    let members = borrowed_json_object_members(input)?;
    if !members.iter().any(|member| member.key == "revert") {
        return Ok(input);
    }
    Ok(members
        .iter()
        .find(|member| member.key == "blocks")
        .map_or(input, |member| member.value))
}

fn canonical_json_fragment(input: &str) -> Result<String, TryReadError> {
    let value: json::Value = json::from_str(input).map_err(TryReadError::Serialization)?;
    json::to_json(&value).map_err(TryReadError::Serialization)
}

fn update_snapshot_wsv_hash<'a>(
    hasher: &mut Blake2b<U32>,
    input: &'a str,
    path: CanonicalWsvPath,
    overrides: CanonicalWsvOverrides<'a>,
) -> Result<(), TryReadError> {
    match input.as_bytes().first().copied() {
        Some(b'{') => update_snapshot_wsv_object_hash(hasher, input, path, overrides),
        Some(b'[') => update_snapshot_wsv_array_hash(hasher, input, overrides),
        Some(_) => {
            // The staged and committed State serializers can spell an
            // otherwise identical scalar with different JSON escapes. Hash
            // the canonical semantic spelling so the pre-WSV checkpoint and
            // its post-commit confirmation cannot diverge on lexical trivia.
            let canonical = canonical_json_fragment(input)?;
            Digest::update(hasher, canonical.as_bytes());
            Ok(())
        }
        None => Err(TryReadError::NonCanonicalSnapshotPayload),
    }
}

fn update_snapshot_wsv_object_hash<'a>(
    hasher: &mut Blake2b<U32>,
    input: &'a str,
    path: CanonicalWsvPath,
    overrides: CanonicalWsvOverrides<'a>,
) -> Result<(), TryReadError> {
    let mut members = borrowed_json_object_members(input)?;
    if path == CanonicalWsvPath::World
        && !members
            .iter()
            .any(|member| member.key == "external_event_buf")
        && let Some(value) = overrides.committed_external_event_buf
    {
        // `WorldBlock` deliberately skips the process-owned event-buffer cell,
        // while committing the overlay leaves the live cell intact. Inject the
        // exact committed value into the staged hash before canonical sorting,
        // matching the tree reference and the post-commit State serializer.
        members.push(BorrowedJsonMember {
            key: "external_event_buf".to_owned(),
            encoded_key: r#""external_event_buf""#,
            value,
        });
    }
    members.sort_unstable_by(|left, right| left.key.cmp(&right.key));
    if members.windows(2).any(|pair| pair[0].key == pair[1].key) {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    Digest::update(hasher, b"{");
    let mut first = true;
    for member in members {
        if canonical_wsv_member_is_redacted(path, &member.key) {
            continue;
        }
        if !first {
            Digest::update(hasher, b",");
        }
        first = false;
        let canonical_key = canonical_json_fragment(member.encoded_key)?;
        Digest::update(hasher, canonical_key.as_bytes());
        Digest::update(hasher, b":");
        let serialized_value =
            if path == CanonicalWsvPath::World && member.key == "external_event_buf" {
                overrides
                    .committed_external_event_buf
                    .unwrap_or(member.value)
            } else {
                member.value
            };
        let value = canonical_wsv_cell_value(path, &member.key, serialized_value)?;
        if path == CanonicalWsvPath::Sumeragi
            && matches!(
                member.key.as_str(),
                "key_allowed_algorithms" | "key_allowed_hsm_providers"
            )
        {
            update_sorted_string_set_hash(hasher, value)?;
        } else {
            update_snapshot_wsv_hash(
                hasher,
                value,
                canonical_wsv_child_path(path, &member.key),
                overrides,
            )?;
        }
    }
    Digest::update(hasher, b"}");
    Ok(())
}

fn update_snapshot_wsv_array_hash<'a>(
    hasher: &mut Blake2b<U32>,
    input: &'a str,
    overrides: CanonicalWsvOverrides<'a>,
) -> Result<(), TryReadError> {
    let items = borrowed_json_array_items(input)?;
    Digest::update(hasher, b"[");
    for (index, item) in items.into_iter().enumerate() {
        if index != 0 {
            Digest::update(hasher, b",");
        }
        update_snapshot_wsv_hash(hasher, item, CanonicalWsvPath::Other, overrides)?;
    }
    Digest::update(hasher, b"]");
    Ok(())
}
fn update_sorted_string_set_hash(
    hasher: &mut Blake2b<U32>,
    input: &str,
) -> Result<(), TryReadError> {
    let items = borrowed_json_array_items(input)?;
    if items.iter().any(|item| !item.starts_with('"')) {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    let mut items = items
        .into_iter()
        .map(canonical_json_fragment)
        .collect::<Result<Vec<_>, _>>()?;
    if items.iter().any(|item| !item.starts_with('"')) {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    items.sort_unstable();
    items.dedup();
    Digest::update(hasher, b"[");
    for (index, item) in items.into_iter().enumerate() {
        if index != 0 {
            Digest::update(hasher, b",");
        }
        Digest::update(hasher, item.as_bytes());
    }
    Digest::update(hasher, b"]");
    Ok(())
}
fn canonical_snapshot_wsv_hash(bytes: &[u8]) -> Result<Hash, TryReadError> {
    canonical_snapshot_wsv_hash_with_overrides(bytes, CanonicalWsvOverrides::default())
}

fn canonical_snapshot_wsv_hash_with_overrides<'a>(
    bytes: &'a [u8],
    overrides: CanonicalWsvOverrides<'a>,
) -> Result<Hash, TryReadError> {
    let input = std::str::from_utf8(bytes)
        .map_err(|_| TryReadError::Serialization(json::Error::InvalidUtf8))?;
    let mut hasher = Blake2b::<U32>::new();
    update_snapshot_wsv_hash(&mut hasher, input, CanonicalWsvPath::Root, overrides)?;
    Ok(Hash::prehashed(hasher.finalize().into()))
}
fn validate_snapshot_sccp_registry_raw(input: &str) -> Result<(), TryReadError> {
    let Some(world) = snapshot_object_field_raw(input, "world")? else {
        return Ok(());
    };
    let Some(registry) = snapshot_object_field_raw(world, "sccp_registry")? else {
        return Ok(());
    };
    crate::state::validate_sccp_registry_cell_json_str(registry)
        .map_err(TryReadError::InvalidSccpRegistry)
}

fn snapshot_sccp_policy_hash_raw(input: &str) -> Result<[u8; 32], TryReadError> {
    validate_snapshot_sccp_registry_raw(input)?;
    let world = snapshot_object_field_raw(input, "world")?
        .ok_or_else(|| TryReadError::Serialization(json::Error::missing_field("world")))?;
    let registry = snapshot_object_field_raw(world, "sccp_registry")?.ok_or_else(|| {
        TryReadError::Serialization(json::Error::missing_field("world.sccp_registry"))
    })?;
    let cell: Cell<SccpRegistryV1> =
        json::from_str(registry).map_err(TryReadError::Serialization)?;
    let validated = ValidatedSccpRegistryV1::try_from_wire(cell.view().get().clone())
        .map_err(TryReadError::InvalidSccpRegistry)?;
    Ok(validated.policy_hash())
}
#[cfg(test)]
fn snapshot_has_space_directory_manifest_section(value: &json::Value) -> bool {
    matches!(
        value,
        json::Value::Object(map) if map.contains_key("space_directory_manifests")
    )
}
#[cfg(test)]
fn snapshot_world_has_field(value: &json::Value, field: &str) -> bool {
    matches!(
        value,
        json::Value::Object(map)
            if matches!(map.get("world"), Some(json::Value::Object(world)) if world.contains_key(field))
    )
}
fn reconcile_emergency_fast_snapshot_boundary(
    snapshot_height: usize,
    snapshot_tip: Option<HashOf<BlockHeader>>,
    block_count: usize,
    kura: &Kura,
    hard_fork_snapshot_bootstrap: bool,
) -> Result<(), TryReadError> {
    if hard_fork_snapshot_bootstrap {
        return Err(TryReadError::InvalidSnapshotBootstrap(
            "emergency Fast mode cannot import or extend an audited snapshot; restart in Strict mode"
                .to_owned(),
        ));
    }
    let (durable_height, durable_boundary_hash) = kura
        .emergency_fast_snapshot_boundary(snapshot_height)
        .map_err(TryReadError::Kura)?;
    if durable_height != block_count || snapshot_height != durable_height {
        return Err(TryReadError::MismatchedHeight {
            snapshot_height,
            kura_height: durable_height,
        });
    }
    match (snapshot_height, snapshot_tip, durable_boundary_hash) {
        (0, None, None) => {}
        (height @ 1.., Some(snapshot_block_hash), Some(kura_block_hash)) => {
            if snapshot_block_hash != kura_block_hash {
                return Err(TryReadError::MismatchedHash {
                    height,
                    snapshot_block_hash,
                    kura_block_hash,
                });
            }
        }
        (height, _, _) => return Err(TryReadError::MissingBlock { height }),
    }
    iroha_logger::warn!(
        snapshot_height,
        durable_height,
        "emergency Fast snapshot restore validated only the exact height boundary; Strict restart must audit the historical hash prefix"
    );
    Ok(())
}
fn reconcile_snapshot_hash_height_with_kura(
    snapshot_hashes: &[HashOf<BlockHeader>],
    block_count: usize,
    kura: &Kura,
    hard_fork_snapshot_bootstrap: bool,
    authenticated_payload: Option<&AuthenticatedSnapshotBootstrapPayload>,
) -> Result<(), TryReadError> {
    if kura.emergency_fast_startup_enabled() {
        return reconcile_emergency_fast_snapshot_boundary(
            snapshot_hashes.len(),
            snapshot_hashes.last().copied(),
            block_count,
            kura,
            hard_fork_snapshot_bootstrap,
        );
    }
    // Verify every retained Kura hash before extending its journal.  Keeping
    // the preflight inside this mutating helper makes it impossible for a
    // caller to persist an attacker-controlled suffix and only then discover
    // that the signed snapshot diverges inside the existing prefix.
    reconcile_snapshot_hashes_with_kura(snapshot_hashes, kura)?;
    let snapshot_height = snapshot_hashes.len();
    if hard_fork_snapshot_bootstrap {
        if snapshot_height < block_count {
            return Err(TryReadError::MismatchedHeight {
                snapshot_height,
                kura_height: block_count,
            });
        }
        let payload = authenticated_payload.ok_or_else(|| {
            TryReadError::InvalidSnapshotBootstrap(
                "audited Kura reconciliation lacks outer-authenticated snapshot evidence"
                    .to_owned(),
            )
        })?;
        let extended = kura
            .reconcile_exact_audited_snapshot_bootstrap(payload)
            .map_err(TryReadError::Kura)?;
        iroha_logger::warn!(
            snapshot_height,
            previous_kura_height = block_count,
            extended,
            "hard-fork snapshot bootstrap: accepted audited snapshot ahead of Kura block bodies"
        );
        return Ok(());
    }
    if snapshot_height > block_count {
        return Err(TryReadError::MismatchedHeight {
            snapshot_height,
            kura_height: block_count,
        });
    }
    Ok(())
}
fn reconcile_snapshot_hashes_with_kura(
    snapshot_hashes: &[HashOf<BlockHeader>],
    kura: &Kura,
) -> Result<(), TryReadError> {
    #[cfg(test)]
    SNAPSHOT_HASH_RECONCILIATION_PASSES.with(|passes| passes.set(passes.get() + 1));
    let kura_height = kura.blocks_count();
    for (idx, snapshot_block_hash) in snapshot_hashes.iter().copied().enumerate() {
        let height = idx + 1;
        let height_nz = NonZeroUsize::new(height).expect("iterating from 1");
        let kura_block_hash = match kura.block_hash_at_height(height_nz) {
            Some(hash) => hash,
            None if height > kura_height => break,
            None => return Err(TryReadError::MissingBlock { height }),
        };
        if kura_block_hash == snapshot_block_hash {
            continue;
        }
        return Err(TryReadError::MismatchedHash {
            height,
            snapshot_block_hash,
            kura_block_hash,
        });
    }
    Ok(())
}
fn validate_snapshot_wsv_checkpoint(
    snapshot_wsv_hash: Hash,
    snapshot_hashes: &[HashOf<BlockHeader>],
    kura: &Kura,
) -> Result<(), TryReadError> {
    let Some(&snapshot_block_hash) = snapshot_hashes.last() else {
        return Ok(());
    };
    let height = snapshot_hashes.len();
    let height_nz = NonZeroUsize::new(height).expect("snapshot height is nonzero");
    if kura.block_hash_at_height(height_nz) != Some(snapshot_block_hash) {
        // A verified snapshot-ahead suffix has no local checkpoint yet. A
        // digest-pinned hard-fork override can likewise replace the local
        // suffix. Prefix reconciliation authorizes those cases separately.
        return Ok(());
    }
    let height_u64 = u64::try_from(height).map_err(|_| {
        TryReadError::Serialization(json::Error::InvalidField {
            field: "state.block_hashes".to_owned(),
            message: "snapshot height exceeds the canonical u64 height domain".to_owned(),
        })
    })?;
    let Some(checkpoint) = kura
        .wsv_checkpoint(height_u64)
        .map_err(TryReadError::Kura)?
    else {
        return Ok(());
    };
    let expected = checkpoint.state_hash();
    let actual = snapshot_wsv_hash;
    if actual != expected {
        return Err(TryReadError::WsvCheckpointMismatch {
            height,
            expected,
            actual,
        });
    }
    Ok(())
}
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn try_read_snapshot_bundle<F>(
    generation: &BoundSnapshotGeneration,
    kura: &Arc<Kura>,
    live_query_store: &LiveQueryStoreHandle,
    block_count: usize,
    merkle_chunk_size: NonZeroUsize,
    resource_policy: SnapshotResourcePolicy,
    verification_key: &PublicKey,
    expected_network_id: &NetworkId,
    bootstrap_policy: &SnapshotBootstrapPolicy,
    initialize_state: &F,
    #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
) -> Result<SnapshotReadOutcome, TryReadError>
where
    F: Fn(&mut State) -> Result<(), TryReadError>,
{
    bootstrap_policy
        .validate()
        .map_err(TryReadError::InvalidSnapshotBootstrap)?;
    let emergency_fast = kura.emergency_fast_startup_enabled();
    let manifest_path = generation
        .generation_dir
        .join(SNAPSHOT_FAST_MANIFEST_FILE_NAME);
    let fast_manifest =
        decode_emergency_fast_manifest(&generation.bytes.fast_manifest, &manifest_path)?;
    if fast_manifest.payload_len != generation.payload.len {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: manifest_path,
            reason: "emergency Fast manifest payload length differs from snapshot.data".to_owned(),
        });
    }
    let digest_path = generation.generation_dir.join(SNAPSHOT_DIGEST_FILE_NAME);
    let signed_payload_digest =
        parse_snapshot_digest_bytes(&generation.bytes.digest, &digest_path)?;
    let actual_digest = hex::encode(signed_payload_digest);
    if !emergency_fast
        && generation
            .payload
            .bytes_sha256
            .expect("Strict snapshot generation binding hashes its payload")
            != signed_payload_digest
    {
        return Err(TryReadError::ChecksumMismatch {
            expected: String::from_utf8_lossy(&generation.bytes.digest).into_owned(),
            actual: hex::encode(
                generation
                    .payload
                    .bytes_sha256
                    .expect("Strict snapshot generation binding hashes its payload"),
            ),
        });
    }
    let signature_hex = std::str::from_utf8(&generation.bytes.signature).map_err(|_| {
        TryReadError::SignatureMalformed("snapshot signature is not UTF-8".to_owned())
    })?;
    let bundle_digest =
        snapshot_bundle_auth_digest(&signed_payload_digest, &generation.bytes.fast_manifest);
    if emergency_fast {
        verify_signature_hex(signature_hex, &bundle_digest, verification_key)?;
        if fast_manifest.has_snapshot_bootstrap_lineage
            || kura.provisional_snapshot_bootstrap_pending()
        {
            return Err(TryReadError::InvalidSnapshotBootstrap(
                "emergency Fast mode cannot authorize or continue hash-only snapshot bootstrap lineage; restart in Strict mode"
                    .to_owned(),
            ));
        }
        if &fast_manifest.network_id != expected_network_id {
            return Err(TryReadError::NetworkIdMismatch {
                expected: *expected_network_id,
                actual: fast_manifest.network_id,
            });
        }
        let snapshot_height = usize::try_from(fast_manifest.committed_height).map_err(|_| {
            TryReadError::InvalidSnapshotBootstrap(
                "emergency Fast manifest height exceeds this host's index width".to_owned(),
            )
        })?;
        let reconcile_started_at = Instant::now();
        reconcile_emergency_fast_snapshot_boundary(
            snapshot_height,
            fast_manifest.tip_hash,
            block_count,
            kura,
            false,
        )?;
        let seed = KuraSeed {
            kura: Arc::clone(kura),
            query_handle: live_query_store.clone(),
            #[cfg(feature = "telemetry")]
            telemetry,
        };
        let mut state = seed
            .into_state_from_emergency_fast_manifest(
                fast_manifest.chain_id.clone(),
                fast_manifest.network_id,
                snapshot_height,
                fast_manifest.tip_hash,
                fast_manifest.sccp_policy_hash,
            )
            .map_err(TryReadError::Serialization)?;
        initialize_state(&mut state)?;
        generation.verify_emergency_fast_selection_unchanged()?;
        iroha_logger::warn!(
            snapshot_height,
            kura_height = block_count,
            validation_ms = reconcile_started_at.elapsed().as_millis(),
            "emergency Fast restore verified only the bounded signed manifest and exact durable tip; snapshot.data and full World semantics remain deferred until Strict restart"
        );
        return Ok(SnapshotReadOutcome { state });
    }
    let payload_authority = match verify_signature_hex(
        signature_hex,
        &bundle_digest,
        verification_key,
    ) {
        Ok(()) => SnapshotPayloadAuthority::NormallySigned,
        Err(error) if bootstrap_policy.authorizes_digest(&actual_digest) => {
            warn!(
                ?error,
                digest = %actual_digest,
                "hard-fork snapshot bootstrap: accepting snapshot signature failure because SHA-256 matches configured audited digest"
            );
            SnapshotPayloadAuthority::ExactAuditedDigestBypass
        }
        Err(error) => return Err(error),
    };
    let payload = read_bound_snapshot_payload(&generation.payload)?.0;
    let bytes = payload.as_slice();
    let bytes_len = bytes.len();
    let payload_preview = snapshot_payload_preview(bytes);
    #[cfg(test)]
    SNAPSHOT_DEEP_VALIDATION_PASSES.with(|passes| passes.set(passes.get() + 1));
    let merkle_path = generation.generation_dir.join(SNAPSHOT_MERKLE_FILE_NAME);
    let merkle_value = json::from_slice::<json::Value>(&generation.bytes.merkle)
        .map_err(TryReadError::MerkleMetadata)?;
    let merkle = SnapshotMerkleMetadata::from_json_value(merkle_value)
        .map_err(|error| merkle_err_to_try_read(error, merkle_path.clone()))?;
    let canonical_merkle = json::to_json(&merkle).map_err(TryReadError::MerkleMetadata)?;
    if canonical_merkle.as_bytes() != generation.bytes.merkle {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: merkle_path,
            reason: "Merkle metadata is not canonical JSON".to_owned(),
        });
    }
    merkle
        .verify_against_bytes(bytes, merkle_chunk_size)
        .map_err(|error| merkle_err_to_try_read(error, generation.generation_dir.clone()))?;
    let summary = match validate_snapshot_json_resources(bytes, resource_policy) {
        Ok(summary) => summary,
        Err(err) => {
            iroha_logger::warn!(
                ?err,
                bytes_len,
                digest = %actual_digest,
                preview = %payload_preview,
                "snapshot JSON parse failed"
            );
            return Err(err);
        }
    };
    let snapshot_wsv_hash = canonical_snapshot_wsv_hash(bytes)?;
    if !summary.has_space_directory_manifests
        && let Some(snapshot_height @ 1..) = summary.block_hash_count
    {
        return Err(TryReadError::MissingSpaceDirectoryManifestSection { snapshot_height });
    }
    let input = std::str::from_utf8(bytes)
        .map_err(|_| TryReadError::Serialization(json::Error::InvalidUtf8))?;
    // Check the governed registry before constructing any live State. Both
    // cell roles decode directly into their final typed registry owners.
    validate_snapshot_sccp_registry_raw(input)?;
    let seed = KuraSeed {
        kura: Arc::clone(kura),
        query_handle: live_query_store.clone(),
        #[cfg(feature = "telemetry")]
        telemetry,
    };
    let decoded_state = seed.into_state_from_json_str(input);
    let mut state = decoded_state.map_err(|err| {
        iroha_logger::warn!(
            ?err,
            bytes_len,
            digest = %actual_digest,
            preview = %payload_preview,
            "snapshot state deserialization failed"
        );
        TryReadError::Serialization(err)
    })?;
    if &state.network_id != expected_network_id {
        return Err(TryReadError::NetworkIdMismatch {
            expected: *expected_network_id,
            actual: state.network_id,
        });
    }
    #[cfg(test)]
    SNAPSHOT_BLOCK_HASH_VECTOR_CLONES.with(|clones| clones.set(clones.get() + 1));
    let snapshot_hashes = state.committed_block_hashes_snapshot();
    let snapshot_height = snapshot_hashes.len();
    let snapshot_height_u64 = u64::try_from(snapshot_height).map_err(|_| {
        TryReadError::InvalidSnapshotBootstrap(
            "snapshot height exceeds the canonical u64 height domain".to_owned(),
        )
    })?;
    let exact_policy_boundary = bootstrap_policy.authorizes(&actual_digest, snapshot_height_u64);
    let has_bootstrap_lineage = state.has_snapshot_v2_bootstrap_candidate();
    if fast_manifest.chain_id != *state.chain_id_ref()
        || fast_manifest.network_id != *state.network_id_ref()
        || fast_manifest.committed_height != snapshot_height_u64
        || fast_manifest.tip_hash != snapshot_hashes.last().copied()
        || fast_manifest.sccp_policy_hash != state.sccp_policy_hash_snapshot()
        || fast_manifest.has_snapshot_bootstrap_lineage != has_bootstrap_lineage
    {
        return Err(TryReadError::SnapshotGenerationInvalid {
            path: generation
                .generation_dir
                .join(SNAPSHOT_FAST_MANIFEST_FILE_NAME),
            reason: "signed emergency Fast manifest differs from the fully decoded snapshot"
                .to_owned(),
        });
    }
    if payload_authority == SnapshotPayloadAuthority::ExactAuditedDigestBypass
        && !exact_policy_boundary
    {
        return Err(TryReadError::InvalidSnapshotBootstrap(
            "snapshot signature bypass matched only the configured digest but not its exact audited height"
                .to_owned(),
        ));
    }
    let authenticated_lineage_authority = if has_bootstrap_lineage {
        let lineage_authority = if exact_policy_boundary {
            SnapshotBootstrapLineageAuthority::exact_audited_boundary()
        } else {
            if payload_authority != SnapshotPayloadAuthority::NormallySigned {
                return Err(TryReadError::InvalidSnapshotBootstrap(
                    "carried bootstrap lineage requires an ordinarily verified snapshot signature"
                        .to_owned(),
                ));
            }
            SnapshotBootstrapLineageAuthority::normally_signed_carried_lineage()
        };
        state
            .authenticate_snapshot_v2_bootstrap_candidate(lineage_authority)
            .map_err(TryReadError::InvalidSnapshotBootstrap)?;
        Some(lineage_authority)
    } else if payload_authority == SnapshotPayloadAuthority::ExactAuditedDigestBypass {
        return Err(TryReadError::InvalidSnapshotBootstrap(
            "signature-bypassed snapshot is missing its typed Sumeragi-v2 bootstrap envelope"
                .to_owned(),
        ));
    } else if kura.provisional_snapshot_bootstrap_pending() {
        return Err(TryReadError::InvalidSnapshotBootstrap(
            "provisional hash-only Kura requires carried bootstrap lineage in the signed snapshot"
                .to_owned(),
        ));
    } else {
        None
    };
    if let Some(authority) = authenticated_lineage_authority {
        let record = state
            .authenticated_snapshot_v2_bootstrap()
            .expect("validated bootstrap candidate was promoted")
            .clone();
        state
            .install_authenticated_snapshot_bootstrap_payload(
                AuthenticatedSnapshotBootstrapPayload::new(
                    record,
                    snapshot_hashes.clone(),
                    authority,
                ),
            )
            .map_err(TryReadError::InvalidSnapshotBootstrap)?;
    }
    let hard_fork_snapshot_bootstrap = exact_policy_boundary && has_bootstrap_lineage;
    if snapshot_height > 0 && !summary.has_space_directory_manifests {
        return Err(TryReadError::MissingSpaceDirectoryManifestSection { snapshot_height });
    }
    // Runtime configuration and the one-block SCCP rollback candidate are semantic checks on the
    // newly decoded, still-isolated state. Canonicality was enforced while each
    // borrowed field was typed-decoded, so no second full payload is built here.
    // All checks remain ahead of snapshot-driven Kura extension or pruning.
    initialize_state(&mut state)?;
    crate::state::validate_sccp_snapshot_revert_candidate(&state)
        .map_err(TryReadError::InvalidSccpRevert)?;
    validate_snapshot_wsv_checkpoint(snapshot_wsv_hash, &snapshot_hashes, kura)?;
    generation.verify_selection_unchanged()?;
    let hash_reconcile_started_at = Instant::now();
    reconcile_snapshot_hash_height_with_kura(
        &snapshot_hashes,
        block_count,
        kura,
        hard_fork_snapshot_bootstrap,
        state.authenticated_snapshot_bootstrap_payload(),
    )?;
    iroha_logger::info!(
        snapshot_height,
        kura_height = block_count,
        validation_ms = hash_reconcile_started_at.elapsed().as_millis(),
        "Validated snapshot block hashes against Kura"
    );
    generation.verify_generation_unchanged()?;
    Ok(SnapshotReadOutcome { state })
}
/// Deserialize [`State`] and install the actual runtime ZK configuration
/// before snapshot reconciliation is allowed to mutate Kura.
///
/// # Errors
///
/// Returns all ordinary snapshot read errors, plus
/// [`TryReadError::ZkConfigInstall`] when the decoded committed SCCP outbox is
/// incompatible with the actual configured pending limits.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_pass_by_value)]
pub fn try_read_snapshot(
    store_dir: impl AsRef<Path>,
    kura: &Arc<Kura>,
    live_query_store_lazy: impl FnOnce() -> LiveQueryStoreHandle,
    block_count: BlockCount,
    merkle_chunk_size: NonZeroUsize,
    verification_key: &PublicKey,
    expected_network_id: &NetworkId,
    zk: &iroha_config::parameters::actual::Zk,
    #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
) -> Result<State, TryReadError> {
    let bootstrap_policy = SnapshotBootstrapPolicy::default();
    try_read_snapshot_with_bootstrap_policy(
        store_dir,
        kura,
        live_query_store_lazy,
        block_count,
        merkle_chunk_size,
        iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES,
        SnapshotResourcePolicy::default(),
        verification_key,
        expected_network_id,
        zk,
        &bootstrap_policy,
        #[cfg(feature = "telemetry")]
        telemetry,
    )
}
/// Read and verify a snapshot with an explicit audited hash-only bootstrap policy.
///
/// The policy is fail-closed: a bootstrap envelope or signature bypass is accepted only when the
/// payload's exact SHA-256 digest and terminal height match the configured authorization.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_pass_by_value)]
pub fn try_read_snapshot_with_bootstrap_policy(
    store_dir: impl AsRef<Path>,
    kura: &Arc<Kura>,
    live_query_store_lazy: impl FnOnce() -> LiveQueryStoreHandle,
    block_count: BlockCount,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
    resource_policy: SnapshotResourcePolicy,
    verification_key: &PublicKey,
    expected_network_id: &NetworkId,
    zk: &iroha_config::parameters::actual::Zk,
    bootstrap_policy: &SnapshotBootstrapPolicy,
    #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
) -> Result<State, TryReadError> {
    try_read_snapshot_with_initializer(
        store_dir,
        kura,
        live_query_store_lazy,
        block_count,
        merkle_chunk_size,
        max_payload_bytes,
        resource_policy,
        verification_key,
        expected_network_id,
        bootstrap_policy,
        &|state| {
            state
                .set_zk(zk.clone())
                .map_err(TryReadError::ZkConfigInstall)
        },
        #[cfg(feature = "telemetry")]
        telemetry,
    )
}
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_pass_by_value)]
fn try_read_snapshot_with_initializer<F>(
    store_dir: impl AsRef<Path>,
    kura: &Arc<Kura>,
    live_query_store_lazy: impl FnOnce() -> LiveQueryStoreHandle,
    BlockCount(block_count): BlockCount,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
    resource_policy: SnapshotResourcePolicy,
    verification_key: &PublicKey,
    expected_network_id: &NetworkId,
    bootstrap_policy: &SnapshotBootstrapPolicy,
    initialize_state: &F,
    #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
) -> Result<State, TryReadError>
where
    F: Fn(&mut State) -> Result<(), TryReadError>,
{
    let store_dir = store_dir.as_ref();
    if matches!(
        std::fs::symlink_metadata(store_dir),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound
    ) {
        return Err(TryReadError::NotFound);
    }
    let payload_limit = u64::try_from(
        max_payload_bytes
            .get()
            .min(resource_policy.max_transient_bytes.get()),
    )
    .unwrap_or(u64::MAX);
    let emergency_fast = kura.emergency_fast_startup_enabled();
    let generation = if emergency_fast {
        bind_current_snapshot_generation_emergency_fast(
            store_dir,
            payload_limit,
            merkle_chunk_size,
        )?
    } else {
        bind_current_snapshot_generation(store_dir, payload_limit, merkle_chunk_size)?
    };
    let live_query_store = live_query_store_lazy();
    let outcome = try_read_snapshot_bundle(
        &generation,
        kura,
        &live_query_store,
        block_count,
        merkle_chunk_size,
        resource_policy,
        verification_key,
        expected_network_id,
        bootstrap_policy,
        initialize_state,
        #[cfg(feature = "telemetry")]
        telemetry,
    )?;
    if !emergency_fast {
        generation.verify_generation_unchanged()?;
    }
    Ok(outcome.state)
}
fn snapshot_publication_error(context: &str, error: impl std::fmt::Display) -> TryWriteError {
    TryWriteError::PublicationIntegrity(format!("{context}: {error}"))
}
fn create_synced_snapshot_temp(
    store_dir: &Path,
    directory_identity: StableSnapshotFileIdentity,
    label: &str,
    bytes: &[u8],
    max_bytes: u64,
) -> Result<(tempfile::NamedTempFile, BoundSnapshotFile), TryWriteError> {
    if direct_snapshot_directory_identity(store_dir)
        .map_err(|error| snapshot_publication_error("verify snapshot directory", error))?
        != directory_identity
    {
        return Err(snapshot_publication_error(
            "create snapshot temp",
            "snapshot directory identity changed",
        ));
    }
    let mut temp = tempfile::Builder::new()
        .prefix(&format!(".snapshot-{label}-"))
        .tempfile_in(store_dir)
        .map_err(|error| TryWriteError::IO(error, store_dir.to_path_buf()))?;
    temp.as_file_mut()
        .write_all(bytes)
        .map_err(|error| TryWriteError::IO(error, temp.path().to_path_buf()))?;
    temp.as_file_mut()
        .flush()
        .map_err(|error| TryWriteError::IO(error, temp.path().to_path_buf()))?;
    temp.as_file()
        .sync_data()
        .map_err(|error| TryWriteError::IO(error, temp.path().to_path_buf()))?;
    let Some((binding, readback)) = bind_snapshot_file(temp.path(), max_bytes)
        .map_err(|error| snapshot_publication_error("bind snapshot temp", error))?
    else {
        return Err(snapshot_publication_error(
            "bind snapshot temp",
            "newly created temp disappeared",
        ));
    };
    if readback != bytes {
        return Err(snapshot_publication_error(
            "verify snapshot temp",
            "synced bytes differ from the intended artifact",
        ));
    }
    if direct_snapshot_directory_identity(store_dir)
        .map_err(|error| snapshot_publication_error("reverify snapshot directory", error))?
        != directory_identity
    {
        return Err(snapshot_publication_error(
            "create snapshot temp",
            "snapshot directory identity changed",
        ));
    }
    Ok((temp, binding))
}
struct PublishedSnapshotGeneration {
    generations_dir: PathBuf,
    generations_dir_identity: StableSnapshotFileIdentity,
    generation_dir: PathBuf,
    generation_dir_identity: StableSnapshotFileIdentity,
    artifacts: Vec<BoundSnapshotFile>,
    name: String,
}
impl PublishedSnapshotGeneration {
    fn verify_unchanged(&self) -> Result<(), TryWriteError> {
        if direct_snapshot_directory_identity(&self.generations_dir)
            .map_err(|error| snapshot_publication_error("verify generations directory", error))?
            != self.generations_dir_identity
            || direct_snapshot_directory_identity(&self.generation_dir)
                .map_err(|error| snapshot_publication_error("verify generation directory", error))?
                != self.generation_dir_identity
            || !snapshot_generation_has_exact_artifact_inventory(&self.generation_dir)
                .map_err(|error| snapshot_publication_error("inventory generation", error))?
        {
            return Err(snapshot_publication_error(
                "verify snapshot generation",
                "directory identity changed",
            ));
        }
        for artifact in &self.artifacts {
            verify_bound_snapshot_file(artifact)
                .map_err(|error| snapshot_publication_error("verify generation artifact", error))?;
        }
        Ok(())
    }
}
#[derive(Clone)]
struct BoundSnapshotDeletionFile {
    path: PathBuf,
    identity: StableSnapshotFileIdentity,
    len: u64,
}
fn canonical_snapshot_digest_name(name: &str) -> bool {
    hex::decode(name).is_ok_and(|bytes| bytes.len() == Hash::LENGTH && hex::encode(bytes) == name)
}
fn snapshot_generation_has_exact_artifact_inventory(path: &Path) -> Result<bool, TryReadError> {
    let expected = BTreeSet::from([
        SNAPSHOT_FILE_NAME.to_owned(),
        SNAPSHOT_DIGEST_FILE_NAME.to_owned(),
        SNAPSHOT_SIGNATURE_FILE_NAME.to_owned(),
        SNAPSHOT_FAST_MANIFEST_FILE_NAME.to_owned(),
        SNAPSHOT_MERKLE_FILE_NAME.to_owned(),
    ]);
    let mut actual = BTreeSet::new();
    for entry in
        std::fs::read_dir(path).map_err(|error| TryReadError::IO(error, path.to_path_buf()))?
    {
        if actual.len() == expected.len() {
            return Ok(false);
        }
        let entry = entry.map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Ok(false);
        };
        if !actual.insert(name) {
            return Ok(false);
        }
    }
    Ok(actual == expected)
}
fn snapshot_generation_is_canonical_for_gc(
    path: &Path,
    generation_name: &str,
    max_payload_bytes: NonZeroUsize,
    merkle_chunk_size: NonZeroUsize,
    verification_key: &PublicKey,
) -> bool {
    let validate = || -> Result<(), TryReadError> {
        let directory_identity = direct_snapshot_directory_identity(path)?;
        if !snapshot_generation_has_exact_artifact_inventory(path)? {
            return Err(TryReadError::SnapshotGenerationInvalid {
                path: path.to_path_buf(),
                reason: "generation does not contain exactly the five canonical artifacts"
                    .to_owned(),
            });
        }
        let payload_path = path.join(SNAPSHOT_FILE_NAME);
        let payload = bind_snapshot_file_handle(
            &payload_path,
            u64::try_from(max_payload_bytes.get()).unwrap_or(u64::MAX),
        )
        .map_err(|error| TryReadError::IO(error, payload_path.clone()))?
        .ok_or_else(|| TryReadError::SnapshotGenerationInvalid {
            path: payload_path,
            reason: "required generation artifact is missing".to_owned(),
        })?;
        let (digest, digest_bytes) = bind_required_snapshot_file(
            &path.join(SNAPSHOT_DIGEST_FILE_NAME),
            SNAPSHOT_DIGEST_MAX_BYTES,
        )?;
        let (signature, signature_bytes) = bind_required_snapshot_file(
            &path.join(SNAPSHOT_SIGNATURE_FILE_NAME),
            SNAPSHOT_SIGNATURE_MAX_BYTES,
        )?;
        let manifest_path = path.join(SNAPSHOT_FAST_MANIFEST_FILE_NAME);
        let (fast_manifest_file, fast_manifest_bytes) =
            bind_required_snapshot_file(&manifest_path, SNAPSHOT_FAST_MANIFEST_MAX_BYTES)?;
        let fast_manifest = decode_emergency_fast_manifest(&fast_manifest_bytes, &manifest_path)?;
        if fast_manifest.payload_len != payload.len {
            return Err(TryReadError::SnapshotGenerationInvalid {
                path: manifest_path,
                reason: "emergency Fast manifest payload length differs from snapshot.data"
                    .to_owned(),
            });
        }
        let merkle_limit = snapshot_merkle_max_bytes(payload.len, merkle_chunk_size);
        let (merkle_file, merkle_bytes) =
            bind_required_snapshot_file(&path.join(SNAPSHOT_MERKLE_FILE_NAME), merkle_limit)?;
        let payload_digest = payload
            .bytes_sha256
            .expect("ordinary snapshot generation binding hashes its payload");
        if hex::encode(&payload_digest) != generation_name
            || digest_bytes != format!("{generation_name}\n").as_bytes()
        {
            return Err(TryReadError::SnapshotGenerationInvalid {
                path: path.to_path_buf(),
                reason: "payload, digest sidecar, and generation name disagree".to_owned(),
            });
        }
        let signature_hex = std::str::from_utf8(&signature_bytes).map_err(|_| {
            TryReadError::SignatureMalformed("snapshot signature is not UTF-8".to_owned())
        })?;
        let bundle_digest = snapshot_bundle_auth_digest(&payload_digest, &fast_manifest_bytes);
        verify_signature_hex(signature_hex, &bundle_digest, verification_key)?;
        let merkle_value =
            json::from_slice::<json::Value>(&merkle_bytes).map_err(TryReadError::MerkleMetadata)?;
        let metadata = SnapshotMerkleMetadata::from_json_value(merkle_value)
            .map_err(|error| merkle_err_to_try_read(error, merkle_file.path.clone()))?;
        let canonical_merkle = json::to_json(&metadata).map_err(TryReadError::MerkleMetadata)?;
        if canonical_merkle.as_bytes() != merkle_bytes {
            return Err(TryReadError::SnapshotGenerationInvalid {
                path: merkle_file.path.clone(),
                reason: "Merkle metadata is not canonical JSON".to_owned(),
            });
        }
        let payload_bytes = read_bound_snapshot_payload(&payload)?.0;
        metadata
            .verify_against_bytes(&payload_bytes, merkle_chunk_size)
            .map_err(|error| merkle_err_to_try_read(error, merkle_file.path.clone()))?;
        if direct_snapshot_directory_identity(path)? != directory_identity
            || !snapshot_generation_has_exact_artifact_inventory(path)?
        {
            return Err(TryReadError::SnapshotGenerationInvalid {
                path: path.to_path_buf(),
                reason: "generation directory identity changed during GC validation".to_owned(),
            });
        }
        for artifact in [
            &payload,
            &digest,
            &signature,
            &fast_manifest_file,
            &merkle_file,
        ] {
            verify_bound_snapshot_file(artifact)?;
        }
        Ok(())
    };
    validate().is_ok()
}
fn bind_snapshot_generation_gc_removal(
    generations_dir: &Path,
    generations_dir_identity: StableSnapshotFileIdentity,
    path: &Path,
    require_complete: bool,
) -> Result<Option<SnapshotGenerationGcRemoval>, TryWriteError> {
    if direct_snapshot_directory_identity(generations_dir)
        .map_err(|error| snapshot_publication_error("bind generations during GC", error))?
        != generations_dir_identity
    {
        return Err(snapshot_publication_error(
            "snapshot generation GC",
            "generations directory identity changed",
        ));
    }
    let directory_identity = direct_snapshot_directory_identity(path)
        .map_err(|error| snapshot_publication_error("bind GC generation", error))?;
    let allowed = [
        SNAPSHOT_FILE_NAME,
        SNAPSHOT_DIGEST_FILE_NAME,
        SNAPSHOT_SIGNATURE_FILE_NAME,
        SNAPSHOT_FAST_MANIFEST_FILE_NAME,
        SNAPSHOT_MERKLE_FILE_NAME,
    ];
    let mut files = Vec::new();
    let mut names = BTreeSet::new();
    let entries =
        std::fs::read_dir(path).map_err(|error| TryWriteError::IO(error, path.to_path_buf()))?;
    for entry in entries {
        let entry = entry.map_err(|error| TryWriteError::IO(error, path.to_path_buf()))?;
        if files.len() >= allowed.len() {
            return Ok(None);
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            return Ok(None);
        };
        if !allowed.contains(&name.as_str()) || !names.insert(name) {
            return Ok(None);
        }
        let artifact_path = entry.path();
        let metadata = secure_file_metadata::from_path(&artifact_path)
            .map_err(|error| TryWriteError::IO(error, artifact_path.clone()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || !regular_file_has_single_link(&metadata)
            || !stable_file_identity_available(stable_file_identity(&metadata))
        {
            return Ok(None);
        }
        files.push(BoundSnapshotDeletionFile {
            path: artifact_path,
            identity: stable_file_identity(&metadata),
            len: metadata.len(),
        });
    }
    if require_complete && names.len() != allowed.len() {
        return Ok(None);
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    let removal = SnapshotGenerationGcRemoval {
        path: path.to_path_buf(),
        directory_identity,
        files,
    };
    verify_snapshot_generation_gc_removal(generations_dir, generations_dir_identity, &removal, 0)?;
    Ok(Some(removal))
}
fn verify_snapshot_generation_gc_removal(
    generations_dir: &Path,
    generations_dir_identity: StableSnapshotFileIdentity,
    removal: &SnapshotGenerationGcRemoval,
    removed_files: usize,
) -> Result<(), TryWriteError> {
    if direct_snapshot_directory_identity(generations_dir)
        .map_err(|error| snapshot_publication_error("verify generations during GC", error))?
        != generations_dir_identity
        || direct_snapshot_directory_identity(&removal.path)
            .map_err(|error| snapshot_publication_error("verify generation during GC", error))?
            != removal.directory_identity
    {
        return Err(snapshot_publication_error(
            "snapshot generation GC",
            "directory identity changed",
        ));
    }
    let expected_names = removal.files[removed_files..]
        .iter()
        .map(|file| {
            file.path
                .file_name()
                .expect("bound snapshot deletion artifact has a name")
                .to_os_string()
        })
        .collect::<BTreeSet<_>>();
    let mut actual_names = BTreeSet::new();
    for entry in std::fs::read_dir(&removal.path)
        .map_err(|error| TryWriteError::IO(error, removal.path.clone()))?
    {
        if actual_names.len() == expected_names.len() {
            return Err(snapshot_publication_error(
                "snapshot generation GC",
                "generation inventory changed before removal",
            ));
        }
        let entry = entry.map_err(|error| TryWriteError::IO(error, removal.path.clone()))?;
        if !actual_names.insert(entry.file_name()) {
            return Err(snapshot_publication_error(
                "snapshot generation GC",
                "generation inventory contains duplicate names",
            ));
        }
    }
    if actual_names != expected_names {
        return Err(snapshot_publication_error(
            "snapshot generation GC",
            "generation inventory changed before removal",
        ));
    }
    for file in &removal.files[removed_files..] {
        let metadata = secure_file_metadata::from_path(&file.path)
            .map_err(|error| TryWriteError::IO(error, file.path.clone()))?;
        if metadata.file_type().is_symlink()
            || !metadata.is_file()
            || !regular_file_has_single_link(&metadata)
            || stable_file_identity(&metadata) != file.identity
            || metadata.len() != file.len
        {
            return Err(snapshot_publication_error(
                "snapshot generation GC",
                "artifact identity changed before removal",
            ));
        }
    }
    Ok(())
}
fn remove_bound_snapshot_generation_directory(
    generations_dir: &Path,
    generations_dir_identity: StableSnapshotFileIdentity,
    removal: &SnapshotGenerationGcRemoval,
) -> Result<(), TryWriteError> {
    verify_snapshot_generation_gc_removal(generations_dir, generations_dir_identity, removal, 0)?;
    for (index, file) in removal.files.iter().enumerate() {
        verify_snapshot_generation_gc_removal(
            generations_dir,
            generations_dir_identity,
            removal,
            index,
        )?;
        std::fs::remove_file(&file.path)
            .map_err(|error| TryWriteError::IO(error, file.path.clone()))?;
    }
    verify_snapshot_generation_gc_removal(
        generations_dir,
        generations_dir_identity,
        removal,
        removal.files.len(),
    )?;
    sync_snapshot_directory(&removal.path, removal.directory_identity)
        .map_err(|error| snapshot_publication_error("sync emptied generation", error))?;
    if direct_snapshot_directory_identity(&removal.path)
        .map_err(|error| snapshot_publication_error("reverify emptied generation", error))?
        != removal.directory_identity
    {
        return Err(snapshot_publication_error(
            "snapshot generation GC",
            "generation identity changed before directory removal",
        ));
    }
    std::fs::remove_dir(&removal.path)
        .map_err(|error| TryWriteError::IO(error, removal.path.clone()))?;
    sync_snapshot_directory(generations_dir, generations_dir_identity)
        .map_err(|error| snapshot_publication_error("sync generations after GC", error))?;
    Ok(())
}
struct SnapshotGenerationGcRemoval {
    path: PathBuf,
    directory_identity: StableSnapshotFileIdentity,
    files: Vec<BoundSnapshotDeletionFile>,
}
struct SnapshotGenerationGcPlan {
    removals: Vec<SnapshotGenerationGcRemoval>,
}
fn plan_snapshot_generation_gc(
    generation: &PublishedSnapshotGeneration,
    previous_generation: Option<&str>,
    max_payload_bytes: NonZeroUsize,
    merkle_chunk_size: NonZeroUsize,
    verification_key: &PublicKey,
) -> Result<SnapshotGenerationGcPlan, TryWriteError> {
    generation.verify_unchanged()?;
    let mut entries = Vec::with_capacity(SNAPSHOT_GENERATION_GC_MAX_ENTRIES.min(64));
    for entry in std::fs::read_dir(&generation.generations_dir)
        .map_err(|error| TryWriteError::IO(error, generation.generations_dir.clone()))?
    {
        if entries.len() == SNAPSHOT_GENERATION_GC_MAX_ENTRIES {
            return Err(snapshot_publication_error(
                "snapshot generation GC",
                "generation entry count exceeds the hard scan bound",
            ));
        }
        entries.push(
            entry.map_err(|error| TryWriteError::IO(error, generation.generations_dir.clone()))?,
        );
    }
    entries.sort_by_key(std::fs::DirEntry::file_name);
    let fallback_previous = if previous_generation == Some(generation.name.as_str()) {
        // An idempotent publication records the current name as both the old
        // and new pointer. Under normal GC there is at most one other
        // authenticated generation, which is therefore the rollback
        // predecessor. If a crash or operator intervention left multiple
        // authenticated extras, chronology is unknowable from the v1 pointer;
        // preserve all of them and fail instead of selecting by directory order.
        let authenticated_extras = entries
            .iter()
            .filter_map(|entry| {
                let name = entry.file_name();
                let name = name.to_str()?;
                if name == generation.name || !canonical_snapshot_digest_name(name) {
                    return None;
                }
                let file_type = entry.file_type().ok()?;
                if !file_type.is_dir() || file_type.is_symlink() {
                    return None;
                }
                snapshot_generation_is_canonical_for_gc(
                    &entry.path(),
                    name,
                    max_payload_bytes,
                    merkle_chunk_size,
                    verification_key,
                )
                .then(|| name.to_owned())
            })
            .collect::<Vec<_>>();
        if authenticated_extras.len() > 1 {
            return Err(snapshot_publication_error(
                "snapshot generation GC",
                "multiple authenticated rollback candidates make previous-generation chronology ambiguous",
            ));
        }
        authenticated_extras.into_iter().next()
    } else {
        None
    };
    let mut removals = Vec::new();
    for entry in entries {
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        if name == generation.name
            || previous_generation == Some(name.as_str())
            || fallback_previous.as_deref() == Some(name.as_str())
        {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| TryWriteError::IO(error, entry.path()))?;
        if !file_type.is_dir() || file_type.is_symlink() {
            continue;
        }
        let is_digest_generation = canonical_snapshot_digest_name(&name);
        if !is_digest_generation && !name.starts_with(".snapshot-generation-") {
            continue;
        }
        let Some(removal) = bind_snapshot_generation_gc_removal(
            &generation.generations_dir,
            generation.generations_dir_identity,
            &entry.path(),
            is_digest_generation,
        )?
        else {
            continue;
        };
        if is_digest_generation {
            if !snapshot_generation_is_canonical_for_gc(
                &entry.path(),
                &name,
                max_payload_bytes,
                merkle_chunk_size,
                verification_key,
            ) {
                continue;
            }
            // Semantic authentication and removal binding are separate checks:
            // reverify the captured identities after authenticating so a
            // same-path replacement can never become a GC target.
            verify_snapshot_generation_gc_removal(
                &generation.generations_dir,
                generation.generations_dir_identity,
                &removal,
                0,
            )?;
        }
        removals.push(removal);
    }
    generation.verify_unchanged()?;
    Ok(SnapshotGenerationGcPlan { removals })
}
fn execute_snapshot_generation_gc(
    generation: &PublishedSnapshotGeneration,
    plan: &SnapshotGenerationGcPlan,
) -> Result<(), TryWriteError> {
    #[cfg(test)]
    let failure_stage = SNAPSHOT_GC_FAILURE_STAGE.with(|stage| stage.replace(0));
    #[cfg(test)]
    if failure_stage == 1 {
        return Err(snapshot_publication_error(
            "snapshot generation GC",
            "injected post-publication removal failure",
        ));
    }
    #[cfg(test)]
    if failure_stage == 3 {
        if let Some(removal) = plan.removals.first() {
            let displaced = removal.path.with_extension("gc-displaced");
            std::fs::rename(&removal.path, &displaced)
                .map_err(|error| TryWriteError::IO(error, removal.path.clone()))?;
            std::fs::create_dir(&removal.path)
                .map_err(|error| TryWriteError::IO(error, removal.path.clone()))?;
            for file in &removal.files {
                let name = file
                    .path
                    .file_name()
                    .expect("bound snapshot deletion artifact has a name");
                std::fs::copy(displaced.join(name), removal.path.join(name))
                    .map_err(|error| TryWriteError::IO(error, removal.path.join(name)))?;
            }
        }
    }
    generation.verify_unchanged()?;
    for removal in &plan.removals {
        remove_bound_snapshot_generation_directory(
            &generation.generations_dir,
            generation.generations_dir_identity,
            removal,
        )?;
        #[cfg(test)]
        if failure_stage == 2
            && plan
                .removals
                .first()
                .is_some_and(|first| std::ptr::eq(first, removal))
        {
            return Err(snapshot_publication_error(
                "snapshot generation GC",
                "injected post-publication directory-sync failure",
            ));
        }
    }
    generation.verify_unchanged()
}
fn create_generation_artifact(
    generation_dir: &Path,
    generation_dir_identity: StableSnapshotFileIdentity,
    name: &str,
    bytes: &[u8],
    max_bytes: u64,
) -> Result<BoundSnapshotFile, TryWriteError> {
    if direct_snapshot_directory_identity(generation_dir)
        .map_err(|error| snapshot_publication_error("verify generation directory", error))?
        != generation_dir_identity
    {
        return Err(snapshot_publication_error(
            "create generation artifact",
            "generation directory identity changed",
        ));
    }
    let path = generation_dir.join(name);
    let mut options = std::fs::OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| TryWriteError::IO(error, path.clone()))?;
    let mut digest = Sha256::new();
    for chunk in bytes.chunks(64 * 1024) {
        file.write_all(chunk)
            .map_err(|error| TryWriteError::IO(error, path.clone()))?;
        digest.update(chunk);
    }
    file.flush()
        .and_then(|()| file.sync_all())
        .map_err(|error| TryWriteError::IO(error, path.clone()))?;
    let written_sha256: [u8; 32] = digest.finalize().into();
    let Some(binding) = bind_snapshot_file_handle(&path, max_bytes)
        .map_err(|error| snapshot_publication_error("bind generation artifact", error))?
    else {
        return Err(snapshot_publication_error(
            "bind generation artifact",
            "new artifact disappeared",
        ));
    };
    let expected_sha256: [u8; 32] = Sha256::digest(bytes).into();
    if binding.len != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
        || binding.bytes_sha256 != Some(expected_sha256)
        || written_sha256 != expected_sha256
        || direct_snapshot_directory_identity(generation_dir)
            .map_err(|error| snapshot_publication_error("reverify generation directory", error))?
            != generation_dir_identity
    {
        return Err(snapshot_publication_error(
            "verify generation artifact",
            "streamed artifact digest, length, or directory identity changed",
        ));
    }
    Ok(binding)
}
#[cfg(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox"
))]
fn publish_generation_directory_noreplace(
    generations_dir: &Path,
    staging_dir: &Path,
    generation_name: &str,
) -> std::io::Result<()> {
    let parent = std::fs::File::open(generations_dir)?;
    let staging_name = staging_dir.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "snapshot staging directory has no basename",
        )
    })?;
    rustix::fs::renameat_with(
        &parent,
        staging_name,
        &parent,
        generation_name,
        rustix::fs::RenameFlags::NOREPLACE,
    )
    .map_err(std::io::Error::from)
}
#[cfg(windows)]
fn publish_generation_directory_noreplace(
    generations_dir: &Path,
    staging_dir: &Path,
    generation_name: &str,
) -> std::io::Result<()> {
    std::fs::rename(staging_dir, generations_dir.join(generation_name))
}
#[cfg(not(any(
    target_vendor = "apple",
    target_os = "linux",
    target_os = "android",
    target_os = "redox",
    windows
)))]
fn publish_generation_directory_noreplace(
    _generations_dir: &Path,
    _staging_dir: &Path,
    _generation_name: &str,
) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        "atomic no-replace directory publication is unsupported on this platform",
    ))
}
#[allow(clippy::too_many_arguments)]
fn bind_existing_snapshot_generation_for_write(
    generations_dir: &Path,
    generations_dir_identity: StableSnapshotFileIdentity,
    digest_hex: &str,
    payload: &[u8],
    digest: &[u8],
    signature: &[u8],
    fast_manifest: &[u8],
    merkle: &[u8],
    merkle_limit: u64,
    verification_key: &PublicKey,
) -> Result<PublishedSnapshotGeneration, TryWriteError> {
    let generation_dir = generations_dir.join(digest_hex);
    let generation_dir_identity = direct_snapshot_directory_identity(&generation_dir)
        .map_err(|error| snapshot_publication_error("bind published generation", error))?;
    if !snapshot_generation_has_exact_artifact_inventory(&generation_dir)
        .map_err(|error| snapshot_publication_error("inventory published generation", error))?
    {
        return Err(snapshot_publication_error(
            "bind published generation",
            "immutable generation does not contain exactly the five canonical artifacts",
        ));
    }
    let manifest_path = generation_dir.join(SNAPSHOT_FAST_MANIFEST_FILE_NAME);
    let decoded_manifest = decode_emergency_fast_manifest(fast_manifest, &manifest_path)
        .map_err(|error| snapshot_publication_error("validate Fast manifest", error))?;
    if decoded_manifest.payload_len != u64::try_from(payload.len()).unwrap_or(u64::MAX) {
        return Err(snapshot_publication_error(
            "validate Fast manifest",
            "manifest payload length differs from snapshot.data",
        ));
    }
    let payload_digest: [u8; 32] = hex::decode(digest_hex)
        .map_err(|error| snapshot_publication_error("decode generation digest", error))?
        .try_into()
        .map_err(|_| {
            snapshot_publication_error("decode generation digest", "wrong SHA-256 length")
        })?;
    let bundle_digest = snapshot_bundle_auth_digest(&payload_digest, fast_manifest);
    let expected = [
        (
            SNAPSHOT_FILE_NAME,
            payload,
            u64::try_from(payload.len()).unwrap_or(u64::MAX),
        ),
        (SNAPSHOT_DIGEST_FILE_NAME, digest, SNAPSHOT_DIGEST_MAX_BYTES),
        (
            SNAPSHOT_SIGNATURE_FILE_NAME,
            signature,
            SNAPSHOT_SIGNATURE_MAX_BYTES,
        ),
        (
            SNAPSHOT_FAST_MANIFEST_FILE_NAME,
            fast_manifest,
            SNAPSHOT_FAST_MANIFEST_MAX_BYTES,
        ),
        (SNAPSHOT_MERKLE_FILE_NAME, merkle, merkle_limit),
    ];
    let mut artifacts = Vec::with_capacity(expected.len());
    for (name, expected_bytes, max_bytes) in expected {
        let path = generation_dir.join(name);
        let binding = if name == SNAPSHOT_FILE_NAME {
            let Some(binding) = bind_snapshot_file_handle(&path, max_bytes)
                .map_err(|error| snapshot_publication_error("bind published payload", error))?
            else {
                return Err(snapshot_publication_error(
                    "bind published generation",
                    "immutable generation is missing snapshot.data",
                ));
            };
            let expected_sha256: [u8; 32] = Sha256::digest(expected_bytes).into();
            if binding.len != u64::try_from(expected_bytes.len()).unwrap_or(u64::MAX)
                || binding.bytes_sha256 != Some(expected_sha256)
            {
                return Err(snapshot_publication_error(
                    "verify published generation",
                    "immutable generation has conflicting snapshot.data bytes",
                ));
            }
            binding
        } else {
            let Some((binding, bytes)) = bind_snapshot_file(&path, max_bytes)
                .map_err(|error| snapshot_publication_error("bind published artifact", error))?
            else {
                return Err(snapshot_publication_error(
                    "bind published generation",
                    format!("immutable generation is missing {name}"),
                ));
            };
            if name == SNAPSHOT_SIGNATURE_FILE_NAME {
                let signature_hex = std::str::from_utf8(&bytes).map_err(|_| {
                    snapshot_publication_error(
                        "verify published generation",
                        "signature is not UTF-8",
                    )
                })?;
                verify_signature_hex(signature_hex, &bundle_digest, verification_key).map_err(
                    |error| snapshot_publication_error("verify generation signature", error),
                )?;
            } else if bytes != expected_bytes {
                return Err(snapshot_publication_error(
                    "verify published generation",
                    format!("immutable generation has conflicting {name} bytes"),
                ));
            }
            binding
        };
        artifacts.push(binding);
    }
    let published = PublishedSnapshotGeneration {
        generations_dir: generations_dir.to_path_buf(),
        generations_dir_identity,
        generation_dir,
        generation_dir_identity,
        artifacts,
        name: digest_hex.to_owned(),
    };
    published.verify_unchanged()?;
    Ok(published)
}
fn publish_immutable_snapshot_generation(
    store_dir: &Path,
    store_dir_identity: StableSnapshotFileIdentity,
    digest_hex: &str,
    payload: &[u8],
    digest: &[u8],
    signature: &[u8],
    fast_manifest: &[u8],
    merkle: &[u8],
    merkle_limit: u64,
    verification_key: &PublicKey,
) -> Result<PublishedSnapshotGeneration, TryWriteError> {
    let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
    match std::fs::symlink_metadata(&generations_dir) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let mut builder = std::fs::DirBuilder::new();
            #[cfg(unix)]
            {
                use std::os::unix::fs::DirBuilderExt;
                builder.mode(0o700);
            }
            match builder.create(&generations_dir) {
                Ok(()) => {}
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(TryWriteError::IO(error, generations_dir.clone())),
            }
        }
        Err(error) => return Err(TryWriteError::IO(error, generations_dir.clone())),
    }
    if direct_snapshot_directory_identity(store_dir)
        .map_err(|error| snapshot_publication_error("verify snapshot root", error))?
        != store_dir_identity
    {
        return Err(snapshot_publication_error(
            "publish snapshot generation",
            "snapshot root identity changed",
        ));
    }
    let generations_dir_identity = direct_snapshot_directory_identity(&generations_dir)
        .map_err(|error| snapshot_publication_error("bind generations directory", error))?;
    let generation_dir = generations_dir.join(digest_hex);
    if std::fs::symlink_metadata(&generation_dir).is_ok() {
        return bind_existing_snapshot_generation_for_write(
            &generations_dir,
            generations_dir_identity,
            digest_hex,
            payload,
            digest,
            signature,
            fast_manifest,
            merkle,
            merkle_limit,
            verification_key,
        );
    }
    let staging = tempfile::Builder::new()
        .prefix(".snapshot-generation-")
        .tempdir_in(&generations_dir)
        .map_err(|error| TryWriteError::IO(error, generations_dir.clone()))?;
    let staging_dir = staging.path().to_path_buf();
    let generation_dir_identity = direct_snapshot_directory_identity(&staging_dir)
        .map_err(|error| snapshot_publication_error("bind staging generation", error))?;
    let payload_limit = u64::try_from(payload.len()).unwrap_or(u64::MAX);
    let artifacts = vec![
        create_generation_artifact(
            &staging_dir,
            generation_dir_identity,
            SNAPSHOT_FILE_NAME,
            payload,
            payload_limit,
        )?,
        create_generation_artifact(
            &staging_dir,
            generation_dir_identity,
            SNAPSHOT_DIGEST_FILE_NAME,
            digest,
            SNAPSHOT_DIGEST_MAX_BYTES,
        )?,
        create_generation_artifact(
            &staging_dir,
            generation_dir_identity,
            SNAPSHOT_SIGNATURE_FILE_NAME,
            signature,
            SNAPSHOT_SIGNATURE_MAX_BYTES,
        )?,
        create_generation_artifact(
            &staging_dir,
            generation_dir_identity,
            SNAPSHOT_FAST_MANIFEST_FILE_NAME,
            fast_manifest,
            SNAPSHOT_FAST_MANIFEST_MAX_BYTES,
        )?,
        create_generation_artifact(
            &staging_dir,
            generation_dir_identity,
            SNAPSHOT_MERKLE_FILE_NAME,
            merkle,
            merkle_limit,
        )?,
    ];
    sync_snapshot_directory(&staging_dir, generation_dir_identity)
        .map_err(|error| snapshot_publication_error("sync staging generation", error))?;
    match publish_generation_directory_noreplace(&generations_dir, &staging_dir, digest_hex) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return bind_existing_snapshot_generation_for_write(
                &generations_dir,
                generations_dir_identity,
                digest_hex,
                payload,
                digest,
                signature,
                fast_manifest,
                merkle,
                merkle_limit,
                verification_key,
            );
        }
        Err(error) => return Err(TryWriteError::IO(error, generation_dir)),
    }
    drop(staging);
    sync_snapshot_directory(&generations_dir, generations_dir_identity)
        .map_err(|error| snapshot_publication_error("sync generations directory", error))?;
    for (binding, name) in artifacts.iter().zip([
        SNAPSHOT_FILE_NAME,
        SNAPSHOT_DIGEST_FILE_NAME,
        SNAPSHOT_SIGNATURE_FILE_NAME,
        SNAPSHOT_FAST_MANIFEST_FILE_NAME,
        SNAPSHOT_MERKLE_FILE_NAME,
    ]) {
        verify_bound_snapshot_file_at(&generation_dir.join(name), binding)
            .map_err(|error| snapshot_publication_error("verify published generation", error))?;
    }
    bind_existing_snapshot_generation_for_write(
        &generations_dir,
        generations_dir_identity,
        digest_hex,
        payload,
        digest,
        signature,
        fast_manifest,
        merkle,
        merkle_limit,
        verification_key,
    )
}
fn publish_snapshot_current_pointer(
    store_dir: &Path,
    store_dir_identity: StableSnapshotFileIdentity,
    generation: &PublishedSnapshotGeneration,
    max_payload_bytes: NonZeroUsize,
    merkle_chunk_size: NonZeroUsize,
    verification_key: &PublicKey,
) -> Result<(), TryWriteError> {
    generation.verify_unchanged()?;
    if direct_snapshot_directory_identity(store_dir)
        .map_err(|error| snapshot_publication_error("verify snapshot root", error))?
        != store_dir_identity
    {
        return Err(snapshot_publication_error(
            "publish current pointer",
            "snapshot root identity changed",
        ));
    }
    let pointer_bytes = format!("{}\n", generation.name).into_bytes();
    let (pointer_temp, pointer_binding) = create_synced_snapshot_temp(
        store_dir,
        store_dir_identity,
        "current",
        &pointer_bytes,
        SNAPSHOT_CURRENT_MAX_BYTES,
    )?;
    let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
    let replaced = bind_snapshot_destination(&pointer_path, SNAPSHOT_CURRENT_MAX_BYTES)
        .map_err(|error| snapshot_publication_error("bind current pointer", error))?;
    let previous_generation = match &replaced {
        BoundSnapshotDestination::Absent => None,
        BoundSnapshotDestination::Present { bytes, .. } => {
            let previous = parse_snapshot_current_pointer(bytes, &pointer_path)
                .map_err(|error| snapshot_publication_error("parse current pointer", error))?;
            let previous_path = store_dir
                .join(SNAPSHOT_GENERATIONS_DIR_NAME)
                .join(&previous);
            if !snapshot_generation_is_canonical_for_gc(
                &previous_path,
                &previous,
                max_payload_bytes,
                merkle_chunk_size,
                verification_key,
            ) {
                return Err(snapshot_publication_error(
                    "validate current generation before replacement",
                    "current points to an invalid or incomplete immutable generation",
                ));
            }
            Some(previous)
        }
    };
    verify_bound_snapshot_destination(&pointer_path, &replaced)
        .map_err(|error| snapshot_publication_error("reverify current pointer", error))?;
    let gc_plan = plan_snapshot_generation_gc(
        generation,
        previous_generation.as_deref(),
        max_payload_bytes,
        merkle_chunk_size,
        verification_key,
    )?;
    generation.verify_unchanged()?;
    if let Err(error) = verify_bound_snapshot_destination(&pointer_path, &replaced) {
        let concurrent = bind_snapshot_file(&pointer_path, SNAPSHOT_CURRENT_MAX_BYTES).map_err(
            |read_error| {
                snapshot_publication_error("read concurrently replaced current pointer", read_error)
            },
        )?;
        if concurrent
            .as_ref()
            .is_some_and(|(_, bytes)| bytes == &pointer_bytes)
        {
            // The other publisher may have installed the same pointer but not
            // yet made its directory entry durable. Sync this exact root and
            // rebind both the immutable generation and pointer before
            // reporting idempotent success.
            sync_snapshot_directory(store_dir, store_dir_identity).map_err(|sync_error| {
                snapshot_publication_error("sync snapshot root", sync_error)
            })?;
            generation.verify_unchanged()?;
            let rebound = bind_snapshot_file(&pointer_path, SNAPSHOT_CURRENT_MAX_BYTES).map_err(
                |read_error| {
                    snapshot_publication_error("rebind concurrent current pointer", read_error)
                },
            )?;
            if rebound
                .as_ref()
                .is_some_and(|(_, bytes)| bytes == &pointer_bytes)
            {
                if let Err(error) = execute_snapshot_generation_gc(generation, &gc_plan) {
                    warn!(
                        %error,
                        current_generation = %generation.name,
                        "snapshot pointer is durable; deferred immutable-generation maintenance after an idempotent publication"
                    );
                }
                return Ok(());
            }
            return Err(snapshot_publication_error(
                "rebind concurrent current pointer",
                "pointer changed while making the idempotent publication durable",
            ));
        }
        return Err(snapshot_publication_error(
            "reverify current pointer",
            error,
        ));
    }
    let persisted = pointer_temp
        .persist(&pointer_path)
        .map_err(|error| TryWriteError::IO(error.error, pointer_path.clone()))?;
    persisted
        .sync_all()
        .map_err(|error| TryWriteError::IO(error, pointer_path.clone()))?;
    verify_bound_snapshot_file_at(&pointer_path, &pointer_binding)
        .map_err(|error| snapshot_publication_error("verify current pointer", error))?;
    sync_snapshot_directory(store_dir, store_dir_identity)
        .map_err(|error| snapshot_publication_error("sync snapshot root", error))?;
    generation.verify_unchanged()?;
    verify_bound_snapshot_file_at(&pointer_path, &pointer_binding)
        .map_err(|error| snapshot_publication_error("reverify current pointer", error))?;
    if let Err(error) = execute_snapshot_generation_gc(generation, &gc_plan) {
        warn!(
            %error,
            current_generation = %generation.name,
            "snapshot pointer is durable; deferred immutable-generation maintenance"
        );
    }
    if let Err(error) = generation.verify_unchanged() {
        error!(
            %error,
            current_generation = %generation.name,
            "snapshot pointer was durably published, but its immutable generation changed during post-publication maintenance"
        );
    }
    if let Err(error) = verify_bound_snapshot_file_at(&pointer_path, &pointer_binding) {
        error!(
            ?error,
            current_generation = %generation.name,
            "snapshot pointer was durably published, but changed during post-publication maintenance"
        );
    }
    Ok(())
}
/// Reconstruct generated bytes through the semantic restart boundary before publication.
///
/// # Errors
/// Returns the same typed semantic error that would make the restart reader reject the payload.
#[cfg(test)]
fn validate_generated_snapshot_for_restart(
    state: &State,
    snapshot_bytes: &[u8],
) -> Result<(), TryReadError> {
    validate_generated_snapshot_for_restart_with_policy(
        state,
        snapshot_bytes,
        SnapshotResourcePolicy::default(),
    )
}
#[cfg(test)]
fn validate_generated_snapshot_for_restart_with_policy(
    state: &State,
    snapshot_bytes: &[u8],
    resource_policy: SnapshotResourcePolicy,
) -> Result<(), TryReadError> {
    let summary = validate_snapshot_json_resources(snapshot_bytes, resource_policy)?;
    if !summary.has_space_directory_manifests {
        return Err(TryReadError::MissingSpaceDirectoryManifestSection {
            snapshot_height: summary.block_hash_count.unwrap_or_default(),
        });
    }
    let input = std::str::from_utf8(snapshot_bytes)
        .map_err(|_| TryReadError::Serialization(json::Error::InvalidUtf8))?;
    validate_snapshot_sccp_registry_raw(input)?;
    let seed = KuraSeed {
        kura: state.kura_handle(),
        query_handle: state.query_handle.clone(),
        #[cfg(feature = "telemetry")]
        telemetry: StateTelemetry::default(),
    };
    let mut restored = seed
        .into_state_from_json_str_without_durable_recovery(input)
        .map_err(TryReadError::Serialization)?;
    if restored.network_id_ref() != state.network_id_ref() {
        return Err(TryReadError::NetworkIdMismatch {
            expected: *state.network_id_ref(),
            actual: *restored.network_id_ref(),
        });
    }
    if restored.has_snapshot_v2_bootstrap_candidate() {
        restored
            .authenticate_snapshot_v2_bootstrap_candidate(
                SnapshotBootstrapLineageAuthority::normally_signed_carried_lineage(),
            )
            .map_err(TryReadError::InvalidSnapshotBootstrap)?;
    }
    restored
        .install_zk_for_isolated_prevalidation(state.zk_snapshot())
        .map_err(TryReadError::ZkConfigInstall)?;
    crate::state::validate_sccp_snapshot_revert_candidate(&restored)
        .map_err(TryReadError::InvalidSccpRevert)?;
    Ok(())
}
/// Serialize, validate, and durably publish one canonical state snapshot.
#[cfg(test)]
fn try_write_snapshot_with_limit(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
) -> Result<(), TryWriteError> {
    try_write_snapshot_with_limit_and_policy(
        state,
        store_dir,
        signing_key,
        merkle_chunk_size,
        max_payload_bytes,
        SnapshotResourcePolicy::default(),
    )
}
fn try_write_snapshot_with_limit_and_policy(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
    resource_policy: SnapshotResourcePolicy,
) -> Result<(), TryWriteError> {
    let _publication_guard = SNAPSHOT_PUBLICATION_LOCK.lock();
    // TODO: Add a `Write`-backed Norito JSON sink so production can emit this
    // canonical payload directly into the authenticated staging descriptor.
    let mut snapshot_json = String::new();
    serialize_state_snapshot(state, &mut snapshot_json);
    try_write_snapshot_payload_with_limit_locked(
        state,
        store_dir,
        signing_key,
        merkle_chunk_size,
        max_payload_bytes,
        resource_policy,
        snapshot_json.into_bytes(),
    )
}
#[cfg(test)]
fn try_write_snapshot_payload_with_limit(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
    snapshot_bytes: Vec<u8>,
) -> Result<(), TryWriteError> {
    // This test-only seam accepts caller-supplied bytes, unlike the production writer whose
    // payload is emitted directly from the typed State. Keep the full restart dry run here so
    // adversarial fixture bytes cannot exercise post-publication geometry compaction.
    validate_generated_snapshot_for_restart(state, &snapshot_bytes)
        .map_err(TryWriteError::RestartValidation)?;
    let _publication_guard = SNAPSHOT_PUBLICATION_LOCK.lock();
    try_write_snapshot_payload_with_limit_locked(
        state,
        store_dir,
        signing_key,
        merkle_chunk_size,
        max_payload_bytes,
        SnapshotResourcePolicy::default(),
        snapshot_bytes,
    )
}
fn try_write_snapshot_payload_with_limit_locked(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
    resource_policy: SnapshotResourcePolicy,
    snapshot_bytes: Vec<u8>,
) -> Result<(), TryWriteError> {
    ensure_state_is_backed_by_kura(state)?;
    if snapshot_bytes.len() > max_payload_bytes.get() {
        return Err(TryWriteError::PayloadTooLarge {
            actual: snapshot_bytes.len(),
            maximum: max_payload_bytes,
        });
    }
    let summary = validate_snapshot_json_resources(&snapshot_bytes, resource_policy)
        .map_err(TryWriteError::RestartValidation)?;
    if !summary.has_space_directory_manifests {
        return Err(TryWriteError::RestartValidation(
            TryReadError::MissingSpaceDirectoryManifestSection {
                snapshot_height: summary.block_hash_count.unwrap_or_default(),
            },
        ));
    }
    let geometry_checkpoint = geometry_checkpoint_from_snapshot(&snapshot_bytes)?;
    let state_height = u64::try_from(state.committed_height()).map_err(|_| {
        TryWriteError::PublicationIntegrity(
            "State height exceeds the u64 manifest domain".to_owned(),
        )
    })?;
    if geometry_checkpoint.chain_id != *state.chain_id_ref()
        || geometry_checkpoint.network_id != *state.network_id_ref()
        || geometry_checkpoint.height != state_height
        || geometry_checkpoint.block_hash != state.latest_block_hash_fast()
        || geometry_checkpoint.sccp_policy_hash != state.sccp_policy_hash_snapshot()
        || geometry_checkpoint.snapshot_v2_bootstrap.is_some()
            != state.authenticated_snapshot_v2_bootstrap().is_some()
    {
        return Err(TryWriteError::PublicationIntegrity(
            "serialized snapshot identity differs from the State being published".to_owned(),
        ));
    }
    ensure_snapshot_commit_evidence(state, &geometry_checkpoint)?;
    let mut store_builder = std::fs::DirBuilder::new();
    store_builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        store_builder.mode(0o700);
    }
    store_builder
        .create(store_dir.as_ref())
        .map_err(|err| TryWriteError::IO(err, store_dir.as_ref().to_path_buf()))?;
    let store_dir = store_dir.as_ref();
    let directory_identity = direct_snapshot_directory_identity(store_dir)
        .map_err(|error| snapshot_publication_error("bind snapshot directory", error))?;
    let digest_bytes: [u8; 32] = Sha256::digest(&snapshot_bytes).into();
    let digest_vec = digest_bytes.to_vec();
    let digest_hex = hex::encode(&digest_vec);
    let merkle = SnapshotMerkleMetadata::from_bytes(&snapshot_bytes, merkle_chunk_size);
    let digest_line = format!("{digest_hex}\n").into_bytes();
    let fast_manifest = EmergencyFastSnapshotManifestV1 {
        version: SNAPSHOT_FAST_MANIFEST_VERSION,
        payload_len: u64::try_from(snapshot_bytes.len()).unwrap_or(u64::MAX),
        chain_id: geometry_checkpoint.chain_id.clone(),
        network_id: geometry_checkpoint.network_id,
        committed_height: geometry_checkpoint.height,
        tip_hash: geometry_checkpoint.block_hash,
        sccp_policy_hash: geometry_checkpoint.sccp_policy_hash,
        has_snapshot_bootstrap_lineage: geometry_checkpoint.snapshot_v2_bootstrap.is_some(),
    };
    fast_manifest
        .validate()
        .map_err(TryWriteError::PublicationIntegrity)?;
    let fast_manifest_bytes = fast_manifest.encode();
    if u64::try_from(fast_manifest_bytes.len()).unwrap_or(u64::MAX)
        > SNAPSHOT_FAST_MANIFEST_MAX_BYTES
    {
        return Err(TryWriteError::PublicationIntegrity(
            "canonical emergency Fast manifest exceeds its fixed size bound".to_owned(),
        ));
    }
    let bundle_digest = snapshot_bundle_auth_digest(&digest_bytes, &fast_manifest_bytes);
    let signature = Signature::try_new(signing_key.private_key(), &bundle_digest)
        .map_err(TryWriteError::Signing)?;
    let signature_hex = hex::encode(signature.payload()).into_bytes();
    let merkle_bytes = json::to_json(&merkle)
        .map_err(TryWriteError::MerkleSerialization)?
        .into_bytes();
    let payload_len = u64::try_from(snapshot_bytes.len()).unwrap_or(u64::MAX);
    let merkle_limit = snapshot_merkle_max_bytes(payload_len, merkle_chunk_size);
    if u64::try_from(merkle_bytes.len()).unwrap_or(u64::MAX) > merkle_limit {
        return Err(TryWriteError::PublicationIntegrity(
            "canonical Merkle metadata exceeds its payload-derived bound".to_owned(),
        ));
    }
    let generation = publish_immutable_snapshot_generation(
        store_dir,
        directory_identity,
        &digest_hex,
        &snapshot_bytes,
        &digest_line,
        &signature_hex,
        &fast_manifest_bytes,
        &merkle_bytes,
        merkle_limit,
        signing_key.public_key(),
    )?;
    publish_snapshot_current_pointer(
        store_dir,
        directory_identity,
        &generation,
        max_payload_bytes,
        merkle_chunk_size,
        signing_key.public_key(),
    )?;
    match state
        .kura()
        .checkpoint_lane_geometry_after_durable_snapshot_with_lineage_root(
            &geometry_checkpoint.lane_config,
            &geometry_checkpoint.incarnations,
            &geometry_checkpoint.activation_heights,
            geometry_checkpoint.lineage_root,
            geometry_checkpoint.height,
            geometry_checkpoint.block_hash,
            geometry_checkpoint.state_hash,
            &geometry_checkpoint.smart_contract_state,
        ) {
        Ok(summary) if summary.compacted_transitions > 0 || summary.removed_archive_roots > 0 => {
            info!(
                compacted_transitions = summary.compacted_transitions,
                removed_archive_roots = summary.removed_archive_roots,
                reclaimed_bytes = summary.reclaimed_bytes,
                snapshot_height = geometry_checkpoint.height,
                "checkpointed snapshot-authoritative lane geometry and reclaimed obsolete archives"
            );
        }
        Ok(_) => {}
        Err(error) => warn!(
            %error,
            snapshot_height = geometry_checkpoint.height,
            "snapshot is durable, but lane geometry archive GC failed closed"
        ),
    }
    Ok(())
}
#[cfg(test)]
fn try_write_snapshot(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
) -> Result<(), TryWriteError> {
    try_write_snapshot_with_limit(
        state,
        store_dir,
        signing_key,
        merkle_chunk_size,
        iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES,
    )
}
struct DurableSnapshotGeometryCheckpoint {
    chain_id: ChainId,
    network_id: NetworkId,
    lane_config: iroha_config::parameters::actual::LaneConfig,
    incarnations: BTreeMap<LaneId, Hash>,
    activation_heights: BTreeMap<LaneId, u64>,
    lineage_root: Hash,
    height: u64,
    block_hash: Option<HashOf<BlockHeader>>,
    state_hash: Hash,
    snapshot_v2_bootstrap: Option<SnapshotV2BootstrapRecord>,
    sccp_policy_hash: [u8; 32],
    smart_contract_state: BTreeMap<StatePath, Vec<u8>>,
}
fn ensure_snapshot_commit_evidence(
    state: &State,
    checkpoint: &DurableSnapshotGeometryCheckpoint,
) -> Result<(), TryWriteError> {
    let kura = state.kura();
    if checkpoint.height == 0 {
        return Ok(());
    }
    let height = NonZeroUsize::new(usize::try_from(checkpoint.height).map_err(|_| {
        TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: "snapshot height exceeds the host index width".to_owned(),
        }
    })?)
    .expect("non-zero snapshot height");
    let block_hash = checkpoint
        .block_hash
        .ok_or_else(|| TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: "non-zero snapshot has no terminal block hash".to_owned(),
        })?;
    let durable_hash =
        kura.get_durable_block_hash(height)
            .ok_or_else(|| TryWriteError::CommitEvidence {
                height: checkpoint.height,
                reason: "terminal block is absent from durable Kura storage".to_owned(),
            })?;
    if durable_hash != block_hash {
        return Err(TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: format!(
                "snapshot terminal block {block_hash} differs from durable Kura block {durable_hash}"
            ),
        });
    }
    if kura.is_hash_only_block_height(height) {
        let bootstrap = checkpoint.snapshot_v2_bootstrap.as_ref().ok_or_else(|| {
            TryWriteError::CommitEvidence {
                height: checkpoint.height,
                reason: "hash-only snapshot has no authenticated Sumeragi-v2 bootstrap record"
                    .to_owned(),
            }
        })?;
        if state.authenticated_snapshot_v2_bootstrap() != Some(bootstrap) {
            return Err(TryWriteError::CommitEvidence {
                height: checkpoint.height,
                reason: "serialized bootstrap record is not the State-authenticated trust root"
                    .to_owned(),
            });
        }
        bootstrap
            .validate()
            .map_err(|error| TryWriteError::CommitEvidence {
                height: checkpoint.height,
                reason: format!("invalid snapshot bootstrap record: {error}"),
            })?;
        let anchor = bootstrap
            .context
            .snapshot_bootstrap
            .as_ref()
            .expect("validated snapshot bootstrap record must contain an anchor");
        if anchor.snapshot_height != checkpoint.height
            || anchor.snapshot_block_hash != block_hash
            || anchor.snapshot_state_hash != checkpoint.state_hash
            || bootstrap.context.network_id != state.network_id
        {
            return Err(TryWriteError::CommitEvidence {
                height: checkpoint.height,
                reason: "snapshot bootstrap anchor does not exactly bind the serialized chain, height, block, and WSV"
                    .to_owned(),
            });
        }
        return Ok(());
    }
    let wsv_checkpoint = kura
        .wsv_checkpoint(checkpoint.height)
        .map_err(|error| TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: format!("failed to verify WSV checkpoint: {error}"),
        })?
        .ok_or_else(|| TryWriteError::CommitEvidenceDeferred {
            height: checkpoint.height,
            reason: "full-body snapshot has no WSV checkpoint".to_owned(),
        })?;
    if wsv_checkpoint.state_hash() != checkpoint.state_hash {
        return Err(TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: format!(
                "snapshot state hash {:?} differs from WSV checkpoint {:?}",
                checkpoint.state_hash,
                wsv_checkpoint.state_hash()
            ),
        });
    }
    let manifest = kura
        .commit_manifest(checkpoint.height)
        .map_err(|error| TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: format!("failed to verify commit manifest: {error}"),
        })?
        .ok_or_else(|| TryWriteError::CommitEvidenceDeferred {
            height: checkpoint.height,
            reason: "full-body snapshot has no commit manifest".to_owned(),
        })?;
    let binding = kura
        .commit_manifest_binding_state(&manifest)
        .map_err(|error| TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: format!("failed to verify checkpoint-to-manifest binding: {error}"),
        })?;
    match binding {
        CommitManifestBindingState::Bound => {}
        CommitManifestBindingState::Unbound => {
            return Err(TryWriteError::CommitEvidenceDeferred {
                height: checkpoint.height,
                reason: "commit manifest publication is not checkpoint-bound yet".to_owned(),
            });
        }
        CommitManifestBindingState::Mismatched => {
            return Err(TryWriteError::CommitEvidence {
                height: checkpoint.height,
                reason: "commit manifest digest conflicts with its WSV checkpoint".to_owned(),
            });
        }
    }
    let (artifact, _receipt) = kura
        .v2_finality_artifact_with_receipt(checkpoint.height)
        .map_err(|error| TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: format!("failed to verify Sumeragi-v2 finality: {error}"),
        })?
        .ok_or_else(|| TryWriteError::CommitEvidenceDeferred {
            height: checkpoint.height,
            reason: "full-body snapshot has no verified Sumeragi-v2 finality artifact".to_owned(),
        })?;
    if artifact.height != checkpoint.height || artifact.block_hash != block_hash {
        return Err(TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: "verified finality artifact does not identify the snapshot terminal block"
                .to_owned(),
        });
    }
    if !manifest.binds_authenticated_v2_commit_authority(&artifact) {
        return Err(TryWriteError::CommitEvidence {
            height: checkpoint.height,
            reason: "commit manifest does not bind the verified v2 authority and execution roots"
                .to_owned(),
        });
    }
    Ok(())
}
fn geometry_checkpoint_from_snapshot(
    bytes: &[u8],
) -> Result<DurableSnapshotGeometryCheckpoint, TryWriteError> {
    let input = std::str::from_utf8(bytes)
        .map_err(|_| TryWriteError::Serialization(json::Error::InvalidUtf8))?;
    let runtime: SnapshotNexusRuntime =
        json::from_str(required_snapshot_object_field(input, "nexus_runtime")?)
            .map_err(TryWriteError::Serialization)?;
    if runtime.version != SnapshotNexusRuntime::VERSION {
        return Err(TryWriteError::Serialization(json::Error::Message(format!(
            "snapshot Nexus runtime version {} cannot prove lane geometry",
            runtime.version
        ))));
    }
    let block_hashes: Vec<HashOf<BlockHeader>> =
        json::from_str(required_snapshot_object_field(input, "block_hashes")?)
            .map_err(TryWriteError::Serialization)?;
    let height = u64::try_from(block_hashes.len()).map_err(|_| {
        TryWriteError::Serialization(json::Error::Message(
            "snapshot block height exceeds u64".to_owned(),
        ))
    })?;
    let network_id: NetworkId =
        json::from_str(required_snapshot_object_field(input, "network_id")?)
            .map_err(TryWriteError::Serialization)?;
    let chain_id: ChainId = json::from_str(required_snapshot_object_field(input, "chain_id")?)
        .map_err(TryWriteError::Serialization)?;
    let snapshot_v2_bootstrap = snapshot_object_field_raw(input, "sumeragi_v2_bootstrap")
        .map_err(TryWriteError::RestartValidation)?
        .map(json::from_str)
        .transpose()
        .map_err(TryWriteError::Serialization)?;
    let lane_count = NonZeroU32::new(runtime.lane_count).ok_or_else(|| {
        TryWriteError::Serialization(json::Error::Message(
            "snapshot Nexus lane count is zero".to_owned(),
        ))
    })?;
    let lane_catalog = LaneCatalog::new(lane_count, runtime.lanes).map_err(|error| {
        TryWriteError::Serialization(json::Error::Message(format!(
            "snapshot Nexus lane catalog is invalid: {error}"
        )))
    })?;
    let lane_config = iroha_config::parameters::actual::LaneConfig::from_catalog(&lane_catalog);
    let mut lineage = BTreeMap::new();
    let mut latest_hashes = BTreeSet::new();
    for entry in runtime.lane_incarnation_lineage {
        let lineage_entry = LaneIncarnationLineage {
            generation: entry.generation,
            incarnation: entry.incarnation,
            activation_height: entry.activation_height,
        };
        if lineage_entry
            .incarnation
            .as_ref()
            .iter()
            .all(|byte| *byte == 0)
            || lineage_entry.activation_height > height
            || !latest_hashes.insert(lineage_entry.incarnation)
            || lineage.insert(entry.lane_id, lineage_entry).is_some()
        {
            return Err(TryWriteError::Serialization(json::Error::Message(
                "snapshot Nexus runtime contains invalid lane incarnation lineage".to_owned(),
            )));
        }
    }
    let mut incarnations = BTreeMap::new();
    let mut activation_heights = BTreeMap::new();
    for lane in lane_catalog.lanes() {
        let entry = lineage.get(&lane.id).ok_or_else(|| {
            TryWriteError::Serialization(json::Error::Message(format!(
                "snapshot Nexus runtime is missing active lane {} lineage",
                lane.id
            )))
        })?;
        incarnations.insert(lane.id, entry.incarnation);
        activation_heights.insert(lane.id, entry.activation_height);
    }
    let world = required_snapshot_object_field(input, "world")?;
    let smart_contract_storage: Storage<StatePath, Vec<u8>> = json::from_str(
        required_snapshot_object_field(world, "smart_contract_state")?,
    )
    .map_err(TryWriteError::Serialization)?;
    let smart_contract_state = smart_contract_storage
        .view()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    let state_hash =
        canonical_snapshot_wsv_hash(bytes).map_err(TryWriteError::RestartValidation)?;
    let sccp_policy_hash =
        snapshot_sccp_policy_hash_raw(input).map_err(TryWriteError::RestartValidation)?;
    Ok(DurableSnapshotGeometryCheckpoint {
        chain_id,
        network_id,
        lane_config,
        incarnations,
        activation_heights,
        lineage_root: lane_incarnation_lineage_root(&network_id, &lineage),
        height,
        block_hash: block_hashes.last().copied(),
        state_hash,
        snapshot_v2_bootstrap,
        sccp_policy_hash,
        smart_contract_state,
    })
}
fn required_snapshot_object_field<'a>(
    object: &'a str,
    field: &str,
) -> Result<&'a str, TryWriteError> {
    snapshot_object_field_raw(object, field)
        .map_err(TryWriteError::RestartValidation)?
        .ok_or_else(|| TryWriteError::Serialization(json::Error::missing_field(field)))
}
fn ensure_state_is_backed_by_kura(state: &State) -> Result<(), TryWriteError> {
    let state_height = state.committed_height();
    let kura_height = state
        .exact_durable_block_count()
        .map_err(TryWriteError::ExactKuraBoundary)?;
    if state_height > kura_height {
        return Err(TryWriteError::StateAheadOfKura {
            state_height,
            kura_height,
        });
    }
    let Some(height) = NonZeroUsize::new(state_height) else {
        return Ok(());
    };
    let state_hash = state.latest_block_hash_fast();
    let kura_hash = state.durable_block_hash(height);
    if state_hash != kura_hash {
        return Err(TryWriteError::LatestBlockHashMismatch {
            height: state_height,
            state_hash,
            kura_hash,
        });
    }
    Ok(())
}
/// Canonical bytes for the committed ledger WSV surface used by replay parity tests.
#[cfg(any(test, feature = "iroha-core-tests"))]
pub(crate) fn canonical_state_snapshot_bytes(state: &State) -> Vec<u8> {
    json::to_json(&canonical_state_snapshot_value(state))
        .expect("state snapshot serialization must succeed")
        .into_bytes()
}
/// Canonical hash for the committed ledger WSV surface.
pub(crate) fn canonical_state_snapshot_hash(state: &State) -> iroha_crypto::Hash {
    let mut snapshot_json = String::new();
    serialize_state_snapshot(state, &mut snapshot_json);
    canonical_snapshot_wsv_hash(snapshot_json.as_bytes())
        .expect("typed State serialization must form a canonical WSV snapshot")
}
/// Canonical bytes of the exact WSV surface that `state_block.commit()` would publish.
///
/// The block remains an uncommitted MVCC overlay, so callers can reject a mismatched
/// durable checkpoint without mutating live state.
#[cfg(test)]
pub(crate) fn canonical_staged_state_snapshot_bytes(state_block: &StateBlock<'_>) -> Vec<u8> {
    let mut json = String::new();
    serialize_staged_state_snapshot(state_block, &mut json);
    let mut value: json::Value =
        json::from_str(&json).expect("staged state snapshot serialization must produce valid JSON");
    let mut event_buffer_json = String::new();
    state_block.json_serialize_committed_external_event_buffer(&mut event_buffer_json);
    let event_buffer = json::from_str(&event_buffer_json)
        .expect("committed event buffer serialization must produce valid JSON");
    value
        .get_mut("world")
        .and_then(json::Value::as_object_mut)
        .expect("staged state snapshot world must be an object")
        .insert("external_event_buf".to_owned(), event_buffer);
    normalize_mv_cell_fields_in_state_value(&mut value);
    normalize_set_like_parameter_fields_in_state_value(&mut value);
    redact_consensus_sidecars_from_state_value(&mut value);
    json::to_json(&value)
        .expect("staged state snapshot canonical serialization must succeed")
        .into_bytes()
}
/// Canonical hash of the exact WSV surface that `state_block.commit()` would publish.
///
/// The block remains an uncommitted MVCC overlay, so callers can reject a mismatched
/// durable checkpoint without mutating live state.
pub(crate) fn canonical_staged_state_snapshot_hash(
    state_block: &StateBlock<'_>,
) -> iroha_crypto::Hash {
    let mut snapshot_json = String::new();
    serialize_staged_state_snapshot(state_block, &mut snapshot_json);
    let mut committed_external_event_buf = String::new();
    state_block.json_serialize_committed_external_event_buffer(&mut committed_external_event_buf);
    canonical_snapshot_wsv_hash_with_overrides(
        snapshot_json.as_bytes(),
        CanonicalWsvOverrides {
            committed_external_event_buf: Some(&committed_external_event_buf),
        },
    )
    .expect("typed staged State serialization must form a canonical WSV snapshot")
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn canonical_state_snapshot_value(state: &State) -> json::Value {
    let mut json = String::new();
    serialize_state_snapshot(state, &mut json);
    let mut value: json::Value =
        json::from_str(&json).expect("state snapshot serialization must produce valid JSON");
    normalize_mv_cell_fields_in_state_value(&mut value);
    normalize_set_like_parameter_fields_in_state_value(&mut value);
    redact_consensus_sidecars_from_state_value(&mut value);
    value
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn normalize_mv_cell_fields_in_state_value(value: &mut json::Value) {
    let Some(state) = value.as_object_mut() else {
        return;
    };
    normalize_serialized_cell_field(state, "commit_topology");
    normalize_serialized_cell_field(state, "prev_commit_topology");
    let Some(world) = state.get_mut("world").and_then(json::Value::as_object_mut) else {
        return;
    };
    for key in [
        "parameters",
        "peers",
        "viral_reward_budget",
        "viral_campaign_budget",
        "executor",
        "executor_data_model",
        "merge_hint_roots",
        "merge_global_state_root",
        "governance_last_unlock_sweep_height",
        "external_event_buf",
    ] {
        normalize_serialized_cell_field(world, key);
    }
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn normalize_serialized_cell_field(map: &mut json::Map, key: &str) {
    let Some(value) = map.get_mut(key) else {
        return;
    };
    let replacement = value
        .as_object()
        .filter(|cell| cell.contains_key("revert"))
        .and_then(|cell| cell.get("blocks"))
        .cloned();
    if let Some(current_value) = replacement {
        *value = current_value;
    }
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn normalize_set_like_parameter_fields_in_state_value(value: &mut json::Value) {
    let Some(sumeragi) = value
        .get_mut("world")
        .and_then(|world| world.get_mut("parameters"))
        .and_then(|parameters| parameters.get_mut("sumeragi"))
        .and_then(json::Value::as_object_mut)
    else {
        return;
    };
    sort_dedup_json_array_field(sumeragi, "key_allowed_algorithms");
    sort_dedup_json_array_field(sumeragi, "key_allowed_hsm_providers");
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn sort_dedup_json_array_field(map: &mut json::Map, key: &str) {
    let Some(values) = map.get_mut(key).and_then(json::Value::as_array_mut) else {
        return;
    };
    values.sort_by_cached_key(canonical_json_sort_key);
    values.dedup();
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn canonical_json_sort_key(value: &json::Value) -> String {
    let mut out = String::new();
    json::JsonSerialize::json_serialize(value, &mut out);
    out
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn redact_consensus_sidecars_from_state_value(value: &mut json::Value) {
    let Some(state) = value.as_object_mut() else {
        return;
    };
    // The signed bootstrap envelope authenticates this WSV; it cannot be part of the WSV hash
    // that its own anchor commits to.
    state.remove("sumeragi_v2_bootstrap");
    // Commit topologies are consensus scheduling caches. Replay reconstructs
    // them from Kura blocks and their authenticated v2 finality artifacts
    // rather than transaction execution, so they must not perturb committed
    // ledger checkpoints.
    state.remove("commit_topology");
    state.remove("prev_commit_topology");
    let Some(world) = value.get_mut("world") else {
        return;
    };
    redact_consensus_sidecars_from_world_value(world);
}
#[cfg(any(test, feature = "iroha-core-tests"))]
fn redact_consensus_sidecars_from_world_value(world: &mut json::Value) {
    let Some(world) = world.as_object_mut() else {
        return;
    };
    // Consensus evidence is asynchronously enriched recovery data, not WSV data committed by
    // the block itself. Including it makes historical checkpoints depend on later peer input.
    world.remove("consensus_evidence");
    // VRF epoch snapshots are maintained by consensus message handling outside
    // block application. Kura replay verifies block-applied WSV data only.
    world.remove("vrf_epochs");
}
/// Canonical bytes for the committed WSV surface used by replay parity tests.
#[cfg(any(test, feature = "iroha-core-tests"))]
#[allow(dead_code)]
pub(crate) fn canonical_state_snapshot_bytes_for_tests(state: &State) -> Vec<u8> {
    canonical_state_snapshot_bytes(state)
}
/// Error variants for snapshot reading
#[derive(thiserror::Error, Debug, displaydoc::Display)]
#[ignore_extra_doc_attributes]
pub enum TryReadError {
    /// The snapshot was not found
    NotFound,
    /// Failed reading/writing {1:?} from disk
    IO(#[source] std::io::Error, PathBuf),
    /// Error (de)serializing state snapshot
    Serialization(#[source] norito::json::Error),
    /// Signed snapshot payload is not the single canonical first-release JSON encoding
    NonCanonicalSnapshotPayload,
    /// Snapshot exceeds a configured typed decode or transient resource boundary: {0}
    SnapshotResourceLimit(String),
    /// Snapshot artifact or directory binding changed at {0:?}
    SnapshotBindingChanged(PathBuf),
    /// Immutable snapshot generation at {path:?} is invalid: {reason}
    SnapshotGenerationInvalid {
        /// Invalid pointer, directory, or artifact path.
        path: PathBuf,
        /// Exact fail-closed integrity violation.
        reason: String,
    },
    /// Snapshot digest file missing at {0:?}
    ChecksumMissing(PathBuf),
    /// Snapshot digest mismatch (expected `{expected}`, got `{actual}`)
    ChecksumMismatch {
        /// Expected digest from the `.sha256` sidecar.
        expected: String,
        /// Actual digest computed from the snapshot payload.
        actual: String,
    },
    /// Snapshot signature file missing at {0:?}
    SignatureMissing(PathBuf),
    /// Snapshot signature malformed (`{0}`)
    SignatureMalformed(String),
    /// Snapshot signature invalid (`{0}`)
    SignatureInvalid(String),
    /// Snapshot Merkle metadata missing at {0:?}
    MerkleMissing(PathBuf),
    /// Snapshot Merkle metadata parse error
    MerkleMetadata(#[source] norito::json::Error),
    /// Snapshot Merkle metadata malformed (`{0}`)
    MerkleMetadataMalformed(String),
    /// Snapshot Merkle root mismatch (expected `{expected}`, got `{actual}`)
    MerkleMismatch {
        /// Root recorded in metadata.
        expected: String,
        /// Root derived from the snapshot payload.
        actual: String,
    },
    /// Snapshot Merkle chunk size mismatch (expected `{expected}`, got `{actual}`)
    MerkleChunkSizeMismatch {
        /// Chunk size requested by the caller.
        expected: NonZeroUsize,
        /// Chunk size recorded in metadata.
        actual: NonZeroUsize,
    },
    /// Snapshot length mismatch (expected `{expected}` bytes, got `{actual}` bytes)
    MerkleLengthMismatch {
        /// Length recorded in metadata.
        expected: u64,
        /// Length derived from the snapshot payload.
        actual: u64,
    },
    /// Snapshot Merkle proof invalid for chunk `{chunk}` (`{reason}`)
    MerkleProofInvalid {
        /// Index of the chunk that failed verification.
        chunk: usize,
        /// Reason the Merkle verification failed.
        reason: String,
    },
    /// Snapshot exact network id mismatch (expected `{expected}`, got `{actual}`)
    NetworkIdMismatch {
        /// Expected genesis-derived network id from configuration.
        expected: NetworkId,
        /// Exact network id recorded in the snapshot payload.
        actual: NetworkId,
    },
    /// Snapshot contains an invalid governed SCCP registry (`{0}`)
    InvalidSccpRegistry(String),
    /// Snapshot contains invalid SCCP state in its one-block MV revert candidate (`{0}`)
    InvalidSccpRevert(String),
    /// Snapshot bootstrap authorization or typed trust root is invalid (`{0}`)
    InvalidSnapshotBootstrap(String),
    /// Snapshot WSV checkpoint mismatch at height `{height}` (expected `{expected:?}`, got `{actual:?}`)
    WsvCheckpointMismatch {
        /// Committed snapshot height whose checkpoint was validated.
        height: usize,
        /// Canonical WSV hash retained by Kura.
        expected: Hash,
        /// Canonical WSV hash reconstructed from the signed snapshot.
        actual: Hash,
    },
    /// Snapshot state is incompatible with runtime ZK configuration: {0}
    ZkConfigInstall(#[source] ZkConfigInstallError),
    /// Snapshot is in a non-consistent state. Snapshot has greater height (`{snapshot_height}`) than kura block store (`{kura_height}`)
    MismatchedHeight {
        /// The amount of block hashes stored by snapshot
        snapshot_height: usize,
        /// The amount of blocks stored by [`Kura`]
        kura_height: usize,
    },
    /// Snapshot is in a non-consistent state. Hash of the block at height `{height}` is different between snapshot (`{snapshot_block_hash}`) and kura (`{kura_block_hash}`)
    MismatchedHash {
        /// Height at which block hashes differs between snapshot and [`Kura`]
        height: usize,
        /// Hash of the block stored in snapshot
        snapshot_block_hash: HashOf<BlockHeader>,
        /// Hash of the block stored in kura
        kura_block_hash: HashOf<BlockHeader>,
    },
    /// Snapshot is in a non-consistent state. Kura is missing block {height}.
    MissingBlock {
        /// Height of the missing block in [`Kura`].
        height: usize,
    },
    /// Snapshot at height `{snapshot_height}` is missing the durable Space Directory manifest section
    MissingSpaceDirectoryManifestSection {
        /// Height recorded by the malformed snapshot.
        snapshot_height: usize,
    },
    /// Failed to reconcile snapshot block hashes with Kura
    Kura(#[source] KuraError),
}
fn merkle_err_to_try_read(err: SnapshotMerkleError, _path: PathBuf) -> TryReadError {
    match err {
        #[cfg(test)]
        SnapshotMerkleError::Missing => TryReadError::MerkleMissing(_path),
        #[cfg(test)]
        SnapshotMerkleError::Io(io) => TryReadError::IO(io, _path),
        SnapshotMerkleError::Parse(err) => TryReadError::MerkleMetadata(err),
        SnapshotMerkleError::ChunkSizeMismatch { expected, actual } => {
            TryReadError::MerkleChunkSizeMismatch { expected, actual }
        }
        SnapshotMerkleError::ChunkSizeInvalid(size) => {
            TryReadError::MerkleMetadataMalformed(format!("invalid chunk size {size}"))
        }
        SnapshotMerkleError::RootMismatch { expected, actual } => {
            TryReadError::MerkleMismatch { expected, actual }
        }
        SnapshotMerkleError::LengthMismatch { expected, actual } => {
            TryReadError::MerkleLengthMismatch { expected, actual }
        }
        SnapshotMerkleError::LeafCountMismatch { expected, actual } => {
            TryReadError::MerkleMetadataMalformed(format!(
                "leaf count mismatch (expected {expected}, got {actual})"
            ))
        }
        SnapshotMerkleError::RootHexMalformed => {
            TryReadError::MerkleMetadataMalformed("invalid Merkle root hex".into())
        }
        SnapshotMerkleError::LeafHexMalformed => {
            TryReadError::MerkleMetadataMalformed("invalid Merkle leaf hex".into())
        }
        SnapshotMerkleError::ProofUnavailable { chunk_index } => TryReadError::MerkleProofInvalid {
            chunk: chunk_index,
            reason: "proof unavailable".into(),
        },
        SnapshotMerkleError::ProofInvalid {
            chunk_index,
            reason,
        } => TryReadError::MerkleProofInvalid {
            chunk: chunk_index,
            reason,
        },
    }
}
/// Error variants for snapshot writing
#[derive(thiserror::Error, Debug, displaydoc::Display)]
enum TryWriteError {
    /// Failed reading/writing {1:?} from disk
    IO(#[source] std::io::Error, PathBuf),
    /// Error (de)serializing World State View snapshot
    Serialization(norito::json::Error),
    /// Generated snapshot is not admissible through the restart reader: {0}
    RestartValidation(#[source] TryReadError),
    /// Error (de)serializing snapshot Merkle metadata
    MerkleSerialization(norito::json::Error),
    /// Error signing snapshot digest
    Signing(#[source] iroha_crypto::Error),
    /// Snapshot publication failed its stable-file or stable-directory integrity check: {0}
    PublicationIntegrity(String),
    /// Canonical snapshot payload is `{actual}` bytes; configured maximum is `{maximum}`
    PayloadTooLarge {
        /// Canonical payload length.
        actual: usize,
        /// Configured reader/writer limit.
        maximum: NonZeroUsize,
    },
    /// Failed to read the exact durable Kura boundary
    ExactKuraBoundary(#[source] KuraError),
    /// Refusing to write snapshot at state height `{state_height}` because durable Kura height is `{kura_height}`
    StateAheadOfKura {
        /// Height recorded by state/block-hash journal.
        state_height: usize,
        /// Height durably indexed by Kura.
        kura_height: usize,
    },
    /// Refusing to write snapshot at height `{height}` because latest state hash `{state_hash:?}` does not match Kura hash `{kura_hash:?}`
    LatestBlockHashMismatch {
        /// Height being snapshotted.
        height: usize,
        /// Latest block hash recorded by state.
        state_hash: Option<HashOf<BlockHeader>>,
        /// Block hash recorded by Kura at the same height.
        kura_hash: Option<HashOf<BlockHeader>>,
    },
    /// Refusing to publish snapshot at height `{height}` because durable commit evidence is incomplete or inconsistent: {reason}
    CommitEvidence {
        /// Height encoded by the serialized snapshot itself.
        height: u64,
        /// Exact fail-closed evidence violation.
        reason: String,
    },
    /// Snapshot at height `{height}` is waiting for its in-flight durable commit tuple: {reason}
    CommitEvidenceDeferred {
        /// Height encoded by the serialized snapshot itself.
        height: u64,
        /// Missing publication step that a later snapshot interval must retry.
        reason: String,
    },
}
#[cfg(test)]
mod tests {
    include!("snapshot/support_policy_tests.rs");
    include!("snapshot/write_roundtrip_tests.rs");
    include!("snapshot/reconciliation_generation_tests.rs");
}
