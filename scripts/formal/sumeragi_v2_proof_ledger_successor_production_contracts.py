# Executed lexically in check_sumeragi_v2_proof_ledger.py; do not import directly.

def _lifecycle_retained_apply_owner_source_fidelity_errors(
    worker_path, worker_source, launch_path, launch_source, errors, require_order,
) -> None:
    """Bind the original physical wait, guarded task, and exact retry queue cut."""

    def method(path, source, context, name, attributes=()):
        expected = (rust_code_tokens(context),)
        rows = [row for row in rust_items(source, name) if row.brace_context == expected]
        if len(rows) != 1:
            errors.append(f"{path}: retained Apply requires exactly one {context}::{name}; found {len(rows)}")
            return None
        row = rows[0]
        _require_rust_item_context(path, row, expected, "retained Apply owner", errors,
                                   expected_attributes=attributes)
        return row

    def sequence(path, row, expected, description):
        _require_rust_token_sequence(path, row, expected, description, errors)

    def structure(path, source, name, expected, attributes):
        rows = rust_struct_items(source, name)
        if len(rows) != 1:
            errors.append(f"{path}: retained Apply requires exactly one struct {name}; found {len(rows)}")
            return
        _require_rust_item_context(path, rows[0], (), "retained Apply private owner", errors,
                                   expected_attributes=attributes)
        sequence(path, rows[0], expected, "Apply must retain its original private guarded owner")

    commands = rust_enum_items(worker_source, "V2IoCommand")
    if len(commands) != 1:
        errors.append(f"{worker_path}: retained Apply requires one defining V2IoCommand enum")
    else:
        _require_rust_item_context(
            worker_path, commands[0], (), "Apply worker command", errors,
            expected_attributes=("#[allow(variant_size_differences, clippy::large_enum_variant)]",),
        )
        sequence(worker_path, commands[0], "LifecycleDecisionApply(LifecycleDecisionApplyTaskV1)",
                 "Apply command must carry the original typed task")
    structure(worker_path, worker_source, "PreparedLifecycleDecisionApplyCompletionV1", """
pub(in crate::sumeragi) struct PreparedLifecycleDecisionApplyCompletionV1 {
    guarded: Box<GuardedLifecycleDecisionApplyWorkerResultV1>,
    work_ack: LifecycleDecisionApplyWorkAckV1,
    dependency: Option<RetainedApplyDependency>,
}
""", ('#[must_use = "the lifecycle Decision Apply result still requires owner settlement"]',))
    structure(launch_path, launch_source, "RetainedLifecycleDecisionApplyDeferredV1", """
pub(in crate::sumeragi) struct RetainedLifecycleDecisionApplyDeferredV1 {
    completion: PreparedLifecycleDecisionApplyCompletionV1,
}
""", ('#[must_use = "deferred lifecycle Decision Apply remains the sole retry owner"]',))
    new = method(worker_path, worker_source, "impl PreparedLifecycleDecisionApplyCompletionV1", "new")
    sequence(worker_path, new, """
let dependency = match guarded.result() {
    LifecycleDecisionApplyWorkerResultV1::Deferred { refusal, .. } => {
        Some(RetainedApplyDependency::new(refusal))
    }
    LifecycleDecisionApplyWorkerResultV1::Applied(_) => None,
};
Self { guarded, work_ack, dependency }
""", "Apply completion must retain the dependency of its exact worker refusal")
    dependency = method(worker_path, worker_source, "impl RetainedApplyDependency", "new")
    for expected in (
        "let wake = busy.waker().clone(); Self::Release { pending: busy.wait.clone().wait_for_release(), wake, resource: busy.resource, }",
        'LocalValidationRefusal::QueueRelease { wait, wake } => Self::Release { pending: wait.clone().wait_for_release(), wake: wake.clone(), resource: "queue-release", }',
        "LocalValidationRefusal::RecoveryRequired(reason) => { Self::RecoveryRequired(reason.clone()) }",
        'LocalValidationRefusal::NativeSourceRecovery { .. } => Self::RecoveryRequired( "validated Apply lost its original Native source custody".into(), )',
    ):
        sequence(worker_path, dependency, expected, "Apply wait must preserve physical identity or fail closed")
    ready = method(worker_path, worker_source, "impl RetainedApplyDependency", "ready")
    sequence(worker_path, ready, """
match self {
    Self::Release { pending, wake, .. } => Ok(std::future::Future::poll(
        std::pin::Pin::new(pending),
        &mut std::task::Context::from_waker(wake),
    ).is_ready()),
    Self::RecoveryRequired(reason) => Err(reason.clone()),
}
""", "Apply retry must poll the same retained future and waker across turns")
    retry = method(worker_path, worker_source, "impl PreparedLifecycleDecisionApplyCompletionV1", "retry_deferred",
                   ("#[allow(clippy::result_large_err)]",))
    sequence(worker_path, retry, """
match self.dependency.as_mut().map(RetainedApplyDependency::ready) {
    Some(Ok(false)) => return LifecycleDecisionApplyDeferredRetryV1::Unavailable(self),
    Some(Ok(true)) => {},
    Some(Err(reason)) => {
        self.work_ack.output_guard.retain_effect_failure(reason);
        return LifecycleDecisionApplyDeferredRetryV1::RestartRequired;
    }
    None => return LifecycleDecisionApplyDeferredRetryV1::RestartRequired,
}
""", "Apply may retry only after its original dependency releases")
    require_order(worker_path, retry, (
        "let Self { guarded, work_ack, dependency } = self",
        "let (result, mut completion_guard) = (*guarded).into_retry_parts()",
        "let LifecycleDecisionApplyWorkerResultV1::Deferred { task, refusal } = result",
        "match work_ack.queue.retry_lifecycle_decision_apply(task)",
        "work_ack.acknowledge_retry_publication()", "completion_guard.disarm()",
    ), "Apply must enqueue the original task before acknowledgement and guard disarm")
    sequence(worker_path, retry, """
Err(LifecycleDecisionApplyRetryQueueErrorV1::Unavailable(task)) => {
    LifecycleDecisionApplyDeferredRetryV1::Unavailable(Self {
        guarded: Box::new(GuardedLifecycleDecisionApplyWorkerResultV1::from_retry_parts(
            LifecycleDecisionApplyWorkerResultV1::Deferred { task, refusal }, completion_guard,
        ),), work_ack, dependency,
    })
}
""", "Unavailable Apply capacity must preserve task, refusal, guard, acknowledgement, and dependency")
    sequence(worker_path, retry, """
Err(LifecycleDecisionApplyRetryQueueErrorV1::InvalidOwner(_task)) => {
    drop(work_ack);
    drop(completion_guard);
    LifecycleDecisionApplyDeferredRetryV1::RestartRequired
}
""", "Changed Apply queue ownership must fail closed")
    queue_retry = method(worker_path, worker_source, "impl V2IoCommandQueue", "retry_lifecycle_decision_apply")
    require_order(worker_path, queue_retry, (
        "let key = task.dispatch_key()", "let mut state = self.lock()",
        "if !state.sender_open || !state.receiver_open",
        ".lifecycle_decision_apply_completion_is_exact(key)",
        ".is_none_or(|tracked| tracked.state != V2IoWorkState::CompletionPending)",
        ".any(|command| command.lifecycle_decision_apply_key() == Some(key))",
        "return Err(LifecycleDecisionApplyRetryQueueErrorV1::InvalidOwner(task))",
        "if state.commands.len() >= self.capacity || !self.admission.try_reserve(V2IoAdmissionClass::Consensus)",
        "return Err(LifecycleDecisionApplyRetryQueueErrorV1::Unavailable(task))",
        ".transfer_lifecycle_decision_apply_completion(key)",
        ".get_mut(&key)", ".state = V2IoWorkState::Queued",
        "state.commands.push_back(task.into_command())", "drop(state)", "self.ready.notify_all()",
    ), "Apply retry must transfer one exact pending completion under the original queue lock")
    into_command = method(worker_path, worker_source,
                          "impl LifecycleDecisionApplyRetryTaskV1 for LifecycleDecisionApplyTaskV1", "into_command")
    sequence(worker_path, into_command, "V2IoCommand::LifecycleDecisionApply(self)",
             "Production Apply retry must move the original typed task")
    commit = method(worker_path, worker_source, "impl LifecycleDecisionApplyCapacityReservationV1<'_>", "commit")
    require_order(worker_path, commit, (
        "self.preflight(&prepared)", "let task = prepared.commit_for_worker()",
        "assert_eq!(task.dispatch_key(), self.key,", "let mut state = self.state.take()",
        "state.lifecycle_decision_applies.insert(self.key,",
        "state: V2IoWorkState::Queued", "replaced.is_none()",
        ".push_back(V2IoCommand::LifecycleDecisionApply(task))",
        "executor_dispatch.commit_after_worker_dispatch()", "drop(state)",
        "self.queue.ready.notify_all()", "operation.complete()",
    ), "Apply dispatch must publish its exact registry task once before releasing reservation")
    worker = method(worker_path, worker_source, "impl V2IoHandle", "spawn")
    sequence(worker_path, worker, """
V2IoCommand::LifecycleDecisionApply(task) => apply_service.execute_retained_lifecycle_apply(
    &context, body_store.as_mut().expect("body store remains live before Retire"),
    &mut retained_validation, task,
)
""", "Apply worker must consume the original Native service, body store, validation and task")
    deferred = method(launch_path, launch_source, "impl RetainedLifecycleDecisionApplyDeferredV1", "retry_after_local_release")
    sequence(launch_path, deferred, "let Self { completion } = self; match completion.retry_deferred()",
             "Deferred Apply must consume its guarded original completion")
    sequence(launch_path, deferred, """
LifecycleDecisionApplyDeferredRetryV1::Unavailable(completion) => {
    ProductionLifecycleDecisionApplyRetryV1::Unavailable(Self { completion })
}
""", "Deferred Apply must return the same completion on unavailable retry")


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
            "let mut live_apply_successor_outputs =",
            "prepare_ready_live_decision_apply_reconciliation(&self.coordinator, ordinal)",
            "executor.exactly_owns_live_lifecycle_decision_apply(&authority)",
            "executor.has_pending_lifecycle_output_admissions()",
            "try_classify_lifecycle_decision_apply_pending_output_census(",
            "executor.pending_lifecycle_output_admission_census()",
            "LifecycleDecisionApplyPendingOutputCensusV1::GenericSettlementPending(predecessor,)",
            "predecessor.apply_dispatch_key().lifecycle_ordinal() != protected_ordinal",
            "predecessor.runtime_ordinal() >= protected_ordinal",
            "ProductionCompletionCarrierStageV1::LiveApplyGenericPredecessorOrder",
            "LifecycleDecisionApplyPendingOutputCensusV1::Successor(attestation)",
            "live_apply_successor_outputs",
            ".insert(protected_ordinal, attestation)",
            "let fence = executor.lifecycle_reducer_fence_observation()",
            "attest_ready_lifecycle_decision_apply(&self.coordinator, *ordinal)",
            "live_apply_successor_outputs.get(ordinal)",
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
let successor_outputs = live_apply_successor_outputs.remove(&ordinal);
let executor_dispatch = executor
    .prepare_lifecycle_decision_apply_executor_dispatch(
        &prepared,
        successor_outputs,
    )
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
    validate_predecessor_ordinal,
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
    || validate_predecessor_ordinal == 0
    || validate_predecessor_ordinal >= key.lifecycle_ordinal()
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

    _lifecycle_retained_apply_owner_source_fidelity_errors(
        worker_path, worker_source, launch_path, launch_source, errors, require_order,
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
            "RetainedLifecycleDecisionApplyDeferredV1 { completion }",
            "settle_applied_lifecycle_decision_apply_completion(owner, executor, completion)",
        ),
        "lifecycle Apply result classification and direct live-height settlement",
    )
    shared_settlement = _require_rust_item(
        launch_path,
        launch_source,
        "settle_applied_lifecycle_decision_apply_completion",
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
            "let LifecycleDecisionApplyWorkerResultV1::Applied(applied) = settled",
            "let published = applied.into_published()",
            "super::super::status::set_v2_status(status)",
            "Ok(ProductionLifecycleDecisionApplyCompletionV1::Applied(published,))",
        ),
        "lifecycle Apply durable terminal settlement and direct status publication",
    )

    deferred_drive = _require_qualified_rust_item(
        launch_path, launch_source, "LaunchedProductionLifecycleV1",
        "drive_lifecycle_decision_apply_deferred", errors,
        "deferred lifecycle Apply retry owner",
    )
    _require_rust_token_sequence(
        launch_path, deferred_drive, "deferred.retry_after_local_release()",
        "deferred lifecycle Apply must retry its retained original dependency",
        errors,
    )
    for current in (settlement, deferred_drive):
        if current is not None:
            reject_aliases(
                launch_path, current.source,
                ("lane_work", "authorizes_sidecar_owner", "sidecar.register"),
                "production Apply ownership",
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
            "settle_pending_kura_applied_decision_apply_completion",
            "settle_applied_lifecycle_decision_apply_completion_with_status",
            "LifecycleDecisionApplyStatusPublicationV1",
            "DeferUntilPendingKuraActivation",
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


_RECOVERED_SUCCESSOR_STATUS_OWNER_RELATIONS = (
    # CompleteTip physical-frame retirement is available to exact signed genesis
    # as well as later rotating heights. Missing-frame genesis remains empty-only;
    # both policies retain the original Kura receipt, physical store and full census.
    ("frame_genesis_context", "crates/iroha_core/src/sumeragi/v2_recovery.rs", "RecoveredCompleteTipActivationAuthority", "authenticates_genesis_lifecycle_context", (), (
        ") -> bool { let verified = self.verified_predecessor.context(); self.artifact.height == 1 && self.artifact.height_context == *verified && verified.height == 1 && verified.parent_commit_qc.is_none() && verified.snapshot_bootstrap.is_none() && matches!(&self.predecessor_signature_policy, BlockSignaturePolicy::GenesisAuthority(_)) && context.height() == 1 && context.id().as_bytes() == self.artifact.context_id().0.as_ref() }",
    )),
    ("frame_policy_receipt", "crates/iroha_core/src/sumeragi/v2_recovery.rs", "RecoveredCompleteTipActivationAuthority", "authorizes_retired_lifecycle", (), (
        ") -> bool { let verified = self.verified_predecessor.context(); let policy_matches_height = match &self.predecessor_signature_policy { BlockSignaturePolicy::GenesisAuthority(_) => { self.authenticates_genesis_lifecycle_context(context) } BlockSignaturePolicy::RotatingLeader => self.artifact.height > 1, }; policy_matches_height && self.artifact.height_context == *verified && verified.height == self.artifact.height && DurableV2PredecessorIdentity::authenticate(&self.artifact, &self.receipt).is_ok_and(|predecessor| predecessor == self.activation.predecessor()) && context.height() == self.artifact.height && context.id().as_bytes() == self.artifact.context_id().0.as_ref() }",
    )),
    ("frame_mint", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "LifecycleLedgerStoreV1", "authenticate_present_frame", (), (
        "let (opened, present) = self.load_with_frame_presence()?; if opened != *expected { return Err(LifecycleLedgerError::InvalidLedger(",
        "if !present { return Ok(None); } let frame = encode_frame(&opened, self.max_frame_bytes)?;",
        "Ok(Some(AuthenticatedPresentLifecycleFrameV1 { store_path: self.path.clone(), #[cfg(all(unix, not(target_os = \"espidf\")))] store_directory_identity: self.directory.identity, context: self.context, max_records: self.max_records, max_frame_bytes: self.max_frame_bytes, ledger_frame_hash: LifecycleDigest::new(Hash::new(frame).into()), }))",
    )),
    ("frame_digest", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "AuthenticatedPresentLifecycleFrameV1", "binds_ledger", (), (
        ") -> bool { ledger.context() == self.context && encode_frame(ledger, self.max_frame_bytes).ok().is_some_and(|frame| { LifecycleDigest::new(Hash::new(frame).into()) == self.ledger_frame_hash }) }",
    )),
    ("frame_target", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "AuthenticatedPresentLifecycleFrameV1", "authorizes_canonical_retired_predecessor", (), (
        ") -> bool { self.store_path.parent().is_some_and(|root| { complete_tip.authorizes_predecessor_lifecycle_root(root) && self.store_path == root.join(LEDGER_FILE) && self.directory_identity_still_exact(root) }) && self.binds_ledger(ledger) && complete_tip.authorizes_retired_lifecycle(ledger.context()) }",
    )),
    ("frame_directory", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "AuthenticatedPresentLifecycleFrameV1", "directory_identity_still_exact", (), (
        "#[cfg(all(unix, not(target_os = \"espidf\")))] { bind_lifecycle_directory_path(root, false)",
        "directory.metadata().map_err(|error|",
        ".is_ok_and(|metadata| { LifecycleStorageIdentity::from_metadata(&metadata) == self.store_directory_identity })",
        "#[cfg(not(all(unix, not(target_os = \"espidf\"))))] { let _ = root; false }",
    )),
    ("frame_rejoin", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "AuthenticatedPresentLifecycleFrameV1", "exactly_matches", (), (
        ") -> bool { let mut exact_target = self.store_path == store.path; #[cfg(all(unix, not(target_os = \"espidf\")))] { exact_target &= self.store_directory_identity == store.directory.identity; } if !exact_target || self.context != store.context || self.max_records != store.max_records || self.max_frame_bytes != store.max_frame_bytes || !self.binds_ledger(ledger) { return false; } store.load_with_frame_presence().is_ok_and(|(opened, present)| present && opened == *ledger) }",
    )),
    ("frame_stage", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_operations.rs", "LifecycleLedgerV1", "stage_complete_tip_terminal_apply_recovery", (), (
        "if self.high_water() == 0 && self.records.is_empty() && self.producer_debts.is_empty() && complete_tip.authenticates_genesis_lifecycle_context(self.context()) { return Ok(( self.clone(), false, CompleteTipPredecessorLifecycleEvidenceV1::EmptyGenesis, )); }",
        "if let Ok(apply_ordinal) = self.authenticate_complete_tip_terminal_apply(complete_tip) { return Ok(( self.clone(), false, CompleteTipPredecessorLifecycleEvidenceV1::TerminalApply(apply_ordinal), )); }",
        "self.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?; if self.context().height() != complete_tip.predecessor().height() { return Err(",
        "(record.work_class() == Some(LifecycleWorkClass::Apply) && record.stage().is_some_and(|stage| { stage.kind() == LifecycleStageKind::ApplyDecision && stage.predecessor_scope() == PredecessorScope::Independent }) && record.terminal() == Some(None) && record.continuation() == Some(DurableContinuation::None) && complete_tip.authorizes_terminal_apply_replay(&record.replay_authority)).then_some(index)",
        "let Some(apply_index) = candidates.next() else { if let Some(present_frame) = present_frame && present_frame.authorizes_canonical_retired_predecessor(self, complete_tip) { return Ok(( self.clone(), false, CompleteTipPredecessorLifecycleEvidenceV1::CanonicalFrame(present_frame), )); } return Err(LifecycleLedgerError::InvalidLedger(",
        "if candidates.next().is_some() { return Err(",
        "let mut staged = self.clone(); staged.records[apply_index].terminal = Some(PersistedTerminalV1::from_schema(TerminalOutcome::Advanced)); staged.validate(MAX_LIFECYCLE_RECORDS_PER_HEIGHT)?; let apply_ordinal = staged.authenticate_complete_tip_terminal_apply(complete_tip)?;",
    )),
    ("frame_evidence", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "CompleteTipPredecessorLifecycleEvidenceV1", "exactly_matches", (), (
        ") -> bool { match self { Self::TerminalApply(apply_ordinal) => ledger.authenticate_complete_tip_terminal_apply(complete_tip).is_ok_and(|ordinal| ordinal == *apply_ordinal), Self::EmptyGenesis => { ledger.high_water() == 0 && ledger.records().is_empty() && ledger.producer_debts.is_empty() && complete_tip.authenticates_genesis_lifecycle_context(ledger.context()) } Self::CanonicalFrame(present) => { present.authorizes_canonical_retired_predecessor(ledger, complete_tip) && present.exactly_matches(store, ledger) } } }",
    )),
    ("frame_retirement_census", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "CompleteTipPredecessorLifecycleEvidenceV1", "authorizes_staged_retirement", (), (
        ") -> bool { if !self.exactly_matches(store, current, complete_tip) || current.context() != retired.context() || current.high_water() != retired.high_water() || current.records().len() != retired.records().len() || !retired.producer_debts.is_empty() || retired.records().iter().any(|record| record.terminal() == Some(None)) { return false; } match self { Self::TerminalApply(apply_ordinal) => retired.authenticate_complete_tip_terminal_apply(complete_tip).is_ok_and(|retired_ordinal| retired_ordinal == *apply_ordinal), Self::EmptyGenesis => current == retired, Self::CanonicalFrame(_) => true, } }",
    )),
    ("frame_retirement_publish", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "AuthenticatedCompleteTipPredecessorStorageV1", "retire", (), (
        "if !self.is_exact()? { return Err(",
        "payload_store.retire_authenticated_cut(serve_payloads, &retained_serve_payloads)?;",
        "let serve_reconciliation = super::open::reconcile_complete_tip_serve_retirement( &terminal.ledger, refreshed_serve_payloads, )?;",
        "let staged = terminal.ledger.stage_finalized_height_all_row_retirement(serve_reconciliation)?;",
        "let retained_predecessor_is_exact = terminal.predecessor_evidence.authorizes_staged_retirement( &terminal.ledger_store, &terminal.ledger, &retired, &terminal.complete_tip, ); if !retained_predecessor_is_exact { return Err(",
        "terminal.ledger_store.persist_exact_successor(&terminal.ledger, &retired)?; if terminal.ledger_store.load()? != retired { return Err(",
        "successor.open_initialized_or_descendant(retired.high_water())?;",
        "predecessor_frame_identity: retired.frame_identity(), successor_frame_identity: successor_ledger.frame_identity(), retained_high_water: retired.high_water(), predecessor_store: terminal.ledger_store, predecessor_ledger: retired, successor_store, successor_ledger,",
    )),

    ("ordinary_frontier", "crates/iroha_core/src/sumeragi/status.rs", "", "validate_v2_successor_snapshot", (), (
        "validate_v2_successor_snapshot_commit_frontier( finalized_height, finalized_height, expected_successor_context_id, successor, )",
    )),
    ("checked_frontier", "crates/iroha_core/src/sumeragi/status.rs", "", "validate_v2_successor_snapshot_commit_frontier", (), (
        "let expected_successor_height = finalized_height.checked_add(1).ok_or(",
        "if successor.height != expected_successor_height { return Err(V2SuccessorActivationError::SuccessorHeightMismatch",
        "if successor.last_committed_height != expected_commit_height { return Err(V2SuccessorActivationError::SuccessorParentMismatch",
        "if successor.height_context_id != expected_successor_context_id { return Err(V2SuccessorActivationError::SuccessorContextMismatch",
        "if !matches!( successor.liveness.last_progress, Some(marker) if marker.generation == successor.liveness.generation && marker.round.context_id == successor.height_context_id && marker.round.height == successor.height && marker.round.view == successor.view && marker.transition == SumeragiV2ProgressTransition::SuccessorHeightActivated && marker.age_ms == 0 ) { return Err(V2SuccessorActivationError::SuccessorMarkerMismatch); }",
    )),
    ("recovered_publish", "crates/iroha_core/src/sumeragi/status.rs", "", "publish_recovered_v2_successor_height_at", (), (
        "let finalized_height = if authority_kind == SUCCESSOR_AUTHORITY_SNAPSHOT_BOOTSTRAP { snapshot_height } else { predecessor.height };",
        "let expected_commit_height = if authority_kind == SUCCESSOR_AUTHORITY_RECOVERED_DECIDED_COMPLETE_TIP { finalized_height.checked_add(1).ok_or( V2SuccessorActivationError::SuccessorHeightOverflow(finalized_height), )? } else { finalized_height };",
        "validate_v2_successor_snapshot_commit_frontier( finalized_height, expected_commit_height, expected_successor_context_id, &successor, )?;",
        "let published = SUMERAGI_V2_STATUS",
        "let trace = ProductionRecoveredSuccessorTraceProjection",
        "let Some(checked_trace) = check_production_recovered_successor_transition(trace) else",
        "let _authorized_trace = checked_trace.into_projection();",
        "if let Some(published) = published { return Err(V2SuccessorActivationError::RecoveredStatusAlreadyPublished( published.height, )); }",
        "set_v2_status_at(successor, now);",
    )),
    ("complete_tip_bridge", "crates/iroha_core/src/sumeragi/status.rs", "", "activate_recovered_complete_tip_v2_height_with_decision_at", (), (
        "decision: Option<super::v2::RecoveredSuccessorDecisionActivationAuthorityV1>,",
        "if !authority.authorizes_successor_status_with_decision(&successor, decision.as_ref()) { return Err(V2SuccessorActivationError::RecoveredCompleteTipAuthorityMismatch); }",
        "let predecessor = authority.predecessor().refinement_projection();",
        "let publication = publish_recovered_v2_successor_height_at( if decision.is_some() { SUCCESSOR_AUTHORITY_RECOVERED_DECIDED_COMPLETE_TIP } else { SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP },",
        "drop(authority); publication",
    )),
    ("retired_status", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "RetiredRecoveredCompleteTipActivationAuthorityV1", "authorizes_successor_status_with_decision", (), (
        "self.authorizes_retained_successor() && self.successor_ledger.context().height() == successor.height && self.complete_tip.successor_context_id() == successor.height_context_id && self.complete_tip.predecessor().height().checked_add(1) == Some(successor.height)",
        "match decision { Some(decision) => self .complete_tip .authorizes_successor_decision_status(decision, successor), None => successor.last_committed_height == self.complete_tip.predecessor().height(), }",
    )),
    ("complete_tip_decision", "crates/iroha_core/src/sumeragi/v2_recovery.rs", "RecoveredCompleteTipActivationAuthority", "authorizes_successor_decision_status", (), (
        "decision.authorizes( &self.artifact.height_context, &self.artifact.commit_qc, self.activation.successor_context_id(), status, )",
    )),
    ("decision_identity", "crates/iroha_core/src/sumeragi/v2_complete_tip_activation.rs", "RecoveredSuccessorDecisionActivationAuthorityV1", "authorizes", (), (
        "self.wal_identity.is_exact() && self.parent_context == *parent && self.parent_commit_qc == *parent_commit_qc && parent.height.checked_add(1) == Some(self.height) && successor_context_id == self.context_id && status.height_context_id == self.context_id && status.height == self.height && status.last_committed_height == self.height && status.last_committed_subject == Some(self.decision.subject) && status.last_commit_qc.as_ref() == Some(&self.decision_status) && status.phase == wire::SumeragiV2StatusPhase::PendingApply && status.body_state == wire::SumeragiV2BodyState::PendingApply && status.pending_persistence_id.is_none() && !status.restart_required",
    )),
    ("decision_mint", "crates/iroha_core/src/sumeragi/v2_complete_tip_activation.rs", "SumeragiV2Adapter", "recovered_successor_decision_activation_authority", (), (
        "self.ensure_ingress()?;",
        "let Some(decision) = self.reducer.durable_state().decision() else { return Ok(None); };",
        "if self.reducer.durable_state().last_id().get() == 0 || self.pending_persistence_id.is_some() || self.reducer.applied_subject().is_some() { return Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch); }",
        "self.authenticate_recovered_wal_frontier()?;",
        "let parent = self .parent_verification .as_ref() .ok_or(AdapterError::RecoveredSuccessorDecisionActivationMismatch)?;",
        "let parent_commit_qc = self .wire_context .parent_commit_qc .as_ref() .ok_or(AdapterError::RecoveredSuccessorDecisionActivationMismatch)?;",
        "let decision = self .registry .qc_to_wire(decision, self.aggregator.as_ref())?;",
        "let decision_status = commit_qc_status(&decision, &self.wire_context)?;",
        "for frame in self.wal.recovered_records().iter().rev() { let (identity, envelope) = self.authenticate_recovered_wal_frame(frame)?; if matches!(envelope.record, WalRecordV2::Decision(candidate) if candidate == decision) { wal_identity = Some(identity); break; } }",
        "wal_identity.ok_or(AdapterError::RecoveredSuccessorDecisionActivationMismatch)?;",
        "if parent.context.height.checked_add(1) != Some(self.wire_context.height) || parent_commit_qc.round.context_id != parent.context.id() || parent_commit_qc.round.height != parent.context.height { return Err(AdapterError::RecoveredSuccessorDecisionActivationMismatch); }",
        "Ok(Some(RecoveredSuccessorDecisionActivationAuthorityV1 { wal_identity, parent_context: parent.context.clone(), parent_commit_qc: parent_commit_qc.clone(), context_id: self.wire_context.id(), height: self.wire_context.height, decision, decision_status, }))",
    )),
    ("runner_publish", "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs", "ProductionLifecycleCompleteTipRunnerActivationV1", "open_and_publish_with_decision", (), (
        "self.ingress_ready.store(false, Ordering::Release);",
        "if !Arc::ptr_eq(&self.block_ingress, launched_ingress) { self.block_ingress.close(); return Err(V2RunnerError::LifecycleActivationIngressMismatch); }",
        "if !retirement.authorizes_successor_status_with_decision(&successor, decision.as_ref()) { self.block_ingress.close(); return Err(V2RunnerError::CompleteTipSuccessorAuthorityInvalid",
        "self.block_ingress.open().map_err(ingress_capacity_error)?;",
        "if let Err(error) = super::super::status::activate_recovered_complete_tip_v2_height_with_decision( retirement, successor, decision, ) { self.block_ingress.close(); return Err(error.into()); }",
        "self.ingress_ready.store(true, Ordering::Release);",
    )),
    ("retained_floor", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "RetiredRecoveredCompleteTipActivationAuthorityV1", "frame_descends_from_retained_floor", (), (
        "ledger.context() == self.successor_store.context && if ledger.records.is_empty() { ledger.producer_debts.is_empty() && ledger.high_water == self.retained_high_water } else { ledger.high_water >= self.retained_high_water && ledger.records.iter().all(|record| { record.ordinal() > self.retained_high_water && record.owner().first_admission_ordinal() > self.retained_high_water }) }",
    )),
    ("retained_parent", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "RetiredRecoveredCompleteTipActivationAuthorityV1", "predecessor_remains_exact", (), (
        "self.predecessor_ledger.frame_identity() == self.predecessor_frame_identity && self .predecessor_store .is_authorized_complete_tip_predecessor_target(&self.complete_tip) && self.predecessor_store.load().ok().as_ref() == Some(&self.predecessor_ledger)",
    )),
    ("retained_frame", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "RetiredRecoveredCompleteTipActivationAuthorityV1", "authorizes_owner_open_successor", (), (
        "successor == &self.successor_ledger && self.successor_descends_from_retirement()",
    )),
    ("retained_successor", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "RetiredRecoveredCompleteTipActivationAuthorityV1", "authorizes_retained_successor", (), (
        "let Some(successor_root) = self.successor_store.path.parent() else { return false; };",
        "self.predecessor_remains_exact() && self.successor_descends_from_retirement() && self.complete_tip.authorizes_successor_lifecycle_target( successor_root, self.successor_ledger.context(), ) && self.successor_store.load().ok().as_ref() == Some(&self.successor_ledger)",
    )),
    ("publication_target", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "LifecycleLedgerStoreV1", "same_publication_target", (), (
        "self.path == other.path && self.directory.same_directory(&other.directory) && self.context == other.context && self.max_records == other.max_records && self.max_frame_bytes == other.max_frame_bytes",
    )),
    ("publication_open", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "LifecycleLedgerStoreV1", "open", (), (
        "owner_open_publications: std::sync::Arc::new(std::sync::Mutex::new(None)),",
        "let ledger = store.load()?; let frame = ledger.frame_identity(); store.owner_open_publications = std::sync::Arc::new(std::sync::Mutex::new(Some((frame, frame)))); Ok((store, ledger))",
    )),
    ("publication_cas", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "LifecycleLedgerStoreV1", "persist_exact_successor", (), (
        "let guard = self.directory.lock()?; let (loaded, frame_present) = self.load_with_frame_presence_locked(&guard)?; if loaded != *current { return Err(LifecycleLedgerError::InvalidLedger(",
        "let publication = self .owner_open_publications .lock() .ok() .filter(|lineage| lineage.is_some()) .map(|_| (current.frame_identity(), successor.frame_identity()));",
        "if current != successor || !frame_present { self.persist_locked(&guard, successor)?; }",
        "if let Some((current_identity, successor_identity)) = publication { self.record_owner_open_publication(current_identity, successor_identity); }",
    )),
    ("publication_record", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "LifecycleLedgerStoreV1", "record_owner_open_publication", (), (
        "let Ok(mut lineage) = self.owner_open_publications.lock() else { return; };",
        "if let Some((_, previous)) = lineage.as_mut() { if *previous == current { *previous = successor; } else { *lineage = None; } }",
    )),
    ("publication_take", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger_store.rs", "LifecycleLedgerStoreV1", "take_owner_open_successor", (), (
        "self.owner_open_publications.lock().ok()?.take()?;",
        "(predecessor_frame_identity != successor_frame_identity).then(|| { AuthenticatedRecoveredOwnerOpenSuccessorV1 { store: self.clone(), context: self.context, predecessor_frame_identity, successor_frame_identity, } })",
    )),
    ("publication_join", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "AuthenticatedRecoveredOwnerOpenSuccessorV1", "authorizes_complete_tip_owner_join", (), (
        "self.store.same_publication_target(retirement_store) && self.store.same_publication_target(owner_store) && self.context == retirement_store.context && self.context == owner_store.context && frozen.context() == self.context && loaded.context() == self.context && coordinator.context() == self.context && frozen.frame_identity() == self.predecessor_frame_identity && loaded.frame_identity() == self.successor_frame_identity && coordinator.frame_identity() == self.successor_frame_identity && retirement_store.load().ok().as_ref() == Some(loaded) && owner_store.load().ok().as_ref() == Some(loaded) && self.store.load().ok().as_ref() == Some(loaded)",
    )),
    ("publication_bind", "crates/iroha_core/src/sumeragi/v2_lifecycle_ledger.rs", "RetiredRecoveredCompleteTipActivationAuthorityV1", "bind_successor_owner",
     ('#[cfg_attr(not(test), allow(dead_code))]',), (
        "let Ok(successor_ledger) = self.successor_store.load() else { return Err(CompleteTipSuccessorOwnerBindErrorV1); };",
        "let retirement_frame_authorizes = self.authorizes_owner_open_successor(&successor_ledger);",
        "let owner_open_publication_authorizes = if retirement_frame_authorizes { false } else if !self.successor_descends_from_retirement() || !self.frame_descends_from_retained_floor(&successor_ledger) || !self.predecessor_remains_exact() { false } else",
        "LifecycleLedgerV1::from_coordinator(&owner.coordinator)",
        "owner .owner_open_successor .as_ref() .is_some_and(|successor| { successor.authorizes_complete_tip_owner_join( &self.successor_store, owner_store, &self.successor_ledger, &successor_ledger, &coordinator_ledger, ) })",
        "if (!retirement_frame_authorizes && !owner_open_publication_authorizes) || !self.matches_successor_owner_ledger(&mut owner, &successor_ledger) { return Err(CompleteTipSuccessorOwnerBindErrorV1); }",
        "owner .owner_open_successor .take()",
        "self.successor_frame_identity = successor_ledger.frame_identity(); self.successor_ledger = successor_ledger; if !self.exactly_matches_successor_owner(&mut owner) { return Err(CompleteTipSuccessorOwnerBindErrorV1); }",
        "Ok(BoundRecoveredCompleteTipSuccessorOwnerV1 { owner, retirement: self, })",
    )),
    ("activation", "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs", "LaunchedProductionLifecycleV1", "activate_with", (), (
        "if let Some(error) = lifecycle_activation_recovery_blocker( self.pending_kura_apply_replay.is_some(), self.executor .pending_kura_apply_recovery_evidence() .is_some(), self.recovered_local_proposal_attempt.is_some(), ) { self.services .lifecycle_output_guard() .close_admission_for_restart(); return Err(error); }",
        "let activation = output_guard .begin_fail_stop_operation()",
        "if !local_proposal.exactly_matches(self.executor.context().id(), current_directive) { return Err(ProductionLifecycleActivationErrorV1::LocalProposalPreparationMismatch); }",
        "let recovered_decision = if matches!( publication, ProductionLifecycleActivationPublicationV1::RecoveredCompleteTip { .. } ) { self.executor .recovered_successor_decision_activation_authority() .map_err(ProductionLifecycleActivationErrorV1::Status)? } else { None };",
        "self.executor .arm_live_clocks(clock_activation, now)",
        ".successor_activation_status_snapshot()",
        "self.completion_observer_activation.take()",
        ".activate_effect_completion_observer(observer)",
        "let runner_activation = publication.open_and_publish( &self.leader_wire_ingress_binding.ingress, status, recovered_decision, )?; activation.complete();",
        "Ok(ActivatedProductionLifecycleV1 { runner_activation, local_proposal, launched: self, })",
    )),
    ("publication_variant", "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs", "ProductionLifecycleActivationPublicationV1", "open_and_publish", (), (
        "self, ingress: &Arc<FairV2Ingress>, status: wire::SumeragiV2Status, decision: Option<super::super::v2::RecoveredSuccessorDecisionActivationAuthorityV1>,",
        "Self::Runner(runner) => { if decision.is_some() { return Err(ProductionLifecycleActivationErrorV1::Status( super::super::v2::AdapterError::RecoveredSuccessorDecisionActivationMismatch, )); } runner.open_and_publish(ingress, status) }",
        "Self::RecoveredCompleteTip { runner, retirement } => { runner.open_and_publish_with_decision(ingress, retirement, status, decision) }",
    )),
    ("ordinary_publication", "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_runner_authority.rs", "ProductionLifecycleRunnerActivationV1", "open_and_publish", (), (
        "self.ingress_ready.store(false, Ordering::Release); if !Arc::ptr_eq(&self.block_ingress, launched_ingress) { self.block_ingress.close(); return Err(V2RunnerError::LifecycleActivationIngressMismatch); } self.block_ingress.open().map_err(ingress_capacity_error)?;",
        "ProductionLifecycleRunnerStatusAuthorityV1::CurrentHeight => { super::super::status::set_v2_status(successor); Ok(()) }",
        "super::super::status::activate_v2_successor_height( expected_predecessor, authority, successor, )",
        "super::super::status::activate_snapshot_bootstrap_v2_height(authority, successor)",
        "if let Err(error) = publication { self.block_ingress.close(); return Err(error); } self.ingress_ready.store(true, Ordering::Release);",
    )),
    ("ready_validate_install", "crates/iroha_core/src/sumeragi/v2_ready_durable_validate_adapter_preview.rs", "PreparedReadyDurableValidatePersistedSign<'_>", "install_registry_and_commit_adapter", ("#[inline(never)]",), (
        "mut self: Box<Self>, reservation: Box<LiveValidateSignRegistryReservation<'_>>",
        "let work = self .registry_work .take()",
        "let reservation = *reservation; work.install_into(reservation); self.commit_after_standalone_admission();",
    )),
    ("ready_validate_commit", "crates/iroha_core/src/sumeragi/v2_ready_durable_validate_adapter_preview.rs", "PreparedReadyDurableValidatePersistedSign<'_>", "commit_after_standalone_admission", ("#[inline(never)]",), (
        "assert!(self.armed && self.persisted_sign.is_none() && self.registry_work.is_none());",
        "let next_reducer = self .next_reducer .take()",
        "let next_registry = self .next_registry .take()",
        "let committed_status = self .committed_status .take()",
        "self.adapter.reducer = next_reducer; self.adapter.registry = next_registry; self.adapter.pending_persistence_id = None; self.adapter.reducer_fence_generation = self.next_fence_generation;",
        "self.armed = false; if self.adapter.status_publication_enabled { super::status::set_v2_status(committed_status); }",
    )),
)


