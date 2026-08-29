#[test]
fn certified_response_queue_refresh_retries_without_closing_output() {
    let driver = include_str!("v2_lifecycle_turn_driver.rs");
    let response_path = source_region(
        driver,
        "if !selected_ingress_is_certified_body_response",
        "fn drive_recovered_ingress_selector",
    );

    let narrowing = source_region(
        response_path,
        "let expected_context = lifecycle_context_for_ingress(self.executor.context());",
        "match contextual",
    );
    let narrowing_retry = source_region(
        narrowing,
        "Err((FairIngressQueueCutError::QueueCutChanged, retained))",
        "Err((error, retained))",
    );
    assert_source_tokens_in_order(
        narrowing_retry,
        &[
            "drop(retained);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!narrowing_retry.contains("close_output_for_restart()"));

    let priority = source_region(
        response_path,
        "let selected_priority = match self",
        "match selected_priority",
    );
    let priority_retry = source_region(
        priority,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        priority_retry,
        &[
            "drop(cut);",
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!priority_retry.contains("close_output_for_restart()"));

    let ordinary = source_region(
        response_path,
        "SelectedCertifiedResponsePriorityV1::OrdinaryClaimed",
        "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
    );
    let recovered = source_region(
        response_path,
        "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
        "self.drive_recovered_ingress_selector(selector, runner)",
    );

    let ordinary_selector_retry = source_region(
        ordinary,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        ordinary_selector_retry,
        &[
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::CertifiedFetchRetry",
        ],
    );
    assert!(!ordinary_selector_retry.contains("close_output_for_restart()"));

    let recovered_selector_retry = source_region(
        recovered,
        "Err(LifecycleIngressSelectorError::QueueCutChanged)",
        "Err(error)",
    );
    assert_source_tokens_in_order(
        recovered_selector_retry,
        &[
            "drop(runner);",
            "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchPreparationRetry",
        ],
    );
    assert!(!recovered_selector_retry.contains("close_output_for_restart()"));

    for structural_failure in [
        source_region(narrowing, "Err((error, retained))", "};"),
        source_region(priority, "Err(error)", "};"),
        source_region(ordinary, "Err(error)", "};"),
        source_region(recovered, "Err(error)", "};"),
    ] {
        assert_source_tokens_in_order(
            structural_failure,
            &[
                "close_output_for_restart();",
                "drop(runner);",
                "ProductionLifecycleIngressSelectionV1::RestartRequired",
            ],
        );
    }
}

#[test]
fn recovered_decision_fetch_phase_a_wakes_waiting_owner_before_queue_publication() {
    let scheduler = include_str!("v2_lifecycle_scheduler_inputs.rs");
    let registry = include_str!("v2_lifecycle_work_registry_validate_recovery_registry_impl.rs");
    let phase_a = source_region(
        scheduler,
        "pub(super) fn persist_recovered_decision_fetch_response_after_runner(",
        "/// Plan, submit, and reblock one exact selected certified-Fetch response.",
    );
    assert_source_tokens_in_order(
        phase_a,
        &[
            "self.coordinator.active_lease.is_some()",
            "attest_scheduler_recovered_fetch_carrier(",
            "capture_lifecycle_capacity_rank(selector)",
            "authenticated_waiting_fetch_ready_row(",
            "prepare_recovered_decision_fetch_response_claim(&task)",
            "let mut next = self.coordinator.stage_durable_transaction();",
            "let lease = match next.plan_turn(inputs)",
            "matches_claimed_dispatched_recovered_decision_fetch(",
            "self.coordinator = next;",
            "claim.commit_with_queue(reservation, task);",
        ],
    );
    let swap = source_token_position(phase_a, "self.coordinator = next;");
    let tail = &phase_a[swap..];
    assert!(!tail.contains("return Err"));
    assert!(!tail.contains("settle_turn("));
    assert!(tail.contains("assert_eq!(self.coordinator.active_lease.as_ref(), Some(&lease))"));
    let waiting_carrier = source_region(
        registry,
        "pub(super) fn matches_waiting_dispatched_recovered_decision_fetch(",
        "/// Join one exact claimed recovered Decision Fetch back to its closed carrier.",
    );
    assert_required_source_tokens(
        waiting_carrier,
        &[
            "coordinator.records.iter().any",
            "*candidate != ordinal",
            "wait.source() == wait_source",
        ],
    );
}

#[test]
fn recovered_decision_fetch_response_claim_precedes_assertion_only_queue_publication() {
    let effects = reviewed_v2_effects_source_for_test();
    let commit = effects
        .split_once("pub(in crate::sumeragi) fn commit_with_queue(")
        .expect("recovered response has one composite commit")
        .1
        .split_once("impl RecoveredDecisionFetchResponseCandidateV1")
        .expect("composite commit stays bounded")
        .0;
    let claim = commit
        .find("owner.commit_exact_response_claim(response_hash)")
        .expect("exact response claim is installed");
    let queue = commit
        .find("queue.commit_recovered_decision_fetch_body_persistence(task)")
        .expect("dedicated persistence is published");
    assert!(claim < queue);
    assert!(commit.contains("assert!(owner.matches_response_claim_preflight"));
    let worker = include_str!("v2_worker.rs");
    let queue_commit = worker
        .split_once("fn commit_recovered_decision_fetch_body_persistence(")
        .expect("dedicated queue commit exists")
        .1
        .split_once("#[cfg(test)]")
        .expect("queue commit stays bounded")
        .0;
    assert!(queue_commit.contains("assert!("));
    assert!(!queue_commit.contains("return Err"));
}
