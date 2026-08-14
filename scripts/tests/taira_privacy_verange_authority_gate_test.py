from __future__ import annotations

import ast
import builtins
import re
from pathlib import Path
from types import SimpleNamespace

import pytest

from scripts import taira_privacy_sealed_controller as controller
from scripts import taira_privacy_governance_authority as governance_authority
from scripts import taira_privacy_verange_case_plan as case_plan


ROOT = Path(__file__).resolve().parents[2]


def _forged_arguments(tag: str) -> dict[str, object]:
    candidate = (tag * 64)[:64]
    if re.fullmatch(r"[0-9a-f]{64}", candidate) is None:
        candidate = "11" * 32
    self_hash = candidate
    return {
        "bundle": SimpleNamespace(
            manifest={
                "candidate_binding_sha256": candidate,
                "privacy_qualification_setup": {
                    "candidate_binding_sha256": candidate,
                    "schema": case_plan.GENESIS_PLAN_SCHEMA,
                    "schema_version": case_plan.GENESIS_PLAN_SCHEMA_VERSION,
                    "setup_authority_account_id": "candidate-controlled",
                    "setup_authority_public_key_hex": self_hash,
                    "setup_requirements_sha256": self_hash,
                },
                "signed_genesis_sha256": self_hash,
                "unsigned_genesis_sha256": self_hash,
            }
        ),
        "candidate_binding_sha256": candidate,
        "cargo_lock_sha256": self_hash,
        "workspace_source_manifest_sha256": self_hash,
        "public_artifacts": SimpleNamespace(
            setup_authority_public_key_hex=self_hash,
            signer_public_key_hex=self_hash,
        ),
        "supervisors": ("reused-candidate-signer",) * 4,
        "supervisor_sha256": self_hash,
        "restart_generation": self_hash,
    }


@pytest.mark.parametrize("tag", ("1", "2", "a", "b"))
def test_forged_manifest_self_hash_source_splice_and_signer_reuse_cannot_plan(
    tag: str,
) -> None:
    with pytest.raises(
        case_plan.VeRangeQualificationPlanError,
        match=case_plan.VERANGE_SETUP_AUTHORITY_PROVISIONING_BARRIER,
    ) as raised:
        case_plan.build_verange_qualification_case_plan_v1(
            **_forged_arguments(tag)  # type: ignore[arg-type]
        )
    assert case_plan.VERANGE_GOVERNANCE_AUTHORITY_ENVELOPE_SCHEMA in str(raised.value)
    assert case_plan.VERANGE_GOVERNANCE_AUTHORITY_REPLAY_NAMESPACE in str(raised.value)