def _reviewed_recovery_owner_item(path, source, owner, name, attributes, errors):
    expected_context = (tuple(rust_code_tokens("impl " + owner)),) if owner else ()
    items = [item for item in rust_items(source, name) if item.brace_context == expected_context]
    if len(items) != 1:
        errors.append(f"{path}: recovered successor status requires exactly one {owner}::{name}; found {len(items)}")
        return None
    item = items[0]
    _require_rust_item_context(path, item, expected_context, "recovered successor status", errors,
                               expected_attributes=attributes)
    return item


def _reviewed_recovery_owner_relation_errors(repo_root, relations, label) -> list[str]:
    """Bind executable defining-owner sequences without masking authority or order."""
    errors: list[str] = []
    sources: dict[str, str] = {}
    for key, relative, owner, name, attributes, sequences in relations:
        path = repo_root / relative
        if relative not in sources:
            try:
                sources[relative] = path.read_text(encoding="utf-8")
            except OSError as error:
                errors.append(f"{path}: {label} source unreadable: {error}")
                continue
        item = _reviewed_recovery_owner_item(path, sources[relative], owner, name, attributes, errors)
        if item is None:
            continue
        tokens = rust_code_tokens(item.source)
        cursor = 0
        for sequence in sequences:
            expected = rust_code_tokens(sequence)
            found = next((position for position in range(cursor, len(tokens) - len(expected) + 1)
                          if tokens[position:position + len(expected)] == expected), None)
            if found is None:
                errors.append(f"{path}: {label} {key} must preserve exact owner/order {sequence!r}")
                break
            cursor = found + len(expected)
    return errors



