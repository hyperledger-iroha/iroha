//! This module contains [`State`] snapshot actor service.
use std::{
    collections::{BTreeMap, BTreeSet},
    io::{Read, Write},
    num::{NonZeroU32, NonZeroU64, NonZeroUsize},
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
    Algorithm, CompactMerkleProof, Hash, HashOf, KeyPair, MerkleTree, MerkleTreeCommitment,
    PublicKey, Signature,
};
use iroha_data_model::{
    ChainId,
    account::AccountId,
    asset::AssetId,
    block::{BlockHeader, consensus_v2::SnapshotV2BootstrapRecord},
    nexus::{LaneCatalog, LaneId},
    state_path::StatePath,
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
    static SNAPSHOT_HASH_RECONCILIATION_PASSES: std::cell::Cell<usize> =
        const { std::cell::Cell::new(0) };
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

#[cfg(unix)]
fn snapshot_unix_owner_and_mode_are_trusted(uid: u32, mode: u32, effective_uid: u32) -> bool {
    uid == effective_uid && mode & 0o022 == 0
}

fn snapshot_metadata_has_trusted_owner_and_mode(metadata: &std::fs::Metadata) -> bool {
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

    let mut file = std::fs::File::open(path)?;
    let opened_before = file.metadata()?;
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
    let capacity = bounded_snapshot_read_capacity(opened_before.len(), max_bytes)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(capacity).map_err(|error| {
        std::io::Error::other(format!(
            "failed to reserve memory for snapshot artifact: {error}"
        ))
    })?;
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
        || !snapshot_metadata_has_trusted_owner_and_mode(&opened_after)
        || !snapshot_metadata_has_trusted_owner_and_mode(&path_after)
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
        || !snapshot_metadata_has_trusted_owner_and_mode(&metadata)
        || !snapshot_metadata_has_trusted_owner_and_mode(&opened)
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

/// Reconstruct generated bytes through the semantic restart boundary before publication.
///
/// # Errors
/// Returns the same typed semantic error that would make the restart reader reject the payload.
fn validate_generated_snapshot_for_restart(
    state: &State,
    snapshot_bytes: &[u8],
) -> Result<(), TryReadError> {
    let value: json::Value =
        json::from_slice(snapshot_bytes).map_err(TryReadError::Serialization)?;
    validate_snapshot_sccp_registry(&value)?;
    let seed = KuraSeed {
        kura: state.kura_handle(),
        query_handle: state.query_handle.clone(),
        #[cfg(feature = "telemetry")]
        telemetry: StateTelemetry::default(),
    };
    let mut restored = seed
        .into_state_from_json_without_durable_recovery(value)
        .map_err(TryReadError::Serialization)?;
    if restored.chain_id_ref() != state.chain_id_ref() {
        return Err(TryReadError::ChainIdMismatch {
            expected: state.chain_id_ref().clone(),
            actual: restored.chain_id_ref().clone(),
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

    let mut canonical_payload = String::new();
    serialize_state_snapshot(&restored, &mut canonical_payload, true);
    if canonical_payload.as_bytes() != snapshot_bytes {
        return Err(TryReadError::NonCanonicalSnapshotPayload);
    }
    Ok(())
}

/// Serialize, validate, and durably publish one canonical state snapshot.
fn try_write_snapshot_with_limit(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
) -> Result<(), TryWriteError> {
    let _publication_guard = SNAPSHOT_PUBLICATION_LOCK.lock();
    let mut snapshot_json = String::new();
    serialize_state_snapshot(state, &mut snapshot_json, true);
    try_write_snapshot_payload_with_limit_locked(
        state,
        store_dir,
        signing_key,
        merkle_chunk_size,
        max_payload_bytes,
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
    let _publication_guard = SNAPSHOT_PUBLICATION_LOCK.lock();
    try_write_snapshot_payload_with_limit_locked(
        state,
        store_dir,
        signing_key,
        merkle_chunk_size,
        max_payload_bytes,
        snapshot_bytes,
    )
}

fn try_write_snapshot_payload_with_limit_locked(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
    max_payload_bytes: NonZeroUsize,
    snapshot_bytes: Vec<u8>,
) -> Result<(), TryWriteError> {
    ensure_state_is_backed_by_kura(state)?;
    if snapshot_bytes.len() > max_payload_bytes.get() {
        return Err(TryWriteError::PayloadTooLarge {
            actual: snapshot_bytes.len(),
            maximum: max_payload_bytes,
        });
    }
    validate_generated_snapshot_for_restart(state, &snapshot_bytes)
        .map_err(TryWriteError::RestartValidation)?;
    let geometry_checkpoint = geometry_checkpoint_from_snapshot_bytes(&snapshot_bytes)?;
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
    let smart_contract_storage: Storage<StatePath, Vec<u8>> =
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
