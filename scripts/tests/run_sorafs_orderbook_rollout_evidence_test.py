"""Tests for scripts/run_sorafs_orderbook_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_orderbook_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_orderbook_rollout_evidence",
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
        "1800200000",
        "--max-evidence-age-secs",
        "86400",
        "--max-route-latency-ms",
        "1200",
        "--max-stream-lag-ms",
        "1500",
        "--max-matcher-lag-ms",
        "750",
        "--min-reconciliation-peers",
        "4",
        "--contract-surface-evidence",
        str(write_payload(payload_dir / "contract-surface.json")),
        "--matcher-service-evidence",
        str(write_payload(payload_dir / "matcher-service.json")),
        "--settlement-service-evidence",
        str(write_payload(payload_dir / "settlement-service.json")),
        "--api-gateway-evidence",
        str(write_payload(payload_dir / "api-gateway.json")),
        "--event-streams-evidence",
        str(write_payload(payload_dir / "event-streams.json")),
        "--sdk-release-evidence",
        str(write_payload(payload_dir / "sdk-release.json")),
        "--observability-evidence",
        str(write_payload(payload_dir / "observability.json")),
        "--reconciliation-evidence",
        str(write_payload(payload_dir / "reconciliation.json")),
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


def test_dry_run_prints_complete_orderbook_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.orderbook.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.orderbook.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 86400,
        "max_matcher_lag_ms": 750,
        "max_route_latency_ms": 1200,
        "max_stream_lag_ms": 1500,
        "min_reconciliation_peers": 4,
        "now_unix": 1800200000,
    }
    assert plan["external_evidence"]["contract_surface"] == [
        str(tmp_path / "payloads" / "contract-surface.json")
    ]
    assert plan["evidence_contract"]["contract_surface"]["schema"] == (
        "sorafs.orderbook.contract_surface_canary.v1"
    )
    assert (
        "contract_digest_hex"
        in plan["evidence_contract"]["contract_surface"]["required_payload_fields"]
    )
    assert (
        "routes"
        in plan["evidence_contract"]["api_gateway"]["required_payload_fields"]
    )
    assert (
        "streams"
        in plan["evidence_contract"]["event_streams"]["required_payload_fields"]
    )
    assert (
        "languages"
        in plan["evidence_contract"]["sdk_release"]["required_payload_fields"]
    )
    assert (
        "contract_mirror_reconciliation_passed"
        in plan["evidence_contract"]["reconciliation"]["required_payload_fields"]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"]["required_payload_fields"]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_orderbook_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "api-gateway.json") in verifier
    assert verifier.count("--contract-surface-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-route-latency-ms" in verifier
    assert "--now-unix" in verifier


def test_response_file_dry_run_prints_complete_orderbook_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "orderbook-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["matcher_service"]
    assert "matcher_service" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_orderbook_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-orderbook-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.orderbook.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--matcher-service-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --matcher-service-evidence" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-api.json"
    evidence_index = args.index("--api-gateway-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--api-gateway-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "contract-surface.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "contract_surface",
            "--contract-surface-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["contract_surface"]
    assert list(plan["evidence_contract"]) == ["contract_surface"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "contract_surface" in verifier


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
