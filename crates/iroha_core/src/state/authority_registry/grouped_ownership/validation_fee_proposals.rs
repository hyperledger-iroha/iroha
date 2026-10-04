//! Exact validation-fee proposal lookup over both original native images.
//!
//! The canonical proposal capture consumes this retained owner directly.
//! This projection does not establish proposal admission,
//! Parliament status consistency, or finalized State authority.

use super::*;
use crate::state::GovernanceProposalRecord;
use iroha_data_model::governance::types::ProposalKind;

const INDEX: &str = "world.validation_fee_proposal_index";

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
        for image in [GroupImage::Current, GroupImage::Predecessor] {
            let corrupt = |mismatch| GroupedOwnershipError::Corrupt {
                index: INDEX,
                image,
                mismatch,
            };
            // visit_image charges physical rows before filtering, including
            // unrelated proposal kinds, masked current rows and undo tombstones.
            visit_image(&self.rows, image, work, |id, proposal, work| {
                if indexed(proposal) {
                    work.charge()?;
                    if get_at(&self.index, image, &(proposal.created_height, *id)).is_none() {
                        return Err(corrupt(GroupMismatch::MissingMember));
                    }
                }
                Ok(())
            })?;
            visit_image(&self.index, image, work, |(height, id), (), work| {
                work.charge()?;
                if !get_at(&self.rows, image, id)
                    .is_some_and(|proposal| indexed(proposal) && proposal.created_height == *height)
                {
                    return Err(corrupt(GroupMismatch::ForeignMember));
                }
                Ok(())
            })?;
        }
        Ok(())
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

#[cfg(test)]
mod tests;
