//! One exact contract-alias inverse/lease relation over both original native images.
//!
//! Stale undeployed and expired bindings remain valid stored records until ordinary cleanup.
//! This checks persisted text, inverse membership and lease windows, not deployment,
//! current authority, finality or alias resolution at a given time.

use super::*;
use crate::state::{
    ContractAliasBindingRecord, alias_lease, authority_registry::original_images::RawStorageImages,
};
use iroha_data_model::smart_contract::{ContractAddress, ContractAlias};
use std::cmp::Ordering;

const TABLE: &str = "world.contract_alias_bindings";
const INDEX: &str = "world.contract_aliases";
// The shared predicate performs at most two comparisons of complete 8-byte scalar pairs.
const LEASE_WINDOW_WORK: u64 = 2 * (8 + 8);
// Existing canonical alias text: name::domain.dataspace, at most three 255-byte Names.
const MAX_ALIAS_TEXT_BYTES: u64 = 3 * iroha_model_base::name::MAX_NAME_BYTES as u64 + 3;
// Current canonical address: 6-byte HRP + separator + ceil(29*8/5) payload + checksum.
const ADDRESS_TEXT_BYTES: u64 = 60;

/// One maximum-text inverse pair, both images, with no physical undo entries.
/// Larger quadratic or undo cuts may defer locally. Actual comparisons use actual lengths;
/// this descriptor reference is not a text, gas, row, consensus or ledger-validity limit.
pub(in super::super) const CONTRACT_ALIAS_WORK_PER_ROW: u64 =
    2 * (4 + LEASE_WINDOW_WORK + 4 * (MAX_ALIAS_TEXT_BYTES + ADDRESS_TEXT_BYTES));

/// Canonical bindings retained with their checked original lookup index.
pub(in super::super) struct CheckedContractAliases<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, ContractAddress, ContractAliasBindingRecord>,
    aliases: CommittedStorageView<'world, ContractAlias, ContractAddress>,
}

impl<'world> CheckedContractAliases<'world> {
    /// Retain and check both native images without allocation or live repair.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self::retain(world)?;
        let result = validate(&checked.rows, &checked.aliases, &mut Work(max_work));
        checked.finish_validation(result)
    }

    fn retain(world: &'world World) -> Result<Self, GroupedOwnershipError> {
        Ok(Self {
            world,
            rows: world
                .contract_alias_bindings
                .try_committed_view_nonblocking()?,
            aliases: world.contract_aliases.try_committed_view_nonblocking()?,
        })
    }

    fn finish_validation(
        self,
        result: Result<(), GroupedOwnershipError>,
    ) -> Result<Self, GroupedOwnershipError> {
        // Preserve the committed original-owner fence even on corruption or local refusal.
        if !self.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(self)
    }

    /// Borrow the exact canonical rows whose inverse and leases were checked.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, ContractAddress, ContractAliasBindingRecord> {
        &self.rows
    }

    /// Recheck every original owner after canonical encoding finishes.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        Ok(self
            .rows
            .try_matches_current(&self.world.contract_alias_bindings)?
            && self
                .aliases
                .try_matches_current(&self.world.contract_aliases)?)
    }
}

/// Check both original contract-alias inverse and stored lease images.
/// The caller separately retains/authenticates the native owners, mode and pool;
/// this relation neither parses text again nor obtains current service authority.
pub(in crate::state) fn validate_original_contract_aliases(
    rows: &impl RawStorageImages<ContractAddress, ContractAliasBindingRecord>,
    aliases: &impl RawStorageImages<ContractAlias, ContractAddress>,
    max_work: u64,
) -> Result<(), GroupedOwnershipError> {
    validate(rows, aliases, &mut Work(max_work))
}

fn validate(
    rows: &impl RawStorageImages<ContractAddress, ContractAliasBindingRecord>,
    aliases: &impl RawStorageImages<ContractAlias, ContractAddress>,
    work: &mut Work,
) -> Result<(), GroupedOwnershipError> {
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
            index: INDEX,
            image,
            mismatch,
        };
        visit_original(rows, image, work, |address, record, work| {
            prepay(work, LEASE_WINDOW_WORK)?;
            if let Some(reason) = alias_lease::violation(
                record.lease_expiry_ms,
                record.grace_until_ms,
                record.bound_at_ms,
            ) {
                return Err(GroupedOwnershipError::Source {
                    table: TABLE,
                    image,
                    reason,
                });
            }
            let mut found = false;
            visit_original(aliases, image, work, |alias, target, work| {
                let same_alias =
                    compare_text(alias.as_ref(), record.alias.as_ref(), work)? == Ordering::Equal;
                let same_address =
                    compare_text(target.as_ref(), address.as_ref(), work)? == Ordering::Equal;
                found |= same_alias && same_address;
                Ok(())
            })?;
            if !found {
                return Err(corrupt(GroupMismatch::MissingMember));
            }
            Ok(())
        })?;
        visit_original(aliases, image, work, |alias, target, work| {
            let mut found = false;
            visit_original(rows, image, work, |address, record, work| {
                let same_address =
                    compare_text(target.as_ref(), address.as_ref(), work)? == Ordering::Equal;
                let same_alias =
                    compare_text(alias.as_ref(), record.alias.as_ref(), work)? == Ordering::Equal;
                found |= same_address && same_alias;
                Ok(())
            })?;
            if !found {
                return Err(corrupt(GroupMismatch::ForeignMember));
            }
            Ok(())
        })?;
    }
    Ok(())
}

