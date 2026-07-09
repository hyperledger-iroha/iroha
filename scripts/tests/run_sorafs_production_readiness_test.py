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


def test_help_marks_final_deployment_context_required(capsys) -> None:
    try:
        MODULE.parse_args(["--help"])
    except SystemExit as error:
        assert error.code == 0
    else:  # pragma: no cover - argparse always exits for --help
        raise AssertionError("expected --help to exit")

    help_text = " ".join(capsys.readouterr().out.split())

    assert (
        "Required final deployment id shared by every required lane summary"
        in help_text
    )
    assert (
        "Required final prod/production environment shared by every required"
        in help_text
    )
    assert "Optional expected deployment id" not in help_text
    assert "Optional expected environment" not in help_text


def test_malformed_integer_arguments_fail_before_validation(capsys) -> None:
    cases = [
        ("--now-unix", "private-key-01", "must be an integer"),
        ("--now-unix", "0", "must be positive"),
        (
            "--max-summary-artifact-age-secs",
            "private-key-02",
            "must be an integer",
        ),
        ("--max-summary-artifact-age-secs", "-1", "must be non-negative"),
    ]

    for flag, value, diagnostic in cases:
        assert MODULE.main([flag, value]) == 2

        captured = capsys.readouterr()
        assert diagnostic in captured.err
        assert value not in captured.err
        assert captured.out == ""


def test_duplicate_required_gate_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "out"),
                "--require-gate",
                "gateway_load",
                "--require-gate",
                "gateway_load",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "duplicate required evidence kind" in captured.err
    assert "gateway_load" not in captured.err
    assert captured.out == ""


def test_unknown_required_gate_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    unknown_gate = "private-key-placeholder"

    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "out"),
                "--require-gate",
                unknown_gate,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind" in captured.err
    assert unknown_gate not in captured.err
    assert captured.out == ""


def test_malformed_required_gate_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    malformed_gate = "gateway_load,"

    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "out"),
                "--require-gate",
                malformed_gate,
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert (
        "--require-kind entries must be non-empty canonical strings"
        in captured.err
    )
    assert malformed_gate not in captured.err
    assert captured.out == ""


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


def test_plan_json_schema_fields_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["schema"] = "sorafs\nproduction"
    rendered["verifier_summary_schema"] = "runtime-only\nschema"

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "production readiness runner plan schema must be canonical" in diagnostics
    assert (
        "production readiness runner plan schema must match the contract"
        in diagnostics
    )
    assert (
        "production readiness runner plan verifier schema must be canonical"
        in diagnostics
    )
    assert (
        "production readiness runner plan verifier schema must match aggregate schema"
        in diagnostics
    )
    assert "sorafs\nproduction" not in diagnostics
    assert "runtime-only\nschema" not in diagnostics


def test_plan_json_top_level_fields_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan fields must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "bad\nfield" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics


def test_plan_json_deployment_context_must_be_final_production(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-staging-a"
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-staging-a" not in "\n".join(errors)

    args = MODULE.parse_args(complete_args(tmp_path))
    args.environment = "staging"
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan environment must be production"
        in errors
    )
    assert "staging" not in "\n".join(errors)


def test_plan_json_deployment_context_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["deployment_context"] = {
        "deployment_id": "gateway\nproduction",
        "environment": 7,
        "private_key": "runtime-only-key-material",
        "bad\nkey": "production",
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan deployment_context fields must be deployment_id and environment"
        in diagnostics
    )
    assert (
        "production readiness runner plan deployment_context keys must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan deployment_context must be canonical"
        in diagnostics
    )
    assert (
        "production readiness runner plan deployment_context must match args"
        in diagnostics
    )
    assert "gateway\nproduction" not in diagnostics
    assert "private_key" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "bad\nkey" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["deployment_context"] = ["deployment_id", "environment"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan deployment_context must be an object"
        in errors
    )
    assert (
        "production readiness runner plan deployment_context must match args"
        in errors
    )


