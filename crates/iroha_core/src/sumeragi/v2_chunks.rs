//! Deterministic, persistent payload dispersal for Sumeragi v2.
//!
//! This module is transport, not consensus. It derives the one canonical
//! chunk sequence committed by [`PayloadManifest`], persists authenticated
//! chunks by manifest hash, and reconstructs exact canonical block bytes. The
//! reducer sees only the resulting body-availability token; READY/DELIVER
//! state and collector selection do not exist here.

use std::{
    fs::{self, File, OpenOptions},
    io::{self, ErrorKind, Write},
    path::{Path, PathBuf},
};

use iroha_crypto::{Hash, HashOf};
use iroha_data_model::block::consensus_v2 as wire;
use iroha_primitives::erasure::rs16;
use norito::codec::{Decode, DecodeAll, Encode};
use thiserror::Error;

const MANIFEST_FILE: &str = "manifest.norito";
const STORE_VERSION: u16 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Decode, Encode)]
struct StoredManifest {
    version: u16,
    manifest: wire::PayloadManifest,
}

/// Canonical encoded payload and the manifest committing to every chunk.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EncodedV2Payload {
    manifest: wire::PayloadManifest,
    chunks: Vec<Vec<u8>>,
}

impl EncodedV2Payload {
    /// Borrow the canonical manifest.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }

    /// Borrow encoded chunks in manifest-index order.
    pub(crate) fn chunks(&self) -> &[Vec<u8>] {
        &self.chunks
    }

    /// Consume the encoded payload.
    pub(crate) fn into_parts(self) -> (wire::PayloadManifest, Vec<Vec<u8>>) {
        (self.manifest, self.chunks)
    }
}

/// Result of admitting one authenticated chunk.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ChunkAdmission {
    /// A previously missing canonical chunk was persisted.
    Stored,
    /// The exact chunk was already durable.
    Duplicate,
}

/// Persistent reconstruction session for one immutable manifest.
#[derive(Debug)]
pub(crate) struct V2ChunkSession {
    directory: PathBuf,
    manifest: wire::PayloadManifest,
    chunks: Vec<Option<Vec<u8>>>,
}

impl V2ChunkSession {
    /// Open or create the exact manifest session and replay persisted chunks.
    ///
    /// Temporary files left by an interrupted atomic write are ignored. A
    /// malformed final manifest or chunk fails closed.
    pub(crate) fn open(
        root: impl AsRef<Path>,
        context: &wire::HeightContext,
        manifest: wire::PayloadManifest,
    ) -> Result<Self, V2ChunkError> {
        manifest.validate(context)?;
        let manifest_hash = HashOf::new(&manifest);
        let directory = root.as_ref().join(hex::encode(manifest_hash.as_ref()));
        fs::create_dir_all(&directory).map_err(|source| io_error(&directory, source))?;
        sync_directory(root.as_ref())?;

        let manifest_path = directory.join(MANIFEST_FILE);
        match fs::read(&manifest_path) {
            Ok(bytes) => {
                let mut cursor = bytes.as_slice();
                let recovered = StoredManifest::decode_all(&mut cursor)
                    .map_err(|error| V2ChunkError::ManifestDecode(error.to_string()))?;
                if recovered.version != STORE_VERSION {
                    return Err(V2ChunkError::UnsupportedStoreVersion(recovered.version));
                }
                if recovered.manifest != manifest {
                    return Err(V2ChunkError::ConflictingManifest);
                }
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                write_atomic_synced(
                    &manifest_path,
                    &StoredManifest {
                        version: STORE_VERSION,
                        manifest: manifest.clone(),
                    }
                    .encode(),
                )?;
            }
            Err(source) => return Err(io_error(&manifest_path, source)),
        }

        let chunk_count = manifest.chunk_hashes.len();
        let mut session = Self {
            directory,
            manifest,
            chunks: vec![None; chunk_count],
        };
        session.replay_chunks()?;
        Ok(session)
    }

