"""Necessary QueuePlan capacity checks; no claim of a future global carrier.

Bind recovered signed geometry, local allocations, canonical sizing and real
service-owned receipt boundaries. Mandatory dynamic metadata remains separate.
"""
from pathlib import Path
import re
from sumeragi_v2_multilane_reviewed_rust_source import _mask_rust_comments
from sumeragi_v2_multilane_geometry_evidence_contract import _code

MODEL = "SumeragiV2QueuePlanAdmissionRegistry"
CAPACITY = "crates/iroha_core/src/sumeragi/admission_capacity.rs"
INPUT = "crates/iroha_core/src/sumeragi/admission_input.rs"
RUNNER = "crates/iroha_core/src/sumeragi/v2_runner.rs"
CANDIDATE = "crates/iroha_core/src/sumeragi/v2_candidate.rs"
PENDING = "crates/iroha_core/src/sumeragi/v2_runner/lifecycle_pending_kura.rs"
HANDLE = "crates/iroha_core/src/sumeragi/mod.rs"
SIZING = "crates/iroha_core/src/torii_proxy/lane_admitted_input.rs"
TORII = "crates/iroha_torii/src/lib.rs"
PERSIST = "persist_queue_plan_admission_certificate"
AGGREGATOR = "execute_torii_proxy_request_across_candidates"
AGGREGATOR_CALLERS = (
    "execute_torii_proxy_request_with_fallback_admitted",
    "forward_incoming_torii_proxy_request",
)

