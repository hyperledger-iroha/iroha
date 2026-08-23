# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _lifecycle_decision_apply_lineage_source_fidelity_errors(
    repo_root: Path,
) -> list[str]:
    """Bind both concrete Apply lineages to one neutral worker/terminal corridor."""

    errors: list[str] = []

    def load(relative: str) -> tuple[Path, str]:
        return _read_reviewed_rust_source(
            repo_root,
            relative,
            errors,
            "lineage-aware lifecycle Decision Apply source",
        )

    def require_order(
        path: Path,
        item: RustItem | None,
        markers: tuple[str, ...],
        description: str,
    ) -> None:
        if item is None:
            return
        tokens = rust_code_tokens(item.source)
        cursor = 0
        for marker in markers:
            marker_tokens = rust_code_tokens(marker)
            positions = tuple(
                index
                for index in range(cursor, len(tokens) - len(marker_tokens) + 1)
                if tokens[index : index + len(marker_tokens)] == marker_tokens
            )
            if not positions:
                errors.append(
                    f"{path}:{item.line}: {description} must contain ordered "
                    f"{marker!r}"
                )
                return
            cursor = positions[0] + len(marker_tokens)

    def reject_aliases(
        path: Path,
        source: str,
        aliases: tuple[str, ...],
        description: str,
    ) -> None:
        tokens = rust_code_tokens(source)
        observed = tuple(
            alias
            for alias in aliases
            if _token_sequence_count(tokens, rust_code_tokens(alias)) != 0
        )
        if observed:
            errors.append(
                f"{path}: {description} retains retired recovered-only aliases "
                f"{observed}"
            )

    registry_path, registry_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry.rs"
    )
    registry_impl_path, registry_impl_source = load(
        "crates/iroha_core/src/sumeragi/"
        "v2_lifecycle_work_registry_validate_recovery_registry_impl.rs"
    )
    scheduler_path, scheduler_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_scheduler_inputs.rs"
    )
    schema_path, schema_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_schema.rs"
    )
    adapter_path, adapter_source = load(
        "crates/iroha_core/src/sumeragi/v2.rs"
    )
    effects_path, effects_source = load(
        "crates/iroha_core/src/sumeragi/v2_effects.rs"
    )
    worker_path, worker_source = load(
        "crates/iroha_core/src/sumeragi/v2_worker.rs"
    )
    worker_services_path, worker_services_source = load(
        "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs"
    )
    launch_path, launch_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"
    )
    lane_path, lane_source = load(
        "crates/iroha_core/src/sumeragi/v2_lane_work.rs"
    )

    _require_rust_source_token_sequence(
        registry_path,
        registry_source,
        """
pub(in crate::sumeragi) enum LifecycleDecisionApplyLineageV1 {
    Live,
    Recovered,
}
""",
        "lifecycle Decision Apply must retain a closed live/recovered lineage",
        errors,
    )
    key_matchers = tuple(
        item
        for item in rust_items(registry_source, "matches")
        if item.brace_context
        == (("impl", "LifecycleDecisionApplyDispatchKeyV1"),)
    )
    if len(key_matchers) != 1:
        errors.append(
            f"{registry_path}: require exactly one full-coordinate "
            "LifecycleDecisionApplyDispatchKeyV1::matches item; found "
            f"{len(key_matchers)}"
        )
    key_matches = key_matchers[0] if len(key_matchers) == 1 else None
    _require_rust_token_sequence(
        registry_path,
        key_matches,
        """
self.context == context.id()
    && self.height == context.height()
    && self.owner == address.owner
    && self.ordinal == address.ordinal
    && self.slot == address.slot
    && self.digest == digest
    && self.lineage == lineage
""",
        "lifecycle Decision Apply key must reject every isolated carrier-coordinate substitution",
        errors,
    )
    _require_rust_source_token_sequence(
        registry_path,
        registry_source,
        """
pub(in crate::sumeragi) struct LifecycleDecisionApplyDispatchKeyV1 {
    context: LifecycleDigest,
    height: u64,
    owner: OwnerId,
    ordinal: u128,
    slot: PhysicalSlotId,
    digest: LifecycleDigest,
    lineage: LifecycleDecisionApplyLineageV1,
}
""",
        "lifecycle Decision Apply worker key must retain every carrier coordinate and lineage",
        errors,
    )
    _require_rust_source_token_sequence(
        registry_path,
        registry_source,
        "impl Drop for LifecycleDecisionApplyDispatchLinearity",
        "lifecycle Decision Apply dispatch identity must remain move-only",
        errors,
    )

    classifier = _require_rust_item(
        registry_impl_path,
        registry_impl_source,
        "attest_ready_lifecycle_decision_apply",
        errors,
    )
    _require_rust_token_sequence(
        registry_impl_path,
        classifier,
        """
let (carrier_matches, lineage, dispatch_key) = match &work.kind {
    ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) => (
        apply.matches_current_ready_record(address, digest, coordinator),
        LifecycleDecisionApplyLineageV1::Live,
        apply.dispatch_key,
    ),
    ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply) => (
        apply.matches_current_ready_record(address, digest, coordinator),
        LifecycleDecisionApplyLineageV1::Recovered,
        apply.dispatch_key,
    ),
    _ => return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::WrongWorkKind),
};
if !carrier_matches || dispatch_key.is_some() {
""",
        "lineage-aware Apply classifier must distinguish both exact undispatched carriers",
        errors,
    )

    live_reconciliation = _require_rust_item(
        registry_impl_path,
        registry_impl_source,
        "prepare_ready_live_decision_apply_reconciliation",
        errors,
    )
    _require_rust_token_sequence(
        registry_impl_path,
        live_reconciliation,
        """
let attestation = self.attest_ready_lifecycle_decision_apply(coordinator, ordinal)?;
let dispatch_key = attestation.dispatch_key();
if dispatch_key.lineage() == LifecycleDecisionApplyLineageV1::Recovered {
    return Ok(None);
}
""",
        "live Apply reconciliation authority must derive from the exact neutral attestation and reject recovered substitution",
        errors,
    )
    _require_rust_token_sequence(
        registry_impl_path,
        live_reconciliation,
        """
let ConcreteLifecycleWorkKind::DurableLiveWalApply(apply) = &work.kind else {
    return Err(ReadyLifecycleDecisionApplyAttestationErrorV1::WrongWorkKind);
};
""",
        "live Apply reconciliation authority must originate only from the live WAL carrier",
        errors,
    )
    _require_rust_token_sequence(
        registry_impl_path,
        live_reconciliation,
        "apply.project_reconciliation(dispatch_key)",
        "live Apply reconciliation must retain the exact carrier key",
        errors,
    )

    dispatch = _require_rust_item(
        registry_impl_path,
        registry_impl_source,
        "prepare_lifecycle_decision_apply_dispatch",
        errors,
    )
    require_order(
        registry_impl_path,
        dispatch,
        (
            "ConcreteLifecycleWorkKind::DurableLiveWalApply(apply)",
            "LifecycleDecisionApplyLineageV1::Live",
            ".project_task(identity)",
            "ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)",
            "LifecycleDecisionApplyLineageV1::Recovered",
            ".project_recovered_apply_task(identity, address)",
            "PreparedLifecycleDecisionApplyDispatchV1",
        ),
        "lineage-aware Apply dispatch",
    )

    terminal_prepare = _require_rust_item(
        registry_impl_path,
        registry_impl_source,
        "prepare_lifecycle_decision_apply_terminal_transition",
        errors,
    )
    _require_rust_token_sequence(
        registry_impl_path,
        terminal_prepare,
        """
ConcreteLifecycleWorkKind::DurableLiveWalApply(apply)
    if apply.matches_claimed_record(address, digest, coordinator, lease)
        && apply.dispatch_key == Some(dispatch_key)
        && dispatch_key.matches(
            coordinator.active_context,
            address,
            digest,
            LifecycleDecisionApplyLineageV1::Live,
        )
""",
        "terminal Apply rejoin must authenticate the exact live carrier and lineage",
        errors,
    )
    _require_rust_token_sequence(
        registry_impl_path,
        terminal_prepare,
        """
ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)
    if apply.matches_claimed_record(address, digest, coordinator, lease)
        && apply.dispatch_key == Some(dispatch_key)
        && dispatch_key.matches(
            coordinator.active_context,
            address,
            digest,
            LifecycleDecisionApplyLineageV1::Recovered,
        )
""",
        "terminal Apply rejoin must authenticate the exact recovered carrier and lineage",
        errors,
    )

    terminal_publish = _require_rust_item(
        registry_impl_path,
        registry_impl_source,
        "publish_lifecycle_decision_apply_terminal_transition",
        errors,
    )
    require_order(
        registry_impl_path,
        terminal_publish,
        (
            "let carrier_matches = match (&work.kind, prepared.lineage)",
            "ConcreteLifecycleWorkKind::DurableLiveWalApply(apply)",
            "LifecycleDecisionApplyLineageV1::Live",
            "ConcreteLifecycleWorkKind::DurableRecoveredDecisionApply(apply)",
            "LifecycleDecisionApplyLineageV1::Recovered",
            "let exact_current =",
            "let exact_staged =",
            "if !exact_current || !exact_staged",
            "match publish()",
            ".remove(&prepared.address)",
        ),
        "lineage-specific Apply terminal publication",
    )

    scheduler_dispatch = _require_rust_item(
        scheduler_path,
        scheduler_source,
        "dispatch_completion_with_runner_debt_and_required_ordinal",
        errors,
    )
    require_order(
        scheduler_path,
        scheduler_dispatch,
        (
            "prepare_ready_live_decision_apply_reconciliation(&self.coordinator, ordinal)",
            "executor.exactly_owns_live_lifecycle_decision_apply(&authority)",
            "let fence = executor.lifecycle_reducer_fence_observation()",
            "attest_ready_lifecycle_decision_apply(&self.coordinator, *ordinal)",
            "services.capture_lifecycle_completion_capacity_census(probes)",
            ".select_apply(ordinal)",
            ".prepare_lifecycle_decision_apply_dispatch(&self.coordinator, &lease)",
        ),
        "live reconciliation, complete Apply census, and neutral worker publication",
    )
    _require_rust_token_sequence(
        scheduler_path,
        scheduler_dispatch,
        """
if !reservation.preflight(&prepared) {
    return Err(ProductionCompletionDispatchErrorV1::ReservedOwnerMismatch);
}
let executor_dispatch = executor
    .prepare_lifecycle_decision_apply_executor_dispatch(&prepared)
    .map_err(ProductionCompletionDispatchErrorV1::LiveApplyReconciliation)?;
reservation.commit(prepared, executor_dispatch);
Ok(ProductionCompletionDispatchV1::ApplyQueued { ordinal })
""",
        "neutral Apply reservation must join executor evidence before one-shot queue publication",
        errors,
    )
    _require_rust_source_token_sequence(
        scheduler_path,
        scheduler_source,
        "InvalidLifecycleDecisionApplyCarrier",
        "scheduler Apply carrier failure must use the lifecycle-neutral class",
        errors,
        count=2,
    )

    schema_rows = tuple(
        item
        for item in rust_items(schema_source, "from_authenticated")
        if _token_sequence_count(
            rust_code_tokens(item.source),
            rust_code_tokens("lifecycle_decision_apply_attestation"),
        )
    )
    if len(schema_rows) != 1:
        errors.append(
            f"{schema_path}: require exactly one authenticated scheduler row "
            f"with lifecycle-neutral Apply authority; found {len(schema_rows)}"
        )
    schema_row = schema_rows[0] if len(schema_rows) == 1 else None
    _require_rust_token_sequence(
        schema_path,
        schema_row,
        """
LifecycleWorkClass::Apply => {
    validate_attestation.is_none()
        && recovered_sign_attestation.is_none()
        && recovered_fetch_attestation.is_none()
        && lifecycle_decision_apply_attestation
            .as_ref()
            .is_some_and(|attestation| attestation.matches_ready_record(record))
}
""",
        "scheduler schema must bind Apply through the lifecycle-neutral attestation local",
        errors,
    )
    schema_capacity_row = _require_rust_item(
        schema_path,
        schema_source,
        "from_authenticated_with_physical_capacity",
        errors,
    )
    _require_rust_token_sequence(
        schema_path,
        schema_capacity_row,
        """
Self::from_authenticated(
    factory,
    record,
    validate_attestation,
    lifecycle_decision_apply_attestation,
    recovered_sign_attestation,
    recovered_fetch_attestation,
    live_debts,
)
""",
        "physical-capacity schema must preserve the lifecycle-neutral Apply attestation",
        errors,
    )

    live_projection = _require_rust_item(
        adapter_path,
        adapter_source,
        "project_live_decision_apply_completion",
        errors,
    )
    _require_rust_token_sequence(
        adapter_path,
        live_projection,
        """
project_lifecycle_decision_apply_completion(
    permit,
    LifecycleDecisionApplyLineageV1::Live,
    context,
    address,
    installed_digest,
    effect,
    validated_receipt,
    completion,
)
""",
        "live Apply completion projection must enter the shared worker corridor with live lineage",
        errors,
    )
    shared_projection = _require_rust_item(
        adapter_path,
        adapter_source,
        "project_lifecycle_decision_apply_completion",
        errors,
    )
    _require_rust_token_sequence(
        adapter_path,
        shared_projection,
        """
if !key.matches_carrier(context, address, installed_digest, lineage)
    || completion.subject() != *subject
    || completion.certificate() != certificate
    || completion.validated_receipt() != validated_receipt
""",
        "shared Apply completion projection must rejoin the exact lineage-tagged carrier",
        errors,
    )

    executor_prepare = _require_rust_item(
        effects_path,
        effects_source,
        "prepare_lifecycle_decision_apply_completion",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        executor_prepare,
        """
let lineage_owner_is_exact = match authority.lineage() {
    LifecycleDecisionApplyLineageV1::Live => self
        .live_lifecycle_decision_apply
        .as_ref()
        .is_some_and(|owner| {
            owner.exactly_matches_completion(
                authority.dispatch_key(),
                authority.tag(),
                authority.subject(),
                authority.receipt(),
                authority.artifact(),
            )
        }),
    LifecycleDecisionApplyLineageV1::Recovered => {
        self.live_lifecycle_decision_apply.is_none()
    }
};
""",
        "executor preparation must distinguish exact live ownership from recovered non-substitution",
        errors,
    )
    _require_rust_token_sequence(
        effects_path,
        executor_prepare,
        "|| !lineage_owner_is_exact",
        "executor preparation must reject an authority-only lineage substitution",
        errors,
    )

    _require_rust_source_token_sequence(
        worker_path,
        worker_source,
        "V2IoCommand::LifecycleDecisionApply(task)",
        "worker command queue must retain the neutral lifecycle Apply variant",
        errors,
        count=3,
    )
    _require_rust_source_token_sequence(
        worker_path,
        worker_source,
        "V2IoCompletion::LifecycleDecisionApply(guarded)",
        "worker completion queue must retain the neutral lifecycle Apply variant",
        errors,
        count=4,
    )
    select_apply = _require_rust_item(
        worker_path,
        worker_source,
        "select_apply",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        select_apply,
        """
let Some(LifecycleCompletionPreparedCapacityV1::Apply {
    key,
    available: true,
}) = self.candidates.remove(&ordinal)
""",
        "neutral lifecycle Apply selection must consume only the frozen exact row",
        errors,
    )
    _require_rust_token_sequence(
        worker_path,
        select_apply,
        """
Ok(LifecycleDecisionApplyCapacityReservationV1 {
    queue: self.queue,
    state: Some(state),
    operation: Some(operation),
    key,
})
""",
        "neutral lifecycle Apply selection must preserve its exact queue key",
        errors,
    )
    capture = _require_rust_item(
        worker_services_path,
        worker_services_source,
        "capture_lifecycle_completion_capacity_census",
        errors,
    )
    _require_rust_token_sequence(
        worker_services_path,
        capture,
        """
LifecycleCompletionCapacityProbeV1::Apply {
    ordinal,
    key,
    executor_available,
} => {
    if key.lifecycle_ordinal() != ordinal
        || !key.matches_height_context(&self.context)
        || !apply_keys.insert(key)
""",
        "shared lifecycle census must bind each Apply probe to one exact height-local key",
        errors,
    )
    settlement = _require_rust_item(
        launch_path,
        launch_source,
        "settle_lifecycle_decision_apply_completion_owner",
        errors,
    )
    require_order(
        launch_path,
        settlement,
        (
            "LifecycleDecisionApplyWorkerResultV1::Deferred",
            "completion.authorizes_sidecar_owner(services, lane_work)",
            "sidecar.register(lane_work)",
            "settle_applied_lifecycle_decision_apply_completion_with_status(",
            "LifecycleDecisionApplyStatusPublicationV1::PublishActiveHeight",
        ),
        "neutral lifecycle Apply result classification and live-height publication mode",
    )
    shared_settlement = _require_rust_item(
        launch_path,
        launch_source,
        "settle_applied_lifecycle_decision_apply_completion_with_status",
        errors,
    )
    require_order(
        launch_path,
        shared_settlement,
        (
            "prepare_lifecycle_decision_apply_terminal_transition",
            "executor.prepare_lifecycle_decision_apply_completion(authority)",
            "publish_lifecycle_decision_apply_terminal_transition",
            "persist_exact_staged_successor(&staged)",
            "owner.coordinator = staged",
            "adapter.commit_after_durable_settlement()",
            "executor.commit_lifecycle_decision_apply_finality(finality)",
            "completion.acknowledge_after_owner_settlement()",
            "LifecycleDecisionApplyStatusPublicationV1::PublishActiveHeight",
            "super::super::status::set_v2_status(status)",
        ),
        "neutral lifecycle Apply durable terminal settlement and mode-gated publication",
    )
    pending_settlement = _require_rust_item(
        launch_path,
        launch_source,
        "settle_pending_kura_applied_decision_apply_completion",
        errors,
    )
    _require_rust_token_sequence(
        launch_path,
        pending_settlement,
        "LifecycleDecisionApplyStatusPublicationV1::DeferUntilPendingKuraActivation",
        "pending-Kura lifecycle Apply must defer status until no-clock activation",
        errors,
    )

    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        "fn defer_missing_lifecycle_decision_apply_sidecar(",
        "lane work must expose only the neutral lifecycle Apply sidecar owner",
        errors,
    )
    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        "lifecycle_decision_apply_sidecar_waits: BTreeSet<HashOf<MergeLedgerEntry>>",
        "lane work must retain the lifecycle-neutral Apply sidecar wait owner",
        errors,
    )
    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        "rejected_lifecycle_decision_apply_sidecars: BTreeMap<HashOf<MergeLedgerEntry>, String>",
        "lane work must retain the lifecycle-neutral Apply sidecar rejection owner",
        errors,
    )
    _require_rust_source_token_sequence(
        lane_path,
        lane_source,
        "fn dispatch_next_lifecycle_decision_apply_sidecar_request(",
        "lane work must expose only the lifecycle-neutral Apply sidecar dispatcher",
        errors,
    )
    lane_constructor = _require_rust_item(
        lane_path,
        lane_source,
        "new_with_output_guard_and_transport_inner",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_constructor,
        """
lifecycle_decision_apply_sidecar_waits: BTreeSet::new(),
rejected_lifecycle_decision_apply_sidecars: BTreeMap::new(),
""",
        "lane construction must initialize distinct neutral lifecycle Apply wait and rejection owners",
        errors,
    )
    sidecar_drive = _require_rust_item(
        launch_path,
        launch_source,
        "drive_lifecycle_decision_apply_deferred",
        errors,
    )
    _require_rust_token_sequence(
        launch_path,
        sidecar_drive,
        "lane_work.dispatch_next_lifecycle_decision_apply_sidecar_request",
        "deferred lifecycle Apply must use the lifecycle-neutral sidecar dispatcher",
        errors,
    )

    reject_aliases(
        registry_path,
        registry_source,
        (
            "ReadyRecoveredDecisionApplyAttestation",
            "RecoveredDecisionApplyDispatchKeyV1",
            "RecoveredDecisionApplyDispatchIdentityV1",
        ),
        "registry Apply corridor",
    )
    reject_aliases(
        worker_path,
        worker_source,
        (
            "V2IoCommand::RecoveredDecisionApply",
            "V2IoCompletion::RecoveredDecisionApply",
            "RecoveredDecisionApplyCapacityReservationV1",
            "RecoveredCompletionCapacityCensusV1",
        ),
        "worker Apply corridor",
    )
    reject_aliases(
        launch_path,
        launch_source,
        (
            "settle_recovered_decision_apply_completion_owner",
            "RetainedRecoveredDecisionApplyDeferredV1",
            "drive_recovered_decision_apply_deferred",
        ),
        "terminal Apply corridor",
    )
    reject_aliases(
        lane_path,
        lane_source,
        (
            "defer_missing_recovered_decision_apply_sidecar",
            "recovered_apply_sidecar_waits",
            "rejected_recovered_apply_sidecars",
            "dispatch_next_recovered_apply_sidecar_request",
        ),
        "sidecar Apply corridor",
    )
    reject_aliases(
        scheduler_path,
        scheduler_source,
        ("InvalidRecoveredDecisionApplyCarrier",),
        "scheduler Apply failure corridor",
    )
    reject_aliases(
        schema_path,
        schema_source,
        ("recovered_apply_attestation",),
        "scheduler schema Apply corridor",
    )
    return errors

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
    height_binding_path, height_binding_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner.rs"
    )
    lifecycle_runner_path, lifecycle_runner_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_run_inner.rs"
    )
    pending_runner_path, pending_runner_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"
    )
    ordinary_consumer_path, ordinary_consumer_source = load(
        "crates/iroha_core/src/sumeragi/v2_runner/ordinary_ingress_consumer.rs"
    )
    if height_binding_source:
        for item_name, expected_sha256 in (
            _PRODUCTION_RECOVERY_EAGER_BLOCK_SYNC_ITEM_SHA256.items()
        ):
            item = _require_rust_item(
                height_binding_path,
                height_binding_source,
                item_name,
                errors,
            )
            _require_rust_item_context(
                height_binding_path,
                item,
                (),
                f"recovery-scoped eager block-sync {item_name} production item",
                errors,
            )
            _require_rust_item_token_sha256(
                height_binding_path,
                item,
                expected_sha256,
                f"recovery-scoped eager block-sync {item_name}",
                errors,
            )
    if runner_source:
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
let pending_kura_apply = recovered.pending_kura_apply();
let (
    verified_context,
    context_store,
    signature_policy,
    lifecycle_storage_authority,
    first_height_authenticated_genesis,
    recovered_successor_activation,
    staged_genesis_nexus_amx_context,
) = recovered.into_parts();
""",
            "durable recovered ownership must retain the recovered successor owners",
            errors,
        )
        _require_rust_token_sequence(
            runner_path,
            run_inner_item,
            """
