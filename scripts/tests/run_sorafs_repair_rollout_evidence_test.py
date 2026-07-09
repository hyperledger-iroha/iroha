"""Tests for scripts/run_sorafs_repair_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_repair_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location("run_sorafs_repair_rollout_evidence", MODULE_PATH)
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
        "1800400000",
        "--max-evidence-age-secs",
        "86400",
        "--max-route-latency-ms",
        "1200",
        "--max-event-lag-secs",
        "600",
        "--max-repair-latency-secs",
        "3600",
        "--min-auditors",
        "3",
        "--auditor-roster-evidence",
        str(write_payload(payload_dir / "auditor-roster.json")),
        "--failure-capture-evidence",
        str(write_payload(payload_dir / "failure-capture.json")),
        "--auditor-api-evidence",
        str(write_payload(payload_dir / "auditor-api.json")),
        "--worker-lifecycle-evidence",
        str(write_payload(payload_dir / "worker-lifecycle.json")),
        "--event-streams-evidence",
        str(write_payload(payload_dir / "event-streams.json")),
        "--governance-handoff-evidence",
        str(write_payload(payload_dir / "governance-handoff.json")),
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


def test_dry_run_prints_complete_repair_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.repair.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.repair.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_event_lag_secs": 600,
        "max_evidence_age_secs": 86400,
        "max_repair_latency_secs": 3600,
        "max_route_latency_ms": 1200,
        "min_auditors": 3,
        "now_unix": 1800400000,
    }
    assert plan["external_evidence"]["failure_capture"] == [
        str(tmp_path / "payloads" / "failure-capture.json")
    ]
    assert plan["evidence_contract"]["auditor_roster"]["schema"] == (
        "sorafs.repair.auditor_roster_canary.v1"
    )
    assert (
        "roster_digest_hex"
        in plan["evidence_contract"]["auditor_roster"]["required_payload_fields"]
    )
    assert (
        "auditors"
        in plan["evidence_contract"]["auditor_roster"]["required_payload_fields"]
    )
    assert (
        "evidence_bundle_digest_hex"
        in plan["evidence_contract"]["failure_capture"]["required_payload_fields"]
    )
    assert (
        "statuses_observed"
        in plan["evidence_contract"]["worker_lifecycle"]["required_payload_fields"]
    )
    assert (
        "sse_delivery_verified"
        in plan["evidence_contract"]["event_streams"]["required_payload_fields"]
    )
    assert (
        "handoff_targets"
        in plan["evidence_contract"]["governance_handoff"]["required_payload_fields"]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["governance_handoff"]["required_payload_fields"]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert (
        "policy_digest_hex"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert (
        "handoff_digest_hex"
        in plan["evidence_contract"]["governance_approval"][
            "required_payload_fields"
        ]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_repair_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "auditor-api.json") in verifier
    assert verifier.count("--failure-capture-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-event-lag-secs" in verifier
    assert "--now-unix" in verifier


def test_plan_json_shape_is_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)

    assert MODULE.validate_plan_json(rendered, plan, args) == []

    assert MODULE.validate_plan_json(["step"], plan, args) == [
        "repair rollout runner plan must be an object"
    ]

    rendered["schema"] = "sorafs.repair.rollout_evidence_collection_plan.v0"
    rendered["unexpected"] = True
    rendered["required_kinds"] = []
    rendered["thresholds"] = {}
    rendered["external_evidence"] = {}
    rendered["evidence_contract"] = {}
    rendered["steps"] = []

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "repair rollout runner plan fields must match the schema-closed contract"
        in diagnostics
    )
    assert "repair rollout runner plan schema must match the contract" in diagnostics
    assert "repair rollout runner plan required_kinds must match args" in diagnostics
    assert "repair rollout runner plan thresholds must match args" in diagnostics
    assert "repair rollout runner plan external_evidence must match args" in diagnostics
    assert (
        "repair rollout runner plan evidence_contract must match checker fields"
        in diagnostics
    )
    assert "runner plan steps must match command plan" in diagnostics
    assert "worker-lifecycle.json" not in diagnostics


def test_plan_json_nested_shapes_are_validated(tmp_path: Path) -> None:
    args = MODULE.parse_args(complete_args(tmp_path))
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["bad\nfield"] = "runtime-only-key-material"
    rendered["schema"] = "sorafs\nrepair"
    rendered["verifier_summary_schema"] = "summary\nschema"
    rendered["required_kinds"] = [
        "auditor_roster",
        "auditor_roster",
        "unknown_kind",
        "bad\nkind",
    ]
    rendered["thresholds"] = {
        "max_evidence_age_secs": -1,
        "max_route_latency_ms": 0,
        "max_event_lag_secs": False,
        "max_repair_latency_secs": "soon",
        "min_auditors": 0,
        "now_unix": 0,
        "bad\nfield": 1,
        "private_key": 2,
    }
    rendered["external_evidence"] = {
        "auditor_roster": [],
        "unknown_kind": ["unknown.json"],
        "failure_capture": "failure-capture.json",
        "bad\nkind": ["auditor-roster.json"],
        "observability": ["bad\npath"],
    }
    rendered["evidence_contract"] = {
        "auditor_roster": {
            "schema": "wrong.schema.v1",
            "required_payload_fields": ["schema", "schema", "bad\nfield"],
            "raw_payload": True,
            "bad\nfield": "runtime-only-key-material",
        },
        "unknown_kind": {
            "schema": "sorafs.repair.unknown.v1",
            "required_payload_fields": [],
        },
        "failure_capture": "contract-shaped-entry",
        "bad\nkind": {
            "schema": MODULE.KIND_BY_NAME["auditor_roster"].schema,
            "required_payload_fields": ["schema"],
        },
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert "repair rollout runner plan fields must be canonical strings" in diagnostics
    assert "repair rollout runner plan schema must be canonical" in diagnostics
    assert "repair rollout runner plan verifier schema must be canonical" in diagnostics
    assert (
        "repair rollout runner plan required_kinds must contain canonical strings"
        in diagnostics
    )
    assert (
        "repair rollout runner plan required_kinds must not contain duplicate kinds"
        in diagnostics
    )
    assert (
        "repair rollout runner plan required_kinds must use known kind names"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds keys must be canonical strings"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds must contain only configured threshold fields"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds.max_evidence_age_secs must be a non-negative integer"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds.max_route_latency_ms must be a positive integer"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds.max_event_lag_secs must be a positive integer"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds.max_repair_latency_secs must be a positive integer"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds.min_auditors must be a positive integer"
        in diagnostics
    )
    assert (
        "repair rollout runner plan thresholds.now_unix must be a positive integer"
        in diagnostics
    )
    assert (
        "repair rollout runner plan external_evidence keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "repair rollout runner plan external_evidence keys must use known kind names"
        in diagnostics
    )
    assert (
        "repair rollout runner plan external_evidence must map each kind to non-empty path lists"
        in diagnostics
    )
    assert (
        "repair rollout runner plan external_evidence paths must be canonical strings"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract keys must be canonical kind names"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract keys must use known kind names"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract must map each kind to a contract object"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract fields must be canonical strings"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract fields must be schema and required_payload_fields"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract schemas must match evidence kind"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract required_payload_fields must be non-empty lists"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract required_payload_fields must contain canonical strings"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract required_payload_fields must not contain duplicate fields"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract required_payload_fields must match checker fields"
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
    payload = write_payload(tmp_path / "auditor-roster.json")
    args = MODULE.parse_args(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800400000",
            "--require-kind",
            "auditor_roster",
            "--auditor-roster-evidence",
            str(payload),
        ]
    )
    plan = MODULE.build_command_plan(args)
    rendered = MODULE.plan_json(plan, args)
    rendered["external_evidence"]["failure_capture"] = [
        str(tmp_path / "failure-capture.json")
    ]
    rendered["evidence_contract"]["failure_capture"] = {
        "schema": MODULE.KIND_BY_NAME["failure_capture"].schema,
        "required_payload_fields": list(
            MODULE.EVIDENCE_REQUIRED_FIELDS["failure_capture"]
        ),
    }

    errors = MODULE.validate_plan_json(rendered, plan, args)
    diagnostics = "\n".join(errors)

    assert (
        "repair rollout runner plan external_evidence must contain only required kinds"
        in diagnostics
    )
    assert (
        "repair rollout runner plan evidence_contract must contain only required kinds"
        in diagnostics
    )
    assert "failure_capture" not in diagnostics


def test_execution_rejects_plan_validation_drift_before_running(
    tmp_path: Path, monkeypatch, capsys
) -> None:
    ran_plan = False

    def fake_validate_plan_json(rendered, plan, args):
        return ["repair rollout runner plan schema must match the contract"]

    def fake_run_plan(plan, out_dir):
        nonlocal ran_plan
        ran_plan = True
        return 0

    monkeypatch.setattr(MODULE, "validate_plan_json", fake_validate_plan_json)
    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    assert MODULE.main(complete_args(tmp_path)) == 2

    assert not ran_plan
    assert (
        "repair rollout runner plan schema must match the contract"
        in capsys.readouterr().err
    )


def test_response_file_dry_run_prints_complete_repair_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(tmp_path / "repair-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["auditor_roster"]
    assert "auditor_api" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_repair_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(tmp_path / "split-repair-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.repair.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert "observability" in plan["evidence_contract"]


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--failure-capture-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing required rollout evidence input" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-worker.json"
    evidence_index = args.index("--worker-lifecycle-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "input evidence file must exist and be a file" in captured.err
    assert "--worker-lifecycle-evidence" not in captured.err
    assert str(missing) not in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "auditor-roster.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--now-unix",
            "1800400000",
            "--require-kind",
            "auditor_roster",
            "--auditor-roster-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["auditor_roster"]
    assert list(plan["evidence_contract"]) == ["auditor_roster"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "auditor_roster" in verifier


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
