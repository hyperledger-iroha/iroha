//! Exact, bounded domain-owner derivation over retained current and undo maps.
//!
//! The checked owner exposes only its original domains reader. Neither a caller
//! assertion nor a freshly acquired table can substitute for the checked cut.
//! This is a scoped capture foundation, not finalized State authority.
//! TODO: consume all derived-owner checks in complete State/Kura publication.

use crate::state::World;
use iroha_data_model::{account::AccountId, domain::Domain};
use iroha_model_base::domain::DomainId;
use mv::{
    PublicationPreparationError,
    storage::{CommittedStorageView, StorageReadOnly},
};
use std::{collections::BTreeSet, convert::Infallible};

/// The image in which a secondary index differs from its canonical source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnershipImage {
    /// Published current rows.
    Current,
    /// Rows reconstructed from the exact original undo map.
    Predecessor,
}

/// The exact relation violated by one retained owner index.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OwnershipMismatch {
    /// A canonical domain is absent from its owner's bucket.
    MissingDomain,
    /// A bucket has no logical members and should not exist.
    EmptyBucket,
    /// A bucket member is absent from canonical domains or belongs elsewhere.
    ForeignDomain,
}

/// Local traversal refusal and source corruption are separate outcomes.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub(crate) enum DomainOwnershipError {
    /// Exhaustion grants no validity verdict; retry with an admitted larger bound.
    #[error("domain-owner derivation exceeds its local work bound")]
    WorkLimit,
    /// The retained source and derived image disagree.
    #[error("domain-owner derivation mismatch in {image:?}: {mismatch:?}")]
    Corrupt {
        /// Current or predecessor cut.
        image: OwnershipImage,
        /// Failed exact relation.
        mismatch: OwnershipMismatch,
    },
    /// Preserve the actual owner release source, change, or poison condition.
    #[error("domain-owner source publication could not be retained: {0:?}")]
    Publication(PublicationPreparationError<Infallible>),
}

impl From<PublicationPreparationError<Infallible>> for DomainOwnershipError {
    fn from(error: PublicationPreparationError<Infallible>) -> Self {
        Self::Publication(error)
    }
}

struct Work(u64);

impl Work {
    fn charge(&mut self) -> Result<(), DomainOwnershipError> {
        self.0 = self
            .0
            .checked_sub(1)
            .ok_or(DomainOwnershipError::WorkLimit)?;
        Ok(())
    }
}

/// Non-forgeable result retaining the exact native readers used by the check.
pub(super) struct CheckedDomainOwnership<'world> {
    world: &'world World,
    domains: CommittedStorageView<'world, DomainId, Domain>,
    owners: CommittedStorageView<'world, AccountId, BTreeSet<DomainId>>,
}

impl<'world> CheckedDomainOwnership<'world> {
    /// Check both native images without allocating, cloning rows or editing undo.
    /// Every visited physical row (including absent preimages) and index member
    /// consumes work before its contents are inspected. Native tree lookups use
    /// bounded key comparisons and stack-only traversal.
    pub(super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, DomainOwnershipError> {
        let checked = Self {
            world,
            domains: world.domains.try_committed_view_nonblocking()?,
            owners: world.domains_by_owner.try_committed_view_nonblocking()?,
        };
        let result = checked.validate(&mut Work(max_work));
        // A mixed observation caused by publication is not index corruption.
        if !checked.matches_current()? {
            return Err(DomainOwnershipError::Publication(
                PublicationPreparationError::Changed,
            ));
        }
        result?;
        Ok(checked)
    }

    /// Borrow the original canonical rows that passed this exact derivation.
    pub(super) fn domains(&self) -> &CommittedStorageView<'world, DomainId, Domain> {
        &self.domains
    }

    /// Check original native identities without reacquiring or substituting rows.
    pub(super) fn matches_current(&self) -> Result<bool, DomainOwnershipError> {
        Ok(self.domains.try_matches_current(&self.world.domains)?
            && self
                .owners
                .try_matches_current(&self.world.domains_by_owner)?)
    }

    fn domain_member(
        &self,
        image: OwnershipImage,
        id: &DomainId,
        domain: &Domain,
    ) -> Result<(), DomainOwnershipError> {
        let members = match image {
            OwnershipImage::Current => self.owners.get(domain.owned_by()),
            OwnershipImage::Predecessor => before(&self.owners, domain.owned_by()),
        };
        if members.is_some_and(|members| members.contains(id)) {
            Ok(())
        } else {
            Err(DomainOwnershipError::Corrupt {
                image,
                mismatch: OwnershipMismatch::MissingDomain,
            })
        }
    }

    fn owner_bucket(
        &self,
        image: OwnershipImage,
        owner: &AccountId,
        members: &BTreeSet<DomainId>,
        work: &mut Work,
    ) -> Result<(), DomainOwnershipError> {
        if members.is_empty() {
            return Err(DomainOwnershipError::Corrupt {
                image,
                mismatch: OwnershipMismatch::EmptyBucket,
            });
        }
        for id in members {
            work.charge()?;
            let domain = match image {
                OwnershipImage::Current => self.domains.get(id),
                OwnershipImage::Predecessor => before(&self.domains, id),
            };
            if !domain.is_some_and(|domain| domain.owned_by() == owner) {
                return Err(DomainOwnershipError::Corrupt {
                    image,
                    mismatch: OwnershipMismatch::ForeignDomain,
                });
            }
        }
        Ok(())
    }

    fn validate(&self, work: &mut Work) -> Result<(), DomainOwnershipError> {
        for (id, domain) in self.domains.current().iter() {
            work.charge()?;
            self.domain_member(OwnershipImage::Current, id, domain)?;
        }
        for (owner, members) in self.owners.current().iter() {
            work.charge()?;
            self.owner_bucket(OwnershipImage::Current, owner, members, work)?;
        }
        // Traverse physical maps rather than a filtering predecessor iterator:
        // insertions, deletions and redundant absent preimages must all be charged.
        for (id, domain) in self.domains.current().iter() {
            work.charge()?;
            if !self.domains.undo().contains_key(id) {
                self.domain_member(OwnershipImage::Predecessor, id, domain)?;
            }
        }
        for (id, prior) in self.domains.undo().iter() {
            work.charge()?;
            if let Some(domain) = prior {
                self.domain_member(OwnershipImage::Predecessor, id, domain)?;
            }
        }
        for (owner, members) in self.owners.current().iter() {
            work.charge()?;
            if !self.owners.undo().contains_key(owner) {
                self.owner_bucket(OwnershipImage::Predecessor, owner, members, work)?;
            }
        }
        for (owner, prior) in self.owners.undo().iter() {
            work.charge()?;
            if let Some(members) = prior {
                self.owner_bucket(OwnershipImage::Predecessor, owner, members, work)?;
            }
        }
        Ok(())
    }
}

fn before<'a, K: mv::Key, V: mv::Value>(
    view: &'a CommittedStorageView<'_, K, V>,
    key: &K,
) -> Option<&'a V> {
    match view.undo().get(key) {
        Some(prior) => prior.as_ref(),
        None => view.current().get(key),
    }
}

#[cfg(test)]
#[path = "domain_ownership/tests.rs"]
mod tests;
