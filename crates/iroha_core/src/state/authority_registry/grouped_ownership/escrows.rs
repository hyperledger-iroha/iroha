//! Exact escrow grouping, including the absence of an optional buyer.

use super::*;
use iroha_data_model::escrow::{AssetEscrowRecord, AssetEscrowStatus, EscrowId};

/// Retained canonical escrows and all three original derived index readers.
pub(in super::super) struct CheckedEscrows<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, EscrowId, AssetEscrowRecord>,
    sellers: CommittedStorageView<'world, AccountId, BTreeSet<EscrowId>>,
    buyers: CommittedStorageView<'world, AccountId, BTreeSet<EscrowId>>,
    statuses: CommittedStorageView<'world, AssetEscrowStatus, BTreeSet<EscrowId>>,
}

impl<'world> CheckedEscrows<'world> {
    /// Check both native images without allocating, rebuilding or repairing.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world.asset_escrows.try_committed_view_nonblocking()?,
            sellers: world
                .asset_escrows_by_seller
                .try_committed_view_nonblocking()?,
            buyers: world
                .asset_escrows_by_buyer
                .try_committed_view_nonblocking()?,
            statuses: world
                .asset_escrows_by_status
                .try_committed_view_nonblocking()?,
        };
        let mut work = Work(max_work);
        let result = validate_group(
            &checked.rows,
            &checked.sellers,
            "world.asset_escrows_by_seller",
            |_, escrow| Some(&escrow.seller),
            &mut work,
        )
        .and_then(|()| {
            validate_group(
                &checked.rows,
                &checked.buyers,
                "world.asset_escrows_by_buyer",
                |_, escrow| escrow.buyer.as_ref(),
                &mut work,
            )
        })
        .and_then(|()| {
            validate_group(
                &checked.rows,
                &checked.statuses,
                "world.asset_escrows_by_status",
                |_, escrow| Some(&escrow.status),
                &mut work,
            )
        });
        // Source/index publication takes precedence over a mixed-cut mismatch.
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    /// Borrow the same canonical rows checked against the retained indexes.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, EscrowId, AssetEscrowRecord> {
        &self.rows
    }

    /// Check all original native identities after encoding, including empty maps.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        Ok(self.rows.try_matches_current(&self.world.asset_escrows)?
            && self
                .sellers
                .try_matches_current(&self.world.asset_escrows_by_seller)?
            && self
                .buyers
                .try_matches_current(&self.world.asset_escrows_by_buyer)?
            && self
                .statuses
                .try_matches_current(&self.world.asset_escrows_by_status)?)
    }
}

#[cfg(test)]
mod tests;
