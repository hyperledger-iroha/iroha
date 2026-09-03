# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(repo_root: Path) -> list[str]:
    """Pin the queue-owned ordinary/Serve ingress turn prerequisite."""

    errors: list[str] = []

    def load(relative: str, label: str) -> tuple[Path, str]:
        return _read_reviewed_rust_source(repo_root, relative, errors, label)

    paths: dict[str, Path] = {}
    sources: dict[str, str] = {}
    for name, relative in (
        ("ingress", "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs"),
        ("selector", "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs"),
        ("driver", "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs"),
        ("runtime", "crates/iroha_core/src/sumeragi/v2_runtime.rs"),
        ("launch", "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"),
        ("launch_tests", "crates/iroha_core/src/sumeragi/v2_lifecycle_launch_tests.rs"),
        (
            "ledger",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs",
        ),
        (
            "preactivation",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_preactivation.rs",
        ),
        (
            "pending_lifecycle",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs",
        ),
        (
            "pending_kura",
            "crates/iroha_core/src/sumeragi/v2_pending_kura_recovery.rs",
        ),
        ("effects", "crates/iroha_core/src/sumeragi/v2_effects.rs"),
        ("apply_tests", "crates/iroha_core/src/sumeragi/v2_apply_tests.rs"),
        ("worker", "crates/iroha_core/src/sumeragi/v2_worker.rs"),
        (
            "worker_services",
            "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs",
        ),
        ("lane_work", "crates/iroha_core/src/sumeragi/v2_lane_work.rs"),
        ("adapter", "crates/iroha_core/src/sumeragi/v2.rs"),
        (
            "scheduler",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        ),
        (
            "registry_output",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_output.rs",
        ),
        (
            "concrete_admission",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_concrete_admission.rs",
        ),
        (
            "effects_settlement",
            "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs",
        ),
        (
            "registry",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        ),
        (
            "registry_recovery_impl",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery_registry_impl.rs",
        ),
        (
            "schema",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs",
        ),
        (
            "coordinator",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs",
        ),
        (
            "open_output",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_open_output_recovery.rs",
        ),
        (
            "coordinator_support",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator_support.rs",
        ),
        ("runner", "crates/iroha_core/src/sumeragi/v2_runner.rs"),
        (
            "runner_test",
            "crates/iroha_core/src/sumeragi/tests/v2_runner_unsealed_00.rs",
        ),
        (
            "ordinary_consumer",
            "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs",
        ),
        (
            "height_driver",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs",
        ),
        (
            "lifecycle_run_inner",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        ),
        (
            "pending_runner",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        ),
        (
            "runner_authority",
            "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs",
        ),
        (
            "preactivation_ingress",
            "crates/iroha_core/src/sumeragi/v2_runner/preactivation_ingress.rs",
        ),
        (
            "startup_test",
            "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
        ),
        (
            "wal_test",
            "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery.rs",
        ),
        (
            "dispatch_test",
            "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_work_registry_validate_dispatch_execution_cases.rs",
        ),
        (
            "ledger_recovery_test",
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs",
        ),
    ):
        path, source = load(relative, f"queue-owned ordinary ingress {name}")
        paths[name] = path
        sources[name] = source
    if any(not source for source in sources.values()):
        return errors

    runtime_tokens = rust_code_tokens(sources["runtime"])
    for retired in (
        "step_recovery_and_take_scheduler_ownership_for_test",
        "RecoveryFifo",
        "RecoveryFifoRetryRetained",
        "RecoveryIdle",
    ):
        if runtime_tokens.count(retired) != 0:
            errors.append(
                f"{paths['runtime']}: retired generic Runtime recovery symbol "
                f"{retired} must remain absent; interrupted-tip recovery is owned "
                "only by the PendingKura lifecycle corridor"
            )

    for source_name, child_path, declaration in (
        (
            "adapter",
            "v2_pending_kura_recovery.rs",
            'mod pending_kura_recovery;',
        ),
        (
            "launch",
            "v2_lifecycle_preactivation.rs",
            'mod preactivation;',
        ),
        (
            "launch",
            "v2_lifecycle_pending_kura.rs",
            'mod pending_kura;',
        ),
        (
            "runner",
            "v2_runner/ordinary_ingress_consumer.rs",
            'pub(in crate::sumeragi) mod ordinary_ingress_consumer;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_height_driver.rs",
            'mod lifecycle_height_driver;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_run_inner.rs",
            'pub(in crate::sumeragi) mod lifecycle_run_inner;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_pending_kura.rs",
            'mod lifecycle_pending_kura;',
        ),
        (
            "runner",
            "v2_runner/lifecycle_runner_authority.rs",
            'mod lifecycle_runner_authority;',
        ),
        (
            "runner",
            "v2_runner/preactivation_ingress.rs",
            'mod preactivation_ingress;',
        ),
        (
            "coordinator",
            "v2_lifecycle_coordinator_support.rs",
            'mod coordinator_support;',
        ),
    ):
        path_token = f'#[path = "{child_path}"]'
        source = sources[source_name]
        if source.count(path_token) != 1 or source.count(declaration) != 1:
            errors.append(
                f"{paths[source_name]}: sealed lifecycle child module wiring "
                f"must retain exactly one {path_token!r} and {declaration!r}"
            )

    launch_test_include = 'include!("v2_lifecycle_launch_tests.rs");'
    if (
        sources["launch"].count("#[cfg(test)]\nmod tests {") != 1
        or sources["launch"].count(launch_test_include) != 1
    ):
        errors.append(
            f"{paths['launch']}: sealed lifecycle test module wiring must "
            "retain exactly one cfg(test) module with one authenticated "
            f"{launch_test_include!r}"
        )

    def item(source_name: str, name: str) -> RustItem | None:
        return _require_rust_item(
            paths[source_name], sources[source_name], name, errors
        )

    def qualified_item(
        source_name: str,
        name: str,
        brace_context: tuple[str, ...],
        label: str,
    ) -> RustItem | None:
        expected_context = (brace_context,)
        matching = [
            rust_item
            for rust_item in rust_items(sources[source_name], name)
            if rust_item.brace_context == expected_context
        ]
        if len(matching) != 1:
            errors.append(
                f"{paths[source_name]}: require exactly one real Rust/Verus "
                f"function item named {name} in {brace_context!r}; "
                f"found {len(matching)}"
            )
            return None
        rust_item = matching[0]
        _require_rust_item_context(
            paths[source_name],
            rust_item,
            expected_context,
            label,
            errors,
        )
        return rust_item

    def require_tokens(
        source_name: str,
        rust_item: RustItem | None,
        label: str,
        tokens: tuple[str, ...],
    ) -> None:
        for token in tokens:
            _require_rust_token_sequence(
                paths[source_name], rust_item, token, label, errors
            )

    def require_order(
        source_name: str,
        rust_item: RustItem | None,
        label: str,
        markers: tuple[str, ...],
    ) -> None:
        if rust_item is None:
            return
        body = rust_code_tokens(rust_item.source)
        cursor = 0
        for marker in markers:
            needle = rust_code_tokens(marker)
            position = next(
                (
                    index
                    for index in range(cursor, len(body) - len(needle) + 1)
                    if body[index : index + len(needle)] == needle
                ),
                -1,
            )
            if position < 0:
                errors.append(
                    f"{paths[source_name]}:{rust_item.line}: {label} must "
                    f"preserve exact order {markers!r}"
                )
                return
            cursor = position + len(needle)

    def require_source_order(
        source_name: str,
        label: str,
        markers: tuple[str, ...],
    ) -> None:
        body = rust_code_tokens(sources[source_name])
        cursor = 0
        for marker in markers:
            needle = rust_code_tokens(marker)
            position = next(
                (
                    index
                    for index in range(cursor, len(body) - len(needle) + 1)
                    if body[index : index + len(needle)] == needle
                ),
                -1,
            )
            if position < 0:
                errors.append(
                    f"{paths[source_name]}: {label} must preserve exact order "
                    f"{markers!r}"
                )
                return
            cursor = position + len(needle)

    def reject_tokens(
        source_name: str,
        rust_item: RustItem | None,
        label: str,
        forbidden: tuple[str, ...],
    ) -> None:
        if rust_item is None:
            return
        body = rust_code_tokens(rust_item.source)
        observed = tuple(
            token
            for token in forbidden
            if _token_sequence_count(body, rust_code_tokens(token))
        )
        if observed:
            errors.append(
                f"{paths[source_name]}:{rust_item.line}: {label} retains "
                f"forbidden ordinary-height authority {observed!r}"
            )

    publication_fence_struct = rust_code_tokens(
        """
pub(super) struct LockedPreparedFairIngressExactDequeue<'a> {
    queue: &'a FairV2Ingress,
    _service_guard: MutexGuard<'a, ()>,
    _producer_publication_guard: MutexGuard<'a, ()>,
    witness: PreparedFairIngressQueueWitness,
    selection: PreparedFairIngressQueueSelection,
}
"""
    )
    ingress_tokens = rust_code_tokens(sources["ingress"])
    if _token_sequence_count(ingress_tokens, publication_fence_struct) != 1:
        errors.append(
            f"{paths['ingress']}: lifecycle publication fence must retain one "
            "move-only service/producer guard carrier"
        )

    lock_publication_fence = _require_qualified_rust_item(
        paths["ingress"],
        sources["ingress"],
        "PreparedFairIngressQueueWitness",
        "lock_exact_dequeue_retaining",
        errors,
        "pre-LedgerV1 exact dequeue publication fence",
        expected_attributes=("#[allow(clippy::result_large_err)]",),
    )
    _require_rust_item_token_sha256(
        paths["ingress"],
        lock_publication_fence,
        _PRODUCTION_LIFECYCLE_INGRESS_PUBLICATION_FENCE_ITEM_SHA256[
            "PreparedFairIngressQueueWitness::lock_exact_dequeue_retaining"
        ],
        "pre-LedgerV1 exact dequeue publication fence",
        errors,
    )
    require_order(
        "ingress",
        lock_publication_fence,
        "service then producer publication lock before final queue preflight",
        (
            "if !self.is_internally_exact()",
            "let service_guard = queue.service_lock.lock()",
            "let producer_publication_guard = queue.producer_publication_lock.lock()",
            "self.revalidate_for_commit(queue)",
            "let state = queue.state.lock()",
            "self.metadata_matches_locked(&state)",
            "LockedPreparedFairIngressExactDequeue {",
            "_service_guard: service_guard",
            "_producer_publication_guard: producer_publication_guard",
        ),
    )

    publication_commit_candidates = [
        rust_item
        for rust_item in rust_items(sources["ingress"], "commit")
        if rust_item.brace_context
        == (("impl", "LockedPreparedFairIngressExactDequeue", "<", "'", "_", ">"),)
    ]
    if len(publication_commit_candidates) != 1:
        errors.append(
            f"{paths['ingress']}: require exactly one assertion-only "
            "LockedPreparedFairIngressExactDequeue::commit; found "
            f"{len(publication_commit_candidates)}"
        )
        publication_commit = None
    else:
        publication_commit = publication_commit_candidates[0]
        _require_rust_item_context(
            paths["ingress"],
            publication_commit,
            (("impl", "LockedPreparedFairIngressExactDequeue", "<", "'", "_", ">"),),
            "post-LedgerV1 assertion-only exact dequeue",
            errors,
        )
    _require_rust_item_token_sha256(
        paths["ingress"],
        publication_commit,
        _PRODUCTION_LIFECYCLE_INGRESS_PUBLICATION_FENCE_ITEM_SHA256[
            "LockedPreparedFairIngressExactDequeue::commit"
        ],
        "post-LedgerV1 assertion-only exact dequeue",
        errors,
    )
    require_order(
        "ingress",
        publication_commit,
        "post-publication assertion dequeue before producer release",
        (
            "_producer_publication_guard",
            "let mut state = queue.state.lock()",
            "witness.metadata_matches_locked(&state)",
            "queue.dequeue_selected_locked(",
            ".expect(\"prevalidated lifecycle dequeue is infallible after publication\")",
            "drop(state)",
            "drop(_producer_publication_guard)",
            "drop(_service_guard)",
        ),
    )

    publication_fence_test_context = (
        ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
    )
    for test_name in (
        "locked_publication_fence_serializes_same_wire_and_reenqueues_after_commit",
        "locked_publication_fence_serializes_unrelated_append_and_preserves_it",
        "dropping_locked_publication_fence_releases_producer_without_dequeue",
    ):
        regression = item("ingress", test_name)
        _require_rust_item_context(
            paths["ingress"],
            regression,
            publication_fence_test_context,
            f"producer-publication-fence regression {test_name}",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            paths["ingress"],
            regression,
            _PRODUCTION_LIFECYCLE_INGRESS_PUBLICATION_FENCE_ITEM_SHA256[test_name],
            f"producer-publication-fence regression {test_name}",
            errors,
        )

    launched_fields = sources["launch"]
    launched_start = launched_fields.find(
        "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1"
    )
    launched_end = launched_fields.find(
        "/// Sole parked lifecycle completion owner for this height.",
        launched_start,
    )
    launched_region = (
        launched_fields[launched_start:launched_end]
        if launched_start >= 0 and launched_end > launched_start
        else ""
    )
    launched_cursor = 0
    for token in (
        "services: ProductionV2Services",
        "pending_kura_apply_replay: Option<super::super::v2::PreparedRecoveredPendingKuraApplyReplayV1>",
        "recovered_local_proposal_attempt:",
        "Option<super::super::v2::RecoveredLifecycleLocalProposalAttemptV1>",
        "pending_lifecycle_completion: Option<PendingLifecycleCompletionV1>",
        "pending_ingress_capacity: Option<PendingIngressCapacityV1>",
        "completion_observer_activation: Option<ProductionV2CompletionObserverActivationPermitV1>",
        "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
    ):
        position = launched_region.find(token, launched_cursor)
        if position < 0:
            errors.append(
                f"{paths['launch']}: launched unified lifecycle Drop order must "
                f"retain ordered field {token!r}"
            )
            break
        launched_cursor = position + len(token)

    aperture_open_candidates = [
        rust_item
        for rust_item in rust_items(
            sources["preactivation_ingress"], "open_canonical_recovery_ingress"
        )
        if not rust_item.brace_context
    ]
    if len(aperture_open_candidates) != 1:
        errors.append(
            f"{paths['preactivation_ingress']}: canonical-recovery aperture "
            f"must retain one free open constructor; found "
            f"{len(aperture_open_candidates)}"
        )
        aperture_open = None
    else:
        aperture_open = aperture_open_candidates[0]
    require_order(
        "preactivation_ingress",
        aperture_open,
        "preactivation canonical-recovery ingress open",
        (
            "Arc::ptr_eq(block_ingress, launched_ingress)",
            "ingress_ready.load(Ordering::Acquire)",
            "block_ingress.state.lock().open",
            "block_ingress.open()",
            "ingress_ready.store(true, Ordering::Release)",
            "ProductionLifecycleCanonicalRecoveryIngressV1",
        ),
    )
    aperture_drop_items = [
        rust_item
        for rust_item in rust_items(sources["preactivation_ingress"], "drop")
        if len(rust_item.brace_context) == 1
        and rust_item.brace_context[0][:3] == ("impl", "Drop", "for")
        and "ProductionLifecycleCanonicalRecoveryIngressV1"
        in rust_item.brace_context[0]
    ]
    if len(aperture_drop_items) != 1:
        errors.append(
            f"{paths['preactivation_ingress']}: canonical-recovery aperture must "
            f"retain one RAII Drop; found {len(aperture_drop_items)}"
        )
    else:
        require_tokens(
            "preactivation_ingress",
            aperture_drop_items[0],
            "preactivation canonical-recovery ingress RAII close",
            ("self.close()",),
        )
    aperture_close = item("preactivation_ingress", "close")
    require_order(
        "preactivation_ingress",
        aperture_close,
        "preactivation canonical-recovery ingress close",
        (
            "self.ingress_ready.store(false, Ordering::Release)",
            "self.block_ingress.close()",
            "self.open = false",
        ),
    )
    aperture_transaction = item(
        "preactivation", "with_canonical_body_recovery_ingress_transaction"
    )
    require_order(
        "preactivation",
        aperture_transaction,
        "launched canonical-recovery aperture transaction",
        (
            "self.with_runner_setup_transaction",
            "activation.open_canonical_recovery_ingress(&launched_ingress)",
            "operation(&aperture, executor, services)",
            "aperture.close_and_verify()",
            "result",
        ),
    )
    require_tokens(
        "preactivation",
        item("preactivation", "with_canonical_body_recovery_ingress"),
        "ordinary preactivation canonical-recovery aperture",
        (
            "self.with_canonical_body_recovery_ingress_transaction(runner, activation, operation)",
        ),
    )
    if sources["lifecycle_run_inner"].count(
        "recover_canonical_bodies_before_activation("
    ) != 3:
        errors.append(
            f"{paths['lifecycle_run_inner']}: lifecycle startup must retain one "
            "canonical recovery helper and exactly two startup repair call sites"
        )

    runner_ingress_retire = item("runner", "retire_lifecycle_runner_ingress")
    require_order(
        "runner",
        runner_ingress_retire,
        "shared lifecycle runner ingress retirement",
        (
            "ingress_ready.store(false, Ordering::Release)",
            "block_ingress.close()",
            "Arc::ptr_eq(block_ingress, launched_ingress)",
        ),
    )
    for owner in (
        "ProductionLifecycleRunnerActivationV1",
        "ProductionLifecycleCompleteTipRunnerActivationV1",
    ):
        matches = [
            rust_item
            for rust_item in rust_items(
                sources["runner_authority"], "retire_unpublished"
            )
            if rust_item.brace_context == (("impl", owner),)
        ]
        if len(matches) != 1:
            errors.append(
                f"{paths['runner_authority']}: {owner} must retain one consuming "
                f"unpublished retirement; found {len(matches)}"
            )
        else:
            require_tokens(
                "runner_authority",
                matches[0],
                f"{owner} unpublished retirement",
                ("retire_lifecycle_runner_ingress(",),
            )

    shutdown_finish = item("launch", "finish_clean_shutdown")
    require_order(
        "launch",
        shutdown_finish,
        "lifecycle clean-shutdown tail",
        (
            "self.leader_wire_ingress_binding.retire()",
            "runner_retirement",
            "ingress_retirement",
            "let Some(operation) = operation",
            "self.services.allow_clean_shutdown()",
            "operation.complete()",
        ),
    )
    complete_tip_shutdown_tail = [
        rust_item
        for rust_item in rust_items(
            sources["launch"], "into_complete_tip_clean_shutdown"
        )
        if rust_item.brace_context
        == (("impl", "LaunchedProductionLifecycleV1"),)
    ]
    if len(complete_tip_shutdown_tail) != 1:
        errors.append(
            f"{paths['launch']}: CompleteTip lifecycle shutdown must retain "
            f"one sealed inner tail; found {len(complete_tip_shutdown_tail)}"
        )
    else:
        require_order(
            "launch",
            complete_tip_shutdown_tail[0],
            "CompleteTip lifecycle clean-shutdown tail",
            (
                "output_guard.begin_fail_stop_operation()",
                "runner.retire_unpublished(&self.leader_wire_ingress_binding.ingress)",
                "drop(retirement)",
                "self.finish_clean_shutdown(operation, runner_retirement)",
            ),
        )
    launched_shutdowns = [
        rust_item
        for rust_item in rust_items(sources["launch"], "into_clean_shutdown")
        if rust_item.brace_context == (("impl", "LaunchedProductionLifecycleV1"),)
    ]
    active_shutdowns = [
        rust_item
        for rust_item in rust_items(sources["launch"], "into_clean_shutdown")
        if rust_item.brace_context == (("impl", "ActivatedProductionLifecycleV1"),)
    ]
    for label, candidates, markers in (
        (
            "unpublished lifecycle clean shutdown",
            launched_shutdowns,
            (
                "output_guard.begin_fail_stop_operation()",
                "runner.retire_unpublished(&self.leader_wire_ingress_binding.ingress)",
                "self.finish_clean_shutdown(operation, runner_retirement)",
            ),
        ),
        (
            "active lifecycle clean shutdown",
            active_shutdowns,
            (
                "let Self { launched, local_proposal, runner_activation, } = self",
                "output_guard.begin_fail_stop_operation()",
                "runner_activation.retire(&launched.leader_wire_ingress_binding.ingress)",
                "drop(local_proposal)",
                "launched.finish_clean_shutdown(operation, runner_retirement)",
            ),
        ),
    ):
        if len(candidates) != 1:
            errors.append(
                f"{paths['launch']}: {label} must retain one consuming method; "
                f"found {len(candidates)}"
            )
            continue
        require_order("launch", candidates[0], label, markers)
        for forbidden in (
            "into_finalized_parts",
            "rollover_finalized_height_outputs",
            "stage_finalized_height_all_row_retirement",
            "finish_height(",
        ):
            if _token_sequence_count(
                rust_code_tokens(candidates[0].source), rust_code_tokens(forbidden)
            ):
                errors.append(
                    f"{paths['launch']}:{candidates[0].line}: {label} must not "
                    f"claim finality through {forbidden!r}"
                )

    complete_tip_setup = [
        rust_item
        for rust_item in rust_items(sources["ledger"], "with_runner_setup")
        if rust_item.brace_context
        == (("impl", "LaunchedRecoveredCompleteTipSuccessorLifecycleV1"),)
    ]
    if len(complete_tip_setup) != 1:
        errors.append(
            f"{paths['ledger']}: CompleteTip closed-ingress runner setup must "
            f"remain one sealed delegate; found {len(complete_tip_setup)}"
        )
    else:
        require_order(
            "ledger",
            complete_tip_setup[0],
            "CompleteTip sealed closed-ingress runner setup",
            (
                "runner",
                "operation",
                "E: From<super::launch::ProductionLifecyclePreActivationErrorV1>",
                "self.launched.with_runner_setup(runner, operation)",
            ),
        )

    complete_tip_shutdown = [
        rust_item
        for rust_item in rust_items(sources["ledger"], "into_clean_shutdown")
        if rust_item.brace_context
        == (("impl", "LaunchedRecoveredCompleteTipSuccessorLifecycleV1"),)
    ]
    if len(complete_tip_shutdown) != 1:
        errors.append(
            f"{paths['ledger']}: CompleteTip clean shutdown must remain one "
            f"sealed delegate; found {len(complete_tip_shutdown)}"
        )
    else:
        require_order(
            "ledger",
            complete_tip_shutdown[0],
            "CompleteTip sealed clean shutdown",
            (
                "let Self {",
                "launched",
                "retirement",
                "} = self",
                "launched.into_complete_tip_clean_shutdown(runner, retirement)",
            ),
        )

    shutdown_behavior = item(
        "startup_test",
        "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
    )
    require_tokens(
        "startup_test",
        shutdown_behavior,
        "production lifecycle clean-shutdown behavior",
        (
            ".with_runner_setup(&mut setup_runner, |executor, services|",
            "launch_non_pending_lifecycle_height_and_shutdown_for_test(",
            ".into_clean_shutdown(&mut runner)",
        ),
    )
    require_order(
        "startup_test",
        shutdown_behavior,
        "production unpublished lifecycle clean-shutdown behavior",
        (
            "if shutdown_before_activation",
            "launch_non_pending_lifecycle_height_and_shutdown_for_test(",
            "None",
            "assert!(!ingress_ready.load(Ordering::Acquire))",
            "assert!(!leader_wire_ingress.state.lock().open)",
            "assert!(!output_guard.restart_required())",
            "continue",
        ),
    )
    complete_tip_shutdown_behavior = item(
        "startup_test",
        "production_empty_genesis_complete_tip_adopts_control_repair_and_launches",
    )
    require_order(
        "startup_test",
        complete_tip_shutdown_behavior,
        "production CompleteTip lifecycle clean-shutdown behavior",
        (
            "production_empty_genesis_complete_tip_fixture_for_test()",
            "adapter.timeout_elapsed(adapter.current_tag())",
            "open_recovered_startup_with_aggregator(",
            "authenticated.has_recovered_control_sign_for_test()",
            "open_production_lifecycle_owner_v1(",
            "assert_ne!( repaired_successor, empty_successor",
            "launch_non_pending_lifecycle_height_and_activate_for_test(",
            "drain_lifecycle_v2_ingress(",
            "LifecycleProducerClaimDispositionV1::AwaitingCompletion",
            "loop",
            "drain_lifecycle_v2_ingress(",
            "LifecycleProducerClaimDispositionV1::Eligible",
            "assert_ne!( broadcast_successor, repaired_successor",
            ".into_clean_shutdown(&mut active_runner)",
            "assert!(!ingress_ready.load(Ordering::Acquire))",
            "assert!(!ingress_state.open)",
            "assert!(ingress_state.leader_wire_lifecycle_gate.is_none())",
            "assert!(!output_guard.restart_required())",
            "assert!(crate::sumeragi::status::v2_status().is_some())",
            "crate::sumeragi::status::clear_v2_status()",
            "assert!(crate::sumeragi::status::v2_status().is_none())",
        ),
    )
    require_order(
        "startup_test",
        shutdown_behavior,
        "production active lifecycle clean-shutdown behavior",
        (
            "if shutdown_after_activation",
            ".into_clean_shutdown(&mut runner)",
            "assert!(!ingress_ready.load(Ordering::Acquire))",
            "assert!(!leader_wire_ingress.state.lock().open)",
            "assert!(!output_guard.restart_required())",
            "crate::sumeragi::status::clear_v2_status()",
            "continue",
        ),
    )

    outcome_source = sources["driver"]
    completion_outcome_start = outcome_source.find(
        "pub(in crate::sumeragi) enum ProductionLifecycleCompletionTurnV1<'cursor>"
    )
    ingress_outcome_end = outcome_source.find(
        "impl LaunchedProductionLifecycleV1", completion_outcome_start
    )
    outcomes = (
        outcome_source[completion_outcome_start:ingress_outcome_end]
        if completion_outcome_start >= 0 and ingress_outcome_end > completion_outcome_start
        else ""
    )
    outcome_tokens = rust_code_tokens(outcomes)
    for token, count in (
        ("PassThrough(LifecycleCurrentRunnerTurn<'cursor>)", 2),
        ("Selected(ProductionLifecycleCompletionSelectionV1)", 2),
        ("Ordinary(LifecycleCurrentRunnerTurn<'cursor>)", 1),
        ("Ready(ProductionLifecycleReadyCompletionTurnV1<'cursor>)", 1),
        ("Selected(ProductionLifecycleIngressSelectionV1)", 1),
        ("Ordinary(ProductionPreparedOrdinaryIngressTurnV1)", 1),
    ):
        observed = _token_sequence_count(outcome_tokens, rust_code_tokens(token))
        if observed != count:
            errors.append(
                f"{paths['driver']}: borrow-bound lifecycle turn outcomes must "
                f"contain {token!r} exactly {count} time(s); found {observed}"
            )
    for forbidden in (
        "LifecycleRunnerRankSnapshot",
        "derive(Clone)",
        "derive(Copy)",
        "fn into_parts(",
    ):
        if _token_sequence_count(outcome_tokens, rust_code_tokens(forbidden)):
            errors.append(
                f"{paths['driver']}: borrow-bound lifecycle turn outcomes expose "
                f"forbidden token {forbidden!r}"
            )

    def launched_completion_item(
        name: str,
        description: str,
        *,
        expected_attributes: tuple[str, ...] = (),
    ):
        expected_context = (("impl", "LaunchedProductionLifecycleV1"),)
        matches = [
            rust_item
            for rust_item in rust_items(sources["driver"], name)
            if rust_item.brace_context == expected_context
        ]
        if len(matches) != 1:
            errors.append(
                f"{paths['driver']}: {description} must have one launched owner; "
                f"found {len(matches)}"
            )
            return None
        target = matches[0]
        _require_rust_item_context(
            paths["driver"],
            target,
            expected_context,
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        return target

    completion_pre_gate = launched_completion_item(
        "drive_completion_pre_gate_inner",
        "lifecycle Completion parked/physical pre-gate implementation",
    )
    ready_completion = launched_completion_item(
        "drive_ready_completion_turn_with_required_ordinal",
        "lifecycle Completion fresh Ready dispatcher",
    )
    completion = launched_completion_item(
        "drive_completion_turn_for_test",
        "test-only composed lifecycle Completion turn driver",
        expected_attributes=("#[cfg(test)]",),
    )
    if rust_items(sources["driver"], "drive_completion_turn"):
        errors.append(
            f"{paths['driver']}: superseded production Completion composition "
            "drive_completion_turn must be absent"
        )
    require_order(
        "driver",
        completion_pre_gate,
        "lifecycle Completion parked-owner and physical-head pre-gate order",
        (
            "self.pending_lifecycle_completion.take()",
            "match pending",
            "self.services.take_next_lifecycle_completion()",
            "ProductionLifecycleCompletionPreGateV1::Ready(",
        ),
    )
    require_tokens(
        "driver",
        completion_pre_gate,
        "lifecycle Completion physical-head ownership",
        (
            "LifecycleCompletionTakeV1::PassThrough",
            "LifecycleCompletionTakeV1::CertifiedServe(completion)",
        ),
    )
    require_order(
        "driver",
        completion_pre_gate,
        "lifecycle Completion unchanged-Validate-fence ordinary bypass",
        (
            "current_validate_fence_wait",
            "self.executor.lifecycle_reducer_fence_observation()",
            "fence.source() == wait.source()",
            "fence.generation() <= wait.observed_generation()",
            "prepare_ordinary_completion_behind_validate_fence()",
            "Ok(true)",
            "ProductionLifecycleCompletionPreGateV1::Ordinary(runner)",
        ),
    )
    if completion_pre_gate is not None:
        ordinary_returns = _token_sequence_count(
            rust_code_tokens(completion_pre_gate.source),
            rust_code_tokens("ProductionLifecycleCompletionPreGateV1::Ordinary(runner)"),
        )
        if ordinary_returns != 4:
            errors.append(
                f"{paths['driver']}:{completion_pre_gate.line}: lifecycle Completion "
                "pre-gate must return the exact ordinary cursor for a foreign runner "
                "rank, the unchanged-Validate-fence bypass, an unpermitted ordinary "
                "head, and an ordinary head whose Ready Proposal Sign is not exact; "
                f"found {ordinary_returns} sites"
            )
    require_order(
        "driver",
        ready_completion,
        "fresh lifecycle Completion Ready-work dispatch",
        (
            "self.owner.classify_completion_ready_work(fence)",
            "ProductionCompletionReadyWorkV1::PassThrough",
            "ProductionLifecycleCompletionTurnV1::PassThrough(runner)",
            "ProductionCompletionReadyWorkV1::CompletionIo",
            "dispatch_completion_with_runner_debt",
            "ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast",
            "refanout_recovered_lifecycle_signed_broadcast_with_runner_debt",
        ),
    )
    require_order(
        "driver",
        completion,
        "test-only composed lifecycle Completion pre-gate and Ready order",
        (
            "self.drive_completion_pre_gate(runner, lane_work)",
            "ProductionLifecycleCompletionPreGateV1::Selected(selected)",
            "ProductionLifecycleCompletionPreGateV1::Ordinary(runner)",
            "ProductionLifecycleCompletionPreGateV1::Ready(ready)",
            "self.drive_ready_completion_turn(ready)",
        ),
    )
    for target, token, count, label in (
        (
            completion_pre_gate,
            "self.services.take_next_lifecycle_completion()",
            1,
            "lifecycle Completion single physical-head classifier",
        ),
        (
            ready_completion,
            "self.owner.classify_completion_ready_work(fence)",
            1,
            "lifecycle Completion single fresh Ready census",
        ),
        (
            completion,
            "self.drive_completion_pre_gate(runner, lane_work)",
            1,
            "composed lifecycle Completion single pre-gate",
        ),
        (
            completion,
            "self.drive_ready_completion_turn(ready)",
            1,
            "composed lifecycle Completion single Ready dispatch",
        ),
    ):
        if target is None:
            continue
        observed = _token_sequence_count(
            rust_code_tokens(target.source), rust_code_tokens(token)
        )
        if observed != count:
            errors.append(
                f"{paths['driver']}:{target.line}: {label} must contain {token!r} "
                f"exactly {count} time(s); found {observed}"
            )

    completion_head = item("worker", "take_next_lifecycle_completion")
    require_order(
        "worker",
        completion_head,
        "unified physical Completion ordinary-head restoration",
        (
            "self.held_io_completion.take()",
            "match completion",
            "ordinary =>",
            "self.held_io_completion = Some(ordinary)",
            "LifecycleCompletionTakeV1::PassThrough",
        ),
    )
    if completion_head is not None:
        for forbidden in ("acknowledge_completion(&ordinary)", "ordinary.into_parts()"):
            if _token_sequence_count(
                rust_code_tokens(completion_head.source), rust_code_tokens(forbidden)
            ):
                errors.append(
                    f"{paths['worker']}:{completion_head.line}: unified physical "
                    f"Completion ordinary-head restoration found {forbidden!r}"
                )

    settlement_family = item("adapter", "settlement_family")
    require_tokens(
        "adapter",
        settlement_family,
        "publication-inert recovered Sign settlement family",
        (
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign",
            "wire::ConsensusMessageV2Payload::Proposal(proposal)",
            "wire::ConsensusMessageV2Payload::Vote(vote)",
            "_ => None",
        ),
    )
    sign_settlement = item("driver", "settle_parked_recovered_sign_completion")
    require_order(
        "driver",
        sign_settlement,
        "unified recovered Sign settlement routing",
        (
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::Broadcast",
            "self.settle_recovered_lifecycle_sign_broadcast()",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalPrepareWal",
            "self.settle_recovered_lifecycle_proposal_prepare_wal()",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::VoteBroadcastAndSign",
            "self.settle_recovered_lifecycle_vote_broadcast_and_sign()",
            "RecoveredLifecycleSignAdapterSettlementFamilyV1::ProposalBroadcastAndSign",
            "self.settle_recovered_lifecycle_proposal_broadcast_and_sign()",
        ),
    )
    sign_classification = item("driver", "classify_parked_recovered_sign_completion")
    require_order(
        "driver",
        sign_classification,
        "single-preview recovered Sign structural classification",
        (
            "completion.project_adapter_completion_authority()",
            "prepare_recovered_lifecycle_sign_completion(authority)",
            "preview.settlement_family()",
            "drop(preview)",
            "class",
        ),
    )

    fetch_phase_a = item("driver", "drive_recovered_ingress_selector")
    require_order(
        "driver",
        fetch_phase_a,
        "recovered Fetch Phase-A service failure",
        (
            "ProductionRecoveredDecisionFetchPersistenceErrorV1::Service",
            "drop(prepared)",
            "self.close_output_for_restart()",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ),
    )
    capacity_retry_items = [
        rust_item
        for rust_item in rust_items(sources["scheduler"], "retry")
        if rust_item.brace_context
        == (("impl", "PreparedProductionIngressCapacityWait"),)
    ]
    if len(capacity_retry_items) != 1:
        errors.append(
            f"{paths['scheduler']}: retained ingress capacity wait consuming "
            f"retry must have one owner; found {len(capacity_retry_items)}"
        )
        capacity_retry = None
    else:
        capacity_retry = capacity_retry_items[0]
    require_order(
        "scheduler",
        capacity_retry,
        "retained ingress capacity wait consuming retry",
        (
            "if self.mode != executor.lifecycle_mode_rank_snapshot()",
            "LifecycleIoCapacityWaitStatus::SamePending",
            "ProductionIngressCapacityRetry::Pending(self)",
            "LifecycleIoCapacityWaitStatus::Released",
            "ProductionIngressCapacityRetry::Released(selector)",
        ),
    )
    capacity_struct_start = sources["scheduler"].find(
        "pub(crate) struct PreparedProductionIngressCapacityWait"
    )
    capacity_struct_end = sources["scheduler"].find(
        "/// Opaque status of one service-owned capacity-generation wait.",
        capacity_struct_start,
    )
    capacity_region = (
        sources["scheduler"][capacity_struct_start:capacity_struct_end]
        if capacity_struct_start >= 0 and capacity_struct_end > capacity_struct_start
        else ""
    )
    for forbidden in (
        "#[derive(Clone)]",
        "pub(crate) selector: PreparedLifecycleIngressSelector",
        "fn selector(",
        "fn into_parts(",
    ):
        if forbidden in capacity_region:
            errors.append(
                f"{paths['scheduler']}: retained ingress capacity wait must "
                f"remain sealed; found {forbidden!r}"
            )
    ready_classifier = item("scheduler", "classify_completion_ready_classes")
    require_order(
        "scheduler",
        ready_classifier,
        "unified Completion Ready supported-coexistence order",
        (
            "LifecycleWorkClass::CertifiedServe",
            "LifecycleWorkClass::ProducerTurn",
            "ProductionCompletionReadyWorkV1::PassThrough",
            "LifecycleWorkClass::Broadcast",
            "ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast",
            "if classes.iter().all(|class|",
            "LifecycleWorkClass::Validate",
            "LifecycleWorkClass::Apply",
            "LifecycleWorkClass::Fetch",
            "ProductionCompletionReadyWorkV1::CompletionIo",
        ),
    )
    schedulable_broadcast_match = _require_qualified_rust_item(
        paths["registry"],
        sources["registry"],
        "SchedulableRetainedDirectBroadcastAttestationV1",
        "matches_schedulable_record",
        errors,
        "fence-schedulable direct Broadcast row rejoin",
    )
    require_order(
        "registry",
        schedulable_broadcast_match,
        "fence-schedulable direct Broadcast row rejoin",
        (
            "record.state == self.state",
            "record.work_class == LifecycleWorkClass::Broadcast",
            "record.owner == self.address.owner",
            "record.ordinal == self.address.ordinal",
            "exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())",
            "Some((self.address.slot, self.digest))",
        ),
    )
    schedulable_broadcast_carrier = item(
        "registry", "attest_schedulable_lifecycle_broadcast_carrier"
    )
    require_order(
        "registry",
        schedulable_broadcast_carrier,
        "fence-schedulable direct Broadcast carrier authentication",
        (
            "coordinator.fault.is_some() || coordinator.active_lease.is_some()",
            "coordinator.records.get(&ordinal)",
            "record.work_class != LifecycleWorkClass::Broadcast",
            "super::LifecycleState::Ready",
            "coordinator.ready_index.contains(&ordinal)",
            "attest_ready_lifecycle_broadcast_carrier(coordinator, ordinal)",
            "ReadyLifecycleBroadcastCarrierV1::RecoveredRefanout",
            "SchedulableLifecycleBroadcastCarrierV1::RecoveredRefanout",
            "super::LifecycleState::Waiting(wait)",
            "!coordinator.ready_index.contains(&ordinal)",
            "fence.source()",
            "super::projection::reducer_fence_wait_source(",
            "coordinator.active_context",
            "wait.source() == fence.source()",
            "wait.observed_generation() < fence.generation()",
            "coordinator.observed_generation.get(&wait.source())",
            "Some(&wait.observed_generation())",
            "exact_single_record_slot(record, LifecycleWorkClass::Broadcast.capacity_class())",
            "ConcreteWorkAddress::new(record.owner, ordinal, slot)",
            "self.entries.get(&address)",
            "work.digest != digest",
            "ConcreteLifecycleWorkKind::PendingAdapter",
            "lifecycle_output_row_matches(coordinator, address, work, effect, pending)",
            "SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput",
            "ConcreteLifecycleWorkKind::DurableRecoveredLifecycleSignedBroadcast(_)",
        ),
    )
    recovered_broadcast_match = _require_qualified_rust_item(
        paths["open_output"],
        sources["open_output"],
        "ReadyRecoveredLifecycleBroadcastAttestationV1",
        "matches_ready_record",
        errors,
        "cold-owner Ready Broadcast row rejoin",
    )
    require_order(
        "open_output",
        recovered_broadcast_match,
        "cold-owner Ready Broadcast row rejoin",
        (
            "record.owner == self.owner",
            "record.ordinal == self.ordinal",
            "record.key == self.key",
            "record.work_class == super::LifecycleWorkClass::Broadcast",
            "record.stage == self.stage",
            "record.state == super::LifecycleState::Ready",
            "record.physical_slots.len() == 1",
            "record.physical_slots.get(&self.slot) == Some(&self.digest)",
            "record.episode.slot_universe.len() == 1",
            "record.episode.slot_universe.contains(&self.slot)",
            "record.episode.consumed_slots == record.episode.slot_universe",
            "record.episode.frozen_predecessors.is_empty()",
        ),
    )
    recovered_broadcast_attestor = item(
        "open_output", "attest_ready_recovered_lifecycle_broadcast"
    )
    require_order(
        "open_output",
        recovered_broadcast_attestor,
        "cold-owner Ready Broadcast authentication",
        (
            "self.recovered_lifecycle_outputs.as_ref()?.entries.get(&ordinal)?",
            "let candidate = output.candidate()",
            "candidate.work_class != super::LifecycleWorkClass::Broadcast",
            "!self.coordinator.ready_index.contains(&ordinal)",
            "!recovered_output_matches_ready_coordinator( &self.verified, &self.coordinator, output, )",
            "candidate.physical_geometry.normalized().ok()?",
            "physical.first_key_value()?",
            "physical.len() != 1",
            "universe.len() != 1",
            "!universe.contains(&slot)",
            "consumed != universe",
            "ReadyRecoveredLifecycleBroadcastAttestationV1",
            "owner: output.owner()",
            "ordinal",
            "key: candidate.key",
            "stage: candidate.stage",
            "slot",
            "digest",
        ),
    )
    completion_broadcast_attestor = item(
        "scheduler", "attest_schedulable_completion_broadcast_carrier"
    )
    require_order(
        "scheduler",
        completion_broadcast_attestor,
        "exclusive registry-or-cold-owner Broadcast authentication",
        (
            "self.attest_ready_recovered_lifecycle_broadcast(ordinal)",
            "attest_schedulable_lifecycle_broadcast_carrier(&self.coordinator, ordinal, fence)",
            "(Some(attestation), Err(RegistryError::Missing))",
            "SchedulableCompletionBroadcastCarrierV1::RetainedRecoveredOutput(attestation)",
            "None, Ok(SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation))",
            "SchedulableCompletionBroadcastCarrierV1::RetainedDirectOutput(attestation)",
            "(None, Ok(SchedulableLifecycleBroadcastCarrierV1::RecoveredRefanout))",
            "SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout",
            "(None, Err(error)) => Err(error)",
            "(Some(_), Ok(_) | Err(_)) => Err(RegistryError::CorruptWork)",
        ),
    )
    schedulable_completion = item("scheduler", "classify_schedulable_completion_work")
    require_order(
        "scheduler",
        schedulable_completion,
        "fence-schedulable Completion Broadcast classification",
        (
            "for ordinal in schedulable",
            "record.work_class != LifecycleWorkClass::Broadcast",
            "self.attest_schedulable_completion_broadcast_carrier(*ordinal, fence)",
            "SchedulableCompletionBroadcastCarrierV1::RetainedDirectOutput(_)",
            "retained_outputs.insert(*ordinal)",
            "retained_direct_outputs.insert(*ordinal)",
            "SchedulableCompletionBroadcastCarrierV1::RetainedRecoveredOutput(_)",
            "retained_outputs.insert(*ordinal)",
            "SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout",
            "classes.push(record.work_class)",
            "oldest_is_retained_direct_output",
            "classify_completion_ready_classes(",
        ),
    )
    ready_work = item("scheduler", "classify_completion_ready_work")
    require_order(
        "scheduler",
        ready_work,
        "fence-schedulable Completion census construction",
        (
            "let exact_ready = self.coordinator.records.iter()",
            "matches!(record.state, LifecycleState::Ready)",
            "if exact_ready != self.coordinator.ready_index",
            "let mut schedulable = exact_ready",
            "matches!( record.state, LifecycleState::Waiting(wait)",
            "wait.source() == fence.source()",
            "wait.observed_generation() < fence.generation()",
            "self.classify_schedulable_completion_work(&schedulable, Some(fence))",
        ),
    )

    ordinary_capture = item("ingress", "capture_next_ingress_turn_cut")
    require_tokens(
        "ingress",
        ordinary_capture,
        "ordinary queue-owned fair winner capture wrapper",
        (
            """
self.capture_next_ingress_turn_cut_at(
    None,
    FairIngressTurnSelectionPolicy::OrdinaryRetireObsolete,
    predicate,
)
""",
        ),
    )

    bounded_capture = item("ingress", "capture_next_ingress_turn_cut_before")
    require_tokens(
        "ingress",
        bounded_capture,
        "explicit-cut exact-predicate fair winner capture wrapper",
        (
            """
self.capture_next_ingress_turn_cut_at(
    Some(physical_cut),
    FairIngressTurnSelectionPolicy::PredicateOnly,
    predicate,
)
""",
        ),
    )

    retiring_bounded_capture = item(
        "ingress",
        "capture_next_ingress_turn_cut_before_with_obsolete_retirement",
    )
    require_tokens(
        "ingress",
        retiring_bounded_capture,
        "explicit-cut obsolete-predecessor retirement capture wrapper",
        (
            """
self.capture_next_ingress_turn_cut_at(
    Some(physical_cut),
    FairIngressTurnSelectionPolicy::OrdinaryRetireObsolete,
    predicate,
)
""",
        ),
    )

    capture = item("ingress", "capture_next_ingress_turn_cut_at")
    require_tokens(
        "ingress",
        capture,
        "shared queue-owned fair winner capture",
        (
            "let service_guard = self.service_lock.lock()",
            "let mut state = self.state.lock()",
            """
let live_physical_cut = u128::from(state.last_admission_ordinal)
    .checked_add(1)
    .ok_or(FairIngressQueueCutError::PositionOverflow)?;
let physical_cut = requested_physical_cut.unwrap_or(live_physical_cut);
if physical_cut == 0 || physical_cut > live_physical_cut {
    return Err(FairIngressQueueCutError::InvalidPhysicalCut);
}
""",
            """
FairIngressTurnSelectionPolicy::PredicateOnly => !selector_occurrences.is_empty(),
""",
            "select_fair_v2_ingress_candidate(",
            """
matches!(
    selection_policy,
    FairIngressTurnSelectionPolicy::OrdinaryRetireObsolete
) && occurrence.is_obsolete()
""",
            "Ok(Some(FairIngressTurnCut {",
            "_service_guard: service_guard",
        ),
    )
    if capture is not None:
        capture_tokens = rust_code_tokens(capture.source)
        for token, count in (
            ("selected_physical_ordinal", 4),
            ("selected_disposition", 2),
        ):
            observed = _token_sequence_count(capture_tokens, rust_code_tokens(token))
            if observed != count:
                errors.append(
                    f"{paths['ingress']}:{capture.line}: queue-owned fair winner "
                    f"capture must contain {token!r} exactly {count} time(s); "
                    f"found {observed}"
                )
    require_order(
        "ingress",
        capture,
        "shared queue-owned fair winner lock, cut, and selection order",
        (
            "self.service_lock.lock()",
            "self.state.lock()",
            "let live_physical_cut = u128::from(state.last_admission_ordinal)",
            "let physical_cut = requested_physical_cut.unwrap_or(live_physical_cut)",
            "if physical_cut == 0 || physical_cut > live_physical_cut",
            "fair_v2_ingress_leader_wire_selector_projection(&state, true, Some(physical_cut))",
            "freeze_live_geometry(",
            "drop(state)",
            "validate_frozen_ownership_outside_state(",
            "select_fair_v2_ingress_candidate(",
            "FairIngressTurnCut {",
        ),
    )

    frozen_geometry = item("ingress", "freeze_live_geometry")
    require_order(
        "ingress",
        frozen_geometry,
        "fair-ingress geometry strictly excludes the supplied physical cut",
        (
            "physical_cut: u128",
            "for (index, entry) in lane.entries.iter().enumerate()",
            "if u128::from(entry.admission_ordinal) >= physical_cut",
            "continue",
            "freeze_geometry(&ready_prefix, lanes, physical_cut)",
        ),
    )

    narrow = item("ingress", "narrow_to_lifecycle")
    require_tokens(
        "ingress",
        narrow,
        "exact winner context narrowing",
        (
            "FairIngressTurnContextCut::Ordinary(self)",
            "mint_pending_identities(bound_context, &self.geometry)",
            "FairIngressTurnContextCut::Lifecycle(cut)",
        ),
    )
    widen = item("ingress", "into_ordinary_turn_cut")
    require_order(
        "ingress",
        widen,
        "exact current-context cut widening",
        (
            "let Self { queue, _service_guard, producer_publication_guard, physical_cut, bound_context, geometry, selector_occurrences, pending_identities: _, leader_wire_projection, selected_identity, selected_positions, selected_disposition, } = self",
            "let selected_physical_ordinal = selected_identity.physical_admission_ordinal",
            "source_for_frozen_ordinal(&geometry, selected_physical_ordinal)",
            ".position(|source| source == selected_source)",
            "FairIngressTurnCut {",
            "queue, _service_guard, producer_publication_guard, physical_cut, geometry, selector_occurrences, leader_wire_projection",
            "bound_context: Some(bound_context)",
            "selected_source_index, selected_physical_ordinal, selected_positions, selected_disposition",
        ),
    )
    exact_dequeue = item("ingress", "dequeue_exact_retaining")
    require_order(
        "ingress",
        exact_dequeue,
        "exact queue-owned physical dequeue",
        (
            "drop(std::mem::take(&mut self.selector_occurrences))",
            "let mut state = self.queue.state.lock()",
            "self.queue.dequeue_selected_locked(",
            "self.selected_source_index",
            "self.selected_physical_ordinal",
            "self.selected_disposition",
        ),
    )

    driver_items = [
        rust_item
        for rust_item in rust_items(sources["driver"], "drive_ingress_turn")
        if rust_item.brace_context
        == (("impl", "LaunchedProductionLifecycleV1"),)
    ]
    if len(driver_items) != 1:
        errors.append(
            f"{paths['driver']}: require exactly one launched queue-owned "
            f"drive_ingress_turn; found {len(driver_items)}"
        )
        driver = None
    else:
        driver = driver_items[0]
    require_order(
        "driver",
        driver,
        "ordinary/recovered ingress owner order",
        (
            "self.pending_ingress_capacity.take()",
            "self.executor.lifecycle_terminal_subject()",
            "capture_next_ingress_turn_cut(",
            "v2_ingress_head_can_drain(",
            "FairV2IngressDequeueDisposition::RetireObsolete",
            "selected_ingress_is_current_certified_serve(",
            "selected_ingress_is_certified_body_response(",
            "cut.narrow_to_lifecycle(expected_context)",
            "FairIngressTurnContextCut::Ordinary(cut)",
            "FairIngressTurnContextCut::Lifecycle(cut)",
            "classify_selected_certified_response_priority(&cut)",
            "SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority",
            "cut.into_ordinary_turn_cut()",
            "SelectedCertifiedResponsePriorityV1::OrdinaryClaimed",
            "capture_lifecycle_ingress_selector(cut)",
            "self.drive_certified_fetch_ingress_selector(selector, runner)",
            "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
            "prepare_recovered_decision_fetch_from_selected_cut(cut)",
            "self.drive_recovered_ingress_selector(selector, runner)",
        ),
    )
    if driver is not None:
        driver_tokens = rust_code_tokens(driver.source)
        for token, count in (
            (
                "dequeue_prepared_ordinary_ingress(",
                4,
            ),
            ("ProductionLifecycleIngressTurnV1::PassThrough(runner)", 2),
        ):
            observed = _token_sequence_count(driver_tokens, rust_code_tokens(token))
            if observed != count:
                errors.append(
                    f"{paths['driver']}:{driver.line}: ordinary exact-winner "
                    f"handoff must contain {token!r} exactly {count} time(s); "
                    f"found {observed}"
                )
    _require_rust_token_sequence(
        paths["driver"],
        driver,
        """
if !selected_ingress_is_certified_body_response(cut.selected_occurrence().inbound()) {
    return dequeue_prepared_ordinary_ingress(
        &ingress,
        cut,
        runner,
        None,
        terminal_subject,
        &self.services,
    );
}
""",
        "selected non-response winner bypasses response census",
        errors,
    )
    if driver is not None:
        capture_start = driver.source.find(".capture_next_ingress_turn_cut(")
        capture_end = driver.source.find("let Some(cut)", capture_start)
        pure_capture = (
            driver.source[capture_start:capture_end]
            if capture_start >= 0 and capture_end > capture_start
            else ""
        )
        if (
            "v2_ingress_head_can_drain" not in pure_capture
            or "prepare_certified_request" in pure_capture
            or "stage_certified_serve_rejection" in pure_capture
        ):
            errors.append(
                f"{paths['driver']}:{driver.line}: physical winner selection "
                "must use only the shared pure drain predicate"
            )

    serve_pre_admission = item(
        "ordinary_consumer", "prepare_current_certified_serve_pre_admission"
    )
    require_order(
        "ordinary_consumer",
        serve_pre_admission,
        "shared current Serve transport/authentication classifier",
        (
            "message.validate_version()",
            "wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request)",
            "request.round.height != active_height",
            "inbound.sender()",
            "inbound.reply_routes()",
            "inbound.ingress_ownership()",
            "reply_routes.semantic_target() != sender",
            "!ownership.validate_exact()",
            "!ownership.matches_message(inbound.message())",
            "!ownership.matches_semantic_origin(sender)",
            "!ownership.matches_reply_routes(Some(reply_routes))",
            "authenticate(request.clone(), sender)",
            "CurrentCertifiedServePreAdmissionV1::Negative",
            "certified_body_request_is_superseded_after_decision(",
            "CurrentCertifiedServePreAdmissionV1::AuthenticatedNegative",
            "CurrentCertifiedServePreAdmissionV1::Authenticated",
        ),
    )
    require_tokens(
        "ordinary_consumer",
        serve_pre_admission,
        "shared current Serve closed pre-admission result",
        (
            "CurrentCertifiedServePreAdmissionV1::Authenticated",
        ),
    )
    if serve_pre_admission is not None:
        pre_admission_tokens = rust_code_tokens(serve_pre_admission.source)
        for token, count in (
            ("CurrentCertifiedServePreAdmissionV1::Service(", 7),
            ("CurrentCertifiedServePreAdmissionV1::Negative", 1),
        ):
            observed = _token_sequence_count(
                pre_admission_tokens, rust_code_tokens(token)
            )
            if observed != count:
                errors.append(
                    f"{paths['ordinary_consumer']}:{serve_pre_admission.line}: "
                    f"shared current Serve classifier must contain {token!r} "
                    f"exactly {count} time(s); found {observed}"
                )
    reject_tokens(
        "ordinary_consumer",
        serve_pre_admission,
        "current Serve classifier owns no queue or service mutation",
        (
            "ProductionV2Services",
            "stage_certified_serve_rejection(",
            "prepare_certified_request(",
            "try_recv",
            "dequeue",
        ),
    )
    token_source = sources["driver"]
    token_start = token_source.find(
        "pub(in crate::sumeragi) struct ProductionPreparedOrdinaryIngressTurnV1"
    )
    token_end = token_source.find(
        "pub(in crate::sumeragi) enum ProductionLifecycleIngressTurnV1", token_start
    )
    token_region = (
        token_source[token_start:token_end]
        if token_start >= 0 and token_end > token_start
        else ""
    )
    for required in (
        "handoff: Option<PreparedDequeuedV2IngressV1>",
        "impl Drop for ProductionPreparedOrdinaryIngressTurnV1",
        "handoff.close_output_for_restart()",
    ):
        if required not in token_region:
            errors.append(
                f"{paths['driver']}: opaque ordinary token omits {required!r}"
            )
    for forbidden in (
        "pub handoff:",
        "pub(crate) handoff:",
        "pub(in crate::sumeragi) handoff:",
        "fn into_parts(",
        "fn services(",
        "fn executor(",
        "derive(Clone)",
        "derive(Copy)",
    ):
        if forbidden in token_region:
            errors.append(
                f"{paths['driver']}: opaque ordinary token exposes forbidden "
                f"surface {forbidden!r}"
            )

    selected_priority_start = sources["selector"].find(
        "pub(crate) enum SelectedCertifiedResponsePriorityV1 {"
    )
    selected_priority_end = sources["selector"].find(
        "impl LifecycleIngressSelectorError", selected_priority_start
    )
    selected_priority_region = (
        sources["selector"][selected_priority_start:selected_priority_end]
        if selected_priority_start >= 0 and selected_priority_end > selected_priority_start
        else ""
    )
    for token in (
        "SelectedCertifiedResponsePriorityV1",
        "DefinitelyNonPriority",
        "OrdinaryClaimed",
        "RecoveredClaimed",
    ):
        if token not in selected_priority_region:
            errors.append(
                f"{paths['selector']}: closed selected certified-response priority "
                f"enum omits {token!r}"
            )
    for source_name in ("selector", "driver"):
        if "selected_cut_is_recovered_decision_fetch" in sources[source_name]:
            errors.append(
                f"{paths[source_name]}: retired boolean certified-response selector remains"
            )

    selected_family = item(
        "selector", "classify_selected_certified_response_priority"
    )
    require_order(
        "selector",
        selected_family,
        "closed selected certified-response priority census",
        (
            "self.validate_lifecycle_ingress_selector_authority()",
            "cut.selected_identity().context() != context",
            "let selected_ordinal = cut.selected_identity().physical_admission_ordinal()",
            "let selected_request_hash = cut.selector_occurrences()",
            "return Ok(SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority)",
            "for occurrence in cut.selector_occurrences()",
            "occurrence.queue_gate() == FairV2IngressQueueGateVerdict::Blocked",
            "let drainable = occurrence.is_obsolete()",
            "message.validate_version().is_err()",
            "if !drainable || message.validate_version().is_err()",
            "response.request_hash != selected_request_hash",
            "probe_certified_response_priority(response, responder)",
            "Ok(CertifiedResponsePriorityProbe::DefinitelyNonPriority(_)) => continue",
            "PreparedCertifiedResponseCandidate::Ordinary(candidate)",
            "PreparedCertifiedResponseCandidate::Recovered(candidate)",
            "response_error_is_remote_nonpriority(&error) => continue",
            "Err(error) =>",
            "LifecycleIngressSelectorError::ExecutorAuthority",
            "response_candidates.insert(occurrence.physical_admission_ordinal(), candidate)",
            ".is_some()",
            "LifecycleIngressSelectorError::InvalidOccurrenceIdentity",
            "lowest_physical_ordinal_per_family(",
            "let mut selected_priority = SelectedCertifiedResponsePriorityV1::DefinitelyNonPriority",
            "revalidate_certified_response_priority_candidate(",
            "revalidate_recovered_decision_fetch_response_candidate(",
            "if !exact",
            "LifecycleIngressSelectorError::CandidateRevalidationDrift",
            "if ordinal == selected_ordinal",
            "SelectedCertifiedResponsePriorityV1::OrdinaryClaimed",
            "SelectedCertifiedResponsePriorityV1::RecoveredClaimed",
            "self.validate_lifecycle_ingress_selector_authority()",
            "if !cut.pre_cut_is_intact()",
            "Ok(selected_priority)",
        ),
    )
    family_prepare = item(
        "selector", "prepare_recovered_decision_fetch_from_selected_cut"
    )
    require_tokens(
        "selector",
        family_prepare,
        "selected-family Phase-A preparation",
        (
            "capture_lifecycle_ingress_selector_for_response_family( cut, Some(selected_request_hash), )",
            "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
        ),
    )

    for source_name, test_name in (
        (
            "ingress",
            "shared_selector_keeps_strict_dependency_blocked_and_obsolete_ordering",
        ),
        (
            "ingress",
            "turn_cut_dequeues_exact_winner_once_and_preserves_ready_rotation",
        ),
        ("ingress", "foreign_winner_dequeues_as_ordinary_without_reselection"),
        ("ingress", "ordinary_head_ignores_later_unowned_invalid_response"),
        (
            "driver",
            "armed_token_closes_output_before_releasing_dequeued_carrier_and_serve_result",
        ),
    ):
        item(source_name, test_name)
    wal_fetch = item(
        "wal_test", "bls_decision_fetch_repairs_and_coalesces_without_rewrite"
    )
    require_order(
        "wal_test",
        wal_fetch,
        "genuine recovered Fetch composite-dispatch behavior",
        (
            "add_recovered_next_vote_completion_for_test(0xCD)",
            "mixed_sign_ordinal > first_summary.0",
            "bind_body_store_to_lifecycle_completion_io_for_test(",
            "install_local_signer_for_test(",
            "dispatch_completion_for_test(",
            "ProductionCompletionDispatchV1::SignQueued",
            "lifecycle_completion_selection_is_exact_for_test(",
            "output_guard.close_admission_for_restart()",
            "output_guard.restart_required()",
            "drop(first)",
            "let mut reopened = reopened",
            "bind_body_store_to_lifecycle_completion_io_for_test(",
            "dispatch_completion_for_test(",
            "ProductionCompletionDispatchV1::FetchDispatched",
            "services.has_pending_exact_output()",
            "planner_io.detach(&mut services)",
        ),
    )
    composite_capture = item("worker", "capture_lifecycle_completion_capacity_census")
    require_order(
        "worker",
        composite_capture,
        "joint lifecycle Completion physical-corridor census",
        (
            "for probe in probes",
            "let fanout = self.recovered_decision_fetch_fanout(&owner)?",
            "begin_fail_stop_operation()",
            "let pending = self.lock_pending_exact_output()?",
            "let state = io.command_tx.queue.lock()",
            "for candidate in census.candidates.values_mut()",
        ),
    )
    require_tokens(
        "worker",
        composite_capture,
        "joint lifecycle Completion physical-corridor census",
        (
            "LifecycleCompletionCapacityProbeV1::Validate",
            "LifecycleCompletionCapacityProbeV1::Apply",
            "LifecycleCompletionCapacityProbeV1::Sign",
            "LifecycleCompletionCapacityProbeV1::Fetch",
            "pending.can_enqueue(fanout)",
        ),
    )
    composite_dispatch = item(
        "scheduler", "dispatch_completion_with_runner_debt_and_required_ordinal"
    )
    require_order(
        "scheduler",
        composite_dispatch,
        "all-row recovered Completion authentication and selection",
        (
            "let current_ready = self.coordinator.ready_index.clone()",
            "let mut exact_ready = current_ready",
            "for ordinal in &exact_ready",
            "capture_lifecycle_completion_capacity_census(probes)",
            "authenticated_ready_row_with_physical_capacity(",
            "let inputs = authenticated_scheduler_inputs(",
            "self.coordinator.plan_turn(inputs)",
            "let ordinal = lease.ordinal()",
            "match expected_class",
        ),
    )
    require_tokens(
        "scheduler",
        composite_dispatch,
        "all-row recovered Completion authentication and selection",
        (
            "census.select_validate(ordinal)",
            "census.select_apply(ordinal)",
            "census.select_sign(ordinal)",
            "census.select_fetch(ordinal)",
            "registration.commit(prepared, wait_source)",
            "output.commit()",
        ),
    )
    require_order(
        "scheduler",
        composite_dispatch,
        "fence-schedulable direct and cold-owner Broadcast dispatch authentication",
        (
            "LifecycleWorkClass::Broadcast",
            "self.attest_schedulable_completion_broadcast_carrier(*ordinal, Some(fence))",
            "SchedulableCompletionBroadcastCarrierV1::RetainedDirectOutput",
            "AuthenticatedLifecycleCompletionReadyV1::RetainedDirectBroadcast",
            "SchedulableCompletionBroadcastCarrierV1::RetainedRecoveredOutput",
            "AuthenticatedLifecycleCompletionReadyV1::RetainedRecoveredBroadcast",
            "SchedulableCompletionBroadcastCarrierV1::RecoveredRefanout",
            "let retained_direct_output = matches!",
            "AuthenticatedLifecycleCompletionReadyV1::RetainedDirectBroadcast",
            "AuthenticatedLifecycleCompletionReadyV1::RetainedRecoveredBroadcast",
            "if retained_direct_output",
            "AuthenticatedLifecycleCompletionReadyV1::RetainedDirectBroadcast",
            "authenticated_schedulable_retained_direct_broadcast_row( &factory, record, attestation, live_debts, )",
            "AuthenticatedLifecycleCompletionReadyV1::RetainedRecoveredBroadcast",
            "authenticated_ready_recovered_lifecycle_broadcast_row( &factory, record, attestation, live_debts, )",
            "let generations = if reducer_fence_wakes.is_empty()",
            "BTreeMap::from([(fence.source(), fence.generation())])",
            "let inputs = authenticated_scheduler_inputs(factory, generations, ready_rows)",
        ),
    )
    if composite_dispatch is not None:
        census_releases = _token_sequence_count(
            rust_code_tokens(composite_dispatch.source),
            rust_code_tokens("census.complete_without_selection()"),
        )
        if census_releases != 4:
            errors.append(
                f"{paths['scheduler']}:{composite_dispatch.line}: unified Completion "
                "dispatch must release its physical census on idle, direct Validate, ordinary Fetch, "
                f"and ordinary Store paths; found {census_releases} release sites"
            )
        physical_rows = _token_sequence_count(
            rust_code_tokens(composite_dispatch.source),
            rust_code_tokens("authenticated_ready_row_with_physical_capacity("),
        )
        if physical_rows != 4:
            errors.append(
                f"{paths['scheduler']}:{composite_dispatch.line}: all-row recovered "
                f"Completion authentication and selection must project exactly four "
                f"physical row classes; found {physical_rows}"
            )
    physical_row = item("schema", "from_authenticated_with_physical_capacity")
    require_tokens(
        "schema",
        physical_row,
        "authenticated Ready physical-capacity bit",
        (
            "Self::from_authenticated(",
            "row.physical_capacity_available = physical_capacity_available",
        ),
    )
    if physical_row is not None:
        physical_capacity_tokens = _token_sequence_count(
            rust_code_tokens(physical_row.source),
            rust_code_tokens("physical_capacity_available"),
        )
        if physical_capacity_tokens != 3:
            errors.append(
                f"{paths['schema']}:{physical_row.line}: authenticated Ready "
                "physical-capacity bit must remain parameter, assignment target, "
                f"and assignment source; found {physical_capacity_tokens}"
            )
    require_order(
        "driver",
        ready_completion,
        "fresh lifecycle Completion Ready composite dispatch",
        (
            "ProductionCompletionReadyWorkV1::CompletionIo",
            "owner.dispatch_completion_with_runner_debt(",
            "if let Err(error) = &result",
            "ProductionLifecycleCompletionSelectionV1::CompletionIoDispatch(result)",
        ),
    )
    behavior_items = {}
    for source_name, test_name in (
        (
            "worker",
            "lifecycle_completion_capacity_census_selects_once_and_drops_fail_stop",
        ),
        (
            "scheduler",
            "composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other",
        ),
        (
            "scheduler",
            "composite_recovered_completion_capacity_unavailable_claims_no_ready_sign",
        ),
        (
            "wal_test",
            "bls_decision_fetch_repairs_and_coalesces_without_rewrite",
        ),
    ):
        behavior_items[test_name] = item(source_name, test_name)
    require_order(
        "scheduler",
        behavior_items[
            "composite_recovered_completion_dispatches_one_ranked_sign_and_preserves_the_other"
        ],
        "composite recovered Completion Sign selection behavior",
        (
            "dispatch_completion_with_runner_debt(&mut services, &mut executor, 0,)",
            "ProductionCompletionDispatchV1::SignQueued { ordinal: paired }",
            "state.records[&paired].state",
            "LifecycleState::Claimed(_)",
            "state.records[&unrelated].state",
            "LifecycleState::Ready",
            "state.active_lease.is_some()",
            "state.fault.is_none()",
            "!output_guard.restart_required()",
        ),
    )
    require_source_order(
        "scheduler",
        "fence-schedulable direct Broadcast coexistence behavior",
        (
            "prospectively_woken_direct_broadcast_is_authenticated_and_sign_is_selected",
            "defer_direct_timeout_broadcast_for_test(0x71)",
            "park_direct_broadcast_before_fence_for_test(direct, fence)",
            "owner.classify_completion_ready_work(fence)",
            "ProductionCompletionReadyWorkV1::CompletionIo",
            "dispatch_completion_with_runner_debt(&mut services, &mut executor, 0)",
            "ProductionCompletionDispatchV1::SignQueued { ordinal: paired }",
            "state.records[&direct].state",
            "LifecycleState::Ready",
            "state.ready_index.contains(&direct)",
            "state.fault.is_none()",
            "!output_guard.restart_required()",
        ),
    )
    require_source_order(
        "scheduler",
        "fence-schedulable direct Broadcast tamper rejection",
        (
            "prospectively_woken_direct_broadcast_rejects_a_mismatched_carrier",
            "defer_direct_timeout_broadcast_for_test(0x73)",
            "park_direct_broadcast_before_fence_for_test(direct, fence)",
            "corrupt_ready_digest_for_test(direct)",
            "owner.classify_completion_ready_work(fence)",
            "ProductionCompletionReadyWorkV1::Invalid",
            "!output_guard.restart_required()",
        ),
    )
    cold_broadcast_retention = item(
        "ledger_recovery_test",
        "cold_broadcast_source_retention_preserves_ready_row_until_exact_acceptance",
    )
    require_order(
        "ledger_recovery_test",
        cold_broadcast_retention,
        "cold-owner Broadcast absence, retention, and terminal progress",
        (
            "owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None)",
            "ProductionCompletionReadyWorkV1::PassThrough",
            "owner.recovered_lifecycle_outputs.take()",
            "owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None)",
            "ProductionCompletionReadyWorkV1::Invalid",
            "owner.recovered_lifecycle_outputs = Some(recovered_outputs)",
            "LifecycleOutputServiceDispositionV1::SourceRetained",
            "RecoveredLifecycleOutputSettlementV1::SourceRetained",
            "owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None)",
            "ProductionCompletionReadyWorkV1::PassThrough",
            "LifecycleOutputServiceDispositionV1::Accepted",
            "RecoveredLifecycleOutputSettlementV1::Completed",
            "!owner.has_recovered_lifecycle_outputs()",
            "LifecycleState::Terminal(TerminalOutcome::Advanced)",
        ),
    )
    cold_broadcast_ordering = item(
        "ledger_recovery_test",
        "later_cold_broadcast_stays_passive_until_an_older_fetch_retires",
    )
    require_order(
        "ledger_recovery_test",
        cold_broadcast_ordering,
        "older Fetch progress with a passive later cold-owner Broadcast",
        (
            "let fetch_ordinal = 1",
            "let broadcast_ordinal = 2",
            "owner.settle_next_recovered_lifecycle_output",
            "RecoveredLifecycleOutputSettlementV1::Deferred",
            "calls.get()",
            "owner.classify_schedulable_completion_work(&owner.coordinator.ready_index, None)",
            "ProductionCompletionReadyWorkV1::CompletionIo",
            "staged.finish_terminal(fetch_ordinal, TerminalOutcome::Cancelled)",
            "owner.coordinator.persist_exact_staged_successor(&staged)",
            "owner.registry.registry_mut().rollback_exact(fetch_address, fetch_digest)",
            "owner.coordinator = staged",
            "owner.settle_next_recovered_lifecycle_output",
            "LifecycleOutputServiceDispositionV1::Accepted",
            "RecoveredLifecycleOutputSettlementV1::Completed",
            "calls.get()",
            "!owner.has_recovered_lifecycle_outputs()",
            "owner.coordinator.records[&broadcast_ordinal].state",
            "LifecycleState::Terminal(TerminalOutcome::Advanced)",
        ),
    )
    require_order(
        "scheduler",
        behavior_items[
            "composite_recovered_completion_capacity_unavailable_claims_no_ready_sign"
        ],
        "composite recovered Completion capacity-unavailable behavior",
        (
            "planner_io.saturate_consensus_prefix(&services)",
            "let before = owner.recovered_broadcast_scheduler_state_for_test(broadcast)",
            "ProductionCompletionDispatchV1::CapacityUnavailable",
            "owner.recovered_broadcast_scheduler_state_for_test(broadcast)",
            "before.records[&paired].state",
            "LifecycleState::Ready",
            "before.records[&unrelated].state",
            "LifecycleState::Ready",
            "!output_guard.restart_required()",
        ),
    )
    require_order(
        "worker",
        behavior_items[
            "lifecycle_completion_capacity_census_selects_once_and_drops_fail_stop"
        ],
        "lifecycle Completion worker Fetch ownership behavior",
        (
            "LifecycleCompletionCapacityProbeV1::Fetch",
            "fetch_census.select_fetch(13)",
            "returned_owner.dispatch_key()",
            "output.abort_before_claim()",
            "!output_guard.restart_required()",
        ),
    )
    startup_source = sources["startup_test"]
    for token in (
        "an exact ordinary winner cannot return the unchanged cursor",
        "an ordinary head cannot be poisoned by a later response family",
        "consume_prepared_ordinary_ingress_turn",
        "invalid-signature response is a drainable ordinary winner",
        "current certified Serve rejection must own ingress",
        "backpressured certified Serve remains lifecycle-owned",
        "released auxiliary capacity must admit exact Serve",
        "released certified Serve must enter lifecycle dispatch directly",
        "ProductionPreparedCertifiedServeTestSettlementV1::Rejected(reason)",
        "terminal Serve replay completion requires lifecycle restart",
        "completed Serve must release one adjacent ProducerTurn",
        "drain_lifecycle_v2_ingress(",
        "consume the exact ordinary runner handoff",
    ):
        if token not in startup_source:
            errors.append(
                f"{paths['startup_test']}: real-cursor ordinary ingress "
                f"regression omits {token!r}"
            )
    if ".drive_ingress_turn(" in sources["runner"]:
        errors.append(
            f"{paths['runner']}: run_inner must enter the lifecycle child instead "
            "of bypassing its owner through a direct ingress-driver call"
        )

    prepared_owner_start = sources["ordinary_consumer"].find(
        "pub(in crate::sumeragi) struct PreparedDequeuedV2IngressV1"
    )
    prepared_owner_end = sources["ordinary_consumer"].find(
        "/// Non-permit fail-stop scope", prepared_owner_start
    )
    prepared_owner = (
        sources["ordinary_consumer"][prepared_owner_start:prepared_owner_end]
        if prepared_owner_start >= 0 and prepared_owner_end > prepared_owner_start
        else ""
    )
    for required in (
        "ingress: Arc<FairV2Ingress>",
        "inbound: Option<InboundBlockMessage>",
        "disposition: FairV2IngressDequeueDisposition",
        "prepared_serve: Option<ProductionPreparedCertifiedServeV1>",
        "terminal_subject: Option<wire::BlockSubject>",
        "output_guard: Arc<ConsensusOutputGuard>",
        "armed: bool",
        "impl Drop for PreparedDequeuedV2IngressV1",
        "self.output_guard.close_admission_for_restart()",
    ):
        if required not in prepared_owner:
            errors.append(
                f"{paths['ordinary_consumer']}: opaque already-dequeued ordinary "
                f"owner omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub ingress:",
        "pub inbound:",
        "pub disposition:",
        "pub prepared_serve:",
        "pub terminal_subject:",
        "pub output_guard:",
        "pub armed:",
        "fn into_parts(",
        "fn inbound(",
        "fn prepared_serve(",
    ):
        if forbidden in prepared_owner:
            errors.append(
                f"{paths['ordinary_consumer']}: opaque already-dequeued ordinary "
                f"owner exposes forbidden surface {forbidden!r}"
            )

    consumer_fail_stop_start = sources["ordinary_consumer"].find(
        "struct PreparedDequeuedV2IngressFailStopScopeV1"
    )
    consumer_fail_stop_end = sources["ordinary_consumer"].find(
        "/// Settle a prepared Serve", consumer_fail_stop_start
    )
    consumer_fail_stop = (
        sources["ordinary_consumer"][consumer_fail_stop_start:consumer_fail_stop_end]
        if consumer_fail_stop_start >= 0
        and consumer_fail_stop_end > consumer_fail_stop_start
        else ""
    )
    for required in (
        "output_guard: Arc<ConsensusOutputGuard>",
        "armed: bool",
        "impl Drop for PreparedDequeuedV2IngressFailStopScopeV1",
        "if self.armed",
        "self.output_guard.close_admission_for_restart()",
    ):
        if required not in consumer_fail_stop:
            errors.append(
                f"{paths['ordinary_consumer']}: ordinary runner-tail non-permit "
                f"fail-stop scope omits {required!r}"
            )
    if "ConsensusFailStopOperation" in consumer_fail_stop:
        errors.append(
            f"{paths['ordinary_consumer']}: ordinary runner-tail fail-stop scope "
            "must not retain an output read permit across nested service work"
        )

    ordinary_consumer = item(
        "ordinary_consumer", "consume_prepared_dequeued_v2_ingress"
    )
    require_order(
        "ordinary_consumer",
        ordinary_consumer,
        "single exact ordinary post-dequeue runner tail",
        (
            "prepared.matches_output_guard(&services_output_guard)",
            "prepared.matches_ingress(receiver)",
            "let initial_admission = services_output_guard.acquire()",
            "drop(initial_admission)",
            "let mut inbound = prepared.inbound.take()",
            "let mut prepared_serve = prepared.prepared_serve.take()",
            "PreparedDequeuedV2IngressFailStopScopeV1::new",
            "macro_rules! finish",
            "let final_admission = services_output_guard.acquire()",
            "fail_stop.complete()",
            "prepared.complete()",
            "drop(final_admission)",
            "match inbound.message()",
        ),
    )
    require_tokens(
        "ordinary_consumer",
        ordinary_consumer,
        "single exact ordinary post-dequeue runner tail",
        (
            "BlockMessage::KuraReplicaAdvert(_) =>",
            "BlockMessage::LaneHistoricalRecoveryResponse(_) => { let _ = lane_work.accept_lane_message_with_ingress_ownership(",
            "FairV2IngressDequeueDisposition::RetireObsolete",
            "wire::ConsensusMessageV2Payload::Proposal(proposal)",
            "wire::ConsensusMessageV2Payload::CertifiedBodyRequest(request)",
            "ProductionPreparedCertifiedServeV1::Rejected(reason)",
            "wire::ConsensusMessageV2Payload::CertifiedBodyResponse(response)",
            "wire::ConsensusMessageV2Payload::CommitCertificateRequest(request)",
            "wire::ConsensusMessageV2Payload::CommitCertificateResponse(response)",
        ),
    )
    if ordinary_consumer is not None:
        ordinary_tokens = rust_code_tokens(ordinary_consumer.source)
        for forbidden in ("FnOnce", "callback", "into_parts("):
            if forbidden in ordinary_consumer.source:
                errors.append(
                    f"{paths['ordinary_consumer']}:{ordinary_consumer.line}: "
                    f"ordinary runner tail exposes forbidden seam {forbidden!r}"
                )

    decided_pre_admission = item("runner", "prepare_decided_lane_recovery_ingress")
    require_order(
        "runner",
        decided_pre_admission,
        "terminal recovery classifies exact current Serve for guarded service",
        (
            "inbound.message().is_lane_local()",
            "BlockMessage::V2(message)",
            "ConsensusMessageV2Payload::CertifiedBodyRequest(request)",
            "request.round.height < active_height",
            "DecidedLaneRecoveryIngressPreparation::HistoricalServe",
            "request.round.height == active_height",
            "DecidedLaneRecoveryIngressPreparation::CurrentServe",
            "DecidedLaneRecoveryIngressPreparation::LeaderWireRetire",
        ),
    )
    reject_tokens(
        "runner",
        decided_pre_admission,
        "terminal recovery classifier owns no current-Serve authentication or dequeue",
        (
            "prepare_current_certified_serve_pre_admission(",
            "authenticate_certified_body_request(",
            "try_recv",
        ),
    )
    decided_authorization = item("runner", "authorize_decided_lane_recovery_drain")
    require_order(
        "runner",
        decided_authorization,
        "terminal recovery authorizes exact current-Serve service",
        (
            "DecidedLaneRecoveryIngressPreparation::CurrentServe",
            "DecidedLaneRecoveryDrainAuthorization::CurrentServe",
            "DecidedLaneRecoveryIngressPreparation::HistoricalServe",
            "DecidedLaneRecoveryDrainAuthorization::HistoricalServe",
            "DecidedLaneRecoveryIngressPreparation::LeaderWireRetire",
            "DecidedLaneRecoveryDrainAuthorization::LeaderWireRetire",
        ),
    )
    reject_tokens(
        "runner",
        decided_authorization,
        "terminal recovery cannot mint coordinator-owned Serve authority",
        (
            "ProductionPreparedCertifiedServeV1",
            "CertifiedServeAdmission",
            "prepare_exact(",
        ),
    )
    blocked_selector = item(
        "runner", "select_blocked_ordinary_lane_local_ingress"
    )
    require_order(
        "runner",
        blocked_selector,
        "blocked ordinary ingress selects only lane-local fair ingress",
        (
            "receiver.try_recv_lifecycle_lane_local_checked(permit)",
            ".map_err(V2RunnerError::Service)",
        ),
    )
    reject_tokens(
        "runner",
        blocked_selector,
        "blocked ordinary lane-local selection owns no global or Apply authority",
        (
            "try_recv_if_checked(",
            "drain_decided_lane_recovery_ingress(",
            "CertifiedBodyRequest",
            "KuraReplicaAdvert",
        ),
    )
    blocked_drain = item("runner", "drain_blocked_ordinary_lane_local_ingress")
    require_order(
        "runner",
        blocked_drain,
        "blocked ordinary ingress verifies and commits exactly one lane-local occurrence",
        (
            "select_blocked_ordinary_lane_local_ingress(receiver, permit)?",
            "inbound.message().is_lane_local()",
            "return Err(V2RunnerError::Service(",
            "lane_work.accept_lane_message_with_ingress_ownership(inbound, active_view)",
            "Ok(true)",
        ),
    )
    decided_commit = item("runner", "commit_decided_lane_recovery_drain")
    require_order(
        "runner",
        decided_commit,
        "terminal current Serve binds ownership before guarded service",
        (
            "DecidedLaneRecoveryDrainAuthorization::CurrentServe",
            "committer.bind_leader_wire()?",
            "committer.commit_current_serve()?",
            "DecidedLaneRecoveryDrainCommitOutcome::CurrentServe",
            "DecidedLaneRecoveryDrainAuthorization::HistoricalServe",
            "committer.bind_leader_wire()?",
            "committer.commit_historical_serve()?",
        ),
    )
    decided_height_scope = item("runner", "permits_height")
    require_order(
        "runner",
        decided_height_scope,
        "terminal certified Serve exact height scope",
        (
            "Self::Current => request == active",
            "Self::Historical => request < active",
        ),
    )
    decided_subject_scope = item("runner", "permits_subject")
    require_order(
        "runner",
        decided_subject_scope,
        "terminal current Serve exact decided-subject scope",
        (
            "Self::Current => request == decided",
            "Self::Historical => true",
        ),
    )
    decided_serve = item("runner", "commit_certified_serve")
    require_order(
        "runner",
        decided_serve,
        "terminal certified Serve guarded durable response",
        (
            "self.take_inbound()?",
            "self.take_bound_leader_wire()?",
            "message.validate_version()",
            "ConsensusMessageV2Payload::CertifiedBodyRequest(request)",
            "scope.permits_height(request.round.height, self.executor.context().height)",
            "if !scope.permits_subject(request.subject, self.decided_subject)",
            "mark_leader_wire_volatile(self.receiver, &ingress_ownership)?",
            "return Ok(())",
            "let Some(reply_routes) = reply_routes",
            "reply_routes.semantic_target() != &sender",
            "let response_peer = sender.clone()",
            "let terminal_ownership = ingress_ownership.clone()",
            "serve_block_sync_while_guarded(",
            "block_sync_server.serve_historical_body(kura, request, &sender, local_key)",
            "post_durable_history_response_on_reply_routes_with_permit(",
            "response_peer",
            "reply_routes",
            "ingress_ownership",
            "response",
            "permit",
            "finalize_bound_block_sync_serve(",
            "|| mark_leader_wire_volatile(self.receiver, &terminal_ownership)",
        ),
    )
    decided_drain = item("runner", "drain_decided_lane_recovery_ingress")
    require_order(
        "runner",
        decided_drain,
        "live terminal drain directly serves authorized current recovery",
        (
            "let decided_subject = executor",
            ".local_proposal_directive()?",
            ".decided_subject()",
            "receiver.try_recv_if_checked(",
            "prepare_decided_lane_recovery_ingress(inbound, executor.context().height)",
            "authorize_decided_lane_recovery_drain(preparation)",
            "authorization.replace(candidate)",
            "ProductionDecidedLaneRecoveryDrainCommitter",
            "decided_subject,",
            "commit_decided_lane_recovery_drain(authorization, &mut committer)",
        ),
    )
    reject_tokens(
        "runner",
        decided_drain,
        "terminal recovery has no retained current-Serve branch",
        (
            "CurrentServeRetain",
            "DecidedLaneRecoveryDrainDecision",
        ),
    )
    current_serve_test = item(
        "runner_test", "drain_decided_lane_recovery_ingress_authorizes_terminal_current_serve"
    )
    require_order(
        "runner_test",
        current_serve_test,
        "terminal current Serve height and decided-subject behavior",
        (
            "let subject = proposal_subject(b\"decided recovery exact subject\")",
            "DecidedLaneRecoveryIngressPreparation::CurrentServe",
            "DecidedLaneRecoveryDrainAuthorization::CurrentServe",
            ".permits_height(context.height, context.height)",
            "DecidedLaneRecoveryServeScope::Current.permits_subject(subject, subject)",
            "proposal_subject(b\"losing decided recovery subject\")",
            "DecidedLaneRecoveryServeScope::Historical.permits_subject(",
        ),
    )
    current_serve_commit_test = item(
        "runner_test", "terminal_current_serve_binds_leader_wire_before_guarded_service"
    )
    require_order(
        "runner_test",
        current_serve_commit_test,
        "terminal current Serve checked commit behavior",
        (
            "DecidedLaneRecoveryDrainAuthorization::CurrentServe",
            "DecidedLaneRecoveryDrainCommitOutcome::CurrentServe",
            "assert_eq!(probe.0, [\"bind\", \"current\"])",
        ),
    )
    lifecycle_consumer = item("driver", "consume_prepared_ordinary_ingress_turn")
    require_order(
        "driver",
        lifecycle_consumer,
        "activated lifecycle ordinary ingress shares the runner tail",
        (
            "turn.handoff.take()",
            "self.launched.close_output_for_restart()",
            "let LaunchedProductionLifecycleV1 { executor, services, leader_wire_ingress_binding, .. }",
            "consume_prepared_dequeued_v2_ingress(",
        ),
    )
    if lifecycle_consumer is not None:
        for forbidden in ("FnOnce", "callback", "into_parts("):
            if forbidden in lifecycle_consumer.source:
                errors.append(
                    f"{paths['driver']}:{lifecycle_consumer.line}: activated ordinary "
                    f"consumer exposes forbidden seam {forbidden!r}"
                )

    apply_completion_cut = item(
        "height_driver", "completion_selection_stops_batch"
    )
    require_tokens(
        "height_driver",
        apply_completion_cut,
        "terminal Apply completion batch cut",
        (
            "ProductionLifecycleCompletionSelectionV1::LifecycleDecisionApplyApplied",
        ),
    )
    apply_ingress_barrier = item("height_driver", "blocks_ingress")
    require_tokens(
        "height_driver",
        apply_ingress_barrier,
        "typed Apply ingress barrier",
        (
            "Self::AwaitingCompletion | Self::AwaitingValidateSidecar | Self::AwaitingApplyCompletion | Self::ApplyTerminalSettled | Self::AwaitingReplayCompletion",
        ),
    )
    apply_yield_barriers = [
        rust_item
        for rust_item in rust_items(sources["height_driver"], "requires_yield")
        if rust_item.brace_context
        == (("impl", "LifecycleProducerClaimDispositionV1"),)
    ]
    if len(apply_yield_barriers) != 1:
        errors.append(
            f"{paths['height_driver']}: durable post-Apply rollover barrier "
            "must retain exactly one producer-claim requires_yield projection; "
            f"found {len(apply_yield_barriers)}"
        )
        apply_yield_barrier = None
    else:
        apply_yield_barrier = apply_yield_barriers[0]
    require_tokens(
        "height_driver",
        apply_yield_barrier,
        "durable post-Apply rollover barrier does not force a completion yield",
        ("!matches!(self, Self::Eligible | Self::ApplyTerminalSettled)",),
    )
    apply_runtime_barrier = item("height_driver", "blocks_runtime")
    require_tokens(
        "height_driver",
        apply_runtime_barrier,
        "typed Apply runtime barrier",
        ("Self::AwaitingApplyCompletion | Self::ApplyTerminalSettled",),
    )
    apply_terminal_projection = item("height_driver", "apply_terminal_settled")
    require_tokens(
        "height_driver",
        apply_terminal_projection,
        "durable post-Apply rollover projection",
        ("matches!(self, Self::ApplyTerminalSettled)",),
    )
    decided_lane_recovery_projection = item(
        "height_driver", "permits_decided_lane_recovery_ingress"
    )
    require_tokens(
        "height_driver",
        decided_lane_recovery_projection,
        "decided Apply barrier recovery ingress authority",
        ("Self::AwaitingApplyCompletion | Self::ApplyTerminalSettled",),
    )
    expected_blocked_lane_local_height_items = {
        "LifecycleBlockedOrdinaryLaneLocalIngressPermitV1",
        "blocked_ordinary_lane_local_ingress_permit",
    }
    observed_blocked_lane_local_height_items = set(
        _PRODUCTION_BLOCKED_ORDINARY_LANE_LOCAL_HEIGHT_ITEM_SHA256
    )
    if observed_blocked_lane_local_height_items != expected_blocked_lane_local_height_items:
        errors.append(
            "blocked ordinary lane-local height source-seal inventory must be exact; "
            f"missing={sorted(expected_blocked_lane_local_height_items - observed_blocked_lane_local_height_items)}, "
            f"extra={sorted(observed_blocked_lane_local_height_items - expected_blocked_lane_local_height_items)}"
        )
    permit_structs = rust_struct_items(
        sources["height_driver"], "LifecycleBlockedOrdinaryLaneLocalIngressPermitV1"
    )
    if len(permit_structs) != 1:
        errors.append(
            f"{paths['height_driver']}: blocked ordinary lane-local permit must have "
            f"exactly one struct declaration; found {len(permit_structs)}"
        )
        blocked_lane_local_permit = None
    else:
        blocked_lane_local_permit = permit_structs[0]
    _require_rust_item_token_sha256(
        paths["height_driver"],
        blocked_lane_local_permit,
        _PRODUCTION_BLOCKED_ORDINARY_LANE_LOCAL_HEIGHT_ITEM_SHA256[
            "LifecycleBlockedOrdinaryLaneLocalIngressPermitV1"
        ],
        "sealed blocked ordinary lane-local ingress permit",
        errors,
    )
    blocked_lane_local_permit_mint = item(
        "height_driver", "blocked_ordinary_lane_local_ingress_permit"
    )
    _require_rust_item_context(
        paths["height_driver"],
        blocked_lane_local_permit_mint,
        (("impl", "LifecycleProducerClaimDispositionV1"),),
        "blocked ordinary lane-local ingress permit mint",
        errors,
    )
    _require_rust_item_token_sha256(
        paths["height_driver"],
        blocked_lane_local_permit_mint,
        _PRODUCTION_BLOCKED_ORDINARY_LANE_LOCAL_HEIGHT_ITEM_SHA256[
            "blocked_ordinary_lane_local_ingress_permit"
        ],
        "blocked ordinary lane-local ingress permit mint",
        errors,
    )
    _successor_recovery_ready_proposal_sign_source_fidelity_errors(
        paths, sources, errors, item, require_order, blocked_lane_local_permit_mint
    )
    expected_apply_terminal_ready_broadcast_items = {
        "height::LifecycleApplyTerminalReadyBroadcastPermitV1",
        "height::apply_terminal_ready_broadcast_permit",
        "height::completion_selection_retries_before_runtime",
        "driver::classify_apply_terminal_ready_work",
        "driver::LaunchedProductionLifecycleV1::drive_apply_terminal_ready_broadcast_turn",
        "driver::ActivatedProductionLifecycleV1::drive_apply_terminal_ready_broadcast_turn",
        "scheduler::ProductionLifecycleOwnerV1::prepare_apply_terminal_direct_broadcast",
        "scheduler::ProductionLifecycleOwnerV1::wake_apply_terminal_direct_broadcast_if_fenced",
        "registry::PreparedApplyTerminalDirectBroadcastV1",
        "registry::ConcreteLifecycleWorkRegistry::prepare_apply_terminal_direct_broadcast",
        "registry::ConcreteLifecycleWorkRegistry::apply_terminal_direct_broadcast_pending_is_exact",
        "admission::ProductionLifecycleOwnerV1::settle_apply_terminal_direct_broadcast",
        "effects::V2EffectExecutor::settle_apply_terminal_direct_broadcast",
    }
    observed_apply_terminal_ready_broadcast_items = set(
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256
    )
    if (
        observed_apply_terminal_ready_broadcast_items
        != expected_apply_terminal_ready_broadcast_items
    ):
        errors.append(
            "post-Apply Ready Broadcast source-seal inventory must be exact; "
            f"missing={sorted(expected_apply_terminal_ready_broadcast_items - observed_apply_terminal_ready_broadcast_items)}, "
            f"extra={sorted(observed_apply_terminal_ready_broadcast_items - expected_apply_terminal_ready_broadcast_items)}"
        )
    apply_terminal_ready_permit_structs = rust_struct_items(
        sources["height_driver"], "LifecycleApplyTerminalReadyBroadcastPermitV1"
    )
    if len(apply_terminal_ready_permit_structs) != 1:
        errors.append(
            f"{paths['height_driver']}: post-Apply Ready Broadcast permit must have "
            f"exactly one struct declaration; found {len(apply_terminal_ready_permit_structs)}"
        )
        apply_terminal_ready_permit = None
    else:
        apply_terminal_ready_permit = apply_terminal_ready_permit_structs[0]
    _require_rust_item_token_sha256(
        paths["height_driver"],
        apply_terminal_ready_permit,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "height::LifecycleApplyTerminalReadyBroadcastPermitV1"
        ],
        "sealed post-Apply Ready Broadcast permit",
        errors,
    )
    apply_terminal_ready_permit_mint = item(
        "height_driver", "apply_terminal_ready_broadcast_permit"
    )
    _require_rust_item_context(
        paths["height_driver"],
        apply_terminal_ready_permit_mint,
        (("impl", "LifecycleProducerClaimDispositionV1"),),
        "post-Apply Ready Broadcast permit mint",
        errors,
    )
    _require_rust_item_token_sha256(
        paths["height_driver"],
        apply_terminal_ready_permit_mint,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "height::apply_terminal_ready_broadcast_permit"
        ],
        "post-Apply Ready Broadcast permit mint",
        errors,
    )
    require_order(
        "height_driver",
        apply_terminal_ready_permit_mint,
        "only the exact terminal Apply disposition may mint Ready Broadcast authority",
        (
            "self.apply_terminal_settled()",
            "Some(LifecycleApplyTerminalReadyBroadcastPermitV1",
            "None",
        ),
    )
    apply_terminal_completion_retry = item(
        "height_driver", "completion_selection_retries_before_runtime"
    )
    _require_rust_item_token_sha256(
        paths["height_driver"],
        apply_terminal_completion_retry,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "height::completion_selection_retries_before_runtime"
        ],
        "post-Apply direct Broadcast Completion re-entry",
        errors,
    )
    require_order(
        "height_driver",
        apply_terminal_completion_retry,
        "applied lifecycle Decision and both direct Broadcast outcomes must re-enter Completion before Runtime",
        (
            "LifecycleDecisionApplyApplied",
            "LifecycleValidatePublished",
            "LifecycleValidateSidecarWoken",
            "ApplyTerminalDirectBroadcastCompleted",
            "ApplyTerminalDirectBroadcastDeferred",
        ),
    )
    apply_terminal_ready_classifier = item(
        "driver", "classify_apply_terminal_ready_work"
    )
    _require_rust_item_context(
        paths["driver"],
        apply_terminal_ready_classifier,
        (),
        "closed post-Apply Ready-work classifier",
        errors,
    )
    _require_rust_item_token_sha256(
        paths["driver"],
        apply_terminal_ready_classifier,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "driver::classify_apply_terminal_ready_work"
        ],
        "closed post-Apply Ready-work classifier",
        errors,
    )
    require_order(
        "driver",
        apply_terminal_ready_classifier,
        "post-Apply Ready work admits only attested direct or recovered Broadcast mutation",
        (
            "ProductionCompletionReadyWorkV1::RecoveredLifecycleBroadcast",
            "ProductionApplyTerminalReadyWorkV1::RecoveredLifecycleBroadcast",
            "ProductionCompletionReadyWorkV1::RetainedDirectOutput",
            "ProductionApplyTerminalReadyWorkV1::RetainedDirectOutput",
            "ProductionCompletionReadyWorkV1::Invalid",
            "ProductionApplyTerminalReadyWorkV1::RestartRequired",
            "ProductionCompletionReadyWorkV1::None",
            "ProductionCompletionReadyWorkV1::PassThrough",
            "ProductionCompletionReadyWorkV1::CompletionIo",
            "ProductionApplyTerminalReadyWorkV1::PassThrough",
        ),
    )
    apply_terminal_direct_wake = _require_qualified_rust_item(
        paths["scheduler"],
        sources["scheduler"],
        "ProductionLifecycleOwnerV1",
        "wake_apply_terminal_direct_broadcast_if_fenced",
        errors,
        "post-Apply direct Broadcast reducer-fence wake",
    )
    _require_rust_item_token_sha256(
        paths["scheduler"],
        apply_terminal_direct_wake,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "scheduler::ProductionLifecycleOwnerV1::wake_apply_terminal_direct_broadcast_if_fenced"
        ],
        "post-Apply direct Broadcast reducer-fence wake",
        errors,
    )
    require_order(
        "scheduler",
        apply_terminal_direct_wake,
        "post-Apply fence wake must authenticate every collateral shared-source wake before publication",
        (
            "if let Some(fault) = self.coordinator.fault",
            "if let Some(lease) = self.coordinator.active_lease.as_ref()",
            "let exact_ready = self.coordinator.records.iter()",
            "if exact_ready != self.coordinator.ready_index",
            "let reducer_fence_wakes = self.coordinator.records.iter()",
            "let mut schedulable = exact_ready",
            "schedulable.extend(reducer_fence_wakes.iter().copied())",
            "let Some(ordinal) = schedulable.first().copied()",
            "if !reducer_fence_wakes.contains(&ordinal)",
            "self.classify_schedulable_completion_work(&schedulable, Some(fence))",
            "ProductionCompletionReadyWorkV1::RetainedDirectOutput",
            "for wake_ordinal in reducer_fence_wakes",
            "attest_schedulable_lifecycle_broadcast_carrier(",
            "attestation.matches_schedulable_record(record)",
            "advance_observed_generation(fence.source(), fence.generation())",
            "let exact_ready_after_wake = self.coordinator.records.iter()",
            "exact_ready_after_wake != self.coordinator.ready_index",
            "self.coordinator.ready_index.first().copied() != Some(ordinal)",
            "Ok(true)",
        ),
    )
    reject_tokens(
        "scheduler",
        apply_terminal_direct_wake,
        "post-Apply direct Broadcast wake may not claim general scheduler or output authority",
        (
            "plan_turn(",
            "settle_pending_lifecycle_output_admissions",
        ),
    )
    apply_terminal_direct_prepare = _require_qualified_rust_item(
        paths["scheduler"],
        sources["scheduler"],
        "ProductionLifecycleOwnerV1",
        "prepare_apply_terminal_direct_broadcast",
        errors,
        "post-Apply direct Broadcast global-minimum authority mint",
    )
    _require_rust_item_token_sha256(
        paths["scheduler"],
        apply_terminal_direct_prepare,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "scheduler::ProductionLifecycleOwnerV1::prepare_apply_terminal_direct_broadcast"
        ],
        "post-Apply direct Broadcast global-minimum authority mint",
        errors,
    )
    require_order(
        "scheduler",
        apply_terminal_direct_prepare,
        "post-Apply direct Broadcast authority must seal only an already-Ready exact minimum",
        (
            "let exact_ready = self.coordinator.records.iter()",
            "if exact_ready != self.coordinator.ready_index",
            "let ordinal = self.coordinator.ready_index.first().copied()?",
            "self.registry.registry().prepare_apply_terminal_direct_broadcast(&self.coordinator, ordinal)",
        ),
    )
    reject_tokens(
        "scheduler",
        apply_terminal_direct_prepare,
        "post-Apply direct Broadcast authority mint may not combine fence wake and service selection",
        ("wake_apply_terminal_direct_broadcast_if_fenced", "advance_observed_generation"),
    )
    prepared_direct_structs = rust_struct_items(
        sources["registry_output"], "PreparedApplyTerminalDirectBroadcastV1"
    )
    if len(prepared_direct_structs) != 1:
        errors.append(
            f"{paths['registry_output']}: post-Apply direct Broadcast authority must "
            f"have exactly one struct declaration; found {len(prepared_direct_structs)}"
        )
        prepared_direct = None
    else:
        prepared_direct = prepared_direct_structs[0]
    _require_rust_item_context(
        paths["registry_output"],
        prepared_direct,
        (),
        "move-only post-Apply direct Broadcast authority",
        errors,
        expected_attributes=(
            '#[must_use = "the attested direct Broadcast must be settled or failed closed"]',
        ),
    )
    _require_rust_item_token_sha256(
        paths["registry_output"],
        prepared_direct,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "registry::PreparedApplyTerminalDirectBroadcastV1"
        ],
        "move-only post-Apply direct Broadcast authority",
        errors,
    )
    require_tokens(
        "registry_output",
        prepared_direct,
        "post-Apply direct Broadcast authority exact identity",
        (
            "address: ConcreteWorkAddress",
            "digest: LifecycleDigest",
            "pending_key: LifecycleOutputAdmissionKeyV1",
            "_linearity: ApplyTerminalDirectBroadcastLinearityV1",
        ),
    )
    _require_rust_source_token_sequence(
        paths["registry_output"],
        sources["registry_output"],
        """
struct ApplyTerminalDirectBroadcastLinearityV1;
impl Drop for ApplyTerminalDirectBroadcastLinearityV1 {
    fn drop(&mut self) {}
}
""",
        "post-Apply direct Broadcast authority must remain move-only",
        errors,
    )
    registry_direct_prepare = _require_qualified_rust_item(
        paths["registry_output"],
        sources["registry_output"],
        "ConcreteLifecycleWorkRegistry",
        "prepare_apply_terminal_direct_broadcast",
        errors,
        "registry post-Apply direct Broadcast authority mint",
    )
    _require_rust_item_token_sha256(
        paths["registry_output"],
        registry_direct_prepare,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "registry::ConcreteLifecycleWorkRegistry::prepare_apply_terminal_direct_broadcast"
        ],
        "registry post-Apply direct Broadcast authority mint",
        errors,
    )
    require_order(
        "registry_output",
        registry_direct_prepare,
        "registry authority must bind one Ready direct Broadcast and its installed pending key",
        (
            "SchedulableLifecycleBroadcastCarrierV1::RetainedDirectOutput(attestation)",
            "attest_schedulable_lifecycle_broadcast_carrier(coordinator, ordinal, None)?",
            "record.state != super::LifecycleState::Ready",
            "coordinator.ready_index.first().copied() != Some(ordinal)",
            "!attestation.matches_schedulable_record(record)",
            "ConcreteLifecycleWorkKind::PendingAdapter",
            "!matches!(effect, AdapterEffect::Broadcast(_))",
            "work.digest != attestation.digest",
            "!lifecycle_output_row_matches(",
            "Ok(PreparedApplyTerminalDirectBroadcastV1",
            "address: attestation.address",
            "digest: attestation.digest",
            "causal_lifecycle_key: *pending.causal_lifecycle_key().as_ref()",
            "effect_identity: *pending.exact_effect_identity().as_ref()",
            "_linearity: ApplyTerminalDirectBroadcastLinearityV1",
        ),
    )
    registry_direct_rejoin = _require_qualified_rust_item(
        paths["registry_output"],
        sources["registry_output"],
        "ConcreteLifecycleWorkRegistry",
        "apply_terminal_direct_broadcast_pending_is_exact",
        errors,
        "registry post-Apply direct Broadcast exact-pending rejoin",
    )
    _require_rust_item_token_sha256(
        paths["registry_output"],
        registry_direct_rejoin,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "registry::ConcreteLifecycleWorkRegistry::apply_terminal_direct_broadcast_pending_is_exact"
        ],
        "registry post-Apply direct Broadcast exact-pending rejoin",
        errors,
    )
    require_order(
        "registry_output",
        registry_direct_rejoin,
        "registry rejoin must preserve the complete Ready address/digest/key/Broadcast identity",
        (
            "record.state == super::LifecycleState::Ready",
            "coordinator.active_lease.is_none()",
            "coordinator.ready_index.first().copied() == Some(prepared.address.ordinal)",
            "work.digest == prepared.digest",
            "pending.key() == prepared.pending_key",
            "matches!(&pending.effect, AdapterEffect::Broadcast(_))",
            "lifecycle_output_row_matches(",
        ),
    )
    owner_direct_settler = _require_qualified_rust_item(
        paths["concrete_admission"],
        sources["concrete_admission"],
        "ProductionLifecycleOwnerV1",
        "settle_apply_terminal_direct_broadcast",
        errors,
        "lifecycle-owner post-Apply direct Broadcast exact settler",
        expected_attributes=("#[allow(clippy::result_large_err)]",),
    )
    _require_rust_item_token_sha256(
        paths["concrete_admission"],
        owner_direct_settler,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "admission::ProductionLifecycleOwnerV1::settle_apply_terminal_direct_broadcast"
        ],
        "lifecycle-owner post-Apply direct Broadcast exact settler",
        errors,
    )
    require_order(
        "concrete_admission",
        owner_direct_settler,
        "post-Apply owner settlement must attest before service and publish only after fsync",
        (
            "apply_terminal_direct_broadcast_pending_is_exact(",
            "LifecycleOutputRegistryFailureV1::ApplyTerminalAttestation",
            "let execution = pending.into_existing_execution()",
            "join_lifecycle_output(&self.coordinator, &execution)",
            "LifecycleOutputRegistryJoinV1::Ready(retirement)",
            "LifecycleOutputRegistryFailureV1::ApplyTerminalAttestation",
            "execution.execute_with(execute)",
            "LifecycleOutputServiceDispositionV1::Accepted",
            "LifecycleOutputServiceDispositionV1::SourceRetained",
            "ProductionLifecycleOutputAdmissionSettlementV1::Deferred",
            "let mut staged = self.coordinator.stage_durable_transaction()",
            "finish_terminal(retirement.ordinal(), super::TerminalOutcome::Advanced)",
            "lifecycle_output_terminal_is_exact(",
            "persist_exact_staged_successor(&staged)",
            "publish_lifecycle_output_terminal_after_fsync(retirement)",
            "self.coordinator = staged",
            "ProductionLifecycleOutputAdmissionSettlementV1::Completed",
        ),
    )
    reject_tokens(
        "concrete_admission",
        owner_direct_settler,
        "post-Apply owner settlement may not borrow generic output admission",
        ("settle_lifecycle_output_admission",),
    )
    executor_direct_context = (
        ("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),
    )
    executor_direct_candidates = [
        candidate
        for candidate in rust_items(
            sources["effects_settlement"], "settle_apply_terminal_direct_broadcast"
        )
        if candidate.brace_context == executor_direct_context
    ]
    if len(executor_direct_candidates) != 1:
        errors.append(
            f"{paths['effects_settlement']}: require exactly one exact executor "
            "post-Apply direct Broadcast settler; "
            f"found {len(executor_direct_candidates)}"
        )
        executor_direct_settler = None
    else:
        executor_direct_settler = executor_direct_candidates[0]
    _require_rust_item_context(
        paths["effects_settlement"],
        executor_direct_settler,
        executor_direct_context,
        "executor post-Apply direct Broadcast exact-key settler",
        errors,
    )
    _require_rust_item_token_sha256(
        paths["effects_settlement"],
        executor_direct_settler,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "effects::V2EffectExecutor::settle_apply_terminal_direct_broadcast"
        ],
        "executor post-Apply direct Broadcast exact-key settler",
        errors,
    )
    require_order(
        "effects_settlement",
        executor_direct_settler,
        "executor settlement must remove and restore only the prepared pending-map key",
        (
            "self.ensure_open()?",
            "let key = prepared.pending_key()",
            "self.lifecycle_decision_apply_successor_outputs",
            "attestation.exactly_matches_terminal_preparation(&prepared)",
            "self.pending_lifecycle_output_admissions.remove(&key)",
            "return Err(self.close(error, services))",
            "owner.settle_apply_terminal_direct_broadcast(",
            "ProductionLifecycleOutputAdmissionSettlementV1::Completed",
            "self.lifecycle_decision_apply_successor_outputs = None",
            "ProductionLifecycleOutputAdmissionSettlementV1::Deferred(pending)",
            "pending.key() != key",
            "pending_lifecycle_output_admissions.insert(key, pending)",
            "ProductionApplyTerminalDirectBroadcastSettlementV1::SourceRetained",
            "ProductionLifecycleOutputAdmissionSettlementV1::AlreadyCompleted",
            "Err(self.close(error, services))",
            "ProductionLifecycleOutputAdmissionSettlementV1::Failed",
            "let pending_key = pending.key()",
            "pending_key != key",
            "pending_lifecycle_output_admissions.insert(key, pending)",
            "Err(self.close(error, services))",
        ),
    )
    reject_tokens(
        "effects_settlement",
        executor_direct_settler,
        "executor post-Apply settlement may not enumerate or broadly drain pending outputs",
        (
            "self.pending_lifecycle_output_admissions.keys()",
            "for key in keys",
            "settle_pending_lifecycle_output_admissions",
        ),
    )
    launched_apply_terminal_ready_driver = _require_qualified_rust_item(
        paths["driver"],
        sources["driver"],
        "LaunchedProductionLifecycleV1",
        "drive_apply_terminal_ready_broadcast_turn",
        errors,
        "launched post-Apply Ready Broadcast driver",
    )
    _require_rust_item_token_sha256(
        paths["driver"],
        launched_apply_terminal_ready_driver,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "driver::LaunchedProductionLifecycleV1::drive_apply_terminal_ready_broadcast_turn"
        ],
        "launched post-Apply Ready Broadcast driver",
        errors,
    )
    require_order(
        "driver",
        launched_apply_terminal_ready_driver,
        "post-Apply Ready driver admits only exact direct or recovered Broadcast settlement",
        (
            "classify_apply_terminal_ready_work(",
            "self.owner.classify_completion_ready_work(fence)",
            "ProductionApplyTerminalReadyWorkV1::PassThrough",
            "return ProductionLifecycleCompletionTurnV1::PassThrough(runner)",
            "ProductionApplyTerminalReadyWorkV1::RestartRequired",
            "self.close_output_for_restart()",
            "ProductionApplyTerminalReadyWorkV1::RetainedDirectOutput",
            "self.owner.prepare_apply_terminal_direct_broadcast()",
            "executor.settle_apply_terminal_direct_broadcast(owner, services, prepared)",
            "ProductionApplyTerminalDirectBroadcastSettlementV1::Completed",
            "ProductionLifecycleCompletionSelectionV1::ApplyTerminalDirectBroadcastCompleted",
            "ProductionApplyTerminalDirectBroadcastSettlementV1::SourceRetained",
            "ProductionLifecycleCompletionSelectionV1::ApplyTerminalDirectBroadcastDeferred",
            "ProductionApplyTerminalReadyWorkV1::RecoveredLifecycleBroadcast",
            "refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
        ),
    )
    if launched_apply_terminal_ready_driver is not None:
        for forbidden in (
            "dispatch_completion_with_runner_debt",
            "dispatch_completion_requiring_ready_ordinal",
            "settle_pending_lifecycle_output_admissions",
        ):
            if forbidden in launched_apply_terminal_ready_driver.source:
                errors.append(
                    f"{paths['driver']}:{launched_apply_terminal_ready_driver.line}: "
                    "post-Apply Ready Broadcast driver exposes forbidden general "
                    f"Completion dispatcher {forbidden!r}"
                )
    activated_apply_terminal_ready_driver = _require_qualified_rust_item(
        paths["driver"],
        sources["driver"],
        "ActivatedProductionLifecycleV1",
        "drive_apply_terminal_ready_broadcast_turn",
        errors,
        "activated post-Apply Ready Broadcast forwarding driver",
    )
    _require_rust_item_token_sha256(
        paths["driver"],
        activated_apply_terminal_ready_driver,
        _PRODUCTION_APPLY_TERMINAL_READY_BROADCAST_ITEM_SHA256[
            "driver::ActivatedProductionLifecycleV1::drive_apply_terminal_ready_broadcast_turn"
        ],
        "activated post-Apply Ready Broadcast forwarding driver",
        errors,
    )
    require_order(
        "driver",
        activated_apply_terminal_ready_driver,
        "activated post-Apply Ready Broadcast forwarding driver",
        (
            "self.launched",
            "drive_apply_terminal_ready_broadcast_turn(ready, permit)",
        ),
    )
    apply_barrier_transition = item("height_driver", "observe_completion")
    require_tokens(
        "height_driver",
        apply_barrier_transition,
        "typed Apply producer-claim transition",
        (
            "Completion::CompletionIoDispatch(Ok(Dispatch::ApplyQueued { .. })), ) => Ok(Self::AwaitingApplyCompletion)",
            "Completion::LifecycleDecisionApplyDeferred",
            "Completion::LifecycleDecisionApplyRequeued",
            "Completion::LifecycleDecisionApplyCompletionDeferred, ) => Ok(Self::AwaitingApplyCompletion)",
            "Completion::LifecycleDecisionApplyApplied",
            "Ok(Self::ApplyTerminalSettled)",
        ),
    )
    apply_terminal_disposition = item(
        "height_driver", "after_terminal_settlement"
    )
    require_order(
        "height_driver",
        apply_terminal_disposition,
        "terminal Apply outer-runtime stop disposition",
        (
            "producer_claim: LifecycleProducerClaimDispositionV1",
            "retry_before_producer: false",
            "terminal_settlement_stops_runtime: true",
        ),
    )
    apply_terminal_disposition_projection = item(
        "height_driver", "terminal_settlement_stops_runtime"
    )
    require_tokens(
        "height_driver",
        apply_terminal_disposition_projection,
        "terminal Apply outer-runtime stop projection",
        ("self.terminal_settlement_stops_runtime",),
    )

    pre_timeout_ingress_results = rust_enum_items(
        sources["driver"], "ProductionPreTimeoutLockedPrepareQcIngressTurnV1"
    )
    if len(pre_timeout_ingress_results) != 1:
        errors.append(
            f"{paths['driver']}: fixed-cut pre-timeout ingress result must have "
            "exactly one enum declaration; found "
            f"{len(pre_timeout_ingress_results)}"
        )
        pre_timeout_ingress_result = None
    else:
        pre_timeout_ingress_result = pre_timeout_ingress_results[0]
    require_tokens(
        "driver",
        pre_timeout_ingress_result,
        "closed fixed-cut pre-timeout ingress result",
        (
            "Empty",
            "ObsoletePredecessor(ProductionPreparedOrdinaryIngressTurnV1)",
            "ExactPrepareProgress(ProductionPreparedOrdinaryIngressTurnV1)",
            "RestartRequired",
        ),
    )
    launched_pre_timeout_ingress = _require_qualified_rust_item(
        paths["driver"],
        sources["driver"],
        "LaunchedProductionLifecycleV1",
        "prepare_pre_timeout_locked_prepare_qc_ingress_turn",
        errors,
        "launched fixed-cut pre-timeout exact Prepare progress fair-ingress preparation",
    )
    require_order(
        "driver",
        launched_pre_timeout_ingress,
        "fixed-cut pre-timeout ingress preserves exact Prepare preview, ordinary gate, and dequeue tail",
        (
            "self.executor.lifecycle_terminal_subject()",
            "Arc::clone(&self.leader_wire_ingress_binding.ingress)",
            "capture_next_ingress_turn_cut_before_with_obsolete_retirement(",
            "cut.physical_cut()",
            "BlockMessage::V2(message)",
            "executor.wire_previews_pre_timeout_locked_prepare_qc(cut, &message.payload)",
            "captured.selected_disposition() == FairV2IngressDequeueDisposition::RetireObsolete",
            "prepare_ordinary_ingress_dequeue(",
            "PreparedOrdinaryIngressDequeueV1::Prepared(turn) if obsolete",
            "ProductionPreTimeoutLockedPrepareQcIngressTurnV1::ObsoletePredecessor(turn)",
            "PreparedOrdinaryIngressDequeueV1::Prepared(turn)",
            "ProductionPreTimeoutLockedPrepareQcIngressTurnV1::ExactPrepareProgress(turn)",
            "PreparedOrdinaryIngressDequeueV1::RestartRequired",
            "ProductionPreTimeoutLockedPrepareQcIngressTurnV1::RestartRequired",
        ),
    )
    activated_pre_timeout_ingress = _require_qualified_rust_item(
        paths["driver"],
        sources["driver"],
        "ActivatedProductionLifecycleV1",
        "prepare_pre_timeout_locked_prepare_qc_ingress_turn",
        errors,
        "activated fixed-cut pre-timeout PrepareQC forwarding preparation",
    )
    require_tokens(
        "driver",
        activated_pre_timeout_ingress,
        "activated fixed-cut pre-timeout forwarding remains sealed",
        (
            "self.launched.prepare_pre_timeout_locked_prepare_qc_ingress_turn(cut)",
        ),
    )

    lifecycle_height_driver = item("height_driver", "drain_lifecycle_v2_ingress")
    require_order(
        "height_driver",
        lifecycle_height_driver,
        "terminal cut or Apply must publish a lower direct-output fence wake before cold-output service",
        (
            "if terminal_finalization_cut.is_some() || producer_claim.apply_terminal_settled()",
            "let direct_output_woken = activated.with_runner_runtime(",
            "let fence = executor.lifecycle_reducer_fence_observation()",
            "owner.wake_apply_terminal_direct_broadcast_if_fenced(fence)",
            "if direct_output_woken",
            "LifecycleV2IngressDrainDispositionV1::retry_before_producer",
            "if producer_claim.apply_terminal_settled()",
            "settle_one_recovered_lifecycle_output(",
            "settled_apply_output_drain_disposition(settlement, producer_claim)",
        ),
    )
    settled_apply_output_drain = item(
        "height_driver", "settled_apply_output_drain_disposition"
    )
    require_order(
        "height_driver",
        settled_apply_output_drain,
        "terminal Apply retained-output settlement",
        (
            "debug_assert!(producer_claim.apply_terminal_settled())",
            "RecoveredLifecycleOutputSettlementV1::SourceRetained",
            "LifecycleV2IngressDrainDispositionV1::retry_before_producer",
            "RecoveredLifecycleOutputSettlementV1::Completed",
            "LifecycleV2IngressDrainDispositionV1::after_terminal_settlement",
            "RecoveredLifecycleOutputSettlementV1::Empty",
            "RecoveredLifecycleOutputSettlementV1::Deferred",
            "None",
        ),
    )
    blocked_runtime_disposition = item(
        "height_driver", "blocked_runtime_drain_disposition"
    )
    require_order(
        "height_driver",
        blocked_runtime_disposition,
        "terminal Apply keeps the runtime fenced after Ready Broadcast handoff",
        (
            "producer_claim.apply_terminal_settled()",
            "LifecycleV2IngressDrainDispositionV1::after_terminal_settlement(producer_claim)",
            "LifecycleV2IngressDrainDispositionV1::ready(producer_claim)",
        ),
    )
    require_order(
        "height_driver",
        lifecycle_height_driver,
        "durable post-Apply drain cut",
        (
            "if producer_claim.apply_terminal_settled()",
            "settle_one_recovered_lifecycle_output(",
            "settled_apply_output_drain_disposition(",
            "let (context_id, height, output_guard)",
        ),
    )

    pre_timeout_target = item("adapter", "pre_timeout_locked_prepare_qc_target")

    recovery_step = item("runtime", "step_recovery")
    require_order(
        "runtime",
        recovery_step,
        "no-clock interrupted-tip recovery retains exact runtime ownership accounting",
        (
            "if self.fail_closed",
            "self.last_scheduler_ownership.is_some()",
            "self.pending_effect_ownership.is_some()",
            "!self.pending_leader_wire_terminals.is_empty()",
            "if self.clocks_armed",
            "RuntimeError::RecoveryAfterClocksArmed",
            "self.driver.all_deferred_admission_ordinals().is_empty()",
            "!self.driver.deferred_work_is_serviceable()",
            "self.dispatch_one_adapter_deferred(now, None)",
            "self.scheduler_arbitration_inputs(now)",
            "self.ingress.pop_next_with_ownership()",
            "scheduled != ScheduledWork::Fifo",
            "command.lifecycle_owner()",
            "self.driver.dispatch(command)",
            "if retry_unadmitted",
            "RuntimeSelectedOwnerKind::FifoRetryRetained",
            "self.retain_effect_ownership(",
            "self.complete_driver_dispatch_leader_wire_owners(",
            "self.observe_effects(now, &effects)",
            "Ok(RuntimeStep::Advanced(effects))",
        ),
    )
    require_order(
        "adapter",
        pre_timeout_target,
        "unchanged-lock pre-timeout target is current, durable, validated, and unfenced",
        (
            "let current_tag = self.reducer.current_tag()",
            "let current_round = reducer::Round::new(current_tag.height(), current_tag.view())",
            "let durable = self.reducer.durable_state()",
            "let locked = durable.locked()?",
            "self.fail_closed",
            "!self.replay_complete",
            "durable.decision().is_some()",
            "locked.round().view() >= current_round.view()",
            "durable.timeout_intent(current_round).is_some()",
            "durable.commit_intent(current_round).is_some()",
            "self.reducer.local_validator().is_none()",
            "self.reducer.pending_persistence_record().is_some()",
            "self.reducer.awaiting_signature().is_some()",
            "self.reducer.body_state(current_round, locked.subject())",
            "reducer::BodyState::Validated",
            "let subject = self.registry.subject(locked.subject()).ok()?",
            "execution_commitment(locked.round(), locked.subject())",
            "execution_commitment(current_round, locked.subject())",
            "(locked_commitment == current_commitment).then_some(",
            "PreTimeoutLockedPrepareQcTargetV1 {",
            "round: self.registry.round_to_wire(current_round)",
            "subject",
            "execution_commitment: current_commitment",
        ),
    )
    pre_timeout_preview = item(
        "adapter", "pre_timeout_locked_prepare_qc_stages_lock_and_commit"
    )
    require_order(
        "adapter",
        pre_timeout_preview,
        "pre-timeout PrepareQC preview uses the ordinary conversion and one cloned LockAndCommit",
        (
            "certificate.phase != wire::GlobalPhase::Prepare",
            "certificate.round != target.round",
            "certificate.proposal_round != target.round",
            "certificate.subject != target.subject",
            "certificate.execution_commitment != target.execution_commitment",
            "self.pre_timeout_locked_prepare_qc_target() != Some(target)",
            "let mut registry = self.registry.clone()",
            "registry.qc_to_core(certificate, &self.wire_context)",
            "let mut reducer = self.reducer.clone()",
            "reducer.step(reducer::Event::QuorumCertificateReceived {",
            "tag: reducer.current_tag()",
            "certificate: core_certificate.clone()",
            "let [reducer::Effect::Persist { entry, .. }] = outcome.effects()",
            "reducer::WalRecord::LockAndCommit { prepare, vote }",
            "prepare == &core_certificate",
            "vote.phase() == reducer::Phase::Commit",
            "vote.subject() == core_certificate.subject()",
        ),
    )

    runtime_driver_context = (
        "impl",
        "RuntimeDriver",
        "for",
        "SumeragiV2Adapter",
    )
    runtime_future_prepare_blocker = qualified_item(
        "runtime",
        "pacemaker_progress_blocked_target_view",
        runtime_driver_context,
        "production future-PrepareQC blocker classifier",
    )
    require_order(
        "runtime",
        runtime_future_prepare_blocker,
        "only an authenticated future-view PrepareQC may block ordinary Progress service",
        (
            "AdapterCommand::Authenticated(authenticated)",
            "authenticated.payload()",
            "wire::ConsensusMessageV2Payload::QuorumCertificate(certificate)",
            "certificate.phase == wire::GlobalPhase::Prepare",
            "certificate.round.view > self.current_tag().view()",
            "Some(certificate.round.view)",
        ),
    )
    runtime_view_release = qualified_item(
        "runtime",
        "pacemaker_progress_releases_view_block",
        runtime_driver_context,
        "production current-round view-release classifier",
    )
    require_order(
        "runtime",
        runtime_view_release,
        "view release must reject non-future targets and accept only strict authenticated current-round recovery",
        (
            "let current_view = self.current_tag().view()",
            "if target_view <= current_view",
            "return false",
            "AdapterCommand::Authenticated(authenticated)",
            "wire_payload_matches_current_strict_timeout_recovery_round(",
            "authenticated.payload()",
            "self.wire_context()",
            "self.current_tag()",
        ),
    )
    runtime_target_binding = qualified_item(
        "runtime",
        "pre_timeout_locked_prepare_qc_target",
        runtime_driver_context,
        "production runtime pre-timeout target binding",
    )
    require_tokens(
        "runtime",
        runtime_target_binding,
        "production runtime target delegates to the adapter",
        ("SumeragiV2Adapter::pre_timeout_locked_prepare_qc_target(self)",),
    )
    runtime_wire_preview_binding = qualified_item(
        "runtime",
        "wire_previews_pre_timeout_locked_prepare_qc",
        runtime_driver_context,
        "production runtime wire preview binding",
    )
    require_tokens(
        "runtime",
        runtime_wire_preview_binding,
        "production runtime wire preview delegates to exact Prepare progress",
        (
            "self.pre_timeout_locked_prepare_progress_is_exact(payload, target)",
        ),
    )
    runtime_command_preview_binding = qualified_item(
        "runtime",
        "command_previews_pre_timeout_locked_prepare_qc",
        runtime_driver_context,
        "production runtime authenticated-command preview binding",
    )
    require_order(
        "runtime",
        runtime_command_preview_binding,
        "runtime command preview admits only authenticated exact Prepare progress",
        (
            "AdapterCommand::Authenticated(authenticated)",
            "self.pre_timeout_locked_prepare_progress_is_exact(",
            "authenticated.payload()",
            "target",
        ),
    )

    runtime_generic_context = (
        "impl",
        "<",
        "D",
        ":",
        "RuntimeDriver",
        ">",
        "SerializedV2Runtime",
        "<",
        "D",
        ">",
    )
    bounded_ingress_context = (
        "impl",
        "<",
        "C",
        ":",
        "ExactRuntimeCommandIdentity",
        ">",
        "BoundedIngress",
        "<",
        "C",
        ">",
    )
    bounded_view_release_authorization = qualified_item(
        "runtime",
        "ordinary_view_blocked_progress_authorization",
        bounded_ingress_context,
        "bounded-ingress blocked-view authorization mint",
    )
    require_order(
        "runtime",
        bounded_view_release_authorization,
        "blocked-view authorization must bind the exact ordinary-selected authenticated Progress minimum",
        (
            "select_bounded_service_class(",
            "if selection.selected != SERVICE_CLASS_PROGRESS",
            "minimum_lifecycle_for_class(CommandClass::Progress)",
            "queued.lifecycle_ordinal == Some(oldest_progress_lifecycle_ordinal)",
            "selected.validate_admission_identity()",
            "selected.identity.kind != RuntimeCommandKind::Authenticated",
            "selected.ingress_ownership.is_none()",
            "blocked_target_view(selected)",
            "cached_queue_occurrence_owner(&self.selection_source_identity)",
            "RuntimeViewBlockedProgressAuthorization::new(",
        ),
    )
    runtime_view_release_authorization = qualified_item(
        "runtime",
        "ordinary_view_blocked_progress_authorization",
        runtime_generic_context,
        "serialized runtime blocked-view authorization wrapper",
    )
    require_tokens(
        "runtime",
        runtime_view_release_authorization,
        "runtime authorization target must come from the production queued-command classifier",
        (
            "self.ingress.ordinary_view_blocked_progress_authorization(",
            "driver.pacemaker_progress_blocked_target_view(&queued.command)",
        ),
    )
    runtime_ordinary_step = qualified_item(
        "runtime",
        "step",
        runtime_generic_context,
        "ordinary blocked-view release step",
    )
    require_order(
        "runtime",
        runtime_ordinary_step,
        "ordinary blocked-view release may consume only the selected FIFO turn and otherwise falls through before schedule mutation",
        (
            "let (work, next_schedule) = self.schedule.select(",
            "if work == ScheduledWork::Fifo",
            "self.ordinary_view_blocked_progress_authorization()",
            "self.dispatch_one_pacemaker_progress(",
            "Some((arbitration.clone(), authorization))",
            "return Ok(step)",
            "self.schedule = next_schedule",
        ),
    )
    runtime_view_release_dispatch = qualified_item(
        "runtime",
        "dispatch_one_pacemaker_progress",
        runtime_generic_context,
        "ordinary blocked-view release dispatcher",
    )
    require_order(
        "runtime",
        runtime_view_release_dispatch,
        "ordinary release must filter exact current-round Progress, avoid no-candidate mutation, consume scheduler debt, fail closed on retry, and retain authorization",
        (
            "let view_release_target = ordinary_view_escape.as_ref()",
            "pacemaker_progress_blocked_target_view(&queued.command)",
            "return false",
            "view_release_target.is_some_and(|target_view|",
            "queued.class != CommandClass::Progress",
            "queued.identity.kind != RuntimeCommandKind::Authenticated",
            "queued.ingress_ownership.is_none()",
            "pacemaker_progress_releases_view_block(",
            "ordinary_view_escape_selected",
            "let Some((command, candidate)) = selected else",
            "return Ok(None)",
            "let schedule_after = if let Some((arbitration, _)) = &ordinary_view_escape",
            "if work != ScheduledWork::Fifo",
            "self.schedule = next_schedule",
            "RuntimeQueueSelectionKind::OrdinaryViewProgress",
            "if ordinary_view_escape_selected && (retry_unadmitted || retained_deferred_ingress)",
            "arbitration.view_blocked_progress_authorization = Some(authorization)",
            "schedule_after",
        ),
    )
    freeze_pre_timeout_cut = qualified_item(
        "runtime",
        "freeze_pre_timeout_locked_prepare_qc_cut",
        runtime_generic_context,
        "serialized runtime fixed pre-timeout cut mint",
    )
    require_order(
        "runtime",
        freeze_pre_timeout_cut,
        "fixed pre-timeout cut freezes the due timeout owner before target mint",
        (
            "self.last_scheduler_ownership.is_some()",
            "self.pending_effect_ownership.is_some()",
            "!self.pending_leader_wire_terminals.is_empty()",
            "self.freeze_due_clock_owners(now)",
            "self.scheduler_arbitration_inputs(now)",
            "if !arbitration.timeout_due",
            "self.driver.pre_timeout_locked_prepare_qc_target()",
            "self.timeout_owner.clone()",
            "self.timeout_owner_physical_cut",
            "target.validate_exact(self.round_tag)",
            "physical_cut > self.ingress_physical_cut",
            "PreTimeoutLockedPrepareQcCutV1 {",
            "tag: self.round_tag",
            "physical_cut",
            "timeout_owner",
            "target",
        ),
    )
    current_pre_timeout_cut = qualified_item(
        "runtime",
        "pre_timeout_locked_prepare_qc_cut_is_current",
        runtime_generic_context,
        "serialized runtime fixed-cut revalidation",
    )
    require_tokens(
        "runtime",
        current_pre_timeout_cut,
        "every pre-timeout use revalidates tag, timeout owner, cut, and target",
        (
            "!self.timeout_emitted",
            "cut.tag == self.round_tag",
            "cut.physical_cut <= self.ingress_physical_cut",
            "self.timeout_owner.as_ref() == Some(&cut.timeout_owner)",
            "self.timeout_owner_physical_cut == Some(cut.physical_cut)",
            "cut.target.validate_exact(cut.tag)",
            "self.driver.pre_timeout_locked_prepare_qc_target() == Some(cut.target)",
        ),
    )
    runtime_wire_preview = qualified_item(
        "runtime",
        "wire_previews_pre_timeout_locked_prepare_qc",
        runtime_generic_context,
        "serialized runtime fair-ingress wire preview",
    )
    require_order(
        "runtime",
        runtime_wire_preview,
        "fair-ingress preview is current-cut and exact Prepare progress only",
        (
            "self.pre_timeout_locked_prepare_qc_cut_is_current(cut)",
            "self.driver.wire_previews_pre_timeout_locked_prepare_qc(",
            "payload",
            "cut.target",
        ),
    )
    runtime_pre_timeout_step = qualified_item(
        "runtime",
        "try_step_pre_timeout_locked_prepare_qc",
        runtime_generic_context,
        "serialized runtime exact pre-timeout dispatch",
    )
    require_order(
        "runtime",
        runtime_pre_timeout_step,
        "pre-timeout dispatch is authenticated Progress, strictly pre-cut, and nonretrying",
        (
            "self.reconcile_fence_retry_blocked_fifo_owners()",
            "self.pre_timeout_locked_prepare_qc_cut_is_current(cut)",
            "self.scheduler_arbitration_inputs(now)",
            "if !timeout_due",
            "self.ingress.pop_pacemaker_progress_with_ownership(",
            "queued.class == CommandClass::Progress",
            "queued.identity.kind == RuntimeCommandKind::Authenticated",
            "queued.ingress_ownership.is_some()",
            "u128::from(physical.source_ordinal) < physical_cut",
            "driver.command_previews_pre_timeout_locked_prepare_qc(",
            """
|_| false,
false,
Some(RuntimeQueueSelectionKind::PreTimeoutLockedPrepareQc),
""",
            "candidate.selection_seal.kind",
            "RuntimeQueueSelectionKind::PreTimeoutLockedPrepareQc",
            "command.lifecycle_owner()",
            "owner.causal_origin().root_class == SERVICE_CLASS_PROGRESS",
            "self.driver.dispatch(command)",
            "RuntimeDispatchIngress::DirectAuthenticated",
            "if retry_unadmitted || retained_deferred_ingress",
            "if !arbitration.timeout_due",
            "arbitration.periodic_timer_due = false",
            "arbitration.fifo_ready = false",
            "arbitration.completion_ready = false",
            "arbitration.progress_ready = false",
            "arbitration.normal_ready = false",
            "arbitration.pre_timeout_locked_prepare_qc_physical_cut = Some(physical_cut)",
            "RuntimeSelectedOwnerKind::PreTimeoutLockedPrepareQc",
            "RuntimeSelectedCandidateOwnership::Exact(candidate)",
            "self.finish_dispatched_step(",
        ),
    )

    scheduler_evidence = qualified_item(
        "runtime",
        "validate_exact",
        ("impl", "RuntimeSchedulerOwnershipEvidence"),
        "pre-timeout scheduler evidence validator",
    )
    require_order(
        "runtime",
        scheduler_evidence,
        "scheduler evidence binds the due timeout to one exact authenticated pre-cut owner",
        (
            "self.pre_timeout_locked_prepare_qc_physical_cut",
            "self.selected == RuntimeSelectedOwnerKind::PreTimeoutLockedPrepareQc",
            "RuntimeSelectedOwnerKind::PreTimeoutLockedPrepareQc",
            "RuntimeSelectedCandidateOwnership::Exact(candidate)",
            "let candidate_is_pre_cut",
            "u128::from(physical.source_ordinal) < physical_cut",
            "self.clocks_armed",
            "self.timeout_due",
            "!self.periodic_timer_due",
            "!self.fifo_ready",
            "!self.completion_ready",
            "!self.progress_ready",
            "!self.normal_ready",
            "candidate.kind == RuntimeCommandKind::Authenticated",
            "candidate.class == SERVICE_CLASS_PROGRESS",
            "candidate_is_pre_cut",
            "RuntimeQueueSelectionKind::PreTimeoutLockedPrepareQc",
        ),
    )
    require_order(
        "runtime",
        scheduler_evidence,
        "ordinary blocked-view evidence must validate exact authorization, FIFO schedule consumption, and ordinary Progress debt",
        (
            "RuntimeQueueSelectionKind::OrdinaryViewProgress",
            "self.view_blocked_progress_authorization.is_some()",
            "authorization.validates_retained_blocker(",
            "scheduled == ScheduledWork::Fifo",
            "self.fifo_owed_after == schedule_after.fifo_owed",
            "!retry_retained",
            "selected != authorization.blocker",
            "service.selected == SERVICE_CLASS_PROGRESS",
            "self.queue_after.service_cursor == service.next",
            "self.queue_after.max_service_debt",
            "self.queue_before.max_service_debt.saturating_add(1)",
            "candidate.selection_seal.matches_scheduler_occurrence(",
        ),
    )

    ordinary_view_release_regression = item(
        "runtime",
        "ordinary_step_skips_only_blocked_prepare_qcs_to_install_matching_tc",
    )
    require_order(
        "runtime",
        ordinary_view_release_regression,
        "ordinary-step future-PrepareQC regression must retain the blocker, service only matching TC, accrue fair debt, reject tampering, and resume ordinary service",
        (
            "RuntimeSelectedOwnerKind::FifoRetryRetained",
            "wire::ConsensusMessageV2Payload::QuorumCertificate(intervening_certificate)",
            "signed_runtime_proposal(&context, &keys, 0xC2)",
            "wire::ConsensusMessageV2Payload::TimeoutCertificate(timeout_certificate)",
            "runtime.schedule.fifo_owed = true",
            "runtime.ingress.next_class = CommandClass::Progress",
            "let normal_debt_before",
            "runtime.step(now)",
            "RuntimeSelectedOwnerKind::PacemakerProgress",
            "tc_scheduler.view_blocked_progress_authorization.is_some()",
            "tc_scheduler.fifo_owed_before",
            "!tc_scheduler.fifo_owed_after",
            "runtime.ingress.next_class, CommandClass::Normal",
            "normal_debt_after, normal_debt_before + 1",
            "RuntimeQueueSelectionKind::OrdinaryViewProgress",
            "tc_scheduler.validate_exact(), Ok(())",
            "authorization.target_view = selected_view",
            "runtime_view_blocked_progress_authorization_projection_hash(authorization)",
            "forged_target.validate_exact().is_err()",
            "normal_scheduler.selected, RuntimeSelectedOwnerKind::Fifo",
            "runtime.take_leader_wire_runtime_terminals().is_empty()",
            "runtime.queued_commands(), 2",
            "AdapterEffect::FetchBody",
            "runtime.queued_commands(), 1",
            "remaining == &intervening_certificate",
        ),
    )

    effect_runtime_context = (
        "impl",
        "EffectRuntime",
        "for",
        "SerializedV2Runtime",
    )
    effect_runtime_freeze = qualified_item(
        "effects",
        "freeze_pre_timeout_locked_prepare_qc_cut",
        effect_runtime_context,
        "effect-runtime fixed-cut forwarding",
    )
    require_tokens(
        "effects",
        effect_runtime_freeze,
        "effect runtime forwards the exact due-timeout cut mint",
        ("SerializedV2Runtime::freeze_pre_timeout_locked_prepare_qc_cut(self, now)",),
    )
    effect_runtime_preview = qualified_item(
        "effects",
        "wire_previews_pre_timeout_locked_prepare_qc",
        effect_runtime_context,
        "effect-runtime wire-preview forwarding",
    )
    require_tokens(
        "effects",
        effect_runtime_preview,
        "effect runtime forwards the exact fixed-cut wire preview",
        (
            "SerializedV2Runtime::wire_previews_pre_timeout_locked_prepare_qc(self, cut, payload)",
        ),
    )
    effect_runtime_step = qualified_item(
        "effects",
        "step_pre_timeout_locked_prepare_qc_effects",
        effect_runtime_context,
        "effect-runtime special-step forwarding",
    )
    require_order(
        "effects",
        effect_runtime_step,
        "effect runtime forwards only the exact special scheduler step",
        (
            "self.try_step_pre_timeout_locked_prepare_qc(now, cut)",
            ".map_err(|error| error.to_string())",
        ),
    )

    effect_executor_context = (
        "impl",
        "<",
        "R",
        ":",
        "EffectRuntime",
        ">",
        "V2EffectExecutor",
        "<",
        "R",
        ">",
    )
    executor_pre_timeout_freeze = qualified_item(
        "effects",
        "freeze_pre_timeout_locked_prepare_qc_cut",
        effect_executor_context,
        "effect executor fixed-cut mint",
    )
    require_order(
        "effects",
        executor_pre_timeout_freeze,
        "executor publishes owners and receiver cut before freezing timeout authority",
        (
            "self.ensure_open()?",
            "self.retained_effect_batch.is_some()",
            "self.parked_effect_batch.is_some()",
            "self.pending_runner_decision_cleanup.is_some()",
            "self.publish_external_lifecycle_owners()?",
            "self.runtime.set_ingress_physical_cut(physical_cut)",
            "self.runtime.freeze_pre_timeout_locked_prepare_qc_cut(now)",
        ),
    )
    executor_pre_timeout_preview = qualified_item(
        "effects",
        "wire_previews_pre_timeout_locked_prepare_qc",
        effect_executor_context,
        "effect executor fixed-cut wire preview",
    )
    require_tokens(
        "effects",
        executor_pre_timeout_preview,
        "executor wire preview is a read-only runtime forwarding",
        ("self.runtime.wire_previews_pre_timeout_locked_prepare_qc(cut, payload)",),
    )
    executor_pre_timeout_step = qualified_item(
        "effects",
        "step_pre_timeout_locked_prepare_qc_once",
        effect_executor_context,
        "effect executor special pre-timeout turn",
    )
    require_order(
        "effects",
        executor_pre_timeout_step,
        "special executor turn owns WAL, scheduler evidence, reconciliation, and effects",
        (
            "self.ensure_open()?",
            "self.retained_effect_batch.is_some()",
            "self.parked_effect_batch.is_some()",
            "self.pending_runner_decision_cleanup.is_some()",
            "self.publish_external_lifecycle_owners()",
            "let decision_before_step",
            "self.output_guard.begin_fail_stop_operation()",
            "self.runtime.step_pre_timeout_locked_prepare_qc_effects(now, cut)",
            "if step.is_some()",
            "self.runtime.take_scheduler_ownership()",
            "wal_step.complete()",
            "self.finish_runtime_step_reconciliation(services)",
            "let decision_after_step",
            "self.plan_runner_decision_cleanup(decision_before_step, decision_after_step)",
            "None | Some(RuntimeStep::Idle)",
            "self.publish_external_lifecycle_owners()",
            "self.publish_status(services)",
            "Some(RuntimeStep::Advanced(effects))",
            "self.consume_pacemaker_effects_with_runner_decision_cleanup(",
        ),
    )

    require_order(
        "height_driver",
        lifecycle_height_driver,
        "activated lifecycle ordinary Completion/Runtime/Ingress batch",
        (
            "outer_ingress_turns(limit, context_id, height)",
            "if !producer_claim.blocks_runtime()",
            "settle_one_recovered_lifecycle_output(",
            "recovered_output_drain_disposition(recovered_output_settlement, producer_claim)",
            "LifecycleRunnerRankTarget::Completion",
            "activated.drive_completion_pre_gate(current_turn, lane_work)",
            "PreGate::Ordinary(ordinary_turn)",
            "drain_one_ordinary_completion_after_lifecycle_pass_through",
            "PreGate::Selected(selected)",
            "PreGate::Ready(ready) if terminal_finalization_cut.is_some()",
            "terminal_ready_broadcast_permit()",
            "drive_apply_terminal_ready_broadcast_turn(ready, permit)",
            "PreGate::Ready(ready) if producer_claim.apply_terminal_settled()",
            "apply_terminal_ready_broadcast_permit()",
            "drive_apply_terminal_ready_broadcast_turn(ready, permit)",
            "PreGate::Ready(ready) if producer_claim.permits_ready_completion()",
            "producer_claim.required_ready_ordinal()",
            "drive_ready_completion_turn_requiring_ordinal(ready, ordinal)",
            "None => activated.drive_ready_completion_turn(ready)",
            "completion_selection_stops_batch(&selected)",
            "LifecycleV2IngressDrainDispositionV1::after_terminal_settlement(",
            "LifecycleRunnerRankTarget::Runtime",
            "if producer_claim.blocks_runtime()",
            "blocked_runtime_drain_disposition(producer_claim)",
            "advance_executor(",
            "LifecycleRunnerRankTarget::Ingress",
            "activated.drive_ingress_turn(current_turn)",
            "activated.consume_prepared_ordinary_ingress_turn(",
        ),
    )
    require_order(
        "height_driver",
        lifecycle_height_driver,
        "bounded exact Prepare progress precedes the owned timeout in one Runtime turn",
        (
            "LifecycleRunnerRankTarget::Runtime =>",
            "if producer_claim.blocks_runtime()",
            "executor.freeze_pre_timeout_locked_prepare_qc_cut(",
            "receiver.next_physical_admission_ordinal()",
            "executor.step_pre_timeout_locked_prepare_qc_once(now, &cut, services)",
            "if pre_timeout_advanced",
            "if let Some(pre_timeout_cut) = pre_timeout_cut",
            "prepare_pre_timeout_locked_prepare_qc_ingress_turn(&pre_timeout_cut)",
            "PreTimeoutIngress::Empty => {}",
            "PreTimeoutIngress::RestartRequired",
            "PreTimeoutIngress::ObsoletePredecessor(prepared)",
            "activated.consume_prepared_ordinary_ingress_turn(",
            "ProductionPreparedOrdinaryIngressConsumptionV1::Continue",
            "LifecycleV2IngressDrainDispositionV1::retry_before_producer(",
            "PreTimeoutIngress::ExactPrepareProgress(prepared)",
            "activated.consume_prepared_ordinary_ingress_turn(",
            "ProductionPreparedOrdinaryIngressConsumptionV1::Continue",
            "activated.with_runner_runtime(",
            "executor.step_pre_timeout_locked_prepare_qc_once(",
            "EffectExecutorStep::Idle",
            "EffectExecutorStep::Advanced",
            "reconcile_executor_locked_body(executor, services)",
            "LifecycleV2IngressDrainDispositionV1::retry_before_producer(",
            "advance_executor(receiver, owner, executor, services, 1)",
        ),
    )
    executor_advance = item("runner", "advance_executor")
    require_order(
        "runner",
        executor_advance,
        "bounded runtime cold-output retry before and after every executor step",
        (
            "for _ in 0..limit.max(1)",
            "settle_one_recovered_lifecycle_output",
            "executor.settle_pending_live_wal_sign_admission(lifecycle_owner, services)",
            "executor.set_ingress_physical_cut(receiver.next_physical_admission_ordinal())",
            "services.prepare_completion_runtime_cut(",
            "V2CompletionRuntimeCutDecisionV1::RetryCompletion",
            "executor.step_after_completion_runtime_cut(completion_cut, services)",
            "V2CompletionRuntimeCutDecisionV1::CapacityRelief",
            "executor.step_completion_capacity_relief_after_cut(completion_cut, services)",
            "settle_one_recovered_lifecycle_output",
            "executor.settle_pending_live_wal_sign_admission(lifecycle_owner, services)",
        ),
    )
    recovered_output_yield = item("runner", "recovered_lifecycle_output_requires_yield")
    require_order(
        "runner",
        recovered_output_yield,
        "cold-output settlement yield and retry classification",
        (
            "RecoveredLifecycleOutputSettlementV1::Completed",
            "RecoveredLifecycleOutputSettlementV1::SourceRetained",
            "true",
            "RecoveredLifecycleOutputSettlementV1::Empty",
            "RecoveredLifecycleOutputSettlementV1::Deferred",
            "false",
        ),
    )
    for source_name, target, token, expected, label in (
        (
            "height_driver",
            lifecycle_height_driver,
            "settle_one_recovered_lifecycle_output(",
            2,
            "terminal and bounded outer-turn cold-output retry",
        ),
        (
            "runner",
            executor_advance,
            "settle_one_recovered_lifecycle_output(",
            2,
            "pre/post executor-step cold-output retry",
        ),
    ):
        if target is None:
            continue
        observed = _token_sequence_count(
            rust_code_tokens(target.source), rust_code_tokens(token)
        )
        if observed != expected:
            errors.append(
                f"{paths[source_name]}:{target.line}: {label} must call "
                f"{token!r} exactly {expected} time(s); found {observed}"
            )
    require_tokens(
        "height_driver",
        lifecycle_height_driver,
        "activated lifecycle ordinary batch selected outcomes",
        (
            "ProductionLifecycleCompletionPreGateV1 as PreGate",
            "ProductionLifecycleCompletionTurnV1 as CompletionTurn",
            "selected.restart_required()",
            "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCapacityPending",
            "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchPreparationRetry",
            "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchCompetingReady",
            "ProductionLifecycleIngressSelectionV1::RecoveredDecisionFetchQueued",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeCapacityPending",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeCompetingReady",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeQueued",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeReplayQueued",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeTerminal",
            "ProductionLifecycleIngressSelectionV1::CertifiedServeRetry",
            "ProductionLifecycleIngressSelectionV1::RestartRequired",
        ),
    )
    if lifecycle_height_driver is not None:
        height_driver_tokens = rust_code_tokens(lifecycle_height_driver.source)
        for token in (
            "CompletionTurn::PassThrough(empty_turn)",
            "CompletionTurn::Selected(selected)",
        ):
            observed = _token_sequence_count(
                height_driver_tokens, rust_code_tokens(token)
            )
            if observed != 3:
                errors.append(
                    f"{paths['height_driver']}:{lifecycle_height_driver.line}: "
                    "the terminal-finalization and settled post-Apply authority "
                    "paths, plus the ordinary Ready branch, "
                    "must each retain "
                    f"{token!r}; found {observed} total occurrence(s)"
                )
    if lifecycle_height_driver is not None:
        height_driver_tokens = rust_code_tokens(lifecycle_height_driver.source)
        for forbidden in (
            "output_guard: &Arc<ConsensusOutputGuard>",
            "drain_v2_ingress(",
            "V2IngressDrainMode",
        ):
            if _token_sequence_count(height_driver_tokens, rust_code_tokens(forbidden)):
                errors.append(
                    f"{paths['height_driver']}:{lifecycle_height_driver.line}: "
                    "activated lifecycle ordinary batch exposes obsolete or "
                    f"caller-substitutable surface {forbidden!r}"
                )
    if sources["lifecycle_run_inner"].count("drain_lifecycle_v2_ingress(") != 1:
        errors.append(
            f"{paths['lifecycle_run_inner']}: activated lifecycle loop must route "
            "exactly its main ordinary batch through the shared lifecycle "
            "height driver"
        )

    lifecycle_live_loop = item(
        "lifecycle_run_inner", "run_lifecycle_active_height"
    )
    require_order(
        "lifecycle_run_inner",
        lifecycle_live_loop,
        "pre-drain lane-only auxiliary-runtime barrier",
        (
            "let lane_only_completion_barrier = producer_claim.blocks_runtime()",
            "if let Some(cut) = terminal_finalization_cut.as_ref()",
            "retry_decided_lane_recovery_exact_output(",
            "else if lane_only_completion_barrier",
            "if producer_claim.permits_decided_lane_recovery_ingress()",
            "if producer_claim.permits_open_decided_lane_recovery_ingress()",
            "drain_decided_lane_recovery_ingress(",
            "producer_claim.blocked_ordinary_lane_local_ingress_permit()",
            "drain_blocked_ordinary_lane_local_ingress(",
            "drain_lane_relay_ingress(",
            "drive_merge_sidecar_recovery(",
            "service_historical_recovery_tick(",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()",
            "dispatch_lane_work_effects(",
            "else",
            "broadcast_npos_beacon_messages(",
            "let discovery_was_outstanding = if terminal_finalization_fenced",
            "else if lane_only_completion_barrier",
            "block_sync_request.is_some()",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "drain_lifecycle_v2_ingress(",
        ),
    )
    require_order(
        "lifecycle_run_inner",
        lifecycle_live_loop,
        "post-reconciliation blocked ordinary lane-local progress",
        (
            "let discovery_was_outstanding = if terminal_finalization_fenced",
            "else if lane_only_completion_barrier",
            "let directive = reconcile_executor_locked_body(executor, services)?",
            "local_proposal.state.reconcile(LocalProposalOwner::from(directive))",
            "lane_work.retain_merge_sidecars_for_global_view(",
            "executor.acknowledge_runner_decision_cleanup(",
            "producer_claim.blocked_ordinary_lane_local_ingress_permit()",
            "drain_blocked_ordinary_lane_local_ingress(",
            "services.replay_buffered_chunks(executor)",
        ),
    )
    if lifecycle_live_loop is not None:
        barrier_start = lifecycle_live_loop.source.find(
            "if lane_only_completion_barrier {"
        )
        barrier_end = lifecycle_live_loop.source.find("} else {", barrier_start)
        barrier_source = (
            lifecycle_live_loop.source[barrier_start:barrier_end]
            if barrier_start >= 0 and barrier_end > barrier_start
            else ""
        )
        barrier_tokens = rust_code_tokens(barrier_source)
        for required in (
            "drain_decided_lane_recovery_ingress(",
            "producer_claim.blocked_ordinary_lane_local_ingress_permit()",
            "drain_blocked_ordinary_lane_local_ingress(",
            "drain_lane_relay_ingress(",
            "drive_merge_sidecar_recovery(",
            "service_historical_recovery_tick(",
            "lane_work.schedule_autonomous_new_view_timeouts(",
            "lane_work.schedule_retransmission()",
            "dispatch_lane_work_effects(",
        ):
            count = _token_sequence_count(barrier_tokens, rust_code_tokens(required))
            if count != 1:
                errors.append(
                    f"{paths['lifecycle_run_inner']}:{lifecycle_live_loop.line}: "
                    "lane-transport-only barrier must retain exactly "
                    f"one {required!r} seam; found {count}"
                )
        for required in (
            "producer_claim.blocked_ordinary_lane_local_ingress_permit()",
            "drain_blocked_ordinary_lane_local_ingress(",
        ):
            count = _token_sequence_count(
                rust_code_tokens(lifecycle_live_loop.source), rust_code_tokens(required)
            )
            if count != 2:
                errors.append(
                    f"{paths['lifecycle_run_inner']}:{lifecycle_live_loop.line}: "
                    "blocked ordinary lane-local progress must retain exactly two "
                    f"{required!r} seams; found {count}"
                )
        forbidden = tuple(
            token
            for token in (
                "reconcile_executor_locked_body(",
                "advance_executor(",
                "retry_exact_output_and_apply_sidecar_admissions(",
                "replay_buffered_chunks(",
                "broadcast_npos_beacon_messages(",
                "service_kura_replica_advert_refresh_turn(",
                "schedule_local_proposal(",
            )
            if _token_sequence_count(barrier_tokens, rust_code_tokens(token))
        )
        if forbidden:
            errors.append(
                f"{paths['lifecycle_run_inner']}:{lifecycle_live_loop.line}: "
                "lane-transport-only barrier retains forbidden "
                f"ordinary runtime authority {forbidden!r}"
            )
    require_order(
        "lifecycle_run_inner",
        lifecycle_live_loop,
        "post-settlement ordinary-runtime cut",
        (
            "producer_claim = drain_disposition.producer_claim()",
            "if drain_disposition.requires_yield()",
            "let (ready_to_finish, executor_slice) = if terminal_finalization_fenced",
            "drain_disposition.terminal_settlement_stops_runtime()",
            "executor.ready_to_finish()",
            "AdvanceExecutorSliceOutcomeV1::Idle",
            "else",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "let executor_slice = advance_executor(",
            "if let AdvanceExecutorSliceOutcomeV1::Yielded(_) = executor_slice",
            "retry_exact_output_and_apply_sidecar_admissions(",
            "let directive = reconcile_executor_locked_body(executor, services)",
            "match executor_slice",
            "AdvanceExecutorSliceOutcomeV1::AdvancedAtSliceBoundary => {}",
            "let terminal_planning_fenced = terminal_finalization_fenced || producer_claim.apply_terminal_settled()",
            "if terminal_planning_fenced && !ready_to_finish",
            "close_admission_for_restart()",
            "let producer_turn = if terminal_planning_fenced",
            "None",
            "if !terminal_planning_fenced && (!ready_to_finish || producer_turn.is_some())",
            "schedule_local_proposal(",
        ),
    )

    preactivation_start = sources["runner"].find(
        "pub(in crate::sumeragi) struct ProductionLifecyclePreActivationRunnerBorrowV1"
    )
    preactivation_end = sources["runner"].find(
        "/// Exact reducer facts which own one local proposal-side work item.",
        preactivation_start,
    )
    preactivation_region = (
        sources["runner"][preactivation_start:preactivation_end]
        if preactivation_start >= 0 and preactivation_end > preactivation_start
        else ""
    )
    for required in (
        "_seal: ProductionLifecyclePreActivationRunnerBorrowSealV1",
        "local_proposal: Option<ProductionLifecycleLocalProposalStateV1>",
        "struct ProductionLifecyclePreActivationRunnerBorrowSealV1;",
        "impl Drop for ProductionLifecyclePreActivationRunnerBorrowSealV1",
        "fn mint_for_recovered_runner() -> Self",
        "local_proposal: Some(ProductionLifecycleLocalProposalStateV1::fresh())",
        "#[cfg(test)]",
        "pub(in crate::sumeragi) fn for_test() -> Self",
        "fn bind_recovered_local_proposal(",
        "let Some(local_proposal) = self.local_proposal.as_mut()",
        "if !local_proposal.state.is_pristine()",
        "LocalProposalState::from_recovered_lifecycle_attempt(true, directive)",
        "fn local_proposal_state_is_pristine(",
        "fn prepared_local_proposal_exactly_matches(",
        "fn prepared_local_proposal_mut(",
        "self.local_proposal.as_mut()",
    ):
        if required not in preactivation_region:
            errors.append(
                f"{paths['runner']}: sealed lifecycle preactivation runner borrow "
                f"omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub _seal:",
        "pub(crate) _seal:",
        "pub(in crate::sumeragi) _seal:",
        "pub local_proposal:",
        "pub(crate) local_proposal:",
        "pub(in crate::sumeragi) local_proposal:",
        "pub(in crate::sumeragi) fn mint_for_recovered_runner",
        "fn into_parts(",
    ):
        if forbidden in preactivation_region:
            errors.append(
                f"{paths['runner']}: sealed lifecycle preactivation runner borrow "
                f"exposes forbidden surface {forbidden!r}"
            )

    proposal_state_start = sources["runner"].find(
        "pub(in crate::sumeragi) struct ProductionLifecycleLocalProposalStateV1"
    )
    proposal_state_end = sources["runner"].find(
        "/// Run the v2-only worker until shutdown", proposal_state_start
    )
    proposal_state_region = (
        sources["runner"][proposal_state_start:proposal_state_end]
        if proposal_state_start >= 0 and proposal_state_end > proposal_state_start
        else ""
    )
    for required in (
        "state: LocalProposalState",
        "fn fresh() -> Self",
        "fn already_attempted(",
    ):
        if required not in proposal_state_region:
            errors.append(
                f"{paths['runner']}: opaque lifecycle local-Proposal state "
                f"omits {required!r}"
            )

    prepared_state_start = sources["launch"].find(
        "pub(in crate::sumeragi) struct ProductionLifecyclePreparedLocalProposalStateV1"
    )
    prepared_state_end = sources["launch"].find(
        "/// Opaque lifecycle stack after clocks", prepared_state_start
    )
    prepared_state_region = (
        sources["launch"][prepared_state_start:prepared_state_end]
        if prepared_state_start >= 0 and prepared_state_end > prepared_state_start
        else ""
    )
    for required in (
        "runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1",
        "context_id: wire::HeightContextId",
        "directive: super::super::v2::LocalProposalDirective",
        "fn exactly_matches(",
        "self.context_id == context_id",
        "self.directive == directive",
        "prepared_local_proposal_exactly_matches(directive)",
    ):
        if required not in prepared_state_region:
            errors.append(
                f"{paths['launch']}: affine prepared local-Proposal state omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub runner:",
        "pub context_id:",
        "pub directive:",
        "fn into_parts(",
    ):
        if forbidden in prepared_state_region:
            errors.append(
                f"{paths['launch']}: affine prepared local-Proposal state exposes "
                f"forbidden surface {forbidden!r}"
            )
    prepared_state_behavior = item(
        "launch_tests", "prepared_local_proposal_state_is_affine_and_context_directive_bound"
    )
    require_order(
        "launch_tests",
        prepared_state_behavior,
        "affine prepared local-Proposal state behavior",
        (
            "prepared.exactly_matches(context_id, directive)",
            "!prepared.exactly_matches(foreign_context, directive)",
            "!prepared.exactly_matches(context_id, foreign_directive)",
        ),
    )
    for forbidden in ("pub state:", "fn into_parts(", "derive(Clone)", "derive(Copy)"):
        if forbidden in proposal_state_region:
            errors.append(
                f"{paths['runner']}: opaque lifecycle local-Proposal state "
                f"exposes forbidden surface {forbidden!r}"
            )
    live_proposal_behavior = item(
        "startup_test", "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout"
    )
    require_order(
        "launch_tests",
        live_proposal_behavior,
        "activated lifecycle retains the exact runner local-Proposal owner",
        (
            "activated.with_runner_runtime(",
            "services.matches_lifecycle_executor_output_guard(executor)",
            "assert!(local_proposal.already_attempted(directive))",
        ),
    )

    runtime_clock = item("runtime", "lifecycle_live_clocks_are_armed")
    require_tokens(
        "runtime",
        runtime_clock,
        "preactivation live-clock state oracle",
        ("self.clocks_armed",),
    )
    effects_clock = item("effects", "lifecycle_live_clocks_are_unarmed")
    require_tokens(
        "effects",
        effects_clock,
        "preactivation executor live-clock state oracle",
        ("!self.runtime.lifecycle_live_clocks_are_armed()",),
    )
    fail_stop_start = sources["preactivation"].find(
        "struct ProductionLifecyclePreActivationFailStopScopeV1"
    )
    fail_stop_end = sources["preactivation"].find(
        "impl LaunchedProductionLifecycleV1", fail_stop_start
    )
    fail_stop_region = (
        sources["preactivation"][fail_stop_start:fail_stop_end]
        if fail_stop_start >= 0 and fail_stop_end > fail_stop_start
        else ""
    )
    for required in (
        "output_guard: Arc<ConsensusOutputGuard>",
        "armed: bool",
        "impl Drop for ProductionLifecyclePreActivationFailStopScopeV1",
        "self.output_guard.close_admission_for_restart()",
    ):
        if required not in fail_stop_region:
            errors.append(
                f"{paths['preactivation']}: lifecycle preactivation non-permit fail-stop "
                f"scope omits {required!r}"
            )
    if "ConsensusFailStopOperation" in fail_stop_region:
        errors.append(
            f"{paths['preactivation']}: lifecycle preactivation fail-stop scope must not "
            "hold an output read permit across nested setup"
        )
    setup = item("preactivation", "with_runner_setup_transaction")
    require_order(
        "preactivation",
        setup,
        "fail-stop closed-ingress lifecycle preactivation setup",
        (
            "let output_guard = self.services.lifecycle_output_guard()",
            "let initial_admission = output_guard.acquire()",
            "ProductionLifecyclePreActivationFailStopScopeV1::new",
            "drop(initial_admission)",
            "matches_lifecycle_executor_output_guard(&self.executor)",
            "self.leader_wire_ingress_binding.ingress.state.lock().open",
            "self.completion_observer_activation.is_none()",
            "self.executor.lifecycle_live_clocks_are_unarmed()",
            "operation(&mut self.executor, &mut self.services)?",
            "matches_lifecycle_executor_output_guard(&self.executor)",
            "self.leader_wire_ingress_binding.ingress.state.lock().open",
            "self.completion_observer_activation.is_none()",
            "self.executor.lifecycle_live_clocks_are_unarmed()",
            "let final_admission = output_guard.acquire()",
            "setup.complete()",
            "drop(final_admission)",
        ),
    )
    if setup is not None:
        setup_tokens = rust_code_tokens(setup.source)
        for token, expected in (
            ("ProductionLifecyclePreActivationErrorV1::OutputClosed", 2),
            ("ProductionLifecyclePreActivationErrorV1::OwnershipMismatch", 2),
            ("ProductionLifecyclePreActivationErrorV1::IngressAlreadyOpen", 2),
            ("ProductionLifecyclePreActivationErrorV1::CompletionObserverMissing", 2),
            ("ProductionLifecyclePreActivationErrorV1::ClocksAlreadyArmed", 2),
        ):
            observed = _token_sequence_count(setup_tokens, rust_code_tokens(token))
            if observed != expected:
                errors.append(
                    f"{paths['preactivation']}:{setup.line}: fail-stop closed-ingress "
                    f"lifecycle preactivation setup must retain {token!r} exactly "
                    f"{expected} time(s); found {observed}"
                )
        for forbidden in (
            "&mut self.owner",
            "ProductionLifecyclePreActivationRunnerBorrowV1",
            "bind_recovered_local_proposal",
            "begin_fail_stop_operation(",
            "arm_live_clocks(",
            "activate_effect_completion_observer(",
            "open_and_publish(",
            "into_parts(",
        ):
            if forbidden in setup.source:
                errors.append(
                    f"{paths['preactivation']}:{setup.line}: preactivation setup exposes "
                    f"forbidden transition {forbidden!r}"
                )
    public_setup = item("preactivation", "with_runner_setup")
    require_tokens(
        "preactivation",
        public_setup,
        "public preactivation runner aperture",
        ("self.with_runner_setup_transaction(operation)",),
    )
    if public_setup is not None:
        for forbidden in (
            "bind_recovered_local_proposal",
            "operation(&mut self.executor",
        ):
            if forbidden in public_setup.source:
                errors.append(
                    f"{paths['preactivation']}:{public_setup.line}: public setup aperture "
                    f"exposes forbidden Proposal mutation {forbidden!r}"
                )
    fail_stop_behavior = item(
        "launch_tests",
        "preactivation_fail_stop_scope_closes_on_drop_and_disarms_on_complete",
    )
    require_order(
        "launch_tests",
        fail_stop_behavior,
        "preactivation non-permit fail-stop behavior",
        (
            "ProductionLifecyclePreActivationFailStopScopeV1::new( Arc::clone(&dropped_guard), )",
            "dropped_guard.restart_required()",
            "ProductionLifecyclePreActivationFailStopScopeV1::new(Arc::clone(&completed_guard)) .complete()",
            "!completed_guard.restart_required()",
        ),
    )
    require_tokens(
        "launch_tests",
        fail_stop_behavior,
        "preactivation non-permit fail-stop behavior",
        (
            "assert!(dropped_guard.restart_required())",
            "assert!(!completed_guard.restart_required())",
        ),
    )

    run_inner = item("runner", "run_inner")
    require_order(
        "runner",
        run_inner,
        "PendingKura and ordinary heights split into sealed lifecycle loops",
        (
            "let pending_kura_apply = recovered.pending_kura_apply()",
            "match pending_kura_apply",
            "None => lifecycle_run_inner::run_non_pending_lifecycle_loop(",
            "Some(pending) => lifecycle_pending_kura::run_pending_kura_lifecycle_height(",
        ),
    )
    lifecycle_loop = item("lifecycle_run_inner", "run_non_pending_lifecycle_loop")
    require_order(
        "lifecycle_run_inner",
        lifecycle_loop,
        "sealed non-Pending lifecycle startup and activation",
        (
            "V2BodyStoreCapacity::new(",
            "V2BodyStore::open_with_policy_and_capacity(",
            ".into_quarantined_recovered_startup()",
            "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
            ".authenticate_final_wal_startup_authority()",
            "bind_production_lifecycle_owner_factory_inputs_v1(",
            "open_production_lifecycle_owner_v1(",
            "launch_non_pending_lifecycle_height(",
            "ProductionLifecyclePreActivationRunnerBorrowV1::mint_for_recovered_runner()",
            "recover_canonical_bodies_before_activation(",
            "initialize_recovered_local_proposal(setup_runner)",
            "let height_started_at = Instant::now()",
            "preactivation.activate(height_started_at, local_proposal)",
            "run_lifecycle_active_height(",
        ),
    )
    lifecycle_active = item("lifecycle_run_inner", "run_lifecycle_active_height")
    _require_rust_token_sequence(
        paths["lifecycle_run_inner"],
        lifecycle_active,
        """
let directive = reconcile_executor_locked_body(executor, services)?;
local_proposal
    .state
    .reconcile(LocalProposalOwner::from(directive));
lane_work.retain_merge_sidecars_for_global_view(
    directive.tag().view(),
    directive.locked_subject(),
    directive.decided_subject(),
)?;
executor.acknowledge_runner_decision_cleanup(
    directive.tag(),
    directive.decided_subject(),
)?;
""",
        "each ordinary reconciliation point must retire the local proposal and losing lane sidecars before acknowledging runner Decision cleanup",
        errors,
        count=2,
    )
    require_order(
        "lifecycle_run_inner",
        lifecycle_active,
        "lifecycle live-height finalization and successor storage handoff",
        (
            "drain_lifecycle_v2_ingress(",
            "claim_producer_turn_for_local_proposal(&mut active_runner)",
            "settle_producer_turn_after_local_proposal(&mut active_runner, attempted)",
            "finalize_lifecycle_height(",
            "DurableV2PredecessorIdentity::authenticate(artifact, receipt)",
            "build_verified_successor(",
            "into_parts_with_lifecycle_storage_authority(",
        ),
    )
    require_order(
        "lifecycle_run_inner",
        lifecycle_active,
        "ordinary finalization must close ingress and finitely drain terminal recovery before consuming finalized rollover",
        (
            "let mut finalized_ingress_closed = false",
            "loop",
            "if rollover_ready",
            "if !finalized_ingress_closed",
            "activated.close_runner_ingress_for_finalized_drain(&mut active_runner, receiver)?",
            "finalized_ingress_closed = true",
            "drain_decided_lane_recovery_ingress(",
            "dispatch_lane_work_effects(",
            "drained.is_some()",
            "if drained_terminal_ingress",
            "continue",
            "receiver.ensure_closed_drained_cut()",
            "finalize_lifecycle_height(",
        ),
    )
    _require_rust_token_sequence(
        paths["lifecycle_run_inner"],
        lifecycle_active,
        "let mut finalized_ingress_closed = false;",
        "ordinary finalization ingress close state must be initialized exactly once and never reopened",
        errors,
    )
    ordinary_close = item(
        "launch", "close_runner_ingress_for_finalized_drain"
    )
    _require_exact_rust_tokens(
        paths["launch"],
        ordinary_close,
        """
pub(in crate::sumeragi) fn close_runner_ingress_for_finalized_drain(
    &self,
    _runner: &mut super::super::v2_runner::ProductionLifecycleActiveRunnerBorrowV1,
    receiver: &Arc<FairV2Ingress>,
) -> Result<(), super::super::v2_runner::V2RunnerError> {
    self.runner_activation.close_ingress(receiver)?;
    if !Arc::ptr_eq(receiver, &self.launched.leader_wire_ingress_binding.ingress) {
        return Err(super::super::v2_runner::V2RunnerError::LifecycleActivationIngressMismatch);
    }
    Ok(())
}
""",
        "ordinary finalized drain must close the passed physical receiver and prove it is the common activated ingress without consuming lifecycle authority",
        errors,
    )
    lifecycle_finalization = item("lifecycle_run_inner", "finalize_lifecycle_height")
    require_order(
        "lifecycle_run_inner",
        lifecycle_finalization,
        "lifecycle finalization output/store/cleanup transaction",
        (
            "activated.into_finalized_rollover(active_runner)",
            "finalized.finality()",
            "prepare_successor(receipt, artifact, &mut lane_work)",
            "finalized.rollover_outputs(",
            "post_output.retire_lifecycle_stores()",
            "cleanup_ready.finish_cleanup(Duration::ZERO, cleanup_supervisor)",
        ),
    )
    require_order(
        "lifecycle_run_inner",
        lifecycle_active,
        "coordinator ProducerTurn claim, attempt, and durable settlement",
        (
            "let (ready_to_finish, executor_slice) = if terminal_finalization_fenced",
            "drain_disposition.terminal_settlement_stops_runtime()",
            "let terminal_planning_fenced = terminal_finalization_fenced || producer_claim.apply_terminal_settled()",
            "if terminal_planning_fenced && !ready_to_finish",
            "let producer_turn = if terminal_planning_fenced",
            "match activated.claim_producer_turn_for_local_proposal(&mut active_runner)",
            "if !terminal_planning_fenced && (!ready_to_finish || producer_turn.is_some())",
            "schedule_local_proposal(",
            "dispatch_lane_work_effects(",
            "if let Some(claimed) = producer_turn",
            "claimed.into_attempted(super::producer_turn_attempt_permit(&mut active_runner))",
            "settle_producer_turn_after_local_proposal(&mut active_runner, attempted)",
            "let finalization_ready = if ready_to_finish",
            "activated.ready_for_finalized_rollover(&mut active_runner)",
            "finalize_lifecycle_height(",
        ),
    )
    startup_setup = item(
        "startup_test",
        "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
    )
    require_order(
        "startup_test",
        startup_setup,
        "production-shaped closed-ingress preactivation setup behavior",
        (
            "ProductionLifecyclePreActivationRunnerBorrowV1::for_test()",
            ".with_runner_setup(&mut setup_runner",
            "services.matches_lifecycle_executor_output_guard(executor)",
            "executor.current_tag()",
            "setup_tag.height()",
            "!leader_wire_ingress.state.lock().open",
        ),
    )

    proposal_type_start = sources["adapter"].find(
        "pub(in crate::sumeragi) struct RecoveredLifecycleLocalProposalAttemptV1"
    )
    proposal_type_end = sources["adapter"].find(
        "/// Adapter and residual replay effects retained", proposal_type_start
    )
    proposal_type = (
        sources["adapter"][proposal_type_start:proposal_type_end]
        if proposal_type_start >= 0 and proposal_type_end > proposal_type_start
        else ""
    )
    for required in (
        "tag: reducer::EventTag",
        "round: wire::ConsensusRound",
        "subject: wire::BlockSubject",
        "fn from_authenticated_durable_current_round(",
        "adapter.reducer.durable_state().proposal_intent(round)",
        "fn from_control(control: &RecoveredWalControlSign) -> Option<Self>",
        "request: SignRequest::Proposal(proposal)",
        "fn exactly_matches_directive(",
        "self.tag == current.tag()",
        "current.decided_subject().is_none()",
        ".locked_body()",
    ):
        if required not in proposal_type:
            errors.append(
                f"{paths['adapter']}: opaque recovered local-Proposal owner "
                f"omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub tag:",
        "pub round:",
        "pub subject:",
        "fn into_parts(",
        "fn tag(",
        "fn round(",
        "fn subject(",
        "fn effect(",
    ):
        if forbidden in proposal_type:
            errors.append(
                f"{paths['adapter']}: opaque recovered local-Proposal owner "
                f"exposes forbidden surface {forbidden!r}"
            )

    proposal_factory = item(
        "adapter", "open_production_lifecycle_owner_v1_at_authenticated_roots"
    )
    require_order(
        "adapter",
        proposal_factory,
        "recovered local-Proposal owner factory dispatch",
        (
            "RecoveredLifecycleLocalProposalAttemptV1::from_authenticated_durable_current_round( &adapter, )",
            "RecoveredWalStartupAuthorityV1::ControlSign(control)",
            "Self::open_recovered_control_authority_branch(",
            "verified, adapter, effects, control, local_proposal_attempt, body_store,",
        ),
    )
    proposal_control = item(
        "adapter", "open_recovered_control_authority_branch"
    )
    require_order(
        "adapter",
        proposal_control,
        "recovered local-Proposal owner projection handoff",
        (
            "RecoveredLifecycleLocalProposalAttemptV1::from_control(&control)",
            "project_recovered_wal_control_sign(&verified, control)",
            "Self::ensure_recovered_body_store_context(&body_store, &verified)",
            "Self::open_recovered_control_projection_branch(",
            "projected, local_proposal_attempt, body_store,",
        ),
    )
    proposal_projection = item(
        "adapter", "open_recovered_control_projection_branch"
    )
    require_order(
        "adapter",
        proposal_projection,
        "recovered local-Proposal owner factory handoff",
        (
            "Self::open_recovered_non_apply_stores(",
            "ProductionLifecycleOwnerV1::open_recovered_control_startup(",
            "ProductionLifecycleAdapterStartupV1::recovered_with_local_proposal_attempt( adapter, effects, local_proposal_attempt, )",
        ),
    )
    proposal_runtime = item("pending_kura", "into_serialized_runtime")
    require_order(
        "pending_kura",
        proposal_runtime,
        "recovered local-Proposal runtime ownership handoff",
        (
            "local_proposal_attempt",
            "pending_kura_apply.is_none() || local_proposal_attempt.is_none()",
            "Ok((runtime, replay, local_proposal_attempt))",
        ),
    )
    proposal_initialize = item(
        "preactivation", "initialize_recovered_local_proposal"
    )
    require_order(
        "preactivation",
        proposal_initialize,
        "closed-ingress recovered local-Proposal initialization",
        (
            "self.recovered_local_proposal_attempt.take()",
            "self.with_runner_setup_transaction(",
            "executor.local_proposal_directive()",
            "recovered.exactly_matches_directive(directive)",
            "runner.bind_recovered_local_proposal(directive)",
            "ProductionLifecyclePreActivationErrorV1::RunnerProposalStateNotPristine",
            "ProductionLifecyclePreActivationErrorV1::RecoveredProposalMismatch",
            "ProductionLifecyclePreparedLocalProposalStateV1 { runner, context_id, directive, }",
            "Ok((directive, prepared))",
        ),
    )
    if proposal_initialize is not None:
        for forbidden in (
            "into_parts(",
            "recovered.tag",
            "recovered.round",
            "recovered.subject",
            "AdapterEffect",
        ):
            if forbidden in proposal_initialize.source:
                errors.append(
                    f"{paths['preactivation']}:{proposal_initialize.line}: recovered "
                    f"local-Proposal initialization exposes forbidden seam {forbidden!r}"
                )
    proposal_bind_call = "runner.bind_recovered_local_proposal(directive)"
    proposal_bind_count = sources["preactivation"].count(proposal_bind_call)
    if proposal_bind_count != 1:
        errors.append(
            f"{paths['preactivation']}: only the WAL-authenticated initializer may "
            f"bind runner local-Proposal state; found {proposal_bind_count} calls"
        )

    complete_tip_proposal_initialize = item(
        "ledger", "initialize_recovered_local_proposal"
    )
    require_order(
        "ledger",
        complete_tip_proposal_initialize,
        "CompleteTip recovered local-Proposal initialization delegation",
        (
            "runner: super::super::v2_runner::ProductionLifecyclePreActivationRunnerBorrowV1",
            "self.launched.initialize_recovered_local_proposal(runner)",
        ),
    )

    proposal_activation_blocker = item(
        "launch", "lifecycle_activation_recovery_blocker"
    )
    require_order(
        "launch",
        proposal_activation_blocker,
        "ordinary activation recovery preflight",
        (
            "pending_kura_replay || pending_kura_evidence",
            "ProductionLifecycleActivationErrorV1::PendingKuraApply",
            "else if recovered_local_proposal",
            "ProductionLifecycleActivationErrorV1::LocalProposalReplayUninitialized",
            "None",
        ),
    )
    activation = item("launch", "activate_with")
    require_order(
        "launch",
        activation,
        "ordinary activation rejects incomplete recovered local-Proposal setup",
        (
            "lifecycle_activation_recovery_blocker(",
            "self.pending_kura_apply_replay.is_some()",
            "self.executor.pending_kura_apply_recovery_evidence().is_some()",
            "self.recovered_local_proposal_attempt.is_some()",
            "close_admission_for_restart()",
            "return Err(error)",
            "self.executor.local_proposal_directive()",
            "local_proposal.exactly_matches( self.executor.context().id(), current_directive )",
            "ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch",
            "let clock_activation = ProductionLifecycleLiveClockActivationPermitV1",
            "self.executor.arm_live_clocks(clock_activation, now)",
        ),
    )
    clock_permit_start = sources["launch"].find(
        "pub(in crate::sumeragi) struct ProductionLifecycleLiveClockActivationPermitV1"
    )
    clock_permit_end = sources["launch"].find(
        "/// Move-only authority for refreshing the live Certified-Serve retirement cut.",
        clock_permit_start,
    )
    clock_permit = (
        sources["launch"][clock_permit_start:clock_permit_end]
        if clock_permit_start >= 0 and clock_permit_end > clock_permit_start
        else ""
    )
    for required in (
        "_seal: ProductionLifecycleLiveClockActivationPermitSealV1",
        "struct ProductionLifecycleLiveClockActivationPermitSealV1;",
        "impl Drop for ProductionLifecycleLiveClockActivationPermitSealV1",
        "#[cfg(test)]",
        "pub(in crate::sumeragi) fn for_test() -> Self",
    ):
        if required not in clock_permit:
            errors.append(
                f"{paths['launch']}: ordinary live-clock permit omits {required!r}"
            )
    for forbidden in (
        "derive(Clone)",
        "derive(Copy)",
        "pub _seal:",
        "pub(crate) _seal:",
        "pub(in crate::sumeragi) _seal:",
    ):
        if forbidden in clock_permit:
            errors.append(
                f"{paths['launch']}: ordinary live-clock permit exposes {forbidden!r}"
            )
    clock_arm = item("effects", "arm_live_clocks")
    require_order(
        "effects",
        clock_arm,
        "affine ordinary live-clock arming",
        (
            "_permit: ProductionLifecycleLiveClockActivationPermitV1",
            "if self.pending_tip_recovery.is_some()",
            "return Err(RuntimeClockError::PendingKuraRecovery)",
            "self.runtime.arm_live_clocks(now)",
        ),
    )
    pending_status = item("effects", "pending_kura_activation_status_snapshot")
    require_order(
        "effects",
        pending_status,
        "completed pending-Kura no-clock status snapshot",
        (
            "self.ready_to_finish()",
            "self.lifecycle_live_clocks_are_unarmed()",
            "PendingKuraApplyRecoveryStage::Completed",
            "return Err(AdapterError::PendingKuraActivationNotReady)",
            "self.runtime.pending_kura_activation_status_snapshot()",
        ),
    )
    proposal_behavior = item(
        "startup_test", "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout"
    )
    require_order(
        "startup_test",
        proposal_behavior,
        "production-shaped recovered local-Proposal initialization behavior",
        (
            "RecoveredLifecycleLocalProposalAttemptV1::for_test(",
            "retain_recovered_local_proposal_attempt_for_test(recovered_attempt)",
            "initialize_recovered_local_proposal(setup_runner)",
            "local_proposal_state.already_attempted(directive)",
            ".activate(Instant::now(), activation, local_proposal_state)",
        ),
    )

    _successor_recovery_pending_kura_tail_source_fidelity_errors(
        paths,
        sources,
        errors,
        item,
        require_order,
        reject_tokens,
        require_tokens,
    )
    return errors