BINDINGS = (
    (CANDIDATE, "fn", "candidate_economic_work_first", (
        "height % 2 == 0",
    )),
    (CANDIDATE, "fn", "npos_effects_prefix", (
        "original\n        .clone()", "effects.v2_evidence_admissions.truncate(count)",
        "effects", ".filter(|effects| !effects.is_empty())",
    )),
    (CANDIDATE, "fn", "fit_evidence_prefix", (
        "let mut low = 0", "effects.v2_evidence_admissions.len()", "while low < high",
        "let mid = low + (high - low).div_ceil(2)",
        ".with_npos_consensus_effects(npos_effects_prefix(original, mid))",
        ".canonical_proposal_wire_len(signatory, algorithm)", "encoded_chunk_count(layout, bytes)?",
        "if bytes <= payload_limit && chunks <= layout.max_chunk_count as usize",
        "low = mid", "high = mid - 1",
        "builder.with_npos_consensus_effects(npos_effects_prefix(original, low))",
    )),
    (CANDIDATE, "method", "V2CandidateAssembler::assemble", (
        "let original_npos_effects = request.attachments.npos_consensus_effects.clone()",
        "request.attachments.npos_consensus_effects = original_npos_effects.clone()",
        "candidate_economic_work_first(request.context.height)", "mandatory.queue_plan_admissions.clear()",
        "autonomous_lane_payloads: prepared_work.autonomous_lane_payloads.clone()",
        "npos_effects_prefix(&original_npos_effects, preferred_count)",
        "let (fitted, count) = fit_evidence_prefix(", "report.evidence_deferred = evidence_count - count",
        "if first_admission_size.is_none() && evidence_count > 0",
        'reason: CandidateWorkDeferral::EvidenceEnvelope',
        "CandidateAssemblyOutcome::WorkDeferred", "begin_fail_stop_operation()",
        "candidate_block_has_proposal_work(", "if canonical_wire.len() != encoded_bytes",
    )),
    (CAPACITY, "fn", "publish_authenticated_capacity", (
        "verified: &VerifiedHeightContext", "config: &SumeragiV2Config",
        "let context = verified.context()", "network_id: context.network_id",
        "protocol_version: context.protocol_version", "layout: context.da_layout",
        "capacity.check_payload_size(&capacity.network_id, capacity.layout.max_payload_size_bytes)?",
        "require_local_payload_capacity(capacity.layout, config)?", "slot.set(capacity)",
    )),
    (CAPACITY, "fn", "require_local_payload_capacity", (
        "let required = layout.max_payload_size_bytes", 'if required == 0 {\n        return Err(',
        "config.limits.max_payload_bytes", "config.limits.ready_body_bytes",
        "config.limits.body_source_bytes", 'if available < required {\n            return Err(',
    )),
    (CAPACITY, "method", "AuthenticatedAdmissionCapacityV1::check_payload_size", (
        'if network_id != &self.network_id {\n            return Err(',
        "wire::expected_encoded_chunk_count(bytes, self.layout)",
        "chunk_count > self.layout.max_chunk_count", ".checked_mul(u64::from(self.layout.chunk_size_bytes))",
        "encoded_bytes > wire::MAX_DA_ENCODED_PAYLOAD_BYTES",
    )),
    (HANDLE, "method", "SumeragiHandle::authenticated_admission_capacity", (
        'if self.emergency_fast_disabled {\n            return Err(AdmissionCapacityUnavailableV1::Disabled);\n        }',
        'if self.output_guard.restart_required() {\n            return Err(AdmissionCapacityUnavailableV1::RestartRequired);\n        }',
        'self.admission_capacity\n            .get()\n            .copied()\n            .ok_or(AdmissionCapacityUnavailableV1::Pending)',
    )),
    (RUNNER, "fn", "run_inner", (
        "let shared_config = config.v2_config(block_cadence, terminal_context.mode)?",
        'super::admission_capacity::publish_authenticated_capacity(\n                &admission_capacity,\n                terminal.verified_context(),\n                &shared_config,\n            )',
        "let shared_config = config.v2_config(block_cadence, verified_context.context().mode)?",
        'super::admission_capacity::publish_authenticated_capacity(\n        &admission_capacity,\n        &verified_context,\n        &shared_config,\n    )',
    )),
    (RUNNER, "fn", "candidate_limits", (
        "super::admission_capacity::require_local_payload_capacity(context.da_layout, config)",
        "let context_payload = usize::try_from(context.da_layout.max_payload_size_bytes)?",
        "let max_payload = NonZeroUsize::new(context_payload).ok_or(V2RunnerError::InvalidLimits)?",
        'CandidateLimits::new(\n        max_transactions,\n        max_payload,',
    )),
    (PENDING, "fn", "run_pending_kura_lifecycle_height", (
        "let shared_config = config.v2_config(block_cadence, context.mode)?",
        'super::super::admission_capacity::require_local_payload_capacity(\n        context.da_layout,\n        &shared_config,\n    )',
        "beacon_readiness.begin_height(context.id())", "runtime_queue_config(&shared_config)?",
    )),
    (INPUT, "method", "SumeragiHandle::check_queue_plan_input_capacity", (
        "let capacity = self.authenticated_admission_capacity()?",
        'if !self.admission_ready() {\n            return Err(QueuePlanInputCapacityErrorV1::Inactive);\n        }',
        'if capacity.network_id() != *network_id {\n            return Err(',
        "validate_queue_plan_binding_for_request(binding, network_id, entrypoint, &plan)",
        "maximum_lane_admitted_input_encoded_len_v1(entrypoint, binding)",
        'require_capacity(\n            "complete input",\n            input_bytes,\n            MAX_QUEUE_PLAN_ADMISSION_BYTES,\n        )?',
        "maximum_lane_admitted_input_envelope_sizes_v1(entrypoint, binding)",
        "u64::try_from(sizes.native_payload_bytes)", 'capacity\n            .check_payload_size(network_id, native_bytes)',
        'sizes.publication_plaintext_bytes,\n                self.block.control_frame_byte_capacity',
        'sizes.republication_plaintext_bytes,\n                self.block.consensus_frame_byte_capacity',
        'sizes.publication_queue_bytes,\n                self.block.outbound_high_frame_byte_capacity',
        'sizes.republication_queue_bytes,\n                self.block.outbound_high_frame_byte_capacity',
        "require_capacity(envelope, required, capacity)?",
    )),
    (INPUT, "fn", "require_capacity", (
        'if required > capacity {\n        return Err(QueuePlanInputCapacityErrorV1::Oversized',
        'envelope,\n            required,\n            capacity', "Ok(())",
    )),
    (SIZING, "fn", "maximum_lane_admitted_input_sizing_value_v1", (
        "binding.validate_structure()?", "validate_queue_plan_binding_for_transaction_and_plan(binding, entrypoint, &plan)?",
        "let threshold = usize::from(coordinator.durability_threshold)",
        ".signature_payload_len()", "right.0.cmp(&left.0).then(left.1.cmp(&right.1))",
        "shapes.truncate(threshold)", "shapes.sort_unstable_by_key(|(_, index)| *index)",
        "signature: Signature::from_bytes(&vec![0xa5; length])", "entrypoint: entrypoint.clone()",
        "binding: binding.clone()",
    )),
    (SIZING, "fn", "maximum_lane_admitted_input_envelope_sizes_v1", (
        "maximum_lane_admitted_input_sizing_value_v1(entrypoint, binding)?",
        "norito::canonical_frame_len(&sizing_only)",
        'if complete_input_bytes > iroha_data_model::block::MAX_QUEUE_PLAN_ADMISSION_BYTES {\n        return Err(',
        "norito::encode_canonical(&sizing_only)", "for bound in &binding.admission_context.route_incarnations",
        "routes.insert((route.lane_id, route.dataspace_id), bound.lane_incarnation)",
        "previous != bound.lane_incarnation", "native_sizing_only.descriptor.validate_structure()?",
        "norito::canonical_frame_len(&native_sizing_only)",
        "crate::NetworkMessage::QueuePlanAdmissionPublication", "crate::NetworkMessage::QueuePlanAdmissionCertificate",
        "queue_plan_direct_frame_sizes_v1(&publication)?", "queue_plan_direct_frame_sizes_v1(&republication)?",
    )),
    (SIZING, "fn", "queue_plan_direct_frame_sizes_v1", (
        "norito::core::DecodeFlagsGuard::enter(norito::core::default_encode_flags())",
        "norito::core::encoded_payload_len(message)",
        'iroha_p2p::network::direct_data_frame_wire_len_from_payload_len::<\n        crate::NetworkMessage,\n    >(payload_bytes)',
        "iroha_p2p::frame_queue_charge(plaintext)", "Ok((plaintext, queued))",
    )),
    (TORII, "fn", "queue_plan_service_input_capacity", (
        'app\n        .sumeragi\n        .as_ref()\n        .ok_or(',
        'QueuePlanInputCapacityErrorV1::Unavailable(\n            AdmissionCapacityUnavailableV1::Pending,\n        )',
        "handle.check_queue_plan_input_capacity(app.state.network_id_ref(), entrypoint, binding)",
    )),
    (TORII, "fn", "queue_plan_service_input_capacity_error", ('let error = queue_plan_capacity_wait::wait(\n'
 '        || queue_plan_service_input_capacity(app, entrypoint, binding),\n'
 '        || queue_plan_capacity_wait::remaining(deadline, deadline_unix_ms),\n'
 '    )\n'
 '    .await\n'
 '    .err()?;',
 'QueuePlanInputCapacityErrorV1::Unavailable(_) | QueuePlanInputCapacityErrorV1::Inactive => {\n'
 '            (\n'
 '                StatusCode::SERVICE_UNAVAILABLE,\n'
 '                "queue_plan_admission_capacity_unavailable",\n'
 '            )\n'
 '        }',
 'Some(torii_proxy_error_response(status, code, error.to_string()))',
 'queue_plan_capacity_wait::WaitError::Deadline(error)',
 'queue_plan_outcome_unknown_response(\n'
 '                binding.entrypoint_hash,\n'
 '                binding.signed_transaction_hash,')),
    (TORII, "fn", "queue_plan_request_service_capacity_error", ('ToriiProxyRequestKindV1::SubmitTransaction',
 'admission: ToriiProxyTransactionAdmissionV1::QueuePlanSynced',
 'admission_binding: Some(binding)',
 'queue_plan_service_input_capacity_error(\n'
 '            app,\n'
 '            transaction,\n'
 '            binding,\n'
 '            deadline,\n'
 '            deadline_unix_ms,\n'
 '        )\n'
 '        .await')),
    (TORII, "fn", "execute_torii_proxy_request_with_fallback_admitted", ('let proxy_memory = match pre_admitted_proxy_memory',
 'None => match acquire_torii_proxy_memory(app)',
 'hold_torii_proxy_memory_in_response_body(response, proxy_memory)',
 'if let Some(response) = queue_plan_request_service_capacity_error(\n'
 '        app,\n'
 '        &request.request,\n'
 '        tokio::time::Instant::from_std(request_started) + TORII_PROXY_EXECUTION_BUDGET,\n'
 '        request.deadline_unix_ms,\n'
 '    )\n'
 '    .await\n'
 '    {\n'
 '        return response;\n'
 '    }',
 'execute_torii_proxy_request_across_candidates(',
 'persist_queue_plan_admission_certificate(')),
    (TORII, "fn", "forward_incoming_torii_proxy_request", ('if let Some(response) = queue_plan_request_service_capacity_error(\n'
 '        app,\n'
 '        &forwarded_request.request,\n'
 '        request_started + TORII_PROXY_EXECUTION_BUDGET,\n'
 '        forwarded_request.deadline_unix_ms,\n'
 '    )\n'
 '    .await\n'
 '    {\n'
 '        return response;\n'
 '    }',
 'execute_torii_proxy_request_across_candidates(')),
    (TORII, "fn", "execute_incoming_torii_proxy_request_with_admission_inner", ('queue_plan_service_input_capacity_error(\n'
 '                app,\n'
 '                &transaction,\n'
 '                &admission_binding,\n'
 '                execution_deadline,\n'
 '                request_head.deadline_unix_ms,\n'
 '            )\n'
 '            .await',
 'push_accepted_transaction_for_ingress_with_routing_plan_strict_durable_claim(',
 'queue_plan_synced_admission_response(',
 'if transaction.admission_intent() != TransactionAdmissionIntent::QueuePlanSynced {',
 'let accepted_tx = match routing::accept_transaction_for_ingress(',
 'if admission_binding.request_id != request_head.request_id {',
 'if admission_binding.request_id != canonical_request_id {',
 'iroha_core::torii_proxy::validate_queue_plan_binding_for_request(\n'
 '                &admission_binding,\n'
 '                app.state.network_id_ref(),\n'
 '                &transaction,\n'
 '                &ingress_plan,\n'
 '            )',
 '.route_plan_with_state(&accepted_tx, app.state.as_ref())',
 'if let Some(response) = canonical_queue_plan_synced_response(\n'
 '                app,\n'
 '                &authenticated,\n'
 '                &admission_binding,\n'
 '                ingress_plan.coordinator_route(),\n'
 '                proxy_memory.as_ref(),\n'
 '                execution_deadline,\n'
 '            ) {\n'
 '                return response;\n'
 '            }',
 'execution_deadline: tokio::time::Instant',
 'let authenticated = match AuthenticatedQueuePlanRetry::from_entrypoint(',
 'authenticated.entrypoint_hash()')),
)

