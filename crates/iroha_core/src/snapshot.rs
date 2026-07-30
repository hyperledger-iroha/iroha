//! This module contains [`State`] snapshot actor service.
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    num::{NonZeroU32, NonZeroUsize},
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, Instant},
};

use hex;
use iroha_config::{
    parameters::{
        actual::{Snapshot as Config, SnapshotBootstrapPolicy},
        defaults,
    },
    snapshot::Mode,
};
use iroha_crypto::{
    Algorithm, CompactMerkleProof, Hash, HashOf, KeyPair, MerkleTree, PublicKey, Signature,
};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::AssetId,
    block::{BlockHeader, consensus_v2::SnapshotV2BootstrapRecord},
    name::Name,
    nexus::{LaneCatalog, LaneId},
};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_logger::prelude::*;
use mv::storage::{Storage, StorageReadOnly};
use norito::codec::Encode as NoritoEncode;
use norito::json::{self, JsonSerialize, JsonSerialize as JsonSerializeTrait};
use sha2::{Digest, Sha256};

#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    kura::{BlockCount, CommitManifestBindingState, Error as KuraError, Kura},
    query::store::LiveQueryStoreHandle,
    state::{
        LaneIncarnationLineage, SnapshotNexusRuntime, SnapshotNoritoBlob,
        SnapshotPublicLaneRewardClaim, SnapshotSpaceDirectoryManifestSet, State, StateBlock,
        WorldReadOnly, ZkConfigInstallError, deserialize::KuraSeed, lane_incarnation_lineage_root,
        public_lane_reward_record_matches_key, public_lane_stake_share_matches_key,
        public_lane_validator_record_matches_key,
    },
};

fn serialize_state_snapshot(
    state: &State,
    out: &mut String,
    include_space_directory_manifests: bool,
) {
    let view = state.view();
    let block_hashes: Vec<HashOf<BlockHeader>> = view.block_hashes.iter().copied().collect();
    let commit_topology = view.commit_topology.to_vec();
    let prev_commit_topology = view.prev_commit_topology.to_vec();
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
    let space_directory_manifests: Vec<_> = if include_space_directory_manifests {
        view.world
            .space_directory_manifests
            .iter()
            .map(|(uaid, value)| SnapshotSpaceDirectoryManifestSet {
                uaid: *uaid,
                encoded_hex: hex::encode(NoritoEncode::encode(value)),
            })
            .collect()
    } else {
        Vec::new()
    };

    out.push('{');
    json::write_json_string("chain_id", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&state.chain_id, out);
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

    if include_space_directory_manifests {
        out.push(',');
        json::write_json_string("space_directory_manifests", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&space_directory_manifests, out);
    }

    out.push(',');
    json::write_json_string("commit_topology", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&commit_topology, out);
    out.push(',');

    json::write_json_string("prev_commit_topology", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&prev_commit_topology, out);
    out.push('}');
}

fn serialize_staged_state_snapshot(state: &StateBlock<'_>, out: &mut String) {
    let world = state.world();
    let block_hashes: Vec<HashOf<BlockHeader>> = state.block_hashes().iter().copied().collect();
    let commit_topology = state.commit_topology.to_vec();
    let prev_commit_topology = state.prev_commit_topology.to_vec();
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
    json::JsonSerialize::json_serialize(&commit_topology, out);
    out.push(',');

    json::write_json_string("prev_commit_topology", out);
    out.push(':');
    json::JsonSerialize::json_serialize(&prev_commit_topology, out);
    out.push('}');
}

// Serialize State as a minimal snapshot wrapper using Norito JSON writer.
impl JsonSerializeTrait for State {
    fn json_serialize(&self, out: &mut String) {
        serialize_state_snapshot(self, out, true);
    }
}

/// Name of the [`State`] snapshot file.
const SNAPSHOT_FILE_NAME: &str = "snapshot.data";
/// Name of the digest accompanying the snapshot file.
const SNAPSHOT_DIGEST_FILE_NAME: &str = "snapshot.sha256";
/// Name of the signature accompanying the digest.
const SNAPSHOT_SIGNATURE_FILE_NAME: &str = "snapshot.sig";
/// Name of the Merkle metadata accompanying the snapshot file.
const SNAPSHOT_MERKLE_FILE_NAME: &str = "snapshot.merkle.json";
/// Directory containing immutable, digest-named complete generations.
const SNAPSHOT_GENERATIONS_DIR_NAME: &str = "generations";
/// Atomically replaced canonical pointer to one immutable generation.
const SNAPSHOT_CURRENT_FILE_NAME: &str = "current";
const SNAPSHOT_DIGEST_MAX_BYTES: u64 = 65;
const SNAPSHOT_CURRENT_MAX_BYTES: u64 = SNAPSHOT_DIGEST_MAX_BYTES;
const SNAPSHOT_SIGNATURE_MAX_BYTES: u64 = 16 * 1024;
const SNAPSHOT_MERKLE_FIXED_OVERHEAD_BYTES: u64 = 1024;
const SNAPSHOT_MERKLE_BYTES_PER_LEAF: u64 = 80;
const SNAPSHOT_GENERATION_GC_MAX_ENTRIES: usize = 4096;
static SNAPSHOT_PUBLICATION_LOCK: parking_lot::Mutex<()> = parking_lot::Mutex::new(());
#[cfg(test)]
std::thread_local! {
    static SNAPSHOT_GC_FAILURE_STAGE: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}
/// Default chunk size used to derive snapshot Merkle metadata.
const _DEFAULT_MERKLE_CHUNK_SIZE: NonZeroUsize = defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES;

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
        Ok(CompactMerkleProof::from_full(proof))
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
        if !proof.verify_sha256(&leaf, &root) {
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
            let result = tokio::task::block_in_place(move || {
                try_write_snapshot_with_limit(
                    &state,
                    store_dir,
                    &signing_key,
                    merkle_chunk_size,
                    max_payload_bytes,
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
fn stable_file_identity(metadata: &std::fs::Metadata) -> StableSnapshotFileIdentity {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(windows)]
fn stable_file_identity(metadata: &std::fs::Metadata) -> StableSnapshotFileIdentity {
    use std::os::windows::fs::MetadataExt;
    (metadata.volume_serial_number(), metadata.file_index())
}

#[cfg(not(any(unix, windows)))]
fn stable_file_identity(_metadata: &std::fs::Metadata) -> StableSnapshotFileIdentity {}

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

fn regular_file_has_single_link(metadata: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        metadata.nlink() == 1
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        metadata.number_of_links() == Some(1)
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        false
    }
}

fn read_bounded_stable_regular_file(
    path: &Path,
    max_bytes: u64,
) -> std::io::Result<Option<Vec<u8>>> {
    let path_before = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if path_before.file_type().is_symlink()
        || !path_before.is_file()
        || !regular_file_has_single_link(&path_before)
        || !stable_file_identity_available(stable_file_identity(&path_before))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact must be a direct single-link regular file",
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

    let mut file = std::fs::File::open(path)?;
    let opened_before = file.metadata()?;
    if !opened_before.is_file()
        || !regular_file_has_single_link(&opened_before)
        || !stable_file_identity_available(stable_file_identity(&opened_before))
        || stable_file_identity(&opened_before) != stable_file_identity(&path_before)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact identity changed while opening",
        ));
    }
    let capacity = usize::try_from(opened_before.len()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact length does not fit memory",
        )
    })?;
    let mut bytes = Vec::with_capacity(capacity);
    std::io::Read::by_ref(&mut file)
        .take(max_bytes.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if u64::try_from(bytes.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact exceeded its read bound",
        ));
    }
    let opened_after = file.metadata()?;
    let path_after = std::fs::symlink_metadata(path)?;
    if path_after.file_type().is_symlink()
        || !path_after.is_file()
        || !regular_file_has_single_link(&path_after)
        || !stable_file_identity_available(stable_file_identity(&path_after))
        || stable_file_identity(&opened_before) != stable_file_identity(&opened_after)
        || stable_file_identity(&opened_before) != stable_file_identity(&path_after)
        || opened_before.len() != opened_after.len()
        || opened_before.len() != u64::try_from(bytes.len()).unwrap_or(u64::MAX)
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "snapshot artifact changed while reading",
        ));
    }
    Ok(Some(bytes))
}

