//! Exact asset-definition ownership, domain and confidential-policy projections.
//!
//! Keep the canonical definition/domain readers and all five derived readers
//! alive through encoding. Balances and their five indexes are a separate check.

use super::confidential_policies::RetainedConfidentialPolicies;
use super::*;
use iroha_data_model::{
    asset::{AssetBalancePolicy, AssetDefinition, AssetDefinitionId},
    domain::Domain,
};

/// Original canonical definitions checked against all definition lookup indexes.
pub(in super::super) struct CheckedAssetDefinitions<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, AssetDefinitionId, AssetDefinition>,
    domains: CommittedStorageView<'world, DomainId, Domain>,
    contexts: CommittedStorageView<'world, AssetDefinitionId, DomainId>,
    by_domain: CommittedStorageView<'world, DomainId, BTreeSet<AssetDefinitionId>>,
    by_owner: CommittedStorageView<'world, AccountId, BTreeSet<AssetDefinitionId>>,
    confidential_policies: RetainedConfidentialPolicies<'world>,
}

impl<'world> CheckedAssetDefinitions<'world> {
    /// Check both retained images without allocation or mutation of live state.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world.asset_definitions.try_committed_view_nonblocking()?,
            domains: world.domains.try_committed_view_nonblocking()?,
            contexts: world
                .asset_definition_domains
                .try_committed_view_nonblocking()?,
            by_domain: world
                .domain_asset_definitions
                .try_committed_view_nonblocking()?,
            by_owner: world
                .asset_definitions_by_owner
                .try_committed_view_nonblocking()?,
            confidential_policies: RetainedConfidentialPolicies::retain(world)?,
        };
        let result = checked.validate(&mut Work(max_work));
        // Native publication changes take precedence over apparent corruption.
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    fn validate(&self, work: &mut Work) -> Result<(), GroupedOwnershipError> {
        for image in [GroupImage::Current, GroupImage::Predecessor] {
            let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
                index: "world.asset_definition_domains",
                image,
                mismatch,
            };
            visit_image(&self.rows, image, work, |id, definition, work| {
                let domain = definition.owning_domain().as_ref();
                if definition.balance_scope_policy() == AssetBalancePolicy::DataspaceRestricted
                    && domain.is_none()
                {
                    return Err(GroupedOwnershipError::Source {
                        table: "world.asset_definitions",
                        image,
                        reason: "restricted definition has no owning domain",
                    });
                }
                if let Some(domain) = domain {
                    // Charge before consulting another original canonical owner.
                    work.charge()?;
                    if get_at(&self.domains, image, domain).is_none() {
                        return Err(GroupedOwnershipError::Source {
                            table: "world.asset_definitions",
                            image,
                            reason: "owning domain is absent",
                        });
                    }
                }
                if get_at(&self.contexts, image, id) != domain {
                    return Err(corrupt(if domain.is_some() {
                        GroupMismatch::MissingMember
                    } else {
                        GroupMismatch::ForeignMember
                    }));
                }
                Ok(())
            })?;
            visit_image(&self.contexts, image, work, |id, domain, _| {
                if !get_at(&self.rows, image, id)
                    .is_some_and(|definition| definition.owning_domain().as_ref() == Some(domain))
                {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                }
                Ok(())
            })?;
        }
        validate_group(
            &self.rows,
            &self.by_owner,
            "world.asset_definitions_by_owner",
            |_, definition| Some(definition.owned_by()),
            work,
        )?;
        validate_group(
            &self.rows,
            &self.by_domain,
            "world.domain_asset_definitions",
            |_, definition| definition.owning_domain().as_ref(),
            work,
        )?;
        self.confidential_policies.validate(&self.rows, work)
    }

    /// Borrow the exact canonical rows whose derived lookups were checked.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, AssetDefinitionId, AssetDefinition> {
        &self.rows
    }

    /// Check every original reader until canonical encoding has completed.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let rows = self
            .rows
            .try_matches_current(&self.world.asset_definitions)?;
        let domains = self.domains.try_matches_current(&self.world.domains)?;
        let contexts = self
            .contexts
            .try_matches_current(&self.world.asset_definition_domains)?;
        let by_domain = self
            .by_domain
            .try_matches_current(&self.world.domain_asset_definitions)?;
        let by_owner = self
            .by_owner
            .try_matches_current(&self.world.asset_definitions_by_owner)?;
        let confidential_policies = self.confidential_policies.matches_current()?;
        Ok(rows && domains && contexts && by_domain && by_owner && confidential_policies)
    }
}

#[cfg(test)]
mod tests;
