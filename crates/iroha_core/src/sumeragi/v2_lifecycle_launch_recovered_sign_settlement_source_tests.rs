fn assert_recovered_vote_broadcast_and_sign_settlement_is_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source_region(
        source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
        "/// Fsync an initial Proposal `PrepareIntent`, then publish both successors.",
    );
    assert_source_tokens_in_order(
        settlement,
        &[
            "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
            "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
            "preview.is_vote_broadcast_and_sign_shape()",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            "output_guard.begin_fail_stop_operation()",
            "transition.persist_exact_successor().is_err()",
            "transition.commit_after_publication();",
            "completion.acknowledge_after_publication();",
            "operation.complete();",
        ],
    );
    assert_forbidden_source_tokens(
        settlement,
        &[
            "project_proposal_exact_output_authority",
            "capture_recovered_lifecycle_proposal_exact_output",
            "output.commit_after_publication()",
        ],
    );
    let transition_commit =
        source_token_position(settlement, "transition.commit_after_publication();");
    let tail = &settlement[transition_commit..];
    assert_forbidden_source_tokens(tail, &["return ", ".is_err()", "?"]);
}

fn assert_recovered_proposal_prepare_wal_settlement_is_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source_region(
        source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_prepare_wal(",
        "/// Settle a recovered Proposal into one Broadcast and one WAL-backed Sign.",
    );
    assert_source_tokens_in_order(
        settlement,
        &[
            "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
            "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
            "RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
            "preview.project_proposal_exact_output_authority()",
            "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
            "output.prepare_wal_append_permit()",
            "append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            "transition.persist_exact_successor().is_err()",
            "transition.commit_after_publication();",
            "completion.acknowledge_after_publication();",
            "output.commit_after_publication();",
        ],
    );
    assert_required_source_tokens(
        settlement,
        &[
            "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
            "Some(PendingLifecycleCompletionV1::RecoveredSign(completion))",
        ],
    );
    assert_forbidden_source_tokens(settlement, &["output.abort_before_publication()"]);
    let wal = source_token_position(
        settlement,
        "append_recovered_lifecycle_proposal_prepare_wal(wal_permit)",
    );
    let transition_commit =
        source_token_position(settlement, "transition.commit_after_publication();");
    let post_wal = &settlement[wal..transition_commit];
    assert!(post_wal.matches("drop(output);").count() >= 3);
    let tail = &settlement[transition_commit..];
    assert_forbidden_source_tokens(tail, &["return ", ".is_err()", "?"]);
}

fn assert_recovered_proposal_broadcast_and_sign_settlement_is_atomic_and_restart_closed() {
    let source = include_str!("v2_lifecycle_launch.rs");
    let settlement = source_region(
        source,
        "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
        "/// Drive and retry one exact missing-sidecar lifecycle Decision Apply owner.",
    );
    assert_source_tokens_in_order(
        settlement,
        &[
            "PendingLifecycleCompletionV1::take_recovered_sign(pending_lifecycle_completion)",
            "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
            "preview.project_proposal_exact_output_authority()",
            "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
            "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
            "transition.persist_exact_successor().is_err()",
            "transition.commit_after_publication();",
            "completion.acknowledge_after_publication();",
            "output.commit_after_publication();",
        ],
    );
    assert_source_token_count(settlement, "output.abort_before_publication()", 2);
    assert_required_source_tokens(
        settlement,
        &[
            "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
            "Some(PendingLifecycleCompletionV1::RecoveredSign(completion))",
            "drop(output);",
        ],
    );
    let transition_commit =
        source_token_position(settlement, "transition.commit_after_publication();");
    let tail = &settlement[transition_commit..];
    assert_forbidden_source_tokens(tail, &["return ", ".is_err()", "?"]);
}
