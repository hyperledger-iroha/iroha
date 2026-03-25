//! Region-based memory manager implementing the IVM memory model.
//!
//! The memory subsystem enforces permissions, alignment and region bounds for
//! all loads and stores. Heap allocation is supported and vector accesses are
//! checked for 16‑byte alignment as required by the specification. Memory is
//! divided into disjoint regions:
//!
//! * **Code** – loaded at address `0x0000_0000` and marked read/execute only.
//! * **Heap** – starts at `0x0010_0000` and grows upward via `SYSCALL_ALLOC`.
//! * **Input** – read-only buffer beginning at `0x0020_0000` (64 KB).
//! * **Output** – read/write buffer beginning at `0x0021_0000`.
//! * **Stack** – 4&nbsp;MB region starting at `0x0030_0000`.
use std::{
    collections::HashSet,
    convert::TryInto,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};

use iroha_crypto::{CompactMerkleProof, Hash, HashOf, MerkleProof, MerkleTree};
use iroha_telemetry::metrics::record_stack_budget_hit;
use likely_stable::{likely, unlikely};
use parking_lot::Mutex;

use crate::{
    byte_merkle_tree::ByteMerkleTree,
    error::{Perm, VMError},
    merkle_utils::compute_memory_leaf_digest,
};

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
/// In accordance with the updated architecture the entire memory image is
/// committed via a Merkle tree.  Writes mark ranges dirty and the [`root`]
/// is recomputed lazily on `commit()` by hashing only the modified chunks.
/// This avoids re-hashing untouched memory while still enabling inclusion
/// paths to be produced after a commit.
/// Zero‑knowledge mode can request inclusion paths for any address.
pub struct Memory {
    data: Vec<u8>,
    stack_limit: u64,
    heap_alloc: u64,
    heap_limit: u64,
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
    /// Addresses read during execution when access tracking is enabled.
    read_log: Mutex<Vec<AccessRange>>,
    /// Log of writes performed during execution (byte-accurate).
    write_log: Mutex<Vec<WriteLogEntry>>,
}

// Default stack limit applied to new memory instances (mutable via setters).
static DEFAULT_STACK_LIMIT: LazyLock<AtomicU64> =
    LazyLock::new(|| AtomicU64::new(Memory::STACK_SIZE));
// Maximum budget cap applied to new memory instances (mutable via setters).
static STACK_BUDGET_LIMIT: LazyLock<AtomicU64> = LazyLock::new(|| AtomicU64::new(u64::MAX));

impl Memory {
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
    /// Logical stack size for guest programs (4 MiB default; hosts override via `ivm::apply_stack_sizes`).
    pub const STACK_SIZE: u64 = 0x0040_0000; // 4 MB stack
    /// Extra slop beyond the nominal stack end (kept zero to trap exactly at the limit).
    pub const STACK_SLOP: u64 = 0;

    /// Default stack limit (bytes) applied to new memory instances.
    pub fn default_stack_limit() -> u64 {
        DEFAULT_STACK_LIMIT.load(Ordering::Relaxed)
    }

    /// Override the default stack limit (bytes) applied to new memory instances.
    pub fn set_default_stack_limit(bytes: u64) {
        DEFAULT_STACK_LIMIT.store(bytes.max(1), Ordering::Relaxed);
    }

    /// Global budget cap applied to stack limits for new memory instances.
    pub fn stack_budget_limit() -> u64 {
        STACK_BUDGET_LIMIT.load(Ordering::Relaxed)
    }

