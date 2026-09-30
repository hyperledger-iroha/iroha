//! First-refusal custody outside deterministic instruction and receipt encodings.

use super::{StateBlock, StateStorageAdmissionError, StateTransaction};

impl StateTransaction<'_, '_> {
    /// Retain the first original local refusal even if contract code handles its inner error.
    pub(crate) fn arm_local_storage_refusal(&mut self, error: StateStorageAdmissionError) {
        if self.local_storage_refusal.is_none() {
            *self.local_storage_refusal = Some(error);
        }
    }

    /// Abort local processing before accounting, receipts, callbacks or successful apply.
    pub(crate) fn require_storage_admission(&self) -> Result<(), StateStorageAdmissionError> {
        match self.local_storage_refusal.as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

impl StateBlock<'_> {
    /// Borrow the original refusal through child discard, without taking or clearing it.
    pub(crate) fn require_storage_admission(&self) -> Result<(), StateStorageAdmissionError> {
        match self.local_storage_refusal.as_ref() {
            Some(error) => Err(error.clone()),
            None => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        kura::Kura,
        query::store::LiveQueryStore,
        state::{State, TransactionsBlockError, World},
    };
    use iroha_allocation::AllocationRefusal;
    use iroha_data_model::block::BlockHeader;
    use std::num::NonZeroU64;

    #[test]
    fn caught_fixed_index_allocation_refusal_cannot_commit_a_world_overlay() {
        let state = State::new_for_testing(
            World::default(),
            Kura::blank_kura_for_testing(),
            LiveQueryStore::start_test(),
        );
        let budget = state.world.operation_index_budget().clone();
        let header = BlockHeader::new(NonZeroU64::MIN, None, None, 1, 0);
        let mut block = state.try_block(header).unwrap();
        let mut transaction = block.try_transaction().unwrap();
        let occupied = budget
            .try_reserve_bytes(budget.limit_bytes() - budget.reserved_bytes())
            .unwrap();
        let (_, original) = transaction
            .world
            .kagemusha_mint_credit_operations
            .try_insert_admitted([9; 32], [8; 32])
            .unwrap_err();
        assert!(matches!(
            &original,
            mv::storage::AdmittedStorageError::Allocation(AllocationRefusal::Capacity { .. })
        ));
        let expected = StateStorageAdmissionError::World(original);
        transaction.arm_local_storage_refusal(expected.clone());
        assert_eq!(
            transaction.require_storage_admission(),
            Err(expected.clone())
        );
        transaction.apply();
        drop(occupied);
        assert_eq!(block.require_storage_admission(), Err(expected));
        assert!(matches!(
            block.commit_world_overlay_for_testing(),
            Err(TransactionsBlockError::LocalStateStorage(
                StateStorageAdmissionError::World(_)
            ))
        ));
        assert!(
            state
                .world
                .kagemusha_mint_credit_operations
                .view()
                .get(&[9; 32])
                .is_none()
        );
    }
}