BINDINGS += (
    (TORII, "fn", "execute_incoming_torii_proxy_request_with_admission", (
        "let budget_observed_at = tokio::time::Instant::now()",
        "queue_plan_capacity_wait::deadline_response(&proxy_request.request, error)",
        "let deadline = budget_observed_at + remaining_budget",
    )),
    ("crates/iroha_torii/src/queue_plan_capacity_wait.rs", "fn", "deadline_response", (
        "admission: super::ToriiProxyTransactionAdmissionV1::QueuePlanSynced",
        'super::queue_plan_outcome_unknown_response(\n            transaction.hash(),\n            super::signed_transaction_hash_for_entrypoint(transaction),\n            reason,\n        )',
        'super::torii_proxy_error_response(\n            super::StatusCode::REQUEST_TIMEOUT,\n            "proxy_deadline_exceeded",\n            reason,\n        )',
    )),
    ("crates/iroha_torii/src/queue_plan_capacity_wait.rs", "fn", "wait", (
        "loop {\n        remaining().map_err(WaitError::Deadline)?;",
        "let budget = remaining().map_err(WaitError::Deadline)?;",
        "match check()",
        "Ok(()) => return remaining().map(|_| ()).map_err(WaitError::Deadline)",
        "Err(QueuePlanInputCapacityErrorV1::Inactive) => {}",
        "Err(error) => return Err(WaitError::Capacity(error))",
        "tokio::time::sleep(budget.min(Duration::from_millis(25))).await",
    )),
    ("crates/iroha_torii/src/queue_plan_capacity_wait.rs", "fn", "remaining", (
        "super::validate_torii_proxy_deadline(deadline_unix_ms)?",
        "checked_duration_since(tokio::time::Instant::now())",
        ".filter(|remaining| !remaining.is_zero())",
        "Ok(absolute.min(local))",
    )),
)