def _recovered_successor_status_owner_errors(repo_root: Path) -> list[str]:
    return _reviewed_recovery_owner_relation_errors(
        repo_root, _RECOVERED_SUCCESSOR_STATUS_OWNER_RELATIONS, "recovered successor status",
    )



_TERMINAL_VALIDATE_OWNER_RELATIONS = (
    ("bound_only", "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs", "DurableValidateRetrySealV1", "lifecycle_ordinal", (), (
        "match self.lifecycle_state() { DurableValidateRetryLifecycleStateV1::Bound(ordinal) => Some(*ordinal), DurableValidateRetryLifecycleStateV1::PendingAdmission | DurableValidateRetryLifecycleStateV1::ResolvedNoSuccessor(_) => None, }",
    )),
    ("bind", "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs", "DurableValidateRetrySealV1", "bind_lifecycle_ordinal", (), (
        "if ordinal == 0 { return Err(",
        "DurableValidateRetryLifecycleStateV1::PendingAdmission => { *state = DurableValidateRetryLifecycleStateV1::Bound(ordinal); Ok(()) }",
        "DurableValidateRetryLifecycleStateV1::Bound(existing) if *existing == ordinal => Ok(())",
        "DurableValidateRetryLifecycleStateV1::Bound(_) => { Err(",
        "DurableValidateRetryLifecycleStateV1::ResolvedNoSuccessor(_) => Err(",
    )),
    ("release", "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs", "DurableValidateRetrySealV1", "release_lifecycle_ordinal", (), (
        "outcome: Arc<super::v2_lifecycle_coordinator::ResolvedLifecycleValidateOutcomeV1>,",
        "if self.lifecycle_state() != &DurableValidateRetryLifecycleStateV1::Bound(outcome.ordinal()) { return Err(",
        "Self::Live { lifecycle_state, .. } | Self::Recovered { lifecycle_state, .. } => { *lifecycle_state = DurableValidateRetryLifecycleStateV1::ResolvedNoSuccessor(outcome) }",
    )),
    ("protected_readmission", "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs", "DurableValidateRetrySealV1", "permits_resolved_readmission", (), (
        "if !matches!( self.lifecycle_state(), DurableValidateRetryLifecycleStateV1::ResolvedNoSuccessor(_) ) { return Ok(false); } current_protected_body_occurrence(effect, incoming, frontier)",
    )),
    ("incoming_authority", "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs", "", "current_protected_body_occurrence", (), (
        "if frontier.tag != Some(*tag) { return Ok(false); }",
        "let binding = incoming .exact_pending_adapter_effect_binding(effect)",
        "let statement = binding.candidate_statement().ok_or_else",
        "if statement.context_id() != round.context_id || statement.proposal_round() != *round || statement.subject() != Some(*subject) { return Err(",
        "None => false,",
        "Some(wire::GlobalPhase::Prepare) => { frontier.decision.is_none() && frontier.lock_is_authoritative && (frontier.locked_body == Some((*round, *subject)) || current_prepare_statement_matches_frontier(statement, *tag, frontier)) && statement.execution_commitment().is_some() }",
        "Some(wire::GlobalPhase::Commit) => frontier.decision.is_some_and(|decision| { statement.round() == decision.0 && statement.proposal_round() == decision.1 && statement.subject() == Some(decision.2) && statement.execution_commitment() == Some(decision.3) })",
    )),
    ("current_prepare", "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs", "", "current_prepare_statement_matches_frontier", (), (
        "frontier.tag == Some(tag) && frontier.decision.is_none() && frontier.highest_prepare.is_some_and(|certificate| { certificate.phase == wire::GlobalPhase::Prepare && certificate.round.view == tag.view() && statement.phase() == Some(wire::GlobalPhase::Prepare) && certificate.round == statement.round() && certificate.proposal_round == statement.proposal_round() && Some(certificate.subject) == statement.subject() && Some(certificate.execution_commitment) == statement.execution_commitment() })",
    )),
    ("cold_mint", "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs", "ResolvedLifecycleValidateOutcomeV1", "from_cold_claim", (), (
        "claim.matches_outcome(&outcome).then_some(Self { origin: ResolvedValidateOriginV1::Cold(claim), outcome, })",
    )),
    ("durable_mint", "crates/iroha_core/src/sumeragi/v2_lifecycle_body_pipeline_transition.rs", "<'coordinator, 'registry, 'adapter> PreparedSealedValidateNoSuccessorTransition<'coordinator, 'registry, 'adapter>", "persist_and_publish", ("#[allow(clippy::result_large_err)]",), (
        "let terminal = staged.records[&parent_ordinal].clone(); let metadata = staged.durable_records[&parent_ordinal].clone();",
        "let pending = preview .prepare_terminal_pending_fingerprint()",
        "if let Err(error) = coordinator.persist_exact_staged_successor(&staged)",
        "coordinator.fault = Some(super::CoordinatorFault::DurabilityFailure); return Err(SealedValidateNoSuccessorPublicationError { _coordinator: coordinator, _preview: preview, _staged: staged, _error: error, });",
        "*coordinator = staged; let outcome = preview.publish_no_successor_after_ledger_fsync(terminal, metadata, pending); assert!(outcome.matches_terminal(coordinator)); Ok(outcome)",
    )),
    ("original_outcome", "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_recovery.rs", "<'registry> PreparedReadyDurableValidateAdapterPreview<'registry, '_>", "publish_no_successor_after_ledger_fsync", (), (
        "assert_eq!(lease.ordinal(), address.ordinal); assert_eq!(lease.owner(), address.owner);",
        "registry .entries .remove(&address)",
        "assert!(work.validates_at(address));",
        "let ConcreteLifecycleWorkKind::DurableValidateCompletion(completion) = work.kind",
        "assert_eq!(terminal.ordinal, address.ordinal); assert_eq!(terminal.owner, address.owner); assert_eq!(terminal.key, lease.key()); assert_eq!(terminal.work_class, LifecycleWorkClass::Validate);",
        "assert_eq!( terminal.state, super::LifecycleState::Terminal(super::TerminalOutcome::Advanced) );",
        "assert_eq!( metadata.continuation, super::schema::DurableContinuation::AdvancedNoSuccessor );",
        "let outcome = ResolvedLifecycleValidateOutcomeV1 { origin: ResolvedValidateOriginV1::Live { terminal, metadata, effect: completion.incumbent.effect, pending, }, outcome: completion.outcome, }; adapter.commit_no_successor_after_durable_ledger(); outcome",
    )),
    ("original_occurrence", "crates/iroha_core/src/sumeragi/v2_effects.rs", "<R: EffectRuntime> V2EffectExecutor<R>", "retain_effect_batch_at_frontier", (), (
        "if let AdapterEffect::ValidateBody { round, subject, .. } = effect && self .cold_resolved_validate_outcomes .contains_key(&(*round, *subject))",
        "if retained_validate_retry_seals.contains_key(&key) || retained_published_validate_retry_markers.contains_key(&key) || self.pending_durable_validate_admissions.contains_key(&key)",
        "current_protected_body_occurrence(effect, evidence, frontier)",
        "let projected = seal .project_retry(effect, evidence)",
        "let readmit_resolved = seal .permits_resolved_readmission(effect, evidence, frontier)",
        "if readmit_resolved { if self.pending_durable_validate_admissions.contains_key(&key)",
        "retained_validate_retry_seals.insert(key, projected.seal); retain_effect.push(true); continue;",
    )),
    ("replay_before_admission", "crates/iroha_core/src/sumeragi/v2_effects_lifecycle_admission_settlement.rs", "<R: EffectRuntime> V2EffectExecutor<R>", "validate_body", (), (
        "let terminal = self.resolved_validate_outcome(key)?.cloned(); if let Some(terminal) = terminal",
        "self.durable_bodies.get(&key).cloned().ok_or_else",
        "self .exact_remote_proposal_validate_authority_certificate(&effect, &ownership)?",
        "self.recovered_bodies.get(&key).cloned().ok_or_else",
        "if recovered != receipt { return Err(",
        "if let Some(previous) = self.pending_resolved_validate_replay.as_ref() && previous.terminal().as_ref() != terminal.as_ref() { return Err(",
        "PendingResolvedValidateReplayV1::seal_exact_protected_body( effect, ownership, manifest, receipt, certificate, terminal, )",
        "let previous = self.pending_resolved_validate_replay.replace(pending); assert_eq!(previous.is_some(), replacing_resolved); return Ok(None);",
        "if let Some(marker) = self .published_lifecycle_validate_retry_markers .get_mut(&key)",
    )),
)


