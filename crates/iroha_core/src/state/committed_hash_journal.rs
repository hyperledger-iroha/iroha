//! Allocation-conscious accessors for the committed block-hash journal.
use super::State;
use iroha_crypto::HashOf;
use iroha_data_model::block::BlockHeader;
use std::num::NonZeroUsize;
impl State {
    /// Snapshot committed block hashes from the block-hash journal.
    ///
    /// This is cheaper than acquiring a full [`StateView`](super::StateView) when only the
    /// committed hash sequence is required.
    #[track_caller]
    pub fn committed_block_hashes_snapshot(&self) -> Vec<HashOf<BlockHeader>> {
        self.block_hashes.view().iter().copied().collect()
    }
    /// Compare an expected committed hash sequence without cloning the journal.
    #[track_caller]
    pub fn committed_block_hashes_match(&self, expected: &[HashOf<BlockHeader>]) -> bool {
        let hashes = self.block_hashes.view();
        hashes.len() == expected.len() && hashes.iter().copied().eq(expected.iter().copied())
    }
    /// Visit committed hashes in one-based height order without cloning the journal.
    ///
    /// # Errors
    ///
    /// Returns the first error produced by `visit`.
    #[track_caller]
    pub fn try_for_each_committed_block_hash<E>(
        &self,
        mut visit: impl FnMut(NonZeroUsize, HashOf<BlockHeader>) -> core::result::Result<(), E>,
    ) -> core::result::Result<(), E> {
        let hashes = self.block_hashes.view();
        for (index, hash) in hashes.iter().copied().enumerate() {
            let height = NonZeroUsize::new(index.saturating_add(1))
                .expect("enumerated block height is non-zero");
            visit(height, hash)?;
        }
        Ok(())
    }
    /// Return the committed block hash at one-based `height` without cloning
    /// the complete block-hash journal.
    #[track_caller]
    pub fn committed_block_hash_at_height(&self, height: u64) -> Option<HashOf<BlockHeader>> {
        let index = usize::try_from(height).ok()?.checked_sub(1)?;
        self.block_hashes.view().get(index).copied()
    }
    /// Resolve a one-based committed height from the in-memory hash journal.
    ///
    /// The lookup scans the retained journal without cloning it. Callers should
    /// prefer [`Self::block_height_by_hash`] when Kura's durable index is
    /// available and use this only for hash-only recovery projections.
    #[track_caller]
    pub fn committed_block_height_for_hash(
        &self,
        hash: HashOf<BlockHeader>,
    ) -> Option<NonZeroUsize> {
        self.block_hashes
            .view()
            .iter()
            .position(|candidate| *candidate == hash)
            .and_then(|index| NonZeroUsize::new(index.saturating_add(1)))
    }
}