// Closed to these two native text key shapes. ConstString ordering equals borrowed UTF-8
// ordering; no address decode, alias segmentation, normalization or owned string is needed.
trait TextKey: mv::Key {
    fn text(&self) -> &str;
}
impl TextKey for ContractAddress {
    fn text(&self) -> &str {
        self.as_ref()
    }
}
impl TextKey for ContractAlias {
    fn text(&self) -> &str {
        self.as_ref()
    }
}
fn prepay(work: &mut Work, units: u64) -> Result<(), GroupedOwnershipError> {
    work.0 = work
        .0
        .checked_sub(units)
        .ok_or(GroupedOwnershipError::WorkLimit)?;
    Ok(())
}
fn compare_text(
    left: &str,
    right: &str,
    work: &mut Work,
) -> Result<Ordering, GroupedOwnershipError> {
    let units = u64::try_from(left.len())
        .ok()
        .and_then(|left| {
            u64::try_from(right.len())
                .ok()
                .and_then(|right| left.checked_add(right))
        })
        .ok_or(GroupedOwnershipError::WorkLimit)?;
    prepay(work, units)?;
    Ok(left.cmp(right))
}
fn compare_text_keys<K: TextKey>(
    left: &K,
    right: &K,
    work: &mut Work,
) -> Result<Ordering, GroupedOwnershipError> {
    compare_text(left.text(), right.text(), work)
}

fn next_physical<'a, K: TextKey + 'a, V: 'a>(
    rows: &mut impl ExactSizeIterator<Item = (&'a K, &'a V)>,
    work: &mut Work,
) -> Result<Option<(&'a K, &'a V)>, GroupedOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    // The sealed native iterator exposes its physical length, so refusal occurs
    // before advancing or inspecting a current, masked, no-op or absent row.
    work.charge()?;
    Ok(rows.next())
}

fn visit_original<K: TextKey, V: mv::Value>(
    rows: &impl RawStorageImages<K, V>,
    image: GroupImage,
    work: &mut Work,
    mut visit: impl FnMut(&K, &V, &mut Work) -> Result<(), GroupedOwnershipError>,
) -> Result<(), GroupedOwnershipError> {
    let mut current = rows.current_entries();
    if image == GroupImage::Current {
        while let Some((key, value)) = next_physical(&mut current, work)? {
            visit(key, value, work)?;
        }
        return Ok(());
    }
    let mut undo = rows.undo_entries();
    let mut current_row = next_physical(&mut current, work)?;
    let mut undo_row = next_physical(&mut undo, work)?;
    loop {
        match (current_row, undo_row) {
            (Some((current_key, current_value)), Some((undo_key, prior))) => {
                match compare_text_keys(current_key, undo_key, work)? {
                    Ordering::Less => {
                        visit(current_key, current_value, work)?;
                        current_row = next_physical(&mut current, work)?;
                    }
                    Ordering::Equal => {
                        if let Some(value) = prior {
                            visit(undo_key, value, work)?;
                        }
                        current_row = next_physical(&mut current, work)?;
                        undo_row = next_physical(&mut undo, work)?;
                    }
                    Ordering::Greater => {
                        if let Some(value) = prior {
                            visit(undo_key, value, work)?;
                        }
                        undo_row = next_physical(&mut undo, work)?;
                    }
                }
            }
            (Some((key, value)), None) => {
                visit(key, value, work)?;
                current_row = next_physical(&mut current, work)?;
            }
            (None, Some((key, prior))) => {
                if let Some(value) = prior {
                    visit(key, value, work)?;
                }
                undo_row = next_physical(&mut undo, work)?;
            }
            (None, None) => return Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "contract_aliases/test_support.rs"]
pub(in crate::state) mod test_support;
#[cfg(test)]
#[path = "contract_aliases/tests.rs"]
mod tests;