def _terminal_validate_owner_errors(repo_root: Path) -> list[str]:
    return _reviewed_recovery_owner_relation_errors(
        repo_root, _TERMINAL_VALIDATE_OWNER_RELATIONS, "terminal Validate owner",
    )


_CHUNK_SIGNING_OWNER_RELATIONS = (
    ("canonical_encoder", "crates/iroha_core/src/sumeragi/v2_chunks.rs", "", "encode_payload", (), (
        "context.validate()?;",
        "if round.context_id != context.id() || round.height != context.height || Hash::new(payload) != subject.payload_hash { return Err(V2ChunkError::PayloadMismatch); }",
        "if payload.is_empty() || payload_len > context.da_layout.max_payload_size_bytes { return Err(V2ChunkError::PayloadTooLarge); }",
        "let chunks = wire::encode_payload_chunks(context.da_layout, payload)?; let manifest = wire::PayloadManifest::derive(context, round, subject, payload_len, &chunks)?; Ok(EncodedV2Payload { manifest, chunks })",
    )),
    ("original_parts", "crates/iroha_core/src/sumeragi/v2_chunks.rs", "EncodedV2Payload", "into_parts", (), (
        "fn into_parts(self) -> (wire::PayloadManifest, Vec<Vec<u8>>) { (self.manifest, self.chunks) }",
    )),
    ("sign_encoded", "crates/iroha_core/src/sumeragi/v2_worker_services_impl.rs", "ProductionV2Services", "sign_payload_chunks", (), (
        "payload: EncodedV2Payload, sender: wire::ValidatorIndex,",
        "let (manifest, chunks) = payload.into_parts(); let validated = wire::ValidatedPayloadManifest::new(&self.context, manifest)",
        "if chunks.len() != validated.manifest().chunk_hashes.len() { return Err(",
        "let manifest_hash = validated.manifest_hash(); let signed = chunks .into_iter() .enumerate() .map(|(index, bytes)|",
        "let index = u32::try_from(index)",
        "let mut chunk = wire::PayloadChunk { manifest_hash, index, bytes, sender, signature: Vec::new(), };",
        "let preimage = validated .committed_chunk_signature_payload(index, sender)",
        ".signature_preimage(); chunk.signature = Signature::try_new(self.key_pair.private_key(), &preimage)",
        "Ok((validated, signed))",
    )),
    ("validate_manifest", "crates/iroha_data_model/src/block/consensus_v2.rs", "ValidatedPayloadManifest", "new", (), (
        "manifest.validate(context)?;",
        "let manifest_hash = HashOf::new(&manifest);",
        "let roster = context .roster .iter() .map(|entry| entry.validator.clone()) .collect::<Vec<_>>() .into();",
        "Ok(Self { manifest, manifest_hash, total_chunks, chunk_size_bytes, epoch: context.epoch, roster, })",
    )),
    ("committed_preimage", "crates/iroha_data_model/src/block/consensus_v2.rs", "ValidatedPayloadManifest", "committed_chunk_signature_payload", (), (
        "let manifest = self.manifest(); let chunk_hash = self.chunk_hash(index)?; self.validator(sender)?;",
        "Ok(PayloadChunkSignaturePayload { protocol_version: PROTOCOL_VERSION, context_id: manifest.round.context_id, epoch: self.epoch, height: manifest.round.height, view: manifest.round.view, subject: manifest.subject, manifest_hash: self.manifest_hash, encoding: manifest.layout.encoding, index, total_chunks: self.total_chunks, chunk_hash, sender, })",
    )),
)


