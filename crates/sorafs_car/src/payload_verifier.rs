//! Native payload verification without materialising a second payload or complete CAR archive.
use crate::{CarBuildPlan, CarStreamingWriter, CarWriteError, CarWriteStats, ChunkStoreError};
use sorafs_chunker::por::PorRootBuilder;
use sorafs_manifest::ManifestV1;
use std::io::{self, Read, Seek, SeekFrom};
use thiserror::Error;

/// Canonical commitments reproduced from a complete payload stream.
#[derive(Debug)]
pub struct PayloadVerification {
    /// Statistics from the canonical CAR writer, which writes into a non-retaining sink.
    pub stats: CarWriteStats,
    /// Number of PoR leaves authenticated by the reproduced root.
    pub por_leaf_count: usize,
}

/// Failure to reproduce one native manifest commitment.
#[derive(Debug, Error)]
pub enum PayloadVerifyError {
    /// A public manifest commitment differs from the native plan or payload.
    #[error("payload manifest commitment mismatch: {0}")]
    Commitment(&'static str),
    /// The canonical writer rejected the plan, bytes, or exact EOF.
    #[error("canonical payload writer failed: {0}")]
    Writer(#[from] CarWriteError),
    /// PoR reconstruction rejected the chunk geometry or bytes.
    #[error("payload PoR reconstruction failed: {0}")]
    Por(#[from] ChunkStoreError),
    /// The payload spool could not be read or rewound.
    #[error("payload spool I/O failed: {0}")]
    Io(#[from] io::Error),
}

/// Reproduce every native manifest commitment using two bounded passes over a payload spool.
///
/// The complete CAR is hashed as the canonical writer emits it into `io::sink()`. PoR reads one
/// chunk at a time and retains a logarithmic Merkle frontier. The caller must authenticate the manifest before invoking this function. The
/// reader is rewound on success; neither the payload nor a CAR copy is retained here.
pub fn verify_payload_reader<R: Read + Seek>(
    manifest: &ManifestV1,
    plan: &CarBuildPlan,
    reader: &mut R,
) -> Result<PayloadVerification, PayloadVerifyError> {
    plan.verify_manifest_metadata(manifest)?;
    if crate::verifier::chunk_profile_from_manifest(manifest).ok() != Some(plan.chunk_profile) {
        return Err(PayloadVerifyError::Commitment("chunk profile"));
    }
    reader.rewind()?;
    let stats = CarStreamingWriter::new(plan).write_from_reader(reader, io::sink())?;
    if stats.root_cids.as_slice() != [manifest.root_cid.clone()]
        || stats.dag_codec != manifest.dag_codec.0
        || stats.car_size != manifest.car_size
        || stats.car_archive_digest.as_bytes() != &manifest.car_digest
    {
        return Err(PayloadVerifyError::Commitment("canonical CAR"));
    }
    reader.rewind()?;
    let (por_root, por_leaf_count) = por_commitment(plan, reader)?;
    if por_root != manifest.por_root {
        return Err(PayloadVerifyError::Commitment("PoR root"));
    }
    reader.rewind()?;
    Ok(PayloadVerification {
        stats,
        por_leaf_count,
    })
}

pub(crate) fn por_commitment<R: Read>(
    plan: &CarBuildPlan,
    reader: &mut R,
) -> Result<([u8; 32], usize), PayloadVerifyError> {
    let validation = plan.validate().map_err(ChunkStoreError::from)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(validation.max_chunk_len())
        .map_err(|_| ChunkStoreError::AllocationFailed {
            context: "streamed PoR chunk",
            requested: validation.max_chunk_len(),
        })?;
    let mut builder = PorRootBuilder::default();
    let mut payload_hasher = blake3::Hasher::new();
    for (index, chunk) in plan.chunks.iter().enumerate() {
        bytes.resize(chunk.length as usize, 0);
        reader.read_exact(&mut bytes)?;
        let digest = builder
            .push_chunk(&bytes)
            .ok_or(ChunkStoreError::PorInvariant {
                context: "streamed PoR chunk extent",
            })?;
        if digest != chunk.digest {
            return Err(ChunkStoreError::DigestMismatch { chunk_index: index }.into());
        }
        payload_hasher.update(&bytes);
    }
    let mut trailing = [0u8; 1];
    if reader.read(&mut trailing)? != 0 || payload_hasher.finalize() != plan.payload_digest {
        return Err(PayloadVerifyError::Commitment("complete payload"));
    }
    if plan.chunks.is_empty() {
        return Ok(([0; 32], 0));
    }
    let leaf_count = builder.leaf_count();
    let root = builder.finish().ok_or(ChunkStoreError::PorInvariant {
        context: "streamed PoR root",
    })?;
    let leaf_count =
        usize::try_from(leaf_count).map_err(|_| ChunkStoreError::PorCountOverflow {
            context: "streamed PoR leaf count",
        })?;
    Ok((root, leaf_count))
}

/// Borrowed reader over eager verified chunks; rewinding never concatenates them.
pub struct ChunkPayloadReader<'a> {
    chunks: &'a [Vec<u8>],
    chunk: usize,
    offset: usize,
}
impl<'a> ChunkPayloadReader<'a> {
    /// Open a borrowed complete payload reader at byte zero.
    pub fn new(chunks: &'a [Vec<u8>]) -> Self {
        Self {
            chunks,
            chunk: 0,
            offset: 0,
        }
    }
}
impl Read for ChunkPayloadReader<'_> {
    fn read(&mut self, output: &mut [u8]) -> io::Result<usize> {
        while let Some(chunk) = self.chunks.get(self.chunk) {
            if self.offset == chunk.len() {
                self.chunk += 1;
                self.offset = 0;
                continue;
            }
            let count = output.len().min(chunk.len() - self.offset);
            output[..count].copy_from_slice(&chunk[self.offset..self.offset + count]);
            self.offset += count;
            return Ok(count);
        }
        Ok(0)
    }
}
impl Seek for ChunkPayloadReader<'_> {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        if position != SeekFrom::Start(0) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk readers support only rewind",
            ));
        }
        self.chunk = 0;
        self.offset = 0;
        Ok(0)
    }
}

#[cfg(test)]
mod tests;
