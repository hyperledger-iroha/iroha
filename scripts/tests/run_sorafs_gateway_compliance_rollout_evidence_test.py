"""Tests for scripts/run_sorafs_gateway_compliance_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_gateway_compliance_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_gateway_compliance_rollout_evidence",
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
        "1800009000",
        "--max-evidence-age-secs",
        "86400",
        "--max-route-latency-ms",
        "1500",
        "--max-reload-latency-ms",
        "300000",
        "--min-gateways",
        "3",
        "--min-denylist-entries",
        "5",
        "--min-honey-probes",
        "4",
        "--feed-promotion-evidence",
        str(write_payload(payload_dir / "feed-promotion.json")),
        "--gateway-reload-evidence",
        str(write_payload(payload_dir / "gateway-reload.json")),
        "--enforcement-probe-evidence",
        str(write_payload(payload_dir / "enforcement-probe.json")),
        "--honey-audit-evidence",
        str(write_payload(payload_dir / "honey-audit.json")),
        "--appeal-override-evidence",
        str(write_payload(payload_dir / "appeal-override.json")),
        "--transparency-publication-evidence",
        str(write_payload(payload_dir / "transparency-publication.json")),
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
    path.write_text("\n".join(["# split response file", *args]), encoding="utf-8")
    return path


def test_dry_run_prints_complete_gateway_compliance_rollout_plan(
    tmp_path: Path, capsys
) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.gateway_compliance.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.gateway_compliance.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_evidence_age_secs": 86400,
        "max_reload_latency_ms": 300000,
        "max_route_latency_ms": 1500,
        "min_denylist_entries": 5,
        "min_gateways": 3,
        "min_honey_probes": 4,
        "now_unix": 1800009000,
    }
    assert plan["external_evidence"]["enforcement_probe"] == [
        str(tmp_path / "payloads" / "enforcement-probe.json")
    ]
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_gateway_compliance_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "gateway-reload.json") in verifier
    assert verifier.count("--gateway-reload-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--min-denylist-entries" in verifier
    assert "--now-unix" in verifier


def test_response_file_dry_run_prints_complete_plan(tmp_path: Path, capsys) -> None:
    args_file = write_args_file(
        tmp_path / "gateway-compliance-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["transparency_publication"]


def test_non_dry_run_executes_without_printing_collection_plan(
    tmp_path: Path, capsys, monkeypatch
) -> None:
    calls = []

    def fake_run_plan(plan, out_dir):
        calls.append((plan, out_dir))
        return 0

    monkeypatch.setattr(MODULE, "run_plan", fake_run_plan)

    exit_code = MODULE.main(complete_args(tmp_path))

    captured = capsys.readouterr()
    assert exit_code == 0
    assert captured.out == ""
    assert captured.err == ""
    assert len(calls) == 1
    assert calls[0][0][0].label == "rollout_evidence_gate"
    assert calls[0][1] == tmp_path / "evidence"


def test_split_response_file_dry_run_prints_complete_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(
        tmp_path / "split-gateway-compliance-rollout.args",
        complete_args(tmp_path),
    )

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.gateway_compliance.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--enforcement-probe-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --enforcement-probe-evidence" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-honey-audit.json"
    evidence_index = args.index("--honey-audit-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--honey-audit-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "feed-promotion.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "feed_promotion",
            "--feed-promotion-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["feed_promotion"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "feed_promotion" in verifier


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
    assert "usage:" not in captured.err
    assert captured.out == ""