let eager_block_sync =
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
                    "let pending_kura_apply = recovered.pending_kura_apply();",
                    ") = recovered.into_parts();",
                    "let eager_block_sync =",
                    "recovered_successor_activation.is_some() || pending_kura_apply.is_some();",
                ),
            )
        active_height = _require_rust_item(
            lifecycle_runner_path,
            lifecycle_runner_source,
            "run_lifecycle_active_height",
            errors,
        )
        _require_rust_token_sequence(
            lifecycle_runner_path,
            active_height,
            """
let mut next_block_sync_attempt =
    initial_block_sync_deadline(height_started_at, round_timeout, *eager_block_sync);
""",
            "height startup must derive its first block-sync deadline from the recovery hint",
            errors,
        )
        _require_rust_token_sequence(
            lifecycle_runner_path,
            active_height,
            """
let discovery_was_outstanding = if lane_only_completion_barrier {
    block_sync_request.is_some()
} else {
    activated.with_runner_runtime(
""",
            "serialized lifecycle ownership must preserve the Apply barrier while sampling the outstanding discovery request",
            errors,
        )
        if active_height is not None:
            require_order(
                lifecycle_runner_path,
                "only authenticated discovered CommitQC admission/coalescing may retain eager block-sync",
                active_height.source,
                (
                    "Ok::<_, V2RunnerError>(block_sync_request.is_some())",
                    "drain_lifecycle_v2_ingress(",
                    "if discovery_was_outstanding && block_sync_request.is_none()",
                    "admitted_discovered_commit_qc = true",
                    "*eager_block_sync = retain_eager_block_sync(false, admitted_discovered_commit_qc)",
                ),
            )
            require_order(
                lifecycle_runner_path,
                "ordinary lifecycle successor handoff",
                active_height.source,
                (
                    "DurableV2PredecessorIdentity::authenticate(artifact, receipt)",
                    "PendingSuccessorConstruction::begin(predecessor)",
                    "build_verified_successor(",
                    "into_parts_with_lifecycle_storage_authority(",
                    "activation.bind(successor_authority)",
                    "retain_eager_block_sync(false, admitted_discovered_commit_qc)",
                ),
            )
        construction_begin = _require_qualified_rust_item(
            runner_path,
            runner_source,
            "PendingSuccessorConstruction",
            "begin",
            errors,
            "applied successor construction begin",
        )
        construction_bind = _require_qualified_rust_item(
            runner_path,
            runner_source,
            "PendingSuccessorConstruction",
            "bind",
            errors,
            "applied successor construction bind",
        )
        construction = "\n".join(
            item.source
            for item in (construction_begin, construction_bind)
            if item is not None
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
            lifecycle_runner_path,
            lifecycle_runner_source,
            "PendingSuccessorActivation",
            "pub(super) enum PendingSuccessorActivation",
            "#[derive(Clone, Copy, Debug, PartialEq, Eq)]\nenum CanonicalRecoveryControlV1",
        )
        recovered_activation = _require_qualified_rust_item(
            lifecycle_runner_path,
            lifecycle_runner_source,
            "PendingSuccessorActivation",
            "recovered",
            errors,
            "recovered successor activation",
        )
        require_tokens(
            lifecycle_runner_path,
            "PendingSuccessorActivation",
            activation,
            (
                "RecoveredSuccessorActivationAuthority::CompleteTip(authority)",
                "RecoveredSuccessorActivationAuthority::SnapshotBootstrap(authority)",
                "let published_height = super::super::status::v2_status().map_or(0, |status| status.height);",
                "stage_before: SUCCESSOR_STAGE_NONE, stage_after: SUCCESSOR_STAGE_NONE, published_height_before: published_height, published_height_after: published_height, restart_required_before: false, restart_required_after: false,",
                "let Some(checked_lifecycle) = check_production_successor_startup_lifecycle_transition(lifecycle) else",
                "return Err(V2RunnerError::SuccessorRefinementRejected);",
                "let _authorized_lifecycle = checked_lifecycle.into_projection();",
                "super::super::status::activate_v2_successor_height( expected_predecessor, authority, successor, )?;",
                "authority.into_canonical_predecessor_storage(local_signer)?",
                ".retire()?",
                "Self::RecoveredCompleteTip { authority: retired }",
                "authority.authorizes_retained_successor()",
                "authority.authorizes_successor_status(successor)",
                "V2RunnerError::CompleteTipSuccessorAuthorityInvalid",
                "predecessor: authority.predecessor()",
                "super::super::status::activate_recovered_complete_tip_v2_height( authority, successor, )?;",
                "super::super::status::activate_snapshot_bootstrap_v2_height(authority, successor)?;",
            ),
        )
        require_order(
            lifecycle_runner_path,
            "PendingSuccessorActivation::recovered",
            recovered_activation.source if recovered_activation is not None else "",
            (
                "match &authority",
                "let published_height = super::super::status::v2_status()",
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
            lifecycle_runner_path,
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
                "match pending_kura_apply",
            ),
        )
        require_order(
            runner_path,
            "run_inner lifecycle branch ownership",
            run_inner,
            (
                "match pending_kura_apply",
                "None => lifecycle_run_inner::run_non_pending_lifecycle_loop(",
                "Some(pending) => lifecycle_pending_kura::run_pending_kura_lifecycle_height(",
            ),
        )
        require_tokens(
            runner_path,
            "runner retains recovered lifecycle storage authority",
            run_inner,
            (
                "lifecycle_storage_authority",
                "first_height_authenticated_genesis",
            ),
        )
        require_token_count(
            runner_path,
            "runner dispatches the recovery-scoped eager block-sync owner to exactly one lifecycle branch",
            run_inner,
            "eager_block_sync",
            3,
        )
        non_pending_loop = _require_rust_item(
            lifecycle_runner_path,
            lifecycle_runner_source,
            "run_non_pending_lifecycle_loop",
            errors,
        )
        if non_pending_loop is not None:
            require_order(
                lifecycle_runner_path,
                "non-pending lifecycle live successor startup",
                non_pending_loop.source,
                (
                    "SumeragiV2Adapter::open_recovered_startup_with_capacity_geometry(",
                    "authenticate_final_wal_startup_authority()",
                    "bind_production_lifecycle_owner_factory_inputs_v1(",
                    "open_production_lifecycle_owner_v1(",
                    "launch_non_pending_lifecycle_height(",
                    "initialize_recovered_local_proposal(setup_runner)",
                    "preactivation.activate(height_started_at, local_proposal)",
                    "run_lifecycle_active_height(",
                ),
            )
        pending_loop = _require_rust_item(
            pending_runner_path,
            pending_runner_source,
            "run_pending_kura_lifecycle_height",
            errors,
        )
        if pending_loop is not None:
            require_order(
                pending_runner_path,
                "pending-Kura lifecycle recovery enters the ordinary live successor loop",
                pending_loop.source,
                (
                    "bind_pending_kura_apply(pending_kura_apply)",
                    "open_production_lifecycle_owner_v1(",
                    "owner.launch(launch_inputs)",
                    "install_pending_kura_apply(&mut setup_runner)",
                    "drive_apply_recovery_turn(&mut setup_runner, control_queue_capacity)",
                    "prepare_lane_recovery(",
                    "activate_no_clock(activation)",
                    "run_pending_active_height(",
                    "run_non_pending_lifecycle_loop(",
                ),
            )
            require_order(
                pending_runner_path,
                "pending-Kura successor handoff",
                pending_loop.source,
                (
                    "run_pending_active_height(",
                    "successor.verified_context",
                    "Some(successor.pending_activation)",
                    "false",
                    "true",
                ),
            )
        historical_ingress = ordinary_consumer_source
        require_tokens(
            ordinary_consumer_path,
            "historical ingress routing",
            historical_ingress,
            (
                "block_sync_server.serve_historical_body( kura, request, &sender, local_key )",
                "block_sync.authenticate_response(response, &sender)",
                "block_sync.enqueue_and_complete(discovered, |message| { executor.enqueue_discovered_commit_certificate(message, ingress_ownership) })",
            ),
        )
        require_token_count(
            ordinary_consumer_path,
            "historical ingress routing omits production refinement tokens when either reviewed route changes",
            historical_ingress,
            "block_sync_server.serve_historical_body(kura, request, &sender, local_key)",
            1,
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
                "pub(in crate::sumeragi) fn activate_recovered_complete_tip_v2_height(",
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
    _successor_production_recovery_source_fidelity_errors(
        repo_root, errors, load, region, require_tokens, require_literals,
        require_token_count, require_literal_count, require_order, reject_tokens,
        runner_path, runner_source, sumeragi_path, sumeragi_source,
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
        adapter_struct = region(
            adapter_path,
            adapter_source,
            "SumeragiV2Adapter status publication latch",
            "pub(crate) struct SumeragiV2Adapter {",
            "\nenum SafetyWalOpenTarget",
        )
        require_tokens(
            adapter_path,
            "SumeragiV2Adapter status publication latch",
            adapter_struct,
            ("status_publication_enabled: bool,",),
        )
        require_token_count(
            adapter_path,
            "adapter status publication latch closed surface",
            adapter_source,
            "status_publication_enabled",
            8,
        )
        require_token_count(
            adapter_path,
            "adapter status publication latch activation surface",
            adapter_source,
            "status_publication_enabled = true",
            3,
        )
        adapter_open = _require_qualified_rust_item(
            adapter_path,
            adapter_source,
            "SumeragiV2Adapter",
            "open_with_aggregator_and_publication_with_capacity",
            errors,
            "deferred status publication constructor",
            expected_attributes=("#[allow(clippy::too_many_arguments)]",),
        )
        _require_rust_token_sequence(
            adapter_path,
            adapter_open,
            """
publish_initial_status: bool,
capacity_geometry: ServicedCandidateCapacityGeometry,
deferred_admission_ordinals: DeferredAdmissionOrdinalSource,
""",
            "deferred status publication constructor must accept the exact latch initializer",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            adapter_open,
            """
replay_complete: false,
status_publication_enabled: publish_initial_status,
fail_closed: false,
""",
            "deferred status publication latch must initialize from publish_initial_status",
            errors,
        )
        _require_rust_token_sequence(
            adapter_path,
            adapter_open,
            """
adapter.replay_complete = true;
adapter.advance_reducer_fence_generation()?;
if publish_initial_status {
    adapter.publish_status()?;
}
""",
            "initial status publication must remain dominated by its constructor latch",
            errors,
        )
        ready_validate_context = (
            ("impl", "PreparedReadyDurableValidatePersistedSign", "<", "'", "_", ">"),
        )
        ready_validate_publications = tuple(
            item
            for item in rust_items(adapter_source, "install_registry_and_commit_adapter")
            if item.brace_context == ready_validate_context
        )
        if len(ready_validate_publications) != 1:
            errors.append(
                f"{adapter_path}: require exactly one Ready-Validate direct status "
                "publication; found "
                f"{len(ready_validate_publications)}"
            )
            ready_validate_publication = None
        else:
            ready_validate_publication = ready_validate_publications[0]
        _require_rust_item_context(
            adapter_path,
            ready_validate_publication,
            ready_validate_context,
            "Ready-Validate direct status publication",
            errors,
            expected_attributes=("#[inline(never)]",),
        )
        _require_rust_token_sequence(
            adapter_path,
            ready_validate_publication,
            """
self.armed = false;
if self.adapter.status_publication_enabled {
    super::status::set_v2_status(committed_status);
}
""",
            "Ready-Validate direct status publication must remain latch-dominated",
            errors,
        )
        if ready_validate_publication is not None:
            require_token_count(
                adapter_path,
                "Ready-Validate direct status publication",
                ready_validate_publication.source,
                "super::status::set_v2_status(committed_status)",
                1,
            )
        publish_status = _require_qualified_rust_item(
            adapter_path,
            adapter_source,
            "SumeragiV2Adapter",
            "publish_status",
            errors,
            "adapter status publication",
        )
        _require_rust_token_sequence(
            adapter_path,
            publish_status,
            """
let status = self.status()?;
if self.status_publication_enabled {
    super::status::set_v2_status(status);
}
Ok(())
""",
            "adapter status publication must compute before its latch-dominated global setter",
            errors,
        )
        if publish_status is not None:
            require_token_count(
                adapter_path,
                "adapter status publication",
                publish_status.source,
                "super::status::set_v2_status(status)",
                1,
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
                "let status = self.status()?",
                "self.status_publication_enabled = true",
                "Ok(status)",
            ),
        )
        successor_activation = _require_qualified_rust_item(
            adapter_path,
            adapter_source,
            "SumeragiV2Adapter",
            "successor_activation_status",
            errors,
            "successor activation status latch",
        )
        _require_rust_token_sequence(
            adapter_path,
            successor_activation,
            """
let status = self.status()?;
self.status_publication_enabled = true;
Ok(status)
""",
            "successor activation may enable status publication only after a successful snapshot",
            errors,
        )
        pending_kura_activation = _require_qualified_rust_item(
            adapter_path,
            adapter_source,
            "SumeragiV2Adapter",
            "pending_kura_activation_status",
            errors,
            "PendingKura activation status latch",
        )
        _require_rust_token_sequence(
            adapter_path,
            pending_kura_activation,
            """
let status = self.status()?;
self.status_publication_enabled = true;
Ok(status)
""",
            "PendingKura activation may enable status publication only after a successful snapshot",
            errors,
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
        pending_snapshot = region(
            runtime_path,
            runtime_source,
            "pending_kura_activation_status_snapshot",
            "pub(crate) fn pending_kura_activation_status_snapshot(",
            "\n    fn body_pipeline_completion_is_owned(",
        )
        require_order(
            runtime_path,
            "pending_kura_activation_status_snapshot",
            pending_snapshot,
            (
                "if self.clocks_armed",
                "AdapterError::PendingKuraActivationNotReady",
                "self.driver.pending_kura_activation_status()",
            ),
        )
    pending_lifecycle_path, pending_lifecycle_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_pending_kura.rs"
    )
    if pending_lifecycle_source:
        pending_activation = _require_qualified_rust_item(
            pending_lifecycle_path,
            pending_lifecycle_source,
            "PreparedPendingKuraLaneRecoveryV1",
            "activate_no_clock",
            errors,
            "PendingKura no-clock activation",
            expected_attributes=("#[allow(dead_code, clippy::result_large_err)]",),
        )
        _require_rust_token_sequence(
            pending_lifecycle_path,
            pending_activation,
            """
let status = launched
    .executor
    .pending_kura_activation_status_snapshot()
    .map_err(ProductionLifecycleActivationErrorV1::Status)?;
""",
            "PendingKura activation must snapshot and open its deferred adapter latch",
            errors,
        )
        if pending_activation is not None:
            require_order(
                pending_lifecycle_path,
                "PendingKura activation status-before-ingress boundary",
                pending_activation.source,
                (
                    "pending_kura_activation_status_snapshot()",
                    "activate_effect_completion_observer(observer)",
                    "runner.open_and_publish_recovered_height(",
                    "activation.complete()",
                ),
            )
    lifecycle_launch_path, lifecycle_launch_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs"
    )
    if lifecycle_launch_source:
        lifecycle_apply_settlement = _require_qualified_rust_item(
            lifecycle_launch_path,
            lifecycle_launch_source,
            "LaunchedProductionLifecycleV1",
            "settle_lifecycle_decision_apply_completion_owner",
            errors,
            "lifecycle Decision Apply settlement publication",
        )
        pending_applied_settlement = _require_rust_item(
            lifecycle_launch_path,
            lifecycle_launch_source,
            "settle_pending_kura_applied_decision_apply_completion",
            errors,
        )
        shared_applied_settlement = _require_rust_item(
            lifecycle_launch_path,
            lifecycle_launch_source,
            "settle_applied_lifecycle_decision_apply_completion_with_status",
            errors,
        )
        _require_rust_token_sequence(
            lifecycle_launch_path,
            lifecycle_apply_settlement,
            """
settle_applied_lifecycle_decision_apply_completion_with_status(
    owner,
    executor,
    completion,
    LifecycleDecisionApplyStatusPublicationV1::PublishActiveHeight,
)
""",
            "live lifecycle Decision Apply settlement must select active-height status publication",
            errors,
        )
        _require_rust_token_sequence(
            lifecycle_launch_path,
            pending_applied_settlement,
            """
settle_applied_lifecycle_decision_apply_completion_with_status(
    owner,
    executor,
    completion,
    LifecycleDecisionApplyStatusPublicationV1::DeferUntilPendingKuraActivation,
)
""",
            "pending-Kura lifecycle Decision Apply settlement must defer status publication until activation",
            errors,
        )
        _require_rust_token_sequence(
            lifecycle_launch_path,
            shared_applied_settlement,
            """
let status = executor.commit_lifecycle_decision_apply_finality(finality);
let settled = completion.acknowledge_after_owner_settlement();
assert!(
    matches!(settled, LifecycleDecisionApplyWorkerResultV1::Applied(_)),
    "borrowed lifecycle Decision Apply result cannot change before acknowledgement"
);
if matches!(
    status_publication,
    LifecycleDecisionApplyStatusPublicationV1::PublishActiveHeight
) {
    super::super::status::set_v2_status(status);
}
Ok(ProductionLifecycleDecisionApplyCompletionV1::Applied)
""",
            "shared lifecycle Decision Apply settlement must preserve its mode-gated final publication",
            errors,
        )
        if shared_applied_settlement is not None:
            require_token_count(
                lifecycle_launch_path,
                "shared lifecycle Decision Apply settlement publication",
                shared_applied_settlement.source,
                "super::super::status::set_v2_status(status)",
                1,
            )
            require_token_count(
                lifecycle_launch_path,
                "shared lifecycle Decision Apply settlement publication",
                shared_applied_settlement.source,
                "status_publication_enabled",
                0,
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
    lifecycle_selector_path, lifecycle_selector_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_selector.rs"
    )
    lifecycle_turn_driver_path, lifecycle_turn_driver_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs"
    )
    ingress_position_path, ingress_position_source = load(
        "crates/iroha_core/src/sumeragi/v2_lifecycle_ingress_position.rs"
    )
    fair_ingress_path, fair_ingress_source = load(
        "crates/iroha_core/src/sumeragi/mod.rs"
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
        certified_response_probe = region(
            effects_path,
            effects_source,
            "probe_certified_response_priority",
            "pub(in crate::sumeragi) fn probe_certified_response_priority(",
            "\n    /// Re-probe one opaque response candidate",
        )
        require_order(
            effects_path,
            "probe_certified_response_priority",
            certified_response_probe,
            (
                ".authenticate_response(",
                "ReadyBody::derive(",
                ".preflight_authenticated_response_claim(&authenticated)",
            ),
        )
        consume = region(
            effects_path,
            effects_source,
            "consume_one",
            "fn consume_one<",
            "\n    fn bind_body_pipeline_owner(",
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
        preledger_restart_regression = _require_rust_item(
            effects_path,
            effects_source,
            "ungated_certified_fetch_phase_b_restarts_before_ledger_without_mutation",
            errors,
        )
        if preledger_restart_regression is not None:
            require_order(
                effects_path,
                "ungated certified Fetch Phase-B pre-Ledger fail-stop regression",
                preledger_restart_regression.source,
                (
                    "certified_fetch_preledger_productive_ingress_token_for_test()",
                    "CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken",
                    "let work_id = completion.work_id()",
                    "let wait_before = owner.fetch_wait_projection_for_test(",
                    "let registry_before = owner.fetch_registry_snapshot_for_test()",
                    "let pending_before = fixture.executor.pending_fetches.clone()",
                    "let certified_before = fixture.executor.certified_work.clone()",
                    "let outstanding_before = fixture.executor.outstanding_requests.hashes()",
                    "let claims_before = fixture.executor.outstanding_requests.response_claim_count()",
                    "let next_work_id_before = fixture.executor.next_work_id",
                    "let ingress_depth_before = ingress.len()",
                    "let ingress_cut_before = ingress.next_physical_admission_ordinal()",
                    "let files_before = regular_file_bytes_below_for_test(owner_directory.path())",
                    "complete_certified_fetch_for_test(",
                    "RestartRequiredBeforeLedger(",
                    "assert_eq!(failure.work_id(), work_id)",
                    "assert_eq!( failure.failure(), CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken, )",
                    "fixture.executor.output_guard.restart_required()",
                    "owner.fetch_wait_projection_for_test(lifecycle_ordinal, lifecycle_source)",
                    "owner.fetch_registry_snapshot_for_test()",
                    "fixture.executor.pending_fetches, pending_before",
                    "fixture.executor.certified_work, certified_before",
                    "fixture.executor.outstanding_requests.hashes()",
                    "fixture.executor.next_work_id, next_work_id_before",
                    "ingress.len(), ingress_depth_before",
                    "regular_file_bytes_below_for_test(owner_directory.path())",
                    "ingress.exact_queued_ungated_occurrence_for_test(response_ordinal)",
                    "certified_fetch_completion_is_pending(work_id)",
                    "!production_services.has_reparked_certified_fetch_completion_for_test()",
                ),
            )
    if lifecycle_selector_source:
        recovered_fetch_next_selector = _require_rust_item(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "prepare_next_recovered_decision_fetch_ingress_selector",
            errors,
        )
        if recovered_fetch_next_selector is not None:
            require_order(
                lifecycle_selector_path,
                "queue-owned recovered Decision Fetch selector",
                recovered_fetch_next_selector.source,
                (
                    "self.lifecycle_terminal_subject()",
                    "capture_next_lifecycle_queue_cut(",
                    "v2_ingress_head_can_drain(occurrence.inbound(), self, terminal_subject)",
                    "self.capture_lifecycle_ingress_selector(cut)",
                    "prepared.queue_witness.selected_disposition()",
                    "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
                    ".selected_claimed_response_family()",
                ),
            )
        ownership_exact = _require_rust_item(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified_fetch_ingress_ownership_is_exact",
            errors,
        )
        if ownership_exact is not None:
            require_order(
                lifecycle_selector_path,
                "certified Fetch exact ingress ownership predicate",
                ownership_exact.source,
                (
                    "ownership.validate_exact()",
                    "ownership.matches_message(inbound.message())",
                    "ownership.matches_semantic_origin(inbound.sender())",
                    "ownership.matches_reply_routes(inbound.reply_routes())",
                ),
            )
        preledger_restart = region(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified Fetch pre-Ledger restart owner",
            "pub(crate) struct CertifiedFetchBodyPersistencePreLedgerRestartError {",
            "\n/// Closed structural rejection for a selected productive carrier before LedgerV1.",
        )
        require_order(
            lifecycle_selector_path,
            "certified Fetch pre-Ledger restart owner",
            preledger_restart,
            (
                "failure: CertifiedFetchPreLedgerProductiveIngressErrorV1",
                "completion: PreparedCertifiedFetchBodyPersistenceCompletion",
                "pub(crate) const fn failure(&self)",
                "self.failure",
                "match self.failure",
                "CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingOwnership",
                "CertifiedFetchPreLedgerProductiveIngressErrorV1::InvalidOwnership",
                "CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken",
                "CertifiedFetchPreLedgerProductiveIngressErrorV1::RuntimeAlreadyBound",
                "pub(crate) const fn work_id(&self)",
                "self.completion.work_id()",
            ),
        )
        preledger_error = region(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified Fetch pre-Ledger productive-ingress error partition",
            "pub(crate) enum CertifiedFetchPreLedgerProductiveIngressErrorV1 {",
            "\n/// Closed structural rejection for the sole post-dequeue Runtime handoff.",
        )
        require_tokens(
            lifecycle_selector_path,
            "certified Fetch pre-Ledger productive-ingress error partition",
            preledger_error,
            (
                "MissingOwnership",
                "InvalidOwnership",
                "MissingLeaderWireToken",
                "RuntimeAlreadyBound",
            ),
        )
        postdequeue_error = region(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified Fetch post-dequeue Runtime-handoff error partition",
            "pub(crate) enum CertifiedFetchPostDequeueRuntimeHandoffErrorV1 {",
            "\nimpl CertifiedFetchPostDequeueRuntimeHandoffErrorV1 {",
        )
        require_tokens(
            lifecycle_selector_path,
            "certified Fetch post-dequeue Runtime-handoff error partition",
            postdequeue_error,
            (
                "MissingOwnership",
                "InvalidOwnership",
                "MissingRuntimeReceipt",
                "MismatchedRuntimeReceipt",
            ),
        )
        preledger_validator = _require_rust_item(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified_fetch_preledger_productive_ingress_token",
            errors,
        )
        if preledger_validator is not None:
            require_order(
                lifecycle_selector_path,
                "certified Fetch pre-Ledger productive-ingress validation",
                preledger_validator.source,
                (
                    "inbound.ingress_ownership()",
                    "CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingOwnership",
                    "certified_fetch_ingress_ownership_is_exact(inbound, ownership)",
                    "CertifiedFetchPreLedgerProductiveIngressErrorV1::InvalidOwnership",
                    "ownership.leader_wire_token()",
                    "CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken",
                    "ownership.leader_wire_runtime_receipt().is_some()",
                    "CertifiedFetchPreLedgerProductiveIngressErrorV1::RuntimeAlreadyBound",
                    "Ok(token)",
                ),
            )
        postdequeue_validator = _require_rust_item(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified_fetch_postdequeue_runtime_receipt",
            errors,
        )
        if postdequeue_validator is not None:
            require_order(
                lifecycle_selector_path,
                "certified Fetch post-dequeue Runtime-receipt validation",
                postdequeue_validator.source,
                (
                    "inbound.ingress_ownership()",
                    "CertifiedFetchPostDequeueRuntimeHandoffErrorV1::MissingOwnership",
                    "certified_fetch_ingress_ownership_is_exact(inbound, ownership)",
                    "CertifiedFetchPostDequeueRuntimeHandoffErrorV1::InvalidOwnership",
                    "ownership.leader_wire_runtime_receipt()",
                    "CertifiedFetchPostDequeueRuntimeHandoffErrorV1::MissingRuntimeReceipt",
                    "receipt.token() != expected_token",
                    "receipt.owner().causal_lifecycle_key() != expected_token.identity_hash()",
                    "receipt.owner().admission_ordinal() != expected_token.scheduler_ordinal()",
                    "CertifiedFetchPostDequeueRuntimeHandoffErrorV1::MismatchedRuntimeReceipt",
                    "Ok(receipt.clone())",
                ),
            )
        preledger_test_wrapper = _require_qualified_rust_item(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "PreparedLifecycleIngressSelector",
            "certified_fetch_preledger_productive_ingress_token_for_test",
            errors,
            "certified Fetch production pre-Ledger test wrapper",
            expected_attributes=("#[cfg(test)]",),
        )
        _require_rust_token_sequence(
            lifecycle_selector_path,
            preledger_test_wrapper,
            "certified_fetch_preledger_productive_ingress_token(family.inbound.as_ref())",
            "certified Fetch production pre-Ledger test wrapper must delegate exactly",
            errors,
        )
        completion_error = region(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified Fetch Phase-B result split",
            "pub(crate) enum CertifiedFetchBodyPersistenceCompletionError {",
            "\n/// Typed reason an authenticated selected response could not wake its exact",
        )
        require_tokens(
            lifecycle_selector_path,
            "certified Fetch Phase-B result split",
            completion_error,
            (
                "Retry(CertifiedFetchBodyPersistenceRetryError)",
                "RestartRequiredBeforeLedger(CertifiedFetchBodyPersistencePreLedgerRestartError)",
                "RestartRequired(CertifiedFetchBodyPersistenceRestartError)",
                "RestartRequiredAfterDequeue(String)",
                "RestartRequiredAfterCommit(String)",
            ),
        )
        certified_fetch_persistence = region(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "complete_certified_fetch_body_persistence",
            "pub(crate) fn complete_certified_fetch_body_persistence(",
            "\n    /// Exercise the pure logical Ready reducer",
        )
        require_order(
            lifecycle_selector_path,
            "complete_certified_fetch_body_persistence",
            certified_fetch_persistence,
            (
                ".prepare_selected_certified_fetch_completion(",
                ".bind_durable_body_receipt(receipt)",
                ".prepare_lifecycle_certified_fetch_completion(candidate, &authenticated)",
                "let output_guard = services.lifecycle_output_guard()",
                "output_guard.close_admission_for_restart()",
                "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(",
                "failure: $failure",
                "certified_fetch_preledger_productive_ingress_token(family.inbound.as_ref())",
                "Err(error)",
                "durable_registry.abort_before_dequeue()",
                "restart_invalid_leader_wire!(error, receipt)",
                ".into_exact_certified_fetch_dequeue(executor, id, &authenticated)",
                "let Some(operation) = output_guard.begin_fail_stop_operation()",
                "persist_exact_staged_successor()",
                "exact_dequeue.commit(ingress)",
                "let runtime_receipt = certified_fetch_postdequeue_runtime_receipt(",
                "dequeued.inbound()",
                "&selected_leader_wire_token",
                "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(",
                "durable_registry.commit_after_exact_dequeue(dequeued)",
                "PreparedCertifiedFetchReadyTransition::Mutation(ready) => ready.commit()",
                "executor.commit_lifecycle_certified_fetch_completion(executor_prepared, &authenticated)",
                "service_prepared.commit(operation.permit())",
                "work_ack.commit()",
                "mark_leader_wire_durable_body_terminal(&runtime_receipt, &durable_body)",
                "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(error)",
                "operation.complete()",
            ),
        )
        dequeue_marker = "let dequeued = match exact_dequeue.commit(ingress)"
        dequeue_offset = certified_fetch_persistence.find(dequeue_marker)
        if dequeue_offset < 0:
            errors.append(
                f"{lifecycle_selector_path}: complete_certified_fetch_body_persistence "
                "lost its exact post-Ledger dequeue boundary"
            )
        else:
            pre_dequeue = certified_fetch_persistence[:dequeue_offset]
            post_dequeue = certified_fetch_persistence[dequeue_offset:]
            require_token_count(
                lifecycle_selector_path,
                "certified Fetch pre-dequeue productive-owner validation",
                pre_dequeue,
                "certified_fetch_preledger_productive_ingress_token(family.inbound.as_ref())",
                1,
            )
            require_token_count(
                lifecycle_selector_path,
                "certified Fetch pre-dequeue invalid-owner fail-stop",
                pre_dequeue,
                "restart_invalid_leader_wire!(error, receipt)",
                1,
            )
            reject_tokens(
                lifecycle_selector_path,
                "certified Fetch pre-dequeue queued-owner validation",
                pre_dequeue,
                (
                    "install_leader_wire_runtime_receipt",
                    "bind_leader_wire_runtime_ownership_locked",
                    "mark_leader_wire_runtime_locked",
                    "RestartRequiredAfterDequeue",
                    "RestartRequiredAfterCommit",
                ),
            )
            require_token_count(
                lifecycle_selector_path,
                "certified Fetch post-dequeue Runtime receipt extraction",
                post_dequeue,
                "certified_fetch_postdequeue_runtime_receipt",
                1,
            )
            require_token_count(
                lifecycle_selector_path,
                "certified Fetch post-dequeue restart boundary",
                post_dequeue,
                "RestartRequiredAfterDequeue",
                1,
            )
            require_token_count(
                lifecycle_selector_path,
                "certified Fetch durable-terminal restart boundary",
                post_dequeue,
                "RestartRequiredAfterCommit",
                1,
            )
        exact_dequeue_bridge = region(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified Fetch exact-dequeue bridge",
            "impl PreparedCertifiedFetchExactDequeue {",
            "\n/// Restart-only failure after LedgerV1 publication was invoked.",
        )
        require_order(
            lifecycle_selector_path,
            "certified Fetch exact-dequeue bridge",
            exact_dequeue_bridge,
            (
                "queue_witness.commit_exact_dequeue_retaining(",
                "ingress_identity.physical_admission_ordinal()",
                "Ok((inbound, disposition))",
                "CertifiedFetchDequeuedResponse",
            ),
        )
    if ingress_position_source:
        exact_dequeue_commit = region(
            ingress_position_path,
            ingress_position_source,
            "queue witness exact-dequeue commit",
            "pub(super) fn commit_exact_dequeue_retaining(",
            "\n    /// Atomically remove the exact selected occurrence after revalidating this",
        )
        require_order(
            ingress_position_path,
            "queue witness exact-dequeue commit",
            exact_dequeue_commit,
            (
                "self.revalidate_for_commit(queue)",
                "self.metadata_matches_locked(&state)",
                "queue.dequeue_selected_locked(",
                "Ok(dequeued)",
            ),
        )
    if fair_ingress_source:
        dequeue_selected = _require_rust_item(
            fair_ingress_path,
            fair_ingress_source,
            "dequeue_selected_locked",
            errors,
        )
        if dequeue_selected is not None:
            require_order(
                fair_ingress_path,
                "sole exact-dequeue leader-wire Runtime receipt mint",
                dequeue_selected.source,
                (
                    "let mut staged_ownership",
                    "staged_ownership.runtime_physical_cut.is_some()",
                    "staged_ownership.freeze_runtime_physical_cut(runtime_physical_cut)",
                    "let has_leader_wire_ownership",
                    "staged_ownership.leader_wire_runtime_receipt().is_some()",
                    "Self::bind_leader_wire_runtime_ownership_locked(state, &mut staged_ownership)",
                    "ingress_ownership = Some(staged_ownership)",
                    ".entries.remove(admitted_index)",
                    "Arc::try_unwrap(entry.inbound)",
                ),
            )
            require_token_count(
                fair_ingress_path,
                "sole exact-dequeue leader-wire Runtime receipt mint",
                dequeue_selected.source,
                "Self::bind_leader_wire_runtime_ownership_locked(state, &mut staged_ownership)",
                1,
            )
        bind_runtime = _require_rust_item(
            fair_ingress_path,
            fair_ingress_source,
            "bind_leader_wire_runtime_ownership_locked",
            errors,
        )
        if bind_runtime is not None:
            require_order(
                fair_ingress_path,
                "leader-wire Runtime receipt mint",
                bind_runtime.source,
                (
                    "ownership.validate_exact()",
                    "ownership.leader_wire_token().cloned()",
                    "ownership.leader_wire_runtime_receipt()",
                    "record.token != token",
                    "owner.causal_lifecycle_key() != token.identity_hash()",
                    "owner.admission_ordinal() != token.scheduler_ordinal()",
                    "Self::mark_leader_wire_runtime_locked(state, &token, owner)",
                    "ownership.install_leader_wire_runtime_receipt(receipt)",
                ),
            )
    if lifecycle_turn_driver_source:
        settle_certified_fetch = _require_rust_item(
            lifecycle_turn_driver_path,
            lifecycle_turn_driver_source,
            "settle_parked_certified_fetch_body_persistence",
            errors,
        )
        if settle_certified_fetch is not None:
            require_order(
                lifecycle_turn_driver_path,
                "certified Fetch Phase-B turn result split",
                settle_certified_fetch.source,
                (
                    "CertifiedFetchBodyPersistenceCompletionError::Retry(error)",
                    "error.into_completion()",
                    "ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRetry",
                    "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(",
                    "ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired",
                    "CertifiedFetchBodyPersistenceCompletionError::RestartRequired(error)",
                    "ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired",
                    "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(",
                    "ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired",
                    "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(",
                    "ProductionLifecycleCompletionSelectionV1::CertifiedFetchBodyPersistenceRestartRequired",
                ),
            )
            require_token_count(
                lifecycle_turn_driver_path,
                "certified Fetch Phase-B retry ownership",
                settle_certified_fetch.source,
                "error.into_completion()",
                1,
            )
            branch_markers = (
                (
                    "Retry",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::Retry(error))",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(",
                    ("error.into_completion()",),
                    ("close_admission_for_restart",),
                ),
                (
                    "RestartRequiredBeforeLedger",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequired(error))",
                    (
                        "error.work_id().get()",
                        "close_admission_for_restart()",
                        "drop(error)",
                    ),
                    ("error.into_completion()",),
                ),
                (
                    "RestartRequired",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequired(error))",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(",
                    ("close_admission_for_restart()", "drop(error)"),
                    ("error.into_completion()",),
                ),
                (
                    "RestartRequiredAfterDequeue",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(",
                    ("close_admission_for_restart()",),
                    ("error.into_completion()",),
                ),
                (
                    "RestartRequiredAfterCommit",
                    "Err(CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(",
                    "\n        }\n    }",
                    ("close_admission_for_restart()",),
                    ("error.into_completion()",),
                ),
            )
            for branch_name, start_marker, end_marker, required, forbidden in branch_markers:
                start = settle_certified_fetch.source.find(start_marker)
                end = settle_certified_fetch.source.find(
                    end_marker, start + len(start_marker)
                )
                if start < 0 or end < 0:
                    errors.append(
                        f"{lifecycle_turn_driver_path}:{settle_certified_fetch.line}: "
                        f"missing certified Fetch Phase-B {branch_name} branch"
                    )
                    continue
                branch = settle_certified_fetch.source[start:end]
                require_tokens(
                    lifecycle_turn_driver_path,
                    f"certified Fetch Phase-B {branch_name} branch",
                    branch,
                    required,
                )
                reject_tokens(
                    lifecycle_turn_driver_path,
                    f"certified Fetch Phase-B {branch_name} branch",
                    branch,
                    forbidden,
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
            "sumeragi::v2::tests::production_recovered_proposal_sign_joins_exact_next_vote_body_store",
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
    errors.extend(
        _lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(repo_root)
    )
    errors.extend(_lifecycle_decision_apply_lineage_source_fidelity_errors(repo_root))
    errors.extend(_successor_recovery_source_fidelity_errors(repo_root))
    return errors