# Existing registry bindings retain this owner's full persistence obligations.
EXTRA_ITEMS = ((TORII, "fn", PERSIST), (TORII, "fn", AGGREGATOR), (RUNNER, "fn", "candidate_attachments"))
SOURCE_RELATIVES = (
    *(Path(p) for p in sorted({p for p, _, _, _ in BINDINGS})),
    Path("scripts/formal/sumeragi_v2_multilane_admission_capacity_contract.py"),
    Path("pytests/scripts/sumeragi_v2_multilane_admission_capacity_contract_test.py"),
)


def validate_aggregator_callers(source, callers, errors):
    """Close the actual library call inventory around the two guarded owners.

    The generic transport aggregator is deliberately protocol-only. Every use
    (including a function-value escape) must remain in one reviewed dispatcher;
    comments and test include files do not create production callers.
    """
    masked = _mask_rust_comments(source)
    use = re.compile(r"\b" + re.escape(AGGREGATOR) + r"\b")
    declaration = re.compile(r"\bfn\s+" + re.escape(AGGREGATOR) + r"\b")
    expected = len(AGGREGATOR_CALLERS)
    if len(declaration.findall(masked)) != 1 or len(use.findall(masked)) != expected + 1:
        errors.append("Admission capacity aggregator production caller inventory changed")
    for symbol in AGGREGATOR_CALLERS:
        if len(use.findall(_mask_rust_comments(callers.get(symbol, "")))) != 1:
            errors.append(f"Admission capacity aggregator must occur once in {symbol}")


