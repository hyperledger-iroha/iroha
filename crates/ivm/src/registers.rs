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
use crate::zk::{RegEvent, with_reg_logger};
use crate::{VMError, parallel::REGISTER_COUNT};
use iroha_crypto::{CompactMerkleProof, Hash, HashOf, MerkleProof, MerkleTree};
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::sync::atomic::{AtomicBool, Ordering};
pub struct Registers {
    /// 256 general purpose 64-bit registers. `r0` is hardwired to zero.
    gpr: [u64; 256],
    /// Privacy tags associated with each register. `false` denotes public data
    /// and `true` denotes private (secret) data.
    tags: [bool; 256],
    /// Execution-scope usage bookkeeping for register-telemetry sampling.
    usage: Mutex<RegisterUsage>,
    /// Merkle tree commitment to the register contents and tags (canonical type).
    tree: Mutex<MerkleTree<[u8; 32]>>,
    /// Dirty flag to defer rebuilds until root/path are requested.
    dirty: AtomicBool,
}
impl Clone for Registers {
    fn clone(&self) -> Self {
        let gpr = self.gpr;
        let tags = self.tags;
        let usage = *self.usage.lock();
        let tree = if self.dirty.load(Ordering::Acquire) {
            MerkleTree::from_hashed_leaves_sha256(register_leaf_digests(&gpr, &tags))
        } else {
            self.tree.lock().clone()
        };
        Registers {
            gpr,
            tags,
            usage: Mutex::new(usage),
            tree: Mutex::new(tree),
            dirty: AtomicBool::new(false),
        }
    }
}
impl Registers {
    #[inline]
    fn record_usage(&self, idx: usize) {
        #[cfg(any(feature = "telemetry", test))]
        {
            debug_assert!(idx < 256);
            let mut usage = self.usage.lock();
            usage.mark(idx);
        }
        #[cfg(not(any(feature = "telemetry", test)))]
        {
            let _ = idx;
        }
    }
    #[inline]
    pub fn new() -> Self {
        let gpr = [0u64; 256];
        let tags = [false; 256];
        let usage = Mutex::new(RegisterUsage::new());
        let zero_leaf: [u8; 32] = {
            let b = [0u8; 9];
            Sha256::digest(b).into()
        };
        let tree = MerkleTree::from_hashed_leaves_sha256(vec![zero_leaf; 256]);
        Registers {
            gpr,
            tags,
            usage,
            tree: Mutex::new(tree),
            dirty: AtomicBool::new(false),
        }
    }
    /// Snapshot of unique register usage since the last reset.
    #[inline]
    pub fn usage_summary(&self) -> RegisterUsageSummary {
        #[cfg(any(feature = "telemetry", test))]
        {
            (*self.usage.lock()).summary()
        }
        #[cfg(not(any(feature = "telemetry", test)))]
        {
            RegisterUsageSummary::default()
        }
    }
    /// Clear the execution-scope usage accounting without touching register contents.
    #[inline]
    pub fn clear_usage(&self) {
        *self.usage.lock() = RegisterUsage::new();
    }
    /// Get the value of register `idx`.
    #[inline]
    pub fn get(&self, idx: usize) -> u64 {
        debug_assert!(idx < 256);
        self.record_usage(idx);
        let val = self.gpr[idx];
        with_reg_logger(|log| {
            let (root, path) = self
                .merkle_root_and_path(idx)
                .expect("register access already validated the index");
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
            let was_dirty = self.dirty.swap(true, Ordering::AcqRel);
            with_reg_logger(|log| {
                if !was_dirty {
                    self.tree.get_mut().update_hashed_leaf_sha256(
                        idx,
                        register_leaf_digest(value, self.tags[idx]),
                    );
                    self.dirty.store(false, Ordering::Release);
                }
                let (root, path) = self
                    .merkle_root_and_path(idx)
                    .expect("register access already validated the index");
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
            let was_dirty = self.dirty.swap(true, Ordering::AcqRel);
            with_reg_logger(|log| {
                if !was_dirty {
                    self.tree
                        .get_mut()
                        .update_hashed_leaf_sha256(idx, register_leaf_digest(self.gpr[idx], value));
                    self.dirty.store(false, Ordering::Release);
                }
                let (root, path) = self
                    .merkle_root_and_path(idx)
                    .expect("register access already validated the index");
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
    /// Record a proof-bearing write for the current value of `idx` without
    /// mutating the register file.
    ///
    /// Host callbacks execute with register logging masked so unrelated VMs
    /// cannot inject events. The caller uses this after the callback to publish
    /// only net changes to the VM that actually resumed execution.
    pub(crate) fn record_write_proof(&self, idx: usize) {
        debug_assert!(idx < 256);
        if idx == 0 {
            return;
        }
        with_reg_logger(|log| {
            let (root, path) = self
                .merkle_root_and_path(idx)
                .expect("register access already validated the index");
            log.record(RegEvent::Write {
                index: idx,
                value: self.gpr[idx],
                tag: self.tags[idx],
                path,
                root,
            });
        });
    }
    /// Zero every private register before clearing its privacy tag.
    ///
    /// Public registers are preserved so hosts may preload ordinary arguments
    /// before replacing a program. Zeroing first prevents logs and Merkle
    /// leaves from ever representing a secret value as public.
    pub(crate) fn scrub_private(&mut self) {
        for index in 1..self.tags.len() {
            if self.tags[index] {
                self.set(index, 0);
                self.set_tag(index, false);
            }
        }
    }
    /// Return whether any general-purpose register is private-tagged.
    pub(crate) fn has_private(&self) -> bool {
        self.tags.iter().any(|tag| *tag)
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
        self.ensure_built_and_lock()
            .root()
            .expect("tree has at least one leaf")
    }
    /// Merkle authentication path for register `idx`.
    ///
    /// # Errors
    /// Returns [`VMError::RegisterOutOfBounds`] when `idx` is not a register index.
    #[inline]
    pub fn merkle_path(&self, idx: usize) -> Result<Vec<[u8; 32]>, VMError> {
        let leaf_index = register_leaf_index(idx)?;
        let proof = self
            .ensure_built_and_lock()
            .get_proof(leaf_index)
            .expect("validated register index exists in the fixed register tree");
        Ok(proof
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect())
    }
    /// Combined helper: return both the typed Merkle root and authentication
    /// path for `idx`. Performs at most one rebuild and borrows the tree once.
    ///
    /// # Errors
    /// Returns [`VMError::RegisterOutOfBounds`] when `idx` is not a register index.
    #[inline]
    pub fn merkle_root_and_path(
        &self,
        idx: usize,
    ) -> Result<(HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>), VMError> {
        let leaf_index = register_leaf_index(idx)?;
        let tree = self.ensure_built_and_lock();
        let root = tree.root().expect("tree has at least one leaf");
        let path = tree
            .get_proof(leaf_index)
            .expect("validated register index exists in the fixed register tree")
            .into_audit_path()
            .into_iter()
            .map(|opt| opt.map(|h| *h.as_ref()).unwrap_or([0u8; 32]))
            .collect();
        Ok((root, path))
    }
    /// Build a compact Merkle proof for the register at `idx`.
    ///
    /// Without truncation the returned root is the full register-tree root.
    /// When `depth_cap` truncates the path, the returned root commits only to
    /// that path fragment and is not a membership commitment.
    ///
    /// # Errors
    /// Returns [`VMError::RegisterOutOfBounds`] when `idx` is not a register index.
    #[inline]
    pub fn merkle_compact(
        &self,
        idx: usize,
        depth_cap: Option<usize>,
    ) -> Result<(CompactMerkleProof<[u8; 32]>, HashOf<MerkleTree<[u8; 32]>>), VMError> {
        let leaf_index = register_leaf_index(idx)?;
        let (root, path) = self.merkle_root_and_path(idx)?;
        let proof = crate::merkle_utils::make_compact_from_path_bytes(&path, leaf_index, depth_cap);
        let leaf_digest = register_leaf_digest(self.gpr[idx], self.tags[idx]);
        let leaf_hash = HashOf::<[u8; 32]>::from_untyped_unchecked(Hash::prehashed(leaf_digest));
        let siblings = proof.siblings().to_vec();
        let merkle_proof = MerkleProof::from_audit_path(proof.dirs(), siblings);
        // A depth cap commits only to this path fragment. It is deliberately
        // not treated as membership in the fixed 256-leaf register tree.
        let adj_root = if usize::from(proof.depth()) < path.len() {
            merkle_proof
                .compute_partial_root_sha256(&leaf_hash, usize::from(proof.depth()))
                .expect("proof height equals compact depth")
        } else {
            root
        };
        Ok((proof, adj_root))
    }
    #[inline]
    fn ensure_built_and_lock(&self) -> parking_lot::MutexGuard<'_, MerkleTree<[u8; 32]>> {
        let mut tree = self.tree.lock();
        if self.dirty.load(Ordering::Acquire) {
            *tree =
                MerkleTree::from_hashed_leaves_sha256(register_leaf_digests(&self.gpr, &self.tags));
            self.dirty.store(false, Ordering::Release);
        }
        tree
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
#[cfg(any(feature = "telemetry", test))]
#[derive(Clone, Copy)]
struct RegisterUsage {
    bitmap: [u64; 4],
    max_index: u16,
}
#[cfg(not(any(feature = "telemetry", test)))]
#[derive(Clone, Copy, Default)]
struct RegisterUsage;
#[cfg(any(feature = "telemetry", test))]
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
#[cfg(not(any(feature = "telemetry", test)))]
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
    #[cfg(test)]
    REGISTER_LEAF_DIGEST_COUNT.with(|count| count.set(count.get() + 1));
    let mut bytes = [0u8; 9];
    bytes[0] = if tag { 1 } else { 0 };
    bytes[1..].copy_from_slice(&value.to_le_bytes());
    Sha256::digest(bytes).into()
}
#[inline]
fn register_leaf_index(idx: usize) -> Result<u32, VMError> {
    if idx >= REGISTER_COUNT {
        return Err(VMError::RegisterOutOfBounds);
    }
    u32::try_from(idx).map_err(|_| VMError::RegisterOutOfBounds)
}
fn register_leaf_digests(gpr: &[u64; 256], tags: &[bool; 256]) -> Vec<[u8; 32]> {
    gpr.iter()
        .zip(tags)
        .map(|(&value, &tag)| register_leaf_digest(value, tag))
        .collect()
}
#[cfg(test)]
thread_local! {
    static REGISTER_LEAF_DIGEST_COUNT: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}
#[cfg(test)]
fn reset_register_leaf_digest_count() {
    REGISTER_LEAF_DIGEST_COUNT.with(|count| count.set(0));
}
#[cfg(test)]
fn register_leaf_digest_count() -> usize {
    REGISTER_LEAF_DIGEST_COUNT.with(std::cell::Cell::get)
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
    #[test]
    fn private_scrub_zeros_tagged_registers_and_preserves_public_values() {
        let mut regs = Registers::new();
        regs.set(2, 0xfeed_face_dead_beef);
        regs.set_tag(2, true);
        regs.set(200, 0x0123_4567_89ab_cdef);
        regs.set_tag(200, true);
        regs.set(7, 0x55aa);

        regs.scrub_private();

        assert_eq!(regs.get(2), 0);
        assert!(!regs.tag(2));
        assert_eq!(regs.get(200), 0);
        assert!(!regs.tag(200));
        assert_eq!(regs.get(7), 0x55aa);
        assert!(!regs.tag(7));
        assert!(!regs.has_private());
    }
    #[test]
    fn register_writes_defer_merkle_hashing_until_the_root_is_read() {
        let mut regs = Registers::new();
        reset_register_leaf_digest_count();
        for value in 1..=1_000_u64 {
            let index = (value as usize % 255) + 1;
            regs.set(index, value);
            regs.set_tag(index, value.is_multiple_of(2));
        }
        assert_eq!(register_leaf_digest_count(), 0);

        let first_root = regs.merkle_root();
        assert_eq!(register_leaf_digest_count(), 256);
        assert_eq!(regs.merkle_root(), first_root);
        assert_eq!(register_leaf_digest_count(), 256);
    }
    #[test]
    fn cloning_reuses_a_clean_tree_and_rebuilds_a_dirty_tree_once() {
        let mut regs = Registers::new();
        reset_register_leaf_digest_count();
        let clean = regs.clone();
        assert_eq!(register_leaf_digest_count(), 0);
        assert_eq!(clean.merkle_root(), regs.merkle_root());

        regs.set(7, 42);
        let dirty = regs.clone();
        assert_eq!(register_leaf_digest_count(), 256);
        assert_eq!(dirty.merkle_root(), regs.merkle_root());
        assert_eq!(register_leaf_digest_count(), 512);
    }
    #[test]
    fn logged_writes_update_only_the_changed_merkle_leaf() {
        let mut regs = Registers::new();
        let log = std::sync::Arc::new(parking_lot::Mutex::new(crate::zk::RegLog::default()));
        reset_register_leaf_digest_count();
        let guard = crate::zk::RegLoggerGuard::install(Some(std::sync::Arc::clone(&log)));

        regs.set(7, 42);
        assert_eq!(register_leaf_digest_count(), 1);
        let after_set = canonical_root_and_path(&regs, 7);

        reset_register_leaf_digest_count();
        regs.set_tag(7, true);
        assert_eq!(register_leaf_digest_count(), 1);
        let after_tag = canonical_root_and_path(&regs, 7);
        drop(guard);

        let log = log.lock();
        assert_eq!(log.events.len(), 2);
        assert_logged_event_matches(&log.events[0], &after_set);
        assert_logged_event_matches(&log.events[1], &after_tag);
        assert_eq!(regs.merkle_root(), after_tag.0);
    }
    #[test]
    fn merkle_proof_apis_reject_out_of_range_indices_without_aliasing() {
        let regs = Registers::new();

        assert!(regs.merkle_path(REGISTER_COUNT - 1).is_ok());
        assert!(regs.merkle_root_and_path(REGISTER_COUNT - 1).is_ok());
        assert!(regs.merkle_compact(REGISTER_COUNT - 1, None).is_ok());

        let mut invalid = vec![REGISTER_COUNT, usize::MAX];
        #[cfg(target_pointer_width = "64")]
        invalid.push((u64::from(u32::MAX) + 1) as usize);

        for idx in invalid {
            assert!(matches!(
                regs.merkle_path(idx),
                Err(VMError::RegisterOutOfBounds)
            ));
            assert!(matches!(
                regs.merkle_root_and_path(idx),
                Err(VMError::RegisterOutOfBounds)
            ));
            assert!(matches!(
                regs.merkle_compact(idx, None),
                Err(VMError::RegisterOutOfBounds)
            ));
            assert!(matches!(
                crate::merkle_utils::registers_compact_bundle(&regs, idx, None),
                Err(VMError::RegisterOutOfBounds)
            ));
        }
    }

    fn canonical_root_and_path(
        regs: &Registers,
        idx: usize,
    ) -> (HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>) {
        let canonical =
            MerkleTree::from_hashed_leaves_sha256(register_leaf_digests(&regs.gpr, &regs.tags));
        let root = canonical.root().expect("non-empty register tree");
        let path = canonical
            .get_proof(idx as u32)
            .expect("valid register index")
            .into_audit_path()
            .into_iter()
            .map(|entry| entry.map(|hash| *hash.as_ref()).unwrap_or([0; 32]))
            .collect::<Vec<_>>();
        (root, path)
    }

    fn assert_logged_event_matches(
        event: &RegEvent,
        expected: &(HashOf<MerkleTree<[u8; 32]>>, Vec<[u8; 32]>),
    ) {
        let (logged_root, logged_path) = match event {
            RegEvent::Read { root, path, .. } | RegEvent::Write { root, path, .. } => (root, path),
        };

        assert_eq!(logged_root, &expected.0);
        assert_eq!(logged_path, &expected.1);
    }
}
