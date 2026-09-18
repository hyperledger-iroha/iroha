//! Exact ordinary or constructor-owned native inputs for the common producer.
//!
//! Native execution consumes a preflight capability; it never fabricates a
//! signed block to reuse the economic executor.

use super::*;
use crate::state::lane_decision_execution::NativeLaneAfterStartV1;
use iroha_data_model::block::{BlockHeader, execution_output::ExecutionInputs};

pub(super) enum ExecutionSource<'source> {
    Ordinary(&'source SignedBlock),
    Native {
        header: BlockHeader,
        groups: &'source [crate::state::VerifiedLaneDecisionGroupV1],
    },
}

impl<'source> ExecutionSource<'source> {
    pub(super) fn native(preflight: NativeLaneAfterStartV1<'source>) -> Self {
        let (header, groups) = preflight.into_source();
        Self::Native { header, groups }
    }

    pub(super) fn header(&self) -> BlockHeader {
        match self {
            Self::Ordinary(block) => block.header(),
            Self::Native { header, .. } => header.clone(),
        }
    }

    pub(super) fn hash(&self) -> HashOf<BlockHeader> {
        self.header().hash()
    }

    pub(super) fn is_native(&self) -> bool {
        matches!(self, Self::Native { .. })
    }

    pub(super) fn network_entrypoint_count(&self) -> usize {
        match self {
            Self::Ordinary(block) => block.network_entrypoint_count(),
            Self::Native { groups, .. } => groups.len(),
        }
    }

    pub(super) fn network_entrypoint_at(
        &self,
        index: usize,
    ) -> Option<&'source TransactionEntrypoint> {
        match self {
            Self::Ordinary(block) => block.network_entrypoint_at(index),
            Self::Native { groups, .. } => groups
                .get(index)
                .map(|group| &group.body().payload().input.entrypoint),
        }
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
