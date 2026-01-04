//! This module contains [`State`] snapshot actor service.
use std::{
    io::{Read, Write},
    num::NonZeroUsize,
    path::{Path, PathBuf},
    sync::Arc,
    time::Duration,
};

use hex;
use iroha_config::{
    parameters::{actual::Snapshot as Config, defaults},
    snapshot::Mode,
};
use iroha_crypto::{CompactMerkleProof, Hash, HashOf, KeyPair, MerkleTree, PublicKey, Signature};
use iroha_data_model::{ChainId, block::BlockHeader};
use iroha_futures::supervisor::{Child, OnShutdown, ShutdownSignal};
use iroha_logger::prelude::*;
use norito::json::{self, JsonDeserialize, JsonSerialize, JsonSerialize as JsonSerializeTrait};
use sha2::{Digest, Sha256};

#[cfg(feature = "telemetry")]
use crate::telemetry::StateTelemetry;
use crate::{
    kura::{BlockCount, Kura},
    query::store::LiveQueryStoreHandle,
    state::{State, deserialize::KuraSeed, storage_transactions::TransactionsBlockError},
};

// Serialize State as a minimal snapshot wrapper using Norito JSON writer.
impl JsonSerializeTrait for State {
    fn json_serialize(&self, out: &mut String) {
        let view = self.view();
        let block_hashes: Vec<HashOf<BlockHeader>> = view.block_hashes.iter().copied().collect();
        let commit_topology = view.commit_topology.to_vec();
        let prev_commit_topology = view.prev_commit_topology.to_vec();

        out.push('{');
        json::write_json_string("chain_id", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&self.chain_id, out);
        out.push(',');
        json::write_json_string("world", out);
        out.push(':');
        self.world.json_serialize(out);
        out.push(',');

        json::write_json_string("block_hashes", out);
        out.push(':');
        json::JsonSerialize::json_serialize(&block_hashes, out);
        out.push(',');

        json::write_json_string("transactions", out);
        out.push(':');
        self.transactions.json_serialize(out);
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
}

/// Name of the [`State`] snapshot file.
const SNAPSHOT_FILE_NAME: &str = "snapshot.data";
/// Name of the temporary [`State`] snapshot file.
const SNAPSHOT_TMP_FILE_NAME: &str = "snapshot.tmp";
/// Name of the digest accompanying the snapshot file.
const SNAPSHOT_DIGEST_FILE_NAME: &str = "snapshot.sha256";
/// Name of the signature accompanying the digest.
const SNAPSHOT_SIGNATURE_FILE_NAME: &str = "snapshot.sig";
/// Name of the Merkle metadata accompanying the snapshot file.
const SNAPSHOT_MERKLE_FILE_NAME: &str = "snapshot.merkle.json";
/// Name of the temporary Merkle metadata file.
const SNAPSHOT_MERKLE_TMP_FILE_NAME: &str = "snapshot.merkle.json.tmp";
/// Default chunk size used to derive snapshot Merkle metadata.
const _DEFAULT_MERKLE_CHUNK_SIZE: NonZeroUsize = defaults::snapshot::MERKLE_CHUNK_SIZE_BYTES;

#[derive(thiserror::Error, Debug, displaydoc::Display)]
enum SnapshotMerkleError {
    /// Snapshot Merkle metadata missing
    Missing,
    /// Snapshot Merkle metadata IO failure
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

#[derive(Debug, Clone, JsonSerialize, JsonDeserialize)]
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

    fn tree(&self) -> Result<MerkleTree<[u8; 32]>, SnapshotMerkleError> {
        let leaves = self.parse_leaves()?;
        Ok(MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(leaves))
    }

    fn verify_self(&self) -> Result<(), SnapshotMerkleError> {
        let chunk_size = self.chunk_size()?;
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
        let _ = chunk_size;
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

    fn from_path(path: &Path) -> Result<Self, SnapshotMerkleError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Err(SnapshotMerkleError::Missing);
            }
            Err(err) => return Err(SnapshotMerkleError::Io(err)),
        };
        json::from_slice(&bytes).map_err(SnapshotMerkleError::Parse)
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
}