    /// Borrow the immutable manifest.
    pub(crate) const fn manifest(&self) -> &wire::PayloadManifest {
        &self.manifest
    }

    /// Persist one structurally authenticated chunk.
    ///
    /// The caller is still responsible for verifying its sender signature.
    /// This boundary independently rechecks manifest identity, index, length,
    /// and content hash before any final file is created.
    pub(crate) fn admit(
        &mut self,
        chunk: &wire::PayloadChunk,
    ) -> Result<ChunkAdmission, V2ChunkError> {
        if chunk.manifest_hash != HashOf::new(&self.manifest) {
            return Err(V2ChunkError::ManifestMismatch);
        }
        self.admit_bytes(chunk.index, &chunk.bytes)
    }

    /// Persist already-authenticated bytes at an exact manifest index.
    pub(crate) fn admit_bytes(
        &mut self,
        index: u32,
        bytes: &[u8],
    ) -> Result<ChunkAdmission, V2ChunkError> {
        let index = usize::try_from(index).map_err(|_| V2ChunkError::ChunkIndexOutOfRange)?;
        self.validate_chunk(index, bytes)?;
        let path = self.chunk_path(index);
        let slot = self
            .chunks
            .get_mut(index)
            .ok_or(V2ChunkError::ChunkIndexOutOfRange)?;
        if let Some(existing) = slot {
            return if existing == bytes {
                Ok(ChunkAdmission::Duplicate)
            } else {
                Err(V2ChunkError::ConflictingChunk)
            };
        }

        write_atomic_synced(&path, bytes)?;
        *slot = Some(bytes.to_vec());
        Ok(ChunkAdmission::Stored)
    }

    /// Reconstruct and verify the canonical payload once enough chunks exist.
    ///
    /// RS16 reconstruction needs any `data_shards` chunks per stripe. Missing
    /// parity chunks are not materialized unless needed to recover data.
    pub(crate) fn reconstruct(&self) -> Result<Option<Vec<u8>>, V2ChunkError> {
        let payload = match self.manifest.layout.encoding {
            wire::PayloadEncoding::Plain => self.reconstruct_plain()?,
            wire::PayloadEncoding::ReedSolomon16 => self.reconstruct_rs16()?,
        };
        let Some(payload) = payload else {
            return Ok(None);
        };
        if u64::try_from(payload.len()).unwrap_or(u64::MAX) != self.manifest.payload_size_bytes
            || Hash::new(&payload) != self.manifest.subject.payload_hash
        {
            return Err(V2ChunkError::PayloadMismatch);
        }
        Ok(Some(payload))
    }

    /// Remove this transport session after a matching Kura receipt authorized
    /// higher-level retirement.
    pub(crate) fn retire(self) -> Result<(), V2ChunkError> {
        let parent = self.directory.parent().map(Path::to_path_buf);
        fs::remove_dir_all(&self.directory).map_err(|source| io_error(&self.directory, source))?;
        if let Some(parent) = parent {
            sync_directory(&parent)?;
        }
        Ok(())
    }

