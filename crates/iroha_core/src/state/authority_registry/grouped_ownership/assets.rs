//! Exact balance partitions and their five derived lookup indexes.
//!
//! Holder membership is existential over the canonical account/definition
//! partition range. A zero partition never removes another nonzero partition.

use super::*;
use crate::state::{AsAssetIdAccountDefinitionCompare, AssetByAccountDefinitionBounds};
use iroha_data_model::{
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId, AssetId, AssetValue},
    domain::Domain,
};

/// Retained balance, definition, domain and derived readers from one native cut.
pub(in super::super) struct CheckedAssets<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, AssetId, AssetValue>,
    definitions: CommittedStorageView<'world, AssetDefinitionId, AssetDefinition>,
    domains: CommittedStorageView<'world, DomainId, Domain>,
    by_definition: CommittedStorageView<'world, AssetDefinitionId, BTreeSet<AssetId>>,
    by_account: CommittedStorageView<'world, AccountId, BTreeSet<AssetId>>,
    by_domain: CommittedStorageView<'world, DomainId, BTreeSet<AssetId>>,
    holders: CommittedStorageView<'world, AssetDefinitionId, BTreeSet<AccountId>>,
    nonzero: CommittedStorageView<'world, AssetDefinitionId, BTreeSet<AccountId>>,
}