def _recovered_chunk_signing_owner_errors(repo_root: Path) -> list[str]:
    errors = _reviewed_recovery_owner_relation_errors(
        repo_root, _CHUNK_SIGNING_OWNER_RELATIONS, "recovered chunk signing owner",
    )
    path = repo_root / "crates/iroha_core/src/sumeragi/v2_chunks.rs"
    source = path.read_text(encoding="utf-8")
    declaration = "pub(crate) struct EncodedV2Payload { manifest: wire::PayloadManifest, chunks: Vec<Vec<u8>>, }"
    if _token_sequence_count(rust_code_tokens(source), rust_code_tokens(declaration)) != 1:
        errors.append(f"{path}: encoded chunk authority must retain private immutable manifest and chunks")
    # Private fields make the canonical encoder the only production constructor.
    if _token_sequence_count(rust_code_tokens(source), rust_code_tokens("EncodedV2Payload { manifest, chunks }")) != 1:
        errors.append(f"{path}: encoded chunk authority requires its sole canonical constructor")
    return errors


_FIXTURE_DELEGATION_OWNER_RELATIONS = (
    ("factory_root", "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs", "", "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout", ("#[test]",), (
        "run_lifecycle_fixture_on_large_stack(",
        "production_lifecycle_owner_factory_binds_the_exact_kura_storage_layout_body, );",
    )),
    ("complete_tip_root", "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs", "", "production_genesis_complete_tip_adopts_control_repair_and_launches", ("#[cfg(feature = \"bls\")]", "#[test]", "#[allow(clippy::too_many_lines)]"), (
        "run_lifecycle_fixture_on_large_stack(",
        "production_genesis_complete_tip_adopts_control_repair_and_launches_body, );",
    )),
    ("marker_root", "crates/iroha_core/src/sumeragi/tests/v2_adapter_04b_lifecycle_startup.rs", "", "production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies", ("#[cfg(feature = \"bls\")]", "#[test]"), (
        "if std::thread::current().name() != Some(",
        ") { return run_marker_replay_test_on_stack(); }",
        "exercise_production_marker_replay_cases(&[ (0xB1_u8, true, false, false, None), (0xB2_u8, false, false, false, None), (0xB3_u8, true, true, false, None), (0xB4_u8, true, false, true, None), (0xB5_u8, true, false, false, Some(false)), (0xB6_u8, true, false, false, Some(true)), (0xB7_u8, true, false, false, Some(true)), ]);",
    )),
    ("marker_thread", "crates/iroha_core/src/sumeragi/tests/v2_adapter_main_00.rs", "", "run_marker_replay_test_on_stack", ("#[cfg(feature = \"bls\")]",), (
        ".spawn(production_lifecycle_factory_replays_markers_with_its_retained_apply_dependencies)",
        "if let Err(payload) = handle.join() { std::panic::resume_unwind(payload); }",
    )),
    ("fixture_thread", "crates/iroha_core/src/sumeragi/tests/v2_adapter_main_01.rs", "", "run_lifecycle_fixture_on_large_stack", (), (
        "name: &'static str, run: fn()",
        ".spawn(run)",
        "if let Err(payload) = handle.join() { std::panic::resume_unwind(payload); }",
    )),
    ("decision_root", "crates/iroha_core/src/sumeragi/tests/v2_adapter_04_wal_recovery_decision_classifier_cases.rs", "", "recovered_decision_fetch_classifier_authenticates_exact_absent_manifest_and_sources", ('#[cfg(feature = "bls")]', "#[test]"), (
        "run_lifecycle_fixture_on_large_stack(",
        "recovered_decision_fetch_classifier_authenticates_exact_absent_manifest_and_sources_body, );",
    )),
)


