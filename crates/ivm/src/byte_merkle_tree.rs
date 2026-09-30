//! Canonical byte-chunk Merkle commitments with fixed leaf ownership.

mod canonical_nodes;
use canonical_nodes::CanonicalNodes;

use crate::VMError;
use crate::execution_memory::{ExecutionBuffer, ExecutionMemoryLease, ExecutionMemoryPlan};
use iroha_crypto::{HashOf, MerkleProof, MerkleTree};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::ops::{Deref, DerefMut};
#[cfg(test)]
use std::sync::atomic::AtomicU64;
use std::sync::atomic::{AtomicUsize, Ordering};

/// One fixed leaf backing. Cold nested VMs keep their prepaid execution charge
/// attached to these digests through idle retention and cache eviction.
enum MerkleLeaves {
    Local(crate::cache_memory::OwnedAllocation<[u8; 32]>),
    Funded(ExecutionBuffer<[u8; 32]>),
}

impl From<crate::cache_memory::OwnedAllocation<[u8; 32]>> for MerkleLeaves {
    fn from(leaves: crate::cache_memory::OwnedAllocation<[u8; 32]>) -> Self {
        Self::Local(leaves)
    }
}

impl Deref for MerkleLeaves {
    type Target = [[u8; 32]];

    fn deref(&self) -> &Self::Target {
        match self {
            Self::Local(leaves) => leaves,
            Self::Funded(leaves) => leaves.as_slice(),
        }
    }
}

impl DerefMut for MerkleLeaves {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Self::Local(leaves) => leaves,
            Self::Funded(leaves) => leaves.as_mut_slice(),
        }
    }
}

impl MerkleLeaves {
    #[cfg(any(target_os = "macos", feature = "cuda", test))]
    fn replace_equal_length(&mut self, digests: &[[u8; 32]]) -> bool {
        if self.len() != digests.len() {
            return false;
        }
        self.copy_from_slice(digests);
        true
    }

    fn try_retain(&self) -> bool {
        match self {
            Self::Local(leaves) => leaves.try_retain(),
            Self::Funded(leaves) => leaves.try_retain(),
        }
    }