    fn replay_chunks(&mut self) -> Result<(), V2ChunkError> {
        for entry in
            fs::read_dir(&self.directory).map_err(|source| io_error(&self.directory, source))?
        {
            let entry = entry.map_err(|source| io_error(&self.directory, source))?;
            let path = entry.path();
            if !entry
                .file_type()
                .map_err(|source| io_error(&path, source))?
                .is_file()
            {
                return Err(V2ChunkError::UnexpectedEntry(path));
            }
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                return Err(V2ChunkError::UnexpectedEntry(path));
            };
            if name == MANIFEST_FILE || name.ends_with(".tmp") {
                continue;
            }
            let Some(index) = parse_chunk_file_name(name) else {
                return Err(V2ChunkError::UnexpectedEntry(path));
            };
            let bytes = fs::read(&path).map_err(|source| io_error(&path, source))?;
            self.validate_chunk(index, &bytes)?;
            let slot = self
                .chunks
                .get_mut(index)
                .ok_or(V2ChunkError::ChunkIndexOutOfRange)?;
            if slot.replace(bytes).is_some() {
                return Err(V2ChunkError::ConflictingChunk);
            }
        }
        Ok(())
    }

    fn validate_chunk(&self, index: usize, bytes: &[u8]) -> Result<(), V2ChunkError> {
        let expected_hash = self
            .manifest
            .chunk_hashes
            .get(index)
            .ok_or(V2ChunkError::ChunkIndexOutOfRange)?;
        let chunk_size = usize::try_from(self.manifest.layout.chunk_size_bytes)
            .map_err(|_| V2ChunkError::InvalidChunkLength)?;
        let expected_len = match self.manifest.layout.encoding {
            wire::PayloadEncoding::Plain => {
                let offset = index
                    .checked_mul(chunk_size)
                    .ok_or(V2ChunkError::InvalidChunkLength)?;
                let payload_size = usize::try_from(self.manifest.payload_size_bytes)
                    .map_err(|_| V2ChunkError::InvalidChunkLength)?;
                payload_size.saturating_sub(offset).min(chunk_size)
            }
            wire::PayloadEncoding::ReedSolomon16 => chunk_size,
        };
        if bytes.len() != expected_len || bytes.is_empty() {
            return Err(V2ChunkError::InvalidChunkLength);
        }
        if Hash::new(bytes) != *expected_hash {
            return Err(V2ChunkError::ChunkHashMismatch);
        }
        Ok(())
    }

    fn reconstruct_plain(&self) -> Result<Option<Vec<u8>>, V2ChunkError> {
        if self.chunks.iter().any(Option::is_none) {
            return Ok(None);
        }
        let payload_size = usize::try_from(self.manifest.payload_size_bytes)
            .map_err(|_| V2ChunkError::PayloadTooLarge)?;
        let mut payload = Vec::with_capacity(payload_size);
        for chunk in &self.chunks {
            payload.extend_from_slice(chunk.as_deref().expect("all chunks checked above"));
        }
        Ok(Some(payload))
    }

    fn reconstruct_rs16(&self) -> Result<Option<Vec<u8>>, V2ChunkError> {
        let data_shards = usize::from(self.manifest.layout.data_shards);
        let parity_shards = usize::from(self.manifest.layout.parity_shards);
        let stripe_width = data_shards
            .checked_add(parity_shards)
            .ok_or(V2ChunkError::InvalidErasureLayout)?;
        if stripe_width == 0 || !self.chunks.len().is_multiple_of(stripe_width) {
            return Err(V2ChunkError::InvalidErasureLayout);
        }
        let chunk_size = usize::try_from(self.manifest.layout.chunk_size_bytes)
            .map_err(|_| V2ChunkError::InvalidChunkLength)?;
        if !chunk_size.is_multiple_of(2) {
            return Err(V2ChunkError::InvalidErasureLayout);
        }
        let symbol_count = chunk_size / 2;
        let payload_size = usize::try_from(self.manifest.payload_size_bytes)
            .map_err(|_| V2ChunkError::PayloadTooLarge)?;
        let mut payload = Vec::with_capacity(payload_size);

        for stripe in self.chunks.chunks_exact(stripe_width) {
            if stripe.iter().filter(|chunk| chunk.is_some()).count() < data_shards {
                return Ok(None);
            }
            let mut symbols = stripe
                .iter()
                .map(|chunk| {
                    chunk
                        .as_deref()
                        .map(|bytes| rs16::symbols_from_chunk(symbol_count, bytes))
                })
                .collect::<Vec<_>>();
            rs16::reconstruct_shards(&mut symbols, data_shards, parity_shards)
                .map_err(|_| V2ChunkError::ReconstructionFailed)?;
            for shard in symbols.iter().take(data_shards) {
                let bytes = rs16::chunk_from_symbols(
                    shard.as_ref().ok_or(V2ChunkError::ReconstructionFailed)?,
                    chunk_size,
                )
                .map_err(|_| V2ChunkError::ReconstructionFailed)?;
                payload.extend_from_slice(&bytes);
            }
        }
        payload.truncate(payload_size);
        Ok(Some(payload))
    }

    fn chunk_path(&self, index: usize) -> PathBuf {
        self.directory.join(format!("{index:010}.chunk"))
    }
}

