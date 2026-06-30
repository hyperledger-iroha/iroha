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
            "--dry-run",
        ]
    )

    assert exit_code == 0
    payload = json.loads(capsys.readouterr().out)
    assert payload["required_gates"] == ["gateway_load"]
    assert payload["external_summaries"] == {
        "gateway_load": [str(gateway_summary)]
    }