    fn make_active(&self) {
        match self {
            Self::Local(leaves) => leaves.make_active(),
            Self::Funded(leaves) => leaves.activate(),
        }
    }
}
/// Merkle tree over fixed-size byte chunks, implemented as a thin adaptor over
/// the canonical `iroha_crypto::MerkleTree<[u8;32]>`.
///
/// - Leaves are SHA-256 of `chunk` bytes, with the final chunk zero-padded to
///   `chunk` length. Empty input yields the hash of `chunk` zero bytes.
/// - Inner nodes are SHA-256(left||right) with left-promotion when right is
///   absent.
/// - Canonical node storage is allocated once with the leaves. Updates reuse
///   its fixed geometry for incremental paths or a complete in-place rebuild.
pub struct ByteMerkleTree {
    chunk: usize,
    zero_hash: [u8; 32],
    leaves: Mutex<MerkleLeaves>,
    nodes: Mutex<CanonicalNodes>,
}
static MERKLE_GPU_MIN_LEAVES: AtomicUsize = AtomicUsize::new(8192);
// Mac Metal uses a qualified synthetic cost profile; an explicit CPU ceiling
// still overrides it. Other builds retain the conservative CPU default.
// Thresholds are configurable via `ivm::set_acceleration_config` (threaded from `iroha_config`).
#[cfg(all(target_arch = "aarch64", target_os = "macos", feature = "metal"))]
static MERKLE_AARCH64_CPU_PREFER_MAX_LEAVES: AtomicUsize = AtomicUsize::new(0);
#[cfg(all(
    target_arch = "aarch64",
    not(all(target_os = "macos", feature = "metal"))
))]
static MERKLE_AARCH64_CPU_PREFER_MAX_LEAVES: AtomicUsize = AtomicUsize::new(32_768);
// The same policy applies to x86/x86_64 hosts with SHA-NI.
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    target_os = "macos",
    feature = "metal"
))]
static MERKLE_X86_CPU_PREFER_MAX_LEAVES: AtomicUsize = AtomicUsize::new(0);
#[cfg(all(
    any(target_arch = "x86", target_arch = "x86_64"),
    not(all(target_os = "macos", feature = "metal"))
))]
static MERKLE_X86_CPU_PREFER_MAX_LEAVES: AtomicUsize = AtomicUsize::new(32_768);
// Test-only observation counters; production exposes the same events via telemetry.
#[cfg(test)]
static MERKLE_REBUILDS: AtomicU64 = AtomicU64::new(0);
#[cfg(test)]
static MERKLE_INCREMENTAL_LEAF_UPDATES: AtomicU64 = AtomicU64::new(0);
pub(crate) fn set_merkle_gpu_min_leaves(n: usize) {
    MERKLE_GPU_MIN_LEAVES.store(n.max(1), Ordering::SeqCst);
}
pub(crate) fn merkle_gpu_min_leaves() -> usize {
    MERKLE_GPU_MIN_LEAVES.load(Ordering::SeqCst)
}
// Backend-specific minimum thresholds (default to the generic GPU threshold)
static MERKLE_METAL_MIN_LEAVES: AtomicUsize = AtomicUsize::new(0);
static MERKLE_CUDA_MIN_LEAVES: AtomicUsize = AtomicUsize::new(0);
pub(crate) fn set_merkle_metal_min_leaves(n: usize) {
    MERKLE_METAL_MIN_LEAVES.store(n, Ordering::SeqCst);
}
pub(crate) fn set_merkle_cuda_min_leaves(n: usize) {
    MERKLE_CUDA_MIN_LEAVES.store(n, Ordering::SeqCst);
}
#[inline]
#[cfg(any(target_os = "macos", test))]
fn merkle_metal_min_leaves() -> usize {
    let v = MERKLE_METAL_MIN_LEAVES.load(Ordering::SeqCst);
    if v == 0 { merkle_gpu_min_leaves() } else { v }
}
#[inline]
#[cfg(feature = "cuda")]
fn merkle_cuda_min_leaves() -> usize {
    let v = MERKLE_CUDA_MIN_LEAVES.load(Ordering::SeqCst);
    if v == 0 { merkle_gpu_min_leaves() } else { v }
}
// Test-only helpers to validate configuration wiring.
#[cfg(test)]
pub(crate) fn test_get_metal_min() -> usize {
    merkle_metal_min_leaves()
}
#[cfg(all(test, feature = "cuda"))]
pub(crate) fn test_get_cuda_min() -> usize {
    merkle_cuda_min_leaves()
}
#[inline]
fn cpu_sha2_available_any() -> bool {
    #[cfg(target_arch = "aarch64")]
    {
        if std::arch::is_aarch64_feature_detected!("sha2") {
            return true;
        }
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        if std::is_x86_feature_detected!("sha") {
            return true;
        }
    }
    false
}
#[inline]
fn prefer_cpu_sha2(leaves: usize) -> bool {
    if !cpu_sha2_available_any() {
        return false;
    }
    #[cfg(target_arch = "aarch64")]
    {
        let max = MERKLE_AARCH64_CPU_PREFER_MAX_LEAVES.load(Ordering::SeqCst);
        if leaves <= max {
            return true;
        }
    }
    #[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
    {
        let max = MERKLE_X86_CPU_PREFER_MAX_LEAVES.load(Ordering::SeqCst);
        if leaves <= max {
            return true;
        }
    }
    false
}
#[cfg(target_arch = "aarch64")]
pub(crate) fn set_prefer_cpu_sha2_max_leaves_aarch64(v: usize) {
    MERKLE_AARCH64_CPU_PREFER_MAX_LEAVES.store(v, Ordering::SeqCst);
}
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
pub(crate) fn set_prefer_cpu_sha2_max_leaves_x86(v: usize) {
    MERKLE_X86_CPU_PREFER_MAX_LEAVES.store(v, Ordering::SeqCst);
}
/// Return cumulative completed canonical-node refreshes and incremental leaf updates.
/// Initial construction and independent template copies do not increment these counters.
#[cfg(test)]
pub fn merkle_update_counters() -> (u64, u64) {
    (
        MERKLE_REBUILDS.load(Ordering::Relaxed),
        MERKLE_INCREMENTAL_LEAF_UPDATES.load(Ordering::Relaxed),
    )
}
impl ByteMerkleTree {
    /// Fixed independent leaf and canonical-node backing demand.
    pub(crate) fn memory_plan(leaf_count: usize) -> Result<ExecutionMemoryPlan, VMError> {
        let leaf_count = leaf_count.max(1);
        let mut plan = ExecutionMemoryPlan::array::<[u8; 32]>(leaf_count)
            .map_err(VMError::AllocationDeferred)?;
        plan.include_child(CanonicalNodes::memory_plan(leaf_count)?)
            .map_err(VMError::AllocationDeferred)?;
        Ok(plan)
    }

    pub(crate) fn runtime_template_memory_plan(&self) -> Result<ExecutionMemoryPlan, VMError> {
        Self::memory_plan(self.leaf_count())
    }

    /// Copy leaves and canonical nodes from original prepaid credit when supplied.
    pub(crate) fn try_clone_for_runtime_template(
        &self,
        mut lease: Option<&mut ExecutionMemoryLease>,
    ) -> Result<Self, VMError> {
        let unavailable =
            || VMError::ExecutionDeferred(crate::error::ExecutionDeferral::AllocationUnavailable);
        // The same node-then-leaf lock order protects complete snapshots.
        let _nodes = self.nodes.lock();
        let leaves = self.leaves.lock();
        let copied_leaves = if let Some(lease) = lease.as_deref_mut() {
            let mut copied =
                ExecutionBuffer::new(leaves.len(), lease).map_err(|_| unavailable())?;
            copied.append(&leaves).map_err(|_| unavailable())?;
            MerkleLeaves::Funded(copied)
        } else {
            let mut copied =
                crate::cache_memory::OwnedAllocation::try_filled_copy(leaves.len(), [0; 32])?;
            copied.copy_from_slice(&leaves);
            MerkleLeaves::Local(copied)
        };
        let nodes = CanonicalNodes::from_leaves(&copied_leaves, lease)?;
        Ok(Self {
            chunk: self.chunk,
            zero_hash: self.zero_hash,
            leaves: Mutex::new(copied_leaves),
            nodes: Mutex::new(nodes),
        })
    }