def test_plan_json_thresholds_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["thresholds"] = {
        "max_summary_artifact_age_secs": -1,
        "now_unix": 0,
        "private_key": 7,
        "bad\nkey": 3,
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan thresholds must contain only max_summary_artifact_age_secs and optional now_unix"
        in diagnostics
    )
    assert (
        "production readiness runner plan thresholds.max_summary_artifact_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "production readiness runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert "production readiness runner plan thresholds must match args" in diagnostics
    assert "private_key" not in diagnostics
    assert "bad\nkey" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["thresholds"] = {"now_unix": args.now_unix}

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan thresholds.max_summary_artifact_age_secs must be present"
        in errors
    )
    assert (
        "production readiness runner plan thresholds.max_summary_artifact_age_secs must be a non-negative integer"
        in errors
    )

    rendered = MODULE.plan_json(plan, args)
    rendered["thresholds"] = ["max_summary_artifact_age_secs"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan thresholds must be an object" in errors
    assert "production readiness runner plan thresholds must match args" in errors


def test_plan_json_required_gates_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["required_gates"] = [
        "gateway_load",
        "gateway_load",
        "unknown_gate",
        "gateway\nload",
    ]

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan required_gates must contain canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan required_gates must not contain duplicate gates"
        in diagnostics
    )
    assert (
        "production readiness runner plan required_gates must use known gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan required_gates must match args"
        in diagnostics
    )
    assert "unknown_gate" not in diagnostics
    assert "gateway\nload" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["required_gates"] = "gateway_load"

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan required_gates must be a list" in errors
    assert (
        "production readiness runner plan required_gates must match args"
        in errors
    )


def test_plan_json_external_summaries_shape_is_validated(tmp_path: Path) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    reputation_summary = write_json(tmp_path / "reputation.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--now-unix",
            "1800800000",
            "--gateway-load-summary",
            str(gateway_summary),
            "--require-gate",
            "gateway_load",
            "--deployment-id",
            "sorafs-mainnet-2026-06",
            "--environment",
            "production",
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_summaries"] = {
        "gateway_load": [str(gateway_summary), str(gateway_summary)],
        "reputation": [str(reputation_summary)],
        "unknown_gate": [str(tmp_path / "unknown.json")],
        "gateway\nload": [str(gateway_summary)],
        "repair": "artifacts/repair/summary.json",
        "por": [7],
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan external_summaries keys must be canonical gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries keys must use known gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain only required gates"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must map each gate to a summary path list"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary path per gate"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries paths must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary per required gate"
        in diagnostics
    )
    assert "unknown_gate" not in diagnostics
    assert "gateway\nload" not in diagnostics
    assert "reputation" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["external_summaries"] = ["gateway_load"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert (
        "production readiness runner plan external_summaries must be an object"
        in errors
    )
    assert (
        "production readiness runner plan external_summaries must contain exactly one summary per required gate"
        in errors
    )


def test_plan_json_summary_contract_shape_is_validated(tmp_path: Path) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    args = MODULE.parse_args(
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
        ]
    )
    plan = MODULE.build_command_plan(args)
    first_kind = MODULE.GATE_BY_NAME["gateway_load"].required_kinds[0]
    rendered = MODULE.plan_json(plan, args)
    rendered["summary_contract"] = {
        "gateway_load": {
            "schema": "wrong.schema.v1",
            "required_kinds": [first_kind, first_kind, "gateway\nload"],
            "raw_payload": "not allowed",
            "bad\nfield": "not allowed",
        },
        "reputation": {
            "schema": MODULE.GATE_BY_NAME["reputation"].schema,
            "required_kinds": list(MODULE.GATE_BY_NAME["reputation"].required_kinds),
        },
        "unknown_gate": {"schema": "sorafs.unknown.v1", "required_kinds": []},
        "gateway\nload": {
            "schema": MODULE.GATE_BY_NAME["gateway_load"].schema,
            "required_kinds": [first_kind],
        },
        "repair": "contract-shaped-entry",
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan summary_contract keys must be canonical gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract keys must use known gate names"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must contain only required gates"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must map each gate to a contract object"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract gate fields must be schema and required_kinds"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract gate fields must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract schemas must match gate schema"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must be non-empty lists"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract required_kinds must match gate contract"
        in diagnostics
    )
    assert (
        "production readiness runner plan summary_contract must match required gates"
        in diagnostics
    )
    assert "unknown_gate" not in diagnostics
    assert "gateway\nload" not in diagnostics
    assert "reputation" not in diagnostics
    assert "raw_payload" not in diagnostics
    assert "bad\nfield" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["summary_contract"] = ["gateway_load"]

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan summary_contract must be an object" in errors
    assert (
        "production readiness runner plan summary_contract must match required gates"
        in errors
    )


