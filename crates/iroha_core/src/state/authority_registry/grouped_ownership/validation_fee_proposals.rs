//! Exact validation-fee proposal lookup over both original native images.
//!
//! The canonical proposal capture consumes this retained owner directly.
//! This projection does not establish proposal admission,
//! Parliament status consistency, or finalized State authority.

use super::*;
use crate::state::{
    GovernanceProposalRecord, authority_registry::original_images::RawStorageImages,
};
use iroha_data_model::governance::types::ProposalKind;
use std::cmp::Ordering;

const INDEX: &str = "world.validation_fee_proposal_index";
const PROPOSAL_KEY_BYTES: u64 = 32;
const INDEX_KEY_BYTES: u64 = 8 + PROPOSAL_KEY_BYTES;

/// One complete indexed proposal, both images, with no physical undo entries.
/// Each image visits two outer rows and two inner rows, comparing both complete
/// 40-byte index keys at each inner visit. Larger/undo cuts can defer locally.
pub(in super::super) const VALIDATION_FEE_PROPOSAL_WORK_PER_ROW: u64 =
    2 * (2 + 2 * (1 + 2 * INDEX_KEY_BYTES));

/// Original canonical proposals and their exact fee lookup reader.
pub(in super::super) struct CheckedValidationFeeProposals<'world> {
    world: &'world World,
    rows: CommittedStorageView<'world, [u8; 32], GovernanceProposalRecord>,
    index: CommittedStorageView<'world, (u64, [u8; 32]), ()>,
}

impl<'world> CheckedValidationFeeProposals<'world> {
    /// Check both complete images without allocating source or projection scratch.
    pub(in super::super) fn capture(
        world: &'world World,
        max_work: u64,
    ) -> Result<Self, GroupedOwnershipError> {
        let checked = Self::retain(world)?;
        let result = checked.validate(&mut Work(max_work));
        checked.finish_validation(result)
    }

    fn retain(world: &'world World) -> Result<Self, GroupedOwnershipError> {
        Ok(Self {
            world,
            rows: world
                .governance_proposals
                .try_committed_view_nonblocking()?,
            index: world
                .validation_fee_proposal_index
                .try_committed_view_nonblocking()?,
        })
    }

    fn validate(&self, work: &mut Work) -> Result<(), GroupedOwnershipError> {
        validate(&self.rows, &self.index, work)
    }

    fn finish_validation(
        self,
        result: Result<(), GroupedOwnershipError>,
    ) -> Result<Self, GroupedOwnershipError> {
        // A mixed publication must not become stable corruption or a work verdict.
        if !self.matches_current()? {
            return Err(PublicationPreparationError::Changed.into());
        }
        result?;
        Ok(self)
    }

    /// Borrow exactly the canonical reader whose fee projection was checked.
    pub(in super::super) fn rows(
        &self,
    ) -> &CommittedStorageView<'world, [u8; 32], GovernanceProposalRecord> {
        &self.rows
    }

    /// Observe both original reader identities through validation and encoding.
    pub(in super::super) fn matches_current(&self) -> Result<bool, GroupedOwnershipError> {
        let rows_current = self
            .rows
            .try_matches_current(&self.world.governance_proposals)?;
        let index_current = self
            .index
            .try_matches_current(&self.world.validation_fee_proposal_index)?;
        Ok(rows_current && index_current)
    }
}

fn indexed(proposal: &GovernanceProposalRecord) -> bool {
    // The retained lookup includes every status. Its consumer separately filters
    // Enacted proposals; filtering here would hide canonical retained references.
    matches!(
        &proposal.kind,
        ProposalKind::ValidationFeePolicy(_) | ProposalKind::ValidationFeePayoutLifecycle(_)
    )
}

/// Check the fee-kind/created-height membership of both original native images.
/// This does not check proposal admission, status history or finalized authority;
/// the caller must independently retain and authenticate the two original owners.
pub(in crate::state) fn validate_original_validation_fee_proposals(
    rows: &impl RawStorageImages<[u8; 32], GovernanceProposalRecord>,
    index: &impl RawStorageImages<(u64, [u8; 32]), ()>,
    max_work: u64,
) -> Result<(), GroupedOwnershipError> {
    validate(rows, index, &mut Work(max_work))
}