    pub(crate) fn try_retain(&self) -> bool {
        let nodes = self.nodes.lock();
        nodes.try_retain() && self.leaves.lock().try_retain()
    }
    pub(crate) fn make_active(&self) {
        let nodes = self.nodes.lock();
        nodes.make_active();
        self.leaves.lock().make_active();
    }
    fn validate_chunk_size(chunk: usize) -> Result<(), VMError> {
        if !(1..=32).contains(&chunk) {
            return Err(VMError::MemoryOutOfBounds);
        }
        Ok(())
    }
    fn compute_zero_hash(chunk: usize) -> [u8; 32] {
        let buf = [0u8; 32];
        let digest = Sha256::digest(&buf[..chunk]);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }
    fn hash_chunk_padded(&self, data: &[u8]) -> [u8; 32] {
        let mut buf = [0u8; 32];
        buf[..data.len()].copy_from_slice(data);
        if buf[..self.chunk].iter().all(|&b| b == 0) {
            return self.zero_hash;
        }
        // Accelerated one-block SHA-256 using our CPU/GPU paths.
        sha256_oneblock32(&buf[..self.chunk])
    }
    fn refresh_locked(&self, nodes: &mut CanonicalNodes) {
        if nodes.is_current() || !nodes.refresh(&self.leaves.lock()) {
            return;
        }
        #[cfg(test)]
        MERKLE_REBUILDS.fetch_add(1, Ordering::Relaxed);
        iroha_telemetry::metrics::global_or_default()
            .ivm_merkle_rebuild_total
            .inc();
    }

    #[cfg(any(target_os = "macos", feature = "cuda"))]
    fn from_leaf_digests(digests: &[[u8; 32]], chunk: usize) -> Result<Self, VMError> {
        let mut leaves =
            crate::cache_memory::OwnedAllocation::try_filled_copy(digests.len(), [0; 32])?;
        leaves.copy_from_slice(digests);
        let nodes = CanonicalNodes::from_leaves(&leaves, None)?;
        Ok(Self {
            chunk,
            zero_hash: Self::compute_zero_hash(chunk),
            leaves: Mutex::new(leaves.into()),
            nodes: Mutex::new(nodes),
        })
    }

    #[cfg(any(target_os = "macos", feature = "cuda", test))]
    fn install_leaf_digests(&self, digests: &[[u8; 32]]) -> bool {
        let mut nodes = self.nodes.lock();
        let mut leaves = self.leaves.lock();
        if !leaves.replace_equal_length(digests) {
            return false;
        }
        nodes.mark_stale();
        true
    }

