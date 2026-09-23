"""Actual ordinary-runner source contracts preserve Native and finalization custody.

Compile the existing checker consumers against the reviewed Rust source loader;
mutations remove authority, reorder settlement, or reopen a forbidden branch.
"""
from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]


def load_checker():
    path = ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    spec = importlib.util.spec_from_file_location("native_runner_owner_checker", path)
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def source_fixture(checker):
    path = Path(checker._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors.__code__.co_filename)
    owner, = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.FunctionDef) and node.name == "_lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors"]
    source_loop, = [node for node in owner.body if isinstance(node, ast.For) and isinstance(node.target, ast.Tuple) and isinstance(node.target.elts[0], ast.Name) and node.target.elts[0].id == "name"]
    paths, sources, errors = {}, {}, []
    for name, relative in ast.literal_eval(source_loop.iter):
        paths[name], sources[name] = checker._read_reviewed_rust_source(ROOT, relative, errors, "ordinary Native runner owner")
    assert not errors, errors
    return path, owner, paths, sources


def build_probe(checker, scope):
    path, owner, paths, sources = source_fixture(checker)
    helpers = [node for node in owner.body if isinstance(node, ast.FunctionDef) and node.name in {"item", "qualified_item", "require_order", "reject_tokens", "require_tokens"}]
    assert len(helpers) == 5
    if scope in ("ingress", "barrier"):
        helper = "_ordinary_native_ingress_owner_source_fidelity_errors" if scope == "ingress" else "_ordinary_native_runner_barrier_source_fidelity_errors"
        statements = ast.parse(f"{helper}(paths,sources,errors,item,qualified_item,require_order,reject_tokens)").body
    else:
        names = {
            "ordinary_consumer", "apply_ingress_barrier", "lifecycle_height_driver",
            "lifecycle_live_loop", "apply_barrier_settlement", "lifecycle_active", "lifecycle_finalization",
        }
        labels = {
            "single exact ordinary post-dequeue runner tail",
            "typed Apply ingress barrier",
            "activated lifecycle ordinary Completion/Runtime/Ingress batch",
            "Apply-only completion barrier runner Decision handoff settlement",
            "post-settlement ordinary-runtime cut",
            "each contiguous ordinary reconciliation point must settle the original local proposal before acknowledging exact runner Decision cleanup",
            "ordinary finalization must close ingress and finitely drain terminal recovery before consuming finalized rollover",
            "post-slice Decision cleanup retires discovered block sync before exact local proposal acknowledgement",
            "lifecycle finalization output/store/cleanup transaction",
            "coordinator ProducerTurn claim, attempt, and durable settlement",
        }
        statements = []
        seen = set()
        for node in owner.body:
            if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name) and target.id in names for target in node.targets):
                statements.append(node)
            if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call):
                selected = {arg.value for arg in node.value.args if isinstance(arg, ast.Constant) and isinstance(arg.value, str)} & labels
                if selected:
                    statements.append(node)
                    seen |= selected
        assert seen == labels, labels - seen
    function = ast.parse("def probe(paths,sources):\n    errors=[]\n    return errors\n").body[0]
    function.body[-1:-1] = helpers + statements
    ns = dict(checker.__dict__)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[function], type_ignores=[])), str(path), "exec"), ns)
    return ns["probe"], paths, sources


def finalization_probe(checker, name):
    outer = checker._successor_recovery_source_fidelity_errors
    outer_path = Path(outer.__code__.co_filename)
    definition, = [node for node in ast.parse(outer_path.read_text()).body if isinstance(node, ast.FunctionDef) and node.name == outer.__name__]
    helpers = [node for node in definition.body if isinstance(node, ast.FunctionDef) and node.name in {"require_order", "require_tokens"}]
    if name == "production":
        function = checker._successor_production_recovery_finalization_tail
        path = Path(function.__code__.co_filename)
        owner, = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.FunctionDef) and node.name == function.__name__]
    else:
        path, owner = outer_path, definition
    statements = [node for node in ast.walk(owner) if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call) and any(isinstance(arg, ast.Constant) and arg.value in ("runner lifecycle finalization preflight", "sealed runner finalized-output reuse") for arg in node.value.args)]
    assert len(statements) == 2
    function = ast.parse("def probe(lifecycle_run_inner_path,lifecycle_run_inner_source,finalized_output_path,finalized_output_source):\n    errors=[]\n    return errors\n").body[0]
    function.body[-1:-1] = helpers + statements
    ns = dict(checker.__dict__)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[function], type_ignores=[])), str(path), "exec"), ns)
    return ns["probe"]


