//! Bounded alias/index consistency over the same retained native current/undo cut.
//!
//! This checks account existence, primary labels and exact reverse membership
//! without rebuilding live State. TODO: consume every source check through the
//! complete State/Kura publication owner before granting finalized authority.

use super::{borrowed_controller_work::prepay_account_id, original_images::RawStorageImages};
use crate::state::{World, account_label_is_pii};
use iroha_data_model::account::{AccountAlias, AccountId, AccountValue};
#[cfg(test)]
use mv::storage::StorageReadOnly;
use mv::{PublicationPreparationError, storage::CommittedStorageView};
use std::{collections::BTreeSet, convert::Infallible};

/// Which exact logical native image failed validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AliasImage {
    /// Current committed rows.
    Current,
    /// Original predecessor reconstructed without changing undo.
    Predecessor,
}

/// The violated relation; errors carry no account labels or private row values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AliasMismatch {
    /// An authoritative alias points to an absent account.
    MissingAccount,
    /// A primary label is absent or belongs to another account.
    PrimaryLabel,
    /// An authoritative label violates the existing raw-PII restriction.
    PrivateLabel,
    /// An authoritative alias is absent from its reverse bucket.
    MissingAlias,
    /// A reverse member is absent or belongs to another account.
    ForeignAlias,
    /// An empty reverse bucket has no authoritative source.
    EmptyBucket,
}

/// Local scheduling refusal remains distinct from inconsistent canonical sources.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum AliasOwnershipError {
    /// Retry with enough admitted work; no validity decision was made.
    #[error("account-alias derivation exceeds its local work bound")]
    WorkLimit,
    /// A retained source or reverse index is inconsistent.
    #[error("account-alias derivation mismatch in {image:?}: {mismatch:?}")]
    Corrupt {
        /// Current or predecessor cut.
        image: AliasImage,
        /// Exact failed source relation.
        mismatch: AliasMismatch,
    },
    /// Original publication contention, poisoning or overlap.
    #[error("account-alias publication could not be retained: {0:?}")]
    Publication(PublicationPreparationError<Infallible>),
}

impl From<PublicationPreparationError<Infallible>> for AliasOwnershipError {
    fn from(error: PublicationPreparationError<Infallible>) -> Self {
        Self::Publication(error)
    }
}

/// Local full-geometry reference for one Single Ed25519 account, one 255-byte
/// primary label with a 255-byte alias-domain segment, and its reverse member.
/// Both images cost 2 * (1364 + 1433 + 1110) without undo. Wider controllers,
/// extra implicit accounts and quadratic physical cuts may require more local
/// work; this is never a name, controller, row, gas or ledger validity limit.
pub(super) const ACCOUNT_ALIAS_WORK_PER_ROW: u64 = 2 * (1364 + 1433 + 1110);

struct Work(u64);
impl Work {
    fn prepay(&mut self, amount: usize) -> Result<(), AliasOwnershipError> {
        let amount = u64::try_from(amount).map_err(|_| AliasOwnershipError::WorkLimit)?;
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or(AliasOwnershipError::WorkLimit)?;
        Ok(())
    }
}
fn corrupt(image: AliasImage, mismatch: AliasMismatch) -> AliasOwnershipError {
    AliasOwnershipError::Corrupt { image, mismatch }
}
trait WorkKey: mv::Key {
    fn prepay(&self, work: &mut Work) -> Result<(), AliasOwnershipError>;
}
impl WorkKey for AccountId {
    fn prepay(&self, work: &mut Work) -> Result<(), AliasOwnershipError> {
        prepay_account_id(self, |amount| work.prepay(amount))
    }
}
impl WorkKey for AccountAlias {
    fn prepay(&self, work: &mut Work) -> Result<(), AliasOwnershipError> {
        work.prepay(self.label.as_ref().len())?;
        work.prepay(1)?;
        if let Some(domain) = &self.domain {
            work.prepay(domain.name().as_ref().len())?;
        }
        work.prepay(8)
    }
}
fn equal<K: WorkKey>(left: &K, right: &K, work: &mut Work) -> Result<bool, AliasOwnershipError> {
    left.prepay(work)?;
    right.prepay(work)?;
    Ok(left == right)
}
fn next_physical<I: ExactSizeIterator>(
    rows: &mut I,
    work: &mut Work,
) -> Result<Option<I::Item>, AliasOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    work.prepay(1)?;
    Ok(rows.next())
}
fn visit_original<'a, K: WorkKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: AliasImage,
    work: &mut Work,
    mut inspect: impl FnMut(&'a K, &'a V, &mut Work) -> Result<(), AliasOwnershipError>,
) -> Result<(), AliasOwnershipError> {
    let mut current = rows.current_entries();
    while let Some((key, value)) = next_physical(&mut current, work)? {
        let mut masked = false;
        if image == AliasImage::Predecessor {
            let mut undo = rows.undo_entries();
            while let Some((prior_key, _)) = next_physical(&mut undo, work)? {
                masked |= equal(key, prior_key, work)?;
            }
        }
        if !masked {
            inspect(key, value, work)?;
        }
    }
    if image == AliasImage::Predecessor {
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
    image: AliasImage,
    key: &K,
    work: &mut Work,
) -> Result<Option<&'a V>, AliasOwnershipError> {
    let mut found = None;
    visit_original(rows, image, work, |candidate, value, work| {
        if equal(candidate, key, work)? {
            found = Some(value);
        }
        Ok(())
    })?;
    Ok(found)
}
fn contains_original(
    members: &BTreeSet<AccountAlias>,
    label: &AccountAlias,
    work: &mut Work,
) -> Result<bool, AliasOwnershipError> {
    let mut found = false;
    let mut members = members.iter();
    while let Some(member) = next_physical(&mut members, work)? {
        found |= equal(member, label, work)?;
    }
    Ok(found)
}

