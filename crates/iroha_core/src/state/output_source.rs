//! Borrowed original signed global inputs for the sole economic output producer.

use super::*;
use iroha_data_model::block::{BlockHeader, execution_output::ExecutionInputs};

pub(super) struct ExecutionSource<'source>(pub(super) &'source SignedBlock);

impl<'source> ExecutionSource<'source> {
    pub(super) fn header(&self) -> BlockHeader {
        self.0.header()
    }

    pub(super) fn hash(&self) -> HashOf<BlockHeader> {
        self.header().hash()
    }

    pub(super) fn network_entrypoint_count(&self) -> usize {
        self.0.network_entrypoint_count()
    }

    pub(super) fn network_entrypoint_at(
        &self,
        index: usize,
    ) -> Option<&'source TransactionEntrypoint> {
        self.0.network_entrypoint_at(index)
    }

    pub(super) fn network_entrypoints(
        &self,
    ) -> impl Iterator<Item = &'source TransactionEntrypoint> + '_ {
        (0..self.network_entrypoint_count()).map(|index| {
            self.network_entrypoint_at(index)
                .expect("immutable source count and indexed projection agree")
        })
    }

    pub(super) fn input_root(&self) -> Option<Hash> {
        MerkleTree::root_from_typed_leaves(
            self.network_entrypoints().map(TransactionEntrypoint::hash),
        )
        .map(Hash::from)
    }
}

impl ExecutionInputs for ExecutionSource<'_> {
    fn input_count(&self) -> usize {
        self.network_entrypoint_count()
    }

    fn input_at(&self, index: usize) -> Option<&TransactionEntrypoint> {
        self.network_entrypoint_at(index)
    }
}
