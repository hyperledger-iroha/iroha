//! CPU register file: 256 general-purpose registers with optional privacy tags.
//!
//! The register file implements the full set of helpers required by the
//! specification, including lane-level accessors for vector registers.
//!
//! The original implementation exposed only 32 general purpose registers and a
//! separate set of vector registers.  The updated architecture requires a much
//! larger register file (256 entries) and associates a 1‑bit privacy tag with
//! each register when zero–knowledge mode is active.  Vector operations no
//! longer use a dedicated register file – instead groups of the general
//! registers are interpreted as vectors.  This module implements that design.
use std::cell::{Cell, RefCell};

use iroha_crypto::{CompactMerkleProof, Hash, HashOf, MerkleProof, MerkleTree};
use sha2::{Digest, Sha256};

use crate::zk::{RegEvent, with_reg_logger};

pub struct Registers {
    /// 256 general purpose 64-bit registers. `r0` is hardwired to zero.
    gpr: [u64; 256],
    /// Privacy tags associated with each register. `false` denotes public data
    /// and `true` denotes private (secret) data.
    tags: [bool; 256],
    /// Execution-scope usage bookkeeping for register-telemetry sampling.
    usage: Cell<RegisterUsage>,
    /// Merkle tree commitment to the register contents and tags (canonical type).
    tree: RefCell<MerkleTree<[u8; 32]>>,
    /// Cached leaf digests for efficient rebuilds of the canonical Merkle tree.
    leaves: Vec<[u8; 32]>,
    /// Dirty flag to defer rebuilds until root/path are requested.
    dirty: Cell<bool>,
    /// Pending register indices awaiting incremental rebuild.
    pending: RefCell<Vec<usize>>,
}

impl Clone for Registers {
    fn clone(&self) -> Self {
        // Avoid cloning RefCell<MerkleTree> via RefCell::clone(), which would
        // attempt to borrow() the inner tree and can panic if a mutable borrow
        // is active in another thread (e.g., during incremental updates).
        // Instead, rebuild a fresh Merkle tree from cached leaf digests.
        let gpr = self.gpr;
        let tags = self.tags;
        let usage = self.usage.get();
        let leaves = self.leaves.clone();
        let tree = MerkleTree::from_hashed_leaves_sha256(leaves.clone());
        let pending = self.pending.borrow().clone();
        Registers {
            gpr,
            tags,
            usage: Cell::new(usage),
            tree: RefCell::new(tree),
            leaves,
            dirty: Cell::new(self.dirty.get()),
            pending: RefCell::new(pending),
        }
    }
}

impl Registers {
    #[inline]
    fn record_usage(&self, idx: usize) {
        #[cfg(any(feature = "iroha_telemetry", test))]
        {
            debug_assert!(idx < 256);
            let mut usage = self.usage.get();
            usage.mark(idx);
            self.usage.set(usage);
        }
        #[cfg(not(any(feature = "iroha_telemetry", test)))]
        {
            let _ = idx;
        }
    }

    #[inline]
    fn mark_pending(&self, idx: usize) {
        let mut pending = self.pending.borrow_mut();
        if !pending.contains(&idx) {
            pending.push(idx);
        }
        self.dirty.set(true);
    }

    #[inline]
    pub fn new() -> Self {
        let gpr = [0u64; 256];
        let tags = [false; 256];
        let usage = Cell::new(RegisterUsage::new());
        let zero_leaf: [u8; 32] = {
            let b = [0u8; 9];
            Sha256::digest(b).into()
        };
        let leaves = vec![zero_leaf; 256];
        let tree = MerkleTree::from_hashed_leaves_sha256(leaves.clone());
        Registers {
            gpr,
            tags,
            usage,
            tree: RefCell::new(tree),
            leaves,
            dirty: Cell::new(false),
            pending: RefCell::new(Vec::new()),
        }
    }

    /// Snapshot of unique register usage since the last reset.
    #[inline]
    pub fn usage_summary(&self) -> RegisterUsageSummary {
        #[cfg(any(feature = "iroha_telemetry", test))]
        {
            self.usage.get().summary()
        }
        #[cfg(not(any(feature = "iroha_telemetry", test)))]
        {
            RegisterUsageSummary::default()
        }
    }

