//! Immutable crash-recovery records for Sumeragi v2 height contexts.
//!
//! The safety WAL deliberately stores reducer facts rather than mutable
//! configuration. A node must nevertheless recover the exact roster, powers,
//! leader seed, DA layout, and proofs of possession that were frozen before it
//! can authenticate that WAL. This store persists those canonical inputs before
//! the corresponding WAL is opened and never overwrites a conflicting height.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::Hash;
use iroha_data_model::block::consensus_v2 as wire;
use norito::codec::{Decode, DecodeAll, Encode};
use thiserror::Error;

use super::v2::VerifiedHeightContext;

const FILE_MAGIC: &[u8; 8] = b"SUMV2CTX";
const FRAME_VERSION: u16 = 1;
const HASH_LEN: usize = 32;
const HEADER_LEN: usize = FILE_MAGIC.len() + 2 + 8 + HASH_LEN;

/// Canonical context and PoPs required to reopen one reducer height.
#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
pub(crate) struct PersistedHeightContext {
    format_version: u16,
    context: wire::HeightContext,
    proofs_of_possession: Vec<Vec<u8>>,
}

impl PersistedHeightContext {
    /// Snapshot an already verified context without weakening its provenance.
    pub(crate) fn from_verified(context: &VerifiedHeightContext) -> Self {
        Self {
            format_version: FRAME_VERSION,
            context: context.context().clone(),
            proofs_of_possession: context.proofs_of_possession().to_vec(),
        }
    }

    /// Borrow the frozen wire context.
    pub(crate) const fn context(&self) -> &wire::HeightContext {
        &self.context
    }

    /// Borrow PoPs in frozen-roster order.
    pub(crate) fn proofs_of_possession(&self) -> &[Vec<u8>] {
        &self.proofs_of_possession
    }

    fn validate_layout(&self) -> Result<(), V2ContextStoreError> {
        if self.format_version != FRAME_VERSION {
            return Err(V2ContextStoreError::UnsupportedVersion(self.format_version));
        }
        self.context.validate().map_err(V2ContextStoreError::Wire)?;
        if self.proofs_of_possession.len() != self.context.roster.len() {
            return Err(V2ContextStoreError::ProofCountMismatch);
        }
        Ok(())
    }
}

/// Append-only context store rooted beside Kura's v2 finality sidecars.
#[derive(Clone, Debug)]
pub(crate) struct V2ContextStore {
    directory: PathBuf,
}

impl V2ContextStore {
    /// Open the store and synchronously create its directory.
    pub(crate) fn open(root: impl AsRef<Path>) -> Result<Self, V2ContextStoreError> {
        let directory = root.as_ref().join("contexts");
        fs::create_dir_all(&directory).map_err(|source| io_error(&directory, source))?;
        sync_directory(root.as_ref())?;
        Ok(Self { directory })
    }

    /// Persist an immutable record before opening its height WAL.
    ///
    /// An exact repeat is idempotent. A different record at the same height is
    /// a safety failure and is never replaced.
    pub(crate) fn persist(
        &self,
        record: &PersistedHeightContext,
    ) -> Result<(), V2ContextStoreError> {
        record.validate_layout()?;
        let path = self.path(record.context.height);
        let frame = encode_frame(record)?;
        match fs::read(&path) {
            Ok(existing) => {
                if existing == frame {
                    return Ok(());
                }
                let recovered = decode_frame(&existing)?;
                if recovered == *record {
                    return Err(V2ContextStoreError::NonCanonicalExistingFrame(path));
                }
                return Err(V2ContextStoreError::ConflictingHeight {
                    height: record.context.height,
                });
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(source) => return Err(io_error(&path, source)),
        }
        write_atomic_synced(&path, &frame)
    }

    /// Load and checksum-verify one exact height record.
    pub(crate) fn load(
        &self,
        height: wire::Height,
    ) -> Result<Option<PersistedHeightContext>, V2ContextStoreError> {
        let path = self.path(height);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(io_error(&path, source)),
        };
        let record = decode_frame(&bytes)?;
        if record.context.height != height {
            return Err(V2ContextStoreError::HeightMismatch {
                expected: height,
                actual: record.context.height,
            });
        }
        Ok(Some(record))
    }

