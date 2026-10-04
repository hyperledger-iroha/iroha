//! Bounded exact grouping checks over original native current and undo maps.
//!
//! Asset, NFT, RWA, escrow and repo captures retain the same source and index
//! readers through row encoding. This supplies no finalized State authority.
//! TODO: consume all derived checks in complete State/Kura publication.

use crate::state::World;
use iroha_data_model::{
    account::AccountId,
    nft::{NftId, NftValue},
    rwa::{RwaId, RwaValue},
};
use iroha_model_base::{domain::DomainId, name::Name};
#[cfg(test)]
use mv::storage::StorageReadOnly;
use mv::{PublicationPreparationError, storage::CommittedStorageView};
use std::{collections::BTreeSet, convert::Infallible};

mod escrows;
#[cfg(test)]
pub(in crate::state) use escrows::test_support as escrow_test_support;
pub(in crate::state) use escrows::validate_original_escrows;
pub(super) use escrows::{CheckedEscrows, ESCROW_WORK_PER_ROW};
mod repo_agreements;
#[cfg(test)]
pub(in crate::state) use repo_agreements::test_support as repo_agreement_test_support;
pub(in crate::state) use repo_agreements::validate_original_repo_agreements;
pub(super) use repo_agreements::{CheckedRepoAgreements, REPO_AGREEMENT_WORK_PER_ROW};
mod asset_definitions;
#[cfg(test)]
pub(in crate::state) use asset_definitions::test_support as asset_definition_test_support;
pub(in crate::state) use asset_definitions::validate_original_asset_definitions;
pub(super) use asset_definitions::{ASSET_DEFINITION_WORK_PER_ROW, CheckedAssetDefinitions};
mod assets;
mod confidential_policies;
#[cfg(test)]
pub(in crate::state) use assets::test_support as asset_balance_test_support;
pub(in crate::state) use assets::validate_original_assets;
pub(super) use assets::{ASSET_BALANCE_WORK_PER_ROW, CheckedAssets};
mod contract_aliases;
#[cfg(test)]
pub(in crate::state) use contract_aliases::test_support as contract_alias_test_support;
pub(in crate::state) use contract_aliases::validate_original_contract_aliases;
pub(super) use contract_aliases::{CONTRACT_ALIAS_WORK_PER_ROW, CheckedContractAliases};
mod account_rekeys;
pub(super) use account_rekeys::CheckedAccountRekeys;
mod validation_fee_proposals;
#[cfg(test)]
pub(in crate::state) use validation_fee_proposals::test_support as validation_fee_proposal_test_support;
pub(in crate::state) use validation_fee_proposals::validate_original_validation_fee_proposals;
pub(super) use validation_fee_proposals::{
    CheckedValidationFeeProposals, VALIDATION_FEE_PROPOSAL_WORK_PER_ROW,
};
mod proof_status;
pub(super) use proof_status::CheckedProofRecords;
pub(in crate::state) use proof_status::validate_original_proofs;
mod contract_subjects;
pub(super) use contract_subjects::CheckedContractSubjects;
pub(in crate::state) use contract_subjects::validate_original_contract_subjects;
mod verifying_keys;
pub(super) use verifying_keys::CheckedVerifyingKeys;

/// Original native image whose exact grouping is being checked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupImage {
    /// Published rows.
    Current,
    /// Exact logical predecessor reconstructed from original undo entries.
    Predecessor,
}

/// Failure of one exact grouping relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GroupMismatch {
    /// A canonical source row is missing from its expected group.
    MissingMember,
    /// A materialized group has no members.
    EmptyGroup,
    /// A group contains an absent source or a member belonging to another group.
    ForeignMember,
}

/// Local work/publication refusals remain distinct from inconsistent indexes.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum GroupedOwnershipError {
    /// Retry locally with sufficient admitted work; no validity verdict follows.
    #[error("grouped ownership capture exceeds its local work bound")]
    WorkLimit,
    /// Canonical rows violate a reference required by their derived projection.
    #[error("grouped ownership source {table} in {image:?}: {reason}")]
    Source {
        /// Canonical inventory identity of the source.
        table: &'static str,
        /// Failed current or predecessor image.
        image: GroupImage,
        /// Fixed allocation-free explanation of the invalid source reference.
        reason: &'static str,
    },
    /// The original source and derived membership disagree.
    #[error("grouped ownership mismatch for {index} in {image:?}: {mismatch:?}")]
    Corrupt {
        /// Canonical inventory identity of the derived index.
        index: &'static str,
        /// Failed current or predecessor image.
        image: GroupImage,
        /// Failed exact relation.
        mismatch: GroupMismatch,
    },
    /// Preserve the actual native publication refusal or identity change.
    #[error("grouped ownership source could not be retained: {0:?}")]
    Publication(PublicationPreparationError<Infallible>),
}

impl From<PublicationPreparationError<Infallible>> for GroupedOwnershipError {
    fn from(error: PublicationPreparationError<Infallible>) -> Self {
        Self::Publication(error)
    }
}

struct Work(u64);

impl Work {
    fn charge(&mut self) -> Result<(), GroupedOwnershipError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(GroupedOwnershipError::WorkLimit)?;
        Ok(())
    }
}

fn get_at<'a, K: mv::Key, V: mv::Value>(
    view: &'a CommittedStorageView<'_, K, V>,
    image: GroupImage,
    key: &K,
) -> Option<&'a V> {
    if image == GroupImage::Predecessor {
        if let Some(prior) = view.undo().get(key) {
            return prior.as_ref();
        }
    }
    view.current().get(key)
}

/// Visit physical rows before filtering: absent undo entries still consume work.
fn visit_image<K: mv::Key, V: mv::Value>(
    view: &CommittedStorageView<'_, K, V>,
    image: GroupImage,
    work: &mut Work,
    mut visit: impl FnMut(&K, &V, &mut Work) -> Result<(), GroupedOwnershipError>,
) -> Result<(), GroupedOwnershipError> {
    for (key, value) in view.current().iter() {
        work.charge()?;
        if image == GroupImage::Current || !view.undo().contains_key(key) {
            visit(key, value, work)?;
        }
    }
    if image == GroupImage::Predecessor {
        for (key, value) in view.undo().iter() {
            work.charge()?;
            if let Some(value) = value {
                visit(key, value, work)?;
            }
        }
    }
    Ok(())
}

mod nfts_rwas;
#[cfg(test)]
pub(in crate::state) use nfts_rwas::test_support as nft_rwa_test_support;
pub(super) use nfts_rwas::{CheckedNfts, CheckedRwas, NFT_WORK_PER_ROW, RWA_WORK_PER_ROW};
pub(in crate::state) use nfts_rwas::{validate_original_nfts, validate_original_rwas};

#[cfg(test)]
mod tests;
