"""Retained Apply source contracts preserve physical waits and move-only custody."""
from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import sys

import pytest

ROOT = Path(__file__).resolve().parents[2]


@pytest.fixture(scope="module")
def checker():
    path = ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    spec = importlib.util.spec_from_file_location("retained_apply_owner_checker", path)
    assert spec and spec.loader
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


@pytest.fixture(scope="module")
def sources(checker):
    paths, sources, errors = {}, {}, []
    for role, name in (("worker", "v2_worker.rs"), ("launch", "v2_lifecycle_launch.rs")):
        paths[role], sources[role] = checker._read_reviewed_rust_source(
            ROOT, "crates/iroha_core/src/sumeragi/" + name, errors,
            "retained Apply defining owner",
        )
    assert not errors, errors
    return paths, sources


@pytest.fixture(scope="module")
def probe(checker):
    """Execute the actual lineage consumer's helper call and order predicate."""
    path = Path(checker._lifecycle_decision_apply_lineage_source_fidelity_errors.__code__.co_filename)
    owner, = [node for node in ast.parse(path.read_text()).body
              if isinstance(node, ast.FunctionDef)
              and node.name == "_lifecycle_decision_apply_lineage_source_fidelity_errors"]
    order, = [node for node in owner.body
              if isinstance(node, ast.FunctionDef) and node.name == "require_order"]
    call, = [node for node in owner.body
             if isinstance(node, ast.Expr) and isinstance(node.value, ast.Call)
             and isinstance(node.value.func, ast.Name)
             and node.value.func.id == "_lifecycle_retained_apply_owner_source_fidelity_errors"]
    func = ast.parse("def probe(worker_path, worker_source, launch_path, launch_source):\n    errors=[]\n    return errors\n").body[0]
    func.body[-1:-1] = [order, call]
    namespace = dict(checker.__dict__)
    exec(compile(ast.fix_missing_locations(ast.Module(body=[func], type_ignores=[])), str(path), "exec"), namespace)
    return namespace["probe"]


def run(probe, paths, sources):
    return probe(paths["worker"], sources["worker"], paths["launch"], sources["launch"])


def test_current_retained_apply_contracts(probe, sources):
    assert run(probe, *sources) == []


def test_complete_live_and_recovered_apply_lineage(checker):
    assert checker._lifecycle_decision_apply_lineage_source_fidelity_errors(ROOT) == []