    fn path(&self, height: wire::Height) -> PathBuf {
        self.directory.join(format!("{height:020}.norito"))
    }
}

fn encode_frame(record: &PersistedHeightContext) -> Result<Vec<u8>, V2ContextStoreError> {
    let payload = record.encode();
    let payload_len = u64::try_from(payload.len()).map_err(|_| V2ContextStoreError::TooLarge)?;
    let digest = Hash::new(&payload);
    let mut frame = Vec::with_capacity(
        HEADER_LEN
            .checked_add(payload.len())
            .ok_or(V2ContextStoreError::TooLarge)?,
    );
    frame.extend_from_slice(FILE_MAGIC);
    frame.extend_from_slice(&FRAME_VERSION.to_le_bytes());
    frame.extend_from_slice(&payload_len.to_le_bytes());
    frame.extend_from_slice(digest.as_ref());
    frame.extend_from_slice(&payload);
    Ok(frame)
}

fn decode_frame(bytes: &[u8]) -> Result<PersistedHeightContext, V2ContextStoreError> {
    if bytes.len() < HEADER_LEN || bytes.get(..FILE_MAGIC.len()) != Some(FILE_MAGIC.as_slice()) {
        return Err(V2ContextStoreError::MalformedFrame);
    }
    let version_offset = FILE_MAGIC.len();
    let version = u16::from_le_bytes(
        bytes[version_offset..version_offset + 2]
            .try_into()
            .map_err(|_| V2ContextStoreError::MalformedFrame)?,
    );
    if version != FRAME_VERSION {
        return Err(V2ContextStoreError::UnsupportedVersion(version));
    }
    let length_offset = version_offset + 2;
    let payload_len = usize::try_from(u64::from_le_bytes(
        bytes[length_offset..length_offset + 8]
            .try_into()
            .map_err(|_| V2ContextStoreError::MalformedFrame)?,
    ))
    .map_err(|_| V2ContextStoreError::TooLarge)?;
    let hash_offset = length_offset + 8;
    let payload_offset = hash_offset + HASH_LEN;
    if bytes.len() != payload_offset.saturating_add(payload_len) {
        return Err(V2ContextStoreError::MalformedFrame);
    }
    let payload = &bytes[payload_offset..];
    if Hash::new(payload).as_ref() != &bytes[hash_offset..payload_offset] {
        return Err(V2ContextStoreError::HashMismatch);
    }
    let mut cursor = payload;
    let record = PersistedHeightContext::decode_all(&mut cursor)
        .map_err(|error| V2ContextStoreError::Decode(error.to_string()))?;
    record.validate_layout()?;
    Ok(record)
}