fn validate(
    rows: &impl RawStorageImages<[u8; 32], GovernanceProposalRecord>,
    index: &impl RawStorageImages<(u64, [u8; 32]), ()>,
    work: &mut Work,
) -> Result<(), GroupedOwnershipError> {
    for image in [GroupImage::Current, GroupImage::Predecessor] {
        let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
            index: INDEX,
            image,
            mismatch,
        };
        visit_original(rows, image, work, |id, proposal, work| {
            if indexed(proposal) {
                let expected = (proposal.created_height, *id);
                let mut found = false;
                visit_original(index, image, work, |key, (), work| {
                    found |= compare_keys(key, &expected, work)? == Ordering::Equal;
                    Ok(())
                })?;
                if !found {
                    return Err(corrupt(GroupMismatch::MissingMember));
                }
            }
            Ok(())
        })?;
        visit_original(index, image, work, |key, (), work| {
            let mut found = false;
            visit_original(rows, image, work, |id, proposal, work| {
                let candidate = (proposal.created_height, *id);
                let same = compare_keys(key, &candidate, work)? == Ordering::Equal;
                found |= same && indexed(proposal);
                Ok(())
            })?;
            if !found {
                return Err(corrupt(GroupMismatch::ForeignMember));
            }
            Ok(())
        })?;
    }
    Ok(())
}

// Private and closed to this relation's two fixed native key shapes. The logical
// visitor still accepts only the sealed original storage pair, never an ad hoc
// iterator or a reconstructed index.
trait FixedKey: mv::Key {
    const BYTES: u64;
}
impl FixedKey for [u8; 32] {
    const BYTES: u64 = PROPOSAL_KEY_BYTES;
}
impl FixedKey for (u64, [u8; 32]) {
    const BYTES: u64 = INDEX_KEY_BYTES;
}

fn compare_keys<K: FixedKey>(
    left: &K,
    right: &K,
    work: &mut Work,
) -> Result<Ordering, GroupedOwnershipError> {
    work.0 = work
        .0
        .checked_sub(2 * K::BYTES)
        .ok_or(GroupedOwnershipError::WorkLimit)?;
    Ok(left.cmp(right))
}

fn next_physical<'a, K: FixedKey + 'a, V: 'a>(
    rows: &mut impl ExactSizeIterator<Item = (&'a K, &'a V)>,
    work: &mut Work,
) -> Result<Option<(&'a K, &'a V)>, GroupedOwnershipError> {
    if rows.len() == 0 {
        return Ok(None);
    }
    // The sealed native iterator exposes its physical length, so refusal occurs
    // before advancing or inspecting a current, masked, no-op or absent row.
    work.charge()?;
    Ok(rows.next())
}

fn visit_original<K: FixedKey, V: mv::Value>(
    rows: &impl RawStorageImages<K, V>,
    image: GroupImage,
    work: &mut Work,
    mut visit: impl FnMut(&K, &V, &mut Work) -> Result<(), GroupedOwnershipError>,
) -> Result<(), GroupedOwnershipError> {
    let mut current = rows.current_entries();
    if image == GroupImage::Current {
        while let Some((key, value)) = next_physical(&mut current, work)? {
            visit(key, value, work)?;
        }
        return Ok(());
    }
    let mut undo = rows.undo_entries();
    let mut current_row = next_physical(&mut current, work)?;
    let mut undo_row = next_physical(&mut undo, work)?;
    loop {
        match (current_row, undo_row) {
            (Some((current_key, current_value)), Some((undo_key, prior))) => {
                match compare_keys(current_key, undo_key, work)? {
                    Ordering::Less => {
                        visit(current_key, current_value, work)?;
                        current_row = next_physical(&mut current, work)?;
                    }
                    Ordering::Equal => {
                        if let Some(value) = prior {
                            visit(undo_key, value, work)?;
                        }
                        current_row = next_physical(&mut current, work)?;
                        undo_row = next_physical(&mut undo, work)?;
                    }
                    Ordering::Greater => {
                        if let Some(value) = prior {
                            visit(undo_key, value, work)?;
                        }
                        undo_row = next_physical(&mut undo, work)?;
                    }
                }
            }
            (Some((key, value)), None) => {
                visit(key, value, work)?;
                current_row = next_physical(&mut current, work)?;
            }
            (None, Some((key, prior))) => {
                if let Some(value) = prior {
                    visit(key, value, work)?;
                }
                undo_row = next_physical(&mut undo, work)?;
            }
            (None, None) => return Ok(()),
        }
    }
}

#[cfg(test)]
#[path = "validation_fee_proposals/test_support.rs"]
pub(in crate::state) mod test_support;
#[cfg(test)]
#[path = "validation_fee_proposals/tests.rs"]
mod tests;