MUTATIONS = (
    ("worker", "impl PreparedLifecycleDecisionApplyCompletionV1", "new",
     "RetainedApplyDependency::new(refusal)", "RetainedApplyDependency::new(foreign)"),
    ("worker", "impl RetainedApplyDependency", "new",
     "busy.wait.clone().wait_for_release()", "foreign.wait_for_release()"),
    ("worker", "impl RetainedApplyDependency", "new",
     "wake: wake.clone()", "wake: foreign.clone()"),
    ("worker", "impl RetainedApplyDependency", "ready",
     "std::pin::Pin::new(pending)", "std::pin::Pin::new(replacement)"),
    ("worker", "impl RetainedApplyDependency", "ready",
     "std::task::Context::from_waker(wake)", "std::task::Context::from_waker(foreign)"),
    ("worker", "impl RetainedApplyDependency", "ready",
     "Self::RecoveryRequired(reason) => Err(reason.clone())", "Self::RecoveryRequired(_) => Ok(true)"),
    ("worker", "impl PreparedLifecycleDecisionApplyCompletionV1", "retry_deferred",
     "Some(Ok(false)) => return LifecycleDecisionApplyDeferredRetryV1::Unavailable(self)", "Some(Ok(false)) => {}"),
    ("worker", "impl PreparedLifecycleDecisionApplyCompletionV1", "retry_deferred",
     "self.dependency.as_mut().map(RetainedApplyDependency::ready)", "fresh_dependency.as_mut().map(RetainedApplyDependency::ready)"),
    ("worker", "impl PreparedLifecycleDecisionApplyCompletionV1", "retry_deferred",
     "retry_lifecycle_decision_apply(task)", "retry_lifecycle_decision_apply(replacement)"),
    ("worker", "impl PreparedLifecycleDecisionApplyCompletionV1", "retry_deferred",
     "work_ack.acknowledge_retry_publication();\n                completion_guard.disarm();",
     "completion_guard.disarm();\n                work_ack.acknowledge_retry_publication();"),
    ("worker", "impl PreparedLifecycleDecisionApplyCompletionV1", "retry_deferred",
     "                    dependency,", "                    dependency: None,"),
    ("worker", "impl PreparedLifecycleDecisionApplyCompletionV1", "retry_deferred",
     "self.work_ack.output_guard.retain_effect_failure(reason);", ""),
    ("worker", "impl V2IoCommandQueue", "retry_lifecycle_decision_apply",
     "lifecycle_decision_apply_completion_is_exact(key)", "lifecycle_decision_apply_completion_is_exact(foreign_key)"),
    ("worker", "impl V2IoCommandQueue", "retry_lifecycle_decision_apply",
     "tracked.state != V2IoWorkState::CompletionPending", "false"),
    ("worker", "impl V2IoCommandQueue", "retry_lifecycle_decision_apply",
     "command.lifecycle_decision_apply_key() == Some(key)", "false"),
    ("worker", "impl V2IoCommandQueue", "retry_lifecycle_decision_apply",
     "self.admission.try_reserve(V2IoAdmissionClass::Consensus)", "true"),
    ("worker", "impl V2IoCommandQueue", "retry_lifecycle_decision_apply",
     "transfer_lifecycle_decision_apply_completion(key)", "transfer_lifecycle_decision_apply_completion(foreign)"),
    ("worker", "impl V2IoCommandQueue", "retry_lifecycle_decision_apply",
     "state.commands.push_back(task.into_command());", ""),
    ("worker", "impl LifecycleDecisionApplyRetryTaskV1 for LifecycleDecisionApplyTaskV1", "into_command",
     "V2IoCommand::LifecycleDecisionApply(self)", "V2IoCommand::LifecycleDecisionApply(replacement)"),
    ("worker", "impl LifecycleDecisionApplyCapacityReservationV1<'_>", "commit",
     "self.preflight(&prepared)", "true"),
    ("worker", "impl LifecycleDecisionApplyCapacityReservationV1<'_>", "commit",
     "task.dispatch_key(),\n            self.key,", "task.dispatch_key(),\n            foreign_key,"),
    ("worker", "impl LifecycleDecisionApplyCapacityReservationV1<'_>", "commit",
     "executor_dispatch.commit_after_worker_dispatch();", ""),
    ("worker", "impl V2IoHandle", "spawn",
     ".execute_retained_lifecycle_apply(", ".execute_reconstructed_lifecycle_apply("),
    ("launch", "impl RetainedLifecycleDecisionApplyDeferredV1", "retry_after_local_release",
     "match completion.retry_deferred()", "match replacement.retry_deferred()"),
    ("launch", "impl RetainedLifecycleDecisionApplyDeferredV1", "retry_after_local_release",
     "ProductionLifecycleDecisionApplyRetryV1::Unavailable(Self { completion })",
     "ProductionLifecycleDecisionApplyRetryV1::Unavailable(Self { completion: replacement })"),
)


@pytest.mark.parametrize(("role", "owner", "name", "old", "new"), MUTATIONS,
                         ids=[f"{row[2]}-{i}" for i, row in enumerate(MUTATIONS)])
def test_lost_wait_or_owner_is_rejected(checker, probe, sources, role, owner, name, old, new):
    paths, original = sources
    source = original[role]
    item, = [item for item in checker.rust_items(source, name)
             if item.brace_context == (checker.rust_code_tokens(owner),)]
    assert item.source.count(old) == 1, (name, old)
    assert source.count(item.source) == 1
    changed = source.replace(item.source, item.source.replace(old, new, 1), 1)
    assert run(probe, paths, {**original, role: changed}), (name, old)


@pytest.mark.parametrize(("name", "old", "new"), (
    ("settle_lifecycle_decision_apply_completion_owner", "RetainedLifecycleDecisionApplyDeferredV1 { completion }", "RetainedLifecycleDecisionApplyDeferredV1 { completion: replacement }"),
    ("settle_applied_lifecycle_decision_apply_completion", "persist_exact_staged_successor(&staged)", "persist_exact_staged_successor(&foreign)"),
    ("settle_applied_lifecycle_decision_apply_completion", "let published = applied.into_published();", "let published = replacement.into_published();"),
    ("settle_applied_lifecycle_decision_apply_completion", "        published,", "        replacement,"),
))
def test_applied_settlement_preserves_original_publication(checker, sources, monkeypatch, name, old, new):
    paths, original = sources
    item, = checker.rust_items(original["launch"], name)
    assert item.source.count(old) == 1
    changed = original["launch"].replace(item.source, item.source.replace(old, new, 1), 1)
    loader = checker._read_reviewed_rust_source

    def read(root, relative, errors, description):
        if relative == "crates/iroha_core/src/sumeragi/v2_lifecycle_launch.rs":
            return paths["launch"], changed
        return loader(root, relative, errors, description)

    monkeypatch.setattr(checker, "_read_reviewed_rust_source", read)
    assert checker._lifecycle_decision_apply_lineage_source_fidelity_errors(ROOT)
