//! Exact retained repo participant groups, including optional custody.

use super::*;
use iroha_data_model::repo::{RepoAgreement, RepoAgreementId};

/// Canonical agreements and their original participant-index readers.
pub(in super::super) struct CheckedRepoAgreements<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, RepoAgreementId, RepoAgreement>,
    initiators: CommittedStorageView<'world, AccountId, BTreeSet<RepoAgreementId>>,
    counterparties: CommittedStorageView<'world, AccountId, BTreeSet<RepoAgreementId>>,
    custodians: CommittedStorageView<'world, AccountId, BTreeSet<RepoAgreementId>>,
}

impl<'world> CheckedRepoAgreements<'world> {
    /// Check every current/predecessor member without allocating or repairing.
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
        let mut work = Work(max_work);
        let result = validate_group(
            &checked.rows,
            &checked.initiators,
            "world.repo_agreements_by_initiator",
            |_, agreement| Some(agreement.initiator()),
            &mut work,
        )
        .and_then(|()| {
            validate_group(
                &checked.rows,
                &checked.counterparties,
                "world.repo_agreements_by_counterparty",
                |_, agreement| Some(agreement.counterparty()),
                &mut work,
            )
        })
        .and_then(|()| {
            validate_group(
                &checked.rows,
                &checked.custodians,
                "world.repo_agreements_by_custodian",
                |_, agreement| agreement.custodian().as_ref(),
                &mut work,
            )
        });
        // A race between the source and index publishers is not stable corruption.
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    /// Borrow exactly the canonical image whose derived groups were checked.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, RepoAgreementId, RepoAgreement> {
        &self.rows
    }

    /// Retain all original source/index identities until encoding completes.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        Ok(self.rows.try_matches_current(&self.world.repo_agreements)?
            && self
                .initiators
                .try_matches_current(&self.world.repo_agreements_by_initiator)?
            && self
                .counterparties
                .try_matches_current(&self.world.repo_agreements_by_counterparty)?
            && self
                .custodians
                .try_matches_current(&self.world.repo_agreements_by_custodian)?)
    }
}

#[cfg(test)]
mod tests;
