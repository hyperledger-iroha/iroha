//! Implementations for transaction queries.

use eyre::Result;
use iroha_data_model::{
    prelude::*,
    query::{dsl::CompoundPredicate, error::QueryExecutionFail, CommittedTransaction},
};
use iroha_telemetry::metrics;
use nonzero_ext::nonzero;

use super::*;
use crate::smartcontracts::ValidQuery;

impl ValidQuery for FindTransactions {
    #[metrics(+"find_transactions")]
    fn execute(
        self,
        filter: CompoundPredicate<CommittedTransaction>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        Ok(state_ro
            .all_blocks(nonzero!(1_usize))
            // Iterate over blocks in descending order (most recent first).
            .rev()
            .flat_map(|block| {
                let block_hash = block.hash();

                // Iterate over transactions in descending order (most recent first).
                let entrypoint_hashes = block.entrypoint_hashes().rev();
                let entrypoints = block.entrypoints_cloned().rev();
                let result_hashes = block.result_hashes().rev();
                let results = block.results().cloned().rev();

                entrypoint_hashes
                    .zip(entrypoints)
                    .zip(result_hashes)
                    .zip(results)
                    .map(|(((entrypoint_hash, entrypoint), result_hash), result)| {
                        CommittedTransaction {
                            block_hash,
                            entrypoint_hash,
                            entrypoint,
                            result_hash,
                            result,
                        }
                    })
                    .collect::<Vec<_>>()
            })
            .filter(move |tx| filter.applies(tx)))
    }
}

#[cfg(test)]
mod tests {
    use iroha_data_model::prelude::{TransactionEntrypoint, TransactionResult};

    use crate::tx::tests::*;

    /// Verifies that all per-field iterators over a committed block are consistent.
    #[tokio::test]
    async fn block_iterators_are_consistent() {
        let mut sandbox = Sandbox::default()
            .with_data_trigger_transfer("bob", 40, "carol")
            .with_time_trigger_transfer_labeled("alice", 1, "alice", 0)
            .with_time_trigger_transfer_labeled("alice", 1, "alice", 1)
            .with_time_trigger_transfer_labeled("alice", 1, "alice", 2)
            .with_time_trigger_transfer("carol", 30, "dave")
            .with_data_trigger_transfer("dave", 20, "eve");
        sandbox.request_transfer("alice", 50, "bob");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        sandbox.request_transfer("eve", 1, "eve");
        let mut block = sandbox.block();
        block.assert_balances([
            ("alice", 60),
            ("bob", 10),
            ("carol", 10),
            ("dave", 10),
            ("eve", 10),
        ]);
        let (_events, committed_block) = block.apply();
        block.assert_balances([
            ("alice", 10),
            ("bob", 20),
            ("carol", 20),
            ("dave", 20),
            ("eve", 30),
        ]);
        let block = committed_block.as_ref();

        // All entrypoint-related iterators yield the same number of elements.
        assert_eq!(10, block.entrypoint_hashes().len());
        assert_eq!(10, block.entrypoints_cloned().len());
        assert_eq!(10, block.result_hashes().len());
        assert_eq!(10, block.results().len());
        assert_eq!(6, block.external_transactions().len());
        assert_eq!(4, block.time_triggers().len());

        // Hashes of entrypoints and results match their respective contents.
        assert_eq!(
            block.entrypoint_hashes().collect::<Vec<_>>(),
            block
                .entrypoints_cloned()
                .map(|e| e.hash())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            block.result_hashes().collect::<Vec<_>>(),
            block
                .results()
                .map(TransactionResult::hash)
                .collect::<Vec<_>>()
        );

        // External and time-triggered entrypoints are merged correctly into a unified view.
        assert_eq!(
            block.entrypoints_cloned().collect::<Vec<_>>(),
            block
                .external_transactions()
                .cloned()
                .map(TransactionEntrypoint::from)
                .chain(
                    block
                        .time_triggers()
                        .cloned()
                        .map(TransactionEntrypoint::from)
                )
                .collect::<Vec<_>>()
        );

        // The order and content of the first and last transactions are as expected.
        assert!(block
            .entrypoints_cloned()
            .next()
            .map(|e| format!("{e:?}"))
            .unwrap()
            .contains("Numeric { inner: 50 }"));
        assert!(block
            .results()
            .next()
            .map(|e| format!("{e:?}"))
            .unwrap()
            .contains("data-bob-carol-0"));

        assert!(block
            .entrypoints_cloned()
            .nth(9)
            .map(|e| format!("{e:?}"))
            .unwrap()
            .contains("time-carol-dave-0"));
        assert!(block
            .results()
            .nth(9)
            .map(|e| format!("{e:?}"))
            .unwrap()
            .contains("data-dave-eve-0"));
    }
}