impl SnapshotMaker {
    /// Start the actor.
    pub fn start(self, shutdown_signal: ShutdownSignal) -> Child {
        Child::new(
            tokio::spawn(self.run(shutdown_signal)),
            OnShutdown::Wait(Duration::from_secs(2)),
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
        let latest_block_hash;
        let at_height;
        {
            let state_view = self.state.view();
            latest_block_hash = state_view.latest_block_hash();
            at_height = state_view.height();
        }

        if latest_block_hash != self.latest_block_hash {
            let state = self.state.clone();
            let store_dir = store_dir.clone();
            let signing_key = self.signing_key.clone();
            let merkle_chunk_size = self.merkle_chunk_size;
            let result = tokio::task::block_in_place(move || {
                try_write_snapshot(&state, store_dir, &signing_key, merkle_chunk_size)
            });

            match result {
                Ok(()) => {
                    iroha_logger::info!(at_height, "Successfully created a snapshot of state");
                    self.latest_block_hash = latest_block_hash;
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
            let latest_block_hash = state.view().latest_block_hash();
            Some(Self {
                state,
                create_every: config.create_every_ms.get(),
                store_dir: config.store_dir.resolve_relative_path(),
                latest_block_hash,
                signing_key,
                merkle_chunk_size: config.merkle_chunk_size_bytes,
            })
        } else {
            None
        }
    }
}

/// Try to deserialize [`State`] from a snapshot file.
///
/// # Errors
/// - IO errors
/// - Deserialization errors
#[allow(clippy::too_many_lines)]
pub fn try_read_snapshot(
    store_dir: impl AsRef<Path>,
    kura: &Arc<Kura>,
    live_query_store_lazy: impl FnOnce() -> LiveQueryStoreHandle,
    BlockCount(block_count): BlockCount,
    merkle_chunk_size: NonZeroUsize,
    verification_key: &PublicKey,
    expected_chain_id: &ChainId,
    #[cfg(feature = "telemetry")] telemetry: StateTelemetry,
) -> Result<State, TryReadError> {
    let mut bytes = Vec::new();
    let path = store_dir.as_ref().join(SNAPSHOT_FILE_NAME);
    let mut file = match std::fs::OpenOptions::new().read(true).open(&path) {
        Ok(file) => file,
        Err(err) => {
            return if err.kind() == std::io::ErrorKind::NotFound {
                Err(TryReadError::NotFound)
            } else {
                Err(TryReadError::IO(err, path.clone()))
            };
        }
    };
    file.read_to_end(&mut bytes)
        .map_err(|err| TryReadError::IO(err, path.clone()))?;

    let digest_path = store_dir.as_ref().join(SNAPSHOT_DIGEST_FILE_NAME);
    let expected_digest = match std::fs::read_to_string(&digest_path) {
        Ok(contents) => contents.trim().to_owned(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(TryReadError::ChecksumMissing(digest_path));
        }
        Err(err) => return Err(TryReadError::IO(err, digest_path)),
    };
    let digest_bytes = Sha256::digest(&bytes);
    let digest_vec = digest_bytes.to_vec();
    let actual_digest = hex::encode(&digest_vec);
    if expected_digest != actual_digest {
        return Err(TryReadError::ChecksumMismatch {
            expected: expected_digest,
            actual: actual_digest,
        });
    }

    let sig_path = store_dir.as_ref().join(SNAPSHOT_SIGNATURE_FILE_NAME);
    let signature_hex = match std::fs::read_to_string(&sig_path) {
        Ok(contents) => contents.trim().to_owned(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Err(TryReadError::SignatureMissing(sig_path));
        }
        Err(err) => return Err(TryReadError::IO(err, sig_path)),
    };
    let signature = Signature::from_hex(&signature_hex)
        .map_err(|_| TryReadError::SignatureMalformed(signature_hex.clone()))?;
    signature
        .verify(verification_key, &digest_vec)
        .map_err(|err| TryReadError::SignatureInvalid(err.to_string()))?;
    let merkle_path = store_dir.as_ref().join(SNAPSHOT_MERKLE_FILE_NAME);
    let merkle_meta = SnapshotMerkleMetadata::from_path(&merkle_path)
        .map_err(|err| merkle_err_to_try_read(err, merkle_path.clone()))?;
    merkle_meta
        .verify_against_bytes(&bytes, merkle_chunk_size)
        .map_err(|err| merkle_err_to_try_read(err, merkle_path.clone()))?;
    #[cfg(test)]
    {
        eprintln!(
            "SNAPSHOT READ DEBUG: first bytes {:?}",
            std::str::from_utf8(&bytes).unwrap_or("<non-utf8>")
        );
    }
    let value: json::Value = match json::from_slice(&bytes) {
        Ok(value) => {
            #[cfg(test)]
            eprintln!("SNAPSHOT PARSE OK");
            value
        }
        Err(err) => {
            #[cfg(test)]
            eprintln!("SNAPSHOT PARSE ERR: {err:?}");
            return Err(TryReadError::Serialization(err));
        }
    };
    let seed = KuraSeed {
        kura: Arc::clone(kura),
        query_handle: live_query_store_lazy(),
        #[cfg(feature = "telemetry")]
        telemetry,
    };
    let state = seed
        .into_state_from_json(value)
        .map_err(TryReadError::Serialization)?;
    if &state.chain_id != expected_chain_id {
        return Err(TryReadError::ChainIdMismatch {
            expected: expected_chain_id.clone(),
            actual: state.chain_id.clone(),
        });
    }
    let (snapshot_height, snapshot_hashes) = {
        let state_view = state.view();
        let hashes = state_view.block_hashes.iter().copied().collect::<Vec<_>>();
        (state_view.height(), hashes)
    };
    if snapshot_height > block_count {
        return Err(TryReadError::MismatchedHeight {
            snapshot_height,
            kura_height: block_count,
        });
    }
    for (idx, snapshot_block_hash) in snapshot_hashes.into_iter().enumerate() {
        let height = idx + 1;
        let kura_block = kura
            .get_block(NonZeroUsize::new(height).expect("iterating from 1"))
            .ok_or(TryReadError::MissingBlock { height })?;
        if kura_block.hash() != snapshot_block_hash {
            if height == snapshot_height {
                iroha_logger::warn!(
                    "Snapshot has incorrect latest block hash, discarding changes made by this block"
                );
                state
                    .block_and_revert(kura_block.header())
                    .commit()
                    .map_err(TryReadError::StateCommit)?;
            } else {
                return Err(TryReadError::MismatchedHash {
                    height,
                    snapshot_block_hash,
                    kura_block_hash: kura_block.hash(),
                });
            }
        }
    }
    Ok(state)
}

/// Serialize and write snapshot to file,
/// overwriting any previously stored data.
///
/// # Errors
/// - IO errors
/// - Serialization errors
fn try_write_snapshot(
    state: &State,
    store_dir: impl AsRef<Path>,
    signing_key: &KeyPair,
    merkle_chunk_size: NonZeroUsize,
) -> Result<(), TryWriteError> {
    std::fs::create_dir_all(store_dir.as_ref())
        .map_err(|err| TryWriteError::IO(err, store_dir.as_ref().to_path_buf()))?;
    let path_to_file = store_dir.as_ref().join(SNAPSHOT_FILE_NAME);
    let path_to_digest_file = store_dir.as_ref().join(SNAPSHOT_DIGEST_FILE_NAME);
    let path_to_signature_file = store_dir.as_ref().join(SNAPSHOT_SIGNATURE_FILE_NAME);
    let path_to_merkle_file = store_dir.as_ref().join(SNAPSHOT_MERKLE_FILE_NAME);
    let path_to_tmp_file = store_dir.as_ref().join(SNAPSHOT_TMP_FILE_NAME);
    let path_to_tmp_digest = store_dir
        .as_ref()
        .join(format!("{SNAPSHOT_DIGEST_FILE_NAME}.tmp"));
    let path_to_tmp_sig = store_dir
        .as_ref()
        .join(format!("{SNAPSHOT_SIGNATURE_FILE_NAME}.tmp"));
    let path_to_tmp_merkle = store_dir.as_ref().join(SNAPSHOT_MERKLE_TMP_FILE_NAME);
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path_to_tmp_file)
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_file.clone()))?;
    json::to_writer(&mut file, state).map_err(TryWriteError::Serialization)?;
    file.flush()
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_file.clone()))?;
    file.sync_data()
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_file.clone()))?;
    #[cfg(test)]
    {
        use std::io::Read as _;
        let mut debug_reader = std::fs::File::open(&path_to_tmp_file).expect("debug open");
        let mut debug_buf = String::new();
        debug_reader
            .read_to_string(&mut debug_buf)
            .expect("debug read");
        eprintln!("SNAPSHOT DEBUG: {debug_buf}");
    }
    let snapshot_bytes = std::fs::read(&path_to_tmp_file)
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_file.clone()))?;
    let digest_bytes = Sha256::digest(&snapshot_bytes);
    let digest_vec = digest_bytes.to_vec();
    let digest_hex = hex::encode(&digest_vec);
    let merkle = SnapshotMerkleMetadata::from_bytes(&snapshot_bytes, merkle_chunk_size);
    std::fs::write(&path_to_tmp_digest, format!("{digest_hex}\n"))
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_digest.clone()))?;
    let signature = Signature::new(signing_key.private_key(), &digest_vec);
    std::fs::write(&path_to_tmp_sig, hex::encode(signature.payload()))
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_sig.clone()))?;
    let mut merkle_file = std::fs::OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(&path_to_tmp_merkle)
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_merkle.clone()))?;
    json::to_writer(&mut merkle_file, &merkle).map_err(TryWriteError::MerkleSerialization)?;
    merkle_file
        .flush()
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_merkle.clone()))?;
    merkle_file
        .sync_data()
        .map_err(|err| TryWriteError::IO(err, path_to_tmp_merkle.clone()))?;
    std::fs::rename(path_to_tmp_file, &path_to_file)
        .map_err(|err| TryWriteError::IO(err, path_to_file.clone()))?;
    std::fs::rename(path_to_tmp_digest, &path_to_digest_file)
        .map_err(|err| TryWriteError::IO(err, path_to_digest_file.clone()))?;
    std::fs::rename(path_to_tmp_sig, &path_to_signature_file)
        .map_err(|err| TryWriteError::IO(err, path_to_signature_file.clone()))?;
    std::fs::rename(path_to_tmp_merkle, &path_to_merkle_file)
        .map_err(|err| TryWriteError::IO(err, path_to_merkle_file.clone()))?;
    Ok(())
}

