"""Recovery contracts require the original retained validator through marker promotion."""

from __future__ import annotations

import ast
import importlib.util
from pathlib import Path
import sys

import pytest


ROOT = Path(__file__).resolve().parents[2]
BODY_STORE = ROOT / "crates/iroha_core/src/sumeragi/v2_body_store.rs"
FACTORY = ROOT / "crates/iroha_core/src/sumeragi/v2_authenticated_recovered_adapter_startup_impl.rs"
TURN_DRIVER = ROOT / "crates/iroha_core/src/sumeragi/v2_lifecycle_turn_driver.rs"
CONSUMERS = (
    ("successor_recovery_source", "_successor_recovery_source_fidelity_errors"),
    ("successor_production_recovery", "_successor_production_recovery_source_fidelity_errors"),
)


@pytest.fixture(scope="module")
def checker():
    path = ROOT / "scripts/formal/check_sumeragi_v2_proof_ledger.py"
    spec = importlib.util.spec_from_file_location("retained_marker_replay_checker", path)
    assert spec is not None and spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


def consumer_probe(checker, consumer, helper, arguments):
    """Execute the real consumer's call without unrelated whole-repository scans."""
    suffix, name = consumer
    path = ROOT / f"scripts/formal/sumeragi_v2_proof_ledger_{suffix}_contracts.py"
    owner, = [node for node in ast.parse(path.read_text()).body
              if isinstance(node, ast.FunctionDef) and node.name == name]
    calls = [node for node in ast.walk(owner)
             if isinstance(node, ast.Expr)
             and isinstance(node.value, ast.Call)
             and isinstance(node.value.func, ast.Attribute)
             and node.value.func.attr == "extend"
             and any(isinstance(arg, ast.Call)
                     and isinstance(arg.func, ast.Name)
                     and arg.func.id == helper
                     for arg in node.value.args)]
    assert len(calls) == 1, f"each recovery gate must invoke {helper} once"
    function = ast.parse(
        f"def replay_probe({arguments}):\n"
        "    errors = []\n"
        "    return errors\n"
    ).body[0]
    function.body[-1:-1] = calls
    module = ast.fix_missing_locations(ast.Module(body=[function], type_ignores=[]))
    namespace = dict(checker.__dict__)
    exec(compile(module, str(path), "exec"), namespace)
    return namespace["replay_probe"]


@pytest.fixture(params=CONSUMERS, ids=lambda value: value[0])
def probe(checker, request):
    return consumer_probe(checker, request.param,
                          "_quarantined_retained_marker_replay_errors",
                          "body_store_path, body_store_source")


@pytest.fixture(params=CONSUMERS, ids=lambda value: value[0])
def factory_probe(checker, request):
    return consumer_probe(checker, request.param, "_retained_replay_factory_errors",
                          "adapter_path, adapter_source")


def test_retained_marker_replay_current_source(probe):
    assert probe(BODY_STORE, BODY_STORE.read_text()) == []


@pytest.mark.parametrize(("old", "new"), (
    ("context: super::v2::VerifiedHeightContext", "context: wire::HeightContext"),
    ("recovered_finality_subject(context.context())", "recovered_finality_subject(&foreign)"),
    ("retain_recovered_markers_for_authority(validation_authority)", "retain_all_markers()"),
    ("NativeApplyService::new(apply_service, &self.0, context)",
     "NativeApplyService::new(apply_service, &foreign_store, context)"),
    ("service.revalidate_recovered_markers(&mut self.0)?;",
     "self.0.revalidate_recovered_markers(|body| validator(body))?;"),
    ("service.revalidate_recovered_markers(&mut self.0)?;",
     "self.0.into_revalidated_startup()?; service.revalidate_recovered_markers(&mut self.0)?;"),
    ("Ok((store, service))", "Ok((store, replacement_service))"),
    ("impl QuarantinedV2BodyStore {", "impl ForeignBodyStore {"),
))
def test_retained_marker_replay_rejects_owner_loss(probe, old, new):
    source = BODY_STORE.read_text()
    begin = source.index("impl QuarantinedV2BodyStore {")
    end = source.index("impl RevalidatedV2BodyStore {", begin)
    region = source[begin:end]
    assert region.count(old) == 1
    changed = source[:begin] + region.replace(old, new, 1) + source[end:]
    assert probe(BODY_STORE, changed), (old, new)


def test_retained_replay_factory_current_source(factory_probe):
    assert factory_probe(FACTORY, FACTORY.read_text()) == []


