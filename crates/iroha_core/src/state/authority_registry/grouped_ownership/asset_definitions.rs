//! Exact definition/domain/group/policy relations over seven original owners.
//!
//! Both committed and frozen consumers share these sealed native-image checks.
//! Work is prepaid source inspection, not allocation or finalized authority.
//! TODO: join every original relation/cell/history to StatePublication and Kura.

use super::confidential_policies::{self, RetainedConfidentialPolicies};
use super::*;
use crate::state::authority_registry::{
    borrowed_controller_work::prepay_account_id, original_images::RawStorageImages,
};
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
        let (transitions, counts) = checked.confidential_policies.originals();
        let result = validate_original_asset_definitions(
            &checked.rows,
            &checked.domains,
            &checked.contexts,
            &checked.by_domain,
            &checked.by_owner,
            transitions,
            counts,
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

    /// Borrow the exact canonical rows whose derived lookups were checked.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, AssetDefinitionId, AssetDefinition> {
        &self.rows
    }

    /// Probe all seven actual native owners before propagating any refusal.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let rows = self.rows.try_matches_current(&self.world.asset_definitions);
        let domains = self.domains.try_matches_current(&self.world.domains);
        let contexts = self
            .contexts
            .try_matches_current(&self.world.asset_definition_domains);
        let by_domain = self
            .by_domain
            .try_matches_current(&self.world.domain_asset_definitions);
        let by_owner = self
            .by_owner
            .try_matches_current(&self.world.asset_definitions_by_owner);
        let [transitions, counts] = self.confidential_policies.current_results();
        let rows = rows?;
        let domains = domains?;
        let contexts = contexts?;
        let by_domain = by_domain?;
        let by_owner = by_owner?;
        let transitions = transitions?;
        let counts = counts?;
        Ok(rows && domains && contexts && by_domain && by_owner && transitions && counts)
    }
}

/// One-row Single Ed25519/domain126/pending-window reference over both images.
/// Wider controllers and quadratic cuts may need more local admitted work; this
/// is not a row, policy, controller, gas, schema or ledger-validity maximum.
pub(in super::super) const ASSET_DEFINITION_WORK_PER_ROW: u64 = 3868;

/// Prepay the existing fixed sixteen UUID bytes before copying/comparing them.
/// The callback preserves the caller's exact local refusal and never reparses an id.
pub(super) fn prepay_asset_definition_id<E>(
    _: &AssetDefinitionId,
    mut prepay: impl FnMut(usize) -> Result<(), E>,
) -> Result<(), E> {
    prepay(16)
}

pub(super) struct AssetDefinitionWork(u64);
impl AssetDefinitionWork {
    pub(super) fn bounded(max_work: u64) -> Self {
        Self(max_work)
    }
    pub(super) fn prepay(&mut self, amount: usize) -> Result<(), GroupedOwnershipError> {
        let amount = u64::try_from(amount).map_err(|_| GroupedOwnershipError::WorkLimit)?;
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or(GroupedOwnershipError::WorkLimit)?;
        Ok(())
    }
}

pub(super) trait AssetWorkKey: mv::Key {
    fn prepay(&self, work: &mut AssetDefinitionWork) -> Result<(), GroupedOwnershipError>;
}
impl AssetWorkKey for AssetDefinitionId {
    fn prepay(&self, work: &mut AssetDefinitionWork) -> Result<(), GroupedOwnershipError> {
        prepay_asset_definition_id(self, |amount| work.prepay(amount))
    }
}
impl AssetWorkKey for DomainId {
    fn prepay(&self, work: &mut AssetDefinitionWork) -> Result<(), GroupedOwnershipError> {
        work.prepay(self.name().as_ref().len())?;
        work.prepay(self.dataspace().as_ref().len())
    }
}
impl AssetWorkKey for AccountId {
    fn prepay(&self, work: &mut AssetDefinitionWork) -> Result<(), GroupedOwnershipError> {
        prepay_account_id(self, |amount| work.prepay(amount))
    }
}
impl AssetWorkKey for u64 {
    fn prepay(&self, work: &mut AssetDefinitionWork) -> Result<(), GroupedOwnershipError> {
        work.prepay(8)
    }
}
impl AssetWorkKey for (u64, AssetDefinitionId) {
    fn prepay(&self, work: &mut AssetDefinitionWork) -> Result<(), GroupedOwnershipError> {
        work.prepay(8)?;
        self.1.prepay(work)
    }
}

pub(super) fn equal<K: AssetWorkKey>(
    left: &K,
    right: &K,
    work: &mut AssetDefinitionWork,
) -> Result<bool, GroupedOwnershipError> {
    left.prepay(work)?;
    right.prepay(work)?;
    Ok(left == right)
}

pub(super) fn next_physical<I: ExactSizeIterator>(
    rows: &mut I,
    work: &mut AssetDefinitionWork,
) -> Result<Option<I::Item>, GroupedOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    work.prepay(1)?;
    Ok(rows.next())
}