    /// Clear the execution-scope usage accounting without touching register contents.
    #[inline]
    pub fn clear_usage(&self) {
        #[cfg(any(feature = "iroha_telemetry", test))]
        self.usage.set(RegisterUsage::new());
    }
    /// Get the value of register `idx`.
    #[inline]
    pub fn get(&self, idx: usize) -> u64 {
        debug_assert!(idx < 256);
        self.record_usage(idx);
        let val = self.gpr[idx];
        with_reg_logger(|log| {
            let (root, path) = self.merkle_root_and_path(idx);
            log.record(RegEvent::Read {
                index: idx,
                value: val,
                tag: self.tags[idx],
                path,
                root,
            });
        });
        val
    }
    /// Set the value of register `idx`. Writes to x0 are ignored (x0 is always 0).
    #[inline]
    pub fn set(&mut self, idx: usize, value: u64) {
        debug_assert!(idx < 256);
        if idx != 0 {
            self.record_usage(idx);
            self.gpr[idx] = value;
            self.leaves[idx] = register_leaf_digest(self.gpr[idx], self.tags[idx]);
            #[cfg(feature = "merkle_incremental")]
            {
                self.tree
                    .borrow_mut()
                    .update_hashed_leaf_sha256(idx, self.leaves[idx]);
                self.dirty.set(false);
            }
            #[cfg(not(feature = "merkle_incremental"))]
            {
                self.mark_pending(idx);
            }
            with_reg_logger(|log| {
                let (root, path) = self.merkle_root_and_path(idx);
                log.record(RegEvent::Write {
                    index: idx,
                    value,
                    tag: self.tags[idx],
                    path,
                    root,
                });
            });
        }
    }

    /// Unconditionally set register `idx`, including r0.
    #[inline]
    pub fn force_set(&mut self, idx: usize, value: u64) {
        debug_assert!(idx < 256);
        self.record_usage(idx);
        self.gpr[idx] = value;
        self.leaves[idx] = register_leaf_digest(self.gpr[idx], self.tags[idx]);
        #[cfg(feature = "merkle_incremental")]
        {
            self.tree
                .borrow_mut()
                .update_hashed_leaf_sha256(idx, self.leaves[idx]);
            self.dirty.set(false);
        }
        #[cfg(not(feature = "merkle_incremental"))]
        {
            self.mark_pending(idx);
        }
        with_reg_logger(|log| {
            let (root, path) = self.merkle_root_and_path(idx);
            log.record(RegEvent::Write {
                index: idx,
                value,
                tag: self.tags[idx],
                path,
                root,
            });
        });
    }

    /// Get the privacy tag of register `idx`.
    #[inline]
    pub fn tag(&self, idx: usize) -> bool {
        debug_assert!(idx < 256);
        self.record_usage(idx);
        self.tags[idx]
    }

    /// Set the privacy tag of register `idx`. Writing to `r0` has no effect.
    #[inline]
    pub fn set_tag(&mut self, idx: usize, value: bool) {
        debug_assert!(idx < 256);
        if idx != 0 {
            self.record_usage(idx);
            self.tags[idx] = value;
            self.leaves[idx] = register_leaf_digest(self.gpr[idx], self.tags[idx]);
            #[cfg(feature = "merkle_incremental")]
            {
                self.tree
                    .borrow_mut()
                    .update_hashed_leaf_sha256(idx, self.leaves[idx]);
                self.dirty.set(false);
            }
            #[cfg(not(feature = "merkle_incremental"))]
            {
                self.mark_pending(idx);
            }
            with_reg_logger(|log| {
                let (root, path) = self.merkle_root_and_path(idx);
                log.record(RegEvent::Write {
                    index: idx,
                    value: self.gpr[idx],
                    tag: value,
                    path,
                    root,
                });
            });
        }
    }

