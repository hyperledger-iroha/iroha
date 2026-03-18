use crate::Value;

/// Multi-version storage for single value
pub struct Cell<V: Value> {
    /// Previous version of value, required to perform revert of the latest changes
    pub(crate) revert: EbrCell<Option<V>>,
    /// Value which represent aggregated changes of multiple blocks
    pub(crate) blocks: EbrCell<V>,
}

impl<V: Value> Cell<V> {
    /// Construct new [`Self`]
    pub fn new(v: V) -> Self {
        Self {
            revert: EbrCell::new(None),
            blocks: EbrCell::new(v),
        }
    }

    /// Create persistent view of storage at certain point in time
    pub fn view(&self) -> View<'_, V> {
        View {
            blocks: self.blocks.read(),
            _marker: core::marker::PhantomData,
        }
    }

    /// Create block to aggregate updates
    pub fn block(&self) -> Block<'_, V> {
        let mut revert = self.revert.write();
        let blocks = self.blocks.write();

        *revert.get_mut() = None;

        Block { revert, blocks }
    }

    /// Create block to aggregate updates and revert changes made in latest block
    pub fn block_and_revert(&self) -> Block<'_, V> {
        let mut revert = self.revert.write();
        let mut blocks = self.blocks.write();

        {
            let revert = core::mem::take(revert.get_mut());
            if let Some(revert) = revert {
                *blocks.get_mut() = revert;
            }
        }

        Block { revert, blocks }
    }
}

impl<V: Value + Default> Default for Cell<V> {
    fn default() -> Self {
        Self::new(V::default())
    }
}

/// Module for [`View`] and it's related impls
mod view {
    use std::ops::Deref;

    use concread::ebrcell::EbrCellReadTxn;

    use super::*;
    /// Consistent view of the storage at the certain version
    pub struct View<'storage, V: Value> {
        pub(crate) blocks: EbrCellReadTxn<V>,
        pub(crate) _marker: core::marker::PhantomData<&'storage V>,
    }

    impl<V: Value> View<'_, V> {
        /// Read entry from the list up to certain version non-inclusive
        pub fn get(&self) -> &V {
            &self.blocks
        }
    }

    impl<V: Value> Deref for View<'_, V> {
        type Target = V;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }
}
use concread::EbrCell;
pub use view::View;

/// Module for [`Block`] and it's related impls
mod block {
    use std::ops::{Deref, DerefMut};

    use concread::ebrcell::EbrCellWriteTxn;

    use super::*;

    /// Batched update to the storage that can be reverted later
    pub struct Block<'storage, V: Value> {
        pub(crate) revert: EbrCellWriteTxn<'storage, Option<V>>,
        pub(crate) blocks: EbrCellWriteTxn<'storage, V>,
    }

    impl<'storage, V: Value> Block<'storage, V> {
        /// Create transaction for the block
        pub fn transaction<'block>(&'block mut self) -> Transaction<'block, 'storage, V>
        where
            'storage: 'block,
        {
            Transaction {
                block: self,
                revert: None,
            }
        }

        /// Apply aggregated changes to the storage
        pub fn commit(self) {
            // Commit fields in the inverse order
            self.blocks.commit();
            self.revert.commit();
        }

        /// Get mutable access to the value stored in
        pub fn get_mut(&mut self) -> &mut V {
            let value = self.blocks.get_mut();
            self.revert.get_or_insert(value.clone());
            value
        }

        /// Read entry from the storage up to certain version non-inclusive
        pub fn get(&self) -> &V {
            &self.blocks
        }
    }

    impl<V: Value> Deref for Block<'_, V> {
        type Target = V;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }

    impl<V: Value> DerefMut for Block<'_, V> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.get_mut()
        }
    }

    /// Part of block's aggregated changes which applied or aborted at the same time
    pub struct Transaction<'block, 'storage, V: Value> {
        pub(crate) revert: Option<V>,
        pub(crate) block: &'block mut Block<'storage, V>,
    }

    impl<'block, 'storage: 'block, V: Value> Transaction<'block, 'storage, V> {
        /// Apply aggregated changes of [`Transaction`] to the [`Block`]
        pub fn apply(mut self) {
            if let Some(prev_value) = core::mem::take(&mut self.revert) {
                self.block.revert.get_or_insert(prev_value);
            }
        }

        /// Get mutable access to the value stored in cell
        pub fn get_mut(&mut self) -> &mut V {
            let value = self.block.blocks.get_mut();
            self.revert.get_or_insert(value.clone());
            value
        }

        /// Read entry from the cell
        pub fn get(&self) -> &V {
            &self.block.blocks
        }
    }

    impl<'block, 'store: 'block, V: Value> Drop for Transaction<'block, 'store, V> {
        fn drop(&mut self) {
            // revert changes made so fur by current transaction
            // if transaction was applied set would be empty
            if let Some(prev_value) = core::mem::take(&mut self.revert) {
                *self.block.blocks.get_mut() = prev_value;
            }
        }
    }

    impl<V: Value> Deref for Transaction<'_, '_, V> {
        type Target = V;

        fn deref(&self) -> &Self::Target {
            self.get()
        }
    }

    impl<V: Value> DerefMut for Transaction<'_, '_, V> {
        fn deref_mut(&mut self) -> &mut Self::Target {
            self.get_mut()
        }
    }
}
pub use block::{Block, Transaction};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get() {
        let cell = Cell::new(0_u64);

        let view0 = cell.view();

        {
            let mut block = cell.block();
            *block.get_mut() = 1;
            block.commit()
        }

        let view1 = cell.view();

        {
            let mut block = cell.block();
            *block.get_mut() = 2;
            block.commit()
        }

        let view2 = cell.view();

        {
            let mut block = cell.block();
            *block.get_mut() = 3;
            block.commit()
        }

        let view3 = cell.view();

        assert_eq!(view0.get(), &0);
        assert_eq!(view1.get(), &1);
        assert_eq!(view2.get(), &2);
        assert_eq!(view3.get(), &3);
    }

    #[test]
    fn transaction_step() {
        let cell = Cell::new(0_u64);

        let mut block = cell.block();

        // Successful transaction
        {
            let mut transaction = block.transaction();
            *transaction.get_mut() = 1;
            transaction.apply();
        }

        // Aborted step
        {
            let mut transaction = block.transaction();
            *transaction.get_mut() = 2;
        }

        // Check that aborted transaction changes don't visible for subsequent transactions
        {
            let transaction = block.transaction();
            assert_eq!(transaction.get(), &1);
        }

        block.commit();

        // Check that effect of aborted step is not visible in the storage after committing transaction
        {
            let view = cell.view();
            assert_eq!(view.get(), &1);
        }
    }

    #[test]
    fn revert() {
        let cell = Cell::new(0_u64);

        {
            let mut block = cell.block();
            *block.get_mut() = 1;
            block.commit()
        }

        {
            let mut block = cell.block();
            *block.get_mut() = 2;
            block.commit()
        }

        let view1 = cell.view();

        {
            let block = cell.block_and_revert();
            block.commit();
        }
        let view2 = cell.view();

        // View is persistent so revert is not visible
        assert_eq!(view1.get(), &2);
        // Revert is visible in the view created after revert was applied
        assert_eq!(view2.get(), &1);
    }
}
