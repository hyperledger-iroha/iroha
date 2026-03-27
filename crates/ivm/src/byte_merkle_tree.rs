use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};

use iroha_crypto::{HashOf, MerkleProof, MerkleTree};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};

/// Merkle tree over fixed-size byte chunks, implemented as a thin adaptor over
/// the canonical `iroha_crypto::MerkleTree<[u8;32]>`.
///
/// - Leaves are SHA-256 of `chunk` bytes, with the final chunk zero-padded to
///   `chunk` length. Empty input yields the hash of `chunk` zero bytes.
/// - Inner nodes are SHA-256(left||right) with left-promotion when right is
///   absent.
/// - Updates only modify the stored leaf digests; the underlying Merkle tree is
///   rebuilt lazily on demand for `root()` and `path()`.
pub struct ByteMerkleTree {
    chunk: usize,
    zero_hash: [u8; 32],
    leaves: Mutex<Vec<[u8; 32]>>,
    cached: Mutex<Option<MerkleTree<[u8; 32]>>>,
}

impl Clone for ByteMerkleTree {
    fn clone(&self) -> Self {
        let leaves = self.leaves.lock().clone();
        ByteMerkleTree {
            chunk: self.chunk,
            zero_hash: self.zero_hash,
            leaves: Mutex::new(leaves),
            cached: Mutex::new(None),
        }
    }
}

static MERKLE_GPU_MIN_LEAVES: AtomicUsize = AtomicUsize::new(8192);
// On AArch64 with ARMv8 SHA2, CPU can outperform GPU for medium trees.
// Prefer CPU hashing for trees up to this many leaves before considering GPU offload.
// Thresholds are configurable via `ivm::set_acceleration_config` (threaded from `iroha_config`).
#[cfg(target_arch = "aarch64")]
static MERKLE_AARCH64_CPU_PREFER_MAX_LEAVES: AtomicUsize = AtomicUsize::new(32_768);
// On x86/x86_64 with Intel SHA-NI, prefer CPU hashing for medium-size trees too.
#[cfg(any(target_arch = "x86", target_arch = "x86_64"))]
static MERKLE_X86_CPU_PREFER_MAX_LEAVES: AtomicUsize = AtomicUsize::new(32_768);
#[allow(dead_code)]
static MERKLE_GPU_ROOTS: AtomicU64 = AtomicU64::new(0);
#[allow(dead_code)]
static MERKLE_CPU_ROOTS: AtomicU64 = AtomicU64::new(0);

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

#[allow(dead_code)]
pub fn merkle_root_counters() -> (u64, u64) {
    (
        MERKLE_GPU_ROOTS.load(Ordering::Relaxed),
        MERKLE_CPU_ROOTS.load(Ordering::Relaxed),
    )
}

impl ByteMerkleTree {
    fn compute_zero_hash(chunk: usize) -> [u8; 32] {
        let buf = [0u8; 32];
        let digest = Sha256::digest(&buf[..chunk]);
        let mut out = [0u8; 32];
        out.copy_from_slice(&digest);
        out
    }

    fn hash_chunk_padded(&self, data: &[u8]) -> [u8; 32] {
        let mut buf = [0u8; 32];
        let len = data.len().min(self.chunk);
        buf[..len].copy_from_slice(&data[..len]);
        if buf[..self.chunk].iter().all(|&b| b == 0) {
            return self.zero_hash;
        }
        // Accelerated one-block SHA-256 using our CPU/GPU paths.
        sha256_oneblock32(&buf[..self.chunk])
    }

    fn rebuild_locked(&self, cached: &mut Option<MerkleTree<[u8; 32]>>) {
        let leaves = self.leaves.lock().clone();
        *cached = Some(MerkleTree::from_hashed_leaves_sha256(leaves));
    }

