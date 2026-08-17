"""Coordinator-owned Certified-Serve production source-fidelity contracts."""


def _lifecycle_certified_serve_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal the only production Certified-Serve lifecycle corridor.

    The sealed path is selector authentication -> durable coordinator
    admission -> complete Ready census -> exact worker reservation ->
    LedgerV1 settlement -> reply delivery -> acknowledgement.  Its adjacent
    ProducerTurn is claimed only by the serialized proposal runner.  Legacy
    queue journals, barriers, gates, and producer episodes are forbidden.
    """

    base = repo_root / "crates" / "iroha_core" / "src" / "sumeragi"
    relative_paths = {
        "registry": "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs",
        "scheduler": "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs",
        "turn": "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs",
        "projection": "crates/iroha_core/src/sumeragi/v2_lifecycle_projection.rs",
        "worker": "crates/iroha_core/src/sumeragi/v2_worker.rs",
        "body_store": "crates/iroha_core/src/sumeragi/v2_body_store.rs",
        "height": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_height_driver.rs",
        "ordinary": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs",
        "pending": "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs",
        "runner": "crates/iroha_core/src/sumeragi/v2_runner.rs",
        "launch": "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs",
        "scheduler_cases": "crates/iroha_core/src/sumeragi/tests/v2_lifecycle_scheduler_certified_serve_cases.rs",
        "ledger_cases": "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_tests_durable_recovery_02.rs",
        "startup_cases": "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs",
    }
    errors: list[str] = []
    sources: dict[str, str] = {}
    paths: dict[str, Path] = {}
    for role, relative in relative_paths.items():
        path = repo_root / relative
        paths[role] = path
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: lifecycle Certified-Serve {role} source must be a regular file"
            )
            continue
        if role.endswith("_cases"):
            sources[role] = path.read_text(encoding="utf-8")
            continue
        reviewed_path, reviewed_source = _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            f"lifecycle Certified-Serve {role} source",
        )
        paths[role] = reviewed_path
        sources[role] = reviewed_source
    if errors:
        return errors

    production_roles = (
        "registry",
        "scheduler",
        "turn",
        "projection",
        "worker",
        "body_store",
        "height",
        "ordinary",
        "pending",
        "runner",
        "launch",
    )
    production_tokens = rust_code_tokens(
        "\n".join(sources[role] for role in production_roles)
    )
    for retired in (
        "CertifiedServeAdmission",
        "CertifiedServeLifecycleId",
        "CertifiedServeIngressGate",
        "CertifiedServeIngressReservation",
        "CertifiedServeBarrier",
        "CertifiedServeProducerEpisode",
        "ExactServePredecessor",
        "prepare_certified_request",
        "serve_certified_request_on_routes",
        "producer_episode_due",
        "producer_episode_active",
        "serve_barrier",
        "serve_replacements",
        "pending_serve_requests",
        "next_serve_admission_ordinal",
    ):
        observed = production_tokens.count(retired)
        if observed:
            errors.append(
                f"{base}: retired Certified-Serve owner token {retired!r} must be absent; "
                f"found {observed}"
            )

    def item(
        role: str,
        owner: str | None,
        name: str,
        description: str,
        *,
        expected_attributes: tuple[str, ...] = (),
    ) -> RustItem | None:
        if owner is None:
            return _require_rust_item(paths[role], sources[role], name, errors)
        expected_context = (("impl", *rust_code_tokens(owner)),)
        matches = [
            candidate
            for candidate in rust_items(sources[role], name)
            if candidate.brace_context == expected_context
        ]
        if len(matches) != 1:
            errors.append(
                f"{paths[role]}: require exactly one real Rust/Verus function item "
                f"named {owner}::{name}; found {len(matches)}"
            )
            return None
        target = matches[0]
        _require_rust_item_context(
            paths[role],
            target,
            expected_context,
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        return target

    def sequence(
        role: str,
        owner: str | None,
        name: str,
        description: str,
        markers: tuple[str, ...],
        *,
        expected_attributes: tuple[str, ...] = (),
    ) -> None:
        target = item(
            role,
            owner,
            name,
            description,
            expected_attributes=expected_attributes,
        )
        if target is None:
            return
        tokens = rust_code_tokens(target.source)
        cursor = -1
        for marker in markers:
            positions = tuple(
                position
                for position in _token_sequence_positions(tokens, rust_code_tokens(marker))
                if position > cursor
            )
            if not positions:
                errors.append(
                    f"{paths[role]}:{target.line}: {description} must retain ordered "
                    f"marker {marker!r}"
                )
                return
            cursor = positions[0]

    sequence(
        "registry",
        "ConcreteLifecycleWorkRegistry",
        "attest_ready_certified_serve_request",
        "Ready Serve registry attestation",
        (
            "coordinator.fault.is_some() || coordinator.active_lease.is_some()",
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "record.work_class == LifecycleWorkClass::CertifiedServe",
            "record.state == super::LifecycleState::Ready",
            "exactly_matches_certified_serve_request(authenticated)",
            "frozen_predecessors(",
            "serve.matches_record(record, metadata, digest)",
            "ReadyCertifiedServeAttestationV1",
        ),
    )
    sequence(
        "registry",
        "ConcreteLifecycleWorkRegistry",
        "project_claimed_certified_serve_dispatch",
        "claimed Serve registry projection",
        (
            "LifecycleLedgerV1::from_coordinator(coordinator)",
            "coordinator.active_lease.as_ref() != Some(&lease)",
            "attestation.matches_claimed_record(record, ledger, &lease)",
            "exactly_matches_certified_serve_request(&attestation.authenticated)",
            "serve.matches_claimed_record(record, metadata, work.digest, &lease)",
            "ClaimedCertifiedServeDispatchV1",
        ),
    )
    sequence(
        "scheduler",
        "CertifiedServeSchedulerObservationV1",
        "from_live_cuts",
        "typed Serve scheduler observation factory",
        (
            "AuthenticatedSchedulerInputsFactory::new()",
            "capacity.authenticated_predecessor_debt(&factory)",
            "dequeue.selector_debt()",
            "runner.debt()",
        ),
    )
    scheduler_claim = item(
        "scheduler", None, "claim_certified_serve_turn_v1", "complete Ready Serve scheduler claim"
    )
    if scheduler_claim is not None:
        scheduler_tokens = rust_code_tokens(scheduler_claim.source)
        for marker in (
            "exact_ready != coordinator.ready_index",
            "exact_ready.len() != observations.len()",
            "record.work_class != LifecycleWorkClass::CertifiedServe",
            "unmatched.iter().any(Option::is_some)",
            "coordinator.plan_turn(inputs)",
            "lease.work_class() == LifecycleWorkClass::CertifiedServe",
            "coordinator.rollback_unpublished_turn(&lease)",
            "project_claimed_certified_serve_dispatch",
            "coordinator.rollback_unpublished_turn(&rollback)",
        ):
            if not _token_sequence_positions(scheduler_tokens, rust_code_tokens(marker)):
                errors.append(
                    f"{paths['scheduler']}:{scheduler_claim.line}: complete Ready Serve "
                    f"scheduler claim must retain {marker!r}"
                )
        for forbidden in ("#[cfg_attr(not(test), allow(dead_code))]", "TODO"):
            if forbidden in scheduler_claim.source:
                errors.append(
                    f"{paths['scheduler']}:{scheduler_claim.line}: live Serve scheduler "
                    f"claim must not retain stale {forbidden!r}"
                )

    sequence(
        "turn",
        None,
        "prepare_and_dispatch_current_certified_serve",
        "current-height Serve lifecycle transaction",
        (
            "prepare_current_certified_serve_pre_admission(",
            "cut.narrow_to_lifecycle(expected_context)",
            "capture_lifecycle_ingress_selector(lifecycle_cut)",
            "selector.into_locked_certified_serve_dequeue(receiver, &authenticated)",
            "capture_lifecycle_certified_serve_capacity(target)",
            "owner.admit_selected_certified_serve",
            "registry.attest_ready_certified_serve_request",
            "CertifiedServeSchedulerObservationV1::from_live_cuts",
            "claim_certified_serve_turn_v1",
            "dequeue.commit()",
            "LifecycleCertifiedServeTaskV1::from_dequeued",
            "reservation.preflight_lifecycle_certified_serve(&task)",
            "reservation.commit_lifecycle_certified_serve(task)",
        ),
    )
    turn = item(
        "turn", None, "prepare_and_dispatch_current_certified_serve", "current-height Serve lifecycle transaction"
    )
    if turn is not None:
        turn_tokens = rust_code_tokens(turn.source)
        for marker in (
            "AdmissionDecision::StutterTerminal",
            "AdmissionDecision::ReplayTerminal",
            "LifecycleCertifiedServeTaskV1::from_terminal_replay",
            "settle_certified_serve_negative",
            "CertifiedServeTerminal",
            "CapacityPending",
            "RestartRequired",
        ):
            if not _token_sequence_positions(turn_tokens, rust_code_tokens(marker)):
                errors.append(
                    f"{paths['turn']}:{turn.line}: Serve lifecycle transaction must retain "
                    f"branch {marker!r}"
                )

    sequence(
        "worker",
        "LifecycleCertifiedServeTaskV1",
        "from_dequeued_parts",
        "opaque Serve worker task construction",
        (
            "HashOf::new(request) != authenticated.request_hash()",
            "&recipient != &authenticated.request().requester",
            "routes.semantic_target() != &recipient",
            "!ownership.validate_exact()",
            "!ownership.matches_message(inbound.message())",
            "!ownership.matches_semantic_origin(Some(&recipient))",
            "!ownership.matches_reply_routes(Some(routes))",
            "inbound.take_ingress_ownership()",
            "inbound.into_message_sender_and_reply_routes()",
            "authority: Some(authority)",
        ),
    )
    sequence(
        "worker",
        "LifecycleIoCapacityReservation<'_>",
        "preflight_lifecycle_certified_serve",
        "Serve worker exact preflight",
        (
            "task.authority_matches_request()",
            "target.kind() == LifecycleIngressIoTargetKind::CertifiedServe",
            "!state.lifecycle_serves.contains_key(&task.lifecycle_ordinal())",
        ),
    )
    sequence(
        "worker",
        "LifecycleIoCapacityReservation<'_>",
        "commit_lifecycle_certified_serve",
        "Serve worker indexed publication",
        (
            "self.preflight_lifecycle_certified_serve(&task)",
            "state.lifecycle_serves.insert(ordinal, tracked)",
            "V2IoCommand::LifecycleCertifiedServe(task)",
            "self.queue.ready.notify_all()",
            "complete()",
        ),
    )
    sequence(
        "worker",
        "PreparedLifecycleCertifiedServeCompletionV1",
        "settle_deliver_and_acknowledge",
        "Serve completion settlement/delivery/acknowledgement",
        (
            "body_readback.take()",
            "result.task.authority.take()",
            "settle_certified_serve_worker_completed",
            "verify_certified_serve_terminal_replay",
            "services.post_to_peer_on_reply_routes",
            "result.task.recipient.clone()",
            "result.task.reply_routes.clone()",
            "result.task.ingress_ownership.clone()",
            "self.queue.acknowledge_lifecycle_certified_serve",
        ),
    )
    sequence(
        "worker",
        "ProductionV2Services",
        "post_to_peer_on_reply_routes",
        "Serve exact-output route publication",
        (
            "reply_routes.semantic_target() != &peer",
            "!ingress_ownership.validate_exact()",
            "!ingress_ownership.matches_reply_routes(Some(&reply_routes))",
            "begin_fail_stop_operation()",
            "if reply_routes.is_empty()",
            "post_block_message_on_reply_routes_while_guarded",
            "ExactFanoutOwnership::SourceRetained",
            "operation.complete()",
        ),
    )
    sequence(
        "worker",
        "ProductionV2Services",
        "drain_lifecycle_certified_serve_completion",
        "dedicated Serve completion drain",
        (
            "take_lifecycle_certified_serve_completion()",
            "V2IoCompletion::LifecycleCertifiedServe(guarded)",
            "prepare_lifecycle_certified_serve_completion",
        ),
    )
    sequence(
        "body_store",
        "V2BodyStore",
        "read_durable_body_for_certified_serve",
        "store-bound Serve body readback",
        (
            "self.load_canonical_wire(receipt)?",
            "store_identity: self.instance_identity()",
        ),
    )
    sequence(
        "projection",
        "super::ProductionLifecycleOwnerV1",
        "settle_certified_serve_worker_completed",
        "worker Serve terminal publication",
        (
            "self.body_store.is_some()",
            "self.body_store_identity.as_ref()",
            "persist_completed_with_worker_readback",
            "publish_certified_serve_terminal",
        ),
        expected_attributes=("#[cfg(any(not(test), feature = \"bls\"))]",),
    )
    sequence(
        "projection",
        "super::ProductionLifecycleOwnerV1",
        "settle_producer_turn_advanced",
        "adjacent ProducerTurn durable terminalization",
        (
            "prepare_producer_turn_terminal_transition",
            "stage_durable_transaction()",
            "reduce_settle_turn(",
            "publish_producer_turn_terminal_transition",
            "persist_exact_staged_successor(&staged)",
            "self.coordinator = staged",
        ),
    )
    sequence(
        "ordinary",
        None,
        "run_lifecycle_active_height",
        "ordinary runner ProducerTurn handoff",
        (
            "claim_producer_turn_for_local_proposal",
            "schedule_local_proposal(",
            "dispatch_lane_work_effects(",
            "producer_turn_attempt_permit(&mut active_runner)",
            "settle_producer_turn_after_local_proposal",
        ),
    )
    sequence(
        "pending",
        None,
        "run_pending_active_height",
        "pending-Kura Serve/ProducerTurn handoff",
        (
            "settle_certified_serve_completion_for_no_clock_recovery",
            "claim_producer_turn_for_no_clock_recovery",
            "producer_turn_attempt_permit(&mut active_runner)",
            "settle_producer_turn_after_no_clock_recovery",
        ),
    )
    sequence(
        "height",
        None,
        "drain_lifecycle_v2_ingress",
        "height-runner Serve completion yield",
        (
            "drive_completion_turn(current_turn, lane_work)",
            "completion_selection_stops_batch(&selected)",
            "return Ok(())",
            "ingress_restart_error(&output_guard)",
        ),
    )
    sequence(
        "launch",
        "ProductionLeaderWireIngressBindingV1",
        "bind",
        "leader-wire-only lifecycle ingress binding",
        (
            "ingress.bind_leader_wire_lifecycle_gate(",
            "ingress.close()",
            "gate: Some(gate)",
        ),
    )
    sequence(
        "launch",
        "ProductionLeaderWireIngressBindingV1",
        "retire",
        "leader-wire-only lifecycle ingress retirement",
        (
            "self.gate.take()",
            "self.ingress.close()",
            "self.ingress.unbind_leader_wire_lifecycle_gate(&gate)",
        ),
    )
    sequence(
        "launch",
        "ProductionLifecycleOwnerV1",
        "launch",
        "leader-wire-only lifecycle launch transfer",
        (
            "leader_wire_launch.open_gate(",
            "leader_wire_restore.scheduler_ordinal_high_watermark()",
            "ProductionLeaderWireIngressBindingV1::bind(",
            "ProductionV2Services::start_with_apply_service(",
            "leader_wire_ingress_binding,",
        ),
        expected_attributes=(
            "#[allow(clippy::result_large_err)]",
            "#[inline(never)]",
        ),
    )

    seal_specs = (
        ("registry", "ConcreteLifecycleWorkRegistry", "attest_ready_certified_serve_request"),
        ("registry", "ConcreteLifecycleWorkRegistry", "project_claimed_certified_serve_dispatch"),
        ("scheduler", "CertifiedServeSchedulerObservationV1", "from_live_cuts"),
        ("scheduler", None, "claim_certified_serve_turn_v1"),
        ("turn", None, "prepare_and_dispatch_current_certified_serve"),
        ("worker", "LifecycleCertifiedServeTaskV1", "from_dequeued_parts"),
        ("worker", "LifecycleIoCapacityReservation<'_>", "preflight_lifecycle_certified_serve"),
        ("worker", "LifecycleIoCapacityReservation<'_>", "commit_lifecycle_certified_serve"),
        ("worker", "PreparedLifecycleCertifiedServeCompletionV1", "settle_deliver_and_acknowledge"),
        ("worker", "ProductionV2Services", "post_to_peer_on_reply_routes"),
        ("worker", "ProductionV2Services", "drain_lifecycle_certified_serve_completion"),
        ("body_store", "V2BodyStore", "read_durable_body_for_certified_serve"),
        ("projection", "super::ProductionLifecycleOwnerV1", "settle_certified_serve_worker_completed"),
        ("projection", "super::ProductionLifecycleOwnerV1", "settle_producer_turn_advanced"),
        ("ordinary", None, "run_lifecycle_active_height"),
        ("pending", None, "run_pending_active_height"),
        ("height", None, "drain_lifecycle_v2_ingress"),
        ("launch", "ProductionLeaderWireIngressBindingV1", "bind"),
        ("launch", "ProductionLeaderWireIngressBindingV1", "retire"),
        ("launch", "ProductionLifecycleOwnerV1", "launch"),
    )
    observed_seal_keys = {
        f"{role}:{owner + '::' if owner else ''}{name}"
        for role, owner, name in seal_specs
    }
    expected_seal_keys = set(_LIFECYCLE_CERTIFIED_SERVE_ITEM_SHA256)
    if observed_seal_keys != expected_seal_keys:
        errors.append(
            f"{base}: lifecycle Certified-Serve item seal inventory mismatch; "
            f"missing={sorted(observed_seal_keys - expected_seal_keys)!r}, "
            f"orphaned={sorted(expected_seal_keys - observed_seal_keys)!r}"
        )
    for role, owner, name in seal_specs:
        key = f"{role}:{owner + '::' if owner else ''}{name}"
        expected = _LIFECYCLE_CERTIFIED_SERVE_ITEM_SHA256.get(key)
        sealed_attributes = {
            "projection:super::ProductionLifecycleOwnerV1::settle_certified_serve_worker_completed": (
                "#[cfg(any(not(test), feature = \"bls\"))]",
            ),
            "launch:ProductionLifecycleOwnerV1::launch": (
                "#[allow(clippy::result_large_err)]",
                "#[inline(never)]",
            ),
        }.get(key, ())
        sealed = item(
            role,
            owner,
            name,
            f"lifecycle Certified-Serve sealed item {key}",
            expected_attributes=sealed_attributes,
        )
        if expected is not None:
            _require_rust_item_token_sha256(
                paths[role], sealed, expected, f"lifecycle Certified-Serve item {key}", errors
            )

    for role, names in {
        "scheduler_cases": (
            "certified_serve_claim_rolls_back_when_its_exact_carrier_drifted",
            "certified_serve_scheduler_cannot_overtake_its_ready_predecessor",
            "certified_serve_scheduler_creates_exactly_one_live_claim",
        ),
        "ledger_cases": (
            "launched_terminal_owner_settles_exact_worker_body_readback",
            "launched_terminal_owner_rejects_foreign_worker_store_instance",
        ),
        "startup_cases": (
            "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies",
        ),
    }.items():
        for name in names:
            test_item = _require_rust_item(paths[role], sources[role], name, errors)
            expected_attributes = (
                ("#[cfg(feature = \"bls\")]", "#[test]")
                if role == "startup_cases"
                else ("#[test]",)
            )
            _require_rust_item_context(
                paths[role],
                test_item,
                (),
                f"lifecycle Certified-Serve regression {name}",
                errors,
                expected_attributes=expected_attributes,
            )

    return errors


def _serve_ingress_ordinal_production_source_fidelity_errors(
    repo_root: Path,
) -> list[str]:
    """Compatibility entry point for the coordinator-owned Serve contract."""

    return _lifecycle_certified_serve_production_source_fidelity_errors(repo_root)


def _serve_lifecycle_production_source_fidelity_errors(
    repo_root: Path,
) -> list[str]:
    """Compatibility entry point for the coordinator-owned Serve contract."""

    return _lifecycle_certified_serve_production_source_fidelity_errors(repo_root)
