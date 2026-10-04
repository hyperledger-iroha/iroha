//! Exact, bounded domain-owner derivation over retained current and undo maps.
//!
//! The checked owner exposes only its original domains reader. Neither a caller
//! assertion nor a freshly acquired table can substitute for the checked cut.
//! This is a scoped capture foundation, not finalized State authority.
//! TODO: consume all derived-owner checks in complete State/Kura publication.

use super::original_images::RawStorageImages;
use crate::state::World;
use iroha_data_model::{account::AccountId, domain::Domain};
use iroha_model_base::domain::DomainId;
#[cfg(test)]
use mv::storage::StorageReadOnly;
use mv::{PublicationPreparationError, storage::CommittedStorageView};
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

/// Non-forgeable result retaining the exact native readers used by the check.
pub(super) struct CheckedDomainOwnership<'world> {
    world: &'world World,
    domains: CommittedStorageView<'world, DomainId, Domain>,
    owners: CommittedStorageView<'world, AccountId, BTreeSet<DomainId>>,
}

impl<'world> CheckedDomainOwnership<'world> {
    /// Check both native images without allocating, cloning rows or editing undo.
    /// Every physical row/member advance and complete borrowed key comparison
    /// is prepaid. The shared relation uses no tree lookup or allocating Ord.
    pub(super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, DomainOwnershipError> {
        let checked = Self {
            world,
            domains: world.domains.try_committed_view_nonblocking()?,
            owners: world.domains_by_owner.try_committed_view_nonblocking()?,
        };
        let result =
            validate_original_domain_ownership(&checked.domains, &checked.owners, max_work);
        checked.finish_validation(result)
    }

    fn finish_validation(
        self,
        result: Result<(), DomainOwnershipError>,
    ) -> Result<Self, DomainOwnershipError> {
        // A mixed observation caused by publication is not index corruption.
        if !self.matches_current()? {
            return Err(DomainOwnershipError::Publication(
                PublicationPreparationError::Changed,
            ));
        }
        result?;
        Ok(self)
    }

    /// Borrow the original canonical rows that passed this exact derivation.
    pub(super) fn domains(&self) -> &CommittedStorageView<'world, DomainId, Domain> {
        &self.domains
    }

    /// Check original native identities without reacquiring or substituting rows.
    pub(super) fn matches_current(&self) -> Result<bool, DomainOwnershipError> {
        let domains = self.domains.try_matches_current(&self.world.domains)?;
        let owners = self
            .owners
            .try_matches_current(&self.world.domains_by_owner)?;
        Ok(domains && owners)
    }
}

/// Local descriptor for one single Ed25519 owner at maximum existing domain text.
/// Wider controllers and quadratic cuts may require a larger admitted allowance;
/// this is never a controller, ledger row, gas or validity limit.
pub(super) const DOMAIN_OWNER_WORK_PER_ROW: u64 = 12 + 8 * (34 + 2 * 63);

struct Work(u64);
impl Work {
    fn prepay(&mut self, amount: usize) -> Result<(), DomainOwnershipError> {
        let amount = u64::try_from(amount).map_err(|_| DomainOwnershipError::WorkLimit)?;
        self.0 = self
            .0
            .checked_sub(amount)
            .ok_or(DomainOwnershipError::WorkLimit)?;
        Ok(())
    }
}

// Equality retains the exact stored relation, including malformed typed keys.
// It cannot construct the error String that AccountId/PublicKey Ord may create.
trait WorkKey: mv::Key {
    fn prepay(&self, work: &mut Work) -> Result<(), DomainOwnershipError>;
}
impl WorkKey for DomainId {
    fn prepay(&self, work: &mut Work) -> Result<(), DomainOwnershipError> {
        work.prepay(self.name().as_ref().len())?;
        work.prepay(self.dataspace().as_ref().len())
    }
}
impl WorkKey for AccountId {
    fn prepay(&self, work: &mut Work) -> Result<(), DomainOwnershipError> {
        super::borrowed_controller_work::prepay_account_id(self, |amount| work.prepay(amount))
    }
}