@pytest.fixture(scope="module")
def checker():
    return load_checker()


@pytest.fixture(scope="module")
def scopes(checker):
    return {scope: build_probe(checker, scope) for scope in ("ingress", "barrier", "ordinary")}


@pytest.mark.parametrize("scope", ("ingress", "barrier", "ordinary"))
def test_current_native_runner_contracts(scopes, scope):
    probe, paths, sources = scopes[scope]
    assert probe(paths, sources) == []


MUTATIONS = (
    ("ingress", "ordinary_consumer", "consume_prepared_native_ingress", "!prepared.matches_ingress(receiver)", "false"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_ingress", "!native_lanes.matches_output_guard(&prepared.output_guard)", "false"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_ingress", "prepared.inbound = Some(inbound);", "drop(inbound);"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_ingress", "return Ok(Some(prepared));", "return Ok(None);"),
    ("ingress", "native_process", "consume_native_ingress", "if self.pending_ingress.is_some()", "if false"),
    ("ingress", "native_process", "consume_native_ingress", "self.pending_ingress =", "let _ ="),
    ("ingress", "native_process", "consume_native_ingress", "&mut self.driver,", "&mut replacement,"),
    ("ingress", "native_process", "poll", "self.consume_native_ingress(prepared, receiver)?;", "drop(prepared);"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_source_response", "!native.admits_source_response(inbound.message())", "false"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_source_response", "!ownership.matches_semantic_origin(inbound.sender())", "false"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_source_response", "!ownership.matches_reply_routes(inbound.reply_routes())", "false"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_source_response", "native.accept_source_response(response, &sender)?;", "native.accept_source_response(response, &foreign)?;"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_source_response", "mark_leader_wire_volatile(receiver, &ownership)?;", ""),
    ("barrier", "lifecycle_run_inner", "run_lifecycle_active_height", "native.take_service_publication(services);", ""),
    ("barrier", "lifecycle_run_inner", "run_lifecycle_active_height", "native.service_sources(services, now)?;", "native.service_sources(foreign, now)?;"),
    ("barrier", "lifecycle_run_inner", "run_lifecycle_active_height", "activated.prepare_native_source_pacemaker_ingress_turn(permit)?", "activated.prepare_native_source_pacemaker_ingress_turn(foreign)?"),
    ("barrier", "lifecycle_run_inner", "run_lifecycle_active_height", "if let Some(_permit) = producer_claim.native_source_pacemaker_escape_permit()", "if true"),
    ("barrier", "height_driver", "native_source_pacemaker_escape_permit", "if matches!(self, Self::AwaitingNativeSource)", "if true"),
    ("barrier", "height_driver", "decided_native_source_recovery_permit", "&& decided_subject_present", "|| decided_subject_present"),
    ("barrier", "lifecycle_run_inner", "run_lifecycle_active_height", "if producer_claim.permits_open_decided_lane_recovery_ingress()", "if true"),
    ("ordinary", "ordinary_consumer", "consume_prepared_dequeued_v2_ingress", "native.consume_native_ingress(prepared, receiver)?;", "drop(prepared);"),
    ("ordinary", "height_driver", "blocks_ingress", "| Self::AwaitingNativeSource", ""),
    ("ordinary", "height_driver", "drain_lifecycle_v2_ingress", "activated.drive_ingress_turn(current_turn, native.has_pending_ingress())", "activated.drive_ingress_turn(current_turn, false)"),
    ("ordinary", "lifecycle_run_inner", "settle_apply_barrier_runner_decision_handoff", "directive.tag(), Some(decided_subject)", "directive.tag(), None"),
    ("ordinary", "lifecycle_run_inner", "run_lifecycle_active_height", "terminal_finalization_fenced || producer_claim.apply_terminal_settled();", "false;"),
    ("ordinary", "lifecycle_run_inner", "run_lifecycle_active_height", "queue_plan.refresh(active_view)?;", "queue_plan.refresh(foreign)?;"),
    ("ordinary", "lifecycle_run_inner", "run_lifecycle_active_height", "activated.close_runner_ingress_for_finalized_drain(&mut active_runner, receiver)?;", ""),
    ("ordinary", "lifecycle_run_inner", "run_lifecycle_active_height", "DecidedLaneRecoveryIngressDrainMode::FinalizedClosedPrefix", "DecidedLaneRecoveryIngressDrainMode::OpenPreflight"),
    ("ordinary", "lifecycle_run_inner", "run_lifecycle_active_height", ".ensure_closed_global_drained_cut()", ".ensure_open()"),
    ("ordinary", "lifecycle_run_inner", "run_lifecycle_active_height", ".ensure_closed_global_drained_cut()", ".ensure_closed_drained_cut()"),
    ("ordinary", "lifecycle_run_inner", "finalize_lifecycle_height", "prepare_successor(receipt, artifact)?", "prepare_successor(foreign, artifact)?"),
    ("ordinary", "lifecycle_run_inner", "finalize_lifecycle_height", "rollover_outputs(active_runner, native, &next_context, control_queue_capacity)", "rollover_outputs(active_runner, replacement, &next_context, control_queue_capacity)"),
    ("ordinary", "lifecycle_run_inner", "finalize_lifecycle_height", "post_output.retire_lifecycle_stores()?;", "post_output.skip_retirement()?;"),
)


@pytest.mark.parametrize(("scope", "source_name", "name", "old", "new"), MUTATIONS, ids=[f"{row[2]}-{i}" for i, row in enumerate(MUTATIONS)])
def test_native_runner_contract_rejects_owner_loss(checker, scopes, scope, source_name, name, old, new):
    probe, paths, sources = scopes[scope]
    source = sources[source_name]
    items = checker.rust_items(source, name)
    assert len(items) == 1, (name, len(items))
    item = items[0]
    assert item.source.count(old) == 1, (name, old, item.source.count(old))
    changed = source.replace(item.source, item.source.replace(old, new, 1), 1)
    assert probe(paths, {**sources, source_name: changed}), (name, old, new)


@pytest.mark.parametrize("consumer", ("production", "recovery"))
def test_current_native_finalization_consumers(checker, scopes, consumer):
    _, paths, sources = scopes["ordinary"]
    assert finalization_probe(checker, consumer)(paths["lifecycle_run_inner"], sources["lifecycle_run_inner"], paths["native_finalized_output"], sources["native_finalized_output"]) == []


@pytest.mark.parametrize("consumer", ("production", "recovery"))
@pytest.mark.parametrize(("source_name", "old", "new"), (
    ("lifecycle_run_inner", "super::preflight_finalized_native_rollover(executor, services, native)", "super::preflight_finalized_native_rollover(executor, services, foreign)"),
    ("lifecycle_run_inner", ".ensure_closed_global_drained_cut()", ".ensure_open()"),
    ("lifecycle_run_inner", ".ensure_closed_global_drained_cut()", ".ensure_closed_drained_cut()"),
    ("native_finalized_output", "handoff_native_height_output_to_durable_reconstruction(receipt, artifact, &authority)", "handoff_native_height_output_to_durable_reconstruction(receipt, artifact, &foreign)"),
    ("native_finalized_output", ".complete_output_handoff(receipt, artifact)", ".complete_output_handoff(foreign, artifact)"),
))
def test_native_finalization_consumers_reject_foreign_owner(checker, scopes, consumer, source_name, old, new):
    _, paths, sources = scopes["ordinary"]
    assert sources[source_name].count(old) == 1
    changed = {**sources, source_name: sources[source_name].replace(old, new, 1)}
    assert finalization_probe(checker, consumer)(paths["lifecycle_run_inner"], changed["lifecycle_run_inner"], paths["native_finalized_output"], changed["native_finalized_output"])


@pytest.mark.parametrize("injected", (
    "advance_executor(receiver, owner, executor, services, None, 1)?;",
    "services.retry_pending_exact_output()?;",
    "retry_exact_output_and_apply_sidecar_admissions(executor, services)?;",
))
def test_blocked_native_branch_rejects_nested_ordinary_runtime(scopes, injected):
    probe, paths, sources = scopes["barrier"]
    source = sources["lifecycle_run_inner"]
    old = "if producer_claim.permits_open_decided_lane_recovery_ingress() {"
    assert source.count(old) == 1
    changed = source.replace(old, old + "\n" + injected, 1)
    errors = probe(paths, {**sources, "lifecycle_run_inner": changed})
    assert any("forbidden ordinary/retired authority" in error for error in errors), errors


def test_native_contract_is_called_by_whole_ordinary_gate(checker):
    assert checker._lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors(ROOT) == []


def fixture_probe(checker, consumer):
    function = checker._successor_recovery_source_fidelity_errors if consumer == "recovery" else checker._successor_production_recovery_source_fidelity_errors
    path = Path(function.__code__.co_filename)
    owner, = [node for node in ast.parse(path.read_text()).body if isinstance(node, ast.FunctionDef) and node.name == function.__name__]
    # Both original consumers use the same order-checking helper contract.
    helper_owner = checker._successor_recovery_source_fidelity_errors
    helper_path = Path(helper_owner.__code__.co_filename)
    definition, = [node for node in ast.parse(helper_path.read_text()).body if isinstance(node, ast.FunctionDef) and node.name == helper_owner.__name__]
    helper, = [node for node in definition.body if isinstance(node, ast.FunctionDef) and node.name == "require_order"]
    statement, = [node for node in ast.walk(owner) if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call) and any(isinstance(arg, ast.Constant) and arg.value == "production lifecycle finalization behavior" for arg in node.value.args)]
    wrapper = ast.parse("def fixture(path,source):\n    errors=[]\n    adapter_path=path\n    lifecycle_startup_test_path=path\n    finalization_behavior=_require_rust_item(path,source,'exercise_production_marker_replay_cases',errors)\n    return errors\n").body[0]
    wrapper.body[-1:-1] = [helper, statement]
    ns = dict(checker.__dict__)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[wrapper], type_ignores=[])), str(path), "exec"), ns)
    return ns["fixture"]


