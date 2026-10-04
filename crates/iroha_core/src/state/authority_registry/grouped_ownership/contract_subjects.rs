//! Four original owners for the complete retained contract-subject relation.

use super::*;
use crate::{
    smartcontracts::code::ContractSubjectBinding,
    state::contract_subject_validation::{self as relation, Image, Work},
};
use iroha_crypto::Hash;
use iroha_data_model::{account::AccountValue, smart_contract::ContractAddress};

/// Canonical bindings retained with their exact account, active-code and reverse owners.
pub(in super::super) struct CheckedContractSubjects<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, ContractAddress, ContractSubjectBinding>,
    reverse: CommittedStorageView<'world, AccountId, ContractAddress>,
    accounts: CommittedStorageView<'world, AccountId, AccountValue>,
    instances: CommittedStorageView<'world, ContractAddress, Hash>,
}
impl<'world> CheckedContractSubjects<'world> {
    /// Retain every original owner and validate both images before encoding.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world
                .contract_subject_bindings
                .try_committed_view_nonblocking()?,
            reverse: world
                .contract_subject_addresses
                .try_committed_view_nonblocking()?,
            accounts: world.accounts.try_committed_view_nonblocking()?,
            instances: world.contract_instances.try_committed_view_nonblocking()?,
        };
        let result = validate_original_contract_subjects(
            &checked.rows,
            &checked.reverse,
            &checked.accounts,
            &checked.instances,
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
    /// Borrow the original canonical rows consumed by the catalog encoder.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, ContractAddress, ContractSubjectBinding> {
        &self.rows
    }
    /// Recheck all four original native publication identities.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        // Compute every original Result before any error propagation.
        let rows = self
            .rows
            .try_matches_current(&self.world.contract_subject_bindings);
        let reverse = self
            .reverse
            .try_matches_current(&self.world.contract_subject_addresses);
        let accounts = self.accounts.try_matches_current(&self.world.accounts);
        let instances = self
            .instances
            .try_matches_current(&self.world.contract_instances);
        let rows = rows?;
        let reverse = reverse?;
        let accounts = accounts?;
        let instances = instances?;
        Ok(rows && reverse && accounts && instances)
    }
}
/// One original subject/lifecycle/inverse relation for committed and frozen owners.
///
/// Preserve source-before-index and Current-before-Predecessor ordering. Inputs
/// are sealed native originals; no rebuild, fresh view or alternate predicate.
pub(in crate::state) fn validate_original_contract_subjects(
    rows: &impl relation::Images<ContractAddress, ContractSubjectBinding>,
    reverse: &impl relation::Images<AccountId, ContractAddress>,
    accounts: &impl relation::Images<AccountId, AccountValue>,
    instances: &impl relation::Images<ContractAddress, Hash>,
    max_work: u64,
) -> Result<(), GroupedOwnershipError> {
    let mut work = Work::bounded(max_work);
    relation::validate_sources(rows, accounts, instances, &mut work)
        .and_then(|()| relation::validate_index(rows, reverse, &mut work))
        .map_err(error)
}

fn image(image: Image) -> GroupImage {
    match image {
        Image::Current => GroupImage::Current,
        Image::Predecessor => GroupImage::Predecessor,
    }
}
fn error(error: relation::Error<'_>) -> GroupedOwnershipError {
    match error.kind {
        relation::ErrorKind::WorkLimit => GroupedOwnershipError::WorkLimit,
        relation::ErrorKind::Source => GroupedOwnershipError::Source {
            image: image(error.image),
            table: error.table,
            reason: error.reason,
        },
        relation::ErrorKind::MissingIndex | relation::ErrorKind::ForeignIndex => {
            GroupedOwnershipError::Corrupt {
                image: image(error.image),
                index: "world.contract_subject_addresses",
                mismatch: if error.kind == relation::ErrorKind::MissingIndex {
                    GroupMismatch::MissingMember
                } else {
                    GroupMismatch::ForeignMember
                },
            }
        }
    }
}

#[cfg(test)]
mod tests;