/// Encode exact canonical payload bytes using the height-frozen DA layout.
pub(crate) fn encode_payload(
    context: &wire::HeightContext,
    round: wire::ConsensusRound,
    subject: wire::BlockSubject,
    payload: &[u8],
) -> Result<EncodedV2Payload, V2ChunkError> {
    context.validate()?;
    if round.context_id != context.id()
        || round.height != context.height
        || Hash::new(payload) != subject.payload_hash
    {
        return Err(V2ChunkError::PayloadMismatch);
    }
    let payload_len = u64::try_from(payload.len()).map_err(|_| V2ChunkError::PayloadTooLarge)?;
    if payload.is_empty() || payload_len > context.da_layout.max_payload_size_bytes {
        return Err(V2ChunkError::PayloadTooLarge);
    }
    let chunks = match context.da_layout.encoding {
        wire::PayloadEncoding::Plain => encode_plain(payload, context.da_layout)?,
        wire::PayloadEncoding::ReedSolomon16 => encode_rs16(payload, context.da_layout)?,
    };
    let manifest = wire::PayloadManifest::derive(context, round, subject, payload_len, &chunks)?;
    Ok(EncodedV2Payload { manifest, chunks })
}

fn encode_plain(
    payload: &[u8],
    layout: wire::DataAvailabilityLayout,
) -> Result<Vec<Vec<u8>>, V2ChunkError> {
    let chunk_size =
        usize::try_from(layout.chunk_size_bytes).map_err(|_| V2ChunkError::InvalidChunkLength)?;
    if chunk_size == 0 {
        return Err(V2ChunkError::InvalidChunkLength);
    }
    Ok(payload.chunks(chunk_size).map(<[u8]>::to_vec).collect())
}

fn encode_rs16(
    payload: &[u8],
    layout: wire::DataAvailabilityLayout,
) -> Result<Vec<Vec<u8>>, V2ChunkError> {
    let chunk_size =
        usize::try_from(layout.chunk_size_bytes).map_err(|_| V2ChunkError::InvalidChunkLength)?;
    let data_shards = usize::from(layout.data_shards);
    let parity_shards = usize::from(layout.parity_shards);
    if chunk_size == 0 || !chunk_size.is_multiple_of(2) || data_shards == 0 || parity_shards == 0 {
        return Err(V2ChunkError::InvalidErasureLayout);
    }
    let data_chunk_count = payload.len().div_ceil(chunk_size);
    let stripe_count = data_chunk_count.div_ceil(data_shards);
    let stripe_width = data_shards
        .checked_add(parity_shards)
        .ok_or(V2ChunkError::InvalidErasureLayout)?;
    let mut encoded = Vec::with_capacity(
        stripe_count
            .checked_mul(stripe_width)
            .ok_or(V2ChunkError::PayloadTooLarge)?,
    );
    let symbol_count = chunk_size / 2;

    for stripe in 0..stripe_count {
        let mut data = Vec::with_capacity(data_shards);
        let mut symbols = Vec::with_capacity(data_shards);
        for within in 0..data_shards {
            let data_index = stripe
                .checked_mul(data_shards)
                .and_then(|base| base.checked_add(within))
                .ok_or(V2ChunkError::PayloadTooLarge)?;
            let offset = data_index
                .checked_mul(chunk_size)
                .ok_or(V2ChunkError::PayloadTooLarge)?;
            let mut chunk = vec![0_u8; chunk_size];
            if offset < payload.len() {
                let end = offset.saturating_add(chunk_size).min(payload.len());
                chunk[..end - offset].copy_from_slice(&payload[offset..end]);
            }
            symbols.push(rs16::symbols_from_chunk(symbol_count, &chunk));
            data.push(chunk);
        }
        let parity = rs16::encode_parity(&symbols, parity_shards)
            .map_err(|_| V2ChunkError::ReconstructionFailed)?;
        encoded.extend(data);
        for shard in parity {
            encoded.push(
                rs16::chunk_from_symbols(&shard, chunk_size)
                    .map_err(|_| V2ChunkError::ReconstructionFailed)?,
            );
        }
    }
    Ok(encoded)
}

