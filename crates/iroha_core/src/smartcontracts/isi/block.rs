//! This module contains trait implementations related to block queries
use eyre::Result;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    query::{dsl::CompoundPredicate, error::QueryExecutionFail},
};
use iroha_telemetry::metrics;
use nonzero_ext::nonzero;

use super::*;
use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

impl ValidQuery for FindBlocks {
    #[metrics(+"find_blocks")]
    fn execute(
        self,
        filter: CompoundPredicate<SignedBlock>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        Ok(state_ro
            .all_blocks(nonzero!(1_usize))
            .rev()
            .filter(move |block| filter.applies(block))
            .map(|block| (*block).clone()))
    }
}

impl ValidQuery for FindBlockHeaders {
    #[metrics(+"find_block_headers")]
    fn execute(
        self,
        filter: CompoundPredicate<BlockHeader>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        Ok(state_ro
            .all_blocks(nonzero!(1_usize))
            .rev()
            .filter(move |block| filter.applies(&block.header()))
            .map(|block| block.header()))
    }
}
