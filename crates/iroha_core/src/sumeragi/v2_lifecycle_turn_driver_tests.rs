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

    let superseded = ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
        ProductionRecoveredLifecycleSignCompletionSelectionV1::Superseded,
    );
    assert!(!superseded.restart_required());

    let runtime_debt = ProductionLifecycleCompletionSelectionV1::RecoveredLifecycleSignCompletion(
        ProductionRecoveredLifecycleSignCompletionSelectionV1::Retry,
    );
    assert!(!runtime_debt.restart_required());
}

#[test]
fn completion_classifier_consumes_composite_fetch_and_refanout_results() {
    let dispatched = ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(Ok(
        ProductionCompletionDispatchV1::CapacityUnavailable {
            protected_live_apply_ordinal: None,
        },
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
        ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyRestartRequired
            .restart_required()
    );
    assert!(
        !ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied.restart_required()
    );
}

#[test]
fn settled_apply_reenters_only_sealed_broadcast_output_paths() {
    use super::super::ProductionCompletionReadyWorkV1 as ReadyWork;
    use super::turn_driver::{
        ProductionApplyTerminalReadyWorkV1 as ApplyTerminalWork, classify_apply_terminal_ready_work,
    };

    for pass_through in [
        ReadyWork::None,
        ReadyWork::PassThrough,
        ReadyWork::CompletionIo,
    ] {
        assert_eq!(
            classify_apply_terminal_ready_work(pass_through),
            ApplyTerminalWork::PassThrough,
            "post-Apply Completion I/O must reach the existing Runtime fence without dispatch"
        );
    }
    assert_eq!(
        classify_apply_terminal_ready_work(ReadyWork::RetainedDirectOutput),
        ApplyTerminalWork::RetainedDirectOutput,
        "a retained direct Broadcast must use its exact pending-output owner"
    );
    assert_eq!(
        classify_apply_terminal_ready_work(ReadyWork::RecoveredLifecycleBroadcast),
        ApplyTerminalWork::RecoveredLifecycleBroadcast,
        "a recovered Broadcast must use its typed refanout owner"
    );
    assert_eq!(
        classify_apply_terminal_ready_work(ReadyWork::Invalid),
        ApplyTerminalWork::RestartRequired,
        "an invalid Ready census must fail closed"
    );

    let turn_driver = include_str!("v2_lifecycle_turn_driver.rs");
    let launched_start = turn_driver
        .find("fn drive_apply_terminal_ready_broadcast_turn<'cursor>(")
        .expect("the launched terminal Ready method remains present");
    let launched_tail = &turn_driver[launched_start..];
    let launched_end = launched_tail
        .find("/// Dispatch fresh Ready work only after")
        .expect("the launched terminal Ready method stays bounded");
    let launched_method = &launched_tail[..launched_end];
    assert!(launched_method.contains("classify_apply_terminal_ready_work("));
    assert!(
        launched_method.contains("refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(")
    );
    assert!(launched_method.contains("settle_apply_terminal_direct_broadcast("));
    assert!(!launched_method.contains("settle_pending_lifecycle_output_admissions("));
    assert!(launched_method.contains("self.close_output_for_restart();"));
    assert!(!launched_method.contains("dispatch_completion_with_runner_debt("));
    assert!(!launched_method.contains("dispatch_completion_requiring_ready_ordinal("));

    let activated_tail = &launched_tail[launched_end..];
    let activated_start = activated_tail
        .find("fn drive_apply_terminal_ready_broadcast_turn<'cursor>(")
        .expect("the activated terminal Ready forwarding method remains present");
    let activated_method = &activated_tail[activated_start..];
    assert!(activated_method.contains(".drive_apply_terminal_ready_broadcast_turn(ready, permit)"));

    let height_driver = include_str!("v2_runner/lifecycle_height_driver.rs");
    assert!(
        height_driver.contains("PreGate::Ready(ready) if producer_claim.apply_terminal_settled()")
    );
    assert!(height_driver.contains(".drive_apply_terminal_ready_broadcast_turn(ready, permit)"));
    assert!(height_driver.contains("blocked_runtime_drain_disposition(producer_claim)"));
}