#[derive(Clone, Debug)]
struct BoundSnapshotFile {
    path: PathBuf,
    handle: Arc<std::fs::File>,
    identity: StableSnapshotFileIdentity,
    len: u64,
    bytes_hash: Hash,
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
    let Some(bytes) = read_bounded_stable_regular_file(path, max_bytes)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?
    else {
        return Ok(None);
    };
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let handle =
        std::fs::File::open(path).map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let opened = handle
        .metadata()
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    if !opened.is_file()
        || !regular_file_has_single_link(&opened)
        || stable_file_identity(&opened) != stable_file_identity(&metadata)
        || opened.len() != metadata.len()
    {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    Ok(Some((
        BoundSnapshotFile {
            path: path.to_path_buf(),
            handle: Arc::new(handle),
            identity: stable_file_identity(&metadata),
            len: metadata.len(),
            bytes_hash: Hash::new(&bytes),
            max_bytes,
        },
        bytes,
    )))
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
    let Some(bytes) = read_bounded_stable_regular_file(path, binding.max_bytes)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?
    else {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    };
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let opened = binding
        .handle
        .metadata()
        .map_err(|error| TryReadError::IO(error, binding.path.clone()))?;
    if !opened.is_file()
        || stable_file_identity(&opened) != binding.identity
        || opened.len() != binding.len
        || stable_file_identity(&metadata) != binding.identity
        || metadata.len() != binding.len
        || Hash::new(&bytes) != binding.bytes_hash
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
    let opened_before = file
        .metadata()
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    if !opened_before.is_dir() || stable_file_identity(&opened_before) != expected_identity {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    file.sync_all()
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    let opened_after = file
        .metadata()
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
    let metadata = std::fs::symlink_metadata(path)
        .map_err(|error| TryReadError::IO(error, path.to_path_buf()))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    let identity = stable_file_identity(&metadata);
    if !stable_file_identity_available(identity) {
        return Err(TryReadError::SnapshotBindingChanged(path.to_path_buf()));
    }
    Ok(identity)
}

struct SnapshotGenerationBytes {
    payload: Vec<u8>,
    digest: Vec<u8>,
    signature: Vec<u8>,
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

fn bind_current_snapshot_generation(
    store_dir: &Path,
    payload_limit: u64,
    merkle_chunk_size: NonZeroUsize,
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
            reason: "generation does not contain exactly the four canonical artifacts".to_owned(),
        });
    }
    let (payload, payload_bytes) =
        bind_required_snapshot_file(&generation_dir.join(SNAPSHOT_FILE_NAME), payload_limit)?;
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
    let payload_len = u64::try_from(payload_bytes.len()).unwrap_or(u64::MAX);
    let merkle_limit = snapshot_merkle_max_bytes(payload_len, merkle_chunk_size);
    let (merkle, merkle_bytes) = bind_required_snapshot_file(
        &generation_dir.join(SNAPSHOT_MERKLE_FILE_NAME),
        merkle_limit,
    )?;
    Ok(BoundSnapshotGeneration {
        store_dir: store_dir.to_path_buf(),
        store_dir_identity,
        generations_dir,
        generations_dir_identity,
        pointer,
        generation_dir,
        generation_dir_identity,
        artifacts: vec![payload, digest, signature, merkle],
        bytes: SnapshotGenerationBytes {
            payload: payload_bytes,
            digest: digest_bytes,
            signature: signature_bytes,
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

fn validate_snapshot_sccp_registry(value: &json::Value) -> Result<(), TryReadError> {
    let json::Value::Object(state) = value else {
        return Ok(());
    };
    let Some(json::Value::Object(world)) = state.get("world") else {
        return Ok(());
    };
    let Some(registry_value) = world.get("sccp_registry") else {
        return Ok(());
    };
    crate::state::validate_sccp_registry_cell_json(registry_value)
        .map_err(TryReadError::InvalidSccpRegistry)
}

fn reconcile_snapshot_hash_height_with_kura(
    snapshot_hashes: &[HashOf<BlockHeader>],
    block_count: usize,
    kura: &Kura,
    hard_fork_snapshot_bootstrap: bool,
    authenticated_payload: Option<&AuthenticatedSnapshotBootstrapPayload>,
) -> Result<(), TryReadError> {
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
            .reconcile_exact_audited_snapshot_bootstrap(Some(block_count), payload)
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
    state: &State,
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
    let actual = canonical_state_snapshot_hash(state);
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
    verification_key: &PublicKey,
    expected_chain_id: &ChainId,
    bootstrap_policy: &SnapshotBootstrapPolicy,
    initialize_state: &F,
    #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
) -> Result<SnapshotReadOutcome, TryReadError>
where
    F: Fn(&mut State) -> Result<(), TryReadError>,
{
    let bytes = generation.bytes.payload.as_slice();
    bootstrap_policy
        .validate()
        .map_err(TryReadError::InvalidSnapshotBootstrap)?;
    let digest_bytes = Sha256::digest(bytes);
    let digest_vec = digest_bytes.to_vec();
    let actual_digest = hex::encode(&digest_vec);
    let bytes_len = bytes.len();
    let payload_preview = snapshot_payload_preview(bytes);
    let expected_digest = format!("{actual_digest}\n");
    if generation.bytes.digest != expected_digest.as_bytes() {
        return Err(TryReadError::ChecksumMismatch {
            expected: String::from_utf8_lossy(&generation.bytes.digest).into_owned(),
            actual: actual_digest,
        });
    }
    let signature_hex = std::str::from_utf8(&generation.bytes.signature).map_err(|_| {
        TryReadError::SignatureMalformed("snapshot signature is not UTF-8".to_owned())
    })?;
    let payload_authority = match verify_signature_hex(signature_hex, &digest_vec, verification_key)
    {
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

    let value: json::Value = match json::from_slice(bytes) {
        Ok(value) => value,
        Err(err) => {
            iroha_logger::warn!(
                ?err,
                bytes_len,
                digest = %actual_digest,
                preview = %payload_preview,
                "snapshot JSON parse failed"
            );
            return Err(TryReadError::Serialization(err));
        }
    };
    // Snapshot signatures authenticate bytes, not semantic validity.  Reject
    // hostile SCCP material before constructing any live state so an invalid
    // verifying key can never be converted into an apparently empty registry.
    validate_snapshot_sccp_registry(&value)?;
    let has_space_directory_manifest_section =
        snapshot_has_space_directory_manifest_section(&value);
    if !has_space_directory_manifest_section
        && let Some(block_hashes) = value
            .as_object()
            .and_then(|state| state.get("block_hashes"))
            .cloned()
            .map(json::from_value::<Vec<HashOf<BlockHeader>>>)
            .transpose()
            .map_err(TryReadError::Serialization)?
        && !block_hashes.is_empty()
    {
        return Err(TryReadError::MissingSpaceDirectoryManifestSection {
            snapshot_height: block_hashes.len(),
        });
    }
    let seed = KuraSeed {
        kura: Arc::clone(kura),
        query_handle: live_query_store.clone(),
        #[cfg(feature = "telemetry")]
        telemetry,
    };
    let mut state = seed.into_state_from_json(value).map_err(|err| {
        iroha_logger::warn!(
            ?err,
            bytes_len,
            digest = %actual_digest,
            preview = %payload_preview,
            "snapshot state deserialization failed"
        );
        TryReadError::Serialization(err)
    })?;
    if &state.chain_id != expected_chain_id {
        return Err(TryReadError::ChainIdMismatch {
            expected: expected_chain_id.clone(),
            actual: state.chain_id.clone(),
        });
    }
    let snapshot_hashes = state.committed_block_hashes_snapshot();
    let snapshot_height = snapshot_hashes.len();
    let snapshot_height_u64 = u64::try_from(snapshot_height).map_err(|_| {
        TryReadError::InvalidSnapshotBootstrap(
            "snapshot height exceeds the canonical u64 height domain".to_owned(),
        )
    })?;
    let exact_policy_boundary = bootstrap_policy.authorizes(&actual_digest, snapshot_height_u64);
    let has_bootstrap_lineage = state.has_snapshot_v2_bootstrap_candidate();
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
    if snapshot_height > 0 && !has_space_directory_manifest_section {
        return Err(TryReadError::MissingSpaceDirectoryManifestSection { snapshot_height });
    }
    // Runtime configuration and the one-block SCCP rollback candidate are semantic checks on the
    // newly decoded, still-isolated state. Run them before the generic canonical-byte comparison
    // so hostile rollback histories retain their precise fail-closed classification. All checks
    // remain ahead of snapshot-driven Kura extension, pruning, or legacy recovery.
    initialize_state(&mut state)?;
    crate::state::validate_sccp_snapshot_revert_candidate(&state)
        .map_err(TryReadError::InvalidSccpRevert)?;
    let mut canonical_payload = String::new();
    serialize_state_snapshot(&state, &mut canonical_payload, true);
    if canonical_payload.as_bytes() != bytes {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    validate_snapshot_wsv_checkpoint(&state, &snapshot_hashes, kura)?;
    generation.verify_selection_unchanged()?;
    let hash_reconcile_started_at = Instant::now();
    reconcile_snapshot_hashes_with_kura(&snapshot_hashes, kura)?;
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
    expected_chain_id: &ChainId,
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
        verification_key,
        expected_chain_id,
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
    verification_key: &PublicKey,
    expected_chain_id: &ChainId,
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
        verification_key,
        expected_chain_id,
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
    verification_key: &PublicKey,
    expected_chain_id: &ChainId,
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
    let payload_limit = u64::try_from(max_payload_bytes.get()).unwrap_or(u64::MAX);
    let generation = bind_current_snapshot_generation(store_dir, payload_limit, merkle_chunk_size)?;
    let live_query_store = live_query_store_lazy();
    let outcome = try_read_snapshot_bundle(
        &generation,
        kura,
        &live_query_store,
        block_count,
        merkle_chunk_size,
        verification_key,
        expected_chain_id,
        bootstrap_policy,
        initialize_state,
        #[cfg(feature = "telemetry")]
        telemetry,
    )?;
    generation.verify_generation_unchanged()?;
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
                reason: "generation does not contain exactly the four canonical artifacts"
                    .to_owned(),
            });
        }
        let (payload, payload_bytes) = bind_required_snapshot_file(
            &path.join(SNAPSHOT_FILE_NAME),
            u64::try_from(max_payload_bytes.get()).unwrap_or(u64::MAX),
        )?;
        let (digest, digest_bytes) = bind_required_snapshot_file(
            &path.join(SNAPSHOT_DIGEST_FILE_NAME),
            SNAPSHOT_DIGEST_MAX_BYTES,
        )?;
        let (signature, signature_bytes) = bind_required_snapshot_file(
            &path.join(SNAPSHOT_SIGNATURE_FILE_NAME),
            SNAPSHOT_SIGNATURE_MAX_BYTES,
        )?;
        let merkle_limit = snapshot_merkle_max_bytes(
            u64::try_from(payload_bytes.len()).unwrap_or(u64::MAX),
            merkle_chunk_size,
        );
        let (merkle_file, merkle_bytes) =
            bind_required_snapshot_file(&path.join(SNAPSHOT_MERKLE_FILE_NAME), merkle_limit)?;

        let payload_digest = Sha256::digest(&payload_bytes).to_vec();
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
        verify_signature_hex(signature_hex, &payload_digest, verification_key)?;
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
        for artifact in [&payload, &digest, &signature, &merkle_file] {
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
        let metadata = std::fs::symlink_metadata(&artifact_path)
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
        let metadata = std::fs::symlink_metadata(&file.path)
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
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&path)
        .map_err(|error| TryWriteError::IO(error, path.clone()))?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|error| TryWriteError::IO(error, path.clone()))?;
    let Some((binding, readback)) = bind_snapshot_file(&path, max_bytes)
        .map_err(|error| snapshot_publication_error("bind generation artifact", error))?
    else {
        return Err(snapshot_publication_error(
            "bind generation artifact",
            "new artifact disappeared",
        ));
    };
    if readback != bytes
        || direct_snapshot_directory_identity(generation_dir)
            .map_err(|error| snapshot_publication_error("reverify generation directory", error))?
            != generation_dir_identity
    {
        return Err(snapshot_publication_error(
            "verify generation artifact",
            "artifact bytes or directory identity changed",
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
            "immutable generation does not contain exactly the four canonical artifacts",
        ));
    }
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
        (SNAPSHOT_MERKLE_FILE_NAME, merkle, merkle_limit),
    ];
    let mut artifacts = Vec::with_capacity(expected.len());
    for (name, expected_bytes, max_bytes) in expected {
        let path = generation_dir.join(name);
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
                snapshot_publication_error("verify published generation", "signature is not UTF-8")
            })?;
            verify_signature_hex(
                signature_hex,
                &hex::decode(digest_hex).map_err(|error| {
                    snapshot_publication_error("decode generation digest", error)
                })?,
                verification_key,
            )
            .map_err(|error| snapshot_publication_error("verify generation signature", error))?;
        } else if bytes != expected_bytes {
            return Err(snapshot_publication_error(
                "verify published generation",
                format!("immutable generation has conflicting {name} bytes"),
            ));
        }
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
    merkle: &[u8],
    merkle_limit: u64,
    verification_key: &PublicKey,
) -> Result<PublishedSnapshotGeneration, TryWriteError> {
    let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
    match std::fs::symlink_metadata(&generations_dir) {
        Ok(_) => {}
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::create_dir(&generations_dir) {
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

/// Serialize and write snapshot to file,
/// overwriting any previously stored data.
///
/// # Errors
/// - IO errors
/// - Serialization errors
fn try_write_snapshot_with_limit(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
) -> Result<(), TryWriteError> {
    ensure_state_is_backed_by_kura(state)?;
    let _publication_guard = SNAPSHOT_PUBLICATION_LOCK.lock();

    std::fs::create_dir_all(store_dir.as_ref())
        .map_err(|err| TryWriteError::IO(err, store_dir.as_ref().to_path_buf()))?;
    let store_dir = store_dir.as_ref();
    let directory_identity = direct_snapshot_directory_identity(store_dir)
        .map_err(|error| snapshot_publication_error("bind snapshot directory", error))?;
    let mut snapshot_json = String::new();
    serialize_state_snapshot(state, &mut snapshot_json, true);
    let snapshot_bytes = snapshot_json.into_bytes();
    if snapshot_bytes.len() > max_payload_bytes.get() {
        return Err(TryWriteError::PayloadTooLarge {
            actual: snapshot_bytes.len(),
            maximum: max_payload_bytes,
        });
    }
    let geometry_checkpoint = geometry_checkpoint_from_snapshot_bytes(&snapshot_bytes)?;
    ensure_snapshot_commit_evidence(state, &geometry_checkpoint)?;
    let digest_bytes = Sha256::digest(&snapshot_bytes);
    let digest_vec = digest_bytes.to_vec();
    let digest_hex = hex::encode(&digest_vec);
    let merkle = SnapshotMerkleMetadata::from_bytes(&snapshot_bytes, merkle_chunk_size);
    let digest_line = format!("{digest_hex}\n").into_bytes();
    let signature = Signature::try_new(signing_key.private_key(), &digest_vec)
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
    lane_config: iroha_config::parameters::actual::LaneConfig,
    incarnations: BTreeMap<LaneId, Hash>,
    activation_heights: BTreeMap<LaneId, u64>,
    lineage_root: Hash,
    height: u64,
    block_hash: Option<HashOf<BlockHeader>>,
    state_hash: Hash,
    snapshot_v2_bootstrap: Option<SnapshotV2BootstrapRecord>,
    smart_contract_state: BTreeMap<Name, Vec<u8>>,
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
            || bootstrap.context.chain_id != state.chain_id
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

fn geometry_checkpoint_from_snapshot_bytes(
    bytes: &[u8],
) -> Result<DurableSnapshotGeometryCheckpoint, TryWriteError> {
    let value: json::Value = json::from_slice(bytes).map_err(TryWriteError::Serialization)?;
    let root = value.as_object().ok_or_else(|| {
        TryWriteError::Serialization(json::Error::Message(
            "snapshot root is not a JSON object".to_owned(),
        ))
    })?;
    let runtime_value = root
        .get("nexus_runtime")
        .cloned()
        .ok_or_else(|| TryWriteError::Serialization(json::Error::missing_field("nexus_runtime")))?;
    let runtime: SnapshotNexusRuntime =
        json::from_value(runtime_value).map_err(TryWriteError::Serialization)?;
    if runtime.version != SnapshotNexusRuntime::VERSION {
        return Err(TryWriteError::Serialization(json::Error::Message(format!(
            "snapshot Nexus runtime version {} cannot prove lane geometry",
            runtime.version
        ))));
    }
    let block_hashes_value = root
        .get("block_hashes")
        .cloned()
        .ok_or_else(|| TryWriteError::Serialization(json::Error::missing_field("block_hashes")))?;
    let block_hashes: Vec<HashOf<BlockHeader>> =
        json::from_value(block_hashes_value).map_err(TryWriteError::Serialization)?;
    let height = u64::try_from(block_hashes.len()).map_err(|_| {
        TryWriteError::Serialization(json::Error::Message(
            "snapshot block height exceeds u64".to_owned(),
        ))
    })?;
    let chain_id_value = root
        .get("chain_id")
        .cloned()
        .ok_or_else(|| TryWriteError::Serialization(json::Error::missing_field("chain_id")))?;
    let chain_id: ChainId =
        json::from_value(chain_id_value).map_err(TryWriteError::Serialization)?;
    let snapshot_v2_bootstrap = root
        .get("sumeragi_v2_bootstrap")
        .cloned()
        .map(json::from_value)
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
    let lineage_root = lane_incarnation_lineage_root(&chain_id, &lineage);

    let smart_contract_state_value = root
        .get("world")
        .and_then(json::Value::as_object)
        .and_then(|world| world.get("smart_contract_state"))
        .cloned()
        .ok_or_else(|| {
            TryWriteError::Serialization(json::Error::missing_field("world.smart_contract_state"))
        })?;
    let smart_contract_storage: Storage<Name, Vec<u8>> =
        json::from_value(smart_contract_state_value).map_err(TryWriteError::Serialization)?;
    let smart_contract_state = smart_contract_storage
        .view()
        .iter()
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();

    let mut canonical_value = value;
    normalize_mv_cell_fields_in_state_value(&mut canonical_value);
    normalize_set_like_parameter_fields_in_state_value(&mut canonical_value);
    redact_consensus_sidecars_from_state_value(&mut canonical_value);
    let canonical_json = json::to_json(&canonical_value).map_err(TryWriteError::Serialization)?;
    Ok(DurableSnapshotGeometryCheckpoint {
        lane_config,
        incarnations,
        activation_heights,
        lineage_root,
        height,
        block_hash: block_hashes.last().copied(),
        state_hash: Hash::new(canonical_json.as_bytes()),
        snapshot_v2_bootstrap,
        smart_contract_state,
    })
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
pub(crate) fn canonical_state_snapshot_bytes(state: &State) -> Vec<u8> {
    canonical_state_snapshot_bytes_with_options(state, true)
}

fn canonical_state_snapshot_bytes_with_options(
    state: &State,
    include_space_directory_manifests: bool,
) -> Vec<u8> {
    json::to_json(&canonical_state_snapshot_value_with_options(
        state,
        include_space_directory_manifests,
    ))
    .expect("state snapshot serialization must succeed")
    .into_bytes()
}

/// Canonical hash for the committed ledger WSV surface.
pub(crate) fn canonical_state_snapshot_hash(state: &State) -> iroha_crypto::Hash {
    iroha_crypto::Hash::new(canonical_state_snapshot_bytes(state))
}

/// Canonical bytes of the exact WSV surface that `state_block.commit()` would publish.
///
/// The block remains an uncommitted MVCC overlay, so callers can reject a mismatched
/// durable checkpoint without mutating live state.
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
    iroha_crypto::Hash::new(canonical_staged_state_snapshot_bytes(state_block))
}

#[cfg(test)]
fn canonical_state_snapshot_value(state: &State) -> json::Value {
    canonical_state_snapshot_value_with_options(state, true)
}

fn canonical_state_snapshot_value_with_options(
    state: &State,
    include_space_directory_manifests: bool,
) -> json::Value {
    let mut json = String::new();
    serialize_state_snapshot(state, &mut json, include_space_directory_manifests);
    let mut value: json::Value =
        json::from_str(&json).expect("state snapshot serialization must produce valid JSON");
    normalize_mv_cell_fields_in_state_value(&mut value);
    normalize_set_like_parameter_fields_in_state_value(&mut value);
    redact_consensus_sidecars_from_state_value(&mut value);
    value
}

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

fn sort_dedup_json_array_field(map: &mut json::Map, key: &str) {
    let Some(values) = map.get_mut(key).and_then(json::Value::as_array_mut) else {
        return;
    };

    values.sort_by_cached_key(canonical_json_sort_key);
    values.dedup();
}

fn canonical_json_sort_key(value: &json::Value) -> String {
    let mut out = String::new();
    json::JsonSerialize::json_serialize(value, &mut out);
    out
}

fn redact_consensus_sidecars_from_state_value(value: &mut json::Value) {
    let Some(state) = value.as_object_mut() else {
        return;
    };
    // The signed bootstrap envelope authenticates this WSV; it cannot be part of the WSV hash
    // that its own anchor commits to.
    state.remove("sumeragi_v2_bootstrap");
    // Commit topologies are consensus scheduling caches. Replay reconstructs
    // them from Kura blocks and commit-roster journals rather than transaction
    // execution, so they must not perturb committed ledger checkpoints.
    state.remove("commit_topology");
    state.remove("prev_commit_topology");

    let Some(world) = value.get_mut("world") else {
        return;
    };
    redact_consensus_sidecars_from_world_value(world);
}

fn redact_consensus_sidecars_from_world_value(world: &mut json::Value) {
    let Some(world) = world.as_object_mut() else {
        return;
    };
    // These stores are asynchronously enriched recovery evidence, not WSV
    // data committed by the block itself. Including them makes historical
    // checkpoints depend on which peer supplied later, richer certificates.
    world.remove("commit_qcs");
    world.remove("consensus_evidence");
    // VRF epoch snapshots are maintained by consensus message handling outside
    // block application. Kura replay verifies block-applied WSV data only.
    world.remove("vrf_epochs");
}

/// Canonical hash for the legacy checkpoint surface used before Space Directory manifests
/// were included in durable snapshots.
#[cfg(test)]
pub(crate) fn legacy_state_snapshot_hash_without_space_directory_manifests(
    state: &State,
) -> iroha_crypto::Hash {
    iroha_crypto::Hash::new(canonical_state_snapshot_bytes_with_options(state, false))
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
    /// Snapshot chain id mismatch (expected `{expected}`, got `{actual}`)
    ChainIdMismatch {
        /// Expected chain id from configuration.
        expected: ChainId,
        /// Chain id recorded in the snapshot payload.
        actual: ChainId,
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
        /// Height recorded by the legacy snapshot.
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
    use std::{
        borrow::Cow,
        fs::File,
        num::{NonZeroU64, NonZeroUsize},
        path::Path,
        sync::{Arc, Barrier},
    };

    use iroha_config::{
        base::WithOrigin,
        kura::{FsyncMode, InitMode},
        parameters::{
            actual::{Kura as KuraConfig, LaneConfig},
            defaults::kura::{
                BLOCK_SYNC_ROSTER_RETENTION, EVICTION_REQUIRED_REPLICAS, FSYNC_INTERVAL,
                MAX_DISK_USAGE_BYTES, MERGE_LEDGER_CACHE_CAPACITY, ROSTER_SIDECAR_RETENTION,
            },
        },
    };
    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature, bls_normal_pop_prove};
    use iroha_data_model::{
        ChainId, Level, Registrable,
        account::{
            AccountAlias, AccountAliasDomain, AccountDetails, AccountId, AccountRekeyRecord,
            AccountValue,
        },
        asset::{AssetDefinition, AssetDefinitionAlias, AssetDefinitionId},
        block::{BlockHeader, SignedBlock},
        consensus::{ConsensusKeyStatus, Qc, QcAggregate, VALIDATOR_SET_HASH_VERSION_V1},
        domain::DomainId,
        isi::{Log, space_directory::PublishSpaceDirectoryManifest},
        metadata::Metadata,
        nexus::{AssetPermissionManifest, DataSpaceId, ManifestVersion, UniversalAccountId},
        peer::PeerId,
        smart_contract::{CHAIN_DISCRIMINANT_MAINNET, ContractAddress, ContractAlias},
        transaction::TransactionBuilder,
    };
    use nonzero_ext::nonzero;
    use tempfile::tempdir;
    use tokio::test;

    use super::*;
    use crate::{
        block::BlockBuilder,
        query::store::LiveQueryStore,
        state::{
            AssetDefinitionAliasBindingRecord, ContractAliasBindingRecord, derive_validator_key_id,
        },
        sumeragi::consensus::{
            PERMISSIONED_TAG, Phase, Vote, default_chain_order_hash, vote_preimage,
        },
        tx::AcceptedTransaction,
    };

    const TEST_CHUNK_SIZE: NonZeroUsize = nonzero!(1024_usize);
    const TEST_CHAIN_ID: &str = "test-chain";
    const SMALL_ORDER_ED25519_R: [u8; 32] = [
        1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        0, 0,
    ];
    const NONCANONICAL_ED25519_R: [u8; 32] = [
        0xee, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
        0xff, 0x7f,
    ];

    fn checked_seeded_keypair(seed: u8, algorithm: Algorithm) -> KeyPair {
        KeyPair::try_from_seed(vec![seed; 32], algorithm)
            .expect("test snapshot seeded keypair should be valid")
    }

    fn checked_random_snapshot_keypair() -> KeyPair {
        KeyPair::try_random().expect("snapshot fixture key generation should succeed")
    }

    fn checked_random_snapshot_bls_keypair() -> KeyPair {
        KeyPair::try_random_with_algorithm(Algorithm::BlsNormal)
            .expect("snapshot BLS fixture key generation should succeed")
    }

    fn current_generation_name(store_dir: &Path) -> String {
        let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
        let pointer = std::fs::read(&pointer_path).expect("read canonical snapshot pointer");
        parse_snapshot_current_pointer(&pointer, &pointer_path)
            .expect("canonical snapshot pointer must name one generation")
    }

    fn current_generation_dir(store_dir: &Path) -> PathBuf {
        store_dir
            .join(SNAPSHOT_GENERATIONS_DIR_NAME)
            .join(current_generation_name(store_dir))
    }

    fn current_generation_artifact(store_dir: &Path, name: &str) -> PathBuf {
        current_generation_dir(store_dir).join(name)
    }

    fn assert_canonical_snapshot_generation(store_dir: &Path) {
        let mut root_entries = std::fs::read_dir(store_dir)
            .expect("read snapshot root")
            .map(|entry| {
                entry
                    .expect("read snapshot root entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        root_entries.sort();
        assert_eq!(
            root_entries,
            vec![
                SNAPSHOT_CURRENT_FILE_NAME.to_owned(),
                SNAPSHOT_GENERATIONS_DIR_NAME.to_owned(),
            ],
            "first-release snapshots expose only the atomic pointer and immutable generations"
        );

        let generation_dir = current_generation_dir(store_dir);
        let generation_name = current_generation_name(store_dir);
        let payload = std::fs::read(generation_dir.join(SNAPSHOT_FILE_NAME))
            .expect("read selected snapshot payload");
        assert_eq!(generation_name, hex::encode(Sha256::digest(&payload)));
        assert_eq!(
            std::fs::read(generation_dir.join(SNAPSHOT_DIGEST_FILE_NAME))
                .expect("read selected snapshot digest"),
            format!("{generation_name}\n").as_bytes()
        );
        let mut artifact_names = std::fs::read_dir(&generation_dir)
            .expect("read selected generation")
            .map(|entry| {
                entry
                    .expect("read generation entry")
                    .file_name()
                    .to_string_lossy()
                    .into_owned()
            })
            .collect::<Vec<_>>();
        artifact_names.sort();
        let mut expected = vec![
            SNAPSHOT_FILE_NAME.to_owned(),
            SNAPSHOT_DIGEST_FILE_NAME.to_owned(),
            SNAPSHOT_SIGNATURE_FILE_NAME.to_owned(),
            SNAPSHOT_MERKLE_FILE_NAME.to_owned(),
        ];
        expected.sort();
        assert_eq!(artifact_names, expected);
    }

    fn signed_complete_wire_finality_for_snapshot_blocks(
        chain_id: &ChainId,
        blocks: &[Arc<SignedBlock>],
    ) -> Vec<iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact> {
        use iroha_data_model::block::consensus_v2::{
            BlockSubject, ConsensusMode, ConsensusRound, DataAvailabilityLayout, DualQuorum,
            ExecutionCommitment, GlobalPhase, HeightContext, PROTOCOL_VERSION, PayloadEncoding,
            QuorumCertificate, ValidatorPower, finality::V2FinalityArtifact,
        };

        let mut keypairs = (0_u8..4)
            .map(|index| {
                checked_seeded_keypair(0xB0_u8.saturating_add(index), Algorithm::BlsNormal)
            })
            .collect::<Vec<_>>();
        keypairs.sort_by(|left, right| {
            PeerId::new(left.public_key().clone()).cmp(&PeerId::new(right.public_key().clone()))
        });
        let roster = keypairs
            .iter()
            .map(|keypair| ValidatorPower {
                validator: PeerId::new(keypair.public_key().clone()),
                power: 1,
            })
            .collect::<Vec<_>>();
        let validator_set_pops = keypairs
            .iter()
            .map(|keypair| {
                bls_normal_pop_prove(keypair.private_key())
                    .expect("derive snapshot-eviction validator PoP")
            })
            .collect::<Vec<_>>();
        let execution_commitment_template = ExecutionCommitment::new(
            Hash::new(b"snapshot eviction parent state"),
            Hash::new(b"snapshot eviction post state"),
            Hash::new(b"snapshot eviction ordinary writes"),
            None,
            0,
            Hash::new(b"snapshot eviction executed block wire placeholder"),
        )
        .expect("snapshot-eviction execution commitment");
        let mut parent: Option<V2FinalityArtifact> = None;
        let mut artifacts = Vec::with_capacity(blocks.len());
        for block in blocks {
            let height = block.header().height().get();
            let context = HeightContext {
                chain_id: chain_id.clone(),
                protocol_version: PROTOCOL_VERSION,
                height,
                epoch: 0,
                epoch_end_height: 100,
                next_epoch_snapshot: None,
                mode: ConsensusMode::Permissioned,
                parent_commit_qc: parent.as_ref().map(|artifact| artifact.commit_qc.clone()),
                snapshot_bootstrap: None,
                quorum: DualQuorum::from_roster(&roster).expect("snapshot-eviction fixture quorum"),
                roster: roster.clone(),
                nexus_amx_context_hash: Hash::new(b"snapshot eviction nexus context"),
                da_layout: DataAvailabilityLayout {
                    encoding: PayloadEncoding::Plain,
                    chunk_size_bytes: 1024,
                    data_shards: 0,
                    parity_shards: 0,
                    max_payload_size_bytes: 4 * 1024 * 1024,
                    max_chunk_count: 4096,
                },
                leader_seed: [0x42; 32],
            };
            let subject = BlockSubject {
                parent_block_hash: block.header().prev_block_hash(),
                block_hash: block.hash(),
                payload_hash: block
                    .canonical_proposal_wire_hash()
                    .expect("canonical snapshot proposal wire"),
            };
            let mut execution_commitment = execution_commitment_template;
            execution_commitment.executed_block_wire_hash = block
                .executed_block_wire_hash()
                .expect("canonical snapshot executed block wire");
            let round = ConsensusRound {
                context_id: context.id(),
                height,
                view: block.header().view_change_index(),
            };
            let mut commit_qc = QuorumCertificate {
                round,
                proposal_round: round,
                phase: GlobalPhase::Commit,
                subject,
                execution_commitment,
                signers: vec![0, 1, 2],
                aggregate_signature: vec![1],
            };
            let preimage = commit_qc
                .signer_preimage(&context, 0)
                .expect("snapshot-eviction signer preimage");
            let signatures = commit_qc
                .signers
                .iter()
                .map(|index| {
                    Signature::try_new(
                        keypairs[usize::try_from(*index).expect("fixture signer index")]
                            .private_key(),
                        &preimage,
                    )
                    .expect("sign snapshot-eviction finality vote")
                    .payload()
                    .to_vec()
                })
                .collect::<Vec<_>>();
            let signature_refs = signatures.iter().map(Vec::as_slice).collect::<Vec<_>>();
            commit_qc.aggregate_signature =
                iroha_crypto::bls_normal_aggregate_signatures(&signature_refs)
                    .expect("aggregate snapshot-eviction finality votes");
            let artifact =
                V2FinalityArtifact::new(context, subject, commit_qc, validator_set_pops.clone());
            artifact
                .verify()
                .expect("snapshot-eviction finality fixture verifies");
            parent = Some(artifact.clone());
            artifacts.push(artifact);
        }
        artifacts
    }

    fn snapshot_gate_fixture() -> (
        State,
        Arc<Kura>,
        Arc<SignedBlock>,
        iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
    ) {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block = signed_block_with_transaction(accepted_log_transaction("snapshot gate"));
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block));
        let artifact = signed_complete_wire_finality_for_snapshot_blocks(
            &state.chain_id,
            std::slice::from_ref(&block),
        )
        .into_iter()
        .next()
        .expect("one snapshot finality artifact");
        (state, kura, block, artifact)
    }

    fn store_snapshot_checkpoint_and_manifest(
        state: &State,
        kura: &Kura,
        block: &SignedBlock,
        state_hash: Hash,
        authority: &iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
    ) {
        let height = block.header().height().get();
        kura.store_wsv_checkpoint(height, block.hash(), state_hash)
            .expect("store snapshot gate WSV checkpoint");
        let manifest =
            crate::kura::CommitManifest::new(height, block.hash(), None, None, state_hash, None)
                .with_authenticated_v2_commit_authority(authority);
        kura.store_commit_manifest(manifest)
            .expect("store checkpoint-bound snapshot gate manifest");
        assert_eq!(state.committed_height(), usize::try_from(height).unwrap());
    }

    fn store_complete_snapshot_commit_evidence(
        state: &State,
        kura: &Kura,
        block: &SignedBlock,
        authority: &iroha_data_model::block::consensus_v2::finality::V2FinalityArtifact,
    ) {
        let state_hash = canonical_state_snapshot_hash(state);
        store_snapshot_checkpoint_and_manifest(state, kura, block, state_hash, authority);
        let _ = kura
            .store_v2_finality_artifact(authority)
            .expect("persist complete-wire snapshot finality");
    }

    fn store_complete_snapshot_commit_evidence_for_blocks(
        state: &State,
        kura: &Kura,
        blocks: &[Arc<SignedBlock>],
    ) {
        let artifacts = signed_complete_wire_finality_for_snapshot_blocks(&state.chain_id, blocks);
        let (terminal_artifact, historical_artifacts) = artifacts
            .split_last()
            .expect("snapshot commit evidence requires a terminal block");
        for artifact in historical_artifacts {
            let _ = kura
                .store_v2_finality_artifact(artifact)
                .expect("persist historical complete-wire snapshot finality");
        }
        let terminal_block = blocks
            .last()
            .expect("snapshot commit evidence requires a terminal block");
        store_complete_snapshot_commit_evidence(state, kura, terminal_block, terminal_artifact);
    }

    fn assert_snapshot_bundle_absent(store_dir: &Path) {
        assert!(
            !store_dir.join(SNAPSHOT_CURRENT_FILE_NAME).exists(),
            "rejected snapshot must not publish a current pointer"
        );
        let generations = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        assert!(
            !generations.exists()
                || std::fs::read_dir(&generations)
                    .expect("read unpublished generations directory")
                    .next()
                    .is_none(),
            "rejected snapshot must not leave a selectable immutable generation"
        );
    }

    #[test]
    async fn bounded_snapshot_reader_rejects_oversized_regular_file() {
        let root = tempdir().expect("tempdir");
        let path = root.path().join("oversized");
        std::fs::write(&path, [0_u8; 9]).expect("write oversized fixture");

        let error = read_bounded_stable_regular_file(&path, 8)
            .expect_err("oversized snapshot artifact must fail before allocation");
        assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
    }

    #[cfg(unix)]
    #[test]
    async fn bounded_snapshot_reader_rejects_symlink_and_hardlink() {
        use std::os::unix::fs::symlink;

        let root = tempdir().expect("tempdir");
        let victim = root.path().join("victim");
        let symlink_path = root.path().join("symlink");
        let hardlink_path = root.path().join("hardlink");
        std::fs::write(&victim, b"sensitive victim bytes").expect("write victim");
        symlink(&victim, &symlink_path).expect("create symlink");
        std::fs::hard_link(&victim, &hardlink_path).expect("create hardlink");

        for path in [&symlink_path, &hardlink_path] {
            let error = read_bounded_stable_regular_file(path, 1024)
                .expect_err("linked snapshot artifact must fail closed");
            assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
        }
        assert_eq!(
            std::fs::read(&victim).expect("read victim"),
            b"sensitive victim bytes"
        );
    }

    #[test]
    async fn snapshot_publication_defers_without_checkpoint_and_selects_nothing() {
        let (state, kura, _block, _artifact) = snapshot_gate_fixture();
        let root = tempdir().expect("snapshot gate temp root");
        let store_dir = root.path().join("snapshot");
        let signing_key = checked_random_snapshot_keypair();

        let error = try_write_snapshot(&state, &store_dir, &signing_key, TEST_CHUNK_SIZE)
            .expect_err("a durable body without its checkpoint must defer snapshot publication");
        assert!(matches!(
            error,
            TryWriteError::CommitEvidenceDeferred { .. }
        ));
        assert_snapshot_bundle_absent(&store_dir);
        assert!(
            try_read_snapshot(
                &store_dir,
                &kura,
                LiveQueryStore::start_test,
                BlockCount(1),
                TEST_CHUNK_SIZE,
                signing_key.public_key(),
                &state.chain_id,
                &state.zk_snapshot(),
                #[cfg(feature = "telemetry")]
                StateTelemetry::new(<_>::default(), true),
            )
            .is_err(),
            "restart must not select a rejected unpublished generation"
        );
    }

    #[test]
    async fn snapshot_publication_defers_bound_manifest_without_finality() {
        let (state, kura, block, artifact) = snapshot_gate_fixture();
        let state_hash = canonical_state_snapshot_hash(&state);
        store_snapshot_checkpoint_and_manifest(&state, &kura, &block, state_hash, &artifact);
        let root = tempdir().expect("snapshot gate temp root");
        let store_dir = root.path().join("snapshot");

        let error = try_write_snapshot(
            &state,
            &store_dir,
            &checked_random_snapshot_keypair(),
            TEST_CHUNK_SIZE,
        )
        .expect_err("checkpoint and manifest without finality must defer publication");
        assert!(matches!(
            error,
            TryWriteError::CommitEvidenceDeferred { .. }
        ));
        assert_snapshot_bundle_absent(&store_dir);
    }

    #[test]
    async fn snapshot_publication_rejects_mismatched_state_hash() {
        let (state, kura, block, artifact) = snapshot_gate_fixture();
        let wrong_state_hash = Hash::new(b"adversarial snapshot state hash");
        store_snapshot_checkpoint_and_manifest(&state, &kura, &block, wrong_state_hash, &artifact);
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store exact finality artifact");
        let root = tempdir().expect("snapshot gate temp root");
        let store_dir = root.path().join("snapshot");

        let error = try_write_snapshot(
            &state,
            &store_dir,
            &checked_random_snapshot_keypair(),
            TEST_CHUNK_SIZE,
        )
        .expect_err("a mismatched WSV checkpoint must fail snapshot publication");
        assert!(matches!(error, TryWriteError::CommitEvidence { .. }));
        assert_snapshot_bundle_absent(&store_dir);
    }

    #[test]
    async fn snapshot_publication_rejects_foreign_manifest_authority() {
        let (state, kura, block, artifact) = snapshot_gate_fixture();
        let foreign_block = signed_block_with_transaction(accepted_log_transaction("foreign"));
        let foreign = signed_complete_wire_finality_for_snapshot_blocks(
            &state.chain_id,
            std::slice::from_ref(&foreign_block),
        )
        .into_iter()
        .next()
        .expect("foreign authority artifact");
        let state_hash = canonical_state_snapshot_hash(&state);
        store_snapshot_checkpoint_and_manifest(&state, &kura, &block, state_hash, &foreign);
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store exact finality artifact");
        let root = tempdir().expect("snapshot gate temp root");
        let store_dir = root.path().join("snapshot");

        let error = try_write_snapshot(
            &state,
            &store_dir,
            &checked_random_snapshot_keypair(),
            TEST_CHUNK_SIZE,
        )
        .expect_err("a foreign manifest authority must fail snapshot publication");
        assert!(matches!(error, TryWriteError::CommitEvidence { .. }));
        assert_snapshot_bundle_absent(&store_dir);
    }

    #[test]
    async fn snapshot_publication_accepts_complete_authenticated_tuple() {
        let (state, kura, block, artifact) = snapshot_gate_fixture();
        let state_hash = canonical_state_snapshot_hash(&state);
        store_snapshot_checkpoint_and_manifest(&state, &kura, &block, state_hash, &artifact);
        let _ = kura
            .store_v2_finality_artifact(&artifact)
            .expect("store exact finality artifact");
        let root = tempdir().expect("snapshot gate temp root");
        let store_dir = root.path().join("snapshot");

        try_write_snapshot(
            &state,
            &store_dir,
            &checked_random_snapshot_keypair(),
            TEST_CHUNK_SIZE,
        )
        .expect("complete authenticated commit tuple must permit publication");
        assert_canonical_snapshot_generation(&store_dir);
    }

    #[test]
    async fn snapshot_fixture_key_generation_preserves_algorithm() {
        assert_eq!(
            checked_random_snapshot_keypair().public_key().algorithm(),
            Algorithm::default()
        );
        assert_eq!(
            checked_random_snapshot_bls_keypair()
                .public_key()
                .algorithm(),
            Algorithm::BlsNormal
        );
    }

    #[test]
    async fn snapshot_bootstrap_policy_requires_exact_canonical_digest_and_height() {
        let digest = "1a0861b04fa35fd0d8ea4c2f38baaa478c7430df3466e9401c53f934671747bd";
        let policy = SnapshotBootstrapPolicy {
            enabled: true,
            audited_sha256: Some(digest.to_owned()),
            audited_height: Some(42),
        };
        assert!(policy.validate().is_ok());
        assert!(policy.authorizes(digest, 42));
        assert!(!policy.authorizes(
            "2a0861b04fa35fd0d8ea4c2f38baaa478c7430df3466e9401c53f934671747bd",
            42
        ));
        assert!(!policy.authorizes(digest, 41));

        let invalid_uppercase = SnapshotBootstrapPolicy {
            audited_sha256: Some(digest.to_ascii_uppercase()),
            ..policy.clone()
        };
        assert!(invalid_uppercase.validate().is_err());

        let disabled = SnapshotBootstrapPolicy::default();
        assert!(disabled.validate().is_ok());
        assert!(!disabled.authorizes(digest, 42));
        let disabled_with_authority = SnapshotBootstrapPolicy {
            enabled: false,
            audited_sha256: Some(digest.to_owned()),
            audited_height: Some(42),
        };
        assert!(disabled_with_authority.validate().is_err());
    }

    fn state_factory_with_kura_and_chain(kura: Arc<Kura>, chain_id: ChainId) -> State {
        let query_handle = LiveQueryStore::start_test();
        State::new_with_chain(
            crate::queue::tests::world_with_test_domains(),
            kura,
            query_handle,
            chain_id,
        )
    }

    fn state_factory_with_kura(kura: Arc<Kura>) -> State {
        state_factory_with_kura_and_chain(kura, ChainId::from(TEST_CHAIN_ID))
    }

    fn state_factory() -> State {
        state_factory_with_kura(Kura::blank_kura_for_testing())
    }

    fn sccp_registry_for_snapshot_test() -> crate::state::SccpOnChainRegistryV1 {
        let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
            iroha_data_model::bridge::SccpNetworkV1::EthereumSepolia,
            iroha_data_model::bridge::SccpRouteActivationV1::Staged,
        );
        crate::state::SccpOnChainRegistryV1 {
            version: 1,
            lanes: vec![iroha_data_model::bridge::SccpGovernedLaneV1 {
                lane_id: route.lane_id,
                native_trust_anchors: Vec::new(),
                current_native_trust_anchor_hash: None,
                routes: vec![route],
            }],
        }
    }

    fn state_with_exact_pending_sccp_snapshot_fixture(
        kura: Arc<Kura>,
    ) -> (
        State,
        iroha_data_model::bridge::SccpOutboundMessageKeyV1,
        iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1,
    ) {
        let exact = iroha_sccp::sccp_exact_outbound_test_fixture_v1();
        let provisional_finality =
            iroha_sccp::decode_taira_bridge_finality_proof(&exact.bundle.finality_proof)
                .expect("exact provisional SCCP finality fixture decodes");
        let payload_bytes = iroha_sccp::canonical_sccp_payload_bytes(&exact.bundle.payload)
            .expect("exact SCCP payload encodes canonically");
        let instruction = crate::bridge::test_record_sccp_message(payload_bytes.clone());
        assert_eq!(
            instruction.context, exact.bundle.commitment.context,
            "exact snapshot block instruction must preserve the bundle context"
        );
        let transaction_key = checked_seeded_keypair(0x34, Algorithm::Ed25519);
        let authority = AccountId::new(transaction_key.public_key().clone());
        let transaction = TransactionBuilder::new(
            ChainId::from(iroha_sccp::SCCP_TAIRA_FINALITY_CHAIN_ID_V1),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_executable(iroha_data_model::transaction::Executable::IvmProved(
            iroha_data_model::transaction::IvmProved {
                bytecode: iroha_data_model::transaction::IvmBytecode::from_compiled(vec![
                    0x01, 0x02, 0x03,
                ]),
                overlay: vec![iroha_data_model::isi::InstructionBox::from(instruction)].into(),
                events_commitment: Hash::new(b"snapshot-sccp-events"),
                gas_policy_commitment: Hash::new(b"snapshot-sccp-gas"),
            },
        ))
        .sign(transaction_key.private_key());
        let entry_hash = transaction.hash_as_entrypoint();
        let block_signer = checked_seeded_keypair(0x35, Algorithm::Ed25519);
        let template_header = provisional_finality.block_header;
        let mut provisional_header = iroha_data_model::block::BlockHeader::new(
            template_header.height(),
            template_header.prev_block_hash(),
            None,
            None,
            u64::try_from(template_header.creation_time().as_millis())
                .expect("fixture creation time fits u64"),
            template_header.view_change_index(),
        );
        provisional_header.set_sccp_commitment_root(template_header.sccp_commitment_root());
        let signature = iroha_data_model::block::BlockSignature::new(
            0,
            iroha_crypto::SignatureOf::try_from_hash(
                block_signer.private_key(),
                provisional_header.hash(),
            )
            .expect("sign provisional retained SCCP header"),
        );
        let mut block = SignedBlock::presigned(signature, provisional_header, vec![transaction]);
        block
            .set_transaction_results(
                Vec::new(),
                &[entry_hash],
                vec![iroha_data_model::transaction::TransactionResultInner::Ok(
                    iroha_data_model::transaction::DataTriggerSequence::default(),
                )],
            )
            .expect("exact retained SCCP block results");
        assert!(
            provisional_finality
                .finality_artifact
                .validate_for_header(&block.header())
                .is_err(),
            "pre-finalization SCCP artifact must reject the completed snapshot block"
        );
        let signature = iroha_data_model::block::BlockSignature::new(
            0,
            iroha_crypto::SignatureOf::try_from_hash(block_signer.private_key(), block.hash())
                .expect("sign completed retained SCCP header"),
        );
        block
            .replace_signatures([signature].into_iter().collect())
            .expect("replace provisional retained SCCP signature");
        block
            .signatures()
            .next()
            .expect("completed retained SCCP signature")
            .signature()
            .verify_hash(block_signer.public_key(), block.hash())
            .expect("completed retained SCCP signature verifies");
        crate::bridge::validate_sccp_commitment_root_for_signed_block(&block)
            .expect("completed snapshot block authenticates its exact SCCP message");

        let exact = exact.with_finalized_block(&block, None);
        let finality = iroha_sccp::decode_taira_bridge_finality_proof(&exact.bundle.finality_proof)
            .expect("exact completed SCCP finality fixture decodes");
        assert_eq!(block.header(), finality.block_header);
        assert_eq!(block.hash(), finality.finality_artifact.block_hash);
        assert_eq!(
            exact.request.public_inputs.finality_block_hash,
            <[u8; 32]>::from(Hash::from(block.hash()))
        );
        finality
            .finality_artifact
            .validate_for_header(&block.header())
            .expect("completed snapshot SCCP artifact binds the exact block header");
        finality
            .finality_artifact
            .verify()
            .expect("completed snapshot SCCP artifact is cryptographically valid");
        let block = Arc::new(block);
        kura.persist_block_with_retained_archive_for_tests(&block)
            .expect("persist exact SCCP block and archive");
        let _ = kura
            .store_v2_finality_artifact(&finality.finality_artifact)
            .expect("persist exact SCCP finality artifact");

        let mut state = state_factory_with_kura_and_chain(
            Arc::clone(&kura),
            ChainId::from(iroha_sccp::SCCP_TAIRA_FINALITY_CHAIN_ID_V1),
        );
        state.push_block_hash_for_testing(block.hash());
        let (_, source_identity, trust_anchor) =
            iroha_sccp::sccp_native_ethereum_transfer_inbound_test_fixture_v1();
        assert_eq!(
            exact.route.source_identity, source_identity,
            "exact snapshot route and native trust anchor must share one source identity"
        );
        state.set_sccp_registry_for_testing(
            crate::state::ValidatedSccpRegistryV1::try_from_wire(
                iroha_data_model::bridge::SccpRegistryV1 {
                    version: 1,
                    lanes: vec![iroha_data_model::bridge::SccpGovernedLaneV1 {
                        lane_id: exact.route.lane_id,
                        native_trust_anchors: vec![trust_anchor],
                        current_native_trust_anchor_hash: Some(trust_anchor.anchor_hash),
                        routes: vec![exact.route.clone()],
                    }],
                },
            )
            .expect("exact outbound snapshot registry validates"),
        );
        let key = iroha_data_model::bridge::SccpOutboundMessageKeyV1 {
            lane: exact.bundle.commitment.context.lane,
            message_id: exact.bundle.commitment.message_id,
        };
        let record = iroha_data_model::bridge::SccpOutboundPendingMessageRecordV1 {
            destination_binding_hash: exact.bundle.commitment.context.destination_binding_hash,
            route_configuration_hash: exact.bundle.commitment.context.route_configuration_hash,
            payload_hash: exact.bundle.commitment.payload_hash,
            payload_bytes,
            recorded_at_height: 1,
            commitment_index: 0,
        };
        state
            .insert_sccp_outbound_message_for_testing(key.clone(), record.clone())
            .expect("insert canonical SCCP outbound snapshot fixture");
        store_complete_snapshot_commit_evidence(
            &state,
            &kura,
            block.as_ref(),
            &finality.finality_artifact,
        );
        (state, key, record)
    }
    fn kura_config_for_snapshot_test(
        store_dir: &Path,
        blocks_in_memory: NonZeroUsize,
    ) -> KuraConfig {
        KuraConfig {
            init_mode: InitMode::Strict,
            store_dir: WithOrigin::inline(store_dir.to_path_buf()),
            max_disk_usage_bytes: MAX_DISK_USAGE_BYTES,
            blocks_in_memory,
            debug_output_new_blocks: false,
            merge_ledger_cache_capacity: MERGE_LEDGER_CACHE_CAPACITY,
            fsync_mode: FsyncMode::Batched,
            fsync_interval: FSYNC_INTERVAL,
            block_sync_roster_retention: BLOCK_SYNC_ROSTER_RETENTION,
            roster_sidecar_retention: ROSTER_SIDECAR_RETENTION,
            eviction_required_replicas: EVICTION_REQUIRED_REPLICAS,
        }
    }

    fn install_active_space_directory_manifest(
        state: &mut State,
    ) -> (UniversalAccountId, DataSpaceId, AccountId) {
        let uaid = UniversalAccountId::from_hash(Hash::new(b"snapshot-space-directory"));
        let dataspace = DataSpaceId::new(7);
        let account_id = AccountId::new(checked_random_snapshot_keypair().public_key().clone());
        let details = AccountDetails::new(Metadata::default(), None, Some(uaid), Vec::new());
        state
            .world
            .accounts
            .insert(account_id.clone(), AccountValue::new(details));

        let manifest = AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid,
            dataspace,
            issued_ms: 1,
            activation_epoch: 1,
            expiry_epoch: None,
            entries: Vec::new(),
        };
        let mut record = crate::nexus::space_directory::SpaceDirectoryManifestRecord::new(manifest);
        record.lifecycle.mark_activated(1);
        let mut set = crate::nexus::space_directory::SpaceDirectoryManifestSet::default();
        set.upsert(record);
        state.world.space_directory_manifests.insert(uaid, set);

        (uaid, dataspace, account_id)
    }

    #[test]
    async fn canonical_wsv_hash_ignores_commit_qc_sidecars() {
        let mut state = state_factory();
        let before = canonical_state_snapshot_bytes_for_tests(&state);
        let key_pair = checked_random_snapshot_bls_keypair();
        let peer = PeerId::new(key_pair.public_key().clone());
        let roster = vec![peer];
        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xC7; Hash::LENGTH]));
        let zero_root = Hash::prehashed([0_u8; Hash::LENGTH]);
        let qc = Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height: 2,
            view: 0,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_owned(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster,
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: vec![0xAA; 96],
            },
        };

        state.insert_commit_qc_for_testing(block_hash, qc);
        let mut restart_snapshot = String::new();
        serialize_state_snapshot(&state, &mut restart_snapshot, true);
        assert!(
            restart_snapshot.contains("\"commit_qcs\""),
            "restart snapshots must retain the historical commit-QC archive"
        );

        let after = canonical_state_snapshot_bytes_for_tests(&state);
        assert_eq!(
            before, after,
            "commit-QC recovery evidence must not affect replay WSV checkpoints"
        );
        let canonical_json =
            String::from_utf8(after).expect("canonical state snapshot should be utf8 json");
        assert!(
            !canonical_json.contains("\"commit_qcs\""),
            "canonical WSV checkpoint surface should omit commit-QC sidecars"
        );
    }

    #[test]
    async fn canonical_wsv_hash_uses_current_mv_cell_values() {
        let state = state_factory();
        let before = canonical_state_snapshot_bytes_for_tests(&state);

        {
            let mut parameters = state.world.parameters.block();
            let current = parameters.get().clone();
            *parameters.get_mut() = current;
            parameters.commit();
        }

        let after = canonical_state_snapshot_bytes_for_tests(&state);
        assert_eq!(
            before, after,
            "MV cell history must not affect replay WSV checkpoints when the current value is unchanged"
        );

        let value = canonical_state_snapshot_value(&state);
        let parameters = value
            .get("world")
            .and_then(|world| world.get("parameters"))
            .and_then(json::Value::as_object)
            .expect("canonical snapshot should contain parameters as a plain object");
        assert!(
            !parameters.contains_key("revert") && !parameters.contains_key("blocks"),
            "canonical WSV checkpoint surface should serialize current cell values"
        );
    }

    fn test_vrf_epoch_record(epoch: u64) -> iroha_data_model::consensus::VrfEpochRecord {
        iroha_data_model::consensus::VrfEpochRecord {
            epoch,
            seed: [0_u8; 32],
            epoch_length: 1,
            commit_deadline_offset: 0,
            reveal_deadline_offset: 0,
            roster_len: 0,
            finalized: false,
            updated_at_height: 0,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        }
    }

    #[test]
    async fn canonical_wsv_hash_ignores_vrf_epoch_sidecars() {
        let state = state_factory();
        let before = canonical_state_snapshot_bytes_for_tests(&state);

        {
            let mut world = state.world.block();
            world.vrf_epochs.insert(0, test_vrf_epoch_record(0));
            world.commit();
        }
        let after = canonical_state_snapshot_bytes_for_tests(&state);

        assert_eq!(
            before, after,
            "VRF epoch sidecars must not affect replay WSV checkpoints"
        );

        let value = canonical_state_snapshot_value(&state);
        let world = value
            .get("world")
            .and_then(json::Value::as_object)
            .expect("canonical snapshot should contain world as an object");
        assert!(
            !world.contains_key("vrf_epochs"),
            "canonical WSV checkpoint surface should omit VRF epoch sidecars"
        );
    }

    #[test]
    async fn canonical_wsv_hash_sorts_sumeragi_key_policy_sets() {
        let state = state_factory();

        {
            let mut parameters = state.world.parameters.block();
            parameters.sumeragi.key_allowed_algorithms = vec![
                Algorithm::Secp256k1,
                Algorithm::Ed25519,
                Algorithm::Secp256k1,
            ];
            parameters.sumeragi.key_allowed_hsm_providers = vec![
                "yubihsm".to_owned(),
                "pkcs11".to_owned(),
                "softkey".to_owned(),
                "pkcs11".to_owned(),
            ];
            parameters.commit();
        }
        let first = canonical_state_snapshot_bytes_for_tests(&state);

        {
            let mut parameters = state.world.parameters.block();
            parameters.sumeragi.key_allowed_algorithms =
                vec![Algorithm::Ed25519, Algorithm::Secp256k1];
            parameters.sumeragi.key_allowed_hsm_providers = vec![
                "pkcs11".to_owned(),
                "softkey".to_owned(),
                "yubihsm".to_owned(),
            ];
            parameters.commit();
        }
        let second = canonical_state_snapshot_bytes_for_tests(&state);

        assert_eq!(
            first, second,
            "set-like Sumeragi key policy fields must not make WSV checkpoints order-sensitive"
        );

        let value = canonical_state_snapshot_value(&state);
        let providers = value
            .get("world")
            .and_then(|world| world.get("parameters"))
            .and_then(|parameters| parameters.get("sumeragi"))
            .and_then(|sumeragi| sumeragi.get("key_allowed_hsm_providers"))
            .and_then(json::Value::as_array)
            .expect("canonical snapshot should contain normalized HSM providers");
        let providers = providers
            .iter()
            .map(|value| match value {
                json::Value::String(provider) => provider.as_str(),
                _ => panic!("HSM provider should serialize as a string"),
            })
            .collect::<Vec<_>>();
        assert_eq!(providers, ["pkcs11", "softkey", "yubihsm"]);
    }

    #[test]
    async fn canonical_state_snapshot_ignores_consensus_evidence_caches() {
        let state = state_factory();
        let expected = canonical_state_snapshot_bytes_for_tests(&state);

        let keypair = checked_random_snapshot_bls_keypair();
        let peer = PeerId::new(keypair.public_key().clone());
        let roster = vec![peer.clone()];
        let block_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA1; Hash::LENGTH]));
        let commit_qc = crate::sumeragi::consensus::Qc {
            phase: crate::sumeragi::consensus::Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: Hash::prehashed([0u8; Hash::LENGTH]),
            post_state_root: Hash::prehashed([1u8; Hash::LENGTH]),
            height: 2,
            view: 1,
            epoch: 0,
            chain_order_hash: crate::sumeragi::consensus::default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: crate::sumeragi::consensus::PERMISSIONED_TAG.to_string(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&roster),
            validator_set_hash_version: iroha_data_model::consensus::VALIDATOR_SET_HASH_VERSION_V1,
            validator_set: roster,
            aggregate: crate::sumeragi::consensus::QcAggregate {
                signers_bitmap: vec![0b0000_0001],
                bls_aggregate_signature: Vec::new(),
            },
        };
        let vrf_epoch = iroha_data_model::consensus::VrfEpochRecord {
            epoch: 0,
            seed: [0x42; 32],
            epoch_length: 10,
            commit_deadline_offset: 2,
            reveal_deadline_offset: 4,
            roster_len: 1,
            finalized: false,
            updated_at_height: 2,
            participants: Vec::new(),
            late_reveals: Vec::new(),
            committed_no_reveal: Vec::new(),
            no_participation: Vec::new(),
            penalties_applied: false,
            penalties_applied_at_height: None,
            validator_election: None,
        };

        {
            let mut world = state.world.block();
            world
                .commit_qcs_mut_for_testing()
                .insert(block_hash, commit_qc);
            world
                .vrf_epochs_mut_for_testing()
                .insert(vrf_epoch.epoch, vrf_epoch);
            world.commit();
        }
        {
            let mut commit_topology = state.commit_topology.block();
            commit_topology.push(peer.clone());
            commit_topology.commit();
        }
        {
            let mut prev_commit_topology = state.prev_commit_topology.block();
            prev_commit_topology.push(peer);
            prev_commit_topology.commit();
        }

        assert_eq!(
            canonical_state_snapshot_bytes_for_tests(&state),
            expected,
            "consensus evidence caches must not perturb canonical replay checkpoints"
        );
    }

    fn sample_space_directory_manifest() -> AssetPermissionManifest {
        AssetPermissionManifest {
            version: ManifestVersion::default(),
            uaid: UniversalAccountId::from_hash(Hash::new(b"snapshot-legacy-manifest")),
            dataspace: DataSpaceId::new(11),
            issued_ms: 1,
            activation_epoch: 1,
            expiry_epoch: None,
            entries: Vec::new(),
        }
    }

    fn insert_account_with_uaid(state: &mut State, uaid: UniversalAccountId) -> AccountId {
        let account_id = AccountId::new(checked_random_snapshot_keypair().public_key().clone());
        let details = AccountDetails::new(Metadata::default(), None, Some(uaid), Vec::new());
        state
            .world
            .accounts
            .insert(account_id.clone(), AccountValue::new(details));
        account_id
    }

    fn accepted_manifest_transaction() -> AcceptedTransaction<'static> {
        let key_pair = checked_seeded_keypair(0x31, Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let transaction = TransactionBuilder::new(
            ChainId::from(TEST_CHAIN_ID),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([PublishSpaceDirectoryManifest {
            manifest: sample_space_directory_manifest(),
        }])
        .sign(key_pair.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
    }

    fn accepted_log_transaction(message: &str) -> AcceptedTransaction<'static> {
        let key_pair = checked_seeded_keypair(0x32, Algorithm::Ed25519);
        let authority = AccountId::new(key_pair.public_key().clone());
        let transaction = TransactionBuilder::new(
            ChainId::from(TEST_CHAIN_ID),
            authority,
            iroha_data_model::transaction::FeePaymentIntent::authority(Vec::new(), None),
        )
        .with_instructions([Log::new(Level::INFO, message.to_owned())])
        .sign(key_pair.private_key());
        AcceptedTransaction::new_unchecked(Cow::Owned(transaction))
    }

    fn signed_block_with_transaction(
        transaction: AcceptedTransaction<'static>,
    ) -> Arc<SignedBlock> {
        signed_block_after_transaction(transaction, None)
    }

    fn signed_block_after_transaction(
        transaction: AcceptedTransaction<'static>,
        latest_block: Option<&SignedBlock>,
    ) -> Arc<SignedBlock> {
        let block_signer = checked_seeded_keypair(0x33, Algorithm::BlsNormal);
        Arc::new(
            BlockBuilder::new(vec![transaction])
                .chain(0, latest_block)
                .sign(block_signer.private_key())
                .unpack(|_| {})
                .into(),
        )
    }

    fn legacy_snapshot_bytes_without_space_directory_section(state: &State) -> Vec<u8> {
        let mut payload = String::new();
        serialize_state_snapshot(state, &mut payload, false);
        payload.into_bytes()
    }

    fn exact_snapshot_payload_bytes(state: &State) -> Vec<u8> {
        let mut payload = String::new();
        serialize_state_snapshot(state, &mut payload, true);
        payload.into_bytes()
    }

    fn publish_test_snapshot_generation(
        store_dir: &std::path::Path,
        bytes: &[u8],
        key_pair: &KeyPair,
    ) -> (StableSnapshotFileIdentity, PublishedSnapshotGeneration) {
        std::fs::create_dir_all(store_dir).expect("snapshot dir");
        let digest_bytes = Sha256::digest(bytes);
        let digest_vec = digest_bytes.to_vec();
        let digest_hex = hex::encode(&digest_vec);
        let digest_line = format!("{digest_hex}\n").into_bytes();
        let signature = Signature::try_new(key_pair.private_key(), &digest_vec)
            .expect("checked snapshot signature");
        let signature_hex = hex::encode(signature.payload()).into_bytes();
        let merkle = SnapshotMerkleMetadata::from_bytes(bytes, TEST_CHUNK_SIZE);
        let merkle_bytes = json::to_json(&merkle)
            .expect("canonical snapshot merkle")
            .into_bytes();
        let merkle_limit = SNAPSHOT_MERKLE_FIXED_OVERHEAD_BYTES.saturating_add(
            u64::try_from(merkle.leaf_hashes_hex.len())
                .unwrap_or(u64::MAX)
                .saturating_mul(SNAPSHOT_MERKLE_BYTES_PER_LEAF),
        );
        let store_identity =
            direct_snapshot_directory_identity(store_dir).expect("bind snapshot root");
        let generation = publish_immutable_snapshot_generation(
            store_dir,
            store_identity,
            &digest_hex,
            bytes,
            &digest_line,
            &signature_hex,
            &merkle_bytes,
            merkle_limit,
            key_pair.public_key(),
        )
        .expect("publish immutable test generation");
        (store_identity, generation)
    }

    fn write_snapshot_bundle_from_bytes(
        store_dir: &std::path::Path,
        bytes: &[u8],
        key_pair: &KeyPair,
    ) {
        let (store_identity, generation) =
            publish_test_snapshot_generation(store_dir, bytes, key_pair);
        publish_snapshot_current_pointer(
            store_dir,
            store_identity,
            &generation,
            defaults::snapshot::MAX_PAYLOAD_BYTES,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
        )
        .expect("publish canonical test pointer");
    }

    fn store_block_and_mark_state_height(
        state: &mut State,
        kura: &Arc<Kura>,
        block: Arc<SignedBlock>,
    ) {
        kura.store_block(Arc::clone(&block)).expect("store block");
        state.push_block_hash_for_testing(block.hash());
    }

    fn signed_commit_qc_for_snapshot(
        chain_id: &ChainId,
        block_hash: HashOf<BlockHeader>,
        height: u64,
        validator: &KeyPair,
    ) -> Qc {
        let validator_set = vec![PeerId::new(validator.public_key().clone())];
        let zero_root = Hash::prehashed([0; Hash::LENGTH]);
        let vote = Vote {
            phase: Phase::Commit,
            block_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height,
            view: 0,
            epoch: 0,
            chain_order_hash: default_chain_order_hash(),
            rechain_seq: 0,
            highest_qc: None,
            signer: 0,
            bls_sig: Vec::new(),
        };
        let preimage = vote_preimage(chain_id, PERMISSIONED_TAG, &vote);
        let signature = Signature::try_new(validator.private_key(), &preimage)
            .expect("snapshot commit vote signature");
        let aggregate = iroha_crypto::bls_normal_aggregate_signatures(&[signature.payload()])
            .expect("snapshot aggregate commit signature");
        Qc {
            phase: Phase::Commit,
            subject_block_hash: block_hash,
            parent_state_root: zero_root,
            post_state_root: zero_root,
            height,
            view: 0,
            epoch: 0,
            chain_order_hash: default_chain_order_hash(),
            rechain_seq: 0,
            mode_tag: PERMISSIONED_TAG.to_owned(),
            highest_qc: None,
            validator_set_hash: HashOf::new(&validator_set),
            validator_set_hash_version: VALIDATOR_SET_HASH_VERSION_V1,
            validator_set,
            aggregate: QcAggregate {
                signers_bitmap: vec![1],
                bls_aggregate_signature: aggregate,
            },
        }
    }

    fn model_rotated_disabled_removed_validator(
        state: &mut State,
        historical_validator: &KeyPair,
    ) -> Vec<u8> {
        let historical_pop = iroha_crypto::bls_normal_pop_prove(historical_validator.private_key())
            .expect("historical validator PoP");
        state.world.register_validator_pop_for_testing(
            historical_validator.public_key().clone(),
            historical_pop.clone(),
        );
        let replacement = checked_random_snapshot_bls_keypair();
        let replacement_pop = iroha_crypto::bls_normal_pop_prove(replacement.private_key())
            .expect("replacement validator PoP");
        state
            .world
            .register_validator_pop_for_testing(replacement.public_key().clone(), replacement_pop);

        let historical_id = derive_validator_key_id(historical_validator.public_key());
        let replacement_id = derive_validator_key_id(replacement.public_key());
        let mut world = state.world.block();
        let mut historical_record = world
            .consensus_keys
            .get(&historical_id)
            .cloned()
            .expect("historical consensus record");
        historical_record.status = ConsensusKeyStatus::Disabled;
        historical_record.expiry_height = Some(2);
        world
            .consensus_keys
            .insert(historical_id.clone(), historical_record);
        let mut replacement_record = world
            .consensus_keys
            .get(&replacement_id)
            .cloned()
            .expect("replacement consensus record");
        replacement_record.replaces = Some(historical_id);
        world
            .consensus_keys
            .insert(replacement_id, replacement_record);
        world.commit();

        assert!(
            state
                .world
                .peers
                .view()
                .iter()
                .all(|peer| peer.public_key() != historical_validator.public_key()),
            "historical validator must be absent from the live peer roster"
        );
        historical_pop
    }

    #[test]
    async fn creates_all_dirs_while_writing_snapshots() {
        let tmp_root = tempdir().unwrap();
        let snapshot_store_dir = tmp_root.path().join("path/to/snapshot/dir");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &snapshot_store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        assert!(Path::exists(snapshot_store_dir.as_path()));
        assert_canonical_snapshot_generation(&snapshot_store_dir);
    }

    #[test]
    async fn can_read_snapshot_after_writing() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let expected_chain_id = state.chain_id.clone();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let kura = Kura::blank_kura_for_testing();
        let snapshot_state = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .unwrap();
        assert_eq!(snapshot_state.chain_id, expected_chain_id);
        assert_eq!(
            canonical_state_snapshot_bytes_for_tests(&snapshot_state),
            canonical_state_snapshot_bytes_for_tests(&state),
            "snapshot roundtrip must preserve canonical WSV bytes"
        );
    }

    #[test]
    async fn signed_snapshot_roundtrip_preserves_authoritative_alias_revert_maps() {
        let tmp_root = tempdir().expect("snapshot tempdir");
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let owner = {
            let accounts = state.world.accounts.view();
            accounts
                .iter()
                .next()
                .map(|(account_id, _)| account_id.clone())
                .expect("fixture account")
        };

        let account_alias = AccountAlias::new(
            "restart_alias".parse().expect("account alias label"),
            Some(AccountAliasDomain::new(
                "wonderland".parse().expect("account alias domain"),
            )),
            DataSpaceId::UNIVERSAL,
        );
        let account_rekey_record = AccountRekeyRecord::new(account_alias.clone(), owner.clone());
        {
            let mut aliases = state.world.account_aliases.block();
            assert!(
                aliases
                    .insert(account_alias.clone(), owner.clone())
                    .is_none()
            );
            aliases.commit();
        }
        {
            let mut records = state.world.account_rekey_records.block();
            assert!(
                records
                    .insert(account_alias.clone(), account_rekey_record)
                    .is_none()
            );
            records.commit();
        }

        let definition_id = AssetDefinitionId::new(
            DomainId::try_new("wonderland", "universal").expect("asset domain"),
            "restart_asset".parse().expect("asset name"),
        );
        let definition = AssetDefinition::numeric(definition_id.clone())
            .with_name("restart asset".to_owned())
            .build(&owner);
        let definition_alias: AssetDefinitionAlias =
            "restart_asset#universal".parse().expect("asset alias");
        let definition_binding = AssetDefinitionAliasBindingRecord {
            alias: definition_alias,
            lease_expiry_ms: None,
            grace_until_ms: None,
            bound_at_ms: 1,
        };
        {
            let mut definitions = state.world.asset_definitions.block();
            assert!(
                definitions
                    .insert(definition_id.clone(), definition)
                    .is_none()
            );
            definitions.commit();
        }
        {
            let mut bindings = state.world.asset_definition_alias_bindings.block();
            assert!(
                bindings
                    .insert(definition_id.clone(), definition_binding)
                    .is_none()
            );
            bindings.commit();
        }

        let contract_address = ContractAddress::derive(
            CHAIN_DISCRIMINANT_MAINNET,
            &owner,
            17,
            DataSpaceId::UNIVERSAL,
        )
        .expect("contract address");
        let contract_alias: ContractAlias =
            "restart_router::universal".parse().expect("contract alias");
        let contract_binding = ContractAliasBindingRecord {
            alias: contract_alias,
            lease_expiry_ms: None,
            grace_until_ms: None,
            bound_at_ms: 1,
        };
        {
            let mut bindings = state.world.contract_alias_bindings.block();
            assert!(
                bindings
                    .insert(contract_address.clone(), contract_binding)
                    .is_none()
            );
            bindings.commit();
        }

        let key_pair = checked_random_snapshot_keypair();
        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("write signed snapshot with authoritative alias revert maps");
        let payload = std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
            .expect("read signed snapshot payload");
        let kura = Kura::blank_kura_for_testing();
        let restored = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(0),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("read signed snapshot without canonical payload drift");

        let mut roundtrip = String::new();
        serialize_state_snapshot(&restored, &mut roundtrip, true);
        assert_eq!(
            roundtrip.as_bytes(),
            payload,
            "restoring derived alias indexes must not alter authoritative snapshot bytes"
        );

        let aliases = restored.world.account_aliases.block_and_revert();
        assert!(aliases.get(&account_alias).is_none());
        aliases.commit();
        let records = restored.world.account_rekey_records.block_and_revert();
        assert!(records.get(&account_alias).is_none());
        records.commit();
        let definitions = restored.world.asset_definitions.block_and_revert();
        assert!(definitions.get(&definition_id).is_none());
        definitions.commit();
        let definition_bindings = restored
            .world
            .asset_definition_alias_bindings
            .block_and_revert();
        assert!(definition_bindings.get(&definition_id).is_none());
        definition_bindings.commit();
        let contract_bindings = restored.world.contract_alias_bindings.block_and_revert();
        assert!(contract_bindings.get(&contract_address).is_none());
        contract_bindings.commit();
    }

    #[test]
    async fn historical_finality_bundle_survives_validator_lifecycle_and_snapshot_restart() {
        let _history_guard = crate::sumeragi::status::commit_history_test_guard();
        crate::sumeragi::status::reset_commit_certs_for_tests();

        let tmp_root = tempdir().expect("snapshot tempdir");
        let store_dir = tmp_root.path().join("snapshot");
        let kura_store_dir = tmp_root.path().join("kura");
        let lane_config = LaneConfig::default();
        let kura_config = kura_config_for_snapshot_test(&kura_store_dir, nonzero!(1_usize));
        let (kura, _) = Kura::new(&kura_config, &lane_config).expect("create persistent Kura");
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block1 = signed_block_with_transaction(accepted_log_transaction("historical-1"));
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("historical-finality-snapshot-restart"),
            Some(block1.as_ref()),
        );
        let block_hash = block2.hash();
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let block3 = signed_block_after_transaction(
            accepted_log_transaction("historical-3"),
            Some(block2.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block3));

        let historical_validator = checked_random_snapshot_bls_keypair();
        let expected_pop =
            model_rotated_disabled_removed_validator(&mut state, &historical_validator);
        let commit_qc =
            signed_commit_qc_for_snapshot(&state.chain_id, block_hash, 2, &historical_validator);
        state.insert_commit_qc_for_testing(block_hash, commit_qc.clone());
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            &[Arc::clone(&block1), Arc::clone(&block2), block3],
        );

        let snapshot_key = checked_random_snapshot_keypair();
        try_write_snapshot(&state, &store_dir, &snapshot_key, TEST_CHUNK_SIZE)
            .expect("write snapshot with historical finality archive");
        let payload_len = kura
            .advertise_required_replicas_for_bench(nonzero!(2_usize))
            .expect("historical block payload length");
        let freed = kura
            .evict_block_bodies_for_bench(payload_len)
            .expect("evict historical block into durable DA sidecar");
        assert!(freed >= payload_len);
        let expected_chain_id = state.chain_id.clone();
        drop(state);
        drop(kura);

        let (kura, block_count) =
            Kura::new(&kura_config, &lane_config).expect("reopen Kura after body eviction");
        assert_eq!(
            kura.get_block(nonzero!(2_usize)).map(|block| block.hash()),
            Some(block_hash),
            "reopened Kura must load the exact historical body from its DA sidecar"
        );
        assert!(
            kura.read_roster_metadata(2).is_none(),
            "fixture must require the snapshot archive rather than a Kura roster sidecar"
        );
        let restored = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            block_count,
            TEST_CHUNK_SIZE,
            snapshot_key.public_key(),
            &expected_chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("restore historical finality archive from snapshot");

        assert_eq!(
            restored.commit_qc_for_block(2, block_hash),
            Some(commit_qc.clone()),
            "snapshot restart must restore the exact historical commit QC"
        );
        assert!(
            crate::sumeragi::status::commit_qc_history().is_empty(),
            "fixture must require durable state rather than process-local QC history"
        );
        let historical_id = derive_validator_key_id(historical_validator.public_key());
        let restored_world = restored.world.view();
        let historical_record = restored_world
            .consensus_keys
            .get(&historical_id)
            .expect("restored historical consensus key");
        assert_eq!(historical_record.status, ConsensusKeyStatus::Disabled);
        assert_eq!(
            historical_record.pop.as_deref(),
            Some(expected_pop.as_slice())
        );
        assert!(
            restored_world
                .peers
                .iter()
                .all(|peer| { peer.public_key() != historical_validator.public_key() })
        );
        assert!(
            restored_world
                .consensus_keys
                .iter()
                .any(|(_, record)| { record.replaces.as_ref() == Some(&historical_id) })
        );
        drop(restored_world);
        crate::sumeragi::status::reset_commit_certs_for_tests();
    }

    #[test]
    async fn signed_snapshot_rejects_malformed_historical_commit_qc_archive_entries() {
        let tmp_root = tempdir().expect("snapshot tempdir");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura_and_chain(
            Arc::clone(&kura),
            iroha_data_model::ChainId::from(iroha_sccp::SCCP_TAIRA_FINALITY_CHAIN_ID_V1),
        );
        let block = signed_block_with_transaction(accepted_log_transaction(
            "malformed-historical-commit-qc-archive",
        ));
        let block_hash = block.hash();
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block));
        let validator = checked_random_snapshot_bls_keypair();
        let valid = signed_commit_qc_for_snapshot(&state.chain_id, block_hash, 1, &validator);
        let other_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xE7; Hash::LENGTH]));
        let malformed = [
            ("wrong-phase", {
                let mut qc = valid.clone();
                qc.phase = Phase::Prepare;
                qc
            }),
            ("wrong-height", {
                let mut qc = valid.clone();
                qc.height = 2;
                qc
            }),
            ("wrong-subject-hash", {
                let mut qc = valid;
                qc.subject_block_hash = other_hash;
                qc
            }),
        ];
        let snapshot_key = checked_random_snapshot_keypair();
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            std::slice::from_ref(&block),
        );

        for (label, malformed_qc) in malformed {
            state.insert_commit_qc_for_testing(block_hash, malformed_qc);
            let store_dir = tmp_root.path().join(label);
            try_write_snapshot(&state, &store_dir, &snapshot_key, TEST_CHUNK_SIZE)
                .expect("write adversarially signed snapshot fixture");
            let error = match try_read_snapshot(
                &store_dir,
                &kura,
                LiveQueryStore::start_test,
                BlockCount(1),
                TEST_CHUNK_SIZE,
                snapshot_key.public_key(),
                &state.chain_id,
                &crate::state::default_zk_config(),
                #[cfg(feature = "telemetry")]
                StateTelemetry::new(<_>::default(), true),
            ) {
                Ok(_) => panic!("signed snapshot with malformed commit-QC archive must reject"),
                Err(error) => error,
            };
            assert!(
                matches!(error, TryReadError::Serialization(_)),
                "unexpected {label} archive rejection: {error:?}"
            );
        }
    }

    #[test]
    async fn snapshot_roundtrip_preserves_exact_sccp_registry() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura_and_chain(
            Arc::clone(&kura),
            iroha_data_model::ChainId::from(iroha_sccp::SCCP_TAIRA_FINALITY_CHAIN_ID_V1),
        );
        let block =
            signed_block_with_transaction(accepted_log_transaction("exact-sccp-registry-snapshot"));
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block));
        let registry = sccp_registry_for_snapshot_test();
        let expected_key = registry.lanes[0].routes[0].key();
        let expected_config = registry.lanes[0].routes[0]
            .route_configuration_hash()
            .expect("exact snapshot route configuration");
        {
            let mut cell = state.world.sccp_registry.block();
            *cell.get_mut() = registry;
            cell.commit();
        }
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            std::slice::from_ref(&block),
        );
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("write exact SCCP registry snapshot");
        let snapshot_bytes =
            std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot bytes");
        let snapshot_value: json::Value =
            json::from_slice(&snapshot_bytes).expect("snapshot JSON should parse");
        assert!(snapshot_world_has_field(&snapshot_value, "sccp_registry"));

        let snapshot_state = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("snapshot read");
        let restored = snapshot_state.sccp_registry_snapshot();
        let route = restored
            .route(&expected_key)
            .expect("exact SCCP route survives snapshot roundtrip");
        assert_eq!(
            route
                .route_configuration_hash()
                .expect("restored route configuration"),
            expected_config
        );
    }

    #[test]
    async fn signed_snapshot_rejects_unknown_root_and_world_fields() {
        for (scope, expected_field) in [
            ("root", "state.future_snapshot_field"),
            ("world", "world.sccp_registry_v2"),
        ] {
            let tmp_root = tempdir().expect("temporary snapshot root");
            let store_dir = tmp_root.path().join("snapshot");
            let kura = Kura::blank_kura_for_testing();
            let state = state_factory_with_kura(Arc::clone(&kura));
            let mut serialized = String::new();
            serialize_state_snapshot(&state, &mut serialized, true);
            let mut snapshot: json::Value =
                json::from_str(&serialized).expect("valid baseline snapshot JSON");
            let json::Value::Object(snapshot_object) = &mut snapshot else {
                panic!("snapshot root must be an object");
            };
            match scope {
                "root" => {
                    assert!(
                        snapshot_object
                            .insert("future_snapshot_field".to_owned(), json::Value::Null,)
                            .is_none()
                    );
                }
                "world" => {
                    let Some(json::Value::Object(world)) = snapshot_object.get_mut("world") else {
                        panic!("snapshot world must be an object");
                    };
                    assert!(
                        world
                            .insert("sccp_registry_v2".to_owned(), json::Value::Null)
                            .is_none()
                    );
                }
                _ => unreachable!("closed test scope"),
            }
            serialized = json::to_json(&snapshot).expect("mutated snapshot JSON encodes");
            let key_pair = checked_random_snapshot_keypair();
            write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);

            let error = match try_read_snapshot(
                &store_dir,
                &kura,
                LiveQueryStore::start_test,
                BlockCount(0),
                TEST_CHUNK_SIZE,
                key_pair.public_key(),
                &state.chain_id,
                &crate::state::default_zk_config(),
                #[cfg(feature = "telemetry")]
                StateTelemetry::new(<_>::default(), true),
            ) {
                Ok(_) => panic!("signed snapshot with an unknown field must fail closed"),
                Err(error) => error,
            };
            match error {
                TryReadError::Serialization(json::Error::InvalidField { field, message }) => {
                    assert_eq!(field, expected_field);
                    assert!(message.contains("unknown field"), "{message}");
                }
                other => panic!("unexpected unknown-field rejection: {other:?}"),
            }
        }
    }

    #[test]
    async fn signed_semantically_valid_wsv_tampering_is_rejected_by_kura_checkpoint() {
        let tmp_root = tempdir().expect("temporary snapshot root");
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block = signed_block_with_transaction(accepted_log_transaction("checkpointed"));
        let block_hash = block.hash();
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block));
        let expected = canonical_state_snapshot_hash(&state);
        kura.store_wsv_checkpoint(1, block_hash, expected)
            .expect("persist canonical WSV checkpoint");
        let key_pair = checked_random_snapshot_keypair();
        let mut serialized = String::new();
        serialize_state_snapshot(&state, &mut serialized, true);
        write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);
        let restored = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &state.zk_snapshot(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("an exact signed snapshot must match its Kura WSV checkpoint");
        assert_eq!(canonical_state_snapshot_hash(&restored), expected);
        drop(restored);

        let injected_account = AccountId::new(
            checked_seeded_keypair(0xD1, Algorithm::Ed25519)
                .public_key()
                .clone(),
        );
        state.world.accounts.insert(
            injected_account,
            AccountValue::new(AccountDetails::new(
                Metadata::default(),
                None,
                None,
                Vec::new(),
            )),
        );
        let actual = canonical_state_snapshot_hash(&state);
        assert_ne!(
            actual, expected,
            "hostile WSV mutation must affect its checkpoint"
        );
        serialized.clear();
        serialize_state_snapshot(&state, &mut serialized, true);
        write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);

        let error = match try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &state.zk_snapshot(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("a signature cannot replace the canonical Kura WSV checkpoint"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TryReadError::WsvCheckpointMismatch {
                height: 1,
                expected: observed_expected,
                actual: observed_actual,
            } if observed_expected == expected && observed_actual == actual
        ));
        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            kura.wsv_checkpoint(1)
                .expect("read checkpoint after rejection")
                .expect("checkpoint remains present")
                .state_hash(),
            expected,
            "rejected snapshot must not replace the durable WSV checkpoint"
        );
    }

    #[test]
    async fn signed_hostile_sccp_registry_snapshots_are_rejected_before_acceptance() {
        enum RegistryCellMutation {
            Replace {
                role: &'static str,
                registry: crate::state::SccpOnChainRegistryV1,
            },
            Remove(&'static str),
            AddUnknown,
        }

        let assert_rejected = |mutation: RegistryCellMutation, expected: &str| {
            let tmp_root = tempdir().expect("temporary snapshot root");
            let store_dir = tmp_root.path().join("snapshot");
            let kura = Kura::blank_kura_for_testing();
            let state = state_factory_with_kura_and_chain(
                Arc::clone(&kura),
                iroha_data_model::ChainId::from(iroha_sccp::SCCP_TAIRA_FINALITY_CHAIN_ID_V1),
            );
            let mut serialized = String::new();
            serialize_state_snapshot(&state, &mut serialized, true);
            let mut snapshot: json::Value =
                json::from_str(&serialized).expect("valid baseline snapshot JSON");
            let json::Value::Object(snapshot_object) = &mut snapshot else {
                panic!("snapshot root must be an object");
            };
            let Some(json::Value::Object(world)) = snapshot_object.get_mut("world") else {
                panic!("snapshot world must be an object");
            };
            let Some(json::Value::Object(cell)) = world.get_mut("sccp_registry") else {
                panic!("snapshot SCCP registry must be one cell envelope");
            };
            match mutation {
                RegistryCellMutation::Replace { role, registry } => {
                    cell.insert(
                        role.to_owned(),
                        json::to_value(&registry).expect("hostile SCCP registry encodes"),
                    );
                }
                RegistryCellMutation::Remove(role) => {
                    assert!(cell.remove(role).is_some(), "baseline cell contains {role}");
                }
                RegistryCellMutation::AddUnknown => {
                    assert!(
                        cell.insert("future_registry".to_owned(), json::Value::Null)
                            .is_none()
                    );
                }
            }
            serialized = json::to_json(&snapshot).expect("mutated snapshot JSON encodes");
            let key_pair = checked_random_snapshot_keypair();
            write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);

            let result = try_read_snapshot(
                &store_dir,
                &kura,
                LiveQueryStore::start_test,
                BlockCount(0),
                TEST_CHUNK_SIZE,
                key_pair.public_key(),
                &state.chain_id,
                &crate::state::default_zk_config(),
                #[cfg(feature = "telemetry")]
                StateTelemetry::new(<_>::default(), true),
            );
            match result {
                Err(TryReadError::InvalidSccpRegistry(error)) => {
                    assert!(error.contains(expected), "{error}");
                }
                Err(error) => panic!("unexpected snapshot error: {error:?}"),
                Ok(_) => panic!("signed hostile SCCP registry snapshot must be rejected"),
            }
        };

        assert_rejected(
            RegistryCellMutation::Replace {
                role: "blocks",
                registry: crate::state::SccpOnChainRegistryV1 {
                    version: 2,
                    lanes: Vec::new(),
                },
            },
            "version",
        );
        assert_rejected(
            RegistryCellMutation::Replace {
                role: "revert",
                registry: crate::state::SccpOnChainRegistryV1 {
                    version: 2,
                    lanes: Vec::new(),
                },
            },
            "revert",
        );
        assert_rejected(RegistryCellMutation::Remove("blocks"), "missing `blocks`");
        assert_rejected(RegistryCellMutation::Remove("revert"), "missing `revert`");
        assert_rejected(RegistryCellMutation::AddUnknown, "unknown field");

        let mut valid = sccp_registry_for_snapshot_test();
        let lane = valid.lanes.remove(0);
        assert_rejected(
            RegistryCellMutation::Replace {
                role: "blocks",
                registry: crate::state::SccpOnChainRegistryV1 {
                    version: 1,
                    lanes: vec![lane.clone(), lane],
                },
            },
            "duplicate",
        );

        let bsc_route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
            iroha_data_model::bridge::SccpNetworkV1::BscTestnet,
            iroha_data_model::bridge::SccpRouteActivationV1::Staged,
        );
        let mut reversed_lanes = vec![
            sccp_registry_for_snapshot_test().lanes.remove(0),
            iroha_data_model::bridge::SccpGovernedLaneV1 {
                lane_id: bsc_route.lane_id,
                native_trust_anchors: Vec::new(),
                current_native_trust_anchor_hash: None,
                routes: vec![bsc_route],
            },
        ];
        reversed_lanes.sort_by_key(|lane| lane.lane_id);
        reversed_lanes.reverse();
        assert_rejected(
            RegistryCellMutation::Replace {
                role: "blocks",
                registry: crate::state::SccpOnChainRegistryV1 {
                    version: 1,
                    lanes: reversed_lanes,
                },
            },
            "canonical lane/route order",
        );

        let mut off_curve = sccp_registry_for_snapshot_test();
        let deployment = match &mut off_curve.lanes[0].routes[0].destination {
            iroha_data_model::bridge::SccpDestinationDeploymentV1::Evm(deployment) => deployment,
            iroha_data_model::bridge::SccpDestinationDeploymentV1::Tron(_) => {
                unreachable!("snapshot fixture is an EVM route")
            }
            iroha_data_model::bridge::SccpDestinationDeploymentV1::Solana(_) => {
                unreachable!("snapshot fixture is an EVM route")
            }
        };
        // (1, 1) is a canonical BN254 field encoding but is not on
        // y^2 = x^3 + 3.  Recompute the embedded key commitment so only the
        // cryptographic curve check—not a stale hash—can reject this fixture.
        let mut one = [0_u8; 32];
        one[31] = 1;
        deployment.verifying_key.alpha1.x = one;
        deployment.verifying_key.alpha1.y = one;
        deployment.verifier_key_hash =
            iroha_data_model::bridge::sccp_groth16_bn254_verifying_key_hash_v1(
                deployment.verifying_key,
            )
            .expect("off-curve point remains structurally canonical");
        let route = &mut off_curve.lanes[0].routes[0];
        let route_configuration_hash = route
            .destination
            .route_configuration_hash(
                route.lane_id,
                &route.route_id,
                &route.asset_key,
                route.revision,
                route.settlement.payload_amount_scale,
            )
            .expect("off-curve point remains structurally valid route input");
        match &mut route.source_identity.emitter {
            iroha_data_model::bridge::SccpSourceEmitterV1::Evm(emitter) => {
                emitter.route_config_hash = route_configuration_hash;
            }
            iroha_data_model::bridge::SccpSourceEmitterV1::Tron(_) => {
                unreachable!("snapshot fixture is an EVM route")
            }
            iroha_data_model::bridge::SccpSourceEmitterV1::Solana(_) => {
                unreachable!("snapshot fixture is an EVM route")
            }
        }
        off_curve
            .validate()
            .expect("structural registry validation must not stand in for curve validation");
        assert_rejected(
            RegistryCellMutation::Replace {
                role: "blocks",
                registry: off_curve,
            },
            "non-curve",
        );
    }

    #[test]
    async fn signed_hostile_sccp_revert_stores_are_rejected_without_mutation() {
        #[derive(Clone, Copy, Debug)]
        enum RevertMutation {
            PendingUsage,
            PendingMessages,
            MessageLocator,
            OrderedIndex,
            TerminalProofs,
            InboundMessages,
            InboundHighWater,
        }

        fn envelope_mut<'a>(world: &'a mut json::Map, field: &str) -> &'a mut json::Map {
            let Some(json::Value::Object(envelope)) = world.get_mut(field) else {
                panic!("{field} must be one MV envelope");
            };
            envelope
        }

        fn storage_blocks<K, V>(entries: impl IntoIterator<Item = (K, V)>) -> json::Value
        where
            K: mv::Key + mv::json::JsonKeyCodec,
            V: mv::Value + json::JsonSerialize,
        {
            let storage: Storage<K, V> = entries.into_iter().collect();
            let json::Value::Object(mut envelope) =
                json::to_value(&storage).expect("typed hostile storage encodes")
            else {
                panic!("typed hostile storage must encode as an envelope");
            };
            envelope
                .remove("blocks")
                .expect("storage envelope contains blocks")
        }

        for mutation in [
            RevertMutation::PendingUsage,
            RevertMutation::PendingMessages,
            RevertMutation::MessageLocator,
            RevertMutation::OrderedIndex,
            RevertMutation::TerminalProofs,
            RevertMutation::InboundMessages,
            RevertMutation::InboundHighWater,
        ] {
            let tmp_root = tempdir().expect("temporary snapshot root");
            let store_dir = tmp_root.path().join("snapshot");
            let kura = Kura::blank_kura_for_testing();
            let (state, key, pending_record) =
                state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
            let mut serialized = String::new();
            serialize_state_snapshot(&state, &mut serialized, true);
            let mut snapshot: json::Value =
                json::from_str(&serialized).expect("valid baseline snapshot JSON");
            let json::Value::Object(snapshot_object) = &mut snapshot else {
                panic!("snapshot root must be an object");
            };
            let Some(json::Value::Object(world)) = snapshot_object.get_mut("world") else {
                panic!("snapshot world must be an object");
            };

            match mutation {
                RevertMutation::PendingUsage => {
                    let current = envelope_mut(world, "sccp_outbound_pending_usage")
                        .get("blocks")
                        .cloned()
                        .expect("usage envelope contains blocks");
                    envelope_mut(world, "sccp_outbound_pending_usage")
                        .insert("revert".to_owned(), current);
                }
                RevertMutation::PendingMessages => {
                    envelope_mut(world, "sccp_outbound_pending_messages")
                        .insert("revert".to_owned(), json::Value::Object(json::Map::new()));
                }
                RevertMutation::MessageLocator => {
                    envelope_mut(world, "sccp_outbound_message_locator")
                        .insert("revert".to_owned(), json::Value::Object(json::Map::new()));
                }
                RevertMutation::OrderedIndex => {
                    envelope_mut(world, "sccp_outbound_message_index")
                        .insert("revert".to_owned(), json::Value::Object(json::Map::new()));
                }
                RevertMutation::TerminalProofs => {
                    let terminal = iroha_data_model::bridge::SccpOutboundProofRecordV1 {
                        payload_hash: pending_record.payload_hash,
                        destination_binding_hash: pending_record.destination_binding_hash,
                        route_configuration_hash: pending_record.route_configuration_hash,
                        finality_block_hash: [0xA1; 32],
                        destination_proof_commitment: [0xA2; 32],
                        finality_height: pending_record.recorded_at_height,
                        commitment_index: pending_record.commitment_index,
                        accepted_at_height: pending_record.recorded_at_height,
                    };
                    assert!(terminal.is_well_formed_for_key(&key));
                    envelope_mut(world, "sccp_outbound_proofs")
                        .insert("revert".to_owned(), storage_blocks([(key, terminal)]));
                }
                RevertMutation::InboundMessages | RevertMutation::InboundHighWater => {
                    let (native, source_identity, trust_anchor) =
                        iroha_sccp::sccp_native_ethereum_transfer_inbound_test_fixture_v1();
                    let validated = iroha_sccp::verify_sccp_native_inbound_message_proof_v1(
                        &native,
                        &source_identity,
                        trust_anchor,
                    )
                    .expect("native hostile-revert fixture verifies");
                    let route = iroha_sccp::sccp_exact_evm_governed_route_test_fixture_v1(
                        iroha_data_model::bridge::SccpNetworkV1::EthereumMainnet,
                        iroha_data_model::bridge::SccpRouteActivationV1::Bidirectional,
                    );
                    let inbound_record = iroha_data_model::bridge::SccpInboundMessageRecordV1 {
                        payload_hash: validated.payload_hash,
                        source_identity_hash: validated.source_identity_hash,
                        route_configuration_hash: route
                            .route_configuration_hash()
                            .expect("fixture route configuration"),
                        trust_anchor: validated.trust_anchor,
                        anchor_interval_height: validated.anchor_interval_height,
                        source_finality_height: validated.source_finality.height,
                        source_finality_hash: validated.source_finality.block_hash,
                        source_proof_commitment: [0xA3; 32],
                        admitted_at_height: 1,
                    };
                    assert!(inbound_record.is_well_formed_for_lane(validated.message_key.lane));
                    if matches!(mutation, RevertMutation::InboundMessages) {
                        envelope_mut(world, "sccp_inbound_messages").insert(
                            "revert".to_owned(),
                            storage_blocks([(validated.message_key, inbound_record)]),
                        );
                    } else {
                        let high_water_key =
                            iroha_data_model::bridge::SccpInboundAnchorHighWaterKeyV1::new(
                                validated.message_key.lane,
                                validated.trust_anchor.anchor_hash,
                            )
                            .expect("validated native fixture forms high-water key");
                        envelope_mut(world, "sccp_inbound_anchor_high_water").insert(
                            "revert".to_owned(),
                            storage_blocks([(high_water_key, validated.anchor_interval_height)]),
                        );
                    }
                }
            }

            serialized = json::to_json(&snapshot).expect("mutated snapshot JSON encodes");
            let key_pair = checked_random_snapshot_keypair();
            write_snapshot_bundle_from_bytes(&store_dir, serialized.as_bytes(), &key_pair);
            let pointer_before =
                std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).expect("read pointer");
            let canonical_hash = kura
                .block_hash_at_height(nonzero!(1_usize))
                .expect("canonical Kura hash");
            let body_before = kura
                .get_block(nonzero!(1_usize))
                .expect("canonical Kura body");
            let retained_before = kura
                .v2_finality_artifact_with_archive(1)
                .expect("read exact retained SCCP material")
                .expect("exact retained SCCP material exists");

            let error = match try_read_snapshot(
                &store_dir,
                &kura,
                LiveQueryStore::start_test,
                BlockCount(1),
                TEST_CHUNK_SIZE,
                key_pair.public_key(),
                &state.chain_id,
                &state.zk_snapshot(),
                #[cfg(feature = "telemetry")]
                StateTelemetry::new(<_>::default(), true),
            ) {
                Ok(_) => panic!("hostile {mutation:?} revert must fail closed"),
                Err(error) => error,
            };
            assert!(
                matches!(error, TryReadError::InvalidSccpRevert(_)),
                "unexpected {mutation:?} rejection: {error:?}"
            );
            assert_eq!(kura.blocks_count(), 1, "{mutation:?} rejection pruned Kura");
            assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
            assert_eq!(
                kura.block_hash_at_height(nonzero!(1_usize)),
                Some(canonical_hash)
            );
            assert_eq!(
                kura.get_block(nonzero!(1_usize)),
                Some(body_before),
                "{mutation:?} rejection changed the canonical block body"
            );
            assert_eq!(
                kura.v2_finality_artifact_with_archive(1)
                    .expect("read retained material after rejection")
                    .expect("retained material still exists"),
                retained_before,
                "{mutation:?} rejection changed retained SCCP evidence"
            );
            assert_eq!(
                std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME))
                    .expect("read pointer after rejection"),
                pointer_before,
                "{mutation:?} rejection changed the selected immutable generation"
            );
        }
    }

    #[test]
    async fn snapshot_roundtrip_preserves_sccp_outbound_pending_messages() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let (state, key, record) =
            state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let snapshot_bytes =
            std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot bytes");
        let snapshot_value: json::Value =
            json::from_slice(&snapshot_bytes).expect("snapshot JSON should parse");
        assert!(
            snapshot_world_has_field(&snapshot_value, "sccp_outbound_pending_messages"),
            "new snapshots must carry the SCCP outbound replay registry"
        );
        assert!(
            snapshot_world_has_field(&snapshot_value, "sccp_outbound_pending_usage"),
            "new snapshots must carry exact SCCP pending usage"
        );

        let snapshot_state = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("snapshot read");

        let restored = snapshot_state
            .view()
            .world
            .sccp_outbound_pending_messages
            .get(&key)
            .cloned()
            .expect("SCCP outbound replay key should survive snapshot roundtrip");
        assert_eq!(restored, record);
        assert_eq!(
            snapshot_state
                .view()
                .world
                .sccp_outbound_pending_usage
                .get()
                .message_count,
            1
        );
    }

    #[test]
    async fn incompatible_sccp_caps_reject_before_snapshot_can_prune_kura() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let (state, _, record) = state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();
        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("write exact SCCP snapshot");

        // Keep every SCCP record/archive association exact so the configured-cap
        // rejection is the first failing boundary, ahead of hash reconciliation.
        let snapshot_bytes =
            std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot bytes");
        let mut snapshot_value: json::Value =
            json::from_slice(&snapshot_bytes).expect("snapshot JSON parses");
        let json::Value::Object(root) = &mut snapshot_value else {
            panic!("snapshot root is an object");
        };
        let Some(json::Value::Array(block_hashes)) = root.get_mut("block_hashes") else {
            panic!("snapshot block hashes are an array");
        };
        assert_eq!(block_hashes.len(), 1);
        let forged_hash =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0xA5; 32]));
        assert_ne!(
            forged_hash,
            state.latest_block_hash_fast().expect("fixture block hash")
        );
        block_hashes[0] = json::to_value(&forged_hash).expect("encode forged block hash");
        let Some(json::Value::Object(runtime)) = root.get_mut("nexus_runtime") else {
            panic!("snapshot Nexus runtime is an object");
        };
        let Some(json::Value::Array(history)) = runtime.get_mut("autoscale_sample_history") else {
            panic!("snapshot autoscale sample history is an array");
        };
        let Some(json::Value::Object(latest_sample)) = history.last_mut() else {
            panic!("snapshot autoscale sample history retains the latest block");
        };
        latest_sample.insert(
            "block_hash".to_owned(),
            json::to_value(&forged_hash).expect("encode forged autoscale sample hash"),
        );
        let mut forged_snapshot_bytes = Vec::new();
        json::to_writer(&mut forged_snapshot_bytes, &snapshot_value)
            .expect("encode forged snapshot");
        write_snapshot_bundle_from_bytes(&store_dir, &forged_snapshot_bytes, &key_pair);

        let canonical_hash = kura
            .block_hash_at_height(nonzero!(1_usize))
            .expect("canonical Kura hash");
        let body_before = kura
            .get_block(nonzero!(1_usize))
            .expect("canonical Kura body");
        let retained_before = kura
            .v2_finality_artifact_with_archive(1)
            .expect("read exact retained SCCP material")
            .expect("exact retained SCCP material exists");
        let mut incompatible = state.zk_snapshot();
        let payload_bytes = u64::try_from(record.payload_bytes.len()).expect("small payload");
        incompatible.sccp.max_pending_outbound_payload_bytes =
            NonZeroU64::new(payload_bytes - 1).expect("fixture payload exceeds one byte");

        let error = match try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &incompatible,
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("incompatible actual SCCP cap must fail before reconciliation"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TryReadError::ZkConfigInstall(
                ZkConfigInstallError::SccpPendingUsageLimitExceeded { .. }
            )
        ));

        assert_eq!(kura.blocks_count(), 1, "rejected snapshot pruned Kura");
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            kura.block_hash_at_height(nonzero!(1_usize)),
            Some(canonical_hash)
        );
        assert_eq!(
            kura.get_block(nonzero!(1_usize)),
            Some(body_before),
            "rejected snapshot changed the canonical block body"
        );
        assert_eq!(
            kura.v2_finality_artifact_with_archive(1)
                .expect("read retained SCCP material after rejection")
                .expect("retained SCCP material still exists"),
            retained_before,
            "rejected snapshot changed retained header, finality, or archive material"
        );
    }

    #[test]
    async fn sccp_snapshot_revert_enforces_actual_pending_cap_after_terminal_compaction() {
        let kura = Kura::blank_kura_for_testing();
        let (mut state, key, pending) =
            state_with_exact_pending_sccp_snapshot_fixture(Arc::clone(&kura));
        let finality_block_hash = kura
            .block_hash_at_height(nonzero!(1_usize))
            .expect("fixture Kura hash");
        let terminal = iroha_data_model::bridge::SccpOutboundProofRecordV1 {
            payload_hash: pending.payload_hash,
            destination_binding_hash: pending.destination_binding_hash,
            route_configuration_hash: pending.route_configuration_hash,
            finality_block_hash: <[u8; 32]>::from(Hash::from(finality_block_hash)),
            destination_proof_commitment: [0xB7; 32],
            finality_height: pending.recorded_at_height,
            commitment_index: pending.commitment_index,
            accepted_at_height: 2,
        };
        state
            .transition_sccp_outbound_message_to_terminal_for_testing(key, terminal)
            .expect("compact the current payload-bearing record to a terminal descriptor");
        state.push_block_hash_for_testing(HashOf::<BlockHeader>::from_untyped_unchecked(
            Hash::prehashed([0xB8; 32]),
        ));

        let payload_bytes = u64::try_from(pending.payload_bytes.len()).expect("small payload");
        let mut lowered = state.zk_snapshot();
        lowered.sccp.max_pending_outbound_messages = NonZeroU64::new(1).expect("one is nonzero");
        lowered.sccp.max_pending_outbound_payload_bytes =
            NonZeroU64::new(payload_bytes - 1).expect("fixture payload exceeds one byte");
        state
            .set_zk(lowered)
            .expect("the compacted current state fits the lowered runtime cap");

        let error = crate::state::validate_sccp_snapshot_revert_candidate(&state)
            .expect_err("rollback must not expose pending state above the actual runtime cap");
        assert!(
            error.contains("exceeds configured limits"),
            "unexpected rollback-cap rejection: {error}"
        );
        let view = state.view();
        assert!(
            view.world
                .sccp_outbound_pending_messages
                .get(&key)
                .is_none(),
            "validation must not roll the current WSV back"
        );
        assert!(
            view.world.sccp_outbound_proofs.get(&key).is_some(),
            "validation must preserve the current terminal descriptor"
        );
    }

    #[test]
    async fn snapshot_write_signature_file_uses_checked_signing_and_verifies_digest() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");

        let digest_hex = std::fs::read_to_string(current_generation_artifact(
            &store_dir,
            SNAPSHOT_DIGEST_FILE_NAME,
        ))
        .expect("snapshot digest");
        let digest = hex::decode(digest_hex.trim()).expect("snapshot digest hex");
        let signature_hex = std::fs::read_to_string(current_generation_artifact(
            &store_dir,
            SNAPSHOT_SIGNATURE_FILE_NAME,
        ))
        .expect("snapshot signature");
        let signature =
            Signature::try_from_hex(signature_hex.trim()).expect("snapshot signature hex");
        signature
            .verify(key_pair.public_key(), &digest)
            .expect("checked snapshot signature must verify");
    }

    #[test]
    async fn snapshot_read_rejects_wrong_key_signature_for_matching_digest() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");

        let digest_hex = std::fs::read_to_string(current_generation_artifact(
            &store_dir,
            SNAPSHOT_DIGEST_FILE_NAME,
        ))
        .expect("snapshot digest");
        let digest = hex::decode(digest_hex.trim()).expect("snapshot digest hex");
        let wrong_key_pair = checked_random_snapshot_keypair();
        let wrong_signature = Signature::try_new(wrong_key_pair.private_key(), &digest)
            .expect("checked wrong-key snapshot signature");
        std::fs::write(
            current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
            hex::encode(wrong_signature.payload()),
        )
        .expect("replace snapshot signature");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("snapshot with wrong-key signature should be rejected")
        };

        assert!(matches!(error, TryReadError::SignatureInvalid(_)));
    }

    #[test]
    async fn snapshot_read_rejects_noncanonical_uppercase_signature_hex() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
        let signature_path = current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME);
        let signature_hex = std::fs::read_to_string(&signature_path).expect("signature hex");
        std::fs::write(&signature_path, signature_hex.to_ascii_uppercase())
            .expect("replace signature with equivalent noncanonical hex");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("uppercase signature hex must not be accepted");
        };

        assert!(matches!(error, TryReadError::SignatureMalformed(_)));
    }

    #[test]
    async fn snapshot_read_rejects_all_zero_signature_sidecar_before_verification() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
        std::fs::write(
            current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
            "00".repeat(64),
        )
        .expect("replace snapshot signature");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("snapshot with all-zero signature should be rejected")
        };

        assert!(matches!(error, TryReadError::SignatureMalformed(_)));
    }

    #[test]
    async fn snapshot_read_rejects_malformed_ed25519_signature_r_before_verification() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
        let signature_hex = std::fs::read_to_string(current_generation_artifact(
            &store_dir,
            SNAPSHOT_SIGNATURE_FILE_NAME,
        ))
        .expect("snapshot signature");
        let valid_signature_bytes = hex::decode(signature_hex.trim()).expect("signature hex");

        for (label, replacement_r) in [
            ("small-order", SMALL_ORDER_ED25519_R),
            ("noncanonical", NONCANONICAL_ED25519_R),
        ] {
            let mut signature_bytes = valid_signature_bytes.clone();
            signature_bytes[..replacement_r.len()].copy_from_slice(&replacement_r);
            std::fs::write(
                current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
                hex::encode(signature_bytes),
            )
            .expect("replace snapshot signature");

            let Err(error) = try_read_snapshot(
                &store_dir,
                &Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test,
                BlockCount(state.view().height()),
                TEST_CHUNK_SIZE,
                key_pair.public_key(),
                &state.chain_id,
                &crate::state::default_zk_config(),
                #[cfg(feature = "telemetry")]
                StateTelemetry::default(),
            ) else {
                panic!("snapshot with malformed Ed25519 signature R should be rejected")
            };

            assert!(
                matches!(error, TryReadError::SignatureMalformed(_)),
                "{label} snapshot signature R produced unexpected error: {error:?}"
            );
        }
    }

    #[test]
    async fn snapshot_read_rejects_malformed_mldsa_signature_lengths_before_verification() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair =
            KeyPair::try_from_seed(b"snapshot-mldsa-signature".to_vec(), Algorithm::MlDsa)
                .expect("snapshot ML-DSA fixture key generation should succeed");

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).expect("snapshot write");
        let signature_hex = std::fs::read_to_string(current_generation_artifact(
            &store_dir,
            SNAPSHOT_SIGNATURE_FILE_NAME,
        ))
        .expect("snapshot signature");
        let valid_signature_bytes = hex::decode(signature_hex.trim()).expect("signature hex");

        for label in ["short", "overlong"] {
            let mut signature_bytes = valid_signature_bytes.clone();
            match label {
                "short" => {
                    signature_bytes
                        .pop()
                        .expect("ML-DSA snapshot signature is non-empty");
                }
                "overlong" => signature_bytes.push(0xA5),
                _ => unreachable!("covered labels"),
            }
            std::fs::write(
                current_generation_artifact(&store_dir, SNAPSHOT_SIGNATURE_FILE_NAME),
                hex::encode(signature_bytes),
            )
            .expect("replace snapshot signature");

            let Err(error) = try_read_snapshot(
                &store_dir,
                &Kura::blank_kura_for_testing(),
                LiveQueryStore::start_test,
                BlockCount(state.view().height()),
                TEST_CHUNK_SIZE,
                key_pair.public_key(),
                &state.chain_id,
                &crate::state::default_zk_config(),
                #[cfg(feature = "telemetry")]
                StateTelemetry::default(),
            ) else {
                panic!("snapshot with malformed ML-DSA signature length should be rejected")
            };

            assert!(
                matches!(error, TryReadError::SignatureMalformed(_)),
                "{label} snapshot ML-DSA signature length produced unexpected error: {error:?}"
            );
        }
    }

    #[test]
    async fn snapshot_roundtrip_preserves_space_directory_manifests_and_rebuilds_bindings() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let mut state = state_factory();
        let (uaid, dataspace, account_id) = install_active_space_directory_manifest(&mut state);
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let snapshot_bytes =
            std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot bytes");
        let snapshot_value: json::Value =
            json::from_slice(&snapshot_bytes).expect("snapshot JSON should parse");
        assert!(
            snapshot_has_space_directory_manifest_section(&snapshot_value),
            "new snapshots must carry a Space Directory manifest section"
        );
        assert!(
            snapshot_world_has_field(&snapshot_value, "kagemusha_replay_keys"),
            "new snapshots must carry Kagemusha replay keys"
        );

        let snapshot_state = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("snapshot read");

        let manifests = snapshot_state.world.space_directory_manifests.view();
        let manifest_set = manifests
            .get(&uaid)
            .expect("manifest set should survive snapshot restore");
        assert!(
            manifest_set.get(&dataspace).is_some(),
            "dataspace manifest should survive snapshot restore"
        );
        drop(manifests);

        let bindings = snapshot_state.world.uaid_dataspaces.view();
        let uaid_bindings = bindings
            .get(&uaid)
            .expect("UAID bindings should be rebuilt after snapshot restore");
        assert!(
            uaid_bindings.is_bound_to(dataspace, &account_id),
            "restored active manifest should bind the account to the dataspace"
        );
    }

    #[test]
    async fn snapshot_missing_space_directory_section_rejects_even_with_kura_history() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let manifest = sample_space_directory_manifest();
        let _account_id = insert_account_with_uaid(&mut state, manifest.uaid);
        let block = signed_block_with_transaction(accepted_manifest_transaction());
        store_block_and_mark_state_height(&mut state, &kura, block);
        let key_pair = checked_random_snapshot_keypair();
        let legacy_bytes = legacy_snapshot_bytes_without_space_directory_section(&state);

        write_snapshot_bundle_from_bytes(&store_dir, &legacy_bytes, &key_pair);

        let error = match try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("missing canonical manifest section must not be reconstructed"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TryReadError::MissingSpaceDirectoryManifestSection { snapshot_height: 1 }
        ));
    }

    #[test]
    async fn snapshot_missing_space_directory_section_rejects_without_manifest_history() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block = signed_block_with_transaction(accepted_log_transaction("legacy"));
        store_block_and_mark_state_height(&mut state, &kura, block);
        let key_pair = checked_random_snapshot_keypair();
        let legacy_bytes = legacy_snapshot_bytes_without_space_directory_section(&state);

        write_snapshot_bundle_from_bytes(&store_dir, &legacy_bytes, &key_pair);

        let error = match try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("non-empty snapshot must carry its canonical manifest section"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TryReadError::MissingSpaceDirectoryManifestSection { snapshot_height: 1 }
        ));
    }

    #[test]
    async fn ordinary_snapshot_hash_reconcile_rejects_ahead_suffix_without_mutation() {
        let tmp_root = tempdir().unwrap();
        let kura_store_dir = tmp_root.path().join("kura");
        let lane_config = LaneConfig::default();
        let kura_config = kura_config_for_snapshot_test(&kura_store_dir, nonzero!(1_usize));
        let (kura, _) = Kura::new(&kura_config, &lane_config).expect("kura init");
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block = signed_block_with_transaction(accepted_log_transaction("canonical"));
        let canonical_hash = block.hash();
        store_block_and_mark_state_height(&mut state, &kura, block);
        let extra_hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x22; 32]));

        let hashes = vec![canonical_hash, extra_hash];
        let error = reconcile_snapshot_hash_height_with_kura(&hashes, 1, &kura, false, None)
            .expect_err("ordinary signed snapshots cannot invent a hash-only suffix");
        assert!(matches!(error, TryReadError::MismatchedHeight { .. }));

        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(kura.block_hash_at_height(nonzero!(2_usize)), None);
        assert!(
            kura.get_block(nonzero!(2_usize)).is_none(),
            "rejected snapshot must not invent a block body"
        );
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);

        drop(state);
        drop(kura);
        let (reopened, BlockCount(reopened_count)) =
            Kura::new(&kura_config, &lane_config).expect("reopen kura");
        assert_eq!(
            reopened_count, 1,
            "cold restart must not discover a rejected hash-only suffix"
        );
        assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 1);
        assert!(
            reopened.get_block(nonzero!(1_usize)).is_some(),
            "rejected recovery must preserve retained block bodies"
        );
        assert!(
            reopened.get_block(nonzero!(2_usize)).is_none(),
            "rejected suffix must remain absent after restart"
        );
    }

    #[test]
    async fn snapshot_hash_reconcile_rejects_forged_prefix_before_extending_suffix() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block = signed_block_with_transaction(accepted_log_transaction("canonical"));
        store_block_and_mark_state_height(&mut state, &kura, block);
        let canonical_hash = kura
            .block_hash_at_height(nonzero!(1_usize))
            .expect("canonical Kura prefix hash");
        let forged_prefix =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x91; 32]));
        let attacker_suffix =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x92; 32]));

        let error = reconcile_snapshot_hash_height_with_kura(
            &[forged_prefix, attacker_suffix],
            1,
            &kura,
            false,
            None,
        )
        .expect_err("a divergent retained prefix must reject before suffix extension");

        assert!(matches!(
            error,
            TryReadError::MismatchedHash { height: 1, .. }
        ));
        assert_eq!(kura.blocks_count(), 1);
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            kura.block_hash_at_height(nonzero!(1_usize)),
            Some(canonical_hash)
        );
        assert!(
            kura.block_hash_at_height(nonzero!(2_usize)).is_none(),
            "rejected snapshot must not persist its attacker-controlled suffix"
        );
    }

    #[test]
    async fn ordinary_signed_snapshot_rejects_kura_tail_loss_without_mutation() {
        let tmp_root = tempdir().unwrap();
        let snapshot_store_dir = tmp_root.path().join("snapshot");
        let source_kura_store_dir = tmp_root.path().join("source-kura");
        let tail_loss_kura_store_dir = tmp_root.path().join("tail-loss-kura");
        let lane_config = LaneConfig::default();
        let source_kura_config =
            kura_config_for_snapshot_test(&source_kura_store_dir, nonzero!(1_usize));
        let tail_loss_kura_config =
            kura_config_for_snapshot_test(&tail_loss_kura_store_dir, nonzero!(1_usize));
        let (kura, _) = Kura::new(&source_kura_config, &lane_config).expect("source Kura init");
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            &[Arc::clone(&block1), block2],
        );

        try_write_snapshot(&state, &snapshot_store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("snapshot write");
        let pointer_before =
            std::fs::read(snapshot_store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();

        let (tail_loss_kura, BlockCount(initial_height)) =
            Kura::new(&tail_loss_kura_config, &lane_config).expect("tail-loss Kura init");
        assert_eq!(initial_height, 0);
        tail_loss_kura
            .store_block(Arc::clone(&block1))
            .expect("persist retained prefix block");
        let prefix_hash = block1.hash();

        let error = match try_read_snapshot(
            &snapshot_store_dir,
            &tail_loss_kura,
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        ) {
            Ok(_) => panic!("ordinary signed snapshot must not repair a lost Kura suffix"),
            Err(error) => error,
        };
        assert!(matches!(
            error,
            TryReadError::MismatchedHeight {
                snapshot_height: 2,
                kura_height: 1
            }
        ));
        assert_eq!(tail_loss_kura.blocks_count(), 1);
        assert_eq!(tail_loss_kura.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            tail_loss_kura.block_hash_at_height(nonzero!(1_usize)),
            Some(prefix_hash)
        );
        assert_eq!(tail_loss_kura.block_hash_at_height(nonzero!(2_usize)), None);
        assert_eq!(
            std::fs::read(snapshot_store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before,
            "rejected recovery must not replace the selected snapshot generation"
        );

        drop(tail_loss_kura);
        let (reopened, BlockCount(reopened_count)) =
            Kura::new(&tail_loss_kura_config, &lane_config).expect("cold reopen tail-loss Kura");
        assert_eq!(reopened_count, 1);
        assert_eq!(reopened.exact_durable_blocks_count().unwrap(), 1);
        assert_eq!(
            reopened.block_hash_at_height(nonzero!(1_usize)),
            Some(prefix_hash)
        );
        assert_eq!(reopened.block_hash_at_height(nonzero!(2_usize)), None);
    }

    #[test]
    async fn snapshot_read_validates_hashes_without_historical_block_body() {
        let tmp_root = tempdir().unwrap();
        let snapshot_store_dir = tmp_root.path().join("snapshot");
        let kura_store_dir = tmp_root.path().join("kura");
        let lane_config = LaneConfig::default();
        let kura_config = kura_config_for_snapshot_test(&kura_store_dir, nonzero!(1_usize));
        let (kura, _) = Kura::new(&kura_config, &lane_config).expect("kura init");
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let block3 = signed_block_after_transaction(
            accepted_log_transaction("third"),
            Some(block2.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block3));
        let expected_snapshot = canonical_state_snapshot_bytes_for_tests(&state);
        let expected_chain_id = state.chain_id.clone();
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            &[
                Arc::clone(&block1),
                Arc::clone(&block2),
                Arc::clone(&block3),
            ],
        );

        try_write_snapshot(&state, &snapshot_store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("snapshot write");
        drop(state);
        drop(kura);

        let (kura, block_count) = Kura::new(&kura_config, &lane_config).expect("kura reopen");
        let historical_height = nonzero!(2_usize);
        let payload_len = kura
            .advertise_required_replicas_for_bench(historical_height)
            .expect("historical payload length");
        let freed = kura
            .evict_block_bodies_for_bench(payload_len)
            .expect("evict historical block body");
        assert!(freed >= payload_len);
        let historical_sidecar_path = lane_config
            .primary()
            .blocks_dir(&kura_store_dir)
            .join("da_blocks")
            .join(format!("{:020}.norito", historical_height.get()));
        assert!(
            historical_sidecar_path.is_file(),
            "expected evicted block sidecar at {}",
            historical_sidecar_path.display()
        );
        std::fs::remove_file(&historical_sidecar_path).expect("remove historical sidecar");
        assert!(
            kura.block_hash_at_height(historical_height).is_some(),
            "hash journal must still contain the historical block"
        );
        assert!(
            kura.get_block(historical_height).is_none(),
            "test fixture must make the historical block body unavailable"
        );

        let snapshot_state = try_read_snapshot(
            &snapshot_store_dir,
            &kura,
            LiveQueryStore::start_test,
            block_count,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &expected_chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("snapshot read should validate historical hashes without block bodies");

        assert_eq!(
            canonical_state_snapshot_bytes_for_tests(&snapshot_state),
            expected_snapshot
        );
    }

    #[test]
    async fn snapshot_hash_reconcile_rejects_non_latest_mismatch() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let block3 = signed_block_after_transaction(
            accepted_log_transaction("third"),
            Some(block2.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block3));

        let mut snapshot_hashes = state.committed_block_hashes_snapshot();
        snapshot_hashes[1] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x44; 32]));

        let err = reconcile_snapshot_hashes_with_kura(&snapshot_hashes, &kura)
            .expect_err("non-latest hash mismatch must reject snapshot");
        assert!(matches!(
            err,
            TryReadError::MismatchedHash { height: 2, .. }
        ));
        assert_eq!(state.committed_height(), 3);
    }

    #[test]
    async fn snapshot_hash_reconcile_rejects_latest_mismatch_without_mutation() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));

        let mut snapshot_hashes = state.committed_block_hashes_snapshot();
        snapshot_hashes[1] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x55; 32]));

        let state_height_before = state.committed_height();
        let state_hash_before = state.latest_block_hash_fast();
        let kura_height_before = kura.exact_durable_blocks_count().unwrap();
        let error = reconcile_snapshot_hashes_with_kura(&snapshot_hashes, &kura)
            .expect_err("latest hash mismatch must reject instead of trusting snapshot undo state");
        assert!(matches!(
            error,
            TryReadError::MismatchedHash { height: 2, .. }
        ));

        assert_eq!(state.committed_height(), state_height_before);
        assert_eq!(
            state.latest_block_hash_fast(),
            state_hash_before,
            "latest mismatch rejection must leave the snapshot WSV untouched"
        );
        assert_eq!(
            kura.exact_durable_blocks_count().unwrap(),
            kura_height_before,
            "latest mismatch rejection must not prune Kura"
        );
        assert_eq!(state.latest_block_hash_fast(), Some(block2.hash()));
    }

    #[test]
    async fn audited_snapshot_hash_reconcile_rejects_every_divergent_existing_hash() {
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let block3 = signed_block_after_transaction(
            accepted_log_transaction("third"),
            Some(block2.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block3));

        let mut snapshot_hashes = state.committed_block_hashes_snapshot();
        snapshot_hashes[1] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x66; 32]));
        snapshot_hashes[2] =
            HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x77; 32]));

        let err = reconcile_snapshot_hashes_with_kura(&snapshot_hashes, &kura)
            .expect_err("audited bootstrap cannot replace any existing Kura hash");
        assert!(matches!(
            err,
            TryReadError::MismatchedHash { height: 2, .. }
        ));
        assert_eq!(state.committed_height(), 3);
    }

    #[test]
    async fn snapshot_read_succeeds_without_selector_bootstrap() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let expected_chain_id = state.chain_id.clone();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let snapshot_state = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .expect("snapshot read");
        assert_eq!(snapshot_state.chain_id, expected_chain_id);
    }

    #[test]
    async fn snapshot_generation_shape_is_exact_and_idempotent() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        assert_canonical_snapshot_generation(&store_dir);
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        let generation_before = current_generation_name(&store_dir);

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("repeating the exact snapshot must be idempotent");

        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before
        );
        assert_eq!(current_generation_name(&store_dir), generation_before);
        assert_eq!(
            std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
                .unwrap()
                .count(),
            1,
            "idempotence must not create another immutable generation"
        );
        assert_canonical_snapshot_generation(&store_dir);
    }

    #[test]
    async fn snapshot_reader_rejects_every_noncanonical_current_pointer() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
        let canonical = std::fs::read(&pointer_path).unwrap();
        let canonical_text = std::str::from_utf8(&canonical).unwrap();
        let digest = canonical_text.trim_end_matches('\n');
        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");
        for malformed in [
            digest.as_bytes().to_vec(),
            format!("{}\n", digest.to_ascii_uppercase()).into_bytes(),
            b"../foreign\n".to_vec(),
            vec![0xff, b'\n'],
        ] {
            std::fs::write(&pointer_path, malformed).unwrap();
            let error =
                bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
                    .err()
                    .expect("noncanonical current pointer must fail closed");
            assert!(matches!(
                error,
                TryReadError::SnapshotGenerationInvalid { .. }
            ));
        }

        let oversized = format!("{digest}\n\n").into_bytes();
        assert!(
            u64::try_from(oversized.len()).unwrap() > SNAPSHOT_CURRENT_MAX_BYTES,
            "oversized fixture must exercise the pre-parse pointer bound"
        );
        std::fs::write(&pointer_path, oversized).unwrap();
        let error = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .err()
            .expect("oversized current pointer must fail before parsing");
        match error {
            TryReadError::IO(error, path) => {
                assert_eq!(error.kind(), std::io::ErrorKind::InvalidData);
                assert_eq!(path, pointer_path);
            }
            other => panic!("unexpected oversized-pointer rejection: {other:?}"),
        }
    }

    #[test]
    async fn bound_generation_rejects_pointer_and_artifact_substitution() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");
        let bound = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("bind canonical generation");
        let pointer_path = store_dir.join(SNAPSHOT_CURRENT_FILE_NAME);
        let pointer_bytes = std::fs::read(&pointer_path).unwrap();
        std::fs::remove_file(&pointer_path).unwrap();
        std::fs::write(&pointer_path, &pointer_bytes).unwrap();
        assert!(
            bound.verify_selection_unchanged().is_err(),
            "same-byte pointer substitution must invalidate the bound generation"
        );

        let rebound = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("rebind substituted pointer");
        let payload_path = current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME);
        let payload_bytes = std::fs::read(&payload_path).unwrap();
        std::fs::remove_file(&payload_path).unwrap();
        std::fs::write(&payload_path, payload_bytes).unwrap();
        assert!(
            rebound.verify_generation_unchanged().is_err(),
            "same-byte artifact substitution must invalidate the bound generation"
        );
    }

    #[test]
    async fn bound_generation_rejects_same_byte_directory_substitution() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");
        let bound = bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("bind canonical generation");
        let generation_dir = current_generation_dir(&store_dir);
        let displaced_dir = generation_dir.with_extension("displaced");
        std::fs::rename(&generation_dir, &displaced_dir).unwrap();
        std::fs::create_dir(&generation_dir).unwrap();
        for name in [
            SNAPSHOT_FILE_NAME,
            SNAPSHOT_DIGEST_FILE_NAME,
            SNAPSHOT_SIGNATURE_FILE_NAME,
            SNAPSHOT_MERKLE_FILE_NAME,
        ] {
            std::fs::copy(displaced_dir.join(name), generation_dir.join(name)).unwrap();
        }

        assert!(
            bound.verify_generation_unchanged().is_err(),
            "same-byte generation-directory substitution must invalidate every binding"
        );
    }

    #[test]
    async fn current_pointer_never_selects_a_partial_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        std::fs::create_dir_all(&generations_dir).unwrap();
        let digest = hex::encode(Sha256::digest(b"partial generation"));
        std::fs::write(
            store_dir.join(SNAPSHOT_CURRENT_FILE_NAME),
            format!("{digest}\n"),
        )
        .unwrap();
        let payload_limit =
            u64::try_from(iroha_config::parameters::defaults::snapshot::MAX_PAYLOAD_BYTES.get())
                .expect("snapshot payload limit fits u64");

        assert!(
            bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE).is_err(),
            "a pointer to a missing generation must fail closed"
        );
        let generation_dir = generations_dir.join(&digest);
        std::fs::create_dir(&generation_dir).unwrap();
        std::fs::write(
            generation_dir.join(SNAPSHOT_FILE_NAME),
            b"partial generation",
        )
        .unwrap();
        assert!(
            bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE).is_err(),
            "a pointer to a partially written generation must fail closed"
        );
    }

    #[test]
    async fn conflicting_immutable_generation_cannot_publish_current_pointer() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let payload = exact_snapshot_payload_bytes(&state);
        let digest = hex::encode(Sha256::digest(&payload));
        let conflicting_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME).join(&digest);
        std::fs::create_dir_all(&conflicting_dir).unwrap();
        let conflicting_payload = b"attacker-preplanted-generation";
        std::fs::write(
            conflicting_dir.join(SNAPSHOT_FILE_NAME),
            conflicting_payload,
        )
        .unwrap();

        let error = try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect_err("a conflicting digest-named generation must fail closed");
        assert!(matches!(error, TryWriteError::PublicationIntegrity(_)));
        assert!(!store_dir.join(SNAPSHOT_CURRENT_FILE_NAME).exists());
        assert_eq!(
            std::fs::read(conflicting_dir.join(SNAPSHOT_FILE_NAME)).unwrap(),
            conflicting_payload
        );
    }

    #[test]
    async fn snapshot_write_reuses_the_exact_immutable_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("initial snapshot write");
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE)
            .expect("snapshot idempotent publication");
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before
        );
        assert_canonical_snapshot_generation(&store_dir);
    }

    #[test]
    async fn snapshot_writer_enforces_the_reader_payload_limit_before_publication() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let payload_len = exact_snapshot_payload_bytes(&state).len();
        let exact_limit = NonZeroUsize::new(payload_len).expect("snapshot payload is non-empty");

        try_write_snapshot_with_limit(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE, exact_limit)
            .expect("payload exactly at the configured bound must publish");
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        let generations_before = std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
            .unwrap()
            .count();

        let smaller_limit = NonZeroUsize::new(payload_len - 1).expect("fixture is larger than one");
        let error = try_write_snapshot_with_limit(
            &state,
            &store_dir,
            &key_pair,
            TEST_CHUNK_SIZE,
            smaller_limit,
        )
        .expect_err("payload one byte over the configured bound must reject");
        assert!(matches!(
            error,
            TryWriteError::PayloadTooLarge { actual, maximum }
                if actual == payload_len && maximum == smaller_limit
        ));
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before,
            "oversize rejection must not replace the authoritative pointer"
        );
        assert_eq!(
            std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
                .unwrap()
                .count(),
            generations_before,
            "oversize rejection must not leave a generation or staging orphan"
        );
    }

    #[test]
    async fn snapshot_generation_gc_retains_current_and_previous_only() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let first = b"first complete generation";
        let second = b"second complete generation";
        let third = b"third complete generation";
        let first_name = hex::encode(Sha256::digest(first));
        let second_name = hex::encode(Sha256::digest(second));
        let third_name = hex::encode(Sha256::digest(third));

        write_snapshot_bundle_from_bytes(&store_dir, first, &key_pair);
        write_snapshot_bundle_from_bytes(&store_dir, second, &key_pair);
        write_snapshot_bundle_from_bytes(&store_dir, third, &key_pair);

        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        assert!(!generations_dir.join(first_name).exists());
        assert!(generations_dir.join(&second_name).is_dir());
        assert!(generations_dir.join(&third_name).is_dir());
        assert_eq!(current_generation_name(&store_dir), third_name);

        write_snapshot_bundle_from_bytes(&store_dir, third, &key_pair);
        assert!(
            generations_dir.join(second_name).is_dir(),
            "idempotent publication must preserve the prior rollback generation"
        );
        assert_eq!(
            std::fs::read_dir(generations_dir).unwrap().count(),
            2,
            "repeated writes must keep storage bounded"
        );
    }

    #[test]
    async fn idempotent_gc_fails_closed_when_rollback_chronology_is_ambiguous() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let current_bytes = b"current generation";
        write_snapshot_bundle_from_bytes(&store_dir, current_bytes, &key_pair);
        let current_name = current_generation_name(&store_dir);
        let (_, first_extra) =
            publish_test_snapshot_generation(&store_dir, b"first extra generation", &key_pair);
        let first_extra_name = first_extra.name.clone();
        let (_, second_extra) =
            publish_test_snapshot_generation(&store_dir, b"second extra generation", &key_pair);
        let second_extra_name = second_extra.name.clone();
        let (store_identity, current) =
            publish_test_snapshot_generation(&store_dir, current_bytes, &key_pair);

        let error = publish_snapshot_current_pointer(
            &store_dir,
            store_identity,
            &current,
            defaults::snapshot::MAX_PAYLOAD_BYTES,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
        )
        .expect_err("GC must not invent chronology for multiple authenticated extras");
        assert!(matches!(error, TryWriteError::PublicationIntegrity(_)));
        assert_eq!(current_generation_name(&store_dir), current_name);
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        for name in [current_name, first_extra_name, second_extra_name] {
            assert!(
                generations_dir.join(name).is_dir(),
                "ambiguous GC must preserve every authenticated generation"
            );
        }
    }

    #[test]
    async fn generation_gc_entry_limit_is_enforced_while_enumerating() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let current_bytes = b"bounded GC old current generation";
        let next_bytes = b"bounded GC new current generation";
        write_snapshot_bundle_from_bytes(&store_dir, current_bytes, &key_pair);
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        for index in 0..SNAPSHOT_GENERATION_GC_MAX_ENTRIES - 1 {
            std::fs::write(generations_dir.join(format!("unknown-{index:04}")), b"keep").unwrap();
        }
        let (store_identity, next) =
            publish_test_snapshot_generation(&store_dir, next_bytes, &key_pair);

        let error = publish_snapshot_current_pointer(
            &store_dir,
            store_identity,
            &next,
            defaults::snapshot::MAX_PAYLOAD_BYTES,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
        )
        .expect_err("MAX+1 entries must stop bounded GC");
        assert!(matches!(error, TryWriteError::PublicationIntegrity(_)));
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before
        );
        assert_eq!(
            std::fs::read_dir(generations_dir).unwrap().count(),
            SNAPSHOT_GENERATION_GC_MAX_ENTRIES + 1
        );
    }

    #[test]
    async fn post_pointer_gc_failures_report_durable_publication_success() {
        for failure_stage in [1, 2] {
            let tmp_root = tempdir().unwrap();
            let store_dir = tmp_root
                .path()
                .join(format!("snapshot-stage-{failure_stage}"));
            let key_pair = checked_random_snapshot_keypair();
            write_snapshot_bundle_from_bytes(&store_dir, b"generation one", &key_pair);
            write_snapshot_bundle_from_bytes(&store_dir, b"generation two", &key_pair);
            let (store_identity, next) =
                publish_test_snapshot_generation(&store_dir, b"generation three", &key_pair);
            let next_name = next.name.clone();
            SNAPSHOT_GC_FAILURE_STAGE.with(|stage| stage.set(failure_stage));

            publish_snapshot_current_pointer(
                &store_dir,
                store_identity,
                &next,
                defaults::snapshot::MAX_PAYLOAD_BYTES,
                TEST_CHUNK_SIZE,
                key_pair.public_key(),
            )
            .expect("a durable pointer is success even when later maintenance fails");

            assert_eq!(current_generation_name(&store_dir), next_name);
            bind_current_snapshot_generation(
                &store_dir,
                u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap(),
                TEST_CHUNK_SIZE,
            )
            .expect("post-maintenance-error current generation remains complete and readable");
        }
    }

    #[test]
    async fn post_pointer_gc_rejects_same_path_generation_substitution() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot-stage-substitution");
        let key_pair = checked_random_snapshot_keypair();
        let stale_name = hex::encode(Sha256::digest(b"generation one"));
        write_snapshot_bundle_from_bytes(&store_dir, b"generation one", &key_pair);
        write_snapshot_bundle_from_bytes(&store_dir, b"generation two", &key_pair);
        let (store_identity, next) =
            publish_test_snapshot_generation(&store_dir, b"generation three", &key_pair);
        let next_name = next.name.clone();
        SNAPSHOT_GC_FAILURE_STAGE.with(|stage| stage.set(3));

        publish_snapshot_current_pointer(
            &store_dir,
            store_identity,
            &next,
            defaults::snapshot::MAX_PAYLOAD_BYTES,
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
        )
        .expect("a durable pointer remains successful when GC rejects a substitution");

        assert_eq!(current_generation_name(&store_dir), next_name);
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);
        assert!(
            generations_dir.join(&stale_name).is_dir(),
            "the replacement at the captured path must survive"
        );
        assert!(
            generations_dir
                .join(&stale_name)
                .with_extension("gc-displaced")
                .is_dir(),
            "the injected displaced tree must remain available for diagnosis"
        );
        bind_current_snapshot_generation(
            &store_dir,
            u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap(),
            TEST_CHUNK_SIZE,
        )
        .expect("substitution rejection cannot damage the published generation");
    }

    #[test]
    async fn snapshot_generation_gc_cleans_safe_orphans_but_preserves_malicious_trees() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        write_snapshot_bundle_from_bytes(&store_dir, b"canonical generation", &key_pair);
        let generations_dir = store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME);

        let safe_orphan = generations_dir.join(".snapshot-generation-orphan");
        std::fs::create_dir(&safe_orphan).unwrap();
        std::fs::write(safe_orphan.join(SNAPSHOT_FILE_NAME), b"partial").unwrap();
        let unknown_tree = generations_dir.join("operator-owned");
        std::fs::create_dir(&unknown_tree).unwrap();
        std::fs::write(unknown_tree.join("sentinel"), b"keep").unwrap();
        let invalid_digest_name = hex::encode(Sha256::digest(b"claimed payload"));
        let invalid_digest_tree = generations_dir.join(invalid_digest_name);
        std::fs::create_dir(&invalid_digest_tree).unwrap();
        std::fs::write(invalid_digest_tree.join(SNAPSHOT_FILE_NAME), b"conflict").unwrap();

        let payload_limit = u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap();
        bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE)
            .expect("orphan and unknown trees cannot affect current selection");
        write_snapshot_bundle_from_bytes(&store_dir, b"canonical generation", &key_pair);

        assert!(
            !safe_orphan.exists(),
            "safe staging orphan should be reclaimed"
        );
        assert!(unknown_tree.join("sentinel").is_file());
        assert!(
            invalid_digest_tree.join(SNAPSHOT_FILE_NAME).is_file(),
            "invalid digest-named trees are conflicts, never GC repair targets"
        );
    }

    #[test]
    async fn concurrent_same_payload_snapshot_writers_publish_one_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = Arc::new(tmp_root.path().join("snapshot"));
        let state = Arc::new(state_factory());
        let key_pair = checked_random_snapshot_keypair();
        let barrier = Arc::new(Barrier::new(3));
        let mut writers = Vec::new();
        for _ in 0..2 {
            let store_dir = Arc::clone(&store_dir);
            let state = Arc::clone(&state);
            let key_pair = key_pair.clone();
            let barrier = Arc::clone(&barrier);
            writers.push(std::thread::spawn(move || {
                barrier.wait();
                try_write_snapshot(&state, store_dir.as_path(), &key_pair, TEST_CHUNK_SIZE)
                    .map_err(|error| error.to_string())
            }));
        }
        barrier.wait();
        for writer in writers {
            writer.join().expect("snapshot writer thread").unwrap();
        }

        assert_canonical_snapshot_generation(&store_dir);
        assert_eq!(
            std::fs::read_dir(store_dir.join(SNAPSHOT_GENERATIONS_DIR_NAME))
                .unwrap()
                .count(),
            1
        );
    }

    #[test]
    async fn pointer_switch_does_not_invalidate_a_pinned_immutable_generation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        write_snapshot_bundle_from_bytes(&store_dir, b"selected generation", &key_pair);
        let payload_limit = u64::try_from(defaults::snapshot::MAX_PAYLOAD_BYTES.get()).unwrap();
        let selected =
            bind_current_snapshot_generation(&store_dir, payload_limit, TEST_CHUNK_SIZE).unwrap();

        write_snapshot_bundle_from_bytes(&store_dir, b"new current generation", &key_pair);
        assert!(selected.verify_selection_unchanged().is_err());
        selected
            .verify_generation_unchanged()
            .expect("post-mutation validation pins the selected immutable generation only");
    }

    #[test]
    async fn cannot_find_snapshot_on_read_is_not_found() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = checked_random_snapshot_keypair();
        let chain_id = ChainId::from(TEST_CHAIN_ID);

        let Err(error) = try_read_snapshot(
            store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(15),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::NotFound));
    }

    #[test]
    async fn cannot_parse_snapshot_on_read_is_error() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        std::fs::create_dir(&store_dir).unwrap();
        let key_pair = checked_random_snapshot_keypair();
        let chain_id = ChainId::from(TEST_CHAIN_ID);
        let corrupted = [1, 4, 1, 2, 3, 4, 1, 4];
        write_snapshot_bundle_from_bytes(&store_dir, &corrupted, &key_pair);

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(15),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert_eq!(format!("{error}"), "Error (de)serializing state snapshot");
    }

    #[test]
    async fn checksum_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        // Corrupt the digest without touching the snapshot bytes.
        std::fs::write(
            current_generation_artifact(&store_dir, SNAPSHOT_DIGEST_FILE_NAME),
            "deadbeef",
        )
        .unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::SnapshotGenerationInvalid { .. }
        ));
    }

    #[test]
    async fn chain_id_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let expected_chain_id = ChainId::from("other-chain");

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &expected_chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::ChainIdMismatch { .. }));
    }

    #[test]
    async fn snapshot_write_rejects_state_ahead_of_kura() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32]));

        {
            let mut block_hashes = state.block_hashes.block();
            block_hashes.push(hash);
            block_hashes.commit_for_tests();
        }

        let Err(error) = try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE) else {
            panic!("snapshot write should reject state ahead of Kura");
        };

        assert!(matches!(
            error,
            TryWriteError::StateAheadOfKura {
                state_height: 1,
                kura_height: 0,
            }
        ));
    }

    #[test]
    async fn missing_checksum_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        std::fs::remove_file(current_generation_artifact(
            &store_dir,
            SNAPSHOT_DIGEST_FILE_NAME,
        ))
        .unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::SnapshotGenerationInvalid { .. }
        ));
    }

    #[test]
    async fn missing_merkle_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        std::fs::remove_file(current_generation_artifact(
            &store_dir,
            SNAPSHOT_MERKLE_FILE_NAME,
        ))
        .unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::SnapshotGenerationInvalid { .. }
        ));
    }

    #[test]
    async fn merkle_root_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX).expect("metadata");
        metadata.root_hex = hex::encode([0xAA; Hash::LENGTH]);
        let mut merkle_file = File::create(&merkle_path).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::MerkleMismatch { .. }));
    }

    #[test]
    async fn merkle_leaf_count_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX).expect("metadata");
        assert!(
            metadata.leaf_hashes_hex.pop().is_some(),
            "expected at least one Merkle leaf"
        );
        let mut merkle_file = File::create(&merkle_path).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::MerkleMetadataMalformed(_)));
    }

    #[test]
    async fn merkle_chunk_size_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX).expect("metadata");
        metadata.chunk_size_bytes = u64::try_from(TEST_CHUNK_SIZE.get() * 2).expect("fits in u64");
        let mut merkle_file = File::create(&merkle_path).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(
            error,
            TryReadError::MerkleChunkSizeMismatch { .. }
        ));
    }

    #[test]
    async fn merkle_metadata_rejects_numeric_string_fields() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let merkle_path = current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME);
        let mut value: norito::json::Value =
            json::from_slice(&std::fs::read(&merkle_path).expect("read merkle"))
                .expect("parse merkle json");
        let map = value.as_object_mut().expect("metadata object");
        map.insert(
            "chunk_size_bytes".to_owned(),
            norito::json::Value::String(TEST_CHUNK_SIZE.get().to_string()),
        );
        let snapshot_len =
            std::fs::metadata(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot metadata")
                .len();
        map.insert(
            "total_len_bytes".to_owned(),
            norito::json::Value::String(snapshot_len.to_string()),
        );
        let mut merkle_file = File::create(&merkle_path).expect("create merkle file");
        json::to_writer(&mut merkle_file, &value).expect("write merkle json");

        let error = SnapshotMerkleMetadata::from_path(&merkle_path, u64::MAX)
            .expect_err("numeric-string Merkle fields are not canonical first-release JSON");
        assert!(matches!(error, SnapshotMerkleError::Parse(_)));
    }

    #[test]
    async fn merkle_chunk_proof_verifies() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = checked_random_snapshot_keypair();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let metadata = SnapshotMerkleMetadata::from_path(
            &current_generation_artifact(&store_dir, SNAPSHOT_MERKLE_FILE_NAME),
            u64::MAX,
        )
        .expect("metadata");
        let snapshot_bytes =
            std::fs::read(current_generation_artifact(&store_dir, SNAPSHOT_FILE_NAME))
                .expect("snapshot bytes");
        let chunk = &snapshot_bytes[..snapshot_bytes.len().min(TEST_CHUNK_SIZE.get())];
        metadata
            .verify_chunk(0, chunk)
            .expect("chunk proof should verify");

        let mut corrupted = chunk.to_vec();
        if corrupted.is_empty() {
            corrupted.push(1);
        } else {
            corrupted[0] ^= 0xFF;
        }
        let Err(err) = metadata.verify_chunk(0, &corrupted) else {
            panic!("corrupted chunk should fail verification");
        };
        assert!(matches!(err, SnapshotMerkleError::ProofInvalid { .. }));
    }

    #[test]
    async fn can_read_multiple_blocks() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        store_complete_snapshot_commit_evidence_for_blocks(&state, &kura, &[block1, block2]);

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let state = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        )
        .unwrap();

        assert_eq!(state.view().height(), 2);
    }

    #[test]
    async fn finalized_snapshot_tip_rejects_replacement_without_mutation() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let mut state = state_factory_with_kura(Arc::clone(&kura));
        let key_pair = checked_random_snapshot_keypair();

        let block1 = signed_block_after_transaction(accepted_log_transaction("first"), None);
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block1));
        let block2 = signed_block_after_transaction(
            accepted_log_transaction("second"),
            Some(block1.as_ref()),
        );
        store_block_and_mark_state_height(&mut state, &kura, Arc::clone(&block2));
        let canonical_tip = block2.hash();
        store_complete_snapshot_commit_evidence_for_blocks(
            &state,
            &kura,
            &[Arc::clone(&block1), block2],
        );

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let pointer_before = std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap();

        // Once the complete commit tuple authorizes a snapshot, the terminal block is final and
        // cannot be replaced by a same-height soft-fork candidate.
        let replacement = signed_block_after_transaction(
            accepted_log_transaction("soft-fork replacement"),
            Some(block1.as_ref()),
        );
        let replacement_hash = replacement.hash();
        assert_ne!(replacement_hash, canonical_tip);
        let error = kura
            .replace_top_block(replacement)
            .expect_err("checkpointed snapshot tip must reject replacement");
        assert!(matches!(
            error,
            crate::kura::Error::CommittedBlockReplacementForbidden { height: 2 }
        ));
        assert_eq!(
            kura.block_hash_at_height(nonzero!(2_usize)),
            Some(canonical_tip)
        );
        assert_eq!(kura.exact_durable_blocks_count().unwrap(), 2);
        assert_eq!(
            std::fs::read(store_dir.join(SNAPSHOT_CURRENT_FILE_NAME)).unwrap(),
            pointer_before,
            "rejected block replacement must not change the selected snapshot generation"
        );

        let restored = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            &crate::state::default_zk_config(),
            #[cfg(feature = "telemetry")]
            <_>::default(),
        )
        .unwrap();

        assert_eq!(restored.view().height(), 2);
        assert_eq!(restored.latest_block_hash_fast(), Some(canonical_tip));
    }
}
