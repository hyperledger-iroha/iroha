//! Focused tests for the lifecycle-owned Completion and Ingress turn boundary.

use super::*;

crate::sumeragi::v2_lifecycle_coordinator::source_contract_test!(
    turn_outcomes_retain_only_the_real_pass_through_cursor
);

#[test]
fn recovered_sign_completion_classifies_only_lost_or_post_publication_owners_for_restart() {
    let retry = ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
        ProductionRecoveredLifecycleSignCompletionSelectionV1::Broadcast(
            ProductionRecoveredLifecycleSignBroadcastSettlementV1::Retry,
        ),
    );
    assert!(!retry.restart_required());

    let absent = ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
        ProductionRecoveredLifecycleSignCompletionSelectionV1::Broadcast(
            ProductionRecoveredLifecycleSignBroadcastSettlementV1::None,
        ),
    );
    assert!(absent.restart_required());

    let capacity = ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
        ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalPrepareWal(
            ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable,
        ),
    );
    assert!(!capacity.restart_required());

    let proposal_restart =
        ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
            ProductionRecoveredLifecycleSignCompletionSelectionV1::ProposalBroadcastAndSign(
                ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired,
            ),
        );
    assert!(proposal_restart.restart_required());

    let vote_applied = ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
        ProductionRecoveredLifecycleSignCompletionSelectionV1::VoteBroadcastAndSign(
            ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::Applied,
        ),
    );
    assert!(!vote_applied.restart_required());
}

#[test]
fn completion_classifier_consumes_composite_fetch_and_refanout_results() {
    let dispatched = ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(Ok(
        ProductionCompletionDispatchV1::CapacityUnavailable,
    ));
    assert!(!dispatched.restart_required());
    let dispatch_error = ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(Err(
        ProductionCompletionDispatchErrorV1::InvalidReadyCensus,
    ));
    assert!(dispatch_error.restart_required());

    let fetch_retry = ProductionLifecycleCompletionSelectionV1::RecoveredDecisionFetchCompletion(
        ProductionRecoveredDecisionFetchStoreSettlementV1::Retry(
            ProductionRecoveredDecisionFetchStoreSettlementFailureV1::Owner,
        ),
    );
    assert!(!fetch_retry.restart_required());
    let fetch_absent = ProductionLifecycleCompletionSelectionV1::RecoveredDecisionFetchCompletion(
        ProductionRecoveredDecisionFetchStoreSettlementV1::None,
    );
    assert!(fetch_absent.restart_required());

    let refanout_capacity =
        ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleBroadcastRefanout(Ok(
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::CapacityUnavailable,
        ));
    assert!(!refanout_capacity.restart_required());
    let refanout_restart =
        ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleBroadcastRefanout(Ok(
            ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired,
        ));
    assert!(refanout_restart.restart_required());
    let refanout_error =
        ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleBroadcastRefanout(Err(
            ProductionRecoveredLifecycleSignedBroadcastRefanoutErrorV1::InvalidReadyCensus,
        ));
    assert!(refanout_error.restart_required());

    assert!(
        ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyRestartRequired
            .restart_required()
    );
    assert!(
        !ProductionLifecycleCompletionSelectionV1::RecoveredDecisionApplyApplied.restart_required()
    );
}
