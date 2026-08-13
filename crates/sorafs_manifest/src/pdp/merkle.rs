//! Canonical Merkle construction for Sora-PDP v1.
use thiserror::Error;
use super::{
    PDP_HOT_LEAF_SIZE_V1, PDP_HOT_LEAVES_PER_SEGMENT_V1, PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1,
    PDP_MAX_SEGMENT_SAMPLES_V1, PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1, PDP_SEGMENT_SIZE_V1,
    PdpHotLeafProofV1, PdpProofLeafV1, PdpSampleV1,
};
const HOT_LEAF_DOMAIN_V1: &[u8] = b"sorafs.pdp.hot-leaf.v1\0";
const SEGMENT_HOT_NODE_DOMAIN_V1: &[u8] = b"sorafs.pdp.segment-hot-node.v1\0";
const SEGMENT_LEAF_DOMAIN_V1: &[u8] = b"sorafs.pdp.segment-leaf.v1\0";
const HOT_NODE_DOMAIN_V1: &[u8] = b"sorafs.pdp.hot-node.v1\0";
const SEGMENT_NODE_DOMAIN_V1: &[u8] = b"sorafs.pdp.segment-node.v1\0";
const HOT_ROOT_DOMAIN_V1: &[u8] = b"sorafs.pdp.hot-root.v1\0";
const SEGMENT_ROOT_DOMAIN_V1: &[u8] = b"sorafs.pdp.segment-root.v1\0";
/// Canonical authentication-path failure.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PdpMerklePathError {
    /// A tree with zero leaves cannot have an authentication path.
    #[error("Merkle tree leaf count must be non-zero")]
    EmptyTree,
    /// The requested leaf is outside the committed tree.
    #[error("Merkle leaf index {index} is outside leaf count {count}")]
    IndexOutOfRange {
        /// Requested leaf index.
        index: u64,
        /// Number of leaves in the tree.
        count: u64,
    },
    /// The authentication path does not have the unique depth implied by the leaf count.
    #[error("Merkle path depth {actual} does not match expected depth {expected}")]
    DepthMismatch {
        /// Required number of sibling hashes.
        expected: usize,
        /// Supplied number of sibling hashes.
        actual: usize,
    },
    /// An odd terminal node did not use canonical self-duplication.
    #[error("Merkle path level {level} has noncanonical odd-node padding")]
    NonCanonicalOddSibling {
        /// Zero-based authentication-path level.
        level: usize,
    },
    /// The supplied path cannot be represented by the fixed-width node hash format.
    #[error("Merkle path depth exceeds the v1 u16 level representation")]
    DepthOverflow,
}
/// Failures while constructing a canonical PDP tree or extracting witnesses.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum PdpMerkleTreeError {
    /// PDP commitments require a non-empty payload.
    #[error("PDP payload must not be empty")]
    EmptyPayload,
    /// Host geometry could not be represented by the v1 wire schema.
    #[error("PDP payload geometry exceeds the v1 representation")]
    GeometryOverflow,
    /// Memory required for the canonical tree could not be reserved.
    #[error("memory allocation for the PDP Merkle tree failed")]
    AllocationFailed,
    /// Proof construction received bytes for a different payload.
    #[error("PDP proof payload length {actual} does not match tree length {expected}")]
    PayloadLengthMismatch {
        /// Tree payload length.
        expected: u64,
        /// Supplied payload length.
        actual: u64,
    },
    /// Proof construction requires at least one sampled segment.
    #[error("PDP proof sample set must not be empty")]
    EmptySampleSet,
    /// A proof request exceeded the protocol's sampled-segment bound.
    #[error(
        "PDP proof request has {found} segment samples; maximum is {PDP_MAX_SEGMENT_SAMPLES_V1}"
    )]
    TooManySegmentSamples {
        /// Number of segment samples supplied by the caller.
        found: usize,
    },
    /// A sampled segment did not request any hot leaves.
    #[error("PDP segment {segment_index} has an empty hot-leaf sample set")]
    EmptyHotLeafSet {
        /// Segment whose sample set was empty.
        segment_index: u64,
    },
    /// One segment requested more hot leaves than the protocol permits.
    #[error(
        "PDP segment {segment_index} requests {found} hot leaves; maximum is {PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1}"
    )]
    TooManyHotLeaves {
        /// Segment whose sample set exceeded the bound.
        segment_index: u64,
        /// Number of hot leaves requested from the segment.
        found: usize,
    },
    /// The complete request exceeded the protocol's hot-leaf witness bound.
    #[error(
        "PDP proof request has {found} hot-leaf samples; maximum is {PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1}"
    )]
    TooManyHotLeavesTotal {
        /// Total number of requested hot-leaf witnesses.
        found: usize,
    },
    /// Segment samples were not strictly increasing.
    #[error("PDP proof segment samples must be strictly increasing and unique")]
    NonCanonicalSegmentOrder,
    /// Hot-leaf indices in one segment were not strictly increasing.
    #[error(
        "PDP hot-leaf samples for segment {segment_index} must be strictly increasing and unique"
    )]
    NonCanonicalHotLeafOrder {
        /// Segment containing the noncanonical sample sequence.
        segment_index: u64,
    },
    /// A challenge referenced a segment outside the tree.
    #[error("PDP segment index {segment_index} is outside the commitment tree")]
    SegmentOutOfRange {
        /// Rejected segment index.
        segment_index: u64,
    },
    /// A challenge referenced a hot leaf outside its segment.
    #[error("PDP hot leaf index {leaf_index} is outside segment {segment_index}")]
    HotLeafOutOfRange {
        /// Segment containing the rejected leaf index.
        segment_index: u64,
        /// Rejected segment-local leaf index.
        leaf_index: u16,
    },
    /// Payload bytes no longer match the tree used to seal the commitment.
    #[error("PDP sampled bytes do not match segment {segment_index} hot leaf {leaf_index}")]
    PayloadDigestMismatch {
        /// Segment containing the changed bytes.
        segment_index: u64,
        /// Segment-local hot-leaf index.
        leaf_index: u16,
    },
    /// Private tree layers were internally inconsistent.
    #[error("PDP Merkle tree internal layers are inconsistent")]
    CorruptTree,
}
/// Failure while extracting PDP witnesses from random-access payload storage.
///
/// Tree and request validation failures are separated from storage failures so
/// callers can distinguish malformed challenges from an unavailable payload.
#[derive(Debug, Error)]
pub enum PdpMerkleReadError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    /// The tree or requested sample set was invalid.
    #[error(transparent)]
    Tree(#[from] PdpMerkleTreeError),
    /// The random-access reader failed.
    #[error("PDP payload read failed at offset {offset} for {length} bytes: {source}")]
    ReadFailed {
        /// Absolute payload byte offset passed to the reader.
        offset: u64,
        /// Exact number of bytes requested.
        length: u32,
        /// Storage-specific read failure.
        #[source]
        source: E,
    },
    /// The reader did not return exactly the requested number of bytes.
    #[error(
        "PDP payload reader returned {actual} bytes at offset {offset}; expected exactly {expected}"
    )]
    ReadLengthMismatch {
        /// Absolute payload byte offset passed to the reader.
        offset: u64,
        /// Exact number of bytes required by the committed leaf.
        expected: usize,
        /// Number of bytes reported by the reader.
        actual: usize,
    },
}
const HOT_LEAVES_PER_SEGMENT_USIZE_V1: usize = PDP_HOT_LEAVES_PER_SEGMENT_V1 as usize;
/// Estimate the retained heap capacity of a canonical PDP tree.
///
/// The estimate covers the two exact boxed node slabs retained by the finished
/// tree. The builder's temporary 256 KiB segment buffer and the bounded
/// temporary local tree used during proof extraction are not retained by the
/// finished tree and are therefore excluded.
///
/// This function performs no allocation. An empty payload returns
/// [`PdpMerkleTreeError::EmptyPayload`], geometry or `usize` arithmetic
/// overflow returns [`PdpMerkleTreeError::GeometryOverflow`], and a single
/// required allocation larger than the host's `isize` allocation limit returns
/// [`PdpMerkleTreeError::AllocationFailed`].
pub fn estimated_heap_bytes(payload_len: u64) -> Result<usize, PdpMerkleTreeError> {
    if payload_len == 0 {
        return Err(PdpMerkleTreeError::EmptyPayload);
    }
    let hot_leaf_count = usize::try_from(hot_leaf_count_for_payload(payload_len))
        .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
    let segment_count = usize::try_from(segment_count_for_payload(payload_len))
        .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
    let hot_node_count = merkle_total_node_count(hot_leaf_count)?;
    let segment_node_count = merkle_total_node_count(segment_count)?;
    checked_allocation_bytes::<[u8; 32]>(hot_node_count)?
        .checked_add(checked_allocation_bytes::<[u8; 32]>(segment_node_count)?)
        .ok_or(PdpMerkleTreeError::GeometryOverflow)
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SegmentGeometryV1 {
    index: u64,
    offset: u64,
    length: u32,
    hot_leaf_start: usize,
    hot_leaf_count: usize,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HotLeafGeometryV1 {
    global_index: u64,
    offset: u64,
    length: u32,
}
/// In-memory canonical PDP tree built over global 4 KiB hot leaves and 256 KiB segments.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdpMerkleTreeV1 {
    payload_len: u64,
    hot_nodes: Box<[[u8; 32]]>,
    segment_nodes: Box<[[u8; 32]]>,
    hot_root: [u8; 32],
    segment_root: [u8; 32],
}
/// Incremental constructor for a canonical Sora-PDP v1 Merkle tree.
///
/// The builder retains at most one 256 KiB payload segment while ingesting
/// bytes. Final trees retain only two exact global Merkle node slabs; all byte
/// geometry is derived from `payload_len` and a sampled segment's bounded
/// 64-leaf local tree is rebuilt when a proof is extracted. All allocations
/// made while constructing the tree are fallible; callers must discard the
/// builder after an error.
#[derive(Debug, Default)]
pub struct PdpMerkleTreeBuilderV1 {
    payload_len: u64,
    pending_segment: Vec<u8>,
    hot_leaf_digests: Vec<[u8; 32]>,
    segment_commitments: Vec<[u8; 32]>,
}
impl PdpMerkleTreeBuilderV1 {
    /// Create an empty streaming tree builder.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
    /// Number of payload bytes successfully accepted so far.
    #[must_use]
    pub fn payload_len(&self) -> u64 {
        self.payload_len
    }
    /// Add the next contiguous payload bytes to the canonical tree.
    ///
    /// Empty updates are no-ops. The cumulative payload length is checked
    /// before any input is consumed. If this method returns an error, callers
    /// must discard the builder rather than retrying the same input.
    pub fn update(&mut self, mut bytes: &[u8]) -> Result<(), PdpMerkleTreeError> {
        let input_len =
            u64::try_from(bytes.len()).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let expected_payload_len = self
            .payload_len
            .checked_add(input_len)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        if !bytes.is_empty() && self.pending_segment.capacity() < PDP_SEGMENT_SIZE_V1 as usize {
            let additional = (PDP_SEGMENT_SIZE_V1 as usize)
                .checked_sub(self.pending_segment.len())
                .ok_or(PdpMerkleTreeError::CorruptTree)?;
            try_reserve_exact(&mut self.pending_segment, additional)?;
        }
        while !bytes.is_empty() {
            let available = (PDP_SEGMENT_SIZE_V1 as usize)
                .checked_sub(self.pending_segment.len())
                .ok_or(PdpMerkleTreeError::CorruptTree)?;
            if available == 0 {
                self.finalize_pending_segment()?;
                continue;
            }
            let take = available.min(bytes.len());
            self.pending_segment.extend_from_slice(&bytes[..take]);
            self.payload_len = self
                .payload_len
                .checked_add(u64::try_from(take).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            bytes = &bytes[take..];
            if self.pending_segment.len() == PDP_SEGMENT_SIZE_V1 as usize {
                self.finalize_pending_segment()?;
            }
        }
        if self.payload_len != expected_payload_len {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        Ok(())
    }
    /// Finish construction and return the canonical tree.
    pub fn finish(mut self) -> Result<PdpMerkleTreeV1, PdpMerkleTreeError> {
        if self.payload_len == 0 {
            return Err(PdpMerkleTreeError::EmptyPayload);
        }
        estimated_heap_bytes(self.payload_len)?;
        if !self.pending_segment.is_empty() {
            self.finalize_pending_segment()?;
        }
        if self.segment_commitments.is_empty() || self.hot_leaf_digests.is_empty() {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        let hot_leaf_count = u64::try_from(self.hot_leaf_digests.len())
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let segment_count = u64::try_from(self.segment_commitments.len())
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        if hot_leaf_count != hot_leaf_count_for_payload(self.payload_len)
            || segment_count != segment_count_for_payload(self.payload_len)
        {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        let hot_nodes = build_compact_merkle_nodes(self.hot_leaf_digests, HOT_NODE_DOMAIN_V1)?;
        let segment_nodes =
            build_compact_merkle_nodes(self.segment_commitments, SEGMENT_NODE_DOMAIN_V1)?;
        let hot_top = hot_nodes
            .last()
            .copied()
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        let segment_top = segment_nodes
            .last()
            .copied()
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        u16::try_from(merkle_path_depth(hot_leaf_count) + 1)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        u16::try_from(merkle_path_depth(segment_count) + 1)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let hot_root = wrap_hot_root_v1(self.payload_len, hot_leaf_count, &hot_top);
        let segment_root = wrap_segment_root_v1(self.payload_len, segment_count, &segment_top);
        Ok(PdpMerkleTreeV1 {
            payload_len: self.payload_len,
            hot_nodes,
            segment_nodes,
            hot_root,
            segment_root,
        })
    }
    fn finalize_pending_segment(&mut self) -> Result<(), PdpMerkleTreeError> {
        if self.pending_segment.is_empty() {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        if self.pending_segment.len() > PDP_SEGMENT_SIZE_V1 as usize {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        let segment_index = u64::try_from(self.segment_commitments.len())
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let segment_offset = segment_index
            .checked_mul(u64::from(PDP_SEGMENT_SIZE_V1))
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let segment_length = u32::try_from(self.pending_segment.len())
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let hot_leaf_start = self.hot_leaf_digests.len();
        let local_leaf_count = self
            .pending_segment
            .len()
            .div_ceil(PDP_HOT_LEAF_SIZE_V1 as usize);
        if local_leaf_count == 0 || local_leaf_count > HOT_LEAVES_PER_SEGMENT_USIZE_V1 {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        let expected_hot_leaf_start = usize::try_from(segment_index)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?
            .checked_mul(HOT_LEAVES_PER_SEGMENT_USIZE_V1)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        if hot_leaf_start != expected_hot_leaf_start {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        try_reserve_geometric(&mut self.hot_leaf_digests, local_leaf_count)?;
        try_reserve_geometric(&mut self.segment_commitments, 1)?;
        for (local_index, leaf_bytes) in self
            .pending_segment
            .chunks(PDP_HOT_LEAF_SIZE_V1 as usize)
            .enumerate()
        {
            let leaf_index =
                u16::try_from(local_index).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
            let global_position = hot_leaf_start
                .checked_add(local_index)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            let global_leaf_index =
                u64::try_from(global_position).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
            let local_offset = u64::try_from(local_index)
                .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?
                .checked_mul(u64::from(PDP_HOT_LEAF_SIZE_V1))
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            let leaf_offset = segment_offset
                .checked_add(local_offset)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            let leaf_length = u32::try_from(leaf_bytes.len())
                .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
            let digest = hash_hot_leaf_v1(
                global_leaf_index,
                segment_index,
                leaf_index,
                leaf_offset,
                leaf_length,
                leaf_bytes,
            );
            self.hot_leaf_digests.push(digest);
        }
        let segment_hot_root = bounded_segment_hot_root(
            self.hot_leaf_digests
                .get(hot_leaf_start..)
                .ok_or(PdpMerkleTreeError::CorruptTree)?,
        )?;
        let hot_leaf_count =
            u16::try_from(local_leaf_count).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let commitment = hash_segment_leaf_v1(
            segment_index,
            segment_offset,
            segment_length,
            hot_leaf_count,
            &segment_hot_root,
        );
        self.segment_commitments.push(commitment);
        self.pending_segment.clear();
        Ok(())
    }
}
impl PdpMerkleTreeV1 {
    /// Build the canonical v1 commitment tree from contiguous payload bytes.
    pub fn from_bytes(payload: &[u8]) -> Result<Self, PdpMerkleTreeError> {
        let mut builder = PdpMerkleTreeBuilderV1::new();
        builder.update(payload)?;
        builder.finish()
    }
    /// Total payload length committed by this tree.
    #[must_use]
    pub fn payload_len(&self) -> u64 {
        self.payload_len
    }
    /// Number of global hot leaves committed by this tree.
    #[must_use]
    pub fn hot_leaf_count(&self) -> u64 {
        hot_leaf_count_for_payload(self.payload_len)
    }
    /// Number of 256 KiB segments committed by this tree.
    #[must_use]
    pub fn segment_count(&self) -> u64 {
        segment_count_for_payload(self.payload_len)
    }
    /// Levels in the global hot tree, including its root.
    #[must_use]
    pub fn hot_tree_height(&self) -> u16 {
        u16::try_from(merkle_path_depth(self.hot_leaf_count()) + 1).unwrap_or(u16::MAX)
    }
    /// Levels in the global segment tree, including its root.
    #[must_use]
    pub fn segment_tree_height(&self) -> u16 {
        u16::try_from(merkle_path_depth(self.segment_count()) + 1).unwrap_or(u16::MAX)
    }
    /// Canonical global hot-leaf commitment root.
    #[must_use]
    pub fn hot_root(&self) -> [u8; 32] {
        self.hot_root
    }
    /// Canonical global segment commitment root.
    #[must_use]
    pub fn segment_root(&self) -> [u8; 32] {
        self.segment_root
    }
    /// Construct exact PDP witnesses for the supplied canonical sample set.
    pub fn prove_samples(
        &self,
        samples: &[PdpSampleV1],
        payload: &[u8],
    ) -> Result<Vec<PdpProofLeafV1>, PdpMerkleTreeError> {
        let actual_len =
            u64::try_from(payload.len()).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        if actual_len != self.payload_len {
            return Err(PdpMerkleTreeError::PayloadLengthMismatch {
                expected: self.payload_len,
                actual: actual_len,
            });
        }
        let result = self.prove_samples_with(samples, |offset, buffer| {
            let Some(start) = usize::try_from(offset).ok() else {
                return Ok::<usize, std::convert::Infallible>(0);
            };
            let Some(end) = start.checked_add(buffer.len()) else {
                return Ok(0);
            };
            let Some(bytes) = payload.get(start..end) else {
                return Ok(0);
            };
            buffer.copy_from_slice(bytes);
            Ok(bytes.len())
        });
        match result {
            Ok(proofs) => Ok(proofs),
            Err(PdpMerkleReadError::Tree(error)) => Err(error),
            Err(PdpMerkleReadError::ReadFailed { source, .. }) => match source {},
            Err(PdpMerkleReadError::ReadLengthMismatch { .. }) => {
                Err(PdpMerkleTreeError::PayloadLengthMismatch {
                    expected: self.payload_len,
                    actual: actual_len,
                })
            }
        }
    }
    /// Construct exact PDP witnesses using bounded random-access payload reads.
    ///
    /// The callback is invoked exactly once for each requested hot leaf, after
    /// the entire sample set and every requested leaf's geometry have been
    /// validated. `offset` is an absolute payload offset and `buffer` has the
    /// exact committed leaf length (at most 4 KiB). The callback must attempt a
    /// single positional read into `buffer` and return the number of bytes it
    /// supplied. Any value other than `buffer.len()` is rejected as a short or
    /// otherwise invalid read; proof construction never retries it.
    ///
    /// This API lets storage backends serve witnesses without materializing the
    /// committed payload. It caps segment and leaf counts to the v1 protocol
    /// maxima before allocating or performing I/O.
    pub fn prove_samples_with<E, F>(
        &self,
        samples: &[PdpSampleV1],
        mut read_at: F,
    ) -> Result<Vec<PdpProofLeafV1>, PdpMerkleReadError<E>>
    where
        E: std::error::Error + Send + Sync + 'static,
        F: FnMut(u64, &mut [u8]) -> Result<usize, E>,
    {
        self.validate_proof_samples(samples)?;
        let mut proof_leaves = Vec::new();
        try_reserve_exact(&mut proof_leaves, samples.len())?;
        for sample in samples {
            let segment_position = usize::try_from(sample.segment_index)
                .map_err(|_| PdpMerkleTreeError::CorruptTree)?;
            let segment = self.segment_geometry(sample.segment_index)?;
            let segment_hot_layers = self.segment_hot_layers(segment)?;
            let mut hot_proofs = Vec::new();
            try_reserve_exact(&mut hot_proofs, sample.hot_leaf_indices.len())?;
            for &leaf_index in &sample.hot_leaf_indices {
                let local_index = usize::from(leaf_index);
                let global_position = segment
                    .hot_leaf_start
                    .checked_add(local_index)
                    .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
                let leaf = self.hot_leaf_geometry(segment, leaf_index)?;
                let byte_count = usize::try_from(leaf.length)
                    .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
                let mut bytes = Vec::new();
                try_reserve_exact(&mut bytes, byte_count)?;
                bytes.resize(byte_count, 0);
                let actual = read_at(leaf.offset, &mut bytes).map_err(|source| {
                    PdpMerkleReadError::ReadFailed {
                        offset: leaf.offset,
                        length: leaf.length,
                        source,
                    }
                })?;
                if actual != byte_count {
                    return Err(PdpMerkleReadError::ReadLengthMismatch {
                        offset: leaf.offset,
                        expected: byte_count,
                        actual,
                    });
                }
                let digest = hash_hot_leaf_v1(
                    leaf.global_index,
                    segment.index,
                    leaf_index,
                    leaf.offset,
                    leaf.length,
                    &bytes,
                );
                let expected_digest = self
                    .hot_nodes
                    .get(global_position)
                    .ok_or(PdpMerkleTreeError::CorruptTree)?;
                if &digest != expected_digest {
                    return Err(PdpMerkleReadError::Tree(
                        PdpMerkleTreeError::PayloadDigestMismatch {
                            segment_index: sample.segment_index,
                            leaf_index,
                        },
                    ));
                }
                hot_proofs.push(PdpHotLeafProofV1 {
                    leaf_index,
                    leaf_offset: leaf.offset,
                    leaf_length: leaf.length,
                    leaf_bytes: bytes,
                    segment_hot_merkle_path: merkle_path(&segment_hot_layers, local_index)?,
                    global_hot_merkle_path: compact_merkle_path(
                        &self.hot_nodes,
                        usize::try_from(self.hot_leaf_count())
                            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?,
                        global_position,
                    )?,
                });
            }
            proof_leaves.push(PdpProofLeafV1 {
                segment_index: segment.index,
                segment_offset: segment.offset,
                segment_length: segment.length,
                segment_merkle_path: compact_merkle_path(
                    &self.segment_nodes,
                    usize::try_from(self.segment_count())
                        .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?,
                    segment_position,
                )?,
                hot_leaves: hot_proofs,
            });
        }
        Ok(proof_leaves)
    }
    fn validate_proof_samples(&self, samples: &[PdpSampleV1]) -> Result<(), PdpMerkleTreeError> {
        if samples.is_empty() {
            return Err(PdpMerkleTreeError::EmptySampleSet);
        }
        if samples.len() > PDP_MAX_SEGMENT_SAMPLES_V1 {
            return Err(PdpMerkleTreeError::TooManySegmentSamples {
                found: samples.len(),
            });
        }
        self.validate_tree_geometry()?;
        let mut previous_segment = None;
        let mut total_hot_leaves = 0usize;
        for sample in samples {
            if previous_segment.is_some_and(|previous| previous >= sample.segment_index) {
                return Err(PdpMerkleTreeError::NonCanonicalSegmentOrder);
            }
            if sample.hot_leaf_indices.is_empty() {
                return Err(PdpMerkleTreeError::EmptyHotLeafSet {
                    segment_index: sample.segment_index,
                });
            }
            if sample.hot_leaf_indices.len() > PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1 {
                return Err(PdpMerkleTreeError::TooManyHotLeaves {
                    segment_index: sample.segment_index,
                    found: sample.hot_leaf_indices.len(),
                });
            }
            total_hot_leaves = total_hot_leaves
                .checked_add(sample.hot_leaf_indices.len())
                .ok_or(PdpMerkleTreeError::TooManyHotLeavesTotal { found: usize::MAX })?;
            if total_hot_leaves > PDP_MAX_TOTAL_HOT_LEAF_SAMPLES_V1 {
                return Err(PdpMerkleTreeError::TooManyHotLeavesTotal {
                    found: total_hot_leaves,
                });
            }
            if sample.segment_index >= self.segment_count() {
                return Err(PdpMerkleTreeError::SegmentOutOfRange {
                    segment_index: sample.segment_index,
                });
            }
            let segment = self.segment_geometry(sample.segment_index)?;
            self.validate_segment_geometry(segment)?;
            let mut previous_leaf = None;
            for &leaf_index in &sample.hot_leaf_indices {
                if leaf_index >= PDP_HOT_LEAVES_PER_SEGMENT_V1
                    || usize::from(leaf_index) >= segment.hot_leaf_count
                {
                    return Err(PdpMerkleTreeError::HotLeafOutOfRange {
                        segment_index: sample.segment_index,
                        leaf_index,
                    });
                }
                if previous_leaf.is_some_and(|previous| previous >= leaf_index) {
                    return Err(PdpMerkleTreeError::NonCanonicalHotLeafOrder {
                        segment_index: sample.segment_index,
                    });
                }
                self.hot_leaf_geometry(segment, leaf_index)?;
                previous_leaf = Some(leaf_index);
            }
            previous_segment = Some(sample.segment_index);
        }
        Ok(())
    }
    fn validate_tree_geometry(&self) -> Result<(), PdpMerkleTreeError> {
        if self.payload_len == 0 {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        let hot_leaf_count = self.hot_leaf_count();
        let segment_count = self.segment_count();
        let hot_leaf_count_usize =
            usize::try_from(hot_leaf_count).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let segment_count_usize =
            usize::try_from(segment_count).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        validate_compact_tree_geometry(&self.hot_nodes, hot_leaf_count_usize)?;
        validate_compact_tree_geometry(&self.segment_nodes, segment_count_usize)?;
        let hot_height = u16::try_from(merkle_path_depth(hot_leaf_count) + 1)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let segment_height = u16::try_from(merkle_path_depth(segment_count) + 1)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let hot_top = self
            .hot_nodes
            .last()
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        let segment_top = self
            .segment_nodes
            .last()
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        if self.hot_tree_height() != hot_height
            || self.segment_tree_height() != segment_height
            || self.hot_root != wrap_hot_root_v1(self.payload_len, hot_leaf_count, hot_top)
            || self.segment_root
                != wrap_segment_root_v1(self.payload_len, segment_count, segment_top)
        {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        Ok(())
    }
    fn segment_geometry(
        &self,
        segment_index: u64,
    ) -> Result<SegmentGeometryV1, PdpMerkleTreeError> {
        if segment_index >= self.segment_count() {
            return Err(PdpMerkleTreeError::SegmentOutOfRange { segment_index });
        }
        let offset = segment_index
            .checked_mul(u64::from(PDP_SEGMENT_SIZE_V1))
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let remaining = self
            .payload_len
            .checked_sub(offset)
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        let length = u32::try_from(remaining.min(u64::from(PDP_SEGMENT_SIZE_V1)))
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let hot_leaf_count =
            usize::try_from(u64::from(length).div_ceil(u64::from(PDP_HOT_LEAF_SIZE_V1)))
                .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let hot_leaf_start = usize::try_from(segment_index)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?
            .checked_mul(HOT_LEAVES_PER_SEGMENT_USIZE_V1)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let hot_leaf_end = hot_leaf_start
            .checked_add(hot_leaf_count)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let retained_hot_leaf_count = usize::try_from(self.hot_leaf_count())
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        if length == 0
            || hot_leaf_count == 0
            || hot_leaf_count > HOT_LEAVES_PER_SEGMENT_USIZE_V1
            || hot_leaf_end > retained_hot_leaf_count
        {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        Ok(SegmentGeometryV1 {
            index: segment_index,
            offset,
            length,
            hot_leaf_start,
            hot_leaf_count,
        })
    }
    fn segment_hot_digests(
        &self,
        segment: SegmentGeometryV1,
    ) -> Result<&[[u8; 32]], PdpMerkleTreeError> {
        let end = segment
            .hot_leaf_start
            .checked_add(segment.hot_leaf_count)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        self.hot_nodes
            .get(segment.hot_leaf_start..end)
            .ok_or(PdpMerkleTreeError::CorruptTree)
    }
    fn expected_segment_commitment(
        &self,
        segment: SegmentGeometryV1,
    ) -> Result<[u8; 32], PdpMerkleTreeError> {
        let local_root = bounded_segment_hot_root(self.segment_hot_digests(segment)?)?;
        let hot_leaf_count = u16::try_from(segment.hot_leaf_count)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        Ok(hash_segment_leaf_v1(
            segment.index,
            segment.offset,
            segment.length,
            hot_leaf_count,
            &local_root,
        ))
    }
    fn validate_segment_geometry(
        &self,
        segment: SegmentGeometryV1,
    ) -> Result<(), PdpMerkleTreeError> {
        let position =
            usize::try_from(segment.index).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let retained_commitment = self
            .segment_nodes
            .get(position)
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        if retained_commitment != &self.expected_segment_commitment(segment)? {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        Ok(())
    }
    fn segment_hot_layers(
        &self,
        segment: SegmentGeometryV1,
    ) -> Result<Vec<Vec<[u8; 32]>>, PdpMerkleTreeError> {
        let layers = build_merkle_layers(
            self.segment_hot_digests(segment)?,
            SEGMENT_HOT_NODE_DOMAIN_V1,
        )?;
        let local_root = layers
            .last()
            .and_then(|layer| layer.first())
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        let hot_leaf_count = u16::try_from(segment.hot_leaf_count)
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let commitment = hash_segment_leaf_v1(
            segment.index,
            segment.offset,
            segment.length,
            hot_leaf_count,
            local_root,
        );
        let position =
            usize::try_from(segment.index).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        if self.segment_nodes.get(position) != Some(&commitment) {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        Ok(layers)
    }
    fn hot_leaf_geometry(
        &self,
        segment: SegmentGeometryV1,
        leaf_index: u16,
    ) -> Result<HotLeafGeometryV1, PdpMerkleTreeError> {
        let local_index = usize::from(leaf_index);
        if local_index >= segment.hot_leaf_count {
            return Err(PdpMerkleTreeError::HotLeafOutOfRange {
                segment_index: segment.index,
                leaf_index,
            });
        }
        let local_offset = u64::from(leaf_index)
            .checked_mul(u64::from(PDP_HOT_LEAF_SIZE_V1))
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let offset = segment
            .offset
            .checked_add(local_offset)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let segment_end = segment
            .offset
            .checked_add(u64::from(segment.length))
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let remaining = segment_end
            .checked_sub(offset)
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        let length = u32::try_from(remaining.min(u64::from(PDP_HOT_LEAF_SIZE_V1)))
            .map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let global_position = segment
            .hot_leaf_start
            .checked_add(local_index)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let global_index =
            u64::try_from(global_position).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        let leaf_end = offset
            .checked_add(u64::from(length))
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        if length == 0
            || leaf_end > self.payload_len
            || self.hot_nodes.get(global_position).is_none()
        {
            return Err(PdpMerkleTreeError::CorruptTree);
        }
        Ok(HotLeafGeometryV1 {
            global_index,
            offset,
            length,
        })
    }
}
pub(super) fn hash_hot_leaf_v1(
    global_leaf_index: u64,
    segment_index: u64,
    segment_leaf_index: u16,
    offset: u64,
    length: u32,
    bytes: &[u8],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(HOT_LEAF_DOMAIN_V1);
    hasher.update(&global_leaf_index.to_le_bytes());
    hasher.update(&segment_index.to_le_bytes());
    hasher.update(&segment_leaf_index.to_le_bytes());
    hasher.update(&offset.to_le_bytes());
    hasher.update(&length.to_le_bytes());
    hasher.update(bytes);
    hasher.finalize().into()
}
pub(super) fn hash_segment_leaf_v1(
    segment_index: u64,
    offset: u64,
    length: u32,
    hot_leaf_count: u16,
    hot_tree_top: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SEGMENT_LEAF_DOMAIN_V1);
    hasher.update(&segment_index.to_le_bytes());
    hasher.update(&offset.to_le_bytes());
    hasher.update(&length.to_le_bytes());
    hasher.update(&hot_leaf_count.to_le_bytes());
    hasher.update(hot_tree_top);
    hasher.finalize().into()
}
pub(super) fn fold_segment_hot_path_v1(
    leaf_index: u64,
    leaf_count: u64,
    leaf: [u8; 32],
    path: &[[u8; 32]],
) -> Result<[u8; 32], PdpMerklePathError> {
    fold_merkle_path(
        SEGMENT_HOT_NODE_DOMAIN_V1,
        leaf_index,
        leaf_count,
        leaf,
        path,
    )
}
pub(super) fn fold_global_hot_path_v1(
    leaf_index: u64,
    leaf_count: u64,
    leaf: [u8; 32],
    path: &[[u8; 32]],
) -> Result<[u8; 32], PdpMerklePathError> {
    fold_merkle_path(HOT_NODE_DOMAIN_V1, leaf_index, leaf_count, leaf, path)
}
pub(super) fn fold_segment_path_v1(
    segment_index: u64,
    segment_count: u64,
    leaf: [u8; 32],
    path: &[[u8; 32]],
) -> Result<[u8; 32], PdpMerklePathError> {
    fold_merkle_path(
        SEGMENT_NODE_DOMAIN_V1,
        segment_index,
        segment_count,
        leaf,
        path,
    )
}
pub(super) fn wrap_hot_root_v1(payload_len: u64, hot_leaf_count: u64, top: &[u8; 32]) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(HOT_ROOT_DOMAIN_V1);
    hasher.update(&payload_len.to_le_bytes());
    hasher.update(&PDP_HOT_LEAF_SIZE_V1.to_le_bytes());
    hasher.update(&hot_leaf_count.to_le_bytes());
    hasher.update(top);
    hasher.finalize().into()
}
pub(super) fn wrap_segment_root_v1(
    payload_len: u64,
    segment_count: u64,
    top: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(SEGMENT_ROOT_DOMAIN_V1);
    hasher.update(&payload_len.to_le_bytes());
    hasher.update(&PDP_SEGMENT_SIZE_V1.to_le_bytes());
    hasher.update(&segment_count.to_le_bytes());
    hasher.update(top);
    hasher.finalize().into()
}
fn build_merkle_layers(
    leaves: &[[u8; 32]],
    node_domain: &[u8],
) -> Result<Vec<Vec<[u8; 32]>>, PdpMerkleTreeError> {
    if leaves.is_empty() {
        return Err(PdpMerkleTreeError::EmptyPayload);
    }
    let mut leaf_layer = Vec::new();
    try_reserve_exact(&mut leaf_layer, leaves.len())?;
    leaf_layer.extend_from_slice(leaves);
    build_merkle_layers_owned(leaf_layer, node_domain)
}
fn build_compact_merkle_nodes(
    mut nodes: Vec<[u8; 32]>,
    node_domain: &[u8],
) -> Result<Box<[[u8; 32]]>, PdpMerkleTreeError> {
    if nodes.is_empty() {
        return Err(PdpMerkleTreeError::EmptyPayload);
    }
    let leaf_count = nodes.len();
    let total_node_count = merkle_total_node_count(leaf_count)?;
    let additional = total_node_count
        .checked_sub(nodes.len())
        .ok_or(PdpMerkleTreeError::CorruptTree)?;
    try_reserve_exact(&mut nodes, additional)?;
    let mut layer_start = 0usize;
    let mut layer_count = leaf_count;
    let mut level = 1u16;
    while layer_count > 1 {
        let next_count = layer_count.div_ceil(2);
        let next_start = nodes.len();
        for parent_index in 0..next_count {
            let left_offset = parent_index
                .checked_mul(2)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            let left_position = layer_start
                .checked_add(left_offset)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            let left = nodes
                .get(left_position)
                .copied()
                .ok_or(PdpMerkleTreeError::CorruptTree)?;
            let right_offset = left_offset
                .checked_add(1)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            let right = if right_offset < layer_count {
                let right_position = layer_start
                    .checked_add(right_offset)
                    .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
                nodes
                    .get(right_position)
                    .copied()
                    .ok_or(PdpMerkleTreeError::CorruptTree)?
            } else {
                left
            };
            nodes.push(hash_merkle_node(
                node_domain,
                level,
                u64::try_from(parent_index).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?,
                &left,
                &right,
            ));
        }
        layer_start = next_start;
        layer_count = next_count;
        level = level
            .checked_add(1)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
    }
    if nodes.len() != total_node_count {
        return Err(PdpMerkleTreeError::CorruptTree);
    }
    Ok(nodes.into_boxed_slice())
}
fn build_merkle_layers_owned(
    leaf_layer: Vec<[u8; 32]>,
    node_domain: &[u8],
) -> Result<Vec<Vec<[u8; 32]>>, PdpMerkleTreeError> {
    if leaf_layer.is_empty() {
        return Err(PdpMerkleTreeError::EmptyPayload);
    }
    let leaf_count =
        u64::try_from(leaf_layer.len()).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
    let mut layers = Vec::new();
    try_reserve_exact(&mut layers, merkle_path_depth(leaf_count) + 1)?;
    layers.push(leaf_layer);
    while layers.last().is_some_and(|level| level.len() > 1) {
        let current = layers.last().ok_or(PdpMerkleTreeError::CorruptTree)?;
        let next_len = current.len().div_ceil(2);
        let mut next = Vec::new();
        try_reserve_exact(&mut next, next_len)?;
        let level =
            u16::try_from(layers.len()).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
        for (parent_index, pair) in current.chunks(2).enumerate() {
            let left = pair
                .first()
                .copied()
                .ok_or(PdpMerkleTreeError::CorruptTree)?;
            let right = pair.get(1).copied().unwrap_or(left);
            next.push(hash_merkle_node(
                node_domain,
                level,
                u64::try_from(parent_index).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?,
                &left,
                &right,
            ));
        }
        layers.push(next);
    }
    Ok(layers)
}
fn bounded_segment_hot_root(leaves: &[[u8; 32]]) -> Result<[u8; 32], PdpMerkleTreeError> {
    if leaves.is_empty() || leaves.len() > HOT_LEAVES_PER_SEGMENT_USIZE_V1 {
        return Err(PdpMerkleTreeError::CorruptTree);
    }
    let mut nodes = [[0u8; 32]; HOT_LEAVES_PER_SEGMENT_USIZE_V1];
    nodes[..leaves.len()].copy_from_slice(leaves);
    let mut count = leaves.len();
    let mut level = 1u16;
    while count > 1 {
        let next_count = count.div_ceil(2);
        for parent_index in 0..next_count {
            let left_index = parent_index
                .checked_mul(2)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            let left = nodes[left_index];
            let right = nodes.get(left_index + 1).copied().unwrap_or(left);
            let right = if left_index + 1 < count { right } else { left };
            nodes[parent_index] = hash_merkle_node(
                SEGMENT_HOT_NODE_DOMAIN_V1,
                level,
                u64::try_from(parent_index).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?,
                &left,
                &right,
            );
        }
        count = next_count;
        level = level
            .checked_add(1)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
    }
    Ok(nodes[0])
}
fn hot_leaf_count_for_payload(payload_len: u64) -> u64 {
    payload_len.div_ceil(u64::from(PDP_HOT_LEAF_SIZE_V1))
}
fn segment_count_for_payload(payload_len: u64) -> u64 {
    payload_len.div_ceil(u64::from(PDP_SEGMENT_SIZE_V1))
}
fn geometric_capacity_for_len(length: usize) -> Result<usize, PdpMerkleTreeError> {
    if length == 0 {
        return Err(PdpMerkleTreeError::EmptyPayload);
    }
    length
        .checked_next_power_of_two()
        .ok_or(PdpMerkleTreeError::GeometryOverflow)
}
fn merkle_parent_node_count(mut leaf_count: usize) -> Result<usize, PdpMerkleTreeError> {
    if leaf_count == 0 {
        return Err(PdpMerkleTreeError::EmptyPayload);
    }
    let mut total = 0usize;
    while leaf_count > 1 {
        leaf_count = leaf_count.div_ceil(2);
        total = total
            .checked_add(leaf_count)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
    }
    Ok(total)
}
fn merkle_total_node_count(leaf_count: usize) -> Result<usize, PdpMerkleTreeError> {
    leaf_count
        .checked_add(merkle_parent_node_count(leaf_count)?)
        .ok_or(PdpMerkleTreeError::GeometryOverflow)
}
fn checked_allocation_bytes<T>(elements: usize) -> Result<usize, PdpMerkleTreeError> {
    let bytes = elements
        .checked_mul(std::mem::size_of::<T>())
        .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
    if bytes > isize::MAX as usize {
        return Err(PdpMerkleTreeError::AllocationFailed);
    }
    Ok(bytes)
}
fn try_reserve_exact<T>(values: &mut Vec<T>, additional: usize) -> Result<(), PdpMerkleTreeError> {
    values
        .try_reserve_exact(additional)
        .map_err(|_| PdpMerkleTreeError::AllocationFailed)
}
fn try_reserve_geometric<T>(
    values: &mut Vec<T>,
    additional: usize,
) -> Result<(), PdpMerkleTreeError> {
    let required = values
        .len()
        .checked_add(additional)
        .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
    if required <= values.capacity() {
        return Ok(());
    }
    let target = geometric_capacity_for_len(required)?;
    let reserve = target
        .checked_sub(values.len())
        .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
    try_reserve_exact(values, reserve)
}
fn validate_compact_tree_geometry(
    nodes: &[[u8; 32]],
    leaf_count: usize,
) -> Result<(), PdpMerkleTreeError> {
    if leaf_count == 0 || nodes.len() != merkle_total_node_count(leaf_count)? {
        return Err(PdpMerkleTreeError::CorruptTree);
    }
    Ok(())
}
fn compact_merkle_path(
    nodes: &[[u8; 32]],
    leaf_count: usize,
    mut index: usize,
) -> Result<Vec<[u8; 32]>, PdpMerkleTreeError> {
    validate_compact_tree_geometry(nodes, leaf_count)?;
    if index >= leaf_count {
        return Err(PdpMerkleTreeError::CorruptTree);
    }
    let mut path = Vec::new();
    let leaf_count_u64 =
        u64::try_from(leaf_count).map_err(|_| PdpMerkleTreeError::GeometryOverflow)?;
    try_reserve_exact(&mut path, merkle_path_depth(leaf_count_u64))?;
    let mut layer_start = 0usize;
    let mut layer_count = leaf_count;
    while layer_count > 1 {
        let current_position = layer_start
            .checked_add(index)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        let current = nodes
            .get(current_position)
            .copied()
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        let sibling_index = index ^ 1;
        let sibling = if sibling_index < layer_count {
            let sibling_position = layer_start
                .checked_add(sibling_index)
                .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
            nodes
                .get(sibling_position)
                .copied()
                .ok_or(PdpMerkleTreeError::CorruptTree)?
        } else {
            current
        };
        path.push(sibling);
        layer_start = layer_start
            .checked_add(layer_count)
            .ok_or(PdpMerkleTreeError::GeometryOverflow)?;
        layer_count = layer_count.div_ceil(2);
        index /= 2;
    }
    Ok(path)
}
fn merkle_path(
    layers: &[Vec<[u8; 32]>],
    mut index: usize,
) -> Result<Vec<[u8; 32]>, PdpMerkleTreeError> {
    let leaf_count = layers
        .first()
        .map(Vec::len)
        .ok_or(PdpMerkleTreeError::CorruptTree)?;
    if index >= leaf_count {
        return Err(PdpMerkleTreeError::CorruptTree);
    }
    let mut path = Vec::new();
    try_reserve_exact(&mut path, layers.len().saturating_sub(1))?;
    for level in layers.iter().take(layers.len().saturating_sub(1)) {
        let current = level
            .get(index)
            .copied()
            .ok_or(PdpMerkleTreeError::CorruptTree)?;
        let sibling_index = index ^ 1;
        path.push(level.get(sibling_index).copied().unwrap_or(current));
        index /= 2;
    }
    Ok(path)
}
fn fold_merkle_path(
    node_domain: &[u8],
    mut index: u64,
    mut count: u64,
    mut current: [u8; 32],
    path: &[[u8; 32]],
) -> Result<[u8; 32], PdpMerklePathError> {
    if count == 0 {
        return Err(PdpMerklePathError::EmptyTree);
    }
    if index >= count {
        return Err(PdpMerklePathError::IndexOutOfRange { index, count });
    }
    let expected = merkle_path_depth(count);
    if path.len() != expected {
        return Err(PdpMerklePathError::DepthMismatch {
            expected,
            actual: path.len(),
        });
    }
    for (path_level, sibling) in path.iter().enumerate() {
        let sibling_index = index ^ 1;
        if sibling_index >= count && sibling != &current {
            return Err(PdpMerklePathError::NonCanonicalOddSibling { level: path_level });
        }
        let (left, right) = if index & 1 == 0 {
            (&current, sibling)
        } else {
            (sibling, &current)
        };
        let level = u16::try_from(path_level + 1).map_err(|_| PdpMerklePathError::DepthOverflow)?;
        current = hash_merkle_node(node_domain, level, index / 2, left, right);
        index /= 2;
        count = count.div_ceil(2);
    }
    Ok(current)
}
fn hash_merkle_node(
    domain: &[u8],
    level: u16,
    parent_index: u64,
    left: &[u8; 32],
    right: &[u8; 32],
) -> [u8; 32] {
    let mut hasher = blake3::Hasher::new();
    hasher.update(domain);
    hasher.update(&level.to_le_bytes());
    hasher.update(&parent_index.to_le_bytes());
    hasher.update(left);
    hasher.update(right);
    hasher.finalize().into()
}
pub(super) fn merkle_path_depth(mut count: u64) -> usize {
    let mut depth = 0usize;
    while count > 1 {
        count = count.div_ceil(2);
        depth += 1;
    }
    depth
}
#[cfg(test)]
mod tests {
    use std::{
        cell::Cell,
        io,
        mem::{size_of, size_of_val},
    };
    use super::*;
    fn deterministic_payload(length: usize) -> Vec<u8> {
        (0..length)
            .map(|index| {
                let mixed = index
                    .wrapping_mul(0x9e37_79b1)
                    .rotate_left((index % usize::BITS as usize) as u32)
                    ^ index.wrapping_mul(131).wrapping_add(17);
                mixed as u8
            })
            .collect()
    }
    fn build_from_chunks<'a>(chunks: impl IntoIterator<Item = &'a [u8]>) -> PdpMerkleTreeV1 {
        let mut builder = PdpMerkleTreeBuilderV1::new();
        for chunk in chunks {
            builder.update(chunk).expect("streaming update");
        }
        builder.finish().expect("finish streaming tree")
    }
    #[derive(Debug)]
    struct RetainedReferenceTreeV1 {
        payload_len: u64,
        hot_layers: Vec<Vec<[u8; 32]>>,
        segment_layers: Vec<Vec<[u8; 32]>>,
        segment_hot_layers: Vec<Vec<Vec<[u8; 32]>>>,
        hot_root: [u8; 32],
        segment_root: [u8; 32],
    }
    impl RetainedReferenceTreeV1 {
        fn from_payload(payload: &[u8]) -> Self {
            assert!(!payload.is_empty());
            let payload_len = payload.len() as u64;
            let mut hot_digests = Vec::new();
            let mut segment_commitments = Vec::new();
            let mut segment_hot_layers = Vec::new();
            for (segment_index, segment_bytes) in
                payload.chunks(PDP_SEGMENT_SIZE_V1 as usize).enumerate()
            {
                let segment_index = segment_index as u64;
                let segment_offset = segment_index * u64::from(PDP_SEGMENT_SIZE_V1);
                let mut local_digests = Vec::new();
                for (local_index, leaf_bytes) in segment_bytes
                    .chunks(PDP_HOT_LEAF_SIZE_V1 as usize)
                    .enumerate()
                {
                    let global_index = hot_digests.len() as u64;
                    let local_index = local_index as u16;
                    let offset =
                        segment_offset + u64::from(local_index) * u64::from(PDP_HOT_LEAF_SIZE_V1);
                    let digest = hash_hot_leaf_v1(
                        global_index,
                        segment_index,
                        local_index,
                        offset,
                        leaf_bytes.len() as u32,
                        leaf_bytes,
                    );
                    hot_digests.push(digest);
                    local_digests.push(digest);
                }
                let local_layers = build_merkle_layers(&local_digests, SEGMENT_HOT_NODE_DOMAIN_V1)
                    .expect("reference local tree");
                let local_root = local_layers.last().expect("local root layer")[0];
                segment_commitments.push(hash_segment_leaf_v1(
                    segment_index,
                    segment_offset,
                    segment_bytes.len() as u32,
                    local_digests.len() as u16,
                    &local_root,
                ));
                segment_hot_layers.push(local_layers);
            }
            let hot_layers =
                build_merkle_layers(&hot_digests, HOT_NODE_DOMAIN_V1).expect("reference hot tree");
            let segment_layers = build_merkle_layers(&segment_commitments, SEGMENT_NODE_DOMAIN_V1)
                .expect("reference segment tree");
            let hot_root = wrap_hot_root_v1(
                payload_len,
                hot_digests.len() as u64,
                &hot_layers.last().expect("hot root layer")[0],
            );
            let segment_root = wrap_segment_root_v1(
                payload_len,
                segment_commitments.len() as u64,
                &segment_layers.last().expect("segment root layer")[0],
            );
            Self {
                payload_len,
                hot_layers,
                segment_layers,
                segment_hot_layers,
                hot_root,
                segment_root,
            }
        }
        fn prove_samples(&self, samples: &[PdpSampleV1], payload: &[u8]) -> Vec<PdpProofLeafV1> {
            assert_eq!(payload.len() as u64, self.payload_len);
            samples
                .iter()
                .map(|sample| {
                    let segment_position = sample.segment_index as usize;
                    let segment_offset = sample.segment_index * u64::from(PDP_SEGMENT_SIZE_V1);
                    let segment_length = (self.payload_len - segment_offset)
                        .min(u64::from(PDP_SEGMENT_SIZE_V1))
                        as u32;
                    let hot_leaf_start = segment_position * HOT_LEAVES_PER_SEGMENT_USIZE_V1;
                    let hot_leaves = sample
                        .hot_leaf_indices
                        .iter()
                        .map(|&leaf_index| {
                            let local_index = usize::from(leaf_index);
                            let global_position = hot_leaf_start + local_index;
                            let offset = segment_offset
                                + u64::from(leaf_index) * u64::from(PDP_HOT_LEAF_SIZE_V1);
                            let length = (u64::from(segment_length)
                                - u64::from(leaf_index) * u64::from(PDP_HOT_LEAF_SIZE_V1))
                            .min(u64::from(PDP_HOT_LEAF_SIZE_V1))
                                as u32;
                            let start = offset as usize;
                            let end = start + length as usize;
                            PdpHotLeafProofV1 {
                                leaf_index,
                                leaf_offset: offset,
                                leaf_length: length,
                                leaf_bytes: payload[start..end].to_vec(),
                                segment_hot_merkle_path: merkle_path(
                                    &self.segment_hot_layers[segment_position],
                                    local_index,
                                )
                                .expect("reference local path"),
                                global_hot_merkle_path: merkle_path(
                                    &self.hot_layers,
                                    global_position,
                                )
                                .expect("reference global path"),
                            }
                        })
                        .collect();
                    PdpProofLeafV1 {
                        segment_index: sample.segment_index,
                        segment_offset,
                        segment_length,
                        segment_merkle_path: merkle_path(&self.segment_layers, segment_position)
                            .expect("reference segment path"),
                        hot_leaves,
                    }
                })
                .collect()
        }
    }
    fn representative_samples(payload_len: usize) -> Vec<PdpSampleV1> {
        let segment_count = payload_len.div_ceil(PDP_SEGMENT_SIZE_V1 as usize);
        (0..segment_count)
            .map(|segment_index| {
                let segment_offset = segment_index * PDP_SEGMENT_SIZE_V1 as usize;
                let segment_length =
                    (payload_len - segment_offset).min(PDP_SEGMENT_SIZE_V1 as usize);
                let leaf_count = segment_length.div_ceil(PDP_HOT_LEAF_SIZE_V1 as usize);
                let mut indices = vec![0, (leaf_count / 2) as u16, (leaf_count - 1) as u16];
                indices.sort_unstable();
                indices.dedup();
                PdpSampleV1 {
                    segment_index: segment_index as u64,
                    hot_leaf_indices: indices,
                }
            })
            .collect()
    }
    fn retained_heap_capacity_bytes(tree: &PdpMerkleTreeV1) -> usize {
        size_of_val(tree.hot_nodes.as_ref()) + size_of_val(tree.segment_nodes.as_ref())
    }
    fn read_payload(payload: &[u8], offset: u64, buffer: &mut [u8]) -> io::Result<usize> {
        let start = usize::try_from(offset)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "offset overflow"))?;
        let end = start
            .checked_add(buffer.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "range overflow"))?;
        let bytes = payload
            .get(start..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "short payload"))?;
        buffer.copy_from_slice(bytes);
        Ok(bytes.len())
    }
    fn rejected_without_reads(
        tree: &PdpMerkleTreeV1,
        samples: &[PdpSampleV1],
    ) -> PdpMerkleTreeError {
        let reads = Cell::new(0usize);
        let result = tree.prove_samples_with(samples, |_, _| {
            reads.set(reads.get() + 1);
            Ok::<usize, io::Error>(0)
        });
        assert_eq!(
            reads.get(),
            0,
            "invalid samples must be rejected before I/O"
        );
        match result {
            Err(PdpMerkleReadError::Tree(error)) => error,
            other => panic!("expected a tree/sample validation failure, got {other:?}"),
        }
    }
    #[test]
    fn compact_tree_matches_retained_reference_across_boundaries_and_splits() {
        let lengths = [
            1usize,
            PDP_HOT_LEAF_SIZE_V1 as usize - 1,
            PDP_HOT_LEAF_SIZE_V1 as usize,
            PDP_HOT_LEAF_SIZE_V1 as usize + 1,
            PDP_SEGMENT_SIZE_V1 as usize - 1,
            PDP_SEGMENT_SIZE_V1 as usize,
            PDP_SEGMENT_SIZE_V1 as usize + 1,
            PDP_SEGMENT_SIZE_V1 as usize * 2 + PDP_HOT_LEAF_SIZE_V1 as usize + 37,
        ];
        for length in lengths {
            let payload = deterministic_payload(length);
            let reference = RetainedReferenceTreeV1::from_payload(&payload);
            let samples = representative_samples(length);
            let expected_proofs = reference.prove_samples(&samples, &payload);
            let split_points = [
                1usize.min(length),
                (PDP_HOT_LEAF_SIZE_V1 as usize - 1).min(length),
                (PDP_SEGMENT_SIZE_V1 as usize - 1).min(length),
            ];
            for split in split_points {
                let compact = build_from_chunks([&payload[..split], &payload[split..]]);
                assert_eq!(compact.hot_root(), reference.hot_root, "length {length}");
                assert_eq!(
                    compact.segment_root(),
                    reference.segment_root,
                    "length {length}"
                );
                assert_eq!(
                    compact
                        .prove_samples(&samples, &payload)
                        .expect("compact proofs"),
                    expected_proofs,
                    "length {length}, split {split}"
                );
            }
        }
    }
    #[test]
    fn compact_paths_match_layer_reference_for_every_odd_boundary() {
        for leaf_count in 1usize..=129 {
            let leaves = (0..leaf_count)
                .map(|index| *blake3::hash(&index.to_le_bytes()).as_bytes())
                .collect::<Vec<_>>();
            let reference =
                build_merkle_layers(&leaves, HOT_NODE_DOMAIN_V1).expect("reference Merkle layers");
            let compact = build_compact_merkle_nodes(leaves, HOT_NODE_DOMAIN_V1)
                .expect("compact Merkle nodes");
            assert_eq!(
                compact.last(),
                reference.last().and_then(|layer| layer.first()),
                "root differs for {leaf_count} leaves"
            );
            for index in 0..leaf_count {
                assert_eq!(
                    compact_merkle_path(&compact, leaf_count, index).expect("compact path"),
                    merkle_path(&reference, index).expect("reference path"),
                    "path differs for leaf {index} of {leaf_count}"
                );
            }
        }
    }
    #[test]
    fn compact_paths_reject_truncated_extended_and_out_of_range_state() {
        let leaves = (0u64..5)
            .map(|index| *blake3::hash(&index.to_le_bytes()).as_bytes())
            .collect::<Vec<_>>();
        let compact =
            build_compact_merkle_nodes(leaves, HOT_NODE_DOMAIN_V1).expect("compact Merkle nodes");
        assert_eq!(
            compact_merkle_path(&compact[..compact.len() - 1], 5, 0),
            Err(PdpMerkleTreeError::CorruptTree)
        );
        let mut extended = compact.to_vec();
        extended.push([0x55; 32]);
        assert_eq!(
            compact_merkle_path(&extended, 5, 0),
            Err(PdpMerkleTreeError::CorruptTree)
        );
        assert_eq!(
            compact_merkle_path(&compact, 5, 5),
            Err(PdpMerkleTreeError::CorruptTree)
        );
        assert_eq!(
            compact_merkle_path(&compact, 0, 0),
            Err(PdpMerkleTreeError::CorruptTree)
        );
    }
    #[test]
    fn finish_retains_exact_compact_node_slabs() {
        let payload = deterministic_payload(PDP_SEGMENT_SIZE_V1 as usize * 2);
        let mut builder = PdpMerkleTreeBuilderV1::new();
        builder.update(&payload).expect("complete segments");
        assert!(builder.pending_segment.is_empty());
        let expected_hot_nodes =
            merkle_total_node_count(builder.hot_leaf_digests.len()).expect("hot geometry");
        let expected_segment_nodes =
            merkle_total_node_count(builder.segment_commitments.len()).expect("segment geometry");
        let tree = builder.finish().expect("compact tree");
        assert_eq!(tree.hot_nodes.len(), expected_hot_nodes);
        assert_eq!(tree.segment_nodes.len(), expected_segment_nodes);
        assert_eq!(
            retained_heap_capacity_bytes(&tree),
            estimated_heap_bytes(tree.payload_len()).expect("heap estimate")
        );
    }
    #[test]
    fn estimator_bounds_actual_retained_capacities() {
        for length in [
            1usize,
            4_095,
            4_096,
            4_097,
            262_143,
            262_144,
            262_145,
            524_288 + 91,
        ] {
            let payload = deterministic_payload(length);
            let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
            let estimate = estimated_heap_bytes(length as u64).expect("heap estimate");
            let actual = retained_heap_capacity_bytes(&tree);
            assert_eq!(
                actual, estimate,
                "retained byte count differs from estimate for {length} bytes"
            );
        }
    }
    #[test]
    fn estimator_is_checked_and_never_allocates_the_estimated_tree() {
        assert_eq!(
            estimated_heap_bytes(0),
            Err(PdpMerkleTreeError::EmptyPayload)
        );
        assert_eq!(
            checked_allocation_bytes::<[u8; 32]>(usize::MAX),
            Err(PdpMerkleTreeError::GeometryOverflow)
        );
        let address_space_excess = isize::MAX as usize / size_of::<[u8; 32]>() + 1;
        assert_eq!(
            checked_allocation_bytes::<[u8; 32]>(address_space_excess),
            Err(PdpMerkleTreeError::AllocationFailed)
        );
        assert_eq!(
            geometric_capacity_for_len(usize::MAX),
            Err(PdpMerkleTreeError::GeometryOverflow)
        );
        assert_eq!(
            merkle_total_node_count(usize::MAX),
            Err(PdpMerkleTreeError::GeometryOverflow)
        );
        if usize::BITS >= 64 {
            let estimate = estimated_heap_bytes(u64::MAX)
                .expect("near-u64 geometry remains representable on a 64-bit host");
            assert!(estimate > 1usize << 57);
        } else {
            assert!(matches!(
                estimated_heap_bytes(u64::MAX),
                Err(PdpMerkleTreeError::GeometryOverflow)
                    | Err(PdpMerkleTreeError::AllocationFailed)
            ));
        }
    }
    #[test]
    fn byte_at_a_time_updates_reuse_the_pending_allocation() {
        let payload = deterministic_payload(PDP_SEGMENT_SIZE_V1 as usize + 17);
        let expected = PdpMerkleTreeV1::from_bytes(&payload).expect("single-update tree");
        let mut builder = PdpMerkleTreeBuilderV1::new();
        let mut pending_capacity_changes = 0usize;
        let mut previous_capacity = builder.pending_segment.capacity();
        for byte in &payload {
            builder
                .update(std::slice::from_ref(byte))
                .expect("byte update");
            if builder.pending_segment.capacity() != previous_capacity {
                pending_capacity_changes += 1;
                previous_capacity = builder.pending_segment.capacity();
            }
        }
        assert_eq!(pending_capacity_changes, 1);
        assert!(previous_capacity >= PDP_SEGMENT_SIZE_V1 as usize);
        assert_eq!(builder.finish().expect("byte-streamed tree"), expected);
    }
    #[test]
    fn random_access_proofs_match_slice_wrapper_and_read_each_leaf_once() {
        let payload = deterministic_payload(PDP_SEGMENT_SIZE_V1 as usize * 2 + 113);
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
        let samples = [
            PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: vec![0, 3, 63],
            },
            PdpSampleV1 {
                segment_index: 1,
                hot_leaf_indices: vec![0, 17],
            },
            PdpSampleV1 {
                segment_index: 2,
                hot_leaf_indices: vec![0],
            },
        ];
        let expected = tree
            .prove_samples(&samples, &payload)
            .expect("slice-backed proofs");
        let mut reads = Vec::new();
        let actual = tree
            .prove_samples_with(&samples, |offset, buffer| {
                reads.push((offset, buffer.len()));
                read_payload(&payload, offset, buffer)
            })
            .expect("random-access proofs");
        assert_eq!(actual, expected);
        assert_eq!(
            reads,
            vec![
                (0, PDP_HOT_LEAF_SIZE_V1 as usize),
                (
                    3 * u64::from(PDP_HOT_LEAF_SIZE_V1),
                    PDP_HOT_LEAF_SIZE_V1 as usize,
                ),
                (
                    63 * u64::from(PDP_HOT_LEAF_SIZE_V1),
                    PDP_HOT_LEAF_SIZE_V1 as usize,
                ),
                (
                    u64::from(PDP_SEGMENT_SIZE_V1),
                    PDP_HOT_LEAF_SIZE_V1 as usize,
                ),
                (
                    u64::from(PDP_SEGMENT_SIZE_V1) + 17 * u64::from(PDP_HOT_LEAF_SIZE_V1),
                    PDP_HOT_LEAF_SIZE_V1 as usize,
                ),
                (2 * u64::from(PDP_SEGMENT_SIZE_V1), 113),
            ]
        );
    }
    #[test]
    fn random_access_reader_short_long_and_failed_reads_are_rejected_without_retry() {
        let payload = deterministic_payload(PDP_HOT_LEAF_SIZE_V1 as usize);
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
        let samples = [PdpSampleV1 {
            segment_index: 0,
            hot_leaf_indices: vec![0],
        }];
        for reported in [
            PDP_HOT_LEAF_SIZE_V1 as usize - 1,
            PDP_HOT_LEAF_SIZE_V1 as usize + 1,
        ] {
            let calls = Cell::new(0usize);
            let result = tree.prove_samples_with(&samples, |_, _| {
                calls.set(calls.get() + 1);
                Ok::<usize, io::Error>(reported)
            });
            assert!(matches!(
                result,
                Err(PdpMerkleReadError::ReadLengthMismatch {
                    offset: 0,
                    expected,
                    actual,
                }) if expected == PDP_HOT_LEAF_SIZE_V1 as usize && actual == reported
            ));
            assert_eq!(calls.get(), 1, "a malformed read must not be retried");
        }
        let calls = Cell::new(0usize);
        let result = tree.prove_samples_with(&samples, |_, _| {
            calls.set(calls.get() + 1);
            Err::<usize, _>(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "injected storage denial",
            ))
        });
        match result {
            Err(PdpMerkleReadError::ReadFailed {
                offset,
                length,
                source,
            }) => {
                assert_eq!(offset, 0);
                assert_eq!(length, PDP_HOT_LEAF_SIZE_V1);
                assert_eq!(source.kind(), io::ErrorKind::PermissionDenied);
            }
            other => panic!("expected storage failure, got {other:?}"),
        }
        assert_eq!(calls.get(), 1, "a failed read must not be retried");
    }
    #[test]
    fn random_access_reader_cannot_substitute_tampered_leaf_bytes() {
        let payload = deterministic_payload(PDP_HOT_LEAF_SIZE_V1 as usize + 1);
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
        let samples = [PdpSampleV1 {
            segment_index: 0,
            hot_leaf_indices: vec![0],
        }];
        let result = tree.prove_samples_with(&samples, |offset, buffer| {
            let count = read_payload(&payload, offset, buffer)?;
            buffer[0] ^= 0x80;
            Ok::<usize, io::Error>(count)
        });
        assert!(matches!(
            result,
            Err(PdpMerkleReadError::Tree(
                PdpMerkleTreeError::PayloadDigestMismatch {
                    segment_index: 0,
                    leaf_index: 0,
                }
            ))
        ));
    }
    #[test]
    fn proof_constructor_rejects_noncanonical_and_out_of_bounds_samples_before_reads() {
        let payload = deterministic_payload(PDP_SEGMENT_SIZE_V1 as usize + 1);
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
        assert_eq!(
            rejected_without_reads(&tree, &[]),
            PdpMerkleTreeError::EmptySampleSet
        );
        let too_many_segments = (0..=PDP_MAX_SEGMENT_SAMPLES_V1)
            .map(|index| PdpSampleV1 {
                segment_index: index as u64,
                hot_leaf_indices: vec![0],
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rejected_without_reads(&tree, &too_many_segments),
            PdpMerkleTreeError::TooManySegmentSamples {
                found: PDP_MAX_SEGMENT_SAMPLES_V1 + 1,
            }
        );
        assert_eq!(
            rejected_without_reads(
                &tree,
                &[PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: Vec::new(),
                }],
            ),
            PdpMerkleTreeError::EmptyHotLeafSet { segment_index: 0 }
        );
        assert_eq!(
            rejected_without_reads(
                &tree,
                &[PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![0; PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1 + 1],
                }],
            ),
            PdpMerkleTreeError::TooManyHotLeaves {
                segment_index: 0,
                found: PDP_MAX_HOT_LEAVES_PER_SEGMENT_SAMPLE_V1 + 1,
            }
        );
        for samples in [
            vec![
                PdpSampleV1 {
                    segment_index: 1,
                    hot_leaf_indices: vec![0],
                },
                PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![0],
                },
            ],
            vec![
                PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![0],
                },
                PdpSampleV1 {
                    segment_index: 0,
                    hot_leaf_indices: vec![1],
                },
            ],
        ] {
            assert_eq!(
                rejected_without_reads(&tree, &samples),
                PdpMerkleTreeError::NonCanonicalSegmentOrder
            );
        }
        for hot_leaf_indices in [vec![1, 0], vec![1, 1]] {
            assert_eq!(
                rejected_without_reads(
                    &tree,
                    &[PdpSampleV1 {
                        segment_index: 0,
                        hot_leaf_indices,
                    }],
                ),
                PdpMerkleTreeError::NonCanonicalHotLeafOrder { segment_index: 0 }
            );
        }
        assert_eq!(
            rejected_without_reads(
                &tree,
                &[PdpSampleV1 {
                    segment_index: tree.segment_count(),
                    hot_leaf_indices: vec![0],
                }],
            ),
            PdpMerkleTreeError::SegmentOutOfRange {
                segment_index: tree.segment_count(),
            }
        );
        assert_eq!(
            rejected_without_reads(
                &tree,
                &[PdpSampleV1 {
                    segment_index: 1,
                    hot_leaf_indices: vec![1],
                }],
            ),
            PdpMerkleTreeError::HotLeafOutOfRange {
                segment_index: 1,
                leaf_index: 1,
            }
        );
    }
    #[test]
    fn proof_constructor_enforces_total_leaf_cap_before_reads() {
        let payload = deterministic_payload(PDP_SEGMENT_SIZE_V1 as usize * 17);
        let tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
        let samples = (0..17u64)
            .map(|segment_index| PdpSampleV1 {
                segment_index,
                hot_leaf_indices: (0..PDP_HOT_LEAVES_PER_SEGMENT_V1).collect(),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            rejected_without_reads(&tree, &samples),
            PdpMerkleTreeError::TooManyHotLeavesTotal { found: 17 * 64 }
        );
    }
    #[test]
    fn proof_constructor_rejects_corrupt_retained_layers_before_reader_callback() {
        let payload = deterministic_payload(PDP_HOT_LEAF_SIZE_V1 as usize);
        let mut tree = PdpMerkleTreeV1::from_bytes(&payload).expect("tree");
        tree.hot_nodes = Vec::new().into_boxed_slice();
        let samples = [PdpSampleV1 {
            segment_index: 0,
            hot_leaf_indices: vec![0],
        }];
        assert_eq!(
            rejected_without_reads(&tree, &samples),
            PdpMerkleTreeError::CorruptTree
        );
    }
    #[test]
    fn boundary_split_points_match_single_update_tree_and_proofs() {
        let length = PDP_SEGMENT_SIZE_V1 as usize + PDP_HOT_LEAF_SIZE_V1 as usize * 3 + 29;
        let payload = deterministic_payload(length);
        let expected = PdpMerkleTreeV1::from_bytes(&payload).expect("single-update tree");
        let samples = [
            PdpSampleV1 {
                segment_index: 0,
                hot_leaf_indices: vec![0, 1, 63],
            },
            PdpSampleV1 {
                segment_index: 1,
                hot_leaf_indices: vec![0, 2, 3],
            },
        ];
        let expected_proof = expected
            .prove_samples(&samples, &payload)
            .expect("single-update proofs");
        for split in [1usize, 4_095, 4_096, 4_097, 262_143, 262_144, 262_145] {
            let actual = build_from_chunks([&payload[..split], &payload[split..]]);
            assert_eq!(actual, expected, "tree differs at split {split}");
            assert_eq!(
                actual
                    .prove_samples(&samples, &payload)
                    .expect("streaming proofs"),
                expected_proof,
                "proofs differ at split {split}"
            );
        }
    }
    #[test]
    fn nonaligned_multi_updates_cross_hot_leaf_and_segment_boundaries() {
        let length = PDP_SEGMENT_SIZE_V1 as usize * 3 + PDP_HOT_LEAF_SIZE_V1 as usize + 73;
        let payload = deterministic_payload(length);
        let expected = PdpMerkleTreeV1::from_bytes(&payload).expect("single-update tree");
        let update_sizes = [3usize, 4_090, 11, 65_531, 7, 196_611, 4_099, 257, 8_191];
        let mut builder = PdpMerkleTreeBuilderV1::new();
        let mut offset = 0usize;
        let mut update_index = 0usize;
        while offset < payload.len() {
            let end = offset
                .saturating_add(update_sizes[update_index % update_sizes.len()])
                .min(payload.len());
            builder
                .update(&payload[offset..end])
                .expect("nonaligned update");
            assert_eq!(builder.payload_len(), end as u64);
            offset = end;
            update_index += 1;
        }
        assert_eq!(builder.finish().expect("streaming tree"), expected);
    }
    #[test]
    fn zero_length_updates_are_exact_noops() {
        let payload = deterministic_payload(PDP_SEGMENT_SIZE_V1 as usize + 113);
        let expected = PdpMerkleTreeV1::from_bytes(&payload).expect("single-update tree");
        let split = PDP_HOT_LEAF_SIZE_V1 as usize + 9;
        let mut builder = PdpMerkleTreeBuilderV1::new();
        builder.update(&[]).expect("leading empty update");
        assert_eq!(builder.payload_len(), 0);
        builder
            .update(&payload[..split])
            .expect("first payload update");
        builder.update(&[]).expect("middle empty update");
        assert_eq!(builder.payload_len(), split as u64);
        builder
            .update(&payload[split..])
            .expect("second payload update");
        builder.update(&[]).expect("trailing empty update");
        assert_eq!(builder.finish().expect("streaming tree"), expected);
    }
    #[test]
    fn deterministic_randomized_split_sequences_are_invariant() {
        let payload = deterministic_payload(
            PDP_SEGMENT_SIZE_V1 as usize * 2 + PDP_HOT_LEAF_SIZE_V1 as usize * 2 + 157,
        );
        let expected = PdpMerkleTreeV1::from_bytes(&payload).expect("single-update tree");
        for seed in 0u64..32 {
            let mut state = seed ^ 0xd1b5_4a32_d192_ed03;
            let mut offset = 0usize;
            let mut builder = PdpMerkleTreeBuilderV1::new();
            while offset < payload.len() {
                state = state
                    .wrapping_mul(6_364_136_223_846_793_005)
                    .wrapping_add(1_442_695_040_888_963_407);
                if state & 7 == 0 {
                    builder.update(&[]).expect("random empty update");
                }
                let update_len = ((state >> 17) as usize % 16_385) + 1;
                let end = offset.saturating_add(update_len).min(payload.len());
                builder
                    .update(&payload[offset..end])
                    .expect("randomized update");
                offset = end;
            }
            assert_eq!(
                builder.finish().expect("randomized streaming tree"),
                expected,
                "tree differs for deterministic seed {seed}"
            );
        }
    }
    #[test]
    fn empty_builder_finish_is_rejected() {
        assert_eq!(
            PdpMerkleTreeBuilderV1::new().finish(),
            Err(PdpMerkleTreeError::EmptyPayload)
        );
    }
    #[test]
    fn cumulative_payload_length_overflow_is_rejected_before_consumption() {
        let mut builder = PdpMerkleTreeBuilderV1::new();
        builder.payload_len = u64::MAX;
        assert_eq!(
            builder.update(&[0x5a]),
            Err(PdpMerkleTreeError::GeometryOverflow)
        );
        assert!(builder.pending_segment.is_empty());
        assert_eq!(builder.payload_len(), u64::MAX);
    }
    #[test]
    fn impossible_allocation_is_reported_without_panicking() {
        let mut bytes = Vec::<u8>::new();
        assert_eq!(
            try_reserve_exact(&mut bytes, usize::MAX),
            Err(PdpMerkleTreeError::AllocationFailed)
        );
        assert!(bytes.is_empty());
    }
}