/// Original canonical reader retained with all sources used to check its index.
pub(super) struct CheckedAccountAliases<'world> {
    world: &'world World,
    accounts: CommittedStorageView<'world, AccountId, AccountValue>,
    aliases: CommittedStorageView<'world, AccountAlias, AccountId>,
    reverse: CommittedStorageView<'world, AccountId, BTreeSet<AccountAlias>>,
}

impl<'world> CheckedAccountAliases<'world> {
    /// Capture and check both logical images without allocation or row cloning.
    pub(super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, AliasOwnershipError> {
        let checked = Self {
            world,
            accounts: world.accounts.try_committed_view_nonblocking()?,
            aliases: world.account_aliases.try_committed_view_nonblocking()?,
            reverse: world
                .account_aliases_by_account
                .try_committed_view_nonblocking()?,
        };
        let result = validate_original_account_aliases(
            &checked.accounts,
            &checked.aliases,
            &checked.reverse,
            max_work,
        );
        checked.finish_validation(result)
    }

    fn finish_validation(
        self,
        result: Result<(), AliasOwnershipError>,
    ) -> Result<Self, AliasOwnershipError> {
        if !self.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(self)
    }

    /// Borrow the exact alias rows whose dependencies passed validation.
    pub(super) fn aliases(&self) -> &CommittedStorageView<'world, AccountAlias, AccountId> {
        &self.aliases
    }

    /// Detect any publication, including an equal-value replacement of a source.
    pub(super) fn matches_current(&self) -> Result<bool, AliasOwnershipError> {
        let accounts = self.accounts.try_matches_current(&self.world.accounts);
        let aliases = self
            .aliases
            .try_matches_current(&self.world.account_aliases);
        let reverse = self
            .reverse
            .try_matches_current(&self.world.account_aliases_by_account);
        let accounts = accounts?;
        let aliases = aliases?;
        let reverse = reverse?;
        Ok(accounts && aliases && reverse)
    }
}

/// Validate both exact alias images on only the closed original native maps.
///
/// Preserve Current accounts/aliases/reverse then Predecessor pass ordering and
/// all existing PII, primary-label, account and reverse-bucket predicates. Every
/// physical advance, full key comparison and PII scan is prepaid. No name or
/// controller is parsed, cloned or given a new validity/authority predicate.
pub(in crate::state) fn validate_original_account_aliases(
    accounts: &impl RawStorageImages<AccountId, AccountValue>,
    aliases: &impl RawStorageImages<AccountAlias, AccountId>,
    reverse: &impl RawStorageImages<AccountId, BTreeSet<AccountAlias>>,
    max_work: u64,
) -> Result<(), AliasOwnershipError> {
    let mut work = Work(max_work);
    for image in [AliasImage::Current, AliasImage::Predecessor] {
        visit_original(accounts, image, &mut work, |account, value, work| {
            work.prepay(1)?;
            if let Some(label) = value.as_ref().label() {
                work.prepay(label.label.as_ref().len())?;
                if account_label_is_pii(label) {
                    return Err(corrupt(image, AliasMismatch::PrivateLabel));
                }
                let Some(bound) = lookup_original(aliases, image, label, work)? else {
                    return Err(corrupt(image, AliasMismatch::PrimaryLabel));
                };
                if !equal(bound, account, work)? {
                    return Err(corrupt(image, AliasMismatch::PrimaryLabel));
                }
            }
            Ok(())
        })?;
        visit_original(aliases, image, &mut work, |label, account, work| {
            work.prepay(label.label.as_ref().len())?;
            if account_label_is_pii(label) {
                return Err(corrupt(image, AliasMismatch::PrivateLabel));
            }
            if lookup_original(accounts, image, account, work)?.is_none() {
                return Err(corrupt(image, AliasMismatch::MissingAccount));
            }
            let Some(members) = lookup_original(reverse, image, account, work)? else {
                return Err(corrupt(image, AliasMismatch::MissingAlias));
            };
            if !contains_original(members, label, work)? {
                return Err(corrupt(image, AliasMismatch::MissingAlias));
            }
            Ok(())
        })?;
        visit_original(reverse, image, &mut work, |account, members, work| {
            work.prepay(1)?;
            if members.is_empty() {
                return Err(corrupt(image, AliasMismatch::EmptyBucket));
            }
            let mut members = members.iter();
            while let Some(label) = next_physical(&mut members, work)? {
                let Some(bound) = lookup_original(aliases, image, label, work)? else {
                    return Err(corrupt(image, AliasMismatch::ForeignAlias));
                };
                if !equal(bound, account, work)? {
                    return Err(corrupt(image, AliasMismatch::ForeignAlias));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
fn get_at<'a, K: mv::Key, V: mv::Value>(
    rows: &'a CommittedStorageView<'_, K, V>,
    image: AliasImage,
    key: &K,
) -> Option<&'a V> {
    if image == AliasImage::Predecessor
        && let Some(prior) = rows.undo().get(key)
    {
        return prior.as_ref();
    }
    rows.current().get(key)
}

#[cfg(test)]
#[path = "account_alias_ownership/test_support.rs"]
pub(in crate::state) mod test_support;

#[cfg(test)]
#[path = "account_alias_ownership/work_tests.rs"]
mod work_tests;

#[cfg(test)]
#[path = "account_alias_ownership/tests.rs"]
mod tests;
