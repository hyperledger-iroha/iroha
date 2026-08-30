//! Region-based memory manager implementing the IVM memory model.
//!
//! The memory subsystem enforces permissions, alignment and region bounds for all loads and stores.
//! Heap allocation is supported and vector accesses are checked for 16‑byte alignment as required
//! by the specification. Memory is divided into disjoint regions:
//!
//! * **Code** – loaded at address `0x0000_0000` and marked read/execute only.
//! * **Heap** – starts at `0x0010_0000` and grows upward via `SYSCALL_ALLOC`.
//! * **Input** – read-only buffer beginning at `0x0020_0000` (64 KB).
//! * **Output** – read/write buffer beginning at `0x0021_0000`.
//! * **Stack** – starts at `0x0030_0000`; ABI V1 derives a deterministic
//!   64&nbsp;KiB–4&nbsp;MiB active limit from the invocation gas budget.
use crate::{
    byte_merkle_tree::ByteMerkleTree,
    error::{Perm, VMError},
    merkle_utils::compute_memory_leaf_digest,
    stack_policy::IvmStackPolicy,
};
use iroha_crypto::{
    CompactMerkleProof, Hash, HashOf, MerkleProof, MerkleTree, MerkleTreeCommitment,
};
use likely_stable::{likely, unlikely};
use parking_lot::Mutex;
use std::{collections::HashSet, convert::TryInto, num::NonZeroU64, sync::Arc, time::Instant};
#[cfg(test)]
std::thread_local! {
    static MEMORY_CLONE_COUNT: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}
