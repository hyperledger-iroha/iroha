"""Tests for scripts/run_sorafs_por_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_por_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location("run_sorafs_por_rollout_evidence", MODULE_PATH)
MODULE = importlib.util.module_from_spec(SPEC)
assert SPEC and SPEC.loader  # pragma: no cover - defensive
sys.modules[SPEC.name] = MODULE
SPEC.loader.exec_module(MODULE)


def write_payload(path: Path) -> Path:
    path.write_text("{}", encoding="utf-8")
    return path


def complete_args(tmp_path: Path) -> list[str]:
    payload_dir = tmp_path / "payloads"
    payload_dir.mkdir()
    return [
        "--out-dir",
        str(tmp_path / "evidence"),
        "--now-unix",
        "1800500000",
        "--max-evidence-age-secs",
        "86400",
        "--max-route-latency-ms",
        "1200",
        "--max-scheduler-lag-secs",
        "600",
        "--max-report-latency-ms",
        "2500",
        "--min-providers",
        "3",
        "--min-challenges",
        "3",
        "--randomness-evidence",
        str(write_payload(payload_dir / "randomness.json")),
        "--scheduler-runtime-evidence",
        str(write_payload(payload_dir / "scheduler-runtime.json")),
        "--validator-replay-evidence",
        str(write_payload(payload_dir / "validator-replay.json")),
        "--reporting-archive-evidence",
        str(write_payload(payload_dir / "reporting-archive.json")),
        "--observability-evidence",
        str(write_payload(payload_dir / "observability.json")),
        "--governance-approval-evidence",
        str(write_payload(payload_dir / "governance-approval.json")),
    ]


def write_args_file(path: Path, args: list[str]) -> Path:
    lines = [
        "# comments and blank lines are ignored",
        "",
    ]
    for index in range(0, len(args), 2):
        option = args[index]
        value = args[index + 1]
        lines.append(f"{option} {json.dumps(value)}")
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def write_split_args_file(path: Path, args: list[str]) -> Path:
    path.write_text(
        "\n".join(["# one token per line also works for long reviewed inputs", *args]),
        encoding="utf-8",
    )
    return path


def test_dry_run_prints_complete_por_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.por.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.por.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 86400,
        "max_report_latency_ms": 2500,
        "max_route_latency_ms": 1200,
        "max_scheduler_lag_secs": 600,
        "min_challenges": 3,
        "min_providers": 3,
        "now_unix": 1800500000,
    }
    assert plan["external_evidence"]["randomness"] == [
        str(tmp_path / "payloads" / "randomness.json")
    ]
    assert plan["evidence_contract"]["randomness"]["schema"] == (
        "sorafs.por.randomness_canary.v1"
    )
    assert (
        "seed_replay_digest_hex"
        in plan["evidence_contract"]["randomness"]["required_payload_fields"]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["randomness"]["required_payload_fields"]
    )
    assert (
        "providers"
        in plan["evidence_contract"]["randomness"]["required_payload_fields"]
    )
    assert (
        "scheduler_runtime_enabled"
        in plan["evidence_contract"]["scheduler_runtime"]["required_payload_fields"]
    )
    assert (
        "validation_bundle_digest_hex"
        in plan["evidence_contract"]["validator_replay"]["required_payload_fields"]
    )
    assert "manual_trigger_route_state" not in plan["evidence_contract"][
        "reporting_archive"
    ]["required_payload_fields"]
    assert (
        "archive_backend"
        in plan["evidence_contract"]["reporting_archive"]["required_payload_fields"]
    )
    assert (
        "forced_challenge_alert_tested"
        in plan["evidence_contract"]["observability"]["required_payload_fields"]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_por_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "scheduler-runtime.json") in verifier
    assert verifier.count("--randomness-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-scheduler-lag-secs" in verifier
    assert "--now-unix" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "PoR rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.por.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "PoR rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "PoR rollout runner plan schema must match the contract" in diagnostics
    assert "PoR rollout runner plan required_kinds must match args" in diagnostics
    assert "PoR rollout runner plan thresholds must match args" in diagnostics
    assert "PoR rollout runner plan external_evidence must match args" in diagnostics
    assert (
        "PoR rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "scheduler-runtime.json" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\npor"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "randomness",
        "randomness",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_evidence_age_secs": -1,
        "max_route_latency_ms": 0,
        "max_scheduler_lag_secs": False,
        "max_report_latency_ms": "soon",
        "min_providers": 0,
        "min_challenges": 0,
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "randomness": [],
        "unknown_kind": ["unknown.json"],
        "scheduler_runtime": "scheduler-runtime.json",
        "bad\nkind": ["randomness.json"],
        "observability": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "randomness": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.por.unknown.v1",
            "required_payload_fields": [],
        },
        "scheduler_runtime": "contract-shaped-entry",
        "bad\nkind": {
            "schema": MODULE.KIND_BY_NAME["randomness"].schema,
            "required_payload_fields": ["schema"],
        },
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "PoR rollout runner plan fields must be canonical strings" in diagnostics
    assert "PoR rollout runner plan schema must be canonical" in diagnostics
    assert "PoR rollout runner plan verifier schema must be canonical" in diagnostics
    assert (
        "PoR rollout runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds.max_evidence_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds.max_route_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds.max_scheduler_lag_secs must be a positive integer"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds.max_report_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds.min_providers must be a positive integer"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds.min_challenges must be a positive integer"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract required_payload_fields must match checker fields"
        in diagnostics
    )
    assert "unknown_kind" not in diagnostics
    assert "bad\nkind" not in diagnostics
    assert "bad\nfield" not in diagnostics
    assert "runtime-only-key-material" not in diagnostics
    assert "private_key" not in diagnostics
    assert "wrong.schema.v1" not in diagnostics


def test_plan_json_rejects_unrequired_external_evidence_and_contracts(
    tmp_path: Path,
) -> None:
    payload = write_payload(tmp_path / "randomness.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800500000",
            "--require-kind",
            "randomness",
            "--randomness-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["scheduler_runtime"] = [
        str(tmp_path / "scheduler-runtime.json")
    ]
    rendered["evidence_contract"]["scheduler_runtime"] = {
        "schema": MODULE.KIND_BY_NAME["scheduler_runtime"].schema,
        "required_payload_fields": list(
            MODULE.EVIDENCE_REQUIRED_FIELDS["scheduler_runtime"]
        ),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "PoR rollout runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "PoR rollout runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "scheduler_runtime" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["PoR rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "PoR rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_por_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "por-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["randomness"]
    assert "scheduler_runtime" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_por_plan(tmp_path: Path, capsys) -> None:
    args_file = write_split_args_file(tmp_path / "split-por-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.por.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert "observability" in plan["evidence_contract"]


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--randomness-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-validator.json"
    evidence_index = args.index("--validator-replay-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--validator-replay-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "randomness.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800500000",
            "--require-kind",
            "randomness",
            "--randomness-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["randomness"]
    assert list(plan["evidence_contract"]) == ["randomness"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "randomness" in verifier


def test_unknown_required_kind_fails_before_plan(tmp_path: Path, capsys) -> None:
    assert (
        MODULE.main(
            [
                "--out-dir",
                str(tmp_path / "evidence"),
                "--require-kind",
                "unknown",
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "unknown required evidence kind" in captured.err
    assert captured.out == ""
