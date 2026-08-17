"""Direct selected-Serve predecessor source-fidelity contracts."""


def _direct_serve_predecessor_production_source_fidelity_errors(
    repo_root: Path = ROOT_DIR,
) -> list[str]:
    """Seal direct predecessor observation and its one-turn worker aperture.

    Runtime reports two booleans from the current lifecycle census; a move-only
    worker guard owns the only transient admission state and closes it on
    success, error, or unwind. Obsolete episode/witness spellings remain only
    in the negative inventory below, where any production occurrence fails.
    """

    base = repo_root / "crates" / "iroha_core" / "src" / "sumeragi"
    paths = {
        "runtime": base / "v2_runtime.rs",
        "effects": base / "v2_effects.rs",
        "worker": base / "v2_worker.rs",
        "runner": base / "v2_runner.rs",
        "ordinary": base / "v2_runner" / "lifecycle_run_inner.rs",
        "pending": base / "v2_runner" / "lifecycle_pending_kura.rs",
        "worker_cases": base / "tests" / "v2_worker_main_02.rs",
        "producer_cases": base / "tests" / "v2_worker_main_03.rs",
        "runner_cases": base / "tests" / "v2_runner_unsealed_00.rs",
    }
    errors: list[str] = []
    for role, path in paths.items():
        if not path.is_file() or path.is_symlink():
            errors.append(
                f"{path}: direct selected-Serve {role} source must be a regular file"
            )
    if errors:
        return errors

    sources = {
        role: path.read_text(encoding="utf-8") for role, path in paths.items()
    }
    for role in ("runtime", "effects", "worker", "runner"):
        _reviewed_path, reviewed_source = _read_reviewed_rust_source(
            repo_root,
            paths[role].relative_to(repo_root).as_posix(),
            errors,
            f"direct selected-Serve {role} source",
        )
        if reviewed_source:
            sources[role] = reviewed_source

    production_source = "\n".join(
        sources[role]
        for role in ("runtime", "effects", "worker", "runner", "ordinary", "pending")
    )
    production_tokens = rust_code_tokens(production_source)
    for obsolete in (
        "ExactServePredecessorEpisodeWitness",
        "CertifiedServeRuntimeEpisodeState",
        "exact_serve_predecessor_episode_witness",
        "last_predecessor_episode_witness",
        "claim_certified_serve_runtime_episode",
        "observe_certified_serve_predecessor_episode_witness",
        "finish_certified_serve_runtime_episode_turn",
    ):
        observed = production_tokens.count(obsolete)
        if observed:
            errors.append(
                f"{paths['runtime']}: obsolete selected-Serve witness/episode token "
                f"{obsolete!r} must be absent from production Rust; found {observed}"
            )

    def require_struct(
        role: str,
        name: str,
        description: str,
        *,
        expected_attributes: tuple[str, ...],
    ) -> RustItem | None:
        path = paths[role]
        items = rust_struct_items(sources[role], name)
        if len(items) != 1:
            errors.append(
                f"{path}: require exactly one private struct {name}; found {len(items)}"
            )
            return None
        item = items[0]
        _require_rust_item_context(
            path,
            item,
            (),
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        if _rust_item_header_tokens(item) != rust_code_tokens(
            f"pub(crate) struct {name}"
        ):
            errors.append(
                f"{path}:{item.line}: {description} must remain crate-internal"
            )
        return item

    def require_enum(
        role: str,
        name: str,
        description: str,
        *,
        expected_attributes: tuple[str, ...],
    ) -> RustItem | None:
        path = paths[role]
        items = rust_enum_items(sources[role], name)
        if len(items) != 1:
            errors.append(
                f"{path}: require exactly one private enum {name}; found {len(items)}"
            )
            return None
        item = items[0]
        _require_rust_item_context(
            path,
            item,
            (),
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        if _rust_item_header_tokens(item) != rust_code_tokens(f"enum {name}"):
            errors.append(f"{path}:{item.line}: {description} must remain private")
        return item

    def require_order(
        role: str,
        item: RustItem | None,
        description: str,
        markers: tuple[str, ...],
    ) -> None:
        if item is None:
            return
        tokens = rust_code_tokens(item.source)
        cursor = -1
        for marker in markers:
            positions = tuple(
                position
                for position in _token_sequence_positions(
                    tokens, rust_code_tokens(marker)
                )
                if position > cursor
            )
            if not positions:
                errors.append(
                    f"{paths[role]}:{item.line}: {description} must retain ordered "
                    f"marker {marker!r}"
                )
                return
            cursor = positions[0]

    def require_digest(
        role: str,
        item: RustItem | None,
        seals: dict[str, str],
        key: str,
        description: str,
    ) -> None:
        expected = seals.get(key)
        if expected is None:
            errors.append(
                f"{paths[role]}: missing reviewed direct-Serve digest for {key}"
            )
            return
        _require_rust_item_token_sha256(
            paths[role], item, expected, description, errors
        )

    completion_evidence = require_struct(
        "runtime",
        "ExactServePredecessorCompletionEvidence",
        "process-local exact predecessor completion evidence",
        expected_attributes=(
            "#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]",
        ),
    )
    _require_rust_token_sequence(
        paths["runtime"],
        completion_evidence,
        "lifecycle_ordinal: u128, lifecycle_ordinal_complement: u128,",
        "completion evidence must retain a complemented immutable ordinal",
        errors,
    )
    require_digest(
        "runtime",
        completion_evidence,
        _DIRECT_SERVE_OBSERVATION_STRUCT_SHA256,
        "ExactServePredecessorCompletionEvidence",
        "direct-Serve completion-evidence carrier",
    )
    validate_evidence = _require_qualified_rust_item(
        paths["runtime"],
        sources["runtime"],
        "ExactServePredecessorCompletionEvidence",
        "validate_exact",
        errors,
        "exact predecessor completion evidence validation",
    )
    _require_rust_token_sequence(
        paths["runtime"],
        validate_evidence,
        "self.lifecycle_ordinal > 0 && self.lifecycle_ordinal_complement == !self.lifecycle_ordinal",
        "completion evidence must reject zero and complement drift",
        errors,
    )
    require_digest(
        "runtime",
        validate_evidence,
        _DIRECT_SERVE_OBSERVATION_ITEM_SHA256,
        "ExactServePredecessorCompletionEvidence::validate_exact",
        "direct-Serve completion-evidence validation",
    )

    observation = require_struct(
        "runtime",
        "ExactServePredecessorObservation",
        "direct selected-Serve predecessor observation",
        expected_attributes=("#[derive(Clone, Copy, Debug, PartialEq, Eq)]",),
    )
    _require_rust_token_sequence(
        paths["runtime"],
        observation,
        "first_target_observation: bool, runnable_predecessor: bool,",
        "direct observation must retain exactly its initial-turn and runnable-prefix facts",
        errors,
    )
    require_digest(
        "runtime",
        observation,
        _DIRECT_SERVE_OBSERVATION_STRUCT_SHA256,
        "ExactServePredecessorObservation",
        "direct-Serve predecessor observation carrier",
    )
    observation_impl = (
        _require_qualified_rust_item(
            paths["runtime"],
            sources["runtime"],
            "ExactServePredecessorObservation",
            "should_open_predecessor_admission",
            errors,
            "direct predecessor-admission decision",
        ),
        _require_qualified_rust_item(
            paths["runtime"],
            sources["runtime"],
            "ExactServePredecessorObservation",
            "has_runnable_predecessor",
            errors,
            "direct runnable-predecessor projection",
        ),
    )
    _require_rust_token_sequence(
        paths["runtime"],
        observation_impl[0],
        "self.first_target_observation || self.runnable_predecessor",
        "initial observation or current runnable predecessor alone may open admission",
        errors,
    )
    require_digest(
        "runtime",
        observation_impl[0],
        _DIRECT_SERVE_OBSERVATION_ITEM_SHA256,
        "ExactServePredecessorObservation::should_open_predecessor_admission",
        "direct predecessor-admission decision",
    )
    _require_rust_token_sequence(
        paths["runtime"],
        observation_impl[1],
        "self.runnable_predecessor",
        "runnable projection must return the direct census fact",
        errors,
    )
    require_digest(
        "runtime",
        observation_impl[1],
        _DIRECT_SERVE_OBSERVATION_ITEM_SHA256,
        "ExactServePredecessorObservation::has_runnable_predecessor",
        "direct runnable-predecessor projection",
    )

    runtime_structs = rust_struct_items(sources["runtime"], "SerializedV2Runtime")
    if len(runtime_structs) != 1:
        errors.append(
            f"{paths['runtime']}: require exactly one SerializedV2Runtime struct; "
            f"found {len(runtime_structs)}"
        )
        runtime_struct = None
    else:
        runtime_struct = runtime_structs[0]
    _require_rust_token_sequence(
        paths["runtime"],
        runtime_struct,
        "exact_serve_target_ordinal: Option<u128>, exact_serve_predecessor_retry_attempted: bool,",
        "runtime must retain only the selected target and one bounded retry latch",
        errors,
    )

    runtime_observation = _require_rust_item(
        paths["runtime"],
        sources["runtime"],
        "exact_serve_predecessor_observation",
        errors,
    )
    require_digest(
        "runtime",
        runtime_observation,
        _DIRECT_SERVE_RUNTIME_ITEM_SHA256,
        "exact_serve_predecessor_observation",
        "serialized runtime direct predecessor census",
    )
    _require_rust_item_context(
        paths["runtime"],
        runtime_observation,
        (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
        "serialized runtime direct predecessor census",
        errors,
    )
    require_order(
        "runtime",
        runtime_observation,
        "direct predecessor census",
        (
            ".recognizes_minted(serve_lifecycle_ordinal)",
            "self.freeze_due_clock_owners(now)",
            "self.active_lifecycle_uses_ordinal(serve_lifecycle_ordinal)",
            "!evidence.validate_exact() || evidence.lifecycle_ordinal() >= serve_lifecycle_ordinal",
            "self.exact_serve_target_ordinal = Some(serve_lifecycle_ordinal)",
            "self.exact_serve_predecessor_retry_attempted = false",
            "self.minimum_runnable_lifecycle_ordinal(now, completion_evidence)",
            "minimum.filter(|ordinal| *ordinal < serve_lifecycle_ordinal)",
            "if self.exact_serve_predecessor_retry_attempted",
            "if predecessor.is_none()",
            "ExactServePredecessorObservation::new(first_target_observation, false,)",
            "ExactServePredecessorObservation::new(first_target_observation, predecessor.is_some(),)",
        ),
    )

    runtime_step = _require_rust_item(
        paths["runtime"], sources["runtime"], "step", errors
    )
    require_digest(
        "runtime",
        runtime_step,
        _DIRECT_SERVE_RUNTIME_ITEM_SHA256,
        "step",
        "serialized runtime retry suppression",
    )
    _require_rust_token_sequence(
        paths["runtime"],
        runtime_step,
        "self.exact_serve_target_ordinal.is_some_and(|target| owner.lifecycle_ordinal() < target) { self.exact_serve_predecessor_retry_attempted = true; }",
        "only a selected strictly older retry may arm direct Serve suppression",
        errors,
    )

    effects_observation = _require_rust_item(
        paths["effects"],
        sources["effects"],
        "exact_serve_predecessor_observation",
        errors,
    )
    _require_rust_item_context(
        paths["effects"],
        effects_observation,
        (("impl", "V2EffectExecutor", "<", "SerializedV2Runtime", ">"),),
        "executor-owned direct predecessor observation",
        errors,
    )
    require_order(
        "effects",
        effects_observation,
        "executor direct predecessor observation",
        (
            "self.ensure_open()?",
            "self.publish_external_lifecycle_owners()?",
            ".exact_serve_predecessor_observation(now, serve_lifecycle_ordinal, completion_evidence)",
        ),
    )
    require_digest(
        "effects",
        effects_observation,
        _DIRECT_SERVE_EFFECT_ITEM_SHA256,
        "exact_serve_predecessor_observation",
        "executor-owned direct predecessor observation",
    )

    admission_state = require_enum(
        "worker",
        "CertifiedServePredecessorAdmissionState",
        "transient selected-Serve predecessor admission state",
        expected_attributes=("#[derive(Clone, Copy, Debug, PartialEq, Eq)]",),
    )
    _require_rust_token_sequence(
        paths["worker"],
        admission_state,
        "Closed, Open { predecessor_ordinal: Option<u128>, },",
        "worker admission state must be only Closed or one-owner Open",
        errors,
    )
    require_digest(
        "worker",
        admission_state,
        _DIRECT_SERVE_WORKER_STRUCT_SHA256,
        "CertifiedServePredecessorAdmissionState",
        "transient predecessor-admission state",
    )

    admission_guard = require_struct(
        "worker",
        "CertifiedServePredecessorAdmissionV1",
        "move-only selected-Serve predecessor admission guard",
        expected_attributes=(
            '#[must_use = "the exact Serve predecessor admission must remain live for its bounded turn"]',
        ),
    )
    _require_rust_token_sequence(
        paths["worker"],
        admission_guard,
        "queue: Arc<V2IoCommandQueue>, output_guard: Arc<ConsensusOutputGuard>, barrier: CertifiedServeBarrier, armed: bool,",
        "admission guard must retain the exact queue, output guard, ticket, and armed bit",
        errors,
    )
    require_digest(
        "worker",
        admission_guard,
        _DIRECT_SERVE_WORKER_STRUCT_SHA256,
        "CertifiedServePredecessorAdmissionV1",
        "move-only predecessor-admission guard",
    )
    guard_finish = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "CertifiedServePredecessorAdmissionV1",
        "finish",
        errors,
        "explicit predecessor-admission retirement",
    )
    require_order(
        "worker",
        guard_finish,
        "explicit predecessor-admission retirement",
        (
            "self.queue.close_serve_predecessor_admission(self.barrier)?",
            "self.armed = false",
            "Ok(())",
        ),
    )
    require_digest(
        "worker",
        guard_finish,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "CertifiedServePredecessorAdmissionV1::finish",
        "explicit predecessor-admission retirement",
    )
    guard_drops = tuple(
        item
        for item in rust_items(sources["worker"], "drop")
        if item.brace_context
        == (("impl", "Drop", "for", "CertifiedServePredecessorAdmissionV1"),)
    )
    if len(guard_drops) != 1:
        errors.append(
            f"{paths['worker']}: require exactly one admission-guard Drop; "
            f"found {len(guard_drops)}"
        )
        guard_drop = None
    else:
        guard_drop = guard_drops[0]
        _require_rust_item_context(
            paths["worker"],
            guard_drop,
            (("impl", "Drop", "for", "CertifiedServePredecessorAdmissionV1"),),
            "fail-stop predecessor-admission Drop",
            errors,
        )
    require_order(
        "worker",
        guard_drop,
        "fail-stop predecessor-admission Drop",
        (
            "if !self.armed { return; }",
            ".close_serve_predecessor_admission(self.barrier)",
            ".is_err()",
            "self.output_guard.close_admission_for_restart()",
        ),
    )
    require_digest(
        "worker",
        guard_drop,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "CertifiedServePredecessorAdmissionV1::drop",
        "fail-stop predecessor-admission Drop",
    )

    queue_open = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCommandQueue",
        "open_serve_predecessor_admission",
        errors,
        "queue-local predecessor-admission open",
    )
    require_order(
        "worker",
        queue_open,
        "queue-local predecessor-admission open",
        (
            "reservation.matches_barrier(barrier)",
            "CertifiedServePredecessorAdmissionState::Closed",
            "CertifiedServePredecessorAdmissionState::Open { predecessor_ordinal: None, }",
            "CertifiedServePredecessorAdmissionState::Open { .. }",
            "Err(",
        ),
    )
    require_digest(
        "worker",
        queue_open,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "V2IoCommandQueue::open_serve_predecessor_admission",
        "queue-local predecessor-admission open",
    )
    queue_capacity = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCommandQueue",
        "serve_predecessor_capacity_available",
        errors,
        "open-admission capacity query",
    )
    _require_rust_token_sequence(
        paths["worker"],
        queue_capacity,
        "if !matches!(reservation.predecessor_admission, CertifiedServePredecessorAdmissionState::Open { .. }) { return Err(",
        "worker capacity must be unavailable outside the open transient admission",
        errors,
    )
    require_digest(
        "worker",
        queue_capacity,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "V2IoCommandQueue::serve_predecessor_capacity_available",
        "open-admission capacity query",
    )
    queue_close = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCommandQueue",
        "close_serve_predecessor_admission",
        errors,
        "queue-local predecessor-admission close",
    )
    require_order(
        "worker",
        queue_close,
        "queue-local predecessor-admission close",
        (
            "reservation.matches_barrier(barrier)",
            "CertifiedServePredecessorAdmissionState::Open { .. }",
            "reservation.predecessor_admission = CertifiedServePredecessorAdmissionState::Closed",
        ),
    )
    require_digest(
        "worker",
        queue_close,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "V2IoCommandQueue::close_serve_predecessor_admission",
        "queue-local predecessor-admission close",
    )
    service_open = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "ProductionV2Services",
        "open_certified_serve_predecessor_admission",
        errors,
        "service-owned predecessor-admission mint",
    )
    require_order(
        "worker",
        service_open,
        "service-owned predecessor-admission mint",
        (
            "io.open_serve_predecessor_admission(barrier)?",
            "CertifiedServePredecessorAdmissionV1::new(",
            "Arc::clone(&io.command_tx.queue)",
            "Arc::clone(&self.output_guard)",
            "barrier",
        ),
    )
    require_digest(
        "worker",
        service_open,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "ProductionV2Services::open_certified_serve_predecessor_admission",
        "service-owned predecessor-admission mint",
    )

    reservation_items = rust_struct_items(
        sources["worker"], "V2IoCertifiedServeIngressReservation"
    )
    if len(reservation_items) != 1:
        errors.append(
            f"{paths['worker']}: require exactly one exact-Serve ingress "
            f"reservation; found {len(reservation_items)}"
        )
        reservation = None
    else:
        reservation = reservation_items[0]
        _require_rust_item_context(
            paths["worker"],
            reservation,
            (),
            "exact-Serve ingress reservation",
            errors,
            expected_attributes=("#[derive(Debug)]",),
        )
    _require_rust_token_sequence(
        paths["worker"],
        reservation,
        "id: CertifiedServeIngressReservationId, lifecycle_id: CertifiedServeLifecycleId, projection: CertifiedServeIngressProjection, request: wire::CertifiedBodyRequest, state: CertifiedServeIngressReservationState, handed_off: Option<Arc<AtomicBool>>, carrier_ordinal: Option<u64>, predecessor_admission: CertifiedServePredecessorAdmissionState,",
        "the exact Serve reservation must retain its logical, physical, payload, and transient-admission identities",
        errors,
    )
    require_digest(
        "worker",
        reservation,
        _DIRECT_SERVE_WORKER_STRUCT_SHA256,
        "V2IoCertifiedServeIngressReservation",
        "exact-Serve ingress reservation",
    )
    reservation_barrier = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCertifiedServeIngressReservation",
        "barrier",
        errors,
        "exact-Serve barrier projection",
    )
    require_order(
        "worker",
        reservation_barrier,
        "exact-Serve barrier projection",
        (
            "if self.handed_off.is_none()",
            "self.carrier_ordinal.ok_or_else",
            "if self.id.0 == 0 || carrier_ordinal == 0",
            "request_hash: self.projection.request_hash",
            "scheduler_ordinal: self.id.0",
            "lifecycle_id: self.lifecycle_id",
            "carrier_ordinal",
        ),
    )
    require_digest(
        "worker",
        reservation_barrier,
        _DIRECT_SERVE_RESERVATION_ITEM_SHA256,
        "barrier",
        "exact-Serve barrier projection",
    )
    reservation_match = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCertifiedServeIngressReservation",
        "matches_barrier",
        errors,
        "exact-Serve barrier identity comparison",
    )
    _require_rust_token_sequence(
        paths["worker"],
        reservation_match,
        "self.id.0 == barrier.scheduler_ordinal && self.lifecycle_id == barrier.lifecycle_id && self.projection.request_hash == barrier.request_hash && self.carrier_ordinal == Some(barrier.carrier_ordinal) && self.handed_off.is_some()",
        "barrier comparison must retain every logical and physical identity component",
        errors,
    )
    require_digest(
        "worker",
        reservation_match,
        _DIRECT_SERVE_RESERVATION_ITEM_SHA256,
        "matches_barrier",
        "exact-Serve barrier identity comparison",
    )

    completion_owners = rust_struct_items(sources["worker"], "V2IoCompletionOwnership")
    if len(completion_owners) != 1:
        errors.append(
            f"{paths['worker']}: require exactly one completion ownership "
            f"carrier; found {len(completion_owners)}"
        )
        completion_owner = None
    else:
        completion_owner = completion_owners[0]
        _require_rust_item_context(
            paths["worker"],
            completion_owner,
            (),
            "copy-only completion ownership carrier",
            errors,
            expected_attributes=("#[derive(Clone, Copy, Debug)]",),
        )
    _require_rust_token_sequence(
        paths["worker"],
        completion_owner,
        "retained_at: Instant, service_debt: u64, requires_runtime_capacity: bool, runtime_lifecycle_ordinal: Option<u128>, recovered_decision_apply: Option<RecoveredDecisionApplyDispatchKeyV1>, recovered_lifecycle_sign: Option<RecoveredLifecycleSignDispatchKeyV1>, recovered_decision_fetch: Option<RecoveredDecisionFetchDispatchKeyV1>,",
        "completion ownership must retain time/debt, runtime-capacity class, and exact lifecycle/recovered-work provenance",
        errors,
    )
    require_digest(
        "worker",
        completion_owner,
        _DIRECT_SERVE_WORKER_STRUCT_SHA256,
        "V2IoCompletionOwnership",
        "copy-only completion ownership carrier",
    )

    completion_items: dict[str, RustItem | None] = {}
    for owner, name, description in (
        (
            "V2IoCommand",
            "runtime_lifecycle_ordinal",
            "I/O command lifecycle provenance projection",
        ),
        (
            "V2IoAdmission",
            "retain_completion",
            "completion ownership retention",
        ),
        (
            "V2IoAdmission",
            "abandon_latest_completion",
            "failed completion publication rollback",
        ),
        (
            "V2IoAdmission",
            "completion_ownership_at",
            "completion ownership lookup",
        ),
        (
            "V2IoHandle",
            "completion_ownership_at",
            "I/O handle completion ownership delegation",
        ),
        ("V2IoHandle", "spawn", "I/O worker completion publication"),
        (
            "LocalCompletion",
            "runtime_lifecycle_ordinal",
            "local reconstruction lifecycle provenance",
        ),
        (
            "ProductionV2Services",
            "certified_serve_predecessor_completion_evidence",
            "non-consuming exact predecessor completion evidence",
        ),
        (
            "ProductionV2Services",
            "take_lifecycle_prefix_completion",
            "strict or inclusive completion-prefix selection",
        ),
        (
            "ProductionV2Services",
            "drain_exact_serve_runtime_predecessor",
            "one-completion exact predecessor drain",
        ),
        (
            "ProductionV2Services",
            "drain_completions_inner",
            "policy-bounded completion drain",
        ),
    ):
        completion_items[f"{owner}::{name}"] = _require_qualified_rust_item(
            paths["worker"], sources["worker"], owner, name, errors, description
        )

    for name, description in (
        (
            "send_completion_with_lifecycle_ordinal",
            "completion provenance forwarding wrapper",
        ),
        (
            "send_tracked_completion_with_lifecycle_ordinal",
            "blocking tracked completion publication",
        ),
        (
            "try_send_tracked_completion_with_lifecycle_ordinal",
            "nonblocking tracked completion publication",
        ),
    ):
        item = _require_rust_item(paths["worker"], sources["worker"], name, errors)
        _require_rust_item_context(
            paths["worker"], item, (), description, errors
        )
        completion_items[name] = item

    for key, item in completion_items.items():
        require_digest(
            "worker",
            item,
            _DIRECT_SERVE_COMPLETION_PROVENANCE_ITEM_SHA256,
            key,
            f"direct-Serve completion provenance seam {key}",
        ) if key in _DIRECT_SERVE_COMPLETION_PROVENANCE_ITEM_SHA256 else None

    command_ordinal = completion_items["V2IoCommand::runtime_lifecycle_ordinal"]
    for sequence in (
        "Self::Sign { task, .. } => Some(task.lifecycle_ordinal())",
        "Self::Store(task) => Some(task.lifecycle_ordinal())",
        "Self::Validate(task) => Some(task.lifecycle_ordinal())",
        "Self::Apply(task) => Some(task.lifecycle_ordinal())",
        "Self::RecoveredDecisionApply(task) => Some(task.dispatch_key().lifecycle_ordinal())",
        "Self::RecoveredLifecycleSign(task) => Some(task.dispatch_key().lifecycle_ordinal())",
        "Self::PersistRecoveredDecisionFetchBody(task) => { Some(task.dispatch_key().lifecycle_ordinal()) }",
    ):
        _require_rust_token_sequence(
            paths["worker"],
            command_ordinal,
            sequence,
            "every completion-producing command must project its immutable runtime lifecycle ordinal",
            errors,
        )

    retain_completion = completion_items["V2IoAdmission::retain_completion"]
    require_order(
        "worker",
        retain_completion,
        "completion publication must atomically retain the exact capacity class and lifecycle provenance",
        (
            "state.owned.len() < self.completion_capacity",
            "state.owned.push_back(V2IoCompletionOwnership",
            "retained_at",
            "service_debt: 0",
            "requires_runtime_capacity",
            "runtime_lifecycle_ordinal",
            "recovered_decision_apply",
            "recovered_lifecycle_sign",
            "recovered_decision_fetch",
        ),
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["V2IoAdmission::abandon_latest_completion"],
        ".owned.pop_back().expect(",
        "a failed send must abandon only the just-retained completion tail",
        errors,
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["V2IoAdmission::completion_ownership_at"],
        ".owned.get(position).copied()",
        "completion ownership lookup must copy the exact indexed record without consuming it",
        errors,
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["V2IoHandle::completion_ownership_at"],
        "self.admission.completion_ownership_at(position)",
        "I/O handle must delegate the exact non-consuming ownership position",
        errors,
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["V2IoHandle::spawn"],
        "let runtime_lifecycle_ordinal = command.runtime_lifecycle_ordinal(); match command",
        "I/O worker must capture exact completion provenance before moving the command",
        errors,
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["V2IoHandle::spawn"],
        "send_completion_with_lifecycle_ordinal(&completion_tx, &worker_admission, Ok(completion), runtime_lifecycle_ordinal,)",
        "I/O worker must forward the pre-execution runtime lifecycle ordinal unchanged",
        errors,
    )
    require_order(
        "worker",
        completion_items["V2IoHandle::spawn"],
        "I/O worker completion publication",
        (
            "let runtime_lifecycle_ordinal = command.runtime_lifecycle_ordinal()",
            "match command",
            "send_completion_with_lifecycle_ordinal(",
            "runtime_lifecycle_ordinal",
        ),
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["LocalCompletion::runtime_lifecycle_ordinal"],
        "Self::Reconstructed { task, .. } => task.lifecycle_ordinal()",
        "every local completion must project the immutable lifecycle ordinal of its original task",
        errors,
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["send_completion_with_lifecycle_ordinal"],
        "send_tracked_completion_with_lifecycle_ordinal(sender, admission, completion, runtime_lifecycle_ordinal,)",
        "production completion wrapper must forward the captured runtime lifecycle ordinal unchanged",
        errors,
    )
    require_order(
        "worker",
        completion_items["send_completion_with_lifecycle_ordinal"],
        "completion provenance forwarding wrapper",
        (
            "let completion = completion.unwrap_or_else(V2IoCompletion::Failed)",
            "send_tracked_completion_with_lifecycle_ordinal(",
            "completion",
            "runtime_lifecycle_ordinal",
        ),
    )
    for key, send in (
        ("send_tracked_completion_with_lifecycle_ordinal", "sender.send(completion)"),
        (
            "try_send_tracked_completion_with_lifecycle_ordinal",
            "sender.try_send(completion)",
        ),
    ):
        require_order(
            "worker",
            completion_items[key],
            f"{key} exact ownership transaction",
            (
                "admission.retain_completion(",
                "runtime_lifecycle_ordinal",
                send,
                "admission.abandon_latest_completion()",
            ),
        )
        _require_rust_token_sequence(
            paths["worker"],
            completion_items[key],
            f"admission.retain_completion(Instant::now(), completion.requires_runtime_capacity(), runtime_lifecycle_ordinal,",
            (
                "blocking completion publication must retain exact ownership before send"
                if key == "send_tracked_completion_with_lifecycle_ordinal"
                else "nonblocking completion publication must retain exact ownership before send"
            ),
            errors,
        )
        _require_rust_token_sequence(
            paths["worker"],
            completion_items[key],
            "admission.abandon_latest_completion()",
            (
                "blocking completion publication must retain exact ownership before send"
                if key == "send_tracked_completion_with_lifecycle_ordinal"
                else "nonblocking completion publication must retain exact ownership before send"
            ),
            errors,
        )

    completion_projection = completion_items[
        "ProductionV2Services::certified_serve_predecessor_completion_evidence"
    ]
    require_order(
        "worker",
        completion_projection,
        "non-consuming exact predecessor completion evidence",
        (
            "if serve_lifecycle_ordinal == 0",
            "usize::from(!runtime_capacity_available && self.held_io_completion.is_some())",
            ".completion_ownership_at(ownership_position)",
            ".filter(|owned| runtime_capacity_available || !owned.requires_runtime_capacity)",
            ".and_then(|owned| owned.runtime_lifecycle_ordinal)",
            "if io_ordinal == Some(0)",
            "io_ordinal.filter(|ordinal| *ordinal < serve_lifecycle_ordinal)",
            "if runtime_capacity_available",
            "completion.runtime_lifecycle_ordinal()",
            "Some(io.min(local))",
            "ExactServePredecessorCompletionEvidence::try_new(ordinal)",
        ),
    )
    require_digest(
        "worker",
        completion_projection,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "ProductionV2Services::certified_serve_predecessor_completion_evidence",
        "non-consuming exact predecessor completion evidence",
    )
    completion_prefix = completion_items[
        "ProductionV2Services::take_lifecycle_prefix_completion"
    ]
    require_order(
        "worker",
        completion_prefix,
        "strict or inclusive completion-prefix selection",
        (
            "if inclusive { ordinal <= lifecycle_cut } else { ordinal < lifecycle_cut }",
            ".completion_ownership_at(ownership_position)",
            "runtime_capacity_available || !owned.requires_runtime_capacity",
            ".min_by_key(|completion| completion.runtime_lifecycle_ordinal())",
            "Some(CompletionSource::Io)",
            "Some(CompletionSource::Local)",
        ),
    )
    _require_rust_token_sequence(
        paths["worker"],
        completion_items["ProductionV2Services::drain_exact_serve_runtime_predecessor"],
        "self.drain_completions_inner(executor, 1, CompletionDrainPolicy::ExactServePredecessor { serve_lifecycle_ordinal, },)",
        "one exact-Serve predecessor drain must admit at most one strictly ticket-indexed completion",
        errors,
    )
    require_digest(
        "worker",
        completion_items["ProductionV2Services::drain_exact_serve_runtime_predecessor"],
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "ProductionV2Services::drain_exact_serve_runtime_predecessor",
        "one-completion exact predecessor drain",
    )
    require_digest(
        "worker",
        completion_prefix,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "ProductionV2Services::take_lifecycle_prefix_completion",
        "strict or inclusive completion-prefix selection",
    )
    completion_drain = completion_items["ProductionV2Services::drain_completions_inner"]
    for sequence, description in (
        ("while attempts < limit", "completion draining must obey its caller-supplied finite bound"),
        (
            "CompletionDrainPolicy::ExactServePredecessor { serve_lifecycle_ordinal, } => self.take_exact_serve_predecessor_completion(",
            "the exact policy must use only the strict ticket-indexed selector",
        ),
        (
            "CompletionDrainPolicy::TimeoutRecoveryPrefix { inclusive_lifecycle_cut, } => self.take_timeout_recovery_prefix_completion(",
            "timeout recovery must retain its separately inclusive selector",
        ),
    ):
        _require_rust_token_sequence(
            paths["worker"], completion_drain, sequence, description, errors
        )
    require_digest(
        "worker",
        completion_drain,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "ProductionV2Services::drain_completions_inner",
        "policy-bounded completion drain",
    )

    queue_states = rust_struct_items(sources["worker"], "V2IoCommandQueueState")
    if len(queue_states) != 1:
        errors.append(
            f"{paths['worker']}: require exactly one I/O queue state; "
            f"found {len(queue_states)}"
        )
        queue_state = None
    else:
        queue_state = queue_states[0]
    _require_rust_token_sequence(
        paths["worker"],
        queue_state,
        "producer_episode_due: bool, producer_episode_active: bool, sender_open: bool, receiver_open: bool,",
        "queue state must keep post-Serve producer debt distinct from its active owner",
        errors,
    )
    require_digest(
        "worker",
        queue_state,
        _DIRECT_SERVE_WORKER_STRUCT_SHA256,
        "V2IoCommandQueueState",
        "I/O queue producer state",
    )
    producer_episodes = rust_struct_items(
        sources["worker"], "CertifiedServeProducerEpisode"
    )
    if len(producer_episodes) != 1:
        errors.append(
            f"{paths['worker']}: require exactly one producer episode guard; "
            f"found {len(producer_episodes)}"
        )
        producer_episode = None
    else:
        producer_episode = producer_episodes[0]
        _require_rust_item_context(
            paths["worker"],
            producer_episode,
            (),
            "exact-Serve state carrier CertifiedServeProducerEpisode",
            errors,
            expected_attributes=("#[must_use]",),
        )
    _require_rust_token_sequence(
        paths["worker"],
        producer_episode,
        "queue: Arc<V2IoCommandQueue>, active: bool,",
        "producer episode guard must own the exact queue and active bit",
        errors,
    )
    require_digest(
        "worker",
        producer_episode,
        _DIRECT_SERVE_WORKER_STRUCT_SHA256,
        "CertifiedServeProducerEpisode",
        "exact-Serve state carrier CertifiedServeProducerEpisode",
    )
    producer_drops = tuple(
        item
        for item in rust_items(sources["worker"], "drop")
        if item.brace_context
        == (("impl", "Drop", "for", "CertifiedServeProducerEpisode"),)
    )
    if len(producer_drops) != 1:
        errors.append(
            f"{paths['worker']}: require exactly one producer episode Drop; "
            f"found {len(producer_drops)}"
        )
        producer_drop = None
    else:
        producer_drop = producer_drops[0]
    require_order(
        "worker",
        producer_drop,
        "ordinary producer episodes must retire under the same queue lock",
        (
            "if !self.active { return; }",
            "let mut state = self.queue.lock()",
            "if !state.producer_episode_active { return; }",
            "state.producer_episode_active = false",
            "drop(state)",
            "self.queue.ready.notify_all()",
            "self.active = false",
        ),
    )
    require_digest(
        "worker",
        producer_drop,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "CertifiedServeProducerEpisode::drop",
        "ordinary producer episodes must retire under the same queue lock",
    )
    channel_builder = _require_rust_item(
        paths["worker"], sources["worker"], "build_v2_io_command_channel", errors
    )
    require_order(
        "worker",
        channel_builder,
        "the command channel initializer must clear producer-episode due immediately before active",
        ("producer_episode_due: false", "producer_episode_active: false"),
    )
    require_digest(
        "worker",
        channel_builder,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "build_v2_io_command_channel",
        "command-channel producer state initialization",
    )
    close_receiver = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCommandQueue",
        "close_receiver",
        errors,
        "receiver teardown must clear producer-episode due before active and Serve rollback",
    )
    require_order(
        "worker",
        close_receiver,
        "receiver teardown must clear producer-episode due before active and Serve rollback",
        (
            "state.receiver_open = false",
            "state.producer_episode_due = false",
            "state.producer_episode_active = false",
            "self.rollback_serve_barrier(&mut state)",
        ),
    )
    require_digest(
        "worker",
        close_receiver,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "V2IoCommandQueue::close_receiver",
        "receiver teardown producer retirement",
    )
    reserve_serve = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCommandQueue",
        "reserve_serve_ingress",
        errors,
        "Serve admission producer exclusion",
    )
    _require_rust_token_sequence(
        paths["worker"],
        reserve_serve,
        "if state.producer_episode_due || state.producer_episode_active { return Err(CertifiedServeIngressReserveError::Busy); }",
        "fresh Serve admission must not cross a due or active producer turn",
        errors,
    )
    require_digest(
        "worker",
        reserve_serve,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "V2IoCommandQueue::reserve_serve_ingress",
        "Serve admission producer exclusion",
    )
    retire_serve = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCommandQueue",
        "retire_selected_serve_ingress_occurrence",
        errors,
        "final frozen Serve retirement must atomically arm exactly one producer episode",
    )
    require_order(
        "worker",
        retire_serve,
        "final frozen Serve retirement must atomically arm exactly one producer episode",
        (
            "let promoted = Self::promote_next_serve_ingress_waiter(state)",
            "!promoted",
            "state.serve_ingress_reservation.is_none()",
            "state.serve_ingress_waiters.is_empty()",
            "state.serve_barrier.is_none()",
            "state.sender_open",
            "state.receiver_open",
            "state.producer_episode_due = true",
            "promoted",
        ),
    )
    require_digest(
        "worker",
        retire_serve,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "V2IoCommandQueue::retire_selected_serve_ingress_occurrence",
        "final Serve-to-producer handoff",
    )
    begin_producer = _require_qualified_rust_item(
        paths["worker"],
        sources["worker"],
        "V2IoCommandQueue",
        "try_begin_producer_episode",
        errors,
        "ordinary producers must consume the one-shot handoff",
    )
    require_order(
        "worker",
        begin_producer,
        "ordinary producers must consume the one-shot handoff",
        (
            "state.serve_ingress_reservation.is_some()",
            "!state.serve_ingress_waiters.is_empty()",
            "state.serve_barrier.is_some()",
            "return Ok(None)",
            "if state.producer_episode_active",
            "state.producer_episode_due = false",
            "state.producer_episode_active = true",
            "CertifiedServeProducerEpisode",
        ),
    )
    require_digest(
        "worker",
        begin_producer,
        _DIRECT_SERVE_WORKER_ITEM_SHA256,
        "V2IoCommandQueue::try_begin_producer_episode",
        "queue-atomic producer handoff",
    )

    test_only_boolean = _require_rust_item(
        paths["runtime"],
        sources["runtime"],
        "older_lifecycle_predates_exact_serve",
        errors,
    )
    _require_rust_item_context(
        paths["runtime"],
        test_only_boolean,
        (("impl", "<", "D", ":", "RuntimeDriver", ">", "SerializedV2Runtime", "<", "D", ">"),),
        "exact-Serve runtime seam older_lifecycle_predates_exact_serve",
        errors,
        expected_attributes=("#[cfg(test)]",),
    )
    _require_rust_token_sequence(
        paths["runtime"],
        test_only_boolean,
        "self.exact_serve_predecessor_observation(now, serve_lifecycle_ordinal, None).map(ExactServePredecessorObservation::has_runnable_predecessor)",
        "the compatibility boolean must be test-only and derive from the complete direct observation",
        errors,
    )
    require_digest(
        "runtime",
        test_only_boolean,
        _DIRECT_SERVE_RUNTIME_ITEM_SHA256,
        "older_lifecycle_predates_exact_serve",
        "test-only direct predecessor boolean compatibility seam",
    )
    duplicate_projection = production_tokens.count(
        "older_runtime_lifecycle_predates_exact_serve"
    )
    if duplicate_projection:
        errors.append(
            f"{paths['effects']}: duplicate executor boolean projection must remain absent; "
            f"found {duplicate_projection}"
        )

    # Bind every reviewed direct-Serve digest to the production item it names.
    # Structural checks above remain the semantic oracle; these whole-item
    # seals prevent an equivalent marker from being retained in dead code.
    for key, expected in _DIRECT_SERVE_OBSERVATION_ITEM_SHA256.items():
        owner, name = key.rsplit("::", 1)
        item = _require_qualified_rust_item(
            paths["runtime"],
            sources["runtime"],
            owner,
            name,
            errors,
            f"reviewed direct observation seam {key}",
        )
        _require_rust_item_token_sha256(
            paths["runtime"],
            item,
            expected,
            f"reviewed direct observation seam {key}",
            errors,
        )

    for key, expected in _DIRECT_SERVE_WORKER_ITEM_SHA256.items():
        if "::" not in key:
            item = _require_rust_item(paths["worker"], sources["worker"], key, errors)
        else:
            owner, name = key.rsplit("::", 1)
            if name == "drop":
                matches = tuple(
                    candidate
                    for candidate in rust_items(sources["worker"], "drop")
                    if candidate.brace_context
                    == (("impl", "Drop", "for", owner),)
                )
                if len(matches) != 1:
                    errors.append(
                        f"{paths['worker']}: require exactly one reviewed {key}; "
                        f"found {len(matches)}"
                    )
                    item = None
                else:
                    item = matches[0]
            else:
                item = _require_qualified_rust_item(
                    paths["worker"],
                    sources["worker"],
                    owner,
                    name,
                    errors,
                    f"reviewed direct worker seam {key}",
                )
        _require_rust_item_token_sha256(
            paths["worker"],
            item,
            expected,
            f"reviewed direct worker seam {key}",
            errors,
        )

    restore_item = _require_rust_item(
        paths["worker"], sources["worker"], "restore_certified_serve_tombstones", errors
    )
    require_digest(
        "worker",
        restore_item,
        _DIRECT_SERVE_RESTORE_ITEM_SHA256,
        "restore_certified_serve_tombstones",
        "restart-restored direct Serve state",
    )

    for key, expected in _DIRECT_SERVE_EFFECT_ITEM_SHA256.items():
        item = (
            effects_observation
            if key == "exact_serve_predecessor_observation"
            else _require_rust_item(paths["effects"], sources["effects"], key, errors)
        )
        _require_rust_item_token_sha256(
            paths["effects"],
            item,
            expected,
            f"reviewed direct executor seam {key}",
            errors,
        )

    for key, expected in _DIRECT_SERVE_RUNTIME_ITEM_SHA256.items():
        item = (
            runtime_observation
            if key == "exact_serve_predecessor_observation"
            else runtime_step
            if key == "step"
            else test_only_boolean
            if key == "older_lifecycle_predates_exact_serve"
            else _require_rust_item(paths["runtime"], sources["runtime"], key, errors)
        )
        _require_rust_item_token_sha256(
            paths["runtime"],
            item,
            expected,
            f"reviewed direct runtime seam {key}",
            errors,
        )

    for key, expected in _DIRECT_SERVE_INGRESS_ITEM_SHA256.items():
        item = _require_rust_item(paths["runtime"], sources["runtime"], key, errors)
        _require_rust_item_token_sha256(
            paths["runtime"],
            item,
            expected,
            f"reviewed direct ingress seam {key}",
            errors,
        )

    runner_seals = (
        ("runner", "advance_executor_once_before_exact_serve", "advance_executor_once_before_exact_serve"),
        ("ordinary", "service_certified_serve_barrier", "ordinary_serve"),
        ("ordinary", "run_lifecycle_active_height", "ordinary_active"),
        ("pending", "service_pending_certified_serve_barrier", "pending_serve"),
        ("pending", "run_pending_active_height", "pending_active"),
    )
    for role, name, key in runner_seals:
        item = _require_rust_item(paths[role], sources[role], name, errors)
        require_digest(
            role,
            item,
            _DIRECT_SERVE_RUNNER_ITEM_SHA256,
            key,
            f"reviewed direct runner seam {key}",
        )

    for owner, name, description in (
        ("V2IoCommandQueue", "try_send_as", "ordinary worker command admission"),
        (
            "V2IoCommandQueue",
            "capture_recovered_lifecycle_sign_capacity",
            "recovered Sign capacity admission",
        ),
        (
            "V2IoCommandQueue",
            "recovered_completion_worker_capacity",
            "recovered completion census",
        ),
        (
            "V2IoCommandQueue",
            "retry_recovered_decision_apply",
            "recovered Apply retry admission",
        ),
    ):
        item = _require_qualified_rust_item(
            paths["worker"],
            sources["worker"],
            owner,
            name,
            errors,
            description,
        )
        for sequence, claim in (
            (
                "command_ordinal >= reservation.id.0",
                "must reject the exact target and later owners",
            ),
            (
                "CertifiedServePredecessorAdmissionState::Open { predecessor_ordinal: None, }",
                "must admit the first strictly older owner only while open",
            ),
            (
                "CertifiedServePredecessorAdmissionState::Open { predecessor_ordinal: Some(existing), } if existing == command_ordinal",
                "must permit bounded fanout only for the already-selected owner",
            ),
            (
                "CertifiedServePredecessorAdmissionState::Closed | CertifiedServePredecessorAdmissionState::Open { .. } => None",
                "must reject closed, later, and second-owner admissions",
            ),
        ):
            _require_rust_token_sequence(
                paths["worker"], item, sequence, f"{description} {claim}", errors
            )
        if name == "try_send_as":
            _require_rust_token_sequence(
                paths["worker"],
                item,
                """
let rolled_back_shutdown = matches!(&command, V2IoCommand::Shutdown)
    && state.serve_barrier.is_none()
    && state.serve_ingress_reservation.is_none()
    && state.serve_barrier_predecessors.is_empty()
    && state.pending_serve_requests.is_empty();
if exact_target_active
    && exact_predecessor_ordinal.is_none()
    && !rolled_back_shutdown
""",
                "ordinary worker command admission must let only an exact rolled-back shutdown bypass a dormant Serve waiter",
                errors,
            )

    ordinary = _require_rust_item(
        paths["ordinary"], sources["ordinary"], "service_certified_serve_barrier", errors
    )
    _require_rust_item_context(
        paths["ordinary"],
        ordinary,
        (),
        "ordinary direct selected-Serve predecessor turn",
        errors,
        expected_attributes=("#[allow(clippy::too_many_arguments, clippy::too_many_lines)]",),
    )
    require_order(
        "ordinary",
        ordinary,
        "ordinary direct selected-Serve predecessor turn",
        (
            ".certified_serve_predecessor_completion_evidence(",
            "executor.exact_serve_predecessor_observation(",
            ".should_open_predecessor_admission()",
            ".open_certified_serve_predecessor_admission(serve_barrier)",
            ".drain_exact_serve_runtime_predecessor(executor, serve_barrier.scheduler_ordinal())",
            "executor.exact_serve_predecessor_observation(",
            ".certified_serve_predecessor_capacity_available(serve_barrier)",
            "advance_executor_once_before_exact_serve(",
            "V2IngressDrainMode::CertifiedFenceEscape",
            "executor.exact_serve_predecessor_observation(",
            "older_predecessor_remains = predecessor.has_runnable_predecessor()",
            "predecessor_admission.finish()",
            "service_certified_serve_barrier_liveness_turn(false",
        ),
    )

    pending = _require_rust_item(
        paths["pending"],
        sources["pending"],
        "service_pending_certified_serve_barrier",
        errors,
    )
    _require_rust_item_context(
        paths["pending"],
        pending,
        (),
        "pending-Kura direct selected-Serve predecessor turn",
        errors,
        expected_attributes=("#[allow(clippy::too_many_arguments, clippy::too_many_lines)]",),
    )
    require_order(
        "pending",
        pending,
        "pending-Kura direct selected-Serve predecessor turn",
        (
            ".certified_serve_predecessor_completion_evidence(",
            "executor.exact_serve_predecessor_observation(",
            ".should_open_predecessor_admission()",
            ".open_certified_serve_predecessor_admission(serve_barrier)",
            ".drain_exact_serve_runtime_predecessor(executor, serve_barrier.scheduler_ordinal())",
            "executor.exact_serve_predecessor_observation(",
            ".certified_serve_predecessor_capacity_available(serve_barrier)",
            "output_guard.close_admission_for_restart()",
            "executor.exact_serve_predecessor_observation(",
            "predecessor_admission.finish()",
            "service_certified_serve_barrier_liveness_turn(true",
        ),
    )
    pending_tokens = rust_code_tokens(pending.source) if pending is not None else ()
    for forbidden in (
        "advance_executor_once_before_exact_serve",
        "CertifiedFenceEscape",
    ):
        if forbidden in pending_tokens:
            errors.append(
                f"{paths['pending']}:{pending.line}: pending-Kura no-clock Serve turn "
                f"must not invoke ordinary predecessor work {forbidden}"
            )

    for role, name, description, required in (
        (
            "runtime",
            "retry_unadmitted_predecessor_gets_one_bounded_serve_attempt",
            "runtime direct-observation retry regression",
            (
                ("first_observation.should_open_predecessor_admission()", 1),
                ("first_observation.has_runnable_predecessor()", 1),
                ("!suppressed.should_open_predecessor_admission()", 1),
                ("!suppressed.has_runnable_predecessor()", 1),
                ("!runtime.exact_serve_predecessor_retry_attempted", 1),
            ),
        ),
        (
            "effects",
            "late_passive_fetch_completion_opens_one_serve_predecessor_admission_and_steps",
            "late passive Fetch direct-observation regression",
            (
                ("initial.should_open_predecessor_admission()", 1),
                ("!initial.has_runnable_predecessor()", 1),
                ("observation.has_runnable_predecessor()", 1),
                ("services.store_tasks.is_empty()", 1),
                ("fixture.executor.ready_bodies.len()", 1),
                ("!terminal.should_open_predecessor_admission()", 1),
                ("!terminal.has_runnable_predecessor()", 1),
            ),
        ),
        (
            "worker_cases",
            "dropping_exact_serve_predecessor_admission_closes_transient_aperture",
            "move-only guard Drop regression",
            (
                (".open_certified_serve_predecessor_admission(barrier)", 3),
                ("drop(predecessor_admission)", 1),
                (".finish()", 1),
            ),
        ),
        (
            "worker_cases",
            "exact_serve_predecessor_admission_is_transient_and_barrier_bound",
            "barrier-bound transient admission regression",
            (
                (".open_serve_predecessor_admission(barrier)", 3),
                (".open_serve_predecessor_admission(barrier).is_err()", 1),
                (".close_serve_predecessor_admission(barrier)", 3),
                (".close_serve_predecessor_admission(barrier).is_err()", 1),
            ),
        ),
        (
            "worker_cases",
            "exact_serve_predecessor_admission_services_older_local_without_admitting_later_io",
            "strict older and same-owner worker admission regression",
            (
                ("let first_ticket_ordinal = 50", 1),
                ("take_exact_serve_predecessor_completion", 3),
                ("let later_ordinal = 70_u128", 1),
            ),
        ),
        (
            "producer_cases",
            "final_serve_retirement_yields_one_producer_episode_before_replenishment",
            "post-Serve producer regression must reject replenishment both before and during the producer episode",
            (
                ("Err(CertifiedServeIngressReserveError::Busy)", 2),
                ("assert!(state.producer_episode_due)", 1),
                ("assert!(state.producer_episode_active)", 1),
                ("drop(producer_episode)", 1),
            ),
        ),
        (
            "runner_cases",
            "closed_certified_serve_predecessor_admission_cannot_veto_pacemaker",
            "closed predecessor admission liveness regression",
            (("service_certified_serve_barrier_liveness_turn(false", 1),),
        ),
    ):
        item = _require_rust_item(paths[role], sources[role], name, errors)
        if role in ("runtime", "effects"):
            expected_context = (("#", "[", "cfg", "(", "test", ")", "]", "mod", "tests"),)
            expected_attributes = ("#[test]",)
        elif role == "runner_cases":
            expected_context = ()
            expected_attributes = ("#[test]", "#[allow(clippy::too_many_lines)]")
        else:
            expected_context = ()
            expected_attributes = ("#[test]",)
        _require_rust_item_context(
            paths[role],
            item,
            expected_context,
            description,
            errors,
            expected_attributes=expected_attributes,
        )
        regression_seals = (
            _DIRECT_SERVE_RUNTIME_REGRESSION_TEST_SHA256
            if role == "runtime"
            else _DIRECT_SERVE_EFFECT_REGRESSION_TEST_SHA256
            if role == "effects"
            else _DIRECT_SERVE_REGRESSION_TEST_SHA256
            if role in ("worker_cases", "producer_cases")
            else {}
        )
        if name in regression_seals:
            _require_rust_item_token_sha256(
                paths[role],
                item,
                regression_seals[name],
                description,
                errors,
            )
        for sequence, count in required:
            _require_rust_token_sequence(
                paths[role], item, sequence, description, errors, count=count
            )

    # The focused checks above explain the protocol boundaries.  Seal every
    # remaining regression too, so adding one to the reviewed inventory cannot
    # leave it as passive metadata that the direct checker never consumes.
    for name, expected_sha256 in _DIRECT_SERVE_REGRESSION_TEST_SHA256.items():
        role = (
            "producer_cases"
            if name
            == "final_serve_retirement_yields_one_producer_episode_before_replenishment"
            else "worker_cases"
        )
        item = _require_rust_item(paths[role], sources[role], name, errors)
        _require_rust_item_context(
            paths[role],
            item,
            (),
            "direct-Serve reviewed regression",
            errors,
            expected_attributes=("#[test]",),
        )
        _require_rust_item_token_sha256(
            paths[role],
            item,
            expected_sha256,
            "direct-Serve reviewed regression",
            errors,
        )

    return errors