def test_plan_json_steps_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["steps"] = [
        {
            "label": "sorafs\nproduction",
            "artifact": 7,
            "command": [sys.executable, "bad\nargument"],
            "raw_payload": "not allowed",
            "bad\nfield": "not allowed",
        },
        "step-shaped-entry",
        {"label": "empty_command", "artifact": None, "command": []},
    ]

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "production readiness runner plan step fields must be label, artifact, and command"
        in diagnostics
    )
    assert (
        "production readiness runner plan step fields must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan step labels must be canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan step artifacts must be null or canonical strings"
        in diagnostics
    )
    assert (
        "production readiness runner plan step commands must be non-empty lists"
        in diagnostics
    )
    assert (
        "production readiness runner plan step commands must contain canonical strings"
        in diagnostics
    )
    assert "production readiness runner plan steps must contain objects" in diagnostics
    assert "production readiness runner plan steps must match command plan" in diagnostics
    assert "sorafs\nproduction" not in diagnostics
    assert "bad\nargument" not in diagnostics
    assert "raw_payload" not in diagnostics
    assert "bad\nfield" not in diagnostics
    assert "step-shaped-entry" not in diagnostics

    rendered = MODULE.plan_json(plan, args)
    rendered["steps"] = "sorafs_production_readiness_gate"

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert "production readiness runner plan steps must be a non-empty list" in errors
    assert "production readiness runner plan steps must match command plan" in errors


def test_plan_json_rejects_unsafe_rendered_paths(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    unsafe_summary = write_json(tmp_path / "private_key_summary.json")
    args.gateway_load_summary = [unsafe_summary]
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    errors = MODULE.validate_plan_json(rendered, plan, args)

    assert MODULE.PLAN_RENDERED_PATH_ERROR in errors
    assert "private_key_summary" not in "\n".join(errors)


def test_plan_json_rejects_tampered_unsafe_rendered_path_positions(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    unsafe_path = str(tmp_path / "bearer%26%2395%3Btoken-summary.json")

    def rendered_with_mutation(position: str) -> dict:
        rendered = MODULE.plan_json(plan, args)
        if position == "external_summaries":
            rendered["external_summaries"]["gateway_load"] = [unsafe_path]
        elif position == "artifact":
            rendered["steps"][0]["artifact"] = unsafe_path
        elif position == "verifier":
            rendered["steps"][0]["command"][1] = unsafe_path
        elif position == "evidence":
            evidence_index = rendered["steps"][0]["command"].index("--evidence") + 1
            rendered["steps"][0]["command"][evidence_index] = unsafe_path
        elif position == "summary_out":
            summary_index = rendered["steps"][0]["command"].index("--summary-out") + 1
            rendered["steps"][0]["command"][summary_index] = unsafe_path
        else:  # pragma: no cover - fixed local matrix
            raise AssertionError(position)
        return rendered

    for position in (
        "external_summaries",
        "artifact",
        "verifier",
        "evidence",
        "summary_out",
    ):
        errors = MODULE.validate_plan_json(
            rendered_with_mutation(position),
            plan,
            args,
        )
        diagnostics = "\n".join(errors)
        assert MODULE.PLAN_RENDERED_PATH_ERROR in errors
        assert "bearer%26%2395%3Btoken-summary" not in diagnostics
        assert "bearer_token" not in diagnostics


def test_rendered_plan_path_guard_ignores_non_path_command_values(
    tmp_path: Path,
) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["steps"][0]["command"].extend(["--future-label", "private_key_label"])

    assert MODULE.rendered_plan_paths_are_safe(rendered)


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


def test_response_file_symlink_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    target = tmp_path / "private-key-args"
    target.write_text("--require-gate gateway_load\n", encoding="utf-8")
    symlink = tmp_path / "production-readiness.args"
    symlink.symlink_to(target)

    assert MODULE.main([f"@{symlink}"]) == 2

    captured = capsys.readouterr()
    assert "@ARGFILE must not be a symlink" in captured.err
    assert "private-key-args" not in captured.err
    assert "production-readiness.args" not in captured.err
    assert captured.out == ""


def test_response_file_malformed_line_fails_before_validation(
    tmp_path: Path,
    capsys,
) -> None:
    args_file = tmp_path / "production-readiness.args"
    args_file.write_text(
        "--require-gate 'private-key-placeholder\n",
        encoding="utf-8",
    )

    assert MODULE.main([f"@{args_file}"]) == 2

    captured = capsys.readouterr()
    assert "@ARGFILE line 1:" in captured.err
    assert "private-key-placeholder" not in captured.err
    assert "production-readiness.args" not in captured.err
    assert captured.out == ""


def test_narrowed_required_gate_plan(tmp_path: Path, capsys) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(CHECKER_PATH),
            "--now-unix",
            "1800800000",
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


def test_staging_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-staging-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-staging-a" not in "\n".join(errors)


def test_numbered_staging_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-staging1-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-staging1-a" not in "\n".join(errors)


def test_compact_staging_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-stagingready-a"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-production deployment markers ['staging']"
        in errors
    )
    assert "gateway-stagingready-a" not in "\n".join(errors)


def test_joined_nonproduction_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-testproduction-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['test']"
        in errors
    )
    assert "gateway-testproduction-202606" not in "\n".join(errors)


