//! Exact identity indexes checked against their original retained account images.
//!
//! Identifiers are stored protocol values; this owner imposes no AccountId hash
//! relation. The committed and frozen owners use one complete prepaid relation.
//! TODO: consume every checked authority through complete State/Kura
//! publication before supplying finalized execution anchors.

use super::{borrowed_controller_work::prepay_account_id, original_images::RawStorageImages};
use crate::state::World;
use iroha_crypto::Hash;
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

/// Local reference for one Single Ed25519 account, one UAID and one opaque id.
/// Both original images cost 2 * (264 + 134 + 200) without undo. Wider keys,
/// longer member vectors and quadratic cuts may need a larger local allowance;
/// this is never a controller, row, gas, ledger or identifier validity limit.
pub(super) const ACCOUNT_IDENTITY_WORK_PER_ROW: u64 = 2 * (264 + 134 + 200);

struct Work(u64);
impl Work {
    fn prepay(&mut self, amount: usize) -> Result<(), IdentityOwnershipError> {
        let amount = u64::try_from(amount).map_err(|_| IdentityOwnershipError::WorkLimit)?;
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or(IdentityOwnershipError::WorkLimit)?;
        Ok(())
    }
}

fn corrupt(image: IdentityImage, mismatch: IdentityMismatch) -> IdentityOwnershipError {
    IdentityOwnershipError::Corrupt { image, mismatch }
}

trait WorkKey: mv::Key {
    fn prepay(&self, work: &mut Work) -> Result<(), IdentityOwnershipError>;
}
impl WorkKey for AccountId {
    fn prepay(&self, work: &mut Work) -> Result<(), IdentityOwnershipError> {
        prepay_account_id(self, |amount| work.prepay(amount))
    }
}
impl WorkKey for UniversalAccountId {
    fn prepay(&self, work: &mut Work) -> Result<(), IdentityOwnershipError> {
        work.prepay(Hash::LENGTH)
    }
}
impl WorkKey for OpaqueAccountId {
    fn prepay(&self, work: &mut Work) -> Result<(), IdentityOwnershipError> {
        work.prepay(Hash::LENGTH)
    }
}
fn equal<K: WorkKey>(left: &K, right: &K, work: &mut Work) -> Result<bool, IdentityOwnershipError> {
    left.prepay(work)?;
    right.prepay(work)?;
    Ok(left == right)
}

fn next_physical<I: ExactSizeIterator>(
    rows: &mut I,
    work: &mut Work,
) -> Result<Option<I::Item>, IdentityOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    work.prepay(1)?;
    Ok(rows.next())
}

fn visit_original<'a, K: WorkKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: IdentityImage,
    work: &mut Work,
    mut inspect: impl FnMut(&'a K, &'a V, &mut Work) -> Result<(), IdentityOwnershipError>,
) -> Result<(), IdentityOwnershipError> {
    let mut current = rows.current_entries();
    while let Some((key, value)) = next_physical(&mut current, work)? {
        let mut masked = false;
        if image == IdentityImage::Predecessor {
            let mut undo = rows.undo_entries();
            while let Some((prior_key, _)) = next_physical(&mut undo, work)? {
                masked |= equal(key, prior_key, work)?;
            }
        }
        if !masked {
            inspect(key, value, work)?;
        }
    }
    if image == IdentityImage::Predecessor {
        let mut undo = rows.undo_entries();
        while let Some((key, prior)) = next_physical(&mut undo, work)? {
            if let Some(value) = prior {
                inspect(key, value, work)?;
            }
        }
    }
    Ok(())
}

fn lookup_original<'a, K: WorkKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: IdentityImage,
    key: &K,
    work: &mut Work,
) -> Result<Option<&'a V>, IdentityOwnershipError> {
    let mut found = None;
    visit_original(rows, image, work, |candidate, value, work| {
        if equal(candidate, key, work)? {
            found = Some(value);
        }
        Ok(())
    })?;
    Ok(found)
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
        let result = validate_original_account_identities(
            &checked.accounts,
            &checked.uaids,
            &checked.opaques,
            max_work,
        );
        checked.finish_validation(result)
    }

    fn finish_validation(
        self,
        result: Result<(), IdentityOwnershipError>,
    ) -> Result<Self, IdentityOwnershipError> {
        if !self.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(self)
    }

    /// Borrow accounts without reacquiring or reconstructing the checked image.
    pub(super) fn accounts(&self) -> &CommittedStorageView<'world, AccountId, AccountValue> {
        &self.accounts
    }

    /// Evaluate every original identity before exposing any validation outcome.
    pub(super) fn matches_current(&self) -> Result<bool, IdentityOwnershipError> {
        let accounts = self.accounts.try_matches_current(&self.world.accounts);
        let uaids = self.uaids.try_matches_current(&self.world.uaid_accounts);
        let opaques = self.opaques.try_matches_current(&self.world.opaque_uaids);
        let accounts = accounts?;
        let uaids = uaids?;
        let opaques = opaques?;
        Ok(accounts && uaids && opaques)
    }
}