@pytest.mark.parametrize("consumer", ("production", "recovery"))
def test_marker_replay_fixture_uses_original_native_publication(checker, scopes, consumer):
    _, paths, sources = scopes["ordinary"]
    assert fixture_probe(checker, consumer)(paths["startup_test"], sources["startup_test"]) == []


@pytest.mark.parametrize("consumer", ("production", "recovery"))
@pytest.mark.parametrize(("old", "new"), (
    ("let mut native = lifecycle_native_process_fixture(&state, &local_signer, &output_guard);", "let mut native = lifecycle_native_process_fixture(&foreign_state, &local_signer, &output_guard);"),
    ("settle_terminal_fixture_runner_handoff(\n                &mut activated,\n                &mut runner,\n                &mut native,", "settle_terminal_fixture_runner_handoff(\n                &mut activated,\n                &mut runner,\n                &mut replacement,"),
    ("successor.parent_commit_qc = Some(artifact.commit_qc.clone());", "successor.parent_commit_qc = None;"),
))
def test_marker_replay_fixture_rejects_substituted_native_authority(checker, scopes, consumer, old, new):
    _, paths, sources = scopes["ordinary"]
    source = sources["startup_test"]
    item, = checker.rust_items(source, "exercise_production_marker_replay_cases")
    assert item.source.count(old) == 1
    changed = source.replace(item.source, item.source.replace(old, new, 1), 1)
    assert fixture_probe(checker, consumer)(paths["startup_test"], changed)


@pytest.mark.parametrize(("scope", "source_name", "name"), (
    ("ingress", "ordinary_consumer", "consume_prepared_native_ingress"),
    ("ingress", "ordinary_consumer", "consume_prepared_native_source_response"),
    ("barrier", "lifecycle_run_inner", "run_lifecycle_active_height"),
))
def test_native_runner_production_owner_cannot_be_cfg_disabled(checker, scopes, scope, source_name, name):
    probe, paths, sources = scopes[scope]
    source = sources[source_name]
    item, = checker.rust_items(source, name)
    changed = source.replace(item.source, "#[cfg(any())]\n" + item.source, 1)
    assert probe(paths, {**sources, source_name: changed})


def test_native_pending_ingress_projection_cannot_discard_custody(scopes):
    probe, paths, sources = scopes["ingress"]
    source = sources["native_process"]
    old = "pub(in crate::sumeragi) fn has_pending_ingress(&self) -> bool {\n        self.pending_ingress.is_some()\n    }"
    assert source.count(old) == 1
    changed = source.replace(old, old.replace("self.pending_ingress.is_some()", "false"), 1)
    assert probe(paths, {**sources, "native_process": changed})