def test_prerelease_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prerelease-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['prerelease']"
        in errors
    )
    assert "gateway-prerelease-202606" not in "\n".join(errors)


def test_tokenized_prerelease_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-rc-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['rc']"
        in errors
    )
    assert "gateway-prod-rc-202606" not in "\n".join(errors)


def test_preview_prerelease_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-productionpreview-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['preview']"
        in errors
    )
    assert "gateway-productionpreview-202606" not in "\n".join(errors)


def test_canary_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-canary-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['canary']"
        in errors
    )
    assert "gateway-prod-canary-202606" not in "\n".join(errors)


def test_stg_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-stg-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['stg']"
        in errors
    )
    assert "gateway-prod-stg-202606" not in "\n".join(errors)


def test_poc_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-poc-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['poc']"
        in errors
    )
    assert "gateway-prod-poc-202606" not in "\n".join(errors)


def test_smoke_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-production-smoke-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['smoke']"
        in errors
    )
    assert "gateway-production-smoke-202606" not in "\n".join(errors)


def test_stress_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-stress-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['stress']"
        in errors
    )
    assert "gateway-prod-stress-202606" not in "\n".join(errors)


def test_shadow_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-shadow-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['shadow']"
        in errors
    )
    assert "gateway-prod-shadow-202606" not in "\n".join(errors)


def test_cutover_deployment_id_fails(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    args.deployment_id = "gateway-prod-cutover-202606"

    errors = MODULE.validate_inputs(args)

    assert (
        "production readiness runner deployment_id must not contain "
        "non-reviewed deployment markers ['cutover']"
        in errors
    )
    assert "gateway-prod-cutover-202606" not in "\n".join(errors)


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


def test_encoded_summary_input_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    unsafe_summary = write_json(tmp_path / "gateway_private&#95;key_summary.json")

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
    assert "gateway_private&#95;key_summary" not in captured.err
    assert "&#95;" not in captured.err
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


def test_encoded_plan_rendered_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    encoded_output = tmp_path / "private%26%2395%3Bkey-output"

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(encoded_output),
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
    assert "private%26%2395%3Bkey-output" not in captured.err
    assert "private_key" not in captured.err


def test_encoded_plan_rendered_summary_output_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    encoded_summary = tmp_path / "bearer%26%2395%3Btoken-summary.json"

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--summary-out",
            str(encoded_summary),
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
    assert "bearer%26%2395%3Btoken-summary" not in captured.err
    assert "bearer_token" not in captured.err


def test_encoded_plan_rendered_verifier_path_components_must_be_plan_safe(
    tmp_path: Path, capsys
) -> None:
    gateway_summary = write_json(tmp_path / "gateway-load.json")
    encoded_verifier = tmp_path / "private%26%2395%3Bkey-verifier.py"
    encoded_verifier.write_text("#!/usr/bin/env python3\n", encoding="utf-8")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "out"),
            "--verifier",
            str(encoded_verifier),
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
    assert "private%26%2395%3Bkey-verifier" not in captured.err
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
            "--now-unix",
            "1800800000",
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
