//! Addition-owned deadline receipts for proposals whose immutable reducers cannot finish.

use super::*;

/// Exact last-height boundary for this decision's execution eligibility.
pub(crate) fn proposal_deadline(proposal: &SorafsAuthorityProposalV1) -> Option<u64> {
    proposal.grid.window(proposal.term).map(|window| {
        if proposal.action == SorafsAuthorityActionV1::Revoke {
            window.end_height
        } else {
            window.cutoff_height
        }
    })
}

/// Whether a retained proposal has its separate canonical deadline receipt.
pub(crate) fn proposal_is_closed(world: &impl WorldReadOnly, content: ProposalContentId) -> bool {
    world
        .sorafs_authority_proposals()
        .get(content.as_bytes())
        .is_some_and(|record| record.deadline_closure.is_some())
}

/// Close pending hosted decisions without changing a shared reducer or authorizing an effect.
pub(crate) fn close_expired_proposals(
    tx: &mut StateTransaction<'_, '_>,
) -> Result<(), InstructionExecutionError> {
    if !active(&tx.world)? {
        return Ok(());
    }
    let height = tx.block_height();
    loop {
        let next = tx
            .world
            .sorafs_authority_proposals
            .iter()
            .find(|(_, record)| {
                record.status == GovernanceProposalStatus::Proposed
                    && record.deadline_closure.is_none()
                    && proposal_deadline(&record.proposal)
                        .is_some_and(|deadline| deadline <= height)
            })
            .map(|(id, record)| (*id, record.clone()));
        let Some((id, mut record)) = next else {
            return Ok(());
        };
        if proposal_deadline(&record.proposal) != Some(height) {
            return Err(invalid(
                "pending authority decision missed its exact deadline closure",
            ));
        }
        for attempt_id in crate::governance::parliament::canonical_governance_attempt_ids_v1(
            record.proposal.proposal_content_id(),
        ) {
            tx.world
                .close_sorafs_authority_attempt_work(attempt_id)
                .map_err(|e| invalid(e.to_string()))?;
        }
        record.deadline_closure = Some(SorafsAuthorityDeadlineClosureV1 {
            closed_at_height: height,
        });
        tx.world.sorafs_authority_proposals.insert(id, record);
    }
}

/// Original transcript horizon for an independently validated deadline receipt.
/// Restore validates the receipt and original current/undo cut before using this bound.
pub(crate) fn retained_attempt_validation_height(
    world: &impl WorldReadOnly,
    content: ProposalContentId,
    committed_height: u64,
) -> u64 {
    world
        .sorafs_authority_proposals()
        .get(content.as_bytes())
        .and_then(|record| record.deadline_closure)
        .map_or(committed_height, |closure| {
            closure.closed_at_height.saturating_sub(1)
        })
}