    /// Override the global budget cap applied to stack limits for new memory instances.
    pub fn set_stack_budget_limit(bytes: u64) {
        STACK_BUDGET_LIMIT.store(bytes.max(1), Ordering::Relaxed);
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
        let indices: Vec<_> = self.dirty_chunks.drain().collect();
        // Heuristic: if more than half the leaves are dirty, prefer a full GPU recompute path
        // (leaves + reducer) when available to avoid serial CPU reductions.
        let total_leaves = (self.data.len() / 32).max(1);
        let large_update = indices.len() * 2 >= total_leaves;
        if large_update && self.tree.recompute_all_leaves_accel(&self.data) {
            // Compute root using accelerated reducer to avoid CPU rebuild.
            let root_bytes =
                crate::byte_merkle_tree::ByteMerkleTree::root_from_bytes_accel(&self.data, 32);
            self.root = HashOf::from_untyped_unchecked(Hash::prehashed(root_bytes));
            self.dirty = false;
            return;
        }
        // CPU path: rehash dirty leaves and rebuild root lazily once.
        self.tree
            .update_leaves_from_bytes_parallel(&self.data, &indices);
        self.root = self.tree.root_hash();
        self.dirty = false;
    }

    /// Commit pending writes by hashing only the dirty chunks if the memory has
    /// been modified since the last commit.
    pub fn commit(&mut self) {
        if self.dirty {
            self.recompute_dirty();
        }
    }

    /// Generate the Merkle authentication path for the 32-byte chunk containing
    /// `addr`. Pending writes are committed before sampling so the returned
    /// path matches the latest memory image.
    pub fn merkle_path(&mut self, addr: u64) -> Vec<[u8; 32]> {
        self.commit();
        const CHUNK: usize = 32;
        let index = (addr as usize) / CHUNK;
        self.tree.path(index)
    }

    /// Return both the current Merkle root (typed `HashOf<MerkleTree<[u8; 32]>>`)
    /// and the authentication path for the 32-byte chunk containing `addr` in a
    /// single operation. Pending writes are committed first to keep the root/path
    /// in sync.
    pub fn merkle_root_and_path(
        &mut self,
        addr: u64,
    ) -> (HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>) {
        self.commit();
        const CHUNK: usize = 32;
        let index = (addr as usize) / CHUNK;
        self.tree.root_and_path(index)
    }

    /// Build a compact Merkle proof for the memory chunk containing `addr`.
    /// Returns the compact proof and the current typed Merkle root. Pending
    /// writes are committed before construction. `depth_cap` can restrict the
    /// number of levels used (at most 32).
    pub fn merkle_compact(
        &mut self,
        addr: u64,
        depth_cap: Option<usize>,
    ) -> (CompactMerkleProof<[u8; 32]>, HashOf<MerkleTree<[u8; 32]>>) {
        let (full_root, path) = self.merkle_root_and_path(addr);
        let leaf_index = (addr / 32) as u32;
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
        let proof_for_root = MerkleProof::from_audit_path(leaf_index, typed_siblings.clone());
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
                .compute_root_sha256(&leaf_hash, depth)
                .unwrap_or(full_root)
        } else {
            full_root
        };

