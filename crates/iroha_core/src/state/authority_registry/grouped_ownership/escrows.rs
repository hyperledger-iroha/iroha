//! Exact escrow grouping over both sealed original native images.
//! Source work does not supply instruction validity, physical funding or finality.

use super::*;
use crate::state::authority_registry::{
    borrowed_controller_work::prepay_account_id, original_images::RawStorageImages,
};
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
        let result = validate_original_escrows(
            &checked.rows,
            &checked.sellers,
            &checked.buyers,
            &checked.statuses,
            max_work,
        );
        checked.finish_validation(result)
    }
    fn finish_validation(
        self,
        result: Result<(), GroupedOwnershipError>,
    ) -> Result<Self, GroupedOwnershipError> {
        if !self.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(self)
    }
    /// Borrow the same canonical rows checked against the retained indexes.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, EscrowId, AssetEscrowRecord> {
        &self.rows
    }
    /// Materialize every original probe before propagating the first native refusal.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let rows = self.rows.try_matches_current(&self.world.asset_escrows);
        let sellers = self
            .sellers
            .try_matches_current(&self.world.asset_escrows_by_seller);
        let buyers = self
            .buyers
            .try_matches_current(&self.world.asset_escrows_by_buyer);
        let statuses = self
            .statuses
            .try_matches_current(&self.world.asset_escrows_by_status);
        let rows = rows?;
        let sellers = sellers?;
        let buyers = buyers?;
        let statuses = statuses?;
        Ok(rows && sellers && buyers && statuses)
    }
}

/// Single Ed25519 seller/buyer and three singleton groups on both images, no undo.
/// This local scheduling reference is neither a validity limit nor a worst-case bound.
pub(in crate::state) const ESCROW_WORK_PER_ROW: u64 = 1366;