fn write_atomic_synced(path: &Path, bytes: &[u8]) -> Result<(), V2ContextStoreError> {
    let parent = path.parent().ok_or_else(|| V2ContextStoreError::Io {
        path: path.to_path_buf(),
        source: io::Error::new(ErrorKind::InvalidInput, "context path has no parent"),
    })?;
    let temporary = path.with_extension("norito.tmp");
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&temporary)
        .map_err(|source| io_error(&temporary, source))?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| io_error(&temporary, source))?;
    fs::rename(&temporary, path).map_err(|source| io_error(path, source))?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<(), V2ContextStoreError> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: io::Error) -> V2ContextStoreError {
    V2ContextStoreError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Fail-closed context persistence or recovery error.
#[derive(Debug, Error)]
pub(crate) enum V2ContextStoreError {
    /// Filesystem operation failed.
    #[error("Sumeragi v2 context-store I/O failed at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying I/O failure.
        #[source]
        source: io::Error,
    },
    /// Context record is not the supported layout revision.
    #[error("unsupported Sumeragi v2 context-store version {0}")]
    UnsupportedVersion(u16),
    /// Frame header or exact length is malformed.
    #[error("malformed Sumeragi v2 context-store frame")]
    MalformedFrame,
    /// Payload checksum failed.
    #[error("Sumeragi v2 context-store hash mismatch")]
    HashMismatch,
    /// Norito payload failed complete decoding.
    #[error("malformed Sumeragi v2 context-store payload: {0}")]
    Decode(String),
    /// Embedded height context failed structural validation.
    #[error("invalid Sumeragi v2 height context: {0}")]
    Wire(wire::ValidationError),
    /// PoP vector is not aligned with the voting roster.
    #[error("Sumeragi v2 context-store PoP count differs from roster length")]
    ProofCountMismatch,
    /// Encoded length cannot be represented safely.
    #[error("Sumeragi v2 context-store frame is too large")]
    TooLarge,
    /// Immutable height already has different contents.
    #[error("conflicting Sumeragi v2 context record at height {height}")]
    ConflictingHeight {
        /// Conflicted chain height.
        height: wire::Height,
    },
    /// Existing bytes decode to the same value but are not canonical bytes.
    #[error("non-canonical Sumeragi v2 context frame at {}", .0.display())]
    NonCanonicalExistingFrame(PathBuf),
    /// File name and embedded height disagree.
    #[error("Sumeragi v2 context height mismatch: expected {expected}, got {actual}")]
    HeightMismatch {
        /// Requested height.
        expected: wire::Height,
        /// Embedded height.
        actual: wire::Height,
    },
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, Hash, KeyPair};
    use iroha_data_model::{ChainId, peer::PeerId};

    use super::*;

    fn record() -> PersistedHeightContext {
        let mut roster = (1_u8..=4)
            .map(|seed| {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::BlsNormal)
                    .expect("deterministic BLS key");
                wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        let context = wire::HeightContext {
            chain_id: ChainId::from("v2-context-store-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            next_epoch_snapshot: None,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"context-store-nexus-amx"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 64,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 4096,
                max_chunk_count: 64,
            },
            leader_seed: [0x51; 32],
        };
        PersistedHeightContext {
            format_version: FRAME_VERSION,
            proofs_of_possession: vec![vec![0xAA; 48]; context.roster.len()],
            context,
        }
    }

    #[test]
    fn record_roundtrips_and_exact_repeat_is_idempotent() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let record = record();
        store.persist(&record).expect("persist record");
        store.persist(&record).expect("repeat exact record");
        assert_eq!(store.load(1).expect("load record"), Some(record));
    }

    #[test]
    fn corruption_and_conflicting_height_fail_closed() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        let record = record();
        store.persist(&record).expect("persist record");

        let path = store.path(1);
        let mut bytes = fs::read(&path).expect("read frame");
        *bytes.last_mut().expect("nonempty frame") ^= 0x80;
        fs::write(&path, bytes).expect("inject corruption");
        assert!(matches!(
            store.load(1),
            Err(V2ContextStoreError::HashMismatch)
        ));

        fs::remove_file(&path).expect("remove corrupt frame");
        store.persist(&record).expect("restore record");
        let mut conflicting = record;
        conflicting.context.leader_seed[0] ^= 1;
        assert!(matches!(
            store.persist(&conflicting),
            Err(V2ContextStoreError::ConflictingHeight { height: 1 })
        ));
    }

    #[test]
    fn incomplete_temporary_frame_is_unacknowledged() {
        let root = tempfile::tempdir().expect("tempdir");
        let store = V2ContextStore::open(root.path()).expect("open store");
        fs::write(store.path(1).with_extension("norito.tmp"), b"partial")
            .expect("write partial temporary frame");
        assert_eq!(store.load(1).expect("missing final path"), None);
        store
            .persist(&record())
            .expect("replace unacknowledged write");
        assert!(store.load(1).expect("load final").is_some());
    }
}