        (compact, root)
    }

    /// Current typed Merkle root, recomputing pending dirty ranges if needed.
    ///
    /// This helper mirrors [`root`](Self::root) but keeps the method name used
    /// by callers that sample the root during execution (e.g., step logs). It
    /// forces a `commit()` so that in-flight writes are reflected in the
    /// returned digest.
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
        let first = start / CHUNK;
        let last = (start + len).div_ceil(CHUNK);
        for i in first..last {
            self.dirty_chunks.insert(i);
        }
        // Mark the tree as dirty so the root is recomputed lazily on the next
        // `commit()` or `root()` call.
        self.dirty = true;
    }

    /// Initialize memory with given code size. Other regions (heap, stack) are also configured.
    pub fn new(code_size: u64) -> Self {
        Self::new_with_stack_limit(code_size, Self::default_stack_limit())
    }

    /// Initialize memory with an explicit stack limit (bytes).
    pub fn new_with_stack_limit(code_size: u64, stack_limit: u64) -> Self {
        let budget = STACK_BUDGET_LIMIT.load(Ordering::Relaxed);
        let effective_stack = stack_limit.max(1).min(budget);
        if effective_stack < stack_limit {
            record_stack_budget_hit();
        }
        let total_size = Memory::STACK_START + effective_stack + Memory::STACK_SLOP;
        let mut mem = Memory {
            data: vec![0u8; total_size as usize],
            stack_limit: effective_stack,
            heap_alloc: 0,
            heap_limit: Memory::HEAP_SIZE,
            code_length: code_size,
            output_cursor: 0,
            root: HashOf::from_untyped_unchecked(Hash::prehashed([0u8; 32])),
            tree: ByteMerkleTree::new((total_size as usize / 32).max(1), 32),
            dirty: false,
            dirty_chunks: HashSet::new(),
            read_log: Mutex::new(Vec::new()),
            write_log: Mutex::new(Vec::new()),
        };
        // initialize root from zeroed memory
        mem.root = mem.tree.root_hash();
        mem
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
        let mask = align.saturating_sub(1);
        let mut off = *cursor;
        if align > 1 {
            let rem = off & mask;
            if rem != 0 {
                off = off.wrapping_add(align - rem);
            }
        }
        let end = off
            .checked_add(bytes.len() as u64)
            .ok_or(VMError::MemoryOutOfBounds)?;
        if end > Memory::INPUT_SIZE {
            return Err(VMError::MemoryOutOfBounds);
        }
        self.preload_input(off, bytes)?;
        *cursor = end;
        Ok(Memory::INPUT_START + off)
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
        if unlikely(new_limit > Memory::HEAP_MAX_SIZE) {
            return Err(VMError::OutOfMemory);
        }
        self.heap_limit = new_limit;
        Ok(self.heap_limit)
    }

    /// Current heap limit in bytes.
    pub fn heap_limit(&self) -> u64 {
        self.heap_limit
    }

    /// Override the active heap limit, keeping the already-allocated region valid.
    pub fn set_heap_limit(&mut self, limit: u64) -> Result<(), VMError> {
        if limit < self.heap_alloc || limit > Memory::HEAP_MAX_SIZE {
            return Err(VMError::OutOfMemory);
        }
        self.heap_limit = limit;
        Ok(())
    }

    /// Update the code region length after loading a program.
    pub fn set_code_length(&mut self, code_size: u64) {
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
    pub fn load_code(&mut self, code: &[u8]) {
        let len = code.len();
        self.data[0..len].copy_from_slice(code);
        self.set_code_length(len as u64);
        self.update_merkle(0, len);
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

    /// Load a 16-bit value (little-endian) from memory.
    #[inline]
    pub fn load_u16(&self, addr: u64) -> Result<u16, VMError> {
        // Ensure 2 bytes and alignment (must be 2-byte aligned)
        if unlikely(!addr.is_multiple_of(2)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 2, Perm::READ)?;
        let bytes: [u8; 2] = self.data[addr as usize..addr as usize + 2]
            .try_into()
            .unwrap();
        self.record_read_range(addr, 2);
        Ok(u16::from_le_bytes(bytes))
    }

    /// Fetch a 16-bit value intended for instruction decoding. Requires execute
    /// permission in addition to read.
    #[inline]
    pub fn fetch_u16(&self, addr: u64) -> Result<u16, VMError> {
        if unlikely(!addr.is_multiple_of(2)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 2, Perm::READ | Perm::EXECUTE)?;
        let bytes: [u8; 2] = self.data[addr as usize..addr as usize + 2]
            .try_into()
            .unwrap();
        self.record_read_range(addr, 2);
        Ok(u16::from_le_bytes(bytes))
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
        let size = out.len() as u32;
        self.check_perm(addr, size, Perm::READ)?;
        let start = addr as usize;
        let end = start + out.len();
        out.copy_from_slice(&self.data[start..end]);
        self.record_read_range(addr, out.len() as u64);
        Ok(())
    }

    /// Load `len` bytes starting at `addr` and return a slice referencing the
    /// underlying memory.
    #[inline]
    pub fn load_region(&self, addr: u64, len: u64) -> Result<&[u8], VMError> {
        if len > u64::from(u32::MAX) {
            return Err(VMError::MemoryAccessViolation {
                addr: addr as u32,
                perm: Perm::READ,
            });
        }
        let len_u32 = len as u32;
        self.check_perm(addr, len_u32, Perm::READ)?;
        let start = addr as usize;
        let len_usize = usize::try_from(len).map_err(|_| VMError::MemoryAccessViolation {
            addr: addr as u32,
            perm: Perm::READ,
        })?;
        let end = start
            .checked_add(len_usize)
            .ok_or(VMError::MemoryAccessViolation {
                addr: addr as u32,
                perm: Perm::READ,
            })?;
        if end > self.data.len() {
            return Err(VMError::MemoryAccessViolation {
                addr: addr as u32,
                perm: Perm::READ,
            });
        }
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
        let size = bytes.len() as u32;
        self.check_perm(addr, size, Perm::WRITE)?;
        // Enforce append-only semantics for OUTPUT region
        self.check_output_append_only(addr, bytes.len() as u64)?;
        let start = addr as usize;
        let end = start + bytes.len();
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

    /// Store a 16-bit value (little-endian) into memory.
    #[inline]
    pub fn store_u16(&mut self, addr: u64, value: u16) -> Result<(), VMError> {
        if unlikely(!addr.is_multiple_of(2)) {
            return Err(VMError::MisalignedAccess { addr: addr as u32 });
        }
        self.check_perm(addr, 2, Perm::WRITE)?;
        self.check_output_append_only(addr, 2)?;
        self.data[addr as usize..addr as usize + 2].copy_from_slice(&value.to_le_bytes());
        self.update_merkle(addr as usize, 2);
        self.record_write(addr, &value.to_le_bytes());
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
        self.write_log.lock().clear();
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

    /// Clear all recorded dirty ranges without committing them.
    pub fn clear_dirty(&mut self) {
        self.dirty_chunks.clear();
        self.dirty = false;
    }

    pub(crate) fn reset_from_template(&mut self, template: &Memory) {
        if self.data.len() != template.data.len() {
            self.data = template.data.clone();
        } else {
            self.data.copy_from_slice(&template.data);
        }
        self.heap_alloc = template.heap_alloc;
        self.heap_limit = template.heap_limit;
        self.code_length = template.code_length;
        self.output_cursor = template.output_cursor;
        self.root = template.root;
        self.dirty = template.dirty;
        self.dirty_chunks = template.dirty_chunks.clone();
        self.tree.reset_from(&template.tree);
        self.clear_tracking();
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
    pub fn overlay_code(&mut self, src: &Memory) {
        let len = src.code_length as usize;
        self.data[0..len].copy_from_slice(&src.data[0..len]);
        self.code_length = len as u64;
        self.update_merkle(0, len);
    }
}

impl Clone for Memory {
    fn clone(&self) -> Self {
        Self {
            data: self.data.clone(),
            stack_limit: self.stack_limit,
            heap_alloc: self.heap_alloc,
            heap_limit: self.heap_limit,
            code_length: self.code_length,
            output_cursor: self.output_cursor,
            root: self.root,
            tree: self.tree.clone(),
            dirty: self.dirty,
            dirty_chunks: self.dirty_chunks.clone(),
            read_log: Mutex::new(Vec::new()),
            write_log: Mutex::new(Vec::new()),
        }
    }
}

impl Memory {
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
    use iroha_crypto::{Hash, HashOf, MerkleProof};

    use super::*;
    use crate::merkle_utils::compute_memory_leaf_digest;

    #[test]
    fn reset_from_template_restores_runtime_regions() {
        let mut base = Memory::new(0);
        base.preload_input(0, &[1, 2, 3, 4])
            .expect("preload template input");

        let mut worker = base.clone();
        worker.alloc(32).expect("alloc");
        worker
            .store_u64(Memory::OUTPUT_START, 0xDEAD_BEEF_DEAD_BEEFu64)
            .expect("store output");
        worker.grow_heap(64).expect("grow heap");

        assert_ne!(&worker.read_output()[..8], &base.read_output()[..8],);

        worker.reset_from_template(&base);

        assert_eq!(worker.heap_alloc, base.heap_alloc);
        assert_eq!(worker.heap_limit(), base.heap_limit());
        assert_eq!(worker.code_len(), base.code_len());
        assert_eq!(worker.read_output(), base.read_output());

        let mut worker_clone = worker.clone();
        let mut base_clone = base.clone();
        assert_eq!(worker_clone.root(), base_clone.root());
    }

    #[test]
    fn preload_input_out_of_bounds_fails() {
        let mut mem = Memory::new(0);
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
    fn alloc_rejects_overflow_sizes() {
        let mut mem = Memory::new(0);
        assert!(matches!(mem.alloc(u64::MAX), Err(VMError::OutOfMemory)));
        // Heap cursor should remain unchanged after failure.
        assert_eq!(mem.heap_alloc, 0);
        let small = mem.alloc(16).expect("small allocation succeeds");
        assert_eq!(small, Memory::HEAP_START);
    }

    #[test]
    fn grow_heap_rejects_overflow() {
        let mut mem = Memory::new(0);
        let original_limit = mem.heap_limit();
        assert!(matches!(mem.grow_heap(u64::MAX), Err(VMError::OutOfMemory)));
        assert_eq!(mem.heap_limit(), original_limit);
        // Growing within bounds still works.
        mem.grow_heap(32).expect("bounded grow succeeds");
        assert_eq!(mem.heap_limit(), original_limit + 32);
    }

    #[test]
    fn store_u128_respects_output_append_only() {
        let mut mem = Memory::new(0);
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
        let mem = Memory::new(0);
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
    fn stack_limit_override_enforced() {
        Memory::set_default_stack_limit(0x2000);
        Memory::set_stack_budget_limit(0x2000);
        let mut mem = Memory::new(0);
        assert_eq!(mem.stack_limit(), 0x2000);
        let ok_addr = Memory::STACK_START + mem.stack_limit() - 8;
        mem.store_u64(ok_addr, 1).expect("write within limit");
        let err = mem.store_u64(mem.stack_top(), 1);
        assert!(matches!(
            err,
            Err(VMError::MemoryAccessViolation {
                perm: Perm::WRITE,
                ..
            })
        ));
        Memory::set_default_stack_limit(Memory::STACK_SIZE);
        Memory::set_stack_budget_limit(u64::MAX);
    }

    #[test]
    fn current_root_recomputes_dirty_state() {
        let mut mem = Memory::new(0);
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
        let mut mem = Memory::new(0);
        let addr = Memory::HEAP_START + 96;
        mem.store_u64(addr, 0xFEED_FACE_DEAD_BEEFu64).unwrap();
        let mut reference = mem.clone();

        let path = mem.merkle_path(addr);
        let root = mem.current_root();

        let expected_path = reference.merkle_path(addr);
        let expected_root = reference.current_root();

        assert_eq!(root, expected_root);
        assert_eq!(path, expected_path);
    }

    #[test]
    fn merkle_compact_without_explicit_commit_matches_path() {
        let mut mem = Memory::new(0);
        let addr = Memory::HEAP_START + 160;
        mem.store_u32(addr, 0x1357_9BDF).unwrap();
        let mut reference = mem.clone();

        let (proof, root) = mem.merkle_compact(addr, Some(12));
        let depth = proof.depth() as usize;
        assert_eq!(proof.siblings().len(), depth);

        let (expected_root, expected_path) = reference.merkle_root_and_path(addr);

        let mut chunk = [0u8; 32];
        reference
            .load_bytes((addr / 32) * 32, &mut chunk)
            .expect("load chunk");
        let leaf_digest = compute_memory_leaf_digest(&chunk);
        let leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_digest));
        let partial_proof = MerkleProof::from_audit_path(
            (addr / 32) as u32,
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
                .compute_root_sha256(&leaf_hash, depth)
                .unwrap_or(expected_root)
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
