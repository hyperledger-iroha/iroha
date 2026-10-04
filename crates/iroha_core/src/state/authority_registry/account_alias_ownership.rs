//! Bounded alias/index consistency over the same retained native current/undo cut.
//!
//! This checks account existence, primary labels and exact reverse membership
//! without rebuilding live State. TODO: consume every source check through the
//! complete State/Kura publication owner before granting finalized authority.

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

struct Work(u64);

impl Work {
    fn charge(&mut self) -> Result<(), AliasOwnershipError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(AliasOwnershipError::WorkLimit)?;
        Ok(())
    }
}

fn corrupt(image: AliasImage, mismatch: AliasMismatch) -> AliasOwnershipError {
    AliasOwnershipError::Corrupt { image, mismatch }
}

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

fn visit<K: mv::Key, V: mv::Value>(
    rows: &CommittedStorageView<'_, K, V>,
    image: AliasImage,
    work: &mut Work,
    mut inspect: impl FnMut(&K, &V, &mut Work) -> Result<(), AliasOwnershipError>,
) -> Result<(), AliasOwnershipError> {
    for (key, value) in rows.current().iter() {
        work.charge()?;
        if image == AliasImage::Current || !rows.undo().contains_key(key) {
            inspect(key, value, work)?;
        }
    }
    if image == AliasImage::Predecessor {
        for (key, prior) in rows.undo().iter() {
            // Absent preimages and redundant touches also consume work.
            work.charge()?;
            if let Some(value) = prior {
                inspect(key, value, work)?;
            }
        }
    }
    Ok(())
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
        let result = checked.validate(&mut Work(max_work));
        if !checked.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(checked)
    }

    /// Borrow the exact alias rows whose dependencies passed validation.
    pub(super) fn aliases(&self) -> &CommittedStorageView<'world, AccountAlias, AccountId> {
        &self.aliases
    }

    /// Detect any publication, including an equal-value replacement of a source.
    pub(super) fn matches_current(&self) -> Result<bool, AliasOwnershipError> {
        Ok(self.accounts.try_matches_current(&self.world.accounts)?
            && self
                .aliases
                .try_matches_current(&self.world.account_aliases)?
            && self
                .reverse
                .try_matches_current(&self.world.account_aliases_by_account)?)
    }

    fn validate(&self, work: &mut Work) -> Result<(), AliasOwnershipError> {
        for image in [AliasImage::Current, AliasImage::Predecessor] {
            visit(&self.accounts, image, work, |account, value, _| {
                if let Some(label) = value.as_ref().label() {
                    if account_label_is_pii(label) {
                        return Err(corrupt(image, AliasMismatch::PrivateLabel));
                    }
                    if get_at(&self.aliases, image, label) != Some(account) {
                        return Err(corrupt(image, AliasMismatch::PrimaryLabel));
                    }
                }
                Ok(())
            })?;
            visit(&self.aliases, image, work, |label, account, _| {
                if account_label_is_pii(label) {
                    return Err(corrupt(image, AliasMismatch::PrivateLabel));
                }
                if get_at(&self.accounts, image, account).is_none() {
                    return Err(corrupt(image, AliasMismatch::MissingAccount));
                }
                if !get_at(&self.reverse, image, account)
                    .is_some_and(|members| members.contains(label))
                {
                    return Err(corrupt(image, AliasMismatch::MissingAlias));
                }
                Ok(())
            })?;
            visit(&self.reverse, image, work, |account, members, work| {
                if members.is_empty() {
                    return Err(corrupt(image, AliasMismatch::EmptyBucket));
                }
                for label in members {
                    work.charge()?;
                    if get_at(&self.aliases, image, label) != Some(account) {
                        return Err(corrupt(image, AliasMismatch::ForeignAlias));
                    }
                }
                Ok(())
            })?;
        }
        Ok(())
    }
}

#[cfg(test)]
#[path = "account_alias_ownership/tests.rs"]
mod tests;