@pytest.mark.parametrize(("old", "new"), (
    ("pending_kura.is_some() && !matches!(self.authority, RecoveredWalStartupAuthorityV1::None)",
     "pending_kura.is_some()"),
    ("Arc::ptr_eq(&adapter_owner, &self.factory_owner)", "true"),
    ("&storage.signature_policy", "&foreign_policy"),
    ("self.adapter.wal.matches_path(&storage.wal_path)", "true"),
    ("context: self.adapter.wire_context.clone()", "context: foreign_context"),
    ("proofs_of_possession: self.adapter.proofs_of_possession.clone()",
     "proofs_of_possession: foreign_roster"),
    ("parent_verification: self.adapter.parent_verification.clone()",
     "parent_verification: foreign_parent"),
    ("let (body_store, apply_service) = body_store", "let (body_store, _) = body_store"),
    ("into_revalidated_lifecycle_startup(apply_service, replay_context, validation_authority)",
     "into_revalidated_lifecycle_startup(&apply_service, &context, validation_authority)"),
    ("into_revalidated_lifecycle_startup(apply_service, replay_context, validation_authority)",
     "into_revalidated_lifecycle_startup(replacement_service, replay_context, validation_authority)"),
    ("owner.with_recovered_kura_binding_and_apply_service(kura_binding, apply_service)",
     "owner.with_recovered_kura_binding_and_apply_service(kura_binding, replacement_service)"),
    ("let (body_store, apply_service) = body_store",
     "body_store.into_revalidated_lifecycle_startup(apply_service, replay_context, validation_authority)?; "
     "let (body_store, apply_service) = body_store"),
    ("fn open_production_lifecycle_owner_with_pending_kura_v1(",
     "fn unowned_factory("),
))
def test_retained_replay_factory_rejects_owner_loss(factory_probe, old, new):
    source = FACTORY.read_text()
    begin = source.index("fn open_production_lifecycle_owner_with_pending_kura_v1(")
    end = source.index("fn open_production_lifecycle_owner_v1_at_authenticated_roots(", begin)
    region = source[begin:end]
    assert region.count(old) == 1
    changed = source[:begin] + region.replace(old, new, 1) + source[end:]
    assert factory_probe(FACTORY, changed), (old, new)


@pytest.fixture(scope="module")
def completion_probe(checker):
    """Run the actual Completion subsection, including its real token predicates."""
    path = ROOT / "scripts/formal/sumeragi_v2_proof_ledger_successor_recovery_tail_contracts.py"
    owner, = [node for node in ast.parse(path.read_text()).body
              if isinstance(node, ast.FunctionDef)
              and node.name == "_lifecycle_turn_driver_ordinary_ingress_source_fidelity_errors"]
    helpers = [node for node in owner.body if isinstance(node, ast.FunctionDef)
               and node.name in {"require_order", "require_tokens", "reject_tokens"}]
    assert len(helpers) == 3
    begin, = [i for i, node in enumerate(owner.body)
              if isinstance(node, ast.FunctionDef) and node.name == "launched_completion_item"]
    end, = [i for i, node in enumerate(owner.body)
            if isinstance(node, ast.Assign) and any(isinstance(target, ast.Name)
                                                   and target.id == "completion_head"
                                                   for target in node.targets)]
    assert begin < end
    function = ast.parse(
        "def completion_probe(source):\n"
        "    paths = {'driver': TURN_DRIVER}\n"
        "    sources = {'driver': source}\n"
        "    errors = []\n"
        "    return errors\n"
    ).body[0]
    function.body[-1:-1] = helpers + owner.body[begin:end]
    module = ast.fix_missing_locations(ast.Module(body=[function], type_ignores=[]))
    namespace = {**checker.__dict__, "TURN_DRIVER": TURN_DRIVER}
    exec(compile(module, str(path), "exec"), namespace)
    return namespace["completion_probe"]


def test_retained_completion_current_source(completion_probe):
    assert completion_probe(TURN_DRIVER.read_text()) == []


@pytest.mark.parametrize(("old", "new"), (
    ("self.runner_turn_matches(", "self.unchecked_runner_turn("),
    ("self.services.retry_local_apply()", "Ok::<_, String>(())"),
    ("if let Err(reason) = self.services.retry_local_apply()",
     "self.services.retry_local_apply()?; if let Err(reason) = self.services.retry_local_apply()"),
    (".retain_effect_failure(reason)", ".ignore_effect_failure(reason)"),
    ("self.retry_local_lifecycle_validate(retained)", "self.retry_foreign_validate(retained)"),
    ("let selected = self.retry_local_lifecycle_validate(retained);",
     "let selected = self.retry_local_lifecycle_validate(retained); "
     "self.drive_registered_lifecycle_validate_sidecar(registration, lane_work);"),
    ("self.services.take_next_lifecycle_completion()", "self.services.skip_next_lifecycle_completion()"),
    ("self.services.take_next_lifecycle_completion()",
     "{ self.services.take_next_lifecycle_completion(); self.services.take_next_lifecycle_completion() }"),
    ("if proposal_sign_preemption.is_none() {", "if false {"),
))
def test_retained_completion_rejects_owner_loss(completion_probe, old, new):
    source = TURN_DRIVER.read_text()
    begin = source.index("fn drive_completion_pre_gate_inner<")
    end = source.index("fn drive_apply_terminal_ready_broadcast_turn<", begin)
    region = source[begin:end]
    assert old in region
    changed = source[:begin] + region.replace(old, new, 1) + source[end:]
    assert completion_probe(changed), (old, new)


def test_retained_completion_rejects_late_apply_retry(completion_probe):
    source = TURN_DRIVER.read_text()
    begin = source.index("        if let Err(reason) = self.services.retry_local_apply()")
    end = source.index("        let current_validate_fence_wait =", begin)
    retry = source[begin:end]
    changed = source[:begin] + source[end:]
    position = changed.index("        match self.services.take_next_lifecycle_completion()")
    changed = changed[:position] + retry + changed[position:]
    assert completion_probe(changed)
