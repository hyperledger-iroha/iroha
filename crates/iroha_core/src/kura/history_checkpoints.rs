//! Node-local checkpoints of authenticated native execution identities.
//!
//! Reading the execution of a committed block requires its native identity:
//! the Iroha header hash, the native core header hash and the certified result
//! `R`. The State keeps only the tip's identity, so a reader authenticates
//! older blocks by walking parent links down from the tip, paying for every
//! newer block. Checkpoints let off-chain readers (Torii collections and the
//! explorer) start a descending walk just above their target instead:
//!
//! * *sparse* checkpoints keep every [`HISTORY_CHECKPOINT_INTERVAL`]-th height,
//!   so a cold read pays for at most that many extra blocks;
//! * *recent* entries keep the last [`RECENT_CAPACITY`] heights that walks
//!   verified, so a reader paging downward starts one block above its target.
//!
//! An identity is recorded only after it was authenticated: by a verified
//! descending walk, by the startup verification of the native prefix, or when
//! an authorized execution advances the tip. A reader trusts an entry only
//! while its Iroha hash equals the reader's committed hash journal at that
//! height, so an entry left by an abandoned block is ignored and dropped.
//! Checkpoints are never persisted, decoded or consulted by consensus:
//! on-chain readers keep walking from the tip, so metered work never depends
//! on node-local state.
use std::collections::{BTreeMap, VecDeque};

use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;
use iroha_sumeragi::types::Hash32;
use parking_lot::RwLock;

/// Heights between consecutive sparse checkpoints; also the most extra blocks
/// a cold off-chain read walks to reach its target.
pub const HISTORY_CHECKPOINT_INTERVAL: u64 = 64;

/// Recently verified heights kept for readers paging through history.
pub const RECENT_CAPACITY: usize = 4096;

/// Entries examined per lookup before falling back to the tip.
const LOOKUP_CANDIDATES: usize = 4;

/// The authenticated native identity of one committed height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryCheckpoint {
    /// Exact Iroha header identity of the block.
    pub iroha_hash: HashOf<BlockHeader>,
    /// Exact native core header identity.
    pub core_hash: Hash32,
    /// Exact certified execution result.
    pub result: Hash32,
}

/// Sparse and recent identities by height.
///
/// Memory grows by one sparse entry per [`HISTORY_CHECKPOINT_INTERVAL`]
/// committed blocks (about 1.6 MiB per million blocks) plus at most
/// [`RECENT_CAPACITY`] recent entries.
#[derive(Debug, Default)]
pub struct HistoryCheckpoints {
    inner: RwLock<Entries>,
}

#[derive(Debug, Default)]
struct Entries {
    sparse: BTreeMap<u64, HistoryCheckpoint>,
    recent: BTreeMap<u64, HistoryCheckpoint>,
    /// Recent heights in insertion order, for eviction.
    recent_order: VecDeque<u64>,
}

impl HistoryCheckpoints {
    /// Whether `height` keeps a sparse checkpoint.
    #[must_use]
    pub const fn is_checkpoint_height(height: u64) -> bool {
        height != 0 && height % HISTORY_CHECKPOINT_INTERVAL == 0
    }

    /// Record an identity a reader just verified: as a recent entry, and as a
    /// sparse checkpoint when `height` is a checkpoint height.
    pub fn record(&self, height: u64, checkpoint: HistoryCheckpoint) {
        let mut entries = self.inner.write();
        if Self::is_checkpoint_height(height) {
            entries.sparse.insert(height, checkpoint);
        }
        if entries.recent.insert(height, checkpoint).is_none() {
            entries.recent_order.push_back(height);
            if entries.recent_order.len() > RECENT_CAPACITY
                && let Some(evicted) = entries.recent_order.pop_front()
            {
                entries.recent.remove(&evicted);
            }
        }
    }

    /// Record an authenticated identity as a sparse checkpoint only (startup
    /// verification and tip advances, which would otherwise churn the recent
    /// entries).
    pub fn record_sparse(&self, height: u64, checkpoint: HistoryCheckpoint) {
        if !Self::is_checkpoint_height(height) {
            return;
        }
        let mut entries = self.inner.write();
        if entries.sparse.get(&height) != Some(&checkpoint) {
            entries.sparse.insert(height, checkpoint);
        }
    }

    /// The nearest entries at or above `target` and at or below `ceiling`,
    /// nearest first; callers verify each against their hash journal.
    #[must_use]
    pub fn candidates(&self, target: u64, ceiling: u64) -> Vec<(u64, HistoryCheckpoint)> {
        if target > ceiling {
            return Vec::new();
        }
        let entries = self.inner.read();
        let mut found: Vec<(u64, HistoryCheckpoint)> = entries
            .recent
            .range(target..=ceiling)
            .take(LOOKUP_CANDIDATES)
            .chain(
                entries
                    .sparse
                    .range(target..=ceiling)
                    .take(LOOKUP_CANDIDATES),
            )
            .map(|(height, checkpoint)| (*height, *checkpoint))
            .collect();
        found.sort_by_key(|(height, _)| *height);
        found.dedup();
        found.truncate(LOOKUP_CANDIDATES);
        found
    }

