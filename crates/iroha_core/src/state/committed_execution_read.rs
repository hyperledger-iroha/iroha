//! Bounded local history reads authenticated by the original native State tip.

use super::*;
use iroha_data_model::query::error::QueryExecutionFail;

impl State {
    /// Read original committed execution from a consistent native State history cut.
    ///
    /// Only the hash journal and opaque execution tip are retained across I/O. The
    /// reverse ancestry walk authenticates complete execution without depending on
    /// a node's particular QC subset or scanning from genesis for a recent retry.
    /// Every source frame is admitted before I/O; target input/output work also
    /// counts. These finite read limits are not resident-graph allocation reservations.
    ///
    /// # Errors
    /// Refuses zero/exceeded limits, missing native authority, unavailable or corrupt
    /// execution ancestry, and a replaced committed journal binding.
    pub fn read_committed_execution(
        &self,
        height: NonZeroUsize,
        max_work: u64,
        max_bytes: u64,
    ) -> Result<crate::sumeragi::certified_chain::CommittedBlock, QueryExecutionFail> {
        if max_work == 0 || max_bytes == 0 {
            return Err(QueryExecutionFail::GasBudgetExceeded);
        }
        let (hashes, tip) = loop {
            let generation = self.state_view_generation();
            if generation % 2 != 0 {
                std::thread::yield_now();
                continue;
            }
            let hashes = self.block_hashes.view();
            let tip = *self.native_execution_tip.view().get();
            if self.state_view_generation() == generation {
                break (hashes, tip);
            }
        };
        let expected = hashes.get(height.get() - 1).copied().ok_or_else(|| {
            QueryExecutionFail::Conversion("committed execution height is unavailable".into())
        })?;
        let source = CanonicalHistorySource::new(&self.kura, &hashes, tip);
        let mut work_left = max_work;
        let mut bytes_left = max_bytes;
        let receipt = source
            .executed_receipt(height, |work, bytes| {
                work_left = work_left
                    .checked_sub(work)
                    .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                bytes_left = bytes_left
                    .checked_sub(bytes)
                    .ok_or(QueryExecutionFail::GasBudgetExceeded)?;
                Ok(())
            })
            .map_err(crate::smartcontracts::isi::query::query_transport_error)?;
        let target_work = u64::try_from(
            receipt
                .block()
                .network_entrypoint_count()
                .max(receipt.block().execution_outputs().len())
                .max(1),
        )
        .map_err(|_| QueryExecutionFail::GasBudgetExceeded)?;
        if target_work > work_left {
            return Err(QueryExecutionFail::GasBudgetExceeded);
        }
        if self.block_hashes.view().get(height.get() - 1).copied() != Some(expected) {
            return Err(QueryExecutionFail::Conversion(
                "committed execution changed during authentication".into(),
            ));
        }
        Ok(receipt)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sumeragi::test_chain::{CertifiedTestChain, TestChainConfig};

    #[test]
    fn committed_execution_read_uses_recent_native_tip_under_exact_limits() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        for _ in 0..7 {
            chain.commit(Vec::new());
        }
        let original = chain.committed(8);
        let wire = original.block().encode_wire().unwrap();
        let work = u64::try_from(
            original
                .block()
                .network_entrypoint_count()
                .max(original.block().execution_outputs().len())
                .max(1),
        )
        .unwrap()
            + 1;
        assert!(
            work < chain.height(),
            "budget cannot authenticate a prefix from genesis"
        );
        let height = NonZeroUsize::new(8).unwrap();
        let bytes = u64::try_from(wire.len()).unwrap();
        let receipt = chain
            .state()
            .read_committed_execution(height, work, bytes)
            .unwrap();
        assert_eq!(receipt.block().encode_wire().unwrap(), wire);
        for (work, bytes) in [(work - 1, bytes), (work, bytes - 1), (0, bytes), (work, 0)] {
            assert!(matches!(
                chain.state().read_committed_execution(height, work, bytes),
                Err(QueryExecutionFail::GasBudgetExceeded)
            ));
        }
    }

    #[test]
    fn committed_execution_read_charges_every_reverse_ancestor_and_refuses_corruption() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        for _ in 0..3 {
            chain.commit(Vec::new());
        }
        let height = NonZeroUsize::new(2).unwrap();
        let bytes = (2..=4)
            .map(|height| {
                u64::try_from(chain.committed(height).block().encode_wire().unwrap().len()).unwrap()
            })
            .sum::<u64>();
        let block = chain.committed(2);
        let work = u64::try_from(
            block
                .block()
                .network_entrypoint_count()
                .max(block.block().execution_outputs().len())
                .max(1),
        )
        .unwrap()
            + 3;
        assert!(
            chain
                .state()
                .read_committed_execution(height, work, bytes)
                .is_ok()
        );
        for (work, bytes) in [(work - 1, bytes), (work, bytes - 1)] {
            assert!(matches!(
                chain.state().read_committed_execution(height, work, bytes),
                Err(QueryExecutionFail::GasBudgetExceeded)
            ));
        }
        chain
            .kura()
            .corrupt_canonical_body_for_testing(NonZeroUsize::new(3).unwrap())
            .unwrap();
        assert!(
            chain
                .state()
                .read_committed_execution(height, work, bytes)
                .is_err()
        );
    }

    #[test]
    fn committed_execution_read_hash_journal_cannot_replace_original_tip_authority() {
        let mut chain =
            CertifiedTestChain::start(TestChainConfig::new(World::new(), 1_000)).unwrap();
        chain.commit(Vec::new());
        let kura = Kura::blank_kura_for_testing();
        let mut state = State::new_with_chain_and_network_id_for_testing(
            World::new(),
            kura.clone(),
            crate::query::store::LiveQueryStore::start_test(),
            "sumeragi-certified-test-chain".parse().unwrap(),
            chain.network_id(),
        );
        for height in 1..=2 {
            let block = chain.committed(height).block().clone();
            kura.store_block(block.clone()).unwrap();
            state.push_block_hash_for_testing(block.hash());
        }
        assert!(
            state
                .read_committed_execution(NonZeroUsize::new(2).unwrap(), 100, 8 * 1024 * 1024)
                .is_err()
        );
    }
}