    /// Unconditionally set the privacy tag of register `idx`.
    #[inline]
    pub fn force_set_tag(&mut self, idx: usize, value: bool) {
        debug_assert!(idx < 256);
        self.record_usage(idx);
        self.tags[idx] = value;
        self.leaves[idx] = register_leaf_digest(self.gpr[idx], self.tags[idx]);
        #[cfg(feature = "merkle_incremental")]
        {
            self.tree
                .borrow_mut()
                .update_hashed_leaf_sha256(idx, self.leaves[idx]);
            self.dirty.set(false);
        }
        #[cfg(not(feature = "merkle_incremental"))]
        {
            self.mark_pending(idx);
        }
        with_reg_logger(|log| {
            let (root, path) = self.merkle_root_and_path(idx);
            log.record(RegEvent::Write {
                index: idx,
                value: self.gpr[idx],
                tag: value,
                path,
                root,
            });
        });
    }

    /// Mutable access for test‑suites and advanced host tooling.
    #[inline]
    pub fn set_raw(&mut self, index: usize, value: u64) {
        self.set(index, value);
    }

    /// Return a copy of all general-purpose registers.
    #[inline]
    pub fn snapshot(&self) -> [u64; 256] {
        self.gpr
    }

    /// Return a copy of all privacy tags.
    #[inline]
    pub fn snapshot_tags(&self) -> [bool; 256] {
        self.tags
    }

    /// Return the Merkle root of the register file.
    #[inline]
    pub fn merkle_root(&self) -> HashOf<MerkleTree<[u8; 32]>> {
        self.ensure_built()
            .borrow()
            .root()
            .expect("tree has at least one leaf")
    }

    /// Merkle authentication path for register `idx`.
    #[inline]
    pub fn merkle_path(&self, idx: usize) -> Vec<[u8; 32]> {
        let proof = self
            .ensure_built()
            .borrow()
            .get_proof(idx as u32)
            .expect("valid index");
        proof
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect()
    }

    /// Combined helper: return both the typed Merkle root and authentication
    /// path for `idx`. Performs at most one rebuild and borrows the tree once.
    #[inline]
    pub fn merkle_root_and_path(
        &self,
        idx: usize,
    ) -> (HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>) {
        let tree_ref = self.ensure_built();
        let tree = tree_ref.borrow();
        let root = tree.root().expect("tree has at least one leaf");
        let path = tree
            .get_proof(idx as u32)
            .expect("valid index")
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        (root, path)
    }

    /// Build a compact Merkle proof for the register at `idx`. Returns the
    /// compact proof and the current typed register Merkle root. `depth_cap`
    /// can restrict the number of levels used (at most 32).
    #[inline]
    pub fn merkle_compact(
        &self,
        idx: usize,
        depth_cap: Option<usize>,
    ) -> (CompactMerkleProof<[u8; 32]>, HashOf<MerkleTree<[u8; 32]>>) {
        let (root, path) = self.merkle_root_and_path(idx);
        let proof = crate::merkle_utils::make_compact_from_path_bytes(&path, idx as u32, depth_cap);
        let leaf_digest = register_leaf_digest(self.gpr[idx], self.tags[idx]);
        let leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_digest));
        let siblings = proof.siblings().to_vec();
        let merkle_proof = MerkleProof::from_audit_path(idx as u32, siblings);
        let adj_root = merkle_proof
            .compute_root_sha256(&leaf_hash, proof.depth() as usize)
            .unwrap_or(root);
        (proof, adj_root)
    }

    #[inline]
    /// Ensure the Merkle tree is rebuilt when marked dirty. This uses a
    /// deterministic full rebuild today; once `iroha_crypto` exposes a
    /// canonical incremental builder, the `merkle_incremental` feature gate
    /// will switch this to an incremental update path.
    fn ensure_built(&self) -> &RefCell<MerkleTree<[u8; 32]>> {
        if self.dirty.get() {
            self.rebuild_tree();
            self.dirty.set(false);
        }
        &self.tree
    }

    #[inline]
    /// Rebuild the Merkle tree from cached leaf digests.
    fn rebuild_tree(&self) {
        #[cfg(feature = "merkle_incremental")]
        {
            let mut pending = self.pending.borrow_mut();
            if pending.is_empty() {
                return;
            }
            pending.sort_unstable();
            pending.dedup();
            let indices: Vec<usize> = pending.drain(..).collect();
            drop(pending);

            let mut tree = self.tree.borrow_mut();
            for idx in indices {
                tree.update_hashed_leaf_sha256(idx, self.leaves[idx]);
            }
        }
        #[cfg(not(feature = "merkle_incremental"))]
        {
            let tree = MerkleTree::from_hashed_leaves_sha256(self.leaves.clone());
            self.tree.replace(tree);
            self.pending.borrow_mut().clear();
        }
    }

    /// Get a vector stored starting at register `idx` (uses two consecutive
    /// registers as a 128-bit value containing four 32-bit lanes).
    #[inline]
    pub fn get_vector(&self, idx: usize) -> [u32; 4] {
        debug_assert!(idx + 1 < 256);
        let lo = self.get(idx);
        let hi = self.get(idx + 1);
        [
            (lo & 0xffff_ffff) as u32,
            (lo >> 32) as u32,
            (hi & 0xffff_ffff) as u32,
            (hi >> 32) as u32,
        ]
    }

    /// Store a vector at register `idx` (two consecutive registers).
    #[inline]
    pub fn set_vector(&mut self, idx: usize, vals: [u32; 4]) {
        debug_assert!(idx + 1 < 256);
        let lo = (vals[0] as u64) | ((vals[1] as u64) << 32);
        let hi = (vals[2] as u64) | ((vals[3] as u64) << 32);
        self.set(idx, lo);
        self.set(idx + 1, hi);
    }
}