    /// Construct a tree from raw bytes, padding the final chunk with zeros.
    pub fn from_bytes(data: &[u8], chunk: usize) -> Self {
        assert!(chunk > 0 && chunk <= 32);
        let zero_hash = Self::compute_zero_hash(chunk);
        // Build via canonical helper to keep hashing/padding semantics exactly the same.
        let canonical = MerkleTree::<[u8; 32]>::from_byte_chunks(data, chunk).expect("valid chunk");
        // Extract leaves as raw bytes (HashOf::as_ref bytes) for update paths.
        let leaves: Vec<[u8; 32]> = canonical
            .leaves()
            .map(|h| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(h.as_ref());
                arr
            })
            .collect();
        ByteMerkleTree {
            chunk,
            zero_hash,
            leaves: Mutex::new(leaves),
            cached: Mutex::new(Some(canonical)),
        }
    }

    /// Construct a tree from raw bytes in parallel using Rayon.
    pub fn from_bytes_parallel(data: &[u8], chunk: usize) -> Self {
        assert!(chunk > 0 && chunk <= 32);
        let zero_hash = Self::compute_zero_hash(chunk);
        // Prefer canonical parallel helper when available; otherwise fall back to sequential.
        let canonical =
            MerkleTree::<[u8; 32]>::from_chunked_bytes_parallel(data, chunk).expect("valid chunk");
        let leaves: Vec<[u8; 32]> = canonical
            .leaves()
            .map(|h| {
                let mut arr = [0u8; 32];
                arr.copy_from_slice(h.as_ref());
                arr
            })
            .collect();
        ByteMerkleTree {
            chunk,
            zero_hash,
            leaves: Mutex::new(leaves),
            cached: Mutex::new(Some(canonical)),
        }
    }

    pub(crate) fn reset_from(&mut self, other: &ByteMerkleTree) {
        self.chunk = other.chunk;
        self.zero_hash = other.zero_hash;
        let leaves_copy = { other.leaves.lock().clone() };
        *self.leaves.lock() = leaves_copy;
        let cached_copy = { other.cached.lock().clone() };
        *self.cached.lock() = cached_copy;
    }

    /// Construct a tree from raw bytes using acceleration when beneficial.
    ///
    /// - If a CUDA backend is available and the number of leaves is above a
    ///   threshold, compute leaf digests on the GPU (one padded block per leaf),
    ///   then build the canonical Merkle tree on the CPU.
    /// - Otherwise, fall back to the canonical parallel builder (Rayon-backed)
    ///   or sequential builder.
    pub fn from_bytes_accel(data: &[u8], chunk: usize) -> Self {
        assert!(chunk > 0 && chunk <= 32);
        #[cfg(any(target_os = "macos", feature = "cuda"))]
        let zero_hash = Self::compute_zero_hash(chunk);
        let leaves_count = data.len().div_ceil(chunk).max(1);

        // Prefer CPU SHA2 on AArch64 for medium sizes to avoid GPU overheads.
        if prefer_cpu_sha2(leaves_count) {
            return Self::from_bytes_parallel(data, chunk);
        }

        // Attempt Metal offload for large trees (macOS)
        #[cfg(target_os = "macos")]
        if crate::vector::metal_available() && leaves_count >= merkle_metal_min_leaves() {
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
            if let Some(digests) = crate::vector::metal_sha256_leaves(&blocks) {
                let canonical = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests.clone());
                return ByteMerkleTree {
                    chunk,
                    zero_hash,
                    leaves: Mutex::new(digests),
                    cached: Mutex::new(Some(canonical)),
                };
            }
        }

        // Attempt CUDA offload for large trees
        #[cfg(feature = "cuda")]
        {
            let bit_len_be = (chunk as u64 * 8).to_be_bytes();
            let make_blocks = || {
                let mut blocks: Vec<[u8; 64]> = Vec::with_capacity(leaves_count);
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
                blocks
            };
            if crate::cuda_available()
                && leaves_count >= merkle_cuda_min_leaves()
                && let Some(digests) = {
                    let blocks = make_blocks();
                    crate::cuda::sha256_leaves_cuda(&blocks)
                }
            {
                let canonical = MerkleTree::<[u8; 32]>::from_hashed_leaves_sha256(digests.clone());
                return ByteMerkleTree {
                    chunk,
                    zero_hash,
                    leaves: Mutex::new(digests),
                    cached: Mutex::new(Some(canonical)),
                };
            }
        }

        // CPU path (parallel when Rayon is enabled)
        Self::from_bytes_parallel(data, chunk)
    }

    /// Recompute all leaf digests from `data` using acceleration when available.
    /// On success, updates internal hashed leaves and invalidates the cached tree.
    /// Returns true if acceleration was used, false otherwise (no changes made).
    pub(crate) fn recompute_all_leaves_accel(&self, data: &[u8]) -> bool {
        let leaves_count = data.len().div_ceil(self.chunk).max(1);
        // Prefer CPU SHA2 path on AArch64 for medium sizes
        if prefer_cpu_sha2(leaves_count) || leaves_count < merkle_gpu_min_leaves() {
            return false;
        }
        let bit_len_be = (self.chunk as u64 * 8).to_be_bytes();
        let mut blocks: Vec<[u8; 64]> = Vec::with_capacity(leaves_count);
        for i in 0..leaves_count {
            let start = i * self.chunk;
            let end = (start + self.chunk).min(data.len());
            let mut block = [0u8; 64];
            if start < end {
                let len = end - start;
                block[..len].copy_from_slice(&data[start..end]);
            }
            block[self.chunk] = 0x80;
            block[56..64].copy_from_slice(&bit_len_be);
            blocks.push(block);
        }
        // Try Metal first
        #[cfg(target_os = "macos")]
        if crate::vector::metal_available()
            && let Some(digests) = crate::vector::metal_sha256_leaves(&blocks)
        {
            let mut leaves = self.leaves.lock();
            *leaves = digests;
            *self.cached.lock() = None;
            return true;
        }
        // Then CUDA
        #[cfg(feature = "cuda")]
        if crate::cuda_available()
            && let Some(digests) = crate::cuda::sha256_leaves_cuda(&blocks)
        {
            let mut leaves = self.leaves.lock();
            *leaves = digests;
            *self.cached.lock() = None;
            return true;
        }
        false
    }

    pub fn new(num_leaves: usize, chunk: usize) -> Self {
        assert!(chunk > 0 && chunk <= 32);
        let zero_hash = Self::compute_zero_hash(chunk);
        let leaves = vec![zero_hash; num_leaves.max(1)];
        ByteMerkleTree {
            chunk,
            zero_hash,
            leaves: Mutex::new(leaves),
            cached: Mutex::new(None),
        }
    }

    /// Return the canonical Merkle root as a typed hash.
    pub fn root_hash(&self) -> HashOf<MerkleTree<[u8; 32]>> {
        let mut cached = self.cached.lock();
        if cached.is_none() {
            self.rebuild_locked(&mut cached);
        }
        cached
            .as_ref()
            .expect("cached tree exists")
            .root()
            .expect("tree has at least one leaf")
    }

    /// Retrieve the canonical Merkle proof for the leaf at `index`.
    pub fn proof(&self, index: usize) -> MerkleProof<[u8; 32]> {
        let mut cached = self.cached.lock();
        if cached.is_none() {
            self.rebuild_locked(&mut cached);
        }
        cached
            .as_ref()
            .expect("cached tree exists")
            .get_proof(index as u32)
            .expect("valid index")
    }

    /// Combined helper returning both the root hash and the Merkle proof.
    pub fn root_and_proof(
        &self,
        index: usize,
    ) -> (HashOf<MerkleTree<[u8; 32]>>, MerkleProof<[u8; 32]>) {
        let mut cached = self.cached.lock();
        if cached.is_none() {
            self.rebuild_locked(&mut cached);
        }
        let tree = cached.as_ref().expect("cached tree exists");
        let root = tree.root().expect("tree has at least one leaf");
        let proof = tree.get_proof(index as u32).expect("valid index");
        (root, proof)
    }

    pub fn root(&self) -> [u8; 32] {
        let root = self.root_hash();
        *root.as_ref()
    }

    /// Update leaf at `index` with new chunk bytes.
    pub fn update_leaf(&self, index: usize, data: &[u8]) {
        // Compute the new digest for this chunk.
        let digest = self.hash_chunk_padded(data);
        {
            // Update the stored leaf digest.
            let mut leaves = self.leaves.lock();
            leaves[index] = digest;
            // Drop lock before touching cached tree to avoid lock-order inversion
            // with callers that may hold `cached` and attempt to lock `leaves`.
        }
        // If a cached tree exists, apply an incremental update; otherwise, leave
        // it invalid so the next query performs a single full rebuild.
        let mut cached = self.cached.lock();
        if let Some(tree) = cached.as_mut() {
            tree.update_hashed_leaf_sha256(index, digest);
        } else {
            // keep as None; next root()/path() will rebuild once.
        }
    }

    /// Authentication path for leaf at `index`.
    ///
    /// Ordering: returns siblings from leaf → root. Each entry is the sibling
    /// hash at the corresponding tree level (missing siblings encoded as all‑zero).
    /// Keep this convention in sync with the mock circuits and tests.
    pub fn path(&self, index: usize) -> Vec<[u8; 32]> {
        let proof = self.proof(index);
        let path = proof
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect::<Vec<_>>();
        if crate::dev_env::debug_compact_enabled() {
            eprintln!("[path] index={} depth={}", index, path.len());
        }
        path
    }

    /// Batch-update a set of leaves by re-hashing the corresponding chunks from
    /// the provided `data` buffer. Intended for high-throughput update paths
    /// (e.g., memory commits) to avoid serial contention on the cached tree.
    ///
    /// After updating the leaf digests, the cached Merkle tree is invalidated;
    /// the next call to `root()` or `path()` will trigger a single rebuild.
    pub fn update_leaves_from_bytes_parallel(&self, data: &[u8], indices: &[usize]) {
        use rayon::prelude::*;

        // Compute all digests first (in parallel) without mutating internal state.
        let chunk = self.chunk;
        let zero_hash = self.zero_hash;
        let digests: Vec<(usize, [u8; 32])> = indices
            .par_iter()
            .map(|&idx| {
                let start = idx.saturating_mul(chunk);
                let end = (start + chunk).min(data.len());
                // Reuse the same padding + zero-fast-path semantics as `hash_chunk_padded`.
                let mut buf = [0u8; 32];
                if start < end {
                    buf[..(end - start)].copy_from_slice(&data[start..end]);
                }
                let digest = if buf[..chunk].iter().all(|&b| b == 0) {
                    zero_hash
                } else {
                    sha256_oneblock32(&buf[..chunk])
                };
                (idx, digest)
            })
            .collect();

        // Apply digests to the leaves vector in a single critical section.
        {
            let mut leaves = self.leaves.lock();
            for (idx, digest) in digests.into_iter() {
                if let Some(slot) = leaves.get_mut(idx) {
                    *slot = digest;
                }
            }
        }

        // Invalidate cached tree to trigger a single rebuild on demand.
        *self.cached.lock() = None;
    }

    /// Return both the Merkle root and the authentication path for `index`.
    /// Performs at most one rebuild of the cached canonical tree if missing.
    pub fn root_and_path(&self, index: usize) -> (HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>) {
        let (root, proof) = self.root_and_proof(index);
        let path = proof
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        (root, path)
    }

    /// Compute a Merkle root directly from raw bytes using acceleration for both leaf hashing
    /// and inner-node reduction when beneficial. Falls back to CPU for small trees or when
    /// accelerators are unavailable.
    pub fn root_from_bytes_accel(data: &[u8], chunk: usize) -> [u8; 32] {
        assert!(chunk > 0 && chunk <= 32);
        let leaves_count = data.len().div_ceil(chunk).max(1);

        // Prefer CPU SHA2 on AArch64 for medium sizes
        if prefer_cpu_sha2(leaves_count) {
            let canonical =
                MerkleTree::<[u8; 32]>::from_byte_chunks(data, chunk).expect("valid chunk");
            let metrics = iroha_telemetry::metrics::global_or_default();
            metrics.merkle_root_cpu_total.inc();
            return *canonical.root().expect("non-empty").as_ref();
        }

        // GPU Metal path (macOS)
        #[cfg(target_os = "macos")]
        if crate::vector::metal_available() && leaves_count >= merkle_metal_min_leaves() {
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
            if let Some(digests) = crate::vector::metal_sha256_leaves(&blocks)
                && let Some(root) = crate::vector::metal_sha256_pairs_reduce(&digests)
            {
                // Telemetry: GPU merkle root
                let metrics = iroha_telemetry::metrics::global_or_default();
                metrics.merkle_root_gpu_total.inc();
                return root;
            }
        }

        // GPU CUDA path
        #[cfg(feature = "cuda")]
        if {
            let min_cuda = merkle_cuda_min_leaves();
            crate::cuda_available() && leaves_count >= min_cuda
        } {
            // Prepare padded blocks similar to Metal path
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
            if let Some(root) = crate::cuda::sha256_leaves_cuda(&blocks)
                .and_then(|digests| crate::cuda::sha256_pairs_reduce_cuda(&digests))
            {
                let metrics = iroha_telemetry::metrics::global_or_default();
                metrics.merkle_root_gpu_total.inc();
                return root;
            }
        }

        // CPU fallback
        let canonical = MerkleTree::<[u8; 32]>::from_byte_chunks(data, chunk).expect("valid chunk");
        let metrics = iroha_telemetry::metrics::global_or_default();
        metrics.merkle_root_cpu_total.inc();
        *canonical.root().expect("non-empty").as_ref()
    }

    /// Compute a SHA-256 digest for inputs up to 32 bytes by constructing a
    /// single padded block and using the accelerated `sha256_compress`.
    /// This leverages Metal/CUDA/ARMv8/x86 SHA-NI where available and falls
    /// back to a scalar implementation otherwise.
    #[inline]
    #[allow(dead_code)]
    fn sha256_from_bytes_le32(input: &[u8]) -> [u8; 32] {
        debug_assert!(input.len() <= 32);
        sha256_oneblock32(input)
    }
}

#[inline]
fn sha256_oneblock32(input: &[u8]) -> [u8; 32] {
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
            let accel = ByteMerkleTree::root_from_bytes_accel(data, chunk);
            let canonical = iroha_crypto::MerkleTree::<[u8; 32]>::from_byte_chunks(data, chunk)
                .expect("valid chunk");
            let expected = *canonical.root().expect("non-empty").as_ref();
            assert_eq!(accel, expected);
        }
    }

    #[test]
    fn recompute_all_leaves_accel_small_tree_returns_false() {
        let data = b"small";
        let chunk = 32;
        let tree = ByteMerkleTree::from_bytes(data, chunk);
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
        let source = ByteMerkleTree::from_bytes(&data_a, 32);
        let other = ByteMerkleTree::from_bytes(&data_b, 32);
        other.update_leaf(1, &[5u8; 32]);

        let mut target = ByteMerkleTree::new(3, 32);
        target.reset_from(&other);
        assert_eq!(target.root(), other.root());

        target.reset_from(&source);
        assert_eq!(target.root(), source.root());
    }
}
