//! Exact identity indexes checked against their original retained account images.
//!
//! Identifiers are stored protocol values; this owner imposes no AccountId hash
//! relation. TODO: consume every checked authority through complete State/Kura
//! publication before supplying finalized execution anchors.

use crate::state::World;
use iroha_data_model::{
    account::{AccountId, AccountValue, OpaqueAccountId},
    nexus::UniversalAccountId,
};
#[cfg(test)]
use mv::storage::StorageReadOnly;
use mv::{PublicationPreparationError, storage::CommittedStorageView};
use std::convert::Infallible;

/// The original logical image containing an inconsistent identity relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdentityImage {
    /// Current committed accounts and indexes.
    Current,
    /// Previous image reconstructed through each original undo map.
    Predecessor,
}

/// Failed identity relation, without account or identifier values in diagnostics.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IdentityMismatch {
    /// Opaque identifiers require an explicit universal identifier.
    OpaqueWithoutUaid,
    /// An account's universal identifier lacks its exact inverse row.
    UaidBinding,
    /// An account's opaque identifier lacks its exact inverse row.
    OpaqueBinding,
    /// A universal index row lacks a matching authoritative account.
    ForeignUaid,
    /// An opaque index row lacks a matching authoritative account member.
    ForeignOpaque,
    /// A source vector repeats an opaque identifier.
    DuplicateOpaque,
}

/// Operational refusal is distinct from malformed authoritative membership.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum IdentityOwnershipError {
    /// Retry with a larger locally admitted work allowance.
    #[error("account-identity derivation exceeds its local work bound")]
    WorkLimit,
    /// An exact source/index relation failed in a stable native image.
    #[error("account-identity derivation mismatch in {image:?}: {mismatch:?}")]
    Corrupt {
        /// Current or previous native image.
        image: IdentityImage,
        /// The violated relation.
        mismatch: IdentityMismatch,
    },
    /// Original source contention, poisoning or publication overlap.
    #[error("account-identity publication could not be retained: {0:?}")]
    Publication(PublicationPreparationError<Infallible>),
}

impl From<PublicationPreparationError<Infallible>> for IdentityOwnershipError {
    fn from(error: PublicationPreparationError<Infallible>) -> Self {
        Self::Publication(error)
    }
}

struct Work(u64);
impl Work {
    fn charge(&mut self) -> Result<(), IdentityOwnershipError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(IdentityOwnershipError::WorkLimit)?;
        Ok(())
    }
}

fn corrupt(image: IdentityImage, mismatch: IdentityMismatch) -> IdentityOwnershipError {
    IdentityOwnershipError::Corrupt { image, mismatch }
}

fn get_at<'a, K: mv::Key, V: mv::Value>(
    rows: &'a CommittedStorageView<'_, K, V>,
    image: IdentityImage,
    key: &K,
) -> Option<&'a V> {
    if image == IdentityImage::Predecessor
        && let Some(prior) = rows.undo().get(key)
    {
        return prior.as_ref();
    }
    rows.current().get(key)
}

fn visit<K: mv::Key, V: mv::Value>(
    rows: &CommittedStorageView<'_, K, V>,
    image: IdentityImage,
    work: &mut Work,
    mut inspect: impl FnMut(&K, &V, &mut Work) -> Result<(), IdentityOwnershipError>,
) -> Result<(), IdentityOwnershipError> {
    for (key, value) in rows.current().iter() {
        work.charge()?;
        if image == IdentityImage::Current || !rows.undo().contains_key(key) {
            inspect(key, value, work)?;
        }
    }
    if image == IdentityImage::Predecessor {
        for (key, prior) in rows.undo().iter() {
            // Even absent and redundant preimages consume native traversal work.
            work.charge()?;
            if let Some(value) = prior {
                inspect(key, value, work)?;
            }
        }
    }
    Ok(())
}

/// The same account reader that passed both derived identity checks.
pub(super) struct CheckedAccountIdentities<'world> {
    world: &'world World,
    accounts: CommittedStorageView<'world, AccountId, AccountValue>,
    uaids: CommittedStorageView<'world, UniversalAccountId, AccountId>,
    opaques: CommittedStorageView<'world, OpaqueAccountId, UniversalAccountId>,
}

impl<'world> CheckedAccountIdentities<'world> {
    /// Retain original native readers; neither allocate, repair nor clone rows.
    pub(super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, IdentityOwnershipError> {
        let checked = Self {
            world,
            accounts: world.accounts.try_committed_view_nonblocking()?,
            uaids: world.uaid_accounts.try_committed_view_nonblocking()?,
            opaques: world.opaque_uaids.try_committed_view_nonblocking()?,
        };
        let result = checked.validate(&mut Work(max_work));
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    /// Borrow accounts without reacquiring or reconstructing the checked image.
    pub(super) fn accounts(&self) -> &CommittedStorageView<'world, AccountId, AccountValue> {
        &self.accounts
    }

    /// Equal-value native writes also invalidate any retained checked image.
    pub(super) fn matches_current(&self) -> Result<bool, IdentityOwnershipError> {
        Ok(self.accounts.try_matches_current(&self.world.accounts)?
            && self.uaids.try_matches_current(&self.world.uaid_accounts)?
            && self.opaques.try_matches_current(&self.world.opaque_uaids)?)
    }

    fn validate(&self, work: &mut Work) -> Result<(), IdentityOwnershipError> {
        for image in [IdentityImage::Current, IdentityImage::Predecessor] {
            let mut source_members = 0_u64;
            visit(&self.accounts, image, work, |account, value, work| {
                let details = value.as_ref();
                let Some(uaid) = details.uaid() else {
                    return if details.opaque_ids().is_empty() {
                        Ok(())
                    } else {
                        Err(corrupt(image, IdentityMismatch::OpaqueWithoutUaid))
                    };
                };
                if get_at(&self.uaids, image, uaid) != Some(account) {
                    return Err(corrupt(image, IdentityMismatch::UaidBinding));
                }
                for opaque in details.opaque_ids() {
                    work.charge()?;
                    source_members = source_members
                        .checked_add(1)
                        .ok_or(IdentityOwnershipError::WorkLimit)?;
                    if get_at(&self.opaques, image, opaque) != Some(uaid) {
                        return Err(corrupt(image, IdentityMismatch::OpaqueBinding));
                    }
                }
                Ok(())
            })?;
            visit(&self.uaids, image, work, |uaid, account, _| {
                if get_at(&self.accounts, image, account).and_then(|value| value.as_ref().uaid())
                    != Some(uaid)
                {
                    return Err(corrupt(image, IdentityMismatch::ForeignUaid));
                }
                Ok(())
            })?;
            let mut index_members = 0_u64;
            visit(&self.opaques, image, work, |opaque, uaid, work| {
                index_members = index_members
                    .checked_add(1)
                    .ok_or(IdentityOwnershipError::WorkLimit)?;
                let account = get_at(&self.uaids, image, uaid)
                    .and_then(|account| get_at(&self.accounts, image, account))
                    .ok_or_else(|| corrupt(image, IdentityMismatch::ForeignOpaque))?;
                for member in account.as_ref().opaque_ids() {
                    work.charge()?;
                    if member == opaque {
                        return Ok(());
                    }
                }
                Err(corrupt(image, IdentityMismatch::ForeignOpaque))
            })?;
            // Both directions are complete and UAIDs are unique. Each index
            // row therefore corresponds to a distinct source member; unequal
            // cardinality now proves a duplicate without an allocated set.
            if source_members != index_members {
                return Err(corrupt(image, IdentityMismatch::DuplicateOpaque));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "account_identity_ownership/tests.rs"]
mod tests;