pub(super) fn visit_original<'a, K: AssetWorkKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: GroupImage,
    work: &mut AssetDefinitionWork,
    mut inspect: impl FnMut(&'a K, &'a V, &mut AssetDefinitionWork) -> Result<(), GroupedOwnershipError>,
) -> Result<(), GroupedOwnershipError> {
    let mut current = rows.current_entries();
    while let Some((key, value)) = next_physical(&mut current, work)? {
        let mut masked = false;
        if image == GroupImage::Predecessor {
            let mut undo = rows.undo_entries();
            while let Some((prior_key, _)) = next_physical(&mut undo, work)? {
                masked |= equal(key, prior_key, work)?;
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

pub(super) fn lookup<'a, K: AssetWorkKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: GroupImage,
    key: &K,
    work: &mut AssetDefinitionWork,
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

fn contains(
    members: &BTreeSet<AssetDefinitionId>,
    key: &AssetDefinitionId,
    work: &mut AssetDefinitionWork,
) -> Result<bool, GroupedOwnershipError> {
    let mut members = members.iter();
    let mut found = false;
    while let Some(candidate) = next_physical(&mut members, work)? {
        found |= equal(key, candidate, work)?;
    }
    Ok(found)
}

fn optional_domain_equal(
    left: &Option<DomainId>,
    right: Option<&DomainId>,
    work: &mut AssetDefinitionWork,
) -> Result<bool, GroupedOwnershipError> {
    work.prepay(2)?;
    match (left.as_ref(), right) {
        (Some(left), Some(right)) => equal(left, right, work),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}

/// The sole complete definition/reference/group/policy relation over original images.
/// Phased Current/Predecessor order and all source/index predicates stay unchanged.
/// No accounts, embedded-id, alias, balance quantity or zk_assets rule is added.
pub(in crate::state) fn validate_original_asset_definitions(
    rows: &impl RawStorageImages<AssetDefinitionId, AssetDefinition>,
    domains: &impl RawStorageImages<DomainId, Domain>,
    contexts: &impl RawStorageImages<AssetDefinitionId, DomainId>,
    by_domain: &impl RawStorageImages<DomainId, BTreeSet<AssetDefinitionId>>,
    by_owner: &impl RawStorageImages<AccountId, BTreeSet<AssetDefinitionId>>,
    transitions: &impl RawStorageImages<(u64, AssetDefinitionId), ()>,
    counts: &impl RawStorageImages<u64, u32>,
    max_work: u64,
) -> Result<(), GroupedOwnershipError> {
    let mut work = AssetDefinitionWork::bounded(max_work);
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
            index: "world.asset_definition_domains",
            image,
            mismatch,
        };
        visit_original(rows, image, &mut work, |id, definition, work| {
            work.prepay(2)?; // balance policy and actual optional-domain tag
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
                if lookup(domains, image, domain, work)?.is_none() {
                    return Err(GroupedOwnershipError::Source {
                        table: "world.asset_definitions",
                        image,
                        reason: "owning domain is absent",
                    });
                }
            }
            if !optional_domain_equal(
                definition.owning_domain(),
                lookup(contexts, image, id, work)?,
                work,
            )? {
                return Err(corrupt(if domain.is_some() {
                    GroupMismatch::MissingMember
                } else {
                    GroupMismatch::ForeignMember
                }));
            }
            Ok(())
        })?;
        visit_original(contexts, image, &mut work, |id, domain, work| {
            let Some(definition) = lookup(rows, image, id, work)? else {
                return Err(corrupt(GroupMismatch::ForeignMember));
            };
            if !optional_domain_equal(definition.owning_domain(), Some(domain), work)? {
                return Err(corrupt(GroupMismatch::ForeignMember));
            }
            Ok(())
        })?;
    }
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
            index: "world.asset_definitions_by_owner",
            image,
            mismatch,
        };
        visit_original(rows, image, &mut work, |id, definition, work| {
            let Some(members) = lookup(by_owner, image, definition.owned_by(), work)? else {
                return Err(corrupt(GroupMismatch::MissingMember));
            };
            if !contains(members, id, work)? {
                return Err(corrupt(GroupMismatch::MissingMember));
            }
            Ok(())
        })?;
        visit_original(by_owner, image, &mut work, |owner, members, work| {
            work.prepay(1)?;
            if members.is_empty() {
                return Err(corrupt(GroupMismatch::EmptyGroup));
            }
            let mut members = members.iter();
            while let Some(id) = next_physical(&mut members, work)? {
                let Some(definition) = lookup(rows, image, id, work)? else {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                };
                if !equal(definition.owned_by(), owner, work)? {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                }
            }
            Ok(())
        })?;
    }
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
            index: "world.domain_asset_definitions",
            image,
            mismatch,
        };
        visit_original(rows, image, &mut work, |id, definition, work| {
            work.prepay(1)?;
            if let Some(domain) = definition.owning_domain().as_ref() {
                let Some(members) = lookup(by_domain, image, domain, work)? else {
                    return Err(corrupt(GroupMismatch::MissingMember));
                };
                if !contains(members, id, work)? {
                    return Err(corrupt(GroupMismatch::MissingMember));
                }
            }
            Ok(())
        })?;
        visit_original(by_domain, image, &mut work, |domain, members, work| {
            work.prepay(1)?;
            if members.is_empty() {
                return Err(corrupt(GroupMismatch::EmptyGroup));
            }
            let mut members = members.iter();
            while let Some(id) = next_physical(&mut members, work)? {
                let Some(definition) = lookup(rows, image, id, work)? else {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                };
                if !optional_domain_equal(definition.owning_domain(), Some(domain), work)? {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                }
            }
            Ok(())
        })?;
    }
    confidential_policies::validate_original_confidential_policies(
        rows,
        transitions,
        counts,
        &mut work,
    )
}

#[cfg(test)]
pub(in crate::state) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod work_tests;
