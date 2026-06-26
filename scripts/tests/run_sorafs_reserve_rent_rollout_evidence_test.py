"""Tests for scripts/run_sorafs_reserve_rent_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_reserve_rent_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_reserve_rent_rollout_evidence",
    MODULE_PATH,
)
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
        "1800007000",
        "--max-ledger-age-secs",
        "604800",
        "--max-lifecycle-lag-secs",
        "900",
        "--max-route-latency-ms",
        "1500",
        "--max-bake-age-secs",
        "1209600",
        "--policy-config-evidence",
        str(write_payload(payload_dir / "policy-config.json")),
        "--quote-matrix-evidence",
        str(write_payload(payload_dir / "quote-matrix.json")),
        "--ledger-digest-evidence",
        str(write_payload(payload_dir / "ledger-digest.json")),
        "--lifecycle-service-evidence",
        str(write_payload(payload_dir / "lifecycle-service.json")),
        "--signed-routes-evidence",
        str(write_payload(payload_dir / "signed-routes.json")),
        "--reserve-movement-evidence",
        str(write_payload(payload_dir / "reserve-movement.json")),
        "--credit-line-evidence",
        str(write_payload(payload_dir / "credit-line.json")),
        "--appeal-policy-evidence",
        str(write_payload(payload_dir / "appeal-policy.json")),
        "--metrics-alerts-evidence",
        str(write_payload(payload_dir / "metrics-alerts.json")),
        "--provider-bake-evidence",
        str(write_payload(payload_dir / "provider-bake.json")),
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
    lines = [
        "# one token per line also works for long reviewed inputs",
        *args,
    ]
    path.write_text("\n".join(lines), encoding="utf-8")
    return path


def test_dry_run_prints_complete_reserve_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reserve_rent.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.reserve_rent.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_bake_age_secs": 1209600,
        "max_ledger_age_secs": 604800,
        "max_lifecycle_lag_secs": 900,
        "max_route_latency_ms": 1500,
        "now_unix": 1800007000,
    }
    assert plan["external_evidence"]["signed_routes"] == [
        str(tmp_path / "payloads" / "signed-routes.json")
    ]
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_reserve_rent_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "ledger-digest.json") in verifier
    assert verifier.count("--signed-routes-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-route-latency-ms" in verifier
    assert "--now-unix" in verifier


def test_response_file_dry_run_prints_complete_reserve_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "reserve-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["provider_bake"]


def test_split_response_file_dry_run_prints_complete_reserve_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-reserve-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.reserve_rent.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--signed-routes-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --signed-routes-evidence" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-ledger.json"
    evidence_index = args.index("--ledger-digest-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--ledger-digest-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "policy-config.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "policy_config",
            "--policy-config-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["policy_config"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "policy_config" in verifier


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
