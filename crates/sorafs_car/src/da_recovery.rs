//! Recover an application blob from the RS16 inventory of its authenticated DA manifest.
//! A source may supply any subset of the manifest's indexed data and parity chunks.

use crate::{PlanFromManifestError, build_plan_from_da_manifest};
use iroha_data_model::da::manifest::{ChunkCommitment, ChunkRole, DaManifestV1};
use iroha_primitives::erasure::rs16::compact::CompactShape;
use std::io;
use thiserror::Error;

/// Recovery failed before a complete, digest-verified payload could be returned.
#[derive(Debug, Error)]
pub enum RecoveryError {
    /// The manifest did not describe the canonical producer-ordered shard inventory.
    #[error(transparent)]
    Manifest(#[from] PlanFromManifestError),
    /// An input exceeded the first-release blob or erasure resource bounds.
    #[error("DA reconstruction exceeds the supported payload or RS16 geometry limits")]
    Geometry,
    /// Reading an actual shard failed for a reason other than its absence or corruption.
    #[error("failed to read DA chunk {index}: {source}")]
    Read {
        /// Canonical manifest index.
        index: u32,
        /// Original input error.
        source: io::Error,
    },
    /// Bounded working storage could not be reserved.
    #[error("could not reserve memory for DA reconstruction")]
    Allocation,
    /// Neither direction of the product code could supply the remaining missing data.
    #[error("insufficient verified RS16 shards to recover the original DA payload")]
    InsufficientShards,
    /// The committed shards did not form a consistent RS16 codeword.
    #[error("reconstructed RS16 shard differs from its manifest commitment or canonical padding")]
    Codeword,
    /// The complete blob or its streaming PoR verification differed from the manifest.
    #[error(transparent)]
    Verification(#[from] crate::payload_verifier::PayloadVerifyError),
    /// The reconstructed payload length or original chunk root differed from the manifest.
    #[error("reconstructed payload differs from the manifest length or PoR chunk root")]
    Payload,
}

// These bounds cover every synchronous V1 Torii blob. Physical storage comprises at most the
// input inventory, one bounded RS16 line and its result, and the final 64 MiB payload.
const MAX_PAYLOAD: u64 = 64 << 20;
const MAX_PARITY: u64 = 128 << 20;
const MAX_CHUNK: u32 = 2 << 20;
const MAX_SHARDS: usize = 64;
const MAX_COMMITMENTS: usize = 16 << 10;

fn filled<T: Clone>(len: usize, value: T) -> Result<Vec<T>, RecoveryError> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(len)
        .map_err(|_| RecoveryError::Allocation)?;
    result.resize(len, value);
    Ok(result)
}

struct Cell<'a> {
    // The final source stripe has implicit all-zero data rows without commitments.
    commitment: Option<&'a ChunkCommitment>,
    bytes: Option<Vec<u8>>,
}

impl Cell<'_> {
    fn present(&self) -> bool {
        self.commitment.is_none() || self.bytes.is_some()
    }

    fn fetch(
        &mut self,
        size: usize,
        read: &mut impl FnMut(&ChunkCommitment) -> io::Result<Option<Vec<u8>>>,
    ) -> Result<(), RecoveryError> {
        let Some(chunk) = self.commitment else {
            return Ok(());
        };
        let Some(mut bytes) = read(chunk).map_err(|source| RecoveryError::Read {
            index: chunk.index,
            source,
        })?
        else {
            return Ok(());
        };
        if bytes.len() == chunk.length as usize
            && blake3::hash(&bytes).as_bytes() == chunk.commitment.as_ref()
        {
            bytes
                .try_reserve_exact(size - bytes.len())
                .map_err(|_| RecoveryError::Allocation)?;
            bytes.resize(size, 0);
            self.bytes = Some(bytes);
        }
        Ok(())
    }

    fn matches(&self, padded: &[u8]) -> bool {
        let Some(chunk) = self.commitment else {
            return padded.iter().all(|byte| *byte == 0);
        };
        let (bytes, padding) = padded.split_at(chunk.length as usize);
        padding.iter().all(|byte| *byte == 0)
            && blake3::hash(bytes).as_bytes() == chunk.commitment.as_ref()
    }
}

