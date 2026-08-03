def _successor_production_source_fidelity_errors(repo_root: Path) -> list[str]:
    """Bind indexed successor and exact-recovery actions to production order."""

    errors: list[str] = []

    def load(relative: str) -> tuple[Path, str]:
        path = repo_root / relative
        if not path.is_file():
            errors.append(f"{path}: missing production successor-refinement source")
            return path, ""
        return path, path.read_text(encoding="utf-8")

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
    recovered_successor_activation,
    mut staged_genesis_nexus_amx_context,
) = recovered.into_parts();
let mut eager_block_sync =
    recovered_successor_activation.is_some() || pending_kura_apply.is_some();
""",
            "durable recovered ownership must initialize eager block-sync",
            errors,
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
let (receipt, artifact, exact_output_handoff) = finality;
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
                "super::status::activate_recovered_v2_successor_height(authority, successor)?;",
                "super::status::activate_snapshot_bootstrap_v2_height(authority, successor)?;",
            ),
        )
        require_order(
            runner_path,
            "PendingSuccessorActivation::recovered",
            activation,
            (
                "match &authority",
                "let published_height = super::status::v2_status()",
                "ProductionSuccessorStartupLifecycleProjection",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return Err(V2RunnerError::SuccessorRefinementRejected)",
                "let _authorized_lifecycle = checked_lifecycle.into_projection()",
                "Ok(match authority",
            ),
        )
        reject_tokens(
            runner_path,
            "PendingSuccessorActivation::recovered",
            activation,
            (
                "production_startup_failure_and_restart_refines_indexed_lifecycle_kernel(",
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
                "block_ingress.open()",
                "ingress_ready.store(true, Ordering::Release)",
                "activation.publish(successor)",
                "close_ingress_for_rollover(ingress_ready, block_ingress)",
            ),
        )
        run_inner = run_inner_item.source if run_inner_item is not None else ""
        require_tokens(
            runner_path,
            "run_inner recovery ownership",
            run_inner,
            (
                "let mut pending_successor_activation = recovered_successor_activation .map(PendingSuccessorActivation::recovered) .transpose()?;",
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
        require_tokens(
            runner_path,
            "historical ingress routing",
            region(
                runner_path,
                runner_source,
                "drain_v2_ingress",
                "fn drain_v2_ingress(",
                "\n#[derive(Clone, Copy, Debug, PartialEq, Eq)]\nenum OuterIngressTurn",
            ),
            (
                "block_sync_server.serve_historical_body( kura, request, &sender, local_key )",
                "executor.accept_certified_body_response_with_ingress_ownership( response, &sender, &ingress_ownership, services, )",
                "block_sync.authenticate_response(response, &sender)",
                "block_sync.enqueue_and_complete(discovered, |message| { executor.enqueue_discovered_commit_certificate(message, ingress_ownership) })",
            ),
        )

    status_path, status_source = load(
        "crates/iroha_core/src/sumeragi/status.rs"
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
            "\nfn activate_recovered_v2_successor_height_at(",
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
        recovered_wrapper = region(
            status_path,
            status_source,
            "activate_recovered_v2_successor_height_at",
            "fn activate_recovered_v2_successor_height_at(",
            "\nfn publish_recovered_v2_successor_height_at(",
        )
        require_tokens(
            status_path,
            "activate_recovered_v2_successor_height_at",
            recovered_wrapper,
            (
                "let (predecessor, expected_successor_context_id) = authority.into_parts();",
                "SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP",
                "predecessor.refinement_projection()",
                "CanonicalIdentityProjection::zero(), 0, CanonicalIdentityProjection::zero(),",
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
        snapshot = region(
            status_path,
            status_source,
            "activate_snapshot_bootstrap_v2_height",
            "pub(crate) fn activate_snapshot_bootstrap_v2_height(",
            "\n/// Register the live bounded transport-to-runner ingress",
        )
        require_tokens(
            status_path,
            "activate_snapshot_bootstrap_v2_height",
            snapshot,
            (
                "authority.into_parts()",
                "SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP",
                "ProductionDurablePredecessorIdentityProjection::default()",
                "snapshot_record_refinement_projection(snapshot_record_hash)",
                "successor_block_refinement_projection(snapshot_block_hash)",
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

    recovery_path, recovery_source = load(
        "crates/iroha_core/src/sumeragi/v2_recovery.rs"
    )
    if recovery_source:
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
                "build_verified_successor(state, &context_store, &parent_artifact, &parent_receipt)?;",
                "RecoveredSuccessorActivationAuthority::CompleteTip( activation, )",
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

    adapter_path, adapter_source = load(
        "crates/iroha_core/src/sumeragi/v2.rs"
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
                "reducer::HeightContext::new( context_id, chain_id, context.height, parent_commit,",
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
                "authenticate_certified_body_request(",
                "verify_historical_quorum_certificate(",
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

    release_path, release_source = load(
        "scripts/run_sumeragi_v2_release_gates.sh"
    )
    if release_source:
        for test in (
            "sumeragi::v2_block_sync::tests::discovery_outputs_only_normal_commit_qc_ingress_and_waits_for_enqueue",
            "sumeragi::v2_block_sync::tests::catch_up_is_strictly_sequential_across_contexts",
            "sumeragi::v2_block_sync::tests::historical_body_comes_from_kura_and_a_non_signer_archive_can_serve",
            "sumeragi::v2_runtime::tests::successor_activation_snapshot_requires_armed_live_clocks",
            "sumeragi::v2_runner::tests::successor_activation_is_published_only_after_ingress_is_open",
            "sumeragi::v2_runner::tests::complete_tip_recovery_uses_the_same_live_successor_boundary",
            "sumeragi::v2_runner::tests::successor_startup_failure_stays_running_and_fails_closed_without_activation",
        ):
            if release_source.count(f"  {test}\n") != 1:
                errors.append(
                    f"{release_path}: production refinement test must be pinned exactly once: {test}"
                )
    return errors


def _successor_stale_token_mutation_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the two-state stale-token successor-start mutation witness."""

    model_path = formal_dir / "SumeragiV2SuccessorStaleTokenMutation.tla"
    bug_cfg_path = formal_dir / "successor_stale_token_bug.cfg"
    fixed_cfg_path = formal_dir / "successor_stale_token_fixed.cfg"
    errors: list[str] = []

    for path in (model_path, bug_cfg_path, fixed_cfg_path):
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: missing required successor stale-token mutation artifact"
            )
    if not model_path.is_file() or model_path.is_symlink():
        return errors

    source = model_path.read_text(encoding="utf-8")

    def require_operator(
        symbol: str,
        *,
        required: tuple[str, ...] = (),
        forbidden: tuple[str, ...] = (),
        exact: str | None = None,
    ) -> None:
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{model_path}: missing mutation operator {symbol}")
            return
        body, line = extracted
        normalized = " ".join(body.split())
        if exact is not None and normalized != exact:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} must equal "
                f"only {exact!r}; found {normalized!r}"
            )
        missing = [token for token in required if token not in normalized]
        if missing:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} omits "
                f"required stale-token behavior {missing}"
            )
        present = [token for token in forbidden if token in normalized]
        if present:
            errors.append(
                f"{model_path}:{line}: mutation operator {symbol} contains "
                f"prohibited repaired behavior {present}"
            )

    require_operator(
        "AppliedSuccessorActivationToken",
        exact=(
            '[kind |-> "Applied", parentContext |-> "Parent", '
            'node |-> "Node", successorContext |-> "Successor"]'
        ),
    )
    require_operator(
        "ExactDurableParentApplicationWitness",
        exact="TRUE",
    )
    require_operator(
        "SuccessorActivationPipelineDistance",
        required=(
            'CASE activationStatus = "Queued" -> 10',
            '/\\ activationStatus = "Running" /\\ '
            "~SuccessorActivationCredentialReady -> 9",
            "/\\ SuccessorActivationCredentialReady /\\ "
            "activationPrerequisites = {} -> 8",
            "[] OTHER -> 0",
        ),
    )
    require_operator(
        "SuccessorActivationRank",
        exact="SuccessorActivationPipelineDistance",
    )
    require_operator(
        "MutationTypeInvariant",
        required=(
            "activationPrerequisites \\subseteq "
            "SuccessorActivationRequiredPrerequisites",
            "activationTokens \\subseteq {AppliedSuccessorActivationToken}",
            'lastTransition \\in {"Initial", "BuggyBegin", "FixedBegin", '
            '"FixedReject", "AppliedFailure"}',
            "previousRank \\in 0..10",
        ),
    )
    require_operator(
        "SuccessorActivationProtocolInvariantProjection",
        exact=(
            "/\\ MutationTypeInvariant "
            "/\\ ExactDurableParentApplicationWitness "
            "/\\ (activationFailurePresent => "
            'activationStatus = "Running") '
            "/\\ SuccessorActivationPipelineDistance \\in 1..10"
        ),
    )
    require_operator(
        "StaleAppliedTokenState",
        exact=(
            '/\\ activationStatus = "Queued" '
            '/\\ predecessorOwnership = "Published" '
            '/\\ activationPrerequisites = {"IngressOpen"} '
            "/\\ activationTokens = {AppliedSuccessorActivationToken} "
            "/\\ activationFailurePresent = FALSE "
            "/\\ activationFailureHistoryPresent = FALSE"
        ),
    )
    require_operator(
        "StaleAppliedTokenInit",
        exact=(
            '/\\ StaleAppliedTokenState /\\ lastTransition = "Initial" '
            "/\\ previousRank = 10"
        ),
    )
    require_operator(
        "BuggyBeginSuccessorActivation",
        required=(
            'activationStatus = "Queued"',
            'predecessorOwnership = "Published"',
            "ExactDurableParentApplicationWitness",
            'activationStatus\' = "Running"',
            'lastTransition\' = "BuggyBegin"',
            "previousRank' = SuccessorActivationRank",
        ),
        forbidden=(
            "activationPrerequisites = {}",
            "AppliedSuccessorActivationToken \\notin activationTokens",
        ),
    )
    require_operator(
        "FixedBeginSuccessorActivation",
        required=(
            'activationStatus = "Queued"',
            'predecessorOwnership = "Published"',
            "ExactDurableParentApplicationWitness",
            "activationPrerequisites = {}",
            "AppliedSuccessorActivationToken \\notin activationTokens",
            'activationStatus\' = "Running"',
        ),
    )
    require_operator(
        "FixedRejectStaleSuccessorActivation",
        exact=(
            '/\\ StaleAppliedTokenState '
            '/\\ lastTransition = "Initial" '
            '/\\ lastTransition\' = "FixedReject" '
            "/\\ previousRank' = SuccessorActivationRank "
            "/\\ UNCHANGED <<activationStatus, predecessorOwnership, "
            "activationPrerequisites, activationTokens, "
            "activationFailurePresent, activationFailureHistoryPresent>>"
        ),
    )
    require_operator(
        "MutationLatchAppliedSuccessorStartupFailure",
        required=(
            'activationStatus = "Running"',
            'predecessorOwnership = "Published"',
            "~activationFailurePresent",
            "activationPrerequisites' = {}",
            "activationTokens' = {}",
            "activationFailurePresent' = TRUE",
            "activationFailureHistoryPresent' = TRUE",
            'lastTransition\' = "AppliedFailure"',
            "previousRank' = SuccessorActivationRank",
            "UNCHANGED <<activationStatus, predecessorOwnership>>",
        ),
    )
    exact_operators = {
        "StaleBuggyBeginIsEnabled": (
            "StaleAppliedTokenState => ENABLED "
            "BuggyBeginSuccessorActivation"
        ),
        "StaleFixedBeginIsDisabled": (
            "StaleAppliedTokenState => ~ENABLED "
            "FixedBeginSuccessorActivation"
        ),
        "StaleAppliedFailureIsDisabled": (
            "StaleAppliedTokenState => ~ENABLED "
            "MutationLatchAppliedSuccessorStartupFailure"
        ),
        "InitialStaleRejectionIsEnabled": (
            '(/\\ StaleAppliedTokenState /\\ lastTransition = "Initial") '
            "=> ENABLED FixedRejectStaleSuccessorActivation"
        ),
        "BuggyBeginViolationWitness": (
            'lastTransition = "BuggyBegin" => '
            "~SuccessorActivationProtocolInvariantProjection"
        ),
        "FixedRejectPreservesStaleState": (
            'lastTransition = "FixedReject" => StaleAppliedTokenState'
        ),
        "AppliedFailurePreservesRunningWitness": (
            'lastTransition = "AppliedFailure" => '
            'activationStatus = "Running"'
        ),
        "BugMutationNext": "BuggyBeginSuccessorActivation",
        "BugMutationSpec": (
            "StaleAppliedTokenInit /\\ "
            "[][BugMutationNext]_MutationVars"
        ),
        "FixedMutationNext": (
            "\\/ FixedBeginSuccessorActivation "
            "\\/ FixedRejectStaleSuccessorActivation "
            "\\/ MutationLatchAppliedSuccessorStartupFailure"
        ),
        "FixedMutationSpec": (
            "StaleAppliedTokenInit /\\ "
            "[][FixedMutationNext]_MutationVars"
        ),
    }
    for symbol, exact in exact_operators.items():
        require_operator(symbol, exact=exact)

    cfg_contracts = {
        bug_cfg_path: (
            "SPECIFICATION BugMutationSpec",
            "CHECK_DEADLOCK FALSE",
            "INVARIANT MutationTypeInvariant",
            "INVARIANT StaleBuggyBeginIsEnabled",
            "INVARIANT StaleFixedBeginIsDisabled",
            "INVARIANT StaleAppliedFailureIsDisabled",
            "INVARIANT BuggyBeginViolationWitness",
            "INVARIANT SuccessorActivationProtocolInvariantProjection",
        ),
        fixed_cfg_path: (
            "SPECIFICATION FixedMutationSpec",
            "CHECK_DEADLOCK FALSE",
            "INVARIANT MutationTypeInvariant",
            "INVARIANT StaleFixedBeginIsDisabled",
            "INVARIANT StaleAppliedFailureIsDisabled",
            "INVARIANT InitialStaleRejectionIsEnabled",
            "INVARIANT FixedRejectPreservesStaleState",
            "INVARIANT AppliedFailurePreservesRunningWitness",
            "INVARIANT SuccessorActivationProtocolInvariantProjection",
        ),
    }
    for cfg_path, expected_lines in cfg_contracts.items():
        if not cfg_path.is_file() or cfg_path.is_symlink():
            continue
        actual_lines = tuple(
            line.strip()
            for line in cfg_path.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("\\*")
        )
        if actual_lines != expected_lines:
            errors.append(
                f"{cfg_path}: successor stale-token mutation configuration "
                f"must equal {expected_lines!r}; found {actual_lines!r}"
            )
    return errors