impl Default for Registers {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(any(feature = "iroha_telemetry", test))]
#[derive(Clone, Copy)]
struct RegisterUsage {
    bitmap: [u64; 4],
    max_index: u16,
}

#[cfg(not(any(feature = "iroha_telemetry", test)))]
#[derive(Clone, Copy, Default)]
struct RegisterUsage;

#[cfg(any(feature = "iroha_telemetry", test))]
impl RegisterUsage {
    const fn new() -> Self {
        Self {
            bitmap: [0; 4],
            max_index: 0,
        }
    }

    fn mark(&mut self, idx: usize) {
        debug_assert!(idx < 256);
        let word = idx / 64;
        let bit = idx % 64;
        self.bitmap[word] |= 1u64 << bit;
        if idx as u16 > self.max_index {
            self.max_index = idx as u16;
        }
    }

    fn summary(self) -> RegisterUsageSummary {
        let unique_registers = self.unique_count();
        let max_index = if unique_registers == 0 {
            0
        } else {
            self.max_index as usize
        };
        RegisterUsageSummary {
            max_index,
            unique_registers,
        }
    }

    fn unique_count(&self) -> u16 {
        self.bitmap
            .iter()
            .map(|word| word.count_ones() as u16)
            .sum()
    }
}

#[cfg(not(any(feature = "iroha_telemetry", test)))]
impl RegisterUsage {
    const fn new() -> Self {
        Self
    }
}

/// Summary of register pressure for a single VM execution.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RegisterUsageSummary {
    pub max_index: usize,
    pub unique_registers: u16,
}

#[inline]
fn register_leaf_digest(value: u64, tag: bool) -> [u8; 32] {
    let mut bytes = [0u8; 9];
    bytes[0] = if tag { 1 } else { 0 };
    bytes[1..].copy_from_slice(&value.to_le_bytes());
    Sha256::digest(bytes).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usage_tracks_max_index_and_unique_registers() {
        let mut regs = Registers::new();

        regs.set(5, 10);
        regs.set(127, 20);
        let _ = regs.get(5);

        let snapshot = regs.usage_summary();
        assert_eq!(snapshot.max_index, 127);
        assert_eq!(snapshot.unique_registers, 2);

        regs.clear_usage();
        let cleared = regs.usage_summary();
        assert_eq!(cleared.unique_registers, 0);
        assert_eq!(cleared.max_index, 0);
    }
}