def _require_lane_predecessor_ordering_source_contracts(
    lane_path: Path,
    lane_ack_items: dict[str, RustItem | None],
    lane_items: dict[str, RustItem | None],
    errors: list[str],
) -> None:
    """Bind raw recovery transport separately from applied-predecessor output."""

    predecessor = lane_ack_items.get(
        "V2LaneWorkAdapter::proposal_predecessor_is_ready_for_progress"
    )
    _require_exact_rust_tokens(
        lane_path,
        predecessor,
        """
fn proposal_predecessor_is_ready_for_progress(
    &self,
    proposal: &LaneBlockProposalV1
) -> bool {
    if self
        .historical_autonomous_recovery_record_for_proposal(proposal)
        .is_some()
        || self.autonomous_payload_is_expected_for(proposal)
    {
        self.state
            .certified_autonomous_lane_block_predecessor_is_globally_applied_cached(proposal)
    } else {
        self.state
            .certified_lane_block_predecessor_is_applied_or_snapshot_anchored_cached(proposal)
    }
}
""",
        "lane predecessor readiness must dispatch autonomous and ordinary proofs to their exact applied-state authorities",
        errors,
    )
    preflight = lane_ack_items.get("V2LaneWorkAdapter::preflight_effect_insertion")
    _require_exact_rust_tokens(
        lane_path,
        preflight,
        """
fn preflight_effect_insertion(
    &mut self,
    effect: &V2LaneWorkEffect,
) -> Result<Hash, LaneWorkEffectInsertionOutcome> {
    let predecessor_ready = match effect {
        V2LaneWorkEffect::PostLaneBlock { message, .. } => {
            self.outbound_lane_message_predecessor_is_ready(message)
        }
        V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } => {
            self.proposal_predecessor_is_ready_for_progress(&certificate.proposal)
        }
        _ => true,
    };
    if !predecessor_ready {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    if !lane_work_effect_reply_routes_have_valid_shape(effect) {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    let key = lane_work_effect_key(effect);
    if self.effect_keys.contains(&key) {
        return Err(
            if self
                .effects
                .iter_mut()
                .find(|queued| lane_work_effect_key(queued) == key)
                .is_some_and(|queued| merge_lane_work_effect_reply_routes(queued, effect))
            {
                LaneWorkEffectInsertionOutcome::Duplicate
            } else {
                LaneWorkEffectInsertionOutcome::Rejected
            },
        );
    }
    if !lane_work_effect_reply_routes_are_valid(effect) {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    if self.effects.len() >= self.limits.effect_capacity.get() {
        return Err(LaneWorkEffectInsertionOutcome::Rejected);
    }
    Ok(key)
}
""",
        "ordinary lane effect preflight must retain exact identity, bounded capacity, complete reply-route history, and predecessor readiness",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        preflight,
        """
let predecessor_ready = match effect {
    V2LaneWorkEffect::PostLaneBlock { message, .. } => {
        self.outbound_lane_message_predecessor_is_ready(message)
    }
    V2LaneWorkEffect::PostDurableLaneCertificate { certificate, .. } => {
        self.proposal_predecessor_is_ready_for_progress(&certificate.proposal)
    }
    _ => true,
};
if !predecessor_ready {
    return Err(LaneWorkEffectInsertionOutcome::Rejected);
}
""",
        "lane effect admission must reject every fresh consensus output whose economic predecessor is not durably applied",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_ack_items.get("V2LaneWorkAdapter::persist_anchored_sessions"),
        """
if !self.proposal_predecessor_is_ready_for_progress(&session.proposal) {
    retained.push_back(session);
    continue;
}
""",
        "anchored lane persistence must retain a certified successor until its economic predecessor is durably applied",
        errors,
    )
    _require_rust_token_sequence(
        lane_path,
        lane_items.get("reconstruct_durable_lane_certificate"),
        """
if !self.proposal_predecessor_is_ready_for_progress(proposal) {
    return Ok(None);
}
""",
        "lane recovery reconstruction must not emit a successor certificate before its economic predecessor is durably applied",
        errors,
    )
    hydration = lane_ack_items.get("V2LaneWorkAdapter::hydrate_canonical_lane_artifacts")
    for expected, description in (
        (
            """
if !raw_slots.insert((lane_id, lane_block_height)) {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration contains a duplicate or cyclic slot".to_owned(),
    ));
}
""",
            "raw lane hydration must fail stop on a duplicate or cyclic predecessor slot",
        ),
        (
            """
if raw_proposals.len().saturating_add(route_chain.len())
    >= ordinary_hydration_capacity
{
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration exceeds bounded session capacity".to_owned(),
    ));
}
let artifact = self
    .kura
    .read_lane_block_artifact_without_sidecar_repair(lane_id, lane_block_height)
    .ok_or_else(|| {
        self.output_guard.close_admission_for_restart();
        V2LaneWorkError::Persistence(
            "canonical raw lane hydration is missing an indexed artifact".to_owned(),
        )
    })?;
""",
            "raw lane hydration must fail stop at the exact bounded inventory and read only the indexed immutable artifact",
        ),
        (
            """
|| !canonical_shape
|| !self.lane_route_active(
    ownership.lane_id,
    ownership.dataspace_id,
    ownership.lane_incarnation,
    ownership.proposal_height,
)
|| self
    .state
    .committed_block_hash_at_height(ownership.proposal_height)
    != Some(artifact.proposal_block_hash)
""",
            "raw lane hydration must reject malformed, inactive, or non-canonical carrier ownership",
        ),
        (
            """
if canonical.as_slice() != [artifact.clone()] {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration found non-unique ownership".to_owned(),
    ));
}
""",
            "raw lane hydration must require one exact canonical ownership artifact",
        ),
        (
            """
if !canonical_raw_lane_predecessor_matches_proposal(
    self.state.as_ref(),
    self.kura.as_ref(),
    &proposal,
) {
    self.output_guard.close_admission_for_restart();
    return Err(V2LaneWorkError::InvalidContext(
        "canonical raw lane hydration found a gap or conflicting predecessor".to_owned(),
    ));
}
lane_block_height = previous_height;
""",
            "raw lane hydration must authenticate every unapplied predecessor link before walking backward",
        ),
        (
            """
route_chain.reverse();
raw_proposals.extend(route_chain);
""",
            "raw lane hydration must restore each predecessor chain in forward application order",
        ),
        (
            """
raw_proposals.sort_by_key(|proposal| {
    let descriptor = &proposal.descriptor;
    (
        descriptor.proposal_height,
        descriptor.lane_id,
        descriptor.dataspace_id,
        descriptor.lane_block_height,
        proposal.proposal_hash,
    )
});
for proposal in raw_proposals {
""",
            "raw lane hydration must install independent chains in canonical deterministic order",
        ),
    ):
        _require_rust_token_sequence(lane_path, hydration, expected, description, errors)
