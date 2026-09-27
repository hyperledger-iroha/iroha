//! Native payload verification without materialising a second payload or complete CAR archive.
use crate::{
    CarBuildPlan, CarStreamingWriter, CarWriteError, CarWriteStats, ChunkStoreError, PorMerkleTree,
};
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
/// The complete CAR is hashed as the canonical writer emits it into `io::sink()`. PoR keeps one
/// chunk subtree and a logarithmic Merkle frontier, releasing each subtree before reading the
/// next chunk. The caller must authenticate the manifest before invoking this function. The
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

fn por_commitment<R: Read>(
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
    let mut frontier = [None; crate::POR_CHUNK_MERKLE_MAX_DEPTH + 1];
    let mut leaf_count = 0u64;
    let mut payload_hasher = blake3::Hasher::new();
    for (index, chunk) in plan.chunks.iter().enumerate() {
        bytes.resize(chunk.length as usize, 0);
        reader.read_exact(&mut bytes)?;
        if blake3::hash(&bytes).as_bytes() != &chunk.digest {
            return Err(ChunkStoreError::DigestMismatch { chunk_index: index }.into());
        }
        payload_hasher.update(&bytes);
        let (_, mut node, next_leaf_count) = PorMerkleTree::build_chunk_tree_from_bytes(
            index,
            chunk.offset,
            chunk.length,
            chunk.digest,
            leaf_count,
            &bytes,
        )?;
        leaf_count = next_leaf_count;
        let mut level = 0usize;
        let mut node_index = index as u64;
        while let Some(left) = frontier[level].take() {
            node = crate::hash_chunk_node(level as u32, node_index / 2, &left, &node);
            node_index /= 2;
            level += 1;
        }
        frontier[level] = Some(node);
    }
    let mut trailing = [0u8; 1];
    if reader.read(&mut trailing)? != 0 || payload_hasher.finalize() != plan.payload_digest {
        return Err(PayloadVerifyError::Commitment("complete payload"));
    }
    if plan.chunks.is_empty() {
        return Ok(([0; 32], 0));
    }
    let mut carry: Option<([u8; 32], usize, u64)> = None;
    for (level, left) in frontier.into_iter().enumerate() {
        let Some(left) = left else { continue };
        carry = Some(match carry {
            None => (left, level, (plan.chunks.len() as u64 - 1) >> level),
            Some((mut right, mut right_level, mut index)) => {
                while right_level < level {
                    right = crate::hash_chunk_node(right_level as u32, index / 2, &right, &right);
                    right_level += 1;
                    index /= 2;
                }
                (
                    crate::hash_chunk_node(level as u32, index / 2, &left, &right),
                    level + 1,
                    index / 2,
                )
            }
        });
    }
    let root = crate::hash_root(
        plan.content_length,
        plan.chunks.len(),
        leaf_count,
        &carry.expect("non-empty plan has a Merkle frontier").0,
    )?;
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
