# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _successor_production_source_fidelity_errors(repo_root: Path) -> list[str]:
    """Bind indexed successor and exact-recovery actions to production order."""
    errors: list[str] = []
    def load(relative: str) -> tuple[Path, str]:
        return _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "production successor-refinement source",
        )
    def region(
        path: Path,
        source: str,
        label: str,
        start_marker: str,
        end_marker: str,
    ) -> str:
        start = source.find(start_marker)
        end = source.find(end_marker, start + len(start_marker)) if start >= 0 else -1
        if start < 0 or end < 0:
            errors.append(f"{path}: missing exact production region {label}")
            return ""
        return source[start:end]
    def require_tokens(path: Path, label: str, body: str, tokens: tuple[str, ...]) -> None:
        body_tokens = rust_code_tokens(body)
        missing = [
            token
            for token in tokens
            if _token_sequence_count(body_tokens, rust_code_tokens(token)) == 0
        ]
        if missing:
            errors.append(
                f"{path}: {label} omits production refinement tokens {missing}"
            )
    def require_literals(path: Path, label: str, body: str, literals: tuple[str, ...]) -> None:
        executable = mask_rust_comments(body)
        invalid = [literal for literal in literals if executable.count(literal) != 1]
        if invalid:
            errors.append(
                f"{path}: {label} must retain each executable literal exactly once {invalid}"
            )
    def require_token_count(
        path: Path,
        label: str,
        body: str,
        token: str,
        expected: int,
    ) -> None:
        observed = _token_sequence_count(
            rust_code_tokens(body), rust_code_tokens(token)
        )
        if observed != expected:
            errors.append(
                f"{path}: {label} must contain {token!r} exactly {expected} "
                f"time(s); found {observed}"
            )
    def require_literal_count(
        path: Path,
        label: str,
        body: str,
        literal: str,
        expected: int,
    ) -> None:
        observed = mask_rust_comments(body).count(literal)
        if observed != expected:
            errors.append(
                f"{path}: {label} must contain exact production literal "
                f"{literal!r} exactly {expected} time(s); found {observed}"
            )

    def require_order(
        path: Path,
        label: str,
        body: str,
        markers: tuple[str, ...],
    ) -> None:
        body_tokens = rust_code_tokens(body)
        cursor = 0
        for marker in markers:
            marker_tokens = rust_code_tokens(marker)
            position = next(
                (
                    index
                    for index in range(
                        cursor,
                        len(body_tokens) - len(marker_tokens) + 1,
                    )
                    if body_tokens[index : index + len(marker_tokens)] == marker_tokens
                ),
                -1,
            )
            if position < 0:
                errors.append(
                    f"{path}: {label} must preserve exact production order {markers}"
                )
                return
            cursor = position + len(marker_tokens)
    def reject_tokens(
        path: Path,
        label: str,
        body: str,
        forbidden: tuple[str, ...],
    ) -> None:
        body_tokens = rust_code_tokens(body)
        observed = tuple(
            token
            for token in forbidden
            if _token_sequence_count(body_tokens, rust_code_tokens(token))
        )
        if observed:
            errors.append(
                f"{path}: {label} must use the opaque checked-transition gate; "
                f"found obsolete direct-kernel forms {observed}"
            )
    runner_path, runner_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner.rs"
    )
    if runner_source:
        for item_name, expected_sha256 in (
            _PRODUCTION_RECOVERY_EAGER_BLOCK_SYNC_ITEM_SHA256.items()
        ):
            item = _require_rust_item(
                runner_path,
                runner_source,
                item_name,
                errors,
            )
            _require_rust_item_context(
                runner_path,
                item,
                (),
                f"recovery-scoped eager block-sync {item_name} production item",
                errors,
            )
            _require_rust_item_token_sha256(
                runner_path,
                item,
                expected_sha256,
                f"recovery-scoped eager block-sync {item_name}",
                errors,
            )
        run_inner_item = _require_rust_item(
            runner_path,
            runner_source,
            "run_inner",
            errors,
        )
        _require_rust_item_context(
            runner_path,
            run_inner_item,
            (),
            "recovery-scoped eager block-sync run_inner production item",
            errors,
            expected_attributes=("#[allow(clippy::too_many_lines)]",),
        )
        _require_rust_token_sequence(
            runner_path,
            run_inner_item,
            """
let mut pending_kura_apply = recovered.pending_kura_apply();
let (
    mut verified_context,
    context_store,
    mut signature_policy,
    _lifecycle_storage_authority,
    _authenticated_genesis,
    recovered_successor_activation,
    mut staged_genesis_nexus_amx_context,
) = recovered.into_parts();
""",
            "durable recovered ownership must retain the recovered successor owners",
            errors,
        )
        _require_rust_token_sequence(
            runner_path,
            run_inner_item,
            """
let mut eager_block_sync =
    recovered_successor_activation.is_some() || pending_kura_apply.is_some();
""",
            "durable recovered ownership must initialize eager block-sync",
            errors,
        )
        if run_inner_item is not None:
            require_order(
                runner_path,
                "durable recovered ownership eager block-sync initialization",
                run_inner_item.source,
                (
                    "let mut pending_kura_apply = recovered.pending_kura_apply();",
                    ") = recovered.into_parts();",
                    "let mut eager_block_sync =",
                    "recovered_successor_activation.is_some() || pending_kura_apply.is_some();",
                ),
            )
        _require_rust_token_sequence(
            runner_path,
            run_inner_item,
            """
let mut next_block_sync_attempt = initial_block_sync_deadline(
    height_started_at, round_timeout, eager_block_sync
);
""",
            "height startup must derive its first block-sync deadline from the recovery hint",
            errors,
        )
        _require_rust_token_sequence(
            runner_path,
            run_inner_item,
            """
let discovery_was_outstanding = block_sync_request.is_some();
drain_v2_ingress(
    &block_rx,
    &mut executor,
    &mut services,
    &mut lane_work,
    output_guard.as_ref(),
    kura.as_ref(),
    &common_config.key_pair,
    block_sync_server
        .as_mut()
        .expect("block-sync server initialized before ingress"),
    &mut block_sync,
    &mut block_sync_request,
    &mut npos_vrf,
    V2IngressDrainMode::Ordinary,
    body_queue_capacity,
)?;
if discovery_was_outstanding && block_sync_request.is_none() {
    admitted_discovered_commit_qc = true;
}
""",
            "only authenticated discovered CommitQC admission/coalescing with "
            "serialized reducer ownership may turn an outstanding request from "
            "Some to None and retain eager block-sync",
            errors,
        )
        _require_rust_token_sequence(
            runner_path,
            run_inner_item,
            """
let (receipt, artifact, lane_work, mut finalized_services) = finality;
eager_block_sync =
    retain_eager_block_sync(recovering_interrupted_tip, admitted_discovered_commit_qc);
let predecessor = DurableV2PredecessorIdentity::authenticate(&artifact, &receipt)?;
""",
            "successor startup must carry interrupted-tip or admitted discovered "
            "CommitQC recovery and clear ordinary live finality",
            errors,
        )
        construction = region(
            runner_path,
            runner_source,
            "PendingSuccessorConstruction",
            "impl PendingSuccessorConstruction {",
            "/// One-shot ownership of an authenticated successor's activation handoff.",
        )
        require_tokens(
            runner_path,
            "PendingSuccessorConstruction",
            construction,
            (
                "super::status::begin_v2_successor_activation(predecessor)?;",
                "expected_predecessor: self.predecessor.refinement_projection(),",
                "authority_predecessor: authority.predecessor().refinement_projection(),",
                "successor_context_id: super::v2_recovery::successor_context_refinement_projection( authority.successor_context_id(), ),",
                "if !production_successor_predecessor_binding_kernel(binding)",
                "PendingSuccessorActivation::Applied { expected_predecessor: self.predecessor, authority, }",
            ),
        )
        require_order(
            runner_path,
            "PendingSuccessorConstruction",
            construction,
            (
                "begin_v2_successor_activation(predecessor)",
                "ProductionSuccessorPredecessorBindingProjection",
                "production_successor_predecessor_binding_kernel(binding)",
                "PendingSuccessorActivation::Applied",
            ),
        )
        activation = region(
            runner_path,
            runner_source,
            "PendingSuccessorActivation",
            "impl PendingSuccessorActivation {",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]",
        )
        recovered_activation = _require_qualified_rust_item(
            runner_path,
            runner_source,
            "PendingSuccessorActivation",
            "recovered",
            errors,
            "recovered successor activation",
        )
        require_tokens(
            runner_path,
            "PendingSuccessorActivation",
            activation,
            (
                "RecoveredSuccessorActivationAuthority::CompleteTip(authority)",
                "RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority)",
                "let published_height = super::status::v2_status().map_or(0, |status| status.height);",
                "stage_before: SUCCESSOR_STAGE_NONE, stage_after: SUCCESSOR_STAGE_NONE, published_height_before: published_height, published_height_after: published_height, restart_required_before: false, restart_required_after: false,",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return Err(V2RunnerError::SuccessorRefinementRejected);",
                "let _authorized_lifecycle = checked_lifecycle.into_projection();",
                "super::status::activate_v2_successor_height( expected_predecessor, authority, successor, )?;",
                "authority.into_canonical_predecessor_storage(local_signer)?",
                ".retire()?",
                "Self::RecoveredCompleteTip { authority: retired }",
                "authority.authorizes_retained_successor()",
                "authority.authorizes_successor_status(successor)",
                "V2RunnerError::CompleteTipSuccessorAuthorityInvalid",
                "predecessor: authority.predecessor()",
                "super::status::activate_recovered_complete_tip_v2_height(authority, successor)?;",
                "super::status::activate_snapshot_bootstrap_v2_height(authority, successor)?;",
            ),
        )
        require_order(
            runner_path,
            "PendingSuccessorActivation::recovered",
            recovered_activation.source if recovered_activation is not None else "",
            (
                "match &authority",
                "let published_height = super::status::v2_status()",
                "ProductionSuccessorStartupLifecycleProjection",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return Err(V2RunnerError::SuccessorRefinementRejected)",
                "let _authorized_lifecycle = checked_lifecycle.into_projection()",
                "Ok(match authority",
                "RecoveredSuccessorActivationAuthority::CompleteTip(authority)",
                "let expected_predecessor = authority.predecessor()",
                "into_canonical_predecessor_storage(local_signer)",
                ".retire()",
                "retired.predecessor() != expected_predecessor",
                "Self::RecoveredCompleteTip { authority: retired }",
                "RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority)",
                "Self::SnapshotBootstrap { authority }",
            ),
        )
        reject_tokens(
            runner_path,
            "PendingSuccessorActivation::recovered",
            recovered_activation.source if recovered_activation is not None else "",
            (
                "production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(",
                "into_activation_after_predecessor_retirement",
                "activate_recovered_v2_successor_height(",
            ),
        )
        open_ingress = region(
            runner_path,
            runner_source,
            "open_ingress_for_active_height",
            "fn open_ingress_for_active_height(",
            "\nfn ingress_capacity_error(",
        )
        require_order(
            runner_path,
            "open_ingress_for_active_height",
            open_ingress,
            (
                "output_guard.begin_fail_stop_operation()",
                "activation.preflight_ingress_open(successor)?",
                "block_ingress.open()",
                "activation.publish(successor)",
                "close_ingress_for_rollover(ingress_ready, block_ingress)",
                "ingress_ready.store(true, Ordering::Release)",
                "ingress_activation.complete()",
            ),
        )
        run_inner = run_inner_item.source if run_inner_item is not None else ""
        require_tokens(
            runner_path,
            "run_inner recovery ownership",
            run_inner,
            (
                "PendingSuccessorActivation::recovered(authority, &common_config.key_pair)",
                "activation.preflight_recovered_startup()?",
                "guard.complete()",
            ),
        )
        require_order(
            runner_path,
            "run_inner CompleteTip restart authority preflight",
            run_inner,
            (
                "let recovered_activation_guard = recovered_successor_activation",
                "PendingSuccessorActivation::recovered(authority, &common_config.key_pair)",
                "activation.preflight_recovered_startup()?",
                "guard.complete()",
                "SumeragiV2Adapter::open_deferred_status_with_capacity_geometry(",
            ),
        )
        require_order(
            runner_path,
            "run_inner live successor startup",
            run_inner,
            (
                "SumeragiV2Adapter::open_deferred_status_with_capacity_geometry(",
                "SerializedV2Runtime::new_with_lifecycle_ordinals(",
                "V2EffectExecutor::open_with_body_store(",
                "ProductionV2Services::start(",
                "executor.consume_effects(std::mem::take(&mut startup_effects), &mut services)?",
                "executor.arm_live_clocks(height_started_at)?",
                "successor_activation_status_snapshot()",
                "open_ingress_for_active_height(",
            ),
        )
        require_order(
            runner_path,
            "run_inner applied successor handoff",
            run_inner,
            (
                "DurableV2PredecessorIdentity::authenticate(&artifact, &receipt)?",
                "PendingSuccessorConstruction::begin(predecessor)?",
                "build_verified_successor(",
                "let (next_verified_context, successor_authority) = successor.into_parts()",
                "activation.bind(successor_authority)?",
            ),
        )
        historical_ingress = region(
            runner_path,
            runner_source,
            "drain_v2_ingress",
            "fn drain_v2_ingress(",
            "\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\nenum OuterIngressTurn",
        )
        require_tokens(
            runner_path,
            "historical ingress routing",
            historical_ingress,
            (
                "block_sync_server.serve_historical_body( kura, request, &sender, local_key )",
                "executor.accept_certified_body_response_with_ingress_ownership( response, &sender, &ingress_ownership, services, )",
                "block_sync.authenticate_response(response, &sender)",
                "block_sync.enqueue_and_complete(discovered, |message| { executor.enqueue_discovered_commit_certificate(message, ingress_ownership) })",
            ),
        )
        require_token_count(
            runner_path,
            "historical ingress routing omits production refinement tokens when either reviewed route changes",
            historical_ingress,
            "block_sync_server.serve_historical_body(kura, request, &sender, local_key)",
            2,
        )
    status_path, status_source = load(
        "crates/iroha_core/src/sumeragi/status.rs"
    )
    first_release_path, first_release_source = load(
        "crates/iroha_core/src/sumeragi/v2_first_release_recovery.rs"
    )
    if status_source:
        begin = region(
            status_path,
            status_source,
            "begin_v2_successor_activation",
            "pub(crate) fn begin_v2_successor_activation(",
            "\nfn validate_v2_successor_snapshot(",
        )
        require_tokens(
            status_path,
            "begin_v2_successor_activation",
            begin,
            (
                "let height = predecessor.height();",
                "validate_v2_predecessor_status(&status, height, SumeragiV2LocalWorkStage::Queued)?;",
                "stage_before: successor_stage_projection(status.liveness.work.successor_height), stage_after: SUCCESSOR_STAGE_RUNNING, published_height_before: status.height, published_height_after: status.height, restart_required_before: status.restart_required, restart_required_after: status.restart_required,",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return Err(V2SuccessorActivationError::RefinementRejected);",
                "let _authorized_lifecycle = checked_lifecycle.into_projection();",
                "update_v2_successor_work_stage_at( height, SumeragiV2LocalWorkStage::Queued, SumeragiV2LocalWorkStage::Running, Instant::now(), )",
            ),
        )
        require_order(
            status_path,
            "begin_v2_successor_activation",
            begin,
            (
                "validate_v2_predecessor_status(",
                "ProductionSuccessorStartupLifecycleProjection",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return Err(V2SuccessorActivationError::RefinementRejected)",
                "let _authorized_lifecycle = checked_lifecycle.into_projection()",
                "update_v2_successor_work_stage_at(",
            ),
        )
        reject_tokens(
            status_path,
            "begin_v2_successor_activation",
            begin,
            (
                "production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(",
            ),
        )
        validate = region(
            status_path,
            status_source,
            "validate_v2_successor_snapshot",
            "fn validate_v2_successor_snapshot(",
            "\nfn activate_v2_successor_height_at(",
        )
        require_tokens(
            status_path,
            "validate_v2_successor_snapshot",
            validate,
            (
                "finalized_height.checked_add(1)",
                "successor.last_committed_height != finalized_height",
                "successor.height_context_id != expected_successor_context_id",
                "marker.round.context_id == successor.height_context_id",
                "marker.transition == SumeragiV2ProgressTransition::SuccessorHeightActivated",
                "marker.age_ms == 0",
            ),
        )
        applied = region(
            status_path,
            status_source,
            "activate_v2_successor_height_at",
            "fn activate_v2_successor_height_at(",
            "\nfn publish_recovered_v2_successor_height_at(",
        )
        require_tokens(
            status_path,
            "activate_v2_successor_height_at",
            applied,
            (
                "let (authority_predecessor, expected_successor_context_id) = authority.into_parts();",
                "validate_v2_predecessor_status( &predecessor_status, finalized_height, SumeragiV2LocalWorkStage::Running, )?;",
                "expected_predecessor: expected_predecessor.refinement_projection(), authority_predecessor: authority_predecessor.refinement_projection(),",
                "predecessor_status_height: predecessor_status.height, predecessor_stage_before: successor_stage_projection( predecessor_status.liveness.work.successor_height, ), predecessor_stage_after: SUCCESSOR_STAGE_COMPLETE,",
                "let Some(checked_trace) = check_production_applied_successor_transition(trace) else",
                "return Err(V2SuccessorActivationError::RefinementRejected);",
                "let _authorized_trace = checked_trace.into_projection();",
                "update_v2_successor_work_stage_at( finalized_height, SumeragiV2LocalWorkStage::Running, SumeragiV2LocalWorkStage::Complete, now, )?;",
            ),
        )
        require_order(
            status_path,
            "activate_v2_successor_height_at",
            applied,
            (
                "authority.into_parts()",
                "validate_v2_successor_snapshot(",
                "validate_v2_predecessor_status(",
                "ProductionAppliedSuccessorTraceProjection",
                "let Some(checked_trace) = check_production_applied_successor_transition(trace) else",
                "return Err(V2SuccessorActivationError::RefinementRejected)",
                "let _authorized_trace = checked_trace.into_projection()",
                "update_v2_successor_work_stage_at(",
                "set_v2_status_at(successor, now)",
            ),
        )
        reject_tokens(
            status_path,
            "activate_v2_successor_height_at",
            applied,
            (
                "production_applied_successor_trace_refines_indexed_activation_kernel(",
            ),
        )
        recovered = region(
            status_path,
            status_source,
            "publish_recovered_v2_successor_height_at",
            "fn publish_recovered_v2_successor_height_at(",
            "\n/// Publish the exact one-shot boundary",
        )
        require_tokens(
            status_path,
            "publish_recovered_v2_successor_height_at",
            recovered,
            (
                "published_status_height_before: published.as_ref().map_or(0, |status| status.height),",
                "let Some(checked_trace) = check_production_recovered_successor_transition(trace) else",
                "return Err(V2SuccessorActivationError::RefinementRejected);",
                "let _authorized_trace = checked_trace.into_projection();",
                "if let Some(published) = published",
                "set_v2_status_at(successor, now);",
            ),
        )
        require_order(
            status_path,
            "publish_recovered_v2_successor_height_at",
            recovered,
            (
                "validate_v2_successor_snapshot(",
                "let published = SUMERAGI_V2_STATUS",
                "ProductionRecoveredSuccessorTraceProjection",
                "let Some(checked_trace) = check_production_recovered_successor_transition(trace) else",
                "if let Some(published) = published",
                "return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(",
                "return Err(V2SuccessorActivationError::RefinementRejected)",
                "let _authorized_trace = checked_trace.into_projection()",
                "if let Some(published)",
                "return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished(",
                "set_v2_status_at(successor, now)",
            ),
        )
        reject_tokens(
            status_path,
            "publish_recovered_v2_successor_height_at",
            recovered,
            (
                "production_recovered_successor_trace_refines_indexed_activation_kernel(",
            ),
        )
        if "update_v2_successor_work_stage_at(" in recovered:
            errors.append(
                f"{status_path}: recovered successor publication may not fabricate "
                "physical predecessor completion"
            )
        snapshot_activation = region(
            status_path,
            status_source,
            "activate_snapshot_bootstrap_v2_height_at",
            "fn activate_snapshot_bootstrap_v2_height_at(",
            "\n/// Publish the authenticated first executable height",
        )
        require_tokens(
            status_path,
            "activate_snapshot_bootstrap_v2_height_at",
            snapshot_activation,
            (
                "authority.into_parts()",
                "SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP",
                "ProductionDurablePredecessorIdentityProjection::default()",
                "snapshot_record_refinement_projection(snapshot_record_hash)",
                "successor_block_refinement_projection(snapshot_block_hash)",
            ),
        )
        require_token_count(
            status_path,
            "activate_snapshot_bootstrap_v2_height_at",
            snapshot_activation,
            "publish_recovered_v2_successor_height_at(",
            1,
        )
        require_token_count(
            status_path,
            "typed recovered status publishers",
            status_source,
            "publish_recovered_v2_successor_height_at(",
            3,
        )
        complete_tip_activation = region(
            status_path,
            status_source,
            "activate_recovered_complete_tip_v2_height",
            "fn activate_recovered_complete_tip_v2_height_at(",
            "\nfn activate_snapshot_bootstrap_v2_height_at(",
        )
        require_tokens(
            status_path,
            "activate_recovered_complete_tip_v2_height",
            complete_tip_activation,
            (
                "authority.authorizes_successor_status(&successor)",
                "V2SuccessorActivationError::RecoveredCompleteTipAuthorityMismatch",
                "let predecessor = authority.predecessor().refinement_projection();",
                "let expected_successor_context_id = successor.height_context_id;",
                "SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP",
                "CanonicalIdentityProjection::zero()",
                "drop(authority);",
                "pub(crate) fn activate_recovered_complete_tip_v2_height(",
                "activate_recovered_complete_tip_v2_height_at(authority, successor, Instant::now())",
            ),
        )
        require_order(
            status_path,
            "activate_recovered_complete_tip_v2_height",
            complete_tip_activation,
            (
                "authority.authorizes_successor_status(&successor)",
                "authority.predecessor().refinement_projection()",
                "publish_recovered_v2_successor_height_at(",
                "SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP",
                "drop(authority)",
            ),
        )
        reject_tokens(
            status_path,
            "activate_recovered_complete_tip_v2_height",
            complete_tip_activation,
            (
                "authority.into_parts()",
                "production_recovered_successor_trace_refines_indexed_activation_kernel(",
            ),
        )
        snapshot_public = region(
            status_path,
            status_source,
            "activate_snapshot_bootstrap_v2_height",
            "pub(crate) fn activate_snapshot_bootstrap_v2_height(",
            "\n/// Register the live bounded transport-to-runner ingress",
        )
        require_tokens(
            status_path,
            "activate_snapshot_bootstrap_v2_height",
            snapshot_public,
            (
                "activate_snapshot_bootstrap_v2_height_at(authority, successor, Instant::now())",
            ),
        )
        restart = region(
            status_path,
            status_source,
            "mark_v2_restart_required",
            "pub(crate) fn mark_v2_restart_required()",
            "\n/// Clear protocol-v2 status during shutdown and isolated tests.",
        )
        require_tokens(
            status_path,
            "mark_v2_restart_required",
            restart,
            (
                "stage_before: successor_stage_projection(status.liveness.work.successor_height), stage_after: successor_stage_projection(status.liveness.work.successor_height), published_height_before: status.height, published_height_after: status.height, restart_required_before: status.restart_required, restart_required_after: true,",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return;",
                "let _authorized_lifecycle = checked_lifecycle.into_projection();",
                "status.restart_required = true;",
            ),
        )
        require_token_count(
            status_path,
            "mark_v2_restart_required",
            restart,
            "return;",
            2,
        )
        require_order(
            status_path,
            "mark_v2_restart_required",
            restart,
            (
                "ProductionSuccessorStartupLifecycleProjection",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return",
                "let _authorized_lifecycle = checked_lifecycle.into_projection()",
                "status.restart_required = true",
            ),
        )
        reject_tokens(
            status_path,
            "mark_v2_restart_required",
            restart,
            (
                "production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(",
            ),
        )
        require_tokens(
            status_path,
            "CompleteTip retirement release wrapper",
            status_source,
            (
                "fn complete_tip_retirement_and_successor_owner_bind_are_release_bound()",
                "crate::sumeragi::v2_first_release_recovery::run_complete_tip_retirement_release_regressions(",
            ),
        )
    if first_release_source:
        require_tokens(
            first_release_path,
            "CompleteTip first-release recovery seam",
            first_release_source,
            (
                "pub(crate) use super::v2_lifecycle_coordinator::{",
                "run_complete_tip_retirement_release_regressions",
            ),
        )
    sumeragi_path, sumeragi_source = load(
        "crates/iroha_core/src/sumeragi/mod.rs"
    )
    if sumeragi_source:
        genesis_runner_bundle = region(
            sumeragi_path,
            sumeragi_source,
            "move-only genesis runner bundle",
            "/// Bundle of genesis block and its publishing key.",
            "\n/// Authenticated lane-local traffic accepted alongside global v2 consensus.",
        )
        require_tokens(
            sumeragi_path,
            "move-only genesis runner bundle",
            genesis_runner_bundle,
            ("v2_bootstrap: Option<GenesisV2Bootstrap>",),
        )
        reject_tokens(
            sumeragi_path,
            "move-only genesis runner bundle",
            genesis_runner_bundle,
            ("Clone", "fn clone("),
        )
        reject_tokens(
            sumeragi_path,
            "move-only genesis runner bundle",
            sumeragi_source,
            ("impl Clone for GenesisWithPubKey",),
        )
    genesis_context_path, genesis_context_source = load(
        "crates/iroha_core/src/sumeragi/v2_context.rs"
    )
    if genesis_context_source:
        genesis_bootstrap = region(
            genesis_context_path,
            genesis_context_source,
            "move-only authenticated genesis bootstrap",
            "/// Verified height-one inputs retained until the production reducer opens its",
            "\n/// Non-forgeable proof that one Nexus/AMX projection",
        )
        require_order(
            genesis_context_path,
            "move-only authenticated genesis bootstrap",
            genesis_bootstrap,
            (
                "authenticated_genesis: AuthenticatedGenesisBodyV1",
                "struct AuthenticatedGenesisBodyV1",
                "signed_block: SignedBlock",
                "authority: iroha_crypto::PublicKey",
                "fn signed_block(&self) -> &SignedBlock",
                "fn authorizes(&self, authority: &iroha_crypto::PublicKey) -> bool",
            ),
        )
        freeze_staged_genesis = _require_rust_item(
            genesis_context_path,
            genesis_context_source,
            "freeze_staged_genesis_v2",
            errors,
        )
        if freeze_staged_genesis is not None:
            require_tokens(
                genesis_context_path,
                "signed genesis bootstrap seal mint",
                freeze_staged_genesis.source,
                (
                    "AuthenticatedGenesisBodyV1::authenticate(genesis)?",
                    "authenticated_genesis,",
                ),
            )
        genesis_authenticate = _require_qualified_rust_item(
            genesis_context_path,
            genesis_context_source,
            "AuthenticatedGenesisBodyV1",
            "authenticate",
            errors,
            "authenticated genesis body mint",
        )
        require_tokens(
            genesis_context_path,
            "signed genesis bootstrap seal mint",
            genesis_authenticate.source if genesis_authenticate is not None else "",
            (
                "let mut transactions = genesis.0.external_transactions()",
                "try_signatory()",
                "signed_block: genesis.0.clone()",
                "authority",
            ),
        )
        require_tokens(
            genesis_context_path,
            "signed genesis bootstrap seal retention",
            genesis_bootstrap,
            ("signed_block: genesis.0.clone()",),
        )
        genesis_bootstrap_owner = region(
            genesis_context_path,
            genesis_context_source,
            "move-only authenticated genesis owner extraction",
            "impl GenesisV2Bootstrap {",
            "\n/// Extract the only voting roster source accepted at fresh genesis:",
        )
        require_order(
            genesis_context_path,
            "signed genesis bootstrap seal retention",
            genesis_bootstrap_owner,
            (
                "impl GenesisV2Bootstrap",
                "fn into_parts( self, )",
                "self.verified_context",
                "self.staged_nexus_amx_context",
                "self.authenticated_genesis",
            ),
        )
        genesis_parts = _require_qualified_rust_item(
            genesis_context_path,
            genesis_context_source,
            "GenesisV2Bootstrap",
            "into_parts",
            errors,
            "authenticated genesis bootstrap transfer",
        )
        if genesis_parts is not None:
            require_order(
                genesis_context_path,
                "signed genesis bootstrap seal transfer",
                genesis_parts.source,
                (
                    "self.verified_context",
                    "self.staged_nexus_amx_context",
                    "self.authenticated_genesis",
                ),
            )
        reject_tokens(
            genesis_context_path,
            "move-only authenticated genesis bootstrap",
            genesis_bootstrap,
            (
                "Clone",
            ),
        )
        reject_tokens(
            genesis_context_path,
            "move-only authenticated genesis bootstrap",
            genesis_context_source,
            (
                "impl Clone for GenesisV2Bootstrap",
                "impl Clone for AuthenticatedGenesisBodyV1",
            ),
        )
    recovery_path, recovery_source = load(
        "crates/iroha_core/src/sumeragi/v2_recovery.rs"
    )
    if recovery_source:
        production_recovery_source = recovery_source.split(
            "\n#[cfg(test)]\nmod tests {", 1
        )[0]
        predecessor_authentication = region(
            recovery_path,
            recovery_source,
            "DurableV2PredecessorIdentity::authenticate",
            "pub(crate) fn authenticate(\n        artifact: &wire::finality::V2FinalityArtifact,",
            "\n    /// Lossless primitive identity consumed by the shared production/Verus kernel.",
        )
        require_tokens(
            recovery_path,
            "DurableV2PredecessorIdentity::authenticate",
            predecessor_authentication,
            (
                "height: artifact.height, block_hash: artifact.block_hash, artifact_hash: HashOf::new(artifact),",
                "receipt.height() != identity.height || receipt.block_hash() != identity.block_hash || receipt.context_id() != artifact.context_id() || receipt.subject() != artifact.subject || receipt.certificate() != artifact.commit_qc.as_ref() || receipt.artifact_hash() != identity.artifact_hash",
                "if !production_durable_predecessor_identity_kernel(identity.refinement_projection())",
            ),
        )
        require_order(
            recovery_path,
            "DurableV2PredecessorIdentity::authenticate",
            predecessor_authentication,
            (
                "let identity = Self",
                "receipt.height() != identity.height",
                "production_durable_predecessor_identity_kernel(identity.refinement_projection())",
                "Ok(identity)",
            ),
        )
        complete_tip_authority = region(
            recovery_path,
            recovery_source,
            "RecoveredCompleteTipActivationAuthority",
            "pub(crate) struct RecoveredCompleteTipActivationAuthority {",
            "\n/// Distinct one-shot authority for the first executable height after an audited snapshot.",
        )
        require_tokens(
            recovery_path,
            "RecoveredCompleteTipActivationAuthority canonical lifecycle target",
            complete_tip_authority,
            (
                "verified_predecessor: VerifiedHeightContext",
                "predecessor_signature_policy: BlockSignaturePolicy",
                "lifecycle_storage: CanonicalCompleteTipLifecycleStorageV1",
                "struct CanonicalLifecycleHeightStorageV1",
                "kura.sumeragi_v2_storage_root().join(\"lifecycle-v1\").join(hex::encode(context_id.0.as_ref()))",
                "CanonicalCompleteTipLifecycleStorageV1::from_kura( kura, artifact.context_id(), artifact.height, verified_successor.context().id(), verified_successor.context().height, )",
                "verified_predecessor.context() != &artifact.height_context",
                "verified_predecessor.proofs_of_possession() != artifact.validator_set_pops.as_slice()",
                "self.lifecycle_storage.predecessor.root == root",
                "self.lifecycle_storage.successor.context_id == self.activation.successor_context_id()",
                "body_store_root: kura.sumeragi_v2_storage_root().join(\"bodies\")",
                "fn authorizes_predecessor_storage_inputs(",
                "self.lifecycle_storage.body_store_root == body_store_root",
                "&self.predecessor_signature_policy == signature_policy",
                "fn into_canonical_predecessor_storage(",
                "fn authorizes_verified_successor(",
                "verified.context().parent_commit_qc.as_ref() == Some(&self.artifact.commit_qc)",
                "verified.verified_predecessor_context() == Some(&self.artifact.height_context)",
                "fn authorizes_successor_body_store(",
            ),
        )
        require_tokens(
            recovery_path,
            "recovered lifecycle storage authority handoff",
            recovery_source,
            (
                "lifecycle_storage_authority: RecoveredLifecycleStorageAuthorityV1",
                "RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(",
                "struct RecoveredLifecycleStorageMintPermitV1",
                "genesis_account: AccountId",
                "fn authorizes(",
                "self.kura_identity.matches(kura)",
                "&self.genesis_account == genesis_account",
                "RecoveredLifecycleStorageMintPermitV1::new(",
                "self.lifecycle_storage_authority",
            ),
        )
        require_token_count(
            recovery_path,
            "recovered lifecycle storage authority handoff",
            production_recovery_source,
            "RecoveredLifecycleStorageAuthorityV1::mint_from_recovered_height(",
            4,
        )
        require_token_count(
            recovery_path,
            "recovered lifecycle storage authority handoff",
            production_recovery_source,
            "RecoveredLifecycleStorageMintPermitV1::new(",
            4,
        )
        require_tokens(
            runner_path,
            "runner retains recovered lifecycle storage authority",
            runner_source,
            ("_lifecycle_storage_authority", "_authenticated_genesis"),
        )
        recover_active_height = _require_rust_item(
            recovery_path,
            production_recovery_source,
            "recover_active_height_with_plan",
            errors,
        )
        if recover_active_height is not None:
            require_tokens(
                recovery_path,
                "recovery-sealed fresh genesis handoff",
                recover_active_height.source,
                (
                    "let (verified_context, staged_genesis_nexus_amx_context, authenticated_genesis) = fresh_genesis.into_parts()",
                    "if !authenticated_genesis.authorizes(&genesis_public_key)",
                    "FreshGenesisAuthorityMismatch",
                    "authenticated_genesis: Some(authenticated_genesis)",
                ),
            )
        require_tokens(
            recovery_path,
            "recovery-sealed fresh genesis owner",
            production_recovery_source,
            (
                "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>",
                "self.authenticated_genesis",
            ),
        )
        ledger_path, ledger_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs"
        )
        if ledger_source:
            open_predecessor_storage = _require_rust_item(
                ledger_path,
                ledger_source,
                "open_complete_tip_predecessor_storage",
                errors,
            )
            _require_rust_item_context(
                ledger_path,
                open_predecessor_storage,
                (),
                "CompleteTip canonical predecessor storage join",
                errors,
                expected_attributes=("#[allow(clippy::too_many_arguments)]",),
            )
            require_order(
                ledger_path,
                "CompleteTip canonical predecessor storage join",
                open_predecessor_storage.source
                if open_predecessor_storage is not None
                else "",
                (
                    "complete_tip.authorizes_predecessor_storage_inputs(",
                    "LifecycleLedgerStoreV1::open(predecessor_root, context)?",
                    "ledger.into_complete_tip_terminal_apply_store_join(ledger_store, complete_tip)?",
                    "CertifiedServePayloadStoreV1::open( predecessor_root, verified_predecessor.context() )?",
                    "recovered.authenticate_for_complete_tip_retirement( &verified_predecessor, local_signer )?",
                    "authenticate_complete_tip_serve_census( &terminal.ledger, &serve_payloads )?",
                    "AuthenticatedCompleteTipPredecessorStorageV1",
                    "cut.is_exact()?",
                    "Ok(cut)",
                ),
            )
            predecessor_store_join = region(
                ledger_path,
                ledger_source,
                "CompleteTip canonical predecessor store join",
                "fn is_authorized_complete_tip_predecessor_target(",
                "\n    /// Compare the complete immutable publication target",
            )
            require_tokens(
                ledger_path,
                "CompleteTip canonical predecessor store join",
                ledger_source,
                (
                    "complete_tip.authorizes_predecessor_lifecycle_root(root)",
                    "self.path == root.join(LEDGER_FILE)",
                    "pub(in crate::sumeragi) fn open_complete_tip_predecessor_storage(",
                    "complete_tip.authorizes_predecessor_storage_inputs(",
                    "CertifiedServePayloadStoreV1::open( predecessor_root, verified_predecessor.context() )?",
                    "recovered.authenticate_for_complete_tip_retirement( &verified_predecessor, local_signer )?",
                    "authenticate_complete_tip_serve_census( &terminal.ledger, &serve_payloads )?",
                    "payload_store.retire_authenticated_cut(serve_payloads, &retained_serve_payloads)?",
                    "reconcile_complete_tip_serve_retirement(",
                    ".stage_complete_tip_all_row_retirement(serve_reconciliation)?",
                    ".persist_exact_successor(&terminal.ledger, &retired)?",
                    "successor.open_initialized_or_descendant(retired.high_water())?",
                    "RetiredRecoveredCompleteTipActivationAuthorityV1",
                    "predecessor_store: LifecycleLedgerStoreV1",
                    "predecessor_ledger: LifecycleLedgerV1",
                    "successor_store: LifecycleLedgerStoreV1",
                    "successor_ledger: LifecycleLedgerV1",
                    "fn bind_successor_owner(",
                    "owner_store.same_publication_target(&self.successor_store)",
                    "LifecycleLedgerV1::from_coordinator(&owner.coordinator)",
                    "authorizes_successor_body_store(body_store, &owner.verified)",
                    "owner.payload_store.matches_lifecycle_storage_root(",
                    "owner.payload_store.validate_authenticated_cut(&owner.serve_payloads)",
                    "authenticated_serve_payloads_match_ledger( &self.successor_ledger, &owner.serve_payloads, )",
                    "adapter_startup.authorizes_verified_context(&owner.verified)",
                    "self.complete_tip.authorizes_successor_kura(owner.kura_binding.as_ref())",
                    "serve_payloads: recovery.into_serve_payloads()",
                ),
            )
            restart_publication = region(
                ledger_path,
                ledger_source,
                "CompleteTip restart publication authority",
                "fn successor_descends_from_retirement(",
                "\n    fn exactly_matches_successor_owner(",
            )
            require_order(
                ledger_path,
                "CompleteTip restart publication authority",
                restart_publication,
                (
                    "self.successor_ledger.context() == self.successor_store.context",
                    "self.successor_ledger.frame_identity() == self.successor_frame_identity",
                    "self.successor_ledger.records.is_empty()",
                    "self.successor_ledger.high_water == self.retained_high_water",
                    "record.ordinal() > self.retained_high_water",
                    "fn authorizes_retained_successor(&self) -> bool",
                    "self.predecessor_ledger.frame_identity() == self.predecessor_frame_identity",
                    ".is_authorized_complete_tip_predecessor_target(&self.complete_tip)",
                    "self.predecessor_store.load().ok().as_ref() == Some(&self.predecessor_ledger)",
                    "self.successor_descends_from_retirement()",
                    "self.complete_tip.authorizes_successor_lifecycle_target(",
                    "self.successor_store.load().ok().as_ref() == Some(&self.successor_ledger)",
                    "fn authorizes_successor_status(",
                    "self.authorizes_retained_successor()",
                    "self.complete_tip.successor_context_id() == successor.height_context_id",
                    ".checked_add(1)",
                    "Some(successor.height)",
                    "successor.last_committed_height == self.complete_tip.predecessor().height()",
                ),
            )
            reject_tokens(
                ledger_path,
                "CompleteTip restart publication authority",
                restart_publication,
                (
                    "#[cfg(test)]",
                    "into_parts",
                    "fn root(",
                    "fn ledger(",
                ),
            )
            successor_owner_bind = region(
                ledger_path,
                ledger_source,
                "CompleteTip exact H+1 owner bind",
                "fn exactly_matches_successor_owner(",
                "\n/// Private Kura-derived target for the empty CompleteTip successor ledger.",
            )
            exact_successor_owner = _require_qualified_rust_item(
                ledger_path,
                ledger_source,
                "RetiredRecoveredCompleteTipActivationAuthorityV1",
                "exactly_matches_successor_owner",
                errors,
                "CompleteTip exact H+1 owner comparison",
            )
            bind_successor_owner = _require_qualified_rust_item(
                ledger_path,
                ledger_source,
                "RetiredRecoveredCompleteTipActivationAuthorityV1",
                "bind_successor_owner",
                errors,
                "CompleteTip exact H+1 owner bind",
                expected_attributes=("#[cfg_attr(not(test), allow(dead_code))]",),
            )
            launch_successor_owner = _require_qualified_rust_item(
                ledger_path,
                ledger_source,
                "BoundRecoveredCompleteTipSuccessorOwnerV1",
                "launch",
                errors,
                "CompleteTip sealed H+1 owner launch",
                expected_attributes=("#[allow(dead_code, clippy::result_large_err)]",),
            )
            require_order(
                ledger_path,
                "CompleteTip exact H+1 owner comparison",
                exact_successor_owner.source
                if exact_successor_owner is not None
                else "",
                (
                    "authorizes_successor_kura(owner.kura_binding.as_ref())",
                    "authorizes_successor_body_store(body_store, &owner.verified)",
                    "validate_authenticated_cut(&owner.serve_payloads)",
                    "authenticated_serve_payloads_match_ledger(",
                    "owner_store.same_publication_target(&self.successor_store)",
                    "LifecycleLedgerV1::from_coordinator(&owner.coordinator)",
                    "exactly_covers_recovered_ready_work(&owner.coordinator)",
                ),
            )
            require_order(
                ledger_path,
                "CompleteTip exact H+1 owner bind",
                bind_successor_owner.source if bind_successor_owner is not None else "",
                (
                    "self.exactly_matches_successor_owner(&mut owner)",
                    "BoundRecoveredCompleteTipSuccessorOwnerV1 { owner, retirement: self, }",
                ),
            )
            require_order(
                ledger_path,
                "CompleteTip sealed H+1 owner launch",
                launch_successor_owner.source
                if launch_successor_owner is not None
                else "",
                (
                    "let Self { owner, retirement } = self",
                    "let launched = owner.launch(inputs)?",
                    "LaunchedRecoveredCompleteTipSuccessorLifecycleV1 { launched, retirement, }",
                ),
            )
            require_order(
                ledger_path,
                "CompleteTip exact H+1 owner bind",
                successor_owner_bind,
                (
                    "fn exactly_matches_successor_owner(",
                    "validate_authenticated_cut(&owner.serve_payloads)",
                    "authenticated_serve_payloads_match_ledger(",
                    "fn bind_successor_owner( self, mut owner: ProductionLifecycleOwnerV1, )",
                    "if !self.exactly_matches_successor_owner(&mut owner)",
                    "BoundRecoveredCompleteTipSuccessorOwnerV1 { owner, retirement: self, }",
                    "struct BoundRecoveredCompleteTipSuccessorOwnerV1 { owner: ProductionLifecycleOwnerV1, retirement: RetiredRecoveredCompleteTipActivationAuthorityV1, }",
                    "impl BoundRecoveredCompleteTipSuccessorOwnerV1",
                    "fn launch( self, inputs: super::launch::ProductionLifecycleLaunchInputsV1, )",
                    "let Self { owner, retirement } = self",
                    "let launched = owner.launch(inputs)?",
                    "LaunchedRecoveredCompleteTipSuccessorLifecycleV1 { launched, retirement, }",
                    "struct LaunchedRecoveredCompleteTipSuccessorLifecycleV1 { launched: super::launch::LaunchedProductionLifecycleV1, retirement: RetiredRecoveredCompleteTipActivationAuthorityV1, }",
                ),
            )
            reject_tokens(
                ledger_path,
                "CompleteTip exact H+1 owner bind",
                successor_owner_bind,
                (
                    "into_parts",
                    "fn into_owner(",
                    "-> ProductionLifecycleOwnerV1",
                    "root(&self)",
                    "ledger(&self)",
                    "fn owner(&self)",
                    "fn retirement(&self)",
                    "fn launched(&self)",
                    "fn into_launched(",
                    "fn into_retirement(",
                ),
            )
            require_token_count(
                ledger_path,
                "CompleteTip exact H+1 bound seal",
                ledger_source,
                "BoundRecoveredCompleteTipSuccessorOwnerV1",
                5,
            )
            require_token_count(
                ledger_path,
                "CompleteTip exact H+1 bound seal",
                ledger_source,
                "impl BoundRecoveredCompleteTipSuccessorOwnerV1",
                2,
            )
            require_token_count(
                ledger_path,
                "CompleteTip exact H+1 launched seal",
                ledger_source,
                "LaunchedRecoveredCompleteTipSuccessorLifecycleV1",
                3,
            )
        adapter_path, adapter_source = load(
            "crates/iroha_core/src/sumeragi/v2.rs"
        )
        apply_path, apply_source = load(
            "crates/iroha_core/src/sumeragi/v2_apply.rs"
        )
        body_store_path, body_store_source = load(
            "crates/iroha_core/src/sumeragi/v2_body_store.rs"
        )
        safety_wal_path, safety_wal_source = load(
            "crates/iroha_core/src/sumeragi/safety_wal.rs"
        )
        adjacent_store_path, adjacent_store_source = load(
            "crates/iroha_core/src/sumeragi/serviced_candidate_store.rs"
        )
        if adapter_source:
            authenticated_startup = region(
                adapter_path,
                adapter_source,
                "authenticated recovered startup projections",
                "pub(crate) fn authenticate_final_wal_startup_authority(",
                "impl AuthenticatedRecoveredAdapterStartup",
            )
            require_order(
                adapter_path,
                "authenticated recovered startup projections",
                authenticated_startup,
                (
                    "authenticate_recovered_wal_frontier()",
                    "recovered_validation_authority(&self.effects)",
                    "authenticate_recovered_wal_vote_sign(&mut self.effects)",
                    "authenticate_recovered_wal_control_sign(&mut self.effects)",
                    "authenticate_recovered_wal_decision_fetch(&mut self.effects)",
                ),
            )
            require_tokens(
                adapter_path,
                "test-only authenticated recovered startup projections",
                adapter_source,
                (
                    "validation_authority: RecoveredValidationAuthority",
                    "#[cfg(test)] const fn recovered_validation_authority(",
                    "#[cfg(test)] fn leader_wire_recovery_authority(",
                    "struct ProductionLeaderWireLaunchAuthorityV1",
                    "fn prepare_leader_wire_launch(",
                ),
            )
            factory_input_binding = _require_qualified_rust_item(
                adapter_path,
                adapter_source,
                "AuthenticatedRecoveredAdapterStartup",
                "bind_production_lifecycle_owner_factory_inputs_v1",
                errors,
                "recovered lifecycle factory-input binding",
                expected_attributes=(
                    "#[allow(clippy::result_large_err, clippy::too_many_arguments)]",
                ),
            )
            require_order(
                adapter_path,
                "recovered lifecycle factory-input binding",
                factory_input_binding.source
                if factory_input_binding is not None
                else "",
                (
                    "storage.kura_identity.matches(kura.as_ref())",
                    "state.matches_kura_instance(&kura)",
                    "state.network_id_ref() != &self.adapter.wire_context.network_id",
                    "let block_cadence = state.sumeragi_block_cadence()",
                    "adapter_owner: Arc::clone(&self.factory_owner)",
                    "storage",
                    "state",
                    "queue",
                    "kura",
                    "block_cadence",
                ),
            )
            canonical_owner_factory = region(
                adapter_path,
                adapter_source,
                "canonical Kura-bound lifecycle-owner factory",
                "pub(in crate::sumeragi) fn open_production_lifecycle_owner_v1(",
                "fn open_production_lifecycle_owner_v1_at_authenticated_roots(",
            )
            owner_factory_item = _require_qualified_rust_item(
                adapter_path,
                adapter_source,
                "AuthenticatedRecoveredAdapterStartup",
                "open_production_lifecycle_owner_v1",
                errors,
                "canonical Kura-bound lifecycle-owner factory",
                expected_attributes=(
                    "#[allow(\n        clippy::result_large_err,\n"
                    "        clippy::too_many_arguments,\n"
                    "        clippy::too_many_lines\n    )]",
                ),
            )
            require_order(
                adapter_path,
                "canonical Kura-bound lifecycle-owner factory",
                owner_factory_item.source if owner_factory_item is not None else "",
                (
                    "factory_inputs: RecoveredLifecycleOwnerFactoryInputsV1",
                    "body_store: super::v2_body_store::QuarantinedV2BodyStore",
                    "if !self.effects.is_empty()",
                    "let RecoveredLifecycleOwnerFactoryInputsV1 { adapter_owner, storage, state, queue, kura, provider_ingest_finalized_archive, reputation_finalized_archive, block_cadence, events_sender, local_signer, } = factory_inputs",
                    "Arc::ptr_eq(&adapter_owner, &self.factory_owner)",
                    "storage.context_id != context.id() || storage.height != context.height",
                    "body_store.matches_lifecycle_storage_root( &storage.body_store_root, &context, &storage.signature_policy, )",
                    "self.adapter.wal.matches_path(&storage.wal_path)",
                    "let apply_service = super::v2_apply::V2ApplyService::new(",
                    "storage.genesis_account.clone()",
                    "apply_service.matches_lifecycle_launch( &state, &kura, &context, &validator_set_pops )",
                    "body_store.into_revalidated_lifecycle_startup( &apply_service, &context, validation_authority )",
                    "let RecoveredLifecycleStorageAuthorityV1 { kura_identity, wal_path, chunk_root, lifecycle_root, .. } = storage",
                    "self.open_production_lifecycle_owner_v1_at_authenticated_roots(",
                    "let kura_binding = RecoveredLifecycleOwnerKuraBindingV1 { kura_identity, wal_path, chunk_root, local_signer: Some(local_signer.public_key().clone()), }",
                    "owner.with_recovered_kura_binding_and_apply_service(kura_binding, apply_service)",
                ),
            )
            reject_tokens(
                adapter_path,
                "canonical Kura-bound lifecycle-owner factory",
                owner_factory_item.source if owner_factory_item is not None else "",
                (
                    "kura: &Kura",
                    "ledger_root: &std::path::Path",
                    "serve_payload_root: &std::path::Path",
                    "body_root: &std::path::Path",
                    "body_signature_policy:",
                    "body_store: super::v2_body_store::V2BodyStore",
                    "body_store: super::v2_body_store::RevalidatedV2BodyStore",
                ),
            )
            storage_mint = _require_qualified_rust_item(
                adapter_path,
                adapter_source,
                "RecoveredLifecycleStorageAuthorityV1",
                "mint_from_recovered_height",
                errors,
                "recovery-minted lifecycle storage authority",
            )
            storage_paths = _require_qualified_rust_item(
                adapter_path,
                adapter_source,
                "RecoveredLifecycleOwnerKuraBindingV1",
                "storage_paths_for_launch",
                errors,
                "recovery-owned lifecycle launch paths",
            )
            require_tokens(
                adapter_path,
                "recovery-minted lifecycle storage authority",
                storage_mint.source if storage_mint is not None else "",
                (
                    "fn mint_from_recovered_height(",
                    "permit: super::v2_recovery::RecoveredLifecycleStorageMintPermitV1",
                    "assert!(permit.authorizes(kura, verified, signature_policy, genesis_account))",
                    "let storage_root = kura.sumeragi_v2_storage_root()",
                    "kura_identity: kura.instance_identity()",
                    "wal_path: storage_root .join(\"wal\") .join(format!(\"{:020}.wal\", context.height))",
                    "chunk_root: storage_root.join(\"chunks\")",
                    "lifecycle_root: storage_root .join(\"lifecycle-v1\") .join(hex::encode(context.id().0.as_ref()))",
                    "body_store_root: storage_root.join(\"bodies\")",
                ),
            )
            require_order(
                adapter_path,
                "recovery-owned lifecycle launch paths",
                storage_paths.source if storage_paths is not None else "",
                (
                    "self.matches_kura(kura)",
                    "RecoveredLifecycleLaunchStoragePathsV1",
                    "wal_path: self.wal_path.clone()",
                    "chunk_root: self.chunk_root.clone()",
                ),
            )
            factory_regression = _require_rust_item(
                adapter_path,
                adapter_source,
                "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout",
                errors,
            )
            require_literals(
                adapter_path,
                "recovery-minted lifecycle storage authority regressions",
                factory_regression.source if factory_regression is not None else "",
                (
                    '"a body store outside the Kura layout must fail closed"',
                    '"a wrong body signature policy must fail closed"',
                ),
            )
            require_tokens(
                adapter_path,
                "recovery-minted lifecycle storage authority",
                adapter_source,
                (
                    "struct RecoveredLifecycleStorageAuthorityV1",
                    "kura_identity: KuraInstanceIdentity",
                    "genesis_account: AccountId",
                    "wal_path: PathBuf",
                    "chunk_root: PathBuf",
                    "struct RecoveredLifecycleOwnerKuraBindingV1 {",
                    "fn matches_identity(&self, identity: &KuraInstanceIdentity) -> bool",
                    "fn storage_paths_for_launch(",
                    "struct RecoveredLifecycleOwnerFactoryInputsV1",
                    "adapter_owner: Arc<AuthenticatedRecoveredAdapterFactoryOwnerV1>",
                    "fn bind_production_lifecycle_owner_factory_inputs_v1(",
                    "permit: super::v2_runner::RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "storage.kura_identity.matches(kura.as_ref())",
                    "state.matches_kura_instance(&kura)",
                    "state.network_id_ref() != &self.adapter.wire_context.network_id",
                    "let block_cadence = state.sumeragi_block_cadence()",
                    "let local_signer = permit.into_local_signer()",
                    "fn mint_from_recovered_height(",
                    "permit: super::v2_recovery::RecoveredLifecycleStorageMintPermitV1",
                    "assert!(permit.authorizes(kura, verified, signature_policy, genesis_account))",
                    "let storage_root = kura.sumeragi_v2_storage_root()",
                    "kura_identity: kura.instance_identity()",
                    "wal_path: storage_root .join(\"wal\") .join(format!(\"{:020}.wal\", context.height))",
                    "chunk_root: storage_root.join(\"chunks\")",
                    "lifecycle_root: storage_root .join(\"lifecycle-v1\") .join(hex::encode(context.id().0.as_ref()))",
                    "body_store_root: storage_root.join(\"bodies\")",
                    "fn production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout()",
                    "fn recovered_lifecycle_factory_inputs_bind_exact_state_kura_and_network()",
                    "fn recovered_lifecycle_factory_inputs_reject_a_same_context_foreign_startup()",
                    "fn production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies()",
                    "fn recovered_wal_sign_status_publication_is_exact_last_and_unwired()",
                    "assert!(context_binding < body_root)",
                    "assert!(body_root < wal_path)",
                    "assert!(wal_path < apply_service)",
                    "assert!(authenticated_roots < kura_binding)",
                ),
            )
            for literal in (
                '"a caller-promoted marker cannot enter production quarantine"',
                '"pre-promoted marker rejection must precede lifecycle-store creation"',
                '"a body store outside the Kura layout must fail closed"',
                '"a wrong body signature policy must fail closed"',
            ):
                require_literal_count(
                    adapter_path,
                    "recovery-minted lifecycle storage authority regressions",
                    adapter_source,
                    literal,
                    1,
                )
            reject_tokens(
                adapter_path,
                "sealed recovered lifecycle factory inputs",
                adapter_source,
                (
                    "fn genesis_account_for_launch(",
                    "impl Clone for RecoveredLifecycleOwnerFactoryInputsV1",
                    "impl Clone for AuthenticatedRecoveredAdapterStartup",
                ),
            )
            require_tokens(
                adapter_path,
                "factory-retained local signer identity",
                canonical_owner_factory,
                (
                    "local_signer",
                    "&local_signer",
                    "local_signer: Some(local_signer.public_key().clone())",
                ),
            )
            reject_tokens(
                adapter_path,
                "factory-retained local signer identity",
                canonical_owner_factory,
                ("local_signer: &KeyPair",),
            )
        if apply_source:
            apply_new = _require_qualified_rust_item(
                apply_path,
                apply_source,
                "V2ApplyService",
                "new",
                errors,
                "exact recovered Apply-service construction",
            )
            require_order(
                apply_path,
                "exact recovered Apply-service construction",
                apply_new.source if apply_new is not None else "",
                (
                    "genesis_account: AccountId",
                    "let network_id = state.network_id",
                    "Self {",
                    "state",
                    "queue",
                    "kura",
                    "network_id",
                    "block_cadence",
                    "genesis_account",
                ),
            )
            apply_match = _require_qualified_rust_item(
                apply_path,
                apply_source,
                "V2ApplyService",
                "matches_lifecycle_launch",
                errors,
                "exact recovered Apply-service launch rejoin",
            )
            require_tokens(
                apply_path,
                "exact recovered Apply-service launch rejoin",
                apply_match.source if apply_match is not None else "",
                (
                    "Arc::ptr_eq(&self.state, state)",
                    "Arc::ptr_eq(&self.kura, kura)",
                    "self.network_id == context.network_id",
                    "self.validator_set_pops == validator_set_pops",
                ),
            )
        if body_store_source:
            require_tokens(
                body_store_path,
                "fresh quarantined recovered body-store cut",
                body_store_source,
                (
                    "struct QuarantinedV2BodyStore(V2BodyStore)",
                    "fn into_quarantined_recovered_startup(",
                    "!self.validated.is_empty() || !self.rejected.is_empty() || !self.retired_revalidation.is_empty()",
                    "V2BodyStoreError::RecoveredMarkersAlreadyPromoted",
                ),
            )
            quarantine = region(
                body_store_path,
                body_store_source,
                "fixed quarantined recovered marker replay",
                "impl QuarantinedV2BodyStore {",
                "impl RevalidatedV2BodyStore {",
            )
            require_order(
                body_store_path,
                "fixed quarantined recovered marker replay",
                quarantine,
                (
                    "fn into_revalidated_lifecycle_startup(",
                    "apply_service.recovered_finality_subject(context)",
                    "self.0.retain_recovered_markers_for_subject(subject)",
                    "self.0.retain_recovered_markers_for_authority(validation_authority)",
                    "self.0.revalidate_recovered_markers(|body|",
                    "apply_service.revalidate_recovered_candidate(context, body)",
                    "self.0.into_revalidated_startup()",
                ),
            )
            reject_tokens(
                body_store_path,
                "fixed quarantined recovered marker replay",
                quarantine,
                (
                    "pub(in crate::sumeragi) fn retain_recovered_markers_for_subject(",
                    "pub(in crate::sumeragi) fn retain_recovered_markers_for_authority(",
                    "pub(in crate::sumeragi) fn revalidate_recovered_markers<",
                    "pub(in crate::sumeragi) fn into_revalidated_startup(",
                ),
            )
            require_tokens(
                body_store_path,
                "revalidated body-store canonical-root oracle",
                body_store_source,
                (
                    "fn matches_lifecycle_storage_root(",
                    "&self.0.signature_policy == signature_policy",
                    "self.0.directory == root.join(hex::encode(context.id().0.as_ref()))",
                    "StoreRootMismatch",
                ),
            )
            terminal_store_join = region(
                ledger_path,
                ledger_source,
                "CompleteTip terminal Apply store join",
                "fn into_complete_tip_terminal_apply_store_join(",
                "\n    /// Purely stage one adapter-authenticated WAL-ahead Validate-to-Sign repair.",
            )
            require_order(
                ledger_path,
                "CompleteTip terminal Apply store join",
                terminal_store_join,
                (
                    "ledger_store.is_authorized_complete_tip_predecessor_target(&complete_tip)",
                    "ledger_store.load()? != self",
                    "self.authenticate_complete_tip_terminal_apply(&complete_tip)?",
                    "cut.is_exact()?",
                    "Ok(cut)",
                ),
            )
            require_tokens(
                ledger_path,
                "CompleteTip foreign target regression",
                ledger_source,
                (
                    "fn complete_tip_terminal_apply_store_join_rejects_an_identical_foreign_target()",
                    "fn complete_tip_all_row_retirement_is_exact_and_restart_idempotent()",
                    "fn complete_tip_retirement_survives_completed_serve_body_cleanup_with_live_work()",
                    "fn complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner()",
                ),
            )
        launch_path, launch_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"
        )
        kura_path, kura_source = load("crates/iroha_core/src/kura.rs")
        owner_path, owner_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs"
        )
        worker_path, worker_source = load(
            "crates/iroha_core/src/sumeragi/v2_worker.rs"
        )
        scheduler_path, scheduler_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs"
        )
        registry_path, registry_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs"
        )
        registry_validate_path, registry_validate_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs"
        )
        wal_recovery_path, wal_recovery_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_wal_recovery.rs"
        )
        selector_path, selector_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs"
        )
        body_pipeline_path, body_pipeline_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs"
        )
        replay_authority_path, replay_authority_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_replay_authority.rs"
        )
        runtime_path, runtime_source = load(
            "crates/iroha_core/src/sumeragi/v2_runtime.rs"
        )
        effects_path, effects_source = load(
            "crates/iroha_core/src/sumeragi/v2_effects.rs"
        )
        transport_path, transport_source = load(
            "crates/iroha_core/src/sumeragi/v2_transport.rs"
        )
        lifecycle_open_path, lifecycle_open_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_open.rs"
        )
        runner_dependency_path, runner_dependency_source = load(
            "crates/iroha_core/src/sumeragi/v2_runner.rs"
        )
        state_path, state_source = load("crates/iroha_core/src/state.rs")
        apply_path, apply_source = load(
            "crates/iroha_core/src/sumeragi/v2_apply.rs"
        )
        if (
            launch_source
            and kura_source
            and owner_source
            and worker_source
            and scheduler_source
            and registry_source
            and registry_validate_source
            and wal_recovery_source
            and selector_source
            and body_pipeline_source
            and replay_authority_source
            and runtime_source
            and effects_source
            and transport_source
            and lifecycle_open_source
            and runner_dependency_source
            and state_source
            and apply_source
        ):
            lifecycle_launch_item = _require_qualified_rust_item(
                launch_path,
                launch_source,
                "ProductionLifecycleOwnerV1",
                "launch",
                errors,
                "Kura-bound production lifecycle launch",
                expected_attributes=("#[allow(clippy::result_large_err)]",),
            )
            lifecycle_launch = (
                lifecycle_launch_item.source
                if lifecycle_launch_item is not None
                else ""
            )
            require_order(
                launch_path,
                "Kura-bound production lifecycle launch",
                lifecycle_launch,
                (
                    "begin_fail_stop_operation()",
                    "Self::launch_local_identity_matches( &context.roster, &inputs.local_peer, inputs.local_validator, &inputs.key_pair, )",
                    "binding.matches_launch_identity(inputs.kura.as_ref(), &inputs.key_pair)",
                    "service.matches_lifecycle_launch( &inputs.state, &inputs.kura, &context, &validator_set_pops, )",
                    "binding.storage_paths_for_launch(inputs.kura.as_ref())",
                    "prepare_leader_wire_launch(launch_storage.wal_path())",
                    "ProductionV2Services::restore_lifecycle_ordinal_source(",
                    "leader_wire_launch.restored_producer_ordinal_high_watermark()",
                    "leader_wire_launch.open_gate(",
                    "leader_wire_restore.scheduler_ordinal_high_watermark()",
                    "ProductionLeaderWireIngressBindingV1::bind(",
                    "self.adapter_startup.take()",
                    "self.body_store.take()",
                    "self.apply_service.take()",
                    "V2EffectExecutor::open_with_body_store(",
                    "if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()",
                    "executor.install_authenticated_genesis_body(authenticated_genesis.signed_block())",
                    "ProductionV2Services::start_with_apply_service(",
                    "ProductionLifecycleApplyServiceLaunchPermitV1",
                    "apply_service,",
                ),
            )
            runner_dependency_permit = region(
                runner_dependency_path,
                runner_dependency_source,
                "runner-sealed recovered lifecycle factory dependency permit",
                "pub(in crate::sumeragi) struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                "/// Cadence-derived process-local deadline",
            )
            require_tokens(
                runner_dependency_path,
                "runner-sealed recovered lifecycle factory dependencies",
                runner_dependency_permit,
                (
                    "struct RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "_seal: RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
                    "local_signer: KeyPair",
                    "fn mint_for_recovered_runner(local_signer: KeyPair) -> Self",
                    "#[cfg(test)] pub(in crate::sumeragi) fn for_test(local_signer: KeyPair) -> Self",
                    "fn into_local_signer(self) -> KeyPair",
                    "impl Drop for RecoveredLifecycleOwnerFactoryDependencyPermitSealV1",
                ),
            )
            reject_tokens(
                runner_dependency_path,
                "runner-sealed recovered lifecycle factory dependencies",
                runner_dependency_permit,
                (
                    "pub(in crate::sumeragi) fn mint_for_recovered_runner(",
                    "pub(crate) fn mint_for_recovered_runner(",
                    "pub fn mint_for_recovered_runner(",
                    "impl Clone for RecoveredLifecycleOwnerFactoryDependencyPermitV1",
                    "fn into_parts(",
                ),
            )
            require_tokens(
                launch_path,
                "local launch identity preflight",
                launch_source,
                (
                    "fn launch_local_identity_matches(",
                    "local_peer.public_key() != key_pair.public_key()",
                    "local_validator.is_none_or(|observed| roster_position == Some(observed))",
                    "fn launch_local_identity_requires_the_bound_key_and_exact_roster_position()",
                ),
            )
            require_token_count(
                launch_path,
                "single retained Apply-service transfer",
                lifecycle_launch,
                "self.apply_service.take()",
                1,
            )
            require_token_count(
                launch_path,
                "single retained Apply-service worker start",
                lifecycle_launch,
                "ProductionV2Services::start_with_apply_service(",
                1,
            )
            reject_tokens(
                launch_path,
                "retained Apply-service continuity",
                lifecycle_launch,
                ("V2ApplyService::new(", "genesis_account_for_launch("),
            )
            require_tokens(
                launch_path,
                "single restored lifecycle ordinal source",
                lifecycle_launch,
                (
                    "inputs.network.reply_route_source_capacity().max(1)",
                    "inputs.auxiliary_io_capacity",
                    "lifecycle_ordinals.clone()",
                    "lifecycle_ordinals,",
                ),
            )
            require_token_count(
                launch_path,
                "certified Serve restore/start capacity parity",
                lifecycle_launch,
                "inputs.auxiliary_io_capacity",
                2,
            )
            require_tokens(
                launch_path,
                "move-only authenticated genesis launch input",
                launch_source,
                (
                    "authenticated_genesis: Option<AuthenticatedGenesisBodyV1>",
                    "if let Some(authenticated_genesis) = inputs.authenticated_genesis.as_ref()",
                    "authenticated_genesis.signed_block()",
                ),
            )
            reject_tokens(
                launch_path,
                "move-only authenticated genesis launch input",
                region(
                    launch_path,
                    launch_source,
                    "sealed production launch inputs",
                    "pub(in crate::sumeragi) struct ProductionLifecycleLaunchInputsV1 {",
                    "\n}",
                ),
                (
                    "authenticated_genesis: Option<SignedBlock>",
                    "genesis_account: AccountId",
                    "chunk_root: PathBuf",
                    "wal_path: PathBuf",
                    "lifecycle_ordinals: RuntimeLifecycleOrdinalSource",
                    "durable_bodies:",
                    "recovered_body_receipts:",
                    "queue: Arc<Queue>",
                    "provider_ingest_finalized_archive:",
                    "reputation_finalized_archive:",
                    "block_cadence: Duration",
                    "events_sender: EventsSender",
                ),
            )
            require_tokens(
                worker_path,
                "sealed replay-service worker transfer",
                worker_source,
                (
                    "fn start_with_apply_service(",
                    "_permit: super::v2_lifecycle_coordinator::ProductionLifecycleApplyServiceLaunchPermitV1",
                    "apply_service.matches_lifecycle_launch(&state, &kura, &context, &validator_set_pops)",
                    "Self::start_inner(",
                ),
            )
            legacy_worker_start = region(
                worker_path,
                worker_source,
                "legacy worker Apply-service construction",
                "pub(crate) fn start(",
                "/// Start with the exact application service used for recovered marker replay.",
            )
            require_order(
                worker_path,
                "legacy worker Apply-service construction",
                legacy_worker_start,
                (
                    "let apply_service = V2ApplyService::new(",
                    "Self::start_inner(",
                ),
            )
            reject_tokens(
                worker_path,
                "legacy worker Apply-service construction",
                legacy_worker_start,
                ("Self::start_with_apply_service(",),
            )
            require_token_count(
                worker_path,
                "sealed replay-service worker transfer",
                worker_source,
                "ProductionLifecycleApplyServiceLaunchPermitV1",
                1,
            )
            require_token_count(
                launch_path,
                "sealed replay-service permit mint",
                launch_source,
                "ProductionLifecycleApplyServiceLaunchPermitV1 {",
                1,
            )
            require_tokens(
                state_path,
                "fixed State/Kura identity oracle",
                state_source,
                (
                    "fn matches_kura_instance(&self, kura: &Arc<Kura>) -> bool",
                    "Arc::ptr_eq(&self.kura, kura)",
                ),
            )
            require_tokens(
                apply_path,
                "fixed recovered Apply-service identity oracle",
                apply_source,
                (
                    "fn matches_lifecycle_launch(",
                    "Arc::ptr_eq(&self.state, state)",
                    "Arc::ptr_eq(&self.kura, kura)",
                    "self.network_id == context.network_id",
                    "self.validator_set_pops == validator_set_pops",
                ),
            )
            require_tokens(
                launch_path,
                "sealed leader-wire launch binding",
                launch_source,
                (
                    "struct ProductionLeaderWireIngressBindingV1",
                    "self.ingress.close()",
                    "self.ingress.unbind_leader_wire_lifecycle_gate(gate)?",
                    "impl Drop for ProductionLeaderWireIngressBindingV1",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            require_tokens(
                adapter_path,
                "sealed adapter leader-wire launch projection",
                adapter_source,
                (
                    "struct ProductionLeaderWireLaunchAuthorityV1",
                    "fn prepare_leader_wire_launch(",
                    "adapter.wal.matches_path(expected_wal_path)",
                    "leader_wire_launch_prepared: false",
                    "!*leader_wire_launch_prepared",
                    "*leader_wire_launch_prepared = true",
                    "fn open_gate(",
                    "body_store: &super::v2_body_store::V2BodyStore",
                    "body_store.matches_context(context)",
                    "body_store.recovery_catalog()",
                    "LeaderWireLifecycleStoreGate::open_with_safety_wal_authority(",
                ),
            )
            require_tokens(
                safety_wal_path,
                "opened safety-WAL directory authority",
                safety_wal_source,
                (
                    "struct SafetyWalServicedCandidateStoreAuthority",
                    "struct SafetyWalLeaderWireStoreAuthority",
                    "direct_lexical_directory_metadata(expected_path)",
                    "open_canonical_directory_nofollow(&canonical_path)",
                    "fn mint_serviced_candidate_store_authority(",
                    "fn mint_leader_wire_store_authority(",
                    "fn publish_atomic(&self, frame: &[u8], maximum: u64",
                    "let durable = rustix::fs::statat(",
                    "fn write_all(&mut self, bytes: &[u8])",
                    "fn sync_data(&mut self)",
                    "BoundSafetyWalDirectory::from_kura_authority(kura, authority)",
                ),
            )
            require_literal_count(
                safety_wal_path,
                "opened safety-WAL exact Kura identity rejection",
                safety_wal_source,
                '"safety-WAL authority belongs to a different Kura instance"',
                1,
            )
            require_tokens(
                kura_path,
                "Kura-root safety-WAL authority",
                kura_source,
                (
                    "struct KuraSafetyWalDirectoryAuthority",
                    "fn mint_safety_wal_directory_authority(",
                    "rustix::fs::openat(&root.file, STORE_ROOT_LOCK_FILE_NAME",
                    "Self::sidecar_file_metadata_unchanged(&lock_before, &linked_metadata)",
                    "rustix::fs::mkdirat(&parent.file, name, rustix::fs::Mode::RWXU)",
                    "Self::open_bound_progress_child_directory(",
                    "kura_identity: self.instance_identity()",
                ),
            )
            reject_tokens(
                safety_wal_path,
                "move-only safety-WAL sibling authorities",
                safety_wal_source,
                (
                    "impl Clone for SafetyWalServicedCandidateStoreAuthority",
                    "impl Clone for SafetyWalLeaderWireStoreAuthority",
                    "impl Copy for SafetyWalServicedCandidateStoreAuthority",
                    "impl Copy for SafetyWalLeaderWireStoreAuthority",
                ),
            )
            require_tokens(
                adjacent_store_path,
                "typed WAL-adjacent production stores",
                adjacent_store_source,
                (
                    "storage: SafetyWalServicedCandidateStoreAuthority",
                    "storage: SafetyWalLeaderWireStoreAuthority",
                    "fn open_with_safety_wal_authority(",
                    "self.storage.read_bounded(self.max_frame_bytes)",
                    "self.storage.publish_atomic(&frame, self.max_frame_bytes)",
                ),
            )
            serviced_candidate_open = _require_qualified_rust_item(
                adjacent_store_path,
                adjacent_store_source,
                "ServicedCandidateStore",
                "open_with_safety_wal_authority",
                errors,
                "typed WAL-adjacent production stores omits production refinement tokens in the serviced-candidate constructor",
            )
            _require_rust_token_sequence(
                adjacent_store_path,
                serviced_candidate_open,
                "storage: SafetyWalServicedCandidateStoreAuthority",
                "typed WAL-adjacent production stores omits production refinement tokens in the serviced-candidate constructor",
                errors,
            )
            leader_wire_open = _require_qualified_rust_item(
                adjacent_store_path,
                adjacent_store_source,
                "LeaderWireLifecycleStoreGate",
                "open_with_safety_wal_authority",
                errors,
                "typed WAL-adjacent production stores omits production refinement tokens in the leader-wire constructor",
                expected_attributes=("#[allow(clippy::too_many_arguments)]",),
            )
            _require_rust_token_sequence(
                adjacent_store_path,
                leader_wire_open,
                "storage: SafetyWalLeaderWireStoreAuthority",
                "typed WAL-adjacent production stores omits production refinement tokens in the leader-wire constructor",
                errors,
            )
            reject_tokens(
                adapter_path,
                "move-only leader-wire launch authority",
                adapter_source,
                (
                    "impl Clone for ProductionLeaderWireLaunchAuthorityV1",
                    "impl Clone for RecoveredLifecycleStorageAuthorityV1",
                    "impl Clone for RecoveredLifecycleLaunchStoragePathsV1",
                ),
            )
            owner_binding = _require_qualified_rust_item(
                owner_path,
                owner_source,
                "ProductionLifecycleOwnerV1",
                "with_recovered_kura_binding_and_apply_service",
                errors,
                "production lifecycle owner Kura/Apply seal",
            )
            require_tokens(
                owner_path,
                "production lifecycle owner Kura seal",
                owner_source,
                (
                    "kura_binding: Option<crate::sumeragi::v2::RecoveredLifecycleOwnerKuraBindingV1>",
                    "apply_service: Option<crate::sumeragi::v2_apply::V2ApplyService>",
                    "fn with_recovered_kura_binding_and_apply_service(",
                    "assert!(self.kura_binding.is_none())",
                    "assert!(self.apply_service.is_none())",
                    "self.kura_binding = Some(binding)",
                    "self.apply_service = Some(apply_service)",
                    "struct ProductionLifecycleApplyServiceLaunchPermitV1",
                    "impl Drop for ProductionLifecycleApplyServiceLaunchPermitSealV1",
                ),
            )
            recovered_sign_dispatch = region(
                scheduler_path,
                scheduler_source,
                "lifecycle-owned recovered Sign dispatch",
                "fn dispatch_recovered_lifecycle_sign_with_runner_debt(",
                "/// Refanout one durable recovered signed Broadcast at the live Completion cursor.",
            )
            require_order(
                scheduler_path,
                "lifecycle-owned recovered Sign dispatch",
                recovered_sign_dispatch,
                (
                    "let Some(body_store_identity) = self.body_store_identity.as_ref()",
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "services.matches_lifecycle_executor_output_guard(executor)",
                    "attest_ready_recovered_lifecycle_sign",
                    "capture_recovered_lifecycle_sign_capacity(dispatch_key)",
                    "self.coordinator.plan_turn(inputs)",
                    "reservation.class() == CapacityClass::Consensus",
                    "prepare_recovered_lifecycle_sign_dispatch",
                    "reservation.preflight(&prepared)",
                    "reservation.commit(prepared)",
                ),
            )
            require_token_count(
                scheduler_path,
                "recovered Sign post-claim rollback",
                recovered_sign_dispatch,
                "self.coordinator.rollback_unpublished_turn(&lease)",
                1,
            )
            require_token_count(
                scheduler_path,
                "recovered Sign reserved post-claim rollback",
                recovered_sign_dispatch,
                "rollback_unpublished_reserved_turn(&lease",
                3,
            )
            require_token_count(
                scheduler_path,
                "recovered Sign reservation release",
                recovered_sign_dispatch,
                "reservation.cancel_uncommitted()",
                6,
            )
            reject_tokens(
                scheduler_path,
                "sealed recovered Sign dispatch",
                recovered_sign_dispatch,
                (
                    "AdapterEffect",
                    "PendingRuntimeEffectBinding",
                    "RuntimeEffectOwnership",
                    "EffectWorkId",
                    "into_parts",
                ),
            )
            recovered_phase_sign = region(
                registry_path,
                registry_source,
                "current-parent-bound recovered PhaseVote carrier",
                "impl DurableRecoveredWalSignWork {",
                "/// Whether one concrete registry row is still an executable adapter effect",
            )
            require_token_count(
                registry_path,
                "current-parent-bound recovered PhaseVote carrier",
                recovered_phase_sign,
                "self.matches_current_terminal_parent(coordinator)",
                2,
            )
            require_token_count(
                registry_path,
                "standalone recovered PhaseVote child",
                recovered_phase_sign,
                "metadata.continuation == super::schema::DurableContinuation::None",
                2,
            )
            require_tokens(
                registry_path,
                "current terminal Validate parent rejoin",
                recovered_phase_sign,
                (
                    "record.state == super::LifecycleState::Terminal(super::TerminalOutcome::Advanced)",
                    "metadata.matches_admission(parent)",
                    "super::schema::DurableContinuation::successor(",
                    "coordinator.key_index.get(&parent.key)",
                    "coordinator.owner_index.get(&parent.causal_root)",
                ),
            )
            recovered_sign_identity = region(
                registry_path,
                registry_source,
                "complete recovered Sign effect identity",
                "impl RecoveredLifecycleSignDispatchIdentityV1 {",
                "/// Read-only coordinates of one exact Waiting Fetch incumbent.",
            )
            require_tokens(
                registry_path,
                "complete recovered Sign effect identity",
                recovered_sign_identity,
                (
                    "&AdapterEffect::Sign {",
                    "request: request.clone()",
                    "adapter_effect_matches_lifecycle_digest(",
                ),
            )
            reject_tokens(
                registry_path,
                "historical recovered Commit identity",
                recovered_sign_identity,
                ("tag.view() ==", "vote.round.view"),
            )
            recovered_sign_task = region(
                worker_path,
                worker_source,
                "opaque recovered Sign worker task/result",
                "pub(in crate::sumeragi) struct RecoveredLifecycleSignTaskV1 {",
                "enum V2IoCommand {",
            )
            require_tokens(
                worker_path,
                "opaque recovered Sign worker task/result",
                recovered_sign_task,
                (
                    "identity: RecoveredLifecycleSignDispatchIdentityV1",
                    "prepared_candidate: Option<PreparedCandidateBody>",
                    "self.task.prepared_candidate == expected_prepared",
                    "outbound_payload: Option<EncodedV2Payload>",
                    "authorizes_request(self.task.tag, &self.task.request)",
                ),
            )
            reject_tokens(
                worker_path,
                "opaque recovered Sign worker task/result",
                recovered_sign_task,
                (
                    "pub tag:",
                    "pub request:",
                    "pub signature:",
                    "pub outbound_payload:",
                    "fn into_parts(",
                    "fn into_result(",
                    "fn into_task(",
                    "fn request(",
                    "fn prepared_candidate(",
                    "fn result(",
                    "fn acknowledgement(",
                    "fn acknowledge(",
                    "fn signature(",
                    "fn outbound_payload(",
                ),
            )
            parked_sign_completion = region(
                worker_path,
                worker_source,
                "parked recovered Sign completion",
                "pub(in crate::sumeragi) struct PreparedRecoveredLifecycleSignCompletionV1 {",
                "/// Result of atomically returning one guarded missing-sidecar Apply",
            )
            reject_tokens(
                worker_path,
                "parked recovered Sign completion",
                parked_sign_completion,
                (
                    "fn into_parts(",
                    "fn into_result(",
                    "fn into_task(",
                    "fn request(",
                    "fn prepared_candidate(",
                    "fn result(",
                    "fn acknowledgement(",
                    "fn acknowledge(",
                    "fn signature(",
                    "fn outbound_payload(",
                    "fn settle(",
                ),
            )
            require_tokens(
                worker_path,
                "adapter-private recovered Sign completion projection",
                parked_sign_completion,
                (
                    "fn project_adapter_completion_authority(",
                    "result.is_exact()",
                    "RecoveredLifecycleSignAdapterCompletionAuthorityV1 {",
                ),
            )
            require_tokens(
                worker_path,
                "post-publication recovered Sign completion acknowledgement",
                parked_sign_completion,
                (
                    "fn acknowledge_after_publication(self)",
                    "self.queue.acknowledge_recovered_lifecycle_sign(key)",
                    "self.guarded.acknowledge_after_publication()",
                ),
            )
            recovered_sign_preview = region(
                adapter_path,
                adapter_source,
                "drop-inert recovered Sign adapter preview",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion(",
                "/// Acknowledge successful application of the exact tagged decision.",
            )
            require_order(
                adapter_path,
                "drop-inert recovered Sign adapter preview",
                recovered_sign_preview,
                (
                    "authority.consume_for_adapter(RecoveredLifecycleSignAdapterCompletionPermitV1::new())",
                    "verify_individual_signature(",
                    "let mut next_reducer = self.reducer.clone()",
                    "next_reducer.step(event.clone())",
                    "if converted.first() != Some(&expected_broadcast)",
                    "Ok(PreparedRecoveredLifecycleSignAdapterCompletionV1 {",
                ),
            )
            require_tokens(
                adapter_path,
                "closed recovered Sign adapter successor shapes",
                recovered_sign_preview,
                (
                    "SignRequest::Proposal(_), Some((persist_tag, entry)), None",
                    "SignRequest::Proposal(_), None, Some(AdapterEffect::Sign { request: SignRequest::Vote(vote), .. })",
                    "vote.phase == wire::GlobalPhase::Prepare",
                    "SignRequest::Vote(_) | SignRequest::TimeoutVote(_), None, possible_next_sign",
                    "next_reducer.pending_persistence_record().is_none()",
                    "next_reducer.awaiting_signature()",
                    "RecoveredLifecycleSignCompletionMismatch",
                ),
            )
            reject_tokens(
                adapter_path,
                "drop-inert recovered Sign adapter preview",
                recovered_sign_preview,
                (
                    "self.wal.append(",
                    "self.reducer =",
                    "self.registry =",
                    "publish_effect",
                    "send(",
                ),
            )
            require_tokens(
                adapter_path,
                "recovered Sign adapter preview behavior regression",
                adapter_source,
                (
                    "fn recovered_timeout_signature_preview_is_exact_and_drop_inert()",
                    "fn production_recovered_proposal_sign_joins_exact_next_vote_body_store()",
                ),
            )
            next_vote_service_join = region(
                worker_path,
                worker_source,
                "single-preview recovered next-Vote body service join",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body<'executor>(",
                "/// Publish the live completion owner",
            )
            require_order(
                worker_path,
                "single-preview recovered next-Vote body service join",
                next_vote_service_join,
                (
                    "self.recovered_lifecycle_next_vote_body_executor_permit(executor)?",
                    "executor.prepare_recovered_lifecycle_sign_completion_with_body(permit, completion)",
                ),
            )
            reject_tokens(
                worker_path,
                "single-preview recovered next-Vote body service join",
                next_vote_service_join,
                (
                    "ValidatedBodyReceipt",
                    "V2BodyStore",
                    "prepare_recovered_lifecycle_sign_completion(completion)",
                    "into_parts",
                ),
            )
            next_vote_executor_join = region(
                effects_path,
                effects_source,
                "single-preview recovered next-Vote body executor join",
                "pub(in crate::sumeragi) fn prepare_recovered_lifecycle_sign_completion_with_body(",
                "/// Publish executor-retained owners",
            )
            require_order(
                effects_path,
                "single-preview recovered next-Vote body executor join",
                next_vote_executor_join,
                (
                    "service.consume_for_executor(",
                    "runtime.prepare_recovered_lifecycle_sign_completion(completion)",
                    "preview.project_broadcast_and_sign_body_lookup(",
                    "authenticate_recovered_lifecycle_next_vote_body_catalogs(",
                    "Ok((preview, body))",
                ),
            )
            next_vote_catalog_join = region(
                effects_path,
                effects_source,
                "exact recovered next-Vote body catalog join",
                "fn authenticate_recovered_lifecycle_next_vote_body_catalogs(",
                "impl V2EffectExecutor<SerializedV2Runtime>",
            )
            require_tokens(
                effects_path,
                "exact recovered next-Vote body catalog join",
                next_vote_catalog_join,
                (
                    "validated_bodies.get(&key) != Some(&validated)",
                    "durable_bodies.get(&key) != Some(durable)",
                    "recovered_bodies.get(&key)",
                    "HashOf::new(manifest) != durable.manifest_hash()",
                    "lookup.matches_recovered_body(manifest, recovered_durable)",
                    "RecoveredLifecycleNextVoteBodyAuthorityMintPermitV1::new()",
                ),
            )
            next_vote_body_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered next-Vote body authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleNextVoteBodyAuthorityV1 {",
                "/// Closed reducer successor shape produced by one exact recovered signature.",
            )
            require_tokens(
                adapter_path,
                "opaque recovered next-Vote body authority",
                next_vote_body_authority,
                (
                    "body_store_identity.same_instance(expected_body_store_identity)",
                    "lookup.matches_adapter_successor(next_sign, expected_proposal_manifest_hash)",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered next-Vote body authority",
                next_vote_body_authority,
                (
                    "impl Clone for RecoveredLifecycleNextVoteBodyAuthorityV1",
                    "fn into_parts(",
                    "fn validated(",
                    "fn body_store_identity(",
                    "fn lookup(",
                ),
            )
            combined_adapter_projection = region(
                adapter_path,
                adapter_source,
                "affine recovered Broadcast-and-next-Sign adapter projection",
                "pub(in crate::sumeragi) fn project_broadcast_and_sign_authority(",
                "/// Exercise fail-closed next-Sign substitution",
            )
            require_order(
                adapter_path,
                "affine recovered Broadcast-and-next-Sign adapter projection",
                combined_adapter_projection,
                (
                    "self.combined_authority_minted",
                    "body_authority.consume_for_adapter(",
                    "self.adapter.authenticate_recovered_lifecycle_next_vote(",
                    "self.combined_authority_minted = true",
                    "RecoveredLifecycleSignBroadcastAndSignAuthorityV1 {",
                ),
            )
            proposal_output_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered Proposal exact-output authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputAuthorityV1 {",
                "/// Adapter-authenticated combined successor of one recovered signature.",
            )
            require_tokens(
                adapter_path,
                "opaque recovered Proposal exact-output authority",
                proposal_output_authority,
                (
                    "body_store_identity: V2BodyStoreInstanceIdentity",
                    "output_guard: Arc<super::output_guard::ConsensusOutputGuard>",
                    "fn consume_for_service(",
                    "fn from_service_retry(",
                    "Self::validated(",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered Proposal exact-output authority",
                proposal_output_authority,
                (
                    "impl Clone for RecoveredLifecycleProposalExactOutputAuthorityV1",
                    "fn into_parts(",
                    "fn proposal(",
                    "fn payload(",
                    "fn body_store_identity(",
                    "fn output_guard(",
                ),
            )
            proposal_output_projection = region(
                adapter_path,
                adapter_source,
                "affine recovered Proposal exact-output projection",
                "pub(in crate::sumeragi) fn project_proposal_exact_output_authority(",
                "fn broadcast_proposal_manifest_hash(",
            )
            require_order(
                adapter_path,
                "affine recovered Proposal exact-output projection",
                proposal_output_projection,
                (
                    "let shape = self.shape()",
                    "self.proposal_output_authority_minted",
                    "!matches!(shape, RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign | RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal)",
                    "shape == RecoveredLifecycleSignAdapterSuccessorShapeV1::ProposalPrepareWal",
                    "self.prepared_prepare_wal.is_none()",
                    "payload.manifest() == &signed.manifest",
                    "self.next_vote_body_store_identity.as_ref()",
                    "self.next_vote_output_guard.as_ref()",
                    "self.proposal_output_authority_minted = true",
                    "RecoveredLifecycleProposalExactOutputAuthorityV1 {",
                ),
            )
            proposal_batch_preflight = region(
                worker_path,
                worker_source,
                "mutation-free atomic Proposal fanout preflight",
                "fn prepare_atomic_fanout_batch(",
                "/// Commit a batch prepared while this exact mutex guard remained held.",
            )
            require_order(
                worker_path,
                "mutation-free atomic Proposal fanout preflight",
                proposal_batch_preflight,
                (
                    "let mut additions = BTreeMap",
                    "aggregate.checked_add(count)",
                    "self.ownership_capacity_available(&additions)?",
                    "self.ownership_state_after_additions(&additions)?",
                    "let project_ids = |first: ExactFanoutFifoId|",
                    "self.source_fifo_owners.clone()",
                    "Some(existing_ids)",
                    "source_fifo_owners.entry(source).or_default().insert(fifo_id)",
                    "PendingExactOutputBatchPlan {",
                ),
            )
            reject_tokens(
                worker_path,
                "mutation-free atomic Proposal fanout preflight",
                proposal_batch_preflight,
                (
                    "self.fanouts.extend(",
                    "self.source_fifo_owners =",
                    "self.reservation_owner_counts =",
                    "self.ownership_units =",
                    "rebase_source_fifo(",
                    "allocate_fanout_fifo_id(",
                    ".enqueue(",
                    "next_fanout_index =",
                ),
            )
            proposal_batch_commit = region(
                worker_path,
                worker_source,
                "assertion-only atomic Proposal fanout commit",
                "fn commit_atomic_fanout_batch(&mut self, plan: PendingExactOutputBatchPlan)",
                "fn is_pending(&self)",
            )
            require_order(
                worker_path,
                "assertion-only atomic Proposal fanout commit",
                proposal_batch_commit,
                (
                    "assert_eq!(self.fanouts.len(), existing_fanout_count",
                    "if let Some(rebased) = rebased_existing_fifo_ids",
                    "fanout.fifo_id = Some(fifo_id)",
                    "self.fanouts.extend(fanouts)",
                    "self.source_fifo_owners = source_fifo_owners",
                    "self.reservation_owner_counts = reservation_owner_counts",
                    "self.ownership_units = ownership_units",
                    "self.shared_ownership_units = shared_ownership_units",
                    "self.next_fanout_fifo_id = next_fanout_fifo_id",
                ),
            )
            reject_tokens(
                worker_path,
                "assertion-only atomic Proposal fanout commit",
                proposal_batch_commit,
                ("?", "drive_pending_exact_output", ".enqueue("),
            )
            proposal_reservation_fields = region(
                worker_path,
                worker_source,
                "fail-stop-first recovered Proposal reservation ownership",
                "pub(in crate::sumeragi) struct RecoveredLifecycleProposalExactOutputReservationV1<'service> {",
                "#[cfg_attr(not(test), allow(dead_code))]\nimpl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
            )
            require_order(
                worker_path,
                "fail-stop-first recovered Proposal reservation ownership",
                proposal_reservation_fields,
                (
                    "operation: Option<ConsensusFailStopOperation<'service>>",
                    "pending: Option<std::sync::MutexGuard<'service, PendingExactOutput>>",
                    "batch: Option<PendingExactOutputBatchPlan>",
                    "authority: Option<super::v2::RecoveredLifecycleProposalExactOutputAuthorityV1>",
                ),
            )
            proposal_reservation_impl = region(
                worker_path,
                worker_source,
                "sealed recovered Proposal reservation methods",
                "impl RecoveredLifecycleProposalExactOutputReservationV1<'_> {",
                "/// Result of reserving exact output for one recovered Decision Fetch request.",
            )
            proposal_reservation_abort = region(
                worker_path,
                proposal_reservation_impl,
                "retry-safe recovered Proposal reservation abort",
                "pub(in crate::sumeragi) fn abort_before_publication(",
                "/// Install both preflighted fanouts in one assertion-only publication tail.",
            )
            require_order(
                worker_path,
                "retry-safe recovered Proposal reservation abort",
                proposal_reservation_abort,
                (
                    "drop(self.pending.take())",
                    "drop(self.batch.take())",
                    ".complete()",
                    "self.authority.take()",
                ),
            )
            proposal_reservation_commit = proposal_reservation_impl.split(
                "/// Install both preflighted fanouts in one assertion-only publication tail.",
                1,
            )[-1]
            require_order(
                worker_path,
                "assertion-only recovered Proposal reservation commit",
                proposal_reservation_commit,
                (
                    "let mut pending = self.pending.take()",
                    "let operation = self.operation.take()",
                    "let batch = self.batch.take()",
                    "let authority = self.authority.take()",
                    "pending.commit_atomic_fanout_batch(batch)",
                    "drop(pending)",
                    "drop(authority)",
                    "operation.complete()",
                ),
            )
            reject_tokens(
                worker_path,
                "sealed recovered Proposal reservation methods",
                proposal_reservation_abort + proposal_reservation_commit,
                ("drive_pending_exact_output", ".enqueue("),
            )
            proposal_output_capture = region(
                worker_path,
                worker_source,
                "retry-safe recovered Proposal exact-output capture",
                "pub(in crate::sumeragi) fn capture_recovered_lifecycle_proposal_exact_output(",
                "/// Consume one carrier-derived recovered Fetch through this exact service key.",
            )
            require_order(
                worker_path,
                "retry-safe recovered Proposal exact-output capture",
                proposal_output_capture,
                (
                    "self.proposal_work_retired",
                    "authority.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new())",
                    "tag != self.active_tag",
                    "self.local_validator != Some(proposal.proposer)",
                    "proposal.manifest != *payload.manifest()",
                    "identity.same_instance(&body_store_identity)",
                    "Arc::ptr_eq(&self.output_guard, &authority_output_guard)",
                    "message.validate_version()",
                    "proposal.validate(&self.context)",
                    "RecoveredLifecycleProposalExactOutputAuthorityV1::from_service_retry(",
                    "payload.into_parts()",
                    "manifest.validate(&self.context)",
                    "chunk.signature_preimage(&self.context, &manifest)",
                    "Signature::try_new(self.key_pair.private_key(), &preimage)",
                    "let peers = self.remote_voters()",
                    "let control = PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::GlobalV2(self.exact_output_scope())",
                    "let chunks = PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::PayloadChunks",
                    "control.into_iter().chain(chunks)",
                    "begin_fail_stop_operation()",
                    "let pending = self.lock_pending_exact_output()?",
                    "pending.prepare_atomic_fanout_batch(fanouts)",
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(retry_authority,)",
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Reserved(",
                    "authority: Some(retry_authority)",
                ),
            )
            require_token_count(
                worker_path,
                "fail-stop recovered Proposal capture errors",
                proposal_output_capture,
                "drop(operation)",
                2,
            )
            reject_tokens(
                worker_path,
                "all-voter recovered Proposal retransmission policy",
                proposal_output_capture,
                ("fast_path_proposals", "remote_voters_for_indices"),
            )
            broadcast_consensus = region(
                worker_path,
                worker_source,
                "production consensus broadcast",
                "fn broadcast_consensus(",
                "fn sign_body_request(",
            )
            proposal_live_atomic = region(
                worker_path,
                broadcast_consensus,
                "live Proposal control-plus-chunk atomic transfer",
                "if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload {",
                "let control = vec![Self::preencode_v2_network_message(message)?]",
            )
            require_order(
                worker_path,
                "live Proposal control-plus-chunk atomic transfer",
                proposal_live_atomic,
                (
                    "self.outbound_chunks.get(&manifest_hash)",
                    "let first_fast_path_send = !self.fast_path_proposals.contains(&proposal.round)",
                    "PendingExactFanout::claimed(",
                    "ExactOutputRolloverClaim::PayloadChunks",
                    "self.enqueue_atomic_fanout_batch_while_guarded(",
                    "ownership == ExactFanoutOwnership::Owned && first_fast_path_send",
                    "self.fast_path_proposals.insert(proposal.round)",
                ),
            )
            reject_tokens(
                worker_path,
                "live Proposal control-plus-chunk atomic transfer",
                proposal_live_atomic,
                (
                    "enqueue_exact_fanout_while_guarded(",
                    "self.fast_path_proposals.insert(proposal.round);\n            let payload_targets",
                ),
            )
            require_tokens(
                worker_path,
                "atomic Proposal output behavior regressions",
                worker_source,
                (
                    "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
                    "fn atomic_fanout_batch_preflights_aggregate_capacity_and_rebases_only_on_commit()",
                    "fn armed_recovered_proposal_output_reservation_fails_stop_on_drop()",
                    "fn proposal_broadcast_reports_source_retained_until_corridor_acceptance()",
                ),
            )
            proposal_output_behavior = region(
                worker_path,
                worker_source,
                "recovered Proposal atomic output behavior",
                "fn recovered_proposal_exact_output_is_atomic_retryable_and_store_bound()",
                "fn prepare_and_commit_votes_reach_every_remote_voter_across_views()",
            )
            require_tokens(
                worker_path,
                "recovered Proposal atomic output behavior",
                proposal_output_behavior,
                (
                    "after, before",
                    "vec![Some(expected_batch_first_fifo), expected_batch_first_fifo.checked_add(1),]",
                    "fanout.peers.iter().cloned().collect::<BTreeSet<_>>()",
                    "wire::ConsensusMessageV2Payload::PayloadChunk(chunk)",
                    "chunk.validate(&service.context, manifest)",
                    "Signature::try_from_bytes(&chunk.signature)",
                    "signature.verify(signer.public_key()",
                    "capture_recovered_lifecycle_proposal_exact_output(retirement_authority).is_err()",
                ),
            )
            require_order(
                worker_path,
                "post-Decision live Proposal output fence",
                broadcast_consensus,
                (
                    "self.proposal_work_retired",
                    "wire::ConsensusMessageV2Payload::Proposal(_)",
                    "begin_fail_stop_operation()",
                    "if let wire::ConsensusMessageV2Payload::Proposal(proposal) = &message.payload",
                ),
            )
            next_vote_candidate_projection = region(
                replay_authority_path,
                replay_authority_source,
                "full executable recovered next-WAL-Vote candidate",
                "pub(in crate::sumeragi) fn into_candidate_projection(",
                "/// Rejoin the retained body marker to one exact recovered phase-vote repair.",
            )
            require_order(
                replay_authority_path,
                "full executable recovered next-WAL-Vote candidate",
                next_vote_candidate_projection,
                (
                    "self.wal_identity.is_exact()",
                    "self.matches_verified_height(verified)",
                    "PendingRuntimeEffectBinding::from_exact_recovered_next_wal_vote(",
                    "self.replay_evidence.project_recovered_vote_candidate(",
                    "RecoveredLifecycleNextWalVoteCandidateProjectionV1 {",
                    "projection.is_exact(verified)",
                ),
            )
            require_tokens(
                runtime_path,
                "runtime-private recovered next-WAL-Vote candidate mint",
                runtime_source,
                (
                    "fn project_recovered_lifecycle_next_wal_vote_candidate(",
                    "RecoveredLifecycleNextWalVoteCandidateProjectionPermitV1::new()",
                    "RecoveredWalCandidateProjectionPermit::new()",
                ),
            )
            require_tokens(
                wal_recovery_path,
                "WAL-bound recovered Broadcast-and-next-Sign projection",
                wal_recovery_source,
                (
                    "fn project_authenticated_signed_broadcast_and_sign(",
                    "next_sign.matches_verified_height(verified)",
                    "next_sign.matches_phase_vote_repair(self)",
                    "project_recovered_lifecycle_next_wal_vote_candidate(verified, next_sign)",
                    "combined.children_are_exact(verified)",
                ),
            )
            combined_cold_projection = region(
                wal_recovery_path,
                wal_recovery_source,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                "impl RecoveredLifecycleSignedBroadcastAndSignProjectionV1 {",
                "fn project_recovered_signed_broadcast(",
            )
            require_order(
                wal_recovery_path,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                combined_cold_projection,
                (
                    "self.cold_adapter_authority_minted",
                    "self.children_are_exact(verified)",
                    "self.next_sign.project_cold_adapter_next_sign(",
                    "RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1::from_recovered_wal(",
                    "self.cold_adapter_authority_minted = true",
                    "candidates.get(&self.broadcast.candidate.key) == Some(&self.broadcast.candidate)",
                    "self.next_sign.owns_spliced_candidate(candidates)",
                ),
            )
            reject_tokens(
                wal_recovery_path,
                "affine recovered Broadcast-and-next-Sign cold adapter projection",
                combined_cold_projection,
                (
                    "fn into_parts(",
                    "pub fn broadcast(",
                    "pub fn next_sign(",
                    "candidates.len() == 2",
                ),
            )
            next_vote_cold_projection = region(
                replay_authority_path,
                replay_authority_source,
                "sealed recovered next-WAL-Vote cold adapter projection",
                "pub(super) fn project_cold_adapter_next_sign(",
                "/// Return the exact installed effect digest",
            )
            require_order(
                replay_authority_path,
                "sealed recovered next-WAL-Vote cold adapter projection",
                next_vote_cold_projection,
                (
                    "RecoveredLifecycleSignBroadcastProjectionPermitV1",
                    "self.is_exact(verified)",
                    "self.seal.effect.clone()",
                ),
            )
            combined_cold_authority = region(
                adapter_path,
                adapter_source,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                "pub(in crate::sumeragi) struct RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1 {",
                "impl RecoveredLifecycleSignColdAdapterAuthorityV1",
            )
            require_tokens(
                adapter_path,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                combined_cold_authority,
                (
                    "broadcast: AdapterEffect",
                    "next_sign: AdapterEffect",
                    "RecoveredLifecycleSignBroadcastProjectionPermitV1",
                    "ConsensusMessageV2Payload::Proposal(proposal)",
                    "ConsensusMessageV2Payload::Vote(vote)",
                    "GlobalPhase::Prepare => tag.view() == next_vote.round.view",
                    "GlobalPhase::Commit => tag.view() >= next_vote.round.view",
                    "relation_is_exact.then_some(Self",
                ),
            )
            reject_tokens(
                adapter_path,
                "opaque recovered Broadcast-and-next-Sign cold adapter authority",
                combined_cold_authority,
                (
                    "fn into_parts(",
                    "fn broadcast(",
                    "fn next_sign(",
                    "impl Clone for RecoveredLifecycleSignBroadcastAndSignColdAdapterAuthorityV1",
                ),
            )
            combined_cold_adapter = region(
                adapter_path,
                adapter_source,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                "pub(in crate::sumeragi) fn advance_recovered_lifecycle_signed_broadcast_and_sign(",
                "/// Seal every adapter-owned input required by the adjacent gate open.",
            )
            require_order(
                adapter_path,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                combined_cold_adapter,
                (
                    "verified.verify_consensus_message(message)",
                    "adapter.reducer.awaiting_signature()",
                    "next_reducer.step(event.clone())",
                    "replayed_broadcast != broadcast",
                    "replayed_next_sign != next_sign",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                ),
            )
            reject_tokens(
                adapter_path,
                "recovered Broadcast-and-next-Sign cold adapter replay",
                combined_cold_adapter,
                ("publish_status", ".append(", "broadcast_consensus", "enqueue("),
            )
            combined_ledger_classifier = region(
                ledger_path,
                ledger_source,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                "pub(in crate::sumeragi) fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
                "/// Stage the exact all-row tombstone successor for CompleteTip retirement.",
            )
            require_tokens(
                ledger_path,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                combined_ledger_classifier,
                (
                    "self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?",
                    "let ledger_frame_identity = self.frame_identity()",
                    "RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records)",
                    "index.unique_parent_index(broadcast_ordinal)",
                    "index.owner_record_count(next_sign_owner) != 1",
                    "index.has_incoming_edge(next_sign_ordinal)",
                    "let next_sign_ordinal = broadcast_ordinal.checked_add(1)?",
                    "signed_broadcast_continuation_is_exact(",
                    "recovered_broadcast_and_next_sign_keys_are_exact(",
                    "next_sign_owner.first_admission_ordinal() != next_sign_ordinal",
                    "parent_record_count == 2",
                    "parent_record_count == 3",
                    "DurableContinuationEdge::ValidateToSignPrepare",
                    "ledger_frame_identity",
                ),
            )
            combined_ledger_enumerator = region(
                ledger_path,
                ledger_source,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                "pub(in crate::sumeragi) fn recovered_lifecycle_signed_broadcast_and_sign_pairs(",
                "fn project_recovered_lifecycle_signed_broadcast_and_sign_at(",
            )
            require_order(
                ledger_path,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                combined_ledger_enumerator,
                (
                    "self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?",
                    "let ledger_frame_identity = self.frame_identity()",
                    "RecoveredLifecycleSignedBroadcastAndSignLedgerIndexV1::new(&self.records)",
                    "self.records.iter()",
                    "project_validated_recovered_lifecycle_signed_broadcast_and_sign_at(",
                    "&index",
                ),
            )
            require_token_count(
                ledger_path,
                "linear recovered Broadcast-and-next-Sign ledger enumeration",
                combined_ledger_enumerator,
                "self.frame_identity()",
                1,
            )
            reject_tokens(
                ledger_path,
                "frame-bound recovered Broadcast-and-next-Sign ledger classifier",
                combined_ledger_classifier,
                ("high_water == next_sign_ordinal", "persist_exact_successor"),
            )
            combined_ledger_reauth = region(
                ledger_path,
                ledger_source,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                "pub(in crate::sumeragi) fn exactly_matches_ledger(&self, ledger: &LifecycleLedgerV1) -> bool {",
                "/// Complete version-one durable lifecycle ledger.",
            )
            require_tokens(
                ledger_path,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                combined_ledger_reauth,
                (
                    "project_recovered_lifecycle_signed_broadcast_and_sign_at(self.broadcast_ordinal)",
                    "== Some(self)",
                ),
            )
            reject_tokens(
                ledger_path,
                "single-hash recovered Broadcast-and-next-Sign ledger reauthentication",
                combined_ledger_reauth,
                ("ledger.frame_identity()",),
            )
            combined_registry_prepare = region(
                registry_path,
                registry_source,
                "opaque recovered Broadcast-and-next-Sign registry preparation",
                "pub(super) fn prepare_recovered_lifecycle_sign_broadcast_and_sign_successor<",
                "impl<'registry, 'adapter> PreparedRecoveredLifecycleSignBroadcastSuccessor",
            )
            require_order(
                registry_path,
                "opaque recovered Broadcast-and-next-Sign registry preparation",
                combined_registry_prepare,
                (
                    "adapter.dispatch_key() != key",
                    "sign.matches_claimed_record(",
                    "adapter.project_broadcast_and_sign_authority(body)",
                    ".project_authenticated_signed_broadcast_and_sign(verified, projection_authority)",
                    "PreparedRecoveredLifecycleSignBroadcastAndSignSuccessor {",
                ),
            )
            reject_tokens(
                registry_path,
                "unpublished recovered Broadcast-and-next-Sign registry preparation",
                combined_registry_prepare,
                (
                    "ValidatedBodyReceipt",
                    "into_parts",
                    "entries.insert",
                    "entries.remove",
                    "persist_exact_successor",
                ),
            )
            combined_transition = region(
                body_pipeline_path,
                body_pipeline_source,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                "fn stage_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                "#[allow(clippy::too_many_arguments, clippy::too_many_lines)]\nfn stage_body_stage_transition_with_payload_relation(",
            )
            require_order(
                body_pipeline_path,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                combined_transition,
                (
                    "stage_recovered_lifecycle_sign_broadcast_transition(coordinator, lease, broadcast)",
                    "first.child_ordinal.checked_add(1)",
                    "staged.reduce_admit(AdmissionRequest::Candidate(next_sign))",
                    "next_sign_owner == broadcast_owner",
                    "staged.high_water != next_sign_ordinal",
                    "capacity_generation_before[&CapacityClass::Effect].saturating_add(1)",
                    "capacity_used_before[&CapacityClass::Consensus].saturating_add(1)",
                    "Ok(StagedRecoveredLifecycleSignBroadcastAndSignTransition {",
                ),
            )
            reject_tokens(
                body_pipeline_path,
                "inert recovered Broadcast-and-next-Sign coordinator staging",
                combined_transition,
                (
                    "persist_exact_successor",
                    "commit_after_publication",
                    "registry.entries",
                ),
            )
            combined_transition_publication = region(
                body_pipeline_path,
                body_pipeline_source,
                "durable recovered Broadcast-and-next-Sign publication",
                "impl PreparedRecoveredLifecycleSignBroadcastAndSignTransition<'_, '_, '_> {",
                "fn map_sealed_successor_projection_error(",
            )
            require_order(
                body_pipeline_path,
                "durable recovered Broadcast-and-next-Sign publication",
                combined_transition_publication,
                (
                    "persist_exact_staged_successor(&self.staged)",
                    "successor.commit_after_publication()",
                    "*coordinator = staged",
                    "if publication_is_vote",
                    "ready_index.contains(&next_sign_ordinal)",
                    "adapter.commit_after_durable_vote_broadcast_and_sign()",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "Proposal publication parks only its durable Broadcast debt",
                combined_transition_publication,
                (
                    "ready_index.remove(&broadcast_ordinal)",
                    "LifecycleState::Waiting(broadcast_wait)",
                    "adapter.commit_after_durable_broadcast_and_sign()",
                ),
            )
            combined_transition_tail = combined_transition_publication.split(
                "successor.commit_after_publication()", 1
            )[-1]
            reject_tokens(
                body_pipeline_path,
                "infallible recovered Proposal two-child publication tail",
                combined_transition_tail,
                ("return", "is_err", "Result"),
            )
            combined_adapter_commit = region(
                adapter_path,
                adapter_source,
                "durable recovered Proposal adapter two-child commit",
                "pub(in crate::sumeragi) fn commit_after_durable_broadcast_and_sign(self)",
                "/// Borrow-bound adapter successor for one registry-owned recovered Apply",
            )
            require_order(
                adapter_path,
                "durable recovered Proposal adapter two-child commit",
                combined_adapter_commit,
                (
                    "RecoveredLifecycleSignAdapterSuccessorShapeV1::BroadcastAndSign",
                    "next_sign: Some(_)",
                    "combined_authority_minted: true",
                    "proposal_output_authority_minted: true",
                    "outbound_payload: Some(_)",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                ),
            )
            combined_vote_adapter_commit = region(
                adapter_path,
                adapter_source,
                "durable recovered Vote adapter two-child commit",
                "pub(in crate::sumeragi) fn commit_after_durable_vote_broadcast_and_sign(self)",
                "/// Borrow-bound adapter successor for one registry-owned recovered Apply",
            )
            require_order(
                adapter_path,
                "durable recovered Vote adapter two-child commit",
                combined_vote_adapter_commit,
                (
                    "self.is_vote_broadcast_and_sign()",
                    "next_sign: Some(_)",
                    "combined_authority_minted: true",
                    "proposal_output_authority_minted: false",
                    "outbound_payload: None",
                    "adapter.reducer = next_reducer",
                    "adapter.registry = next_registry",
                ),
            )
            require_tokens(
                registry_validate_path,
                "follow-on recovered WAL Vote remains an executable Sign carrier",
                registry_validate_source,
                (
                    "ConcreteLifecycleWorkKind::DurableRecoveredLifecycleNextWalVoteSign(sign)",
                    "PreparedRecoveredLifecycleSignCarrier::NextWalVote(sign)",
                ),
            )
            recovered_sign_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Sign-to-Broadcast settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_sign_broadcast(",
                "/// Settle a recovered Prepare Vote into Broadcast plus Commit Sign.",
            )
            require_order(
                launch_path,
                "restart-closed recovered Sign-to-Broadcast settlement",
                recovered_sign_settlement,
                (
                    "recovered_lifecycle_sign_completion.take()",
                    "prepare_recovered_lifecycle_sign_completion(authority)",
                    "prepare_recovered_lifecycle_sign_broadcast_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_transition(",
                    "output_guard.begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                ),
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Sign-to-Broadcast settlement",
                recovered_sign_settlement,
                (
                    "ProductionRecoveredLifecycleSignBroadcastSettlementV1::RestartRequired",
                    "ProductionRecoveredLifecycleSignBroadcastSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "durable recovered Sign-to-Broadcast settlement leaves output to its child",
                recovered_sign_settlement,
                (
                    "capture_recovered_lifecycle_signed_broadcast_refanout",
                    "output.commit_after_publication()",
                    "TurnOutcome::Terminal",
                ),
            )
            recovered_sign_tail = recovered_sign_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Sign-to-Broadcast post-fsync tail",
                recovered_sign_tail,
                ("return", "Result", "is_err"),
            )
            recovered_vote_two_child_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Vote Broadcast-and-next-Sign settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_vote_broadcast_and_sign(",
                "/// Fsync an initial Proposal `PrepareIntent`, then publish both successors.",
            )
            require_order(
                launch_path,
                "restart-closed recovered Vote Broadcast-and-next-Sign settlement",
                recovered_vote_two_child_settlement,
                (
                    "recovered_lifecycle_sign_completion.take()",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "preview.is_vote_broadcast_and_sign_shape()",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "output_guard.begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                    "ProductionRecoveredLifecycleVoteBroadcastAndSignSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "Vote settlement leaves durable output to typed refanout",
                recovered_vote_two_child_settlement,
                (
                    "project_proposal_exact_output_authority",
                    "capture_recovered_lifecycle_proposal_exact_output",
                    "output.commit_after_publication()",
                    "TurnOutcome::Terminal",
                ),
            )
            recovered_vote_two_child_tail = recovered_vote_two_child_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Vote two-child post-fsync tail",
                recovered_vote_two_child_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_proposal_two_child_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                "pub(in crate::sumeragi) fn settle_recovered_lifecycle_proposal_broadcast_and_sign(",
                "/// Refanout one durable recovered signed Broadcast",
            )
            require_order(
                launch_path,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                recovered_proposal_two_child_settlement,
                (
                    "recovered_lifecycle_sign_completion.take()",
                    "prepare_recovered_lifecycle_sign_completion_with_body(executor, authority)",
                    "preview.project_proposal_exact_output_authority()",
                    "capture_recovered_lifecycle_proposal_exact_output(output_authority)",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_successor(",
                    "prepare_recovered_lifecycle_sign_broadcast_and_sign_transition(",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "completion.acknowledge_after_publication()",
                    "output.commit_after_publication()",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::Applied",
                ),
            )
            require_token_count(
                launch_path,
                "typed recovered Proposal pre-fsync output release",
                recovered_proposal_two_child_settlement,
                "output.abort_before_publication()",
                2,
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Proposal Broadcast-and-next-Sign settlement",
                recovered_proposal_two_child_settlement,
                (
                    "RecoveredLifecycleProposalExactOutputCaptureV1::Unavailable(authority)",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::CapacityUnavailable",
                    "*recovered_lifecycle_sign_completion = Some(completion)",
                    "drop(output)",
                    "ProductionRecoveredLifecycleProposalBroadcastAndSignSettlementV1::RestartRequired",
                ),
            )
            recovered_proposal_two_child_tail = recovered_proposal_two_child_settlement.split(
                "transition.commit_after_publication();", 1
            )[-1]
            reject_tokens(
                launch_path,
                "infallible recovered Proposal two-child post-fsync tail",
                recovered_proposal_two_child_tail,
                ("return", "Result", "is_err", "?"),
            )
            recovered_broadcast_refanout = region(
                scheduler_path,
                scheduler_source,
                "restart-safe recovered signed-Broadcast refanout",
                "fn refanout_recovered_lifecycle_signed_broadcast_with_runner_debt(",
                "/// Sign, reserve, claim, and publish the sole recovered Decision Fetch",
            )
            require_order(
                scheduler_path,
                "restart-safe recovered signed-Broadcast refanout",
                recovered_broadcast_refanout,
                (
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "if exact_ready != self.coordinator.ready_index",
                    "work_class == LifecycleWorkClass::Broadcast",
                    "recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal",
                    "attest_ready_recovered_lifecycle_signed_broadcast",
                    "for ready_ordinal in &exact_ready",
                    "attest_ready_recovered_lifecycle_sign(",
                    "self.coordinator.plan_turn(inputs)",
                    "project_claimed_recovered_lifecycle_signed_broadcast_output",
                    "capture_recovered_lifecycle_signed_broadcast_refanout(authority)",
                    "let wait_source = super::WaitSource::Recovery(wait_digest)",
                    "settle_turn(lease, super::TurnOutcome::Blocked(wait))",
                    "output.commit_after_publication()",
                ),
            )
            require_tokens(
                scheduler_path,
                "restart-safe recovered signed-Broadcast refanout",
                recovered_broadcast_refanout,
                (
                    "rollback_unpublished_turn(&lease)",
                    "close_admission_for_restart()",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::CapacityUnavailable",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::RestartRequired",
                    "ProductionRecoveredLifecycleSignedBroadcastRefanoutV1::Refanned",
                    "attest_ready_recovered_lifecycle_signed_broadcast_and_next_vote(",
                ),
            )
            reject_tokens(
                scheduler_path,
                "volatile recovered signed-Broadcast refanout wait",
                recovered_broadcast_refanout,
                (
                    "persist_exact_successor",
                    "TurnOutcome::Terminal",
                    "exact_ready.len() == 2",
                    "exact_ready.len() != 2",
                ),
            )
            require_tokens(
                registry_validate_path,
                "retained recovered Broadcast-and-next-Vote pair seal",
                registry_validate_source,
                (
                    "fn recovered_lifecycle_signed_broadcast_declares_next_vote(",
                    "fn recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(",
                    "let (next, next_digest) = broadcast.paired_next_sign?",
                    "next.ordinal == broadcast_ordinal.checked_add(1)?",
                    "next_record.state == super::LifecycleState::Ready",
                    "next_record.owner == next.owner",
                    "next_record.physical_slots.get(&next.slot) == Some(&next_digest)",
                    "self.recovered_lifecycle_signed_broadcast_paired_next_vote_ordinal(",
                    ") != Some(next_sign_ordinal)",
                    "DurableRecoveredLifecycleNextWalVoteSign(next_sign)",
                ),
            )
            require_tokens(
                worker_path,
                "durable recovered signed-Broadcast service capture",
                worker_source,
                (
                    "fn capture_recovered_lifecycle_signed_broadcast_refanout(",
                    "authority.consume_for_service(RecoveredLifecycleSignBroadcastOutputPermitV1::new())",
                    "PendingExactFanout::claimed(",
                    "pending.can_enqueue(fanout)",
                    "fn capture_recovered_lifecycle_cold_proposal_message(",
                    "output.consume_for_service(RecoveredLifecycleProposalExactOutputPermitV1::new())",
                    "self.proposal_work_retired",
                    "pending.prepare_atomic_fanout_batch(fanouts)",
                    "cold_durable_proposal_refanout_atomically_owns_control_and_chunks",
                ),
            )
            require_tokens(
                ledger_path,
                "cold recovered signed-Broadcast ledger join",
                ledger_source,
                (
                    "fn authenticate_recovered_control_signed_broadcast(",
                    "fn authenticate_recovered_phase_signed_broadcast_repair(",
                    "project_recovered_signed_broadcast_child(self.context())",
                    "recover_durable_signed_broadcast(verified, child)",
                    "broadcast.exactly_matches_record(",
                ),
            )
            require_tokens(
                wal_recovery_path,
                "cold recovered signed-Broadcast WAL and roster join",
                wal_recovery_source,
                (
                    "fn recover_durable_signed_broadcast(",
                    "verified.verify_consensus_message(message)",
                    "fn project_cold_adapter_authority(",
                    "RecoveredLifecycleSignColdAdapterAuthorityV1::from_recovered_wal(",
                ),
            )
            require_tokens(
                adapter_path,
                "cold recovered signed-Broadcast reducer fast-forward",
                adapter_source,
                (
                    "fn advance_recovered_lifecycle_signed_broadcast(",
                    "verify_individual_signature(",
                    "let [reducer::Effect::Broadcast(message)] = core_effects.as_slice()",
                    "replayed != broadcast",
                    "next_reducer.pending_persistence_record().is_some()",
                    "next_reducer.awaiting_signature().is_some()",
                ),
            )
            require_literal_count(
                adapter_path,
                "cold recovered signed-Broadcast reducer fast-forward",
                adapter_source,
                '"Proposal cold replay requires its body and Prepare WAL successor"',
                2,
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered signed-Broadcast storage census",
                lifecycle_open_source,
                (
                    "PhaseBroadcast(",
                    "PhaseBroadcastAndSign(",
                    "PhaseBroadcastAndNextSign(",
                    "ControlBroadcast(",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_durable_fetch_startup",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_sign_and_durable_fetch_startup",
                    "assemble_storage_only_with_recovered_phase_broadcast_and_next_sign_and_durable_fetch_startup",
                    "assemble_storage_only_with_recovered_control_broadcast_and_durable_fetch_startup",
                ),
            )
            require_tokens(
                ledger_path,
                "cold recovered phase Broadcast-and-Sign ledger join",
                ledger_source,
                (
                    "fn authenticate_recovered_phase_signed_broadcast_and_sign(",
                    "combined.broadcast_exactly_matches(&broadcast)",
                    "combined.exactly_matches_fresh_records(",
                    "fn revalidates_recovered_phase_signed_broadcast_and_sign(",
                ),
            )
            require_tokens(
                registry_path,
                "cold recovered phase Broadcast-and-Sign registry join",
                registry_source,
                (
                    "fn prepare_cold_adapter_startup(",
                    "authenticate_recovered_lifecycle_next_vote_body(&mut preview)",
                    "project_authenticated_cold_signed_broadcast_and_sign(verified, seal)",
                    "authenticate_recovered_phase_signed_broadcast_and_sign(",
                    "advance_recovered_lifecycle_signed_broadcast_and_sign(",
                    "fn install_recovered_broadcast_and_next_vote(",
                    "paired_next_sign: Some((next_sign_address, next_sign_digest))",
                    "fn phase_broadcast_and_next_vote_projection(",
                    "owns_recovered_phase_broadcast_and_next_sign(",
                ),
            )
            require_tokens(
                adapter_path,
                "cold recovered phase owner handoff",
                adapter_source,
                (
                    "install_recovered_sign(&body_store)",
                    "prepare_cold_adapter_startup(&verified, adapter_startup, body_store)",
                ),
            )
            recovered_phase_broadcast_assembly = region(
                lifecycle_open_path,
                lifecycle_open_source,
                "cold recovered phase-Broadcast storage assembly",
                "fn assemble_storage_only_with_recovered_phase_broadcast_and_durable_fetch_startup(",
                "/// Assemble the exact standalone control Sign with every durable Fetch.",
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered phase-Broadcast storage assembly",
                recovered_phase_broadcast_assembly,
                (
                    "RecoveredWalStartupProjectionV1::PhaseBroadcast(projection, broadcast)",
                    "assemble_storage_only_with_terminal_validate_outcomes(",
                ),
            )
            recovered_control_broadcast_assembly = region(
                lifecycle_open_path,
                lifecycle_open_source,
                "cold recovered control-Broadcast storage assembly",
                "fn assemble_storage_only_with_recovered_control_broadcast_and_durable_fetch_startup(",
                "/// Assemble the standalone Decision Fetch with every durable body-backed Fetch.",
            )
            require_tokens(
                lifecycle_open_path,
                "cold recovered control-Broadcast storage assembly",
                recovered_control_broadcast_assembly,
                (
                    "RecoveredWalStartupProjectionV1::ControlBroadcast(control, broadcast)",
                    "assemble_storage_only_with_terminal_validate_outcomes(",
                ),
            )
            require_tokens(
                worker_path,
                "dedicated recovered Sign queue ownership",
                worker_source,
                (
                    "recovered_lifecycle_signs:",
                    "BTreeMap<RecoveredLifecycleSignDispatchKeyV1, V2IoTrackedRecoveredLifecycleSignV1>",
                    "fn transfer_recovered_lifecycle_sign_completion_at(",
                    "io.prepare_recovered_lifecycle_sign_completion(guarded, ownership_position)",
                    "fn recovered_lifecycle_signing_is_exact_and_class_sensitive_for_all_three_families()",
                    "fn recovered_lifecycle_sign_queue_retains_exact_owner_through_opaque_extraction()",
                    "fn recovered_lifecycle_sign_capacity_unavailable_leaves_no_dedicated_index()",
                ),
            )
            recovered_sign_capacity = region(
                worker_path,
                worker_source,
                "recovered Sign capacity capture release",
                "fn capture_recovered_lifecycle_sign_capacity<'a>(",
                "fn begin_decision_serve_reconciliation(",
            )
            require_token_count(
                worker_path,
                "recovered Sign capacity capture release",
                recovered_sign_capacity,
                "operation.complete()",
                5,
            )
            reject_tokens(
                worker_path,
                "recovered Sign capacity capture release",
                recovered_sign_capacity,
                ("drop(operation)",),
            )
            rollback_unpublished = region(
                owner_path,
                owner_source,
                "unpublished recovered Sign claim rollback",
                "fn rollback_unpublished_turn(&mut self, lease: &TurnLease) -> bool {",
                "/// Rebuild records after seeding the ordinal high-water mark.",
            )
            require_tokens(
                owner_path,
                "unpublished recovered Sign claim rollback",
                rollback_unpublished,
                (
                    "lease.output_reservation.is_some()",
                    "assert!( inserted,",
                    "self.active_lease = None",
                ),
            )
            reject_tokens(
                owner_path,
                "unpublished recovered Sign claim rollback",
                rollback_unpublished,
                ("debug_assert!",),
            )
            require_tokens(
                owner_path,
                "unpublished recovered Sign rollback regression",
                owner_source,
                (
                    "fn unpublished_turn_rollback_restores_ready_and_clears_the_active_lease()",
                ),
            )
            launched_owner_fields = region(
                launch_path,
                launch_source,
                "launched recovered Sign Drop order",
                "pub(in crate::sumeragi) struct LaunchedProductionLifecycleV1 {",
                "/// Result of draining one dedicated recovered Apply worker completion.",
            )
            require_order(
                launch_path,
                "launched recovered Sign Drop order",
                launched_owner_fields,
                (
                    "services: ProductionV2Services",
                    "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            recovered_fetch_dispatch = region(
                scheduler_path,
                scheduler_source,
                "lifecycle-owned recovered Decision Fetch dispatch",
                "fn dispatch_recovered_decision_fetch_with_runner_debt(",
                "/// Persist one selected recovered Decision Fetch response",
            )
            require_order(
                scheduler_path,
                "lifecycle-owned recovered Decision Fetch dispatch",
                recovered_fetch_dispatch,
                (
                    "attest_ready_recovered_decision_fetch",
                    "take_request_authority()",
                    "authenticate_recovered_decision_fetch_request(authority)",
                    "capture_recovered_decision_fetch_exact_output(&owner)",
                    "prepare_recovered_decision_fetch_request_registration(owner)",
                    "self.coordinator.plan_turn(inputs)",
                    "prepare_recovered_decision_fetch_dispatch",
                    "registration.commit(prepared)",
                    "output.commit()",
                ),
            )
            require_tokens(
                scheduler_path,
                "lifecycle-owned recovered Decision Fetch dispatch",
                recovered_fetch_dispatch,
                (
                    "services.matches_lifecycle_body_store(body_store_identity)",
                    "services.matches_lifecycle_executor_output_guard(executor)",
                    "ReadyRecoveredDecisionFetchDemandV1::ExactOutputAndExecutor",
                    "output.abort_before_claim()",
                    "self.coordinator.rollback_unpublished_turn(&lease)",
                    "assert_eq!(installed, dispatch_key)",
                ),
            )
            reject_tokens(
                scheduler_path,
                "sealed recovered Decision Fetch request dispatch",
                recovered_fetch_dispatch,
                (
                    "EffectWorkId",
                    "RuntimeEffectOwnership",
                    "PendingRuntimeEffectBinding",
                    "into_parts",
                    "settle",
                ),
            )
            recovered_fetch_phase_a = region(
                scheduler_path,
                scheduler_source,
                "recovered Decision Fetch response persistence Phase A",
                "fn persist_recovered_decision_fetch_response_after_runner(",
                "/// Plan, submit, and reblock one exact selected certified-Fetch response.",
            )
            require_order(
                scheduler_path,
                "recovered Decision Fetch response persistence Phase A",
                recovered_fetch_phase_a,
                (
                    "capture_lifecycle_capacity_rank(selector)",
                    "reservation.preflight_recovered_decision_fetch_target_absent()",
                    "executor.prepare_recovered_decision_fetch_body_persistence(prepared)",
                    "matches_claimed_dispatched_recovered_decision_fetch(",
                    "reservation.preflight_recovered_decision_fetch_body_persistence(&task)",
                    "executor.prepare_recovered_decision_fetch_response_claim(&task)",
                    "claim.commit_with_queue(reservation, task)",
                    "assert_eq!(self.coordinator.active_lease.as_ref(), Some(&lease))",
                ),
            )
            require_tokens(
                scheduler_path,
                "recovered Decision Fetch response persistence Phase A",
                scheduler_source,
                (
                    "runner.target() != LifecycleRunnerRankTarget::Ingress",
                    "ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignRunnerObservation",
                ),
            )
            require_tokens(
                effects_path,
                "recovered Decision Fetch foreign-cursor owner regression",
                effects_source,
                (
                    "fn lifecycle_selector_capture_censuses_competing_response_family_exactly_once()",
                    "owner.persist_recovered_decision_fetch_response(",
                    "Err(ProductionRecoveredDecisionFetchPersistenceErrorV1::ForeignRunnerObservation)",
                ),
            )
            require_literal_count(
                effects_path,
                "recovered Decision Fetch foreign-cursor owner regression",
                effects_source,
                '"a foreign Ingress cursor cannot change the recovered Fetch lease or registry row"',
                1,
            )
            require_tokens(
                launch_path,
                "recovered Decision Fetch source-order regression",
                launch_source,
                (
                    "fn recovered_decision_fetch_phase_a_rejects_foreign_ingress_cursor_before_mutation()",
                ),
            )
            recovered_fetch_ready = region(
                registry_validate_path,
                registry_validate_source,
                "closed Ready and claimed recovered Decision Fetch carrier",
                "pub(super) fn attest_ready_recovered_decision_fetch(",
                "/// Project a comparison-only seal for this exact registry instance.",
            )
            require_tokens(
                registry_validate_path,
                "closed Ready and claimed recovered Decision Fetch carrier",
                recovered_fetch_ready,
                (
                    "fetch.dispatch_key.is_some()",
                    "fetch.matches_current_ready_record(address, digest, coordinator)",
                    "RecoveredDecisionFetchDispatchIdentityV1::new(",
                    "project_recovered_decision_fetch_request(identity)",
                    "fn matches_claimed_dispatched_recovered_decision_fetch(",
                    "fetch.dispatch_key == Some(key)",
                    "fetch.matches_claimed_record(address, digest, coordinator, lease)",
                    "fn prepare_recovered_decision_fetch_dispatch(",
                ),
            )
            recovered_fetch_projection = region(
                wal_recovery_path,
                wal_recovery_source,
                "payload-free recovered Decision Fetch projection",
                "pub(super) fn project_recovered_decision_fetch_request(",
                "/// Prove the authenticated recovery cut retains this exact Fetch.",
            )
            require_tokens(
                wal_recovery_path,
                "payload-free recovered Decision Fetch projection",
                recovered_fetch_projection,
                (
                    "AdapterEffect::FetchBody {",
                    "manifest: None",
                    "certificate: Some(certificate)",
                    "RecoveredDecisionFetchRequestAuthorityV1::from_registry_projection(",
                ),
            )
            reject_tokens(
                wal_recovery_path,
                "payload-free recovered Decision Fetch projection",
                recovered_fetch_projection,
                ("EffectWorkId", "RuntimeEffectOwnership", "into_parts"),
            )
            recovered_fetch_registration = region(
                effects_path,
                effects_source,
                "dedicated recovered Decision Fetch request owner census",
                "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_request_registration(",
                "/// Take ownership of an exact-body store opened during sealed preflight.",
            )
            require_tokens(
                effects_path,
                "dedicated recovered Decision Fetch request owner census",
                recovered_fetch_registration,
                (
                    "self.validated_certified_request_presence().is_err()",
                    "self.outstanding_requests.len().checked_add(self.recovered_decision_fetches.len())",
                    "owner.conflicts_with_ordinary_tracker(&self.outstanding_requests)",
                    "owner.matches_body_coordinates(pending.task.round, pending.task.subject)",
                    "PreparedRecoveredDecisionFetchRequestRegistrationV1 { executor: self, owner: Some(owner), }",
                ),
            )
            require_tokens(
                effects_path,
                "complete recovered Decision Fetch request census and terminal fence",
                effects_source,
                (
                    "recovered_decision_fetches: BTreeMap<",
                    "recovered_decision_fetch_by_request: BTreeMap<",
                    "fn recovered_decision_fetch_request_index_is_exact_and_empty(&self) -> bool",
                    "self.recovered_decision_fetch_request_index_is_exact_and_empty()",
                    "fn validated_certified_request_presence(",
                    "Ok(!pending_hashes.is_empty() || !recovered_hashes.is_empty())",
                ),
            )
            ordinary_fetch_admission = region(
                effects_path,
                effects_source,
                "ordinary and recovered Decision Fetch coordinate fence",
                "fn begin_fetch<S: V2EffectServices>(",
                "fn retained_body_manifest_hash(",
            )
            require_tokens(
                effects_path,
                "ordinary and recovered Decision Fetch coordinate fence",
                ordinary_fetch_admission,
                (
                    "self.recovered_decision_fetches.values()",
                    "owner.matches_body_coordinates(round, subject)",
                ),
            )
            require_literal_count(
                effects_path,
                "ordinary and recovered Decision Fetch coordinate fence",
                ordinary_fetch_admission,
                '"body-fetch coordinates already have a recovered Decision Fetch owner"',
                1,
            )
            require_tokens(
                effects_path,
                "symmetric recovered Decision Fetch owner census",
                effects_source,
                (
                    "owner.matches_body_coordinates(pending.task.round, pending.task.subject)",
                    "fn recovered_decision_fetch_fences_later_ordinary_body_coordinates()",
                    "executor.validated_certified_request_presence()",
                ),
            )
            recovered_fetch_selector = region(
                selector_path,
                selector_source,
                "typed recovered Decision Fetch selector consumption",
                "pub(in crate::sumeragi) fn prepare_recovered_decision_fetch_body_persistence(",
                "/// Consume one exact selected family into a bounded body-store command.",
            )
            require_order(
                selector_path,
                "typed recovered Decision Fetch selector consumption",
                recovered_fetch_selector,
                (
                    "self.revalidate_recovered_decision_fetch_response_candidate(",
                    "PreparedCertifiedResponseCandidate::Recovered(candidate)",
                    "let authenticated = candidate.into_authenticated_response()",
                    "RecoveredDecisionFetchBodyPersistenceTaskV1 {",
                ),
            )
            require_tokens(
                selector_path,
                "typed recovered Decision Fetch selector target",
                selector_source,
                (
                    "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
                    "LifecycleIngressIoTargetKind::RecoveredDecisionFetchBodyPersistence",
                    "target.matches_recovered_decision_fetch_key(dispatch_key)",
                ),
            )
            recovered_fetch_claim = region(
                effects_path,
                effects_source,
                "recovered Decision Fetch response claim publication",
                "pub(in crate::sumeragi) fn commit_with_queue(",
                "impl RecoveredDecisionFetchResponseCandidateV1",
            )
            require_order(
                effects_path,
                "recovered Decision Fetch response claim publication",
                recovered_fetch_claim,
                (
                    "owner.matches_response_claim_preflight(response_hash, preflight)",
                    "owner.commit_exact_response_claim(response_hash)",
                    "queue.commit_recovered_decision_fetch_body_persistence(task)",
                ),
            )
            recovered_fetch_mixed_head = region(
                worker_path,
                worker_source,
                "recovered Decision Fetch mixed completion head fence",
                "fn take_io_completion(&mut self, runtime_capacity_available: bool)",
                "fn take_recovered_decision_apply_completion(",
            )
            require_order(
                worker_path,
                "recovered Decision Fetch mixed completion head fence",
                recovered_fetch_mixed_head,
                (
                    "let ownership_position =",
                    "io.completion_ownership_at(ownership_position)",
                    "owned.recovered_decision_fetch.is_some()",
                    "return IoCompletionTake::retained_runtime()",
                    "io.try_recv_completion_unacknowledged()",
                ),
            )
            require_tokens(
                worker_path,
                "dedicated recovered Decision Fetch worker ownership",
                worker_source,
                (
                    "PersistRecoveredDecisionFetchBody(RecoveredDecisionFetchBodyPersistenceTaskV1)",
                    "recovered_decision_fetch_bodies: BTreeMap<RecoveredDecisionFetchDispatchKeyV1, V2IoTrackedRecoveredDecisionFetchBodyV1>",
                    "V2IoCompletion::RecoveredDecisionFetchBodyPersisted",
                    "V2IoCompletionAcknowledgement::RecoveredDecisionFetchRetained",
                    "fn drain_recovered_decision_fetch_body_completion(",
                    "fn recovered_decision_fetch_queue_transitions_and_parks_until_dedicated_extraction()",
                ),
            )
            parked_fetch_completion = region(
                worker_path,
                worker_source,
                "opaque parked recovered Decision Fetch completion",
                "pub(in crate::sumeragi) struct PreparedRecoveredDecisionFetchBodyCompletionV1 {",
                "impl PreparedRecoveredLifecycleSignCompletionV1",
            )
            reject_tokens(
                worker_path,
                "opaque parked recovered Decision Fetch completion",
                parked_fetch_completion,
                (
                    "fn into_parts(",
                    "fn durable_receipt(",
                    "fn response(",
                    "fn acknowledge(",
                    "fn settle(",
                ),
            )
            recovered_fetch_settlement = region(
                launch_path,
                launch_source,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                "pub(in crate::sumeragi) fn settle_recovered_decision_fetch_store(",
                "/// Reserve, claim, and queue one recovered Sign",
            )
            require_order(
                launch_path,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                (
                    "prepare_lifecycle_ingress_selector(",
                    "prepare_recovered_decision_fetch_owner_retirement(",
                    "into_locked_recovered_decision_fetch_dequeue(",
                    "prepare_recovered_decision_fetch_store_adapter_authority(",
                    "prepare_recovered_decision_fetch_store_adapter(",
                    "prepare_recovered_decision_fetch_store_successor(",
                    "prepare_recovered_decision_fetch_store_transition(",
                    "begin_fail_stop_operation()",
                    "transition.persist_exact_successor().is_err()",
                    "transition.commit_after_publication()",
                    "commit_recovered_decision_fetch_owner_retirement(retirement)",
                    "locked_dequeue.commit()",
                    "completion.acknowledge_after_publication()",
                    "operation.complete()",
                ),
            )
            require_tokens(
                launch_path,
                "restart-closed recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                (
                    "*recovered_decision_fetch_body_completion = Some(completion)",
                    "owner.coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure)",
                    "ProductionRecoveredDecisionFetchStoreSettlementV1::RestartRequired",
                    "ProductionRecoveredDecisionFetchStoreSettlementV1::Applied",
                ),
            )
            reject_tokens(
                launch_path,
                "dedicated recovered Decision Fetch-to-Store settlement",
                recovered_fetch_settlement,
                ("EffectWorkId", "RuntimeEffectOwnership", "into_parts"),
            )
            require_tokens(
                worker_path,
                "recovered Decision Fetch worker acknowledgement tail",
                region(
                    worker_path,
                    worker_source,
                    "recovered Decision Fetch worker acknowledgement tail",
                    "fn acknowledge_recovered_decision_fetch_body(",
                    "fn prepare_certified_fetch_body_persistence_ack(",
                ),
                (
                    "fn acknowledge_recovered_decision_fetch_body(",
                    ".recovered_decision_fetch_bodies",
                    ".remove(&key)",
                ),
            )
            require_tokens(
                worker_path,
                "recovered Decision Fetch guarded acknowledgement tail",
                worker_source,
                (
                    "fn acknowledge_after_publication(mut self)",
                    "self.drop_guard.disarm()",
                ),
            )
            require_tokens(
                ledger_path,
                "recovered Decision Store cold restart and marker-prefix closure",
                ledger_source,
                (
                    "fn authenticate_recovered_decision_fetch_store(",
                    "fn open_recovered_decision_store_startup(",
                    "fn stage_recovered_decision_apply_projection(",
                    "successor_records_after_live_store(",
                    "fn recovered_decision_store_crash_prefix_restarts_once_then_stutters()",
                    "fn recovered_decision_store_restart_rejects_an_exact_child_key_collision()",
                ),
            )
            require_tokens(
                body_pipeline_path,
                "recovered Decision Fetch payload-free parent transition",
                body_pipeline_source,
                (
                    "fn stage_recovered_decision_fetch_store_transition(",
                    "DurablePayloadReference::None",
                    "DurableContinuationEdge::FetchToStore",
                    "BodyStagePayloadRelationV1::RecoveredDecisionFetch",
                    "fn persist_exact_successor(",
                    "fn commit_after_publication(self)",
                ),
            )
            require_tokens(
                adapter_path,
                "recovered Decision Store cold adapter reconstruction",
                adapter_source,
                (
                    "fn advance_recovered_decision_fetch_store(",
                    "project_store_adapter_authority(body)",
                    "project_decision_fetch_store(verified, projection_body, preview.store_effect())",
                    "preview.commit_after_durable_settlement()",
                ),
            )
            require_tokens(
                body_store_path,
                "recovered Decision Store body-frame reconstruction",
                body_store_source,
                (
                    "struct RecoveredDecisionFetchStoreBodyAuthorityV1",
                    "fn recovered_decision_fetch_store_body(",
                    "Ok(RecoveredDecisionFetchStoreBodyAuthorityV1 { manifest: manifest.clone(), durable: durable.clone(), })",
                ),
            )
            require_tokens(
                lifecycle_open_path,
                "typed recovered Decision Store storage census",
                lifecycle_open_source,
                (
                    "RecoveredWalStartupProjectionV1::DecisionStore",
                    "assemble_storage_only_with_recovered_decision_store_and_durable_fetch_startup",
                    "recovered_decision_store_chain_records(",
                ),
            )
            require_tokens(
                registry_validate_path,
                "dedicated recovered Decision Store registry install",
                registry_validate_source,
                (
                    "RecoveredWalRegistrySlotV1::DecisionStore",
                    "fn install_recovered_wal_decision_store<'registry>(",
                    "ConcreteLifecycleWorkKind::DurableRecoveredDecisionStore",
                ),
            )
            require_order(
                launch_path,
                "launched recovered Decision Fetch Drop order",
                launched_owner_fields,
                (
                    "services: ProductionV2Services",
                    "recovered_decision_fetch_body_completion: Option<PreparedRecoveredDecisionFetchBodyCompletionV1>",
                    "recovered_lifecycle_sign_completion: Option<PreparedRecoveredLifecycleSignCompletionV1>",
                    "leader_wire_ingress_binding: ProductionLeaderWireIngressBindingV1",
                ),
            )
            request_scoped_response = region(
                transport_path,
                transport_source,
                "request-scoped certified response authentication",
                "pub(in crate::sumeragi) fn authenticate_response(",
                "/// Certified-body response admitted for one outstanding exact request.",
            )
            require_tokens(
                transport_path,
                "request-scoped certified response authentication",
                request_scoped_response,
                (
                    "authenticate_certified_body_response_for_request(",
                    "response.validate_against(",
                    "verify_signature(",
                    "decode_framed_signed_block(&response.body)",
                    "AuthenticatedCertifiedBodyResponse { response }",
                ),
            )
            require_tokens(
                owner_path,
                "production lifecycle owner Kura/Apply seal",
                owner_binding.source if owner_binding is not None else "",
                (
                    "fn with_recovered_kura_binding_and_apply_service(",
                    "assert!(self.kura_binding.is_none())",
                    "assert!(self.apply_service.is_none())",
                    "self.kura_binding = Some(binding)",
                    "self.apply_service = Some(apply_service)",
                ),
            )
            require_token_count(
                adapter_path,
                "sole production lifecycle Kura/Apply binding",
                adapter_source,
                "owner.with_recovered_kura_binding_and_apply_service(kura_binding, apply_service)",
                1,
            )
            require_tokens(
                kura_path,
                "process-local Kura identity seal",
                kura_source,
                (
                    "instance_identity: Arc<KuraInstanceIdentityMarker>",
                    "struct KuraInstanceIdentity(Arc<KuraInstanceIdentityMarker>)",
                    "Arc::ptr_eq(&self.0, &kura.instance_identity)",
                    "Arc::ptr_eq(&self.0, &other.0)",
                    "fn instance_identity(&self) -> KuraInstanceIdentity",
                    "fn instance_identity_names_only_the_exact_live_kura()",
                    "store_root_directory: BoundProgressDirectory",
                    "Self::open_safety_wal_store_root_directory(&store_root, &store_root_lock_file)?",
                ),
            )
        payload_store_path, payload_store_source = load(
            "crates/iroha_core/src/sumeragi/v2_certified_serve_payload_store.rs"
        )
        coordinator_path, coordinator_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_coordinator.rs"
        )
        if payload_store_source and lifecycle_open_source and coordinator_source:
            retirement_authenticate = _require_qualified_rust_item(
                payload_store_path,
                payload_store_source,
                "CertifiedServePayloadRecoveryCut",
                "authenticate_for_complete_tip_retirement",
                errors,
                "CompleteTip retirement-only payload authentication",
            )
            require_tokens(
                payload_store_path,
                "CompleteTip retirement-only payload authentication",
                retirement_authenticate.source
                if retirement_authenticate is not None
                else "",
                ("self.authenticate_inner(verified, local_signer, None)",),
            )
            authenticate_inner = _require_qualified_rust_item(
                payload_store_path,
                payload_store_source,
                "CertifiedServePayloadRecoveryCut",
                "authenticate_inner",
                errors,
                "CompleteTip body-independent Completed metadata authority",
            )
            require_order(
                payload_store_path,
                "CompleteTip body-independent Completed metadata authority",
                authenticate_inner.source if authenticate_inner is not None else "",
                (
                    "PersistedCertifiedServePayloadStateV1::Completed",
                    "usize::try_from(persisted_responder)",
                    ".filter(|index| *index < context.roster.len())",
                    ".certificate .signers .binary_search(&persisted_responder)",
                    "return Err(CertifiedServePayloadRecoveryError::InvalidResponse(",
                    "let responder_peer = &context.roster[responder_index].validator",
                    "manifest.validate(context)",
                ),
            )
            payload_census = region(
                payload_store_path,
                payload_store_source,
                "CompleteTip Serve payload directory census",
                "fn reload_payload_census_strict(",
                "\n    /// Verify that a post-authentication startup cut still covers the complete",
            )
            require_order(
                payload_store_path,
                "CompleteTip Serve payload directory census",
                payload_census,
                (
                    "fs::symlink_metadata(&self.directory)",
                    "directory_metadata.file_type().is_symlink() || !directory_metadata.is_dir()",
                    "self.max_entries.checked_mul(2)",
                    "fs::read_dir(&self.directory)",
                    "fs::symlink_metadata(&path)",
                    "metadata.file_type().is_symlink() || !metadata.is_file()",
                    "!has_canonical_hash_name(name, FILE_SUFFIX)",
                    "payloads.len() >= self.max_entries",
                    "self.load_path(&path, metadata.len())?",
                    "self.path_for(payload.id()) != path",
                    "payloads.insert(payload.id(), payload).is_some()",
                    "Ok(payloads)",
                ),
            )
            require_tokens(
                payload_store_path,
                "CompleteTip body-independent Completed metadata authority",
                payload_store_source,
                (
                    "fn authenticate_for_complete_tip_retirement(",
                    ".certificate .signers .binary_search(&persisted_responder)",
                    "body_revalidated: body_store.is_some()",
                    "fn permits_payload_store_ahead_terminal_rebind(&self) -> bool",
                    "fn retirement_rejects_completed_metadata_from_a_noncertified_responder()",
                    "fn reload_payload_census_strict(",
                    "let observed = self.reload_payload_census_strict()?",
                    "observed_ids != self.indexed || cut_ids != observed_ids",
                    "fn authenticated_cut_rejects_a_later_valid_payload_from_a_second_store_owner()",
                    "fn authenticated_cut_rejects_store_directory_symlink_replacement()",
                ),
            )
            if authenticate_inner is not None:
                require_literal_count(
                    payload_store_path,
                    "CompleteTip body-independent Completed metadata authority",
                    authenticate_inner.source,
                    '"persisted response signer lost certified local retention authority"',
                    1,
                )
            require_tokens(
                lifecycle_open_path,
                "CompleteTip bodyless completion promotion guard",
                lifecycle_open_source,
                (
                    "completed.permits_payload_store_ahead_terminal_rebind()",
                    "pub(super) fn into_serve_payloads(self)",
                    "authenticated_serve_payloads_match_ledger(",
                ),
            )
            require_tokens(
                coordinator_path,
                "production lifecycle owner retained Serve census",
                coordinator_source,
                (
                    "struct ProductionLifecycleOwnerV1",
                    "serve_payloads: crate::sumeragi::v2_certified_serve_payload_store::AuthenticatedCertifiedServePayloadRecoveryCut",
                    "fn run_complete_tip_retirement_release_regressions()",
                    "ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_survives_completed_serve_body_cleanup_with_live_work()",
                    "ledger::tests::durable_ready_fetch_recovery::complete_tip_retirement_binds_only_the_exact_unlaunched_successor_owner()",
                ),
            )
        snapshot_authority = region(
            recovery_path,
            recovery_source,
            "SnapshotSuccessorActivationAuthority::new",
            "fn new(record: &wire::SnapshotV2BootstrapRecord) -> Self",
            "\n    /// Imported snapshot height which anchors the first executable context.",
        )
        require_tokens(
            recovery_path,
            "SnapshotSuccessorActivationAuthority::new",
            snapshot_authority,
            (
                "record.context.snapshot_bootstrap.as_ref()",
                "expect(\"verified snapshot activation authority retains its anchor\")",
                "record_hash: HashOf::new(record), snapshot_height: anchor.snapshot_height, snapshot_block_hash: anchor.snapshot_block_hash, successor_context_id: record.context.id(),",
            ),
        )
        recovery = region(
            recovery_path,
            recovery_source,
            "recover_active_height_with_plan",
            "pub(crate) fn recover_active_height_with_plan(",
            "\nfn verify_state_kura_prefix(",
        )
        require_tokens(
            recovery_path,
            "recover_active_height_with_plan snapshot authority",
            recovery,
            (
                "authenticate_v2_snapshot_replay_boundary(kura, state, &replay_plan)?;",
                "if record.context() != &bootstrap.context || record.proofs_of_possession() != bootstrap.validator_set_pops",
                "let verified_context = VerifiedHeightContext::snapshot_bootstrap(bootstrap)?;",
                "RecoveredSuccessorActivationAuthority::SnapshotBootstrap( SnapshotSuccessorActivationAuthority::new(bootstrap), )",
            ),
        )
        require_order(
            recovery_path,
            "recover_active_height_with_plan snapshot authority",
            recovery,
            (
                "authenticate_v2_snapshot_replay_boundary(",
                "is_entirely_audited_snapshot_import()",
                "authenticated_snapshot_v2_bootstrap()",
                "record.context() != &bootstrap.context",
                "VerifiedHeightContext::snapshot_bootstrap(bootstrap)",
                "SnapshotSuccessorActivationAuthority::new(bootstrap)",
            ),
        )
        require_tokens(
            recovery_path,
            "recover_active_height_with_plan complete-tip authority",
            recovery,
            (
                "kura.v2_finality_artifact_with_receipt(durable_height)?",
                "let predecessor_record = context_store.load(durable_height)?",
                "let verified_predecessor = verify_persisted_height( kura, state, &context_store, predecessor_record, durable_height, )?;",
                "let predecessor_signature_policy = if durable_height == 1 { BlockSignaturePolicy::GenesisAuthority(genesis_public_key.clone()) } else { BlockSignaturePolicy::RotatingLeader };",
                "build_verified_successor(state, &context_store, &parent_artifact, &parent_receipt)?;",
                "let (verified_context, activation) = successor.into_parts();",
                "RecoveredCompleteTipActivationAuthority::authenticate( parent_artifact, parent_receipt, verified_predecessor, predecessor_signature_policy, &verified_context, activation, kura, )?;",
                "RecoveredSuccessorActivationAuthority::CompleteTip( complete_tip_activation, )",
            ),
        )
        require_order(
            recovery_path,
            "recover_active_height_with_plan complete-tip authority",
            recovery,
            (
                "verify_persisted_height(",
                "build_verified_successor(",
                "successor.into_parts()",
                "RecoveredCompleteTipActivationAuthority::authenticate(",
                "RecoveredSuccessorActivationAuthority::CompleteTip(",
            ),
        )
        verified_successor = region(
            recovery_path,
            recovery_source,
            "build_verified_successor",
            "pub(crate) fn build_verified_successor(",
            "\nfn verify_persisted_height(",
        )
        require_tokens(
            recovery_path,
            "build_verified_successor",
            verified_successor,
            (
                "DurableV2PredecessorIdentity::authenticate(parent_artifact, parent_receipt)?;",
                "if state_height != parent_height || state_block_hash != Some(predecessor.block_hash)",
                "if parent_record.context() != &parent_artifact.height_context",
                "VerifiedHeightContext::successor( expected, proofs, parent_artifact, parent_receipt, parent_record.proofs_of_possession(), )?;",
                "DurableSuccessorActivationAuthority { predecessor, successor_context_id: verified.context().id(), }",
                "DurableSuccessorActivationAuthority { predecessor, successor_context_id: verified_context.context().id(), }",
            ),
        )
        require_order(
            recovery_path,
            "build_verified_successor",
            verified_successor,
            (
                "DurableV2PredecessorIdentity::authenticate(",
                "state_height != parent_height",
                "parent_record.context() != &parent_artifact.height_context",
                "VerifiedHeightContext::successor(",
                "DurableSuccessorActivationAuthority",
            ),
        )
    adapter_path, adapter_source = _read_reviewed_rust_source(
        repo_root,
        "crates/iroha_core/src/sumeragi/v2.rs",
        errors,
        "production successor-refinement source",
    )
    if adapter_source:
        adapter_test_context = (
            ("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),
        )
        for test_name, expected_sha256 in (
            _SUCCESSOR_PARENT_BINDING_TEST_SHA256.items()
        ):
            test_item = _require_rust_item(
                adapter_path, adapter_source, test_name, errors
            )
            if test_item is not None:
                expected_attributes = (
                    ("#[test]",)
                    if test_name
                    == "successor_core_context_preserves_the_parent_certificate_binding"
                    else ('#[cfg(feature = "bls")]', "#[test]")
                )
                delimiter_context = tuple(
                    (opener, header)
                    for opener, _position, header in test_item.delimiter_context
                )
                expected_delimiters = tuple(
                    ("{", header) for header in adapter_test_context
                )
                if (
                    test_item.brace_context != adapter_test_context
                    or delimiter_context != expected_delimiters
                    or test_item.ancestor_inner_attributes
                    or test_item.attributes != expected_attributes
                ):
                    errors.append(
                        f"{adapter_path}:{test_item.line}: successor parent-QC "
                        f"regression {test_name} must remain the exact reviewed "
                        "unit-test item in the adapter tests module"
                    )
            _require_rust_item_token_sha256(
                adapter_path,
                test_item,
                expected_sha256,
                f"successor parent-QC regression {test_name}",
                errors,
            )
        core_context = region(
            adapter_path,
            adapter_source,
            "WireRegistry::core_context",
            "    fn core_context(\n",
            "\n    fn validator_id(",
        )
        require_tokens(
            adapter_path,
            "WireRegistry::core_context",
            core_context,
            (
                ".map(|certificate| self.register_parent_qc(certificate))",
                "reducer::HeightContext::new( context_id, network_id, context.height, parent_commit,",
            ),
        )
        require_order(
            adapter_path,
            "WireRegistry::core_context",
            core_context,
            (
                "self.register_parent_qc(certificate)",
                "reducer::HeightContext::new(",
            ),
        )
        parent_reference = region(
            adapter_path,
            adapter_source,
            "WireRegistry::qc_reference_to_core_for_context",
            "    fn qc_reference_to_core_for_context(\n",
            "\n    /// Register the predecessor CommitQC frozen into a successor context.",
        )
        require_tokens(
            adapter_path,
            "WireRegistry::qc_reference_to_core_for_context",
            parent_reference,
            (
                "reference.round.context_id != expected_context_id",
                "reference.proposal_round.context_id != expected_context_id",
                "reference.proposal_round.height != reference.round.height",
                "reference.proposal_round != reference.round",
                "self.register_execution_commitment( proposal_round, subject, reference.execution_commitment, )?;",
                "reducer::CertificateRef::new_with_proposal_round( context_id(reference.round.context_id), round, proposal_round, Self::phase_to_core(reference.phase), subject, )",
            ),
        )
        require_order(
            adapter_path,
            "WireRegistry::qc_reference_to_core_for_context",
            parent_reference,
            (
                "if reference.round.context_id != expected_context_id",
                "if reference.proposal_round != reference.round",
                "self.register_execution_commitment(",
                "reducer::CertificateRef::new_with_proposal_round(",
            ),
        )
        parent_registration = region(
            adapter_path,
            adapter_source,
            "WireRegistry::register_parent_qc",
            "    fn register_parent_qc(\n",
            "\n    fn qc_to_core(",
        )
        require_tokens(
            adapter_path,
            "WireRegistry::register_parent_qc",
            parent_registration,
            (
                ".wire_context .as_ref() .and_then(|context| context.parent_commit_qc.as_ref()) .map(wire::QuorumCertificate::as_ref) .ok_or(AdapterError::ParentContextMismatch)?;",
                "if !reference.same_commit_decision(frozen)",
                "return Err(AdapterError::ParentContextMismatch);",
                "let core = self.qc_reference_to_core_for_context( &reference, frozen.round.context_id )?;",
                "self.certificates.insert(core, certificate.clone());",
                "Ok(core)",
            ),
        )
        require_order(
            adapter_path,
            "WireRegistry::register_parent_qc",
            parent_registration,
            (
                "context.parent_commit_qc.as_ref()",
                "reference.same_commit_decision(frozen)",
                "self.qc_reference_to_core_for_context(",
                "self.certificates.insert(",
                "Ok(core)",
            ),
        )
        proposal_justification = region(
            adapter_path,
            adapter_source,
            "WireRegistry::justification_to_core",
            "    fn justification_to_core(\n",
            "\n    fn justification_to_wire(",
        )
        require_tokens(
            adapter_path,
            "WireRegistry::justification_to_core",
            proposal_justification,
            (
                "wire::ProposalJustification::ParentCommit(parent)",
                ".map(|certificate| self.register_parent_qc(certificate))",
                "reducer::ProposalJustification::ParentCommit(reference)",
            ),
        )
        require_order(
            adapter_path,
            "WireRegistry::justification_to_core",
            proposal_justification,
            (
                "wire::ProposalJustification::ParentCommit(parent)",
                "self.register_parent_qc(certificate)",
                "reducer::ProposalJustification::ParentCommit(reference)",
            ),
        )
        parent_authority = region(
            adapter_path,
            adapter_source,
            "verify_proposal_justification_authority",
            "fn verify_proposal_justification_authority(\n",
            "\n/// Reauthenticate every external authority proof embedded",
        )
        require_tokens(
            adapter_path,
            "verify_proposal_justification_authority",
            parent_authority,
            (
                "(Some(certificate), Some(parent_verification)) => verify_quorum_certificate( &parent_verification.context, certificate, &parent_verification.proofs_of_possession, )",
                "(None, None) | (None, Some(_)) | (Some(_), None) => { Err(AdapterError::ParentContextMismatch) }",
            ),
        )
        require_order(
            adapter_path,
            "verify_proposal_justification_authority",
            parent_authority,
            (
                "wire::ProposalJustification::ParentCommit(parent)",
                "verify_quorum_certificate(",
                "&parent_verification.context",
                "&parent_verification.proofs_of_possession",
            ),
        )
        authenticated_ingress = region(
            adapter_path,
            adapter_source,
            "verify_authenticated_message",
            "fn verify_authenticated_message(\n",
            "\nfn verify_roster_proofs(",
        )
        require_tokens(
            adapter_path,
            "verify_authenticated_message",
            authenticated_ingress,
            (
                "wire::ConsensusMessageV2Payload::Proposal(proposal)",
                "proposal.validate(context)?;",
                "verify_individual_signature( context, proposal.proposer, &proposal.signature, &proposal.signature_preimage(), )?;",
                "verify_proposal_justification_authority( context, parent_verification, &proposal.justification, proofs_of_possession, )",
            ),
        )
        require_order(
            adapter_path,
            "verify_authenticated_message",
            authenticated_ingress,
            (
                "proposal.validate(context)",
                "verify_individual_signature(",
                "verify_proposal_justification_authority(",
            ),
        )
        deferred_open = region(
            adapter_path,
            adapter_source,
            "open_deferred_status",
            "pub(crate) fn open_deferred_status(",
            "\n    #[allow(clippy::too_many_arguments)]\n    fn open_with_aggregator(",
        )
        require_tokens(
            adapter_path,
            "open_deferred_status",
            deferred_open,
            ("Self::open_with_aggregator_and_publication(", "false,"),
        )
        marker = region(
            adapter_path,
            adapter_source,
            "successor_activation_status",
            "pub(crate) fn successor_activation_status(",
            "\n    fn liveness_status(",
        )
        require_order(
            adapter_path,
            "successor_activation_status",
            marker,
            (
                "SumeragiV2ProgressTransition::SuccessorHeightActivated",
                "self.status()",
            ),
        )
    runtime_path, runtime_source = load(
        "crates/iroha_core/src/sumeragi/v2_runtime.rs"
    )
    if runtime_source:
        snapshot = region(
            runtime_path,
            runtime_source,
            "successor_activation_status_snapshot",
            "pub(crate) fn successor_activation_status_snapshot(",
            "\n    fn body_pipeline_completion_is_owned(",
        )
        require_order(
            runtime_path,
            "successor_activation_status_snapshot",
            snapshot,
            (
                "if !self.clocks_armed",
                "AdapterError::SuccessorClocksNotArmed",
                "self.driver.successor_activation_status()",
            ),
        )
    block_sync_path, block_sync_source = load(
        "crates/iroha_core/src/sumeragi/v2_block_sync.rs"
    )
    if block_sync_source:
        enqueue = region(
            block_sync_path,
            block_sync_source,
            "enqueue_and_complete",
            "pub(crate) fn enqueue_and_complete<",
            "\n    /// Number of bounded outstanding requests.",
        )
        require_order(
            block_sync_path,
            "enqueue_and_complete",
            enqueue,
            (
                "let message = discovered.message()",
                "enqueue(message.clone())",
                "admission.matches(&message)",
                "self.complete(discovered)",
            ),
        )
        historical = region(
            block_sync_path,
            block_sync_source,
            "build_historical_body_response",
            "fn build_historical_body_response(",
            "\nfn ensure_key_identity(",
        )
        require_order(
            block_sync_path,
            "build_historical_body_response",
            historical,
            (
                "kura.v2_finality_artifact(height)?",
                "let context = &artifact.height_context",
                "let proofs_of_possession = &artifact.validator_set_pops",
                "authenticate_certified_body_request_with_validator_pops(",
                "let request = authenticated.request()",
                "request.subject != artifact.subject",
                "let Some(responder_position)",
                ".position(|entry| entry.validator == responder_peer)",
                "return Ok(None);",
                "kura\n        .get_block(block_height)",
                "block.hash() != request.subject.block_hash",
                "block.canonical_resultless_proposal()",
                *HISTORICAL_BODY_RESPONSE_PHASE_MARKERS,
                "encode_payload(",
                "Signature::new(responder_key.private_key(), &response.signature_preimage())",
                "response.validate_against(",
            ),
        )
    effects_path, effects_source = load(
        "crates/iroha_core/src/sumeragi/v2_effects.rs"
    )
    if effects_source:
        require_tokens(
            effects_path,
            "recovered Sign foreign-cursor owner regression",
            effects_source,
            (
                "owner.dispatch_recovered_lifecycle_sign(",
                "Err(ProductionRecoveredLifecycleSignDispatchErrorV1::ForeignRunnerObservation)",
            ),
        )
        require_literal_count(
            effects_path,
            "recovered Sign foreign-cursor owner regression",
            effects_source,
            '"a non-Completion runner cursor cannot claim or mutate a recovered Sign owner"',
            1,
        )
        certified = region(
            effects_path,
            effects_source,
            "accept_certified_body_response",
            "pub(crate) fn accept_certified_body_response<",
            "\n    /// Accept a durable application completion",
        )
        require_order(
            effects_path,
            "accept_certified_body_response",
            certified,
            (
                "self.outstanding_requests.authenticate_response(",
                "ReadyBody::derive(",
                "self.plan_fetch_completion(",
                "services.complete_certified_body_fetch(",
                "self.commit_fetch_completion(plan)",
            ),
        )
        consume = region(
            effects_path,
            effects_source,
            "consume_one",
            "fn consume_one<",
            "\n    fn ensure_pending_tip_recovery_effect_is_local(",
        )
        require_order(
            effects_path,
            "consume_one body pipeline",
            consume,
            (
                "AdapterEffect::FetchBody",
                "AdapterEffect::StoreBody",
                "AdapterEffect::ValidateBody",
                "AdapterEffect::Apply",
            ),
        )
    release_path = repo_root / "scripts" / "run_sumeragi_v2_release_gates.sh"
    if not release_path.is_file() or release_path.is_symlink():
        errors.append(
            f"{release_path}: production successor release inventory must be a "
            "regular source file"
        )
        release_source = ""
    else:
        try:
            release_source = release_path.read_text(encoding="utf-8")
        except (OSError, UnicodeDecodeError) as error:
            errors.append(
                f"{release_path}: cannot read production successor release "
                f"inventory: {error}"
            )
            release_source = ""
    if release_source:
        for test in (
            "sumeragi::v2_block_sync::tests::discovery_outputs_only_normal_commit_qc_ingress_and_waits_for_enqueue",
            "sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts",
            "sumeragi::v2_block_sync::tests::historical_body_uses_self_contained_kura_finality_without_context_store",
            "sumeragi::v2_runtime::tests::successor_activation_snapshot_requires_armed_live_clocks",
            "sumeragi::v2_runner::tests::successor_activation_is_published_only_after_ingress_is_open",
            "sumeragi::v2_runner::tests::complete_tip_recovery_requires_authenticated_predecessor_retirement",
            "sumeragi::status::v2_liveness_watchdog_tests::complete_tip_retirement_and_successor_owner_bind_are_release_bound",
            "sumeragi::v2_runner::tests::successor_startup_failure_stays_running_and_fails_closed_without_activation",
        ):
            if release_source.count(f"  {test}\n") != 1:
                errors.append(
                    f"{release_path}: production refinement test must be pinned exactly once: {test}"
                )
    return errors