    fn checked_leaf_index(&self, index: usize) -> Result<u32, VMError> {
        if index >= self.leaves.lock().len() {
            return Err(VMError::MemoryOutOfBounds);
        }
        u32::try_from(index).map_err(|_| VMError::MemoryOutOfBounds)
    }
    /// Construct a tree from raw bytes, padding the final chunk with zeros.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] unless `chunk` is in `1..=32`;
    /// local leaf-allocation refusal defers execution.
    pub fn from_bytes(data: &[u8], chunk: usize) -> Result<Self, VMError> {
        Self::from_bytes_hashed(data, chunk, false)
    }
    /// Construct fixed leaf and canonical node backing, hashing chunks in parallel.
    ///
    /// # Errors
    /// Returns a bounds error for invalid chunk geometry or a local allocation deferral.
    pub fn from_bytes_parallel(data: &[u8], chunk: usize) -> Result<Self, VMError> {
        Self::from_bytes_hashed(data, chunk, true)
    }
    fn from_bytes_hashed(data: &[u8], chunk: usize, parallel: bool) -> Result<Self, VMError> {
        use rayon::prelude::*;
        Self::validate_chunk_size(chunk)?;
        let zero_hash = Self::compute_zero_hash(chunk);
        let mut leaves = crate::cache_memory::OwnedAllocation::try_filled_copy(
            data.len().div_ceil(chunk).max(1),
            zero_hash,
        )?;
        let hash = |(index, leaf): (usize, &mut [u8; 32])| {
            let start = index * chunk;
            let end = start.saturating_add(chunk).min(data.len());
            let mut bytes = [0; 32];
            if start < end {
                bytes[..end - start].copy_from_slice(&data[start..end]);
            }
            *leaf = if bytes[..chunk].iter().all(|byte| *byte == 0) {
                zero_hash
            } else {
                sha256_oneblock32(&bytes[..chunk])
            };
        };
        if parallel {
            leaves.par_iter_mut().enumerate().for_each(hash);
        } else {
            leaves.iter_mut().enumerate().for_each(hash);
        }
        let nodes = CanonicalNodes::from_leaves(&leaves, None)?;
        Ok(Self {
            chunk,
            zero_hash,
            leaves: Mutex::new(leaves.into()),
            nodes: Mutex::new(nodes),
        })
    }
    /// Whether two trees have the same immutable leaf geometry.
    pub(crate) fn has_same_shape(&self, other: &ByteMerkleTree) -> bool {
        self.chunk == other.chunk && self.leaf_count() == other.leaf_count()
    }
    /// Byte width of one Merkle leaf.
    pub(crate) fn chunk_size(&self) -> usize {
        self.chunk
    }
    /// Restore selected leaves from a shape-compatible immutable template
    /// without cloning the complete leaf vector or canonical tree.
    pub(crate) fn reset_leaves_from(
        &mut self,
        other: &ByteMerkleTree,
        indices: impl IntoIterator<Item = usize>,
    ) {
        assert!(
            self.has_same_shape(other),
            "Merkle template shape must be validated before dirty-leaf reset"
        );
        let mut indices = indices.into_iter().peekable();
        if indices.peek().is_none() {
            return;
        }
        let mut nodes = self.nodes.lock();
        let template = other.leaves.lock();
        let mut leaves = self.leaves.lock();
        for index in indices {
            if let (Some(destination), Some(source)) = (leaves.get_mut(index), template.get(index))
            {
                *destination = *source;
            }
        }
        drop(leaves);
        drop(template);
        // Keep prepaid canonical storage; Memory already knows the template
        // root, so refill those slots only if a root or proof is requested.
        nodes.mark_stale();
    }
    /// Construct a tree from raw bytes using acceleration when beneficial.
    ///
    /// - If a CUDA backend is available and the number of leaves is above a
    ///   threshold, compute leaf digests on the GPU (one padded block per leaf),
    ///   then build the canonical Merkle tree on the CPU.
    /// - Otherwise, fall back to the canonical parallel builder (Rayon-backed)
    ///   or sequential builder.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] unless `chunk` is in `1..=32`.
    pub fn from_bytes_accel(data: &[u8], chunk: usize) -> Result<Self, VMError> {
        Self::validate_chunk_size(chunk)?;
        let leaves_count = data.len().div_ceil(chunk).max(1);
        // Prefer CPU SHA2 on AArch64 for medium sizes to avoid GPU overheads.
        if prefer_cpu_sha2(leaves_count) {
            return Self::from_bytes_parallel(data, chunk);
        }
        // Attempt Metal offload for large trees (macOS)
        #[cfg(target_os = "macos")]
        if leaves_count >= merkle_metal_min_leaves()
            && let Some(selected) = crate::vector::select_metal_merkle(
                crate::vector::MetalMerkleWork::Leaves,
                leaves_count,
            )
        {
            let mut blocks: Vec<[u8; 64]> = Vec::with_capacity(leaves_count);
            let bit_len_be = (chunk as u64 * 8).to_be_bytes();
            for i in 0..leaves_count {
                let start = i * chunk;
                let end = (start + chunk).min(data.len());
                let mut block = [0u8; 64];
                if start < end {
                    let len = end - start;
                    block[..len].copy_from_slice(&data[start..end]);
                }
                block[chunk] = 0x80;
                block[56..64].copy_from_slice(&bit_len_be);
                blocks.push(block);
            }
            if let Some(digests) = selected
                .run(|| crate::vector::metal_sha256_leaves(&blocks))
                .flatten()
                && digests.len() == leaves_count
            {
                return Self::from_leaf_digests(&digests, chunk);
            }
        }
        // CUDA keeps generated padded chunks and complete output in owned host storage.
        #[cfg(feature = "cuda")]
        if leaves_count >= merkle_cuda_min_leaves()
            && let Some(output) =
                crate::cuda::sha256_leaf_chunks_cuda_attempt(data, chunk, leaves_count)
            && output.len() == leaves_count
        {
            // HostOutput stays alive until both destination owners are complete.
            return Self::from_leaf_digests(output.as_slice(), chunk);
        }
        // CPU path (parallel when Rayon is enabled)
        Self::from_bytes_parallel(data, chunk)
    }
    /// Recompute all leaf digests from `data` using acceleration when available.
    /// On success, updates leaves and marks the retained canonical nodes stale.
    /// Returns true if acceleration was used, false otherwise (no changes made).
    pub(crate) fn recompute_all_leaves_accel(&self, data: &[u8]) -> bool {
        let leaves_count = self.leaf_count();
        // Prefer CPU SHA2 path on AArch64 for medium sizes
        if prefer_cpu_sha2(leaves_count) || leaves_count < merkle_gpu_min_leaves() {
            return false;
        }
        #[cfg(target_os = "macos")]
        if leaves_count >= merkle_metal_min_leaves()
            && let Some(selected) = crate::vector::select_metal_merkle(
                crate::vector::MetalMerkleWork::Leaves,
                leaves_count,
            )
        {
            let bit_len_be = (self.chunk as u64 * 8).to_be_bytes();
            let mut blocks: Vec<[u8; 64]> = Vec::with_capacity(leaves_count);
            for i in 0..leaves_count {
                let start = i * self.chunk;
                let end = (start + self.chunk).min(data.len());
                let mut block = [0u8; 64];
                if start < end {
                    block[..end - start].copy_from_slice(&data[start..end]);
                }
                block[self.chunk] = 0x80;
                block[56..64].copy_from_slice(&bit_len_be);
                blocks.push(block);
            }
            if let Some(digests) = selected
                .run(|| crate::vector::metal_sha256_leaves(&blocks))
                .flatten()
            {
                return self.install_leaf_digests(&digests);
            }
        }
        #[cfg(feature = "cuda")]
        if let Some(output) =
            crate::cuda::sha256_leaf_chunks_cuda_attempt(data, self.chunk, leaves_count)
        {
            // Validate exact geometry, then copy the full HostOutput while its
            // original allocation owner is still alive. Canonical nodes remain prepaid.
            return self.install_leaf_digests(output.as_slice());
        }
        false
    }
    /// Rehash every fixed leaf in place on the CPU. A large memory commit must
    /// keep the original prepaid backing instead of replacing the whole tree.
    pub(crate) fn recompute_all_leaves_parallel(&self, data: &[u8]) {
        use rayon::prelude::*;

        let chunk = self.chunk;
        let zero_hash = self.zero_hash;
        let mut nodes = self.nodes.lock();
        let mut leaves = self.leaves.lock();
        leaves.par_iter_mut().enumerate().for_each(|(index, leaf)| {
            let start = index.saturating_mul(chunk);
            let end = start.saturating_add(chunk).min(data.len());
            let mut buf = [0u8; 32];
            if start < end {
                buf[..end - start].copy_from_slice(&data[start..end]);
            }
            *leaf = if buf[..chunk].iter().all(|&byte| byte == 0) {
                zero_hash
            } else {
                sha256_oneblock32(&buf[..chunk])
            };
        });
        nodes.mark_stale();
    }
    /// Construct a zero-filled tree with at least one leaf.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] unless `chunk` is in `1..=32`.
    pub fn new(num_leaves: usize, chunk: usize) -> Result<Self, VMError> {
        Self::validate_chunk_size(chunk)?;
        let zero_hash = Self::compute_zero_hash(chunk);
        let leaves =
            crate::cache_memory::OwnedAllocation::try_filled_copy(num_leaves.max(1), zero_hash)?;
        let nodes = CanonicalNodes::new(num_leaves.max(1), zero_hash, None)?;
        Ok(ByteMerkleTree {
            chunk,
            zero_hash,
            leaves: Mutex::new(leaves.into()),
            nodes: Mutex::new(nodes),
        })
    }
    /// Construct fixed zero leaves from credit reserved before either guest
    /// image or leaf backing is allocated.
    pub(crate) fn new_funded(
        num_leaves: usize,
        chunk: usize,
        lease: &mut ExecutionMemoryLease,
    ) -> Result<Self, VMError> {
        Self::validate_chunk_size(chunk)?;
        let zero_hash = Self::compute_zero_hash(chunk);
        let leaf_count = num_leaves.max(1);
        let mut leaves = ExecutionBuffer::new(leaf_count, lease).map_err(|_| {
            VMError::ExecutionDeferred(crate::error::ExecutionDeferral::AllocationUnavailable)
        })?;
        let zeros = [zero_hash; 128];
        while leaves.as_slice().len() < leaf_count {
            let remaining = leaf_count - leaves.as_slice().len();
            leaves
                .append(&zeros[..remaining.min(zeros.len())])
                .expect("fixed prepaid leaf backing has sufficient capacity");
        }
        let nodes = CanonicalNodes::new(leaf_count, zero_hash, Some(lease))?;
        Ok(Self {
            chunk,
            zero_hash,
            leaves: Mutex::new(MerkleLeaves::Funded(leaves)),
            nodes: Mutex::new(nodes),
        })
    }
    pub(crate) fn leaf_count(&self) -> usize {
        self.leaves.lock().len()
    }
    /// Return the canonical Merkle root as a typed hash.
    pub fn root_hash(&self) -> HashOf<MerkleTree<[u8; 32]>> {
        let mut nodes = self.nodes.lock();
        self.refresh_locked(&mut nodes);
        nodes.root().expect("tree has at least one leaf")
    }
    /// Retrieve the canonical Merkle proof for the leaf at `index`.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] when `index` is not an exact leaf index or cannot be
    /// represented by the canonical proof format.
    pub fn proof(&self, index: usize) -> Result<MerkleProof<[u8; 32]>, VMError> {
        let index = self.checked_leaf_index(index)?;
        let mut nodes = self.nodes.lock();
        self.refresh_locked(&mut nodes);
        nodes.get_proof(index).ok_or(VMError::MemoryOutOfBounds)
    }
    /// Combined helper returning both the root hash and the Merkle proof.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] when `index` is not an exact leaf index or cannot be
    /// represented by the canonical proof format.
    pub fn root_and_proof(
        &self,
        index: usize,
    ) -> Result<(HashOf<MerkleTree<[u8; 32]>>, MerkleProof<[u8; 32]>), VMError> {
        let index = self.checked_leaf_index(index)?;
        let mut nodes = self.nodes.lock();
        self.refresh_locked(&mut nodes);
        let root = nodes.root().expect("tree has at least one leaf");
        let proof = nodes.get_proof(index).ok_or(VMError::MemoryOutOfBounds)?;
        Ok((root, proof))
    }
    pub fn root(&self) -> [u8; 32] {
        let root = self.root_hash();
        *root.as_ref()
    }
    /// Update leaf at `index` with new chunk bytes.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] when `index` is not an exact leaf index or `data`
    /// is larger than the configured leaf width.
    pub fn update_leaf(&self, index: usize, data: &[u8]) -> Result<(), VMError> {
        if data.len() > self.chunk {
            return Err(VMError::MemoryOutOfBounds);
        }
        self.checked_leaf_index(index)?;
        let digest = self.hash_chunk_padded(data);
        let mut nodes = self.nodes.lock();
        self.leaves.lock()[index] = digest;
        if nodes.is_current() {
            nodes.update_leaf_digest(index, digest);
        }
        Ok(())
    }
    /// Authentication path for leaf at `index`.
    ///
    /// Ordering: returns siblings from leaf → root. Each entry is the sibling
    /// hash at the corresponding tree level (missing siblings encoded as all‑zero).
    /// Keep this convention in sync with canonical proof consumers and tests.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] when `index` is not an exact leaf index or cannot be
    /// represented by the canonical proof format.
    pub fn path(&self, index: usize) -> Result<Vec<[u8; 32]>, VMError> {
        let proof = self.proof(index)?;
        let path = proof
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect::<Vec<_>>();
        if crate::dev_env::debug_compact_enabled() {
            eprintln!("[path] index={} depth={}", index, path.len());
        }
        Ok(path)
    }
    /// Rehash the original memory bitmap directly into fixed leaf backing.
    ///
    /// Sparse writes touch only their set bits; larger updates hash disjoint
    /// groups of 64 leaves in parallel. Cached paths are updated in ascending
    /// index order. No index or digest vector is copied or allocated here.
    pub(crate) fn update_dirty_leaves_from_bytes(
        &mut self,
        data: &[u8],
        dirty: &crate::memory::dirty_chunks::DirtyChunks,
    ) {
        use rayon::prelude::*;

        // Root/proof refresh takes nodes then leaves; every mutation follows
        // the same order so a query cannot publish mismatched representations.
        let mut nodes = self.nodes.lock();
        let mut leaves = self.leaves.lock();
        assert_eq!(dirty.chunks(), leaves.len(), "dirty-leaf geometry mismatch");
        if dirty.is_empty() {
            return;
        }
        let chunk = self.chunk;
        let zero_hash = self.zero_hash;
        let digest = |index: usize| {
            let start = index.saturating_mul(chunk);
            let end = start.saturating_add(chunk).min(data.len());
            let mut bytes = [0_u8; 32];
            if start < end {
                bytes[..end - start].copy_from_slice(&data[start..end]);
            }
            if bytes[..chunk].iter().all(|&byte| byte == 0) {
                zero_hash
            } else {
                sha256_oneblock32(&bytes[..chunk])
            }
        };
        if dirty.len() < 256 {
            for index in dirty.iter() {
                leaves[index] = digest(index);
            }
        } else {
            leaves
                .par_chunks_mut(64)
                .zip(dirty.words().par_iter())
                .enumerate()
                .for_each(|(word_index, (group, word))| {
                    let mut remaining = *word;
                    while remaining != 0 {
                        let offset = remaining.trailing_zeros() as usize;
                        remaining &= remaining - 1;
                        group[offset] = digest(word_index * 64 + offset);
                    }
                });
        }
        if nodes.is_current() {
            for index in dirty.iter() {
                nodes.update_leaf_digest(index, leaves[index]);
            }
            let updated = dirty.len() as u64;
            #[cfg(test)]
            MERKLE_INCREMENTAL_LEAF_UPDATES.fetch_add(updated, Ordering::Relaxed);
            iroha_telemetry::metrics::global_or_default()
                .ivm_merkle_incremental_leaf_updates_total
                .inc_by(updated);
        }
    }
    /// Return both the Merkle root and the authentication path for `index`.
    /// Refreshes retained canonical nodes in place at most once.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] when `index` is not an exact leaf index or cannot be
    /// represented by the canonical proof format.
    pub fn root_and_path(
        &self,
        index: usize,
    ) -> Result<(HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>), VMError> {
        let (root, proof) = self.root_and_proof(index)?;
        let path = proof
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        Ok((root, path))
    }
    /// Compute a Merkle root directly from raw bytes using acceleration for both leaf hashing
    /// and inner-node reduction when beneficial. Falls back to CPU for small trees or when
    /// accelerators are unavailable.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] unless `chunk` is in `1..=32`.
    pub fn root_from_bytes_accel(data: &[u8], chunk: usize) -> Result<[u8; 32], VMError> {
        Self::validate_chunk_size(chunk)?;
        let leaves_count = data.len().div_ceil(chunk).max(1);
        // Prefer CPU SHA2 on AArch64 for medium sizes
        if prefer_cpu_sha2(leaves_count) {
            let canonical =
                MerkleTree::<[u8; 32]>::from_byte_chunks(data, chunk).expect("valid chunk");
            let metrics = iroha_telemetry::metrics::global_or_default();
            metrics.merkle_root_cpu_total.inc();
            return Ok(*canonical.root().expect("non-empty").as_ref());
        }
        // GPU Metal path (macOS)
        #[cfg(target_os = "macos")]
        if leaves_count >= merkle_metal_min_leaves()
            && let Some(selected) = crate::vector::select_metal_merkle(
                crate::vector::MetalMerkleWork::Root,
                leaves_count,
            )
        {
            let mut blocks: Vec<[u8; 64]> = Vec::with_capacity(leaves_count);
            let bit_len_be = (chunk as u64 * 8).to_be_bytes();
            for i in 0..leaves_count {
                let start = i * chunk;
                let end = (start + chunk).min(data.len());
                let mut block = [0u8; 64];
                if start < end {
                    let len = end - start;
                    block[..len].copy_from_slice(&data[start..end]);
                }
                block[chunk] = 0x80;
                block[56..64].copy_from_slice(&bit_len_be);
                blocks.push(block);
            }
            if let Some(root) = selected
                .run(|| {
                    let digests = crate::vector::metal_sha256_leaves(&blocks)?;
                    crate::vector::metal_merkle_root(&digests)
                })
                .flatten()
            {
                // Telemetry: GPU merkle root
                let metrics = iroha_telemetry::metrics::global_or_default();
                metrics.merkle_root_gpu_total.inc();
                return Ok(root);
            }
        }
        // GPU CUDA path
        #[cfg(feature = "cuda")]
        let use_cuda = {
            let min_cuda = merkle_cuda_min_leaves();
            crate::cuda_available() && leaves_count >= min_cuda
        };
        #[cfg(feature = "cuda")]
        if use_cuda {
            if let Some(root) = crate::cuda::sha256_merkle_root_cuda(data, chunk) {
                let metrics = iroha_telemetry::metrics::global_or_default();
                metrics.merkle_root_gpu_total.inc();
                return Ok(root);
            }
        }
        // CPU fallback
        let canonical = MerkleTree::<[u8; 32]>::from_byte_chunks(data, chunk).expect("valid chunk");
        let metrics = iroha_telemetry::metrics::global_or_default();
        metrics.merkle_root_cpu_total.inc();
        Ok(*canonical.root().expect("non-empty").as_ref())
    }
}
#[inline]
pub(crate) fn sha256_oneblock32(input: &[u8]) -> [u8; 32] {
    debug_assert!(input.len() <= 32);
    // IV
    let mut state = [
        0x6a09e667u32,
        0xbb67ae85,
        0x3c6ef372,
        0xa54ff53a,
        0x510e527f,
        0x9b05688c,
        0x1f83d9ab,
        0x5be0cd19,
    ];
    // Build a single padded block (fits since input <= 55 bytes, here <= 32)
    let mut block = [0u8; 64];
    let len = input.len();
    block[..len].copy_from_slice(input);
    block[len] = 0x80;
    let bit_len_be = (len as u64 * 8).to_be_bytes();
    block[56..64].copy_from_slice(&bit_len_be);
    // Use accelerated sha256_compress (Metal/CUDA/ARM SHA2/x86 SHA-NI/scalar)
    crate::vector::sha256_compress(&mut state, &block);
    let mut out = [0u8; 32];
    for (i, w) in state.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(&w.to_be_bytes());
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    use iroha_allocation::AllocationBudget;

    #[test]
    fn funded_leaves_keep_prepaid_owner_and_match_local_updates() {
        let plan = ByteMerkleTree::memory_plan(4).unwrap();
        let total_bytes = plan.requested_bytes();
        let budget = AllocationBudget::new(total_bytes);
        let mut lease = ExecutionMemoryLease::reserve(&budget, plan).unwrap();
        let funded = ByteMerkleTree::new_funded(4, 32, &mut lease).unwrap();
        drop(lease);
        assert_eq!(budget.reserved_bytes(), total_bytes);
        let local = ByteMerkleTree::new(4, 32).unwrap();
        assert_eq!(funded.root_hash(), local.root_hash());
        funded.update_leaf(2, &[0x5a; 32]).unwrap();
        local.update_leaf(2, &[0x5a; 32]).unwrap();
        assert_eq!(funded.root_hash(), local.root_hash());
        let digests = funded.leaves.lock().to_vec();
        assert!(funded.leaves.lock().replace_equal_length(&digests));
        assert!(!funded.leaves.lock().replace_equal_length(&digests[..3]));
        assert_eq!(budget.reserved_bytes(), total_bytes);
        let mut data = [0u8; 128];
        data[0] = 0x39;
        data[127] = 0xb7;
        funded.recompute_all_leaves_parallel(&data);
        local.recompute_all_leaves_parallel(&data);
        let expected = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32)
            .unwrap()
            .root()
            .unwrap();
        assert_eq!(funded.root_hash(), expected);
        assert_eq!(local.root_hash(), expected);
        assert_eq!(budget.reserved_bytes(), total_bytes);
        let cloned = funded.try_clone_for_runtime_template(None).unwrap();
        assert_eq!(cloned.root_hash(), funded.root_hash());
        assert_eq!(budget.reserved_bytes(), total_bytes);
        drop(funded);
        assert_eq!(budget.reserved_bytes(), 0);
        assert_eq!(cloned.root_hash(), local.root_hash());
    }

    #[test]
    fn fallible_template_tree_copy_preserves_root_and_independent_leaves() {
        let source = ByteMerkleTree::new(3, 32).expect("bounded test tree");
        source.update_leaf(1, &[0x5a; 32]).expect("update source");
        let copied = source
            .try_clone_for_runtime_template(None)
            .expect("bounded tree copy");
        assert_eq!(copied.root_hash(), source.root_hash());
        copied.update_leaf(1, &[0x33; 32]).expect("update copy");
        assert_ne!(copied.root_hash(), source.root_hash());
    }

    #[test]
    fn constructor_defers_unrepresentable_leaf_storage() {
        assert!(matches!(
            ByteMerkleTree::new(usize::MAX, 32),
            Err(VMError::ExecutionDeferred(
                crate::error::ExecutionDeferral::AllocationUnavailable
            ))
        ));
    }
    #[test]
    fn sha256_oneblock32_matches_sha2() {
        for &len in &[0usize, 1, 7, 16, 31, 32] {
            let mut v = vec![0u8; len];
            for (i, b) in v.iter_mut().enumerate() {
                *b = (i as u8).wrapping_mul(13).wrapping_add(5);
            }
            let ours = sha256_oneblock32(&v);
            let theirs = sha2::Sha256::digest(&v);
            assert_eq!(ours.as_slice(), &theirs[..]);
        }
    }
    #[test]
    fn root_from_bytes_accel_matches_canonical() {
        let samples: &[(&[u8], usize)] = &[
            (b"", 32),
            (b"a", 1),
            (b"hello world", 16),
            (&[0u8; 100], 32),
        ];
        for &(data, chunk) in samples {
            let accel = ByteMerkleTree::root_from_bytes_accel(data, chunk).unwrap();
            let canonical = iroha_crypto::MerkleTree::<[u8; 32]>::from_byte_chunks(data, chunk)
                .expect("valid chunk");
            let expected = *canonical.root().expect("non-empty").as_ref();
            assert_eq!(accel, expected);
        }
    }
    #[test]
    fn large_accelerated_merkle_paths_match_canonical() {
        let mut data = vec![0u8; 8_193 * 32];
        for (index, byte) in data.iter_mut().enumerate() {
            *byte = (index as u8).wrapping_mul(17).wrapping_add(3);
        }
        let canonical = MerkleTree::<[u8; 32]>::from_byte_chunks(&data, 32).unwrap();
        let expected = *canonical.root().unwrap().as_ref();
        assert_eq!(
            ByteMerkleTree::root_from_bytes_accel(&data, 32).unwrap(),
            expected
        );
        assert_eq!(
            ByteMerkleTree::from_bytes_accel(&data, 32).unwrap().root(),
            expected
        );
    }
    #[test]
    fn recompute_all_leaves_accel_small_tree_returns_false() {
        let data = b"small";
        let chunk = 32;
        let tree = ByteMerkleTree::from_bytes(data, chunk).unwrap();
        // Below GPU threshold; must return false (no acceleration used)
        assert!(!tree.recompute_all_leaves_accel(data));
    }
    #[test]
    fn set_per_backend_thresholds_applied() {
        // Capture current values
        let metal0 = test_get_metal_min();
        #[cfg(feature = "cuda")]
        let cuda0 = test_get_cuda_min();
        // Apply new thresholds via public API
        crate::set_acceleration_config(crate::AccelerationConfig {
            resource_limits: iroha_accel::RegistryLimits::STANDARD,
            enable_simd: true,
            enable_metal: true,
            enable_cuda: true,
            max_gpus: None,
            merkle_min_leaves_gpu: None,
            merkle_min_leaves_metal: Some(1234),
            merkle_min_leaves_cuda: Some(5678),
            prefer_cpu_sha2_max_leaves_aarch64: None,
            prefer_cpu_sha2_max_leaves_x86: None,
        });
        assert_eq!(test_get_metal_min(), 1234);
        #[cfg(feature = "cuda")]
        {
            assert_eq!(test_get_cuda_min(), 5678);
        }
        // Restore
        set_merkle_metal_min_leaves(metal0);
        #[cfg(feature = "cuda")]
        {
            set_merkle_cuda_min_leaves(cuda0);
        }
    }
    #[test]
    fn reset_from_restores_state() {
        let data_a = vec![0u8; 96];
        let data_b = vec![1u8; 96];
        let source = ByteMerkleTree::from_bytes(&data_a, 32).unwrap();
        let other = ByteMerkleTree::from_bytes(&data_b, 32).unwrap();
        other.update_leaf(1, &[5u8; 32]).unwrap();
        let mut target = ByteMerkleTree::new(3, 32).unwrap();
        target.reset_leaves_from(&other, 0..3);
        assert_eq!(target.root(), other.root());
        target.reset_leaves_from(&source, 0..3);
        assert_eq!(target.root(), source.root());
    }
    #[test]
    fn selected_leaf_reset_restores_template_without_tree_clone() {
        let template = ByteMerkleTree::from_bytes(&[0u8; 128], 32).unwrap();
        let mut worker = template.try_clone_for_runtime_template(None).unwrap();
        assert!(worker.has_same_shape(&template));
        assert_eq!(worker.chunk_size(), 32);
        assert!(!worker.has_same_shape(&ByteMerkleTree::new(4, 16).unwrap()));
        worker.update_leaf(1, &[0xAA; 32]).unwrap();
        worker.update_leaf(3, &[0x55; 32]).unwrap();
        assert_ne!(worker.root(), template.root());
        worker.reset_leaves_from(&template, [3, 1, 3, 1]);
        assert_eq!(worker.root(), template.root());
    }
}

#[cfg(test)]
mod dirty_bitmap_tests;

#[cfg(test)]
mod canonical_node_tests;