/// Reset the current test thread's full-memory clone counter.
#[cfg(test)]
pub(crate) fn reset_memory_clone_count() {
    MEMORY_CLONE_COUNT.set(0);
}
/// Return the number of full-memory clones on the current test thread.
#[cfg(test)]
pub(crate) fn memory_clone_count() -> u64 {
    MEMORY_CLONE_COUNT.get()
}
/// Memory read range recorded for conflict detection in parallel execution.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessRange {
    pub addr: u64,
    pub len: u64,
}
/// Memory write entry capturing the exact bytes written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WriteLogEntry {
    pub addr: u64,
    pub bytes: Vec<u8>,
}
/// Memory manager for the VM, with fixed regions for code, heap, and stack.
///
/// In accordance with the updated architecture the entire memory image is committed via a Merkle
/// tree. Writes mark ranges dirty and the [`root`] is recomputed lazily on `commit()` by hashing
/// only the modified chunks. This avoids re-hashing untouched memory while still enabling inclusion
/// paths to be produced after a commit. Zero‑knowledge mode can request inclusion paths for any
/// address.
pub struct Memory {
    data: Vec<u8>,
    stack_limit: u64,
    heap_alloc: u64,
    heap_limit: u64,
    heap_max_limit: u64,
    /// Whether any heap byte may differ from the zeroed program-load baseline.
    heap_contains_data: bool,
    code_length: u64,
    /// Append-only cursor for the OUTPUT region. Enforces append-only semantics.
    output_cursor: u64,
    /// Merkle root of the entire memory image. Updated when `commit()` is
    /// called to batch multiple writes together.
    root: HashOf<MerkleTree<[u8; 32]>>,
    tree: ByteMerkleTree,
    /// Flag indicating that memory contents have changed since the last commit.
    dirty: bool,
    /// Set of Merkle leaf indices modified since the last commit.
    dirty_chunks: HashSet<usize>,
    /// Leaf indices modified since the last runtime-template reset.
    ///
    /// Unlike `dirty_chunks`, this set is not drained by Merkle commits. It
    /// lets warm VM reuse restore only pages the guest actually changed.
    modified_chunks: HashSet<usize>,
    /// Transaction-owned leaves tracked independently from runtime-template lifecycle changes.
    ///
    /// Program loaders deliberately clear `modified_chunks` after installing a
    /// new template. Block execution keeps this second set active so a loader
    /// invoked from a host callback cannot hide earlier transaction writes.
    block_modified_chunks: Option<HashSet<usize>>,
    /// Number of times a program loader established a new runtime baseline.
    template_generation: u64,
    /// Opaque identity of the runtime baseline that owns this memory image.
    ///
    /// Ordinary independent clones receive a fresh identity. Runtime-template
    /// snapshots and exact sequential-block checkpoints explicitly preserve it.
    baseline_lineage: Arc<()>,
    /// Addresses read during execution when access tracking is enabled.
    read_log: Mutex<Vec<AccessRange>>,
    /// Log of writes performed during execution (byte-accurate).
    write_log: Mutex<Vec<WriteLogEntry>>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemoryGeometry {
    pub(crate) bytes: usize,
    pub(crate) stack_limit: u64,
    pub(crate) heap_max_limit: u64,
    pub(crate) merkle_chunk_bytes: usize,
    pub(crate) merkle_leaves: usize,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct MemoryTemplateMismatch {
    pub(crate) current: MemoryGeometry,
    pub(crate) template: MemoryGeometry,
}
impl Memory {
    /// Alignment enforced for the ABI V1 guest stack top.
    pub const STACK_ALIGNMENT: u64 = IvmStackPolicy::V1.stack_alignment_bytes();
    /// Define static addresses for memory regions
    pub const HEAP_START: u64 = 0x0010_0000;
    /// Maximum heap size allowed (from HEAP_START up to INPUT_START).
    pub const HEAP_MAX_SIZE: u64 = Self::INPUT_START - Self::HEAP_START;
    /// Default heap limit exposed to guest programs.
    ///
    /// Kotodama contracts currently do not auto-grow the heap, so starting at
    /// the full pre-input window avoids spurious `OutOfMemory` traps for
    /// larger but still bounded contracts such as SoraSwap DLMM.
    pub const HEAP_SIZE: u64 = Self::HEAP_MAX_SIZE;
    pub const INPUT_START: u64 = 0x0020_0000;
    pub const INPUT_SIZE: u64 = 0x0001_0000; // 64 KB input
    pub const OUTPUT_START: u64 = Self::INPUT_START + Self::INPUT_SIZE;
    pub const OUTPUT_SIZE: u64 = 0x0000_8000; // 32 KB output
    pub const STACK_START: u64 = 0x0030_0000;
    /// Maximum logical stack size for ABI V1 guest programs.
    pub const STACK_SIZE: u64 = IvmStackPolicy::V1.maximum_stack_bytes();
    /// Extra slop beyond the nominal stack end (kept zero to trap exactly at the limit).
    pub const STACK_SLOP: u64 = 0;
    /// Align a stack byte count to the VM guest-stack boundary.
    #[must_use]
    pub fn align_stack_bytes(bytes: u64) -> u64 {
        let bytes = bytes.max(Self::STACK_ALIGNMENT);
        bytes - (bytes % Self::STACK_ALIGNMENT)
    }
    /// Current stack limit (bytes) enforced for this memory instance.
    pub fn stack_limit(&self) -> u64 {
        self.stack_limit
    }
    /// Top-of-stack address (exclusive).
    pub fn stack_top(&self) -> u64 {
        Memory::STACK_START + self.stack_limit
    }
    /// Update only the modified Merkle leaves and recompute the root.
    fn recompute_dirty(&mut self) {
        let started_at = Instant::now();
        let indices: Vec<_> = self.dirty_chunks.drain().collect();
        // Heuristic: if more than half the leaves are dirty, prefer a full
        // accelerated leaf recompute when available.
        let total_leaves = self.tree.leaf_count();
        let large_update = indices.len() * 2 >= total_leaves;
        let dirty_count = indices.len() as f64;
        let mut commit_path = "incremental";
        if large_update && self.tree.recompute_all_leaves_accel(&self.data) {
            self.root = self.tree.root_hash();
            self.dirty = false;
            commit_path = "accel";
            let metrics = iroha_telemetry::metrics::global_or_default();
            metrics
                .ivm_memory_commit_ms
                .with_label_values(&[commit_path])
                .observe(started_at.elapsed().as_secs_f64() * 1_000.0);
            metrics
                .ivm_memory_commit_dirty_chunks
                .with_label_values(&[commit_path])
                .observe(dirty_count);
            return;
        }
        if large_update {
            self.tree =
                ByteMerkleTree::from_bytes_parallel_with_leaf_count(&self.data, 32, total_leaves);
            commit_path = "full_rebuild";
            iroha_telemetry::metrics::global_or_default()
                .ivm_merkle_rebuild_total
                .inc();
        } else {
            self.tree
                .update_leaves_from_bytes_parallel(&self.data, &indices);
        }
        self.root = self.tree.root_hash();
        self.dirty = false;
        let metrics = iroha_telemetry::metrics::global_or_default();
        metrics
            .ivm_memory_commit_ms
            .with_label_values(&[commit_path])
            .observe(started_at.elapsed().as_secs_f64() * 1_000.0);
        metrics
            .ivm_memory_commit_dirty_chunks
            .with_label_values(&[commit_path])
            .observe(dirty_count);
    }
    /// Commit pending writes by hashing only the dirty chunks if the memory has
    /// been modified since the last commit.
    pub fn commit(&mut self) {
        if self.dirty {
            self.recompute_dirty();
        }
    }
    #[cfg(test)]
    pub(crate) fn dirty_for_testing(&self) -> bool {
        self.dirty
    }
    fn merkle_leaf_index(&self, addr: u64) -> Result<usize, VMError> {
        let addr = usize::try_from(addr).map_err(|_| VMError::MemoryOutOfBounds)?;
        if addr >= self.data.len() {
            return Err(VMError::MemoryOutOfBounds);
        }
        Ok(addr / 32)
    }
    /// Generate the Merkle authentication path for the 32-byte chunk containing `addr`.
    /// Pending writes are committed before sampling so the returned path matches the latest
    /// memory image.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] unless `addr` names an exact byte in this memory
    /// image. In particular, the exclusive end address is not rounded into the final tree leaf.
    pub fn merkle_path(&mut self, addr: u64) -> Result<Vec<[u8; 32]>, VMError> {
        let index = self.merkle_leaf_index(addr)?;
        self.commit();
        self.tree.path(index)
    }
    /// Return both the current Merkle root (typed `HashOf<MerkleTree<[u8; 32]>>`)
    /// and the authentication path for the 32-byte chunk containing `addr` in a single operation.
    /// Pending writes are committed first to keep the root/path in sync.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] unless `addr` names an exact byte in this memory
    /// image.
    pub fn merkle_root_and_path(
        &mut self,
        addr: u64,
    ) -> Result<(HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>), VMError> {
        let index = self.merkle_leaf_index(addr)?;
        self.commit();
        self.tree.root_and_path(index)
    }
    /// Build a compact Merkle proof for the memory chunk containing `addr`.
    ///
    /// Pending writes are committed before construction. Without truncation the returned root is
    /// the full memory-tree root. When `depth_cap` truncates the path, the returned root commits
    /// only to that path fragment and is not a membership commitment.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] unless `addr` names an exact byte in this memory
    /// image.
    pub fn merkle_compact(
        &mut self,
        addr: u64,
        depth_cap: Option<usize>,
    ) -> Result<(CompactMerkleProof<[u8; 32]>, HashOf<MerkleTree<[u8; 32]>>), VMError> {
        let leaf_index = self.merkle_leaf_index(addr)?;
        let leaf_index = u32::try_from(leaf_index).map_err(|_| VMError::MemoryOutOfBounds)?;
        let (full_root, path) = self.merkle_root_and_path(addr)?;
        let mut depth = path.len().min(32);
        if let Some(cap) = depth_cap {
            depth = depth.min(cap);
        }
        let dirs = if depth == 32 {
            leaf_index
        } else {
            leaf_index & ((1u32 << depth) - 1)
        };
        let typed_siblings: Vec<Option<HashOf<[u8; 32]>>> = path
            .iter()
            .take(depth)
            .map(|b| {
                if *b == [0u8; 32] {
                    None
                } else {
                    Some(HashOf::from_untyped_unchecked(Hash::prehashed(*b)))
                }
            })
            .collect();
        let proof_for_root = MerkleProof::from_audit_path(dirs, typed_siblings.clone());
        let compact = CompactMerkleProof::from_parts(depth as u8, dirs, typed_siblings);
        let root: HashOf<MerkleTree<[u8; 32]>> = if depth < path.len() {
            let base = (addr / 32) * 32;
            let start = base as usize;
            let end = (start + 32).min(self.data.len());
            let mut chunk = [0u8; 32];
            chunk[..(end - start)].copy_from_slice(&self.data[start..end]);
            let leaf_digest = compute_memory_leaf_digest(&chunk);
            let leaf_hash =
                HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_digest));
            proof_for_root
                .compute_partial_root_sha256(&leaf_hash, depth)
                .expect("proof height equals compact depth")
        } else {
            full_root
        };
        Ok((compact, root))
    }
    /// Return the current full-tree root and exact local memory geometry as one
    /// membership commitment.
    ///
    /// This commitment is authoritative only for this in-process [`Memory`]
    /// instance. Protocols transporting a root must authenticate the count
    /// alongside it rather than reconstructing a count from the proof depth.
    pub fn merkle_commitment(&mut self) -> MerkleTreeCommitment<[u8; 32]> {
        self.commit();
        let leaf_count = u64::try_from(self.tree.leaf_count())
            .ok()
            .and_then(NonZeroU64::new)
            .expect("memory tree always has a non-zero leaf count representable as u64");
        MerkleTreeCommitment::new(self.root, leaf_count)
    }
    /// Current typed Merkle root, recomputing pending dirty ranges if needed.
    ///
    /// This helper mirrors [`root`](Self::root) but keeps the method name used by callers that
    /// sample the root during execution (e.g., step logs). It forces a `commit()` so that in-flight
    /// writes are reflected in the returned digest.
    pub fn current_root(&mut self) -> HashOf<MerkleTree<[u8; 32]>> {
        self.commit();
        self.root
    }
    /// Return the current Merkle root of memory, recomputing it if any writes
    /// have occurred since the last call.
    pub fn root(&mut self) -> HashOf<MerkleTree<[u8; 32]>> {
        self.commit();
        self.root
    }
    fn update_merkle(&mut self, start: usize, len: usize) {
        const CHUNK: usize = 32;
        let end = start.saturating_add(len);
        let heap_start = Memory::HEAP_START as usize;
        let heap_end = heap_start + Memory::HEAP_MAX_SIZE as usize;
        if start < heap_end && end > heap_start {
            self.heap_contains_data = true;
        }
        let first = start / CHUNK;
        let last = end.div_ceil(CHUNK);
        for i in first..last {
            self.dirty_chunks.insert(i);
            self.modified_chunks.insert(i);
            if let Some(block_modified) = &mut self.block_modified_chunks {
                block_modified.insert(i);
            }
        }
        // Mark the tree as dirty so the root is recomputed lazily on the next
        // `commit()` or `root()` call.
        self.dirty = true;
    }
    /// Initialize an empty memory image with the canonical ABI V1 maximum stack.
    ///
    /// Code becomes executable only after a successful [`Self::load_code`].
    #[must_use]
    pub fn new() -> Self {
        Self::new_with_stack_limit(IvmStackPolicy::V1.maximum_stack_bytes())
            .expect("the canonical ABI V1 memory geometry is valid")
    }
    /// Initialize memory with an explicit stack limit (bytes).
    ///
    /// This low-level constructor is used for runtime templates and focused
    /// memory tests. Production VMs derive its argument exclusively from the
    /// immutable ABI stack policy in `IvmConfig`.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] when `stack_limit` exceeds the
    /// ABI V1 maximum or the resulting memory geometry is not representable on
    /// the host.
    pub fn new_with_stack_limit(stack_limit: u64) -> Result<Self, VMError> {
        if stack_limit > IvmStackPolicy::V1.maximum_stack_bytes() {
            return Err(VMError::MemoryOutOfBounds);
        }
        let stack_limit = Self::align_stack_bytes(stack_limit);
        let total_size = Memory::STACK_START
            .checked_add(stack_limit)
            .and_then(|size| size.checked_add(Memory::STACK_SLOP))
            .ok_or(VMError::MemoryOutOfBounds)?;
        let total_size = usize::try_from(total_size).map_err(|_| VMError::MemoryOutOfBounds)?;
        let mut mem = Memory {
            data: vec![0u8; total_size],
            stack_limit,
            heap_alloc: 0,
            heap_limit: Memory::HEAP_SIZE,
            heap_max_limit: Memory::HEAP_MAX_SIZE,
            heap_contains_data: false,
            code_length: 0,
            output_cursor: 0,
            root: HashOf::from_untyped_unchecked(Hash::prehashed([0u8; 32])),
            tree: ByteMerkleTree::new(total_size.div_ceil(32).max(1), 32)?,
            dirty: false,
            dirty_chunks: HashSet::new(),
            modified_chunks: HashSet::new(),
            block_modified_chunks: None,
            template_generation: 0,
            baseline_lineage: Arc::new(()),
            read_log: Mutex::new(Vec::new()),
            write_log: Mutex::new(Vec::new()),
        };
        // initialize root from zeroed memory
        mem.root = mem.tree.root_hash();
        Ok(mem)
    }
    /// Preload data into the input region. Used by tests/host before execution.
    pub fn preload_input(&mut self, offset: u64, bytes: &[u8]) -> Result<(), VMError> {
        if offset > Memory::INPUT_SIZE {
            return Err(VMError::MemoryOutOfBounds);
        }
        let len = bytes.len() as u64;
        let end_off = offset.checked_add(len).ok_or(VMError::MemoryOutOfBounds)?;
        if end_off > Memory::INPUT_SIZE {
            return Err(VMError::MemoryOutOfBounds);
        }
        let start = (Memory::INPUT_START + offset) as usize;
        let end = start + bytes.len();
        self.data[start..end].copy_from_slice(bytes);
        if crate::dev_env::decode_trace_enabled() {
            let h = &self.data[start..(start + bytes.len().min(7))];
            eprintln!("preload_input off=0x{offset:x} wrote header bytes: {h:02x?}");
        }
        self.update_merkle(start, bytes.len());
        Ok(())
    }
    /// Tiny INPUT allocator helper: write `bytes` at the next aligned offset pointed to by `cursor`.
    ///
    /// - `cursor` is an offset relative to `INPUT_START` that the caller maintains.
    /// - `align` must be a power of two; defaults to 8 in most callers.
    /// - Returns the absolute pointer to the beginning of the written bytes.
    pub fn input_write_aligned(
        &mut self,
        cursor: &mut u64,
        bytes: &[u8],
        align: u64,
    ) -> Result<u64, VMError> {
        if !align.is_power_of_two() {
            return Err(VMError::MemoryOutOfBounds);
        }
        let mask = align - 1;
        let off = cursor
            .checked_add(mask)
            .map(|value| value & !mask)
            .ok_or(VMError::MemoryOutOfBounds)?;
        let len = u64::try_from(bytes.len()).map_err(|_| VMError::MemoryOutOfBounds)?;
        let end = off.checked_add(len).ok_or(VMError::MemoryOutOfBounds)?;
        if end > Memory::INPUT_SIZE {
            return Err(VMError::MemoryOutOfBounds);
        }
        self.preload_input(off, bytes)?;
        *cursor = end;
        Memory::INPUT_START
            .checked_add(off)
            .ok_or(VMError::MemoryOutOfBounds)
    }
    #[inline]
    pub fn alloc(&mut self, size: u64) -> Result<u64, VMError> {
        let aligned = size
            .checked_add(7)
            .map(|v| v & !7)
            .ok_or(VMError::OutOfMemory)?;
        if aligned != 0 {
            let new_alloc = self
                .heap_alloc
                .checked_add(aligned)
                .ok_or(VMError::OutOfMemory)?;
            if unlikely(new_alloc > self.heap_limit) {
                return Err(VMError::OutOfMemory);
            }
            let addr = Memory::HEAP_START + self.heap_alloc;
            self.heap_alloc = new_alloc;
            Ok(addr)
        } else {
            Ok(Memory::HEAP_START + self.heap_alloc)
        }
    }
    /// Grow the heap by `additional` bytes, returning the new limit.
    pub fn grow_heap(&mut self, additional: u64) -> Result<u64, VMError> {
        let aligned = additional
            .checked_add(7)
            .map(|v| v & !7)
            .ok_or(VMError::OutOfMemory)?;
        if aligned == 0 {
            return Ok(self.heap_limit);
        }
        let new_limit = self
            .heap_limit
            .checked_add(aligned)
            .ok_or(VMError::OutOfMemory)?;
        if unlikely(new_limit > self.heap_max_limit) {
            return Err(VMError::OutOfMemory);
        }
        self.heap_limit = new_limit;
        Ok(self.heap_limit)
    }
    /// Current heap limit in bytes.
    pub fn heap_limit(&self) -> u64 {
        self.heap_limit
    }
    /// Per-instance ceiling for heap growth.
    pub fn heap_max_limit(&self) -> u64 {
        self.heap_max_limit
    }
    /// Number of heap bytes currently owned by successful allocations.
    pub(crate) fn heap_allocated_len(&self) -> u64 {
        self.heap_alloc
    }
    /// Override the active heap limit, keeping the already-allocated region valid.
    pub fn set_heap_limit(&mut self, limit: u64) -> Result<(), VMError> {
        if limit < self.heap_alloc || limit > self.heap_max_limit {
            return Err(VMError::OutOfMemory);
        }
        self.heap_limit = limit;
        Ok(())
    }
    /// Set the absolute per-instance heap ceiling and clamp the active limit to it.
    ///
    /// Unlike [`Self::set_heap_limit`], this limit cannot be bypassed by [`Self::grow_heap`]. Hosts
    /// use it to apply deterministic governance limits before guest execution.
    pub fn set_heap_max_limit(&mut self, limit: u64) -> Result<(), VMError> {
        if limit < self.heap_alloc || limit > Memory::HEAP_MAX_SIZE {
            return Err(VMError::OutOfMemory);
        }
        self.heap_max_limit = limit;
        self.heap_limit = self.heap_limit.min(limit);
        Ok(())
    }
    /// Clear all physical heap bytes before installing a different program.
    pub(crate) fn clear_program_heap(&mut self) {
        if !self.heap_contains_data && self.heap_alloc == 0 {
            return;
        }
        let start = Memory::HEAP_START as usize;
        let end = start + Memory::HEAP_MAX_SIZE as usize;
        self.data[start..end].fill(0);
        self.heap_alloc = 0;
        self.update_merkle(start, end - start);
        self.heap_contains_data = false;
    }
    /// Update the code region length after loading a program.
    fn set_code_length(&mut self, code_size: u64) {
        self.code_length = code_size;
    }
    /// Return the current code length in bytes.
    pub fn code_len(&self) -> u64 {
        self.code_length
    }
    /// Copy out the code bytes currently loaded in the code region.
    pub fn read_code_bytes(&self) -> Vec<u8> {
        let len = self.code_length as usize;
        self.data[0..len].to_vec()
    }
    /// Load program bytes into the beginning of memory (code region).
    ///
    /// The complete request is validated before any bytes or metadata change.
    ///
    /// # Errors
    /// Returns [`VMError::MemoryOutOfBounds`] when the program overlaps the
    /// heap boundary or is not representable in the physical memory image.
    pub fn load_code(&mut self, code: &[u8]) -> Result<(), VMError> {
        let len = code.len();
        let code_region_end = usize::try_from(Memory::HEAP_START)
            .map_err(|_| VMError::MemoryOutOfBounds)?
            .min(self.data.len());
        if len > code_region_end {
            return Err(VMError::MemoryOutOfBounds);
        }
        let old_len = usize::try_from(self.code_length).map_err(|_| VMError::MemoryOutOfBounds)?;
        if old_len > code_region_end {
            return Err(VMError::MemoryOutOfBounds);
        }
        self.data[0..len].copy_from_slice(code);
        if len < old_len {
            self.data[len..old_len].fill(0);
        }
        self.set_code_length(len as u64);
        let modified_len = len.max(old_len);
        if modified_len != 0 {
            self.update_merkle(0, modified_len);
        }
        if crate::dev_env::debug_wsv_enabled() {
            let dump = |start: usize, count: usize| {
                if start >= len {
                    let end = start + count;
                    eprintln!(
                        "[mem.load_code] bytes[0x{start:x}..0x{end:x}] skipped (len=0x{len:x})"
                    );
                    return;
                }
                let end = (start + count).min(len);
                let mut s = String::new();
                for b in &self.data[start..end] {
                    use core::fmt::Write as _;
                    let _ = write!(&mut s, "{b:02x}");
                }
                eprintln!("[mem.load_code] bytes[0x{start:x}..0x{end:x}] = {s}");
            };
            let ranges = [(0usize, 64usize), (0x1c, 16), (0x20, 64), (0x28, 64)];
            for (st, cnt) in ranges {
                dump(st, cnt);
            }
        }
        Ok(())
    }
    /// Determine the permissions for the address range `[addr, addr + size)`.
    #[inline]
    fn region_perm(&self, addr: u64, size: u32) -> Option<Perm> {
        let end = addr.checked_add(size as u64)?;
        if end <= self.code_length {
            return Some(Perm::READ | Perm::EXECUTE);
        }
        if addr >= Memory::HEAP_START && end <= Memory::HEAP_START + self.heap_limit {
            return Some(Perm::READ | Perm::WRITE);
        }
        if addr >= Memory::INPUT_START && end <= Memory::INPUT_START + Memory::INPUT_SIZE {
            return Some(Perm::READ);
        }
        if addr >= Memory::OUTPUT_START && end <= Memory::OUTPUT_START + Memory::OUTPUT_SIZE {
            return Some(Perm::READ | Perm::WRITE);
        }
        let stack_end = Memory::STACK_START + self.stack_limit;
        if addr >= Memory::STACK_START && end <= stack_end {
            return Some(Perm::READ | Perm::WRITE);
        }
        if addr > stack_end && end <= stack_end + Memory::STACK_SLOP {
            return Some(Perm::READ | Perm::WRITE);
        }
        None
    }
    /// Check that an address range has the required permissions.
    #[inline]
    fn check_perm(&self, addr: u64, size: u32, required: Perm) -> Result<(), VMError> {
        if let Some(perm) = self.region_perm(addr, size) {
            if likely(perm.contains(required)) {
                Ok(())
            } else {
                Err(VMError::MemoryAccessViolation {
                    addr: addr as u32,
                    perm: required,
                })
            }
        } else {
            Err(VMError::MemoryAccessViolation {
                addr: addr as u32,
                perm: required,
            })
        }
    }
    /// Load an 8-bit value from memory.
    #[inline]
    pub fn load_u8(&self, addr: u64) -> Result<u8, VMError> {
        self.check_perm(addr, 1, Perm::READ)?;
        self.record_read_range(addr, 1);
        Ok(self.data[addr as usize])
    }
    /// Load a 32-bit value (little-endian) from memory.
    #[inline]
    pub fn load_u32(&self, addr: u64) -> Result<u32, VMError> {
        if unlikely(!addr.is_multiple_of(4)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 4, Perm::READ)?;
        let bytes: [u8; 4] = self.data[addr as usize..addr as usize + 4]
            .try_into()
            .unwrap();
        self.record_read_range(addr, 4);
        Ok(u32::from_le_bytes(bytes))
    }
    /// Load a 64-bit value from memory.
    #[inline]
    pub fn load_u64(&self, addr: u64) -> Result<u64, VMError> {
        if unlikely(!addr.is_multiple_of(8)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 8, Perm::READ)?;
        self.record_read_range(addr, 8);
        let bytes: [u8; 8] = self.data[addr as usize..addr as usize + 8]
            .try_into()
            .unwrap();
        Ok(u64::from_le_bytes(bytes))
    }
    /// Load a 128-bit value from memory (little endian).
    #[inline]
    pub fn load_u128(&self, addr: u64) -> Result<u128, VMError> {
        if unlikely(!addr.is_multiple_of(16)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 16, Perm::READ)?;
        let bytes: [u8; 16] = self.data[addr as usize..addr as usize + 16]
            .try_into()
            .unwrap();
        self.record_read_range(addr, 16);
        Ok(u128::from_le_bytes(bytes))
    }
    /// Copy `out.len()` bytes starting at `addr` into `out`.
    #[inline]
    pub fn load_bytes(&self, addr: u64, out: &mut [u8]) -> Result<(), VMError> {
        let len = u64::try_from(out.len()).map_err(|_| VMError::MemoryAccessViolation {
            addr: addr as u32,
            perm: Perm::READ,
        })?;
        let (start, end) = self.checked_region_bounds_for(addr, len, Perm::READ)?;
        out.copy_from_slice(&self.data[start..end]);
        self.record_read_range(addr, len);
        Ok(())
    }
    fn checked_region_bounds_for(
        &self,
        addr: u64,
        len: u64,
        required: Perm,
    ) -> Result<(usize, usize), VMError> {
        let violation = || VMError::MemoryAccessViolation {
            addr: addr as u32,
            perm: required,
        };
        let len_u32 = u32::try_from(len).map_err(|_| violation())?;
        self.check_perm(addr, len_u32, required)?;
        let start = usize::try_from(addr).map_err(|_| violation())?;
        let len_usize = usize::try_from(len).map_err(|_| VMError::MemoryAccessViolation {
            addr: addr as u32,
            perm: required,
        })?;
        let Some(end) = start.checked_add(len_usize) else {
            return Err(violation());
        };
        if end > self.data.len() {
            return Err(violation());
        }
        Ok((start, end))
    }
    fn checked_region_bounds(&self, addr: u64, len: u64) -> Result<(usize, usize), VMError> {
        self.checked_region_bounds_for(addr, len, Perm::READ)
    }
    /// Inspect `len` bytes without recording a guest-visible memory access.
    ///
    /// This is reserved for side-effect-free host quote preparation. Actual syscall execution must
    /// use [`Self::load_region`] so access tracing remains complete.
    #[inline]
    pub(crate) fn inspect_region(&self, addr: u64, len: u64) -> Result<&[u8], VMError> {
        let (start, end) = self.checked_region_bounds(addr, len)?;
        Ok(&self.data[start..end])
    }
    /// Load `len` bytes starting at `addr` and return a slice referencing the underlying memory.
    #[inline]
    pub fn load_region(&self, addr: u64, len: u64) -> Result<&[u8], VMError> {
        let (start, end) = self.checked_region_bounds(addr, len)?;
        self.record_read_range(addr, len);
        if crate::dev_env::debug_wsv_enabled() && len <= 64 {
            let win_start = start.saturating_sub(16);
            let win_end = (end + 16).min(self.data.len());
            let mut s = String::new();
            for b in &self.data[win_start..win_end] {
                use core::fmt::Write as _;
                let _ = write!(&mut s, "{b:02x}");
            }
            eprintln!(
                "[mem.load_region] addr=0x{addr:x} len={len} window[0x{win_start:x}..0x{win_end:x}] = {s}"
            );
        }
        Ok(&self.data[start..end])
    }
    /// Copy bytes from `bytes` into memory starting at `addr`.
    #[inline]
    pub fn store_bytes(&mut self, addr: u64, bytes: &[u8]) -> Result<(), VMError> {
        let len = u64::try_from(bytes.len()).map_err(|_| VMError::MemoryAccessViolation {
            addr: addr as u32,
            perm: Perm::WRITE,
        })?;
        let (start, end) = self.checked_region_bounds_for(addr, len, Perm::WRITE)?;
        // Enforce append-only semantics for OUTPUT region
        self.check_output_append_only(addr, len)?;
        self.data[start..end].copy_from_slice(bytes);
        self.update_merkle(start, bytes.len());
        self.record_write(addr, bytes);
        Ok(())
    }
    /// Store an 8-bit value into memory.
    #[inline]
    pub fn store_u8(&mut self, addr: u64, value: u8) -> Result<(), VMError> {
        self.check_perm(addr, 1, Perm::WRITE)?;
        self.check_output_append_only(addr, 1)?;
        self.data[addr as usize] = value;
        self.update_merkle(addr as usize, 1);
        self.record_write(addr, &[value]);
        Ok(())
    }
    /// Store a 32-bit value (little-endian) into memory.
    #[inline]
    pub fn store_u32(&mut self, addr: u64, value: u32) -> Result<(), VMError> {
        if unlikely(!addr.is_multiple_of(4)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 4, Perm::WRITE)?;
        self.check_output_append_only(addr, 4)?;
        self.data[addr as usize..addr as usize + 4].copy_from_slice(&value.to_le_bytes());
        self.update_merkle(addr as usize, 4);
        self.record_write(addr, &value.to_le_bytes());
        Ok(())
    }
    /// Store a 64-bit value into memory.
    #[inline]
    pub fn store_u64(&mut self, addr: u64, value: u64) -> Result<(), VMError> {
        if unlikely(!addr.is_multiple_of(8)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 8, Perm::WRITE)?;
        self.check_output_append_only(addr, 8)?;
        self.data[addr as usize..addr as usize + 8].copy_from_slice(&value.to_le_bytes());
        self.record_write(addr, &value.to_le_bytes());
        self.update_merkle(addr as usize, 8);
        Ok(())
    }
    /// Store a 128-bit value into memory.
    #[inline]
    pub fn store_u128(&mut self, addr: u64, value: u128) -> Result<(), VMError> {
        if unlikely(!addr.is_multiple_of(16)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 16, Perm::WRITE)?;
        self.check_output_append_only(addr, 16)?;
        self.data[addr as usize..addr as usize + 16].copy_from_slice(&value.to_le_bytes());
        self.update_merkle(addr as usize, 16);
        self.record_write(addr, &value.to_le_bytes());
        Ok(())
    }
    /// Obtain a slice of the entire output region without allocating.
    #[inline]
    pub fn read_output(&self) -> &[u8] {
        let start = Memory::OUTPUT_START as usize;
        let end = start + Memory::OUTPUT_SIZE as usize;
        &self.data[start..end]
    }
    /// Number of bytes in the append-only prefix written by the guest.
    #[inline]
    pub fn output_used_len(&self) -> u64 {
        self.output_cursor
    }
    /// Borrow only the append-only output prefix written by the guest.
    #[inline]
    pub fn read_output_used(&self) -> &[u8] {
        let start = Memory::OUTPUT_START as usize;
        let used = usize::try_from(self.output_cursor).unwrap_or(Memory::OUTPUT_SIZE as usize);
        &self.data[start..start.saturating_add(used)]
    }
    /// Clear the OUTPUT region and reset the append-only cursor.
    pub(crate) fn clear_output(&mut self) {
        let start = Memory::OUTPUT_START as usize;
        let end = start + Memory::OUTPUT_SIZE as usize;
        if self.output_cursor == 0 && self.data[start..end].iter().all(|b| *b == 0) {
            return;
        }
        self.data[start..end].fill(0);
        self.output_cursor = 0;
        self.update_merkle(start, end - start);
    }
    /// Clear recorded access information.
    pub fn clear_tracking(&self) {
        self.read_log.lock().clear();
        let mut write_log = self.write_log.lock();
        for entry in write_log.iter_mut() {
            entry.bytes.fill(0);
        }
        write_log.clear();
    }
    /// Snapshot the set of ranges read since the last clear.
    pub fn read_set(&self) -> Vec<AccessRange> {
        self.read_log.lock().clone()
    }
    /// Snapshot of writes made since the last clear.
    pub fn write_log(&self) -> Vec<WriteLogEntry> {
        self.write_log.lock().clone()
    }
    /// Ranges of memory that have been modified since the last commit.
    pub fn dirty_ranges(&self) -> Vec<(usize, usize)> {
        const CHUNK: usize = 32;
        let mut indices: Vec<_> = self.dirty_chunks.iter().copied().collect();
        indices.sort_unstable();
        let mut ranges = Vec::new();
        let mut start = None;
        let mut prev = 0;
        for idx in indices {
            if let Some(s) = start {
                if idx == prev + 1 {
                    prev = idx;
                    continue;
                } else {
                    ranges.push((s * CHUNK, (prev + 1) * CHUNK));
                }
            }
            start = Some(idx);
            prev = idx;
        }
        if let Some(s) = start {
            ranges.push((s * CHUNK, (prev + 1) * CHUNK));
        }
        ranges
    }
    /// Mark the current bytes as an immutable runtime-template baseline.
    pub(crate) fn mark_template_clean(&mut self) {
        self.modified_chunks.clear();
        self.template_generation = self
            .template_generation
            .checked_add(1)
            .expect("IVM memory template generation exhausted");
        self.baseline_lineage = Arc::new(());
        self.clear_tracking();
    }
    /// Start independent dirty tracking for a block worker's transaction sequence.
    pub(crate) fn begin_block_transaction_tracking(&mut self) {
        self.block_modified_chunks = Some(HashSet::new());
    }
    /// Return the current runtime-template lifecycle generation.
    pub(crate) const fn template_generation(&self) -> u64 {
        self.template_generation
    }
    /// Whether this memory image descends from the same captured runtime baseline.
    pub(crate) fn shares_baseline_lineage(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.baseline_lineage, &other.baseline_lineage)
    }

    /// Clone a runtime-template baseline without allowing an independent image
    /// to counterfeit that baseline later.
    pub(crate) fn clone_for_runtime_template(&self) -> Self {
        let mut template = self.clone();
        template.baseline_lineage = Arc::clone(&self.baseline_lineage);
        template
    }
    pub(crate) fn reset_from_template(
        &mut self,
        template: &Memory,
    ) -> Result<(), MemoryTemplateMismatch> {
        self.reset_from_template_inner(template, false)
    }
    /// Validate that two images can use the same in-place reset geometry.
    pub(crate) fn ensure_template_geometry(
        &self,
        template: &Memory,
    ) -> Result<(), MemoryTemplateMismatch> {
        let current = self.geometry();
        let template = template.geometry();
        if current == template {
            Ok(())
        } else {
            Err(MemoryTemplateMismatch { current, template })
        }
    }
    /// Restore transaction-owned memory, including code installed since the block template.
    pub(crate) fn reset_for_block_transaction(
        &mut self,
        template: &Memory,
    ) -> Result<(), MemoryTemplateMismatch> {
        self.reset_from_template_inner(template, true)
    }
    fn reset_from_template_inner(
        &mut self,
        template: &Memory,
        restore_code: bool,
    ) -> Result<(), MemoryTemplateMismatch> {
        self.ensure_template_geometry(template)?;
        const CHUNK: usize = 32;
        let mut modified = self.modified_chunks.iter().copied().collect::<Vec<_>>();
        if restore_code {
            if let Some(block_modified) = &self.block_modified_chunks {
                modified.extend(block_modified.iter().copied());
            } else {
                // A host can replace the public Memory value during a
                // callback. Losing the active tracker must degrade to an exact
                // reset rather than allowing untracked bytes to survive.
                modified.extend(0..self.data.len().div_ceil(CHUNK));
            }
            let code_bytes = self.code_length.max(template.code_length);
            let code_bytes = usize::try_from(code_bytes)
                .expect("IVM code length always fits the host address space");
            modified.extend(0..code_bytes.div_ceil(CHUNK));
        }
        modified.sort_unstable();
        modified.dedup();
        for index in &modified {
            let start = index.saturating_mul(CHUNK);
            if start >= self.data.len() {
                continue;
            }
            let end = (start + CHUNK).min(self.data.len());
            self.data[start..end].copy_from_slice(&template.data[start..end]);
        }
        self.tree.reset_leaves_from(&template.tree, &modified);
        self.heap_alloc = template.heap_alloc;
        self.heap_limit = template.heap_limit;
        self.heap_max_limit = template.heap_max_limit;
        self.heap_contains_data = template.heap_contains_data;
        self.code_length = template.code_length;
        self.output_cursor = template.output_cursor;
        self.root = template.root;
        self.dirty = template.dirty;
        self.dirty_chunks = template.dirty_chunks.clone();
        self.modified_chunks.clear();
        if restore_code && let Some(block_modified) = &mut self.block_modified_chunks {
            block_modified.clear();
        }
        self.clear_tracking();
        Ok(())
    }
    fn geometry(&self) -> MemoryGeometry {
        MemoryGeometry {
            bytes: self.data.len(),
            stack_limit: self.stack_limit,
            heap_max_limit: self.heap_max_limit,
            merkle_chunk_bytes: self.tree.chunk_size(),
            merkle_leaves: self.tree.leaf_count(),
        }
    }
    fn record_read_range(&self, addr: u64, len: u64) {
        self.read_log.lock().push(AccessRange { addr, len });
    }
    fn record_write(&self, addr: u64, bytes: &[u8]) {
        self.write_log.lock().push(WriteLogEntry {
            addr,
            bytes: bytes.to_vec(),
        });
    }
    /// Overwrite just the code region with bytes from another Memory.
    pub fn overlay_code(&mut self, src: &Memory) -> Result<(), VMError> {
        let len = src.code_length as usize;
        self.load_code(&src.data[0..len])
    }
}

