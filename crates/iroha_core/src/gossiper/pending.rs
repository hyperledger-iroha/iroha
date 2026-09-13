//! One bounded active gossip retained across a typed State publication wait.
use super::{Arc, RetainedGossip, TRANSACTION_GOSSIP_MAX_SIZE, TransactionGossip};

// The same mandatory count bound controls raw decoding and typed admission.
const WORDS: usize = (TRANSACTION_GOSSIP_MAX_SIZE.get() as usize).div_ceil(64);

/// Completed entries never re-enter validation or queue admission on a retry.
pub(super) struct GossipProgress {
    complete: [u64; WORDS],
    wait_height: Option<u64>,
}
impl Default for GossipProgress {
    fn default() -> Self {
        Self {
            complete: [0; WORDS],
            wait_height: None,
        }
    }
}
impl GossipProgress {
    pub(super) fn begin_pass(&mut self) {
        self.wait_height = None;
    }
    pub(super) fn is_complete(&self, index: usize) -> bool {
        self.complete[index / 64] & (1 << (index % 64)) != 0
    }
    pub(super) fn complete(&mut self, index: usize) {
        self.complete[index / 64] |= 1 << (index % 64);
    }
    pub(super) fn defer(&mut self, index: usize, height: u64) {
        self.complete[index / 64] &= !(1 << (index % 64));
        // The first published dependency may release some entries even when a
        // different entry needs a later frontier. Each is reclassified separately.
        self.wait_height = Some(self.wait_height.map_or(height, |old| old.min(height)));
    }
    pub(super) fn wait_height(&self) -> Option<u64> {
        self.wait_height
    }
}

/// The original active-message slot, never an additional queue or credit owner.
pub(super) struct PendingGossip {
    pub(super) message: RetainedGossip<Arc<TransactionGossip>>,
    pub(super) progress: GossipProgress,
    /// First-receipt monotonic deadline, bounded by the configured queue TTL.
    pub(super) deadline: tokio::time::Instant,
    pub(super) required_height: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn canonical_batch_bitmap_preserves_terminal_entries_and_earliest_wait() {
        let limit = TRANSACTION_GOSSIP_MAX_SIZE.get() as usize;
        assert_eq!(WORDS * 64, limit.next_multiple_of(64));
        let mut progress = GossipProgress::default();
        for index in 0..limit {
            progress.complete(index);
        }
        progress.defer(0, 8);
        progress.defer(limit - 1, 4);
        assert_eq!(progress.wait_height(), Some(4));
        assert!(!progress.is_complete(0));
        assert!(!progress.is_complete(limit - 1));
        for index in 1..limit - 1 {
            assert!(progress.is_complete(index));
        }
        progress.begin_pass();
        assert_eq!(progress.wait_height(), None);
        for index in 1..limit - 1 {
            assert!(progress.is_complete(index));
        }
        progress.complete(0);
        progress.defer(limit - 1, 9);
        assert!(progress.is_complete(0));
        assert_eq!(progress.wait_height(), Some(9));
    }
}
