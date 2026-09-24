"""Actual parsed-owner and semantic mutation checks for pre-receipt capacity."""
from __future__ import annotations

import ast
import copy
import importlib.util
import sys
from pathlib import Path

import pytest


def support():
    path = Path(__file__).with_name("sumeragi_v2_multilane_models_test.py")
    spec = importlib.util.spec_from_file_location("admission_capacity_support", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def captured(tmp_path_factory):
    helper = support()
    checker = helper.load_checker()
    c = checker.admission_capacity_contract
    root = tmp_path_factory.mktemp("admission-capacity")
    helper.copy_reviewed_source_fixture_with_includes(root, checker, {
        *(p for p in c.SOURCE_RELATIVES if p.suffix == ".rs"),
    })
    items, errors = {}, []
    keys = {(p, k, s) for p, k, s, _ in c.BINDINGS} | set(c.EXTRA_ITEMS)
    with checker._reviewed_rust_source_cache():
        for key in sorted(keys):
            items[key] = checker._rust_binding_item(root, *key, "capacity fixture", errors)
    assert errors == []
    result = root, checker, helper.canonical_models(), items
    assert validate(result) == []
    return result


def validate(captured, *, altered=None, models=None):
    root, checker, original_models, original = captured
    items = original if altered is None else altered
    errors = []
    checker.admission_capacity_contract.validate_owners(
        root, original_models if models is None else models, errors,
        lambda _root, path, kind, symbol, _label, _errors: items[(path, kind, symbol)])
    return errors


def test_admission_capacity_accepts_actual_sources(captured):
    assert validate(captured) == []


@pytest.fixture(scope="module")
def proxy_deadline(captured):
    helper = support()
    helper.load_checker()
    import sumeragi_v2_multilane_queue_plan_contract as contract

    return captured, contract, helper.canonical_queue_plan_retry_items()


def test_proxy_deadline_retry_and_capacity_share_the_complete_reviewed_owner(proxy_deadline):
    """Neither gate may overwrite the other's tokens for this one physical owner."""
    captured, contract, retry_items = proxy_deadline
    _, checker, models, _ = captured
    binding = contract.QUEUE_PLAN_PROXY_DEADLINE_BINDING
    assert binding in contract.QUEUE_PLAN_CANONICAL_RETRY_BINDINGS
    assert binding in checker.admission_capacity_contract.BINDINGS
    model, = [m for m in models if m["module"] == checker.admission_capacity_contract.MODEL]
    owner, = [r for r in model["production_symbols"]
              if (r["path"], r["kind"], r["symbol"]) == binding[:3]]
    assert tuple(owner["required_tokens"]) == binding[3]
    assert validate(captured) == []
    errors = []
    contract.validate_canonical_queue_plan_retry(retry_items, errors)
    assert errors == [], errors


@pytest.mark.parametrize("old,new", [
    ("let deadline = budget_observed_at + remaining_budget;",
     "let deadline = tokio::time::Instant::now() + remaining_budget;"),
    (".checked_sub(TORII_PROXY_RESPONSE_EGRESS_RESERVE)", ".checked_sub(Duration::ZERO)"),
    (".filter(|budget| !budget.is_zero())", ".filter(|_| true)"),
    ("absolute_budget.min(TORII_PROXY_EXECUTION_BUDGET)", "absolute_budget.max(TORII_PROXY_EXECUTION_BUDGET)"),
    ("queue_plan_capacity_wait::deadline_response(&proxy_request.request, error)", "generic_deadline_response(error)"),
    ("match tokio::time::timeout_at(\n        deadline,",
     "match tokio::time::timeout_at(\n        tokio::time::Instant::now(),"),
    ("            proxy_memory,\n            deadline,",
     "            proxy_memory,\n            tokio::time::Instant::now(),"),
    ("    let deadline = budget_observed_at + remaining_budget;",
     "    let budget_observed_at = tokio::time::Instant::now();\n"
     "    let deadline = budget_observed_at + remaining_budget;"),
    ("    let remaining_budget = absolute_budget.min(TORII_PROXY_EXECUTION_BUDGET);",
     "    let absolute_budget = TORII_PROXY_EXECUTION_BUDGET;\n"
     "    let remaining_budget = absolute_budget.min(TORII_PROXY_EXECUTION_BUDGET);"),
    ("    let deadline = budget_observed_at + remaining_budget;",
     "    let remaining_budget = TORII_PROXY_EXECUTION_BUDGET;\n"
     "    let deadline = budget_observed_at + remaining_budget;"),
    ("    match tokio::time::timeout_at(",
     "    let deadline = tokio::time::Instant::now() + remaining_budget;\n"
     "    match tokio::time::timeout_at("),
], ids=[
    "no-clock-rebase", "reserve-egress", "reject-exhausted-budget", "cap-execution",
    "retain-queue-plan-ambiguity", "outer-original-deadline", "inner-original-deadline",
    "no-clock-shadow", "no-absolute-budget-shadow", "no-budget-shadow", "no-deadline-shadow",
])
def test_proxy_deadline_both_gates_reject_timeout_mutation(proxy_deadline, old, new):
    """Independent consumers enforce the same original absolute-deadline cut."""
    captured, contract, retry_items = proxy_deadline
    key = contract.QUEUE_PLAN_PROXY_DEADLINE_BINDING[:3]
    source = retry_items[key]
    assert source.count(old) == 1
    mutated = source.replace(old, new, 1)
    altered_retry = retry_items.copy()
    altered_retry[key] = mutated
    retry_errors = []
    contract.validate_canonical_queue_plan_retry(altered_retry, retry_errors)
    altered_capacity = captured[3].copy()
    altered_capacity[key] = mutated
    capacity_errors = validate(captured, altered=altered_capacity)
    assert any(key[2] in error for error in retry_errors), retry_errors
    assert any(key[2] in error for error in capacity_errors), capacity_errors


@pytest.mark.parametrize("statement,anchor", [
    ("    let budget_observed_at = tokio::time::Instant::now();\n", "    let remaining_budget ="),
    ("    let budget_observed_at = tokio::time::Instant::now();\n", "    let deadline ="),
    ("    let request_id = proxy_request.request_id.clone();\n", "    let budget_observed_at ="),
], ids=["observe-before-validation", "observe-before-hashing", "observe-before-setup"])
def test_proxy_deadline_both_gates_reject_late_original_observation(proxy_deadline, statement, anchor):
    """All literal tokens remain; their ordering must still bind the original cut."""
    captured, contract, retry_items = proxy_deadline
    key = contract.QUEUE_PLAN_PROXY_DEADLINE_BINDING[:3]
    source = retry_items[key]
    assert source.count(statement) == source.count(anchor) == 1
    mutated = source.replace(statement, "", 1).replace(anchor, statement + anchor, 1)
    altered_retry = retry_items.copy()
    altered_retry[key] = mutated
    retry_errors = []
    contract.validate_canonical_queue_plan_retry(altered_retry, retry_errors)
    altered_capacity = captured[3].copy()
    altered_capacity[key] = mutated
    capacity_errors = validate(captured, altered=altered_capacity)
    assert any(key[2] in error and "order" in error for error in retry_errors), retry_errors
    assert any(key[2] in error and "order" in error for error in capacity_errors), capacity_errors


def test_proxy_deadline_both_gates_allow_independent_setup_order(proxy_deadline):
    """Request-id copying and QueuePlan hashing need only follow the same clock."""
    captured, contract, retry_items = proxy_deadline
    key = contract.QUEUE_PLAN_PROXY_DEADLINE_BINDING[:3]
    source = retry_items[key]
    statement = "    let request_id = proxy_request.request_id.clone();\n"
    anchor = "    let deadline ="
    assert source.count(statement) == source.count(anchor) == 1
    mutated = source.replace(statement, "", 1).replace(anchor, statement + anchor, 1)
    altered_retry = retry_items.copy()
    altered_retry[key] = mutated
    retry_errors = []
    contract.validate_canonical_queue_plan_retry(altered_retry, retry_errors)
    altered_capacity = captured[3].copy()
    altered_capacity[key] = mutated
    assert retry_errors == [], retry_errors
    assert validate(captured, altered=altered_capacity) == []


@pytest.mark.parametrize("name,rust_type,value", [
    ("budget_observed_at", "tokio::time::Instant", "tokio::time::Instant::now()"),
    ("absolute_budget", "Duration", "TORII_PROXY_EXECUTION_BUDGET"),
    ("remaining_budget", "Duration", "TORII_PROXY_EXECUTION_BUDGET"),
    ("deadline", "tokio::time::Instant", "tokio::time::Instant::now() + remaining_budget"),
])
@pytest.mark.parametrize("declaration", ["let {name}: {rust_type}", "let mut {name}", "let mut {name}: {rust_type}"])
def test_proxy_deadline_both_gates_reject_typed_or_mutable_shadow(
    proxy_deadline, name, rust_type, value, declaration
):
    """Valid Rust shadow declarations cannot retain tokens while replacing authority."""
    captured, contract, retry_items = proxy_deadline
    key = contract.QUEUE_PLAN_PROXY_DEADLINE_BINDING[:3]
    source = retry_items[key]
    anchors = {
        "budget_observed_at": "    let deadline =",
        "absolute_budget": "    let remaining_budget =",
        "remaining_budget": "    let deadline =",
        "deadline": "    match tokio::time::timeout_at(",
    }
    anchor = anchors[name]
    assert source.count(anchor) == 1
    shadow = declaration.format(name=name, rust_type=rust_type)
    mutated = source.replace(anchor, f"    {shadow} = {value};\n" + anchor, 1)
    altered_retry = retry_items.copy()
    altered_retry[key] = mutated
    retry_errors = []
    contract.validate_canonical_queue_plan_retry(altered_retry, retry_errors)
    altered_capacity = captured[3].copy()
    altered_capacity[key] = mutated
    capacity_errors = validate(captured, altered=altered_capacity)
    expected = f"canonical QueuePlan deadline owner is rebound: {name}"
    assert any(expected in error for error in retry_errors), retry_errors
    assert any(expected in error for error in capacity_errors), capacity_errors


def test_admission_capacity_gate_and_source_closure_are_connected():
    checker = support().load_checker()
    tree = ast.parse(Path(checker.__file__).read_text())
    defs = {n.name: n for n in tree.body if isinstance(n, ast.FunctionDef)}
    assert sum(isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
               and isinstance(n.func.value, ast.Name)
               and n.func.value.id == "admission_capacity_contract"
               and n.func.attr == "validate_owners"
               for n in ast.walk(defs["_validate"])) == 1
    assert any(isinstance(n, ast.Attribute) and n.attr == "SOURCE_RELATIVES"
               and isinstance(n.value, ast.Name) and n.value.id == "admission_capacity_contract"
               for n in ast.walk(defs["source_manifest_sha256"]))


@pytest.mark.parametrize("mutation", ["missing", "duplicate", "weakened"])
def test_admission_capacity_requires_exact_owner_ledger(captured, mutation):
    _, checker, models, _ = captured
    c = checker.admission_capacity_contract
    for path, kind, symbol, _ in c.BINDINGS:
        changed = copy.deepcopy(models)
        owner = next(m for m in changed if m["module"] == c.MODEL)
        rows = owner["production_symbols"]
        index = next(i for i, r in enumerate(rows)
                     if (r["path"], r["kind"], r["symbol"]) == (path, kind, symbol))
        if mutation == "missing":
            rows.pop(index)
        elif mutation == "duplicate":
            rows.append(dict(rows[index]))
        else:
            rows[index]["required_tokens"] = []
        assert any("ledger owner differs" in e for e in validate(captured, models=changed)), symbol


@pytest.mark.parametrize("symbol,old,new", [
    ("candidate_economic_work_first", "height % 2 == 0", "true"),
    ("npos_effects_prefix", "effects.v2_evidence_admissions.truncate(count)", "effects.v2_evidence_admissions.clear()"),
    ("npos_effects_prefix", "effects.v2_evidence_admissions.truncate(count)", "effects.v2_evidence_admissions.truncate(count); effects.penalty_actions.clear()"),
    ("npos_effects_prefix", "effects.v2_evidence_admissions.truncate(count)", "effects.v2_evidence_admissions.truncate(count); effects.finalized_global_beacon_pulse = None"),
    ("fit_evidence_prefix", "bytes <= payload_limit", "true"),
    ("fit_evidence_prefix", "chunks <= layout.max_chunk_count as usize", "true"),
    ("fit_evidence_prefix", "canonical_proposal_wire_len(signatory, algorithm)", "estimated_input_len(signatory, algorithm)"),
    ("V2CandidateAssembler::assemble_at_generation", "candidate_economic_work_first(request.context.height)", "candidate_economic_work_first(view)"),
    ("V2CandidateAssembler::assemble_at_generation", "autonomous_lane_payloads: prepared_work.autonomous_lane_payloads.clone()", "autonomous_lane_payloads: Vec::new()"),
    ("V2CandidateAssembler::assemble_at_generation", "report.evidence_deferred = evidence_count - count", "report.evidence_deferred = 0"),
    ("V2CandidateAssembler::assemble_at_generation", "if first_admission_size.is_none() && evidence_count > 0", "if false"),
    ("NativeRunnerProcess::assemble_candidate", "if completed.owner == owner", "if true"),
    ("NativeRunnerProcess::assemble_candidate", "if self.candidate_job.is_some()", "if false"),
    ("NativeRunnerProcess::assemble_candidate", "self.retain_candidate_source(assembly.source);", ""),
    ("NativeRunnerProcess::assemble_candidate", "work_provider: &decisions", "work_provider: &unrelated_decisions"),
    ("NativeRunnerProcess::assemble_candidate", "output_guard: &guard", "output_guard: &foreign_guard"),
    ("V2CandidateAssembler::assemble_native", "!request.work_provider.belongs_to(request.state)", "false"),
    ("V2CandidateAssembler::assemble_native", "request.attachments.certified_merge_entry.is_some()", "false"),
    ("V2CandidateAssembler::assemble_native", "if !validate_request_at_generation(&request, state_generation)?", "if false"),
    ("V2CandidateAssembler::assemble_native", "Err(_) if !candidate_state_generation_is_current(request.state, state_generation)", "Err(_) if false"),
    ("V2CandidateAssembler::assemble_native", "work_provider: NativeCandidateWork(&source)", "work_provider: OrdinaryWork::default()"),
    ("NativeCandidateWork::prepare", "if !candidates.is_empty()", "if false"),
    ("schedule_local_proposal", "native.retain_candidate_source(source);", ""),
    ("candidate_attachments", "let npos_consensus_effects =", "effects.v2_evidence_admissions.clear(); let npos_consensus_effects ="),
    ("publish_authenticated_capacity", "require_local_payload_capacity(capacity.layout, config)?;", ""),
    ("publish_authenticated_capacity", "layout: context.da_layout", "layout: default_layout()"),
    ("require_local_payload_capacity", "if available < required", "if available > required"),
    ("require_local_payload_capacity", "config.limits.ready_body_bytes", "u64::MAX"),
    ("require_local_payload_capacity", "config.limits.body_source_bytes", "u64::MAX"),
    ("AuthenticatedAdmissionCapacityV1::check_payload_size", "chunk_count > self.layout.max_chunk_count", "false"),
    ("SumeragiHandle::authenticated_admission_capacity", "self.output_guard.restart_required()", "false"),
    ("run_inner", "terminal.verified_context(), &shared_config,)", "terminal.verified_context(), &default_config,)"),
    ("run_inner", "&admission_capacity, &verified_context, &shared_config,)", "&admission_capacity, &verified_context, &default_config,)"),
    ("candidate_limits", "NonZeroUsize::new(context_payload)", "NonZeroUsize::new(context_payload.min(config.limits.max_payload_bytes as usize))"),
    ("run_pending_kura_lifecycle_height", "require_local_payload_capacity(context.da_layout, &shared_config,)", "unchecked_local_capacity(context.da_layout, &shared_config,)"),
    ("SumeragiHandle::check_queue_plan_input_capacity", "if !self.admission_ready() { return Err(QueuePlanInputCapacityErrorV1::Inactive); }", ""),
    ("SumeragiHandle::check_queue_plan_input_capacity", "capacity.network_id() != *network_id", "false"),
    ("SumeragiHandle::check_queue_plan_input_capacity", "validate_queue_plan_binding_for_request(binding, network_id, entrypoint, &plan)", "unchecked_binding(binding, network_id, entrypoint, &plan)"),
    ("SumeragiHandle::check_queue_plan_input_capacity", "sizes.native_payload_bytes", "sizes.complete_input_bytes"),
    ("SumeragiHandle::check_queue_plan_input_capacity", "self.block.control_frame_byte_capacity", "usize::MAX"),
    ("SumeragiHandle::check_queue_plan_input_capacity", "self.block.consensus_frame_byte_capacity", "self.block.block_sync_frame_byte_capacity"),
    ("SumeragiHandle::check_queue_plan_input_capacity", "sizes.publication_queue_bytes", "sizes.publication_plaintext_bytes"),
    ("SumeragiHandle::check_queue_plan_input_capacity", "sizes.republication_queue_bytes", "sizes.republication_plaintext_bytes"),
    ("require_capacity", "required > capacity", "required < capacity"),
    ("maximum_lane_admitted_input_sizing_value_v1", "right.0.cmp(&left.0)", "left.0.cmp(&right.0)"),
    ("maximum_lane_admitted_input_sizing_value_v1", "shapes.truncate(threshold)", "shapes.truncate(1)"),
    ("maximum_lane_admitted_input_envelope_sizes_v1", "norito::canonical_frame_len(&native_sizing_only)", "norito::canonical_frame_len(&control_bytes)"),
    ("maximum_lane_admitted_input_envelope_sizes_v1", "previous != bound.lane_incarnation", "false"),
    ("queue_plan_direct_frame_sizes_v1", "iroha_p2p::frame_queue_charge(plaintext)", "Some(plaintext)"),
    ("queue_plan_service_input_capacity", "app.sumeragi.as_ref().ok_or(", "foreign_handle.as_ref().ok_or("),
    ("queue_plan_service_input_capacity_error", "QueuePlanInputCapacityErrorV1::Inactive", "QueuePlanInputCapacityErrorV1::Invalid(_)"),
    ("wait", "Err(QueuePlanInputCapacityErrorV1::Inactive) => {}", "Err(_) => {}"),
    ("wait", "let budget = remaining().map_err(WaitError::Deadline)?;", "let budget = Duration::from_secs(64);"),
    ("remaining", "Ok(absolute.min(local))", "Ok(absolute)"),
    ("deadline_response", "transaction.hash()", "unverified_binding.entrypoint_hash"),
    ("execute_incoming_torii_proxy_request_with_admission", "queue_plan_capacity_wait::deadline_response(&proxy_request.request, error)", "generic_deadline_response(error)"),
    ("execute_torii_proxy_request_across_candidates", "queue_plan_capacity_wait::deadline_response(&request.request, error)", "generic_deadline_response(error)"),
    ("execute_torii_proxy_request_across_candidates", "return queue_plan_outcome_unknown_response( expected.entrypoint_hash, expected.signed_transaction_hash, error, );", "return generic_deadline_response(error);"),
    ("execute_torii_proxy_request_with_fallback_admitted", 'queue_plan_request_service_capacity_error(\n        app,\n        &request.request,\n        tokio::time::Instant::from_std(request_started) + TORII_PROXY_EXECUTION_BUDGET,\n        request.deadline_unix_ms,\n    )\n    .await', "None"),
    ("forward_incoming_torii_proxy_request", 'queue_plan_request_service_capacity_error(\n        app,\n        &forwarded_request.request,\n        request_started + TORII_PROXY_EXECUTION_BUDGET,\n        forwarded_request.deadline_unix_ms,\n    )\n    .await', "None"),
    ("execute_incoming_torii_proxy_request_with_admission_inner", 'queue_plan_service_input_capacity_error(\n                app,\n                &transaction,\n                &admission_binding,\n                execution_deadline,\n                request_head.deadline_unix_ms,\n            )\n            .await', "queue_plan_complete_input_capacity_error(&transaction, binding)"),
    ("persist_queue_plan_admission_certificate", "queue_plan_service_input_capacity(app, expected_entrypoint, expected_binding)", "Ok::<(), String>(())"),
])
def test_admission_capacity_rejects_semantic_mutation(captured, symbol, old, new):
    _, checker, _, items = captured
    c = checker.admission_capacity_contract
    key = next(key for key in items if key[2] == symbol)
    # Mutate the executable projection of the actual parsed owner. This does not
    # write shared sources, bypass source parsing, or let comments supply guards.
    source = c._code(items[key])
    before, after = c._code(old), c._code(new)
    assert source.count(before) == 1, (symbol, before, source.count(before))
    changed = dict(items)
    changed[key] = source.replace(before, after, 1)
    assert validate(captured, altered=changed), symbol


def test_admission_capacity_rejects_publication_before_local_capacity(captured):
    _, checker, _, items = captured
    c = checker.admission_capacity_contract
    key = next(k for k in items if k[2] == "publish_authenticated_capacity")
    source = c._code(items[key])
    check = c._code("require_local_payload_capacity(capacity.layout, config)?;")
    source = source.replace(check, "", 1)
    source = source.replace("slot.set(capacity)", "slot.set(capacity);" + check, 1)
    changed = dict(items)
    changed[key] = source
    assert any("ordering" in e for e in validate(captured, altered=changed))


def test_candidate_rejects_state_block_probe_under_publication_lease(captured):
    _, checker, _, items = captured
    c = checker.admission_capacity_contract
    key = next(k for k in items if k[2] == "V2CandidateAssembler::assemble_at_generation")
    source = c._code(items[key])
    lease = c._code("let _state_publication = request.state.consensus_publication_lease()")
    assert source.count(lease) == 1
    changed = dict(items)
    changed[key] = source.replace(
        lease,
        lease + c._code("request.state.deterministic_start_work_pending(&candidate_header)?;"),
        1,
    )
    assert any("reopened a State block" in e for e in validate(captured, altered=changed))


def test_admission_capacity_rejects_sizing_before_memory_admission(captured):
    _, checker, _, items = captured
    c = checker.admission_capacity_contract
    key = next(k for k in items if k[2] == "execute_torii_proxy_request_with_fallback_admitted")
    source = c._code(items[key])
    check = c._code('if let Some(response) = queue_plan_request_service_capacity_error(\n        app,\n        &request.request,\n        tokio::time::Instant::from_std(request_started) + TORII_PROXY_EXECUTION_BUDGET,\n        request.deadline_unix_ms,\n    )\n    .await { return response; }')
    assert source.count(check) == 1
    source = source.replace(check, "", 1)
    source = source.replace("letproxy_memory=", check + "letproxy_memory=", 1)
    changed = dict(items)
    changed[key] = source
    assert any("ordering" in e for e in validate(captured, altered=changed))


@pytest.mark.parametrize("extra", [
    "fn unguarded() { execute_torii_proxy_request_across_candidates(); }",
    "fn escaped() { let dispatch = execute_torii_proxy_request_across_candidates; }",
])
def test_admission_capacity_rejects_unowned_aggregator_caller(captured, extra):
    root, checker, _, items = captured
    c = checker.admission_capacity_contract
    callers = {key[2]: value for key, value in items.items()}
    errors = []
    c.validate_aggregator_callers((root / c.TORII).read_text() + extra, callers, errors)
    assert any("caller inventory" in e for e in errors)


def test_admission_capacity_exact_raw_tokens_match_parsed_owners(captured):
    _, checker, _, items = captured
    for path, kind, symbol, tokens in checker.admission_capacity_contract.BINDINGS:
        for token in tokens:
            assert token in items[(path, kind, symbol)], (symbol, token)