impl Default for Memory {
    fn default() -> Self {
        Self::new()
    }
}
impl Clone for Memory {
    fn clone(&self) -> Self {
        #[cfg(test)]
        MEMORY_CLONE_COUNT.set(MEMORY_CLONE_COUNT.get().saturating_add(1));
        Self {
            data: self.data.clone(),
            stack_limit: self.stack_limit,
            heap_alloc: self.heap_alloc,
            heap_limit: self.heap_limit,
            heap_max_limit: self.heap_max_limit,
            heap_contains_data: self.heap_contains_data,
            code_length: self.code_length,
            output_cursor: self.output_cursor,
            root: self.root,
            tree: self.tree.clone(),
            dirty: self.dirty,
            dirty_chunks: self.dirty_chunks.clone(),
            modified_chunks: HashSet::new(),
            block_modified_chunks: None,
            template_generation: self.template_generation,
            baseline_lineage: Arc::new(()),
            read_log: Mutex::new(Vec::new()),
            write_log: Mutex::new(Vec::new()),
        }
    }
}
impl Memory {
    /// Restore tracking state omitted by the ordinary independent-memory clone.
    ///
    /// A sequential block checkpoint uses a regular [`Clone`] for the large
    /// byte and Merkle-tree image, then calls this helper so a later runtime
    /// template reset still knows which pre-block bytes must be restored.
    pub(crate) fn preserve_checkpoint_tracking_from(&mut self, source: &Self) {
        self.modified_chunks.clone_from(&source.modified_chunks);
        self.block_modified_chunks
            .clone_from(&source.block_modified_chunks);
        self.baseline_lineage = Arc::clone(&source.baseline_lineage);
        self.read_log = Mutex::new(source.read_log.lock().clone());
        self.write_log = Mutex::new(source.write_log.lock().clone());
    }