    /// Drop `checkpoint` at `height` after it contradicted a hash journal.
    pub fn forget(&self, height: u64, checkpoint: &HistoryCheckpoint) {
        let mut entries = self.inner.write();
        if entries.sparse.get(&height) == Some(checkpoint) {
            entries.sparse.remove(&height);
        }
        if entries.recent.get(&height) == Some(checkpoint) {
            entries.recent.remove(&height);
            entries.recent_order.retain(|recent| *recent != height);
        }
    }

    /// Number of sparse checkpoints.
    #[cfg(test)]
    #[must_use]
    pub fn sparse_len(&self) -> usize {
        self.inner.read().sparse.len()
    }

    /// Number of recent entries.
    #[cfg(test)]
    #[must_use]
    pub fn recent_len(&self) -> usize {
        self.inner.read().recent.len()
    }

    /// Forget every entry, as after a restart without the startup walk.
    #[cfg(test)]
    pub fn clear(&self) {
        *self.inner.write() = Entries::default();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use iroha_crypto::Hash;

    fn checkpoint(seed: u64) -> HistoryCheckpoint {
        let byte = u8::try_from(seed % 251).expect("fits");
        HistoryCheckpoint {
            iroha_hash: HashOf::from_untyped_unchecked(Hash::prehashed([byte; 32])),
            core_hash: Hash32([byte; 32]),
            result: Hash32([byte.wrapping_add(1); 32]),
        }
    }

    #[test]
    fn sparse_records_keep_only_checkpoint_heights() {
        let checkpoints = HistoryCheckpoints::default();
        checkpoints.record_sparse(1, checkpoint(1));
        checkpoints.record_sparse(HISTORY_CHECKPOINT_INTERVAL - 1, checkpoint(2));
        assert_eq!(checkpoints.sparse_len(), 0);
        checkpoints.record_sparse(HISTORY_CHECKPOINT_INTERVAL, checkpoint(3));
        checkpoints.record(2 * HISTORY_CHECKPOINT_INTERVAL, checkpoint(4));
        assert_eq!(checkpoints.sparse_len(), 2);
        assert_eq!(checkpoints.recent_len(), 1, "only walk records are recent");
    }

    #[test]
    fn candidates_are_the_nearest_at_or_above_the_target() {
        let checkpoints = HistoryCheckpoints::default();
        let interval = HISTORY_CHECKPOINT_INTERVAL;
        for step in 1..=8 {
            checkpoints.record_sparse(step * interval, checkpoint(step));
        }
        checkpoints.record(interval + 3, checkpoint(99));
        let found = checkpoints.candidates(interval + 1, 8 * interval);
        assert_eq!(found.len(), LOOKUP_CANDIDATES);
        assert_eq!(
            found[0],
            (interval + 3, checkpoint(99)),
            "a recent entry is nearest"
        );
        assert_eq!(found[1].0, 2 * interval);
        assert!(
            checkpoints
                .candidates(7 * interval + 1, 7 * interval)
                .is_empty()
        );
        assert_eq!(
            checkpoints.candidates(interval, 3 * interval)[0].0,
            interval,
            "an entry at the target itself is the nearest"
        );
    }

    #[test]
    fn recent_entries_are_bounded_and_evicted_oldest_first() {
        let checkpoints = HistoryCheckpoints::default();
        let first = 1_000_001;
        for height in first..first + RECENT_CAPACITY as u64 + 10 {
            checkpoints.record(height, checkpoint(height));
        }
        assert_eq!(checkpoints.recent_len(), RECENT_CAPACITY);
        assert!(
            checkpoints
                .candidates(first, first + 9)
                .iter()
                .all(|(height, _)| HistoryCheckpoints::is_checkpoint_height(*height)),
            "the oldest recent entries were evicted"
        );
        assert_eq!(
            checkpoints.candidates(first + 10, first + 10)[0].0,
            first + 10
        );
    }

    #[test]
    fn contradicted_entries_are_replaced_or_forgotten() {
        let checkpoints = HistoryCheckpoints::default();
        let height = HISTORY_CHECKPOINT_INTERVAL;
        checkpoints.record(height, checkpoint(1));
        checkpoints.record(height, checkpoint(2));
        assert_eq!(checkpoints.candidates(1, height)[0].1, checkpoint(2));
        checkpoints.forget(height, &checkpoint(1));
        assert_eq!(
            checkpoints.sparse_len(),
            1,
            "only the exact stale entry is dropped"
        );
        checkpoints.forget(height, &checkpoint(2));
        assert_eq!(checkpoints.sparse_len(), 0);
        assert_eq!(checkpoints.recent_len(), 0);
    }
}