/// Check the complete account/UAID/opaque inverse relation on both original images.
///
/// Sealed inputs retain only committed or actual frozen native maps. Every
/// physical advance and complete key comparison is prepaid, including absent,
/// masked and equal-value preimages. No account hash relation, controller
/// validity, alias/metadata rule, execution authority or source repair is added.
pub(in crate::state) fn validate_original_account_identities(
    accounts: &impl RawStorageImages<AccountId, AccountValue>,
    uaids: &impl RawStorageImages<UniversalAccountId, AccountId>,
    opaques: &impl RawStorageImages<OpaqueAccountId, UniversalAccountId>,
    max_work: u64,
) -> Result<(), IdentityOwnershipError> {
    let mut work = Work(max_work);
    for image in [IdentityImage::Current, IdentityImage::Predecessor] {
        let mut source_members = 0_u64;
        visit_original(accounts, image, &mut work, |account, value, work| {
            let details = value.as_ref();
            let Some(uaid) = details.uaid() else {
                return if details.opaque_ids().is_empty() {
                    Ok(())
                } else {
                    Err(corrupt(image, IdentityMismatch::OpaqueWithoutUaid))
                };
            };
            let Some(bound) = lookup_original(uaids, image, uaid, work)? else {
                return Err(corrupt(image, IdentityMismatch::UaidBinding));
            };
            if !equal(bound, account, work)? {
                return Err(corrupt(image, IdentityMismatch::UaidBinding));
            }
            let mut members = details.opaque_ids().iter();
            while let Some(opaque) = next_physical(&mut members, work)? {
                source_members = source_members
                    .checked_add(1)
                    .ok_or(IdentityOwnershipError::WorkLimit)?;
                let Some(bound) = lookup_original(opaques, image, opaque, work)? else {
                    return Err(corrupt(image, IdentityMismatch::OpaqueBinding));
                };
                if !equal(bound, uaid, work)? {
                    return Err(corrupt(image, IdentityMismatch::OpaqueBinding));
                }
            }
            Ok(())
        })?;
        visit_original(uaids, image, &mut work, |uaid, account, work| {
            let Some(value) = lookup_original(accounts, image, account, work)? else {
                return Err(corrupt(image, IdentityMismatch::ForeignUaid));
            };
            let Some(bound) = value.as_ref().uaid() else {
                return Err(corrupt(image, IdentityMismatch::ForeignUaid));
            };
            if !equal(bound, uaid, work)? {
                return Err(corrupt(image, IdentityMismatch::ForeignUaid));
            }
            Ok(())
        })?;
        let mut index_members = 0_u64;
        visit_original(opaques, image, &mut work, |opaque, uaid, work| {
            index_members = index_members
                .checked_add(1)
                .ok_or(IdentityOwnershipError::WorkLimit)?;
            let Some(account) = lookup_original(uaids, image, uaid, work)? else {
                return Err(corrupt(image, IdentityMismatch::ForeignOpaque));
            };
            let Some(value) = lookup_original(accounts, image, account, work)? else {
                return Err(corrupt(image, IdentityMismatch::ForeignOpaque));
            };
            let mut found = false;
            let mut members = value.as_ref().opaque_ids().iter();
            while let Some(member) = next_physical(&mut members, work)? {
                found |= equal(member, opaque, work)?;
            }
            if !found {
                return Err(corrupt(image, IdentityMismatch::ForeignOpaque));
            }
            Ok(())
        })?;
        // Complete inverse directions and unique UAIDs make unequal cardinality
        // an exact duplicate-source proof, without an allocated scratch set.
        if source_members != index_members {
            return Err(corrupt(image, IdentityMismatch::DuplicateOpaque));
        }
    }
    Ok(())
}

#[cfg(test)]
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

#[cfg(test)]
#[path = "account_identity_ownership/test_support.rs"]
pub(in crate::state) mod test_support;

#[cfg(test)]
#[path = "account_identity_ownership/tests.rs"]
mod tests;