    #[inline]
    fn check_output_append_only(&mut self, addr: u64, len: u64) -> Result<(), VMError> {
        // Only enforce within OUTPUT region; allow arbitrary writes elsewhere.
        let start = Memory::OUTPUT_START;
        let end = Memory::OUTPUT_START + Memory::OUTPUT_SIZE;
        let write_end = addr.saturating_add(len);
        if addr >= start && write_end <= end {
            // Convert absolute addr to offset within OUTPUT
            let off = addr - start;
            // Relaxed monotonic append: allow forward writes at or beyond the
            // current cursor; disallow rewinding into already-written region.
            if off < self.output_cursor {
                return Err(VMError::MemoryAccessViolation {
                    addr: addr as u32,
                    perm: Perm::WRITE,
                });
            }
            let new_end = off.saturating_add(len);
            if new_end > self.output_cursor {
                self.output_cursor = new_end;
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::merkle_utils::compute_memory_leaf_digest;
    use iroha_crypto::{Hash, HashOf, MerkleProof};
    #[test]
    fn reset_from_template_restores_runtime_regions() {
        let mut base = Memory::new();
        base.preload_input(0, &[1, 2, 3, 4])
            .expect("preload template input");
        base.set_heap_limit(Memory::HEAP_MAX_SIZE - 128)
            .expect("lower template heap limit");
        let mut worker = base.clone();
        worker.alloc(32).expect("alloc");
        worker
            .store_u64(Memory::OUTPUT_START, 0xDEAD_BEEF_DEAD_BEEFu64)
            .expect("store output");
        worker.grow_heap(64).expect("grow heap");
        assert!(!worker.modified_chunks.is_empty());
        let _ = worker.root();
        assert!(worker.dirty_chunks.is_empty());
        assert!(
            !worker.modified_chunks.is_empty(),
            "Merkle commits must retain reset tracking"
        );
        assert_ne!(&worker.read_output()[..8], &base.read_output()[..8],);
        worker
            .reset_from_template(&base)
            .expect("worker and template geometries match");
        assert_eq!(worker.heap_alloc, base.heap_alloc);
        assert_eq!(worker.heap_limit(), base.heap_limit());
        assert_eq!(worker.code_len(), base.code_len());
        assert_eq!(worker.read_output(), base.read_output());
        assert!(worker.modified_chunks.is_empty());
        let mut worker_clone = worker.clone();
        let mut base_clone = base.clone();
        assert_eq!(worker_clone.root(), base_clone.root());
    }
    #[test]
    fn warm_reset_does_not_copy_unmodified_memory_chunks() {
        let base = Memory::new();
        let mut worker = base.clone();
        let tracked_address = Memory::HEAP_START;
        worker
            .store_u8(tracked_address, 0xA5)
            .expect("write tracked heap byte");
        // Deliberately perturb a different chunk without going through a Memory
        // write API. This is a test-only probe: a whole-image reset would erase
        // it, while a dirty-chunk reset must leave it untouched.
        const MERKLE_LEAF_BYTES: usize = 32;
        let untracked_address = usize::try_from(Memory::HEAP_START).expect("heap start fits usize")
            + 2 * MERKLE_LEAF_BYTES;
        worker.data[untracked_address] = 0x5A;
        worker
            .reset_from_template(&base)
            .expect("worker and template geometries match");
        assert_eq!(
            worker.data[usize::try_from(tracked_address).expect("tracked address fits usize")],
            0,
            "tracked chunks must be restored from the template"
        );
        assert_eq!(
            worker.data[untracked_address], 0x5A,
            "warm reset must not copy the complete memory image"
        );
    }
    #[test]
    fn block_reset_restores_code_hidden_by_template_cleaning() {
        let mut template = Memory::new();
        let expected_root = template.current_root();
        let mut worker = template.clone();
        worker.begin_block_transaction_tracking();
        worker.load_code(&[0xA5; 65]).unwrap();
        worker.commit();
        worker.mark_template_clean();
        assert!(worker.modified_chunks.is_empty());
        assert_ne!(worker.current_root(), expected_root);

        worker
            .reset_for_block_transaction(&template)
            .expect("worker and block template geometries match");

        assert_eq!(&worker.data[..96], &[0; 96]);
        assert_eq!(worker.code_len(), 0);
        assert_eq!(worker.current_root(), expected_root);
    }
    #[test]
    fn shorter_code_load_clears_prior_tail_and_matches_fresh_root() {
        let short = [0x11, 0x22, 0x33, 0x44];
        let mut historical = Memory::new();
        historical.load_code(&[0xA5; 65]).unwrap();
        historical.commit();
        historical.load_code(&short).unwrap();

        let mut fresh = Memory::new();
        fresh.load_code(&short).unwrap();

        assert_eq!(&historical.data[..short.len()], &short);
        assert!(
            historical.data[short.len()..65]
                .iter()
                .all(|byte| *byte == 0)
        );
        assert_eq!(historical.current_root(), fresh.current_root());

        let mut overlaid = Memory::new();
        overlaid.load_code(&[0x5A; 65]).unwrap();
        overlaid.overlay_code(&fresh).unwrap();
        assert!(overlaid.data[short.len()..65].iter().all(|byte| *byte == 0));
        assert_eq!(overlaid.current_root(), fresh.current_root());
    }
    #[test]
    fn code_load_enforces_the_exact_code_region_and_is_atomic() {
        let mut memory = Memory::new();
        assert_eq!(memory.code_len(), 0);
        assert!(matches!(
            memory.load_u8(0),
            Err(VMError::MemoryAccessViolation {
                perm: Perm::READ,
                ..
            })
        ));

        let boundary = usize::try_from(Memory::HEAP_START).unwrap();
        let valid = vec![0xA5; boundary];
        memory
            .load_code(&valid)
            .expect("the code region's full extent is valid");
        assert_eq!(memory.code_len(), Memory::HEAP_START);
        assert_eq!(memory.load_u8(Memory::HEAP_START - 1), Ok(0xA5));

        let invalid = vec![0x5A; boundary + 1];
        assert_eq!(memory.load_code(&invalid), Err(VMError::MemoryOutOfBounds));
        assert_eq!(memory.code_len(), Memory::HEAP_START);
        assert_eq!(memory.load_u8(0), Ok(0xA5));
        assert_eq!(memory.load_u8(Memory::HEAP_START - 1), Ok(0xA5));
    }
    #[test]
    fn block_tracking_survives_template_cleaning_for_non_code_writes() {
        let mut template = Memory::new();
        let expected_root = template.current_root();
        let mut worker = template.clone();
        worker.begin_block_transaction_tracking();
        worker
            .store_u64(Memory::HEAP_START, 0xDEAD_BEEF)
            .expect("write transaction heap");
        worker.load_code(&[0xA5; 4]).unwrap();
        worker.commit();
        worker.mark_template_clean();
        assert!(worker.modified_chunks.is_empty());

        worker
            .reset_for_block_transaction(&template)
            .expect("worker and block template geometries match");

        assert_eq!(worker.load_u64(Memory::HEAP_START), Ok(0));
        assert_eq!(worker.current_root(), expected_root);
    }
    #[test]
    fn program_heap_clear_scrubs_inactive_capacity_and_resets_allocator() {
        let mut memory = Memory::new();
        memory
            .store_u64(Memory::HEAP_START, 0x1111)
            .expect("write active heap");
        let later_address = Memory::HEAP_START + 0x20_000;
        memory
            .store_u64(later_address, 0x2222)
            .expect("write future inactive heap");
        assert_eq!(memory.alloc(16), Ok(Memory::HEAP_START));
        memory
            .set_heap_max_limit(0x1_000)
            .expect("tighten heap authority above allocation");
        let mut pristine = Memory::new();
        pristine
            .set_heap_max_limit(0x1_000)
            .expect("match heap authority");

        memory.clear_program_heap();

        assert_eq!(memory.heap_allocated_len(), 0);
        assert_eq!(memory.heap_limit(), pristine.heap_limit());
        assert_eq!(memory.heap_max_limit(), pristine.heap_max_limit());
        assert_eq!(memory.data[Memory::HEAP_START as usize], 0);
        assert_eq!(memory.data[later_address as usize], 0);
        assert_eq!(memory.current_root(), pristine.current_root());
        assert_eq!(memory.alloc(8), Ok(Memory::HEAP_START));
    }
    #[test]
    fn runtime_template_geometry_mismatch_fails_without_replacing_memory() {
        let mut worker = Memory::new_with_stack_limit(Memory::STACK_ALIGNMENT).unwrap();
        let template = Memory::new_with_stack_limit(2 * Memory::STACK_ALIGNMENT).unwrap();
        worker
            .store_u8(Memory::HEAP_START, 0xA5)
            .expect("dirty worker memory");
        assert_eq!(
            worker
                .load_u8(Memory::HEAP_START)
                .expect("read dirty worker memory"),
            0xA5
        );
        let worker_geometry = worker.geometry();
        let template_geometry = template.geometry();
        let worker_data = worker.data.clone();
        let worker_root = worker.root;
        let worker_dirty = worker.dirty;
        let worker_dirty_chunks = worker.dirty_chunks.clone();
        let worker_modified_chunks = worker.modified_chunks.clone();
        let worker_reads = worker.read_set();
        let worker_writes = worker.write_log();
        let error = worker
            .reset_from_template(&template)
            .expect_err("different stack geometry must reject warm reset");
        assert_eq!(error.current, worker_geometry);
        assert_eq!(error.template, template_geometry);
        assert_eq!(worker.geometry(), worker_geometry);
        assert_eq!(worker.data, worker_data);
        assert_eq!(worker.root, worker_root);
        assert_eq!(worker.dirty, worker_dirty);
        assert_eq!(worker.dirty_chunks, worker_dirty_chunks);
        assert_eq!(worker.modified_chunks, worker_modified_chunks);
        assert_eq!(worker.read_set(), worker_reads);
        assert_eq!(worker.write_log(), worker_writes);
    }
    #[test]
    fn commit_small_dirty_set_uses_incremental_merkle_update() {
        let mut mem = Memory::new();
        let baseline = mem.root();
        let (_, updates_before) = crate::byte_merkle_tree::merkle_update_counters();
        mem.store_u8(Memory::HEAP_START, 0xAA)
            .expect("store in heap");
        mem.store_u8(Memory::HEAP_START + 1, 0x55)
            .expect("store in same chunk");
        let updated = mem.root();
        assert_ne!(updated, baseline, "memory root should change after writes");
        let (_, updates_after) = crate::byte_merkle_tree::merkle_update_counters();
        assert!(
            updates_after >= updates_before.saturating_add(1),
            "same-chunk writes should produce one incremental leaf update"
        );
        assert_eq!(
            mem.dirty_chunks.len(),
            0,
            "commit should drain dirty chunks"
        );
    }
    #[test]
    fn commit_large_dirty_set_matches_full_rebuild_root() {
        let data = vec![0u8; 32 * 8];
        let tree = ByteMerkleTree::from_bytes(&data, 32).unwrap();
        let root = tree.root_hash();
        let mut mem = Memory {
            data,
            stack_limit: Memory::STACK_ALIGNMENT,
            heap_alloc: 0,
            heap_limit: Memory::HEAP_SIZE,
            heap_max_limit: Memory::HEAP_MAX_SIZE,
            heap_contains_data: false,
            code_length: 0,
            output_cursor: 0,
            root,
            tree,
            dirty: false,
            dirty_chunks: HashSet::new(),
            modified_chunks: HashSet::new(),
            block_modified_chunks: None,
            template_generation: 0,
            baseline_lineage: Arc::new(()),
            read_log: Mutex::new(Vec::new()),
            write_log: Mutex::new(Vec::new()),
        };
        mem.data[0..32].fill(0xAA);
        mem.data[32..64].fill(0x55);
        mem.data[64..96].fill(0x11);
        mem.data[96..128].fill(0x22);
        mem.dirty_chunks.extend([0, 1, 2, 3]);
        mem.dirty = true;
        let updated = mem.root();
        let expected = MerkleTree::<[u8; 32]>::from_byte_chunks(&mem.data, 32)
            .expect("canonical tree")
            .root()
            .expect("root");
        assert_eq!(updated, expected);
        assert!(
            mem.dirty_chunks.is_empty(),
            "large commit should drain dirty chunks"
        );
    }
    #[test]
    fn large_commit_keeps_unaligned_memory_tree_shape() {
        let data = vec![0u8; 32 * 8 + 16];
        let tree = ByteMerkleTree::new(8, 32).unwrap();
        let root = tree.root_hash();
        let mut incremental = Memory {
            data: data.clone(),
            stack_limit: Memory::STACK_ALIGNMENT,
            heap_alloc: 0,
            heap_limit: Memory::HEAP_SIZE,
            heap_max_limit: Memory::HEAP_MAX_SIZE,
            heap_contains_data: false,
            code_length: 0,
            output_cursor: 0,
            root,
            tree,
            dirty: false,
            dirty_chunks: HashSet::new(),
            modified_chunks: HashSet::new(),
            block_modified_chunks: None,
            template_generation: 0,
            baseline_lineage: Arc::new(()),
            read_log: Mutex::new(Vec::new()),
            write_log: Mutex::new(Vec::new()),
        };
        let mut rebuilt = incremental.clone();
        incremental.data[0..32].fill(0xAA);
        incremental.data[32..64].fill(0x55);
        incremental.dirty_chunks.extend([0, 1]);
        incremental.dirty = true;
        incremental.commit();
        incremental.data[64..96].fill(0x11);
        incremental.data[96..128].fill(0x22);
        incremental.dirty_chunks.extend([2, 3]);
        incremental.dirty = true;
        let incremental_root = incremental.root();
        rebuilt.data[0..32].fill(0xAA);
        rebuilt.data[32..64].fill(0x55);
        rebuilt.data[64..96].fill(0x11);
        rebuilt.data[96..128].fill(0x22);
        rebuilt.dirty_chunks.extend([0, 1, 2, 3]);
        rebuilt.dirty = true;
        let rebuilt_root = rebuilt.root();
        assert_eq!(rebuilt.tree.leaf_count(), 8);
        assert_eq!(rebuilt_root, incremental_root);
    }
    #[test]
    fn preload_input_out_of_bounds_fails() {
        let mut mem = Memory::new();
        // Offset equal to INPUT_SIZE should be rejected even for empty writes.
        assert!(matches!(
            mem.preload_input(Memory::INPUT_SIZE, &[1]),
            Err(VMError::MemoryOutOfBounds)
        ));
        // Writing past the end should also fail.
        assert!(matches!(
            mem.preload_input(Memory::INPUT_SIZE - 1, &[1, 2]),
            Err(VMError::MemoryOutOfBounds)
        ));
    }
    #[test]
    fn input_write_aligned_rejects_invalid_alignment_and_overflow() {
        let mut mem = Memory::new();
        let baseline = mem.current_root();

        for align in [0, 3] {
            let mut cursor = 1;
            assert_eq!(
                mem.input_write_aligned(&mut cursor, &[0xA5], align),
                Err(VMError::MemoryOutOfBounds)
            );
            assert_eq!(cursor, 1, "failed allocation must preserve the cursor");
        }

        let mut cursor = u64::MAX;
        assert_eq!(
            mem.input_write_aligned(&mut cursor, &[0xA5], 8),
            Err(VMError::MemoryOutOfBounds)
        );
        assert_eq!(cursor, u64::MAX);
        assert_eq!(mem.current_root(), baseline, "failed writes must be atomic");
    }
    #[test]
    fn alloc_rejects_overflow_sizes() {
        let mut mem = Memory::new();
        assert!(matches!(mem.alloc(u64::MAX), Err(VMError::OutOfMemory)));
        // Heap cursor should remain unchanged after failure.
        assert_eq!(mem.heap_alloc, 0);
        let small = mem.alloc(16).expect("small allocation succeeds");
        assert_eq!(small, Memory::HEAP_START);
    }
    #[test]
    fn per_instance_heap_ceiling_cannot_be_bypassed_by_growth() {
        let mut mem = Memory::new();
        mem.set_heap_max_limit(64)
            .expect("install governed heap ceiling");
        assert_eq!(mem.heap_limit(), 64);
        assert_eq!(mem.heap_max_limit(), 64);
        assert_eq!(mem.alloc(64), Ok(Memory::HEAP_START));
        assert_eq!(mem.grow_heap(8), Err(VMError::OutOfMemory));
        assert_eq!(mem.alloc(1), Err(VMError::OutOfMemory));
    }
    #[test]
    fn grow_heap_rejects_overflow() {
        let mut mem = Memory::new();
        mem.set_heap_limit(Memory::HEAP_MAX_SIZE - 64)
            .expect("lower heap limit before bounded grow");
        let original_limit = mem.heap_limit();
        assert!(matches!(mem.grow_heap(u64::MAX), Err(VMError::OutOfMemory)));
        assert_eq!(mem.heap_limit(), original_limit);
        // Growing within bounds still works.
        mem.grow_heap(32).expect("bounded grow succeeds");
        assert_eq!(mem.heap_limit(), original_limit + 32);
    }
    #[test]
    fn store_u128_respects_output_append_only() {
        let mut mem = Memory::new();
        let base = Memory::OUTPUT_START;
        mem.store_u128(base, 0x0123_4567_89AB_CDEF_0123_4567_89AB_CDEF)
            .expect("initial append succeeds");
        let err = mem.store_u128(base, 0xDEAD_BEEF_DEAD_BEEF_DEAD_BEEF_DEAD_BEEF);
        assert!(matches!(err, Err(VMError::MemoryAccessViolation { .. })));
        mem.store_u128(base + 16, 0x1111_2222_3333_4444_5555_6666_7777_8888)
            .expect("append at cursor succeeds");
    }
    #[test]
    fn load_region_rejects_oversized_len() {
        let mem = Memory::new();
        let err = mem.load_region(Memory::HEAP_START, u64::from(u32::MAX) + 1);
        assert!(matches!(
            err,
            Err(VMError::MemoryAccessViolation {
                perm: Perm::READ,
                ..
            })
        ));
    }
    #[test]
    fn byte_slice_access_rejects_ranges_crossing_region_boundaries() {
        let mut mem = Memory::new();
        let final_heap_byte = Memory::HEAP_START + Memory::HEAP_MAX_SIZE - 1;
        let mut output = [0_u8; 2];
        assert!(matches!(
            mem.load_bytes(final_heap_byte, &mut output),
            Err(VMError::MemoryAccessViolation {
                perm: Perm::READ,
                ..
            })
        ));
        assert!(matches!(
            mem.store_bytes(final_heap_byte, &[1, 2]),
            Err(VMError::MemoryAccessViolation {
                perm: Perm::WRITE,
                ..
            })
        ));
    }
    #[test]
    fn quote_inspection_does_not_mutate_memory_access_tracking() {
        let mut mem = Memory::new();
        let address = mem.alloc(4).expect("allocate quote fixture");
        mem.store_bytes(address, &[1, 2, 3, 4])
            .expect("write quote fixture");
        mem.clear_tracking();
        assert_eq!(
            mem.inspect_region(address, 4).expect("inspect fixture"),
            &[1, 2, 3, 4]
        );
        assert!(mem.read_set().is_empty());
        mem.load_region(address, 4).expect("tracked load fixture");
        assert_eq!(
            mem.read_set(),
            vec![AccessRange {
                addr: address,
                len: 4
            }]
        );
    }
    #[test]
    fn canonical_stack_limit_boundary_is_enforced() {
        let mut mem = Memory::new();
        assert_eq!(mem.stack_limit(), IvmStackPolicy::V1.maximum_stack_bytes());
        let ok_addr = mem.stack_top() - 1;
        mem.store_u8(ok_addr, 1).expect("write within limit");
        let err = mem.store_u8(mem.stack_top(), 1);
        assert!(matches!(
            err,
            Err(VMError::MemoryAccessViolation {
                perm: Perm::WRITE,
                ..
            })
        ));
    }
    #[test]
    fn explicit_unaligned_stack_limit_is_normalized_before_exposure() {
        let mut mem = Memory::new_with_stack_limit(0x60a04).unwrap();
        assert_eq!(mem.stack_limit() % Memory::STACK_ALIGNMENT, 0);
        assert_eq!(mem.stack_top() % Memory::STACK_ALIGNMENT, 0);
        mem.store_u64(mem.stack_top() - 8, 7)
            .expect("aligned stack top must accept 64-bit stores");
    }
    #[test]
    fn stack_constructor_enforces_v1_limits() {
        let minimum = Memory::new_with_stack_limit(0).unwrap();
        assert_eq!(minimum.stack_limit(), Memory::STACK_ALIGNMENT);

        let maximum = Memory::new_with_stack_limit(Memory::STACK_SIZE).unwrap();
        assert_eq!(maximum.stack_limit(), Memory::STACK_SIZE);
        assert_eq!(
            maximum.stack_top(),
            Memory::STACK_START + Memory::STACK_SIZE
        );

        assert!(matches!(
            Memory::new_with_stack_limit(Memory::STACK_SIZE + 1),
            Err(VMError::MemoryOutOfBounds)
        ));
        assert!(matches!(
            Memory::new_with_stack_limit(u64::MAX),
            Err(VMError::MemoryOutOfBounds)
        ));
    }
    #[test]
    fn memory_merkle_helpers_reject_the_exclusive_end_address() {
        let mut memory = Memory::new_with_stack_limit(Memory::STACK_ALIGNMENT).unwrap();
        let final_byte = memory.stack_top() - 1;
        assert!(memory.merkle_path(final_byte).is_ok());
        assert!(memory.merkle_root_and_path(final_byte).is_ok());
        assert!(memory.merkle_compact(final_byte, None).is_ok());

        for invalid in [memory.stack_top(), u64::MAX] {
            assert_eq!(memory.merkle_path(invalid), Err(VMError::MemoryOutOfBounds));
            assert_eq!(
                memory.merkle_root_and_path(invalid),
                Err(VMError::MemoryOutOfBounds)
            );
            assert_eq!(
                memory.merkle_compact(invalid, None),
                Err(VMError::MemoryOutOfBounds)
            );
        }
    }
    #[test]
    fn current_root_recomputes_dirty_state() {
        let mut mem = Memory::new();
        let baseline = mem.current_root();
        let addr = Memory::HEAP_START;
        mem.store_u64(addr, 0xCAFEBABE_DEADBEEF).unwrap();
        mem.store_u32(addr + 32, 0xA5A5_5A5A).unwrap();
        let mut clone = mem.clone();
        let expected = clone.root();
        let observed = mem.current_root();
        assert_ne!(observed, baseline);
        assert_eq!(observed, expected);
        assert!(mem.dirty_ranges().is_empty());
    }
    #[test]
    fn merkle_path_without_explicit_commit_reflects_writes() {
        let mut mem = Memory::new();
        let addr = Memory::HEAP_START + 96;
        mem.store_u64(addr, 0xFEED_FACE_DEAD_BEEFu64).unwrap();
        let mut reference = mem.clone();
        let path = mem.merkle_path(addr).unwrap();
        let root = mem.current_root();
        let expected_path = reference.merkle_path(addr).unwrap();
        let expected_root = reference.current_root();
        assert_eq!(root, expected_root);
        assert_eq!(path, expected_path);
    }
    #[test]
    fn merkle_compact_without_explicit_commit_matches_path() {
        let mut mem = Memory::new();
        let addr = Memory::HEAP_START + 160;
        mem.store_u32(addr, 0x1357_9BDF).unwrap();
        let mut reference = mem.clone();
        let (proof, root) = mem.merkle_compact(addr, Some(12)).unwrap();
        let depth = proof.depth() as usize;
        assert_eq!(proof.siblings().len(), depth);
        assert_ne!(
            proof.dirs(),
            (addr / 32) as u32,
            "depth-capped proof must use only its encoded direction bits"
        );
        let (expected_root, expected_path) = reference.merkle_root_and_path(addr).unwrap();
        let mut chunk = [0u8; 32];
        reference
            .load_bytes((addr / 32) * 32, &mut chunk)
            .expect("load chunk");
        let leaf_digest = compute_memory_leaf_digest(&chunk);
        let leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_digest));
        let partial_proof = MerkleProof::from_audit_path(
            proof.dirs(),
            expected_path
                .iter()
                .take(depth)
                .map(|b| {
                    if *b == [0u8; 32] {
                        None
                    } else {
                        Some(HashOf::from_untyped_unchecked(Hash::prehashed(*b)))
                    }
                })
                .collect(),
        );
        let expected_compact_root = if depth < expected_path.len() {
            partial_proof
                .compute_partial_root_sha256(&leaf_hash, depth)
                .expect("proof height equals compact depth")
        } else {
            expected_root
        };
        assert_eq!(root, expected_compact_root);
        assert!(depth <= expected_path.len());
        for (i, sibling) in proof.siblings().iter().enumerate() {
            if i >= depth {
                break;
            }
            let sib_bytes = sibling
                .as_ref()
                .map(|hash| {
                    let mut arr = [0u8; 32];
                    arr.copy_from_slice(hash.as_ref());
                    arr
                })
                .unwrap_or([0u8; 32]);
            assert_eq!(sib_bytes, expected_path[i]);
        }
    }
}