def _fixture_delegation_owner_errors(repo_root: Path) -> list[str]:
    return _reviewed_recovery_owner_relation_errors(
        repo_root, _FIXTURE_DELEGATION_OWNER_RELATIONS, "reviewed fixture delegation",
    )


_CONSUMER_ELIGIBILITY_OWNER_RELATIONS = (
    ("adapter_mint", "crates/iroha_core/src/sumeragi/v2.rs", "SumeragiV2Adapter", "leader_wire_recovery_authority", (), (
        "LeaderWireRecoveryAuthority::from_adapter(self)",
    )),
    ("wal_mint", "crates/iroha_core/src/sumeragi/v2_leader_wire_consumer.rs", "LeaderWireRecoveryAuthority", "from_adapter", (), (
        "adapter.ensure_ingress()?; let durable = adapter.reducer.durable_state(); let tag = adapter.reducer.current_tag();",
        "let protected_lock = durable .locked()",
        "adapter.registry.round_to_wire(certificate.proposal_round()), adapter.registry.subject(certificate.subject())?",
        "let protected_commit_statement = durable .locked() .filter(|locked| { locked.round().view() == tag.view() || durable.commit_intent_for_lock(locked).is_some() })",
        "vote_statement_hash( adapter.registry.round_to_wire(locked.proposal_round()), adapter.registry.subject(locked.subject())?, &adapter .registry .execution_commitment(locked.round(), locked.subject())?, )",
        "Ok(Self { context_id: adapter.frozen_wire_context_id(), height: adapter.wire_context.height, owner: adapter.fingerprints.node.into(), consumer_tag: tag, wal_id: durable.last_id(), decision_durable: durable.decision().is_some(), highest_prepare_view: durable.highest_prepare().map(|qc| qc.round().view()), installed_timeout_view: durable.last_timeout().map(|tc| tc.round().view()), protected_lock, protected_commit_statement,",
    )),
    ("authority_hash", "crates/iroha_core/src/sumeragi/v2_leader_wire_consumer.rs", "LeaderWireRecoveryAuthority", "projection_hash", (), (
        "bytes.extend(self.context_id.encode()); bytes.extend(self.height.to_le_bytes()); bytes.extend(self.owner);",
        "bytes.extend(self.consumer_tag.height().to_le_bytes()); bytes.extend(self.consumer_tag.view().to_le_bytes()); bytes.extend(self.consumer_tag.generation().get().to_le_bytes()); bytes.extend(self.wal_id.get().to_le_bytes());",
        "bytes.push(u8::from(self.decision_durable)); bytes.extend(self.highest_prepare_view.encode()); bytes.extend(self.installed_timeout_view.encode()); bytes.extend(self.protected_lock.encode()); bytes.extend(self.protected_commit_statement.encode());",
    )),
    ("accepts", "crates/iroha_core/src/sumeragi/v2_leader_wire_consumer.rs", "LeaderWireRecoveryAuthority", "consumer_accepts", (), (
        "if phase.source_class() != FairV2IngressLeaderWireSourceClass::Control { return true; } if self.decision_durable { return false; } let current_view = self.consumer_tag.view();",
        "Phase::Proposal | Phase::PrepareVote => view == current_view, Phase::CommitVote => exact_commit,",
        "Phase::PrepareQc => { view <= current_view && self .highest_prepare_view .is_none_or(|highest| view >= highest) }",
        "Phase::CommitQc => true, Phase::TimeoutVote => reducer::timeout_vote_view_is_admissible(current_view, view),",
        "view.checked_add(1).is_some() && (view >= current_view || reducer::strict_same_round_timeout_upgrade_is_allowed( reducer::StrictSameRoundTimeoutUpgradeProjection { current_view, timeout_view: view, installed_same_round: self.installed_timeout_view == Some(view), selected_prepare_present: timeout_prepare_view.is_some(), selected_prepare_view: timeout_prepare_view.unwrap_or(0), highest_prepare_present: self.highest_prepare_view.is_some(), highest_prepare_view: self.highest_prepare_view.unwrap_or(0), locked_prepare_present: self.protected_lock.is_some(), locked_prepare_view: self .protected_lock .map_or(0, |lock| lock.0.view), }, ))",
    )),
    ("retains", "crates/iroha_core/src/sumeragi/v2_leader_wire_consumer.rs", "LeaderWireRecoveryAuthority", "retains", (), (
        "if phase.source_class() != FairV2IngressLeaderWireSourceClass::Control { return true; } if self.decision_durable { return false; } let current_view = self.consumer_tag.view();",
        "Phase::Proposal | Phase::PrepareVote | Phase::TimeoutVote => view >= current_view, Phase::CommitVote => view >= current_view || exact_commit, Phase::PrepareQc => self .highest_prepare_view .is_none_or(|highest| view >= highest), Phase::CommitQc => true,",
        "Phase::TimeoutCertificate => { self.consumer_accepts(phase, view, exact_commit, timeout_prepare_view) }",
    )),
    ("waiting", "crates/iroha_core/src/sumeragi/v2_leader_wire_consumer.rs", "LeaderWireRecoveryAuthority", "consumer_waits_for", (), (
        "let exact_commit = position.commit_statement.is_some() && position.commit_statement == self.protected_commit_statement;",
        "position.context_id == self.context_id && position.height == self.height && self.retains( position.phase, position.view, exact_commit, position.timeout_prepare_view, ) && !self.consumer_accepts( position.phase, position.view, exact_commit, position.timeout_prepare_view, )",
    )),
    ("driver", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "RuntimeDriver for SumeragiV2Adapter", "leader_wire_consumer_authority", (), (
        "self.leader_wire_recovery_authority().map(Some)",
    )),
    ("refresh", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<D: RuntimeDriver> SerializedV2Runtime<D>", "refresh_ingress_consumer_eligibility", (), (
        "let authority = self .driver .leader_wire_consumer_authority() .map_err(|error| self.close(error))?;",
        "#[cfg(not(test))] if authority.is_none() { self.latch_fail_closed(",
        "return Err(RuntimeError::FailClosed); }",
        "if authority.is_some_and(|authority| authority.consumer_tag() != self.driver.current_tag())",
        "return Err(RuntimeError::FailClosed);",
        "self.ingress .refresh_consumer_authority(authority)",
    )),
    ("ingress_refresh", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<C: ExactRuntimeCommandIdentity> BoundedIngress<C>", "refresh_consumer_authority", (), (
        "let _ = self.oldest_lifecycle_ordinal()?; for queued in &self.commands { let owner = queued .cached_queue_occurrence_owner(&self.selection_source_identity) .ok_or(EnqueueError::FailClosed)?;",
        "if !queued.validate_cached_admission_identity() || !owner.validate_exact() || owner.class != queued.class.service_code() || owner.consumer_position != queued.command.leader_wire_consumer_position() { return Err(EnqueueError::FailClosed); } } self.consumer_authority = authority;",
    )),
    ("ingress_wait", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<C: ExactRuntimeCommandIdentity> BoundedIngress<C>", "consumer_waits", (), (
        "self.consumer_authority.is_some_and(|authority| { queued .cached_queue_occurrence_owner(&self.selection_source_identity) .and_then(|owner| owner.consumer_position) .is_some_and(|position| authority.consumer_waits_for(position)) })",
    )),
    ("ready", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<C: ExactRuntimeCommandIdentity> BoundedIngress<C>", "class_readiness", (), (
        "self.commands .iter() .any(|queued| queued.class == class && !self.consumer_waits(queued))",
    )),
    ("minimum", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<C: ExactRuntimeCommandIdentity> BoundedIngress<C>", "minimum_lifecycle_for_class", (), (
        "self.commands .iter() .filter(|queued| queued.class == class && !self.consumer_waits(queued)) .filter_map(|queued| queued.lifecycle_ordinal) .min()",
    )),
    ("snapshot_hash", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "", "runtime_queue_ownership_snapshot_projection_hash", (), (
        "&(Arc::as_ptr(&snapshot.source_identity) as usize).to_le_bytes()",
        "for owner in &snapshot.occurrence_owners",
        "owner.projection_hash.as_ref()",
        "match snapshot.consumer_authority { None => projection.push(0), Some(authority) => { projection.push(1); append_runtime_identity_field(&mut projection, authority.projection_hash().as_ref()); } } append_runtime_identity_u64(&mut projection, snapshot.consumer_pending_count);",
    )),
    ("snapshot_partition", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "RuntimeQueueOwnershipSnapshot", "validate_identity", (), (
        "self.occurrence_scan_complete && u64::try_from(self.occurrence_owners.len()) == Ok(self.projection.len)",
        "owner.validate_exact() && Arc::ptr_eq(&owner.source_identity, &self.source_identity)",
        "owner.class == class && !self.consumer_waits_at(*index)",
        "let pending_count = self .occurrence_owners .iter() .enumerate() .filter(|(index, _)| self.consumer_waits_at(*index)) .count();",
        "self.projection_hash == runtime_queue_ownership_snapshot_projection_hash(self)",
        "u64::try_from(pending_count) == Ok(self.consumer_pending_count)",
        "count.checked_add(self.consumer_pending_count) == Some(self.projection.len)",
    )),
    ("select", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<C: ExactRuntimeCommandIdentity> BoundedIngress<C>", "pop_next_with_selection_kind", (), (
        "(RuntimeQueueSelectionKind::Ordinary, None) => true,",
        "let queue_before = self.ownership_snapshot();",
        "let (completion_ready, progress_ready, normal_ready) = self.class_readiness(); let selection = select_bounded_service_class( cursor_before, completion_ready, progress_ready, normal_ready, );",
        "check_production_body_service_effective_lock_transition(service_trace)",
        ".minimum_lifecycle_for_class(class) .ok_or(EnqueueError::FailClosed)?;",
        "queued.class == class && !self.consumer_waits(queued) && queued.lifecycle_ordinal == Some(oldest_class_lifecycle_ordinal)",
        "if !selected.identity_deep_validated || !identity.validate_exact() || !ingress_exact || !selected.causal_origin.validate_exact() || selected.causal_origin.root_lifecycle_ordinal != Some(lifecycle_ordinal)",
        "self.mint_selection_seal( selection_kind, lifecycle_upper_bound, &queue_before,",
        "if !runtime_fifo_candidate_ingress_is_exact(&candidate)",
        "let _authorized_service = checked_service.into_projection(); self.next_class = next;",
        "queued.class == skipped_class && queued.lifecycle_ordinal == skipped_minimum && !self.consumer_waits(queued)",
        "oldest.eligible_skips = oldest .eligible_skips .checked_add(1)",
        "let command = self .commands .remove(index)",
        "Ok(Some((command, candidate)))",
    )),
    ("selected_seal", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "RuntimeQueueSelectionSeal", "matches_scheduler_occurrence", (), (
        "self.validate_identity() && self.scheduler_handoff_is_claimed() && Arc::ptr_eq(&self.source_identity, &before.source_identity) && Arc::ptr_eq(&self.source_identity, &after.source_identity)",
        "self.queue_before_snapshot_hash == before.projection_hash",
        "self.consumer_pending_count == before.consumer_pending_count && before.consumer_authority == after.consumer_authority && !before.consumer_waits_at(self.selected_position as usize)",
        "self.selected_identity == candidate.identity && self.selected_tag == candidate.tag && self.selected_causal_origin_hash == candidate.causal_origin.projection_hash",
        "if retry_retained { after.projection.len == before.projection.len && after.occurrence_owners == before.occurrence_owners && after.occurrence_lifecycle_ordinals == before.occurrence_lifecycle_ordinals } else { after.projection.len.checked_add(1) == Some(before.projection.len)",
    )),
    ("step", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<D: RuntimeDriver> SerializedV2Runtime<D>", "step", (), (
        "self.refresh_ingress_consumer_eligibility()?;",
        "self.reconcile_fence_retry_blocked_fifo_owners()",
        "self.freeze_due_clock_owners(now, external)",
        "let (work, next_schedule) = self.schedule.select( arbitration.timeout_due, arbitration.periodic_timer_due, arbitration.fifo_ready, ); self.schedule = next_schedule;",
        "ScheduledWork::Fifo => { return self.dispatch_selected_fifo( now, selected_round_tag, schedule_before, queue_before, arbitration, next_schedule, RuntimeQueueSelectionKind::Ordinary, RuntimeSelectedOwnerKind::Fifo, RuntimeSelectedOwnerKind::FifoRetryRetained, None, external, ); }",
    )),
    ("dispatch", "crates/iroha_core/src/sumeragi/v2_runtime.rs", "<D: RuntimeDriver> SerializedV2Runtime<D>", "dispatch_selected_fifo", ("#[allow(clippy::too_many_arguments)]",), (
        ".pop_next_with_selection_kind(queue_selection_kind, lifecycle_upper_bound)",
        "owner.lifecycle_ordinal() == candidate.lifecycle_ordinal && owner.causal_origin() == &candidate.causal_origin",
        "let retry_command = command.clone();",
        "match self.driver.dispatch(command)",
        "if retry_unadmitted { if self .ingress .restore_selected_command(retry_command, &candidate) .is_err()",
        "self.retain_scheduler_ownership( retry_selected_kind, selected_round_tag, RuntimeSelectedCandidateOwnership::Exact(candidate), queue_before, queue_after, arbitration, schedule_before, next_schedule, )?; return Ok(RuntimeStep::Advanced(Vec::new()));",
        "self.retain_scheduler_ownership( selected_kind, selected_round_tag, RuntimeSelectedCandidateOwnership::Exact(candidate), queue_before, queue_after, arbitration, schedule_before, next_schedule, )?;",
        "self.finish_dispatched_step( now, effects, RuntimeEffectSource::Fifo, owner, parent_statement, producer_handoff, retained_deferred_ingress, )",
    )),
    ("regression", "crates/iroha_core/src/sumeragi/tests/v2_runtime_unsealed_02_owner_retirement_and_fairness.rs", "", "ordinary_step_skips_only_blocked_prepare_qcs_to_install_matching_tc", ("#[test]",), (
        "Ok(RuntimeStep::Idle)",
        "assert_eq!(retained.selected, RuntimeSelectedOwnerKind::Idle); assert_eq!(retained.validate_exact(), Ok(()));",
        "Some(&certificate_receipt)",
        "wire::ConsensusMessageV2Payload::QuorumCertificate( intervening_certificate.clone(), )",
        "signed_runtime_proposal(&context, &keys, 0xC2)",
        "wire::ConsensusMessageV2Payload::TimeoutCertificate( timeout_certificate, )",
        "runtime.schedule.fifo_owed = true; runtime.ingress.next_class = CommandClass::Progress;",
        "assert_eq!(tc_scheduler.selected, RuntimeSelectedOwnerKind::Fifo); assert!(tc_scheduler.fifo_owed_before); assert!(!tc_scheduler.fifo_owed_after);",
        "assert_eq!(normal_debt_after, normal_debt_before + 1);",
        "tc_candidate.selection_seal.kind, RuntimeQueueSelectionKind::Ordinary",
        "forged_partition .queue_before_snapshot .consumer_pending_count -= 1;",
        "forged_partition.projection_hash = runtime_scheduler_projection_hash(&forged_partition);",
        "forged_partition.validate_exact().is_err()",
        ".leader_wire_recovery_authority()",
        ".advance_leader_wire_recovery_cut(entered_authority)",
        "assert_eq!(runtime.queued_commands(), 3);",
        "assert_eq!(normal_scheduler.selected, RuntimeSelectedOwnerKind::Fifo);",
        "assert!(runtime.take_leader_wire_runtime_terminals().is_empty()); assert_eq!(runtime.queued_commands(), 2);",
        "AdapterEffect::FetchBody",
        "assert_eq!(runtime.queued_commands(), 1);",
        "remaining == &intervening_certificate",
    )),
)