def test_public_builder_barrier_precedes_argument_path_and_io_access(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    def forbidden(*_args: object, **_kwargs: object) -> object:
        raise AssertionError("authority barrier allowed path or file I/O")

    monkeypatch.setattr(builtins, "open", forbidden)
    monkeypatch.setattr(Path, "open", forbidden)
    monkeypatch.setattr(Path, "read_bytes", forbidden)
    monkeypatch.setattr(Path, "read_text", forbidden)

    with pytest.raises(
        case_plan.VeRangeQualificationPlanError,
        match=case_plan.VERANGE_SETUP_AUTHORITY_PROVISIONING_BARRIER,
    ):
        case_plan.build_verange_qualification_case_plan_v1(
            **_forged_arguments("c")  # type: ignore[arg-type]
        )


def test_barrier_is_first_operation_and_has_no_caller_escape() -> None:
    source = (ROOT / "scripts/taira_privacy_verange_case_plan.py").read_text(
        encoding="utf-8"
    )
    module = ast.parse(source)
    function = next(
        node
        for node in module.body
        if isinstance(node, ast.FunctionDef)
        and node.name == "build_verange_qualification_case_plan_v1"
    )
    operations = list(function.body)
    if (
        operations
        and isinstance(operations[0], ast.Expr)
        and isinstance(operations[0].value, ast.Constant)
        and isinstance(operations[0].value.value, str)
    ):
        operations.pop(0)
    first = operations[0]
    assert isinstance(first, ast.Expr)
    assert isinstance(first.value, ast.Call)
    assert isinstance(first.value.func, ast.Name)
    assert first.value.func.id == "_require_authenticated_verange_governance_authority_v1"
    assert first.value.args == [] and first.value.keywords == []

    barrier = next(
        node
        for node in module.body
        if isinstance(node, ast.FunctionDef)
        and node.name == "_require_authenticated_verange_governance_authority_v1"
    )
    names = {node.id for node in ast.walk(barrier) if isinstance(node, ast.Name)}
    attributes = {
        node.attr for node in ast.walk(barrier) if isinstance(node, ast.Attribute)
    }
    assert not ({"environ", "getenv", "open", "Path"} & (names | attributes))
    governance_source = (
        ROOT / "scripts/taira_privacy_governance_authority.py"
    ).read_text(encoding="utf-8")
    assert case_plan.VERANGE_GOVERNANCE_AUTHORITY_ENVELOPE_SCHEMA == (
        governance_authority.AUTHORITY_ENVELOPE_SCHEMA
    )
    assert case_plan.VERANGE_GOVERNANCE_AUTHORITY_REPLAY_NAMESPACE == (
        governance_authority.REPLAY_NAMESPACE
    )
    assert governance_authority.AUTHORITY_ENVELOPE_SCHEMA in governance_source
    assert governance_authority.REPLAY_NAMESPACE in governance_source


def test_no_lower_production_planner_or_registered_runner_bypasses_barrier() -> None:
    case_plan_path = ROOT / "scripts/taira_privacy_verange_case_plan.py"
    case_plan_source = case_plan_path.read_text(encoding="utf-8")
    sealed_source = (ROOT / "scripts/taira_privacy_sealed_controller.py").read_text(
        encoding="utf-8"
    )
    assert controller.build_verange_qualification_case_plan_v1 is (
        case_plan.build_verange_qualification_case_plan_v1
    )
    private_builder = "_build_untrusted_verange_qualification_case_plan_v1"
    assert case_plan_source.count(f"def {private_builder}(") == 1
    assert case_plan_source.count("def build_verange_qualification_case_plan_v1(") == 1
    production_callers = []
    for path in sorted((ROOT / "scripts").glob("*.py")):
        if path == case_plan_path:
            continue
        if private_builder in path.read_text(encoding="utf-8"):
            production_callers.append(path.name)
    assert production_callers == []
    assert private_builder not in sealed_source
    assert dict(controller.CONTROLLER_CASE_RUNNERS) == {}
    assert controller.VERANGE_PROTOCOL not in controller.CONTROLLER_CASE_RUNNERS
    assert case_plan.VERANGE_SETUP_AUTHORITY_PROVISIONING_BARRIER in (
        controller.controller_case_blockers(controller.VERANGE_PROTOCOL)
    )


def test_frozen_genesis_model_has_no_setup_authority_or_activation_bundle() -> None:
    composer = (ROOT / "scripts/prepare_taira_empty_reset_bundle.py").read_text(
        encoding="utf-8"
    )
    bootstrap_validator = (
        ROOT / "configs/soranexus/taira/validate_privacy_bootstrap.py"
    ).read_text(encoding="utf-8")
    driver = (
        ROOT / "crates/iroha_core/src/bin/privacy_exact12_action_driver.rs"
    ).read_text(encoding="utf-8")

    assert case_plan.GENESIS_PLAN_MANIFEST_FIELD not in composer
    assert 'rollout["activation_state"] != "not-executed"' in bootstrap_validator
    assert 'rollout["genesis_activation_forbidden"] is not True' in bootstrap_validator
    assert "genesis must grant CanEnactGovernance to the authority exactly once" in (
        bootstrap_validator
    )
    assert "fn verange_setup_authority_v1(candidate: &[u8; 32])" in driver
    assert "derive_nonzero_verange_setup_seed(candidate)" in driver
    assert "MissingNativePublicOnlyVeRangePolicyActivationTransactionBundle" in driver
    assert "registration_instruction_norito_hex" not in driver