/// Error variants for snapshot reading
#[derive(thiserror::Error, Debug, displaydoc::Display)]
pub enum TryReadError {
    /// The snapshot was not found
    NotFound,
    /// Failed reading/writing {1:?} from disk
    IO(#[source] std::io::Error, PathBuf),
    /// Error (de)serializing state snapshot
    Serialization(norito::json::Error),
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
    /// Snapshot is in a non-consistent state. Snapshot has greater height (`snapshot_height`) than kura block store (`kura_height`)
    MismatchedHeight {
        /// The amount of block hashes stored by snapshot
        snapshot_height: usize,
        /// The amount of blocks stored by [`Kura`]
        kura_height: usize,
    },
    /// Snapshot is in a non-consistent state. Hash of the block at height `height` is different between snapshot (`snapshot_block_hash`) and kura (`kura_block_hash`)
    MismatchedHash {
        /// Height at which block hashes differs between snapshot and [`Kura`]
        height: usize,
        /// Hash of the block stored in snapshot
        snapshot_block_hash: HashOf<BlockHeader>,
        /// Hash of the block stored in kura
        kura_block_hash: HashOf<BlockHeader>,
    },
    /// Snapshot is in a non-consistent state. Kura is missing block `height`.
    MissingBlock {
        /// Height of the missing block in [`Kura`].
        height: usize,
    },
    /// Failed to reconcile snapshot state with Kura while committing a block revert
    StateCommit(TransactionsBlockError),
}

fn merkle_err_to_try_read(err: SnapshotMerkleError, path: PathBuf) -> TryReadError {
    match err {
        SnapshotMerkleError::Missing => TryReadError::MerkleMissing(path),
        SnapshotMerkleError::Io(io) => TryReadError::IO(io, path),
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
}

#[cfg(test)]
mod tests {
    use std::{fs::File, io::Write, num::NonZeroUsize};

    use iroha_crypto::{Algorithm, Hash, HashOf, KeyPair, Signature};
    use iroha_data_model::{ChainId, peer::PeerId};
    use nonzero_ext::nonzero;
    use tempfile::tempdir;
    use tokio::test;

    use super::*;
    use crate::{
        block::ValidBlock, query::store::LiveQueryStore, sumeragi::network_topology::Topology,
    };

    const TEST_CHUNK_SIZE: NonZeroUsize = nonzero!(1024_usize);
    const TEST_CHAIN_ID: &str = "test-chain";

    fn state_factory() -> State {
        let kura = Kura::blank_kura_for_testing();
        let query_handle = LiveQueryStore::start_test();
        let mut state = State::new(
            crate::queue::tests::world_with_test_domains(),
            kura,
            query_handle,
        );
        state.chain_id = ChainId::from(TEST_CHAIN_ID);
        state
    }

    #[test]
    async fn creates_all_dirs_while_writing_snapshots() {
        let tmp_root = tempdir().unwrap();
        let snapshot_store_dir = tmp_root.path().join("path/to/snapshot/dir");
        let state = state_factory();
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &snapshot_store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        assert!(Path::exists(snapshot_store_dir.as_path()))
    }

    #[test]
    async fn can_read_snapshot_after_writing() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let _wsv = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            StateTelemetry::new(<_>::default(), true),
        )
        .unwrap();
    }

