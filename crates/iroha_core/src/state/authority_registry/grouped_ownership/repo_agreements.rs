//! Exact repo agreement grouping over both sealed original native images.
//! Source work does not supply instruction validity, physical funding or finality.

use super::*;
use crate::state::authority_registry::{
    borrowed_controller_work::prepay_account_id, original_images::RawStorageImages,
};
use iroha_data_model::repo::{RepoAgreement, RepoAgreementId};

/// Retained canonical repo_agreements and all three original derived index readers.
pub(in super::super) struct CheckedRepoAgreements<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, RepoAgreementId, RepoAgreement>,
    initiators: CommittedStorageView<'world, AccountId, BTreeSet<RepoAgreementId>>,
    counterparties: CommittedStorageView<'world, AccountId, BTreeSet<RepoAgreementId>>,
    custodians: CommittedStorageView<'world, AccountId, BTreeSet<RepoAgreementId>>,
}
impl<'world> CheckedRepoAgreements<'world> {
    /// Check both native images without allocating, rebuilding or repairing.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world.repo_agreements.try_committed_view_nonblocking()?,
            initiators: world
                .repo_agreements_by_initiator
                .try_committed_view_nonblocking()?,
            counterparties: world
                .repo_agreements_by_counterparty
                .try_committed_view_nonblocking()?,
            custodians: world
                .repo_agreements_by_custodian
                .try_committed_view_nonblocking()?,
        };
        let result = validate_original_repo_agreements(
            &checked.rows,
            &checked.initiators,
            &checked.counterparties,
            &checked.custodians,
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
    ) -> &CommittedStorageView<'world, RepoAgreementId, RepoAgreement> {
        &self.rows
    }
    /// Materialize every original probe before propagating the first native refusal.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let rows = self.rows.try_matches_current(&self.world.repo_agreements);
        let initiators = self
            .initiators
            .try_matches_current(&self.world.repo_agreements_by_initiator);
        let counterparties = self
            .counterparties
            .try_matches_current(&self.world.repo_agreements_by_counterparty);
        let custodians = self
            .custodians
            .try_matches_current(&self.world.repo_agreements_by_custodian);
        let rows = rows?;
        let initiators = initiators?;
        let counterparties = counterparties?;
        let custodians = custodians?;
        Ok(rows && initiators && counterparties && custodians)
    }
}

/// Fifteen-byte Name and three Single Ed25519 participant groups on both images, no undo.
/// This local scheduling reference is neither a validity limit nor a worst-case bound.
pub(in crate::state) const REPO_AGREEMENT_WORK_PER_ROW: u64 = 1222;

struct RepoWork(u64);
impl RepoWork {
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
trait RepoKey: mv::Key {
    fn prepay(&self, work: &mut RepoWork) -> Result<(), GroupedOwnershipError>;
}
impl RepoKey for RepoAgreementId {
    fn prepay(&self, work: &mut RepoWork) -> Result<(), GroupedOwnershipError> {
        work.prepay(self.name().as_ref().len()) // transparent retained Name UTF-8 bytes
    }
}
impl RepoKey for AccountId {
    fn prepay(&self, work: &mut RepoWork) -> Result<(), GroupedOwnershipError> {
        prepay_account_id(self, |amount| work.prepay(amount))
    }
}
fn equal<K: RepoKey>(
    left: &K,
    right: &K,
    work: &mut RepoWork,
) -> Result<bool, GroupedOwnershipError> {
    left.prepay(work)?;
    right.prepay(work)?;
    Ok(left == right)
}
fn next_physical<I: ExactSizeIterator>(
    rows: &mut I,
    work: &mut RepoWork,
) -> Result<Option<I::Item>, GroupedOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    work.prepay(1)?;
    Ok(rows.next())
}
fn visit_original<'a, K: RepoKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: GroupImage,
    work: &mut RepoWork,
    mut inspect: impl FnMut(&'a K, &'a V, &mut RepoWork) -> Result<(), GroupedOwnershipError>,
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
fn lookup<'a, K: RepoKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: GroupImage,
    key: &K,
    work: &mut RepoWork,
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
fn contains<K: RepoKey>(
    members: &BTreeSet<K>,
    key: &K,
    work: &mut RepoWork,
) -> Result<bool, GroupedOwnershipError> {
    let mut members = members.iter();
    let mut found = false;
    while let Some(candidate) = next_physical(&mut members, work)? {
        found |= equal(key, candidate, work)?;
    }
    Ok(found)
}
fn initiator<'a>(
    agreement: &'a RepoAgreement,
    _: &mut RepoWork,
) -> Result<Option<&'a AccountId>, GroupedOwnershipError> {
    Ok(Some(agreement.initiator()))
}
fn counterparty<'a>(
    agreement: &'a RepoAgreement,
    _: &mut RepoWork,
) -> Result<Option<&'a AccountId>, GroupedOwnershipError> {
    Ok(Some(agreement.counterparty()))
}
fn custodian<'a>(
    agreement: &'a RepoAgreement,
    work: &mut RepoWork,
) -> Result<Option<&'a AccountId>, GroupedOwnershipError> {
    work.prepay(1)?;
    Ok(agreement.custodian().as_ref())
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
fn validate_repo_agreement_group<G: RepoKey>(
    rows: &impl RawStorageImages<RepoAgreementId, RepoAgreement>,
    groups: &impl RawStorageImages<G, BTreeSet<RepoAgreementId>>,
    index: &'static str,
    project: impl for<'a> Fn(
        &'a RepoAgreement,
        &mut RepoWork,
    ) -> Result<Option<&'a G>, GroupedOwnershipError>,
    work: &mut RepoWork,
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
/// The sole initiator, counterparty and optional-custodian relation over both original images.
/// Preserve source-before-inverse and each group's Current-before-Predecessor order.
/// No record ID, account/reference, custody, quantity or instruction validity is added.
pub(in crate::state) fn validate_original_repo_agreements(
    rows: &impl RawStorageImages<RepoAgreementId, RepoAgreement>,
    initiators: &impl RawStorageImages<AccountId, BTreeSet<RepoAgreementId>>,
    counterparties: &impl RawStorageImages<AccountId, BTreeSet<RepoAgreementId>>,
    custodians: &impl RawStorageImages<AccountId, BTreeSet<RepoAgreementId>>,
    max_work: u64,
) -> Result<(), GroupedOwnershipError> {
    let mut work = RepoWork::bounded(max_work);
    validate_repo_agreement_group(
        rows,
        initiators,
        "world.repo_agreements_by_initiator",
        initiator,
        &mut work,
    )?;
    validate_repo_agreement_group(
        rows,
        counterparties,
        "world.repo_agreements_by_counterparty",
        counterparty,
        &mut work,
    )?;
    validate_repo_agreement_group(
        rows,
        custodians,
        "world.repo_agreements_by_custodian",
        custodian,
        &mut work,
    )
}
#[cfg(test)]
pub(in crate::state) mod test_support;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod work_tests;
