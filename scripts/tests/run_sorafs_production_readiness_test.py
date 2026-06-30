"""Tests for scripts/run_sorafs_production_readiness.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_production_readiness.py"
SPEC = importlib.util.spec_from_file_location("run_sorafs_production_readiness", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


CHECKER_PATH = Path(__file__).resolve().parents[1] / "check_sorafs_production_readiness.py"


def write_json(path: Path) -> Path:
    path.write_text('{"schema":"placeholder"}\n', encoding="utf-8")
    return path


def complete_args(tmp_path: Path) -> list[str]:
    args = [
        "--out-dir",
        str(tmp_path / "out"),
        "--verifier",
        str(CHECKER_PATH),
        "--deployment-id",
        "sorafs-mainnet-2026-06",
        "--environment",
        "production",
        "--now-unix",
        "1800800000",
    ]
    for gate, flag in MODULE.SUMMARY_FLAGS_BY_GATE.items():
        args.extend([flag, str(write_json(tmp_path / f"{gate}.json"))])
    return args


def test_dry_run_prints_complete_aggregate_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["schema"] == "sorafs.production_readiness.collection_plan.v1"
    assert payload["verifier_summary_schema"] == MODULE.SUMMARY_SCHEMA
    assert payload["deployment_context"] == {
        "deployment_id": "sorafs-mainnet-2026-06",
        "environment": "production",
    }
    assert set(payload["summary_contract"]) == set(MODULE.DEFAULT_REQUIRED_GATES)
    assert payload["summary_contract"]["gateway_load"]["required_kinds"]
    assert payload["steps"][0]["label"] == "sorafs_production_readiness_gate"
    assert "check_sorafs_production_readiness.py" in payload["steps"][0]["command"][1]


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "production readiness runner plan must be an object"
    ]

    rendered["thresholds"]["now_unix"] = float("inf")
    errors = MODULE.validate_plan_json(rendered, plan, args)
    assert "production readiness runner plan must be strict JSON renderable" in errors
    assert "inf" not in "\n".join(errors)
    rendered = MODULE.plan_json(plan, args)

    rendered["private_key"] = "runtime-only-key-material"
    rendered["external_summaries"] = {
        "gateway_load": [
            "artifacts/sorafs/gateway-load/summary.json",
            "artifacts/sorafs/gateway-load/summary-copy.json",
        ]
    }
    rendered["summary_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)
    assert (
        "production readiness runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary per required gate"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must match required gates"
        in diagnostics
    )
    assert "production readiness runner plan steps must match command plan" in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "summary-copy" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["production readiness runner plan steps must match command plan"]

    def fake_run_command_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    assert exit_code == 2
    assert not ran_plan
    assert (
        "production readiness runner plan steps must match command plan"
        in capsys.readouterr().err
    )


def test_execution_rejects_non_object_plan_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_plan_json(plan, args):
        return ["step"]

    def fake_run_command_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "plan_json", fake_plan_json)
    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    captured = capsys.readouterr()
    assert exit_code == 2
    assert not ran_plan
    assert captured.out == ""
    assert "production readiness runner plan must be an object" in captured.err


def test_execution_rejects_unrenderable_plan_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    original_plan_json = MODULE.plan_json
    ran_plan = False

    def fake_plan_json(plan, args):
        rendered = original_plan_json(plan, args)
        rendered["thresholds"]["max_summary_artifact_age_secs"] = float("inf")
        return rendered

    def fake_run_command_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "plan_json", fake_plan_json)
    monkeypatch.setattr(MODULE, "run_command_plan", fake_run_command_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    captured = capsys.readouterr()
    assert exit_code == 2
    assert not ran_plan
    assert captured.out == ""
    assert (
        "production readiness runner plan must be strict JSON renderable"
        in captured.err
    )
    assert "inf" not in captured.err


def test_missing_required_summary_fails(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--require-gate",
            "gateway_load",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert "missing required production readiness summary input" in captured.err
    assert "gateway_load" not in captured.err


def test_unrequired_summary_flag_fails(tmp_path: Path, capsys) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    reputation_summary = write_json(tmp_path / "reputation.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--reputation-summary",
            str(reputation_summary),
            "--require-gate",
            "gateway_load",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert "summary supplied for unrequired production readiness gate" in captured.err
    assert "reputation" not in captured.err


def test_duplicate_required_summary_flag_fails(tmp_path: Path, capsys) -> None:
    first_summary = write_json(tmp_path / "gateway-load.json")
    second_summary = write_json(tmp_path / "gateway-load-copy.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(first_summary),
            "--gateway-load-summary",
            str(second_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert (
        "production readiness runner requires exactly one summary input per required gate"
        in captured.err
    )
    assert "gateway-load-copy" not in captured.err


def test_response_file_arguments_pass(tmp_path: Path, capsys) -> None:
    args_file = tmp_path / "production-readiness.args"
    args_file.write_text("\n".join(complete_args(tmp_path) + ["--dry-run"]) + "\n", encoding="utf-8")

    assert MODULE.main([f"@{args_file}"]) == 0
    assert json.loads(capsys.readouterr().out)["schema"] == (
        "sorafs.production_readiness.collection_plan.v1"
    )


def test_narrowed_required_gate_plan(tmp_path: Path, capsys) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    assert exit_code == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["required_gates"] == ["gateway_load"]
    assert payload["external_summaries"] == {
        "gateway_load": [str(gateway_summary)]
    }
    assert payload["deployment_context"] == {
        "deployment_id": "sorafs-mainnet-2026-06",
        "environment": "production",
    }


def test_partial_deployment_context_fails(tmp_path: Path, capsys) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--dry-run",
        ]
    )

    assert exit_code == 2
    captured = capsys.readouterr()
    assert (
        "production readiness runner requires --deployment-id and --environment"
        in captured.err
    )


def test_malformed_deployment_context_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = " sorafs-mainnet-2026-06"
    args.environment = "prod\nsecret"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment context must use canonical labels"
        in errors
    )
    rendered = "\n".join(errors)
    assert "sorafs-mainnet-2026-06" not in rendered
    assert "prod\nsecret" not in rendered


def test_nonproduction_environment_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.environment = "staging"

    errors = MODULE.validate_inputs(args)

    assert "production readiness runner environment must be production" in errors
    assert "staging" not in "\n".join(errors)


def test_unreviewed_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-notproductionready-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['notproductionready']"
        in errors
    )
    assert "gateway-notproductionready-a" not in "\n".join(errors)


def test_summary_input_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    unsafe_summary = write_json(tmp_path / "gateway_private_key_summary.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(unsafe_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert (
        "production readiness runner summary input paths must not contain "
        "secret-looking, control-character, parent, current, or platform-specific components"
        in captured.err
    )
    assert captured.out == ""
    assert "gateway_private_key_summary" not in captured.err
    assert "private_key" not in captured.err


def test_plan_rendered_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "private_key_output"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "private_key_output" not in captured.err
    assert "private_key" not in captured.err


def test_plan_rendered_summary_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--summary-out",
            str(tmp_path / "bearer_token_summary.json"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "bearer_token_summary" not in captured.err
    assert "bearer_token" not in captured.err


def test_plan_rendered_verifier_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    unsafe_verifier = tmp_path / "private_key_verifier.py"
    unsafe_verifier.write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(unsafe_verifier),
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
            "--dry-run",
        ]
    )

    captured = capsys.readouterr()
    assert exit_code == 2
    assert MODULE.PLAN_RENDERED_PATH_ERROR in captured.err
    assert captured.out == ""
    assert "private_key_verifier" not in captured.err
    assert "private_key" not in captured.err


def test_plan_rendered_path_safety_rejects_drive_prefix() -> None:
    assert not MODULE.plan_rendered_path_is_safe(Path("C:/sorafs/summary.json"))


def test_summary_input_path_safety_accepts_digest_labels(tmp_path: Path) -> None:
    safe_summary = tmp_path / "gateway_load_digest.json"
    write_json(safe_summary)
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--gateway-load-summary",
            str(safe_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
        ]
    )

    assert MODULE.validate_inputs(args) == []
