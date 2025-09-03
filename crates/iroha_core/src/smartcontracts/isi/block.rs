//! This module contains trait implementations related to block queries

use std::sync::Arc;

use eyre::Result;
use iroha_data_model::{
    block::{BlockHeader, SignedBlock},
    query::{dsl::CompoundPredicate, error::QueryExecutionFail},
};
use iroha_telemetry::metrics;
use nonzero_ext::nonzero;

use super::*;
use crate::{smartcontracts::ValidQuery, state::StateReadOnly};

fn iter_blocks(
    state: &impl StateReadOnly,
    order: Order,
) -> impl Iterator<Item = Arc<SignedBlock>> + '_ {
    let iter = state.all_blocks(nonzero!(1usize));

    let iter: Box<dyn Iterator<Item = Arc<SignedBlock>>> = match order {
        Order::Ascending => Box::new(iter),
        Order::Descending => Box::new(iter.rev()),
    };

    iter
}

impl ValidQuery for FindBlocks {
    #[metrics(+"find_blocks")]
    fn execute(
        self,
        filter: CompoundPredicate<SignedBlock>,
        state_ro: &impl StateReadOnly,
    ) -> Result<impl Iterator<Item = Self::Item>, QueryExecutionFail> {
        Ok(iter_blocks(state_ro, self.order)
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
        Ok(iter_blocks(state_ro, self.order)
            .filter(move |block| filter.applies(&block.header()))
            .map(|block| block.header()))
    }
}
