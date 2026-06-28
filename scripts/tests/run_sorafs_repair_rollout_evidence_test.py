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
        "iroha_config_bound"
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
    assert "missing --failure-capture-evidence" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-worker.json"
    evidence_index = args.index("--worker-lifecycle-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--worker-lifecycle-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "auditor-roster.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
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
    assert "unknown required evidence kind `unknown`" in captured.err
    assert captured.out == ""
