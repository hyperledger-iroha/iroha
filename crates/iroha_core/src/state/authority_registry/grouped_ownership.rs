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
pub(super) use escrows::CheckedEscrows;
mod repo_agreements;
pub(super) use repo_agreements::CheckedRepoAgreements;
mod asset_definitions;
pub(super) use asset_definitions::CheckedAssetDefinitions;
mod assets;
mod confidential_policies;
pub(super) use assets::CheckedAssets;
mod contract_aliases;
pub(super) use contract_aliases::CheckedContractAliases;
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

fn validate_group<K: mv::Key, V: mv::Value, G: mv::Key>(
    source: &CommittedStorageView<'_, K, V>,
    groups: &CommittedStorageView<'_, G, BTreeSet<K>>,
    index: &'static str,
    project: impl for<'a> Fn(&'a K, &'a V) -> Option<&'a G>,
    work: &mut Work,
) -> Result<(), GroupedOwnershipError> {
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
            index,
            image,
            mismatch,
        };
        visit_image(source, image, work, |key, value, _| {
            if let Some(group) = project(key, value)
                && !get_at(groups, image, group).is_some_and(|members| members.contains(key))
            {
                return Err(corrupt(GroupMismatch::MissingMember));
            }
            Ok(())
        })?;
        visit_image(groups, image, work, |group, members, work| {
            if members.is_empty() {
                return Err(corrupt(GroupMismatch::EmptyGroup));
            }
            for key in members {
                work.charge()?;
                if !get_at(source, image, key)
                    .is_some_and(|value| project(key, value) == Some(group))
                {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

/// Checked NFT rows and both exact derived indexes, retaining original readers.
pub(super) struct CheckedNfts<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, NftId, NftValue>,
    owners: CommittedStorageView<'world, AccountId, BTreeSet<NftId>>,
    domains: CommittedStorageView<'world, DomainId, BTreeSet<NftId>>,
}

impl<'world> CheckedNfts<'world> {
    /// Check both current and predecessor grouping without allocating or repairs.
    pub(super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world.nfts.try_committed_view_nonblocking()?,
            owners: world.nfts_by_owner.try_committed_view_nonblocking()?,
            domains: world.nfts_by_domain.try_committed_view_nonblocking()?,
        };
        let mut work = Work(max_work);
        let result = validate_group(
            &checked.rows,
            &checked.owners,
            "world.nfts_by_owner",
            |_, nft| Some(&nft.owned_by),
            &mut work,
        )
        .and_then(|()| {
            validate_group(
                &checked.rows,
                &checked.domains,
                "world.nfts_by_domain",
                |id, _| Some(id.domain()),
                &mut work,
            )
        });
        // A publication race is never reported as stable semantic corruption.
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    /// Borrow the exact canonical rows whose indexes passed the checks.
    pub(super) fn rows(&self) -> &CommittedStorageView<'world, NftId, NftValue> {
        &self.rows
    }

    /// Verify all original native owners without replacing any reader.
    pub(super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        Ok(self.rows.try_matches_current(&self.world.nfts)?
            && self.owners.try_matches_current(&self.world.nfts_by_owner)?
            && self
                .domains
                .try_matches_current(&self.world.nfts_by_domain)?)
    }
}

/// Checked RWA rows and exact owner, status and frozen-state indexes.
pub(super) struct CheckedRwas<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, RwaId, RwaValue>,
    owners: CommittedStorageView<'world, AccountId, BTreeSet<RwaId>>,
    statuses: CommittedStorageView<'world, Option<Name>, BTreeSet<RwaId>>,
    frozen: CommittedStorageView<'world, bool, BTreeSet<RwaId>>,
}

impl<'world> CheckedRwas<'world> {
    /// Check complete native images, charging before every row/member inspection.
    pub(super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self {
            world,
            rows: world.rwas.try_committed_view_nonblocking()?,
            owners: world.rwas_by_owner.try_committed_view_nonblocking()?,
            statuses: world.rwas_by_status.try_committed_view_nonblocking()?,
            frozen: world.rwas_by_frozen.try_committed_view_nonblocking()?,
        };
        let mut work = Work(max_work);
        let result = validate_group(
            &checked.rows,
            &checked.owners,
            "world.rwas_by_owner",
            |_, rwa| Some(&rwa.owned_by),
            &mut work,
        )
        .and_then(|()| {
            validate_group(
                &checked.rows,
                &checked.statuses,
                "world.rwas_by_status",
                |_, rwa| Some(&rwa.status),
                &mut work,
            )
        })
        .and_then(|()| {
            validate_group(
                &checked.rows,
                &checked.frozen,
                "world.rwas_by_frozen",
                |_, rwa| Some(&rwa.is_frozen),
                &mut work,
            )
        });
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    /// Borrow the original canonical rows checked against all three indexes.
    pub(super) fn rows(&self) -> &CommittedStorageView<'world, RwaId, RwaValue> {
        &self.rows
    }

    /// Check original source/index identities through final row encoding.
    pub(super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        Ok(self.rows.try_matches_current(&self.world.rwas)?
            && self.owners.try_matches_current(&self.world.rwas_by_owner)?
            && self
                .statuses
                .try_matches_current(&self.world.rwas_by_status)?
            && self
                .frozen
                .try_matches_current(&self.world.rwas_by_frozen)?)
    }
}

#[cfg(test)]
mod tests;