fn equal<K: WorkKey>(left: &K, right: &K, work: &mut Work) -> Result<bool, DomainOwnershipError> {
    left.prepay(work)?;
    right.prepay(work)?;
    Ok(left == right)
}

fn next_physical<I: ExactSizeIterator>(
    rows: &mut I,
    work: &mut Work,
) -> Result<Option<I::Item>, DomainOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    work.prepay(1)?;
    Ok(rows.next())
}

fn visit_original<'a, K: WorkKey, V: mv::Value>(
    rows: &'a impl RawStorageImages<K, V>,
    image: OwnershipImage,
    work: &mut Work,
    mut inspect: impl FnMut(&'a K, &'a V, &mut Work) -> Result<(), DomainOwnershipError>,
) -> Result<(), DomainOwnershipError> {
    let mut current = rows.current_entries();
    while let Some((key, value)) = next_physical(&mut current, work)? {
        let mut masked = false;
        if image == OwnershipImage::Predecessor {
            let mut undo = rows.undo_entries();
            while let Some((prior_key, _)) = next_physical(&mut undo, work)? {
                // Keep the complete scan: masked/no-op/absent candidates are funded.
                masked |= equal(key, prior_key, work)?;
            }
        }
        if !masked {
            inspect(key, value, work)?;
        }
    }
    if image == OwnershipImage::Predecessor {
        let mut undo = rows.undo_entries();
        while let Some((key, prior)) = next_physical(&mut undo, work)? {
            if let Some(value) = prior {
                inspect(key, value, work)?;
            }
        }
    }
    Ok(())
}

/// Check the complete original domain/bucket relation over both native images.
///
/// The sealed inputs are only retained committed or original frozen MV owners.
/// No account existence, embedded record-id or execution authority rule is added.
/// Work refusal grants no validity verdict; every advance/equality is prepaid.
pub(in crate::state) fn validate_original_domain_ownership(
    domains: &impl RawStorageImages<DomainId, Domain>,
    owners: &impl RawStorageImages<AccountId, BTreeSet<DomainId>>,
    max_work: u64,
) -> Result<(), DomainOwnershipError> {
    let mut work = Work(max_work);
    for image in [OwnershipImage::Current, OwnershipImage::Predecessor] {
        let corrupt = |mismatch| DomainOwnershipError::Corrupt { image, mismatch };
        visit_original(domains, image, &mut work, |id, domain, work| {
            let mut found = false;
            visit_original(owners, image, work, |owner, members, work| {
                let owner_matches = equal(domain.owned_by(), owner, work)?;
                let mut members = members.iter();
                while let Some(member) = next_physical(&mut members, work)? {
                    let domain_matches = equal(id, member, work)?;
                    found |= owner_matches && domain_matches;
                }
                Ok(())
            })?;
            if !found {
                return Err(corrupt(OwnershipMismatch::MissingDomain));
            }
            Ok(())
        })?;
        visit_original(owners, image, &mut work, |owner, members, work| {
            if members.is_empty() {
                return Err(corrupt(OwnershipMismatch::EmptyBucket));
            }
            let mut members = members.iter();
            while let Some(member) = next_physical(&mut members, work)? {
                let mut found = false;
                visit_original(domains, image, work, |id, domain, work| {
                    let domain_matches = equal(id, member, work)?;
                    let owner_matches = equal(domain.owned_by(), owner, work)?;
                    found |= domain_matches && owner_matches;
                    Ok(())
                })?;
                if !found {
                    return Err(corrupt(OwnershipMismatch::ForeignDomain));
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

#[cfg(test)]
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
#[path = "domain_ownership/test_support.rs"]
pub(in crate::state) mod test_support;

#[cfg(test)]
#[path = "domain_ownership/tests.rs"]
mod tests;