def _successor_activation_rank_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the exact finite-rank corridor used by successor liveness."""

    proof_path = formal_dir / "SumeragiV2SuccessorActivationRefinementProofs.tla"
    if not proof_path.is_file():
        return []

    source = proof_path.read_text(encoding="utf-8")
    errors: list[str] = []
    operator_contracts = {
        "SuccessorActivationRankCarrier": "0..21",
        "SuccessorActivationPipelineDistance": " ".join(
            r'''
            LET successorContext ==
                  CanonicalIndexedContext(parentContext.height + 1)
                marker ==
                  SuccessorActivationMarker(parentContext, node, successorContext)
            IN CASE successorActivationStatus[parentContext][node] = "Queued" -> 10
               [] /\ successorActivationStatus[parentContext][node] = "Running"
                  /\ ~SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                      -> 9
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node] = {}
                      -> 8
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationAdapterPrerequisites
                      -> 7
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationRuntimePrerequisites
                      -> 6
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationServicePrerequisites
                      -> 5
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationStartupPrerequisites
                      -> 4
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationClockPrerequisites
                  /\ marker \notin preparedSuccessorActivationMarkers
                      -> 3
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationClockPrerequisites
                  /\ marker \in preparedSuccessorActivationMarkers
                      -> 2
               [] /\ SuccessorActivationCredentialReady(
                        parentContext, node, successorContext)
                  /\ successorActivationPrerequisites[parentContext][node]
                       = SuccessorActivationRequiredPrerequisites
                      -> 1
               [] OTHER -> 0
            '''.split()
        ),
        "SuccessorActivationRank": (
            "IF SuccessorPublicationOrSuperseded(parentContext, node) THEN 0 "
            "ELSE IF successorPredecessorStatusOwnership[parentContext][node] "
            '= "Published" THEN 11 + '
            "SuccessorActivationPipelineDistance(parentContext, node) "
            "ELSE SuccessorActivationPipelineDistance(parentContext, node)"
        ),
        "SuccessorActivationPending": (
            "IndexedSuccessorActivationPending(parentContext, node)"
        ),
        "SuccessorActivationHasDurableParentWitness": (
            "/\\ \\E application \\in Chain!DecisionEvidenceSet: "
            "ExactDurableParentApplication(parentContext, node, application)"
        ),
        "SuccessorActivationAtRank": (
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationRank(parentContext, node) = rank"
        ),
        "SuccessorActivationFailureAbsent": (
            "SuccessorActivationOwner(parentContext, node) "
            "\\notin successorActivationFailures"
        ),
        "SuccessorActivationPendingStructureProperty": (
            "[](\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationPending(parentContext, node) "
            "=> /\\ SuccessorActivationHasDurableParentWitness( "
            "parentContext, node) "
            "/\\ SuccessorActivationPipelineDistance(parentContext, node) "
            "\\in 1..10 "
            "/\\ SuccessorActivationRank(parentContext, node) "
            "\\in SuccessorActivationRankCarrier "
            "/\\ ENABLED <<IndexedSuccessorActivationProgressStep( "
            "parentContext, node)>>_(IndexedChainVars))"
        ),
        "SuccessorActivationStepDecreasesRankProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[][ /\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ IndexedSuccessorActivationProgressStep(parentContext, node) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "< SuccessorActivationRank(parentContext, node) "
            "]_IndexedChainVars"
        ),
        "SuccessorActivationPendingIsNotOrphanedProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[][ /\\ SuccessorActivationPending(parentContext, node) "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ SuccessorActivationPending(parentContext, node)' "
            "]_IndexedChainVars"
        ),
        "SuccessorActivationOutcomeIsStableProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[][ /\\ SuccessorPublicationOrSuperseded(parentContext, node) "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> SuccessorPublicationOrSuperseded(parentContext, node)' "
            "]_IndexedChainVars"
        ),
        "SuccessorActivationRankProgressProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "SuccessorActivationAtRank(parentContext, node, rank) "
            "~> (SuccessorPublicationOrSuperseded(parentContext, node) "
            "\\/ \\E lower \\in SetLessThan( rank, OpToRel(<, Nat), "
            "SuccessorActivationRankCarrier): "
            "SuccessorActivationAtRank(parentContext, node, lower))"
        ),
        "SuccessorActivationStarvationFreedomProperty": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node)"
        ),
        "SuccessorActivationTemporalKernel": (
            "/\\ []IndexedCompositionInvariant "
            "/\\ []SuccessorActivationProtocolInvariant "
            "/\\ [][IndexedChainNext]_IndexedChainVars "
            "/\\ WF_IndexedChainVars( "
            "IndexedSuccessorActivationProgressStep(parentContext, node))"
        ),
        "SuccessorActivationFailureFreeSuffix": (
            "[]SuccessorActivationFailureAbsent(parentContext, node)"
        ),
        "FailedSuccessorStartupRestartStep": (
            "\\E successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "RehydrateFailedSuccessorStartup( "
            "parentContext, node, successorContext, application)"
        ),
    }
    for symbol, exact_body in operator_contracts.items():
        extracted = _top_level_operator_body(
            source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{proof_path}: missing successor-rank operator {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != exact_body:
            errors.append(
                f"{proof_path}:{line}: {symbol} must equal only "
                f"{exact_body!r}; found {normalized!r}"
            )

    theorem_contracts = {
        "SuccessorActivationPendingRankTierClassification": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationShape "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "=> \\/ /\\ successorPredecessorStatusOwnership"
            "[parentContext][node] = \"Published\" "
            "/\\ SuccessorActivationRank(parentContext, node) \\in 12..21 "
            "\\/ /\\ successorPredecessorStatusOwnership"
            "[parentContext][node] = \"Absent\" "
            "/\\ SuccessorActivationRank(parentContext, node) \\in 1..10",
            (
                "SuccessorActivationShape",
                "SuccessorActivationProtocolInvariant",
                "SuccessorActivationRank",
                "Isa",
            ),
        ),
        "ExactDurableParentApplicationHasAdmissibleSuccessorContext": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in ValidatorIds, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ Chain!ChainEpochInvariant "
            "/\\ ExactDurableParentApplication(parentContext, node, application) "
            "=> CanonicalIndexedContext(parentContext.height + 1) "
            "\\in AdmissibleContextRecords",
            (
                "Chain!ChainEpochTypeInvariant",
                "Chain!NodesDoNotOutrunCertificates",
                "Chain!CertifiedPrefixBacked",
                "FrozenContextAdmissible",
                "Isa",
            ),
        ),
        "SuccessorActivationProgressPreservesProtocolInvariant": (
            "\\A selectedParent \\in AdmissibleContextRecords, "
            "selectedNode \\in ValidatorIds: "
            "Chain!ChainEpochInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ IndexedSuccessorActivationProgressStep( "
            "selectedParent, selectedNode) "
            "=> SuccessorActivationProtocolInvariant'",
            (
                "ExactDurableParentApplicationHasAdmissibleSuccessorContext",
                "ExpandENABLED",
                "Isa",
            ),
        ),
        "IndexedActionPreservesSuccessorActivationProtocolInvariant": (
            "IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ IndexedChainNext "
            "=> SuccessorActivationProtocolInvariant'",
            (
                "IndexedProductActionPreservesSuccessorActivationProtocolInvariant",
                "SuccessorActivationProgressPreservesProtocolInvariant",
                "DEF IndexedCompositionInvariant",
            ),
        ),
        "CleanCompleteTipRestartDescendsPublishedTier": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ RehydrateCleanCompleteTipSuccessorStartup( "
            "parentContext, node, successorContext, application) "
            "=> /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "< SuccessorActivationRank(parentContext, node)",
            (
                "CleanCompleteTipRestartCrossesPublishedToAbsentTier",
                "Isa",
            ),
        ),
        "FailureFreeBracketExcludesSuccessorResetActions": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "=> /\\ ~SuccessorStartupFailureStep(parentContext, node) "
            "/\\ ~FailedSuccessorStartupRestartStep(parentContext, node)",
            (
                "SuccessorStartupFailureStep",
                "FailedSuccessorStartupRestartStep",
                "LatchAppliedSuccessorStartupFailure",
                "LatchRecoveredSuccessorStartupFailure",
                "RehydrateFailedSuccessorStartup",
                "Isa",
            ),
        ),
        "CleanCompleteTipRestartCrossesPublishedToAbsentTier": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ RehydrateCleanCompleteTipSuccessorStartup( "
            "parentContext, node, successorContext, application) "
            "=> /\\ SuccessorActivationRank(parentContext, node) \\in 12..21 "
            "/\\ successorPredecessorStatusOwnership'[parentContext][node] "
            "= \"Absent\" "
            "/\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' = 10 "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)'",
            (
                "SuccessorActivationRank",
                "SuccessorActivationPipelineDistance",
                "RehydrateCleanCompleteTipSuccessorStartup",
                "ExactDurableParentApplication",
                "Isa",
            ),
        ),
        "RecoveredAuthenticationDescendsAbsentTier": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "successorContext \\in AdmissibleContextRecords, "
            "application \\in Chain!DecisionEvidenceSet: "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ AuthenticateRecoveredSuccessorActivation( "
            "parentContext, node, successorContext, application) "
            "=> /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ successorPredecessorStatusOwnership'[parentContext][node] "
            "= \"Absent\" "
            "/\\ SuccessorActivationRank(parentContext, node) = 10 "
            "/\\ SuccessorActivationRank(parentContext, node)' = 8 "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)'",
            (
                "AuthenticateRecoveredSuccessorActivation",
                "SuccessorActivationCredentialReady",
                "ExactSuccessorActivationToken",
                "ExactCompleteTipRecoveryAuthority",
                "SuccessorActivationRank",
                "SuccessorActivationPipelineDistance",
                "Isa",
            ),
        ),
        "SuccessorActivationFailureFreeProgressStrictlyDecreasesRank": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ IndexedSuccessorActivationProgressStep(parentContext, node) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "< SuccessorActivationRank(parentContext, node)",
            (
                "FailureFreeBracketExcludesSuccessorResetActions",
                "SuccessorActivationPendingRankTierClassification",
                "RecoveredAuthenticationDescendsAbsentTier",
                "CleanCompleteTipRestartDescendsPublishedTier",
                "LatchAppliedSuccessorStartupFailure",
                "LatchRecoveredSuccessorStartupFailure",
                "RehydrateFailedSuccessorStartup",
                "Isa",
            ),
        ),
        "IndexedProductActionDoesNotRaisePendingSuccessorRank": (
            "\\A initialContext \\in JoinedContexts, "
            "parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ IndexedProductActionAt(initialContext) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "<= SuccessorActivationRank(parentContext, node)",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "IndexedProductActionAt",
                "IndexedReceiptClassification",
                "QueueSuccessorActivation",
                "Isa",
            ),
        ),
        "OtherOwnerProgressFramesPendingSuccessorRankOrSupersedes": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "selectedParent \\in AdmissibleContextRecords, "
            "selectedNode \\in ValidatorIds: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationOwner(selectedParent, selectedNode) "
            "# SuccessorActivationOwner(parentContext, node) "
            "/\\ IndexedSuccessorActivationProgressStep( "
            "selectedParent, selectedNode) "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "= SuccessorActivationRank(parentContext, node)",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "SuccessorActivationOwner",
                "IndexedSuccessorActivationProgressStep",
                "Isa",
            ),
        ),
        "IndexedStepRetainsExactDurableParentWitnessOrExits": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationHasDurableParentWitness( "
            "parentContext, node)'",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "IndexedStepPreservesSuccessorActivationProtocolInvariant",
                "SuccessorActivationHasDurableParentWitness",
                "Isa",
            ),
        ),
        "IndexedFailureFreeStepDoesNotRaiseSuccessorActivationRank": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorPublicationOrSuperseded(parentContext, node)' "
            "\\/ /\\ SuccessorActivationPending(parentContext, node)' "
            "/\\ SuccessorActivationRank(parentContext, node)' "
            "<= SuccessorActivationRank(parentContext, node)",
            (
                "IndexedStepDoesNotOrphanSuccessorActivation",
                "IndexedProductActionDoesNotRaisePendingSuccessorRank",
                "OtherOwnerProgressFramesPendingSuccessorRankOrSupersedes",
                "FailureFreeBracketExcludesSuccessorResetActions",
                "SuccessorActivationFailureFreeProgressStrictlyDecreasesRank",
                "IndexedChainNext",
                "Isa",
            ),
        ),
        "SuccessorActivationFailureFreeRankPersistsOrExits": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "/\\ IndexedCompositionInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationAtRank(parentContext, node, rank) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ [IndexedChainNext]_IndexedChainVars "
            "=> \\/ SuccessorActivationAtRank(parentContext, node, rank)' "
            "\\/ SuccessorActivationRankExit(parentContext, node, rank)'",
            (
                "IndexedFailureFreeStepDoesNotRaiseSuccessorActivationRank",
                "IndexedStepRetainsExactDurableParentWitnessOrExits",
                "IndexedStepPreservesSuccessorActivationProtocolInvariant",
                "Isa",
            ),
        ),
        "SuccessorActivationFailureFreeProgressExitsCurrentRank": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "/\\ Chain!ChainEpochInvariant "
            "/\\ SuccessorActivationProtocolInvariant "
            "/\\ SuccessorActivationAtRank(parentContext, node, rank) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node) "
            "/\\ SuccessorActivationFailureAbsent(parentContext, node)' "
            "/\\ <<IndexedSuccessorActivationProgressStep( "
            "parentContext, node)>>_(IndexedChainVars) "
            "=> SuccessorActivationRankExit(parentContext, node, rank)'",
            (
                "SuccessorActivationFailureFreeProgressStrictlyDecreasesRank",
                "SuccessorActivationProgressPreservesProtocolInvariant",
                "Isa",
            ),
        ),
        "FailureFreeSuccessorActivationRankLeadsToExit": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive, "
            "rank \\in SuccessorActivationRankCarrier: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationAtRank(parentContext, node, rank) "
            "~> SuccessorActivationRankExit(parentContext, node, rank))",
            (
                "SuccessorActivationFailureFreeRankPersistsOrExits",
                "SuccessorActivationAtRankEnablesFairProgress",
                "SuccessorActivationFailureFreeProgressExitsCurrentRank",
                "Chain!ChainEpochInvariant",
                "DEF IndexedCompositionInvariant",
                "WF_IndexedChainVars",
                "PTL",
            ),
        ),
        "FailureFreeSuccessorActivationRankConverges": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> \\A rank \\in SuccessorActivationRankCarrier: "
            "SuccessorActivationAtRank(parentContext, node, rank) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node)",
            (
                "SuccessorActivationRankOrderingIsWellFounded",
                "FailureFreeSuccessorActivationRankLeadsToExit",
                "WellFoundedLeadsTo",
            ),
        ),
        "FailureFreeSuccessorActivationConverges": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node))",
            (
                "FailureFreeSuccessorActivationRankConverges",
                "SuccessorActivationRankExistentialLift",
                "SuccessorActivationPendingHasRankWitness",
                "PTL",
            ),
        ),
        "SuccessorActivationTemporalKernelIsSuffixClosed": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "SuccessorActivationTemporalKernel(parentContext, node) "
            "=> []SuccessorActivationTemporalKernel(parentContext, node)",
            ("PTL", "SuccessorActivationTemporalKernel"),
        ),
        "FailureFreeSuccessorActivationConvergenceAtEverySuffix": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "[]( /\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node)))",
            ("FailureFreeSuccessorActivationConverges", "PTL"),
        ),
        "SuccessorActivationPendingReachesFailureFreeSuffixOrOutcome": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ <>SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> (SuccessorPublicationOrSuperseded(parentContext, node) "
            "\\/ /\\ SuccessorActivationPending(parentContext, node) "
            "/\\ SuccessorActivationFailureFreeSuffix( "
            "parentContext, node)))",
            (
                "IndexedStepRetainsExactDurableParentWitnessOrExits",
                "SuccessorActivationTemporalKernel",
                "SuccessorActivationFailureFreeSuffix",
                "PTL",
            ),
        ),
        "EventualFailureFreeSuffixLiftsSuccessorConvergence": (
            "\\A parentContext \\in AdmissibleContextRecords, "
            "node \\in Responsive: "
            "/\\ SuccessorActivationTemporalKernel(parentContext, node) "
            "/\\ <>SuccessorActivationFailureFreeSuffix(parentContext, node) "
            "=> (SuccessorActivationPending(parentContext, node) "
            "~> SuccessorPublicationOrSuperseded(parentContext, node))",
            (
                "SuccessorActivationTemporalKernelIsSuffixClosed",
                "FailureFreeSuccessorActivationConvergenceAtEverySuffix",
                "SuccessorActivationPendingReachesFailureFreeSuffixOrOutcome",
                "PTL",
            ),
        ),
        "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom": (
            "IndexedChainSpec => "
            "SuccessorActivationStarvationFreedomProperty",
            (
                "IndexedChainSpecEstablishesSuccessorActivationTemporalKernel",
                "EventualFailureFreeSuccessorStartupSuffix",
                "EventualFailureFreeSuffixLiftsSuccessorConvergence",
            ),
        ),
        "IndexedChainSpecEstablishesSuccessorActivationRankProgress": (
            "IndexedChainSpec => SuccessorActivationRankProgressProperty",
            (
                "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom",
                "SuccessorActivationRankProgressProperty",
                "PTL",
            ),
        ),
    }
    exact_proof_token_counts = {
        "IndexedActionPreservesSuccessorActivationProtocolInvariant": {
            "DEF IndexedCompositionInvariant": 2,
        },
        "FailureFreeSuccessorActivationRankLeadsToExit": {
            "Chain!ChainEpochInvariant": 1,
            "DEF IndexedCompositionInvariant": 1,
        },
    }
    for symbol, (exact_statement, required_proof_tokens) in (
        theorem_contracts.items()
    ):
        theorem = _top_level_theorem_body(
            source, symbol, preserve_string_contents=True
        )
        if theorem is None:
            errors.append(f"{proof_path}: missing successor-rank theorem {symbol}")
            continue
        theorem_body, line = theorem
        observed_statement = _tla_statement_without_proof(theorem_body)
        if observed_statement != exact_statement:
            errors.append(
                f"{proof_path}:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {observed_statement!r}"
            )
        theorem_parts = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b",
            theorem_body,
            maxsplit=1,
        )
        if len(theorem_parts) != 2:
            errors.append(
                f"{proof_path}:{line}: {symbol} must retain an explicit "
                "non-vacuous proof body"
            )
            continue
        observed_proof = theorem_parts[1]
        for required_token in required_proof_tokens:
            if not _tla_dependency_present(observed_proof, required_token):
                errors.append(
                    f"{proof_path}:{line}: {symbol} proof must invoke "
                    f"{required_token}"
                )
        for exact_token, exact_count in exact_proof_token_counts.get(
            symbol, {}
        ).items():
            observed_count = len(
                _tla_dependency_positions(observed_proof, exact_token)
            )
            if observed_count != exact_count:
                errors.append(
                    f"{proof_path}:{line}: {symbol} proof must contain "
                    f"{exact_token!r} exactly {exact_count} time(s); found "
                    f"{observed_count}"
                )
        if re.search(
            r"(?:\bOBVIOUS\b|\bASSUME\s+FALSE\b|\bBY\s+TRUE\b|"
            r"\bPROVE\s+TRUE\b)",
            observed_proof,
        ):
            errors.append(
                f"{proof_path}:{line}: {symbol} proof may not use a "
                "vacuous assertion"
            )

    chain_path = formal_dir / "SumeragiV2ChainEpochRefinement.tla"
    if chain_path.is_file():
        chain_source = chain_path.read_text(encoding="utf-8")
        pending = _top_level_operator_body(
            chain_source,
            "IndexedSuccessorActivationPending",
            preserve_string_contents=True,
        )
        exact_pending = (
            "/\\ parentContext \\in AdmissibleContextRecords "
            "/\\ node \\in ValidatorIds "
            "/\\ parentContext.height < MaxHeight "
            "/\\ successorActivationStatus[parentContext][node] "
            '\\in {"Queued", "Running"} '
            "/\\ ~SuccessorPublicationOrSuperseded(parentContext, node)"
        )
        if pending is None:
            errors.append(
                f"{chain_path}: missing IndexedSuccessorActivationPending"
            )
        else:
            body, line = pending
            normalized = " ".join(body.split())
            if normalized != exact_pending:
                errors.append(
                    f"{chain_path}:{line}: IndexedSuccessorActivationPending "
                    f"must equal only {exact_pending!r}; found {normalized!r}"
                )

    theorem_symbol = "SuccessorActivationStarvationFreedomObligation"
    theorem = _top_level_theorem_body(
        source, theorem_symbol, preserve_string_contents=True
    )
    exact_statement = (
        "IndexedChainSpec "
        "=> /\\ SuccessorActivationPendingStructureProperty "
        "/\\ SuccessorActivationStepDecreasesRankProperty "
        "/\\ SuccessorActivationPendingIsNotOrphanedProperty "
        "/\\ SuccessorActivationOutcomeIsStableProperty "
        "/\\ SuccessorActivationRankProgressProperty "
        "/\\ SuccessorActivationStarvationFreedomProperty"
    )
    if theorem is None:
        errors.append(f"{proof_path}: missing {theorem_symbol}")
    else:
        body, line = theorem
        theorem_parts = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )
        statement = theorem_parts[0]
        normalized = " ".join(statement.split())
        if normalized != exact_statement:
            errors.append(
                f"{proof_path}:{line}: {theorem_symbol} must state only "
                f"{exact_statement!r}; found {normalized!r}"
            )
        if len(theorem_parts) != 2:
            errors.append(
                f"{proof_path}:{line}: {theorem_symbol} must retain the "
                "explicit candidate TLAPS proof while strict verification "
                "remains pending"
            )
        else:
            aggregate_proof = theorem_parts[1]
            required_aggregate_dependencies = (
                "IndexedChainSpecEstablishesSuccessorActivationPendingStructure",
                "IndexedChainSpecEstablishesSuccessorActivationStepDecrease",
                "IndexedChainSpecEstablishesSuccessorActivationNonOrphaning",
                "IndexedChainSpecEstablishesSuccessorActivationOutcomeStability",
                "IndexedChainSpecEstablishesSuccessorActivationRankProgress",
                "IndexedChainSpecEstablishesSuccessorActivationStarvationFreedom",
            )
            for dependency in required_aggregate_dependencies:
                if len(
                    _tla_dependency_positions(aggregate_proof, dependency)
                ) != 1:
                    errors.append(
                        f"{proof_path}:{line}: {theorem_symbol} proof must "
                        f"invoke {dependency} exactly once"
                    )
            if re.search(
                r"(?:\bOBVIOUS\b|\bASSUME\s+FALSE\b|\bBY\s+TRUE\b|"
                r"\bPROVE\s+TRUE\b)",
                aggregate_proof,
            ):
                errors.append(
                    f"{proof_path}:{line}: {theorem_symbol} proof may not "
                    "use a vacuous assertion"
                )

    equivalence_symbol = "SuccessorActivationStarvationMatchesChainProgress"
    equivalence = _top_level_theorem_body(
        source, equivalence_symbol, preserve_string_contents=True
    )
    exact_equivalence = (
        "SuccessorActivationStarvationFreedomProperty "
        "<=> IndexedSuccessorActivationProgress"
    )
    if equivalence is None:
        errors.append(f"{proof_path}: missing {equivalence_symbol}")
    else:
        body, line = equivalence
        statement = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )[0]
        normalized = " ".join(statement.split())
        if normalized != exact_equivalence:
            errors.append(
                f"{proof_path}:{line}: {equivalence_symbol} must state only "
                f"{exact_equivalence!r}; found {normalized!r}"
            )
    return errors


def _async_historical_recovery_source_fidelity_errors(
    formal_dir: Path,
) -> list[str]:
    """Pin the exact all-responsive historical-recovery proof boundary."""

    path = formal_dir / "SumeragiV2AsyncHistoricalRecoveryLivenessProofs.tla"
    if not path.is_file():
        return [f"{path}: missing Async historical-recovery liveness child"]

    raw_source = path.read_text(encoding="utf-8")
    source = strip_tla_comments(raw_source, preserve_string_contents=True)
    errors: list[str] = []

    extends = re.search(r"(?m)^EXTENDS\s+([^\n]+)$", source)
    exact_extends = "SumeragiV2AsyncTimeoutOwnershipProofs, TLAPS"
    if extends is None or " ".join(extends.group(1).split()) != exact_extends:
        errors.append(
            f"{path}: Async historical-recovery child must extend exactly "
            f"{exact_extends!r}"
        )

    operator_contracts = {
        "HistoricalRecoveryTargetDecisionProgressProperty": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node)) "
            "~> NodeHasDecision(node)"
        ),
        "ResponsiveDecisionApplicationProgressProperty": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ NodeHasDecision(node)) "
            "~> NodeHasApplication(node)"
        ),
        "HistoricalRecoveryAsyncTemporalPrerequisites": (
            "/\\ HistoricalRecoveryTargetDecisionProgressProperty(specification) "
            "/\\ ResponsiveDecisionApplicationProgressProperty(specification)"
        ),
        "HistoricalProtectedCandidateOwned": (
            "/\\ candidate.node \\in Responsive "
            "/\\ HistoricalRecoveryTarget(candidate.node) "
            "/\\ ProtectedCandidateOwned(candidate)"
        ),
        "HistoricalProtectedOwnedAtServiceRank": (
            "/\\ gst /\\ HistoricalProtectedCandidateOwned(candidate) "
            "/\\ CandidateServiceRank(candidate) = rank"
        ),
        "HistoricalProtectedServiceOwnershipExit": (
            "~HistoricalProtectedCandidateOwned(candidate)"
        ),
        "HistoricalProtectedServiceRankProgressProperty": (
            "specification => \\A candidate \\in AsyncCandidateSet, "
            "rank \\in OwnedServiceRankCarrier: "
            "HistoricalProtectedOwnedAtServiceRank(candidate, rank) "
            "~> (HistoricalProtectedServiceOwnershipExit(candidate) "
            "\\/ \\E lower \\in SetLessThan( rank, "
            "OwnedServiceRankOrdering, OwnedServiceRankCarrier): "
            "HistoricalProtectedOwnedAtServiceRank(candidate, lower))"
        ),
        "HistoricalProtectedStageRankProgressProperty": (
            "specification => \\A candidate \\in AsyncCandidateSet, "
            "position \\in Nat: (gst "
            "/\\ HistoricalProtectedCandidateOwned(candidate) "
            "/\\ CandidateServiceRank(candidate) = <<stage, position>>) "
            "~> (HistoricalProtectedServiceOwnershipExit(candidate) "
            "\\/ \\E lower \\in SetLessThan( <<stage, position>>, "
            "OwnedServiceRankOrdering, OwnedServiceRankCarrier): "
            "HistoricalProtectedOwnedAtServiceRank(candidate, lower))"
        ),
        "HistoricalProtectedStage2RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 2)"
        ),
        "HistoricalProtectedStage3RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 3)"
        ),
        "HistoricalProtectedStage4RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 4)"
        ),
        "HistoricalProtectedStage5RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 5)"
        ),
        "HistoricalProtectedStage6RankProgressProperty": (
            "HistoricalProtectedStageRankProgressProperty(specification, 6)"
        ),
        "HistoricalProtectedServiceRankLeafProperties": (
            "/\\ HistoricalProtectedStage2RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage3RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage4RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage5RankProgressProperty(specification) "
            "/\\ HistoricalProtectedStage6RankProgressProperty(specification)"
        ),
        "HistoricalProtectedCandidateStarvationProperty": (
            "specification => \\A candidate \\in AsyncCandidateSet: "
            "(gst /\\ HistoricalProtectedCandidateOwned(candidate)) "
            "~> HistoricalProtectedServiceOwnershipExit(candidate)"
        ),
        "HistoricalCommitCertificateDiscoveryPending": (
            "/\\ AsyncStrongTypeInvariant /\\ gst "
            "/\\ HistoricalCommitCertificateDiscoveryDue(node)"
        ),
        "HistoricalCommitCertificateDiscoveryOutcome": (
            "\\/ NodeHasDecision(node) "
            "\\/ /\\ HistoricalRecoveryTarget(node) "
            "/\\ ActiveCommitCertificateRequests(node) # {}"
        ),
        "HistoricalCommitCertificateDiscoveryPersistenceObligation": (
            "\\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPending(node) "
            "/\\ [AsyncNext]_AsyncAllVars "
            "=> HistoricalCommitCertificateDiscoveryPending(node)' "
            "\\/ HistoricalCommitCertificateDiscoveryOutcome(node)'"
        ),
        "HistoricalCommitCertificateDiscoveryPersistenceUnless": (
            "[][HistoricalCommitCertificateDiscoveryPending(node) "
            "/\\ ~HistoricalCommitCertificateDiscoveryOutcome(node) "
            "=> HistoricalCommitCertificateDiscoveryPending(node)' "
            "\\/ HistoricalCommitCertificateDiscoveryOutcome(node)']_AsyncAllVars"
        ),
        "HistoricalCommitCertificateDiscoveryPersistenceProperty": (
            "specification => \\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPersistenceUnless(node)"
        ),
        "HistoricalRecoveryTargetRemoteServerInvariant": (
            "\\A node \\in Responsive: HistoricalRecoveryTarget(node) "
            "=> CommitCertificateRequestOutbox(node) # {}"
        ),
        "HistoricalRecoveryTargetRemoteServerProperty": (
            "specification => []HistoricalRecoveryTargetRemoteServerInvariant"
        ),
        "HistoricalCommitCertificateDiscoveryClockProgressProperty": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node)) "
            "~> (NodeHasDecision(node) "
            "\\/ /\\ HistoricalRecoveryTarget(node) "
            "/\\ \\/ ActiveCommitCertificateRequests(node) # {} "
            "\\/ asyncNow >= AsyncRoundTimeout)"
        ),
        "HistoricalCommitCertificateRequestScheduled": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E request \\in ActiveCommitCertificateRequests(node): "
            "ItemScheduled(request)"
        ),
        "HistoricalCommitCertificateResponseScheduled": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E response \\in AsyncNetworkItems: "
            "/\\ response.kind = \"CommitCertificateResponse\" "
            "/\\ response.envelope.recipient = node "
            "/\\ CommitCertificateResponseAuthorized(response) "
            "/\\ ItemScheduled(response)"
        ),
        "HistoricalCommitDecisionDirectEvidence": (
            "/\\ candidate.evidence \\in asyncSentItems "
            '/\\ candidate.evidence.kind = "CommitQC" '
            "/\\ candidate.evidence.envelope = "
            "QcEnvelope(candidate.node, qc) "
            "/\\ candidate.causalOrigin = "
            "AsyncDeliveryCandidateCausalOriginAt("
            "candidate.evidence, context)"
        ),
        "HistoricalCommitDecisionResponseEvidence": (
            "/\\ candidate.evidence \\in asyncSentItems "
            '/\\ candidate.evidence.kind = "CommitCertificateResponse" '
            "/\\ candidate.evidence.envelope.recipient = candidate.node "
            "/\\ candidate.evidence.envelope.qc = qc "
            "/\\ CommitCertificateRequestAuthorized( "
            "candidate.evidence.envelope.request) "
            "/\\ candidate.causalOrigin = "
            "AsyncCommitCertificateResponseCandidateCausalOriginAt( "
            "candidate.evidence, context)"
        ),
        "HistoricalCommitDecisionCandidateOwned": (
            "\\E candidate \\in AsyncCandidateSet, qc \\in commitQCs: "
            "/\\ candidate.node = node /\\ candidate.kind = kind "
            '/\\ kind \\in {"DeliverQC", "BeginDecision", '
            '"PersistDecision"} '
            "/\\ qc.context = context /\\ qc.phase = \"Commit\" "
            "/\\ candidate.consumerContext = context "
            "/\\ candidate.view = qc.view "
            "/\\ candidate.subject = qc.subject "
            "/\\ HistoricalProtectedCandidateOwned(candidate) "
            "/\\ \\/ HistoricalCommitDecisionDirectEvidence(candidate, qc) "
            "\\/ HistoricalCommitDecisionResponseEvidence(candidate, qc) "
            '/\\ IF kind = "DeliverQC" THEN candidate.item = '
            'IF candidate.evidence.kind = "CommitQC" '
            "THEN candidate.evidence "
            "ELSE DiscoveredCommitQcItem(candidate.evidence) "
            "ELSE candidate.item = NoAsyncItem"
        ),
        "HistoricalActiveRequestRetransmissionProgressLeaf": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ ActiveCommitCertificateRequests(node) # {}) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitCertificateRequestScheduled(node))"
        ),
        "HistoricalCommitRequestServeProgressLeaf": (
            "StarvationFreedomProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitCertificateRequestScheduled(node)) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitCertificateResponseScheduled(node)))"
        ),
        "HistoricalCommitResponseAdmissionProgressLeaf": (
            "specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitCertificateResponseScheduled(node)) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitDecisionCandidateOwned( "
            "node, \"DeliverQC\"))"
        ),
        "HistoricalCommitDeliveryProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitDecisionCandidateOwned(node, \"DeliverQC\")) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitDecisionCandidateOwned( "
            "node, \"BeginDecision\")))"
        ),
        "HistoricalBeginDecisionProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitDecisionCandidateOwned( "
            "node, \"BeginDecision\")) "
            "~> (NodeHasDecision(node) "
            "\\/ HistoricalCommitDecisionCandidateOwned( "
            "node, \"PersistDecision\")))"
        ),
        "HistoricalPersistDecisionProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalCommitDecisionCandidateOwned( "
            "node, \"PersistDecision\")) ~> NodeHasDecision(node))"
        ),
        "HistoricalCommitCertificateConcreteLeafProperties": (
            "/\\ HistoricalActiveRequestRetransmissionProgressLeaf(specification) "
            "/\\ HistoricalCommitRequestServeProgressLeaf(specification) "
            "/\\ HistoricalCommitResponseAdmissionProgressLeaf(specification) "
            "/\\ HistoricalCommitDeliveryProgressLeaf(specification) "
            "/\\ HistoricalBeginDecisionProgressLeaf(specification) "
            "/\\ HistoricalPersistDecisionProgressLeaf(specification)"
        ),
        "HistoricalDecisionRecordMatches": (
            "/\\ decision \\in decisions /\\ decision.node = node "
            "/\\ decision.qc.context = context "
            "/\\ decision.qc.phase = \"Commit\""
        ),
        "HistoricalDecisionPipelineKindOwned": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E decision \\in decisions: "
            "/\\ HistoricalDecisionRecordMatches(node, decision) "
            "/\\ DecisionPipelineKindOwned(node, decision.qc, kind)"
        ),
        "HistoricalDecisionCertifiedRequestActive": (
            "/\\ HistoricalRecoveryTarget(node) "
            "/\\ \\E decision \\in decisions: "
            "/\\ HistoricalDecisionRecordMatches(node, decision) "
            "/\\ DecisionCertifiedRequestActive(node, decision.qc)"
        ),
        "HistoricalDecisionRecoveryFrontier": (
            "\\/ NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"FetchBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned("
            "node, \"RequestCertifiedBody\") "
            "\\/ HistoricalDecisionCertifiedRequestActive(node) "
            "\\/ HistoricalDecisionPipelineKindOwned("
            "node, \"FetchCertifiedBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"StoreBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"ValidateBody\") "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"Apply\")"
        ),
        "HistoricalDecisionFrontierAvailabilityProperty": (
            "specification => []\\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ NodeHasDecision(node)) "
            "=> HistoricalDecisionRecoveryFrontier(node)"
        ),
        "HistoricalDecisionFetchProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned(node, \"FetchBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"RequestCertifiedBody\") "
            "\\/ HistoricalDecisionCertifiedRequestActive(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"ValidateBody\")))"
        ),
        "HistoricalDecisionRequestBodyProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned( "
            "node, \"RequestCertifiedBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionCertifiedRequestActive(node)))"
        ),
        "HistoricalDecisionCertifiedResponseProgressLeaf": (
            "(/\\ StarvationFreedomProperty(specification) "
            "/\\ HistoricalProtectedCandidateStarvationProperty(specification)) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionCertifiedRequestActive(node)) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"FetchCertifiedBody\")))"
        ),
        "HistoricalDecisionFetchCertifiedProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned( "
            "node, \"FetchCertifiedBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"StoreBody\")))"
        ),
        "HistoricalDecisionStoreProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned(node, \"StoreBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned( "
            "node, \"ValidateBody\")))"
        ),
        "HistoricalDecisionValidateProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned("
            "node, \"ValidateBody\")) "
            "~> (NodeHasApplication(node) "
            "\\/ HistoricalDecisionPipelineKindOwned(node, \"Apply\")))"
        ),
        "HistoricalDecisionApplyProgressLeaf": (
            "HistoricalProtectedCandidateStarvationProperty(specification) "
            "=> (specification => \\A node \\in Responsive: "
            "(gst /\\ HistoricalDecisionPipelineKindOwned(node, \"Apply\")) "
            "~> NodeHasApplication(node))"
        ),
        "HistoricalDecisionConcreteLeafProperties": (
            "/\\ HistoricalDecisionFetchProgressLeaf(specification) "
            "/\\ HistoricalDecisionRequestBodyProgressLeaf(specification) "
            "/\\ HistoricalDecisionCertifiedResponseProgressLeaf(specification) "
            "/\\ HistoricalDecisionFetchCertifiedProgressLeaf(specification) "
            "/\\ HistoricalDecisionStoreProgressLeaf(specification) "
            "/\\ HistoricalDecisionValidateProgressLeaf(specification) "
            "/\\ HistoricalDecisionApplyProgressLeaf(specification)"
        ),
        "ResponsiveDecisionServiceOwnershipInvariant": (
            "\\A node \\in Responsive: "
            "(gst /\\ NodeHasDecision(node) /\\ ~NodeHasApplication(node)) "
            "=> \\/ node \\in AsyncCurrentResponsiveVoters "
            "\\/ HistoricalRecoveryTarget(node)"
        ),
        "ResponsiveDecisionServiceOwnershipProperty": (
            "specification => []ResponsiveDecisionServiceOwnershipInvariant"
        ),
        "HistoricalRecoveryAsyncTemporalClosurePremises": (
            "/\\ HistoricalCommitCertificateDiscoveryPersistenceProperty(specification) "
            "/\\ HistoricalRecoveryTargetRemoteServerProperty(specification) "
            "/\\ HistoricalCommitCertificateDiscoveryClockProgressProperty(specification) "
            "/\\ HistoricalProtectedServiceRankLeafProperties(specification) "
            "/\\ HistoricalCommitCertificateConcreteLeafProperties(specification) "
            "/\\ HistoricalDecisionFrontierAvailabilityProperty(specification) "
            "/\\ HistoricalDecisionConcreteLeafProperties(specification) "
            "/\\ ResponsiveDecisionServiceOwnershipProperty(specification) "
            "/\\ ApplicationCompletionProgressProperty(specification)"
        ),
        "HistoricalRecoveryAsyncRemainingCorridorPremises": (
            "/\\ HistoricalCommitCertificateDiscoveryClockProgressProperty(specification) "
            "/\\ HistoricalProtectedServiceRankLeafProperties(specification) "
            "/\\ HistoricalCommitCertificateConcreteLeafProperties(specification) "
            "/\\ HistoricalDecisionFrontierAvailabilityProperty(specification) "
            "/\\ HistoricalDecisionConcreteLeafProperties(specification) "
            "/\\ ApplicationCompletionProgressProperty(specification)"
        ),
        "HistoricalLockedBodyRecoveryOutcome": (
            "\\/ HistoricalLockedBodySourceRetired(node, qc) "
            "\\/ HistoricalLockedBodyRecoveryTerminal(node, qc)"
        ),
        "HistoricalLockedCommitCarrierRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedCommitRecoveryWitness(node, qc) "
            "/\\ ~HistoricalLockedBodyValidated(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyRestartAuthority(node, qc) "
            "\\/ HistoricalLockedBodyFetchOwned(node, qc) "
            "\\/ HistoricalLockedCertifiedRequestActive(node, qc) "
            "\\/ HistoricalLockedBodyValidateOwned(node, qc))"
        ),
        "HistoricalLockedRestartRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyRestartAuthority(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyFetchOwned(node, qc))"
        ),
        "HistoricalLockedFetchRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyFetchOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedCertifiedRequestActive(node, qc) "
            "\\/ HistoricalLockedBodyValidateOwned(node, qc))"
        ),
        "HistoricalLockedRequestCandidateProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyRequestOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedCertifiedRequestActive(node, qc))"
        ),
        "HistoricalLockedActiveRequestProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedCertifiedRequestActive(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyCertifiedFetchOwned(node, qc))"
        ),
        "HistoricalLockedCertifiedFetchProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyCertifiedFetchOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyStoreOwned(node, qc))"
        ),
        "HistoricalLockedStoreRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyStoreOwned(node, qc)) "
            "~> (HistoricalLockedBodyRecoveryOutcome(node, qc) "
            "\\/ HistoricalLockedBodyValidateOwned(node, qc))"
        ),
        "HistoricalLockedValidateRecoveryProgressLeaf": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(/\\ gst "
            "/\\ HistoricalLockedPrepareSource(node, qc) "
            "/\\ HistoricalLockedBodyValidateOwned(node, qc)) "
            "~> HistoricalLockedBodyRecoveryOutcome(node, qc)"
        ),
        "HistoricalLockedBodyRecoveryConeLeafProperties": (
            "/\\ HistoricalLockedCommitCarrierRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedRestartRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedFetchRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedRequestCandidateProgressLeaf(specification) "
            "/\\ HistoricalLockedActiveRequestProgressLeaf(specification) "
            "/\\ HistoricalLockedCertifiedFetchProgressLeaf(specification) "
            "/\\ HistoricalLockedStoreRecoveryProgressLeaf(specification) "
            "/\\ HistoricalLockedValidateRecoveryProgressLeaf(specification)"
        ),
        "HistoricalLockedBodyRecoveryConeProperty": (
            "specification => \\A node \\in AsyncCurrentResponsiveVoters, "
            "qc \\in prepareQCs: "
            "(gst /\\ HistoricalLockedPrepareSource(node, qc)) "
            "~> HistoricalLockedBodyRecoveryOutcome(node, qc)"
        ),
    }
    for symbol, exact_body in operator_contracts.items():
        extracted = _top_level_operator_body(
            raw_source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{path}: missing Async historical operator {symbol}")
            continue
        body, line = extracted
        normalized = " ".join(body.split())
        if normalized != exact_body:
            errors.append(
                f"{path}:{line}: {symbol} must equal only "
                f"{exact_body!r}; found {normalized!r}"
            )

    endpoint_symbols = (
        "HistoricalRecoveryTargetDecisionProgressProperty",
        "ResponsiveDecisionApplicationProgressProperty",
        "HistoricalRecoveryAsyncTemporalPrerequisites",
    )
    for symbol in endpoint_symbols:
        if _symbol_exists(source, symbol, theorem_only=True):
            errors.append(
                f"{path}: {symbol} must remain an operator property until its "
                "exact corridor is proved without extra premises"
            )

    if re.search(r"(?m)^CONSTANTS?\b", source):
        errors.append(
            f"{path}: Async historical-recovery child may not replace exact "
            "temporal predicates with unconstrained constants"
        )
    if re.search(r"\bResponsiveProtectedCandidateOwned\b", source):
        errors.append(
            f"{path}: historical rank may not reuse the current-voter-only "
            "ResponsiveProtectedCandidateOwned predicate"
        )

    theorem_contracts = {
        "HistoricalProtectedServiceRankProgressFromStageLeaves": (
            "\\A specification: "
            "HistoricalProtectedServiceRankLeafProperties(specification) "
            "=> HistoricalProtectedServiceRankProgressProperty(specification)",
            (
                "HistoricalProtectedStage2RankProgressProperty",
                "HistoricalProtectedStage3RankProgressProperty",
                "HistoricalProtectedStage4RankProgressProperty",
                "HistoricalProtectedStage5RankProgressProperty",
                "HistoricalProtectedStage6RankProgressProperty",
                "HistoricalProtectedStageRankProgressProperty",
            ),
        ),
        "HistoricalProtectedCandidateHasServiceRank": (
            "\\A candidate: /\\ AsyncTypeInvariant /\\ gst "
            "/\\ HistoricalProtectedCandidateOwned(candidate) "
            "=> \\E rank \\in OwnedServiceRankCarrier: "
            "HistoricalProtectedOwnedAtServiceRank(candidate, rank)",
            (
                "ScheduledCandidateServiceRankInCarrier",
                "HistoricalProtectedOwnedAtServiceRank",
            ),
        ),
        "HistoricalProtectedServiceRankProgressImpliesStarvation": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalProtectedServiceRankProgressProperty( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalProtectedCandidateStarvationProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "OwnedServiceRankOrderingWellFounded",
                "WellFoundedLeadsTo",
                "HistoricalProtectedCandidateHasServiceRank",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryReadinessFromClock": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalRecoveryTargetRemoteServerProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalCommitCertificateDiscoveryClockProgressProperty( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node)) "
            "~> (HistoricalCommitCertificateDiscoveryPending(node) "
            "\\/ HistoricalCommitCertificateDiscoveryOutcome(node))",
            (
                "DEF HistoricalRecoveryTargetRemoteServerProperty",
                "DEF HistoricalCommitCertificateDiscoveryClockProgressProperty",
                "HistoricalRecoveryTargetRemoteServerInvariant",
                "HistoricalCommitCertificateDiscoveryPending",
                "HistoricalCommitCertificateDiscoveryOutcome",
            ),
        ),
        "DirectHistoricalCommitCertificateDiscoveryPublishes": (
            "\\A node \\in ValidatorIds: "
            "DirectHistoricalCommitCertificateDiscoveryStep(node) "
            "=> /\\ HistoricalRecoveryTarget(node)' "
            "/\\ ActiveCommitCertificateRequests(node)' # {}",
            (
                "CommitCertificateDiscoveryStepWork",
                "PublishCommitCertificateRequests",
                "ActiveCommitCertificateRequests",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryPrefixIsEnabled": (
            "\\A node \\in ValidatorIds: "
            "HistoricalCommitCertificateDiscoveryDue(node) "
            "=> ENABLED DirectHistoricalCommitCertificateDiscoveryStep(node)",
            (
                "ExpandENABLED",
                "DirectHistoricalCommitCertificateDiscoveryStep",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryPendingEnablesFairPrefix": (
            "\\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPending(node) "
            "=> ENABLED "
            "<<PostGstHistoricalCommitCertificateDiscovery(node)>>_AsyncAllVars",
            (
                "HistoricalRecoveryTargetsAreValidators",
                "HistoricalCommitCertificateDiscoveryPrefixIsEnabled",
                "DirectHistoricalCommitCertificateDiscoveryPublishes",
                "ENABLEDaxioms",
            ),
        ),
        "HistoricalCommitCertificateDiscoveryFairStepPublishes": (
            "\\A node \\in Responsive: "
            "/\\ HistoricalCommitCertificateDiscoveryPending(node) "
            "/\\ <<PostGstHistoricalCommitCertificateDiscovery(node)>>_AsyncAllVars "
            "=> HistoricalCommitCertificateDiscoveryOutcome(node)'",
            (
                "DirectHistoricalCommitCertificateDiscoveryPublishes",
                "HistoricalCommitCertificateDiscoveryOutcome",
            ),
        ),
        "FairHistoricalCommitCertificateDiscoveryFromPersistence": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalCommitCertificateDiscoveryPersistenceProperty( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "HistoricalCommitCertificateDiscoveryPending(node) "
            "~> HistoricalCommitCertificateDiscoveryOutcome(node)",
            (
                "HistoricalCommitCertificateDiscoveryPendingEnablesFairPrefix",
                "HistoricalCommitCertificateDiscoveryFairStepPublishes",
                "HistoricalCommitCertificateDiscoveryPersistenceUnless",
                "HistoricalCommitCertificateDiscoveryPersistenceProperty",
                "WF_AsyncAllVars(",
                "PostGstHistoricalCommitCertificateDiscovery(node)",
            ),
        ),
        "HistoricalActiveCommitCertificateRequestReachesDecision": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalProtectedServiceRankLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalCommitCertificateConcreteLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ ActiveCommitCertificateRequests(node) # {}) "
            "~> NodeHasDecision(node)",
            (
                "HistoricalProtectedServiceRankProgressFromStageLeaves",
                "HistoricalProtectedServiceRankProgressImpliesStarvation",
                "StarvationFreedomObligation",
                "HistoricalActiveRequestRetransmissionProgressLeaf",
                "HistoricalCommitRequestServeProgressLeaf",
                "HistoricalCommitResponseAdmissionProgressLeaf",
                "HistoricalCommitDeliveryProgressLeaf",
                "HistoricalBeginDecisionProgressLeaf",
                "HistoricalPersistDecisionProgressLeaf",
            ),
        ),
        "HistoricalTargetDecisionReachesApplicationFromConcreteLeaves": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalProtectedServiceRankLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalDecisionFrontierAvailabilityProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalDecisionConcreteLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "=> \\A node \\in Responsive: "
            "(gst /\\ HistoricalRecoveryTarget(node) "
            "/\\ NodeHasDecision(node)) ~> NodeHasApplication(node)",
            (
                "HistoricalProtectedServiceRankProgressFromStageLeaves",
                "HistoricalProtectedServiceRankProgressImpliesStarvation",
                "StarvationFreedomObligation",
                "HistoricalDecisionFrontierAvailabilityProperty",
                "HistoricalDecisionFetchProgressLeaf",
                "HistoricalDecisionRequestBodyProgressLeaf",
                "HistoricalDecisionCertifiedResponseProgressLeaf",
                "HistoricalDecisionFetchCertifiedProgressLeaf",
                "HistoricalDecisionStoreProgressLeaf",
                "HistoricalDecisionValidateProgressLeaf",
                "HistoricalDecisionApplyProgressLeaf",
            ),
        ),
        "HistoricalRecoveryTargetDecisionFromExactCorridor": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalRecoveryAsyncTemporalClosurePremises( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalRecoveryTargetDecisionProgressProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "FairHistoricalCommitCertificateDiscoveryFromPersistence",
                "HistoricalCommitCertificateDiscoveryReadinessFromClock",
                "HistoricalActiveCommitCertificateRequestReachesDecision",
                "HistoricalCommitCertificateDiscoveryOutcome",
            ),
        ),
        "ResponsiveDecisionApplicationFromExactCorridor": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalRecoveryAsyncTemporalClosurePremises( "
            "AsyncSpecAt(initialContext)) "
            "=> ResponsiveDecisionApplicationProgressProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "HistoricalTargetDecisionReachesApplicationFromConcreteLeaves",
                "ApplicationCompletionProgressProperty",
                "ResponsiveDecisionServiceOwnershipProperty",
                "AsyncSpecAlwaysUsesFixedResponsiveVoters",
            ),
        ),
        "HistoricalRecoveryAsyncTemporalPrerequisitesFromExactCorridor": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ ProtectedServiceFiniteRunnerEpisodeClosureProperty( "
            "AsyncSpecAt(initialContext)) "
            "/\\ HistoricalRecoveryAsyncTemporalClosurePremises( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalRecoveryAsyncTemporalPrerequisites( "
            "AsyncSpecAt(initialContext))",
            (
                "HistoricalRecoveryTargetDecisionFromExactCorridor",
                "ResponsiveDecisionApplicationFromExactCorridor",
            ),
        ),
        "HistoricalLockedBodyRecoveryConeComposesFromExactLeaves": (
            "\\A initialContext: /\\ AsyncSpecAt(initialContext) "
            "/\\ HistoricalLockedBodyRecoveryConeLeafProperties( "
            "AsyncSpecAt(initialContext)) "
            "=> HistoricalLockedBodyRecoveryConeProperty( "
            "AsyncSpecAt(initialContext))",
            (
                "AsyncSpecAlwaysHistoricalLockedBodyRecoveryStage",
                "HistoricalLockedCommitCarrierRecoveryProgressLeaf",
                "HistoricalLockedRestartRecoveryProgressLeaf",
                "HistoricalLockedFetchRecoveryProgressLeaf",
                "HistoricalLockedRequestCandidateProgressLeaf",
                "HistoricalLockedActiveRequestProgressLeaf",
                "HistoricalLockedCertifiedFetchProgressLeaf",
                "HistoricalLockedStoreRecoveryProgressLeaf",
                "HistoricalLockedValidateRecoveryProgressLeaf",
                "HistoricalLockedBodyRecoveryStageInvariant",
                "HistoricalLockedBodyRecoveryOutcome",
                "PTL",
            ),
        ),
    }

    for symbol, (exact_statement, proof_tokens) in theorem_contracts.items():
        extracted = _top_level_theorem_body(
            raw_source, symbol, preserve_string_contents=True
        )
        if extracted is None:
            errors.append(f"{path}: missing Async historical theorem {symbol}")
            continue
        body, line = extracted
        parts = re.split(
            r"(?m)^[ \t]*(?:BY|PROOF|OBVIOUS)\b", body, maxsplit=1
        )
        statement = " ".join(parts[0].split())
        if statement != exact_statement:
            errors.append(
                f"{path}:{line}: {symbol} must state only "
                f"{exact_statement!r}; found {statement!r}"
            )
        proof = parts[1] if len(parts) == 2 else ""
        missing = tuple(
            token
            for token in proof_tokens
            if not _tla_dependency_present(proof, token)
        )
        vacuous = re.search(
            r"(?:\bASSUME\s+FALSE\b|\bPROVE\s+TRUE\b|\bBY\s+TRUE\b)",
            proof,
        )
        if len(parts) != 2 or missing or vacuous is not None:
            errors.append(
                f"{path}:{line}: {symbol} proof must retain exact historical "
                "dependencies without a vacuous proof; "
                f"missing={missing!r}, vacuous={vacuous is not None}, "
                f"has_proof={len(parts) == 2}"
            )

    return errors
