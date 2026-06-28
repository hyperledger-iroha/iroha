"""Tests for scripts/run_sorafs_hedging_rollout_evidence.py."""

from __future__ import annotations

import importlib.util
import json
import sys
from pathlib import Path


MODULE_PATH = Path(__file__).resolve().parents[1] / "run_sorafs_hedging_rollout_evidence.py"
SPEC = importlib.util.spec_from_file_location(
    "run_sorafs_hedging_rollout_evidence",
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
        "1800005000",
        "--max-feed-lag-secs",
        "300",
        "--max-cycle-age-secs",
        "86400",
        "--max-divergence-bps",
        "250",
        "--min-billing-cycles",
        "2",
        "--feed-collector-evidence",
        str(write_payload(payload_dir / "feed-collector.json")),
        "--reference-price-evidence",
        str(write_payload(payload_dir / "reference-price.json")),
        "--billing-cycle-evidence",
        str(write_payload(payload_dir / "billing-cycle-a.json")),
        "--billing-cycle-evidence",
        str(write_payload(payload_dir / "billing-cycle-b.json")),
        "--statement-publication-evidence",
        str(write_payload(payload_dir / "statement-publication.json")),
        "--reconciliation-evidence",
        str(write_payload(payload_dir / "reconciliation.json")),
        "--metrics-alerts-evidence",
        str(write_payload(payload_dir / "metrics-alerts.json")),
        "--native-bridge-release-evidence",
        str(write_payload(payload_dir / "native-bridge-release.json")),
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


def test_dry_run_prints_complete_hedging_rollout_plan(tmp_path: Path, capsys) -> None:
    exit_code = MODULE.main([*complete_args(tmp_path), "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.hedging_billing.rollout_evidence_collection_plan.v1"
    assert plan["verifier_summary_schema"] == "sorafs.hedging_billing.rollout_evidence_gate.v1"
    assert plan["required_kinds"] == list(MODULE.DEFAULT_REQUIRED_KINDS)
    assert plan["thresholds"] == {
        "max_cycle_age_secs": 86400,
        "max_divergence_bps": 250,
        "max_feed_lag_secs": 300,
        "min_billing_cycles": 2,
        "now_unix": 1800005000,
    }
    assert plan["external_evidence"]["billing_cycle"] == [
        str(tmp_path / "payloads" / "billing-cycle-a.json"),
        str(tmp_path / "payloads" / "billing-cycle-b.json"),
    ]
    assert plan["evidence_contract"]["billing_cycle"]["schema"] == (
        "sorafs.billing.cycle_canary.v1"
    )
    assert (
        "statement_digests_hex"
        in plan["evidence_contract"]["billing_cycle"]["required_payload_fields"]
    )
    assert (
        "reconciled_line_item_count"
        in plan["evidence_contract"]["reconciliation"]["required_payload_fields"]
    )
    assert (
        "sdk_wrappers_verified"
        in plan["evidence_contract"]["native_bridge_release"]["required_payload_fields"]
    )
    assert (
        "iroha_config_bound"
        in plan["evidence_contract"]["governance_approval"]["required_payload_fields"]
    )
    assert [step["label"] for step in plan["steps"]] == ["rollout_evidence_gate"]
    verifier = plan["steps"][0]["command"]
    assert "check_sorafs_hedging_rollout_evidence.py" in verifier[1]
    assert "--evidence" in verifier
    assert str(tmp_path / "payloads" / "feed-collector.json") in verifier
    assert verifier.count("--billing-cycle-evidence") == 0
    assert verifier.count("--require-kind") == len(MODULE.DEFAULT_REQUIRED_KINDS)
    assert "--max-feed-lag-secs" in verifier
    assert "--now-unix" in verifier


def test_response_file_dry_run_prints_complete_hedging_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_args_file(tmp_path / "hedging-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"
    assert plan["external_evidence"]["feed_collector"]
    assert "feed_collector" in plan["evidence_contract"]


def test_split_response_file_dry_run_prints_complete_hedging_plan(
    tmp_path: Path, capsys
) -> None:
    args_file = write_split_args_file(tmp_path / "split-hedging-rollout.args", complete_args(tmp_path))

    exit_code = MODULE.main([f"@{args_file}", "--dry-run"])

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["schema"] == "sorafs.hedging_billing.rollout_evidence_collection_plan.v1"
    assert plan["steps"][0]["label"] == "rollout_evidence_gate"


def test_missing_required_kind_evidence_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    evidence_index = args.index("--reference-price-evidence")
    del args[evidence_index : evidence_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "missing --reference-price-evidence" in captured.err
    assert captured.out == ""


def test_missing_payload_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    missing = tmp_path / "missing-feed.json"
    evidence_index = args.index("--feed-collector-evidence") + 1
    args[evidence_index] = str(missing)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--feed-collector-evidence" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_missing_verifier_fails_before_plan(tmp_path: Path, capsys) -> None:
    missing = tmp_path / "missing-verifier.py"

    assert (
        MODULE.main(
            [
                *complete_args(tmp_path),
                "--verifier",
                str(missing),
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--verifier" in captured.err
    assert str(missing) in captured.err
    assert captured.out == ""


def test_out_dir_file_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    out_dir = tmp_path / "not-a-dir"
    out_dir.write_text("not a directory", encoding="utf-8")
    args[args.index("--out-dir") + 1] = str(out_dir)

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--out-dir" in captured.err
    assert "must be a directory" in captured.err
    assert captured.out == ""


def test_summary_out_directory_fails_before_plan(tmp_path: Path, capsys) -> None:
    summary_dir = tmp_path / "summary-dir"
    summary_dir.mkdir()

    assert (
        MODULE.main(
            [
                *complete_args(tmp_path),
                "--summary-out",
                str(summary_dir),
                "--dry-run",
            ]
        )
        == 2
    )

    captured = capsys.readouterr()
    assert "--summary-out" in captured.err
    assert "must not be a directory" in captured.err
    assert captured.out == ""


def test_billing_cycle_minimum_fails_before_plan(tmp_path: Path, capsys) -> None:
    args = complete_args(tmp_path)
    cycle_index = args.index("--billing-cycle-evidence")
    del args[cycle_index : cycle_index + 2]

    assert MODULE.main([*args, "--dry-run"]) == 2

    captured = capsys.readouterr()
    assert "--billing-cycle-evidence count" in captured.err
    assert captured.out == ""


def test_subset_gate_requires_only_selected_kind(tmp_path: Path, capsys) -> None:
    payload = write_payload(tmp_path / "feed-collector.json")

    exit_code = MODULE.main(
        [
            "--out-dir",
            str(tmp_path / "evidence"),
            "--require-kind",
            "feed_collector",
            "--feed-collector-evidence",
            str(payload),
            "--dry-run",
        ]
    )

    assert exit_code == 0
    plan = json.loads(capsys.readouterr().out)
    assert plan["required_kinds"] == ["feed_collector"]
    assert list(plan["evidence_contract"]) == ["feed_collector"]
    verifier = plan["steps"][0]["command"]
    assert verifier.count("--require-kind") == 1
    assert "feed_collector" in verifier


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
