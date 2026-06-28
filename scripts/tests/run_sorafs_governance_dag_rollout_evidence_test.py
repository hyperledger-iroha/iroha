"""Tests for scripts/run_sorafs_governance_dag_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = (
    Path(__file__).resolve().parents[1] / "run_sorafs_governance_dag_rollout_evidence.py"
)
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_governance_dag_rollout_evidence",
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
        "1800300000",
        "--max-evidence-age-secs",
        "86400",
        "--max-route-latency-ms",
        "1200",
        "--max-pin-lag-secs",
        "1200",
        "--max-head-age-secs",
        "1500",
        "--min-blocks",
        "4",
        "--min-payload-kinds",
        "6",
        "--ingest-service-evidence",
        str(write_payload(payload_dir / "ingest-service.json")),
        "--publisher-service-evidence",
        str(write_payload(payload_dir / "publisher-service.json")),
        "--mirror-datastore-evidence",
        str(write_payload(payload_dir / "mirror-datastore.json")),
        "--operator-recovery-evidence",
        str(write_payload(payload_dir / "operator-recovery.json")),
        "--dashboard-api-evidence",
        str(write_payload(payload_dir / "dashboard-api.json")),
        "--observability-evidence",
        str(write_payload(payload_dir / "observability.json")),
        "--ipfs-ipns-e2e-evidence",
        str(write_payload(payload_dir / "ipfs-ipns-e2e.json")),
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


def test_dry_run_prints_complete_governance_dag_rollout_plan(
    tmp_path: Path, capsys
) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.governance_dag.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.governance_dag.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 86400,
        "max_head_age_secs": 1500,
        "max_pin_lag_secs": 1200,
        "max_route_latency_ms": 1200,
        "min_blocks": 4,
        "min_payload_kinds": 6,
        "now_unix": 1800300000,
    }
    assert plan["external_evidence"]["publisher_service"] == [
        str(tmp_path / "payloads" / "publisher-service.json")
    ]
    assert plan["evidence_contract"]["publisher_service"]["schema"] == (
        "sorafs.governance_dag.publisher_service_canary.v1"
    )
    assert (
        "ipfs_cluster_pinning_enabled"
        in plan["evidence_contract"]["publisher_service"]["required_payload_fields"]
    )
    assert (
        "rocksdb_ipld_enabled"
        in plan["evidence_contract"]["mirror_datastore"]["required_payload_fields"]
    )
    assert (
        "public_head_resolved"
        in plan["evidence_contract"]["ipfs_ipns_e2e"]["required_payload_fields"]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"]["required_payload_fields"]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_governance_dag_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "dashboard-api.json") in verifier
    assert verifier.count("--publisher-service-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-pin-lag-secs" in verifier
    assert "--now-unix" in verifier


def test_response_file_dry_run_prints_complete_governance_dag_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "governance-dag-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["ingest_service"]
    assert "ingest_service" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_governance_dag_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-governance-dag-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.governance_dag.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--publisher-service-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --publisher-service-evidence" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-dashboard.json"
    evidence_index = args.index("--dashboard-api-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--dashboard-api-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "ingest-service.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "ingest_service",
            "--ingest-service-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["ingest_service"]
    assert list(plan["evidence_contract"]) == ["ingest_service"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "ingest_service" in verifier


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