struct EscrowWork(u64);
impl EscrowWork {
    fn bounded(max_work: u64) -> Self {
        Self(max_work)
    }
    fn prepay(&mut self, amount: usize) -> Result<(), GroupedOwnershipError> {
        let amount = u64::try_from(amount).map_err(|_| GroupedOwnershipError::WorkLimit)?;
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or(GroupedOwnershipError::WorkLimit)?;
        Ok(())
    }
}
trait EscrowKey: mv::Key {
    fn prepay(&self, work: &mut EscrowWork) -> Result<(), GroupedOwnershipError>;
}
impl EscrowKey for EscrowId {
    fn prepay(&self, work: &mut EscrowWork) -> Result<(), GroupedOwnershipError> {
        work.prepay(32) // existing transparent Hash storage, not a hash-validity check
    }
}
impl EscrowKey for AccountId {
    fn prepay(&self, work: &mut EscrowWork) -> Result<(), GroupedOwnershipError> {
        prepay_account_id(self, |amount| work.prepay(amount))
    }
}
impl EscrowKey for AssetEscrowStatus {
    fn prepay(&self, work: &mut EscrowWork) -> Result<(), GroupedOwnershipError> {
        work.prepay(1)
    }
}
fn equal<K: EscrowKey>(
    left: &K,
    right: &K,
    work: &mut EscrowWork,
) -> Result<bool, GroupedOwnershipError> {
    left.prepay(work)?;
    right.prepay(work)?;
    Ok(left == right)
}
fn next_physical<I: ExactSizeIterator>(
    rows: &mut I,
    work: &mut EscrowWork,
) -> Result<Option<I::Item>, GroupedOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    work.prepay(1)?;
    Ok(rows.next())
}
fn visit_original<'a, K: EscrowKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: GroupImage,
    work: &mut EscrowWork,
    mut inspect: impl FnMut(&'a K, &'a V, &mut EscrowWork) -> Result<(), GroupedOwnershipError>,
) -> Result<(), GroupedOwnershipError> {
    let mut current = rows.current_entries();
    while let Some((key, value)) = next_physical(&mut current, work)? {
        let mut masked = false;
        if image == GroupImage::Predecessor {
            let mut undo = rows.undo_entries();
            while let Some((prior, _)) = next_physical(&mut undo, work)? {
                masked |= equal(key, prior, work)?;
            }
        }
        if !masked {
            inspect(key, value, work)?;
        }
    }
    if image == GroupImage::Predecessor {
        let mut undo = rows.undo_entries();
        while let Some((key, prior)) = next_physical(&mut undo, work)? {
            work.prepay(1)?;
            if let Some(value) = prior {
                inspect(key, value, work)?;
            }
        }
    }
    Ok(())
}
fn lookup<'a, K: EscrowKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: GroupImage,
    key: &K,
    work: &mut EscrowWork,
) -> Result<Option<&'a V>, GroupedOwnershipError> {
    let mut found = None;
    visit_original(rows, image, work, |candidate, value, work| {
        if equal(key, candidate, work)? {
            found = Some(value);
        }
        Ok(())
    })?;
    Ok(found)
}
fn contains<K: EscrowKey>(
    members: &BTreeSet<K>,
    key: &K,
    work: &mut EscrowWork,
) -> Result<bool, GroupedOwnershipError> {
    let mut members = members.iter();
    let mut found = false;
    while let Some(candidate) = next_physical(&mut members, work)? {
        found |= equal(key, candidate, work)?;
    }
    Ok(found)
}
fn seller<'a>(
    record: &'a AssetEscrowRecord,
    _: &mut EscrowWork,
) -> Result<Option<&'a AccountId>, GroupedOwnershipError> {
    Ok(Some(&record.seller))
}
fn buyer<'a>(
    record: &'a AssetEscrowRecord,
    work: &mut EscrowWork,
) -> Result<Option<&'a AccountId>, GroupedOwnershipError> {
    work.prepay(1)?;
    Ok(record.buyer.as_ref())
}
fn status<'a>(
    record: &'a AssetEscrowRecord,
    _: &mut EscrowWork,
) -> Result<Option<&'a AssetEscrowStatus>, GroupedOwnershipError> {
    Ok(Some(&record.status))
}
fn group_error(
    index: &'static str,
    image: GroupImage,
    mismatch: GroupMismatch,
) -> GroupedOwnershipError {
    GroupedOwnershipError::Corrupt {
        index,
        image,
        mismatch,
    }
}
fn validate_escrow_group<G: EscrowKey>(
    rows: &impl RawStorageImages<EscrowId, AssetEscrowRecord>,
    groups: &impl RawStorageImages<G, BTreeSet<EscrowId>>,
    index: &'static str,
    project: impl for<'a> Fn(
        &'a AssetEscrowRecord,
        &mut EscrowWork,
    ) -> Result<Option<&'a G>, GroupedOwnershipError>,
    work: &mut EscrowWork,
) -> Result<(), GroupedOwnershipError> {
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        visit_original(rows, image, work, |id, record, work| {
            if let Some(group) = project(record, work)? {
                let found = if let Some(members) = lookup(groups, image, group, work)? {
                    contains(members, id, work)?
                } else {
                    false
                };
                if !found {
                    return Err(group_error(index, image, GroupMismatch::MissingMember));
                }
            }
            Ok(())
        })?;
        visit_original(groups, image, work, |group, members, work| {
            work.prepay(1)?;
            if members.is_empty() {
                return Err(group_error(index, image, GroupMismatch::EmptyGroup));
            }
            let mut members = members.iter();
            while let Some(id) = next_physical(&mut members, work)? {
                let Some(record) = lookup(rows, image, id, work)? else {
                    return Err(group_error(index, image, GroupMismatch::ForeignMember));
                };
                let matches = if let Some(projected) = project(record, work)? {
                    equal(projected, group, work)?
                } else {
                    false
                };
                if !matches {
                    return Err(group_error(index, image, GroupMismatch::ForeignMember));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}
/// The sole seller, optional-buyer and status relation over both original images.
/// Preserve source-before-inverse and each group's Current-before-Predecessor order.
/// No record ID, account/reference, custody, quantity or instruction validity is added.
pub(in crate::state) fn validate_original_escrows(
    rows: &impl RawStorageImages<EscrowId, AssetEscrowRecord>,
    sellers: &impl RawStorageImages<AccountId, BTreeSet<EscrowId>>,
    buyers: &impl RawStorageImages<AccountId, BTreeSet<EscrowId>>,
    statuses: &impl RawStorageImages<AssetEscrowStatus, BTreeSet<EscrowId>>,
    max_work: u64,
) -> Result<(), GroupedOwnershipError> {
    let mut work = EscrowWork::bounded(max_work);
    validate_escrow_group(
        rows,
        sellers,
        "world.asset_escrows_by_seller",
        seller,
        &mut work,
    )?;
    validate_escrow_group(
        rows,
        buyers,
        "world.asset_escrows_by_buyer",
        buyer,
        &mut work,
    )?;
    validate_escrow_group(
        rows,
        statuses,
        "world.asset_escrows_by_status",
        status,
        &mut work,
    )
}
#[cfg(test)]
pub(in crate::state) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod work_tests;
