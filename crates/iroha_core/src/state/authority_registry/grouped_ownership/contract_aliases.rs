//! Exact contract-alias inverse checks over both original retained images.
//!
//! Stale undeployed and expired bindings remain valid stored records until
//! ordinary cleanup. This checks the persisted inverse and lease relation, not
//! deployment, current authority, finality or alias resolution at a given time.

use super::*;
use crate::state::{ContractAliasBindingRecord, alias_lease};
use iroha_data_model::smart_contract::{ContractAddress, ContractAlias};

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
        let checked = Self {
            world,
            rows: world
                .contract_alias_bindings
                .try_committed_view_nonblocking()?,
            aliases: world.contract_aliases.try_committed_view_nonblocking()?,
        };
        let result = checked.validate(&mut Work(max_work));
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    fn validate(&self, work: &mut Work) -> Result<(), GroupedOwnershipError> {
        for image in [GroupImage::Current, GroupImage::Predecessor] {
            let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
                index: "world.contract_aliases",
                image,
                mismatch,
            };
            visit_image(&self.rows, image, work, |address, record, _| {
                if let Some(reason) = alias_lease::violation(
                    record.lease_expiry_ms,
                    record.grace_until_ms,
                    record.bound_at_ms,
                ) {
                    return Err(GroupedOwnershipError::Source {
                        table: "world.contract_alias_bindings",
                        image,
                        reason,
                    });
                }
                if get_at(&self.aliases, image, &record.alias) != Some(address) {
                    return Err(corrupt(GroupMismatch::MissingMember));
                }
                Ok(())
            })?;
            visit_image(&self.aliases, image, work, |alias, address, _| {
                if !get_at(&self.rows, image, address).is_some_and(|record| &record.alias == alias)
                {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                }
                Ok(())
            })?;
        }
        Ok(())
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

#[cfg(test)]
mod tests;
