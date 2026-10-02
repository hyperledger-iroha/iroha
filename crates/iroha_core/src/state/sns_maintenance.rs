//! Borrowed Time-maintenance authority and its original source journal.

use std::ops::{Deref, DerefMut};

use super::{StateBlock, StateStorageAdmissionError, StateTransaction};
use iroha_crypto::Hash;

/// Exact active Time owner; only the checked State producer constructs it.
pub(crate) struct SnsTimeMaintenance<'block, 'state> {
    pub(super) block: &'block mut StateBlock<'state>,
}

impl<'state> SnsTimeMaintenance<'_, 'state> {
    /// Open a disposable renewal under this original Time/World owner.
    pub(crate) fn renewal_transaction(
        &mut self,
    ) -> Result<SnsMaintenanceTransaction<'_, 'state>, StateStorageAdmissionError> {
        Ok(SnsMaintenanceTransaction {
            transaction: self.block.try_transaction()?,
        })
    }
}

impl<'state> Deref for SnsTimeMaintenance<'_, 'state> {
    type Target = StateBlock<'state>;

    fn deref(&self) -> &Self::Target {
        self.block
    }
}

impl DerefMut for SnsTimeMaintenance<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.block
    }
}

/// A renewal transaction cannot be manufactured from an ordinary transaction.
pub(crate) struct SnsMaintenanceTransaction<'block, 'state> {
    transaction: StateTransaction<'block, 'state>,
}

impl SnsMaintenanceTransaction<'_, '_> {
    /// Bind the prepared charge to the same ledger before any monetary effect.
    pub(crate) fn retain_source(&mut self, identity: Hash) -> Result<(), String> {
        self.transaction
            .fastpq_source_quota
            .retain_sns_purpose(identity)
    }

    /// Publish only through the ordinary transaction's complete preflight.
    pub(crate) fn apply(self) {
        self.transaction.apply();
    }
}

impl<'block, 'state> Deref for SnsMaintenanceTransaction<'block, 'state> {
    type Target = StateTransaction<'block, 'state>;

    fn deref(&self) -> &Self::Target {
        &self.transaction
    }
}

impl DerefMut for SnsMaintenanceTransaction<'_, '_> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.transaction
    }
}