fn parse_chunk_file_name(name: &str) -> Option<usize> {
    name.strip_suffix(".chunk")?.parse().ok()
}

fn write_atomic_synced(path: &Path, bytes: &[u8]) -> Result<(), V2ChunkError> {
    let parent = path.parent().ok_or_else(|| V2ChunkError::Io {
        path: path.to_path_buf(),
        source: io::Error::new(ErrorKind::InvalidInput, "path has no parent"),
    })?;
    fs::create_dir_all(parent).map_err(|source| io_error(parent, source))?;
    let tmp = path.with_extension(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map_or_else(|| "tmp".to_owned(), |extension| format!("{extension}.tmp")),
    );
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(&tmp)
        .map_err(|source| io_error(&tmp, source))?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .and_then(|()| file.sync_all())
        .map_err(|source| io_error(&tmp, source))?;
    fs::rename(&tmp, path).map_err(|source| io_error(path, source))?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<(), V2ChunkError> {
    if path.as_os_str().is_empty() {
        return Ok(());
    }
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| io_error(path, source))
}

fn io_error(path: &Path, source: io::Error) -> V2ChunkError {
    V2ChunkError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Deterministic chunk encoding, persistence, or reconstruction failure.
#[derive(Debug, Error)]
pub(crate) enum V2ChunkError {
    /// Manifest or height context failed canonical structural validation.
    #[error(transparent)]
    Wire(#[from] wire::ValidationError),
    /// Filesystem operation failed.
    #[error("Sumeragi v2 chunk store I/O failed at {path}: {source}")]
    Io {
        /// Affected path.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// Persisted manifest could not be decoded completely.
    #[error("persisted Sumeragi v2 manifest is malformed: {0}")]
    ManifestDecode(String),
    /// Persisted session layout version is unsupported.
    #[error("unsupported Sumeragi v2 chunk-store version {0}")]
    UnsupportedStoreVersion(u16),
    /// An existing session directory belongs to different manifest bytes.
    #[error("Sumeragi v2 chunk session manifest conflicts with the requested manifest")]
    ConflictingManifest,
    /// A payload or reconstructed body does not match its subject.
    #[error("Sumeragi v2 payload bytes do not match the manifest subject")]
    PayloadMismatch,
    /// Payload length is zero, over the height limit, or not representable.
    #[error("Sumeragi v2 payload length is outside the height limits")]
    PayloadTooLarge,
    /// Chunk referenced another manifest.
    #[error("Sumeragi v2 chunk references another manifest")]
    ManifestMismatch,
    /// Chunk index is outside the committed sequence.
    #[error("Sumeragi v2 chunk index is outside the manifest")]
    ChunkIndexOutOfRange,
    /// Chunk length differs from the layout-defined exact length.
    #[error("Sumeragi v2 chunk has an invalid length")]
    InvalidChunkLength,
    /// Chunk bytes do not match their committed hash.
    #[error("Sumeragi v2 chunk hash mismatch")]
    ChunkHashMismatch,
    /// A final chunk path already contains different bytes.
    #[error("Sumeragi v2 chunk conflicts with an existing durable chunk")]
    ConflictingChunk,
    /// RS16 layout arithmetic or profile is invalid.
    #[error("invalid Sumeragi v2 RS16 layout")]
    InvalidErasureLayout,
    /// Enough shards existed but deterministic RS16 recovery failed.
    #[error("Sumeragi v2 RS16 reconstruction failed")]
    ReconstructionFailed,
    /// Session directory contains an unrecognized final path.
    #[error("unexpected entry in Sumeragi v2 chunk store: {}", .0.display())]
    UnexpectedEntry(PathBuf),
}

#[cfg(test)]
mod tests {
    use iroha_crypto::{Algorithm, KeyPair};
    use iroha_data_model::{ChainId, block::BlockHeader, peer::PeerId};

    use super::*;

    fn context(encoding: wire::PayloadEncoding) -> wire::HeightContext {
        let mut roster = (1_u8..=4)
            .map(|seed| {
                let key = KeyPair::try_from_seed(vec![seed; 32], Algorithm::Ed25519)
                    .expect("deterministic key");
                wire::ValidatorPower {
                    validator: PeerId::new(key.public_key().clone()),
                    power: 1,
                }
            })
            .collect::<Vec<_>>();
        roster.sort_by(|left, right| left.validator.cmp(&right.validator));
        wire::HeightContext {
            chain_id: ChainId::from("sumeragi-v2-chunk-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 2,
            epoch: 0,
            epoch_end_height: u64::MAX,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: Some(parent_qc(&roster)),
            quorum: wire::DualQuorum::from_roster(&roster).expect("quorum"),
            roster,
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding,
                chunk_size_bytes: 8,
                data_shards: u16::from(encoding == wire::PayloadEncoding::ReedSolomon16) * 3,
                parity_shards: u16::from(encoding == wire::PayloadEncoding::ReedSolomon16) * 2,
                max_payload_size_bytes: 1024,
                max_chunk_count: 256,
            },
            leader_seed: [0x55; 32],
        }
    }

    fn parent_qc(roster: &[wire::ValidatorPower]) -> wire::QuorumCertificate {
        let parent_context = wire::HeightContext {
            chain_id: ChainId::from("sumeragi-v2-chunk-test"),
            protocol_version: wire::PROTOCOL_VERSION,
            height: 1,
            epoch: 0,
            epoch_end_height: u64::MAX,
            mode: wire::ConsensusMode::Permissioned,
            parent_commit_qc: None,
            quorum: wire::DualQuorum::from_roster(roster).expect("quorum"),
            roster: roster.to_vec(),
            nexus_amx_context_hash: Hash::new(b"nexus amx context"),
            da_layout: wire::DataAvailabilityLayout {
                encoding: wire::PayloadEncoding::Plain,
                chunk_size_bytes: 8,
                data_shards: 0,
                parity_shards: 0,
                max_payload_size_bytes: 1024,
                max_chunk_count: 256,
            },
            leader_seed: [0x55; 32],
        };
        let subject = wire::BlockSubject {
            parent_block_hash: None,
            block_hash: HashOf::from_untyped_unchecked(Hash::new(b"parent block")),
            payload_hash: Hash::new(b"parent payload"),
        };
        wire::QuorumCertificate {
            round: wire::ConsensusRound {
                context_id: parent_context.id(),
                height: 1,
                view: 0,
            },
            phase: wire::GlobalPhase::Commit,
            subject,
            signers: vec![0, 1, 2],
            aggregate_signature: vec![0xA5; 48],
        }
    }

    fn encode_fixture(
        encoding: wire::PayloadEncoding,
        payload: &[u8],
    ) -> (wire::HeightContext, EncodedV2Payload) {
        let context = context(encoding);
        let subject = wire::BlockSubject {
            parent_block_hash: context
                .parent_commit_qc
                .as_ref()
                .map(|qc| qc.subject.block_hash),
            block_hash: HashOf::<BlockHeader>::from_untyped_unchecked(Hash::new(b"block")),
            payload_hash: Hash::new(payload),
        };
        let round = wire::ConsensusRound {
            context_id: context.id(),
            height: context.height,
            view: 4,
        };
        let encoded = encode_payload(&context, round, subject, payload).expect("encode payload");
        (context, encoded)
    }

    #[test]
    fn plain_payload_roundtrips_and_reopens() {
        let payload = b"plain persistent payload";
        let (context, encoded) = encode_fixture(wire::PayloadEncoding::Plain, payload);
        let root = tempfile::tempdir().expect("tempdir");
        let mut session = V2ChunkSession::open(root.path(), &context, encoded.manifest.clone())
            .expect("open session");
        for (index, chunk) in encoded.chunks.iter().enumerate() {
            session
                .admit_bytes(u32::try_from(index).expect("index"), chunk)
                .expect("persist chunk");
        }
        assert_eq!(
            session.reconstruct().expect("reconstruct"),
            Some(payload.to_vec())
        );
        drop(session);

        let reopened =
            V2ChunkSession::open(root.path(), &context, encoded.manifest).expect("reopen session");
        assert_eq!(
            reopened.reconstruct().expect("reconstruct"),
            Some(payload.to_vec())
        );
    }

    #[test]
    fn rs16_recovers_missing_data_from_any_quorum_per_stripe() {
        let payload = b"RS16 payload spanning more than one deterministic stripe";
        let (context, encoded) = encode_fixture(wire::PayloadEncoding::ReedSolomon16, payload);
        let width = usize::from(context.da_layout.data_shards + context.da_layout.parity_shards);
        for first_missing in 0..width {
            for second_missing in first_missing + 1..width {
                let root = tempfile::tempdir().expect("tempdir");
                let mut session =
                    V2ChunkSession::open(root.path(), &context, encoded.manifest.clone())
                        .expect("open session");
                for (index, chunk) in encoded.chunks.iter().enumerate() {
                    let within = index % width;
                    if within == first_missing || within == second_missing {
                        continue;
                    }
                    session
                        .admit_bytes(u32::try_from(index).expect("index"), chunk)
                        .expect("persist shard");
                }
                assert_eq!(
                    session.reconstruct().expect("reconstruct"),
                    Some(payload.to_vec()),
                    "failed with missing shard positions {first_missing} and {second_missing}"
                );
            }
        }
    }

    #[test]
    fn corruption_conflicts_and_insufficient_shards_are_rejected_or_pending() {
        let payload = b"adversarial chunk payload";
        let (context, encoded) = encode_fixture(wire::PayloadEncoding::ReedSolomon16, payload);
        let root = tempfile::tempdir().expect("tempdir");
        let mut session = V2ChunkSession::open(root.path(), &context, encoded.manifest.clone())
            .expect("open session");
        let mut corrupt = encoded.chunks[0].clone();
        corrupt[0] ^= 0x80;
        assert!(matches!(
            session.admit_bytes(0, &corrupt),
            Err(V2ChunkError::ChunkHashMismatch)
        ));
        session
            .admit_bytes(0, &encoded.chunks[0])
            .expect("persist canonical chunk");
        assert_eq!(session.reconstruct().expect("pending reconstruction"), None);

        let path = session.chunk_path(0);
        fs::write(&path, &corrupt).expect("inject final-path corruption");
        drop(session);
        assert!(matches!(
            V2ChunkSession::open(root.path(), &context, encoded.manifest),
            Err(V2ChunkError::ChunkHashMismatch)
        ));
    }

    #[test]
    fn encoding_is_deterministic_and_subject_bound() {
        let payload = b"same payload";
        let (context, first) = encode_fixture(wire::PayloadEncoding::ReedSolomon16, payload);
        let second = encode_payload(
            &context,
            first.manifest.round,
            first.manifest.subject,
            payload,
        )
        .expect("repeat encoding");
        assert_eq!(first, second);

        let mut wrong = first.manifest.subject;
        wrong.payload_hash = Hash::new(b"another payload");
        assert!(matches!(
            encode_payload(&context, first.manifest.round, wrong, payload),
            Err(V2ChunkError::PayloadMismatch)
        ));
    }
}