impl<'world> CheckedAssets<'world> {
    /// Check both original images without allocating or repairing derived rows.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world.assets.try_committed_view_nonblocking()?,
            definitions: world.asset_definitions.try_committed_view_nonblocking()?,
            domains: world.domains.try_committed_view_nonblocking()?,
            by_definition: world
                .asset_definition_assets
                .try_committed_view_nonblocking()?,
            by_account: world.assets_by_account.try_committed_view_nonblocking()?,
            by_domain: world.assets_by_domain.try_committed_view_nonblocking()?,
            holders: world
                .asset_definition_holders
                .try_committed_view_nonblocking()?,
            nonzero: world
                .asset_definition_nonzero_holders
                .try_committed_view_nonblocking()?,
        };
        let result = checked.validate(&mut Work(max_work));
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    fn validate(&self, work: &mut Work) -> Result<(), GroupedOwnershipError> {
        validate_group(
            &self.rows,
            &self.by_definition,
            "world.asset_definition_assets",
            |id, _| Some(id.definition()),
            work,
        )?;
        validate_group(
            &self.rows,
            &self.by_account,
            "world.assets_by_account",
            |id, _| Some(id.account()),
            work,
        )?;
        for image in [GroupImage::Current, GroupImage::Predecessor] {
            visit_image(&self.rows, image, work, |id, value, work| {
                work.charge()?;
                let definition = get_at(&self.definitions, image, id.definition()).ok_or(
                    GroupedOwnershipError::Source {
                        table: "world.assets",
                        image,
                        reason: "asset definition is absent",
                    },
                )?;
                if definition.balance_scope_policy() == AssetBalancePolicy::DataspaceRestricted
                    && definition.owning_domain().is_none()
                {
                    return Err(GroupedOwnershipError::Source {
                        table: "world.asset_definitions",
                        image,
                        reason: "restricted definition has no owning domain",
                    });
                }
                if let Some(domain) = definition.owning_domain() {
                    work.charge()?;
                    if get_at(&self.domains, image, domain).is_none() {
                        return Err(GroupedOwnershipError::Source {
                            table: "world.asset_definitions",
                            image,
                            reason: "owning domain is absent",
                        });
                    }
                    require_member(&self.by_domain, image, domain, id, "world.assets_by_domain")?;
                }
                require_member(
                    &self.holders,
                    image,
                    id.definition(),
                    id.account(),
                    "world.asset_definition_holders",
                )?;
                if !value.as_ref().is_zero() {
                    require_member(
                        &self.nonzero,
                        image,
                        id.definition(),
                        id.account(),
                        "world.asset_definition_nonzero_holders",
                    )?;
                }
                Ok(())
            })?;
            visit_image(&self.by_domain, image, work, |domain, members, work| {
                nonempty(members, "world.assets_by_domain", image)?;
                for id in members {
                    work.charge()?;
                    if get_at(&self.rows, image, id).is_none() {
                        return Err(foreign("world.assets_by_domain", image));
                    }
                    work.charge()?;
                    if !get_at(&self.definitions, image, id.definition()).is_some_and(
                        |definition| definition.owning_domain().as_ref() == Some(domain),
                    ) {
                        return Err(foreign("world.assets_by_domain", image));
                    }
                }
                Ok(())
            })?;
            for (groups, nonzero, index) in [
                (&self.holders, false, "world.asset_definition_holders"),
                (
                    &self.nonzero,
                    true,
                    "world.asset_definition_nonzero_holders",
                ),
            ] {
                visit_image(groups, image, work, |definition, members, work| {
                    nonempty(members, index, image)?;
                    for account in members {
                        work.charge()?;
                        if !self.has_partition(image, account, definition, nonzero, work)? {
                            return Err(foreign(index, image));
                        }
                    }
                    Ok(())
                })?;
            }
        }
        Ok(())
    }

    /// Search only the borrowed canonical account/definition range. Current rows
    /// hidden by undo and absent undo entries consume work before being filtered.
    fn has_partition(
        &self,
        image: GroupImage,
        account: &AccountId,
        definition: &AssetDefinitionId,
        nonzero: bool,
        work: &mut Work,
    ) -> Result<bool, GroupedOwnershipError> {
        for (id, value) in self
            .rows
            .current()
            .range::<_, dyn AsAssetIdAccountDefinitionCompare>(AssetByAccountDefinitionBounds::new(
                account, definition,
            ))
        {
            work.charge()?;
            if (image == GroupImage::Current || !self.rows.undo().contains_key(id))
                && (!nonzero || !value.as_ref().is_zero())
            {
                return Ok(true);
            }
        }
        if image == GroupImage::Predecessor {
            for (_, value) in self
                .rows
                .undo()
                .range::<_, dyn AsAssetIdAccountDefinitionCompare>(
                    AssetByAccountDefinitionBounds::new(account, definition),
                )
            {
                work.charge()?;
                if value
                    .as_ref()
                    .is_some_and(|value| !nonzero || !value.as_ref().is_zero())
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }

    /// Borrow precisely the source balance rows validated against all indexes.
    pub(in super::super) fn rows(&self) -> &CommittedStorageView<'world, AssetId, AssetValue> {
        &self.rows
    }

    /// Detect publication through any of the eight original native owners.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        Ok(self.rows.try_matches_current(&self.world.assets)?
            && self
                .definitions
                .try_matches_current(&self.world.asset_definitions)?
            && self.domains.try_matches_current(&self.world.domains)?
            && self
                .by_definition
                .try_matches_current(&self.world.asset_definition_assets)?
            && self
                .by_account
                .try_matches_current(&self.world.assets_by_account)?
            && self
                .by_domain
                .try_matches_current(&self.world.assets_by_domain)?
            && self
                .holders
                .try_matches_current(&self.world.asset_definition_holders)?
            && self
                .nonzero
                .try_matches_current(&self.world.asset_definition_nonzero_holders)?)
    }
}

fn require_member<G: mv::Key, K: mv::Key>(
    groups: &CommittedStorageView<'_, G, BTreeSet<K>>,
    image: GroupImage,
    group: &G,
    member: &K,
    index: &'static str,
) -> Result<(), GroupedOwnershipError> {
    if !get_at(groups, image, group).is_some_and(|members| members.contains(member)) {
        return Err(GroupedOwnershipError::Corrupt {
            index,
            image,
            mismatch: GroupMismatch::MissingMember,
        });
    }
    Ok(())
}

fn nonempty<K>(
    members: &BTreeSet<K>,
    index: &'static str,
    image: GroupImage,
) -> Result<(), GroupedOwnershipError> {
    if members.is_empty() {
        return Err(GroupedOwnershipError::Corrupt {
            index,
            image,
            mismatch: GroupMismatch::EmptyGroup,
        });
    }
    Ok(())
}

fn foreign(index: &'static str, image: GroupImage) -> GroupedOwnershipError {
    GroupedOwnershipError::Corrupt {
        index,
        image,
        mismatch: GroupMismatch::ForeignMember,
    }
}

#[cfg(test)]
mod tests;