/// Load a canonical blob by manifest index. The caller authenticates the manifest and supplies
/// length-bounded shard bodies. Missing, wrong-length and digest-invalid bodies are erasures.
/// Data is requested first. When necessary, verified global and cross-stripe parity are used
/// iteratively until the original payload can be checked against its BLAKE3 commitment.
///
/// # Errors
/// Returns a typed error on invalid geometry, source I/O, allocation refusal, inconsistent
/// commitments, insufficient recoverable rows, or a final blob digest mismatch.
pub fn recover_payload(
    manifest: &DaManifestV1,
    mut read: impl FnMut(&ChunkCommitment) -> io::Result<Option<Vec<u8>>>,
) -> Result<Vec<u8>, RecoveryError> {
    let k = usize::from(manifest.erasure_profile.data_shards);
    let m = usize::from(manifest.erasure_profile.parity_shards);
    let extra = usize::from(manifest.erasure_profile.row_parity_stripes);
    if manifest.total_size > MAX_PAYLOAD
        || manifest.chunk_size > MAX_CHUNK
        || manifest.chunk_size < 2
        || !manifest.chunk_size.is_multiple_of(2)
        || !(1..=MAX_SHARDS).contains(&k)
        || !(1..=MAX_SHARDS).contains(&m)
        || extra > MAX_SHARDS
        || manifest.chunks.len() > MAX_COMMITMENTS
    {
        return Err(RecoveryError::Geometry);
    }
    let plan = build_plan_from_da_manifest(manifest)?;
    let data_count = plan.chunks.len();
    let stripes = data_count.div_ceil(k);
    if data_count > 1024
        || (extra > 0 && stripes > MAX_SHARDS)
        || (manifest.chunks.len() - data_count) as u64 * u64::from(manifest.chunk_size) > MAX_PARITY
    {
        return Err(RecoveryError::Geometry);
    }
    let size = manifest.chunk_size as usize;
    let width = k + m;
    let height = stripes + extra;
    let mut cells: Vec<_> = (0..width * height)
        .map(|_| Cell {
            commitment: None,
            bytes: None,
        })
        .collect();
    // The producer emits each source stripe in data/parity order, followed by column-major
    // cross-stripe commitments. The RS16 matrix itself is row-major, including implicit zeros.
    let mut inventory = manifest.chunks.iter();
    for (n, stripe) in cells.chunks_exact_mut(width).take(stripes).enumerate() {
        for cell in stripe.iter_mut().take((data_count - n * k).min(k)) {
            cell.commitment = inventory.next();
        }
        for cell in &mut stripe[k..] {
            cell.commitment = inventory.next();
        }
    }
    for column in 0..width {
        for row in stripes..height {
            cells[row * width + column].commitment = inventory.next();
        }
    }
    let missing_data = |cells: &[Cell<'_>]| {
        cells.iter().any(|cell| {
            cell.commitment
                .is_some_and(|chunk| chunk.role == ChunkRole::Data)
                && !cell.present()
        })
    };
    for stripe in cells.chunks_exact_mut(width).take(stripes) {
        for cell in &mut stripe[..k] {
            cell.fetch(size, &mut read)?;
        }
    }
    if missing_data(&cells) {
        let zero = filled(size, 0_u8)?;
        // A healthy stripe requires no gateway parity reads. An incomplete stripe
        // needs only enough verified rows to recover its k original source rows.
        for row in 0..stripes {
            let start = row * width;
            let stripe = &mut cells[start..start + width];
            let mut missing = stripe[..k].iter().filter(|cell| !cell.present()).count();
            if missing == 0 {
                continue;
            }
            for parity in &mut stripe[k..] {
                parity.fetch(size, &mut read)?;
                if parity.present() {
                    missing -= 1;
                    if missing == 0 {
                        break;
                    }
                }
            }
            if missing == 0 {
                let indexes: Vec<_> = (start..start + width).collect();
                repair_line(&mut cells, &indexes, k, &zero)?;
            }
        }
        if missing_data(&cells) {
            for cell in &mut cells[stripes * width..] {
                cell.fetch(size, &mut read)?;
            }
        }
        while missing_data(&cells) {
            let mut progressed = false;
            for row in 0..height {
                let indexes: Vec<_> = (row * width..(row + 1) * width).collect();
                progressed |= repair_line(&mut cells, &indexes, k, &zero)?;
            }
            if extra != 0 && missing_data(&cells) {
                for column in 0..width {
                    let indexes: Vec<_> = (column..cells.len()).step_by(width).collect();
                    progressed |= repair_line(&mut cells, &indexes, stripes, &zero)?;
                }
            }
            if !progressed {
                return Err(RecoveryError::InsufficientShards);
            }
        }
    }

    let mut payload = Vec::new();
    payload
        .try_reserve_exact(manifest.total_size as usize)
        .map_err(|_| RecoveryError::Allocation)?;
    for cell in cells {
        if let Some(chunk) = cell
            .commitment
            .filter(|chunk| chunk.role == ChunkRole::Data)
        {
            let bytes = cell.bytes.ok_or(RecoveryError::InsufficientShards)?;
            payload.extend_from_slice(&bytes[..chunk.length as usize]);
        }
    }
    let (chunk_root, _) = crate::payload_verifier::por_commitment(&plan, &mut payload.as_slice())?;
    if payload.len() as u64 != manifest.total_size || &chunk_root != manifest.chunk_root.as_ref() {
        return Err(RecoveryError::Payload);
    }
    Ok(payload)
}

fn repair_line(
    cells: &mut [Cell<'_>],
    indexes: &[usize],
    k: usize,
    zero: &[u8],
) -> Result<bool, RecoveryError> {
    let present = indexes
        .iter()
        .filter(|&&index| cells[index].present())
        .count();
    if present < k || present == indexes.len() {
        return Ok(false);
    }
    let size = zero.len();
    let shape = CompactShape::new(k * size, k, indexes.len() - k, size)
        .map_err(|_| RecoveryError::Geometry)?;
    let received: Vec<_> = indexes
        .iter()
        .map(|&index| {
            let cell = &cells[index];
            if cell.commitment.is_none() {
                Some(zero)
            } else {
                cell.bytes.as_deref()
            }
        })
        .collect();
    let mut codeword = filled(shape.encoded_bytes(), 0)?;
    shape
        .reconstruct_into(
            &received,
            &mut filled(shape.payload_bytes(), 0)?,
            &mut codeword,
            &mut filled(shape.workspace_words(), 0)?,
            |position, bytes| cells[indexes[position]].matches(bytes),
        )
        .map_err(|_| RecoveryError::Codeword)?;
    for (&index, recovered) in indexes.iter().zip(codeword.chunks_exact(size)) {
        if !cells[index].present() {
            let mut bytes = Vec::new();
            bytes
                .try_reserve_exact(size)
                .map_err(|_| RecoveryError::Allocation)?;
            bytes.extend_from_slice(recovered);
            cells[index].bytes = Some(bytes);
        }
    }
    Ok(true)
}
