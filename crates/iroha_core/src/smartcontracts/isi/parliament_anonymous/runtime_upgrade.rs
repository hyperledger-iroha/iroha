//! The anonymous family's sole original signed runtime proposal creator.

use super::*;
use iroha_data_model::{
    governance::{
        anonymous_ballot::{
            AnonymousRuntimeUpgradeRequestV1, CreateAnonymousRuntimeUpgradeAttemptV1,
        },
        types::RuntimeUpgradeProposal,
    },
    runtime::lifecycle::{RootScopeV1, RuntimeUpgradeActionV1},
};
use iroha_executor_data_model::permission::governance::CanProposeRuntimeChangeSet;

/// One bounded action, including the definitions and provenance its original signed source owns.
const MAX_RUNTIME_REQUEST_BYTES: usize = 1 << 20;

impl Execute for CreateAnonymousRuntimeUpgradeAttemptV1 {
    fn execute(
        self,
        authority: &AccountId,
        state: &mut StateTransaction<'_, '_>,
    ) -> Result<(), Error> {
        state
            .consume_direct_parliament_runtime(&self)
            .map_err(|error| match error {
                ExecutionAttemptError::Deferred(original) => {
                    attempt_refusal(state, ExecutionAttemptError::Deferred(original))
                }
                ExecutionAttemptError::Rejected(reason) => refused(reason),
            })?;
        crate::runtime::change_set::anonymous::require_global(state)
            .map_err(|error| attempt_refusal(state, error))?;
        let permission: Permission = CanProposeRuntimeChangeSet.into();
        if !has_exact_permission(&state.world, authority, &permission) {
            return Err(refused(
                "not permitted: CanProposeRuntimeChangeSet required",
            ));
        }
        let length = norito::core::classify_decode_attempt(|| norito::canonical_frame_len(&self))
            .map_err(|original| {
            state.attempt_error_to_instruction_error(
                crate::execution_attempt::canonical_decode_attempt_error(original, refused),
            )
        })?;
        if length > MAX_RUNTIME_REQUEST_BYTES {
            return Err(refused("runtime request exceeds one MiB"));
        }
        let (proposal, attempt_sequence) = match self.request {
            AnonymousRuntimeUpgradeRequestV1::Initial { action, provenance } => {
                if matches!(&action, RuntimeUpgradeActionV1::Schedule(change) if change.root != RootScopeV1::Universal)
                {
                    return Err(refused(
                        "SORA Parliament governs only the universal runtime root",
                    ));
                }
                let (notice, proposed_ms) = crate::runtime::change_set::anonymous::admit_proposal(
                    &action,
                    &provenance,
                    state,
                )
                .map_err(|error| attempt_refusal(state, error))?;
                (
                    RuntimeUpgradeProposal {
                        proposal_operator: authority.clone(),
                        action,
                        notice,
                        proposed_ms,
                    },
                    0,
                )
            }
            AnonymousRuntimeUpgradeRequestV1::Retry { predecessor } => {
                let previous = state
                    .world
                    .parliament_anonymous_attempts
                    .get(&predecessor)
                    .ok_or_else(|| refused("runtime retry needs its exact stored predecessor"))?;
                if !matches!(
                    previous.attempt().status,
                    GovernanceAttemptStatusV1::Rejected
                        | GovernanceAttemptStatusV1::Superseded
                        | GovernanceAttemptStatusV1::ExecutionFailed
                ) {
                    return Err(refused(
                        "runtime retry needs a terminal non-enacted predecessor",
                    ));
                }
                let ProposalKind::RuntimeUpgrade(proposal) = previous.proposal() else {
                    return Err(refused("runtime retry names a different proposal kind"));
                };
                if proposal.proposal_operator != *authority {
                    return Err(refused(
                        "runtime retry must retain its original signed operator",
                    ));
                }
                let sequence = previous
                    .attempt()
                    .sequence
                    .checked_add(1)
                    .ok_or_else(|| refused("runtime attempt sequence exhausted"))?;
                (proposal.clone(), sequence)
            }
        };
        create_attempt(
            CreateAnonymousParliamentAttemptV1 {
                proposal: ProposalKind::RuntimeUpgrade(proposal),
                attempt_sequence,
            },
            state,
        )
    }
}