def _consumer_eligibility_owner_errors(repo_root: Path) -> list[str]:
    """Bind retained physical ingress to the WAL consumer and ordinary fair FIFO."""
    return _reviewed_recovery_owner_relation_errors(
        repo_root, _CONSUMER_ELIGIBILITY_OWNER_RELATIONS, "consumer eligibility owner",
    )


def _successor_production_source_fidelity_errors(repo_root: Path) -> list[str]:
    """Bind indexed successor and exact-recovery actions to production order."""
    errors: list[str] = _recovered_successor_status_owner_errors(repo_root)
    errors.extend(_terminal_validate_owner_errors(repo_root))
    errors.extend(_recovered_chunk_signing_owner_errors(repo_root))
    errors.extend(_fixture_delegation_owner_errors(repo_root))
    errors.extend(_consumer_eligibility_owner_errors(repo_root))
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
let discovery_was_outstanding = if terminal_finalization_fenced {
    false
} else if lane_only_completion_barrier {
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
                "authority.into_kura_bound_canonical_predecessor_storage(kura, local_signer)?",
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
                "into_kura_bound_canonical_predecessor_storage(kura, local_signer)",
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
                "PendingSuccessorActivation::recovered( authority, kura.as_ref(), &common_config.key_pair, )",
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
                "PendingSuccessorActivation::recovered( authority, kura.as_ref(), &common_config.key_pair, )",
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
                    "prepare_lane_recovery::<V2RunnerError>(&mut setup_runner)",
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
                    "reservation_reconciliation_pending",
                    "true",
                ),
            )
        historical_ingress = ordinary_consumer_source
        require_tokens(
            ordinary_consumer_path,
            "historical ingress routing",
            historical_ingress,
            (
                "HistoricalBodyServeTask::from_bound_ingress( request, sender, authenticated_via, reply_routes, ingress_ownership, )",
                "task.and_then(|task| block_sync_server.try_enqueue_historical_body(task))",
                "block_sync.authenticate_response(response, &sender)",
                "block_sync.enqueue_and_complete(discovered, |message| { executor.enqueue_discovered_commit_certificate(message, ingress_ownership) })",
            ),
        )
        require_token_count(
            ordinary_consumer_path,
            "historical ingress routing omits production refinement tokens when either reviewed route changes",
            historical_ingress,
            "block_sync_server.try_enqueue_historical_body(task)",
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
                "validate_v2_successor_snapshot_commit_frontier( finalized_height, finalized_height, expected_successor_context_id, successor, )",
                "finalized_height.checked_add(1)",
                "successor.last_committed_height != expected_commit_height",
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
                "validate_v2_successor_snapshot_commit_frontier(",
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
            "fn activate_recovered_complete_tip_v2_height_with_decision_at(",
            "\nfn activate_snapshot_bootstrap_v2_height_at(",
        )
        require_tokens(
            status_path,
            "activate_recovered_complete_tip_v2_height",
            complete_tip_activation,
            (
                "authority.authorizes_successor_status_with_decision(&successor, decision.as_ref())",
                "V2SuccessorActivationError::RecoveredCompleteTipAuthorityMismatch",
                "let predecessor = authority.predecessor().refinement_projection();",
                "let expected_successor_context_id = successor.height_context_id;",
                "SUCCESSOR_AUTHORITY_RECOVERED_COMPLETE_TIP",
                "CanonicalIdentityProjection::zero()",
                "drop(authority);",
                "pub(in crate::sumeragi) fn activate_recovered_complete_tip_v2_height_with_decision(",
                "activate_recovered_complete_tip_v2_height_with_decision_at( authority, successor, decision, Instant::now(), )",
            ),
        )
        require_order(
            status_path,
            "activate_recovered_complete_tip_v2_height",
            complete_tip_activation,
            (
                "authority.authorizes_successor_status_with_decision(&successor, decision.as_ref())",
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
#[cfg(test)]
status_publication_attempts: 0,
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
            for item in rust_items(adapter_source, "commit_after_standalone_admission")
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
        shared_applied_settlement = _require_rust_item(
            lifecycle_launch_path,
            lifecycle_launch_source,
            "settle_applied_lifecycle_decision_apply_completion",
            errors,
        )
        _require_rust_token_sequence(
            lifecycle_launch_path,
            lifecycle_apply_settlement,
            """
settle_applied_lifecycle_decision_apply_completion(owner, executor, completion)
""",
            "live lifecycle Decision Apply settlement must enter the sole direct settlement cut",
            errors,
        )
        _require_rust_token_sequence(
            lifecycle_launch_path,
            shared_applied_settlement,
            """
let status = executor.commit_lifecycle_decision_apply_finality(finality);
let settled = completion.acknowledge_after_owner_settlement();
let LifecycleDecisionApplyWorkerResultV1::Applied(applied) = settled else {
    unreachable!("borrowed lifecycle Apply result cannot change before acknowledgement")
};
let published = applied.into_published();
super::super::status::set_v2_status(status);
Ok(ProductionLifecycleDecisionApplyCompletionV1::Applied(published,))
""",
            "lifecycle Decision Apply settlement must publish only after durable finality and acknowledgement",
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
            require_token_count(
                lifecycle_launch_path,
                "shared lifecycle Decision Apply settlement publication",
                shared_applied_settlement.source,
                "LifecycleDecisionApplyStatusPublicationV1",
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
                "let responder = PeerId::new(responder_key.public_key().clone())",
                "kura\n        .get_block(block_height)",
                "block.hash() != request.subject.block_hash",
                "block.canonical_resultless_proposal()",
                "!proposal.is_resultless_proposal()",
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
                    "assert_eq!( failure.productive_ingress_failure(), Some(CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken), )",
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
        recovered_fetch_selected_family = _require_rust_item(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "prepare_recovered_decision_fetch_from_selected_cut",
            errors,
        )
        if recovered_fetch_selected_family is not None:
            require_order(
                lifecycle_selector_path,
                "queue-owned recovered Decision Fetch selected family",
                recovered_fetch_selected_family.source,
                (
                    "let selected_ordinal = cut.selected_identity().physical_admission_ordinal()",
                    "let selected_request_hash = cut.selector_occurrences()",
                    "occurrence.physical_admission_ordinal() == selected_ordinal",
                    "Some(response.request_hash)",
                    "self.capture_lifecycle_ingress_selector_for_response_family(",
                    "Some(selected_request_hash)",
                    "prepared.queue_witness.selected_disposition()",
                    "PreparedLifecycleIngressIoTarget::RecoveredDecisionFetchBodyPersistence",
                    ".selected_claimed_response_family()",
                    "family.candidate.recovered()",
                    "LifecycleIngressSelectorError::CandidateRevalidationDrift",
                    "Ok(prepared)",
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
                "failure: CertifiedFetchBodyPersistencePreLedgerFailure",
                "completion: PreparedCertifiedFetchBodyPersistenceCompletion",
                "pub(crate) const fn productive_ingress_failure(",
                "match &self.failure",
                "CertifiedFetchBodyPersistencePreLedgerFailure::ProductiveIngress(error) => Some(*error)",
                "_ => None",
                "pub(crate) const fn reason(&self)",
                "self.failure.reason()",
                "pub(crate) fn detail(&self)",
                "self.failure.detail()",
                "pub(crate) const fn work_id(&self)",
                "self.completion.work_id()",
            ),
        )
        retry_classifier = _require_qualified_rust_item(
            lifecycle_selector_path, lifecycle_selector_source,
            "CertifiedFetchBodyPersistencePreLedgerFailure", "permits_fresh_queue_retry",
            errors, "certified Fetch changing-queue retry classifier",
        )
        _require_rust_token_sequence(
            lifecycle_selector_path, retry_classifier,
            """fn permits_fresh_queue_retry(&self) -> bool { matches!(self,
                Self::FreshSelector(LifecycleIngressSelectorError::QueueCutChanged)
                | Self::FreshSelector(LifecycleIngressSelectorError::QueueCutCapture(
                    FairIngressQueueCutError::QueueCutChanged))
                | Self::Queue(FairIngressQueueCutError::QueueCutChanged)
            ) }""",
            "certified Fetch retry requires exactly a changed queue cut", errors,
        )
        reject_tokens(
            lifecycle_selector_path, "certified Fetch permanent rejection is not retry authority",
            preledger_restart, ("fn into_completion",),
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
            "certified_fetch_preledger_ingress_mode",
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
                    "ownership.leader_wire_runtime_receipt().is_some()",
                    "CertifiedFetchPreLedgerProductiveIngressErrorV1::RuntimeAlreadyBound",
                    "ownership.leader_wire_token().cloned()",
                    "!ownership.request_bound_non_roster_completion()",
                    "CertifiedFetchPreLedgerIngressModeV1::DurableLeaderWire(token)",
                    "ownership.request_bound_non_roster_completion()",
                    "CertifiedFetchPreLedgerIngressModeV1::RequestBoundArchive",
                    "CertifiedFetchPreLedgerProductiveIngressErrorV1::MissingLeaderWireToken",
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
        postdequeue_handoff = _require_rust_item(
            lifecycle_selector_path,
            lifecycle_selector_source,
            "certified_fetch_postdequeue_ingress_handoff",
            errors,
        )
        if postdequeue_handoff is not None:
            require_order(
                lifecycle_selector_path,
                "certified Fetch post-dequeue ingress-mode handoff",
                postdequeue_handoff.source,
                (
                    "match expected",
                    "CertifiedFetchPreLedgerIngressModeV1::DurableLeaderWire(token)",
                    "certified_fetch_postdequeue_runtime_receipt(inbound, token).map(Some)",
                    "CertifiedFetchPreLedgerIngressModeV1::RequestBoundArchive",
                    "inbound.ingress_ownership()",
                    "certified_fetch_ingress_ownership_is_exact(inbound, ownership)",
                    "!ownership.request_bound_non_roster_completion()",
                    "ownership.leader_wire_token().is_some()",
                    "ownership.leader_wire_runtime_receipt().is_some()",
                    "Ok(None)",
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
                "let output_guard = services.lifecycle_output_guard()",
                "macro_rules! reject_before_ledger",
                "let failure = $failure",
                "PreparedCertifiedFetchBodyPersistenceCompletion::from_parts(",
                "if failure.permits_fresh_queue_retry()",
                "CertifiedFetchBodyPersistenceCompletionError::Retry(",
                "output_guard.close_admission_for_restart()",
                "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredBeforeLedger(",
                ".prepare_selected_certified_fetch_completion(",
                ".bind_durable_body_receipt(receipt)",
                "executor.prepare_lifecycle_certified_fetch_completion( candidate, &authenticated, durable_registry.durable_body_receipt(), )",
                "let decision_exclusion = executor_prepared.decision_exclusion().copied()",
                "if let Some(exclusion) = decision_exclusion.as_ref()",
                "staged.cancel_excluded_decision(exclusion, durable_registry.durable_body_receipt())",
                "certified_fetch_preledger_ingress_mode(family.inbound.as_ref())",
                "Err(error)",
                "durable_registry.abort_before_dequeue()",
                "reject_before_ledger!(CertifiedFetchBodyPersistencePreLedgerFailure::ProductiveIngress(error), receipt)",
                ".into_exact_certified_fetch_dequeue(executor, id, &authenticated)",
                "exact_dequeue.lock(ingress)",
                "let Some(operation) = output_guard.begin_fail_stop_operation()",
                "persist_exact_staged_successor()",
                "exact_dequeue.commit()",
                "let runtime_receipt = certified_fetch_postdequeue_ingress_handoff(",
                "dequeued.inbound()",
                "&selected_ingress_mode",
                "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterDequeue(",
                "if let Some(exclusion) = decision_exclusion.as_ref()",
                "durable_registry.commit_cancelled_after_exact_dequeue(dequeued, exclusion)",
                "durable_registry.commit_after_exact_dequeue(dequeued)",
                "PreparedCertifiedFetchReadyTransition::Mutation(ready) => ready.commit()",
                "executor.commit_lifecycle_certified_fetch_completion(executor_prepared, &authenticated)",
                "service_prepared.commit(operation.permit())",
                "work_ack.commit()",
                "if let Some(runtime_receipt) = runtime_receipt",
                "mark_leader_wire_durable_body_terminal(&runtime_receipt, &durable_body)",
                "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(",
                "if decision_exclusion.is_none()",
                "services.retry_locked_candidate_after_durable_body(subject)",
                "CertifiedFetchBodyPersistenceCompletionError::RestartRequiredAfterCommit(format!(",
                "operation.complete()",
            ),
        )
        cancellation = _require_rust_item(
            lifecycle_selector_path, lifecycle_selector_source, "cancel_excluded_decision", errors,
        )
        if cancellation is not None:
            require_order(
                lifecycle_selector_path, "certified Fetch native Decision cancellation",
                cancellation.source,
                ("!exclusion.matches_durable_body(receipt)",
                 "self.location.ordinal()", "self.next.records.get(&ordinal)",
                 "record.work_class != LifecycleWorkClass::Fetch",
                 "record.state != LifecycleState::Ready",
                 "self.next.finish_terminal(ordinal, super::TerminalOutcome::Cancelled)"),
            )
        require_order(
            lifecycle_selector_path, "certified Fetch Ready and cancellation authority split",
            certified_fetch_persistence,
            ("let durable_registry = if decision_exclusion.is_none()",
             "check_production_historical_body_pipeline_transition(historical_trace)",
             "retain_historical_body_pipeline_owner(checked_transition, durable_registry)",
             "checked_transition.into_projection()", "let exact_dequeue = match exact_dequeue.lock(ingress)"),
        )
        registry_execution_path, registry_execution_source = load(
            "crates/iroha_core/src/sumeragi/v2_lifecycle_work_registry_validate_execution.rs"
        )
        cancelled_commit = _require_rust_item(
            registry_execution_path, registry_execution_source,
            "commit_cancelled_after_exact_dequeue", errors,
        )
        if cancelled_commit is not None:
            require_order(
                registry_execution_path, "certified Fetch cancelled registry exact receipt",
                cancelled_commit.source,
                ("assert!(exclusion.matches_durable_body(self.durable_receipt.durable_body()))",
                 "self.commit_response_dequeue(dequeued, true)"),
            )
        registry_commit = _require_rust_item(
            registry_execution_path, registry_execution_source, "commit_response_dequeue", errors,
        )
        if registry_commit is not None:
            require_order(
                registry_execution_path, "certified Fetch cancellation preserves exact dequeue checks",
                registry_commit.source,
                ("assert_eq!(dequeued.ingress_identity(), self.ingress_identity)",
                 "assert!(incumbent.validates_at(address))", "assert!(exact_selected_response_matches(",
                 ".remove(&address)", "if cancelled { return; }",
                 "let completion = CertifiedFetchCompletion", "self.registry.entries.insert(address, row)"),
            )
        dequeue_marker = "let dequeued = exact_dequeue.commit()"
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
                "certified_fetch_preledger_ingress_mode(family.inbound.as_ref())",
                1,
            )
            require_token_count(
                lifecycle_selector_path,
                "certified Fetch pre-dequeue invalid-owner fail-stop",
                pre_dequeue,
                "reject_before_ledger!(CertifiedFetchBodyPersistencePreLedgerFailure::ProductiveIngress(error), receipt)",
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
                "certified_fetch_postdequeue_ingress_handoff",
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
                "certified Fetch durable-terminal and locked-body wake restart boundaries",
                post_dequeue,
                "RestartRequiredAfterCommit",
                2,
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
                "queue_witness.lock_exact_dequeue_retaining(",
                "ingress_identity.physical_admission_ordinal()",
                "Ok(locked) => Ok(LockedPreparedCertifiedFetchExactDequeue",
                "queue_witness: locked.unlock_retaining()",
                "let (inbound, disposition) = locked.commit()",
                "assert_eq!(disposition, FairV2IngressDequeueDisposition::Admit)",
                "CertifiedFetchDequeuedResponse",
            ),
        )
    if ingress_position_source:
        exact_dequeue_lock = region(
            ingress_position_path,
            ingress_position_source,
            "queue witness exact-dequeue prelock",
            "pub(super) fn lock_exact_dequeue_retaining<'a>(",
            "\n    /// Atomically remove the exact selected occurrence while retaining this",
        )
        require_order(
            ingress_position_path,
            "queue witness exact-dequeue prelock",
            exact_dequeue_lock,
            (
                "Arc::ptr_eq(&self.queue_identity, &queue.queue_identity)",
                "self.selected_identity.context != expected_context",
                "self.selected_identity.physical_admission_ordinal != expected_physical_ordinal",
                "self.is_internally_exact()",
                "queue.service_lock.lock()",
                "queue.producer_publication_lock.lock()",
                "self.revalidate_for_commit(queue)",
                "selection.disposition == self.selected_disposition",
                "queue.state.lock()",
                "self.metadata_matches_locked(&state)",
                "Ok(LockedPreparedFairIngressExactDequeue",
            ),
        )
        exact_dequeue_commit = region(
            ingress_position_path,
            ingress_position_source,
            "locked queue witness exact-dequeue commit",
            "impl LockedPreparedFairIngressExactDequeue<'_> {",
            "\nimpl FairIngressQueueCut<'_> {",
        )
        require_order(
            ingress_position_path,
            "locked queue witness exact-dequeue commit",
            exact_dequeue_commit,
            (
                "pub(super) fn commit(self)",
                "let mut state = queue.state.lock()",
                "witness.metadata_matches_locked(&state)",
                "queue.dequeue_selected_locked(",
                ".expect(\"prevalidated lifecycle dequeue is infallible after publication\")",
                "drop(_producer_publication_guard)",
                "drop(_service_guard)",
                "dequeued",
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