def validate_owners(root, models, errors, rust_binding_item):
    """Require the authenticated capacity owner before the actual promise cuts."""
    owners = [m for m in models if m.get("module") == MODEL]
    rows = owners[0].get("production_symbols", ()) if len(owners) == 1 else ()
    items = {}
    for path, kind, symbol, tokens in BINDINGS:
        matches = [r for r in rows if (r.get("path"), r.get("kind"), r.get("symbol")) == (path, kind, symbol)]
        if len(matches) != 1 or tuple(matches[0].get("required_tokens", ())) != tokens:
            errors.append(f"Admission capacity ledger owner differs: {symbol}")
        raw = rust_binding_item(root, path, kind, symbol, "Admission capacity", errors)
        if raw is None:
            continue
        items[symbol] = _code(raw)
        for token in tokens:
            if _code(token) not in items[symbol]:
                errors.append(f"Admission capacity {symbol} missing relation {token!r}")
    for path, kind, symbol in EXTRA_ITEMS:
        raw = rust_binding_item(root, path, kind, symbol, "Admission capacity", errors)
        if raw is not None:
            items[symbol] = _code(raw)

    validate_aggregator_callers((root / TORII).read_text(), items, errors)

    # Prefix fitting may edit only admissions: penalties and the finalized pulse
    # remain byte-identical. Comments cannot supply this relation.
    prefix = items.get("npos_effects_prefix", "")
    if any(token in prefix for token in ("penalty_actions", "finalized_global_beacon_pulse")):
        errors.append("Admission capacity evidence selection edits mandatory effects")
    merge = items.get("candidate_attachments", "")
    for token in (
        "super::v2_candidate::candidate_economic_work_first(context.height)",
        "&& effects.penalty_actions.is_empty() && queue_plan_admissions.is_empty()",
        ".filter(|(_, entry, _)| entry.execution_batch.is_some())",
        "if preferred_merge_entry.is_some() { effects.v2_evidence_admissions.clear(); }",
        "let selected_merge_entry = if preferred_merge_entry.is_some() { preferred_merge_entry }",
        "!effects.v2_evidence_admissions.is_empty() || !effects.penalty_actions.is_empty()",
    ):
        if _code(token) not in merge:
            errors.append(f"Admission capacity merge opportunity lost {token!r}")

    def ordered(symbol, *relations):
        item = items.get(symbol, "")
        cursor = 0
        for relation in relations:
            found = item.find(_code(relation), cursor)
            if found < 0:
                errors.append(f"Admission capacity {symbol} lost ordering relation {relation!r}")
                break
            cursor = found + len(_code(relation))

    ordered("V2CandidateAssembler::assemble", "let original_npos_effects =", "fit_evidence_prefix(",
            "let mut builder = self.prepare_block_builder(", "let (fitted, count) = fit_evidence_prefix(",
            "report.evidence_deferred =", "CandidateAssemblyOutcome::WorkDeferred", "begin_fail_stop_operation()")
    ordered("candidate_attachments", "let preferred_merge_entry =", "effects.v2_evidence_admissions.clear()",
            "let npos_consensus_effects =", "let merge_selection =", "let selected_merge_entry =")
    ordered("publish_authenticated_capacity", "capacity.check_payload_size(",
            "require_local_payload_capacity(capacity.layout, config)?", "slot.set(capacity)")
    ordered("run_inner", "terminal.verified_context(), &shared_config,)",
            ".map_err(V2RunnerError::Service)?", "startup_recovery.ready()",
            "&verified_context, &shared_config,)", ".map_err(V2RunnerError::Service)?",
            "claim_runner_lifecycle_process_generation(")
    ordered("run_pending_kura_lifecycle_height", "require_local_payload_capacity(",
            ".map_err(V2RunnerError::Service)?", "beacon_readiness.begin_height(")
    ordered("candidate_limits", "require_local_payload_capacity(", "CandidateLimits::new(")
    if ".min(" in items.get("candidate_limits", ""):
        errors.append("Admission capacity candidate silently narrows the signed envelope")
    ordered("SumeragiHandle::check_queue_plan_input_capacity", "self.authenticated_admission_capacity()?",
            "if !self.admission_ready() { return Err(QueuePlanInputCapacityErrorV1::Inactive); }",
            "validate_queue_plan_binding_for_request(", "require_capacity(\"complete input\",",
            "maximum_lane_admitted_input_envelope_sizes_v1(", "capacity.check_payload_size(",
            "require_capacity(envelope, required, capacity)?")
    ordered("execute_torii_proxy_request_with_fallback_admitted",
            "let proxy_memory = match pre_admitted_proxy_memory",
            "acquire_torii_proxy_memory(app)",
            'queue_plan_request_service_capacity_error(\n        app,\n        &request.request,\n        tokio::time::Instant::from_std(request_started) + TORII_PROXY_EXECUTION_BUDGET,\n        request.deadline_unix_ms,\n    )\n    .await',
            "take_local_torii_proxy_fast_path(")
    outer_capacity_call = 'queue_plan_request_service_capacity_error(\n        app,\n        &request.request,\n        tokio::time::Instant::from_std(request_started) + TORII_PROXY_EXECUTION_BUDGET,\n        request.deadline_unix_ms,\n    )\n    .await'
    forwarded_capacity_call = 'queue_plan_request_service_capacity_error(\n        app,\n        &forwarded_request.request,\n        request_started + TORII_PROXY_EXECUTION_BUDGET,\n        forwarded_request.deadline_unix_ms,\n    )\n    .await'
    for symbol, request in (("execute_torii_proxy_request_with_fallback_admitted", "request"),
                            ("forward_incoming_torii_proxy_request", "forwarded_request")):
        ordered(symbol, outer_capacity_call if request == "request" else forwarded_capacity_call,
                "execute_torii_proxy_request_across_candidates(")
    ordered("execute_incoming_torii_proxy_request_with_admission_inner",
            'queue_plan_service_input_capacity_error(\n                app,\n                &transaction,\n                &admission_binding,\n                execution_deadline,\n                request_head.deadline_unix_ms,\n            )\n            .await',
            "push_accepted_transaction_for_ingress_with_routing_plan_strict_durable_claim(",
            "queue_plan_synced_admission_response(")
    ordered("execute_torii_proxy_request_across_candidates",
            "let budget_observed_at = tokio::time::Instant::now()",
            "queue_plan_capacity_wait::remaining(",
            "queue_plan_capacity_wait::deadline_response(&request.request, error)",
            "let execution_deadline = (budget_observed_at + execution_budget)",
            ".min(execution_started + TORII_PROXY_EXECUTION_BUDGET)")
    ordered("execute_incoming_torii_proxy_request_with_admission_inner",
            "canonical_queue_plan_synced_response(",
            "queue_plan_service_input_capacity_error(",
            "canonical_queue_plan_synced_response(",
            "let accepted_tx = match routing::accept_transaction_for_ingress(")
    ordered(PERSIST, "QueuePlanAdmissionCertificateStrengthV1::Quorum",
            "if let Err(error) = queue_plan_capacity_wait::wait( || queue_plan_service_input_capacity(app, expected_entrypoint, expected_binding), || deadline.remaining(), ).await",
            "return queue_plan_outcome_unknown_response(", "norito::encode_canonical(&input)",
            "deadline.persist(&app.state, &input_bytes).await", "disseminate_queue_plan_admission_publication(")