    #[test]
    async fn cannot_find_snapshot_on_read_is_not_found() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let key_pair = KeyPair::random();
        let chain_id = ChainId::from(TEST_CHAIN_ID);

        let Err(error) = try_read_snapshot(
            store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(15),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &chain_id,
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
        let key_pair = KeyPair::random();
        let chain_id = ChainId::from(TEST_CHAIN_ID);
        let corrupted = [1, 4, 1, 2, 3, 4, 1, 4];
        {
            let mut file = File::create(store_dir.join(SNAPSHOT_FILE_NAME)).unwrap();
            file.write_all(&corrupted).unwrap();
        }
        let digest_bytes = Sha256::digest(corrupted);
        let digest_vec = digest_bytes.to_vec();
        let digest = hex::encode(&digest_vec);
        std::fs::write(store_dir.join(SNAPSHOT_DIGEST_FILE_NAME), digest).unwrap();
        let sig = Signature::new(key_pair.private_key(), &digest_vec);
        std::fs::write(
            store_dir.join(SNAPSHOT_SIGNATURE_FILE_NAME),
            hex::encode(sig.payload()),
        )
        .unwrap();
        let merkle = SnapshotMerkleMetadata::from_bytes(&corrupted, TEST_CHUNK_SIZE);
        let mut merkle_file = File::create(store_dir.join(SNAPSHOT_MERKLE_FILE_NAME)).unwrap();
        json::to_writer(&mut merkle_file, &merkle).unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(15),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &chain_id,
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
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        // Corrupt the digest without touching the snapshot bytes.
        std::fs::write(store_dir.join(SNAPSHOT_DIGEST_FILE_NAME), "deadbeef").unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::ChecksumMismatch { .. }));
    }

    #[test]
    async fn chain_id_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();
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
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::ChainIdMismatch { .. }));
    }

    #[test]
    async fn missing_kura_block_is_reported() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();
        let hash = HashOf::<BlockHeader>::from_untyped_unchecked(Hash::prehashed([0x11; 32]));

        {
            let mut block_hashes = state.block_hashes.block();
            block_hashes.push(hash);
            block_hashes.commit_for_tests();
        }

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(1),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("missing Kura block should error");
        };

        assert!(matches!(error, TryReadError::MissingBlock { height: 1 }));
    }

    #[test]
    async fn missing_checksum_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        std::fs::remove_file(store_dir.join(SNAPSHOT_DIGEST_FILE_NAME)).unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::ChecksumMissing(_)));
    }

    #[test]
    async fn missing_merkle_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        std::fs::remove_file(store_dir.join(SNAPSHOT_MERKLE_FILE_NAME)).unwrap();

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::MerkleMissing(_)));
    }

    #[test]
    async fn merkle_root_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&store_dir.join(SNAPSHOT_MERKLE_FILE_NAME))
                .expect("metadata");
        metadata.root_hex = hex::encode([0xAA; Hash::LENGTH]);
        let mut merkle_file =
            File::create(store_dir.join(SNAPSHOT_MERKLE_FILE_NAME)).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        ) else {
            panic!("should not be ok")
        };

        assert!(matches!(error, TryReadError::MerkleMismatch { .. }));
    }

    #[test]
    async fn merkle_chunk_size_mismatch_rejected() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let mut metadata =
            SnapshotMerkleMetadata::from_path(&store_dir.join(SNAPSHOT_MERKLE_FILE_NAME))
                .expect("metadata");
        metadata.chunk_size_bytes = u64::try_from(TEST_CHUNK_SIZE.get() * 2).expect("fits in u64");
        let mut merkle_file =
            File::create(store_dir.join(SNAPSHOT_MERKLE_FILE_NAME)).expect("merkle file");
        json::to_writer(&mut merkle_file, &metadata).expect("write merkle");

        let Err(error) = try_read_snapshot(
            &store_dir,
            &Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
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
    async fn merkle_chunk_proof_verifies() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let state = state_factory();
        let key_pair = KeyPair::random();

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();
        let metadata =
            SnapshotMerkleMetadata::from_path(&store_dir.join(SNAPSHOT_MERKLE_FILE_NAME))
                .expect("metadata");
        let snapshot_bytes =
            std::fs::read(store_dir.join(SNAPSHOT_FILE_NAME)).expect("snapshot bytes");
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
        let state = state_factory();
        let key_pair = KeyPair::random();

        let peer_key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(peer_key_pair.public_key().clone());
        let topology = Topology::new(vec![peer_id]);
        let valid_block =
            ValidBlock::new_dummy_and_modify_header(peer_key_pair.private_key(), |header| {
                header.set_height(nonzero!(1u64));
            });
        let committed_block = valid_block
            .clone()
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        {
            let mut state_block = state.block(committed_block.as_ref().header());
            let _events =
                state_block.apply_without_execution(&committed_block, topology.as_ref().to_owned());
            state_block.commit().unwrap();
        }
        kura.store_block(committed_block)
            .expect("store first block");

        let valid_block =
            ValidBlock::new_dummy_and_modify_header(peer_key_pair.private_key(), |header| {
                header.set_height(nonzero!(2u64));
            });
        let committed_block = valid_block
            .clone()
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        {
            let mut state_block = state.block(committed_block.as_ref().header());
            let _events =
                state_block.apply_without_execution(&committed_block, topology.as_ref().to_owned());
            state_block.commit().unwrap();
        }
        kura.store_block(committed_block)
            .expect("store second block");

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let state = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            StateTelemetry::default(),
        )
        .unwrap();

        assert_eq!(state.view().height(), 2);
    }

    #[test]
    async fn can_read_last_block_incorrect() {
        let tmp_root = tempdir().unwrap();
        let store_dir = tmp_root.path().join("snapshot");
        let kura = Kura::blank_kura_for_testing();
        let state = state_factory();
        let key_pair = KeyPair::random();

        let peer_key_pair = KeyPair::random_with_algorithm(Algorithm::BlsNormal);
        let peer_id = PeerId::new(peer_key_pair.public_key().clone());
        let topology = Topology::new(vec![peer_id]);
        let valid_block =
            ValidBlock::new_dummy_and_modify_header(peer_key_pair.private_key(), |header| {
                header.set_height(nonzero!(1u64));
            });
        let committed_block = valid_block
            .clone()
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        {
            let mut state_block = state.block(committed_block.as_ref().header());
            let _events =
                state_block.apply_without_execution(&committed_block, topology.as_ref().to_owned());
            state_block.commit().unwrap();
        }
        kura.store_block(committed_block)
            .expect("store first block");

        let valid_block =
            ValidBlock::new_dummy_and_modify_header(peer_key_pair.private_key(), |header| {
                header.set_height(nonzero!(2u64));
            });
        let committed_block = valid_block
            .clone()
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();

        {
            let mut state_block = state.block(committed_block.as_ref().header());
            let _events =
                state_block.apply_without_execution(&committed_block, topology.as_ref().to_owned());
            state_block.commit().unwrap();
        }
        kura.store_block(committed_block)
            .expect("store second block");

        // Store inside kura different block at the same height with different view change
        // index. This imitates a snapshot created for a block which is later discarded as a
        // soft-fork.
        let valid_block =
            ValidBlock::new_dummy_and_modify_header(peer_key_pair.private_key(), |header| {
                header.set_height(nonzero!(2u64));
                header.set_view_change_index(header.view_change_index() + 1);
            });
        let committed_block = valid_block
            .clone()
            .commit(&topology)
            .unpack(|_| {})
            .unwrap();
        kura.replace_top_block(committed_block)
            .expect("replace top block");

        try_write_snapshot(&state, &store_dir, &key_pair, TEST_CHUNK_SIZE).unwrap();

        let state = try_read_snapshot(
            &store_dir,
            &kura,
            LiveQueryStore::start_test,
            BlockCount(state.view().height()),
            TEST_CHUNK_SIZE,
            key_pair.public_key(),
            &state.chain_id,
            #[cfg(feature = "telemetry")]
            <_>::default(),
        )
        .unwrap();

        // Invalid block was discarded
        assert_eq!(state.view().height(), 1);
    }
}
