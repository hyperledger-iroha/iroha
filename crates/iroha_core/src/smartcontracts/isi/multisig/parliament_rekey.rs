//! Fail-closed account-rekey guards for immutable Parliament bindings.

use iroha_data_model::{
    account::AccountId,
    governance::types::{GovernanceAttemptStatusV1, ProposalContentId, ProposalKind},
    isi::error::InstructionExecutionError,
    validation_fee::ValidationFeeTreasuryPayoutBindingV1,
};
use mv::storage::StorageReadOnly;

use crate::{
    governance::parliament::{
        MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1, validate_parliament_randomness_redraw_lineage_v1,
    },
    state::{GovernanceProposalStatus, StateTransaction},
};

fn payout_binding_references_account(
    binding: &ValidationFeeTreasuryPayoutBindingV1,
    account: &AccountId,
) -> bool {
    &binding.treasury_account_id == account
        || &binding.pool_vault_account_id == account
        || binding
            .recipients
            .iter()
            .any(|recipient| &recipient.account_id == account)
}

fn validation_fee_proposal_references_account(
    proposal: &ProposalKind,
    account: &AccountId,
) -> bool {
    match proposal {
        ProposalKind::ValidationFeePolicy(payload) => {
            &payload.proposal_operator == account
                || &payload.policy.treasury_account_id == account
                || payload
                    .policy
                    .treasury_payout_binding
                    .as_ref()
                    .is_some_and(|binding| payout_binding_references_account(binding, account))
        }
        ProposalKind::ValidationFeePayoutLifecycle(payload) => {
            &payload.proposal_operator == account
                || payout_binding_references_account(&payload.payout_binding, account)
        }
        ProposalKind::DeployContract(_)
        | ProposalKind::RuntimeUpgrade(_)
        | ProposalKind::SccpRouteGovernance(_)
        | ProposalKind::SorafsProviderGovernance(_)
        | ProposalKind::MusubiRegistryGovernance(_)
        | ProposalKind::ContractLifecycleGovernance(_)
        | ProposalKind::ContractEmergencyHold(_)
        | ProposalKind::GlobalDataTriggerPermissionGovernance(_) => false,
    }
}

fn terminal_status_matches_attempt(
    status: GovernanceProposalStatus,
    attempt_status: GovernanceAttemptStatusV1,
) -> bool {
    matches!(
        (status, attempt_status),
        (
            GovernanceProposalStatus::Rejected,
            GovernanceAttemptStatusV1::Rejected
        ) | (
            GovernanceProposalStatus::Superseded,
            GovernanceAttemptStatusV1::Superseded
        ) | (
            GovernanceProposalStatus::ExecutionFailed,
            GovernanceAttemptStatusV1::ExecutionFailed
        )
    )
}

/// Prove that a terminal fee proposal has no remaining proposal-wide redraw.
///
/// This intentionally repeats the restore-time history checks at the rekey boundary. A missing,
/// sparse, malformed, or proposal-mismatched history must not be mistaken for exhausted history.
fn terminal_validation_fee_retry_budget_is_exhausted(
    state_transaction: &StateTransaction<'_, '_>,
    proposal_id: [u8; 32],
    proposal: &crate::state::GovernanceProposalRecord,
) -> bool {
    let Some(operator) = (match &proposal.kind {
        ProposalKind::ValidationFeePolicy(payload) => Some(&payload.proposal_operator),
        ProposalKind::ValidationFeePayoutLifecycle(payload) => Some(&payload.proposal_operator),
        ProposalKind::DeployContract(_)
        | ProposalKind::RuntimeUpgrade(_)
        | ProposalKind::SccpRouteGovernance(_)
        | ProposalKind::SorafsProviderGovernance(_)
        | ProposalKind::MusubiRegistryGovernance(_)
        | ProposalKind::ContractLifecycleGovernance(_)
        | ProposalKind::ContractEmergencyHold(_)
        | ProposalKind::GlobalDataTriggerPermissionGovernance(_) => None,
    }) else {
        return false;
    };
    if proposal.kind.fingerprint() != proposal_id || operator != &proposal.proposer {
        return false;
    }

    let proposal_content_id = ProposalContentId::new(proposal_id);
    let mut attempts = state_transaction
        .world
        .parliament_attempts
        .iter()
        .filter(|(_, attempt)| attempt.proposal_content_id() == proposal_content_id)
        .collect::<Vec<_>>();
    attempts.sort_unstable_by_key(|(_, attempt)| attempt.attempt().sequence);
    let Some((_, latest)) = attempts.last().copied() else {
        return false;
    };
    if latest.randomness_redraws_used_v1() != Ok(MAX_PARLIAMENT_RANDOMNESS_REDRAWS_V1)
        || !terminal_status_matches_attempt(proposal.status, latest.attempt().status)
        || validate_parliament_randomness_redraw_lineage_v1(
            attempts.iter().map(|(_, attempt)| *attempt),
        )
        .is_err()
    {
        return false;
    }

    attempts.iter().enumerate().all(|(index, entry)| {
        let (key, attempt) = *entry;
        let Ok(expected_sequence) = u32::try_from(index) else {
            return false;
        };
        key == &attempt.attempt().id
            && attempt.attempt().sequence == expected_sequence
            && attempt.validate().is_ok()
            && attempt
                .validate_proposal_bindings_v1(&proposal.kind)
                .is_ok()
            && (index + 1 == attempts.len()
                || matches!(
                    attempt.attempt().status,
                    GovernanceAttemptStatusV1::Rejected
                        | GovernanceAttemptStatusV1::Superseded
                        | GovernanceAttemptStatusV1::ExecutionFailed
                ))
    })
}

/// Return whether the proposal contains one of the validation-fee preimages
/// whose proposer is part of its immutable fingerprint.
pub(super) const fn is_validation_fee_proposal(proposal: &ProposalKind) -> bool {
    matches!(
        proposal,
        ProposalKind::ValidationFeePolicy(_) | ProposalKind::ValidationFeePayoutLifecycle(_)
    )
}

/// Reject a controller-derived account-ID change that would strand a hash-bound
/// Parliament member or invalidate an immutable validation-fee authorization.
pub(super) fn ensure_account_rekey_preserves_bindings(
    state_transaction: &StateTransaction<'_, '_>,
    old_account: &AccountId,
) -> Result<(), InstructionExecutionError> {
    if state_transaction
        .world
        .parliament_attempts
        .iter()
        .any(|(_, attempt)| attempt.retains_citizenship_bond(old_account))
    {
        return Err(InstructionExecutionError::InvariantViolation(
            format!(
                "cannot rekey account {old_account}: it is retained by an active or certified Parliament attempt"
            )
            .into(),
        ));
    }
    for (proposal_id, proposal) in state_transaction.world.governance_proposals.iter() {
        if !is_validation_fee_proposal(&proposal.kind)
            || (proposal.proposer != *old_account
                && !validation_fee_proposal_references_account(&proposal.kind, old_account))
        {
            continue;
        }
        let rekey_is_safe = matches!(
            proposal.status,
            GovernanceProposalStatus::Rejected
                | GovernanceProposalStatus::Superseded
                | GovernanceProposalStatus::ExecutionFailed
        ) && terminal_validation_fee_retry_budget_is_exhausted(
            state_transaction,
            *proposal_id,
            proposal,
        );
        if !rekey_is_safe {
            return Err(InstructionExecutionError::InvariantViolation(
                format!(
                    "cannot rekey account {old_account}: it is retained by a live, operational, retryable, or noncanonical validation-fee Parliament authorization"
                )
                .into(),
            ));
        }
    }
    Ok(())
}
